use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex};

use tracing::{debug, warn};

const DEFAULT_ZMQ_SOCKET_RCVHWM: i32 = 100_000;
const MIN_ZMQ_SOCKET_RCVHWM: i32 = 1_000;
const MAX_ZMQ_SOCKET_RCVHWM: i32 = 1_000_000;

pub struct ZmqMessage {
    pub cursor: u64,
    pub topic: String,
    pub body_hex: String,
    pub body_size: usize,
    pub sequence: u32,
    pub timestamp: u64,
    pub event_hash: Option<String>,
}

pub struct ZmqState {
    pub connected: bool,
    pub address: String,
    pub buffer_limit: usize,
    pub next_cursor: u64,
    pub messages: VecDeque<ZmqMessage>,
}

impl Default for ZmqState {
    fn default() -> Self {
        Self {
            connected: false,
            address: String::new(),
            buffer_limit: crate::rpc::DEFAULT_ZMQ_BUFFER_LIMIT,
            next_cursor: 1,
            messages: VecDeque::new(),
        }
    }
}

pub struct ZmqSharedState {
    pub state: Mutex<ZmqState>,
    pub changed: Condvar,
}

impl Default for ZmqSharedState {
    fn default() -> Self {
        Self {
            state: Mutex::new(ZmqState::default()),
            changed: Condvar::new(),
        }
    }
}

pub struct ZmqHandle {
    shutdown: Arc<AtomicBool>,
    thread: std::thread::JoinHandle<()>,
}

pub fn start_zmq_subscriber(address: &str, state: Arc<ZmqSharedState>) -> ZmqHandle {
    let shutdown = Arc::new(AtomicBool::new(false));
    let flag = Arc::clone(&shutdown);
    let addr = address.to_string();

    let thread = std::thread::spawn(move || {
        let ctx = zmq2::Context::new();
        let socket = match ctx.socket(zmq2::SUB) {
            Ok(s) => s,
            Err(e) => {
                warn!(error = %e, "failed to create ZMQ subscriber socket");
                return;
            }
        };

        socket.set_rcvtimeo(500).ok();
        let rcvhwm = zmq_socket_rcvhwm();
        if socket.set_rcvhwm(rcvhwm).is_err() {
            warn!(rcvhwm, "failed to apply ZMQ subscriber rcvhwm");
        } else {
            debug!(rcvhwm, "configured ZMQ subscriber rcvhwm");
        }
        for topic in &["hashblock", "hashtx"] {
            socket.set_subscribe(topic.as_bytes()).ok();
        }

        if let Err(e) = socket.connect(&addr) {
            warn!(address = %addr, error = %e, "failed to connect ZMQ subscriber");
            return;
        }

        debug!(address = %addr, "connected ZMQ subscriber");
        {
            let mut s = state.state.lock().unwrap();
            s.connected = true;
            s.address = addr;
        }
        state.changed.notify_all();

        while !flag.load(Ordering::Relaxed) {
            let parts = match socket.recv_multipart(0) {
                Ok(p) => p,
                Err(zmq2::Error::EAGAIN) => continue,
                Err(e) => {
                    warn!(error = %e, "ZMQ receive error");
                    break;
                }
            };

            if parts.len() < 3 {
                continue;
            }

            let topic = String::from_utf8_lossy(&parts[0]).to_string();
            let body = &parts[1];
            let body_hex = hex_encode(&body[..body.len().min(80)]);
            let event_hash = (body.len() >= 32).then(|| hash_from_notification(body));
            let body_size = body.len();
            let sequence = if parts[2].len() >= 4 {
                u32::from_le_bytes([parts[2][0], parts[2][1], parts[2][2], parts[2][3]])
            } else {
                0
            };
            let timestamp = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();

            let mut s = state.state.lock().unwrap();
            let limit = s.buffer_limit.clamp(
                crate::rpc::MIN_ZMQ_BUFFER_LIMIT,
                crate::rpc::MAX_ZMQ_BUFFER_LIMIT,
            );
            if s.messages.len() >= limit {
                s.messages.pop_front();
            }
            let cursor = s.next_cursor;
            s.next_cursor = s.next_cursor.saturating_add(1);
            s.messages.push_back(ZmqMessage {
                cursor,
                topic,
                body_hex,
                body_size,
                sequence,
                timestamp,
                event_hash,
            });
            drop(s);
            state.changed.notify_all();
        }

        {
            let mut s = state.state.lock().unwrap();
            mark_disconnected(&mut s);
        }
        state.changed.notify_all();
        debug!("stopped ZMQ subscriber");
    });

    ZmqHandle { shutdown, thread }
}

pub fn stop_zmq_subscriber(handle: ZmqHandle) {
    handle.shutdown.store(true, Ordering::Relaxed);
    let _ = handle.thread.join();
}

fn hex_encode(data: &[u8]) -> String {
    use std::fmt::Write;
    let mut s = String::with_capacity(data.len() * 2);
    for &b in data {
        write!(s, "{b:02x}").unwrap();
    }
    s
}

fn hash_from_notification(bytes: &[u8]) -> String {
    hex_encode(&bytes[..32])
}

fn mark_disconnected(state: &mut ZmqState) {
    state.connected = false;
    state.address.clear();
}

fn zmq_socket_rcvhwm() -> i32 {
    std::env::var("ZMQ_SOCKET_RCVHWM")
        .ok()
        .and_then(|v| v.parse::<i32>().ok())
        .unwrap_or(DEFAULT_ZMQ_SOCKET_RCVHWM)
        .clamp(MIN_ZMQ_SOCKET_RCVHWM, MAX_ZMQ_SOCKET_RCVHWM)
}

#[cfg(test)]
mod tests {
    use super::{ZmqState, mark_disconnected};

    #[test]
    fn disconnect_clears_connection_address() {
        let mut state = ZmqState {
            connected: true,
            address: "tcp://127.0.0.1:29000".to_string(),
            ..ZmqState::default()
        };
        mark_disconnected(&mut state);
        assert!(!state.connected);
        assert!(state.address.is_empty());
    }
}
