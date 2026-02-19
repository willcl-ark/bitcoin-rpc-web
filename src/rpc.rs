use std::sync::{Arc, Mutex};

pub struct RpcConfig {
    pub url: String,
    pub user: String,
    pub password: String,
    pub wallet: String,
    pub zmq_address: String,
}

impl Default for RpcConfig {
    fn default() -> Self {
        Self {
            url: "http://127.0.0.1:8332".into(),
            user: String::new(),
            password: String::new(),
            wallet: String::new(),
            zmq_address: String::new(),
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
    let msg: serde_json::Value = match serde_json::from_str(body) {
        Ok(v) => v,
        Err(e) => return format!(r#"{{"error":"{e}"}}"#),
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

    let agent = ureq::Agent::config_builder()
        .http_status_as_error(false)
        .build()
        .new_agent();

    let payload = envelope.to_string();
    match agent
        .post(&url)
        .header("Authorization", &basic_auth(&user, &password))
        .content_type("application/json")
        .send(payload.as_bytes())
    {
        Ok(mut resp) => resp.body_mut().read_to_string().unwrap_or_default(),
        Err(e) => format!(r#"{{"error":"{}"}}"#, e),
    }
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
    if let Some(addr) = msg["zmq_address"].as_str() {
        if cfg.zmq_address != addr {
            cfg.zmq_address = addr.into();
            zmq_changed = true;
        }
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

    let octets: Vec<u8> = match host
        .split('.')
        .map(|s| s.parse::<u8>())
        .collect::<Result<Vec<_>, _>>()
    {
        Ok(v) if v.len() == 4 => v,
        _ => return false,
    };

    matches!(
        (octets[0], octets[1]),
        (127, _) | (10, _) | (192, 168) | (100, 64..=127)
    ) || (octets[0] == 172 && (16..=31).contains(&octets[1]))
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
