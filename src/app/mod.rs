pub mod constants;
pub mod message;
pub mod state;
pub mod subscription;
pub mod update;
pub mod view;

pub fn run() -> iced::Result {
    iced::application("Bitcoin Core RPC", update::update, view::view)
        .subscription(subscription::subscriptions)
        .default_font(iced::Font::MONOSPACE)
        .theme(|state| state.colors.to_iced_theme())
        .run_with(|| {
            (
                state::State::default(),
                iced::Task::perform(async {}, |_| message::Message::DashboardTick),
            )
        })
}
