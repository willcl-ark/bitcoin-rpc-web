use std::sync::{Arc, Mutex};

mod logging;
mod music;
mod protocol;
mod rpc;
mod rpc_limiter;
mod thread_pool;
mod zmq;

struct RuntimeTuning {
    rpc_threads: usize,
    zmq_poll_threads: usize,
}

fn bounded_from_env(var: &str, default: usize, min: usize, max: usize) -> usize {
    let parsed = std::env::var(var)
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(default);
    parsed.clamp(min, max)
}

fn runtime_tuning() -> RuntimeTuning {
    let cpus = std::thread::available_parallelism()
        .map(usize::from)
        .unwrap_or(2)
        .clamp(1, 64);
    let default_rpc = cpus.clamp(2, 8);
    let default_zmq_poll = (cpus / 2).clamp(1, 4);
    RuntimeTuning {
        rpc_threads: bounded_from_env("RPC_THREADS", default_rpc, 1, 64),
        zmq_poll_threads: bounded_from_env("ZMQ_POLL_THREADS", default_zmq_poll, 1, 32),
    }
}

struct AppContext {
    config: Arc<Mutex<rpc::RpcConfig>>,
    rpc_limiter: Arc<rpc_limiter::RpcLimiter>,
    rpc_pool: Arc<thread_pool::ThreadPool>,
    zmq_poll_pool: Arc<thread_pool::ThreadPool>,
    music_runtime: Arc<music::MusicRuntime>,
    zmq_state: Arc<zmq::ZmqSharedState>,
    zmq_handle: Arc<Mutex<Option<zmq::ZmqHandle>>>,
}

fn build_app_context(tuning: &RuntimeTuning) -> AppContext {
    AppContext {
        config: Arc::new(Mutex::new(rpc::RpcConfig::default())),
        rpc_limiter: rpc_limiter::RpcLimiter::new(tuning.rpc_threads),
        rpc_pool: thread_pool::ThreadPool::new(tuning.rpc_threads),
        zmq_poll_pool: thread_pool::ThreadPool::new(tuning.zmq_poll_threads),
        music_runtime: Arc::new(music::start_music()),
        zmq_state: Arc::new(zmq::ZmqSharedState::default()),
        zmq_handle: Arc::new(Mutex::new(None)),
    }
}

fn shutdown_zmq(zmq_handle: &Arc<Mutex<Option<zmq::ZmqHandle>>>) {
    let mut handle = zmq_handle.lock().unwrap();
    if let Some(h) = handle.take() {
        zmq::stop_zmq_subscriber(h);
    }
}

#[cfg(target_os = "linux")]
fn main() {
    use gtk::prelude::*;
    use wry::WebViewBuilderExtUnix;

    logging::init();
    let tuning = runtime_tuning();

    gtk::init().unwrap();

    let window = gtk::Window::new(gtk::WindowType::Toplevel);
    window.set_title("Bitcoin Core RPC");
    window.set_default_size(1200, 800);

    let vbox = gtk::Box::new(gtk::Orientation::Vertical, 0);
    window.add(&vbox);

    let app = build_app_context(&tuning);

    let _webview = protocol::build_webview(
        app.config,
        app.rpc_limiter,
        app.rpc_pool,
        app.zmq_poll_pool,
        app.music_runtime,
        app.zmq_state,
        Arc::clone(&app.zmq_handle),
    )
    .build_gtk(&vbox)
    .unwrap();

    let zmq_handle_for_shutdown = Arc::clone(&app.zmq_handle);
    window.connect_delete_event(move |_, _| {
        shutdown_zmq(&zmq_handle_for_shutdown);
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
    ctx: AppContext,
}

#[cfg(not(target_os = "linux"))]
impl winit::application::ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {
        let attrs = winit::window::Window::default_attributes().with_title("Bitcoin Core RPC");
        let window = event_loop.create_window(attrs).unwrap();
        let webview = protocol::build_webview(
            Arc::clone(&self.ctx.config),
            Arc::clone(&self.ctx.rpc_limiter),
            Arc::clone(&self.ctx.rpc_pool),
            Arc::clone(&self.ctx.zmq_poll_pool),
            Arc::clone(&self.ctx.music_runtime),
            Arc::clone(&self.ctx.zmq_state),
            Arc::clone(&self.ctx.zmq_handle),
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
            shutdown_zmq(&self.ctx.zmq_handle);
            event_loop.exit();
        }
    }
}

#[cfg(not(target_os = "linux"))]
fn main() {
    logging::init();
    let tuning = runtime_tuning();

    let event_loop = winit::event_loop::EventLoop::new().unwrap();
    let mut app = App {
        window: None,
        webview: None,
        ctx: build_app_context(&tuning),
    };
    event_loop.run_app(&mut app).unwrap();
    shutdown_zmq(&app.ctx.zmq_handle);
}
