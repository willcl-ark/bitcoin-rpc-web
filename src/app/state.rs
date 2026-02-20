use std::sync::Arc;
use std::time::Instant;

use crate::core::config_store::ConfigStore;
use crate::core::dashboard_service::DashboardSnapshot;
use crate::core::rpc_client::{MAX_ZMQ_BUFFER_LIMIT, MIN_ZMQ_BUFFER_LIMIT, RpcClient, RpcConfig};
use crate::core::schema::SchemaIndex;
use crate::zmq::{ZmqHandle, ZmqSharedState, start_zmq_subscriber, stop_zmq_subscriber};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Tab {
    #[default]
    Dashboard,
    Rpc,
    Config,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DashboardPartialSet {
    MempoolOnly,
    ChainAndMempool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PeerSortField {
    Id,
    Address,
    Direction,
    ConnectionType,
    Ping,
}

#[derive(Debug, Clone)]
pub struct ZmqUiEvent {
    pub topic: String,
    pub event_hash: String,
    pub timestamp: u64,
}

#[derive(Debug, Clone)]
pub struct ConfigForm {
    pub url: String,
    pub user: String,
    pub password: String,
    pub wallet: String,
    pub poll_interval_secs: String,
    pub zmq_address: String,
    pub zmq_buffer_limit: String,
}

impl From<&RpcConfig> for ConfigForm {
    fn from(config: &RpcConfig) -> Self {
        Self {
            url: config.url.clone(),
            user: config.user.clone(),
            password: config.password.clone(),
            wallet: config.wallet.clone(),
            poll_interval_secs: config.poll_interval_secs.to_string(),
            zmq_address: config.zmq_address.clone(),
            zmq_buffer_limit: config.zmq_buffer_limit.to_string(),
        }
    }
}

pub struct State {
    pub active_tab: Tab,
    pub config_store: Option<ConfigStore>,
    pub config_store_path: Option<String>,
    pub config_store_error: Option<String>,
    pub config_form: ConfigForm,
    pub runtime_config: RpcConfig,
    pub rpc_client: RpcClient,
    pub zmq_state: Arc<ZmqSharedState>,
    pub zmq_handle: Option<ZmqHandle>,
    pub connect_in_flight: bool,
    pub save_in_flight: bool,
    pub config_status: Option<String>,
    pub config_error: Option<String>,
    pub schema_index: Option<SchemaIndex>,
    pub schema_error: Option<String>,
    pub rpc_search: String,
    pub rpc_selected_method: Option<String>,
    pub rpc_params_input: String,
    pub rpc_batch_mode: bool,
    pub rpc_batch_input: String,
    pub rpc_execute_in_flight: bool,
    pub rpc_response: Option<String>,
    pub rpc_error: Option<String>,
    pub dashboard_snapshot: Option<DashboardSnapshot>,
    pub dashboard_in_flight: bool,
    pub dashboard_error: Option<String>,
    pub dashboard_selected_peer_id: Option<i64>,
    pub dashboard_peer_sort: PeerSortField,
    pub dashboard_peer_sort_desc: bool,
    pub dashboard_pending_partial: Option<DashboardPartialSet>,
    pub dashboard_last_refresh_at: Option<Instant>,
    pub zmq_connected: bool,
    pub zmq_connected_address: String,
    pub zmq_last_cursor: u64,
    pub zmq_events_seen: u64,
    pub zmq_last_topic: Option<String>,
    pub zmq_last_event_at: Option<u64>,
    pub zmq_recent_events: Vec<ZmqUiEvent>,
}

impl State {
    pub fn new() -> Self {
        let mut config_store = None;
        let mut config_store_path = None;
        let mut config_store_error = None;

        let runtime_config = match ConfigStore::new() {
            Ok(store) => {
                config_store_path = Some(store.path().display().to_string());
                let loaded = match store.load() {
                    Ok(config) => config,
                    Err(error) => {
                        config_store_error = Some(format!("failed to load config: {error}"));
                        RpcConfig::default()
                    }
                };
                config_store = Some(store);
                loaded
            }
            Err(error) => {
                config_store_error = Some(format!("failed to resolve config path: {error}"));
                RpcConfig::default()
            }
        };

        let zmq = ZmqSharedState::default();
        zmq.state.lock().expect("zmq state lock").buffer_limit = runtime_config
            .zmq_buffer_limit
            .clamp(MIN_ZMQ_BUFFER_LIMIT, MAX_ZMQ_BUFFER_LIMIT);

        let (schema_index, schema_error) = match SchemaIndex::load_default() {
            Ok(index) => (Some(index), None),
            Err(error) => (None, Some(error)),
        };

        let mut state = Self {
            active_tab: Tab::default(),
            config_store,
            config_store_path,
            config_store_error,
            config_form: ConfigForm::from(&runtime_config),
            rpc_client: RpcClient::new(runtime_config.clone()),
            runtime_config,
            zmq_state: Arc::new(zmq),
            zmq_handle: None,
            connect_in_flight: false,
            save_in_flight: false,
            config_status: None,
            config_error: None,
            schema_index,
            schema_error,
            rpc_search: String::new(),
            rpc_selected_method: Some("getblockchaininfo".to_string()),
            rpc_params_input: "[]".to_string(),
            rpc_batch_mode: false,
            rpc_batch_input: "[]".to_string(),
            rpc_execute_in_flight: false,
            rpc_response: None,
            rpc_error: None,
            dashboard_snapshot: None,
            dashboard_in_flight: false,
            dashboard_error: None,
            dashboard_selected_peer_id: None,
            dashboard_peer_sort: PeerSortField::Id,
            dashboard_peer_sort_desc: false,
            dashboard_pending_partial: None,
            dashboard_last_refresh_at: None,
            zmq_connected: false,
            zmq_connected_address: String::new(),
            zmq_last_cursor: 0,
            zmq_events_seen: 0,
            zmq_last_topic: None,
            zmq_last_event_at: None,
            zmq_recent_events: Vec::new(),
        };

        let startup_zmq = state.runtime_config.zmq_address.trim().to_string();
        if !startup_zmq.is_empty() {
            state.zmq_handle = Some(start_zmq_subscriber(&startup_zmq, state.zmq_state.clone()));
        }

        state
    }
}

impl Default for State {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for State {
    fn drop(&mut self) {
        if let Some(handle) = self.zmq_handle.take() {
            stop_zmq_subscriber(handle);
        }
    }
}
