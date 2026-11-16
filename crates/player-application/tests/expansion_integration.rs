use player_application::{
    AppError, AppResult, AudioDevice, AudioEvent, AudioPort, EqualizerSettings, GainGrouping,
    LoudnessValue, PlayerCommand, PlayerService, SavedState, SleepMode, StateStore,
    StatisticsSettings,
};
use player_domain::{ListeningEvent, RepeatMode, Track};
use std::{
    collections::{BTreeMap, HashMap, VecDeque},
    sync::{Arc, Mutex},
};

#[derive(Default)]
struct Data {
    json: Option<String>,
    history: BTreeMap<String, ListeningEvent>,
    fail_save: bool,
}
#[derive(Default, Clone)]
struct Memory(Arc<Mutex<Data>>);
impl StateStore for Memory {
    fn load(&mut self) -> AppResult<Option<String>> {
        Ok(self.0.lock().unwrap().json.clone())
    }
    fn save(&mut self, json: &str) -> AppResult<()> {
        let mut data = self.0.lock().unwrap();
        if data.fail_save {
            return Err(AppError::new("storage_error", "disk full"));
        }
        data.json = Some(json.into());
        Ok(())
    }
    fn record_listening(&mut self, event: &ListeningEvent) -> AppResult<()> {
        self.0
            .lock()
            .unwrap()
            .history
            .insert(event.id.clone(), event.clone());
        Ok(())
    }
    fn listening_history(
        &mut self,
        from: Option<i64>,
        to: Option<i64>,
    ) -> AppResult<Vec<ListeningEvent>> {
        Ok(self
            .0
            .lock()
            .unwrap()
            .history
            .values()
            .filter(|e| {
                e.started_at >= from.unwrap_or(i64::MIN) && e.started_at < to.unwrap_or(i64::MAX)
            })
            .cloned()
            .collect())
    }
    fn clear_listening(&mut self) -> AppResult<()> {
        self.0.lock().unwrap().history.clear();
        Ok(())
    }
}
#[derive(Default)]
struct Engine {
    properties: HashMap<String, String>,
    events: VecDeque<AudioEvent>,
    commands: Vec<Vec<String>>,
    filters: BTreeMap<String, String>,
}
struct Audio(Arc<Mutex<Engine>>);
impl AudioPort for Audio {
    fn command(&mut self, args: &[&str]) -> AppResult<()> {
        let mut engine = self.0.lock().unwrap();
        engine
            .commands
            .push(args.iter().map(|s| s.to_string()).collect());
        match args {
            ["loadfile", _, "replace"] => {
                engine.properties.extend(
                    [
                        ("playlist-pos", "0"),
                        ("pause", "no"),
                        ("duration", "120"),
                        ("time-pos", "0"),
                        ("eof-reached", "no"),
                        ("idle-active", "no"),
                    ]
                    .map(|(k, v)| (k.into(), v.into())),
                );
                engine.events.push_back(AudioEvent::FileLoaded);
            }
            ["playlist-clear"] => {
                engine.properties.insert("playlist-pos".into(), "0".into());
            }
            ["seek", position, _] => {
                engine
                    .properties
                    .insert("time-pos".into(), position.to_string());
            }
            ["af", "add", filter] => {
                let key = filter.split(':').next().unwrap().to_string();
                engine.filters.insert(key, filter.to_string());
            }
            ["af", "remove", filter] => {
                engine.filters.remove(*filter);
            }
            _ => {}
        }
        Ok(())
    }
    fn set_property(&mut self, k: &str, v: &str) -> AppResult<()> {
        self.0.lock().unwrap().properties.insert(k.into(), v.into());
        Ok(())
    }
    fn get_property(&mut self, k: &str) -> Option<String> {
        let engine = self.0.lock().unwrap();
        if k == "af" {
            Some(format!("{:?}", engine.filters))
        } else {
            engine.properties.get(k).cloned()
        }
    }
    fn poll_event(&mut self) -> Option<AudioEvent> {
        self.0.lock().unwrap().events.pop_front()
    }
    fn version(&self) -> String {
        "mock".into()
    }
    fn devices(&mut self) -> Vec<AudioDevice> {
        vec![]
    }
}
fn create(store: Memory) -> (PlayerService, Arc<Mutex<Engine>>) {
    let engine = Arc::new(Mutex::new(Engine::default()));
    (
        PlayerService::new(Box::new(store), Ok(Box::new(Audio(engine.clone())))).unwrap(),
        engine,
    )
}
fn track(id: &str) -> Track {
    Track {
        id: id.into(),
        title: id.into(),
        path: format!("/missing/{id}.flac"),
        duration: 120.,
        content_fingerprint: Some(format!("hash-{id}")),
        ..Default::default()
    }
}
fn command(service: &mut PlayerService, value: serde_json::Value) {
    service
        .dispatch(serde_json::from_value(value).unwrap())
        .unwrap();
}
fn load(service: &mut PlayerService) {
    service
        .ingest(vec![track("a"), track("b")], vec![], "play", vec![])
        .unwrap();
    service.tick();
}

#[test]
fn playlist_membership_is_search_dialog_safe_atomic_and_separate_from_queue_save() {
    let store = Memory::default();
    let (mut service, _) = create(store.clone());
    load(&mut service);
    command(
        &mut service,
        serde_json::json!({"type":"create_playlist","name":"Empty"}),
    );
    let first = service.state.playlists[0].id.clone();
    assert!(service.state.playlists[0].track_ids.is_empty());
    assert!(!service.state.playlists[0].pinned);
    service
        .dispatch(PlayerCommand::SavePlaylist {
            name: "Queue copy".into(),
            playlist_id: None,
        })
        .unwrap();
    let second = service.state.playlists[1].id.clone();
    assert_eq!(service.state.playlists[1].track_ids, vec!["a", "b"]);
    command(
        &mut service,
        serde_json::json!({"type":"set_playlist_membership","trackIds":["a"],"playlistIds":[first],"preservePlaylistIds":[second]}),
    );
    assert_eq!(service.state.playlists[0].track_ids, vec!["a"]);
    assert_eq!(service.state.playlists[1].track_ids, vec!["a", "b"]);
    command(
        &mut service,
        serde_json::json!({"type":"set_playlist_membership","trackIds":["a"],"playlistIds":[first]}),
    );
    assert_eq!(service.state.playlists[1].track_ids, vec!["b"]);
    let previous = serde_json::to_value(&service.state.playlists).unwrap();
    assert!(service.dispatch(serde_json::from_value(serde_json::json!({"type":"set_playlist_membership","trackIds":["b"],"playlistIds":["missing"]})).unwrap()).is_err());
    assert_eq!(
        serde_json::to_value(&service.state.playlists).unwrap(),
        previous
    );
    store.0.lock().unwrap().fail_save = true;
    assert!(service
        .dispatch(PlayerCommand::CreatePlaylist {
            name: "unsaved".into()
        })
        .is_err());
    assert_eq!(service.state.playlists.len(), 2);
}

#[test]
fn favorite_and_playlist_identity_survive_rescan_and_unique_content_move() {
    let store = Memory::default();
    let (mut service, _) = create(store.clone());
    load(&mut service);
    service
        .dispatch(PlayerCommand::SetFavorite {
            track_ids: vec!["a".into()],
            favorite: true,
        })
        .unwrap();
    service
        .dispatch(PlayerCommand::SavePlaylist {
            name: "Saved".into(),
            playlist_id: None,
        })
        .unwrap();
    let mut moved = track("new-path-id");
    moved.content_fingerprint = Some("hash-a".into());
    moved.path = "/new/folder/a.flac".into();
    moved.title = "Renamed title".into();
    service
        .ingest(vec![moved], vec!["/new/folder".into()], "library", vec![])
        .unwrap();
    assert_eq!(service.state.favorite_track_ids, vec!["a"]);
    assert_eq!(
        service
            .state
            .tracks
            .iter()
            .find(|t| t.id == "a")
            .unwrap()
            .path,
        "/new/folder/a.flac"
    );
    assert_eq!(service.state.playlists[0].track_ids, vec!["a", "b"]);
    assert_eq!(
        service.active().current_entry().unwrap().track.title,
        "Renamed title"
    );
    let mut rescan = track("new-path-id");
    rescan.content_fingerprint = Some("hash-a".into());
    rescan.path = "/new/folder/a.flac".into();
    service
        .ingest(vec![rescan], vec!["/new/folder".into()], "library", vec![])
        .unwrap();
    assert_eq!(service.state.tracks.len(), 2);
    let (restored, _) = create(store);
    assert_eq!(restored.state.favorite_track_ids, vec!["a"]);
}

#[test]
fn ambiguous_identical_copies_never_merge_track_identity() {
    let (mut service, _) = create(Memory::default());
    let a = track("a");
    let mut b = track("b");
    b.content_fingerprint = a.content_fingerprint.clone();
    service
        .ingest(vec![a, b], vec![], "library", vec![])
        .unwrap();
    let mut moved = track("c");
    moved.content_fingerprint = Some("hash-a".into());
    service
        .ingest(vec![moved], vec![], "library", vec![])
        .unwrap();
    assert_eq!(service.state.tracks.len(), 3);
}

#[test]
fn settings_migrate_fullscreen_and_legacy_playlist_sidebar_defaults() {
    let mut value = serde_json::to_value(SavedState::default()).unwrap();
    value["settings"]["shortcuts"] =
        serde_json::json!({"fullscreen":"F11","nowPlaying":"Alt+KeyN"});
    value["playlists"] = serde_json::json!([{"id":"old","name":"Old","trackIds":[],"createdAt":0}]);
    let store = Memory::default();
    store.0.lock().unwrap().json = Some(value.to_string());
    let (service, _) = create(store);
    assert!(service.state.playlists[0].pinned);
    assert_eq!(service.state.playlists[0].icon.value, "music");
    assert!(!service.state.settings.shortcuts.contains_key("fullscreen"));
    assert_eq!(service.state.settings.shortcuts["nowPlaying"], "Alt+KeyN");
}

#[test]
fn equalizer_and_analyzed_folder_gain_own_only_their_filters() {
    let store = Memory::default();
    let (mut service, engine) = create(store.clone());
    load(&mut service);
    engine
        .lock()
        .unwrap()
        .filters
        .insert("@other".into(), "@other:volume=0.5".into());
    service
        .dispatch(PlayerCommand::SetEqualizer {
            equalizer: EqualizerSettings {
                enabled: true,
                bands: [5., 0., -2., 1., 0.],
                preamp: 0.,
            },
        })
        .unwrap();
    service
        .dispatch(PlayerCommand::SetLoudnessValues {
            values: vec![LoudnessValue {
                track_id: "a".into(),
                gain: 3.,
                peak: 0.5,
                grouping: GainGrouping::Folder,
                group_key: Some("/missing".into()),
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
    {
        let e = engine.lock().unwrap();
        assert!(e.filters["@sonora-equalizer"].contains("volume=-5.0000dB"));
        assert!(e.filters["@sonora-replaygain"].contains("3.0000dB"));
        assert_eq!(e.properties["replaygain"], "no");
        assert!(e.filters.contains_key("@other"));
    }
    service
        .dispatch(PlayerCommand::SetEqualizer {
            equalizer: EqualizerSettings::default(),
        })
        .unwrap();
    assert!(!engine
        .lock()
        .unwrap()
        .filters
        .contains_key("@sonora-equalizer"));
    assert!(engine.lock().unwrap().filters.contains_key("@other"));
    store.0.lock().unwrap().fail_save = true;
    assert!(service
        .dispatch(PlayerCommand::SetGainGrouping {
            grouping: GainGrouping::Album
        })
        .is_err());
    assert_eq!(service.state.settings.gain_grouping, GainGrouping::Folder);
}

#[test]
fn queue_timer_ends_even_with_repeat_all_and_track_timer_bypasses_repeat_one() {
    let (mut service, engine) = create(Memory::default());
    load(&mut service);
    service
        .dispatch(PlayerCommand::SetRepeat {
            mode: RepeatMode::All,
        })
        .unwrap();
    service
        .dispatch(PlayerCommand::SetSleepTimer {
            mode: SleepMode::Queue,
            minutes: 0.,
        })
        .unwrap();
    engine.lock().unwrap().properties.extend([
        ("playlist-pos".into(), "1".into()),
        ("time-pos".into(), "120".into()),
        ("eof-reached".into(), "yes".into()),
    ]);
    service.tick();
    assert!(service.snapshot().sleep_timer.is_none());
    assert_eq!(service.playback().status, "paused");
    assert_eq!(service.active().current_entry().unwrap().track.id, "b");
    service
        .dispatch(PlayerCommand::PlayTracks {
            track_ids: vec!["a".into(), "b".into()],
            order_mode: None,
        })
        .unwrap();
    service.tick();
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
    assert_eq!(engine.lock().unwrap().properties["loop-file"], "no");
    engine
        .lock()
        .unwrap()
        .properties
        .insert("eof-reached".into(), "yes".into());
    service.tick();
    assert_eq!(service.playback().status, "paused");
    assert_eq!(service.active().current_entry().unwrap().track.id, "a");
    assert_eq!(engine.lock().unwrap().properties["loop-file"], "inf");
}

#[test]
fn statistics_recalculate_threshold_filter_ranges_and_export_without_snapshot_history() {
    let store = Memory::default();
    for (id, started, listened, duration) in [
        ("one", 10, 60., 120.),
        ("two", 20, 30., 120.),
        ("three", 30, 250., 600.),
    ] {
        store.0.lock().unwrap().history.insert(
            id.into(),
            ListeningEvent {
                id: id.into(),
                track_id: "a".into(),
                title: "Title".into(),
                artist: "Artist".into(),
                album: "Album".into(),
                path: "/a".into(),
                started_at: started,
                listened_seconds: listened,
                duration,
            },
        );
    }
    let (mut service, _) = create(store.clone());
    let all = service.statistics(None, None).unwrap();
    assert_eq!(all.plays, 2);
    assert_eq!(all.listened_seconds, 340.);
    assert_eq!(all.tracks.len(), 1);
    assert_eq!(all.artists[0].plays, 2);
    assert_eq!(service.statistics(Some(20), Some(30)).unwrap().plays, 0);
    service
        .dispatch(PlayerCommand::SetStatisticsSettings {
            settings: StatisticsSettings {
                enabled: true,
                percent: 20.,
                seconds: 0.,
            },
        })
        .unwrap();
    assert_eq!(service.statistics(None, None).unwrap().plays, 3);
    let export = service.export_history().unwrap();
    assert_eq!(export["events"].as_array().unwrap().len(), 3);
    assert!(!serde_json::to_value(service.snapshot())
        .unwrap()
        .as_object()
        .unwrap()
        .contains_key("history"));
    service.clear_statistics().unwrap();
    assert_eq!(service.statistics(None, None).unwrap().plays, 0);
}

#[test]
fn new_settings_roundtrip_optional_sections_and_legacy_import_preserves_omitted_preferences() {
    let (mut service, _) = create(Memory::default());
    command(
        &mut service,
        serde_json::json!({"type":"set_library_options","openExternalNowPlaying":false,"suggestExternalFolders":false}),
    );
    command(
        &mut service,
        serde_json::json!({"type":"set_statistics_settings","settings":{"enabled":false,"percent":75,"seconds":0}}),
    );
    command(
        &mut service,
        serde_json::json!({"type":"set_equalizer","equalizer":{"enabled":true,"bands":[3,0,-2,1,0],"preamp":-4}}),
    );
    command(
        &mut service,
        serde_json::json!({"type":"set_gain_grouping","grouping":"folder"}),
    );
    command(
        &mut service,
        serde_json::json!({"type":"set_global_search","enabled":true,"binding":"Alt+KeyP"}),
    );
    let sections = vec!["general".into(), "audio".into(), "device".into()];
    let document = service.export_settings_document(sections.clone()).unwrap();
    let (mut imported, _) = create(Memory::default());
    imported
        .dispatch(PlayerCommand::ImportSettings { document, sections })
        .unwrap();
    let settings = &imported.state.settings;
    assert!(
        !settings.open_external_now_playing
            && !settings.suggest_external_folders
            && !settings.statistics.enabled
    );
    assert_eq!(settings.statistics.percent, 75.);
    assert_eq!(settings.equalizer.bands, [3., 0., -2., 1., 0.]);
    assert_eq!(settings.gain_grouping, GainGrouping::Folder);
    assert_eq!(settings.global_search.binding, "Alt+KeyP");
    command(
        &mut imported,
        serde_json::json!({"type":"import_settings","document":{"format":"sonora-settings","version":1,"exportedAt":0,"sections":{"general":{"language":"en","closeToTray":true},"audio":{"volume":45,"muted":false,"replayGain":false,"loudnessNormalization":false}}},"sections":["general","audio"]}),
    );
    assert_eq!(imported.state.settings.statistics.percent, 75.);
    assert_eq!(imported.state.settings.equalizer.bands[0], 3.);
    assert_eq!(imported.state.settings.gain_grouping, GainGrouping::Folder);
}

#[test]
fn group_cache_is_invalidated_when_group_membership_changes_but_track_gain_is_kept() {
    let (mut service, _) = create(Memory::default());
    load(&mut service);
    let group = player_application::loudness_group_key(&track("a"), GainGrouping::Folder);
    service
        .dispatch(PlayerCommand::SetLoudnessValues {
            values: vec![
                LoudnessValue {
                    track_id: "a".into(),
                    gain: 1.,
                    peak: 0.5,
                    grouping: GainGrouping::Track,
                    group_key: None,
                },
                LoudnessValue {
                    track_id: "a".into(),
                    gain: 2.,
                    peak: 0.5,
                    grouping: GainGrouping::Folder,
                    group_key: Some(group.clone()),
                },
                LoudnessValue {
                    track_id: "b".into(),
                    gain: 2.,
                    peak: 0.5,
                    grouping: GainGrouping::Folder,
                    group_key: Some(group),
                },
            ],
        })
        .unwrap();
    service
        .ingest(vec![track("c")], vec![], "library", vec![])
        .unwrap();
    assert_eq!(service.state.loudness_values.len(), 1);
    assert_eq!(
        service.state.loudness_values[0].grouping,
        GainGrouping::Track
    );
}
