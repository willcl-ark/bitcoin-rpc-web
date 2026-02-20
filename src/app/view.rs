use iced::widget::{button, column, container, row, text};
use iced::{Element, Fill};

use crate::app::message::Message;
use crate::app::state::{State, Tab};
use crate::ui::components;

pub fn view(state: &State) -> Element<'_, Message> {
    let nav = column![
        text("Bitcoin RPC").size(28).color(components::TEXT),
        text("Native desktop control plane")
            .size(14)
            .color(components::MUTED),
        nav_button("Dashboard", Tab::Dashboard, state.active_tab),
        nav_button("RPC", Tab::Rpc, state.active_tab),
        nav_button("Config", Tab::Config, state.active_tab),
    ]
    .spacing(12)
    .padding(20)
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
                .width(230)
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
    button(text(label))
        .width(Fill)
        .style(components::nav_button_style(tab == active_tab))
        .on_press(Message::SelectTab(tab))
        .into()
}
