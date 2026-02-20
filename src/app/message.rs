use crate::app::state::{DashboardPartialSet, PeerSortField, Tab};
use crate::core::dashboard_service::{DashboardPartialUpdate, DashboardSnapshot};
use crate::core::rpc_client::RpcConfig;

#[derive(Debug, Clone)]
pub enum Message {
    SelectTab(Tab),
    ConfigUrlChanged(String),
    ConfigUserChanged(String),
    ConfigPasswordChanged(String),
    ConfigWalletChanged(String),
    ConfigPollIntervalChanged(String),
    ConfigZmqAddressChanged(String),
    ConfigZmqBufferLimitChanged(String),
    ConfigConnectPressed,
    ConfigConnectFinished(Result<RpcConfig, String>),
    ConfigReloadPressed,
    ConfigReloadFinished(Result<RpcConfig, String>),
    ConfigSavePressed,
    ConfigSaveFinished(Result<RpcConfig, String>),
    RpcSearchChanged(String),
    RpcMethodSelected(String),
    RpcParamsChanged(String),
    RpcBatchModeToggled(bool),
    RpcBatchChanged(String),
    RpcExecutePressed,
    RpcExecuteFinished(Result<String, String>),
    DashboardTick,
    DashboardLoaded(Result<DashboardSnapshot, String>),
    DashboardPeerSelected(i64),
    DashboardPeerDetailClosed,
    DashboardPeerSortPressed(PeerSortField),
    DashboardPartialRefreshRequested(DashboardPartialSet),
    DashboardPartialLoaded(Result<DashboardPartialUpdate, String>),
    ZmqPollTick,
}
