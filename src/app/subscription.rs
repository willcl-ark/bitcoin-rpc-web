use std::time::Duration;

use iced::keyboard::{self, Key, Modifiers, key::Named};
use iced::time;

use crate::app::message::{KeyboardShortcut, Message};
use crate::app::state::State;

pub fn subscriptions(state: &State) -> iced::Subscription<Message> {
    let interval_secs = state.config.runtime.poll_interval_secs.clamp(1, 3600);
    let mut subs = vec![
        time::every(Duration::from_secs(interval_secs)).map(|_| Message::DashboardTick),
        time::every(Duration::from_millis(300)).map(|_| Message::ZmqPollTick),
    ];
    if state.music.is_some() {
        subs.push(time::every(Duration::from_millis(500)).map(|_| Message::MusicPollTick));
    }
    subs.push(keyboard::on_key_press(map_key_press));
    iced::Subscription::batch(subs)
}

fn map_key_press(key: Key, modifiers: Modifiers) -> Option<Message> {
    let no_meta = !modifiers.control() && !modifiers.alt() && !modifiers.logo();
    if !no_meta {
        return None;
    }

    let shortcut = match key.as_ref() {
        Key::Character("?") => KeyboardShortcut::ToggleHelp,
        Key::Character("d") | Key::Character("D") if !modifiers.shift() => {
            KeyboardShortcut::SwitchToDashboard
        }
        Key::Character("r") | Key::Character("R") if !modifiers.shift() => {
            KeyboardShortcut::SwitchToRpc
        }
        Key::Character("c") | Key::Character("C") if !modifiers.shift() => {
            KeyboardShortcut::SwitchToConfig
        }
        Key::Named(Named::Tab) if modifiers.shift() => KeyboardShortcut::FocusPrevInput,
        Key::Named(Named::Tab) => KeyboardShortcut::FocusNextInput,
        Key::Named(Named::Enter) if !modifiers.shift() => KeyboardShortcut::ExecuteRpc,
        Key::Named(Named::Escape) => KeyboardShortcut::CloseHelp,
        _ => return None,
    };

    Some(Message::KeyboardShortcut(shortcut))
}
