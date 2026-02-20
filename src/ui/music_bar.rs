use iced::widget::{button, container, row, slider, text, Space};
use iced::{Background, Border, Element, Fill, Shadow, Theme};

use crate::app::message::Message;
use crate::app::state::State;
use crate::ui::components::{self, ColorTheme};

pub fn view(state: &State) -> Element<'_, Message> {
    let fs = state.config.runtime.font_size;
    let colors = &state.colors;
    let sidebar_label = if state.sidebar_visible {
        "[HIDE NAV]"
    } else {
        "[SHOW NAV]"
    };
    let sidebar_toggle = button(text(sidebar_label).size(fs.saturating_sub(3)).color(colors.fg))
        .style(components::utility_button_style(colors, false))
        .padding([2, 8])
        .on_press(Message::SidebarTogglePressed);

    let content: Element<'_, Message> = if let Some(_music) = &state.music {
        let snap = &state.music_snapshot;
        let prev = transport_button(colors, "|<", Message::MusicPrev, fs);
        let play_pause = transport_button(
            colors,
            if snap.playing { "||" } else { ">" },
            Message::MusicPlayPause,
            fs,
        );
        let next = transport_button(colors, ">|", Message::MusicNext, fs);

        let track = text(&snap.track_name)
            .size(fs.saturating_sub(2))
            .color(colors.fg_dim);

        let vol_slider = slider(0.0..=1.0, snap.volume, Message::MusicSetVolume)
            .width(100)
            .step(0.01);

        let mute_label = if snap.muted { "M" } else { "V" };
        let mute_btn = button(text(mute_label).size(fs.saturating_sub(3)).color(colors.fg))
            .style(components::utility_button_style(colors, snap.muted))
            .padding([2, 6])
            .on_press(Message::MusicToggleMute);

        row![
            sidebar_toggle,
            Space::with_width(12),
            prev,
            play_pause,
            next,
            track,
            Space::with_width(Fill),
            vol_slider,
            mute_btn
        ]
        .spacing(8)
        .align_y(iced::Alignment::Center)
        .into()
    } else {
        row![sidebar_toggle, Space::with_width(Fill)]
            .spacing(8)
            .align_y(iced::Alignment::Center)
            .into()
    };

    container(content)
        .style(bar_style(colors))
        .padding([4, 12])
        .width(Fill)
        .into()
}

fn transport_button<'a>(
    colors: &ColorTheme,
    label: &'a str,
    msg: Message,
    fs: u16,
) -> Element<'a, Message> {
    button(text(label).size(fs.saturating_sub(2)).color(colors.fg))
        .style(components::utility_button_style(colors, false))
        .padding([2, 8])
        .on_press(msg)
        .into()
}

fn bar_style(colors: &ColorTheme) -> impl Fn(&Theme) -> container::Style + 'static {
    let bg1 = colors.bg1;
    let border = colors.border;
    let fg = colors.fg;
    move |_theme| container::Style {
        background: Some(Background::Color(bg1)),
        border: Border {
            radius: 0.0.into(),
            width: 1.0,
            color: border,
        },
        text_color: Some(fg),
        shadow: Shadow::default(),
        ..container::Style::default()
    }
}
