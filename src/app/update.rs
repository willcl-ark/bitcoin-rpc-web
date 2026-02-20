use iced::Task;
use serde_json::Value;

use crate::app::message::Message;
use crate::app::state::{ConfigForm, State};
use crate::core::config_store::ConfigStore;
use crate::core::dashboard_service::{DashboardService, DashboardSnapshot};
use crate::core::rpc_client::{
    MAX_ZMQ_BUFFER_LIMIT, MIN_ZMQ_BUFFER_LIMIT, RpcClient, RpcConfig, allow_insecure,
    is_safe_rpc_host,
};
use crate::zmq::{start_zmq_subscriber, stop_zmq_subscriber};

pub fn update(state: &mut State, message: Message) -> Task<Message> {
    match message {
        Message::SelectTab(tab) => {
            state.active_tab = tab;
        }
        Message::ConfigUrlChanged(value) => {
            state.config_form.url = value;
            clear_form_feedback(state);
        }
        Message::ConfigUserChanged(value) => {
            state.config_form.user = value;
            clear_form_feedback(state);
        }
        Message::ConfigPasswordChanged(value) => {
            state.config_form.password = value;
            clear_form_feedback(state);
        }
        Message::ConfigWalletChanged(value) => {
            state.config_form.wallet = value;
            clear_form_feedback(state);
        }
        Message::ConfigPollIntervalChanged(value) => {
            state.config_form.poll_interval_secs = value;
            clear_form_feedback(state);
        }
        Message::ConfigZmqAddressChanged(value) => {
            state.config_form.zmq_address = value;
            clear_form_feedback(state);
        }
        Message::ConfigZmqBufferLimitChanged(value) => {
            state.config_form.zmq_buffer_limit = value;
            clear_form_feedback(state);
        }
        Message::ConfigConnectPressed => {
            if state.connect_in_flight {
                return Task::none();
            }

            let next_config = match parse_config_form(&state.config_form) {
                Ok(config) => config,
                Err(error) => {
                    state.config_error = Some(error);
                    state.config_status = None;
                    return Task::none();
                }
            };

            if !is_safe_rpc_host(&next_config.url) && !allow_insecure() {
                state.config_error = Some(
                    "RPC URL must be localhost/private unless DANGER_INSECURE_RPC=1".to_string(),
                );
                state.config_status = None;
                return Task::none();
            }

            state.connect_in_flight = true;
            state.config_error = None;
            state.config_status = Some("Connecting...".to_string());

            return Task::perform(test_rpc_config(next_config), Message::ConfigConnectFinished);
        }
        Message::ConfigConnectFinished(result) => {
            state.connect_in_flight = false;

            match result {
                Ok(config) => {
                    apply_runtime_config(state, config);
                    state.config_status = Some("Connected successfully.".to_string());
                    return Task::perform(async {}, |_| Message::DashboardTick);
                }
                Err(error) => {
                    state.config_status = None;
                    state.config_error = Some(error);
                }
            }
        }
        Message::ConfigReloadPressed => {
            let store = match &state.config_store {
                Some(store) => store.clone(),
                None => {
                    state.config_error = Some("config store unavailable".to_string());
                    state.config_status = None;
                    return Task::none();
                }
            };

            state.config_error = None;
            state.config_status = Some("Reloading...".to_string());
            return Task::perform(load_config(store), Message::ConfigReloadFinished);
        }
        Message::ConfigReloadFinished(result) => match result {
            Ok(config) => {
                apply_runtime_config(state, config);
                state.config_status = Some("Settings reloaded.".to_string());
                return Task::perform(async {}, |_| Message::DashboardTick);
            }
            Err(error) => {
                state.config_status = None;
                state.config_error = Some(error);
            }
        },
        Message::ConfigSavePressed => {
            if state.save_in_flight {
                return Task::none();
            }

            let store = match &state.config_store {
                Some(store) => store.clone(),
                None => {
                    state.config_error = Some("config store unavailable".to_string());
                    state.config_status = None;
                    return Task::none();
                }
            };

            let config = match parse_config_form(&state.config_form) {
                Ok(config) => config,
                Err(error) => {
                    state.config_error = Some(error);
                    state.config_status = None;
                    return Task::none();
                }
            };

            if !is_safe_rpc_host(&config.url) && !allow_insecure() {
                state.config_error = Some(
                    "RPC URL must be localhost/private unless DANGER_INSECURE_RPC=1".to_string(),
                );
                state.config_status = None;
                return Task::none();
            }

            state.save_in_flight = true;
            state.config_error = None;
            state.config_status = Some("Saving...".to_string());
            return Task::perform(save_config(store, config), Message::ConfigSaveFinished);
        }
        Message::ConfigSaveFinished(result) => {
            state.save_in_flight = false;

            match result {
                Ok(config) => {
                    apply_runtime_config(state, config);
                    state.config_status = Some("Settings saved.".to_string());
                    return Task::perform(async {}, |_| Message::DashboardTick);
                }
                Err(error) => {
                    state.config_status = None;
                    state.config_error = Some(error);
                }
            }
        }
        Message::RpcSearchChanged(value) => {
            state.rpc_search = value;
        }
        Message::RpcMethodSelected(method) => {
            state.rpc_selected_method = Some(method);
            state.rpc_error = None;
        }
        Message::RpcParamsChanged(value) => {
            state.rpc_params_input = value;
            state.rpc_error = None;
        }
        Message::RpcBatchModeToggled(enabled) => {
            state.rpc_batch_mode = enabled;
            state.rpc_error = None;
        }
        Message::RpcBatchChanged(value) => {
            state.rpc_batch_input = value;
            state.rpc_error = None;
        }
        Message::RpcExecutePressed => {
            if state.rpc_execute_in_flight {
                return Task::none();
            }

            state.rpc_execute_in_flight = true;
            state.rpc_error = None;
            state.rpc_response = None;
            let client = state.rpc_client.clone();

            if state.rpc_batch_mode {
                let batch_text = state.rpc_batch_input.clone();
                return Task::perform(
                    run_batch_rpc(client, batch_text),
                    Message::RpcExecuteFinished,
                );
            }

            let method = match &state.rpc_selected_method {
                Some(method) => method.clone(),
                None => {
                    state.rpc_execute_in_flight = false;
                    state.rpc_error = Some("Select an RPC method first".to_string());
                    return Task::none();
                }
            };
            let params_text = state.rpc_params_input.clone();
            return Task::perform(
                run_single_rpc(client, method, params_text),
                Message::RpcExecuteFinished,
            );
        }
        Message::RpcExecuteFinished(result) => {
            state.rpc_execute_in_flight = false;
            match result {
                Ok(response) => {
                    state.rpc_response = Some(response);
                    state.rpc_error = None;
                }
                Err(error) => {
                    state.rpc_response = None;
                    state.rpc_error = Some(error);
                }
            }
        }
        Message::DashboardTick => {
            if state.dashboard_in_flight {
                return Task::none();
            }
            state.dashboard_in_flight = true;
            let client = state.rpc_client.clone();
            return Task::perform(load_dashboard(client), Message::DashboardLoaded);
        }
        Message::DashboardLoaded(result) => {
            state.dashboard_in_flight = false;
            match result {
                Ok(snapshot) => {
                    let selected_is_valid = state
                        .dashboard_selected_peer_id
                        .is_some_and(|id| snapshot.peers.iter().any(|peer| peer.id == id));
                    if !selected_is_valid {
                        state.dashboard_selected_peer_id = snapshot.peers.first().map(|p| p.id);
                    }
                    state.dashboard_snapshot = Some(snapshot);
                    state.dashboard_error = None;
                }
                Err(error) => {
                    state.dashboard_error = Some(error);
                }
            }
        }
        Message::DashboardPeerSelected(peer_id) => {
            state.dashboard_selected_peer_id = Some(peer_id);
        }
    }

    Task::none()
}

fn clear_form_feedback(state: &mut State) {
    state.config_error = None;
    state.config_status = None;
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
    let previous_zmq = state.runtime_config.zmq_address.clone();
    state.runtime_config = config.clone();
    state.rpc_client = RpcClient::new(config.clone());
    state.config_form = ConfigForm::from(&config);
    state.config_error = None;
    apply_zmq_runtime(state, &previous_zmq);
}

fn apply_zmq_runtime(state: &mut State, previous_address: &str) {
    {
        let mut zmq_state = state.zmq_state.state.lock().expect("zmq state lock");
        zmq_state.buffer_limit = state
            .runtime_config
            .zmq_buffer_limit
            .clamp(MIN_ZMQ_BUFFER_LIMIT, MAX_ZMQ_BUFFER_LIMIT);
    }
    state.zmq_state.changed.notify_all();

    let current = state.runtime_config.zmq_address.trim().to_string();
    if current == previous_address {
        return;
    }

    if let Some(handle) = state.zmq_handle.take() {
        stop_zmq_subscriber(handle);
    }

    if current.is_empty() {
        return;
    }

    state.zmq_handle = Some(start_zmq_subscriber(&current, state.zmq_state.clone()));
}
