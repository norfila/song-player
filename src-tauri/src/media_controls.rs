//! Windows System Media Transport Controls. Playback remains in the coordinator;
//! native callbacks enqueue commands instead of touching libmpv or blocking COM.
use std::sync::{atomic::AtomicBool, Arc};

pub fn spawn(app: tauri::AppHandle, running: Arc<AtomicBool>) {
    #[cfg(target_os = "windows")]
    windows_controls::spawn(app, running);
    #[cfg(not(target_os = "windows"))]
    let _ = (app, running);
}

#[cfg(target_os = "windows")]
mod windows_controls {
    use super::*;
    use player_application::{AppSnapshot, PlayerCommand};
    use souvlaki::{
        MediaControlEvent, MediaControls, MediaMetadata, MediaPlayback, MediaPosition,
        PlatformConfig, SeekDirection,
    };
    use std::{
        ffi::c_void,
        sync::{atomic::Ordering, mpsc},
        time::Duration,
    };
    use tauri::{Emitter, Manager};

    // The worker owns its WinRT apartment. This avoids relying on the WebView/UI
    // thread's COM initialization or moving apartment-bound objects to a thread.
    #[link(name = "runtimeobject")]
    extern "system" {
        fn RoInitialize(init_type: u32) -> i32;
        fn RoUninitialize();
    }

    struct RuntimeApartment;
    impl RuntimeApartment {
        fn new() -> Result<Self, i32> {
            // SAFETY: This is called once on a new worker thread; 1 is
            // RO_INIT_MULTITHREADED. Every successful initialization is balanced.
            let status = unsafe { RoInitialize(1) };
            if status < 0 {
                Err(status)
            } else {
                Ok(Self)
            }
        }
    }
    impl Drop for RuntimeApartment {
        fn drop(&mut self) {
            // SAFETY: Same thread and one successful RoInitialize above.
            unsafe { RoUninitialize() };
        }
    }

    #[derive(Default, PartialEq, Eq)]
    struct Metadata {
        title: String,
        artist: String,
        album: String,
        duration: Option<Duration>,
    }

    pub(super) fn spawn(app: tauri::AppHandle, running: Arc<AtomicBool>) {
        // Obtain the HWND while setting up the application. Only its numeric
        // value crosses the thread boundary; no Send assertion for raw pointers.
        let hwnd = match app
            .get_webview_window("main")
            .and_then(|window| window.hwnd().ok())
        {
            Some(hwnd) if !hwnd.0.is_null() => hwnd.0 as usize,
            _ => {
                eprintln!("System media controls: main window handle unavailable");
                return;
            }
        };
        if let Err(error) = std::thread::Builder::new()
            .name("system-media-controls".into())
            .spawn(move || run(app, running, hwnd))
        {
            eprintln!("System media controls: cannot start worker: {error}");
        }
    }

    fn run(app: tauri::AppHandle, running: Arc<AtomicBool>, hwnd: usize) {
        let _apartment = match RuntimeApartment::new() {
            Ok(apartment) => apartment,
            Err(status) => {
                eprintln!("System media controls: WinRT initialization failed (0x{status:08X})");
                return;
            }
        };
        let mut controls = match MediaControls::new(PlatformConfig {
            dbus_name: "songplayer",
            display_name: "Song Player",
            hwnd: Some(hwnd as *mut c_void),
        }) {
            Ok(controls) => controls,
            Err(error) => {
                eprintln!("System media controls: unavailable: {error}");
                return;
            }
        };
        // Bounded and nonblocking: holding a media key cannot stall a Windows
        // callback or build up an unbounded command backlog.
        let (send, receive) = mpsc::sync_channel(32);
        let callback_running = running.clone();
        if let Err(error) = controls.attach(move |event| {
            if callback_running.load(Ordering::Relaxed) {
                let _ = send.try_send(event);
            }
        }) {
            eprintln!("System media controls: cannot attach controls: {error}");
            return;
        }
        let mut last_metadata = None;
        let mut last_playback = None;
        let mut metadata_error_logged = false;
        let mut playback_error_logged = false;
        while running.load(Ordering::Relaxed) {
            let event = match receive.recv_timeout(Duration::from_millis(500)) {
                Ok(event) => Some(event),
                Err(mpsc::RecvTimeoutError::Timeout) => None,
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
            };
            if !running.load(Ordering::Relaxed) {
                break;
            }
            let Ok(mut snapshot) = crate::host::snapshot_for_media(&app) else {
                continue;
            };
            if let Some(event) = event {
                match event {
                    MediaControlEvent::Raise => {
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                    MediaControlEvent::Quit => app.exit(0),
                    event => {
                        if let Some(command) = command_for_event(event, &snapshot) {
                            match crate::host::execute(&app, command) {
                                Ok(updated) => snapshot = updated,
                                Err(error) => {
                                    let _ = app.emit("player://error", error);
                                }
                            }
                        }
                    }
                }
            }
            let metadata = metadata_for(&snapshot);
            if last_metadata.as_ref() != Some(&metadata) {
                // Explicit empty strings clear old title/artist/album when the
                // queue is cleared; None would retain Windows' previous values.
                let result = controls.set_metadata(MediaMetadata {
                    title: Some(&metadata.title),
                    artist: Some(&metadata.artist),
                    album: Some(&metadata.album),
                    duration: metadata.duration,
                    cover_url: None,
                });
                match result {
                    Ok(()) => {
                        last_metadata = Some(metadata);
                        metadata_error_logged = false;
                    }
                    Err(error) if !metadata_error_logged => {
                        eprintln!("System media controls: metadata update failed: {error}");
                        metadata_error_logged = true;
                    }
                    Err(_) => {}
                }
            }
            let playback = playback_for(&snapshot);
            if last_playback.as_ref() != Some(&playback) {
                match controls.set_playback(playback.clone()) {
                    Ok(()) => {
                        last_playback = Some(playback);
                        playback_error_logged = false;
                    }
                    Err(error) if !playback_error_logged => {
                        eprintln!("System media controls: playback update failed: {error}");
                        playback_error_logged = true;
                    }
                    Err(_) => {}
                }
            }
        }
        let _ = controls.set_playback(MediaPlayback::Stopped);
        // controls detach before the apartment is uninitialized (reverse drop
        // order), including normal application exit and every early return.
    }

    fn seconds(value: f64) -> Option<Duration> {
        Duration::try_from_secs_f64(value).ok()
    }

    fn metadata_for(snapshot: &AppSnapshot) -> Metadata {
        let track = snapshot.playback.track_id.as_ref().and_then(|id| {
            snapshot
                .tracks
                .iter()
                .find(|track| &track.id == id)
                .or_else(|| {
                    snapshot
                        .sessions
                        .iter()
                        .flat_map(|session| &session.entries)
                        .map(|entry| &entry.track)
                        .find(|track| &track.id == id)
                })
        });
        match track {
            Some(track) => Metadata {
                title: track.title.clone(),
                artist: track.artist.clone(),
                album: track.album.clone(),
                duration: seconds(snapshot.playback.duration)
                    .filter(|duration| !duration.is_zero()),
            },
            None => Metadata::default(),
        }
    }

    fn playback_for(snapshot: &AppSnapshot) -> MediaPlayback {
        let progress = seconds(snapshot.playback.position.max(0.0)).map(MediaPosition);
        match snapshot.playback.status.as_str() {
            "playing" => MediaPlayback::Playing { progress },
            "paused" => MediaPlayback::Paused { progress },
            _ => MediaPlayback::Stopped,
        }
    }

    fn command_for_event(
        event: MediaControlEvent,
        snapshot: &AppSnapshot,
    ) -> Option<PlayerCommand> {
        let relative_seek = |direction: SeekDirection, amount: f64| {
            let offset = match direction {
                SeekDirection::Forward => amount,
                SeekDirection::Backward => -amount,
            };
            let mut position = (snapshot.playback.position + offset).max(0.0);
            if snapshot.playback.duration.is_finite() && snapshot.playback.duration > 0.0 {
                position = position.min(snapshot.playback.duration);
            }
            PlayerCommand::Seek { position }
        };
        Some(match event {
            MediaControlEvent::Play => PlayerCommand::Play,
            MediaControlEvent::Pause => PlayerCommand::Pause,
            MediaControlEvent::Toggle => PlayerCommand::PlayPause,
            MediaControlEvent::Stop => PlayerCommand::Stop,
            MediaControlEvent::Next => PlayerCommand::Next,
            MediaControlEvent::Previous => PlayerCommand::Previous,
            MediaControlEvent::Seek(direction) => relative_seek(direction, 10.0),
            MediaControlEvent::SeekBy(direction, amount) => {
                relative_seek(direction, amount.as_secs_f64())
            }
            MediaControlEvent::SetPosition(position) => PlayerCommand::Seek {
                position: position.0.as_secs_f64(),
            },
            MediaControlEvent::SetVolume(volume) if volume.is_finite() => {
                PlayerCommand::SetVolume {
                    volume: volume.clamp(0.0, 1.0) * 100.0,
                }
            }
            _ => return None,
        })
    }
}
