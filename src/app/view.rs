use iced::widget::{button, column, container, row, stack, text};
use iced::{Background, Border, Color, Element, Fill, Shadow, Theme};

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

    let base = container(column![main_area, crate::ui::music_bar::view(state)].width(Fill))
        .style(components::app_surface(colors))
        .padding(10)
        .height(Fill)
        .width(Fill);

    if state.shortcuts_visible {
        stack![
            base,
            shortcuts_overlay(state)
        ]
        .into()
    } else {
        base.into()
    }
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

fn shortcuts_overlay(state: &State) -> Element<'_, Message> {
    let fs = state.config.runtime.font_size;
    let colors = &state.colors;
    let panel = container(
        column![
            text("SHORTCUTS").size(fs + 6).color(colors.accent),
            text("?  toggle shortcuts").size(fs).color(colors.fg),
            text("d  dashboard").size(fs).color(colors.fg),
            text("r  rpc").size(fs).color(colors.fg),
            text("c  config").size(fs).color(colors.fg),
            text("tab  next input").size(fs).color(colors.fg),
            text("shift+tab  previous input").size(fs).color(colors.fg),
            text("enter  execute rpc (rpc tab)").size(fs).color(colors.fg),
            text("esc  close shortcuts").size(fs).color(colors.fg_dim),
        ]
        .spacing(6),
    )
    .padding(16)
    .width(360)
    .style(components::panel_style(colors));

    container(panel)
        .width(Fill)
        .height(Fill)
        .align_x(iced::alignment::Horizontal::Center)
        .align_y(iced::alignment::Vertical::Center)
        .style(move |_theme: &Theme| iced::widget::container::Style {
            background: Some(Background::Color(Color {
                a: 0.55,
                ..colors.bg
            })),
            border: Border::default(),
            text_color: None,
            shadow: Shadow::default(),
        })
        .into()
}
