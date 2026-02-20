use std::fmt;
use std::net::{IpAddr, Ipv4Addr};

use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const DEFAULT_ZMQ_BUFFER_LIMIT: usize = 5000;
pub const MIN_ZMQ_BUFFER_LIMIT: usize = 50;
pub const MAX_ZMQ_BUFFER_LIMIT: usize = 100000;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RpcConfig {
    pub url: String,
    pub user: String,
    pub password: String,
    pub wallet: String,
    pub poll_interval_secs: u64,
    pub zmq_address: String,
    pub zmq_buffer_limit: usize,
}

impl Default for RpcConfig {
    fn default() -> Self {
        Self {
            url: "http://127.0.0.1:8332".to_string(),
            user: String::new(),
            password: String::new(),
            wallet: String::new(),
            poll_interval_secs: 5,
            zmq_address: String::new(),
            zmq_buffer_limit: DEFAULT_ZMQ_BUFFER_LIMIT,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct RpcCall {
    pub id: Value,
    pub method: String,
    pub params: Value,
}

impl RpcCall {
    pub fn new(id: Value, method: impl Into<String>, params: Value) -> Self {
        Self {
            id,
            method: method.into(),
            params,
        }
    }
}

#[derive(Debug, Clone)]
pub struct RpcClient {
    config: RpcConfig,
    agent: ureq::Agent,
}

impl RpcClient {
    pub fn new(config: RpcConfig) -> Self {
        let agent = ureq::Agent::config_builder()
            .http_status_as_error(false)
            .build()
            .new_agent();
        Self { config, agent }
    }

    pub fn endpoint_url(&self) -> String {
        if self.config.wallet.is_empty() {
            return self.config.url.clone();
        }
        format!("{}/wallet/{}", self.config.url, self.config.wallet)
    }

    pub fn call(&self, method: &str, params: Value) -> Result<Value, RpcError> {
        let call = serde_json::json!({
            "method": method,
            "params": params,
        });
        let payload = normalize_call(&call, 1)?;
        let response = self.post_json(&payload)?;
        extract_result(&response)
    }

    pub fn batch(&self, calls: &[RpcCall]) -> Result<Vec<Value>, RpcError> {
        if calls.is_empty() {
            return Ok(Vec::new());
        }

        let payload = Value::Array(
            calls
                .iter()
                .map(|call| {
                    serde_json::json!({
                        "jsonrpc": "2.0",
                        "id": call.id,
                        "method": call.method,
                        "params": call.params,
                    })
                })
                .collect(),
        );

        let response = self.post_json(&payload)?;
        let array = response.as_array().ok_or_else(|| {
            RpcError::InvalidResponse("expected batch array response".to_string())
        })?;

        array.iter().map(extract_result).collect()
    }

    pub fn post_json(&self, payload: &Value) -> Result<Value, RpcError> {
        let body = payload.to_string();
        let response = self
            .agent
            .post(self.endpoint_url())
            .header(
                "Authorization",
                &basic_auth(&self.config.user, &self.config.password),
            )
            .content_type("application/json")
            .send(body.as_bytes())
            .map_err(|e| RpcError::Transport(e.to_string()))?;

        let mut response = response;
        let text = response
            .body_mut()
            .read_to_string()
            .map_err(|e| RpcError::Transport(e.to_string()))?;

        serde_json::from_str::<Value>(&text)
            .map_err(|e| RpcError::InvalidResponse(format!("invalid json response: {e}")))
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum RpcError {
    InvalidRequest(String),
    Transport(String),
    InvalidResponse(String),
    Rpc {
        code: Option<i64>,
        message: String,
        data: Option<Value>,
    },
}

impl fmt::Display for RpcError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RpcError::InvalidRequest(message) => write!(f, "invalid request: {message}"),
            RpcError::Transport(message) => write!(f, "transport error: {message}"),
            RpcError::InvalidResponse(message) => write!(f, "invalid response: {message}"),
            RpcError::Rpc { message, .. } => write!(f, "rpc error: {message}"),
        }
    }
}

impl std::error::Error for RpcError {}

pub fn allow_insecure() -> bool {
    static ALLOWED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ALLOWED.get_or_init(|| {
        std::env::var("DANGER_INSECURE_RPC")
            .ok()
            .is_some_and(|value| value == "1")
    })
}

fn normalize_call(call: &Value, fallback_id: u64) -> Result<Value, RpcError> {
    let method = call["method"]
        .as_str()
        .ok_or_else(|| RpcError::InvalidRequest("missing RPC method".to_string()))?;
    let params = call
        .get("params")
        .cloned()
        .unwrap_or_else(|| Value::Array(Vec::new()));
    let id = call
        .get("id")
        .cloned()
        .unwrap_or_else(|| serde_json::json!(fallback_id));
    let jsonrpc = call
        .get("jsonrpc")
        .cloned()
        .unwrap_or_else(|| Value::String("2.0".to_string()));

    Ok(serde_json::json!({
        "jsonrpc": jsonrpc,
        "id": id,
        "method": method,
        "params": params,
    }))
}

fn extract_result(response: &Value) -> Result<Value, RpcError> {
    if response.get("error").is_some() && !response["error"].is_null() {
        let code = response["error"]["code"].as_i64();
        let message = response["error"]["message"]
            .as_str()
            .map(ToOwned::to_owned)
            .unwrap_or_else(|| "unknown rpc error".to_string());
        let data = response["error"].get("data").cloned();
        return Err(RpcError::Rpc {
            code,
            message,
            data,
        });
    }

    response
        .get("result")
        .cloned()
        .ok_or_else(|| RpcError::InvalidResponse("missing result field".to_string()))
}

pub fn is_safe_rpc_host(url: &str) -> bool {
    let host = match url.find("://") {
        Some(index) => {
            let after = &url[index + 3..];
            let after = after.split('/').next().unwrap_or(after);
            let after = after.split('?').next().unwrap_or(after);
            let after = after.rsplit('@').next().unwrap_or(after);

            if after.starts_with('[') {
                after
                    .trim_start_matches('[')
                    .split(']')
                    .next()
                    .unwrap_or(after)
            } else {
                after.split(':').next().unwrap_or(after)
            }
        }
        None => return false,
    };

    if host.eq_ignore_ascii_case("localhost") {
        return true;
    }

    let ip = match host.parse::<IpAddr>() {
        Ok(ip) => ip,
        Err(_) => return false,
    };

    match ip {
        IpAddr::V4(v4) => v4.is_loopback() || v4.is_private() || is_cgnat(v4),
        IpAddr::V6(v6) => {
            v6.is_loopback()
                || v6.is_unique_local()
                || v6.is_unicast_link_local()
                || v6
                    .to_ipv4_mapped()
                    .is_some_and(|mapped| mapped.is_loopback() || mapped.is_private())
        }
    }
}

fn is_cgnat(v4: Ipv4Addr) -> bool {
    let octets = v4.octets();
    octets[0] == 100 && (64..=127).contains(&octets[1])
}

fn basic_auth(user: &str, password: &str) -> String {
    use std::io::Write;

    let mut buffer = Vec::new();
    write!(buffer, "{user}:{password}").expect("in-memory write should never fail");
    format!("Basic {}", base64_encode(&buffer))
}

fn base64_encode(data: &[u8]) -> String {
    const CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut output = String::new();

    for chunk in data.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = if chunk.len() > 1 { chunk[1] as u32 } else { 0 };
        let b2 = if chunk.len() > 2 { chunk[2] as u32 } else { 0 };
        let triple = (b0 << 16) | (b1 << 8) | b2;

        output.push(CHARS[((triple >> 18) & 0x3f) as usize] as char);
        output.push(CHARS[((triple >> 12) & 0x3f) as usize] as char);
        output.push(if chunk.len() > 1 {
            CHARS[((triple >> 6) & 0x3f) as usize] as char
        } else {
            '='
        });
        output.push(if chunk.len() > 2 {
            CHARS[(triple & 0x3f) as usize] as char
        } else {
            '='
        });
    }

    output
}

#[cfg(test)]
mod tests {
    use serde_json::Value;

    use super::{RpcError, extract_result, is_safe_rpc_host, normalize_call};

    fn normalize_payload(input: &Value) -> Result<(String, Value), RpcError> {
        if let Some(calls) = input.as_array() {
            let mut out = Vec::with_capacity(calls.len());
            for (index, call) in calls.iter().enumerate() {
                out.push(normalize_call(call, (index + 1) as u64)?);
            }
            return Ok((format!("batch[{}]", calls.len()), Value::Array(out)));
        }
        Ok((
            input["method"].as_str().unwrap_or("").to_string(),
            normalize_call(input, 1)?,
        ))
    }

    #[test]
    fn single_payload_is_normalized() {
        let (method, payload) = normalize_payload(&serde_json::json!({
            "method": "getblockcount",
            "params": []
        }))
        .expect("payload should normalize");

        assert_eq!(method, "getblockcount");
        assert_eq!(payload["jsonrpc"], "2.0");
        assert_eq!(payload["id"], 1);
        assert_eq!(payload["method"], "getblockcount");
    }

    #[test]
    fn batch_payload_is_normalized() {
        let (method, payload) = normalize_payload(&serde_json::json!([
            {"method": "uptime", "params": []},
            {"id": 99, "method": "getmempoolinfo", "params": []}
        ]))
        .expect("payload should normalize");

        assert_eq!(method, "batch[2]");
        let calls = payload.as_array().expect("batch should be array");
        assert_eq!(calls[0]["id"], 1);
        assert_eq!(calls[1]["id"], 99);
    }

    #[test]
    fn missing_method_maps_to_invalid_request() {
        let error = normalize_payload(&serde_json::json!({ "params": [] }))
            .expect_err("payload without method should fail");

        assert!(matches!(error, RpcError::InvalidRequest(_)));
    }

    #[test]
    fn safe_host_validation_matches_expected_ranges() {
        assert!(is_safe_rpc_host("http://127.0.0.1:8332"));
        assert!(is_safe_rpc_host("http://10.0.0.5:8332"));
        assert!(is_safe_rpc_host("http://[::1]:8332"));
        assert!(is_safe_rpc_host("http://100.64.1.2:8332"));
        assert!(!is_safe_rpc_host("http://8.8.8.8:8332"));
        assert!(!is_safe_rpc_host("http://example.com:8332"));
    }

    #[test]
    fn rpc_error_display_is_stable() {
        let error = RpcError::Rpc {
            code: Some(-1),
            message: "boom".to_string(),
            data: None,
        };
        assert_eq!(format!("{error}"), "rpc error: boom");
    }

    #[test]
    fn rpc_error_response_maps_to_rpc_error_variant() {
        let result = extract_result(&serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "error": { "code": -8, "message": "wallet not found" }
        }));

        assert!(matches!(result, Err(RpcError::Rpc { code: Some(-8), .. })));
    }
}
