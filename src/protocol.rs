use std::borrow::Cow;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tracing::{debug, warn};
use wry::http::Response;
use wry::http::header::{ACCESS_CONTROL_ALLOW_ORIGIN, CONTENT_TYPE};

use crate::music;
use crate::rpc::{self, RpcConfig};
use crate::rpc_limiter::RpcLimiter;
use crate::thread_pool::ThreadPool;
use crate::zmq::{self, ZmqHandle, ZmqSharedState};

pub fn build_webview(
    config: Arc<Mutex<RpcConfig>>,
    rpc_limiter: Arc<RpcLimiter>,
    rpc_pool: Arc<ThreadPool>,
    zmq_poll_pool: Arc<ThreadPool>,
    music_runtime: Arc<music::MusicRuntime>,
    zmq_state: Arc<ZmqSharedState>,
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
                    let responder = Arc::new(Mutex::new(Some(responder)));
                    let cfg = Arc::clone(&cfg);
                    let async_responder = Arc::clone(&responder);
                    if rpc_pool
                        .execute(move || {
                            let _permit = permit;
                            let result = rpc::do_rpc(&body, &cfg);
                            respond_once(&async_responder, json_response(&result));
                        })
                        .is_err()
                    {
                        warn!("rpc worker pool unavailable");
                        respond_once(&responder, json_error_response("rpc worker pool unavailable"));
                    }
                } else {
                    warn!("rpc request rejected due to in-flight limit");
                    responder.respond(json_error_response("rpc worker pool saturated; try again"));
                }
                return;
            }

            if path == "/config" {
                let body = request_body(&req, &query);
                let result = rpc::update_config(&body, &cfg);
                {
                    let limit = cfg.lock().unwrap().zmq_buffer_limit;
                    let mut state = zmq_state.state.lock().unwrap();
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
                responder.respond(json_value_response(serde_json::json!({ "allowed": allowed })));
                return;
            }

            if path == "/features" {
                responder.respond(json_value_response(serde_json::json!({
                    "audio": music::is_enabled()
                })));
                return;
            }

            if path == "/zmq/decode-rawtx" {
                let timestamp = query_param_u64(&query, "timestamp");
                let sequence = query_param_u64(&query, "sequence");
                let result = if let (Some(timestamp), Some(sequence)) = (timestamp, sequence) {
                    let raw_hex = {
                        let s = zmq_state.state.lock().unwrap();
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
                let since = query_param_u64(&query, "since").unwrap_or(0);
                let wait_ms = query_param_u64(&query, "wait_ms")
                    .unwrap_or(0)
                    .clamp(0, 30_000);
                let state = Arc::clone(&zmq_state);
                let responder = Arc::new(Mutex::new(Some(responder)));
                let async_responder = Arc::clone(&responder);
                if zmq_poll_pool
                    .execute(move || {
                        if wait_ms > 0 {
                            let timeout = Duration::from_millis(wait_ms);
                            let guard = state.state.lock().unwrap();
                            let _ = state.changed.wait_timeout_while(guard, timeout, |s| {
                                s.messages.back().is_none_or(|m| m.cursor <= since)
                            });
                        }
                        let result = zmq_messages_response(&state, since);
                        respond_once(&async_responder, json_response(&result));
                    })
                    .is_err()
                {
                    warn!("zmq poll worker pool unavailable");
                    respond_once(&responder, json_error_response("zmq poll worker pool unavailable"));
                }
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

fn json_value_response(value: serde_json::Value) -> Response<Cow<'static, [u8]>> {
    json_response(&value.to_string())
}

fn json_error_response(message: &str) -> Response<Cow<'static, [u8]>> {
    json_value_response(serde_json::json!({ "error": message }))
}

fn respond_once(
    responder: &Arc<Mutex<Option<wry::RequestAsyncResponder>>>,
    response: Response<Cow<'static, [u8]>>,
) {
    if let Ok(mut guard) = responder.lock()
        && let Some(async_responder) = guard.take() {
            async_responder.respond(response);
        }
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
        if b[i] == b'%' && i + 2 < b.len()
            && let Ok(byte) =
                u8::from_str_radix(std::str::from_utf8(&b[i + 1..i + 3]).unwrap_or(""), 16)
            {
                out.push(byte);
                i += 3;
                continue;
            }
        out.push(if b[i] == b'+' { b' ' } else { b[i] });
        i += 1;
    }
    String::from_utf8_lossy(&out).to_string()
}

fn request_body(req: &wry::http::Request<Vec<u8>>, query: &str) -> String {
    if req.method() == wry::http::Method::POST {
        if let Some(encoded) = req
            .headers()
            .get("x-app-json")
            .and_then(|v| v.to_str().ok())
        {
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

fn zmq_messages_response(zmq_state: &Arc<ZmqSharedState>, since: u64) -> String {
    let s = zmq_state.state.lock().unwrap();
    let mut truncated = false;
    let messages: Vec<serde_json::Value> = s
        .messages
        .iter()
        .filter(|m| m.cursor > since)
        .map(|m| {
            serde_json::json!({
                "cursor": m.cursor,
                "topic": m.topic,
                "body_hex": m.body_hex,
                "body_size": m.body_size,
                "sequence": m.sequence,
                "timestamp": m.timestamp,
                "event_hash": m.event_hash,
            })
        })
        .collect();
    if since > 0
        && !messages.is_empty()
        && s.messages
            .iter()
            .find(|m| m.cursor > since)
            .is_some_and(|m| m.cursor > since.saturating_add(1))
    {
        truncated = true;
    }
    let cursor = s.messages.back().map_or(0, |m| m.cursor);
    serde_json::json!({
        "connected": s.connected,
        "address": s.address,
        "buffer_limit": s.buffer_limit,
        "cursor": cursor,
        "truncated": truncated,
        "messages": messages,
    })
    .to_string()
}
