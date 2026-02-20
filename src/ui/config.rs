use iced::widget::{button, column, container, row, text, text_input};
use iced::{Element, Fill};

use crate::app::message::Message;
use crate::app::state::State;

pub fn view(state: &State) -> Element<'_, Message> {
    let form = &state.config_form;

    let connect_button = if state.connect_in_flight {
        button("Connecting...")
    } else {
        button("Connect").on_press(Message::ConfigConnectPressed)
    };

    let save_button = if state.save_in_flight {
        button("Saving...")
    } else {
        button("Save").on_press(Message::ConfigSavePressed)
    };

    let mut content = column![
        text("Config").size(26),
        text("RPC URL"),
        text_input("http://127.0.0.1:8332", &form.url)
            .on_input(Message::ConfigUrlChanged)
            .padding(8),
        text("RPC User"),
        text_input("rpcuser", &form.user)
            .on_input(Message::ConfigUserChanged)
            .padding(8),
        text("RPC Password"),
        text_input("rpcpassword", &form.password)
            .on_input(Message::ConfigPasswordChanged)
            .padding(8),
        text("Wallet (optional)"),
        text_input("wallet name", &form.wallet)
            .on_input(Message::ConfigWalletChanged)
            .padding(8),
        text("Poll Interval (seconds)"),
        text_input("5", &form.poll_interval_secs)
            .on_input(Message::ConfigPollIntervalChanged)
            .padding(8),
        text("ZMQ Address (optional)"),
        text_input("tcp://127.0.0.1:28332", &form.zmq_address)
            .on_input(Message::ConfigZmqAddressChanged)
            .padding(8),
        text("ZMQ Buffer Limit"),
        text_input("5000", &form.zmq_buffer_limit)
            .on_input(Message::ConfigZmqBufferLimitChanged)
            .padding(8),
        row![connect_button, save_button].spacing(12),
    ]
    .spacing(8);

    if let Some(path) = &state.config_store_path {
        content = content.push(text(format!("Config file: {path}")));
    }
    if let Some(error) = &state.config_store_error {
        content = content.push(text(format!("Config store error: {error}")));
    }
    if let Some(status) = &state.config_status {
        content = content.push(text(status));
    }
    if let Some(error) = &state.config_error {
        content = content.push(text(format!("Error: {error}")));
    }

    container(content.padding(24).spacing(10))
        .width(Fill)
        .height(Fill)
        .into()
}
