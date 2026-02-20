use iced::widget::{button, column, container, row, text};
use iced::{Element, Fill};

use crate::app::message::Message;
use crate::app::state::{State, Tab};
use crate::ui::components;

pub fn view(state: &State) -> Element<'_, Message> {
    let nav = column![
        text("BITCOIN RPC // MCU")
            .size(21)
            .color(components::ACCENT),
        text("NODE CONTROL").size(11).color(components::MUTED),
        text("SECTOR NAV").size(10).color(components::AMBER),
        nav_button("DASHBOARD", Tab::Dashboard, state.active_tab),
        nav_button("RPC", Tab::Rpc, state.active_tab),
        nav_button("CONFIG", Tab::Config, state.active_tab),
    ]
    .spacing(8)
    .padding(16)
    .width(180);

    let content = match state.active_tab {
        Tab::Dashboard => crate::ui::dashboard::view(state),
        Tab::Rpc => crate::ui::rpc::view(state),
        Tab::Config => crate::ui::config::view(state),
    };

    container(
        row![
            container(nav)
                .style(components::panel_style())
                .width(250)
                .height(Fill),
            content
        ]
        .spacing(10)
        .height(Fill)
        .width(Fill),
    )
    .style(components::app_surface())
    .padding(14)
    .height(Fill)
    .width(Fill)
    .into()
}

fn nav_button(label: &'static str, tab: Tab, active_tab: Tab) -> Element<'static, Message> {
    button(text(format!("[{}]", label)))
        .width(Fill)
        .style(components::nav_button_style(tab == active_tab))
        .padding([8, 10])
        .on_press(Message::SelectTab(tab))
        .into()
}
