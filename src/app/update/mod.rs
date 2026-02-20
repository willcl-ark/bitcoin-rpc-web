mod config;
mod dashboard;
mod keyboard;
mod music;
mod rpc;
mod zmq;

use iced::Task;

use crate::app::message::Message;
use crate::app::state::State;
use crate::core::rpc_client::{RpcConfig, allow_insecure, is_safe_rpc_host};

const UNSAFE_HOST_ERROR: &str = "RPC URL must be localhost/private unless DANGER_INSECURE_RPC=1";

pub fn validate_rpc_host(config: &RpcConfig) -> Result<(), String> {
    if is_safe_rpc_host(&config.url) || allow_insecure() {
        Ok(())
    } else {
        Err(UNSAFE_HOST_ERROR.to_string())
    }
}

pub fn update(state: &mut State, message: Message) -> Task<Message> {
    match message {
        Message::ThemeChanged(name) => {
            state.theme_name = name;
            state.colors = name.colors();
            Task::none()
        }
        Message::SidebarTogglePressed => {
            state.sidebar_visible = !state.sidebar_visible;
            Task::none()
        }
        Message::KeyboardShortcut(shortcut) => {
            keyboard::handle_keyboard_shortcut(state, shortcut)
        }
        Message::SelectTab(tab) => {
            state.active_tab = tab;
            state.focused_input = None;
            Task::none()
        }

        Message::ConfigUrlChanged(..)
        | Message::ConfigUserChanged(..)
        | Message::ConfigPasswordChanged(..)
        | Message::ConfigWalletChanged(..)
        | Message::ConfigPollIntervalChanged(..)
        | Message::ConfigZmqAddressChanged(..)
        | Message::ConfigZmqBufferLimitChanged(..)
        | Message::ConfigFontSizeChanged(..)
        | Message::ConfigStartAudioPlayingChanged(..)
        | Message::ConfigConnectPressed
        | Message::ConfigConnectFinished(..)
        | Message::ConfigReloadPressed
        | Message::ConfigReloadFinished(..)
        | Message::ConfigSavePressed
        | Message::ConfigSaveFinished(..) => config::handle_config(state, message),

        Message::RpcSearchChanged(..)
        | Message::RpcCategoryToggled(..)
        | Message::RpcMethodSelected(..)
        | Message::RpcParamsChanged(..)
        | Message::RpcBatchModeToggled(..)
        | Message::RpcBatchChanged(..)
        | Message::RpcExecutePressed
        | Message::RpcExecuteFinished(..) => rpc::handle_rpc(state, message),

        Message::DashboardTick
        | Message::DashboardLoaded(..)
        | Message::DashboardPeerSelected(..)
        | Message::DashboardPeerDetailClosed
        | Message::DashboardPeerSortPressed(..)
        | Message::NetinfoLevelChanged(..)
        | Message::DashboardPartialRefreshRequested(..)
        | Message::DashboardPartialLoaded(..)
        | Message::DashboardPaneResized(..) => dashboard::handle_dashboard(state, message),

        Message::ZmqPollTick => zmq::handle_zmq(state),

        Message::MusicPlayPause
        | Message::MusicNext
        | Message::MusicPrev
        | Message::MusicSetVolume(..)
        | Message::MusicToggleMute
        | Message::MusicPollTick => music::handle_music(state, message),
    }
}
