use iced::Element;

use crate::app::message::Message;

pub fn view<'a>() -> Element<'a, Message> {
    crate::ui::components::placeholder_card(
        "Dashboard",
        "Phase 0 placeholder: dashboard UI arrives in later migration stages.",
    )
}
