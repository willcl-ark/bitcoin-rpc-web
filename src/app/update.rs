use iced::Task;

use crate::app::message::Message;
use crate::app::state::State;

pub fn update(state: &mut State, message: Message) -> Task<Message> {
    match message {
        Message::SelectTab(tab) => {
            state.active_tab = tab;
        }
    }

    Task::none()
}
