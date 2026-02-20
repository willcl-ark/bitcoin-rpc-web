pub mod message;
pub mod state;
pub mod subscription;
pub mod update;
pub mod view;

pub fn run() -> iced::Result {
    iced::application("Bitcoin Core RPC", update::update, view::view)
        .run_with(|| (state::State::default(), iced::Task::none()))
}
