use std::time::Duration;

use iced::time;

use crate::app::message::Message;
use crate::app::state::State;

pub fn subscriptions(state: &State) -> iced::Subscription<Message> {
    let interval_secs = state.runtime_config.poll_interval_secs.clamp(1, 3600);
    time::every(Duration::from_secs(interval_secs)).map(|_| Message::DashboardTick)
}
