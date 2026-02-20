use iced::Task;

use crate::app::message::Message;
use crate::app::state::{ConfigForm, FocusField, State};
use crate::core::config_store::ConfigStore;
use crate::core::rpc_client::{
    MAX_FONT_SIZE, MAX_ZMQ_BUFFER_LIMIT, MIN_FONT_SIZE, MIN_ZMQ_BUFFER_LIMIT, RpcClient, RpcConfig,
};
use crate::zmq::{start_zmq_subscriber, stop_zmq_subscriber};

use super::validate_rpc_host;

pub fn handle_config(state: &mut State, message: Message) -> Task<Message> {
    match message {
        Message::ConfigUrlChanged(value) => {
            state.config.form.url = value;
            state.focused_input = Some(FocusField::ConfigUrl);
            clear_form_feedback(state);
        }
        Message::ConfigUserChanged(value) => {
            state.config.form.user = value;
            state.focused_input = Some(FocusField::ConfigUser);
            clear_form_feedback(state);
        }
        Message::ConfigPasswordChanged(value) => {
            state.config.form.password = value;
            state.focused_input = Some(FocusField::ConfigPassword);
            clear_form_feedback(state);
        }
        Message::ConfigWalletChanged(value) => {
            state.config.form.wallet = value;
            state.focused_input = Some(FocusField::ConfigWallet);
            clear_form_feedback(state);
        }
        Message::ConfigPollIntervalChanged(value) => {
            state.config.form.poll_interval_secs = value;
            state.focused_input = Some(FocusField::ConfigPollInterval);
            clear_form_feedback(state);
        }
        Message::ConfigZmqAddressChanged(value) => {
            state.config.form.zmq_address = value;
            state.focused_input = Some(FocusField::ConfigZmqAddress);
            clear_form_feedback(state);
        }
        Message::ConfigZmqBufferLimitChanged(value) => {
            state.config.form.zmq_buffer_limit = value;
            state.focused_input = Some(FocusField::ConfigZmqBufferLimit);
            clear_form_feedback(state);
        }
        Message::ConfigFontSizeChanged(value) => {
            state.config.form.font_size = value;
            state.focused_input = Some(FocusField::ConfigFontSize);
            clear_form_feedback(state);
        }
        Message::ConfigStartAudioPlayingChanged(value) => {
            state.config.form.start_audio_playing = value;
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

            if let Err(error) = validate_rpc_host(&next_config) {
                state.config.error = Some(error);
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
                if let Err(error) = validate_rpc_host(&config) {
                    state.config.status = None;
                    state.config.error = Some(error);
                    return Task::none();
                }
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

            if let Err(error) = validate_rpc_host(&config) {
                state.config.error = Some(error);
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
        _ => {}
    }

    Task::none()
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

    let font_size = form
        .font_size
        .trim()
        .parse::<u16>()
        .map_err(|_| "Font size must be a positive integer".to_string())?
        .clamp(MIN_FONT_SIZE, MAX_FONT_SIZE);

    Ok(RpcConfig {
        url: url.to_string(),
        user: form.user.clone(),
        password: form.password.clone(),
        wallet: form.wallet.clone(),
        poll_interval_secs,
        zmq_address: form.zmq_address.trim().to_string(),
        zmq_buffer_limit,
        font_size,
        start_audio_playing: form.start_audio_playing,
    })
}

async fn test_rpc_config(config: RpcConfig) -> Result<RpcConfig, String> {
    let client = RpcClient::new(config.clone());
    client
        .call("getblockchaininfo", serde_json::json!([]))
        .map_err(|error| error.to_string())?;
    Ok(config)
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

pub fn apply_runtime_config(state: &mut State, config: RpcConfig) {
    let previous_zmq = state.config.runtime.zmq_address.clone();
    state.config.runtime = config.clone();
    state.rpc.client = RpcClient::new(config.clone());
    state.config.form = ConfigForm::from(&config);
    state.config.error = None;
    state.dashboard.request_gen = state.dashboard.request_gen.wrapping_add(1);
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
