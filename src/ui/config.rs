use iced::widget::{button, column, container, row, text, text_input};
use iced::{Element, Fill};

use crate::app::message::Message;
use crate::app::state::{FocusField, State};
use crate::ui::components;

pub fn view(state: &State) -> Element<'_, Message> {
    let form = &state.config.form;
    let fs = state.config.runtime.font_size;
    let colors = &state.colors;

    let connect_button = if state.config.connect_in_flight {
        button("Connecting...").style(components::action_button_style(colors))
    } else {
        button("Connect")
            .style(components::action_button_style(colors))
            .on_press(Message::ConfigConnectPressed)
    };

    let save_button = if state.config.save_in_flight {
        button("Saving...").style(components::action_button_style(colors))
    } else {
        button("Save")
            .style(components::action_button_style(colors))
            .on_press(Message::ConfigSavePressed)
    };
    let reload_button = if state.config.save_in_flight || state.config.connect_in_flight {
        button("Reload").style(components::action_button_style(colors))
    } else {
        button("Reload")
            .style(components::action_button_style(colors))
            .on_press(Message::ConfigReloadPressed)
    };

    let mut content = column![
        text("CONNECTION MATRIX")
            .size(fs + 10)
            .color(colors.accent),
        text("RPC, WALLET AND ZMQ RUNTIME SETTINGS")
            .size(fs.saturating_sub(2))
            .color(colors.fg_dim),
        text("RPC URL").size(fs),
        text_input("http://127.0.0.1:8332", &form.url)
            .id(FocusField::ConfigUrl.id())
            .on_input(Message::ConfigUrlChanged)
            .padding(8)
            .style(components::input_style(colors)),
        text("RPC User").size(fs),
        text_input("rpcuser", &form.user)
            .id(FocusField::ConfigUser.id())
            .on_input(Message::ConfigUserChanged)
            .padding(8)
            .style(components::input_style(colors)),
        text("RPC Password").size(fs),
        text_input("rpcpassword", &form.password)
            .id(FocusField::ConfigPassword.id())
            .on_input(Message::ConfigPasswordChanged)
            .secure(true)
            .padding(8)
            .style(components::input_style(colors)),
        text("Wallet (optional)").size(fs),
        text_input("wallet name", &form.wallet)
            .id(FocusField::ConfigWallet.id())
            .on_input(Message::ConfigWalletChanged)
            .padding(8)
            .style(components::input_style(colors)),
        text("Poll Interval (seconds)").size(fs),
        text_input("5", &form.poll_interval_secs)
            .id(FocusField::ConfigPollInterval.id())
            .on_input(Message::ConfigPollIntervalChanged)
            .padding(8)
            .style(components::input_style(colors)),
        text("ZMQ Address (optional)").size(fs),
        text_input("tcp://127.0.0.1:28332", &form.zmq_address)
            .id(FocusField::ConfigZmqAddress.id())
            .on_input(Message::ConfigZmqAddressChanged)
            .padding(8)
            .style(components::input_style(colors)),
        text("ZMQ Buffer Limit").size(fs),
        text_input("5000", &form.zmq_buffer_limit)
            .id(FocusField::ConfigZmqBufferLimit.id())
            .on_input(Message::ConfigZmqBufferLimitChanged)
            .padding(8)
            .style(components::input_style(colors)),
        text("Font Size").size(fs),
        text_input("14", &form.font_size)
            .id(FocusField::ConfigFontSize.id())
            .on_input(Message::ConfigFontSizeChanged)
            .padding(8)
            .style(components::input_style(colors)),
        row![connect_button, save_button, reload_button].spacing(12),
    ]
    .spacing(8)
    .width(Fill);

    if let Some(path) = &state.config.store_path {
        content = content.push(
            text(format!("Config file: {path}"))
                .size(fs)
                .color(colors.fg_dim),
        );
    }
    if let Some(error) = &state.config.store_error {
        content = content.push(
            text(format!("Config error: {error}"))
                .size(fs)
                .color(colors.red),
        );
    }
    if let Some(status) = &state.config.status {
        content = content.push(text(status).size(fs).color(colors.fg_dim));
    }
    if let Some(error) = &state.config.error {
        content = content.push(
            text(format!("ERR: {error}"))
                .size(fs)
                .color(colors.red),
        );
    }

    container(content.padding(24).spacing(10))
        .style(components::panel_style(colors))
        .width(Fill)
        .height(Fill)
        .into()
}
