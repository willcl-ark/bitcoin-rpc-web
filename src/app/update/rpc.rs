use iced::Task;
use serde_json::Value;

use crate::app::message::Message;
use crate::app::state::{FocusField, State};
use crate::core::rpc_client::RpcClient;

pub fn handle_rpc(state: &mut State, message: Message) -> Task<Message> {
    match message {
        Message::RpcSearchChanged(value) => {
            state.rpc.search = value;
            state.focused_input = Some(FocusField::RpcSearch);
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
            state.focused_input = Some(FocusField::RpcParams);
            state.rpc.error = None;
        }
        Message::RpcBatchModeToggled(enabled) => {
            state.rpc.batch_mode = enabled;
            state.focused_input = None;
            state.rpc.error = None;
        }
        Message::RpcBatchChanged(value) => {
            state.rpc.batch_input = value;
            state.focused_input = Some(FocusField::RpcBatch);
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
        _ => {}
    }

    Task::none()
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
