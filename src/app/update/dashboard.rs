use std::time::{Duration, Instant};

use iced::Task;

use crate::app::constants::ZMQ_REFRESH_DEBOUNCE_MS;
use crate::app::message::Message;
use crate::app::state::{DashboardPartialSet, State};
use crate::core::dashboard_service::{DashboardPartialUpdate, DashboardService, DashboardSnapshot};
use crate::core::rpc_client::RpcClient;

pub fn handle_dashboard(state: &mut State, message: Message) -> Task<Message> {
    match message {
        Message::DashboardTick => {
            if state.dashboard.in_flight {
                return Task::none();
            }
            return start_dashboard_refresh(state);
        }
        Message::DashboardLoaded(rgen, result) => {
            if rgen != state.dashboard.request_gen {
                state.dashboard.in_flight = false;
                return Task::none();
            }
            state.dashboard.in_flight = false;
            match result {
                Ok(snapshot) => {
                    let selected_is_valid = state
                        .dashboard
                        .selected_peer_id
                        .is_some_and(|id| snapshot.peers.iter().any(|peer| peer.id == id));
                    if !selected_is_valid {
                        state.dashboard.selected_peer_id = None;
                    }
                    state.dashboard.snapshot = Some(snapshot);
                    state.dashboard.error = None;
                }
                Err(error) => {
                    state.dashboard.error = Some(error);
                }
            }
            return schedule_pending_partial_if_ready(state);
        }
        Message::DashboardPeerSelected(peer_id) => {
            state.dashboard.selected_peer_id = Some(peer_id);
        }
        Message::DashboardPeerDetailClosed => {
            state.dashboard.selected_peer_id = None;
        }
        Message::DashboardPeerSortPressed(field) => {
            if state.dashboard.peer_sort == field {
                state.dashboard.peer_sort_desc = !state.dashboard.peer_sort_desc;
            } else {
                state.dashboard.peer_sort = field;
                state.dashboard.peer_sort_desc = false;
            }
        }
        Message::NetinfoLevelChanged(level) => {
            state.dashboard.netinfo_level = level.min(4);
        }
        Message::DashboardPartialRefreshRequested(partial) => {
            if state.dashboard.in_flight {
                return Task::none();
            }
            if state.dashboard.snapshot.is_none() {
                return start_dashboard_refresh(state);
            }
            return start_partial_dashboard_refresh(state, partial);
        }
        Message::DashboardPartialLoaded(rgen, result) => {
            if rgen != state.dashboard.request_gen {
                state.dashboard.in_flight = false;
                return Task::none();
            }
            state.dashboard.in_flight = false;
            match result {
                Ok(partial) => {
                    if let Some(snapshot) = state.dashboard.snapshot.as_mut() {
                        match partial {
                            DashboardPartialUpdate::Mempool(mempool) => {
                                snapshot.mempool = mempool;
                            }
                            DashboardPartialUpdate::ChainAndMempool { chain, mempool } => {
                                snapshot.chain = chain;
                                snapshot.mempool = mempool;
                            }
                        }
                        state.dashboard.error = None;
                    } else {
                        return start_dashboard_refresh(state);
                    }
                }
                Err(error) => {
                    state.dashboard.error = Some(error);
                }
            }
            return schedule_pending_partial_if_ready(state);
        }
        Message::DashboardPaneResized(event) => {
            state.dashboard.panes.resize(event.split, event.ratio);
        }
        _ => {}
    }

    Task::none()
}

pub fn schedule_pending_partial_if_ready(state: &mut State) -> Task<Message> {
    if let Some(partial) = state.dashboard.pending_partial
        && can_run_debounced_refresh(state)
    {
        state.dashboard.pending_partial = None;
        return Task::perform(
            async move { partial },
            Message::DashboardPartialRefreshRequested,
        );
    }
    Task::none()
}

pub fn start_dashboard_refresh(state: &mut State) -> Task<Message> {
    state.dashboard.in_flight = true;
    state.dashboard.last_refresh_at = Some(Instant::now());
    let client = state.rpc.client.clone();
    let rgen = state.dashboard.request_gen;
    Task::perform(load_dashboard(client), move |r| {
        Message::DashboardLoaded(rgen, r)
    })
}

fn start_partial_dashboard_refresh(
    state: &mut State,
    partial: DashboardPartialSet,
) -> Task<Message> {
    state.dashboard.in_flight = true;
    state.dashboard.last_refresh_at = Some(Instant::now());
    let client = state.rpc.client.clone();
    let rgen = state.dashboard.request_gen;
    Task::perform(load_dashboard_partial(client, partial), move |r| {
        Message::DashboardPartialLoaded(rgen, r)
    })
}

pub fn can_run_debounced_refresh(state: &State) -> bool {
    state
        .dashboard
        .last_refresh_at
        .is_none_or(|t| t.elapsed() >= Duration::from_millis(ZMQ_REFRESH_DEBOUNCE_MS))
}

async fn load_dashboard(client: RpcClient) -> Result<DashboardSnapshot, String> {
    let service = DashboardService::new(client);
    service.fetch_snapshot().map_err(|e| e.to_string())
}

async fn load_dashboard_partial(
    client: RpcClient,
    partial: DashboardPartialSet,
) -> Result<DashboardPartialUpdate, String> {
    let service = DashboardService::new(client);
    match partial {
        DashboardPartialSet::MempoolOnly => service.fetch_mempool_update(),
        DashboardPartialSet::ChainAndMempool => service.fetch_chain_and_mempool_update(),
    }
    .map_err(|e| e.to_string())
}
