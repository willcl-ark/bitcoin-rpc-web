use iced::widget::{button, column, container, row, text};
use iced::{Element, Fill};

use crate::app::message::Message;
use crate::app::state::{State, Tab, ThemeName};
use crate::ui::components::{self, ColorTheme};

pub fn view(state: &State) -> Element<'_, Message> {
    let fs = state.config.runtime.font_size;
    let colors = &state.colors;
    let mut theme_row = row![].spacing(3);
    for name in ThemeName::ALL {
        theme_row = theme_row.push(
            button(
                text(name.label())
                    .size(fs.saturating_sub(4))
                    .color(if *name == state.theme_name {
                        colors.accent
                    } else {
                        colors.fg_dim
                    }),
            )
            .style(components::utility_button_style(
                colors,
                *name == state.theme_name,
            ))
            .padding([1, 4])
            .on_press(Message::ThemeChanged(*name)),
        );
    }

    let nav = column![
        text("BITCOIN CORE").size(fs + 7).color(colors.accent),
        text("NODE CONTROL")
            .size(fs.saturating_sub(3))
            .color(colors.fg_dim),
        text("SECTOR NAV")
            .size(fs.saturating_sub(4))
            .color(colors.orange),
        nav_button(colors, "DASHBOARD", Tab::Dashboard, state.active_tab, fs),
        nav_button(colors, "RPC", Tab::Rpc, state.active_tab, fs),
        nav_button(colors, "CONFIG", Tab::Config, state.active_tab, fs),
        text("THEME")
            .size(fs.saturating_sub(4))
            .color(colors.orange),
        theme_row,
    ]
    .spacing(6)
    .padding(12)
    .width(180);

    let content = match state.active_tab {
        Tab::Dashboard => crate::ui::dashboard::view(state),
        Tab::Rpc => crate::ui::rpc::view(state),
        Tab::Config => crate::ui::config::view(state),
    };

    let main_area = if state.sidebar_visible {
        row![
            container(nav)
                .style(components::panel_style(colors))
                .width(235)
                .height(Fill),
            content
        ]
        .spacing(8)
        .height(Fill)
        .width(Fill)
    } else {
        row![content].height(Fill).width(Fill)
    };

    container(column![main_area, crate::ui::music_bar::view(state)].width(Fill))
        .style(components::app_surface(colors))
        .padding(10)
        .height(Fill)
        .width(Fill)
        .into()
}

fn nav_button(
    colors: &ColorTheme,
    label: &'static str,
    tab: Tab,
    active_tab: Tab,
    fs: u16,
) -> Element<'static, Message> {
    button(text(format!("[{}]", label)).size(fs))
        .width(Fill)
        .style(components::nav_button_style(colors, tab == active_tab))
        .padding([6, 8])
        .on_press(Message::SelectTab(tab))
        .into()
}
