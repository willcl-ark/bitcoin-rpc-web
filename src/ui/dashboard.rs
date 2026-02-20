use chrono::{DateTime, Utc};
use iced::widget::{column, container, horizontal_space, pane_grid, row, scrollable, text};
use iced::{Element, Fill};
use iced::{alignment, widget::text::Wrapping};

use crate::app::message::Message;
use crate::app::state::{DashboardPane, State};
use crate::ui::components::{self, ColorTheme};
use crate::ui::peer_table;

pub fn view(state: &State) -> Element<'_, Message> {
    let fs = state.config.runtime.font_size;
    let colors = &state.colors;

    let top_strip: Element<'_, Message> = if let Some(snapshot) = &state.dashboard.snapshot {
        let chain_fields = vec![
            ("network", snapshot.chain.chain.clone()),
            ("blocks", snapshot.chain.blocks.to_string()),
            ("headers", snapshot.chain.headers.to_string()),
            (
                "verification",
                format!("{:.4}%", snapshot.chain.verification_progress * 100.0),
            ),
            ("difficulty", format_difficulty(snapshot.chain.difficulty)),
        ];
        let mempool_fields = vec![
            ("transactions", snapshot.mempool.transactions.to_string()),
            ("bytes", snapshot.mempool.bytes.to_string()),
            ("usage", snapshot.mempool.usage.to_string()),
            ("max", snapshot.mempool.maxmempool.to_string()),
        ];
        let network_fields = {
            let n = &snapshot.network;
            let mut fields = vec![
                ("version", n.version.to_string()),
                ("subversion", n.subversion.clone()),
                (
                    "connections",
                    format!(
                        "in {}, out {}, total {}",
                        n.connections_in, n.connections_out, n.connections
                    ),
                ),
                ("time offset", format!("{}s", n.timeoffset)),
                ("uptime", format!("{}s", snapshot.uptime_secs)),
                ("relay fee", format!("{:.8} BTC/kvB", n.relayfee)),
            ];
            if !n.proxies.is_empty() {
                fields.push(("proxies", n.proxies.clone()));
            }
            fields
        };
        let traffic_fields = vec![
            (
                "recv",
                format!("{} bytes", snapshot.traffic.total_bytes_recv),
            ),
            (
                "sent",
                format!("{} bytes", snapshot.traffic.total_bytes_sent),
            ),
        ];

        let max_lines = chain_fields
            .len()
            .max(mempool_fields.len())
            .max(network_fields.len())
            .max(traffic_fields.len());

        row![
            summary_card(colors, "Chain", chain_fields, fs, max_lines)
                .width(iced::Length::FillPortion(1)),
            summary_card(colors, "Mempool", mempool_fields, fs, max_lines)
                .width(iced::Length::FillPortion(1)),
            summary_card(colors, "Network", network_fields, fs, max_lines)
                .width(iced::Length::FillPortion(1)),
            summary_card(colors, "Traffic", traffic_fields, fs, max_lines)
                .width(iced::Length::FillPortion(1)),
        ]
        .spacing(8)
        .into()
    } else {
        container(text("NO DASHBOARD DATA YET").size(fs).color(colors.fg_dim))
            .style(components::card_style(colors))
            .padding(14)
            .into()
    };

    let mut root = column![
        row![
            text("DASHBOARD").size(fs + 10).color(colors.accent),
            text("TELEMETRY + PEERING")
                .size(fs.saturating_sub(2))
                .color(colors.orange)
        ]
        .spacing(12),
        top_strip,
        dashboard_panes(state)
    ]
    .spacing(8)
    .height(Fill)
    .width(Fill);

    if let Some(error) = &state.dashboard.error {
        root = root.push(text(format!("ERR: {error}")).size(fs).color(colors.red));
    }

    container(root).padding(12).width(Fill).height(Fill).into()
}

fn dashboard_panes(state: &State) -> Element<'_, Message> {
    let colors = &state.colors;

    pane_grid::PaneGrid::new(&state.dashboard.panes, |_, pane, _| {
        let content = match pane {
            DashboardPane::Main => container(peer_table::peer_table(state))
                .style(components::panel_style(colors))
                .padding(8)
                .width(Fill)
                .height(Fill)
                .into(),
            DashboardPane::Zmq => zmq_panel(state),
        };

        pane_grid::Content::new(content)
    })
    .spacing(8)
    .on_resize(12, Message::DashboardPaneResized)
    .width(Fill)
    .height(Fill)
    .into()
}

fn zmq_panel(state: &State) -> Element<'_, Message> {
    let fs = state.config.runtime.font_size;
    let colors = &state.colors;

    let zmq_status = if state.zmq.connected {
        format!("connected ({})", state.zmq.connected_address)
    } else if state.zmq.connected_address.is_empty() {
        "disabled".to_string()
    } else {
        format!("disconnected ({})", state.zmq.connected_address)
    };

    let zmq_summary = summary_card(
        colors,
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
        fs,
        0,
    );

    let mut live_rows = column![
        row![
            text("TOPIC")
                .size(fs)
                .width(iced::Length::Fixed(110.0))
                .color(colors.fg_dim),
            text("EVENT")
                .size(fs)
                .width(Fill)
                .color(colors.fg_dim)
                .wrapping(Wrapping::None),
            text("TIME")
                .size(fs)
                .width(iced::Length::Fixed(95.0))
                .align_x(alignment::Horizontal::Right)
                .color(colors.fg_dim)
                .wrapping(Wrapping::None),
        ]
        .spacing(4)
    ]
    .spacing(2);

    if state.zmq.recent_events.is_empty() {
        live_rows = live_rows.push(text("No ZMQ events yet.").size(fs).color(colors.fg_dim));
    } else {
        for evt in state.zmq.recent_events.iter().rev() {
            live_rows = live_rows.push(
                row![
                    text(&evt.topic)
                        .size(fs)
                        .color(colors.accent)
                        .width(iced::Length::Fixed(110.0))
                        .wrapping(Wrapping::None),
                    text(&evt.event_hash)
                        .size(fs)
                        .width(Fill)
                        .wrapping(Wrapping::None),
                    text(format_event_time(evt.timestamp))
                        .size(fs)
                        .width(iced::Length::Fixed(95.0))
                        .align_x(alignment::Horizontal::Right)
                        .wrapping(Wrapping::None),
                ]
                .spacing(4),
            );
        }
    }

    container(
        row![
            container(zmq_summary)
                .style(components::card_style(colors))
                .padding(8)
                .height(Fill)
                .width(iced::Length::FillPortion(2)),
            container(
                column![
                    text("LIVE EVENTS").size(fs).color(colors.accent),
                    scrollable(live_rows)
                        .height(Fill)
                        .direction(scrollable::Direction::Vertical(
                            scrollable::Scrollbar::new()
                                .width(6)
                                .scroller_width(6)
                                .spacing(2),
                        ))
                ]
                .spacing(4)
            )
            .style(components::card_style(colors))
            .padding(8)
            .height(Fill)
            .width(iced::Length::FillPortion(5)),
        ]
        .spacing(8),
    )
    .style(components::panel_style(colors))
    .padding(8)
    .width(Fill)
    .height(Fill)
    .into()
}

fn summary_card<'a>(
    colors: &ColorTheme,
    title: &'a str,
    lines: Vec<(&'a str, String)>,
    fs: u16,
    max_lines: usize,
) -> iced::widget::Container<'a, Message> {
    let count = lines.len();
    let mut content = column![text(title.to_uppercase()).size(fs).color(colors.accent)].spacing(3);
    for (key, value) in lines {
        content = content.push(
            row![
                text(format!("{key}:")).size(fs).color(colors.fg_dim),
                horizontal_space(),
                text(value).size(fs).color(colors.fg)
            ]
            .spacing(3),
        );
    }
    for _ in count..max_lines {
        content = content.push(text(" ").size(fs));
    }
    container(content)
        .padding(8)
        .style(components::card_style(colors))
}

fn format_difficulty(d: f64) -> String {
    if d >= 1e15 {
        format!("{:.2}P", d / 1e15)
    } else if d >= 1e12 {
        format!("{:.2}T", d / 1e12)
    } else if d >= 1e9 {
        format!("{:.2}G", d / 1e9)
    } else if d >= 1e6 {
        format!("{:.2}M", d / 1e6)
    } else {
        format!("{:.1}", d)
    }
}

fn format_event_time(timestamp: u64) -> String {
    DateTime::from_timestamp(timestamp as i64, 0)
        .map(|dt: DateTime<Utc>| dt.format("%H:%M:%S").to_string())
        .unwrap_or_else(|| timestamp.to_string())
}
