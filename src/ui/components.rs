use iced::theme::Palette;
use iced::widget::{button, checkbox, column, container, text, text_input};
use iced::{Background, Border, Color, Element, Fill, Shadow, Theme};

use crate::app::message::Message;

pub const BG: Color = Color::from_rgb(0.01, 0.03, 0.06);
pub const PANEL: Color = Color::from_rgb(0.02, 0.08, 0.12);
pub const PANEL_ALT: Color = Color::from_rgb(0.03, 0.11, 0.17);
pub const PANEL_ALT_2: Color = Color::from_rgb(0.04, 0.14, 0.22);
pub const ACCENT: Color = Color::from_rgb(0.18, 0.93, 0.98);
pub const ACCENT_ALT: Color = Color::from_rgb(0.04, 0.65, 0.84);
pub const AMBER: Color = Color::from_rgb(0.98, 0.69, 0.16);
pub const ERROR_RED: Color = Color::from_rgb(0.95, 0.32, 0.29);
pub const TEXT: Color = Color::from_rgb(0.81, 0.92, 0.98);
pub const MUTED: Color = Color::from_rgb(0.44, 0.63, 0.75);
pub const BORDER: Color = Color::from_rgb(0.11, 0.37, 0.48);
pub const BORDER_HOT: Color = Color::from_rgb(0.22, 0.86, 0.95);

pub fn placeholder_card<'a>(title: &'a str, body: &'a str) -> Element<'a, Message> {
    container(column![text(title).size(24).color(TEXT), text(body).color(MUTED)].spacing(8))
        .padding(24)
        .width(Fill)
        .style(panel_style())
        .into()
}

pub fn mission_theme() -> Theme {
    Theme::custom(
        "Mission Control".to_string(),
        Palette {
            background: BG,
            text: TEXT,
            primary: ACCENT,
            success: ACCENT_ALT,
            danger: ERROR_RED,
        },
    )
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
            radius: 0.0.into(),
            width: 1.0,
            color: BORDER_HOT,
        },
        shadow: Shadow::default(),
        text_color: Some(TEXT),
        ..container::Style::default()
    }
}

pub fn card_style() -> impl Fn(&Theme) -> container::Style {
    |_theme| container::Style {
        background: Some(Background::Color(PANEL_ALT)),
        border: Border {
            radius: 0.0.into(),
            width: 1.0,
            color: BORDER,
        },
        text_color: Some(TEXT),
        ..container::Style::default()
    }
}

pub fn nav_button_style(active: bool) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |_theme, status| {
        let (base, border) = if active {
            (Color::from_rgb(0.06, 0.23, 0.31), BORDER_HOT)
        } else {
            (PANEL_ALT, BORDER)
        };
        let background = match status {
            button::Status::Hovered => {
                if active {
                    Color::from_rgb(0.08, 0.29, 0.38)
                } else {
                    PANEL_ALT_2
                }
            }
            button::Status::Pressed => {
                if active {
                    Color::from_rgb(0.05, 0.18, 0.24)
                } else {
                    Color::from_rgb(0.03, 0.10, 0.15)
                }
            }
            button::Status::Disabled => Color::from_rgba(base.r, base.g, base.b, 0.5),
            _ => base,
        };

        button::Style {
            background: Some(Background::Color(background)),
            border: Border {
                radius: 0.0.into(),
                width: 1.0,
                color: border,
            },
            text_color: if active { ACCENT } else { TEXT },
            shadow: Shadow::default(),
        }
    }
}

pub fn row_button_style(selected: bool) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |_theme, status| {
        let (base, border, text_color) = if selected {
            (Color::from_rgb(0.06, 0.20, 0.28), BORDER_HOT, ACCENT)
        } else {
            (PANEL, BORDER, TEXT)
        };

        let background = match status {
            button::Status::Hovered => {
                if selected {
                    Color::from_rgb(0.08, 0.25, 0.34)
                } else {
                    PANEL_ALT
                }
            }
            button::Status::Pressed => {
                if selected {
                    Color::from_rgb(0.05, 0.16, 0.22)
                } else {
                    Color::from_rgb(0.02, 0.07, 0.11)
                }
            }
            button::Status::Disabled => Color::from_rgba(base.r, base.g, base.b, 0.6),
            _ => base,
        };

        button::Style {
            background: Some(Background::Color(background)),
            border: Border {
                radius: 0.0.into(),
                width: 1.0,
                color: border,
            },
            text_color,
            shadow: Shadow::default(),
        }
    }
}

pub fn utility_button_style(active: bool) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |_theme, status| {
        let edge = if active { BORDER_HOT } else { BORDER };
        let text_color = if active { ACCENT } else { MUTED };
        let background = match status {
            button::Status::Hovered => Color::from_rgb(0.04, 0.14, 0.20),
            button::Status::Pressed => Color::from_rgb(0.03, 0.10, 0.15),
            button::Status::Disabled => {
                Color::from_rgba(PANEL_ALT.r, PANEL_ALT.g, PANEL_ALT.b, 0.4)
            }
            _ => PANEL_ALT,
        };

        button::Style {
            background: Some(Background::Color(background)),
            border: Border {
                radius: 0.0.into(),
                width: 1.0,
                color: edge,
            },
            text_color,
            shadow: Shadow::default(),
        }
    }
}

pub fn action_button_style() -> impl Fn(&Theme, button::Status) -> button::Style {
    |_theme, status| {
        let background = match status {
            button::Status::Hovered => Color::from_rgb(1.0, 0.76, 0.30),
            button::Status::Pressed => Color::from_rgb(0.90, 0.58, 0.10),
            button::Status::Disabled => Color::from_rgba(AMBER.r, AMBER.g, AMBER.b, 0.4),
            _ => AMBER,
        };
        button::Style {
            background: Some(Background::Color(background)),
            border: Border {
                radius: 0.0.into(),
                width: 1.0,
                color: Color::from_rgb(0.30, 0.20, 0.04),
            },
            text_color: Color::from_rgb(0.06, 0.05, 0.02),
            shadow: Shadow::default(),
        }
    }
}

pub fn input_style() -> impl Fn(&Theme, text_input::Status) -> text_input::Style {
    |_theme, status| {
        let mut style = text_input::Style {
            background: Background::Color(PANEL_ALT),
            border: Border {
                radius: 0.0.into(),
                width: 1.0,
                color: BORDER,
            },
            icon: MUTED,
            placeholder: MUTED,
            value: TEXT,
            selection: Color::from_rgba(ACCENT.r, ACCENT.g, ACCENT.b, 0.35),
        };

        style.border.color = match status {
            text_input::Status::Focused => BORDER_HOT,
            text_input::Status::Hovered => ACCENT_ALT,
            text_input::Status::Disabled => Color::from_rgba(BORDER.r, BORDER.g, BORDER.b, 0.5),
            _ => BORDER,
        };

        if matches!(status, text_input::Status::Disabled) {
            style.background =
                Background::Color(Color::from_rgba(PANEL_ALT.r, PANEL_ALT.g, PANEL_ALT.b, 0.6));
            style.value = MUTED;
        }

        style
    }
}

pub fn checkbox_style() -> impl Fn(&Theme, checkbox::Status) -> checkbox::Style {
    |_theme, status| match status {
        checkbox::Status::Active { is_checked } => checkbox::Style {
            background: Background::Color(if is_checked { ACCENT_ALT } else { PANEL_ALT }),
            icon_color: BG,
            border: Border {
                radius: 0.0.into(),
                width: 1.0,
                color: if is_checked { ACCENT } else { BORDER },
            },
            text_color: Some(TEXT),
        },
        checkbox::Status::Hovered { is_checked } => checkbox::Style {
            background: Background::Color(if is_checked { ACCENT } else { PANEL_ALT_2 }),
            icon_color: BG,
            border: Border {
                radius: 0.0.into(),
                width: 1.0,
                color: if is_checked { BORDER_HOT } else { ACCENT_ALT },
            },
            text_color: Some(TEXT),
        },
        checkbox::Status::Disabled { is_checked } => checkbox::Style {
            background: Background::Color(if is_checked {
                Color::from_rgba(ACCENT_ALT.r, ACCENT_ALT.g, ACCENT_ALT.b, 0.4)
            } else {
                Color::from_rgba(PANEL_ALT.r, PANEL_ALT.g, PANEL_ALT.b, 0.5)
            }),
            icon_color: MUTED,
            border: Border {
                radius: 0.0.into(),
                width: 1.0,
                color: Color::from_rgba(BORDER.r, BORDER.g, BORDER.b, 0.5),
            },
            text_color: Some(MUTED),
        },
    }
}
