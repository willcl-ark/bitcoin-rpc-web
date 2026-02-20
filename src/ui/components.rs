use iced::widget::{button, column, container, text};
use iced::{Background, Border, Color, Element, Fill, Shadow, Theme};

use crate::app::message::Message;

pub const BG: Color = Color::from_rgb(0.06, 0.08, 0.11);
pub const PANEL: Color = Color::from_rgb(0.10, 0.13, 0.18);
pub const PANEL_ALT: Color = Color::from_rgb(0.13, 0.17, 0.23);
pub const ACCENT: Color = Color::from_rgb(0.16, 0.57, 0.84);
pub const ACCENT_ALT: Color = Color::from_rgb(0.08, 0.40, 0.63);
pub const TEXT: Color = Color::from_rgb(0.92, 0.95, 0.98);
pub const MUTED: Color = Color::from_rgb(0.66, 0.72, 0.80);

pub fn placeholder_card<'a>(title: &'a str, body: &'a str) -> Element<'a, Message> {
    container(column![text(title).size(24).color(TEXT), text(body).color(MUTED)].spacing(8))
        .padding(24)
        .width(Fill)
        .style(panel_style())
        .into()
}

pub fn app_surface() -> impl Fn(&Theme) -> container::Style {
    |_theme| container::Style {
        background: Some(Background::Color(BG)),
        text_color: Some(TEXT),
        ..container::Style::default()
    }
}

pub fn panel_style() -> impl Fn(&Theme) -> container::Style {
    |_theme| container::Style {
        background: Some(Background::Color(PANEL)),
        border: Border {
            radius: 12.0.into(),
            width: 1.0,
            color: Color::from_rgb(0.18, 0.23, 0.30),
        },
        shadow: Shadow {
            color: Color::from_rgba(0.0, 0.0, 0.0, 0.30),
            offset: iced::Vector::new(0.0, 4.0),
            blur_radius: 12.0,
        },
        text_color: Some(TEXT),
        ..container::Style::default()
    }
}

pub fn card_style() -> impl Fn(&Theme) -> container::Style {
    |_theme| container::Style {
        background: Some(Background::Color(PANEL_ALT)),
        border: Border {
            radius: 10.0.into(),
            width: 1.0,
            color: Color::from_rgb(0.22, 0.29, 0.38),
        },
        text_color: Some(TEXT),
        ..container::Style::default()
    }
}

pub fn nav_button_style(active: bool) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |_theme, status| {
        let (base, border) = if active {
            (ACCENT, Color::from_rgb(0.60, 0.85, 1.0))
        } else {
            (PANEL_ALT, Color::from_rgb(0.25, 0.33, 0.42))
        };
        let background = match status {
            button::Status::Hovered => {
                if active {
                    ACCENT
                } else {
                    Color::from_rgb(0.17, 0.22, 0.30)
                }
            }
            button::Status::Pressed => {
                if active {
                    ACCENT_ALT
                } else {
                    Color::from_rgb(0.12, 0.17, 0.24)
                }
            }
            button::Status::Disabled => Color::from_rgba(base.r, base.g, base.b, 0.5),
            _ => base,
        };

        button::Style {
            background: Some(Background::Color(background)),
            border: Border {
                radius: 9.0.into(),
                width: 1.0,
                color: border,
            },
            text_color: TEXT,
            shadow: Shadow::default(),
        }
    }
}

pub fn action_button_style() -> impl Fn(&Theme, button::Status) -> button::Style {
    |_theme, status| {
        let background = match status {
            button::Status::Hovered => Color::from_rgb(0.20, 0.62, 0.90),
            button::Status::Pressed => Color::from_rgb(0.10, 0.44, 0.70),
            button::Status::Disabled => Color::from_rgba(0.20, 0.62, 0.90, 0.45),
            _ => ACCENT,
        };
        button::Style {
            background: Some(Background::Color(background)),
            border: Border {
                radius: 8.0.into(),
                width: 0.0,
                color: Color::TRANSPARENT,
            },
            text_color: TEXT,
            shadow: Shadow::default(),
        }
    }
}
