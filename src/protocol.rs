use std::borrow::Cow;
use std::sync::{Arc, Mutex};

use tracing::{debug, warn};
use wry::http::Response;
use wry::http::header::{ACCESS_CONTROL_ALLOW_ORIGIN, CONTENT_TYPE};

use crate::music;
use crate::rpc::{self, RpcConfig};
use crate::rpc_limiter::RpcLimiter;
use crate::zmq::{self, ZmqHandle, ZmqState};

pub fn build_webview(
    config: Arc<Mutex<RpcConfig>>,
    rpc_limiter: Arc<RpcLimiter>,
    music_runtime: Arc<music::MusicRuntime>,
    zmq_state: Arc<Mutex<ZmqState>>,
    zmq_handle: Arc<Mutex<Option<ZmqHandle>>>,
) -> wry::WebViewBuilder<'static> {
    let cfg = Arc::clone(&config);
    wry::WebViewBuilder::new()
        .with_asynchronous_custom_protocol("app".into(), move |_id, req, responder| {
            let path = req.uri().path().to_string();
            let query = req.uri().query().unwrap_or("").to_string();
            debug!(method = %req.method(), path, query_bytes = query.len(), "protocol request");

            if path == "/rpc" {
                let body = request_body(&req, &query);
                if let Some(permit) = rpc_limiter.try_acquire() {
                    let cfg = Arc::clone(&cfg);
                    std::thread::spawn(move || {
                        let _permit = permit;
                        let result = rpc::do_rpc(&body, &cfg);
                        responder.respond(json_response(&result));
                    });
                } else {
                    warn!("rpc request rejected due to in-flight limit");
                    responder.respond(json_response(
                        r#"{"error":"rpc worker pool saturated; try again"}"#,
                    ));
                }
                return;
            }

            if path == "/config" {
                let body = request_body(&req, &query);
                let result = rpc::update_config(&body, &cfg);
                {
                    let limit = cfg.lock().unwrap().zmq_buffer_limit;
                    let mut state = zmq_state.lock().unwrap();
                    state.buffer_limit = limit;
                    while state.messages.len() > state.buffer_limit {
                        state.messages.pop_front();
                    }
                }
                if result.zmq_changed {
                    let mut handle = zmq_handle.lock().unwrap();
                    if let Some(h) = handle.take() {
                        zmq::stop_zmq_subscriber(h);
                    }
                    let addr = cfg.lock().unwrap().zmq_address.clone();
                    if !addr.is_empty() {
                        *handle = Some(zmq::start_zmq_subscriber(&addr, Arc::clone(&zmq_state)));
                    }
                }
                let resp_body = if result.insecure_blocked {
                    r#"{"ok":true,"insecure_blocked":true}"#
                } else {
                    r#"{"ok":true}"#
                };
                responder.respond(json_response(resp_body));
                return;
            }

            if path == "/allow-insecure-rpc" {
                let allowed = rpc::allow_insecure();
                responder.respond(json_response(&format!(r#"{{"allowed":{allowed}}}"#)));
                return;
            }

            if path == "/features" {
                responder.respond(json_response(&format!(
                    r#"{{"audio":{}}}"#,
                    music::is_enabled()
                )));
                return;
            }

            if path == "/zmq/decode-rawtx" {
                let timestamp = query_param_u64(&query, "timestamp");
                let sequence = query_param_u64(&query, "sequence");
                let result = if let (Some(timestamp), Some(sequence)) = (timestamp, sequence) {
                    let raw_hex = {
                        let s = zmq_state.lock().unwrap();
                        s.messages
                            .iter()
                            .rev()
                            .find(|m| {
                                m.topic == "rawtx"
                                    && m.timestamp == timestamp
                                    && m.sequence as u64 == sequence
                            })
                            .and_then(|m| m.body_full_hex.as_deref())
                            .map(str::to_owned)
                    };
                    if let Some(raw_hex) = raw_hex {
                        let body = serde_json::json!({
                            "method": "decoderawtransaction",
                            "params": [raw_hex],
                        });
                        rpc::do_rpc(&body.to_string(), &cfg)
                    } else {
                        r#"{"error":"rawtx message not found"}"#.to_string()
                    }
                } else {
                    r#"{"error":"invalid rawtx selector"}"#.to_string()
                };
                responder.respond(json_response(&result));
                return;
            }

            if path == "/zmq/messages" {
                let s = zmq_state.lock().unwrap();
                let messages: Vec<serde_json::Value> = s
                    .messages
                    .iter()
                    .map(|m| {
                        serde_json::json!({
                            "topic": m.topic,
                            "body_hex": m.body_hex,
                            "body_size": m.body_size,
                            "sequence": m.sequence,
                            "timestamp": m.timestamp,
                            "event_hash": m.event_hash,
                        })
                    })
                    .collect();
                let result = serde_json::json!({
                    "connected": s.connected,
                    "address": s.address,
                    "buffer_limit": s.buffer_limit,
                    "messages": messages,
                });
                responder.respond(json_response(&result.to_string()));
                return;
            }

            if let Some(result) =
                music::handle_music_request(&path, &percent_decode(&query), &music_runtime)
            {
                responder.respond(json_response(&result));
                return;
            }

            responder.respond(serve_asset(&path));
        })
        .with_devtools(cfg!(debug_assertions))
        .with_url("app://localhost/index.html")
}

fn json_response(body: &str) -> Response<Cow<'static, [u8]>> {
    Response::builder()
        .header(CONTENT_TYPE, "application/json")
        .header(ACCESS_CONTROL_ALLOW_ORIGIN, "*")
        .body(Cow::Owned(body.as_bytes().to_vec()))
        .unwrap()
}

fn serve_asset(path: &str) -> Response<Cow<'static, [u8]>> {
    let (mime, content): (&str, &[u8]) = match path {
        "/" | "/index.html" => ("text/html", include_bytes!("../web/index.html")),
        "/style.css" => ("text/css", include_bytes!("../web/style.css")),
        "/app.js" => ("text/javascript", include_bytes!("../web/app.js")),
        "/openrpc.json" => ("application/json", include_bytes!("../assets/openrpc.json")),
        _ => {
            return Response::builder()
                .status(404)
                .body(Cow::Borrowed(b"Not found" as &[u8]))
                .unwrap();
        }
    };
    Response::builder()
        .header(CONTENT_TYPE, mime)
        .header(ACCESS_CONTROL_ALLOW_ORIGIN, "*")
        .body(Cow::Borrowed(content))
        .unwrap()
}

fn percent_decode(input: &str) -> String {
    let mut out = Vec::new();
    let b = input.as_bytes();
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            if let Ok(byte) =
                u8::from_str_radix(std::str::from_utf8(&b[i + 1..i + 3]).unwrap_or(""), 16)
            {
                out.push(byte);
                i += 3;
                continue;
            }
        }
        out.push(if b[i] == b'+' { b' ' } else { b[i] });
        i += 1;
    }
    String::from_utf8_lossy(&out).to_string()
}

fn request_body(req: &wry::http::Request<Vec<u8>>, query: &str) -> String {
    if req.method() == wry::http::Method::POST {
        if let Some(encoded) = req.headers().get("x-app-json").and_then(|v| v.to_str().ok()) {
            let decoded = percent_decode(encoded);
            if !decoded.is_empty() {
                return decoded;
            }
        }
        let body = req.body();
        if !body.is_empty() {
            return String::from_utf8_lossy(body).to_string();
        }
    }
    percent_decode(query)
}

fn query_param_u64(query: &str, key: &str) -> Option<u64> {
    query
        .split('&')
        .find_map(|pair| {
            let mut iter = pair.splitn(2, '=');
            let k = iter.next()?;
            let v = iter.next().unwrap_or("");
            (k == key).then_some(percent_decode(v))
        })
        .and_then(|v| v.parse::<u64>().ok())
}
