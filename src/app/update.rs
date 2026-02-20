use iced::Task;
use serde_json::Value;
use std::time::{Duration, Instant};

use crate::app::message::Message;
use crate::app::state::{ConfigForm, DashboardPartialSet, State, ZmqUiEvent};
use crate::core::config_store::ConfigStore;
use crate::core::dashboard_service::{DashboardPartialUpdate, DashboardService, DashboardSnapshot};
use crate::core::rpc_client::{
    MAX_ZMQ_BUFFER_LIMIT, MIN_ZMQ_BUFFER_LIMIT, RpcClient, RpcConfig, allow_insecure,
    is_safe_rpc_host,
};
use crate::zmq::{start_zmq_subscriber, stop_zmq_subscriber};

const ZMQ_REFRESH_DEBOUNCE_MS: u64 = 800;

pub fn update(state: &mut State, message: Message) -> Task<Message> {
    match message {
        Message::SelectTab(tab) => {
            state.active_tab = tab;
        }
        Message::ConfigUrlChanged(value) => {
            state.config.form.url = value;
            clear_form_feedback(state);
        }
        Message::ConfigUserChanged(value) => {
            state.config.form.user = value;
            clear_form_feedback(state);
        }
        Message::ConfigPasswordChanged(value) => {
            state.config.form.password = value;
            clear_form_feedback(state);
        }
        Message::ConfigWalletChanged(value) => {
            state.config.form.wallet = value;
            clear_form_feedback(state);
        }
        Message::ConfigPollIntervalChanged(value) => {
            state.config.form.poll_interval_secs = value;
            clear_form_feedback(state);
        }
        Message::ConfigZmqAddressChanged(value) => {
            state.config.form.zmq_address = value;
            clear_form_feedback(state);
        }
        Message::ConfigZmqBufferLimitChanged(value) => {
            state.config.form.zmq_buffer_limit = value;
            clear_form_feedback(state);
        }
        Message::ConfigConnectPressed => {
            if state.config.connect_in_flight {
                return Task::none();
            }

            let next_config = match parse_config_form(&state.config.form) {
                Ok(config) => config,
                Err(error) => {
                    state.config.error = Some(error);
                    state.config.status = None;
                    return Task::none();
                }
            };

            if !is_safe_rpc_host(&next_config.url) && !allow_insecure() {
                state.config.error = Some(
                    "RPC URL must be localhost/private unless DANGER_INSECURE_RPC=1".to_string(),
                );
                state.config.status = None;
                return Task::none();
            }

            state.config.connect_in_flight = true;
            state.config.error = None;
            state.config.status = Some("Connecting...".to_string());

            return Task::perform(test_rpc_config(next_config), Message::ConfigConnectFinished);
        }
        Message::ConfigConnectFinished(result) => {
            state.config.connect_in_flight = false;

            match result {
                Ok(config) => {
                    apply_runtime_config(state, config);
                    state.config.status = Some("Connected successfully.".to_string());
                    return Task::perform(async {}, |_| Message::DashboardTick);
                }
                Err(error) => {
                    state.config.status = None;
                    state.config.error = Some(error);
                }
            }
        }
        Message::ConfigReloadPressed => {
            let store = match &state.config.store {
                Some(store) => store.clone(),
                None => {
                    state.config.error = Some("config store unavailable".to_string());
                    state.config.status = None;
                    return Task::none();
                }
            };

            state.config.error = None;
            state.config.status = Some("Reloading...".to_string());
            return Task::perform(load_config(store), Message::ConfigReloadFinished);
        }
        Message::ConfigReloadFinished(result) => match result {
            Ok(config) => {
                apply_runtime_config(state, config);
                state.config.status = Some("Settings reloaded.".to_string());
                return Task::perform(async {}, |_| Message::DashboardTick);
            }
            Err(error) => {
                state.config.status = None;
                state.config.error = Some(error);
            }
        },
        Message::ConfigSavePressed => {
            if state.config.save_in_flight {
                return Task::none();
            }

            let store = match &state.config.store {
                Some(store) => store.clone(),
                None => {
                    state.config.error = Some("config store unavailable".to_string());
                    state.config.status = None;
                    return Task::none();
                }
            };

            let config = match parse_config_form(&state.config.form) {
                Ok(config) => config,
                Err(error) => {
                    state.config.error = Some(error);
                    state.config.status = None;
                    return Task::none();
                }
            };

            if !is_safe_rpc_host(&config.url) && !allow_insecure() {
                state.config.error = Some(
                    "RPC URL must be localhost/private unless DANGER_INSECURE_RPC=1".to_string(),
                );
                state.config.status = None;
                return Task::none();
            }

            state.config.save_in_flight = true;
            state.config.error = None;
            state.config.status = Some("Saving...".to_string());
            return Task::perform(save_config(store, config), Message::ConfigSaveFinished);
        }
        Message::ConfigSaveFinished(result) => {
            state.config.save_in_flight = false;

            match result {
                Ok(config) => {
                    apply_runtime_config(state, config);
                    state.config.status = Some("Settings saved.".to_string());
                    return Task::perform(async {}, |_| Message::DashboardTick);
                }
                Err(error) => {
                    state.config.status = None;
                    state.config.error = Some(error);
                }
            }
        }
        Message::RpcSearchChanged(value) => {
            state.rpc.search = value;
        }
        Message::RpcCategoryToggled(category) => {
            if !state.rpc.collapsed_categories.remove(&category) {
                state.rpc.collapsed_categories.insert(category);
            }
        }
        Message::RpcMethodSelected(method) => {
            state.rpc.selected_method = Some(method);
            state.rpc.error = None;
        }
        Message::RpcParamsChanged(value) => {
            state.rpc.params_input = value;
            state.rpc.error = None;
        }
        Message::RpcBatchModeToggled(enabled) => {
            state.rpc.batch_mode = enabled;
            state.rpc.error = None;
        }
        Message::RpcBatchChanged(value) => {
            state.rpc.batch_input = value;
            state.rpc.error = None;
        }
        Message::RpcExecutePressed => {
            if state.rpc.execute_in_flight {
                return Task::none();
            }

            state.rpc.execute_in_flight = true;
            state.rpc.error = None;
            state.rpc.response = None;
            let client = state.rpc.client.clone();

            if state.rpc.batch_mode {
                let batch_text = state.rpc.batch_input.clone();
                return Task::perform(
                    run_batch_rpc(client, batch_text),
                    Message::RpcExecuteFinished,
                );
            }

            let method = match &state.rpc.selected_method {
                Some(method) => method.clone(),
                None => {
                    state.rpc.execute_in_flight = false;
                    state.rpc.error = Some("Select an RPC method first".to_string());
                    return Task::none();
                }
            };
            let params_text = state.rpc.params_input.clone();
            return Task::perform(
                run_single_rpc(client, method, params_text),
                Message::RpcExecuteFinished,
            );
        }
        Message::RpcExecuteFinished(result) => {
            state.rpc.execute_in_flight = false;
            match result {
                Ok(response) => {
                    state.rpc.response = Some(response);
                    state.rpc.error = None;
                }
                Err(error) => {
                    state.rpc.response = None;
                    state.rpc.error = Some(error);
                }
            }
        }
        Message::DashboardTick => {
            if state.dashboard.in_flight {
                return Task::none();
            }
            return start_dashboard_refresh(state);
        }
        Message::DashboardLoaded(result) => {
            state.dashboard.in_flight = false;
            match result {
                Ok(snapshot) => {
                    let selected_is_valid = state
                        .dashboard
                        .selected_peer_id
                        .is_some_and(|id| snapshot.peers.iter().any(|peer| peer.id == id));
                    if !selected_is_valid {
                        state.dashboard.selected_peer_id = None;
                    }
                    state.dashboard.snapshot = Some(snapshot);
                    state.dashboard.error = None;
                }
                Err(error) => {
                    state.dashboard.error = Some(error);
                }
            }
            return schedule_pending_partial_if_ready(state);
        }
        Message::DashboardPeerSelected(peer_id) => {
            state.dashboard.selected_peer_id = Some(peer_id);
        }
        Message::DashboardPeerDetailClosed => {
            state.dashboard.selected_peer_id = None;
        }
        Message::DashboardPeerSortPressed(field) => {
            if state.dashboard.peer_sort == field {
                state.dashboard.peer_sort_desc = !state.dashboard.peer_sort_desc;
            } else {
                state.dashboard.peer_sort = field;
                state.dashboard.peer_sort_desc = false;
            }
        }
        Message::DashboardPartialRefreshRequested(partial) => {
            if state.dashboard.in_flight {
                return Task::none();
            }
            if state.dashboard.snapshot.is_none() {
                return start_dashboard_refresh(state);
            }
            return start_partial_dashboard_refresh(state, partial);
        }
        Message::DashboardPartialLoaded(result) => {
            state.dashboard.in_flight = false;
            match result {
                Ok(partial) => {
                    if let Some(snapshot) = state.dashboard.snapshot.as_mut() {
                        match partial {
                            DashboardPartialUpdate::Mempool(mempool) => {
                                snapshot.mempool = mempool;
                            }
                            DashboardPartialUpdate::ChainAndMempool { chain, mempool } => {
                                snapshot.chain = chain;
                                snapshot.mempool = mempool;
                            }
                        }
                        state.dashboard.error = None;
                    } else {
                        return start_dashboard_refresh(state);
                    }
                }
                Err(error) => {
                    state.dashboard.error = Some(error);
                }
            }
            return schedule_pending_partial_if_ready(state);
        }
        Message::ZmqPollTick => {
            poll_zmq_feed(state);
            if let Some(partial) = state.dashboard.pending_partial
                && !state.dashboard.in_flight
                && can_run_debounced_refresh(state)
            {
                state.dashboard.pending_partial = None;
                return Task::perform(
                    async move { partial },
                    Message::DashboardPartialRefreshRequested,
                );
            }
        }
        Message::MusicPlayPause => {
            if let Some(rt) = &state.music {
                rt.play_pause();
                state.music_snapshot = rt.snapshot();
            }
        }
        Message::MusicNext => {
            if let Some(rt) = &state.music {
                rt.next();
                state.music_snapshot = rt.snapshot();
            }
        }
        Message::MusicPrev => {
            if let Some(rt) = &state.music {
                rt.prev();
                state.music_snapshot = rt.snapshot();
            }
        }
        Message::MusicSetVolume(v) => {
            if let Some(rt) = &state.music {
                rt.set_volume(v);
                state.music_snapshot = rt.snapshot();
            }
        }
        Message::MusicToggleMute => {
            if let Some(rt) = &state.music {
                rt.toggle_mute();
                state.music_snapshot = rt.snapshot();
            }
        }
        Message::MusicPollTick => {
            if let Some(rt) = &state.music {
                state.music_snapshot = rt.snapshot();
            }
        }
    }

    Task::none()
}

fn schedule_pending_partial_if_ready(state: &mut State) -> Task<Message> {
    if let Some(partial) = state.dashboard.pending_partial
        && can_run_debounced_refresh(state)
    {
        state.dashboard.pending_partial = None;
        return Task::perform(
            async move { partial },
            Message::DashboardPartialRefreshRequested,
        );
    }
    Task::none()
}

fn start_dashboard_refresh(state: &mut State) -> Task<Message> {
    state.dashboard.in_flight = true;
    state.dashboard.last_refresh_at = Some(Instant::now());
    let client = state.rpc.client.clone();
    Task::perform(load_dashboard(client), Message::DashboardLoaded)
}

fn start_partial_dashboard_refresh(
    state: &mut State,
    partial: DashboardPartialSet,
) -> Task<Message> {
    state.dashboard.in_flight = true;
    state.dashboard.last_refresh_at = Some(Instant::now());
    let client = state.rpc.client.clone();
    Task::perform(
        load_dashboard_partial(client, partial),
        Message::DashboardPartialLoaded,
    )
}

fn can_run_debounced_refresh(state: &State) -> bool {
    state
        .dashboard
        .last_refresh_at
        .is_none_or(|t| t.elapsed() >= Duration::from_millis(ZMQ_REFRESH_DEBOUNCE_MS))
}

fn poll_zmq_feed(state: &mut State) {
    let mut saw_hashblock = false;
    let mut saw_hashtx = false;
    let mut next_cursor = state.zmq.last_cursor;

    {
        let zmq_state = state.zmq_state.state.lock().expect("zmq state lock");
        state.zmq.connected = zmq_state.connected;
        state.zmq.connected_address = zmq_state.address.clone();
        state.zmq.events_seen = zmq_state.next_cursor.saturating_sub(1);

        for message in zmq_state.messages.iter() {
            if message.cursor <= state.zmq.last_cursor {
                continue;
            }

            next_cursor = next_cursor.max(message.cursor);
            state.zmq.last_topic = Some(message.topic.clone());
            state.zmq.last_event_at = Some(message.timestamp);

            match message.topic.as_str() {
                "hashblock" => saw_hashblock = true,
                "hashtx" => saw_hashtx = true,
                _ => {}
            }
        }

        state.zmq.recent_events = zmq_state
            .messages
            .iter()
            .rev()
            .take(80)
            .map(|m| ZmqUiEvent {
                topic: m.topic.clone(),
                event_hash: m.event_hash.clone().unwrap_or_else(|| m.body_hex.clone()),
                timestamp: m.timestamp,
            })
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect();
    }

    state.zmq.last_cursor = next_cursor;

    if saw_hashblock {
        merge_pending_partial(state, DashboardPartialSet::ChainAndMempool);
    } else if saw_hashtx {
        merge_pending_partial(state, DashboardPartialSet::MempoolOnly);
    }
}

fn merge_pending_partial(state: &mut State, next: DashboardPartialSet) {
    state.dashboard.pending_partial = Some(match (state.dashboard.pending_partial, next) {
        (Some(DashboardPartialSet::ChainAndMempool), _) => DashboardPartialSet::ChainAndMempool,
        (_, DashboardPartialSet::ChainAndMempool) => DashboardPartialSet::ChainAndMempool,
        _ => DashboardPartialSet::MempoolOnly,
    });
}

fn clear_form_feedback(state: &mut State) {
    state.config.error = None;
    state.config.status = None;
}

fn parse_config_form(form: &ConfigForm) -> Result<RpcConfig, String> {
    let url = form.url.trim();
    if url.is_empty() {
        return Err("RPC URL is required".to_string());
    }

    let poll_interval_secs = form
        .poll_interval_secs
        .trim()
        .parse::<u64>()
        .map_err(|_| "Poll interval must be a positive integer".to_string())?
        .clamp(1, 3600);

    let zmq_buffer_limit = form
        .zmq_buffer_limit
        .trim()
        .parse::<usize>()
        .map_err(|_| "ZMQ buffer limit must be an integer".to_string())?
        .clamp(MIN_ZMQ_BUFFER_LIMIT, MAX_ZMQ_BUFFER_LIMIT);

    Ok(RpcConfig {
        url: url.to_string(),
        user: form.user.clone(),
        password: form.password.clone(),
        wallet: form.wallet.clone(),
        poll_interval_secs,
        zmq_address: form.zmq_address.trim().to_string(),
        zmq_buffer_limit,
    })
}

async fn test_rpc_config(config: RpcConfig) -> Result<RpcConfig, String> {
    let client = RpcClient::new(config.clone());
    client
        .call("getblockchaininfo", serde_json::json!([]))
        .map_err(|error| error.to_string())?;
    Ok(config)
}

async fn run_single_rpc(
    client: RpcClient,
    method: String,
    params_text: String,
) -> Result<String, String> {
    let params: Value =
        serde_json::from_str(&params_text).map_err(|e| format!("invalid params json: {e}"))?;
    let response = client.call(&method, params).map_err(|e| e.to_string())?;
    serde_json::to_string_pretty(&response).map_err(|e| format!("failed to format response: {e}"))
}

async fn run_batch_rpc(client: RpcClient, batch_text: String) -> Result<String, String> {
    let payload: Value =
        serde_json::from_str(&batch_text).map_err(|e| format!("invalid batch json: {e}"))?;
    if !payload.is_array() {
        return Err("batch mode expects a JSON array".to_string());
    }
    let response = client.post_json(&payload).map_err(|e| e.to_string())?;
    serde_json::to_string_pretty(&response).map_err(|e| format!("failed to format response: {e}"))
}

async fn load_dashboard(client: RpcClient) -> Result<DashboardSnapshot, String> {
    let service = DashboardService::new(client);
    service.fetch_snapshot().map_err(|e| e.to_string())
}

async fn load_dashboard_partial(
    client: RpcClient,
    partial: DashboardPartialSet,
) -> Result<DashboardPartialUpdate, String> {
    let service = DashboardService::new(client);
    match partial {
        DashboardPartialSet::MempoolOnly => service.fetch_mempool_update(),
        DashboardPartialSet::ChainAndMempool => service.fetch_chain_and_mempool_update(),
    }
    .map_err(|e| e.to_string())
}

async fn load_config(store: ConfigStore) -> Result<RpcConfig, String> {
    store
        .load()
        .map_err(|error| format!("failed to load config: {error}"))
}

async fn save_config(store: ConfigStore, config: RpcConfig) -> Result<RpcConfig, String> {
    store
        .save(&config)
        .map_err(|error| format!("failed to save config: {error}"))?;
    Ok(config)
}

fn apply_runtime_config(state: &mut State, config: RpcConfig) {
    let previous_zmq = state.config.runtime.zmq_address.clone();
    state.config.runtime = config.clone();
    state.rpc.client = RpcClient::new(config.clone());
    state.config.form = ConfigForm::from(&config);
    state.config.error = None;
    apply_zmq_runtime(state, &previous_zmq);
}

fn apply_zmq_runtime(state: &mut State, previous_address: &str) {
    {
        let mut zmq_state = state.zmq_state.state.lock().expect("zmq state lock");
        zmq_state.buffer_limit = state
            .config
            .runtime
            .zmq_buffer_limit
            .clamp(MIN_ZMQ_BUFFER_LIMIT, MAX_ZMQ_BUFFER_LIMIT);
    }
    state.zmq_state.changed.notify_all();

    let current = state.config.runtime.zmq_address.trim().to_string();
    if current == previous_address {
        return;
    }

    if let Some(handle) = state.zmq_handle.take() {
        stop_zmq_subscriber(handle);
    }

    if current.is_empty() {
        state.zmq.connected = false;
        state.zmq.connected_address.clear();
        state.zmq.last_cursor = 0;
        state.zmq.last_topic = None;
        state.zmq.last_event_at = None;
        state.zmq.recent_events.clear();
        state.dashboard.pending_partial = None;
        return;
    }

    state.zmq_handle = Some(start_zmq_subscriber(&current, state.zmq_state.clone()));
}
