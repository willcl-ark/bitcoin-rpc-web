use std::sync::{Arc, Mutex};

mod logging;
mod music;
mod protocol;
mod rpc;
mod rpc_limiter;
mod zmq;

#[cfg(target_os = "linux")]
fn main() {
    use gtk::prelude::*;
    use wry::WebViewBuilderExtUnix;

    logging::init();

    // Work around WebKitGTK DMA-BUF renderer freeze on Wayland
    unsafe { std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1") };

    gtk::init().unwrap();

    let window = gtk::Window::new(gtk::WindowType::Toplevel);
    window.set_title("Bitcoin Core RPC");
    window.set_default_size(1200, 800);

    let vbox = gtk::Box::new(gtk::Orientation::Vertical, 0);
    window.add(&vbox);

    let config = Arc::new(Mutex::new(rpc::RpcConfig::default()));
    let rpc_limiter = rpc_limiter::RpcLimiter::new(8);
    let music_runtime = Arc::new(music::start_music());
    let zmq_state = Arc::new(Mutex::new(zmq::ZmqState::default()));
    let zmq_handle = Arc::new(Mutex::new(None));

    let _webview =
        protocol::build_webview(config, rpc_limiter, music_runtime, zmq_state, zmq_handle)
            .build_gtk(&vbox)
            .unwrap();

    window.connect_delete_event(|_, _| {
        gtk::main_quit();
        gtk::glib::Propagation::Stop
    });

    window.show_all();
    gtk::main();
}

#[cfg(not(target_os = "linux"))]
struct App {
    window: Option<winit::window::Window>,
    webview: Option<wry::WebView>,
    config: Arc<Mutex<rpc::RpcConfig>>,
    rpc_limiter: Arc<rpc_limiter::RpcLimiter>,
    music_runtime: Arc<music::MusicRuntime>,
    zmq_state: Arc<Mutex<zmq::ZmqState>>,
    zmq_handle: Arc<Mutex<Option<zmq::ZmqHandle>>>,
}

#[cfg(not(target_os = "linux"))]
impl winit::application::ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {
        let attrs = winit::window::Window::default_attributes().with_title("Bitcoin Core RPC");
        let window = event_loop.create_window(attrs).unwrap();
        let webview = protocol::build_webview(
            Arc::clone(&self.config),
            Arc::clone(&self.rpc_limiter),
            Arc::clone(&self.music_runtime),
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
    logging::init();

    let event_loop = winit::event_loop::EventLoop::new().unwrap();
    let mut app = App {
        window: None,
        webview: None,
        config: Arc::new(Mutex::new(rpc::RpcConfig::default())),
        rpc_limiter: rpc_limiter::RpcLimiter::new(8),
        music_runtime: Arc::new(music::start_music()),
        zmq_state: Arc::new(Mutex::new(zmq::ZmqState::default())),
        zmq_handle: Arc::new(Mutex::new(None)),
    };
    event_loop.run_app(&mut app).unwrap();
}
