//! Run with `cargo test -p player-application --test native_playback -- --ignored`.
//! The native runtime is required; generated PCM uses mpv's null audio output.
use player_application::{
    AppError, AppResult, AudioDevice, AudioEvent, AudioPort, PlayerCommand, PlayerService,
    StateStore,
};
use player_domain::{RepeatMode, Track};
use player_infrastructure::{EngineEvent, MpvEngine};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

struct NativeAudio(Arc<Mutex<MpvEngine>>);

impl AudioPort for NativeAudio {
    fn command(&mut self, args: &[&str]) -> AppResult<()> {
        self.0
            .lock()
            .unwrap()
            .command(args)
            .map_err(|error| AppError::new(error.code, error.detail))
    }
    fn set_property(&mut self, name: &str, value: &str) -> AppResult<()> {
        self.0
            .lock()
            .unwrap()
            .set_property(name, value)
            .map_err(|error| AppError::new(error.code, error.detail))
    }
    fn get_property(&mut self, name: &str) -> Option<String> {
        self.0.lock().unwrap().get_property(name)
    }
    fn poll_event(&mut self) -> Option<AudioEvent> {
        self.0
            .lock()
            .unwrap()
            .poll_event()
            .map(|event| match event {
                EngineEvent::TrackChanged { .. } => AudioEvent::FileLoaded,
                EngineEvent::Ended { reason, error } => AudioEvent::Ended { reason, error },
            })
    }
    fn version(&self) -> String {
        self.0.lock().unwrap().version()
    }
    fn devices(&mut self) -> Vec<AudioDevice> {
        self.0
            .lock()
            .unwrap()
            .list_devices()
            .into_iter()
            .map(|device| AudioDevice {
                id: device.id,
                name: device.name,
            })
            .collect()
    }
}

#[derive(Default)]
struct MemoryStore(Option<String>);
impl StateStore for MemoryStore {
    fn load(&mut self) -> AppResult<Option<String>> {
        Ok(self.0.clone())
    }
    fn save(&mut self, json: &str) -> AppResult<()> {
        self.0 = Some(json.into());
        Ok(())
    }
}

struct TestAudioFiles {
    directory: PathBuf,
    paths: Vec<PathBuf>,
}
impl TestAudioFiles {
    fn new() -> Self {
        let directory =
            std::env::temp_dir().join(format!("SongPlayer-service-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&directory).unwrap();
        let paths = ["Erster – Grüße 🎵.wav", "Zweiter – 日本語.wav"]
            .into_iter()
            .map(|name| {
                let path = directory.join(name);
                write_wav(&path, 2);
                path
            })
            .collect();
        Self { directory, paths }
    }
    fn tracks(&self) -> Vec<Track> {
        self.paths
            .iter()
            .enumerate()
            .map(|(index, path)| Track {
                id: format!("native-{}", index + 1),
                path: path.to_string_lossy().into_owned(),
                title: path.file_name().unwrap().to_string_lossy().into_owned(),
                duration: 2.,
                track_number: Some(index as u32 + 1),
                ..Track::default()
            })
            .collect()
    }
}
impl Drop for TestAudioFiles {
    fn drop(&mut self) {
        for path in &self.paths {
            let _ = std::fs::remove_file(path);
        }
        let _ = std::fs::remove_dir(&self.directory);
    }
}

fn write_wav(path: &Path, seconds: u32) {
    let pcm_length = 8_000_u32 * 2 * seconds;
    let mut wav = Vec::new();
    wav.extend(b"RIFF");
    wav.extend((36 + pcm_length).to_le_bytes());
    wav.extend(b"WAVEfmt ");
    wav.extend(16_u32.to_le_bytes());
    wav.extend(1_u16.to_le_bytes());
    wav.extend(1_u16.to_le_bytes());
    wav.extend(8_000_u32.to_le_bytes());
    wav.extend(16_000_u32.to_le_bytes());
    wav.extend(2_u16.to_le_bytes());
    wav.extend(16_u16.to_le_bytes());
    wav.extend(b"data");
    wav.extend(pcm_length.to_le_bytes());
    wav.resize(44 + pcm_length as usize, 0);
    std::fs::write(path, wav).unwrap();
}

fn current(service: &PlayerService) -> Option<&str> {
    service
        .active()
        .current_entry()
        .map(|entry| entry.track.id.as_str())
}

fn wait_until(
    service: &mut PlayerService,
    label: &str,
    predicate: impl Fn(&PlayerService) -> bool,
) {
    let deadline = Instant::now() + Duration::from_secs(6);
    while Instant::now() < deadline {
        service.tick();
        if predicate(service) {
            return;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    let playback = service.playback();
    panic!(
        "Timed out waiting for {label}: status={}, track={:?}, position={}, warnings={:?}",
        playback.status,
        playback.track_id,
        playback.position,
        service.snapshot().warnings
    );
}

fn tick_for(service: &mut PlayerService, duration: Duration) {
    let deadline = Instant::now() + duration;
    while Instant::now() < deadline {
        service.tick();
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[test]
#[ignore = "requires the native libmpv runtime; uses null audio output"]
fn real_service_plays_unicode_wavs_and_observes_repeat_pause_seek_and_restart() {
    let files = TestAudioFiles::new();
    let mut engine = MpvEngine::new(&[])
        .expect("Install the project libmpv runtime before running this ignored test");
    eprintln!("Testing PlayerService with {}", engine.version());
    engine.set_property("ao", "null").unwrap();
    let mut service = PlayerService::new(
        Box::new(MemoryStore::default()),
        Ok(Box::new(NativeAudio(Arc::new(Mutex::new(engine))))),
    )
    .unwrap();
    service
        .ingest(files.tracks(), vec![], "play", vec![])
        .unwrap();
    wait_until(&mut service, "first Unicode WAV", |service| {
        current(service) == Some("native-1") && service.playback().position > 0.1
    });

    service.dispatch(PlayerCommand::Pause).unwrap();
    tick_for(&mut service, Duration::from_millis(50));
    let paused_at = service.playback().position;
    tick_for(&mut service, Duration::from_millis(150));
    assert_eq!(service.playback().status, "paused");
    assert!((service.playback().position - paused_at).abs() < 0.1);
    service
        .dispatch(PlayerCommand::Seek { position: 0.5 })
        .unwrap();
    wait_until(&mut service, "paused seek", |service| {
        (service.playback().position - 0.5).abs() < 0.1
    });
    // Audio-only mpv can expose the pre-seek clock again while paused, even
    // after seeking=no. The coordinator must keep displaying/persisting the
    // requested paused position rather than briefly reaching it and regressing.
    for _ in 0..20 {
        service.tick();
        assert_eq!(service.playback().status, "paused");
        assert!(
            (service.playback().position - 0.5).abs() < 0.1,
            "Paused seek target regressed to {}",
            service.playback().position
        );
        assert!((service.active().position - 0.5).abs() < 0.1);
        std::thread::sleep(Duration::from_millis(10));
    }
    service.dispatch(PlayerCommand::Play).unwrap();
    for _ in 0..10 {
        service.tick();
        assert_eq!(service.playback().status, "playing");
        assert!(
            service.playback().position >= 0.4,
            "Playback resumed before the seek target: {}",
            service.playback().position
        );
        std::thread::sleep(Duration::from_millis(10));
    }
    wait_until(&mut service, "play after pause", |service| {
        service.playback().position > 0.7
    });

    service
        .dispatch(PlayerCommand::SetRepeat {
            mode: RepeatMode::One,
        })
        .unwrap();
    service
        .dispatch(PlayerCommand::Seek { position: 1.9 })
        .unwrap();
    wait_until(&mut service, "natural Repeat One", |service| {
        current(service) == Some("native-1") && service.playback().position < 0.5
    });
    assert_eq!(service.playback().status, "playing");
    service.dispatch(PlayerCommand::Next).unwrap();
    wait_until(&mut service, "manual Next despite Repeat One", |service| {
        current(service) == Some("native-2") && service.playback().position > 0.1
    });

    service
        .dispatch(PlayerCommand::SetRepeat {
            mode: RepeatMode::All,
        })
        .unwrap();
    service
        .dispatch(PlayerCommand::Seek { position: 1.9 })
        .unwrap();
    wait_until(&mut service, "Repeat All starts another pass", |service| {
        current(service) == Some("native-1") && service.playback().position > 0.1
    });
    service
        .dispatch(PlayerCommand::SetRepeat {
            mode: RepeatMode::Off,
        })
        .unwrap();
    service
        .dispatch(PlayerCommand::Seek { position: 1.9 })
        .unwrap();
    wait_until(&mut service, "automatic second track", |service| {
        current(service) == Some("native-2") && service.playback().position > 0.1
    });
    service
        .dispatch(PlayerCommand::Seek { position: 1.9 })
        .unwrap();
    wait_until(&mut service, "natural queue end", |service| {
        service.playback().status == "stopped"
    });
    service.dispatch(PlayerCommand::Play).unwrap();
    wait_until(&mut service, "Play restarts completed queue", |service| {
        current(service) == Some("native-1") && service.playback().position > 0.1
    });
    assert!(service.snapshot().warnings.is_empty());
    service.dispatch(PlayerCommand::Stop).unwrap();
}

#[test]
#[ignore = "requires the native libmpv runtime; uses null audio output"]
fn normalization_can_toggle_during_playback_without_removing_other_audio_filters() {
    let files = TestAudioFiles::new();
    let mut engine = MpvEngine::new(&[]).expect("Install the project libmpv runtime");
    engine.set_property("ao", "null").unwrap();
    engine
        .command(&["af", "add", "@test-volume:lavfi=[volume=0.8]"])
        .unwrap();
    let engine = Arc::new(Mutex::new(engine));
    let mut saved = player_application::SavedState::default();
    saved.settings.loudness_normalization = true;
    let mut service = PlayerService::new(
        Box::new(MemoryStore(Some(serde_json::to_string(&saved).unwrap()))),
        Ok(Box::new(NativeAudio(engine.clone()))),
    )
    .unwrap();
    let filters = engine.lock().unwrap().get_property("af").unwrap();
    assert!(filters.contains("songplayer-loudness"), "{filters}");
    assert!(filters.contains("dynaudnorm"), "{filters}");
    assert!(filters.contains("test-volume"), "{filters}");
    service
        .ingest(files.tracks(), vec![], "play", vec![])
        .unwrap();
    wait_until(&mut service, "normalized audio output", |service| {
        current(service) == Some("native-1") && service.playback().position > 0.1
    });
    service
        .dispatch(PlayerCommand::SetLoudnessNormalization { enabled: true })
        .unwrap();
    let filters = engine.lock().unwrap().get_property("af").unwrap();
    assert_eq!(filters.matches("songplayer-loudness").count(), 1, "{filters}");
    service
        .dispatch(PlayerCommand::SetLoudnessNormalization { enabled: false })
        .unwrap();
    let filters = engine.lock().unwrap().get_property("af").unwrap();
    assert!(!filters.contains("songplayer-loudness"), "{filters}");
    assert!(filters.contains("test-volume"), "{filters}");
    service
        .dispatch(PlayerCommand::SetLoudnessNormalization { enabled: true })
        .unwrap();
    wait_until(
        &mut service,
        "continued audio after normalization toggle",
        |service| service.playback().position > 0.3,
    );
    assert!(service.snapshot().warnings.is_empty());
    service.dispatch(PlayerCommand::Stop).unwrap();
}

#[test]
#[ignore = "requires the native libmpv runtime; uses null audio output"]
fn settings_import_and_undo_preserve_native_stream_position_pause_and_other_filters() {
    let files = TestAudioFiles::new();
    let mut native = MpvEngine::new(&[]).expect("Install the project libmpv runtime");
    native.set_property("ao", "null").unwrap();
    native
        .command(&["af", "add", "@test-volume:lavfi=[volume=0.8]"])
        .unwrap();
    let engine = Arc::new(Mutex::new(native));
    let mut service = PlayerService::new(
        Box::new(MemoryStore::default()),
        Ok(Box::new(NativeAudio(engine.clone()))),
    )
    .unwrap();
    service
        .ingest(files.tracks(), vec![], "play", vec![])
        .unwrap();
    wait_until(
        &mut service,
        "native stream before settings import",
        |service| service.playback().position > 0.75,
    );
    service.dispatch(PlayerCommand::Pause).unwrap();
    tick_for(&mut service, Duration::from_millis(50));
    let position = service.playback().position;
    let track = current(&service).unwrap().to_owned();
    service.dispatch(PlayerCommand::ImportSettings {
        document: serde_json::json!({ "format": "song-player-settings", "version": 1, "exportedAt": 0,
            "sections": { "audio": { "volume": 38, "muted": true, "replayGain": true, "loudnessNormalization": true } } }),
        sections: vec!["audio".into()],
    }).unwrap();
    tick_for(&mut service, Duration::from_millis(50));
    assert_eq!(service.playback().status, "paused");
    assert_eq!(current(&service), Some(track.as_str()));
    // Rebuilding the audio filter chain adjusts mpv's reported clock by roughly
    // one 150 ms normalization frame; it must not restart or resume the stream.
    assert!(
        (service.playback().position - position).abs() < 0.25,
        "Import moved position from {position} to {} (native {:?})",
        service.playback().position,
        engine.lock().unwrap().get_property("time-pos")
    );
    let filters = engine.lock().unwrap().get_property("af").unwrap();
    assert!(
        filters.contains("songplayer-loudness") && filters.contains("test-volume"),
        "{filters}"
    );
    assert_eq!(
        engine.lock().unwrap().get_property("volume").as_deref(),
        Some("38.000000")
    );
    service.dispatch(PlayerCommand::UndoSettingsImport).unwrap();
    tick_for(&mut service, Duration::from_millis(50));
    assert_eq!(service.playback().status, "paused");
    assert_eq!(current(&service), Some(track.as_str()));
    assert!(
        (service.playback().position - position).abs() < 0.25,
        "Undo moved position from {position} to {}",
        service.playback().position
    );
    let filters = engine.lock().unwrap().get_property("af").unwrap();
    assert!(
        !filters.contains("songplayer-loudness") && filters.contains("test-volume"),
        "{filters}"
    );
    assert_eq!(service.state.settings.volume, 75.);
    assert!(!service.state.settings.muted);
    assert!(!service.snapshot().settings_undo_available);
    service.dispatch(PlayerCommand::Stop).unwrap();
}

#[test]
#[ignore = "requires the native libmpv runtime; uses null audio output"]
fn native_equalizer_folder_gain_and_sleep_timer_preserve_filters_and_pause_at_track_end() {
    use player_application::{EqualizerSettings, GainGrouping, LoudnessValue, SleepMode};
    let files = TestAudioFiles::new();
    let mut native = MpvEngine::new(&[]).expect("Install the project libmpv runtime");
    native.set_property("ao", "null").unwrap();
    native
        .command(&["af", "add", "@test-volume:lavfi=[volume=0.8]"])
        .unwrap();
    let engine = Arc::new(Mutex::new(native));
    let mut service = PlayerService::new(
        Box::new(MemoryStore::default()),
        Ok(Box::new(NativeAudio(engine.clone()))),
    )
    .unwrap();
    service
        .ingest(files.tracks(), vec![], "play", vec![])
        .unwrap();
    wait_until(&mut service, "native before equalizer", |s| {
        s.playback().position > 0.15
    });
    service
        .dispatch(PlayerCommand::SetEqualizer {
            equalizer: EqualizerSettings {
                enabled: true,
                bands: [3., -1., 0., 2., 1.],
                preamp: 0.,
            },
        })
        .unwrap();
    service
        .dispatch(PlayerCommand::SetLoudnessValues {
            values: vec![LoudnessValue {
                track_id: "native-1".into(),
                gain: 2.,
                peak: 0.5,
                grouping: GainGrouping::Folder,
                group_key: Some(files.directory.to_string_lossy().into_owned()),
            }],
        })
        .unwrap();
    service
        .dispatch(PlayerCommand::SetGainGrouping {
            grouping: GainGrouping::Folder,
        })
        .unwrap();
    service
        .dispatch(PlayerCommand::SetReplayGain { enabled: true })
        .unwrap();
    let filters = engine.lock().unwrap().get_property("af").unwrap();
    assert!(
        filters.contains("songplayer-equalizer")
            && filters.contains("songplayer-replaygain")
            && filters.contains("test-volume"),
        "{filters}"
    );
    service
        .dispatch(PlayerCommand::SetRepeat {
            mode: RepeatMode::One,
        })
        .unwrap();
    service
        .dispatch(PlayerCommand::SetSleepTimer {
            mode: SleepMode::Track,
            minutes: 0.,
        })
        .unwrap();
    service
        .dispatch(PlayerCommand::Seek { position: 1.8 })
        .unwrap();
    wait_until(&mut service, "sleep after track despite Repeat One", |s| {
        s.snapshot().sleep_timer.is_none() && s.playback().status == "paused"
    });
    assert_eq!(current(&service), Some("native-1"));
    tick_for(&mut service, Duration::from_millis(120));
    assert_eq!(service.playback().status, "paused");
    service.dispatch(PlayerCommand::Play).unwrap();
    wait_until(&mut service, "play restarts after sleep boundary", |s| {
        s.playback().status == "playing"
            && s.playback().position > 0.1
            && s.playback().position < 1.
    });
    assert_eq!(current(&service), Some("native-1"));
    service
        .dispatch(PlayerCommand::SetEqualizer {
            equalizer: EqualizerSettings::default(),
        })
        .unwrap();
    service
        .dispatch(PlayerCommand::SetReplayGain { enabled: false })
        .unwrap();
    let filters = engine.lock().unwrap().get_property("af").unwrap();
    assert!(
        !filters.contains("songplayer-equalizer")
            && !filters.contains("songplayer-replaygain")
            && filters.contains("test-volume"),
        "{filters}"
    );
    service.dispatch(PlayerCommand::Stop).unwrap();
}

#[test]
#[ignore = "requires the native libmpv runtime; uses null audio output"]
fn native_listening_counts_actual_audio_but_not_pause_or_seek_and_records_replays() {
    use player_domain::ListeningEvent;
    struct HistoryStore {
        json: Option<String>,
        history: std::collections::BTreeMap<String, ListeningEvent>,
    }
    impl StateStore for HistoryStore {
        fn load(&mut self) -> AppResult<Option<String>> {
            Ok(self.json.clone())
        }
        fn save(&mut self, json: &str) -> AppResult<()> {
            self.json = Some(json.into());
            Ok(())
        }
        fn record_listening(&mut self, event: &ListeningEvent) -> AppResult<()> {
            self.history.insert(event.id.clone(), event.clone());
            Ok(())
        }
        fn listening_history(
            &mut self,
            _: Option<i64>,
            _: Option<i64>,
        ) -> AppResult<Vec<ListeningEvent>> {
            Ok(self.history.values().cloned().collect())
        }
    }
    let files = TestAudioFiles::new();
    let mut native = MpvEngine::new(&[]).unwrap();
    native.set_property("ao", "null").unwrap();
    let audio = NativeAudio(Arc::new(Mutex::new(native)));
    let mut service = PlayerService::new(
        Box::new(HistoryStore {
            json: None,
            history: Default::default(),
        }),
        Ok(Box::new(audio)),
    )
    .unwrap();
    service
        .dispatch(PlayerCommand::SetStatisticsSettings {
            settings: player_application::StatisticsSettings {
                enabled: true,
                percent: 10.,
                seconds: 0.,
            },
        })
        .unwrap();
    service
        .ingest(vec![files.tracks().remove(0)], vec![], "play", vec![])
        .unwrap();
    wait_until(&mut service, "enough audio for one play", |s| {
        s.playback().position > 0.4
    });
    service.dispatch(PlayerCommand::Pause).unwrap();
    let before = service.statistics(None, None).unwrap();
    assert_eq!(before.plays, 1);
    service
        .dispatch(PlayerCommand::SetStatisticsSettings {
            settings: player_application::StatisticsSettings {
                enabled: true,
                percent: 10.,
                seconds: 120.,
            },
        })
        .unwrap();
    tick_for(&mut service, Duration::from_millis(200));
    let paused = service.statistics(None, None).unwrap();
    assert!((paused.listened_seconds - before.listened_seconds).abs() < 0.06);
    service
        .dispatch(PlayerCommand::Seek { position: 1.8 })
        .unwrap();
    tick_for(&mut service, Duration::from_millis(80));
    assert!(service.statistics(None, None).unwrap().listened_seconds < 0.8);
    service.dispatch(PlayerCommand::Play).unwrap();
    wait_until(&mut service, "first pass completed", |s| {
        s.playback().status == "stopped"
    });
    let first = service.statistics(None, None).unwrap();
    assert_eq!(first.plays, 1);
    assert!(first.listened_seconds < 1.);
    assert_eq!(
        service.export_history().unwrap()["events"]
            .as_array()
            .unwrap()
            .len(),
        1,
        "Changing a threshold must not split the current listening pass"
    );
    service.dispatch(PlayerCommand::Play).unwrap();
    wait_until(&mut service, "replayed audio", |s| {
        s.playback().position > 0.4
    });
    service.dispatch(PlayerCommand::Pause).unwrap();
    assert_eq!(service.statistics(None, None).unwrap().plays, 2);
    service.dispatch(PlayerCommand::Stop).unwrap();
}

#[test]
#[ignore = "requires the native libmpv runtime; uses null audio output"]
fn native_full_listen_qualifies_at_one_hundred_percent_but_seek_to_end_does_not() {
    use player_domain::ListeningEvent;
    #[derive(Default)]
    struct History {
        json: Option<String>,
        events: std::collections::BTreeMap<String, ListeningEvent>,
    }
    impl StateStore for History {
        fn load(&mut self) -> AppResult<Option<String>> {
            Ok(self.json.clone())
        }
        fn save(&mut self, value: &str) -> AppResult<()> {
            self.json = Some(value.into());
            Ok(())
        }
        fn record_listening(&mut self, value: &ListeningEvent) -> AppResult<()> {
            self.events.insert(value.id.clone(), value.clone());
            Ok(())
        }
        fn listening_history(
            &mut self,
            _: Option<i64>,
            _: Option<i64>,
        ) -> AppResult<Vec<ListeningEvent>> {
            Ok(self.events.values().cloned().collect())
        }
    }
    let files = TestAudioFiles::new();
    let mut native = MpvEngine::new(&[]).unwrap();
    native.set_property("ao", "null").unwrap();
    let audio = NativeAudio(Arc::new(Mutex::new(native)));
    let mut service =
        PlayerService::new(Box::new(History::default()), Ok(Box::new(audio))).unwrap();
    service
        .dispatch(PlayerCommand::SetStatisticsSettings {
            settings: player_application::StatisticsSettings {
                enabled: true,
                percent: 100.,
                seconds: 0.,
            },
        })
        .unwrap();
    service
        .ingest(vec![files.tracks().remove(0)], vec![], "play", vec![])
        .unwrap();
    wait_until(&mut service, "complete two second WAV", |s| {
        s.playback().status == "stopped"
    });
    let first = service.statistics(None, None).unwrap();
    assert_eq!(
        first.plays, 1,
        "Full track did not qualify: {} listened seconds",
        first.listened_seconds
    );
    assert!(first.listened_seconds >= 2.);
    service.dispatch(PlayerCommand::Play).unwrap();
    wait_until(&mut service, "partial second pass", |s| {
        s.playback().position > 0.15
    });
    service
        .dispatch(PlayerCommand::Seek { position: 1.9 })
        .unwrap();
    wait_until(&mut service, "seeked pass ended", |s| {
        s.playback().status == "stopped"
    });
    let skipped = service.statistics(None, None).unwrap();
    assert_eq!(
        skipped.plays, 1,
        "Seek to EOF must not qualify as full listening"
    );
    assert!(skipped.listened_seconds < 3.);
    service.dispatch(PlayerCommand::Play).unwrap();
    wait_until(&mut service, "third pass before pause", |s| {
        s.playback().position > 0.3
    });
    service.dispatch(PlayerCommand::Pause).unwrap();
    tick_for(&mut service, Duration::from_millis(200));
    service.dispatch(PlayerCommand::Play).unwrap();
    wait_until(&mut service, "full pass with pause completed", |s| {
        s.playback().status == "stopped"
    });
    assert_eq!(
        service.statistics(None, None).unwrap().plays,
        2,
        "A pause must not prevent an otherwise complete pass"
    );
}

#[test]
#[ignore = "requires the native libmpv runtime; uses null audio output"]
fn native_full_listens_count_at_automatic_track_transition_and_repeat_one_boundary() {
    use player_domain::ListeningEvent;
    #[derive(Default)]
    struct History(std::collections::BTreeMap<String, ListeningEvent>);
    impl StateStore for History {
        fn load(&mut self) -> AppResult<Option<String>> {
            Ok(None)
        }
        fn save(&mut self, _: &str) -> AppResult<()> {
            Ok(())
        }
        fn record_listening(&mut self, value: &ListeningEvent) -> AppResult<()> {
            self.0.insert(value.id.clone(), value.clone());
            Ok(())
        }
        fn listening_history(
            &mut self,
            _: Option<i64>,
            _: Option<i64>,
        ) -> AppResult<Vec<ListeningEvent>> {
            Ok(self.0.values().cloned().collect())
        }
    }
    let files = TestAudioFiles::new();
    let mut native = MpvEngine::new(&[]).unwrap();
    native.set_property("ao", "null").unwrap();
    let audio = NativeAudio(Arc::new(Mutex::new(native)));
    let mut service =
        PlayerService::new(Box::new(History::default()), Ok(Box::new(audio))).unwrap();
    service
        .dispatch(PlayerCommand::SetStatisticsSettings {
            settings: player_application::StatisticsSettings {
                enabled: true,
                percent: 100.,
                seconds: 0.,
            },
        })
        .unwrap();
    service
        .ingest(files.tracks(), vec![], "play", vec![])
        .unwrap();
    wait_until(&mut service, "second track after natural transition", |s| {
        current(s) == Some("native-2") && s.playback().position > 0.3
    });
    assert_eq!(
        service.statistics(None, None).unwrap().plays,
        1,
        "Automatic transition must finish the preceding full pass"
    );
    service
        .dispatch(PlayerCommand::SetRepeat {
            mode: RepeatMode::One,
        })
        .unwrap();
    wait_until(&mut service, "repeat one wrapped to start", |s| {
        s.playback().position < 0.2
    });
    assert_eq!(
        service.statistics(None, None).unwrap().plays,
        2,
        "Natural Repeat One must finish a complete pass"
    );
    service
        .dispatch(PlayerCommand::PlayTracks {
            track_ids: vec!["native-1".into()],
            order_mode: None,
        })
        .unwrap();
    wait_until(&mut service, "partial replacement pass", |s| {
        s.playback().position > 0.15
    });
    assert_eq!(
        service.statistics(None, None).unwrap().plays,
        2,
        "Replacing playback must not complete the interrupted pass"
    );
    service.dispatch(PlayerCommand::Next).unwrap();
    assert_eq!(
        service.statistics(None, None).unwrap().plays,
        2,
        "Manual Next must not complete a partial pass"
    );
}
