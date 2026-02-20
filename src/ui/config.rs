use iced::Element;

use crate::app::message::Message;

pub fn view<'a>() -> Element<'a, Message> {
    crate::ui::components::placeholder_card(
        "Config",
        "Phase 0 placeholder: configuration form and persistence wiring pending.",
    )
}
