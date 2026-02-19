use std::borrow::Cow;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use rodio::{OutputStream, OutputStreamHandle, Sink, Source};
use wry::http::header::{ACCESS_CONTROL_ALLOW_ORIGIN, CONTENT_TYPE};
use wry::http::Response;
use xmrs::import::amiga::amiga_module::AmigaModule;
use xmrs::module::Module;
use xmrsplayer::xmrsplayer::XmrsPlayer;

fn log_enabled() -> bool {
    static ENABLED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ENABLED.get_or_init(|| std::env::var_os("RUST_LOG").is_some())
}

macro_rules! dbg_log {
    ($($arg:tt)*) => {
        if log_enabled() {
            eprintln!($($arg)*);
        }
    };
}

struct RpcConfig {
    url: String,
    user: String,
    password: String,
    wallet: String,
    zmq_address: String,
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
            dbg_log!("[asset] 404: {path}");
            return Response::builder()
                .status(404)
                .body(Cow::Borrowed(b"Not found" as &[u8]))
                .unwrap();
        }
    };
    dbg_log!("[asset] 200 {path} ({mime}, {} bytes)", content.len());
    Response::builder()
        .header(CONTENT_TYPE, mime)
        .header(ACCESS_CONTROL_ALLOW_ORIGIN, "*")
        .body(Cow::Borrowed(content))
        .unwrap()
}

fn do_rpc(body: &str, config: &Arc<Mutex<RpcConfig>>) -> String {
    dbg_log!("[rpc] parsing body ({} bytes): {:?}", body.len(), &body[..body.len().min(200)]);
    let msg: serde_json::Value = match serde_json::from_str(body) {
        Ok(v) => v,
        Err(e) => {
            dbg_log!("[rpc] JSON parse error: {e}");
            return format!(r#"{{"error":"{e}"}}"#);
        }
    };

    let method = msg["method"].as_str().unwrap_or("");
    let params = &msg["params"];
    dbg_log!("[rpc] method={method} params={params}");

    let cfg = config.lock().unwrap();
    let mut url = cfg.url.clone();
    let user = cfg.user.clone();
    let password = cfg.password.clone();
    let wallet = cfg.wallet.clone();
    drop(cfg);

    if !wallet.is_empty() {
        url = format!("{url}/wallet/{wallet}");
    }

    dbg_log!("[rpc] POST {url} (user={user:?})");

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
    let result = match agent
        .post(&url)
        .header("Authorization", &basic_auth(&user, &password))
        .content_type("application/json")
        .send(payload.as_bytes())
    {
        Ok(mut resp) => {
            let status = resp.status();
            let body = resp.body_mut().read_to_string().unwrap_or_default();
            dbg_log!("[rpc] response HTTP {status} ({} bytes): {:?}", body.len(), &body[..body.len().min(200)]);
            body
        }
        Err(e) => {
            dbg_log!("[rpc] request error: {e}");
            format!(r#"{{"error":"{}"}}"#, e)
        }
    };

    result
}

fn redact_password(body: &str) -> String {
    match serde_json::from_str::<serde_json::Value>(body) {
        Ok(mut v) => {
            if v.get("password").is_some() {
                v["password"] = serde_json::Value::String("*****".into());
            }
            v.to_string()
        }
        Err(_) => body.to_string(),
    }
}

fn update_config(body: &str, config: &Arc<Mutex<RpcConfig>>) -> bool {
    dbg_log!("[config] body: {:?}", redact_password(body));
    let msg: serde_json::Value = match serde_json::from_str(body) {
        Ok(v) => v,
        Err(e) => {
            dbg_log!("[config] parse error: {e}");
            return false;
        }
    };
    let mut cfg = config.lock().unwrap();
    if let Some(url) = msg["url"].as_str() {
        cfg.url = url.into();
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
    dbg_log!("[config] updated: url={:?} user={:?} wallet={:?} zmq={:?}", cfg.url, cfg.user, cfg.wallet, cfg.zmq_address);
    zmq_changed
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

fn percent_decode(input: &str) -> String {
    let mut out = Vec::new();
    let b = input.as_bytes();
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            if let Ok(byte) = u8::from_str_radix(
                std::str::from_utf8(&b[i + 1..i + 3]).unwrap_or(""),
                16,
            ) {
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

// --- ZMQ subscriber ---

struct ZmqMessage {
    topic: String,
    body_hex: String,
    body_size: usize,
    sequence: u32,
    timestamp: u64,
}

struct ZmqState {
    connected: bool,
    address: String,
    messages: VecDeque<ZmqMessage>,
}

struct ZmqHandle {
    shutdown: Arc<AtomicBool>,
    thread: std::thread::JoinHandle<()>,
}

fn hex_encode(data: &[u8]) -> String {
    use std::fmt::Write;
    let mut s = String::with_capacity(data.len() * 2);
    for &b in data {
        write!(s, "{b:02x}").unwrap();
    }
    s
}

fn start_zmq_subscriber(address: &str, state: Arc<Mutex<ZmqState>>) -> ZmqHandle {
    let shutdown = Arc::new(AtomicBool::new(false));
    let flag = Arc::clone(&shutdown);
    let addr = address.to_string();

    let thread = std::thread::spawn(move || {
        let ctx = zmq2::Context::new();
        let socket = match ctx.socket(zmq2::SUB) {
            Ok(s) => s,
            Err(e) => {
                dbg_log!("[zmq] failed to create socket: {e}");
                return;
            }
        };

        socket.set_rcvtimeo(500).ok();
        for topic in &["hashblock", "hashtx", "rawblock", "rawtx", "sequence"] {
            socket.set_subscribe(topic.as_bytes()).ok();
        }

        if let Err(e) = socket.connect(&addr) {
            dbg_log!("[zmq] connect failed ({addr}): {e}");
            return;
        }

        dbg_log!("[zmq] connected to {addr}");
        state.lock().unwrap().connected = true;

        while !flag.load(Ordering::Relaxed) {
            let parts = match socket.recv_multipart(0) {
                Ok(p) => p,
                Err(zmq2::Error::EAGAIN) => continue,
                Err(e) => {
                    dbg_log!("[zmq] recv error: {e}");
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

            dbg_log!("[zmq] {topic} seq={sequence} body={body_size}bytes");

            let mut s = state.lock().unwrap();
            if s.messages.len() >= 100 {
                s.messages.pop_front();
            }
            s.messages.push_back(ZmqMessage {
                topic, body_hex, body_size, sequence, timestamp,
            });
        }

        state.lock().unwrap().connected = false;
        dbg_log!("[zmq] subscriber stopped");
    });

    ZmqHandle { shutdown, thread }
}

fn stop_zmq_subscriber(handle: ZmqHandle) {
    handle.shutdown.store(true, Ordering::Relaxed);
    let _ = handle.thread.join();
}

const SAMPLE_RATE: u32 = 48000;

struct Tune {
    name: &'static str,
    module: &'static Module,
}

fn load_tunes() -> Vec<Tune> {
    let raw: &[(&str, &[u8])] = &[
        ("Hymn to Aurora", include_bytes!("../tunes/hymn_to_aurora.mod")),
        ("Musiklinjen", include_bytes!("../tunes/musiklinjen.mod")),
        ("Playing with Sound", include_bytes!("../tunes/playingw.mod")),
        ("Sundance", include_bytes!("../tunes/purple_motion_-_sundance.mod")),
        ("Resii", include_bytes!("../tunes/resii.mod")),
        ("Space Debris", include_bytes!("../tunes/space_debris.mod")),
        ("Stardust Memories", include_bytes!("../tunes/stardstm.mod")),
        ("Toy Story", include_bytes!("../tunes/toy_story.mod")),
        ("Toy Title", include_bytes!("../tunes/toytitle.mod")),
    ];
    raw.iter()
        .filter_map(|(name, data)| {
            match AmigaModule::load(data) {
                Ok(amiga) => {
                    let module = Box::leak(Box::new(amiga.to_module()));
                    Some(Tune { name, module })
                }
                Err(e) => {
                    dbg_log!("[music] failed to load {name}: {e:?}");
                    None
                }
            }
        })
        .collect()
}

struct ModSource {
    player: XmrsPlayer<'static>,
    buffer: Vec<f32>,
    pos: usize,
}

impl ModSource {
    fn new(module: &'static Module) -> Self {
        let mut player = XmrsPlayer::new(module, SAMPLE_RATE as f32, 0, false);
        player.set_max_loop_count(2);
        player.amplification = 0.5;
        Self { player, buffer: Vec::with_capacity(2048), pos: 0 }
    }
}

impl Iterator for ModSource {
    type Item = f32;
    fn next(&mut self) -> Option<f32> {
        if self.pos >= self.buffer.len() {
            self.buffer.clear();
            self.pos = 0;
            for _ in 0..1024 {
                match self.player.sample(true) {
                    Some((l, r)) => {
                        let mix = (l + r) * 0.5;
                        self.buffer.push(mix);
                        self.buffer.push(mix);
                    }
                    None => break,
                }
            }
            if self.buffer.is_empty() {
                return None;
            }
        }
        let s = self.buffer[self.pos];
        self.pos += 1;
        Some(s)
    }
}

impl Source for ModSource {
    fn current_frame_len(&self) -> Option<usize> { None }
    fn channels(&self) -> u16 { 2 }
    fn sample_rate(&self) -> u32 { SAMPLE_RATE }
    fn total_duration(&self) -> Option<Duration> { None }
}

enum MusicCmd {
    PlayPause,
    Next,
    Prev,
    SetVolume(f32),
    ToggleMute,
}

struct MusicState {
    current_track: usize,
    track_count: usize,
    track_name: String,
    playing: bool,
    volume: f32,
    muted: bool,
}

fn make_sink(handle: &OutputStreamHandle, module: &'static Module, volume: f32) -> Sink {
    let sink = Sink::try_new(handle).unwrap();
    let source = ModSource::new(module);
    sink.append(source);
    sink.set_volume(volume);
    sink
}

fn shuffle(tunes: &mut Vec<Tune>) {
    let mut seed = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos() as u64;
    for i in (1..tunes.len()).rev() {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        let j = (seed as usize) % (i + 1);
        tunes.swap(i, j);
    }
}

fn start_music(mut tunes: Vec<Tune>) -> (mpsc::Sender<MusicCmd>, Arc<Mutex<MusicState>>) {
    shuffle(&mut tunes);
    let (tx, rx) = mpsc::channel();
    let state = Arc::new(Mutex::new(MusicState {
        current_track: 0,
        track_count: tunes.len(),
        track_name: tunes.first().map_or("", |t| t.name).to_string(),
        playing: true,
        volume: 1.0,
        muted: false,
    }));
    let st = Arc::clone(&state);

    std::thread::spawn(move || {
        let (_stream, handle) = match OutputStream::try_default() {
            Ok(s) => s,
            Err(e) => {
                dbg_log!("[music] failed to open audio: {e}");
                return;
            }
        };

        let mut sink = make_sink(&handle, tunes[0].module, 1.0);

        loop {
            match rx.recv_timeout(Duration::from_millis(500)) {
                Ok(cmd) => {
                    let mut s = st.lock().unwrap();
                    match cmd {
                        MusicCmd::PlayPause => {
                            if s.playing {
                                sink.pause();
                                s.playing = false;
                            } else {
                                sink.play();
                                s.playing = true;
                            }
                        }
                        MusicCmd::Next => {
                            s.current_track = (s.current_track + 1) % tunes.len();
                            s.track_name = tunes[s.current_track].name.to_string();
                            s.playing = true;
                            let vol = if s.muted { 0.0 } else { s.volume };
                            drop(sink);
                            sink = make_sink(&handle, tunes[s.current_track].module, vol);
                        }
                        MusicCmd::Prev => {
                            s.current_track = if s.current_track == 0 {
                                tunes.len() - 1
                            } else {
                                s.current_track - 1
                            };
                            s.track_name = tunes[s.current_track].name.to_string();
                            s.playing = true;
                            let vol = if s.muted { 0.0 } else { s.volume };
                            drop(sink);
                            sink = make_sink(&handle, tunes[s.current_track].module, vol);
                        }
                        MusicCmd::SetVolume(v) => {
                            s.volume = v.clamp(0.0, 1.0);
                            if !s.muted {
                                sink.set_volume(s.volume);
                            }
                        }
                        MusicCmd::ToggleMute => {
                            s.muted = !s.muted;
                            sink.set_volume(if s.muted { 0.0 } else { s.volume });
                        }
                    }
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    if sink.empty() {
                        let mut s = st.lock().unwrap();
                        s.current_track = (s.current_track + 1) % tunes.len();
                        s.track_name = tunes[s.current_track].name.to_string();
                        let vol = if s.muted { 0.0 } else { s.volume };
                        drop(sink);
                        sink = make_sink(&handle, tunes[s.current_track].module, vol);
                    }
                }
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
            }
        }
    });

    (tx, state)
}

fn handle_music(
    path: &str,
    query: &str,
    tx: &mpsc::Sender<MusicCmd>,
    state: &Arc<Mutex<MusicState>>,
) -> String {
    match path {
        "/music/status" => {
            let s = state.lock().unwrap();
            format!(
                r#"{{"track":"{}","index":{},"count":{},"playing":{},"volume":{},"muted":{}}}"#,
                s.track_name, s.current_track, s.track_count, s.playing, s.volume, s.muted
            )
        }
        "/music/playpause" => {
            let _ = tx.send(MusicCmd::PlayPause);
            r#"{"ok":true}"#.into()
        }
        "/music/next" => {
            let _ = tx.send(MusicCmd::Next);
            r#"{"ok":true}"#.into()
        }
        "/music/prev" => {
            let _ = tx.send(MusicCmd::Prev);
            r#"{"ok":true}"#.into()
        }
        "/music/volume" => {
            let v: f32 = percent_decode(query).parse().unwrap_or(0.5);
            let _ = tx.send(MusicCmd::SetVolume(v));
            r#"{"ok":true}"#.into()
        }
        "/music/mute" => {
            let _ = tx.send(MusicCmd::ToggleMute);
            r#"{"ok":true}"#.into()
        }
        _ => r#"{"error":"unknown music endpoint"}"#.into(),
    }
}

fn build_webview(
    config: Arc<Mutex<RpcConfig>>,
    music_tx: mpsc::Sender<MusicCmd>,
    music_state: Arc<Mutex<MusicState>>,
    zmq_state: Arc<Mutex<ZmqState>>,
    zmq_handle: Arc<Mutex<Option<ZmqHandle>>>,
) -> wry::WebViewBuilder<'static> {
    let cfg = Arc::clone(&config);
    wry::WebViewBuilder::new()
        .with_asynchronous_custom_protocol("app".into(), move |_id, req, responder| {
            let path = req.uri().path().to_string();
            let query = req.uri().query().unwrap_or("").to_string();

            dbg_log!("[proto] {} path={path} query={}b", req.method(), query.len());

            if path == "/rpc" {
                let body = percent_decode(&query);
                dbg_log!("[proto] /rpc body: {:?}", &body[..body.len().min(200)]);
                let cfg = Arc::clone(&cfg);
                std::thread::spawn(move || {
                    let result = do_rpc(&body, &cfg);
                    dbg_log!("[proto] /rpc response: {} bytes", result.len());
                    responder.respond(json_response(&result));
                });
                return;
            }

            if path == "/config" {
                let body = percent_decode(&query);
                dbg_log!("[proto] /config body: {:?}", redact_password(&body));
                let zmq_changed = update_config(&body, &cfg);
                if zmq_changed {
                    let mut handle = zmq_handle.lock().unwrap();
                    if let Some(h) = handle.take() {
                        stop_zmq_subscriber(h);
                    }
                    let addr = cfg.lock().unwrap().zmq_address.clone();
                    if !addr.is_empty() {
                        *handle = Some(start_zmq_subscriber(&addr, Arc::clone(&zmq_state)));
                    }
                }
                responder.respond(json_response(r#"{"ok":true}"#));
                return;
            }

            if path == "/zmq/messages" {
                let s = zmq_state.lock().unwrap();
                let messages: Vec<serde_json::Value> = s.messages.iter().map(|m| {
                    serde_json::json!({
                        "topic": m.topic,
                        "body_hex": m.body_hex,
                        "body_size": m.body_size,
                        "sequence": m.sequence,
                        "timestamp": m.timestamp,
                    })
                }).collect();
                let result = serde_json::json!({
                    "connected": s.connected,
                    "address": s.address,
                    "messages": messages,
                });
                responder.respond(json_response(&result.to_string()));
                return;
            }

            if path.starts_with("/music/") {
                let result = handle_music(&path, &query, &music_tx, &music_state);
                responder.respond(json_response(&result));
                return;
            }

            responder.respond(serve_asset(&path));
        })
        .with_devtools(cfg!(debug_assertions))
        .with_url("app://localhost/index.html")
}

// --- Linux: GTK windowing ---

#[cfg(target_os = "linux")]
fn main() {
    use gtk::prelude::*;
    use wry::WebViewBuilderExtUnix;

    // Work around WebKitGTK DMA-BUF renderer freeze on Wayland
    unsafe { std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1") };

    dbg_log!("[main] gtk::init");
    gtk::init().unwrap();

    let window = gtk::Window::new(gtk::WindowType::Toplevel);
    window.set_title("Bitcoin Core RPC");
    window.set_default_size(1200, 800);

    let vbox = gtk::Box::new(gtk::Orientation::Vertical, 0);
    window.add(&vbox);

    let config = Arc::new(Mutex::new(RpcConfig {
        url: "http://127.0.0.1:8332".into(),
        user: String::new(),
        password: String::new(),
        wallet: String::new(),
        zmq_address: String::new(),
    }));

    dbg_log!("[main] loading tunes");
    let tunes = load_tunes();
    dbg_log!("[main] loaded {} tunes", tunes.len());
    let (music_tx, music_state) = start_music(tunes);

    let zmq_state = Arc::new(Mutex::new(ZmqState {
        connected: false,
        address: String::new(),
        messages: VecDeque::new(),
    }));
    let zmq_handle = Arc::new(Mutex::new(None));

    dbg_log!("[main] building webview");
    let _webview = build_webview(config, music_tx, music_state, zmq_state, zmq_handle).build_gtk(&vbox).unwrap();
    dbg_log!("[main] webview built, showing window");

    window.connect_delete_event(|_, _| {
        gtk::main_quit();
        gtk::glib::Propagation::Stop
    });

    window.show_all();
    dbg_log!("[main] entering gtk::main");
    gtk::main();
    dbg_log!("[main] gtk::main returned");
}

// --- Non-Linux: winit windowing ---

#[cfg(not(target_os = "linux"))]
struct App {
    window: Option<winit::window::Window>,
    webview: Option<wry::WebView>,
    config: Arc<Mutex<RpcConfig>>,
    music_tx: mpsc::Sender<MusicCmd>,
    music_state: Arc<Mutex<MusicState>>,
    zmq_state: Arc<Mutex<ZmqState>>,
    zmq_handle: Arc<Mutex<Option<ZmqHandle>>>,
}

#[cfg(not(target_os = "linux"))]
impl winit::application::ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {
        let attrs =
            winit::window::Window::default_attributes().with_title("Bitcoin Core RPC");
        let window = event_loop.create_window(attrs).unwrap();
        let webview = build_webview(
            Arc::clone(&self.config),
            self.music_tx.clone(),
            Arc::clone(&self.music_state),
            Arc::clone(&self.zmq_state),
            Arc::clone(&self.zmq_handle),
        )
            .build(&window)
            .unwrap();
        self.window = Some(window);
        self.webview = Some(webview);
    }

    fn window_event(
        &mut self,
        event_loop: &winit::event_loop::ActiveEventLoop,
        _window_id: winit::window::WindowId,
        event: winit::event::WindowEvent,
    ) {
        if let winit::event::WindowEvent::CloseRequested = event {
            event_loop.exit();
        }
    }
}

#[cfg(not(target_os = "linux"))]
fn main() {
    let tunes = load_tunes();
    let (music_tx, music_state) = start_music(tunes);

    let event_loop = winit::event_loop::EventLoop::new().unwrap();
    let mut app = App {
        window: None,
        webview: None,
        config: Arc::new(Mutex::new(RpcConfig {
            url: "http://127.0.0.1:8332".into(),
            user: String::new(),
            password: String::new(),
            wallet: String::new(),
            zmq_address: String::new(),
        })),
        music_tx,
        music_state,
        zmq_state: Arc::new(Mutex::new(ZmqState {
            connected: false,
            address: String::new(),
            messages: VecDeque::new(),
        })),
        zmq_handle: Arc::new(Mutex::new(None)),
    };
    event_loop.run_app(&mut app).unwrap();
}
