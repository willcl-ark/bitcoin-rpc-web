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

    let mut left = column![
        text("Dashboard").size(32).color(components::TEXT),
        card(
            "ZMQ Feed",
            vec![
                format!("status: {zmq_status}"),
                format!(
                    "refresh state: {}",
                    if state.dashboard_in_flight {
                        "syncing"
                    } else {
                        "idle"
                    }
                ),
                format!("events seen: {}", state.zmq_events_seen),
                format!(
                    "last topic: {}",
                    state.zmq_last_topic.as_deref().unwrap_or("-")
                ),
                format!(
                    "last event unix: {}",
                    state
                        .zmq_last_event_at
                        .map(|v| v.to_string())
                        .unwrap_or_else(|| "-".to_string())
                ),
            ],
        )
    ]
    .spacing(10);

    if let Some(error) = &state.dashboard_error {
        left = left.push(card(
            "Refresh Error",
            vec![
                error.clone(),
                "Periodic refresh continues in background.".to_string(),
            ],
        ));
    }

    if let Some(snapshot) = &state.dashboard_snapshot {
        left = left
            .push(card(
                "Chain",
                vec![
                    format!("network: {}", snapshot.chain.chain),
                    format!("blocks: {}", snapshot.chain.blocks),
                    format!("headers: {}", snapshot.chain.headers),
                    format!("verification: {:.4}", snapshot.chain.verification_progress),
                ],
            ))
            .push(card(
                "Mempool",
                vec![
                    format!("transactions: {}", snapshot.mempool.transactions),
                    format!("bytes: {}", snapshot.mempool.bytes),
                    format!("usage: {}", snapshot.mempool.usage),
                    format!("max: {}", snapshot.mempool.maxmempool),
                ],
            ))
            .push(card(
                "Network",
                vec![
                    format!("version: {}", snapshot.network.version),
                    format!("subversion: {}", snapshot.network.subversion),
                    format!("protocol: {}", snapshot.network.protocol_version),
                    format!("connections: {}", snapshot.network.connections),
                    format!("uptime: {}s", snapshot.uptime_secs),
                ],
            ))
            .push(card(
                "Traffic",
                vec![
                    format!("recv: {} bytes", snapshot.traffic.total_bytes_recv),
                    format!("sent: {} bytes", snapshot.traffic.total_bytes_sent),
                ],
            ));
    } else if !state.dashboard_in_flight {
        left = left.push(text("No dashboard data yet."));
    }

    let peers = peer_list(state);
    let detail = peer_detail(state);

    let content = row![
        container(scrollable(left).height(Fill))
            .style(components::panel_style())
            .padding(14)
            .width(iced::Length::FillPortion(2))
            .height(Fill),
        container(peers)
            .style(components::panel_style())
            .padding(12)
            .width(iced::Length::FillPortion(2))
            .height(Fill),
        container(detail)
            .style(components::panel_style())
            .padding(12)
            .width(iced::Length::FillPortion(2))
            .height(Fill),
    ]
    .spacing(12)
    .height(Fill);

    container(content)
        .padding(12)
        .width(Fill)
        .height(Fill)
        .into()
}

fn peer_list(state: &State) -> Element<'_, Message> {
    let mut list = column![text("Peers").size(24).color(components::TEXT)].spacing(8);
    if let Some(snapshot) = &state.dashboard_snapshot {
        if snapshot.peers.is_empty() {
            list = list.push(text("No peers").color(components::MUTED));
        } else {
            for peer in &snapshot.peers {
                let selected = state.dashboard_selected_peer_id == Some(peer.id);
                let ping = peer
                    .ping_time
                    .map(|v| format!("{v:.3}s"))
                    .unwrap_or_else(|| "-".to_string());
                let label = format!(
                    "{} {} ({})  ping {}",
                    if selected { "selected" } else { "peer" },
                    peer.id,
                    peer.addr,
                    ping
                );
                list = list.push(
                    button(text(label))
                        .width(Fill)
                        .style(components::nav_button_style(selected))
                        .on_press(Message::DashboardPeerSelected(peer.id)),
                );
            }
        }
    } else {
        list = list.push(text("No peer data").color(components::MUTED));
    }

    scrollable(list).into()
}

fn peer_detail(state: &State) -> Element<'_, Message> {
    let mut detail = column![text("Peer Detail").size(24).color(components::TEXT)].spacing(8);
    if let Some(snapshot) = &state.dashboard_snapshot {
        let selected = state
            .dashboard_selected_peer_id
            .and_then(|id| snapshot.peers.iter().find(|peer| peer.id == id));

        if let Some(peer) = selected {
            detail = detail.push(card(
                "Selected Peer",
                vec![
                    format!("id: {}", peer.id),
                    format!("addr: {}", peer.addr),
                    format!("inbound: {}", peer.inbound),
                    format!("type: {}", peer.connection_type),
                    format!(
                        "ping: {}",
                        peer.ping_time
                            .map(|v| format!("{v:.6}"))
                            .unwrap_or_else(|| "-".to_string())
                    ),
                ],
            ));
        } else {
            detail = detail.push(text("Select a peer from the list.").color(components::MUTED));
        }
    } else {
        detail = detail.push(text("No peer data").color(components::MUTED));
    }

    scrollable(detail).into()
}

fn card<'a>(title: &'a str, lines: Vec<String>) -> Element<'a, Message> {
    let mut content = column![text(title).size(21).color(components::TEXT)].spacing(6);
    for line in lines {
        content = content.push(text(line).color(components::MUTED));
    }
    container(content)
        .padding(12)
        .style(components::card_style())
        .into()
}
