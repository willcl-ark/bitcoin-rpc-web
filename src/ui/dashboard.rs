use chrono::{DateTime, Utc};
use iced::widget::{button, column, container, horizontal_space, row, scrollable, text};
use iced::{Color, Element, Fill};
use iced::{alignment, widget::text::Wrapping};
use serde_json::Value;

use crate::app::message::Message;
use crate::app::state::{PeerSortField, State};
use crate::core::dashboard_service::PeerSummary;
use crate::ui::components;

pub fn view(state: &State) -> Element<'_, Message> {
    let zmq_status = if state.zmq.connected {
        format!("connected ({})", state.zmq.connected_address)
    } else if state.zmq.connected_address.is_empty() {
        "disabled".to_string()
    } else {
        format!("disconnected ({})", state.zmq.connected_address)
    };

    let top_strip: Element<'_, Message> = if let Some(snapshot) = &state.dashboard.snapshot {
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
                if state.dashboard.in_flight {
                    "syncing".to_string()
                } else {
                    "idle".to_string()
                },
            ),
            ("events seen", state.zmq.events_seen.to_string()),
            (
                "last topic",
                state.zmq.last_topic.as_deref().unwrap_or("-").to_string(),
            ),
            (
                "last event unix",
                state
                    .zmq
                    .last_event_at
                    .map(|v| v.to_string())
                    .unwrap_or_else(|| "-".to_string()),
            ),
        ],
    );

    let mut live_rows = column![
        row![
            text("TOPIC")
                .width(iced::Length::Fixed(110.0))
                .color(components::MUTED),
            text("EVENT")
                .width(Fill)
                .color(components::MUTED)
                .wrapping(Wrapping::None),
            text("TIME")
                .width(iced::Length::Fixed(95.0))
                .align_x(alignment::Horizontal::Right)
                .color(components::MUTED)
                .wrapping(Wrapping::None),
        ]
        .spacing(4)
    ]
    .spacing(2);

    if state.zmq.recent_events.is_empty() {
        live_rows = live_rows.push(text("No ZMQ events yet.").color(components::MUTED));
    } else {
        for evt in state.zmq.recent_events.iter().rev() {
            live_rows = live_rows.push(
                row![
                    text(&evt.topic)
                        .color(components::ACCENT)
                        .width(iced::Length::Fixed(110.0))
                        .wrapping(Wrapping::None),
                    text(&evt.event_hash).width(Fill).wrapping(Wrapping::None),
                    text(format_event_time(evt.timestamp))
                        .width(iced::Length::Fixed(95.0))
                        .align_x(alignment::Horizontal::Right)
                        .wrapping(Wrapping::None),
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
                    scrollable(live_rows)
                        .height(160)
                        .direction(scrollable::Direction::Vertical(
                            scrollable::Scrollbar::new()
                                .width(6)
                                .scroller_width(6)
                                .spacing(2),
                        ))
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

    if let Some(error) = &state.dashboard.error {
        root = root.push(text(format!("ERR: {error}")).color(components::ERROR_RED));
    }

    container(root).padding(12).width(Fill).height(Fill).into()
}

fn peer_table(state: &State) -> Element<'_, Message> {
    let header = row![
        sort_header(state, "ID", PeerSortField::Id).width(iced::Length::FillPortion(1)),
        sort_header(state, "Address", PeerSortField::Address).width(iced::Length::FillPortion(3)),
        sort_header(state, "Subver", PeerSortField::Subversion).width(iced::Length::FillPortion(3)),
        sort_header(state, "Dir", PeerSortField::Direction).width(iced::Length::FillPortion(1)),
        sort_header(state, "Type", PeerSortField::ConnectionType)
            .width(iced::Length::FillPortion(2)),
        sort_header(state, "Ping", PeerSortField::Ping).width(iced::Length::FillPortion(1)),
    ]
    .spacing(4);

    let mut rows = column![text("PEERS").size(15).color(components::ACCENT), header].spacing(2);

    if let Some(snapshot) = &state.dashboard.snapshot {
        let subver_scale = subversion_major_scale(&snapshot.peers);
        for peer in sorted_peers(state, &snapshot.peers) {
            let selected = state.dashboard.selected_peer_id == Some(peer.id);
            let ping = peer
                .ping_time
                .map(|v| format!("{v:.3}s"))
                .unwrap_or_else(|| "-".to_string());
            let ping_color = ping_color(peer.ping_time);
            let connection_type_color = connection_type_color(&peer.connection_type);
            let subver_color = subversion_color(&peer.subver, &subver_scale);

            let row_line = row![
                text(peer.id.to_string()).width(iced::Length::FillPortion(1)),
                text(peer.addr.clone()).width(iced::Length::FillPortion(3)),
                text(peer.subver.clone())
                    .color(subver_color)
                    .width(iced::Length::FillPortion(3)),
                text(if peer.inbound { "IN" } else { "OUT" })
                    .color(if peer.inbound {
                        components::AMBER
                    } else {
                        components::ACCENT_ALT
                    })
                    .width(iced::Length::FillPortion(1)),
                text(peer.connection_type.clone())
                    .color(connection_type_color)
                    .width(iced::Length::FillPortion(2)),
                text(ping)
                    .color(ping_color)
                    .width(iced::Length::FillPortion(1)),
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

    let detail_panel = if let Some(snapshot) = &state.dashboard.snapshot
        && let Some(selected_id) = state.dashboard.selected_peer_id
        && let Some(raw) = snapshot.peer_details.get(&selected_id)
    {
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
                    scrollable(peer_detail_grid(raw)).height(170),
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
                text(format!("{key}:")).color(components::MUTED),
                horizontal_space(),
                text(value).color(components::TEXT)
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
    let active = state.dashboard.peer_sort == field;
    let marker = if active {
        if state.dashboard.peer_sort_desc {
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
    sorted.sort_by(|a, b| match state.dashboard.peer_sort {
        PeerSortField::Id => a.id.cmp(&b.id),
        PeerSortField::Address => a.addr.cmp(&b.addr),
        PeerSortField::Subversion => a.subver.cmp(&b.subver),
        PeerSortField::Direction => a.inbound.cmp(&b.inbound),
        PeerSortField::ConnectionType => a.connection_type.cmp(&b.connection_type),
        PeerSortField::Ping => {
            let ap = a.ping_time.unwrap_or(f64::INFINITY);
            let bp = b.ping_time.unwrap_or(f64::INFINITY);
            ap.partial_cmp(&bp).unwrap_or(std::cmp::Ordering::Equal)
        }
    });
    if state.dashboard.peer_sort_desc {
        sorted.reverse();
    }
    sorted
}

fn format_event_time(timestamp: u64) -> String {
    DateTime::from_timestamp(timestamp as i64, 0)
        .map(|dt: DateTime<Utc>| dt.format("%H:%M:%S").to_string())
        .unwrap_or_else(|| timestamp.to_string())
}

fn peer_detail_grid<'a>(raw: &'a Value) -> Element<'a, Message> {
    let items = peer_detail_items(raw);
    if items.is_empty() {
        return text("No peer detail available.")
            .color(components::MUTED)
            .into();
    }

    let mut grid = column![].spacing(4);
    for chunk in items.chunks(3) {
        let mut line = row![].spacing(10);
        for (key, value) in chunk {
            line = line.push(
                container(
                    column![
                        text(key.to_uppercase()).size(11).color(components::MUTED),
                        text(value.clone()).size(12).color(components::TEXT)
                    ]
                    .spacing(1),
                )
                .width(iced::Length::FillPortion(1))
                .padding([2, 4]),
            );
        }
        for _ in chunk.len()..3 {
            line = line.push(container(text("")).width(iced::Length::FillPortion(1)));
        }
        grid = grid.push(line);
    }

    grid.into()
}

fn peer_detail_items(raw: &Value) -> Vec<(String, String)> {
    let Some(obj) = raw.as_object() else {
        return Vec::new();
    };

    let priority_keys = [
        "id",
        "addr",
        "subver",
        "network",
        "connection_type",
        "inbound",
        "version",
        "servicesnames",
        "permissions",
        "pingtime",
        "minping",
        "lastsend",
        "lastrecv",
        "bytessent",
        "bytesrecv",
        "mapped_as",
        "synced_headers",
        "synced_blocks",
        "startingheight",
        "timeoffset",
        "relaytxes",
        "presynced_headers",
        "addrbind",
        "addrlocal",
    ];

    let mut out = Vec::new();
    for key in priority_keys {
        if let Some(value) = obj.get(key) {
            out.push((key.to_string(), compact_value(value)));
        }
    }

    for (key, value) in obj {
        if priority_keys.contains(&key.as_str()) {
            continue;
        }
        out.push((key.clone(), compact_value(value)));
    }

    out
}

fn compact_value(value: &Value) -> String {
    match value {
        Value::Null => "-".to_string(),
        Value::Bool(v) => {
            if *v {
                "true".to_string()
            } else {
                "false".to_string()
            }
        }
        Value::Number(n) => n.to_string(),
        Value::String(s) => s.clone(),
        Value::Array(values) => {
            if values.is_empty() {
                "[]".to_string()
            } else if values.len() <= 4 {
                values
                    .iter()
                    .map(compact_value)
                    .collect::<Vec<_>>()
                    .join(", ")
            } else {
                format!("[{} items]", values.len())
            }
        }
        Value::Object(map) => {
            if map.is_empty() {
                "{}".to_string()
            } else {
                let mut sample = map
                    .iter()
                    .take(3)
                    .map(|(k, v)| format!("{k}:{}", compact_value(v)))
                    .collect::<Vec<_>>()
                    .join(", ");
                if map.len() > 3 {
                    sample.push_str(&format!(" … ({} keys)", map.len()));
                }
                sample
            }
        }
    }
}

fn connection_type_color(kind: &str) -> Color {
    match kind.to_ascii_lowercase().as_str() {
        "inbound" => Color::from_rgb(0.30, 0.84, 1.0),
        "manual" => Color::from_rgb(0.96, 0.79, 0.27),
        "feeler" => Color::from_rgb(0.63, 0.83, 1.0),
        "outbound-full-relay" => components::GREEN,
        "block-relay-only" => Color::from_rgb(0.45, 0.76, 0.98),
        "addr-fetch" => Color::from_rgb(0.96, 0.70, 0.20),
        "private-broadcast" => Color::from_rgb(0.97, 0.54, 0.26),
        _ => components::TEXT,
    }
}

fn subversion_major_scale(peers: &[PeerSummary]) -> Vec<i64> {
    let mut versions = peers
        .iter()
        .filter_map(|peer| extract_subversion_major(&peer.subver))
        .collect::<Vec<_>>();
    versions.sort_unstable();
    versions.dedup();
    versions
}

fn extract_subversion_major(subver: &str) -> Option<i64> {
    let (_, after_colon) = subver.split_once(':')?;
    let major = after_colon
        .trim_start_matches('/')
        .split('.')
        .next()
        .unwrap_or_default()
        .trim_matches('/');
    major.parse::<i64>().ok()
}

fn subversion_color(subver: &str, scale: &[i64]) -> Color {
    let Some(major) = extract_subversion_major(subver) else {
        return components::TEXT;
    };
    let Some(idx) = scale.iter().position(|v| *v == major) else {
        return components::TEXT;
    };

    if scale.len() <= 1 {
        return components::ACCENT_ALT;
    }

    let red = components::ERROR_RED;
    let orange = components::AMBER;
    let green = components::GREEN;
    let t = idx as f32 / (scale.len() - 1) as f32;

    if t <= 0.5 {
        lerp_color(red, orange, t * 2.0)
    } else {
        lerp_color(orange, green, (t - 0.5) * 2.0)
    }
}

fn ping_color(ping_secs: Option<f64>) -> Color {
    let Some(ping) = ping_secs else {
        return components::MUTED;
    };

    let green = components::GREEN;
    let orange = components::AMBER;
    let red = components::ERROR_RED;

    if ping <= 2.0 {
        lerp_color(green, orange, (ping / 2.0) as f32)
    } else if ping < 5.0 {
        lerp_color(orange, red, ((ping - 2.0) / 3.0) as f32)
    } else {
        red
    }
}

fn lerp_color(a: Color, b: Color, t: f32) -> Color {
    let k = t.clamp(0.0, 1.0);
    Color::from_rgb(
        a.r + (b.r - a.r) * k,
        a.g + (b.g - a.g) * k,
        a.b + (b.b - a.b) * k,
    )
}
