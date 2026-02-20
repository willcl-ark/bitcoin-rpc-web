use iced::Task;
use iced::widget::text_input;

use crate::app::message::{KeyboardShortcut, Message};
use crate::app::state::{FocusField, State, Tab};

use super::rpc::handle_rpc;

pub fn handle_keyboard_shortcut(state: &mut State, shortcut: KeyboardShortcut) -> Task<Message> {
    if state.focused_input.is_some() && shortcut.is_character_shortcut() {
        return Task::none();
    }

    match shortcut {
        KeyboardShortcut::ToggleHelp => {
            state.shortcuts_visible = !state.shortcuts_visible;
            Task::none()
        }
        KeyboardShortcut::CloseHelp => {
            state.shortcuts_visible = false;
            Task::none()
        }
        KeyboardShortcut::SwitchToDashboard => {
            state.active_tab = Tab::Dashboard;
            state.focused_input = None;
            Task::none()
        }
        KeyboardShortcut::SwitchToRpc => {
            state.active_tab = Tab::Rpc;
            state.focused_input = None;
            Task::none()
        }
        KeyboardShortcut::SwitchToConfig => {
            state.active_tab = Tab::Config;
            state.focused_input = None;
            Task::none()
        }
        KeyboardShortcut::FocusNextInput => focus_input(state, false),
        KeyboardShortcut::FocusPrevInput => focus_input(state, true),
        KeyboardShortcut::ExecuteRpc => {
            if state.active_tab == Tab::Rpc && !state.rpc.execute_in_flight {
                handle_rpc(state, Message::RpcExecutePressed)
            } else {
                Task::none()
            }
        }
    }
}

fn focus_input(state: &mut State, reverse: bool) -> Task<Message> {
    let order: &[FocusField] = match state.active_tab {
        Tab::Rpc => {
            if state.rpc.batch_mode {
                &[FocusField::RpcSearch, FocusField::RpcBatch]
            } else {
                &[FocusField::RpcSearch, FocusField::RpcParams]
            }
        }
        Tab::Config => &[
            FocusField::ConfigUrl,
            FocusField::ConfigUser,
            FocusField::ConfigPassword,
            FocusField::ConfigWallet,
            FocusField::ConfigPollInterval,
            FocusField::ConfigZmqAddress,
            FocusField::ConfigZmqBufferLimit,
            FocusField::ConfigFontSize,
        ],
        Tab::Dashboard => &[],
    };

    if order.is_empty() {
        return Task::none();
    }

    let current_index = state
        .focused_input
        .and_then(|current| order.iter().position(|field| *field == current));

    let next_index = match (current_index, reverse) {
        (Some(i), false) => (i + 1) % order.len(),
        (Some(i), true) => (i + order.len() - 1) % order.len(),
        (None, false) => 0,
        (None, true) => order.len() - 1,
    };

    let next = order[next_index];
    state.focused_input = Some(next);

    let id = next.id();
    Task::batch([
        text_input::focus(id.clone()),
        text_input::move_cursor_to_end(id),
    ])
}
