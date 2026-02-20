use iced::Element;

use crate::app::message::Message;

pub fn view<'a>() -> Element<'a, Message> {
    crate::ui::components::placeholder_card(
        "RPC",
        "Phase 0 placeholder: RPC method explorer and execution UI pending.",
    )
}
