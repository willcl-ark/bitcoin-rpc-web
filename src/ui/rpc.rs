use std::collections::BTreeMap;

use iced::widget::{button, checkbox, column, container, row, scrollable, text, text_input};
use iced::{Element, Fill};

use crate::app::message::Message;
use crate::app::state::State;
use crate::ui::components;

pub fn view(state: &State) -> Element<'_, Message> {
    let method_list = if let Some(schema) = &state.rpc.schema {
        let mut grouped: BTreeMap<String, Vec<_>> = BTreeMap::new();
        for method in schema.search(&state.rpc.search).into_iter().take(400) {
            grouped
                .entry(method.category.clone())
                .or_default()
                .push(method);
        }

        let mut list = column![text("METHOD GROUPS").size(16).color(components::ACCENT)].spacing(6);

        if grouped.is_empty() {
            list = list.push(text("No methods match current search.").color(components::MUTED));
        }

        for (category, mut methods) in grouped {
            methods.sort_by(|a, b| a.name.cmp(&b.name));
            let collapsed = state.rpc.collapsed_categories.contains(&category);
            let marker = if collapsed { "[+]" } else { "[-]" };
            let category_label =
                format!("{marker} {} ({})", category.to_uppercase(), methods.len());
            list = list.push(
                button(text(category_label))
                    .width(Fill)
                    .style(components::utility_button_style(!collapsed))
                    .padding([4, 8])
                    .on_press(Message::RpcCategoryToggled(category.clone())),
            );

            if collapsed {
                continue;
            }

            for method in methods {
                let selected = state.rpc.selected_method.as_deref() == Some(method.name.as_str());
                let label = if selected {
                    format!("> {}", method.name)
                } else {
                    format!("  {}", method.name)
                };
                list = list.push(
                    button(text(label))
                        .width(Fill)
                        .style(components::row_button_style(selected))
                        .padding([4, 8])
                        .on_press(Message::RpcMethodSelected(method.name.clone())),
                );
            }
        }
        scrollable(list).height(Fill)
    } else {
        scrollable(column![text("Schema unavailable")])
    };

    let selected_summary = state
        .rpc
        .selected_method
        .as_ref()
        .and_then(|name| {
            state.rpc.schema.as_ref().and_then(|schema| {
                schema
                    .methods()
                    .iter()
                    .find(|m| &m.name == name)
                    .and_then(|m| m.summary.as_ref())
            })
        })
        .cloned()
        .unwrap_or_else(|| "Select a method from the list.".to_string());

    let execute_button = if state.rpc.execute_in_flight {
        button("Running...").style(components::action_button_style())
    } else {
        button("Execute")
            .style(components::action_button_style())
            .on_press(Message::RpcExecutePressed)
    };

    let mut right = column![
        text("RPC EXECUTION CONSOLE")
            .size(24)
            .color(components::ACCENT),
        text("SEARCH, INSPECT, EXECUTE")
            .size(12)
            .color(components::MUTED),
        text_input("Search methods", &state.rpc.search)
            .on_input(Message::RpcSearchChanged)
            .padding(8)
            .style(components::input_style()),
        text(format!(
            "Selected method: {}",
            state.rpc.selected_method.as_deref().unwrap_or("(none)")
        )),
        text(selected_summary).color(components::MUTED),
        checkbox("Batch mode", state.rpc.batch_mode)
            .on_toggle(Message::RpcBatchModeToggled)
            .style(components::checkbox_style()),
    ]
    .spacing(10);

    if state.rpc.batch_mode {
        right = right.push(text("Batch request JSON array")).push(
            text_input(
                r#"[{"method":"getblockchaininfo","params":[]}]"#,
                &state.rpc.batch_input,
            )
            .on_input(Message::RpcBatchChanged)
            .padding(8)
            .style(components::input_style()),
        );
    } else {
        right = right.push(text("Params JSON")).push(
            text_input("[]", &state.rpc.params_input)
                .on_input(Message::RpcParamsChanged)
                .padding(8)
                .style(components::input_style()),
        );
    }

    right = right.push(execute_button);

    if let Some(error) = &state.rpc.schema_error {
        right = right.push(text(format!("Schema error: {error}")).color(components::ERROR_RED));
    }
    if let Some(error) = &state.rpc.error {
        right = right.push(text(format!("ERR: {error}")).color(components::ERROR_RED));
    }
    if let Some(response) = &state.rpc.response {
        right = right
            .push(text("RESPONSE").size(14).color(components::ACCENT))
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
