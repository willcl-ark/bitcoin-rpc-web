use std::time::Duration;

use iced::time;

use crate::app::message::Message;
use crate::app::state::State;

pub fn subscriptions(state: &State) -> iced::Subscription<Message> {
    let interval_secs = state.config.runtime.poll_interval_secs.clamp(1, 3600);
    let mut subs = vec![
        time::every(Duration::from_secs(interval_secs)).map(|_| Message::DashboardTick),
        time::every(Duration::from_millis(300)).map(|_| Message::ZmqPollTick),
    ];
    if state.music.is_some() {
        subs.push(time::every(Duration::from_millis(500)).map(|_| Message::MusicPollTick));
    }
    iced::Subscription::batch(subs)
}
