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
    .spacing(6)
    .padding(12)
    .width(180);

    let content = match state.active_tab {
        Tab::Dashboard => crate::ui::dashboard::view(state),
        Tab::Rpc => crate::ui::rpc::view(state),
        Tab::Config => crate::ui::config::view(state),
    };

    let main_area = row![
        container(nav)
            .style(components::panel_style())
            .width(235)
            .height(Fill),
        content
    ]
    .spacing(8)
    .height(Fill)
    .width(Fill);

    container(
        column![main_area, crate::ui::music_bar::view(state)].width(Fill),
    )
    .style(components::app_surface())
    .padding(10)
    .height(Fill)
    .width(Fill)
    .into()
}

fn nav_button(label: &'static str, tab: Tab, active_tab: Tab) -> Element<'static, Message> {
    button(text(format!("[{}]", label)))
        .width(Fill)
        .style(components::nav_button_style(tab == active_tab))
        .padding([6, 8])
        .on_press(Message::SelectTab(tab))
        .into()
}
