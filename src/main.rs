mod app;
mod core;
mod logging;
mod rpc;
mod rpc_limiter;
mod thread_pool;
mod ui;
mod zmq;

#[cfg(feature = "audio")]
mod music;

fn main() -> iced::Result {
    logging::init();
    app::run()
}
