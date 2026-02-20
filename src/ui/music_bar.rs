use iced::widget::{button, container, row, slider, text, Space};
use iced::{Background, Border, Element, Fill, Shadow, Theme};

use crate::app::message::Message;
use crate::app::state::State;
use crate::ui::components::{self, BORDER, MUTED, PANEL, TEXT};

pub fn view(state: &State) -> Element<'_, Message> {
    if state.music.is_none() {
        return Space::new(0, 0).into();
    }

    let snap = &state.music_snapshot;
    let fs = state.config.runtime.font_size;

    let prev = transport_button("|<", Message::MusicPrev, fs);
    let play_pause = transport_button(
        if snap.playing { "||" } else { ">" },
        Message::MusicPlayPause,
        fs,
    );
    let next = transport_button(">|", Message::MusicNext, fs);

    let track = text(&snap.track_name)
        .size(fs.saturating_sub(2))
        .color(MUTED);

    let vol_slider = slider(0.0..=1.0, snap.volume, Message::MusicSetVolume)
        .width(100)
        .step(0.01);

    let mute_label = if snap.muted { "M" } else { "V" };
    let mute_btn = button(text(mute_label).size(fs.saturating_sub(3)).color(TEXT))
        .style(components::utility_button_style(snap.muted))
        .padding([2, 6])
        .on_press(Message::MusicToggleMute);

    container(
        row![prev, play_pause, next, track, Space::with_width(Fill), vol_slider, mute_btn]
            .spacing(8)
            .align_y(iced::Alignment::Center),
    )
    .style(bar_style())
    .padding([4, 12])
    .width(Fill)
    .into()
}

fn transport_button(label: &str, msg: Message, fs: u16) -> Element<'_, Message> {
    button(text(label).size(fs.saturating_sub(2)).color(TEXT))
        .style(components::utility_button_style(false))
        .padding([2, 8])
        .on_press(msg)
        .into()
}

fn bar_style() -> impl Fn(&Theme) -> container::Style {
    |_theme| container::Style {
        background: Some(Background::Color(PANEL)),
        border: Border {
            radius: 0.0.into(),
            width: 1.0,
            color: BORDER,
        },
        text_color: Some(TEXT),
        shadow: Shadow::default(),
        ..container::Style::default()
    }
}
