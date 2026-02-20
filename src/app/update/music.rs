use iced::Task;

use crate::app::message::Message;
use crate::app::state::State;
use crate::music::MusicRuntime;

pub fn handle_music(state: &mut State, message: Message) -> Task<Message> {
    match message {
        Message::MusicPlayPause => with_music(state, MusicRuntime::play_pause),
        Message::MusicNext => with_music(state, MusicRuntime::next),
        Message::MusicPrev => with_music(state, MusicRuntime::prev),
        Message::MusicSetVolume(v) => with_music(state, |rt| rt.set_volume(v)),
        Message::MusicToggleMute => with_music(state, MusicRuntime::toggle_mute),
        Message::MusicPollTick => {
            if let Some(rt) = &state.music {
                state.music_snapshot = rt.snapshot();
            }
        }
        _ => {}
    }
    Task::none()
}

fn with_music(state: &mut State, f: impl FnOnce(&MusicRuntime)) {
    if let Some(rt) = &state.music {
        f(rt);
        state.music_snapshot = rt.snapshot();
    }
}
