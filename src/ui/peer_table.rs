use std::time::SystemTime;

use iced::widget::{button, column, container, row, scrollable, text};
use iced::{Color, Element, Fill};
use iced::{alignment, widget::text::Wrapping};
use serde_json::Value;

use crate::app::message::Message;
use crate::app::state::{PeerSortField, State};
use crate::core::dashboard_service::PeerSummary;
use crate::ui::components::{self, ColorTheme};

pub fn peer_table(state: &State) -> Element<'_, Message> {
    let level = state.dashboard.netinfo_level;
    let fs = state.config.runtime.font_size;
    let colors = &state.colors;

    macro_rules! cell {
        ($content:expr, $w:expr) => {
            text(($content).to_string())
                .size(fs)
                .width(iced::Length::Fixed($w))
                .wrapping(Wrapping::None)
        };
    }

    let mut level_btns = row![text("PEERS").size(fs + 1).color(colors.accent)]
        .spacing(6)
        .align_y(alignment::Vertical::Center);
    for i in 0..=4u8 {
        level_btns = level_btns.push(
            button(text(i.to_string()).size(fs).color(if i == level {
                colors.accent
            } else {
                colors.fg_dim
            }))
            .style(components::utility_button_style(colors, i == level))
            .padding([1, 6])
            .on_press(Message::NetinfoLevelChanged(i)),
        );
    }

    let mut content = column![level_btns].spacing(2);

    if let Some(snapshot) = &state.dashboard.snapshot {
        if level >= 1 {
            let now = SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .map(|d: std::time::Duration| d.as_secs() as i64)
                .unwrap_or(0);

            let mut header = row![].spacing(2);
            header = header
                .push(
                    sort_header(colors, state, "<->", PeerSortField::Direction)
                        .width(iced::Length::Fixed(35.0)),
                )
                .push(
                    sort_header(colors, state, "type", PeerSortField::ConnectionType)
                        .width(iced::Length::Fixed(65.0)),
                )
                .push(
                    sort_header(colors, state, "net", PeerSortField::Network)
                        .width(iced::Length::Fixed(55.0)),
                )
                .push(cell!("serv", 60.0).color(colors.fg_dim))
                .push(cell!("v", 22.0).color(colors.fg_dim))
                .push(
                    sort_header(colors, state, "mping", PeerSortField::MinPing)
                        .width(iced::Length::Fixed(68.0)),
                )
                .push(
                    sort_header(colors, state, "ping", PeerSortField::Ping)
                        .width(iced::Length::Fixed(60.0)),
                )
                .push(cell!("send", 50.0).color(colors.fg_dim))
                .push(cell!("recv", 50.0).color(colors.fg_dim))
                .push(cell!("txn", 45.0).color(colors.fg_dim))
                .push(cell!("blk", 45.0).color(colors.fg_dim))
                .push(cell!("hb", 28.0).color(colors.fg_dim))
                .push(cell!("addrp", 55.0).color(colors.fg_dim))
                .push(cell!("addrl", 50.0).color(colors.fg_dim))
                .push(
                    sort_header(colors, state, "age", PeerSortField::Age)
                        .width(iced::Length::Fixed(50.0)),
                )
                .push(
                    sort_header(colors, state, "id", PeerSortField::Id)
                        .width(iced::Length::Fixed(38.0)),
                );
            if level == 2 || level == 4 {
                header = header.push(
                    sort_header(colors, state, "address", PeerSortField::Address).width(Fill),
                );
            }
            if level == 3 || level == 4 {
                header = header.push(
                    sort_header(colors, state, "version", PeerSortField::Version).width(Fill),
                );
            }
            content = content.push(header);

            let mut units = row![].spacing(2);
            units = units
                .push(cell!("", 35.0))
                .push(cell!("", 65.0))
                .push(cell!("", 55.0))
                .push(cell!("", 60.0))
                .push(cell!("", 22.0))
                .push(cell!("ms", 68.0).color(colors.fg_dim))
                .push(cell!("ms", 60.0).color(colors.fg_dim))
                .push(cell!("sec", 50.0).color(colors.fg_dim))
                .push(cell!("sec", 50.0).color(colors.fg_dim))
                .push(cell!("min", 45.0).color(colors.fg_dim))
                .push(cell!("min", 45.0).color(colors.fg_dim))
                .push(cell!("", 28.0))
                .push(cell!("", 55.0))
                .push(cell!("", 50.0))
                .push(cell!("min", 50.0).color(colors.fg_dim))
                .push(cell!("", 38.0));
            content = content.push(units);

            for peer in sorted_peers(state, &snapshot.peers) {
                let selected = state.dashboard.selected_peer_id == Some(peer.id);
                let type_short = connection_type_short(&peer.connection_type);
                let type_color = connection_type_color(colors, &peer.connection_type);
                let dir_color = if peer.inbound {
                    colors.orange
                } else {
                    colors.blue
                };
                let hb = match (peer.is_bip152_hb_to, peer.is_bip152_hb_from) {
                    (true, true) => ".*",
                    (true, false) => ". ",
                    (false, true) => " *",
                    (false, false) => "  ",
                };
                let txn = if !peer.is_tx_relay {
                    "*".to_string()
                } else {
                    relative_mins(now, peer.last_transaction)
                };
                let addrp = if !peer.is_addr_relay_enabled {
                    ".".to_string()
                } else {
                    peer.addr_processed.to_string()
                };
                let addrl = if peer.addr_rate_limited > 0 {
                    peer.addr_rate_limited.to_string()
                } else {
                    String::new()
                };

                let mut data_row = row![].spacing(2);
                data_row = data_row
                    .push(cell!(if peer.inbound { "in" } else { "out" }, 35.0).color(dir_color))
                    .push(cell!(type_short, 65.0).color(type_color))
                    .push(cell!(&peer.network, 55.0))
                    .push(cell!(&peer.services, 60.0))
                    .push(cell!(peer.transport_version, 22.0))
                    .push(
                        cell!(ping_ms_string(peer.min_ping), 68.0)
                            .color(ping_color(colors, peer.min_ping)),
                    )
                    .push(
                        cell!(ping_ms_string(peer.ping_time), 60.0)
                            .color(ping_color(colors, peer.ping_time)),
                    )
                    .push(cell!(relative_secs(now, peer.last_send), 50.0))
                    .push(cell!(relative_secs(now, peer.last_recv), 50.0))
                    .push(cell!(txn, 45.0))
                    .push(cell!(relative_mins(now, peer.last_block), 45.0))
                    .push(cell!(hb, 28.0))
                    .push(cell!(addrp, 55.0))
                    .push(cell!(addrl, 50.0))
                    .push(cell!(relative_mins(now, peer.conn_time), 50.0))
                    .push(cell!(peer.id, 38.0));
                if level == 2 || level == 4 {
                    data_row = data_row.push(
                        text(peer.addr.clone())
                            .size(fs)
                            .width(Fill)
                            .wrapping(Wrapping::None),
                    );
                }
                if level == 3 || level == 4 {
                    data_row = data_row.push(
                        text(format!("{}{}", peer.version, peer.subver))
                            .size(fs)
                            .width(Fill)
                            .wrapping(Wrapping::None),
                    );
                }

                content = content.push(
                    button(data_row)
                        .width(Fill)
                        .style(components::row_button_style(colors, selected))
                        .padding([1, 4])
                        .on_press(Message::DashboardPeerSelected(peer.id)),
                );
            }
        }

        content = content.push(connection_counts(colors, &snapshot.peers, fs));
    } else {
        content = content.push(text("No peer data").size(fs).color(colors.fg_dim));
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
                            .size(fs)
                            .color(colors.accent),
                        button(text("Close").size(fs).color(colors.fg_dim))
                            .style(components::utility_button_style(colors, false))
                            .on_press(Message::DashboardPeerDetailClosed),
                    ]
                    .spacing(8),
                    scrollable(peer_detail_grid(colors, raw, fs)).height(170),
                ]
                .spacing(6),
            )
            .style(components::card_style(colors))
            .padding(8)
            .width(Fill),
        )
    } else {
        None
    };

    let table = scrollable(content).height(Fill);
    if let Some(detail) = detail_panel {
        column![table, detail].spacing(6).into()
    } else {
        table.into()
    }
}

fn sort_header<'a>(
    colors: &ColorTheme,
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
    let fs = state.config.runtime.font_size;
    button(text(format!("{label}{marker}")).size(fs).color(if active {
        colors.accent
    } else {
        colors.fg_dim
    }))
    .style(components::table_header_button_style(colors, active))
    .padding([1, 2])
    .on_press(Message::DashboardPeerSortPressed(field))
}

fn sorted_peers<'a>(state: &State, peers: &'a [PeerSummary]) -> Vec<&'a PeerSummary> {
    let mut sorted: Vec<&PeerSummary> = peers.iter().collect();
    sorted.sort_by(|a, b| {
        let cmp = match state.dashboard.peer_sort {
            PeerSortField::Id => a.id.cmp(&b.id),
            PeerSortField::Direction => a.inbound.cmp(&b.inbound),
            PeerSortField::ConnectionType => a.connection_type.cmp(&b.connection_type),
            PeerSortField::Network => a.network.cmp(&b.network),
            PeerSortField::MinPing => {
                let ap = a.min_ping.unwrap_or(f64::INFINITY);
                let bp = b.min_ping.unwrap_or(f64::INFINITY);
                a.inbound
                    .cmp(&b.inbound)
                    .then(ap.partial_cmp(&bp).unwrap_or(std::cmp::Ordering::Equal))
            }
            PeerSortField::Ping => {
                let ap = a.ping_time.unwrap_or(f64::INFINITY);
                let bp = b.ping_time.unwrap_or(f64::INFINITY);
                ap.partial_cmp(&bp).unwrap_or(std::cmp::Ordering::Equal)
            }
            PeerSortField::Age => a.conn_time.cmp(&b.conn_time),
            PeerSortField::Address => a.addr.cmp(&b.addr),
            PeerSortField::Version => a.version.cmp(&b.version),
        };
        if state.dashboard.peer_sort_desc {
            cmp.reverse()
        } else {
            cmp
        }
    });
    sorted
}

fn peer_detail_grid<'a>(colors: &ColorTheme, raw: &'a Value, fs: u16) -> Element<'a, Message> {
    let items = peer_detail_items(raw);
    if items.is_empty() {
        return text("No peer detail available.")
            .size(fs)
            .color(colors.fg_dim)
            .into();
    }

    let mut grid = column![].spacing(4);
    for chunk in items.chunks(3) {
        let mut line = row![].spacing(10);
        for (key, value) in chunk {
            line = line.push(
                container(
                    column![
                        text(key.to_uppercase())
                            .size(fs.saturating_sub(3))
                            .color(colors.fg_dim),
                        text(value.clone())
                            .size(fs.saturating_sub(2))
                            .color(colors.fg)
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

fn connection_type_short(kind: &str) -> &str {
    match kind {
        "outbound-full-relay" => "full",
        "block-relay-only" => "block",
        "addr-fetch" => "addr",
        "inbound" => "in",
        _ => kind,
    }
}

fn connection_type_color(colors: &ColorTheme, kind: &str) -> Color {
    match kind {
        "outbound-full-relay" => colors.green,
        "block-relay-only" => colors.blue,
        "manual" => colors.yellow,
        "feeler" => colors.cyan,
        "addr-fetch" => colors.orange,
        "inbound" => colors.cyan,
        _ => colors.fg,
    }
}

fn ping_ms_string(secs: Option<f64>) -> String {
    secs.map(|s| format!("{:.0}", s * 1000.0))
        .unwrap_or_default()
}

fn relative_secs(now: i64, ts: i64) -> String {
    if ts == 0 {
        return String::new();
    }
    (now - ts).to_string()
}

fn relative_mins(now: i64, ts: i64) -> String {
    if ts == 0 {
        return String::new();
    }
    ((now - ts) / 60).to_string()
}

fn connection_counts<'a>(
    colors: &ColorTheme,
    peers: &[PeerSummary],
    fs: u16,
) -> Element<'a, Message> {
    let known_nets = ["ipv4", "ipv6", "onion", "i2p", "cjdns"];
    let active_nets: Vec<&str> = known_nets
        .iter()
        .copied()
        .filter(|net| peers.iter().any(|p| p.network == *net))
        .collect();

    let w_label: f32 = 60.0;
    let w_col: f32 = 55.0;

    let count = |net: &str, inbound: Option<bool>| -> usize {
        peers
            .iter()
            .filter(|p| p.network == net && inbound.is_none_or(|ib| p.inbound == ib))
            .count()
    };

    let fg_dim = colors.fg_dim;
    let fg = colors.fg;

    let mut header = row![].spacing(2);
    header = header.push(text("").width(iced::Length::Fixed(w_label)));
    for net in &active_nets {
        header = header.push(
            text(*net)
                .size(fs)
                .color(fg_dim)
                .width(iced::Length::Fixed(w_col))
                .align_x(alignment::Horizontal::Right),
        );
    }
    header = header
        .push(
            text("total")
                .size(fs)
                .color(fg_dim)
                .width(iced::Length::Fixed(w_col))
                .align_x(alignment::Horizontal::Right),
        )
        .push(
            text("block")
                .size(fs)
                .color(fg_dim)
                .width(iced::Length::Fixed(w_col))
                .align_x(alignment::Horizontal::Right),
        );

    let mut grid = column![header].spacing(1);

    for (label, dir) in [("in", Some(true)), ("out", Some(false)), ("total", None)] {
        let mut r = row![].spacing(2);
        r = r.push(
            text(label)
                .size(fs)
                .color(fg_dim)
                .width(iced::Length::Fixed(w_label)),
        );
        let mut total = 0usize;
        for net in &active_nets {
            let c = count(net, dir);
            total += c;
            r = r.push(
                text(c.to_string())
                    .size(fs)
                    .color(fg)
                    .width(iced::Length::Fixed(w_col))
                    .align_x(alignment::Horizontal::Right),
            );
        }
        r = r.push(
            text(total.to_string())
                .size(fs)
                .color(fg)
                .width(iced::Length::Fixed(w_col))
                .align_x(alignment::Horizontal::Right),
        );
        let bc = if label == "out" {
            peers
                .iter()
                .filter(|p| p.connection_type == "block-relay-only" && !p.inbound)
                .count()
        } else {
            0
        };
        r = r.push(
            text(if bc > 0 {
                bc.to_string()
            } else {
                String::new()
            })
            .size(fs)
            .color(fg)
            .width(iced::Length::Fixed(w_col))
            .align_x(alignment::Horizontal::Right),
        );
        grid = grid.push(r);
    }

    grid.into()
}

fn ping_color(colors: &ColorTheme, ping_secs: Option<f64>) -> Color {
    let Some(ping) = ping_secs else {
        return colors.fg_dim;
    };
    if ping <= 0.25 {
        colors.green
    } else if ping <= 0.75 {
        colors.orange
    } else {
        colors.red
    }
}
