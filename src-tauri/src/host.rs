use player_application::{
    AppError, AppResult, AppSnapshot, AudioDevice, AudioEvent, AudioPort, PlayerCommand,
    PlayerService, PlaylistIcon, StateStore, Statistics,
};
use player_infrastructure::{
    read_m3u8, scan_paths, write_m3u8, AudioTools, EngineEvent, InfraError, MpvEngine, Store,
};
use std::{
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::Duration,
};
use tauri::{AppHandle, Emitter, Manager, State};

#[path = "audio_jobs.rs"]
mod audio_jobs;
#[path = "config_file.rs"]
mod config_file;
#[path = "desktop.rs"]
mod desktop;
#[path = "global_search.rs"]
mod global_search;

struct NativeAudio(MpvEngine);
fn audio_error(error: InfraError) -> AppError {
    AppError::new("audio_error", error)
}
impl AudioPort for NativeAudio {
    fn command(&mut self, args: &[&str]) -> AppResult<()> {
        self.0.command(args).map_err(audio_error)
    }
    fn set_property(&mut self, name: &str, value: &str) -> AppResult<()> {
        self.0.set_property(name, value).map_err(audio_error)
    }
    fn get_property(&mut self, name: &str) -> Option<String> {
        self.0.get_property(name)
    }
    fn poll_event(&mut self) -> Option<AudioEvent> {
        self.0.poll_event().map(|e| match e {
            EngineEvent::TrackChanged { .. } => AudioEvent::FileLoaded,
            EngineEvent::Ended { reason, error } => AudioEvent::Ended { reason, error },
        })
    }
    fn version(&self) -> String {
        self.0.version()
    }
    fn devices(&mut self) -> Vec<AudioDevice> {
        self.0
            .list_devices()
            .into_iter()
            .map(|d| AudioDevice {
                id: d.id,
                name: d.name,
            })
            .collect()
    }
}
struct SqliteState(Store);
impl StateStore for SqliteState {
    fn load(&mut self) -> AppResult<Option<String>> {
        self.0
            .load_json()
            .map_err(|e| AppError::new("storage_error", e))
    }
    fn save(&mut self, json: &str) -> AppResult<()> {
        self.0
            .save_json(json)
            .map_err(|e| AppError::new("storage_error", e))
    }
    fn record_listening(&mut self, event: &player_domain::ListeningEvent) -> AppResult<()> {
        self.0
            .record_listening(event)
            .map_err(|e| AppError::new("storage_error", e))
    }
    fn listening_history(
        &mut self,
        from: Option<i64>,
        to: Option<i64>,
    ) -> AppResult<Vec<player_domain::ListeningEvent>> {
        self.0
            .listening_history(from, to)
            .map_err(|e| AppError::new("storage_error", e))
    }
    fn clear_listening(&mut self) -> AppResult<()> {
        self.0
            .clear_listening()
            .map_err(|e| AppError::new("storage_error", e))
    }
    fn listening_years(&mut self) -> AppResult<Vec<i32>> {
        self.0
            .listening_years()
            .map_err(|e| AppError::new("storage_error", e))
    }
}
struct HostState {
    player: Mutex<PlayerService>,
    cover_dir: PathBuf,
    scanning: Mutex<()>,
    icon_dir: PathBuf,
    preview_dir: PathBuf,
    audio_tools: Result<AudioTools, String>,
    audio_jobs: audio_jobs::AudioJobs,
    external_launch: Mutex<Option<ExternalLaunch>>,
    commands: Mutex<()>,
}
#[derive(Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct ExternalLaunch {
    id: String,
    paths: Vec<String>,
    folders: Vec<String>,
}
fn lock_player(state: &HostState) -> AppResult<std::sync::MutexGuard<'_, PlayerService>> {
    state
        .player
        .lock()
        .map_err(|_| AppError::new("internal_error", "Playback coordinator lock poisoned"))
}
#[tauri::command]
fn get_snapshot(state: State<'_, HostState>) -> AppResult<AppSnapshot> {
    Ok(lock_player(&state)?.snapshot())
}

#[tauri::command]
fn take_external_launch(state: State<'_, HostState>) -> AppResult<Option<ExternalLaunch>> {
    Ok(state
        .external_launch
        .lock()
        .map_err(|e| AppError::new("internal_error", e))?
        .take())
}
#[tauri::command]
fn get_statistics(
    state: State<'_, HostState>,
    from: Option<i64>,
    to: Option<i64>,
) -> AppResult<Statistics> {
    if from.is_some_and(|v| v < 0)
        || to.is_some_and(|v| v < 0)
        || from.zip(to).is_some_and(|(a, b)| a >= b)
    {
        return Err(AppError::new(
            "invalid_input",
            "Invalid statistics date range",
        ));
    }
    lock_player(&state)?.statistics(from, to)
}
#[tauri::command]
fn get_statistics_years(state: State<'_, HostState>) -> AppResult<Vec<i32>> {
    lock_player(&state)?.statistics_years()
}
#[tauri::command]
fn clear_statistics(state: State<'_, HostState>) -> AppResult<()> {
    lock_player(&state)?.clear_statistics()
}
#[tauri::command]
fn export_history(state: State<'_, HostState>) -> AppResult<serde_json::Value> {
    lock_player(&state)?.export_history()
}

#[tauri::command]
async fn export_history_file(app: AppHandle, path: String) -> AppResult<()> {
    tauri::async_runtime::spawn_blocking(move || {
        let document = lock_player(&app.state::<HostState>())?.export_history()?;
        config_file::write_json_atomic(Path::new(&path), &document)
            .map_err(|error| AppError::new("export_error", error))
    })
    .await
    .map_err(|error| AppError::new("internal_error", error))?
}

#[tauri::command]
async fn configure_global_search(app: AppHandle, enabled: bool, binding: String) -> AppResult<()> {
    tauri::async_runtime::spawn_blocking(move || {
        execute(&app, PlayerCommand::SetGlobalSearch { enabled, binding }).map(|_| ())
    })
    .await
    .map_err(|e| AppError::new("internal_error", e))?
}

fn image_extension(bytes: &[u8]) -> AppResult<&'static str> {
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        Ok("png")
    } else if bytes.starts_with(b"\xff\xd8\xff") {
        Ok("jpg")
    } else if bytes.len() >= 12 && &bytes[..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        Ok("webp")
    } else {
        Err(AppError::new(
            "invalid_input",
            "Choose a valid PNG, JPEG or WebP image",
        ))
    }
}
#[tauri::command]
async fn import_playlist_icon(app: AppHandle, path: String) -> AppResult<PlaylistIcon> {
    tauri::async_runtime::spawn_blocking(move || {
        use std::io::Read;
        let state = app.state::<HostState>();
        let mut bytes = vec![];
        std::fs::File::open(&path)
            .map_err(|e| AppError::new("import_error", e))?
            .take(8 * 1024 * 1024 + 1)
            .read_to_end(&mut bytes)
            .map_err(|e| AppError::new("import_error", e))?;
        if bytes.is_empty() || bytes.len() > 8 * 1024 * 1024 {
            return Err(AppError::new(
                "invalid_input",
                "Playlist images must be smaller than 8 MiB",
            ));
        }
        let extension = image_extension(&bytes)?;
        let tool = state
            .audio_tools
            .as_ref()
            .map_err(|e| AppError::new("audio_tools_missing", e))?;
        // Decode the immutable copied bytes, so changing the selected source
        // between validation and copy cannot introduce unvalidated content.
        let fingerprint = player_infrastructure::content_sha256(&bytes);
        let destination = state.icon_dir.join(format!("{fingerprint}.{extension}"));
        if !destination.exists() {
            config_file::write_atomic(&destination, &bytes)
                .map_err(|e| AppError::new("import_error", e))?;
        }
        if let Err(error) = tool.validate_image(&destination) {
            let _ = std::fs::remove_file(&destination);
            return Err(AppError::new("invalid_input", error));
        }
        Ok(PlaylistIcon {
            kind: "image".into(),
            value: destination.to_string_lossy().into_owned(),
        })
    })
    .await
    .map_err(|e| AppError::new("internal_error", e))?
}

#[tauri::command]
async fn get_default_app_status() -> AppResult<desktop::DefaultAppStatus> {
    tauri::async_runtime::spawn_blocking(desktop::default_app_status)
        .await
        .map_err(|error| AppError::new("internal_error", error))?
}

#[tauri::command]
fn open_default_apps_settings() -> AppResult<()> {
    desktop::open_default_apps_settings()
}

#[tauri::command]
fn quit_app(app: AppHandle) {
    app.exit(0);
}

pub(crate) fn snapshot_for_media(app: &AppHandle) -> AppResult<AppSnapshot> {
    Ok(lock_player(&app.state::<HostState>())?.media_snapshot())
}
pub(crate) fn execute(app: &AppHandle, command: PlayerCommand) -> AppResult<AppSnapshot> {
    let state = app.state::<HostState>();
    let _commands = state
        .commands
        .lock()
        .map_err(|e| AppError::new("internal_error", e))?;
    let (previous_global, next_global) = {
        let player = lock_player(&state)?;
        let previous = player.state.settings.global_search.clone();
        let next = match &command {
            PlayerCommand::SetGlobalSearch { enabled, binding } => {
                Some(player_application::GlobalSearchSettings {
                    enabled: *enabled,
                    binding: binding.clone(),
                })
            }
            PlayerCommand::ImportSettings { document, sections }
                if sections.iter().any(|s| s == "device") =>
            {
                document
                    .get("sections")
                    .and_then(|v| v.get("device"))
                    .and_then(|v| v.get("globalSearch"))
                    .map(|v| {
                        serde_json::from_value::<player_application::GlobalSearchSettings>(
                            v.clone(),
                        )
                    })
                    .transpose()
                    .map_err(|e| AppError::new("invalid_input", e))?
            }
            PlayerCommand::UndoSettingsImport => player
                .state
                .settings_import_undo
                .as_ref()
                .map(|u| u.settings.global_search.clone()),
            _ => None,
        };
        (previous, next)
    };
    let global_changed = next_global
        .as_ref()
        .is_some_and(|next| next != &previous_global);
    if global_changed {
        let next = next_global.as_ref().unwrap();
        app.state::<global_search::GlobalSearch>()
            .configure(next.enabled, next.binding.clone())?;
    }
    let language_change = matches!(
        &command,
        PlayerCommand::SetLanguage { .. }
            | PlayerCommand::ImportSettings { .. }
            | PlayerCommand::UndoSettingsImport
    );
    let result;
    let snapshot;
    {
        let state = app.state::<HostState>();
        let mut player = lock_player(&state)?;
        result = player.dispatch(command);
        snapshot = player.snapshot();
    }
    let _ = app.emit("player://state", &snapshot);
    if language_change {
        let _ = rebuild_menu(app, &snapshot.settings.language);
    }
    if result.is_err() && global_changed {
        let _ = app
            .state::<global_search::GlobalSearch>()
            .configure(previous_global.enabled, previous_global.binding);
    }
    result.map(|_| snapshot)
}
#[tauri::command]
async fn dispatch(app: AppHandle, command: PlayerCommand) -> AppResult<AppSnapshot> {
    tauri::async_runtime::spawn_blocking(move || execute(&app, command))
        .await
        .map_err(|e| AppError::new("internal_error", e))?
}

fn expand_playlists(paths: Vec<String>) -> AppResult<Vec<String>> {
    let mut result = vec![];
    for path in paths {
        let ext = Path::new(&path)
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_lowercase();
        if ext == "m3u" || ext == "m3u8" {
            result
                .extend(read_m3u8(Path::new(&path)).map_err(|e| AppError::new("import_error", e))?);
        } else {
            result.push(path);
        }
    }
    Ok(result)
}
fn canonical_key(path: &str) -> String {
    let normalized = std::fs::canonicalize(path)
        .unwrap_or_else(|_| PathBuf::from(path))
        .to_string_lossy()
        .into_owned();
    #[cfg(windows)]
    {
        if let Some(unc) = normalized.strip_prefix(r"\\?\UNC\") {
            format!(r"\\{unc}").to_lowercase()
        } else {
            normalized
                .strip_prefix(r"\\?\")
                .unwrap_or(&normalized)
                .to_lowercase()
        }
    }
    #[cfg(not(windows))]
    {
        normalized
    }
}
fn scan_requested(
    paths: &[String],
    recursive: bool,
    cover_dir: &Path,
) -> AppResult<player_infrastructure::ScanResult> {
    let mut scan =
        scan_paths(paths, recursive, cover_dir).map_err(|e| AppError::new("import_error", e))?;
    if paths.iter().all(|p| !Path::new(p).is_dir()) {
        // The scanner deduplicates for indexing; a playlist's intentional repeats
        // and explicit file order are restored after metadata extraction.
        let tracks: std::collections::HashMap<_, _> = scan
            .tracks
            .into_iter()
            .map(|t| (canonical_key(&t.path), t))
            .collect();
        scan.tracks = paths
            .iter()
            .filter_map(|p| tracks.get(&canonical_key(p)).cloned())
            .collect();
    }
    Ok(scan)
}
fn import_files(
    app: &AppHandle,
    paths: Vec<String>,
    mode: &str,
    recursive: bool,
    external: bool,
) -> AppResult<AppSnapshot> {
    let state = app.state::<HostState>();
    let _scan_lock = state
        .scanning
        .lock()
        .map_err(|_| AppError::new("internal_error", "Scan lock poisoned"))?;
    let explicit_files: Vec<String> = if external {
        paths
            .iter()
            .filter(|p| {
                Path::new(p).is_file() && player_infrastructure::is_audio_path(Path::new(p))
            })
            .cloned()
            .collect()
    } else {
        vec![]
    };
    let paths = expand_playlists(paths)?;
    let scan = scan_requested(&paths, recursive, &state.cover_dir)?;
    let mut player = lock_player(&state)?;
    if external && !player.active().snapshot().entries.is_empty() {
        let name = paths
            .first()
            .and_then(|p| Path::new(p).file_stem())
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_default();
        player.dispatch(PlayerCommand::NewSession { name })?;
    }
    let result = player.ingest(scan.tracks, scan.folders, mode, scan.warnings);
    let snapshot = player.snapshot();
    let external_launch = if result.is_ok() && !explicit_files.is_empty() {
        let mut folders = vec![];
        for file in &explicit_files {
            if let Some(parent) = Path::new(file).parent() {
                let key = canonical_key(&parent.to_string_lossy());
                let inside = |roots: &[String]| {
                    roots
                        .iter()
                        .any(|root| Path::new(&key).starts_with(canonical_key(root)))
                };
                if !(inside(&snapshot.library_folders) && !inside(&snapshot.excluded_folders))
                    && !folders
                        .iter()
                        .any(|folder: &String| canonical_key(folder) == key)
                {
                    // Use the path selected by the shell for readable display; all
                    // containment comparisons above use canonical component paths.
                    folders.push(parent.to_string_lossy().into_owned());
                }
            }
        }
        static NEXT_LAUNCH: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        Some(ExternalLaunch {
            id: format!(
                "{}-{}",
                std::process::id(),
                NEXT_LAUNCH.fetch_add(1, Ordering::Relaxed)
            ),
            paths: explicit_files,
            folders,
        })
    } else {
        None
    };
    drop(player);
    let _ = app.emit("player://state", &snapshot);
    if let Some(launch) = external_launch {
        if let Ok(mut pending) = state.external_launch.lock() {
            *pending = Some(launch.clone());
        }
        let _ = app.emit("player://external-open", launch);
    }
    result.map(|_| snapshot)
}
#[tauri::command]
async fn import_paths(
    app: AppHandle,
    paths: Vec<String>,
    mode: String,
    recursive: bool,
) -> AppResult<AppSnapshot> {
    tauri::async_runtime::spawn_blocking(move || import_files(&app, paths, &mode, recursive, false))
        .await
        .map_err(|e| AppError::new("internal_error", e))?
}
#[tauri::command]
async fn import_playlist(app: AppHandle, path: String) -> AppResult<AppSnapshot> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<HostState>();
        let _scan = state
            .scanning
            .lock()
            .map_err(|_| AppError::new("internal_error", "Scan lock"))?;
        let paths = read_m3u8(Path::new(&path)).map_err(|e| AppError::new("import_error", e))?;
        let scan = scan_requested(&paths, false, &state.cover_dir)?;
        let name = Path::new(&path)
            .file_stem()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let mut player = lock_player(&state)?;
        player.import_playlist(name, scan.tracks, scan.warnings)?;
        let snapshot = player.snapshot();
        drop(player);
        let _ = app.emit("player://state", &snapshot);
        Ok(snapshot)
    })
    .await
    .map_err(|e| AppError::new("internal_error", e))?
}
#[tauri::command]
async fn export_playlist(
    app: AppHandle,
    path: String,
    playlist_id: Option<String>,
) -> AppResult<()> {
    tauri::async_runtime::spawn_blocking(move || {
        let tracks =
            lock_player(&app.state::<HostState>())?.export_tracks(playlist_id.as_deref())?;
        write_m3u8(Path::new(&path), &tracks).map_err(|e| AppError::new("export_error", e))
    })
    .await
    .map_err(|e| AppError::new("internal_error", e))?
}
#[tauri::command]
fn get_audio_devices(state: State<'_, HostState>) -> AppResult<Vec<AudioDevice>> {
    Ok(lock_player(&state)?.audio_devices())
}

#[tauri::command]
fn export_settings_document(
    state: State<'_, HostState>,
    sections: Vec<String>,
) -> AppResult<serde_json::Value> {
    lock_player(&state)?.export_settings_document(sections)
}

#[tauri::command]
async fn read_config_file(path: String) -> AppResult<serde_json::Value> {
    tauri::async_runtime::spawn_blocking(move || {
        use std::io::Read;
        let file = std::fs::File::open(path).map_err(|e| AppError::new("import_error", e))?;
        let mut bytes = Vec::new();
        file.take(player_application::MAX_CONFIG_BYTES as u64 + 1)
            .read_to_end(&mut bytes)
            .map_err(|e| AppError::new("import_error", e))?;
        if bytes.len() > player_application::MAX_CONFIG_BYTES {
            return Err(AppError::new(
                "invalid_input",
                "Configuration exceeds 2 MiB",
            ));
        }
        serde_json::from_slice(&bytes).map_err(|e| AppError::new("import_error", e))
    })
    .await
    .map_err(|e| AppError::new("internal_error", e))?
}

#[tauri::command]
async fn write_config_file(path: String, document: serde_json::Value) -> AppResult<()> {
    tauri::async_runtime::spawn_blocking(move || {
        let bytes =
            serde_json::to_vec_pretty(&document).map_err(|e| AppError::new("export_error", e))?;
        if bytes.len() > player_application::MAX_CONFIG_BYTES {
            return Err(AppError::new(
                "invalid_input",
                "Configuration exceeds 2 MiB",
            ));
        }
        config_file::write_atomic(Path::new(&path), &bytes)
            .map_err(|e| AppError::new("export_error", e))
    })
    .await
    .map_err(|e| AppError::new("internal_error", e))?
}

fn external_files(app: AppHandle, paths: Vec<String>) {
    let files: Vec<String> = paths
        .into_iter()
        .filter(|p| Path::new(p).is_file() || Path::new(p).is_dir())
        .collect();
    if !files.is_empty() {
        if app.try_state::<HostState>().is_none() {
            if let Ok(mut pending) = early_launches().lock() {
                pending.extend(files);
            }
            return;
        }
        tauri::async_runtime::spawn_blocking(move || {
            if let Err(error) = import_files(&app, files, "play", true, true) {
                let _ = app.emit("player://error", error);
            }
        });
    }
}
fn early_launches() -> &'static Mutex<Vec<String>> {
    static LAUNCHES: std::sync::OnceLock<Mutex<Vec<String>>> = std::sync::OnceLock::new();
    LAUNCHES.get_or_init(|| Mutex::new(vec![]))
}

fn translated(language: &str, key: &str) -> String {
    let raw = if language == "en" {
        include_str!("../../src/i18n/locales/en.json")
    } else {
        include_str!("../../src/i18n/locales/de.json")
    };
    let value = serde_json::from_str::<serde_json::Value>(raw).ok();
    value
        .and_then(|v| {
            v.get("native")
                .and_then(|m| m.get(key))
                .and_then(|v| v.as_str())
                .map(str::to_owned)
        })
        .unwrap_or_else(|| key.into())
}
fn rebuild_menu(app: &AppHandle, language: &str) -> tauri::Result<()> {
    use tauri::menu::{Menu, MenuItem, PredefinedMenuItem, Submenu};
    // Windows/Linux shortcuts are customizable in the frontend. Keep tray
    // actions free of hardcoded bindings that could intercept the new choices.
    let native_accelerator = |binding| cfg!(target_os = "macos").then_some(binding);
    let open = MenuItem::with_id(
        app,
        "open",
        translated(language, "openFiles"),
        true,
        native_accelerator("CmdOrCtrl+O"),
    )?;
    let folder = MenuItem::with_id(
        app,
        "folder",
        translated(language, "openFolder"),
        true,
        native_accelerator("CmdOrCtrl+Shift+O"),
    )?;
    let quit = MenuItem::with_id(
        app,
        "quit",
        translated(language, "quit"),
        true,
        native_accelerator("CmdOrCtrl+Q"),
    )?;
    let sep = PredefinedMenuItem::separator(app)?;
    let file = Submenu::with_items(
        app,
        translated(language, "file"),
        true,
        &[&open, &folder, &sep, &quit],
    )?;
    let toggle = MenuItem::with_id(
        app,
        "toggle",
        translated(language, "playPause"),
        true,
        None::<&str>,
    )?;
    let previous = MenuItem::with_id(
        app,
        "previous",
        translated(language, "previous"),
        true,
        None::<&str>,
    )?;
    let next = MenuItem::with_id(
        app,
        "next",
        translated(language, "next"),
        true,
        None::<&str>,
    )?;
    let playback = Submenu::with_items(
        app,
        translated(language, "playback"),
        true,
        &[&toggle, &previous, &next],
    )?;
    let menu = Menu::with_items(app, &[&file, &playback])?;
    // Windows/Linux use the in-app titlebar. macOS keeps its system app menu.
    #[cfg(target_os = "macos")]
    app.set_menu(menu)?;
    #[cfg(not(target_os = "macos"))]
    drop(menu);
    if let Some(tray) = app.tray_by_id("main") {
        let show = MenuItem::with_id(
            app,
            "show",
            translated(language, "show"),
            true,
            None::<&str>,
        )?;
        tray.set_menu(Some(Menu::with_items(
            app,
            &[&show, &toggle, &previous, &next, &quit],
        )?))?;
    }
    Ok(())
}
fn handle_menu(app: &AppHandle, id: &str) {
    use tauri_plugin_dialog::DialogExt;
    match id {
        "quit" => app.exit(0),
        "show" => {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.unminimize();
                let _ = window.set_focus();
            }
        }
        "open" | "folder" => {
            let handle = app.clone();
            if id == "folder" {
                app.dialog().file().pick_folder(move |path| {
                    if let Some(path) = path.and_then(|p| p.into_path().ok()) {
                        let app = handle.clone();
                        tauri::async_runtime::spawn_blocking(move || {
                            if let Err(e) = import_files(
                                &app,
                                vec![path.to_string_lossy().into_owned()],
                                "library",
                                true,
                                false,
                            ) {
                                let _ = app.emit("player://error", e);
                            }
                        });
                    }
                });
            } else {
                app.dialog()
                    .file()
                    .add_filter(
                        "Audio",
                        &[
                            "mp3", "flac", "wav", "m4a", "ogg", "opus", "aac", "aiff", "wma", "wv",
                            "ape",
                        ],
                    )
                    .pick_files(move |paths| {
                        if let Some(paths) = paths {
                            let files = paths
                                .into_iter()
                                .filter_map(|p| p.into_path().ok())
                                .map(|p| p.to_string_lossy().into_owned())
                                .collect();
                            external_files(handle.clone(), files);
                        }
                    });
            }
        }
        _ => {
            let command = match id {
                "toggle" => PlayerCommand::PlayPause,
                "previous" => PlayerCommand::Previous,
                "next" => PlayerCommand::Next,
                _ => return,
            };
            let app = app.clone();
            tauri::async_runtime::spawn_blocking(move || {
                if let Err(e) = execute(&app, command) {
                    let _ = app.emit("player://error", e);
                }
            });
        }
    }
}

fn spawn_workers(app: AppHandle, running: Arc<AtomicBool>) {
    let monitor_app = app.clone();
    let monitor_running = running.clone();
    std::thread::spawn(move || {
        use notify::{RecursiveMode, Watcher};
        let (send, recv) = std::sync::mpsc::channel();
        let mut watcher = match notify::recommended_watcher(move |event| {
            let _ = send.send(event);
        }) {
            Ok(w) => w,
            Err(e) => {
                eprintln!("Library watcher: {e}");
                return;
            }
        };
        let mut watched = Vec::<String>::new();
        while monitor_running.load(Ordering::Relaxed) {
            let folders = lock_player(&monitor_app.state::<HostState>())
                .map(|p| p.state.library_folders.clone())
                .unwrap_or_default();
            for old in watched.iter().filter(|p| !folders.contains(p)) {
                let _ = watcher.unwatch(Path::new(old));
            }
            for new in folders.iter().filter(|p| !watched.contains(p)) {
                let _ = watcher.watch(Path::new(new), RecursiveMode::Recursive);
            }
            watched = folders;
            if let Ok(Ok(event)) = recv.recv_timeout(Duration::from_secs(1)) {
                if matches!(event.kind, notify::EventKind::Access(_)) {
                    continue;
                }
                std::thread::sleep(Duration::from_millis(500));
                while recv.try_recv().is_ok() {}
                if !watched.is_empty() {
                    let _ = import_files(&monitor_app, watched.clone(), "rescan", true, false);
                }
            }
        }
    });
    std::thread::spawn(move || {
        let mut ticks = 0_u32;
        while running.load(Ordering::Relaxed) {
            std::thread::sleep(Duration::from_millis(200));
            let state = app.state::<HostState>();
            if let Ok(mut player) = lock_player(&state) {
                let changed = player.tick();
                let playback = player.playback();
                let snapshot = changed.then(|| player.snapshot());
                ticks += 1;
                if changed || ticks.is_multiple_of(50) {
                    if let Err(error) = player.persist() {
                        eprintln!("State save: {error}");
                    }
                }
                drop(player);
                let _ = app.emit("player://playback", playback);
                if let Some(snapshot) = snapshot {
                    let _ = app.emit("player://state", snapshot);
                }
            };
        }
    });
}

pub fn run() {
    let running = Arc::new(AtomicBool::new(true));
    let shutdown = running.clone();
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, args, cwd| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.unminimize();
                let _ = window.set_focus();
            }
            let paths = args
                .into_iter()
                .skip(1)
                .map(|arg| {
                    let path = PathBuf::from(arg);
                    if path.is_absolute() {
                        path
                    } else {
                        Path::new(&cwd).join(path)
                    }
                })
                .map(|p| p.to_string_lossy().into_owned())
                .collect();
            external_files(app.clone(), paths);
        }))
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            get_snapshot,
            dispatch,
            import_paths,
            import_playlist,
            export_playlist,
            get_audio_devices,
            export_settings_document,
            read_config_file,
            write_config_file,
            get_default_app_status,
            open_default_apps_settings,
            take_external_launch,
            get_statistics,
            get_statistics_years,
            clear_statistics,
            export_history,
            export_history_file,
            import_playlist_icon,
            configure_global_search,
            audio_jobs::start_loudness_analysis,
            audio_jobs::start_audio_export,
            audio_jobs::get_audio_job,
            audio_jobs::cancel_audio_job,
            audio_jobs::get_audio_waveform,
            audio_jobs::get_audio_preview,
            quit_app
        ])
        .on_menu_event(|app, event| handle_menu(app, event.id().as_ref()))
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                let close_to_tray = window
                    .app_handle()
                    .try_state::<HostState>()
                    .and_then(|state| {
                        lock_player(&state)
                            .ok()
                            .map(|player| player.state.settings.close_to_tray)
                    })
                    .unwrap_or(false);
                if close_to_tray {
                    // Prevent destruction only after hiding succeeds, so a
                    // platform/tray failure cannot strand an invisible app.
                    if window.hide().is_ok() {
                        api.prevent_close();
                    }
                }
            }
        })
        .setup(move |app| {
            let custom_data = std::env::var_os("SONG_PLAYER_DATA_DIR").map(PathBuf::from);
            let data_dir = custom_data.clone().unwrap_or(app.path().app_data_dir()?);
            let cover_dir = match custom_data {
                Some(dir) => dir.join("covers"),
                None => app.path().app_cache_dir()?.join("covers"),
            };
            std::fs::create_dir_all(&data_dir)?;
            std::fs::create_dir_all(&cover_dir)?;
            let icon_dir = data_dir.join("playlist-icons");
            let preview_dir = data_dir.join("audio-previews");
            std::fs::create_dir_all(&icon_dir)?;
            std::fs::create_dir_all(&preview_dir)?;
            app.asset_protocol_scope()
                .allow_directory(&cover_dir, true)?;
            app.asset_protocol_scope()
                .allow_directory(&icon_dir, true)?;
            app.asset_protocol_scope()
                .allow_directory(&preview_dir, true)?;
            let mut search = vec![];
            if let Ok(dir) = app.path().resource_dir() {
                search.push(dir.join("mpv"));
                search.push(dir.join("resources/mpv"));
            }
            search.push(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/mpv"));
            let executable = if cfg!(windows) {
                "ffmpeg.exe"
            } else {
                "ffmpeg"
            };
            let mut tool_search = vec![];
            if let Ok(dir) = app.path().resource_dir() {
                tool_search.push(dir.join("ffmpeg").join(executable));
                tool_search.push(dir.join("resources/ffmpeg").join(executable));
            }
            tool_search.push(
                PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("resources/ffmpeg")
                    .join(executable),
            );
            let audio_tools = tool_search
                .into_iter()
                .find(|p| p.is_file())
                .ok_or_else(|| "FFmpeg runtime not found; run scripts/setup-ffmpeg.ps1".to_owned())
                .and_then(|p| AudioTools::new(p).map_err(|e| e.to_string()));
            let audio = MpvEngine::new(&search)
                .map(|e| Box::new(NativeAudio(e)) as Box<dyn AudioPort>)
                .map_err(|e| e.to_string());
            let store = Store::open(&data_dir.join("song-player.sqlite"))?;
            let mut service = PlayerService::new(Box::new(SqliteState(store)), audio)?;
            for session in &mut service.state.sessions {
                session.sanitize();
            }
            let locale = service.state.settings.language.clone();
            let global_preferences = service.state.settings.global_search.clone();
            app.manage(HostState {
                player: Mutex::new(service),
                cover_dir,
                scanning: Mutex::new(()),
                icon_dir,
                preview_dir,
                audio_tools,
                audio_jobs: audio_jobs::AudioJobs::default(),
                external_launch: Mutex::new(None),
                commands: Mutex::new(()),
            });
            let global = global_search::GlobalSearch::spawn(app.handle().clone(), running.clone());
            if let Err(error) =
                global.configure(global_preferences.enabled, global_preferences.binding)
            {
                eprintln!("Global search: {error}");
                if let Ok(mut player) = lock_player(&app.state::<HostState>()) {
                    player.report_warning(format!("Global search: {error}"));
                }
                let _ = app.emit("player://error", error);
            }
            app.manage(global);
            // The window starts hidden and the web view shows it once the first
            // view is rendered. If that never happens, show it anyway so the app
            // can't stay invisible.
            let startup = app.handle().clone();
            std::thread::spawn(move || {
                for _ in 0..40 {
                    std::thread::sleep(Duration::from_millis(100));
                    let Some(window) = startup.get_webview_window("main") else {
                        return;
                    };
                    // Once shown, later hiding (close to tray) is the user's choice.
                    if window.is_visible().unwrap_or(true) {
                        return;
                    }
                }
                if let Some(window) = startup.get_webview_window("main") {
                    let _ = window.show();
                    let _ = window.set_focus();
                }
            });
            tauri::tray::TrayIconBuilder::with_id("main")
                .icon(app.default_window_icon().expect("bundled icon").clone())
                .tooltip("Song Player")
                .on_tray_icon_event(|tray, event| {
                    if matches!(
                        event,
                        tauri::tray::TrayIconEvent::DoubleClick {
                            button: tauri::tray::MouseButton::Left,
                            ..
                        }
                    ) {
                        if let Some(window) = tray.app_handle().get_webview_window("main") {
                            let _ = window.show();
                            let _ = window.unminimize();
                            let _ = window.set_focus();
                        }
                    }
                })
                .build(app)?;
            rebuild_menu(app.handle(), &locale)?;
            spawn_workers(app.handle().clone(), running.clone());
            crate::media_controls::spawn(app.handle().clone(), running.clone());
            let mut args: Vec<String> = std::env::args_os()
                .skip(1)
                .map(|p| p.to_string_lossy().into_owned())
                .collect();
            if let Ok(mut pending) = early_launches().lock() {
                args.extend(pending.drain(..));
            }
            external_files(app.handle().clone(), args);
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("Song Player could not initialize");
    app.run(move |app, event| match event {
        tauri::RunEvent::Exit => {
            shutdown.store(false, Ordering::Relaxed);
            if let Some(state) = app.try_state::<HostState>() {
                state.audio_jobs.cancel();
                if let Ok(mut player) = lock_player(&state) {
                    let _ = player.persist();
                }
            }
        }
        #[cfg(target_os = "macos")]
        tauri::RunEvent::Opened { urls } => {
            external_files(
                app.clone(),
                urls.into_iter()
                    .filter_map(|url| url.to_file_path().ok())
                    .map(|path| path.to_string_lossy().into_owned())
                    .collect(),
            );
        }
        _ => {}
    });
}
