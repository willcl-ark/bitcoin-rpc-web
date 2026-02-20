use iced::widget::{button, column, container, row, scrollable, text};
use iced::{Element, Fill};

use crate::app::message::Message;
use crate::app::state::State;
use crate::ui::components;

pub fn view(state: &State) -> Element<'_, Message> {
    let zmq_status = if state.zmq_connected {
        format!("connected ({})", state.zmq_connected_address)
    } else if state.zmq_connected_address.is_empty() {
        "disabled".to_string()
    } else {
        format!("disconnected ({})", state.zmq_connected_address)
    };

    let top_strip: Element<'_, Message> = if let Some(snapshot) = &state.dashboard_snapshot {
        row![
            summary_card(
                "Chain",
                vec![
                    ("network", snapshot.chain.chain.clone()),
                    ("blocks", snapshot.chain.blocks.to_string()),
                    ("headers", snapshot.chain.headers.to_string()),
                    (
                        "verification",
                        format!("{:.4}", snapshot.chain.verification_progress)
                    ),
                ]
            )
            .width(iced::Length::FillPortion(1)),
            summary_card(
                "Mempool",
                vec![
                    ("transactions", snapshot.mempool.transactions.to_string()),
                    ("bytes", snapshot.mempool.bytes.to_string()),
                    ("usage", snapshot.mempool.usage.to_string()),
                    ("max", snapshot.mempool.maxmempool.to_string()),
                ]
            )
            .width(iced::Length::FillPortion(1)),
            summary_card(
                "Network",
                vec![
                    ("version", snapshot.network.version.to_string()),
                    ("subversion", snapshot.network.subversion.clone()),
                    ("protocol", snapshot.network.protocol_version.to_string()),
                    ("connections", snapshot.network.connections.to_string()),
                    ("uptime", format!("{}s", snapshot.uptime_secs)),
                ]
            )
            .width(iced::Length::FillPortion(1)),
            summary_card(
                "Traffic",
                vec![
                    (
                        "recv",
                        format!("{} bytes", snapshot.traffic.total_bytes_recv)
                    ),
                    (
                        "sent",
                        format!("{} bytes", snapshot.traffic.total_bytes_sent)
                    ),
                ]
            )
            .width(iced::Length::FillPortion(1)),
        ]
        .spacing(10)
        .into()
    } else {
        container(text("No dashboard data yet.").color(components::MUTED))
            .style(components::card_style())
            .padding(14)
            .into()
    };

    let main_body = row![
        container(peer_table(state))
            .style(components::panel_style())
            .padding(12)
            .width(iced::Length::FillPortion(3))
            .height(Fill),
        container(peer_detail(state))
            .style(components::panel_style())
            .padding(12)
            .width(iced::Length::FillPortion(2))
            .height(Fill),
    ]
    .spacing(12)
    .height(Fill);

    let zmq_panel = container(summary_card(
        "ZMQ Feed",
        vec![
            ("status", zmq_status),
            (
                "refresh",
                if state.dashboard_in_flight {
                    "syncing".to_string()
                } else {
                    "idle".to_string()
                },
            ),
            ("events seen", state.zmq_events_seen.to_string()),
            (
                "last topic",
                state.zmq_last_topic.as_deref().unwrap_or("-").to_string(),
            ),
            (
                "last event unix",
                state
                    .zmq_last_event_at
                    .map(|v| v.to_string())
                    .unwrap_or_else(|| "-".to_string()),
            ),
        ],
    ))
    .style(components::panel_style())
    .padding(10)
    .width(Fill);

    let mut root = column![
        text("Dashboard").size(32).color(components::TEXT),
        top_strip,
        main_body,
        zmq_panel
    ]
    .spacing(12)
    .height(Fill)
    .width(Fill);

    if let Some(error) = &state.dashboard_error {
        root = root.push(
            text(format!("Refresh error: {error}")).color(iced::Color::from_rgb(0.96, 0.58, 0.58)),
        );
    }

    container(root).padding(12).width(Fill).height(Fill).into()
}

fn peer_table(state: &State) -> Element<'_, Message> {
    let header = row![
        text("ID")
            .width(iced::Length::FillPortion(1))
            .color(components::MUTED),
        text("Address")
            .width(iced::Length::FillPortion(4))
            .color(components::MUTED),
        text("Dir")
            .width(iced::Length::FillPortion(1))
            .color(components::MUTED),
        text("Type")
            .width(iced::Length::FillPortion(2))
            .color(components::MUTED),
        text("Ping")
            .width(iced::Length::FillPortion(1))
            .color(components::MUTED),
    ]
    .spacing(8);

    let mut rows = column![text("Peers").size(24).color(components::TEXT), header].spacing(8);

    if let Some(snapshot) = &state.dashboard_snapshot {
        for peer in &snapshot.peers {
            let selected = state.dashboard_selected_peer_id == Some(peer.id);
            let ping = peer
                .ping_time
                .map(|v| format!("{v:.3}s"))
                .unwrap_or_else(|| "-".to_string());

            let row_line = row![
                text(peer.id.to_string()).width(iced::Length::FillPortion(1)),
                text(peer.addr.clone()).width(iced::Length::FillPortion(4)),
                text(if peer.inbound { "in" } else { "out" }).width(iced::Length::FillPortion(1)),
                text(peer.connection_type.clone()).width(iced::Length::FillPortion(2)),
                text(ping).width(iced::Length::FillPortion(1)),
            ]
            .spacing(8);

            rows = rows.push(
                button(row_line)
                    .width(Fill)
                    .style(components::nav_button_style(selected))
                    .on_press(Message::DashboardPeerSelected(peer.id)),
            );
        }
    } else {
        rows = rows.push(text("No peer data").color(components::MUTED));
    }

    scrollable(rows).into()
}

fn peer_detail(state: &State) -> Element<'_, Message> {
    let mut detail = column![text("Peer Detail").size(24).color(components::TEXT)].spacing(10);
    if let Some(snapshot) = &state.dashboard_snapshot {
        let selected = state
            .dashboard_selected_peer_id
            .and_then(|id| snapshot.peers.iter().find(|peer| peer.id == id));

        if let Some(peer) = selected {
            detail = detail.push(summary_card(
                "Selected Peer",
                vec![
                    ("id", peer.id.to_string()),
                    ("addr", peer.addr.clone()),
                    ("inbound", peer.inbound.to_string()),
                    ("type", peer.connection_type.clone()),
                    (
                        "ping",
                        peer.ping_time
                            .map(|v| format!("{v:.6}s"))
                            .unwrap_or_else(|| "-".to_string()),
                    ),
                ],
            ));
        } else {
            detail = detail.push(text("Select a peer from the table.").color(components::MUTED));
        }
    } else {
        detail = detail.push(text("No peer data").color(components::MUTED));
    }

    scrollable(detail).into()
}

fn summary_card<'a>(
    title: &'a str,
    lines: Vec<(&'a str, String)>,
) -> iced::widget::Container<'a, Message> {
    let mut content = column![text(title).size(21).color(components::TEXT)].spacing(6);
    for (key, value) in lines {
        content = content.push(
            row![
                text(format!("{key}:"))
                    .color(components::MUTED)
                    .width(iced::Length::FillPortion(2)),
                text(value)
                    .color(components::MUTED)
                    .width(iced::Length::FillPortion(5))
            ]
            .spacing(6),
        );
    }
    container(content)
        .padding(12)
        .style(components::card_style())
}
