use iced::widget::{button, column, container, row, scrollable, text};
use iced::{Element, Fill};

use crate::app::message::Message;
use crate::app::state::State;

pub fn view(state: &State) -> Element<'_, Message> {
    let mut left = column![text("Dashboard").size(26)].spacing(10);

    if state.dashboard_in_flight {
        left = left.push(text("Refreshing..."));
    }
    if let Some(error) = &state.dashboard_error {
        left = left.push(text(format!("Error: {error}")));
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
        scrollable(left)
            .width(iced::Length::FillPortion(2))
            .height(Fill),
        container(peers)
            .width(iced::Length::FillPortion(2))
            .height(Fill),
        container(detail)
            .width(iced::Length::FillPortion(2))
            .height(Fill),
    ]
    .spacing(12)
    .height(Fill);

    container(content)
        .padding(24)
        .width(Fill)
        .height(Fill)
        .into()
}

fn peer_list(state: &State) -> Element<'_, Message> {
    let mut list = column![text("Peers").size(22)].spacing(8);
    if let Some(snapshot) = &state.dashboard_snapshot {
        if snapshot.peers.is_empty() {
            list = list.push(text("No peers"));
        } else {
            for peer in &snapshot.peers {
                let selected = state.dashboard_selected_peer_id == Some(peer.id);
                let ping = peer
                    .ping_time
                    .map(|v| format!("{v:.3}s"))
                    .unwrap_or_else(|| "-".to_string());
                let label = format!(
                    "{} {} ({}) ping {}",
                    if selected { ">" } else { "-" },
                    peer.id,
                    peer.addr,
                    ping
                );
                list = list.push(
                    button(text(label))
                        .width(Fill)
                        .on_press(Message::DashboardPeerSelected(peer.id)),
                );
            }
        }
    } else {
        list = list.push(text("No peer data"));
    }

    scrollable(list).into()
}

fn peer_detail(state: &State) -> Element<'_, Message> {
    let mut detail = column![text("Peer Detail").size(22)].spacing(8);
    if let Some(snapshot) = &state.dashboard_snapshot {
        let selected = state
            .dashboard_selected_peer_id
            .and_then(|id| snapshot.peers.iter().find(|peer| peer.id == id));

        if let Some(peer) = selected {
            detail = detail
                .push(text(format!("id: {}", peer.id)))
                .push(text(format!("addr: {}", peer.addr)))
                .push(text(format!("inbound: {}", peer.inbound)))
                .push(text(format!("type: {}", peer.connection_type)))
                .push(text(format!(
                    "ping: {}",
                    peer.ping_time
                        .map(|v| format!("{v:.6}"))
                        .unwrap_or_else(|| "-".to_string())
                )));
        } else {
            detail = detail.push(text("Select a peer from the list."));
        }
    } else {
        detail = detail.push(text("No peer data"));
    }

    scrollable(detail).into()
}

fn card<'a>(title: &'a str, lines: Vec<String>) -> Element<'a, Message> {
    let mut content = column![text(title).size(20)].spacing(6);
    for line in lines {
        content = content.push(text(line));
    }
    container(content).padding(12).into()
}
