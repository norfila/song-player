//! Application use cases. This crate depends only on the domain and abstract ports.
use player_domain::{
    ListeningEvent, PlayOrder, QueueEntry, RepeatMode, Session, SessionSnapshot, Track,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashSet};
use std::time::{Instant, SystemTime, UNIX_EPOCH};
use uuid::Uuid;
mod audio_preferences;
mod expansion;
mod listening;
mod settings;
pub use expansion::{
    loudness_group_key, EqualizerSettings, GainGrouping, GlobalSearchSettings, LoudnessValue,
    PlaylistIcon, SleepMode, SleepTimer, StatisticsSettings,
};
pub use listening::{ArtistStatistics, Statistics, TrackStatistics};
pub use settings::{
    AppearanceSettings, SettingsImportUndo, ThemeColors, ThemeDefinition, MAX_CONFIG_BYTES,
};

#[derive(Debug, Clone, Serialize)]
pub struct AppError {
    pub code: String,
    pub details: String,
}
impl AppError {
    pub fn new(code: &str, details: impl ToString) -> Self {
        Self {
            code: code.into(),
            details: details.to_string(),
        }
    }
}
impl std::fmt::Display for AppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code, self.details)
    }
}
impl std::error::Error for AppError {}
pub type AppResult<T> = Result<T, AppError>;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioDevice {
    pub id: String,
    pub name: String,
}
pub enum AudioEvent {
    FileLoaded,
    Ended {
        reason: String,
        error: Option<String>,
    },
}

pub trait AudioPort: Send {
    fn command(&mut self, args: &[&str]) -> AppResult<()>;
    fn set_property(&mut self, name: &str, value: &str) -> AppResult<()>;
    fn get_property(&mut self, name: &str) -> Option<String>;
    fn poll_event(&mut self) -> Option<AudioEvent>;
    fn version(&self) -> String;
    fn devices(&mut self) -> Vec<AudioDevice>;
}
pub trait StateStore: Send {
    fn load(&mut self) -> AppResult<Option<String>>;
    fn save(&mut self, json: &str) -> AppResult<()>;
    fn record_listening(&mut self, _event: &ListeningEvent) -> AppResult<()> {
        Ok(())
    }
    fn listening_history(
        &mut self,
        _from: Option<i64>,
        _to: Option<i64>,
    ) -> AppResult<Vec<ListeningEvent>> {
        Ok(vec![])
    }
    /// Calendar years with recorded listening time, in the user's local time zone.
    fn listening_years(&mut self) -> AppResult<Vec<i32>> {
        Ok(vec![])
    }
    fn clear_listening(&mut self) -> AppResult<()> {
        Ok(())
    }
}

// Own only this labelled filter so future equalizers or user filters survive
// normalization toggles. `af add` replaces an existing matching label in mpv.
const NORMALIZATION_LABEL: &str = "@sonora-loudness";
const NORMALIZATION_FILTER: &str =
    "@sonora-loudness:lavfi=[dynaudnorm=f=150:g=15:p=0.95:m=10:r=0.16:n=1:t=0.001]";

pub const DEFAULT_SHORTCUTS: [(&str, &str); 11] = [
    ("playPause", "Space"),
    ("previous", "Ctrl+ArrowLeft"),
    ("next", "Ctrl+ArrowRight"),
    ("seekBack", "ArrowLeft"),
    ("seekForward", "ArrowRight"),
    ("openFiles", "Ctrl+KeyO"),
    ("addFolder", "Ctrl+Shift+KeyO"),
    ("search", "Ctrl+KeyF"),
    ("nowPlaying", "Ctrl+Shift+KeyP"),
    ("toggleQueue", "Ctrl+Shift+KeyQ"),
    ("quit", "Ctrl+KeyQ"),
];

fn default_shortcuts() -> BTreeMap<String, String> {
    DEFAULT_SHORTCUTS
        .into_iter()
        .map(|(action, binding)| (action.into(), binding.into()))
        .collect()
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct PanelWidths {
    pub sidebar: f64,
    pub queue: f64,
}
impl Default for PanelWidths {
    fn default() -> Self {
        Self {
            sidebar: 220.,
            queue: 310.,
        }
    }
}
impl PanelWidths {
    fn bounded(sidebar: f64, queue: f64) -> AppResult<Self> {
        if !sidebar.is_finite() || !queue.is_finite() {
            return Err(AppError::new(
                "invalid_input",
                "Panel widths must be finite",
            ));
        }
        Ok(Self {
            sidebar: sidebar.clamp(160., 360.),
            queue: queue.clamp(240., 560.),
        })
    }
}

fn validate_shortcuts(input: BTreeMap<String, String>) -> AppResult<BTreeMap<String, String>> {
    let mut bindings = default_shortcuts();
    for (action, binding) in input {
        if action == "fullscreen" {
            continue;
        }
        if !bindings.contains_key(&action) || !valid_shortcut(&binding) {
            return Err(AppError::new(
                "invalid_input",
                "Unsupported keyboard shortcut",
            ));
        }
        bindings.insert(action, binding);
    }
    let mut assigned = HashSet::new();
    if bindings
        .values()
        .filter(|b| !b.is_empty())
        .any(|b| !assigned.insert(b))
    {
        return Err(AppError::new(
            "invalid_input",
            "Duplicate keyboard shortcut",
        ));
    }
    Ok(bindings)
}

fn valid_shortcut(binding: &str) -> bool {
    if binding.is_empty() {
        return true;
    }
    if [
        "Alt+F4",
        "Alt+Tab",
        "Ctrl+Alt+Delete",
        "Ctrl+Escape",
        "Ctrl+Shift+Escape",
        "Alt+Escape",
        "Alt+Space",
    ]
    .contains(&binding)
    {
        return false;
    }
    let parts: Vec<_> = binding.split('+').collect();
    let Some((code, modifiers)) = parts.split_last() else {
        return false;
    };
    let mut last = 0;
    for modifier in modifiers {
        let order = match *modifier {
            "Ctrl" => 1,
            "Alt" => 2,
            "Shift" => 3,
            _ => return false,
        };
        if order <= last {
            return false;
        }
        last = order;
    }
    let named = [
        "Space",
        "Enter",
        "Escape",
        "Tab",
        "Backspace",
        "Delete",
        "Insert",
        "Home",
        "End",
        "PageUp",
        "PageDown",
        "ArrowLeft",
        "ArrowRight",
        "ArrowUp",
        "ArrowDown",
        "Minus",
        "Equal",
        "BracketLeft",
        "BracketRight",
        "Backslash",
        "IntlBackslash",
        "Semicolon",
        "Quote",
        "Backquote",
        "Comma",
        "Period",
        "Slash",
        "NumpadAdd",
        "NumpadSubtract",
        "NumpadMultiply",
        "NumpadDivide",
        "NumpadDecimal",
        "NumpadEnter",
        "NumpadEqual",
    ];
    named.contains(code)
        || code
            .strip_prefix("Key")
            .is_some_and(|s| s.len() == 1 && s.as_bytes()[0].is_ascii_uppercase())
        || code
            .strip_prefix("Digit")
            .is_some_and(|s| s.len() == 1 && s.as_bytes()[0].is_ascii_digit())
        || code
            .strip_prefix("Numpad")
            .is_some_and(|s| s.len() == 1 && s.as_bytes()[0].is_ascii_digit())
        || code
            .strip_prefix('F')
            .and_then(|s| s.parse::<u8>().ok())
            .is_some_and(|n| (1..=24).contains(&n) && format!("F{n}") == *code)
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    pub language: String,
    pub volume: f64,
    pub muted: bool,
    pub output_device: String,
    pub replay_gain: bool,
    pub loudness_normalization: bool,
    pub close_to_tray: bool,
    pub default_app_reminder: bool,
    pub default_app_hint_seen: bool,
    pub appearance: AppearanceSettings,
    pub shortcuts: BTreeMap<String, String>,
    pub panel_widths: PanelWidths,
    pub open_external_now_playing: bool,
    pub suggest_external_folders: bool,
    pub statistics: StatisticsSettings,
    pub equalizer: EqualizerSettings,
    pub gain_grouping: GainGrouping,
    pub global_search: GlobalSearchSettings,
    pub seekbar_style: SeekbarStyle,
}
/// How the playback position is drawn in the player bar and Now Playing.
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SeekbarStyle {
    #[default]
    Line,
    Waveform,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            language: "de".into(),
            volume: 75.,
            muted: false,
            output_device: "auto".into(),
            replay_gain: false,
            loudness_normalization: false,
            close_to_tray: false,
            default_app_reminder: true,
            default_app_hint_seen: false,
            appearance: AppearanceSettings::default(),
            shortcuts: default_shortcuts(),
            panel_widths: PanelWidths::default(),
            open_external_now_playing: true,
            suggest_external_folders: true,
            statistics: StatisticsSettings::default(),
            equalizer: EqualizerSettings::default(),
            gain_grouping: GainGrouping::default(),
            global_search: GlobalSearchSettings::default(),
            seekbar_style: SeekbarStyle::default(),
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Playlist {
    pub id: String,
    pub name: String,
    pub track_ids: Vec<String>,
    pub created_at: i64,
    #[serde(default = "expansion::legacy_pinned")]
    pub pinned: bool,
    #[serde(default)]
    pub icon: PlaylistIcon,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SavedState {
    pub version: u32,
    pub tracks: Vec<Track>,
    pub library_track_ids: Vec<String>,
    pub library_folders: Vec<String>,
    /// Subfolders of a library folder that the user removed from the library.
    /// Rescans of the containing folder skip them until the user adds them again.
    pub excluded_folders: Vec<String>,
    pub playlists: Vec<Playlist>,
    pub sessions: Vec<Session>,
    pub active_session_id: String,
    pub settings: Settings,
    pub settings_import_undo: Option<SettingsImportUndo>,
    pub favorite_track_ids: Vec<String>,
    pub loudness_values: Vec<LoudnessValue>,
}
impl Default for SavedState {
    fn default() -> Self {
        let session = Session::new("");
        Self {
            version: 1,
            tracks: vec![],
            library_track_ids: vec![],
            library_folders: vec![],
            excluded_folders: vec![],
            playlists: vec![],
            active_session_id: session.id.clone(),
            sessions: vec![session],
            settings: Settings::default(),
            settings_import_undo: None,
            favorite_track_ids: vec![],
            loudness_values: vec![],
        }
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EngineInfo {
    pub available: bool,
    pub version: String,
    pub error: Option<String>,
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Playback {
    pub sequence: u64,
    pub status: String,
    pub position: f64,
    pub duration: f64,
    pub volume: f64,
    pub muted: bool,
    pub current_entry_id: Option<String>,
    pub track_id: Option<String>,
    pub session_id: Option<String>,
}
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSnapshot {
    pub revision: u64,
    pub tracks: Vec<Track>,
    pub library_track_ids: Vec<String>,
    pub library_folders: Vec<String>,
    pub excluded_folders: Vec<String>,
    pub playlists: Vec<Playlist>,
    pub sessions: Vec<SessionSnapshot>,
    pub active_session_id: String,
    pub settings: Settings,
    pub playback: Playback,
    pub engine: EngineInfo,
    pub warnings: Vec<String>,
    pub settings_undo_available: bool,
    pub favorite_track_ids: Vec<String>,
    pub loudness_values: Vec<LoudnessValue>,
    pub sleep_timer: Option<SleepTimer>,
}

#[derive(Debug, Deserialize)]
#[serde(
    tag = "type",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
pub enum PlayerCommand {
    CreatePlaylist {
        name: String,
    },
    SetPlaylistPinned {
        playlist_id: String,
        pinned: bool,
    },
    SetPlaylistIcon {
        playlist_id: String,
        icon: PlaylistIcon,
    },
    SetPlaylistMembership {
        track_ids: Vec<String>,
        playlist_ids: Vec<String>,
        #[serde(default)]
        preserve_playlist_ids: Vec<String>,
    },
    SetFavorite {
        track_ids: Vec<String>,
        favorite: bool,
    },
    SetLibraryOptions {
        open_external_now_playing: bool,
        suggest_external_folders: bool,
    },
    SetStatisticsSettings {
        settings: StatisticsSettings,
    },
    SetEqualizer {
        equalizer: EqualizerSettings,
    },
    SetGainGrouping {
        grouping: GainGrouping,
    },
    SetGlobalSearch {
        enabled: bool,
        binding: String,
    },
    SetSleepTimer {
        mode: SleepMode,
        minutes: f64,
    },
    CancelSleepTimer,
    SetLoudnessValues {
        values: Vec<LoudnessValue>,
    },
    PlayPause,
    Play,
    Pause,
    Stop,
    Next,
    Previous,
    Seek {
        position: f64,
    },
    SetVolume {
        volume: f64,
    },
    SetMuted {
        muted: bool,
    },
    SetRepeat {
        mode: RepeatMode,
    },
    SetOrder {
        mode: PlayOrder,
    },
    SetShuffle {
        enabled: bool,
    },
    PlayEntry {
        entry_id: String,
    },
    RemoveEntry {
        entry_id: String,
    },
    MoveEntry {
        entry_id: String,
        before_entry_id: Option<String>,
    },
    ClearQueue,
    PlayTracks {
        track_ids: Vec<String>,
        order_mode: Option<PlayOrder>,
    },
    AppendTracks {
        track_ids: Vec<String>,
    },
    EnqueueNextTracks {
        track_ids: Vec<String>,
    },
    NewSession {
        name: String,
    },
    ActivateSession {
        session_id: String,
    },
    RenameSession {
        session_id: String,
        name: String,
    },
    DeleteSession {
        session_id: String,
    },
    DeleteSessions {
        session_ids: Vec<String>,
    },
    SavePlaylist {
        name: String,
        playlist_id: Option<String>,
    },
    DeletePlaylist {
        playlist_id: String,
    },
    PlayPlaylist {
        playlist_id: String,
    },
    RenamePlaylist {
        playlist_id: String,
        name: String,
    },
    AddToPlaylist {
        playlist_id: String,
        track_ids: Vec<String>,
    },
    RemoveFromPlaylist {
        playlist_id: String,
        index: usize,
    },
    SetLanguage {
        language: String,
    },
    RemoveLibraryFolder {
        path: String,
    },
    SetAudioDevice {
        id: String,
    },
    SetReplayGain {
        enabled: bool,
    },
    SetLoudnessNormalization {
        enabled: bool,
    },
    SetCloseToTray {
        enabled: bool,
    },
    SetDefaultAppReminder {
        enabled: bool,
    },
    AcknowledgeDefaultAppHint,
    SetAppearance {
        appearance: AppearanceSettings,
    },
    ImportSettings {
        document: serde_json::Value,
        sections: Vec<String>,
    },
    UndoSettingsImport,
    SetShortcuts {
        shortcuts: BTreeMap<String, String>,
    },
    SetSeekbarStyle {
        style: SeekbarStyle,
    },
    SetPanelWidths {
        sidebar: f64,
        queue: f64,
    },
}

struct PausedSeek {
    position: f64,
    resumed_at: Option<Instant>,
}
impl PausedSeek {
    fn new(position: f64) -> Self {
        Self {
            position,
            resumed_at: None,
        }
    }
}

pub struct PlayerService {
    revision: u64,
    playback_sequence: u64,
    pub state: SavedState,
    store: Box<dyn StateStore>,
    audio: Option<Box<dyn AudioPort>>,
    engine_info: EngineInfo,
    status: String,
    position: f64,
    duration: f64,
    engine_entries: Vec<String>,
    resume_position: Option<f64>,
    paused_seek: Option<PausedSeek>,
    eof_handled: bool,
    consecutive_errors: usize,
    awaiting_load: bool,
    warnings: Vec<String>,
    listening: listening::ListeningTracker,
    sleep_timer: Option<SleepTimer>,
    sleep_deadline: Option<Instant>,
}
impl PlayerService {
    pub fn new(
        mut store: Box<dyn StateStore>,
        audio: Result<Box<dyn AudioPort>, String>,
    ) -> AppResult<Self> {
        let mut state: SavedState = match store.load()? {
            Some(json) => {
                serde_json::from_str(&json).map_err(|e| AppError::new("storage_error", e))?
            }
            None => SavedState::default(),
        };
        if state.version > 1 {
            return Err(AppError::new(
                "storage_error",
                "Database created by a newer application",
            ));
        }
        if state.sessions.is_empty() {
            state.sessions.push(Session::new(""));
        }
        for session in &mut state.sessions {
            session.sanitize();
        }
        if !state
            .sessions
            .iter()
            .any(|s| s.id == state.active_session_id)
        {
            state.active_session_id = state.sessions[0].id.clone();
        }
        if !state.settings.volume.is_finite() {
            state.settings.volume = 75.;
        }
        state.settings.volume = state.settings.volume.clamp(0., 100.);
        state.settings.default_app_hint_seen |= !state.settings.default_app_reminder;
        state.settings.appearance =
            settings::validate_appearance(state.settings.appearance).unwrap_or_default();
        if let Some(previous) = state.settings_import_undo.as_mut() {
            previous.settings.appearance =
                settings::validate_appearance(previous.settings.appearance.clone())
                    .unwrap_or_default();
            previous.settings.shortcuts =
                validate_shortcuts(std::mem::take(&mut previous.settings.shortcuts))
                    .unwrap_or_else(|_| default_shortcuts());
        }
        state.settings.panel_widths = PanelWidths::bounded(
            state.settings.panel_widths.sidebar,
            state.settings.panel_widths.queue,
        )
        .unwrap_or_default();
        state.settings.shortcuts =
            validate_shortcuts(std::mem::take(&mut state.settings.shortcuts))
                .unwrap_or_else(|_| default_shortcuts());
        if state.settings.statistics.validate().is_err() {
            state.settings.statistics = StatisticsSettings::default();
        }
        if state.settings.equalizer.validate().is_err() {
            state.settings.equalizer = EqualizerSettings::default();
        }
        if !valid_shortcut(&state.settings.global_search.binding) {
            state.settings.global_search = GlobalSearchSettings::default();
        }
        for playlist in &mut state.playlists {
            if playlist.icon.validate().is_err() {
                playlist.icon = PlaylistIcon::default();
            }
        }
        let (audio, engine_info) = match audio {
            Ok(engine) => {
                let version = engine.version();
                (
                    Some(engine),
                    EngineInfo {
                        available: true,
                        version,
                        error: None,
                    },
                )
            }
            Err(error) => (
                None,
                EngineInfo {
                    available: false,
                    version: String::new(),
                    error: Some(error),
                },
            ),
        };
        let mut service = Self {
            revision: 0,
            playback_sequence: 0,
            state,
            store,
            audio,
            engine_info,
            status: "stopped".into(),
            position: 0.,
            duration: 0.,
            engine_entries: vec![],
            resume_position: None,
            paused_seek: None,
            eof_handled: false,
            consecutive_errors: 0,
            awaiting_load: false,
            warnings: vec![],
            listening: Default::default(),
            sleep_timer: None,
            sleep_deadline: None,
        };
        service.position = service.active().position;
        service.duration = service
            .active()
            .current_entry()
            .map(|e| e.track.duration)
            .unwrap_or(0.);
        if let Some(engine) = service.audio.as_mut() {
            engine.set_property("volume", &service.state.settings.volume.to_string())?;
            engine.set_property(
                "mute",
                if service.state.settings.muted {
                    "yes"
                } else {
                    "no"
                },
            )?;
            if engine
                .set_property("audio-device", &service.state.settings.output_device)
                .is_err()
            {
                engine.set_property("audio-device", "auto")?;
                service.state.settings.output_device = "auto".into();
            }
            engine.set_property(
                "replaygain",
                if service.state.settings.replay_gain {
                    "track"
                } else {
                    "no"
                },
            )?;
            if service.state.settings.loudness_normalization {
                engine.command(&["af", "add", NORMALIZATION_FILTER])?;
            }
        }
        service.apply_equalizer(&service.state.settings.equalizer.clone())?;
        service.apply_current_gain()?;
        Ok(service)
    }
    pub fn active(&self) -> &Session {
        self.state
            .sessions
            .iter()
            .find(|s| s.id == self.state.active_session_id)
            .expect("active session invariant")
    }
    fn active_mut(&mut self) -> &mut Session {
        let id = self.state.active_session_id.clone();
        self.state
            .sessions
            .iter_mut()
            .find(|s| s.id == id)
            .expect("active session invariant")
    }
    pub fn playback(&self) -> Playback {
        let current = self.active().current_entry();
        Playback {
            sequence: self.playback_sequence,
            status: self.status.clone(),
            position: self.position,
            duration: self.duration,
            volume: self.state.settings.volume,
            muted: self.state.settings.muted,
            current_entry_id: current.map(|e| e.id.clone()),
            track_id: current.map(|e| e.track.id.clone()),
            session_id: Some(self.state.active_session_id.clone()),
        }
    }
    pub fn snapshot(&self) -> AppSnapshot {
        AppSnapshot {
            revision: self.revision,
            tracks: self.state.tracks.clone(),
            library_track_ids: self.state.library_track_ids.clone(),
            library_folders: self.state.library_folders.clone(),
            excluded_folders: self.state.excluded_folders.clone(),
            playlists: self.state.playlists.clone(),
            sessions: self.state.sessions.iter().map(Session::snapshot).collect(),
            active_session_id: self.state.active_session_id.clone(),
            settings: self.state.settings.clone(),
            playback: self.playback(),
            engine: self.engine_info.clone(),
            warnings: self.warnings.clone(),
            settings_undo_available: self.state.settings_import_undo.is_some(),
            favorite_track_ids: self.state.favorite_track_ids.clone(),
            loudness_values: self.state.loudness_values.clone(),
            sleep_timer: self.sleep_timer.clone(),
        }
    }
    /// Native media controls need only the current track, not a copy of the library.
    pub fn media_snapshot(&self) -> AppSnapshot {
        AppSnapshot {
            revision: self.revision,
            tracks: self
                .active()
                .current_entry()
                .map(|e| vec![e.track.clone()])
                .unwrap_or_default(),
            library_track_ids: vec![],
            library_folders: vec![],
            excluded_folders: vec![],
            playlists: vec![],
            sessions: vec![],
            active_session_id: self.state.active_session_id.clone(),
            settings: self.state.settings.clone(),
            playback: self.playback(),
            engine: self.engine_info.clone(),
            warnings: vec![],
            settings_undo_available: self.state.settings_import_undo.is_some(),
            favorite_track_ids: vec![],
            loudness_values: vec![],
            sleep_timer: self.sleep_timer.clone(),
        }
    }
    pub fn persist(&mut self) -> AppResult<()> {
        self.flush_listening()?;
        let position = self.position;
        self.active_mut().position = position;
        let json =
            serde_json::to_string(&self.state).map_err(|e| AppError::new("storage_error", e))?;
        self.store.save(&json)
    }
    fn engine(&mut self) -> AppResult<&mut Box<dyn AudioPort>> {
        self.audio
            .as_mut()
            .ok_or_else(|| AppError::new("engine_unavailable", "libmpv could not be loaded"))
    }
    fn tracks(&self, ids: &[String]) -> Vec<Track> {
        ids.iter()
            .filter_map(|id| self.state.tracks.iter().find(|t| &t.id == id).cloned())
            .collect()
    }
    fn warning(&mut self, text: String) {
        self.warnings.push(text);
        if self.warnings.len() > 12 {
            self.warnings.remove(0);
        }
    }
    pub fn report_warning(&mut self, text: String) {
        self.warning(text);
        self.revision += 1;
    }
    fn stop(&mut self) -> AppResult<()> {
        self.finish_listening();
        if let Some(engine) = self.audio.as_mut() {
            engine.command(&["stop"])?;
        }
        self.engine_entries.clear();
        self.status = "stopped".into();
        self.position = 0.;
        self.active_mut().position = 0.;
        self.resume_position = None;
        self.paused_seek = None;
        self.awaiting_load = false;
        Ok(())
    }
    /// Remove the given sessions. One session always remains: deleting the last
    /// one replaces it with a fresh empty session, and deleting the active one
    /// stops playback and activates the first remaining session.
    fn delete_sessions(&mut self, session_ids: &[String]) -> AppResult<()> {
        let active_deleted = session_ids.contains(&self.state.active_session_id);
        if active_deleted {
            self.stop()?;
        }
        self.state.sessions.retain(|s| !session_ids.contains(&s.id));
        if self.state.sessions.is_empty() {
            self.state.sessions.push(Session::new(""));
        }
        if active_deleted {
            self.state.active_session_id = self.state.sessions[0].id.clone();
            self.position = self.active().position;
            self.duration = self
                .active()
                .current_entry()
                .map(|e| e.track.duration)
                .unwrap_or(0.);
        }
        Ok(())
    }
    /// Rebuild only future engine entries while preserving the currently playing stream.
    fn sync_playlist(&mut self, restart: bool, play: bool) -> AppResult<()> {
        let snapshot = self.active().snapshot();
        let Some(current) = self.active().current_entry().cloned() else {
            return self.stop();
        };
        let index = snapshot
            .entries
            .iter()
            .position(|e| e.id == current.id)
            .unwrap_or(0);
        let future: Vec<QueueEntry> = if self
            .sleep_timer
            .as_ref()
            .is_some_and(|t| t.mode == SleepMode::Track)
        {
            vec![]
        } else {
            snapshot.entries.into_iter().skip(index + 1).collect()
        };
        let repeat_one = matches!(self.active().repeat, RepeatMode::One)
            && !self
                .sleep_timer
                .as_ref()
                .is_some_and(|t| matches!(t.mode, SleepMode::Track | SleepMode::Queue));
        let saved_position = self.position;
        let engine = self.engine()?;
        if restart {
            engine.set_property("pause", "yes")?;
            engine.command(&["loadfile", &current.track.path, "replace"])?;
        } else {
            engine.command(&["playlist-clear"])?;
        }
        for entry in &future {
            engine.command(&["loadfile", &entry.track.path, "append"])?;
        }
        engine.set_property("loop-file", if repeat_one { "inf" } else { "no" })?;
        engine.set_property("pause", if play { "no" } else { "yes" })?;
        self.engine_entries = std::iter::once(current.id)
            .chain(future.into_iter().map(|e| e.id))
            .collect();
        self.duration = current.track.duration;
        self.status = if play { "playing" } else { "paused" }.into();
        self.eof_handled = false;
        if restart {
            self.finish_listening();
            if saved_position <= 0. {
                self.listening.begin_pass();
            }
            self.awaiting_load = true;
            self.paused_seek = None;
            self.position = saved_position;
            self.resume_position = if saved_position > 0. {
                Some(saved_position)
            } else {
                None
            };
        }
        self.apply_current_gain()?;
        Ok(())
    }
    fn start_current(&mut self) -> AppResult<()> {
        self.position = 0.;
        self.active_mut().position = 0.;
        self.consecutive_errors = 0;
        self.sync_playlist(true, true)
    }
    fn play(&mut self) -> AppResult<()> {
        self.engine()?;
        // A boundary sleep timer leaves the current track paused at EOF. A
        // later Play must load it again and restore the full future queue;
        // clearing pause on an already ended mpv stream cannot resume audio.
        if self.eof_handled
            && !self.engine_entries.is_empty()
            && self.active().current_entry().is_some()
        {
            self.position = 0.;
            self.active_mut().position = 0.;
            return self.sync_playlist(true, true);
        }
        if self.active().current_entry().is_none() {
            self.active_mut().next(true);
        }
        if self.active().current_entry().is_none() {
            if let Some(first) = self.active().snapshot().entries.first() {
                let id = first.id.clone();
                self.active_mut().play_entry(&id);
                self.position = 0.;
            }
        }
        if self.active().current_entry().is_none() {
            return Ok(());
        }
        if self.engine_entries.is_empty() || self.status == "stopped" {
            self.sync_playlist(true, true)
        } else {
            self.engine()?.set_property("pause", "no")?;
            if let Some(target) = self.paused_seek.as_mut() {
                target.resumed_at = Some(Instant::now());
            }
            self.status = "playing".into();
            Ok(())
        }
    }
    pub fn dispatch(&mut self, command: PlayerCommand) -> AppResult<()> {
        // A natural transition may have happened since the host's last tick.
        // Reconcile it before changing a playlist or calculating Next/Previous.
        self.tick();
        self.revision += 1;
        self.playback_sequence += 1;
        match command {
            PlayerCommand::CreatePlaylist { name } => return self.create_playlist(name),
            PlayerCommand::SetPlaylistPinned {
                playlist_id,
                pinned,
            } => {
                return self.library_mutation(|state| {
                    state
                        .playlists
                        .iter_mut()
                        .find(|p| p.id == playlist_id)
                        .ok_or_else(|| AppError::new("not_found", "Playlist"))?
                        .pinned = pinned;
                    Ok(())
                })
            }
            PlayerCommand::SetPlaylistIcon { playlist_id, icon } => {
                icon.validate()?;
                return self.library_mutation(|state| {
                    state
                        .playlists
                        .iter_mut()
                        .find(|p| p.id == playlist_id)
                        .ok_or_else(|| AppError::new("not_found", "Playlist"))?
                        .icon = icon;
                    Ok(())
                });
            }
            PlayerCommand::SetPlaylistMembership {
                track_ids,
                playlist_ids,
                preserve_playlist_ids,
            } => {
                return self.set_playlist_membership(track_ids, playlist_ids, preserve_playlist_ids)
            }
            PlayerCommand::SetFavorite {
                track_ids,
                favorite,
            } => return self.set_favorite(track_ids, favorite),
            PlayerCommand::SetLibraryOptions {
                open_external_now_playing,
                suggest_external_folders,
            } => {
                return self.library_mutation(|state| {
                    state.settings.open_external_now_playing = open_external_now_playing;
                    state.settings.suggest_external_folders = suggest_external_folders;
                    Ok(())
                })
            }
            PlayerCommand::SetStatisticsSettings { settings } => {
                settings.validate()?;
                let collection_changed = settings.enabled != self.state.settings.statistics.enabled;
                self.library_mutation(|state| {
                    state.settings.statistics = settings;
                    Ok(())
                })?;
                if collection_changed {
                    self.finish_listening();
                }
                return Ok(());
            }
            PlayerCommand::SetEqualizer { equalizer } => {
                equalizer.validate()?;
                let previous = self.state.settings.equalizer.clone();
                if let Err(error) = self.apply_equalizer(&equalizer) {
                    let _ = self.apply_equalizer(&previous);
                    return Err(error);
                }
                self.state.settings.equalizer = equalizer;
                if let Err(error) = self.persist() {
                    self.state.settings.equalizer = previous.clone();
                    let _ = self.apply_equalizer(&previous);
                    return Err(error);
                }
                return Ok(());
            }
            PlayerCommand::SetGainGrouping { grouping } => {
                let previous = self.state.settings.gain_grouping;
                self.state.settings.gain_grouping = grouping;
                if let Err(error) = self.apply_current_gain().and_then(|_| self.persist()) {
                    self.state.settings.gain_grouping = previous;
                    let _ = self.apply_current_gain();
                    return Err(error);
                }
                return Ok(());
            }
            PlayerCommand::SetGlobalSearch { enabled, binding } => {
                if !valid_shortcut(&binding)
                    || binding.is_empty()
                    || !["Ctrl+", "Alt+"].iter().any(|m| binding.contains(m))
                    || binding == "Ctrl+KeyF"
                {
                    return Err(AppError::new(
                        "invalid_input",
                        "Invalid global search shortcut",
                    ));
                }
                return self.library_mutation(|state| {
                    state.settings.global_search = GlobalSearchSettings { enabled, binding };
                    Ok(())
                });
            }
            PlayerCommand::SetSleepTimer { mode, minutes } => {
                if mode == SleepMode::Minutes
                    && (!minutes.is_finite() || !(1. ..=1440.).contains(&minutes))
                {
                    return Err(AppError::new(
                        "invalid_input",
                        "Timer must be between 1 and 1440 minutes",
                    ));
                }
                self.sleep_timer = Some(SleepTimer {
                    mode,
                    ends_at: (mode == SleepMode::Minutes)
                        .then(|| now() + (minutes * 60.).round() as i64),
                });
                self.sleep_deadline = (mode == SleepMode::Minutes)
                    .then(|| Instant::now() + std::time::Duration::from_secs_f64(minutes * 60.));
                if !self.engine_entries.is_empty() {
                    self.sync_playlist(false, self.status == "playing")?;
                } else {
                    self.apply_sleep_loop()?;
                }
                return Ok(());
            }
            PlayerCommand::CancelSleepTimer => {
                self.sleep_timer = None;
                self.sleep_deadline = None;
                if !self.engine_entries.is_empty() {
                    self.sync_playlist(false, self.status == "playing")?;
                } else {
                    self.apply_sleep_loop()?;
                }
                return Ok(());
            }
            PlayerCommand::SetLoudnessValues { values } => return self.update_loudness(values),
            PlayerCommand::PlayPause => {
                if self.status == "playing" {
                    self.engine()?.set_property("pause", "yes")?;
                    self.status = "paused".into();
                } else {
                    self.play()?;
                }
            }
            PlayerCommand::Play => self.play()?,
            PlayerCommand::Pause => {
                if !self.engine_entries.is_empty() {
                    self.engine()?.set_property("pause", "yes")?;
                    self.status = "paused".into();
                }
            }
            PlayerCommand::Stop => self.stop()?,
            PlayerCommand::Next => {
                self.engine()?;
                if self.active_mut().next(true).is_some() {
                    self.start_current()?;
                } else {
                    self.stop()?;
                }
            }
            PlayerCommand::Previous => {
                self.engine()?;
                if self.position > 3. {
                    self.finish_listening();
                    self.position = 0.;
                    if self.engine_entries.is_empty() {
                        self.start_current()?;
                    } else {
                        self.engine()?.command(&["seek", "0", "absolute+exact"])?;
                        self.paused_seek = (self.status == "paused").then(|| PausedSeek::new(0.));
                    }
                } else if self.active_mut().previous().is_some() {
                    self.start_current()?;
                }
            }
            PlayerCommand::Seek { position } => {
                if !position.is_finite() {
                    return Err(AppError::new("invalid_input", "Invalid seek position"));
                }
                if self.engine_entries.is_empty() {
                    return Ok(());
                }
                let position = position.clamp(0., self.duration.max(0.));
                self.engine()?
                    .command(&["seek", &position.to_string(), "absolute+exact"])?;
                self.position = position;
                self.active_mut().position = position;
                self.paused_seek = (self.status == "paused").then(|| PausedSeek::new(position));
                self.listening.mark_seek();
            }
            PlayerCommand::SetVolume { volume } => {
                if !volume.is_finite() {
                    return Err(AppError::new("invalid_input", "Invalid volume"));
                }
                let volume = volume.clamp(0., 100.);
                if let Some(engine) = self.audio.as_mut() {
                    engine.set_property("volume", &volume.to_string())?;
                }
                self.state.settings.volume = volume;
            }
            PlayerCommand::SetMuted { muted } => {
                if let Some(engine) = self.audio.as_mut() {
                    engine.set_property("mute", if muted { "yes" } else { "no" })?;
                }
                self.state.settings.muted = muted;
            }
            PlayerCommand::SetRepeat { mode } => {
                if let Some(engine) = self.audio.as_mut() {
                    engine.set_property(
                        "loop-file",
                        if matches!(mode, RepeatMode::One)
                            && !self.sleep_timer.as_ref().is_some_and(|t| {
                                matches!(t.mode, SleepMode::Track | SleepMode::Queue)
                            })
                        {
                            "inf"
                        } else {
                            "no"
                        },
                    )?;
                }
                self.active_mut().repeat = mode;
            }
            PlayerCommand::SetOrder { mode } => {
                self.active_mut().set_order(mode);
                if !self.engine_entries.is_empty() {
                    self.sync_playlist(false, self.status == "playing")?;
                }
            }
            PlayerCommand::SetShuffle { enabled } => {
                self.active_mut().set_shuffle(enabled);
                if !self.engine_entries.is_empty() {
                    self.sync_playlist(false, self.status == "playing")?;
                }
            }
            PlayerCommand::PlayEntry { entry_id } => {
                self.engine()?;
                if !self.active_mut().play_entry(&entry_id) {
                    return Err(AppError::new("not_found", "Queue entry"));
                }
                self.start_current()?;
            }
            PlayerCommand::RemoveEntry { entry_id } => {
                let previous = self.active().current_entry().map(|e| e.id.clone());
                self.active_mut().remove(&entry_id);
                let changed = previous != self.active().current_entry().map(|e| e.id.clone());
                if changed {
                    self.position = 0.;
                    self.duration = self
                        .active()
                        .current_entry()
                        .map(|e| e.track.duration)
                        .unwrap_or(0.);
                    self.resume_position = None;
                    self.paused_seek = None;
                }
                if !self.engine_entries.is_empty() {
                    self.sync_playlist(changed, self.status == "playing")?;
                }
            }
            PlayerCommand::MoveEntry {
                entry_id,
                before_entry_id,
            } => {
                self.active_mut()
                    .move_entry(&entry_id, before_entry_id.as_deref());
                if !self.engine_entries.is_empty() {
                    self.sync_playlist(false, self.status == "playing")?;
                }
            }
            PlayerCommand::ClearQueue => {
                self.stop()?;
                self.active_mut().clear();
                self.duration = 0.;
            }
            PlayerCommand::PlayTracks {
                track_ids,
                order_mode,
            } => {
                self.engine()?;
                let tracks = self.tracks(&track_ids);
                if tracks.is_empty() {
                    return Err(AppError::new("empty_selection", "No tracks"));
                }
                if let Some(mode) = order_mode {
                    self.active_mut().set_order(mode);
                }
                self.active_mut().replace(tracks);
                self.active_mut().next(true);
                self.start_current()?;
            }
            PlayerCommand::AppendTracks { track_ids } => {
                let tracks = self.tracks(&track_ids);
                self.active_mut().append(tracks);
                if !self.engine_entries.is_empty() {
                    self.sync_playlist(false, self.status == "playing")?;
                }
            }
            PlayerCommand::EnqueueNextTracks { track_ids } => {
                let tracks = self.tracks(&track_ids);
                self.active_mut().enqueue_next(tracks);
                if !self.engine_entries.is_empty() {
                    self.sync_playlist(false, self.status == "playing")?;
                }
            }
            PlayerCommand::NewSession { name } => {
                self.persist()?;
                if let Some(engine) = self.audio.as_mut() {
                    engine.command(&["stop"])?;
                }
                self.engine_entries.clear();
                self.status = "stopped".into();
                self.position = 0.;
                self.resume_position = None;
                self.paused_seek = None;
                self.awaiting_load = false;
                let session = Session::new(name.trim());
                self.state.active_session_id = session.id.clone();
                self.state.sessions.push(session);
                self.duration = 0.;
            }
            PlayerCommand::ActivateSession { session_id } => {
                if !self.state.sessions.iter().any(|s| s.id == session_id) {
                    return Err(AppError::new("not_found", "Session"));
                }
                self.persist()?; // save the old session before stop resets transient state
                let old_position = self.position;
                if let Some(engine) = self.audio.as_mut() {
                    engine.command(&["stop"])?;
                }
                self.active_mut().position = old_position;
                self.engine_entries.clear();
                self.status = "stopped".into();
                self.resume_position = None;
                self.paused_seek = None;
                self.awaiting_load = false;
                self.state.active_session_id = session_id;
                self.position = self.active().position;
                self.duration = self
                    .active()
                    .current_entry()
                    .map(|e| e.track.duration)
                    .unwrap_or(0.);
            }
            PlayerCommand::RenameSession { session_id, name } => {
                let name = valid_name(name)?;
                let session = self
                    .state
                    .sessions
                    .iter_mut()
                    .find(|s| s.id == session_id)
                    .ok_or_else(|| AppError::new("not_found", "Session"))?;
                session.name = name;
            }
            PlayerCommand::DeleteSession { session_id } => self.delete_sessions(&[session_id])?,
            PlayerCommand::DeleteSessions { session_ids } => self.delete_sessions(&session_ids)?,
            PlayerCommand::SavePlaylist { name, playlist_id } => {
                let name = valid_name(name)?;
                let track_ids = self
                    .active()
                    .snapshot()
                    .entries
                    .iter()
                    .map(|e| e.track.id.clone())
                    .collect();
                if let Some(id) = playlist_id {
                    let playlist = self
                        .state
                        .playlists
                        .iter_mut()
                        .find(|p| p.id == id)
                        .ok_or_else(|| AppError::new("not_found", "Playlist"))?;
                    playlist.name = name;
                    playlist.track_ids = track_ids;
                } else {
                    self.state.playlists.push(Playlist {
                        id: Uuid::new_v4().to_string(),
                        name,
                        track_ids,
                        created_at: now(),
                        pinned: false,
                        icon: PlaylistIcon::default(),
                    });
                }
            }
            PlayerCommand::DeletePlaylist { playlist_id } => {
                self.state.playlists.retain(|p| p.id != playlist_id)
            }
            PlayerCommand::PlayPlaylist { playlist_id } => {
                let playlist = self
                    .state
                    .playlists
                    .iter()
                    .find(|p| p.id == playlist_id)
                    .ok_or_else(|| AppError::new("not_found", "Playlist"))?
                    .clone();
                self.engine()?;
                self.persist()?;
                let mut session = Session::new(playlist.name);
                session.replace(self.tracks(&playlist.track_ids));
                session.next(true);
                self.state.active_session_id = session.id.clone();
                self.state.sessions.push(session);
                self.start_current()?;
            }
            PlayerCommand::RenamePlaylist { playlist_id, name } => {
                let name = valid_name(name)?;
                self.state
                    .playlists
                    .iter_mut()
                    .find(|p| p.id == playlist_id)
                    .ok_or_else(|| AppError::new("not_found", "Playlist"))?
                    .name = name;
            }
            PlayerCommand::AddToPlaylist {
                playlist_id,
                track_ids,
            } => {
                let tracks = self.tracks(&track_ids);
                self.state
                    .playlists
                    .iter_mut()
                    .find(|p| p.id == playlist_id)
                    .ok_or_else(|| AppError::new("not_found", "Playlist"))?
                    .track_ids
                    .extend(tracks.into_iter().map(|t| t.id));
            }
            PlayerCommand::RemoveFromPlaylist { playlist_id, index } => {
                let playlist = self
                    .state
                    .playlists
                    .iter_mut()
                    .find(|p| p.id == playlist_id)
                    .ok_or_else(|| AppError::new("not_found", "Playlist"))?;
                if index < playlist.track_ids.len() {
                    playlist.track_ids.remove(index);
                }
            }
            PlayerCommand::SetLanguage { language } => {
                if !["de", "en"].contains(&language.as_str()) {
                    return Err(AppError::new("invalid_input", "Unsupported language"));
                }
                self.state.settings.language = language;
            }
            PlayerCommand::RemoveLibraryFolder { path } => self.remove_library_folder(&path),
            PlayerCommand::SetAudioDevice { id } => {
                self.engine()?.set_property("audio-device", &id)?;
                self.state.settings.output_device = id;
            }
            PlayerCommand::SetReplayGain { enabled } => {
                let previous = self.state.settings.replay_gain;
                self.state.settings.replay_gain = enabled;
                if let Err(error) = self.apply_current_gain().and_then(|_| self.persist()) {
                    self.state.settings.replay_gain = previous;
                    let _ = self.apply_current_gain();
                    return Err(error);
                }
                return Ok(());
            }
            PlayerCommand::SetLoudnessNormalization { enabled } => {
                self.engine()?.command(if enabled {
                    &["af", "add", NORMALIZATION_FILTER]
                } else {
                    &["af", "remove", NORMALIZATION_LABEL]
                })?;
                self.state.settings.loudness_normalization = enabled;
            }
            PlayerCommand::SetCloseToTray { enabled } => {
                self.state.settings.close_to_tray = enabled
            }
            PlayerCommand::SetDefaultAppReminder { enabled } => {
                let previous =
                    std::mem::replace(&mut self.state.settings.default_app_reminder, enabled);
                if let Err(error) = self.persist() {
                    self.state.settings.default_app_reminder = previous;
                    return Err(error);
                }
                return Ok(());
            }
            PlayerCommand::AcknowledgeDefaultAppHint => {
                self.state.settings.default_app_hint_seen = true;
            }
            PlayerCommand::SetAppearance { appearance } => {
                let appearance = settings::validate_appearance(appearance)?;
                let previous = std::mem::replace(&mut self.state.settings.appearance, appearance);
                if let Err(error) = self.persist() {
                    self.state.settings.appearance = previous;
                    return Err(error);
                }
                return Ok(());
            }
            PlayerCommand::ImportSettings { document, sections } => {
                return self.import_settings(document, sections);
            }
            PlayerCommand::UndoSettingsImport => {
                return self.undo_settings_import();
            }
            PlayerCommand::SetShortcuts { shortcuts } => {
                self.state.settings.shortcuts = validate_shortcuts(shortcuts)?;
            }
            PlayerCommand::SetSeekbarStyle { style } => {
                self.state.settings.seekbar_style = style;
            }
            PlayerCommand::SetPanelWidths { sidebar, queue } => {
                self.state.settings.panel_widths = PanelWidths::bounded(sidebar, queue)?;
            }
        }
        self.persist()
    }
    /// Removes a folder from the library without touching the files. Library
    /// folders inside it are dropped; if it lies inside a remaining library
    /// folder it is remembered as excluded so rescans do not add it back.
    /// Tracks outside the folder, including individually added files, stay.
    fn remove_library_folder(&mut self, path: &str) {
        self.state
            .library_folders
            .retain(|root| !path_within(root, path));
        self.state
            .excluded_folders
            .retain(|excluded| !path_within(excluded, path));
        if self
            .state
            .library_folders
            .iter()
            .any(|root| path_within(path, root))
        {
            self.state.excluded_folders.push(path.to_owned());
        }
        let removed: HashSet<String> = self
            .state
            .tracks
            .iter()
            .filter(|track| path_within(&track.path, path))
            .map(|track| track.id.clone())
            .collect();
        self.state
            .library_track_ids
            .retain(|id| !removed.contains(id));
    }
    pub fn ingest(
        &mut self,
        mut tracks: Vec<Track>,
        folders: Vec<String>,
        mode: &str,
        warnings: Vec<String>,
    ) -> AppResult<()> {
        self.tick();
        self.revision += 1;
        self.playback_sequence += 1;
        if !["play", "append", "next", "library", "rescan"].contains(&mode) {
            return Err(AppError::new("invalid_input", "Import mode"));
        }
        let mut folders = folders;
        let mode = if mode == "rescan" {
            // A background rescan reads the folder list before scanning. Only
            // refresh folders that are still in the library, so a folder the
            // user removed meanwhile does not come back.
            folders.retain(|folder| {
                self.state
                    .library_folders
                    .iter()
                    .any(|root| path_within(folder, root) && path_within(root, folder))
            });
            if folders.is_empty() {
                return Ok(());
            }
            tracks.retain(|track| {
                folders
                    .iter()
                    .any(|folder| path_within(&track.path, folder))
            });
            "library"
        } else {
            mode
        };
        for warning in warnings {
            self.warning(warning);
        }
        if tracks.is_empty() && !(mode == "library" && !folders.is_empty()) {
            return Err(AppError::new(
                "no_audio_files",
                "No supported audio files found",
            ));
        }
        self.relink_scanned_tracks(&mut tracks);
        for track in &tracks {
            if let Some(old) = self.state.tracks.iter_mut().find(|t| t.id == track.id) {
                let added = old.added_at;
                *old = track.clone();
                old.added_at = added;
            } else {
                self.state.tracks.push(track.clone());
            }
        }
        match mode {
            "library" => {
                // Adding a folder that is not yet in the library lifts earlier
                // removals of its subfolders; rescans of existing folders keep them.
                let added: Vec<&String> = folders
                    .iter()
                    .filter(|folder| !self.state.library_folders.contains(folder))
                    .collect();
                self.state
                    .excluded_folders
                    .retain(|excluded| !added.iter().any(|folder| path_within(excluded, folder)));
                let excluded = self.state.excluded_folders.clone();
                let is_excluded =
                    |path: &str| excluded.iter().any(|folder| path_within(path, folder));
                let ids: Vec<String> = tracks
                    .iter()
                    .filter(|track| !is_excluded(&track.path))
                    .map(|track| track.id.clone())
                    .collect();
                let scanned_ids: HashSet<String> = ids.iter().cloned().collect();
                let retained: HashSet<String> = self
                    .state
                    .tracks
                    .iter()
                    .filter(|track| {
                        !folders
                            .iter()
                            .any(|folder| path_within(&track.path, folder))
                            || scanned_ids.contains(&track.id)
                    })
                    .map(|track| track.id.clone())
                    .collect();
                self.state
                    .library_track_ids
                    .retain(|id| retained.contains(id));
                for id in ids {
                    if !self.state.library_track_ids.contains(&id) {
                        self.state.library_track_ids.push(id);
                    }
                }
                for folder in folders {
                    if !self.state.library_folders.contains(&folder) {
                        self.state.library_folders.push(folder);
                    }
                }
                if !self.engine_entries.is_empty() {
                    self.sync_playlist(false, self.status == "playing")?;
                }
            }
            "play" => {
                self.active_mut().replace(tracks);
                self.active_mut().next(true);
                self.position = 0.;
                self.persist()?;
                self.start_current()?;
            }
            "next" => {
                self.active_mut().enqueue_next(tracks);
                if !self.engine_entries.is_empty() {
                    self.sync_playlist(false, self.status == "playing")?;
                }
            }
            _ => {
                self.active_mut().append(tracks);
                if !self.engine_entries.is_empty() {
                    self.sync_playlist(false, self.status == "playing")?;
                }
            }
        }
        self.persist()
    }
    pub fn import_playlist(
        &mut self,
        name: String,
        mut tracks: Vec<Track>,
        warnings: Vec<String>,
    ) -> AppResult<()> {
        self.revision += 1;
        self.playback_sequence += 1;
        self.relink_scanned_tracks(&mut tracks);
        let ids: Vec<String> = tracks.iter().map(|t| t.id.clone()).collect();
        for warning in warnings {
            self.warning(warning);
        }
        for track in tracks {
            if let Some(old) = self.state.tracks.iter_mut().find(|t| t.id == track.id) {
                *old = track;
            } else {
                self.state.tracks.push(track);
            }
        }
        if ids.is_empty() {
            return Err(AppError::new(
                "no_audio_files",
                "Playlist contains no readable audio files",
            ));
        }
        self.state.playlists.push(Playlist {
            id: Uuid::new_v4().to_string(),
            name,
            track_ids: ids,
            created_at: now(),
            pinned: false,
            icon: PlaylistIcon::default(),
        });
        self.persist()
    }
    pub fn export_tracks(&self, playlist_id: Option<&str>) -> AppResult<Vec<Track>> {
        if let Some(id) = playlist_id {
            let playlist = self
                .state
                .playlists
                .iter()
                .find(|p| p.id == id)
                .ok_or_else(|| AppError::new("not_found", "Playlist"))?;
            Ok(self.tracks(&playlist.track_ids))
        } else {
            Ok(self
                .active()
                .snapshot()
                .entries
                .into_iter()
                .map(|e| e.track)
                .collect())
        }
    }
    pub fn audio_devices(&mut self) -> Vec<AudioDevice> {
        self.audio.as_mut().map(|e| e.devices()).unwrap_or_default()
    }
    /// Polling runs on a dedicated host worker. Returns true only for structural changes.
    pub fn tick(&mut self) -> bool {
        self.playback_sequence += 1;
        if self.expire_sleep_timer() {
            return true;
        }
        if self.audio.is_none() || self.engine_entries.is_empty() {
            return false;
        }
        let mut changed = false;
        let mut had_error = false;
        for _ in 0..32 {
            let Some(event) = self.audio.as_mut().and_then(|e| e.poll_event()) else {
                break;
            };
            match event {
                AudioEvent::FileLoaded => self.awaiting_load = false,
                AudioEvent::Ended { reason, error } if reason == "error" => {
                    self.consecutive_errors += 1;
                    had_error = true;
                    self.awaiting_load = false;
                    self.warning(error.unwrap_or_else(|| "Audio file could not be played".into()));
                    changed = true;
                }
                _ => {}
            }
        }
        // loadfile is asynchronous; all queried properties may still describe
        // the previous stream until FILE_LOADED (or an error) has arrived.
        if self.awaiting_load {
            return changed;
        }
        let index = self
            .audio
            .as_mut()
            .and_then(|e| e.get_property("playlist-pos"))
            .and_then(|v| v.parse::<usize>().ok());
        if let Some(id) = index.and_then(|i| self.engine_entries.get(i).cloned()) {
            if self.active().current_entry().map(|e| e.id.as_str()) != Some(id.as_str()) {
                if had_error {
                    self.finish_listening();
                } else {
                    self.finish_listening_naturally();
                }
                self.active_mut().play_entry(&id);
                self.listening.begin_pass();
                self.position = 0.;
                self.paused_seek = None;
                self.eof_handled = false;
                changed = true;
                if let Err(error) = self.apply_current_gain() {
                    self.warning(error.to_string());
                }
            }
        }
        let engine = self.audio.as_mut().expect("audio checked");
        let paused = engine.get_property("pause").as_deref() == Some("yes");
        if let Some(duration) = engine
            .get_property("duration")
            .and_then(|v| v.parse::<f64>().ok())
            .filter(|v| v.is_finite() && *v >= 0.)
        {
            self.duration = duration;
        }
        if !self.awaiting_load {
            if let Some(position) = self.resume_position.take() {
                if self.duration > 0. {
                    let position = position.min((self.duration - 0.05).max(0.));
                    self.listening.mark_seek();
                    if engine
                        .command(&["seek", &position.to_string(), "absolute+exact"])
                        .is_err()
                    {
                        self.resume_position = Some(position);
                    } else if paused {
                        self.paused_seek = Some(PausedSeek::new(position));
                    }
                } else {
                    self.resume_position = Some(position);
                }
            }
        }
        if let Some(position) = engine
            .get_property("time-pos")
            .and_then(|v| v.parse::<f64>().ok())
            .filter(|v| v.is_finite() && *v >= 0.)
        {
            // With paused audio, mpv can acknowledge an exact seek and then
            // report the old audio clock until playback resumes. Keep the
            // requested position until the running clock reaches its target.
            if let Some(target) = self.paused_seek.as_mut() {
                let elapsed = if paused {
                    target.resumed_at = None;
                    0.
                } else {
                    target
                        .resumed_at
                        .get_or_insert_with(Instant::now)
                        .elapsed()
                        .as_secs_f64()
                };
                if !paused
                    && position >= target.position - 0.03
                    && position <= target.position + elapsed + 0.5
                {
                    self.paused_seek = None;
                    self.position = position;
                } else {
                    self.position = target.position;
                }
            } else {
                self.position = position;
            }
        }
        let eof = engine.get_property("eof-reached").as_deref() == Some("yes");
        let idle = engine.get_property("idle-active").as_deref() == Some("yes");
        if !had_error && !eof && !idle && self.position > 0.1 {
            self.consecutive_errors = 0;
        }
        if !eof && !idle {
            self.status = if paused { "paused" } else { "playing" }.into();
        }
        self.observe_listening();
        if (eof || idle) && !self.awaiting_load && !self.eof_handled && self.status != "stopped" {
            self.eof_handled = true;
            if had_error {
                self.finish_listening();
            } else {
                self.finish_listening_naturally();
            }
            let sleep_boundary = self.sleep_timer.as_ref().is_some_and(|t| {
                t.mode == SleepMode::Track
                    || (t.mode == SleepMode::Queue && !self.active().has_upcoming())
            });
            if sleep_boundary {
                self.finish_listening();
                self.finish_sleep_timer();
                let position = self.position;
                self.active_mut().position = position;
                return true;
            }
            // Bound recovery when every file is broken, even with repeat-all enabled.
            let all_failed = self.consecutive_errors >= self.active().snapshot().entries.len();
            let bypass_repeat = had_error
                || self
                    .sleep_timer
                    .as_ref()
                    .is_some_and(|t| t.mode == SleepMode::Queue);
            if !all_failed && self.active_mut().next(bypass_repeat).is_some() {
                self.position = 0.;
                if let Err(error) = self.sync_playlist(true, true) {
                    self.warning(error.to_string());
                    let _ = self.stop();
                }
            } else {
                let _ = self.stop();
            }
            changed = true;
        }
        let position = self.position;
        self.active_mut().position = position;
        if changed {
            self.revision += 1;
        }
        changed
    }
}
fn valid_name(name: String) -> AppResult<String> {
    let name = name.trim();
    if name.is_empty() || name.chars().count() > 160 {
        Err(AppError::new(
            "invalid_name",
            "Name must contain 1–160 characters",
        ))
    } else {
        Ok(name.into())
    }
}
fn path_within(path: &str, folder: &str) -> bool {
    #[cfg(windows)]
    {
        std::path::Path::new(&path.to_lowercase())
            .starts_with(std::path::Path::new(&folder.to_lowercase()))
    }
    #[cfg(not(windows))]
    {
        std::path::Path::new(path).starts_with(std::path::Path::new(folder))
    }
}
fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}
