use iced::widget::{button, column, row, text};
use iced::{Element, Fill};

use crate::app::message::Message;
use crate::app::state::{State, Tab};

pub fn view(state: &State) -> Element<'_, Message> {
    let nav = column![
        nav_button("Dashboard", Tab::Dashboard),
        nav_button("RPC", Tab::Rpc),
        nav_button("Config", Tab::Config),
    ]
    .spacing(12)
    .padding(16)
    .width(180);

    let content = match state.active_tab {
        Tab::Dashboard => crate::ui::dashboard::view(),
        Tab::Rpc => crate::ui::rpc::view(state),
        Tab::Config => crate::ui::config::view(state),
    };

    row![nav, content]
        .spacing(8)
        .height(Fill)
        .width(Fill)
        .into()
}

fn nav_button(label: &'static str, tab: Tab) -> Element<'static, Message> {
    button(text(label))
        .width(Fill)
        .on_press(Message::SelectTab(tab))
        .into()
}
