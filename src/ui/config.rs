use iced::widget::{button, column, container, row, text, text_input};
use iced::{Element, Fill};

use crate::app::message::Message;
use crate::app::state::State;
use crate::ui::components;

pub fn view(state: &State) -> Element<'_, Message> {
    let form = &state.config_form;

    let connect_button = if state.connect_in_flight {
        button("Connecting...").style(components::action_button_style())
    } else {
        button("Connect")
            .style(components::action_button_style())
            .on_press(Message::ConfigConnectPressed)
    };

    let save_button = if state.save_in_flight {
        button("Saving...").style(components::action_button_style())
    } else {
        button("Save")
            .style(components::action_button_style())
            .on_press(Message::ConfigSavePressed)
    };
    let reload_button = if state.save_in_flight || state.connect_in_flight {
        button("Reload").style(components::action_button_style())
    } else {
        button("Reload")
            .style(components::action_button_style())
            .on_press(Message::ConfigReloadPressed)
    };

    let mut content = column![
        text("CONNECTION MATRIX").size(24).color(components::ACCENT),
        text("RPC, WALLET AND ZMQ RUNTIME SETTINGS")
            .size(12)
            .color(components::MUTED),
        text("RPC URL"),
        text_input("http://127.0.0.1:8332", &form.url)
            .on_input(Message::ConfigUrlChanged)
            .padding(8)
            .style(components::input_style()),
        text("RPC User"),
        text_input("rpcuser", &form.user)
            .on_input(Message::ConfigUserChanged)
            .padding(8)
            .style(components::input_style()),
        text("RPC Password"),
        text_input("rpcpassword", &form.password)
            .on_input(Message::ConfigPasswordChanged)
            .secure(true)
            .padding(8)
            .style(components::input_style()),
        text("Wallet (optional)"),
        text_input("wallet name", &form.wallet)
            .on_input(Message::ConfigWalletChanged)
            .padding(8)
            .style(components::input_style()),
        text("Poll Interval (seconds)"),
        text_input("5", &form.poll_interval_secs)
            .on_input(Message::ConfigPollIntervalChanged)
            .padding(8)
            .style(components::input_style()),
        text("ZMQ Address (optional)"),
        text_input("tcp://127.0.0.1:28332", &form.zmq_address)
            .on_input(Message::ConfigZmqAddressChanged)
            .padding(8)
            .style(components::input_style()),
        text("ZMQ Buffer Limit"),
        text_input("5000", &form.zmq_buffer_limit)
            .on_input(Message::ConfigZmqBufferLimitChanged)
            .padding(8)
            .style(components::input_style()),
        row![connect_button, save_button, reload_button].spacing(12),
    ]
    .spacing(8)
    .width(Fill);

    if let Some(path) = &state.config_store_path {
        content = content.push(text(format!("Config file: {path}")).color(components::MUTED));
    }
    if let Some(error) = &state.config_store_error {
        content = content.push(text(format!("Config error: {error}")).color(components::ERROR_RED));
    }
    if let Some(status) = &state.config_status {
        content = content.push(text(status).color(components::MUTED));
    }
    if let Some(error) = &state.config_error {
        content = content.push(text(format!("ERR: {error}")).color(components::ERROR_RED));
    }

    container(content.padding(24).spacing(10))
        .style(components::panel_style())
        .width(Fill)
        .height(Fill)
        .into()
}
