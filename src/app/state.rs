use std::collections::BTreeSet;
use std::sync::Arc;
use std::time::Instant;

use iced::widget::pane_grid;

use crate::core::config_store::ConfigStore;
use crate::ui::components::ColorTheme;
use crate::core::dashboard_service::DashboardSnapshot;
use crate::core::rpc_client::{MAX_ZMQ_BUFFER_LIMIT, MIN_ZMQ_BUFFER_LIMIT, RpcClient, RpcConfig};
use crate::core::schema::SchemaIndex;
use crate::music::{MusicRuntime, MusicSnapshot};
use crate::zmq::{ZmqHandle, ZmqSharedState, start_zmq_subscriber, stop_zmq_subscriber};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ThemeName {
    #[default]
    MissionControl,
    Everforest,
    GruvboxMaterial,
    MaterialDeepOcean,
    Nord,
    OneDark,
}

impl ThemeName {
    pub const ALL: &[Self] = &[
        Self::MissionControl,
        Self::Everforest,
        Self::GruvboxMaterial,
        Self::MaterialDeepOcean,
        Self::Nord,
        Self::OneDark,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::MissionControl => "MC",
            Self::Everforest => "EF",
            Self::GruvboxMaterial => "GR",
            Self::MaterialDeepOcean => "MA",
            Self::Nord => "NO",
            Self::OneDark => "OD",
        }
    }

    pub fn colors(self) -> ColorTheme {
        match self {
            Self::MissionControl => ColorTheme::default(),
            Self::Everforest => ColorTheme::everforest(),
            Self::GruvboxMaterial => ColorTheme::gruvbox_material(),
            Self::MaterialDeepOcean => ColorTheme::material_deep_ocean(),
            Self::Nord => ColorTheme::nord(),
            Self::OneDark => ColorTheme::onedark(),
        }
    }
}

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PeerSortField {
    Id,
    Direction,
    ConnectionType,
    Network,
    #[default]
    MinPing,
    Ping,
    Age,
    Address,
    Version,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DashboardPane {
    Main,
    Zmq,
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
    pub font_size: String,
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
            font_size: config.font_size.to_string(),
        }
    }
}

pub struct ConfigState {
    pub store: Option<ConfigStore>,
    pub store_path: Option<String>,
    pub store_error: Option<String>,
    pub form: ConfigForm,
    pub runtime: RpcConfig,
    pub connect_in_flight: bool,
    pub save_in_flight: bool,
    pub status: Option<String>,
    pub error: Option<String>,
}

pub struct RpcState {
    pub client: RpcClient,
    pub schema: Option<SchemaIndex>,
    pub schema_error: Option<String>,
    pub search: String,
    pub collapsed_categories: BTreeSet<String>,
    pub selected_method: Option<String>,
    pub params_input: String,
    pub batch_mode: bool,
    pub batch_input: String,
    pub execute_in_flight: bool,
    pub response: Option<String>,
    pub error: Option<String>,
}

pub struct DashboardState {
    pub snapshot: Option<DashboardSnapshot>,
    pub in_flight: bool,
    pub error: Option<String>,
    pub selected_peer_id: Option<i64>,
    pub peer_sort: PeerSortField,
    pub peer_sort_desc: bool,
    pub pending_partial: Option<DashboardPartialSet>,
    pub last_refresh_at: Option<Instant>,
    pub netinfo_level: u8,
    pub panes: pane_grid::State<DashboardPane>,
}

pub struct ZmqViewState {
    pub connected: bool,
    pub connected_address: String,
    pub last_cursor: u64,
    pub events_seen: u64,
    pub last_topic: Option<String>,
    pub last_event_at: Option<u64>,
    pub recent_events: Vec<ZmqUiEvent>,
}

pub struct State {
    pub colors: ColorTheme,
    pub theme_name: ThemeName,
    pub sidebar_visible: bool,
    pub active_tab: Tab,
    pub config: ConfigState,
    pub rpc: RpcState,
    pub dashboard: DashboardState,
    pub zmq: ZmqViewState,
    pub zmq_state: Arc<ZmqSharedState>,
    pub zmq_handle: Option<ZmqHandle>,
    pub music: Option<MusicRuntime>,
    pub music_snapshot: MusicSnapshot,
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

        let zmq_shared = ZmqSharedState::default();
        zmq_shared
            .state
            .lock()
            .expect("zmq state lock")
            .buffer_limit = runtime_config
            .zmq_buffer_limit
            .clamp(MIN_ZMQ_BUFFER_LIMIT, MAX_ZMQ_BUFFER_LIMIT);

        let (schema_index, schema_error) = match SchemaIndex::load_default() {
            Ok(index) => (Some(index), None),
            Err(error) => (None, Some(error)),
        };
        let collapsed_categories = schema_index
            .as_ref()
            .map(|schema| {
                schema
                    .methods()
                    .iter()
                    .map(|method| method.category.clone())
                    .collect()
            })
            .unwrap_or_default();

        let mut state = Self {
            colors: ColorTheme::default(),
            theme_name: ThemeName::default(),
            sidebar_visible: true,
            active_tab: Tab::default(),
            config: ConfigState {
                store: config_store,
                store_path: config_store_path,
                store_error: config_store_error,
                form: ConfigForm::from(&runtime_config),
                runtime: runtime_config.clone(),
                connect_in_flight: false,
                save_in_flight: false,
                status: None,
                error: None,
            },
            rpc: RpcState {
                client: RpcClient::new(runtime_config),
                schema: schema_index,
                schema_error,
                search: String::new(),
                collapsed_categories,
                selected_method: Some("getblockchaininfo".to_string()),
                params_input: "[]".to_string(),
                batch_mode: false,
                batch_input: "[]".to_string(),
                execute_in_flight: false,
                response: None,
                error: None,
            },
            dashboard: DashboardState {
                snapshot: None,
                in_flight: false,
                error: None,
                selected_peer_id: None,
                peer_sort: PeerSortField::default(),
                peer_sort_desc: false,
                pending_partial: None,
                last_refresh_at: None,
                netinfo_level: 3,
                panes: pane_grid::State::with_configuration(
                    pane_grid::Configuration::Split {
                        axis: pane_grid::Axis::Horizontal,
                        ratio: 0.76,
                        a: Box::new(pane_grid::Configuration::Pane(DashboardPane::Main)),
                        b: Box::new(pane_grid::Configuration::Pane(DashboardPane::Zmq)),
                    },
                ),
            },
            zmq: ZmqViewState {
                connected: false,
                connected_address: String::new(),
                last_cursor: 0,
                events_seen: 0,
                last_topic: None,
                last_event_at: None,
                recent_events: Vec::new(),
            },
            zmq_state: Arc::new(zmq_shared),
            zmq_handle: None,
            music: None,
            music_snapshot: MusicSnapshot::default(),
        };

        if crate::music::is_enabled() {
            let rt = crate::music::start_music();
            state.music_snapshot = rt.snapshot();
            state.music = Some(rt);
        }

        let startup_zmq = state.config.runtime.zmq_address.trim().to_string();
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
