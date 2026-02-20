mod app;
mod core;
mod logging;
mod ui;
mod zmq;

mod music;

fn main() -> iced::Result {
    logging::init();
    app::run()
}
