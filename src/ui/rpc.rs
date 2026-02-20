use iced::widget::{button, checkbox, column, container, row, scrollable, text, text_input};
use iced::{Element, Fill};

use crate::app::message::Message;
use crate::app::state::State;

pub fn view(state: &State) -> Element<'_, Message> {
    let method_list = if let Some(schema) = &state.schema_index {
        let methods = schema.search(&state.rpc_search);
        let mut list = column![].spacing(6);
        for method in methods.iter().take(200) {
            let selected = state.rpc_selected_method.as_deref() == Some(method.name.as_str());
            let label = if selected {
                format!("> {}", method.name)
            } else {
                method.name.clone()
            };
            list = list.push(
                button(text(label))
                    .width(Fill)
                    .on_press(Message::RpcMethodSelected(method.name.clone())),
            );
        }
        scrollable(list).height(Fill)
    } else {
        scrollable(column![text("Schema unavailable")])
    };

    let selected_summary = state
        .rpc_selected_method
        .as_ref()
        .and_then(|name| {
            state.schema_index.as_ref().and_then(|schema| {
                schema
                    .methods()
                    .iter()
                    .find(|m| &m.name == name)
                    .and_then(|m| m.summary.as_ref())
            })
        })
        .cloned()
        .unwrap_or_else(|| "Select a method from the list.".to_string());

    let execute_button = if state.rpc_execute_in_flight {
        button("Running...")
    } else {
        button("Execute").on_press(Message::RpcExecutePressed)
    };

    let mut right = column![
        text("RPC").size(26),
        text_input("Search methods", &state.rpc_search)
            .on_input(Message::RpcSearchChanged)
            .padding(8),
        text(format!(
            "Selected method: {}",
            state.rpc_selected_method.as_deref().unwrap_or("(none)")
        )),
        text(selected_summary),
        checkbox("Batch mode", state.rpc_batch_mode).on_toggle(Message::RpcBatchModeToggled),
    ]
    .spacing(10);

    if state.rpc_batch_mode {
        right = right.push(text("Batch request JSON array")).push(
            text_input(
                r#"[{"method":"getblockchaininfo","params":[]}]"#,
                &state.rpc_batch_input,
            )
            .on_input(Message::RpcBatchChanged)
            .padding(8),
        );
    } else {
        right = right.push(text("Params JSON")).push(
            text_input("[]", &state.rpc_params_input)
                .on_input(Message::RpcParamsChanged)
                .padding(8),
        );
    }

    right = right.push(execute_button);

    if let Some(error) = &state.schema_error {
        right = right.push(text(format!("Schema error: {error}")));
    }
    if let Some(error) = &state.rpc_error {
        right = right.push(text(format!("Error: {error}")));
    }
    if let Some(response) = &state.rpc_response {
        right = right
            .push(text("Response"))
            .push(scrollable(text(response)));
    }

    let layout = row![
        container(method_list).width(280).height(Fill),
        container(right).width(Fill)
    ]
    .spacing(16)
    .height(Fill);

    container(layout)
        .padding(24)
        .width(Fill)
        .height(Fill)
        .into()
}
