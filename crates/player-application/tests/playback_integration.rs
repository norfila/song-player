//! Application-level checks use observable engine commands rather than an audio
//! device, so queue transitions and persistence remain reproducible in CI.
use player_application::{
    AppResult, AudioDevice, AudioEvent, AudioPort, PlayerCommand, PlayerService, SavedState,
    StateStore,
};
use player_domain::{PlayOrder, RepeatMode, Track};
use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};

#[derive(Default)]
struct EngineState {
    commands: Vec<Vec<String>>,
    properties: HashMap<String, String>,
    events: VecDeque<AudioEvent>,
    defer_file_loaded: bool,
    fail_property_once: Option<String>,
}

struct MockAudio(Arc<Mutex<EngineState>>);

impl AudioPort for MockAudio {
    fn command(&mut self, args: &[&str]) -> AppResult<()> {
        let mut engine = self.0.lock().unwrap();
        engine
            .commands
            .push(args.iter().map(|value| (*value).to_owned()).collect());
        match args {
            ["loadfile", _, "replace"] => {
                for (key, value) in [
                    ("playlist-pos", "0"),
                    ("time-pos", "0"),
                    ("duration", "120"),
                    ("idle-active", "no"),
                    ("eof-reached", "no"),
                ] {
                    engine.properties.insert(key.into(), value.into());
                }
                if !engine.defer_file_loaded {
                    engine.events.push_back(AudioEvent::FileLoaded);
                }
            }
            ["stop"] => {
                engine.properties.insert("idle-active".into(), "yes".into());
                engine.properties.remove("playlist-pos");
            }
            ["playlist-clear"] => {
                // mpv retains the current stream while removing every other
                // playlist item; that retained stream now occupies index zero.
                engine.properties.insert("playlist-pos".into(), "0".into());
            }
            ["seek", position, _] => {
                engine
                    .properties
                    .insert("time-pos".into(), (*position).into());
            }
            _ => {}
        }
        Ok(())
    }

    fn set_property(&mut self, name: &str, value: &str) -> AppResult<()> {
        let mut engine = self.0.lock().unwrap();
        if engine.fail_property_once.as_deref() == Some(name) {
            engine.fail_property_once = None;
            return Err(player_application::AppError::new(
                "audio_error",
                "Simulated device rejection",
            ));
        }
        engine.properties.insert(name.into(), value.into());
        Ok(())
    }

    fn get_property(&mut self, name: &str) -> Option<String> {
        self.0.lock().unwrap().properties.get(name).cloned()
    }

    fn poll_event(&mut self) -> Option<AudioEvent> {
        self.0.lock().unwrap().events.pop_front()
    }

    fn version(&self) -> String {
        "mock-mpv".into()
    }
    fn devices(&mut self) -> Vec<AudioDevice> {
        vec![]
    }
}

#[derive(Default, Clone)]
struct MemoryStore(Arc<Mutex<Option<String>>>);

impl StateStore for MemoryStore {
    fn load(&mut self) -> AppResult<Option<String>> {
        Ok(self.0.lock().unwrap().clone())
    }
    fn save(&mut self, json: &str) -> AppResult<()> {
        *self.0.lock().unwrap() = Some(json.into());
        Ok(())
    }
}

fn create(store: MemoryStore) -> (PlayerService, Arc<Mutex<EngineState>>) {
    let engine = Arc::new(Mutex::new(EngineState::default()));
    let service =
        PlayerService::new(Box::new(store), Ok(Box::new(MockAudio(engine.clone())))).unwrap();
    (service, engine)
}

fn track(number: u32) -> Track {
    Track {
        id: format!("track-{number}"),
        path: format!("/music/{number}.flac"),
        title: format!("Track {number}"),
        duration: 120.,
        track_number: Some(number),
        ..Track::default()
    }
}

fn load(service: &mut PlayerService, count: u32) {
    service
        .ingest((1..=count).map(track).collect(), vec![], "play", vec![])
        .unwrap();
}

fn set_properties(engine: &Arc<Mutex<EngineState>>, values: &[(&str, &str)]) {
    let mut engine = engine.lock().unwrap();
    for (key, value) in values {
        engine.properties.insert((*key).into(), (*value).into());
    }
}

fn current(service: &PlayerService) -> Option<String> {
    service
        .active()
        .current_entry()
        .map(|entry| entry.track.id.clone())
}

fn sections(names: &[&str]) -> Vec<String> {
    names.iter().map(|s| (*s).into()).collect()
}

fn transfer_document(values: serde_json::Value) -> serde_json::Value {
    serde_json::json!({ "format": "sonora-settings", "version": 1, "exportedAt": 1_700_000_000_000_i64, "sections": values })
}

fn test_appearance() -> serde_json::Value {
    serde_json::json!({
        "activeThemeId": "my-gold", "customThemes": [{
            "version": 1, "id": "my-gold", "name": "My Gold", "base": "gold",
            "fontFamily": "sans", "fontScale": 1.1, "radius": 8.0, "spacing": 1.05, "shadows": 0.3,
            "colors": { "background": "#141414", "panel": "#202020", "surface": "#242424",
                "surfaceHover": "#343434", "border": "#404040", "text": "#FFFFFF", "secondary": "#DDDDDD",
                "muted": "#999999", "accent": "#D7BC7B", "accentText": "#141414", "sidebar": "#141414",
                "player": "#242424", "selection": "#51452F", "success": "#89C79A", "error": "#EF9999" }
        }]
    })
}

#[test]
fn appearance_and_setup_hint_migrate_from_previous_versions() {
    let mut saved = serde_json::to_value(SavedState::default()).unwrap();
    saved["settings"] = serde_json::json!({ "defaultAppReminder": false, "volume": 23. });
    let store = MemoryStore(Arc::new(Mutex::new(Some(saved.to_string()))));
    let (mut service, _) = create(store.clone());
    assert!(!service.snapshot().settings.default_app_reminder);
    assert!(service.snapshot().settings.default_app_hint_seen);
    assert_eq!(
        service.snapshot().settings.appearance.active_theme_id,
        "dark"
    );
    assert_eq!(service.snapshot().settings.volume, 23.);
    service
        .dispatch(PlayerCommand::AcknowledgeDefaultAppHint)
        .unwrap();
    assert!(service.snapshot().settings.default_app_hint_seen);
    let (restored, _) = create(store);
    assert!(!restored.snapshot().settings.default_app_reminder);
    assert!(restored.snapshot().settings.default_app_hint_seen);
}

#[test]
fn default_app_reminder_remains_enabled_after_hint_acknowledgement_until_explicit_opt_out() {
    let store = MemoryStore::default();
    let (mut service, _) = create(store.clone());
    assert!(service.snapshot().settings.default_app_reminder);
    service
        .dispatch(PlayerCommand::AcknowledgeDefaultAppHint)
        .unwrap();
    let (mut restored, _) = create(store.clone());
    assert!(restored.snapshot().settings.default_app_hint_seen);
    assert!(restored.snapshot().settings.default_app_reminder);

    restored
        .dispatch(PlayerCommand::SetDefaultAppReminder { enabled: false })
        .unwrap();
    assert!(!restored.snapshot().settings.default_app_reminder);
    let (restarted, _) = create(store);
    assert!(!restarted.snapshot().settings.default_app_reminder);
}

#[test]
fn failed_default_app_reminder_save_restores_previous_preference() {
    struct ReadOnlyStore(String);
    impl StateStore for ReadOnlyStore {
        fn load(&mut self) -> AppResult<Option<String>> {
            Ok(Some(self.0.clone()))
        }
        fn save(&mut self, _: &str) -> AppResult<()> {
            Err(player_application::AppError::new(
                "storage_error",
                "disk full",
            ))
        }
    }
    for previous in [true, false] {
        let mut saved = SavedState::default();
        saved.settings.default_app_reminder = previous;
        let mut service = PlayerService::new(
            Box::new(ReadOnlyStore(serde_json::to_string(&saved).unwrap())),
            Err("No audio engine needed for reminder preferences".into()),
        )
        .unwrap();
        let result = service.dispatch(PlayerCommand::SetDefaultAppReminder { enabled: !previous });
        assert_eq!(result.unwrap_err().code, "storage_error");
        assert_eq!(service.snapshot().settings.default_app_reminder, previous);
    }
}

#[test]
fn custom_appearance_roundtrips_and_invalid_themes_cannot_replace_it() {
    let (mut service, _) = create(MemoryStore::default());
    let appearance = test_appearance();
    service
        .dispatch(PlayerCommand::SetAppearance {
            appearance: serde_json::from_value(appearance.clone()).unwrap(),
        })
        .unwrap();
    let doc = service
        .export_settings_document(sections(&["appearance"]))
        .unwrap();
    assert_eq!(doc["sections"]["appearance"], appearance);
    for invalid in [
        {
            let mut v = appearance.clone();
            v["customThemes"][0]["colors"]["accent"] = "url(evil)".into();
            v
        },
        {
            let mut v = appearance.clone();
            v["customThemes"][0]["fontScale"] = 3.into();
            v
        },
        {
            let mut v = appearance.clone();
            v["customThemes"][0]["radius"] = (-1).into();
            v
        },
        {
            let mut v = appearance.clone();
            v["customThemes"][0]["fontFamily"] = "external-url".into();
            v
        },
        {
            let mut v = appearance.clone();
            v["customThemes"][0]["id"] = "dark".into();
            v
        },
        {
            let mut v = appearance.clone();
            v["customThemes"][0]["name"] = " ".into();
            v
        },
        {
            let mut v = appearance.clone();
            v["activeThemeId"] = "missing".into();
            v
        },
        {
            let mut v = appearance.clone();
            let duplicate = v["customThemes"][0].clone();
            v["customThemes"].as_array_mut().unwrap().push(duplicate);
            v
        },
    ] {
        assert!(service
            .dispatch(PlayerCommand::SetAppearance {
                appearance: serde_json::from_value(invalid).unwrap()
            })
            .is_err());
        assert_eq!(
            serde_json::to_value(service.snapshot().settings.appearance).unwrap(),
            appearance
        );
    }
}

#[test]
fn settings_transfer_roundtrips_all_categories_and_keeps_music_and_queues() {
    let (mut source, _) = create(MemoryStore::default());
    source
        .dispatch(PlayerCommand::SetLanguage {
            language: "en".into(),
        })
        .unwrap();
    source
        .dispatch(PlayerCommand::SetVolume { volume: 42. })
        .unwrap();
    source
        .dispatch(PlayerCommand::SetMuted { muted: true })
        .unwrap();
    source
        .dispatch(PlayerCommand::SetCloseToTray { enabled: true })
        .unwrap();
    source
        .dispatch(PlayerCommand::SetAppearance {
            appearance: serde_json::from_value(test_appearance()).unwrap(),
        })
        .unwrap();
    source.state.library_folders = vec!["C:/Music/Export".into()];
    let all = sections(&[
        "general",
        "appearance",
        "audio",
        "shortcuts",
        "layout",
        "device",
        "library",
    ]);
    let document = source.export_settings_document(all.clone()).unwrap();
    assert!(document["exportedAt"].as_i64().unwrap() > 1_000_000_000_000);
    let (mut destination, _) = create(MemoryStore::default());
    load(&mut destination, 3);
    destination.state.library_folders = vec!["C:/Music/Existing".into()];
    let queue_before = serde_json::to_value(destination.snapshot().sessions).unwrap();
    let tracks_before = serde_json::to_value(destination.snapshot().tracks).unwrap();
    destination
        .dispatch(PlayerCommand::ImportSettings {
            document,
            sections: all,
        })
        .unwrap();
    assert_eq!(destination.snapshot().settings.language, "en");
    assert_eq!(destination.snapshot().settings.volume, 42.);
    assert!(destination.snapshot().settings.muted);
    assert!(destination.snapshot().settings.close_to_tray);
    assert_eq!(
        destination.snapshot().settings.appearance.active_theme_id,
        "my-gold"
    );
    assert_eq!(
        destination.state.library_folders,
        vec!["C:/Music/Existing", "C:/Music/Export"]
    );
    assert_eq!(
        serde_json::to_value(destination.snapshot().sessions).unwrap(),
        queue_before
    );
    assert_eq!(
        serde_json::to_value(destination.snapshot().tracks).unwrap(),
        tracks_before
    );
}

#[test]
fn settings_transfer_applies_only_selected_sections_and_undo_survives_restart() {
    let store = MemoryStore::default();
    let (mut service, _) = create(store.clone());
    let document = transfer_document(serde_json::json!({
        "general": { "language": "en", "closeToTray": true },
        "audio": { "volume": 2, "muted": true, "replayGain": true, "loudnessNormalization": true }
    }));
    service
        .dispatch(PlayerCommand::ImportSettings {
            document,
            sections: sections(&["general"]),
        })
        .unwrap();
    assert_eq!(service.snapshot().settings.language, "en");
    assert_eq!(service.snapshot().settings.volume, 75.);
    assert!(service.snapshot().settings_undo_available);
    let (mut restored, _) = create(store);
    assert!(restored.snapshot().settings_undo_available);
    restored
        .dispatch(PlayerCommand::UndoSettingsImport)
        .unwrap();
    assert_eq!(restored.snapshot().settings.language, "de");
    assert!(!restored.snapshot().settings.close_to_tray);
    assert!(!restored.snapshot().settings_undo_available);
    assert!(restored
        .dispatch(PlayerCommand::UndoSettingsImport)
        .is_err());
}

#[test]
fn invalid_selected_settings_leave_preferences_and_previous_undo_untouched() {
    let (mut service, _) = create(MemoryStore::default());
    service
        .dispatch(PlayerCommand::ImportSettings {
            document: transfer_document(
                serde_json::json!({ "general": { "language": "en", "closeToTray": true } }),
            ),
            sections: sections(&["general"]),
        })
        .unwrap();
    let before = serde_json::to_value(&service.state.settings).unwrap();
    for document in [
        transfer_document(
            serde_json::json!({ "general": { "language": "de", "closeToTray": false }, "audio": { "volume": 101, "muted": false, "replayGain": false, "loudnessNormalization": false } }),
        ),
        {
            let mut v = transfer_document(serde_json::json!({}));
            v["version"] = 2.into();
            v
        },
        {
            let mut v = transfer_document(serde_json::json!({}));
            v["format"] = "other-app".into();
            v
        },
    ] {
        assert!(service
            .dispatch(PlayerCommand::ImportSettings {
                document,
                sections: sections(&["general", "audio"])
            })
            .is_err());
        assert_eq!(
            serde_json::to_value(&service.state.settings).unwrap(),
            before
        );
        assert!(service.snapshot().settings_undo_available);
    }
    service.dispatch(PlayerCommand::UndoSettingsImport).unwrap();
    assert_eq!(service.snapshot().settings.language, "de");
}

#[test]
fn settings_transfer_rejects_empty_unknown_missing_or_duplicate_sections() {
    let (mut service, _) = create(MemoryStore::default());
    for selection in [
        vec![],
        sections(&["unknown"]),
        sections(&["general", "general"]),
        sections(&["audio"]),
    ] {
        assert!(service
            .dispatch(PlayerCommand::ImportSettings {
                document: transfer_document(serde_json::json!({})),
                sections: selection,
            })
            .is_err());
    }
    assert!(service.export_settings_document(vec![]).is_err());
    assert!(service
        .export_settings_document(sections(&["unknown"]))
        .is_err());
    assert!(!service.snapshot().settings_undo_available);
}

#[test]
fn importing_audio_preferences_changes_native_settings_without_reloading_current_track() {
    let (mut service, engine) = create(MemoryStore::default());
    load(&mut service, 3);
    set_properties(&engine, &[("time-pos", "37")]);
    service.tick();
    engine.lock().unwrap().commands.clear();
    let before = current(&service);
    service.dispatch(PlayerCommand::ImportSettings {
        document: transfer_document(serde_json::json!({ "audio": { "volume": 34, "muted": true, "replayGain": true, "loudnessNormalization": true } })),
        sections: sections(&["audio"]),
    }).unwrap();
    assert_eq!(current(&service), before);
    assert_eq!(service.playback().position, 37.);
    assert_eq!(service.playback().status, "playing");
    let engine = engine.lock().unwrap();
    assert_eq!(engine.properties["volume"], "34");
    assert_eq!(engine.properties["mute"], "yes");
    assert_eq!(engine.properties["replaygain"], "track");
    assert_eq!(engine.commands.len(), 1);
    assert_eq!(engine.commands[0][..2], ["af", "add"]);
}

#[test]
fn failed_settings_save_rolls_back_preferences_native_audio_and_undo_snapshot() {
    struct FailingStore;
    impl StateStore for FailingStore {
        fn load(&mut self) -> AppResult<Option<String>> {
            Ok(None)
        }
        fn save(&mut self, _: &str) -> AppResult<()> {
            Err(player_application::AppError::new(
                "storage_error",
                "disk full",
            ))
        }
    }
    let engine = Arc::new(Mutex::new(EngineState::default()));
    let mut service = PlayerService::new(
        Box::new(FailingStore),
        Ok(Box::new(MockAudio(engine.clone()))),
    )
    .unwrap();
    let result = service.dispatch(PlayerCommand::ImportSettings {
        document: transfer_document(serde_json::json!({ "general": { "language": "en", "closeToTray": true }, "audio": { "volume": 10, "muted": true, "replayGain": true, "loudnessNormalization": true } })),
        sections: sections(&["general", "audio"]),
    });
    assert_eq!(result.unwrap_err().code, "storage_error");
    assert_eq!(service.snapshot().settings.language, "de");
    assert_eq!(service.snapshot().settings.volume, 75.);
    assert!(!service.snapshot().settings_undo_available);
    let engine = engine.lock().unwrap();
    assert_eq!(engine.properties["volume"], "75");
    assert_eq!(engine.properties["mute"], "no");
    assert_eq!(engine.properties["replaygain"], "no");
    assert_eq!(
        engine.commands.last().unwrap(),
        &["af", "remove", "@sonora-loudness"]
    );
}

#[test]
fn native_audio_failure_does_not_partially_apply_an_import() {
    let (mut service, engine) = create(MemoryStore::default());
    engine.lock().unwrap().fail_property_once = Some("audio-device".into());
    let result = service.dispatch(PlayerCommand::ImportSettings {
        document: transfer_document(serde_json::json!({
            "audio": { "volume": 10, "muted": true, "replayGain": true, "loudnessNormalization": true },
            "device": { "outputDevice": "missing-device" }
        })),
        sections: sections(&["audio", "device"]),
    });
    assert_eq!(result.unwrap_err().code, "audio_error");
    assert_eq!(service.snapshot().settings.volume, 75.);
    assert_eq!(service.snapshot().settings.output_device, "auto");
    assert!(!service.snapshot().settings_undo_available);
    let engine = engine.lock().unwrap();
    assert_eq!(engine.properties["volume"], "75");
    assert_eq!(engine.properties["mute"], "no");
    assert_eq!(engine.properties["audio-device"], "auto");
}

#[test]
fn failed_appearance_save_restores_active_theme_and_custom_designs() {
    struct ReadOnlyStore(String);
    impl StateStore for ReadOnlyStore {
        fn load(&mut self) -> AppResult<Option<String>> {
            Ok(Some(self.0.clone()))
        }
        fn save(&mut self, _: &str) -> AppResult<()> {
            Err(player_application::AppError::new(
                "storage_error",
                "disk full",
            ))
        }
    }
    let mut saved = SavedState::default();
    saved.settings.appearance = serde_json::from_value(test_appearance()).unwrap();
    let previous = saved.settings.appearance.clone();
    let mut service = PlayerService::new(
        Box::new(ReadOnlyStore(serde_json::to_string(&saved).unwrap())),
        Err("No audio engine needed for appearance".into()),
    )
    .unwrap();
    let result = service.dispatch(PlayerCommand::SetAppearance {
        appearance: player_application::AppearanceSettings::default(),
    });
    assert_eq!(result.unwrap_err().code, "storage_error");
    assert_eq!(service.snapshot().settings.appearance, previous);
    assert_eq!(service.state.settings.appearance.custom_themes.len(), 1);
    assert!(!service.snapshot().settings_undo_available);
}

#[test]
fn settings_import_rejects_invalid_shortcuts_folders_and_oversized_documents() {
    let (mut service, _) = create(MemoryStore::default());
    for (section, value) in [
        ("shortcuts", serde_json::json!({ "next": "Space" })),
        ("shortcuts", serde_json::json!({ "invented": "KeyJ" })),
        (
            "library",
            serde_json::json!({ "folders": ["C:/Music\u{0}"] }),
        ),
        (
            "layout",
            serde_json::json!({ "panelWidths": { "sidebar": 0, "queue": 500 } }),
        ),
    ] {
        let mut fields = serde_json::Map::new();
        fields.insert(section.into(), value);
        assert!(service
            .dispatch(PlayerCommand::ImportSettings {
                document: transfer_document(fields.into()),
                sections: sections(&[section])
            })
            .is_err());
    }
    let document = transfer_document(
        serde_json::json!({ "device": { "outputDevice": "x".repeat(player_application::MAX_CONFIG_BYTES) } }),
    );
    assert!(service
        .dispatch(PlayerCommand::ImportSettings {
            document,
            sections: sections(&["device"])
        })
        .is_err());
    assert!(!service.snapshot().settings_undo_available);
}

#[test]
fn old_saved_settings_receive_desktop_defaults_without_losing_audio_preferences() {
    let mut saved = serde_json::to_value(SavedState::default()).unwrap();
    saved["settings"] = serde_json::json!({ "language": "en", "volume": 31.5, "muted": true });
    let store = MemoryStore(Arc::new(Mutex::new(Some(saved.to_string()))));
    let (service, _) = create(store);
    let settings = service.snapshot().settings;
    assert_eq!(settings.language, "en");
    assert_eq!(settings.volume, 31.5);
    assert!(settings.muted);
    assert!(!settings.close_to_tray);
    assert!(settings.default_app_reminder);
    assert_eq!(settings.shortcuts["nowPlaying"], "Ctrl+Shift+KeyP");
    assert_eq!(settings.panel_widths.sidebar, 220.);
    assert_eq!(settings.panel_widths.queue, 310.);
}

#[test]
fn desktop_preferences_persist_and_panel_widths_are_bounded() {
    let store = MemoryStore::default();
    let (mut service, _) = create(store.clone());
    for command in [
        PlayerCommand::SetCloseToTray { enabled: true },
        PlayerCommand::SetDefaultAppReminder { enabled: false },
        PlayerCommand::SetPanelWidths {
            sidebar: 120.,
            queue: 800.,
        },
        PlayerCommand::SetShortcuts {
            shortcuts: [
                ("nowPlaying".into(), "Alt+KeyN".into()),
                ("quit".into(), String::new()),
            ]
            .into(),
        },
    ] {
        service.dispatch(command).unwrap();
    }
    let (restored, _) = create(store);
    let settings = restored.snapshot().settings;
    assert!(settings.close_to_tray);
    assert!(!settings.default_app_reminder);
    assert_eq!(settings.panel_widths.sidebar, 160.);
    assert_eq!(settings.panel_widths.queue, 560.);
    assert_eq!(settings.shortcuts["nowPlaying"], "Alt+KeyN");
    assert_eq!(settings.shortcuts["quit"], "");
    assert_eq!(settings.shortcuts["playPause"], "Space");
}

#[test]
fn invalid_desktop_preferences_leave_previous_values_intact() {
    let (mut service, _) = create(MemoryStore::default());
    let original = service.snapshot().settings;
    for binding in [
        "Alt+F4",
        "Alt+Tab",
        "Alt+Space",
        "Alt+Escape",
        "Ctrl+Alt+Delete",
        "Ctrl+Escape",
        "Ctrl+Shift+Escape",
        "Shift+Ctrl+KeyP",
        "Ctrl+Ctrl+KeyP",
        "Meta+KeyP",
        "KeyAA",
        "F25",
        "F01",
        "ShiftLeft",
        "Space",
    ] {
        let error = service
            .dispatch(PlayerCommand::SetShortcuts {
                shortcuts: [("nowPlaying".into(), binding.into())].into(),
            })
            .unwrap_err();
        assert_eq!(error.code, "invalid_input", "{binding}");
        assert_eq!(service.snapshot().settings.shortcuts, original.shortcuts);
    }
    assert!(service
        .dispatch(PlayerCommand::SetShortcuts {
            shortcuts: [("unknown".into(), "KeyA".into())].into()
        })
        .is_err());
    for invalid in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert!(service
            .dispatch(PlayerCommand::SetPanelWidths {
                sidebar: invalid,
                queue: 320.
            })
            .is_err());
        assert_eq!(
            service.snapshot().settings.panel_widths,
            original.panel_widths
        );
    }
}

#[test]
fn persisted_shortcuts_merge_new_defaults_and_recover_from_invalid_values() {
    let mut saved = serde_json::to_value(SavedState::default()).unwrap();
    saved["settings"]["shortcuts"] = serde_json::json!({ "nowPlaying": "Alt+NumpadEqual" });
    saved["settings"]["panelWidths"] = serde_json::json!({ "sidebar": 2000, "queue": 10 });
    let store = MemoryStore(Arc::new(Mutex::new(Some(saved.to_string()))));
    let (service, _) = create(store);
    assert_eq!(
        service.snapshot().settings.shortcuts["nowPlaying"],
        "Alt+NumpadEqual"
    );
    assert_eq!(
        service.snapshot().settings.shortcuts["openFiles"],
        "Ctrl+KeyO"
    );
    assert_eq!(service.snapshot().settings.panel_widths.sidebar, 360.);
    assert_eq!(service.snapshot().settings.panel_widths.queue, 240.);
    saved["settings"]["shortcuts"] = serde_json::json!({ "nowPlaying": "Alt+F4" });
    let store = MemoryStore(Arc::new(Mutex::new(Some(saved.to_string()))));
    let (service, _) = create(store);
    assert_eq!(
        service.snapshot().settings.shortcuts["nowPlaying"],
        "Ctrl+Shift+KeyP"
    );
}

#[test]
fn restore_waits_for_play_and_resumes_at_saved_position() {
    let store = MemoryStore::default();
    let (mut service, engine) = create(store.clone());
    load(&mut service, 3);
    set_properties(&engine, &[("time-pos", "37.5")]);
    service.tick();
    service.persist().unwrap();

    let (mut restored, restored_engine) = create(store);
    assert_eq!(restored.playback().status, "stopped");
    assert_eq!(restored.playback().position, 37.5);
    assert!(restored_engine.lock().unwrap().commands.is_empty());
    restored.dispatch(PlayerCommand::Play).unwrap();
    restored.tick();
    assert_eq!(restored.playback().status, "playing");
    assert_eq!(restored.playback().position, 37.5);
    assert!(restored_engine.lock().unwrap().commands.iter().any(|args| {
        args == &[
            "seek".to_owned(),
            "37.5".to_owned(),
            "absolute+exact".to_owned(),
        ]
    }));
}

#[test]
fn stale_engine_properties_cannot_advance_or_seek_before_file_loaded() {
    let mut state = SavedState::default();
    state.sessions[0].append((1..=3).map(track).collect());
    state.sessions[0].next(true);
    state.sessions[0].position = 44.;
    let store = MemoryStore::default();
    *store.0.lock().unwrap() = Some(serde_json::to_string(&state).unwrap());
    let (mut service, engine) = create(store);
    engine.lock().unwrap().defer_file_loaded = true;

    service.dispatch(PlayerCommand::Play).unwrap();
    // mpv may still expose the previous stream's playlist index and EOF until
    // its asynchronous load completes, even though loadfile has returned.
    set_properties(
        &engine,
        &[
            ("playlist-pos", "1"),
            ("time-pos", "100"),
            ("duration", "110"),
            ("eof-reached", "yes"),
            ("idle-active", "yes"),
        ],
    );
    service.tick();
    assert_eq!(current(&service).as_deref(), Some("track-1"));
    assert_eq!(service.playback().position, 44.);
    assert!(!engine
        .lock()
        .unwrap()
        .commands
        .iter()
        .any(|args| args.first().is_some_and(|value| value == "seek")));

    set_properties(
        &engine,
        &[
            ("playlist-pos", "0"),
            ("time-pos", "0"),
            ("duration", "120"),
            ("eof-reached", "no"),
            ("idle-active", "no"),
        ],
    );
    engine
        .lock()
        .unwrap()
        .events
        .push_back(AudioEvent::FileLoaded);
    service.tick();
    assert_eq!(current(&service).as_deref(), Some("track-1"));
    assert_eq!(service.playback().position, 44.);
    let seeks = engine
        .lock()
        .unwrap()
        .commands
        .iter()
        .filter(|args| args.first().is_some_and(|value| value == "seek"))
        .cloned()
        .collect::<Vec<_>>();
    assert_eq!(
        seeks,
        vec![vec![
            "seek".to_owned(),
            "44".to_owned(),
            "absolute+exact".to_owned()
        ]]
    );
}

#[test]
fn play_can_restart_a_queue_after_its_last_track_finishes() {
    let (mut service, engine) = create(MemoryStore::default());
    load(&mut service, 1);
    set_properties(&engine, &[("eof-reached", "yes"), ("idle-active", "yes")]);
    assert!(service.tick());
    assert_eq!(service.playback().status, "stopped");
    service.dispatch(PlayerCommand::Play).unwrap();
    assert_eq!(service.playback().status, "playing");
    assert_eq!(current(&service).as_deref(), Some("track-1"));
}

#[test]
fn shuffle_and_next_priority_preserve_current_stream_and_position() {
    let (mut service, engine) = create(MemoryStore::default());
    load(&mut service, 5);
    set_properties(&engine, &[("time-pos", "29")]);
    service.tick();
    let current_id = service.active().current_entry().unwrap().id.clone();
    let replacements_before = engine
        .lock()
        .unwrap()
        .commands
        .iter()
        .filter(|args| {
            args.first().is_some_and(|v| v == "loadfile")
                && args.get(2).is_some_and(|v| v == "replace")
        })
        .count();
    service
        .dispatch(PlayerCommand::SetOrder {
            mode: PlayOrder::ShuffleTracks,
        })
        .unwrap();
    service
        .ingest(vec![track(9)], vec![], "next", vec![])
        .unwrap();
    assert_eq!(service.active().current_entry().unwrap().id, current_id);
    assert_eq!(service.playback().position, 29.);
    let replacements_after = engine
        .lock()
        .unwrap()
        .commands
        .iter()
        .filter(|args| {
            args.first().is_some_and(|v| v == "loadfile")
                && args.get(2).is_some_and(|v| v == "replace")
        })
        .count();
    assert_eq!(replacements_before, replacements_after);
    service.dispatch(PlayerCommand::Next).unwrap();
    assert_eq!(current(&service).as_deref(), Some("track-9"));
}

#[test]
fn shuffle_toggle_preserves_the_native_stream_and_restores_the_saved_queue_sort() {
    let store = MemoryStore::default();
    let (mut service, engine) = create(store.clone());
    service
        .ingest(
            (1..=5)
                .map(|number| Track {
                    title: format!("Title {}", 6 - number),
                    ..track(number)
                })
                .collect(),
            vec![],
            "play",
            vec![],
        )
        .unwrap();
    set_properties(&engine, &[("time-pos", "29")]);
    service.tick();
    let current_id = service.active().current_entry().unwrap().id.clone();
    service
        .dispatch(PlayerCommand::SetOrder {
            mode: PlayOrder::Title,
        })
        .unwrap();
    let ordered = service.active().snapshot();
    assert_eq!(
        ordered
            .entries
            .iter()
            .map(|entry| entry.track.id.as_str())
            .collect::<Vec<_>>(),
        vec!["track-1", "track-5", "track-4", "track-3", "track-2"]
    );
    engine.lock().unwrap().commands.clear();
    for enabled in [true, false] {
        let command: PlayerCommand = serde_json::from_value(serde_json::json!({
            "type": "set_shuffle", "enabled": enabled
        }))
        .unwrap();
        service.dispatch(command).unwrap();
        assert_eq!(service.active().current_entry().unwrap().id, current_id);
        assert_eq!(service.playback().position, 29.);
        assert_eq!(service.playback().status, "playing");
    }
    assert_eq!(service.active().snapshot(), ordered);
    {
        let engine = engine.lock().unwrap();
        let commands = &engine.commands;
        assert!(commands.iter().any(|args| args == &["playlist-clear"]));
        assert!(
            commands.iter().all(|args| {
                args.first().is_some_and(|name| name == "playlist-clear")
                    || (args.first().is_some_and(|name| name == "loadfile")
                        && args.get(2).is_some_and(|mode| mode == "append"))
            }),
            "toggling may rebuild future entries, but must not stop, seek, or reload the stream"
        );
    }

    service
        .dispatch(PlayerCommand::SetShuffle { enabled: true })
        .unwrap();
    let (mut restored, restored_engine) = create(store);
    let commands_before = restored_engine.lock().unwrap().commands.len();
    restored
        .dispatch(PlayerCommand::SetShuffle { enabled: false })
        .unwrap();
    assert_eq!(restored.active().snapshot(), ordered);
    assert_eq!(restored.playback().position, 29.);
    assert_eq!(restored.playback().status, "stopped");
    assert_eq!(
        restored_engine.lock().unwrap().commands.len(),
        commands_before
    );
}

#[test]
fn engine_auto_advance_updates_domain_history_for_previous() {
    let (mut service, engine) = create(MemoryStore::default());
    load(&mut service, 3);
    set_properties(&engine, &[("playlist-pos", "1"), ("time-pos", "1")]);
    assert!(service.tick());
    assert_eq!(current(&service).as_deref(), Some("track-2"));
    service.dispatch(PlayerCommand::Previous).unwrap();
    assert_eq!(current(&service).as_deref(), Some("track-1"));
    service.dispatch(PlayerCommand::Next).unwrap();
    assert_eq!(current(&service).as_deref(), Some("track-2"));
}

#[test]
fn manual_next_overrides_repeat_one() {
    let (mut service, engine) = create(MemoryStore::default());
    load(&mut service, 2);
    service
        .dispatch(PlayerCommand::SetRepeat {
            mode: RepeatMode::One,
        })
        .unwrap();
    assert_eq!(
        engine
            .lock()
            .unwrap()
            .properties
            .get("loop-file")
            .map(String::as_str),
        Some("inf")
    );
    service.dispatch(PlayerCommand::Next).unwrap();
    assert_eq!(current(&service).as_deref(), Some("track-2"));
}

#[test]
fn a_broken_file_is_skipped_even_with_repeat_one_enabled() {
    let (mut service, engine) = create(MemoryStore::default());
    load(&mut service, 2);
    service
        .dispatch(PlayerCommand::SetRepeat {
            mode: RepeatMode::One,
        })
        .unwrap();
    engine.lock().unwrap().events.push_back(AudioEvent::Ended {
        reason: "error".into(),
        error: Some("broken file".into()),
    });
    set_properties(&engine, &[("eof-reached", "yes"), ("idle-active", "yes")]);
    assert!(service.tick());
    assert_eq!(current(&service).as_deref(), Some("track-2"));
    assert_eq!(service.snapshot().warnings.len(), 1);
}

#[test]
fn a_burst_of_broken_files_stops_repeat_all_instead_of_restarting_forever() {
    let (mut service, engine) = create(MemoryStore::default());
    load(&mut service, 3);
    service
        .dispatch(PlayerCommand::SetRepeat {
            mode: RepeatMode::All,
        })
        .unwrap();
    for _ in 0..3 {
        engine.lock().unwrap().events.push_back(AudioEvent::Ended {
            reason: "error".into(),
            error: Some("broken file".into()),
        });
    }
    set_properties(&engine, &[("eof-reached", "yes"), ("idle-active", "yes")]);
    assert!(service.tick());
    assert_eq!(service.playback().status, "stopped");
    assert_eq!(service.snapshot().warnings.len(), 3);
}

#[test]
fn opening_a_new_session_preserves_the_old_resume_position() {
    let store = MemoryStore::default();
    let (mut service, engine) = create(store.clone());
    load(&mut service, 2);
    let old_id = service.state.active_session_id.clone();
    set_properties(&engine, &[("time-pos", "51")]);
    service.tick();
    service
        .dispatch(PlayerCommand::NewSession {
            name: "Second".into(),
        })
        .unwrap();
    assert_eq!(
        service
            .state
            .sessions
            .iter()
            .find(|session| session.id == old_id)
            .unwrap()
            .position,
        51.
    );
    let saved: SavedState =
        serde_json::from_str(store.0.lock().unwrap().as_ref().unwrap()).unwrap();
    assert_eq!(
        saved
            .sessions
            .iter()
            .find(|session| session.id == old_id)
            .unwrap()
            .position,
        51.
    );
    service
        .dispatch(PlayerCommand::ActivateSession { session_id: old_id })
        .unwrap();
    assert_eq!(service.playback().position, 51.);
}

#[test]
fn deleting_several_sessions_keeps_one_usable_session() {
    let (mut service, _engine) = create(MemoryStore::default());
    load(&mut service, 2);
    let first = service.state.active_session_id.clone();
    for name in ["Second", "Third"] {
        service
            .dispatch(PlayerCommand::NewSession { name: name.into() })
            .unwrap();
    }
    let third = service.state.active_session_id.clone();
    let second = service.state.sessions[1].id.clone();

    // Inactive sessions disappear without touching the active one.
    service
        .dispatch(PlayerCommand::DeleteSessions {
            session_ids: vec![second.clone()],
        })
        .unwrap();
    assert_eq!(service.state.sessions.len(), 2);
    assert_eq!(service.state.active_session_id, third);

    // Deleting the active session falls back to the first remaining one.
    service
        .dispatch(PlayerCommand::DeleteSessions {
            session_ids: vec![third, second],
        })
        .unwrap();
    assert_eq!(service.state.sessions.len(), 1);
    assert_eq!(service.state.active_session_id, first);
    assert_eq!(service.playback().status, "stopped");

    // Deleting the last session replaces it with a fresh empty one.
    service
        .dispatch(PlayerCommand::DeleteSessions {
            session_ids: vec![first.clone()],
        })
        .unwrap();
    assert_eq!(service.state.sessions.len(), 1);
    assert_ne!(service.state.active_session_id, first);
    assert!(service.active().snapshot().entries.is_empty());
}

#[test]
fn restore_sanitizes_dangling_session_references() {
    let mut state = SavedState::default();
    state.sessions[0].append(vec![track(1)]);
    state.sessions[0].next(true);
    let mut json = serde_json::to_value(state).unwrap();
    json["sessions"][0]["currentEntryId"] = serde_json::json!("missing-entry");
    json["sessions"][0]["position"] = serde_json::json!(-10.0);
    let store = MemoryStore::default();
    *store.0.lock().unwrap() = Some(serde_json::to_string(&json).unwrap());
    let (service, _) = create(store);
    assert!(service.playback().current_entry_id.is_none());
    assert_eq!(service.playback().position, 0.);
    assert_eq!(service.active().snapshot().entries.len(), 1);
}

fn saved_session(tracks: Vec<Track>, position: f64) -> MemoryStore {
    let mut state = SavedState {
        tracks: tracks.clone(),
        ..SavedState::default()
    };
    state.sessions[0].append(tracks);
    state.sessions[0].next(true);
    state.sessions[0].position = position;
    let store = MemoryStore::default();
    *store.0.lock().unwrap() = Some(serde_json::to_string(&state).unwrap());
    store
}

#[test]
fn removing_restored_current_resets_position_and_duration_without_loading_audio() {
    let mut second = track(2);
    second.duration = 240.;
    let store = saved_session(vec![track(1), second], 37.);
    let (mut service, engine) = create(store.clone());
    let removed = service.active().current_entry().unwrap().id.clone();
    service
        .dispatch(PlayerCommand::RemoveEntry { entry_id: removed })
        .unwrap();
    assert_eq!(current(&service).as_deref(), Some("track-2"));
    assert_eq!(service.playback().position, 0.);
    assert_eq!(service.playback().duration, 240.);
    assert_eq!(service.active().position, 0.);
    assert!(engine.lock().unwrap().commands.is_empty());
    let restored: SavedState =
        serde_json::from_str(store.0.lock().unwrap().as_ref().unwrap()).unwrap();
    assert_eq!(restored.sessions[0].position, 0.);

    let last = service.active().current_entry().unwrap().id.clone();
    service
        .dispatch(PlayerCommand::RemoveEntry { entry_id: last })
        .unwrap();
    assert!(current(&service).is_none());
    assert_eq!(service.playback().position, 0.);
    assert_eq!(service.playback().duration, 0.);
}

#[test]
fn previous_after_restore_loads_the_current_track_from_the_start() {
    let (mut service, engine) = create(saved_session(vec![track(1), track(2)], 37.));
    service.dispatch(PlayerCommand::Previous).unwrap();
    assert_eq!(current(&service).as_deref(), Some("track-1"));
    assert_eq!(service.playback().position, 0.);
    assert_eq!(service.playback().status, "playing");
    assert!(engine.lock().unwrap().commands.iter().any(|args| {
        args == &[
            "loadfile".to_owned(),
            "/music/1.flac".to_owned(),
            "replace".to_owned(),
        ]
    }));
}

#[test]
fn next_reconciles_a_pending_native_transition_before_advancing_again() {
    let (mut service, engine) = create(MemoryStore::default());
    load(&mut service, 3);
    service.tick();
    set_properties(&engine, &[("playlist-pos", "1"), ("time-pos", "1")]);
    engine
        .lock()
        .unwrap()
        .events
        .push_back(AudioEvent::FileLoaded);
    // There is intentionally no worker tick between the native transition and
    // this user command. Next must advance from audible track 2 to track 3.
    service.dispatch(PlayerCommand::Next).unwrap();
    assert_eq!(current(&service).as_deref(), Some("track-3"));
}

#[test]
fn sorting_reconciles_a_pending_native_transition_without_requeueing_the_audible_track() {
    let (mut service, engine) = create(MemoryStore::default());
    load(&mut service, 4);
    service.tick();
    set_properties(&engine, &[("playlist-pos", "1"), ("time-pos", "12")]);
    engine
        .lock()
        .unwrap()
        .events
        .push_back(AudioEvent::FileLoaded);
    service
        .dispatch(PlayerCommand::SetOrder {
            mode: PlayOrder::ShuffleTracks,
        })
        .unwrap();
    assert_eq!(current(&service).as_deref(), Some("track-2"));
    assert_eq!(service.playback().position, 12.);
    let snapshot = service.active().snapshot();
    let index = snapshot
        .entries
        .iter()
        .position(|entry| Some(&entry.id) == snapshot.current_entry_id.as_ref())
        .unwrap();
    let future = snapshot.entries[index + 1..]
        .iter()
        .map(|entry| entry.track.id.as_str())
        .collect::<std::collections::HashSet<_>>();
    assert_eq!(future, ["track-3", "track-4"].into_iter().collect());
    service.tick();
    assert_eq!(current(&service).as_deref(), Some("track-2"));
}

#[test]
fn library_rescan_reconciles_only_scanned_folders_and_accepts_an_empty_folder() {
    let (mut service, _) = create(MemoryStore::default());
    service
        .ingest(
            vec![track(1), track(2)],
            vec!["/music".into()],
            "library",
            vec![],
        )
        .unwrap();
    let mut other = track(9);
    other.path = "/other/9.flac".into();
    service
        .ingest(vec![other], vec!["/other".into()], "library", vec![])
        .unwrap();
    assert_eq!(service.state.library_track_ids.len(), 3);

    service
        .ingest(vec![track(2)], vec!["/music".into()], "library", vec![])
        .unwrap();
    assert_eq!(
        service
            .state
            .library_track_ids
            .iter()
            .map(String::as_str)
            .collect::<std::collections::HashSet<_>>(),
        ["track-2", "track-9"].into_iter().collect()
    );
    service
        .ingest(vec![], vec!["/music".into()], "library", vec![])
        .unwrap();
    assert_eq!(service.state.library_track_ids, vec!["track-9".to_owned()]);
    assert!(service.state.library_folders.contains(&"/music".to_owned()));
    assert!(
        service
            .state
            .tracks
            .iter()
            .any(|track| track.id == "track-1"),
        "Removing a missing library reference must preserve playlist/session track data"
    );
}

#[test]
fn published_revisions_increase_across_commands_imports_and_playback_ticks() {
    fn counter(value: impl serde::Serialize, key: &str) -> u64 {
        serde_json::to_value(value)
            .unwrap()
            .get(key)
            .and_then(serde_json::Value::as_u64)
            .expect("Every state/playback payload needs its ordering counter")
    }
    let (mut service, engine) = create(MemoryStore::default());
    let initial = counter(service.snapshot(), "revision");
    load(&mut service, 2);
    let after_import = counter(service.snapshot(), "revision");
    assert!(after_import > initial);
    service.dispatch(PlayerCommand::Pause).unwrap();
    let after_command = counter(service.snapshot(), "revision");
    assert!(after_command > after_import);
    service.dispatch(PlayerCommand::Play).unwrap();
    let before_tick_sequence = counter(service.playback(), "sequence");
    let before_tick_revision = counter(service.snapshot(), "revision");
    set_properties(&engine, &[("time-pos", "8")]);
    service.tick();
    assert!(counter(service.playback(), "sequence") > before_tick_sequence);
    assert!(counter(service.snapshot(), "revision") >= before_tick_revision);
    set_properties(&engine, &[("playlist-pos", "1"), ("time-pos", "1")]);
    engine
        .lock()
        .unwrap()
        .events
        .push_back(AudioEvent::FileLoaded);
    service.tick();
    assert!(counter(service.snapshot(), "revision") > before_tick_revision);
}

#[test]
fn normalization_owns_one_filter_persists_and_restores_without_enabling_replaygain() {
    let store = MemoryStore::default();
    let (mut service, engine) = create(store.clone());
    assert!(!service.state.settings.loudness_normalization);
    service
        .dispatch(PlayerCommand::SetLoudnessNormalization { enabled: true })
        .unwrap();
    assert!(service.snapshot().settings.loudness_normalization);
    assert!(!service.state.settings.replay_gain);
    let saved: SavedState =
        serde_json::from_str(store.0.lock().unwrap().as_deref().unwrap()).unwrap();
    assert!(saved.settings.loudness_normalization);
    let (mut restored, restored_engine) = create(store);
    assert!(restored.state.settings.loudness_normalization);
    let restored_commands = restored_engine.lock().unwrap().commands.clone();
    assert_eq!(restored_commands.len(), 1);
    assert_eq!(restored_commands[0][0..2], ["af", "add"]);
    assert!(restored_commands[0][2].starts_with("@sonora-loudness:lavfi=[dynaudnorm="));
    assert_eq!(engine.lock().unwrap().commands, restored_commands);
    restored
        .dispatch(PlayerCommand::SetLoudnessNormalization { enabled: false })
        .unwrap();
    assert!(!restored.state.settings.loudness_normalization);
    assert_eq!(
        restored_engine.lock().unwrap().commands.last().unwrap(),
        &["af", "remove", "@sonora-loudness"]
    );
}

#[test]
fn older_settings_without_normalization_default_to_disabled() {
    let settings: player_application::Settings = serde_json::from_str(
        r#"{"language":"en","volume":60,"muted":false,"outputDevice":"auto","replayGain":true}"#,
    )
    .unwrap();
    assert!(!settings.loudness_normalization);
    assert!(settings.replay_gain);
}

fn track_at(id: &str, path: &str) -> Track {
    Track {
        id: id.into(),
        path: path.into(),
        title: id.into(),
        duration: 120.,
        ..Track::default()
    }
}

fn library_ids(service: &PlayerService) -> std::collections::HashSet<String> {
    service.state.library_track_ids.iter().cloned().collect()
}

#[test]
fn removing_a_library_folder_keeps_other_folders_and_loose_files_and_persists() {
    let store = MemoryStore::default();
    let (mut service, _) = create(store.clone());
    service
        .ingest(
            vec![
                track_at("a", "/music/a.flac"),
                track_at("b", "/music/sub/b.flac"),
            ],
            vec!["/music".into()],
            "library",
            vec![],
        )
        .unwrap();
    service
        .ingest(
            vec![track_at("c", "/other/c.flac")],
            vec!["/other".into()],
            "library",
            vec![],
        )
        .unwrap();
    // An individually added file outside every library folder.
    service
        .ingest(
            vec![track_at("d", "/loose/d.flac")],
            vec![],
            "library",
            vec![],
        )
        .unwrap();

    service
        .dispatch(PlayerCommand::RemoveLibraryFolder {
            path: "/music".into(),
        })
        .unwrap();
    assert_eq!(service.state.library_folders, vec!["/other".to_owned()]);
    assert_eq!(
        library_ids(&service),
        ["c", "d"].into_iter().map(String::from).collect()
    );

    let (reloaded, _) = create(store);
    assert_eq!(reloaded.state.library_folders, vec!["/other".to_owned()]);
    assert_eq!(
        library_ids(&reloaded),
        ["c", "d"].into_iter().map(String::from).collect()
    );
}

#[test]
fn removed_subfolder_stays_removed_on_rescan_until_added_again() {
    let (mut service, _) = create(MemoryStore::default());
    let scan = || {
        vec![
            track_at("a", "/music/a.flac"),
            track_at("b", "/music/sub/b.flac"),
        ]
    };
    service
        .ingest(scan(), vec!["/music".into()], "library", vec![])
        .unwrap();
    service
        .dispatch(PlayerCommand::RemoveLibraryFolder {
            path: "/music/sub".into(),
        })
        .unwrap();
    assert_eq!(service.state.library_folders, vec!["/music".to_owned()]);
    assert_eq!(
        service.state.excluded_folders,
        vec!["/music/sub".to_owned()]
    );
    assert_eq!(
        library_ids(&service),
        ["a"].into_iter().map(String::from).collect()
    );

    service
        .ingest(scan(), vec!["/music".into()], "rescan", vec![])
        .unwrap();
    assert_eq!(
        library_ids(&service),
        ["a"].into_iter().map(String::from).collect()
    );

    service
        .ingest(
            vec![track_at("b", "/music/sub/b.flac")],
            vec!["/music/sub".into()],
            "library",
            vec![],
        )
        .unwrap();
    assert!(service.state.excluded_folders.is_empty());
    assert_eq!(
        library_ids(&service),
        ["a", "b"].into_iter().map(String::from).collect()
    );
}

#[test]
fn rescan_does_not_restore_a_folder_removed_during_the_scan() {
    let (mut service, _) = create(MemoryStore::default());
    service
        .ingest(
            vec![track_at("a", "/music/a.flac")],
            vec!["/music".into()],
            "library",
            vec![],
        )
        .unwrap();
    service
        .dispatch(PlayerCommand::RemoveLibraryFolder {
            path: "/music".into(),
        })
        .unwrap();
    // The watcher read the folder list before the removal and scans it afterwards.
    service
        .ingest(
            vec![track_at("a", "/music/a.flac")],
            vec!["/music".into()],
            "rescan",
            vec![],
        )
        .unwrap();
    assert!(service.state.library_folders.is_empty());
    assert!(service.state.library_track_ids.is_empty());
}

#[test]
fn seekbar_style_persists_and_travels_with_the_layout_section() {
    use player_application::SeekbarStyle;
    let store = MemoryStore::default();
    let (mut source, _) = create(store.clone());
    assert_eq!(source.state.settings.seekbar_style, SeekbarStyle::Line);
    source
        .dispatch(PlayerCommand::SetSeekbarStyle {
            style: SeekbarStyle::Waveform,
        })
        .unwrap();
    let (reloaded, _) = create(store);
    assert_eq!(
        reloaded.state.settings.seekbar_style,
        SeekbarStyle::Waveform
    );

    let document = source
        .export_settings_document(sections(&["layout"]))
        .unwrap();
    let (mut destination, _) = create(MemoryStore::default());
    destination
        .dispatch(PlayerCommand::ImportSettings {
            document,
            sections: sections(&["layout"]),
        })
        .unwrap();
    assert_eq!(
        destination.state.settings.seekbar_style,
        SeekbarStyle::Waveform
    );
}
