use iced::theme::Palette;
use iced::widget::{button, checkbox, container, text_input};
use iced::{Background, Border, Color, Shadow, Theme};

pub struct ColorTheme {
    pub bg: Color,
    pub bg1: Color,
    pub bg2: Color,
    pub bg3: Color,
    pub fg: Color,
    pub fg_dim: Color,
    pub border: Color,
    pub border_focus: Color,
    pub accent: Color,
    pub red: Color,
    pub orange: Color,
    pub yellow: Color,
    pub green: Color,
    pub blue: Color,
    pub cyan: Color,
    pub purple: Color,
}

impl Default for ColorTheme {
    fn default() -> Self {
        Self {
            bg: Color::from_rgb(0.01, 0.03, 0.06),
            bg1: Color::from_rgb(0.02, 0.08, 0.12),
            bg2: Color::from_rgb(0.03, 0.11, 0.17),
            bg3: Color::from_rgb(0.04, 0.14, 0.22),
            fg: Color::from_rgb(0.81, 0.92, 0.98),
            fg_dim: Color::from_rgb(0.44, 0.63, 0.75),
            border: Color::from_rgb(0.08, 0.27, 0.36),
            border_focus: Color::from_rgb(0.14, 0.64, 0.74),
            accent: Color::from_rgb(0.18, 0.93, 0.98),
            red: Color::from_rgb(0.95, 0.32, 0.29),
            orange: Color::from_rgb(0.98, 0.69, 0.16),
            yellow: Color::from_rgb(0.96, 0.79, 0.27),
            green: Color::from_rgb(0.20, 0.84, 0.46),
            blue: Color::from_rgb(0.04, 0.65, 0.84),
            cyan: Color::from_rgb(0.18, 0.93, 0.98),
            purple: Color::from_rgb(0.44, 0.63, 0.75),
        }
    }
}

impl ColorTheme {
    pub fn to_iced_theme(&self) -> Theme {
        Theme::custom(
            "Mission Control".to_string(),
            Palette {
                background: self.bg,
                text: self.fg,
                primary: self.accent,
                success: self.blue,
                danger: self.red,
            },
        )
    }
}

pub fn with_alpha(c: Color, a: f32) -> Color {
    Color::from_rgba(c.r, c.g, c.b, a)
}

pub fn lighten(c: Color, amount: f32) -> Color {
    Color::from_rgb(
        (c.r + amount).min(1.0),
        (c.g + amount).min(1.0),
        (c.b + amount).min(1.0),
    )
}

pub fn darken(c: Color, amount: f32) -> Color {
    Color::from_rgb(
        (c.r - amount).max(0.0),
        (c.g - amount).max(0.0),
        (c.b - amount).max(0.0),
    )
}

pub fn app_surface(colors: &ColorTheme) -> impl Fn(&Theme) -> container::Style + 'static {
    let bg = colors.bg;
    let fg = colors.fg;
    move |_theme| container::Style {
        background: Some(Background::Color(bg)),
        text_color: Some(fg),
        ..container::Style::default()
    }
}

pub fn panel_style(colors: &ColorTheme) -> impl Fn(&Theme) -> container::Style + 'static {
    let bg1 = colors.bg1;
    let border_focus = colors.border_focus;
    let fg = colors.fg;
    move |_theme| container::Style {
        background: Some(Background::Color(bg1)),
        border: Border {
            radius: 0.0.into(),
            width: 1.0,
            color: border_focus,
        },
        shadow: Shadow::default(),
        text_color: Some(fg),
        ..container::Style::default()
    }
}

pub fn card_style(colors: &ColorTheme) -> impl Fn(&Theme) -> container::Style + 'static {
    let bg2 = colors.bg2;
    let border = colors.border;
    let fg = colors.fg;
    move |_theme| container::Style {
        background: Some(Background::Color(bg2)),
        border: Border {
            radius: 0.0.into(),
            width: 1.0,
            color: border,
        },
        text_color: Some(fg),
        ..container::Style::default()
    }
}

pub fn nav_button_style(
    colors: &ColorTheme,
    active: bool,
) -> impl Fn(&Theme, button::Status) -> button::Style + 'static {
    let bg2 = colors.bg2;
    let bg3 = colors.bg3;
    let border = colors.border;
    let border_focus = colors.border_focus;
    let accent = colors.accent;
    let fg = colors.fg;
    move |_theme, status| {
        let (base, edge) = if active {
            (lighten(bg3, 0.04), border_focus)
        } else {
            (bg2, border)
        };
        let background = match status {
            button::Status::Hovered => {
                if active {
                    lighten(bg3, 0.08)
                } else {
                    bg3
                }
            }
            button::Status::Pressed => {
                if active {
                    darken(bg3, 0.02)
                } else {
                    darken(bg2, 0.02)
                }
            }
            button::Status::Disabled => with_alpha(base, 0.5),
            _ => base,
        };

        button::Style {
            background: Some(Background::Color(background)),
            border: Border {
                radius: 0.0.into(),
                width: 1.0,
                color: edge,
            },
            text_color: if active { accent } else { fg },
            shadow: Shadow::default(),
        }
    }
}

pub fn row_button_style(
    colors: &ColorTheme,
    selected: bool,
) -> impl Fn(&Theme, button::Status) -> button::Style + 'static {
    let blue = colors.blue;
    let border_focus = colors.border_focus;
    let accent = colors.accent;
    let fg = colors.fg;
    move |_theme, status| {
        let (base, border, border_width, text_color) = if selected {
            (with_alpha(blue, 0.16), border_focus, 1.0, accent)
        } else {
            (Color::TRANSPARENT, Color::TRANSPARENT, 0.0, fg)
        };

        let background = match status {
            button::Status::Hovered => {
                if selected {
                    with_alpha(blue, 0.22)
                } else {
                    with_alpha(blue, 0.10)
                }
            }
            button::Status::Pressed => {
                if selected {
                    with_alpha(blue, 0.28)
                } else {
                    with_alpha(blue, 0.14)
                }
            }
            button::Status::Disabled => with_alpha(base, 0.6),
            _ => base,
        };

        button::Style {
            background: Some(Background::Color(background)),
            border: Border {
                radius: 0.0.into(),
                width: border_width,
                color: border,
            },
            text_color,
            shadow: Shadow::default(),
        }
    }
}

pub fn utility_button_style(
    colors: &ColorTheme,
    active: bool,
) -> impl Fn(&Theme, button::Status) -> button::Style + 'static {
    let bg2 = colors.bg2;
    let bg3 = colors.bg3;
    let border = colors.border;
    let border_focus = colors.border_focus;
    let accent = colors.accent;
    let fg_dim = colors.fg_dim;
    move |_theme, status| {
        let edge = if active { border_focus } else { border };
        let text_color = if active { accent } else { fg_dim };
        let background = match status {
            button::Status::Hovered => bg3,
            button::Status::Pressed => darken(bg2, 0.02),
            button::Status::Disabled => with_alpha(bg2, 0.4),
            _ => bg2,
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

pub fn table_header_button_style(
    colors: &ColorTheme,
    active: bool,
) -> impl Fn(&Theme, button::Status) -> button::Style + 'static {
    let blue = colors.blue;
    let border = colors.border;
    let accent = colors.accent;
    let fg_dim = colors.fg_dim;
    move |_theme, status| {
        let base_bg = if active {
            with_alpha(blue, 0.10)
        } else {
            Color::TRANSPARENT
        };
        let background = match status {
            button::Status::Hovered => with_alpha(blue, 0.14),
            button::Status::Pressed => with_alpha(blue, 0.20),
            button::Status::Disabled => with_alpha(base_bg, 0.6),
            _ => base_bg,
        };

        button::Style {
            background: Some(Background::Color(background)),
            border: Border {
                radius: 0.0.into(),
                width: if active { 1.0 } else { 0.0 },
                color: if active { border } else { Color::TRANSPARENT },
            },
            text_color: if active { accent } else { fg_dim },
            shadow: Shadow::default(),
        }
    }
}

pub fn action_button_style(
    colors: &ColorTheme,
) -> impl Fn(&Theme, button::Status) -> button::Style + 'static {
    let orange = colors.orange;
    let bg = colors.bg;
    move |_theme, status| {
        let background = match status {
            button::Status::Hovered => lighten(orange, 0.07),
            button::Status::Pressed => darken(orange, 0.08),
            button::Status::Disabled => with_alpha(orange, 0.4),
            _ => orange,
        };
        button::Style {
            background: Some(Background::Color(background)),
            border: Border {
                radius: 0.0.into(),
                width: 1.0,
                color: darken(orange, 0.68),
            },
            text_color: bg,
            shadow: Shadow::default(),
        }
    }
}

pub fn input_style(
    colors: &ColorTheme,
) -> impl Fn(&Theme, text_input::Status) -> text_input::Style + 'static {
    let bg2 = colors.bg2;
    let border = colors.border;
    let border_focus = colors.border_focus;
    let blue = colors.blue;
    let fg_dim = colors.fg_dim;
    let fg = colors.fg;
    let accent = colors.accent;
    move |_theme, status| {
        let mut style = text_input::Style {
            background: Background::Color(bg2),
            border: Border {
                radius: 0.0.into(),
                width: 1.0,
                color: border,
            },
            icon: fg_dim,
            placeholder: fg_dim,
            value: fg,
            selection: with_alpha(accent, 0.35),
        };

        style.border.color = match status {
            text_input::Status::Focused => border_focus,
            text_input::Status::Hovered => blue,
            text_input::Status::Disabled => with_alpha(border, 0.5),
            _ => border,
        };

        if matches!(status, text_input::Status::Disabled) {
            style.background = Background::Color(with_alpha(bg2, 0.6));
            style.value = fg_dim;
        }

        style
    }
}

pub fn checkbox_style(
    colors: &ColorTheme,
) -> impl Fn(&Theme, checkbox::Status) -> checkbox::Style + 'static {
    let bg = colors.bg;
    let bg2 = colors.bg2;
    let bg3 = colors.bg3;
    let border = colors.border;
    let border_focus = colors.border_focus;
    let blue = colors.blue;
    let accent = colors.accent;
    let fg = colors.fg;
    let fg_dim = colors.fg_dim;
    move |_theme, status| match status {
        checkbox::Status::Active { is_checked } => checkbox::Style {
            background: Background::Color(if is_checked { blue } else { bg2 }),
            icon_color: bg,
            border: Border {
                radius: 0.0.into(),
                width: 1.0,
                color: if is_checked { accent } else { border },
            },
            text_color: Some(fg),
        },
        checkbox::Status::Hovered { is_checked } => checkbox::Style {
            background: Background::Color(if is_checked { accent } else { bg3 }),
            icon_color: bg,
            border: Border {
                radius: 0.0.into(),
                width: 1.0,
                color: if is_checked { border_focus } else { blue },
            },
            text_color: Some(fg),
        },
        checkbox::Status::Disabled { is_checked } => checkbox::Style {
            background: Background::Color(if is_checked {
                with_alpha(blue, 0.4)
            } else {
                with_alpha(bg2, 0.5)
            }),
            icon_color: fg_dim,
            border: Border {
                radius: 0.0.into(),
                width: 1.0,
                color: with_alpha(border, 0.5),
            },
            text_color: Some(fg_dim),
        },
    }
}
