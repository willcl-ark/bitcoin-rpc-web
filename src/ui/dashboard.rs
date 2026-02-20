use chrono::{DateTime, Utc};
use iced::widget::{button, column, container, row, scrollable, text};
use iced::{Element, Fill};

use crate::app::message::Message;
use crate::app::state::{PeerSortField, State};
use crate::core::dashboard_service::PeerSummary;
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
        .spacing(8)
        .into()
    } else {
        container(text("NO DASHBOARD DATA YET").color(components::MUTED))
            .style(components::card_style())
            .padding(14)
            .into()
    };

    let main_body = container(peer_table(state))
        .style(components::panel_style())
        .padding(8)
        .width(Fill)
        .height(Fill);

    let zmq_summary = summary_card(
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
    );

    let mut live_rows = column![
        row![
            text("TOPIC")
                .width(iced::Length::FillPortion(1))
                .color(components::MUTED),
            text("EVENT")
                .width(iced::Length::FillPortion(4))
                .color(components::MUTED),
            text("TIME")
                .width(iced::Length::FillPortion(2))
                .color(components::MUTED),
        ]
        .spacing(4)
    ]
    .spacing(2);

    if state.zmq_recent_events.is_empty() {
        live_rows = live_rows.push(text("No ZMQ events yet.").color(components::MUTED));
    } else {
        for evt in state.zmq_recent_events.iter().rev() {
            live_rows = live_rows.push(
                row![
                    text(&evt.topic)
                        .color(components::ACCENT)
                        .width(iced::Length::FillPortion(1)),
                    text(&evt.event_hash).width(iced::Length::FillPortion(4)),
                    text(format_event_time(evt.timestamp)).width(iced::Length::FillPortion(2)),
                ]
                .spacing(4),
            );
        }
    }

    let zmq_panel = container(
        row![
            container(zmq_summary)
                .style(components::card_style())
                .padding(8)
                .width(iced::Length::FillPortion(2)),
            container(
                column![
                    text("LIVE EVENTS").size(14).color(components::ACCENT),
                    scrollable(live_rows).height(160)
                ]
                .spacing(4)
            )
            .style(components::card_style())
            .padding(8)
            .width(iced::Length::FillPortion(5)),
        ]
        .spacing(8),
    )
    .style(components::panel_style())
    .padding(8)
    .width(Fill);

    let mut root = column![
        row![
            text("DASHBOARD").size(24).color(components::ACCENT),
            text("TELEMETRY + PEERING")
                .size(12)
                .color(components::AMBER)
        ]
        .spacing(12),
        top_strip,
        main_body,
        zmq_panel
    ]
    .spacing(8)
    .height(Fill)
    .width(Fill);

    if let Some(error) = &state.dashboard_error {
        root = root.push(text(format!("ERR: {error}")).color(components::ERROR_RED));
    }

    container(root).padding(12).width(Fill).height(Fill).into()
}

fn peer_table(state: &State) -> Element<'_, Message> {
    let header = row![
        sort_header(state, "ID", PeerSortField::Id).width(iced::Length::FillPortion(1)),
        sort_header(state, "Address", PeerSortField::Address).width(iced::Length::FillPortion(3)),
        text("Subver")
            .color(components::MUTED)
            .width(iced::Length::FillPortion(3)),
        sort_header(state, "Dir", PeerSortField::Direction).width(iced::Length::FillPortion(1)),
        sort_header(state, "Type", PeerSortField::ConnectionType)
            .width(iced::Length::FillPortion(2)),
        sort_header(state, "Ping", PeerSortField::Ping).width(iced::Length::FillPortion(1)),
    ]
    .spacing(4);

    let mut rows = column![text("PEERS").size(15).color(components::ACCENT), header].spacing(2);

    if let Some(snapshot) = &state.dashboard_snapshot {
        for peer in sorted_peers(state, &snapshot.peers) {
            let selected = state.dashboard_selected_peer_id == Some(peer.id);
            let ping = peer
                .ping_time
                .map(|v| format!("{v:.3}s"))
                .unwrap_or_else(|| "-".to_string());

            let row_line = row![
                text(peer.id.to_string()).width(iced::Length::FillPortion(1)),
                text(peer.addr.clone()).width(iced::Length::FillPortion(3)),
                text(peer.subver.clone()).width(iced::Length::FillPortion(3)),
                text(if peer.inbound { "IN" } else { "OUT" })
                    .color(if peer.inbound {
                        components::AMBER
                    } else {
                        components::ACCENT_ALT
                    })
                    .width(iced::Length::FillPortion(1)),
                text(peer.connection_type.clone()).width(iced::Length::FillPortion(2)),
                text(ping).width(iced::Length::FillPortion(1)),
            ]
            .spacing(4);

            rows = rows.push(
                button(row_line)
                    .width(Fill)
                    .style(components::row_button_style(selected))
                    .padding([1, 4])
                    .on_press(Message::DashboardPeerSelected(peer.id)),
            );
        }
    } else {
        rows = rows.push(text("No peer data").color(components::MUTED));
    }

    let detail_panel = if let Some(snapshot) = &state.dashboard_snapshot
        && let Some(selected_id) = state.dashboard_selected_peer_id
        && let Some(raw) = snapshot.peer_details.get(&selected_id)
    {
        let rendered = serde_json::to_string_pretty(raw)
            .unwrap_or_else(|_| "{\"error\":\"failed to format peer\"}".to_string());
        Some(
            container(
                column![
                    row![
                        text(format!("PEER {selected_id} DETAIL"))
                            .size(14)
                            .color(components::ACCENT),
                        button(text("Close").color(components::MUTED))
                            .style(components::utility_button_style(false))
                            .on_press(Message::DashboardPeerDetailClosed),
                    ]
                    .spacing(8),
                    scrollable(text(rendered).size(12).color(components::TEXT)).height(170),
                ]
                .spacing(6),
            )
            .style(components::card_style())
            .padding(8)
            .width(Fill),
        )
    } else {
        None
    };

    let table = scrollable(rows).height(Fill);
    if let Some(detail) = detail_panel {
        column![table, detail].spacing(6).into()
    } else {
        table.into()
    }
}

fn summary_card<'a>(
    title: &'a str,
    lines: Vec<(&'a str, String)>,
) -> iced::widget::Container<'a, Message> {
    let mut content = column![
        text(title.to_uppercase())
            .size(14)
            .color(components::ACCENT)
    ]
    .spacing(3);
    for (key, value) in lines {
        content = content.push(
            row![
                text(format!("{key}:"))
                    .color(components::MUTED)
                    .width(iced::Length::FillPortion(2)),
                text(value)
                    .color(components::TEXT)
                    .width(iced::Length::FillPortion(5))
            ]
            .spacing(3),
        );
    }
    container(content)
        .padding(8)
        .style(components::card_style())
}

fn sort_header<'a>(
    state: &State,
    label: &'a str,
    field: PeerSortField,
) -> iced::widget::Button<'a, Message> {
    let active = state.dashboard_peer_sort == field;
    let marker = if active {
        if state.dashboard_peer_sort_desc {
            " \u{25BC}"
        } else {
            " \u{25B2}"
        }
    } else {
        ""
    };
    button(text(format!("{label}{marker}")).size(11).color(if active {
        components::ACCENT
    } else {
        components::MUTED
    }))
    .style(components::table_header_button_style(active))
    .padding([1, 2])
    .on_press(Message::DashboardPeerSortPressed(field))
}

fn sorted_peers<'a>(state: &State, peers: &'a [PeerSummary]) -> Vec<&'a PeerSummary> {
    let mut sorted: Vec<&PeerSummary> = peers.iter().collect();
    sorted.sort_by(|a, b| match state.dashboard_peer_sort {
        PeerSortField::Id => a.id.cmp(&b.id),
        PeerSortField::Address => a.addr.cmp(&b.addr),
        PeerSortField::Direction => a.inbound.cmp(&b.inbound),
        PeerSortField::ConnectionType => a.connection_type.cmp(&b.connection_type),
        PeerSortField::Ping => {
            let ap = a.ping_time.unwrap_or(f64::INFINITY);
            let bp = b.ping_time.unwrap_or(f64::INFINITY);
            ap.partial_cmp(&bp).unwrap_or(std::cmp::Ordering::Equal)
        }
    });
    if state.dashboard_peer_sort_desc {
        sorted.reverse();
    }
    sorted
}

fn format_event_time(timestamp: u64) -> String {
    DateTime::from_timestamp(timestamp as i64, 0)
        .map(|dt: DateTime<Utc>| dt.format("%Y-%m-%d %H:%M:%S").to_string())
        .unwrap_or_else(|| timestamp.to_string())
}
