use std::sync::Arc;

pub struct MusicRuntime {
    inner: imp::InnerRuntime,
}

pub fn is_enabled() -> bool {
    imp::is_enabled()
}

pub fn start_music() -> MusicRuntime {
    MusicRuntime {
        inner: imp::start_music(),
    }
}

pub fn handle_music_request(
    path: &str,
    query: &str,
    runtime: &Arc<MusicRuntime>,
) -> Option<String> {
    if !path.starts_with("/music/") {
        return None;
    }
    Some(imp::handle_music_request(path, query, &runtime.inner))
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
        track_count: usize,
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

    pub(super) fn is_enabled() -> bool {
        true
    }

    pub(super) fn start_music() -> InnerRuntime {
        let mut tunes = load_tunes();
        shuffle(&mut tunes);
        debug!(tracks = tunes.len(), "initialized music runtime");

        let (tx, rx) = mpsc::channel();
        let state = Arc::new(Mutex::new(MusicState {
            current_track: 0,
            track_count: tunes.len(),
            track_name: tunes.first().map_or("", |t| t.name).to_string(),
            playing: !tunes.is_empty(),
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

            loop {
                match rx.recv_timeout(Duration::from_millis(500)) {
                    Ok(cmd) => {
                        let mut s = st.lock().unwrap();
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
                            let mut s = st.lock().unwrap();
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

    pub(super) fn handle_music_request(path: &str, query: &str, runtime: &InnerRuntime) -> String {
        match path {
            "/music/status" => {
                let s = runtime.state.lock().unwrap();
                serde_json::json!({
                    "enabled": true,
                    "track": s.track_name,
                    "index": s.current_track,
                    "count": s.track_count,
                    "playing": s.playing,
                    "volume": s.volume,
                    "muted": s.muted,
                })
                .to_string()
            }
            "/music/playpause" => {
                let _ = runtime.tx.send(MusicCmd::PlayPause);
                r#"{"ok":true}"#.into()
            }
            "/music/next" => {
                let _ = runtime.tx.send(MusicCmd::Next);
                r#"{"ok":true}"#.into()
            }
            "/music/prev" => {
                let _ = runtime.tx.send(MusicCmd::Prev);
                r#"{"ok":true}"#.into()
            }
            "/music/volume" => {
                let v: f32 = query.parse().unwrap_or(0.5);
                let _ = runtime.tx.send(MusicCmd::SetVolume(v));
                r#"{"ok":true}"#.into()
            }
            "/music/mute" => {
                let _ = runtime.tx.send(MusicCmd::ToggleMute);
                r#"{"ok":true}"#.into()
            }
            _ => r#"{"error":"unknown music endpoint"}"#.into(),
        }
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

    pub(super) fn is_enabled() -> bool {
        false
    }

    pub(super) fn start_music() -> InnerRuntime {
        InnerRuntime
    }

    pub(super) fn handle_music_request(
        path: &str,
        _query: &str,
        _runtime: &InnerRuntime,
    ) -> String {
        match path {
            "/music/status" => r#"{"enabled":false}"#.into(),
            "/music/playpause" | "/music/next" | "/music/prev" | "/music/volume"
            | "/music/mute" => r#"{"ok":false,"error":"audio feature disabled"}"#.into(),
            _ => r#"{"error":"unknown music endpoint"}"#.into(),
        }
    }
}
