use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use tracing::{debug, warn};

pub struct ZmqMessage {
    pub topic: String,
    pub body_hex: String,
    pub body_size: usize,
    pub sequence: u32,
    pub timestamp: u64,
}

pub struct ZmqState {
    pub connected: bool,
    pub address: String,
    pub messages: VecDeque<ZmqMessage>,
}

impl Default for ZmqState {
    fn default() -> Self {
        Self {
            connected: false,
            address: String::new(),
            messages: VecDeque::new(),
        }
    }
}

pub struct ZmqHandle {
    shutdown: Arc<AtomicBool>,
    thread: std::thread::JoinHandle<()>,
}

pub fn start_zmq_subscriber(address: &str, state: Arc<Mutex<ZmqState>>) -> ZmqHandle {
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
        for topic in &["hashblock", "hashtx", "rawblock", "rawtx", "sequence"] {
            socket.set_subscribe(topic.as_bytes()).ok();
        }

        if let Err(e) = socket.connect(&addr) {
            warn!(address = %addr, error = %e, "failed to connect ZMQ subscriber");
            return;
        }

        debug!(address = %addr, "connected ZMQ subscriber");
        state.lock().unwrap().connected = true;
        state.lock().unwrap().address = addr;

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

            let mut s = state.lock().unwrap();
            if s.messages.len() >= 100 {
                s.messages.pop_front();
            }
            s.messages.push_back(ZmqMessage {
                topic,
                body_hex,
                body_size,
                sequence,
                timestamp,
            });
        }

        state.lock().unwrap().connected = false;
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
