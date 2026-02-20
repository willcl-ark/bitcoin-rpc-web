use iced::widget::{button, column, container, row, text, text_input};
use iced::{Element, Fill};

use crate::app::message::Message;
use crate::app::state::State;
use crate::ui::components;

pub fn view(state: &State) -> Element<'_, Message> {
    let form = &state.config.form;
    let fs = state.config.runtime.font_size;

    let connect_button = if state.config.connect_in_flight {
        button("Connecting...").style(components::action_button_style())
    } else {
        button("Connect")
            .style(components::action_button_style())
            .on_press(Message::ConfigConnectPressed)
    };

    let save_button = if state.config.save_in_flight {
        button("Saving...").style(components::action_button_style())
    } else {
        button("Save")
            .style(components::action_button_style())
            .on_press(Message::ConfigSavePressed)
    };
    let reload_button = if state.config.save_in_flight || state.config.connect_in_flight {
        button("Reload").style(components::action_button_style())
    } else {
        button("Reload")
            .style(components::action_button_style())
            .on_press(Message::ConfigReloadPressed)
    };

    let mut content = column![
        text("CONNECTION MATRIX")
            .size(fs + 10)
            .color(components::ACCENT),
        text("RPC, WALLET AND ZMQ RUNTIME SETTINGS")
            .size(fs.saturating_sub(2))
            .color(components::MUTED),
        text("RPC URL").size(fs),
        text_input("http://127.0.0.1:8332", &form.url)
            .on_input(Message::ConfigUrlChanged)
            .padding(8)
            .style(components::input_style()),
        text("RPC User").size(fs),
        text_input("rpcuser", &form.user)
            .on_input(Message::ConfigUserChanged)
            .padding(8)
            .style(components::input_style()),
        text("RPC Password").size(fs),
        text_input("rpcpassword", &form.password)
            .on_input(Message::ConfigPasswordChanged)
            .secure(true)
            .padding(8)
            .style(components::input_style()),
        text("Wallet (optional)").size(fs),
        text_input("wallet name", &form.wallet)
            .on_input(Message::ConfigWalletChanged)
            .padding(8)
            .style(components::input_style()),
        text("Poll Interval (seconds)").size(fs),
        text_input("5", &form.poll_interval_secs)
            .on_input(Message::ConfigPollIntervalChanged)
            .padding(8)
            .style(components::input_style()),
        text("ZMQ Address (optional)").size(fs),
        text_input("tcp://127.0.0.1:28332", &form.zmq_address)
            .on_input(Message::ConfigZmqAddressChanged)
            .padding(8)
            .style(components::input_style()),
        text("ZMQ Buffer Limit").size(fs),
        text_input("5000", &form.zmq_buffer_limit)
            .on_input(Message::ConfigZmqBufferLimitChanged)
            .padding(8)
            .style(components::input_style()),
        text("Font Size").size(fs),
        text_input("14", &form.font_size)
            .on_input(Message::ConfigFontSizeChanged)
            .padding(8)
            .style(components::input_style()),
        row![connect_button, save_button, reload_button].spacing(12),
    ]
    .spacing(8)
    .width(Fill);

    if let Some(path) = &state.config.store_path {
        content = content.push(
            text(format!("Config file: {path}"))
                .size(fs)
                .color(components::MUTED),
        );
    }
    if let Some(error) = &state.config.store_error {
        content = content.push(
            text(format!("Config error: {error}"))
                .size(fs)
                .color(components::ERROR_RED),
        );
    }
    if let Some(status) = &state.config.status {
        content = content.push(text(status).size(fs).color(components::MUTED));
    }
    if let Some(error) = &state.config.error {
        content = content.push(
            text(format!("ERR: {error}"))
                .size(fs)
                .color(components::ERROR_RED),
        );
    }

    container(content.padding(24).spacing(10))
        .style(components::panel_style())
        .width(Fill)
        .height(Fill)
        .into()
}
