use iced::widget::{column, container, text};
use iced::{Element, Fill};

use crate::app::message::Message;

pub fn placeholder_card<'a>(title: &'a str, body: &'a str) -> Element<'a, Message> {
    container(column![text(title).size(24), text(body)].spacing(8))
        .padding(24)
        .width(Fill)
        .into()
}
