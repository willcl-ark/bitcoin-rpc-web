use std::net::{IpAddr, Ipv4Addr};
use std::sync::{Arc, Mutex, OnceLock};

use tracing::{debug, warn};

pub const DEFAULT_ZMQ_BUFFER_LIMIT: usize = 5000;
pub const MIN_ZMQ_BUFFER_LIMIT: usize = 50;
pub const MAX_ZMQ_BUFFER_LIMIT: usize = 100000;

pub struct RpcConfig {
    pub url: String,
    pub user: String,
    pub password: String,
    pub wallet: String,
    pub zmq_address: String,
    pub zmq_buffer_limit: usize,
}

impl Default for RpcConfig {
    fn default() -> Self {
        Self {
            url: "http://127.0.0.1:8332".into(),
            user: String::new(),
            password: String::new(),
            wallet: String::new(),
            zmq_address: String::new(),
            zmq_buffer_limit: DEFAULT_ZMQ_BUFFER_LIMIT,
        }
    }
}

pub struct ConfigUpdateResult {
    pub zmq_changed: bool,
    pub insecure_blocked: bool,
}

pub fn allow_insecure() -> bool {
    static ALLOWED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ALLOWED.get_or_init(|| {
        std::env::var("DANGER_INSECURE_RPC")
            .ok()
            .is_some_and(|v| v == "1")
    })
}

pub fn do_rpc(body: &str, config: &Arc<Mutex<RpcConfig>>) -> String {
    debug!(bytes = body.len(), "rpc request received");
    let msg: serde_json::Value = match serde_json::from_str(body) {
        Ok(v) => v,
        Err(e) => {
            warn!(error = %e, "rpc request JSON parse failed");
            return json_error(e.to_string());
        }
    };

    let method = msg["method"].as_str().unwrap_or("");
    let params = &msg["params"];

    let cfg = config.lock().unwrap();
    let mut url = cfg.url.clone();
    let user = cfg.user.clone();
    let password = cfg.password.clone();
    let wallet = cfg.wallet.clone();
    drop(cfg);

    if !wallet.is_empty() {
        url = format!("{url}/wallet/{wallet}");
    }

    let envelope = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": method,
        "params": params,
    });

    let payload = envelope.to_string();
    debug!(method, url = %url, "rpc POST");
    match rpc_agent()
        .post(&url)
        .header("Authorization", &basic_auth(&user, &password))
        .content_type("application/json")
        .send(payload.as_bytes())
    {
        Ok(mut resp) => {
            let status = resp.status();
            let out = resp.body_mut().read_to_string().unwrap_or_default();
            debug!(method, status = %status, bytes = out.len(), "rpc response");
            out
        }
        Err(e) => {
            warn!(method, error = %e, "rpc transport error");
            json_error(e.to_string())
        }
    }
}

fn json_error(message: String) -> String {
    serde_json::json!({ "error": message }).to_string()
}

fn rpc_agent() -> &'static ureq::Agent {
    static AGENT: OnceLock<ureq::Agent> = OnceLock::new();
    AGENT.get_or_init(|| {
        ureq::Agent::config_builder()
            .http_status_as_error(false)
            .build()
            .new_agent()
    })
}

pub fn update_config(body: &str, config: &Arc<Mutex<RpcConfig>>) -> ConfigUpdateResult {
    let msg: serde_json::Value = match serde_json::from_str(body) {
        Ok(v) => v,
        Err(_) => {
            return ConfigUpdateResult {
                zmq_changed: false,
                insecure_blocked: false,
            };
        }
    };

    let mut cfg = config.lock().unwrap();
    let mut insecure_blocked = false;
    if let Some(url) = msg["url"].as_str() {
        if is_safe_rpc_host(url) || allow_insecure() {
            cfg.url = url.into();
        } else {
            warn!(url, "blocked non-local RPC URL");
            insecure_blocked = true;
        }
    }
    if let Some(user) = msg["user"].as_str() {
        cfg.user = user.into();
    }
    if let Some(password) = msg["password"].as_str() {
        cfg.password = password.into();
    }
    if let Some(wallet) = msg["wallet"].as_str() {
        cfg.wallet = wallet.into();
    }
    let mut zmq_changed = false;
    if let Some(addr) = msg["zmq_address"].as_str()
        && cfg.zmq_address != addr {
            cfg.zmq_address = addr.into();
            zmq_changed = true;
        }
    if let Some(limit) = parse_usize(&msg["zmq_buffer_limit"]) {
        cfg.zmq_buffer_limit = limit.clamp(MIN_ZMQ_BUFFER_LIMIT, MAX_ZMQ_BUFFER_LIMIT);
    }

    ConfigUpdateResult {
        zmq_changed,
        insecure_blocked,
    }
}

fn is_safe_rpc_host(url: &str) -> bool {
    let host = match url.find("://") {
        Some(i) => {
            let after = &url[i + 3..];
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

    is_safe_rpc_ip(ip)
}

fn is_safe_rpc_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => v4.is_loopback() || v4.is_private() || is_cgnat(v4),
        IpAddr::V6(v6) => {
            v6.is_loopback()
                || v6.is_unique_local()
                || v6.is_unicast_link_local()
                || v6
                    .to_ipv4_mapped()
                    .is_some_and(|mapped| is_safe_rpc_ip(IpAddr::V4(mapped)))
        }
    }
}

fn is_cgnat(v4: Ipv4Addr) -> bool {
    let octets = v4.octets();
    octets[0] == 100 && (64..=127).contains(&octets[1])
}

fn basic_auth(user: &str, password: &str) -> String {
    use std::io::Write;
    let mut buf = Vec::new();
    write!(buf, "{user}:{password}").unwrap();
    format!("Basic {}", base64_encode(&buf))
}

fn base64_encode(data: &[u8]) -> String {
    const CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for chunk in data.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = if chunk.len() > 1 { chunk[1] as u32 } else { 0 };
        let b2 = if chunk.len() > 2 { chunk[2] as u32 } else { 0 };
        let triple = (b0 << 16) | (b1 << 8) | b2;
        out.push(CHARS[(triple >> 18 & 0x3F) as usize] as char);
        out.push(CHARS[(triple >> 12 & 0x3F) as usize] as char);
        if chunk.len() > 1 {
            out.push(CHARS[(triple >> 6 & 0x3F) as usize] as char);
        } else {
            out.push('=');
        }
        if chunk.len() > 2 {
            out.push(CHARS[(triple & 0x3F) as usize] as char);
        } else {
            out.push('=');
        }
    }
    out
}

fn parse_usize(value: &serde_json::Value) -> Option<usize> {
    if let Some(n) = value.as_u64() {
        return usize::try_from(n).ok();
    }
    value.as_str().and_then(|s| s.trim().parse::<usize>().ok())
}

#[cfg(test)]
mod tests {
    use super::{
        MAX_ZMQ_BUFFER_LIMIT, MIN_ZMQ_BUFFER_LIMIT, RpcConfig, is_safe_rpc_host, json_error,
        update_config,
    };
    use std::sync::{Arc, Mutex};

    #[test]
    fn safe_ipv4_hosts_are_allowed() {
        assert!(is_safe_rpc_host("http://127.0.0.1:8332"));
        assert!(is_safe_rpc_host("http://10.0.0.2:8332"));
        assert!(is_safe_rpc_host("http://172.16.1.2:8332"));
        assert!(is_safe_rpc_host("http://192.168.1.2:8332"));
        assert!(is_safe_rpc_host("http://100.64.1.2:8332"));
        assert!(is_safe_rpc_host("http://localhost:8332"));
    }

    #[test]
    fn safe_ipv6_hosts_are_allowed() {
        assert!(is_safe_rpc_host("http://[::1]:8332"));
        assert!(is_safe_rpc_host("http://[fd00::1]:8332"));
        assert!(is_safe_rpc_host("http://[fe80::1]:8332"));
        assert!(is_safe_rpc_host("http://[::ffff:127.0.0.1]:8332"));
    }

    #[test]
    fn public_or_invalid_hosts_are_blocked() {
        assert!(!is_safe_rpc_host("http://8.8.8.8:8332"));
        assert!(!is_safe_rpc_host("http://[2001:4860:4860::8888]:8332"));
        assert!(!is_safe_rpc_host("http://example.com:8332"));
        assert!(!is_safe_rpc_host("not-a-url"));
    }

    #[test]
    fn zmq_buffer_limit_is_clamped_to_safe_bounds() {
        let cfg = Arc::new(Mutex::new(RpcConfig::default()));

        update_config(r#"{"zmq_buffer_limit":10}"#, &cfg);
        assert_eq!(cfg.lock().unwrap().zmq_buffer_limit, MIN_ZMQ_BUFFER_LIMIT);

        update_config(r#"{"zmq_buffer_limit":200000}"#, &cfg);
        assert_eq!(cfg.lock().unwrap().zmq_buffer_limit, MAX_ZMQ_BUFFER_LIMIT);
    }

    #[test]
    fn error_json_is_valid_and_escaped() {
        let out = json_error("bad \"quote\"\nline".to_string());
        let v: serde_json::Value = serde_json::from_str(&out).expect("valid JSON error envelope");
        assert_eq!(v["error"].as_str(), Some("bad \"quote\"\nline"));
    }
}
