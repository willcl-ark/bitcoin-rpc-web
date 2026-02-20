use iced::Task;

use crate::app::message::Message;
use crate::app::state::{DashboardPartialSet, State, ZmqUiEvent};

use super::dashboard::can_run_debounced_refresh;

const ZMQ_RECENT_EVENTS_CAP: usize = 80;

pub fn handle_zmq(state: &mut State) -> Task<Message> {
    poll_zmq_feed(state);
    if let Some(partial) = state.dashboard.pending_partial
        && !state.dashboard.in_flight
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

fn poll_zmq_feed(state: &mut State) {
    let mut saw_hashblock = false;
    let mut saw_hashtx = false;

    {
        let zmq_state = state.zmq_state.state.lock().expect("zmq state lock");
        state.zmq.connected = zmq_state.connected;
        state.zmq.connected_address = zmq_state.address.clone();
        state.zmq.events_seen = zmq_state.next_cursor.saturating_sub(1);

        for message in zmq_state.messages.iter() {
            if message.cursor <= state.zmq.last_cursor {
                continue;
            }

            state.zmq.last_cursor = message.cursor;
            state.zmq.last_topic = Some(message.topic.clone());
            state.zmq.last_event_at = Some(message.timestamp);

            state.zmq.recent_events.push(ZmqUiEvent {
                topic: message.topic.clone(),
                event_hash: message
                    .event_hash
                    .clone()
                    .unwrap_or_else(|| message.body_hex.clone()),
                timestamp: message.timestamp,
            });

            match message.topic.as_str() {
                "hashblock" => saw_hashblock = true,
                "hashtx" => saw_hashtx = true,
                _ => {}
            }
        }
    }

    let overflow = state
        .zmq
        .recent_events
        .len()
        .saturating_sub(ZMQ_RECENT_EVENTS_CAP);
    if overflow > 0 {
        state.zmq.recent_events.drain(..overflow);
    }

    if saw_hashblock {
        merge_pending_partial(state, DashboardPartialSet::ChainAndMempool);
    } else if saw_hashtx {
        merge_pending_partial(state, DashboardPartialSet::MempoolOnly);
    }
}

fn merge_pending_partial(state: &mut State, next: DashboardPartialSet) {
    state.dashboard.pending_partial = Some(match (state.dashboard.pending_partial, next) {
        (Some(DashboardPartialSet::ChainAndMempool), _) => DashboardPartialSet::ChainAndMempool,
        (_, DashboardPartialSet::ChainAndMempool) => DashboardPartialSet::ChainAndMempool,
        _ => DashboardPartialSet::MempoolOnly,
    });
}
