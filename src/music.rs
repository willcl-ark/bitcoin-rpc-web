pub struct MusicSnapshot {
    pub track_name: String,
    pub playing: bool,
    pub volume: f32,
    pub muted: bool,
}

impl Default for MusicSnapshot {
    fn default() -> Self {
        Self {
            track_name: String::new(),
            playing: false,
            volume: 1.0,
            muted: false,
        }
    }
}

pub struct MusicRuntime {
    inner: imp::InnerRuntime,
}

impl MusicRuntime {
    pub fn snapshot(&self) -> MusicSnapshot {
        imp::snapshot(&self.inner)
    }

    pub fn play_pause(&self) {
        imp::play_pause(&self.inner);
    }

    pub fn next(&self) {
        imp::next_track(&self.inner);
    }

    pub fn prev(&self) {
        imp::prev_track(&self.inner);
    }

    pub fn set_volume(&self, v: f32) {
        imp::set_volume(&self.inner, v);
    }

    pub fn toggle_mute(&self) {
        imp::toggle_mute(&self.inner);
    }
}

pub fn is_enabled() -> bool {
    imp::is_enabled()
}

pub fn start_music(start_playing: bool) -> MusicRuntime {
    MusicRuntime {
        inner: imp::start_music(start_playing),
    }
}

#[cfg(feature = "audio")]
mod imp {
    use std::sync::mpsc;
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    use rodio::{OutputStream, OutputStreamHandle, Sink, Source};
    use tracing::{debug, warn};
    use xmrs::import::amiga::amiga_module::AmigaModule;
    use xmrs::module::Module;
    use xmrsplayer::xmrsplayer::XmrsPlayer;

    const SAMPLE_RATE: u32 = 48000;

    struct Tune {
        name: &'static str,
        module: &'static Module,
    }

    pub(super) struct InnerRuntime {
        tx: mpsc::Sender<MusicCmd>,
        state: Arc<Mutex<MusicState>>,
    }

    enum MusicCmd {
        PlayPause,
        Next,
        Prev,
        SetVolume(f32),
        ToggleMute,
    }

    struct MusicState {
        current_track: usize,
        track_name: String,
        playing: bool,
        volume: f32,
        muted: bool,
    }

    struct ModSource {
        player: XmrsPlayer<'static>,
        buffer: Vec<f32>,
        pos: usize,
    }

    impl ModSource {
        fn new(module: &'static Module) -> Self {
            let mut player = XmrsPlayer::new(module, SAMPLE_RATE as f32, 0, false);
            player.set_max_loop_count(2);
            player.amplification = 0.5;
            Self {
                player,
                buffer: Vec::with_capacity(2048),
                pos: 0,
            }
        }
    }

    impl Iterator for ModSource {
        type Item = f32;

        fn next(&mut self) -> Option<f32> {
            if self.pos >= self.buffer.len() {
                self.buffer.clear();
                self.pos = 0;
                for _ in 0..1024 {
                    match self.player.sample(true) {
                        Some((l, r)) => {
                            let mix = (l + r) * 0.5;
                            self.buffer.push(mix);
                            self.buffer.push(mix);
                        }
                        None => break,
                    }
                }
                if self.buffer.is_empty() {
                    return None;
                }
            }
            let s = self.buffer[self.pos];
            self.pos += 1;
            Some(s)
        }
    }

    impl Source for ModSource {
        fn current_frame_len(&self) -> Option<usize> {
            None
        }

        fn channels(&self) -> u16 {
            2
        }

        fn sample_rate(&self) -> u32 {
            SAMPLE_RATE
        }

        fn total_duration(&self) -> Option<Duration> {
            None
        }
    }

    pub(super) fn snapshot(runtime: &InnerRuntime) -> super::MusicSnapshot {
        let s = runtime.state.lock().expect("music state lock");
        super::MusicSnapshot {
            track_name: s.track_name.clone(),
            playing: s.playing,
            volume: s.volume,
            muted: s.muted,
        }
    }

    pub(super) fn play_pause(runtime: &InnerRuntime) {
        let _ = runtime.tx.send(MusicCmd::PlayPause);
    }

    pub(super) fn next_track(runtime: &InnerRuntime) {
        let _ = runtime.tx.send(MusicCmd::Next);
    }

    pub(super) fn prev_track(runtime: &InnerRuntime) {
        let _ = runtime.tx.send(MusicCmd::Prev);
    }

    pub(super) fn set_volume(runtime: &InnerRuntime, v: f32) {
        let _ = runtime.tx.send(MusicCmd::SetVolume(v));
    }

    pub(super) fn toggle_mute(runtime: &InnerRuntime) {
        let _ = runtime.tx.send(MusicCmd::ToggleMute);
    }

    pub(super) fn is_enabled() -> bool {
        true
    }

    pub(super) fn start_music(start_playing: bool) -> InnerRuntime {
        let mut tunes = load_tunes();
        shuffle(&mut tunes);
        debug!(tracks = tunes.len(), "initialized music runtime");

        let (tx, rx) = mpsc::channel();
        let state = Arc::new(Mutex::new(MusicState {
            current_track: 0,
            track_name: tunes.first().map_or("", |t| t.name).to_string(),
            playing: !tunes.is_empty() && start_playing,
            volume: 1.0,
            muted: false,
        }));
        let st = Arc::clone(&state);

        std::thread::spawn(move || {
            if tunes.is_empty() {
                return;
            }

            let (_stream, handle) = match OutputStream::try_default() {
                Ok(s) => s,
                Err(e) => {
                    warn!(error = %e, "failed to open default audio output");
                    return;
                }
            };

            let mut sink = make_sink(&handle, tunes[0].module, 1.0);
            if !start_playing {
                sink.pause();
            }

            loop {
                match rx.recv_timeout(Duration::from_millis(500)) {
                    Ok(cmd) => {
                        let mut s = st.lock().expect("music state lock");
                        match cmd {
                            MusicCmd::PlayPause => {
                                if s.playing {
                                    sink.pause();
                                    s.playing = false;
                                } else {
                                    sink.play();
                                    s.playing = true;
                                }
                            }
                            MusicCmd::Next => {
                                s.current_track = (s.current_track + 1) % tunes.len();
                                s.track_name = tunes[s.current_track].name.to_string();
                                s.playing = true;
                                let vol = if s.muted { 0.0 } else { s.volume };
                                drop(sink);
                                sink = make_sink(&handle, tunes[s.current_track].module, vol);
                            }
                            MusicCmd::Prev => {
                                s.current_track = if s.current_track == 0 {
                                    tunes.len() - 1
                                } else {
                                    s.current_track - 1
                                };
                                s.track_name = tunes[s.current_track].name.to_string();
                                s.playing = true;
                                let vol = if s.muted { 0.0 } else { s.volume };
                                drop(sink);
                                sink = make_sink(&handle, tunes[s.current_track].module, vol);
                            }
                            MusicCmd::SetVolume(v) => {
                                s.volume = v.clamp(0.0, 1.0);
                                if !s.muted {
                                    sink.set_volume(s.volume);
                                }
                            }
                            MusicCmd::ToggleMute => {
                                s.muted = !s.muted;
                                sink.set_volume(if s.muted { 0.0 } else { s.volume });
                            }
                        }
                    }
                    Err(mpsc::RecvTimeoutError::Timeout) => {
                        if sink.empty() {
                            let mut s = st.lock().expect("music state lock");
                            s.current_track = (s.current_track + 1) % tunes.len();
                            s.track_name = tunes[s.current_track].name.to_string();
                            let vol = if s.muted { 0.0 } else { s.volume };
                            drop(sink);
                            sink = make_sink(&handle, tunes[s.current_track].module, vol);
                        }
                    }
                    Err(mpsc::RecvTimeoutError::Disconnected) => break,
                }
            }
        });

        InnerRuntime { tx, state }
    }

    fn load_tunes() -> Vec<Tune> {
        let raw: &[(&str, &[u8])] = &[
            (
                "Hymn to Aurora",
                include_bytes!("../tunes/hymn_to_aurora.mod"),
            ),
            ("Musiklinjen", include_bytes!("../tunes/musiklinjen.mod")),
            (
                "Playing with Sound",
                include_bytes!("../tunes/playingw.mod"),
            ),
            (
                "Sundance",
                include_bytes!("../tunes/purple_motion_-_sundance.mod"),
            ),
            ("Resii", include_bytes!("../tunes/resii.mod")),
            ("Space Debris", include_bytes!("../tunes/space_debris.mod")),
            ("Stardust Memories", include_bytes!("../tunes/stardstm.mod")),
            ("Toy Story", include_bytes!("../tunes/toy_story.mod")),
            ("Toy Title", include_bytes!("../tunes/toytitle.mod")),
        ];
        raw.iter()
            .filter_map(|(name, data)| match AmigaModule::load(data) {
                Ok(amiga) => {
                    let module = Box::leak(Box::new(amiga.to_module()));
                    Some(Tune { name, module })
                }
                Err(e) => {
                    warn!(track = *name, error = ?e, "failed to load module track");
                    None
                }
            })
            .collect()
    }

    fn make_sink(handle: &OutputStreamHandle, module: &'static Module, volume: f32) -> Sink {
        let sink = Sink::try_new(handle).unwrap();
        let source = ModSource::new(module);
        sink.append(source);
        sink.set_volume(volume);
        sink
    }

    fn shuffle(tunes: &mut [Tune]) {
        let mut seed = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as u64;
        for i in (1..tunes.len()).rev() {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            let j = (seed as usize) % (i + 1);
            tunes.swap(i, j);
        }
    }
}

#[cfg(not(feature = "audio"))]
mod imp {
    pub(super) struct InnerRuntime;

    pub(super) fn snapshot(_runtime: &InnerRuntime) -> super::MusicSnapshot {
        super::MusicSnapshot::default()
    }

    pub(super) fn play_pause(_runtime: &InnerRuntime) {}
    pub(super) fn next_track(_runtime: &InnerRuntime) {}
    pub(super) fn prev_track(_runtime: &InnerRuntime) {}
    pub(super) fn set_volume(_runtime: &InnerRuntime, _v: f32) {}
    pub(super) fn toggle_mute(_runtime: &InnerRuntime) {}

    pub(super) fn is_enabled() -> bool {
        false
    }

    pub(super) fn start_music(_start_playing: bool) -> InnerRuntime {
        InnerRuntime
    }
}
