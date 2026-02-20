use crate::app::state::{DashboardPartialSet, PeerSortField, Tab, ThemeName};
use crate::core::dashboard_service::{DashboardPartialUpdate, DashboardSnapshot};
use crate::core::rpc_client::RpcConfig;
use iced::widget::pane_grid;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyboardShortcut {
    ToggleHelp,
    CloseHelp,
    SwitchToDashboard,
    SwitchToRpc,
    SwitchToConfig,
    FocusNextInput,
    FocusPrevInput,
    ExecuteRpc,
}

impl KeyboardShortcut {
    pub fn is_character_shortcut(self) -> bool {
        matches!(
            self,
            Self::ToggleHelp
                | Self::SwitchToDashboard
                | Self::SwitchToRpc
                | Self::SwitchToConfig
        )
    }
}

#[derive(Debug, Clone)]
pub enum Message {
    ThemeChanged(ThemeName),
    SidebarTogglePressed,
    KeyboardShortcut(KeyboardShortcut),
    SelectTab(Tab),
    ConfigUrlChanged(String),
    ConfigUserChanged(String),
    ConfigPasswordChanged(String),
    ConfigWalletChanged(String),
    ConfigPollIntervalChanged(String),
    ConfigZmqAddressChanged(String),
    ConfigZmqBufferLimitChanged(String),
    ConfigFontSizeChanged(String),
    ConfigStartAudioPlayingChanged(bool),
    ConfigConnectPressed,
    ConfigConnectFinished(Result<RpcConfig, String>),
    ConfigReloadPressed,
    ConfigReloadFinished(Result<RpcConfig, String>),
    ConfigSavePressed,
    ConfigSaveFinished(Result<RpcConfig, String>),
    RpcSearchChanged(String),
    RpcCategoryToggled(String),
    RpcMethodSelected(String),
    RpcParamsChanged(String),
    RpcBatchModeToggled(bool),
    RpcBatchChanged(String),
    RpcExecutePressed,
    RpcExecuteFinished(Result<String, String>),
    DashboardTick,
    DashboardLoaded(u64, Result<DashboardSnapshot, String>),
    DashboardPeerSelected(i64),
    DashboardPeerDetailClosed,
    DashboardPeerSortPressed(PeerSortField),
    NetinfoLevelChanged(u8),
    DashboardPartialRefreshRequested(DashboardPartialSet),
    DashboardPartialLoaded(u64, Result<DashboardPartialUpdate, String>),
    DashboardPaneResized(pane_grid::ResizeEvent),
    ZmqPollTick,
    MusicPlayPause,
    MusicNext,
    MusicPrev,
    MusicSetVolume(f32),
    MusicToggleMute,
    MusicPollTick,
}
