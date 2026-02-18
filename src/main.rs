use std::borrow::Cow;
use std::sync::{Arc, Mutex};

use wry::http::header::{ACCESS_CONTROL_ALLOW_ORIGIN, CONTENT_TYPE};
use wry::http::Response;

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

fn update_config(body: &str, config: &Arc<Mutex<RpcConfig>>) {
    dbg_log!("[config] body: {:?}", redact_password(body));
    let msg: serde_json::Value = match serde_json::from_str(body) {
        Ok(v) => v,
        Err(e) => {
            dbg_log!("[config] parse error: {e}");
            return;
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
    dbg_log!("[config] updated: url={:?} user={:?} wallet={:?}", cfg.url, cfg.user, cfg.wallet);
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

fn build_webview(config: Arc<Mutex<RpcConfig>>) -> wry::WebViewBuilder<'static> {
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
                update_config(&body, &cfg);
                responder.respond(json_response(r#"{"ok":true}"#));
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
    }));

    dbg_log!("[main] building webview");
    let _webview = build_webview(config).build_gtk(&vbox).unwrap();
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
}

#[cfg(not(target_os = "linux"))]
impl winit::application::ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {
        let attrs =
            winit::window::Window::default_attributes().with_title("Bitcoin Core RPC");
        let window = event_loop.create_window(attrs).unwrap();
        let webview = build_webview(Arc::clone(&self.config))
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
    let event_loop = winit::event_loop::EventLoop::new().unwrap();
    let mut app = App {
        window: None,
        webview: None,
        config: Arc::new(Mutex::new(RpcConfig {
            url: "http://127.0.0.1:8332".into(),
            user: String::new(),
            password: String::new(),
            wallet: String::new(),
        })),
    };
    event_loop.run_app(&mut app).unwrap();
}
