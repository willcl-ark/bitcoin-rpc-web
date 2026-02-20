use iced::widget::{button, checkbox, column, container, row, scrollable, text, text_input};
use iced::{Element, Fill};

use crate::app::message::Message;
use crate::app::state::State;
use crate::ui::components;

pub fn view(state: &State) -> Element<'_, Message> {
    let method_list = if let Some(schema) = &state.schema_index {
        let methods = schema.search(&state.rpc_search);
        let mut list = column![text("Methods").size(24).color(components::TEXT)].spacing(8);
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
                    .style(components::nav_button_style(selected))
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
        button("Running...").style(components::action_button_style())
    } else {
        button("Execute")
            .style(components::action_button_style())
            .on_press(Message::RpcExecutePressed)
    };

    let mut right = column![
        text("RPC Explorer").size(28).color(components::TEXT),
        text("Search, inspect and execute methods directly.")
            .size(14)
            .color(components::MUTED),
        text_input("Search methods", &state.rpc_search)
            .on_input(Message::RpcSearchChanged)
            .padding(8),
        text(format!(
            "Selected method: {}",
            state.rpc_selected_method.as_deref().unwrap_or("(none)")
        )),
        text(selected_summary).color(components::MUTED),
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
        right = right.push(
            text(format!("Schema error: {error}")).color(iced::Color::from_rgb(0.96, 0.58, 0.58)),
        );
    }
    if let Some(error) = &state.rpc_error {
        right = right
            .push(text(format!("Error: {error}")).color(iced::Color::from_rgb(0.96, 0.58, 0.58)));
    }
    if let Some(response) = &state.rpc_response {
        right = right
            .push(text("Response").size(18).color(components::TEXT))
            .push(
                container(scrollable(text(response).size(14).color(components::MUTED)))
                    .style(components::card_style())
                    .padding(10),
            );
    }

    let layout = row![
        container(method_list)
            .style(components::panel_style())
            .padding(12)
            .width(300)
            .height(Fill),
        container(right)
            .style(components::panel_style())
            .padding(16)
            .width(Fill)
    ]
    .spacing(16)
    .height(Fill);

    container(layout)
        .padding(12)
        .width(Fill)
        .height(Fill)
        .into()
}
