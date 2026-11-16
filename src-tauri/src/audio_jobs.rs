use super::{lock_player, HostState};
use player_application::{AppError, AppResult, GainGrouping, LoudnessValue, PlayerCommand};
use player_domain::Track;
use player_infrastructure::{write_gain_tags, AudioTools};
use serde::Serialize;
use std::{
    collections::{BTreeMap, HashSet},
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc, Mutex,
    },
    time::{Instant, SystemTime, UNIX_EPOCH},
};
use tauri::{AppHandle, Emitter, Manager, State};

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct AudioJob {
    pub id: String,
    pub kind: String,
    pub status: String,
    pub progress: f64,
    pub message: Option<String>,
    pub output_path: Option<String>,
}
pub(super) struct AudioJobs {
    current: Mutex<Option<AudioJob>>,
    cancel: Mutex<Arc<AtomicBool>>,
}
impl Default for AudioJobs {
    fn default() -> Self {
        Self {
            current: Mutex::new(None),
            cancel: Mutex::new(Arc::new(AtomicBool::new(false))),
        }
    }
}
impl AudioJobs {
    fn reserve(&self, kind: &str) -> AppResult<(AudioJob, Arc<AtomicBool>)> {
        let mut current = self
            .current
            .lock()
            .map_err(|e| AppError::new("internal_error", e))?;
        if current.as_ref().is_some_and(|j| j.status == "running") {
            return Err(AppError::new(
                "audio_job_running",
                "An audio task is already running",
            ));
        }
        let cancel = Arc::new(AtomicBool::new(false));
        *self
            .cancel
            .lock()
            .map_err(|e| AppError::new("internal_error", e))? = cancel.clone();
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let id = format!(
            "{}-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        );
        let job = AudioJob {
            id,
            kind: kind.into(),
            status: "running".into(),
            progress: 0.0,
            message: None,
            output_path: None,
        };
        *current = Some(job.clone());
        Ok((job, cancel))
    }
    fn update(&self, app: &AppHandle, id: &str, update: impl FnOnce(&mut AudioJob)) {
        if let Ok(mut current) = self.current.lock() {
            if let Some(job) = current.as_mut().filter(|j| j.id == id) {
                update(job);
                let _ = app.emit("player://audio-job", job.clone());
            }
        }
    }
    fn finish(&self, app: &AppHandle, id: &str, result: AppResult<Option<String>>) {
        self.update(app, id, |job| match result {
            Ok(path) => {
                job.status = "completed".into();
                job.progress = 1.0;
                job.output_path = path;
            }
            Err(error) => {
                job.status = if error.code == "cancelled" {
                    "cancelled"
                } else {
                    "failed"
                }
                .into();
                job.message = Some(error.details);
            }
        });
    }
    pub fn cancel(&self) {
        if let Ok(cancel) = self.cancel.lock() {
            cancel.store(true, Ordering::Relaxed);
        }
    }
}
fn tools(state: &HostState) -> AppResult<AudioTools> {
    state
        .audio_tools
        .clone()
        .map_err(|e| AppError::new("audio_tools_missing", e))
}
fn track(state: &HostState, id: &str) -> AppResult<Track> {
    lock_player(state)?
        .state
        .tracks
        .iter()
        .find(|t| t.id == id)
        .cloned()
        .ok_or_else(|| AppError::new("not_found", "Track"))
}
fn processing(error: player_infrastructure::InfraError) -> AppError {
    AppError::new(error.code, error.detail)
}

#[tauri::command]
pub(super) fn get_audio_job(state: State<'_, HostState>) -> AppResult<Option<AudioJob>> {
    Ok(state
        .audio_jobs
        .current
        .lock()
        .map_err(|e| AppError::new("internal_error", e))?
        .clone())
}
#[tauri::command]
pub(super) fn cancel_audio_job(state: State<'_, HostState>) {
    state.audio_jobs.cancel();
}

fn group_key(track: &Track, grouping: GainGrouping) -> String {
    player_application::loudness_group_key(track, grouping)
}

fn validate_group(
    expected: &[Track],
    current: &[Track],
    key: &str,
    grouping: GainGrouping,
) -> AppResult<()> {
    let members = |tracks: &[Track]| {
        tracks
            .iter()
            .filter(|track| group_key(track, grouping) == key)
            .map(|track| {
                (
                    track.id.clone(),
                    (
                        super::canonical_key(&track.path),
                        track.content_fingerprint.clone(),
                    ),
                )
            })
            .collect::<BTreeMap<_, _>>()
    };
    if members(expected) != members(current) {
        return Err(AppError::new(
            "analysis_sources_changed",
            "The analyzed songs or group membership changed. Please run the analysis again.",
        ));
    }
    Ok(())
}
#[derive(PartialEq, Eq)]
struct SourceStamp {
    length: u64,
    modified: SystemTime,
}
fn source_stamps(tracks: &[Track]) -> AppResult<Vec<SourceStamp>> {
    tracks
        .iter()
        .map(|track| {
            let metadata = std::fs::metadata(&track.path)
                .map_err(|e| AppError::new("analysis_sources_changed", e))?;
            Ok(SourceStamp {
                length: metadata.len(),
                modified: metadata
                    .modified()
                    .map_err(|e| AppError::new("analysis_sources_changed", e))?,
            })
        })
        .collect()
}
#[tauri::command]
pub(super) fn start_loudness_analysis(
    app: AppHandle,
    track_ids: Vec<String>,
    grouping: GainGrouping,
    write_tags: bool,
) -> AppResult<AudioJob> {
    if track_ids.is_empty() || track_ids.len() > 100_000 {
        return Err(AppError::new(
            "invalid_input",
            "Select audio tracks to analyze",
        ));
    }
    if write_tags && grouping == GainGrouping::Folder {
        return Err(AppError::new(
            "invalid_input",
            "Folder ReplayGain is stored in the app; no interoperable folder tag exists",
        ));
    }
    let state = app.state::<HostState>();
    let tool = tools(&state)?;
    let all = lock_player(&state)?.state.tracks.clone();
    let selected: HashSet<_> = track_ids.into_iter().collect();
    if selected.iter().any(|id| !all.iter().any(|t| &t.id == id)) {
        return Err(AppError::new(
            "not_found",
            "A selected track no longer exists",
        ));
    }
    let keys: HashSet<_> = all
        .iter()
        .filter(|t| selected.contains(&t.id))
        .map(|t| group_key(t, grouping))
        .collect();
    let mut groups = BTreeMap::<String, Vec<Track>>::new();
    for track in all {
        let key = group_key(&track, grouping);
        if keys.contains(&key) {
            groups.entry(key).or_default().push(track);
        }
    }
    if write_tags
        && groups.values().flatten().any(|t| {
            !Path::new(&t.path)
                .extension()
                .and_then(|e| e.to_str())
                .is_some_and(|e| {
                    ["flac", "mp3", "ogg", "oga", "m4a"].contains(&e.to_ascii_lowercase().as_str())
                })
        })
    {
        return Err(AppError::new(
            "unsupported_tags",
            "Tag writing supports FLAC, MP3, Ogg Vorbis and M4A; analyze other formats in the app",
        ));
    }
    let (job, cancel) = state.audio_jobs.reserve("analysis")?;
    let response = job.clone();
    let _ = app.emit("player://audio-job", &job);
    tauri::async_runtime::spawn_blocking(move || {
        let total = groups.values().map(|g| g.len()).sum::<usize>().max(1) as f64;
        let mut completed = 0_f64;
        let mut last = Instant::now() - std::time::Duration::from_secs(1);
        let result = (|| -> AppResult<Option<String>> {
            for (key, mut tracks) in groups {
                tracks.sort_by(|a, b| {
                    a.disc_number
                        .cmp(&b.disc_number)
                        .then(a.track_number.cmp(&b.track_number))
                        .then(a.path.cmp(&b.path))
                });
                let inputs = tracks
                    .iter()
                    .map(|t| (PathBuf::from(&t.path), t.duration))
                    .collect::<Vec<_>>();
                let size = tracks.len() as f64;
                let initial_stamps = source_stamps(&tracks)?;
                let gain = tool
                    .analyze(&inputs, &cancel, |fraction| {
                        if last.elapsed().as_millis() >= 150 {
                            app.state::<HostState>()
                                .audio_jobs
                                .update(&app, &job.id, |j| {
                                    j.progress = ((completed + fraction * size) / total).min(0.99)
                                });
                            last = Instant::now();
                        }
                    })
                    .map_err(processing)?;
                // Commit each completed group before moving to the next. Cancel
                // retains completed work and never stores partial group values.
                let group_values = tracks
                    .iter()
                    .map(|t| LoudnessValue {
                        track_id: t.id.clone(),
                        gain: gain.gain,
                        peak: gain.peak,
                        grouping,
                        group_key: if grouping == GainGrouping::Track {
                            None
                        } else {
                            Some(key.clone())
                        },
                    })
                    .collect::<Vec<_>>();
                let state = app.state::<HostState>();
                // Scans/imports cannot change known tracks during this batch.
                // Keep playback commands free while metadata copies are written;
                // revalidate under the coordinator lock at the final commit.
                let _scan = state
                    .scanning
                    .lock()
                    .map_err(|e| AppError::new("internal_error", e))?;
                validate_group(&tracks, &lock_player(&state)?.state.tracks, &key, grouping)?;
                if initial_stamps != source_stamps(&tracks)? {
                    return Err(AppError::new(
                        "analysis_sources_changed",
                        "An audio file changed during analysis. Please run the analysis again.",
                    ));
                }
                if cancel.load(Ordering::Relaxed) {
                    return Err(AppError::new("cancelled", "Audio task cancelled"));
                }
                if write_tags {
                    let mut written = vec![];
                    let tag_result = (|| -> AppResult<()> {
                        for track in &tracks {
                            if cancel.load(Ordering::Relaxed) {
                                return Err(AppError::new("cancelled", "Audio task cancelled"));
                            }
                            write_gain_tags(
                                Path::new(&track.path),
                                gain,
                                grouping == GainGrouping::Album,
                            )
                            .map_err(processing)?;
                            written.push(track.path.clone());
                        }
                        Ok(())
                    })();
                    // Batch refresh invalidates the previous group exactly once.
                    // On a later file failure, even successful earlier metadata
                    // writes receive their new fingerprints before returning.
                    if !written.is_empty() {
                        let scan = super::scan_requested(&written, false, &state.cover_dir)?;
                        // Our own tag writes legitimately change fingerprints.
                        // Keep the analyzed group identity, and expect exactly
                        // the refreshed bytes when validating the commit.
                        for track in &mut tracks {
                            if let Some(refreshed) = scan.tracks.iter().find(|candidate| {
                                super::canonical_key(&candidate.path)
                                    == super::canonical_key(&track.path)
                            }) {
                                track.content_fingerprint = refreshed.content_fingerprint.clone();
                            }
                        }
                        lock_player(&state)?.refresh_scanned_metadata(scan.tracks)?;
                    }
                    tag_result?;
                }
                let _commands = state
                    .commands
                    .lock()
                    .map_err(|e| AppError::new("internal_error", e))?;
                let mut player = lock_player(&state)?;
                validate_group(&tracks, &player.state.tracks, &key, grouping)?;
                player.dispatch(PlayerCommand::SetLoudnessValues {
                    values: group_values,
                })?;
                let snapshot = player.snapshot();
                drop(player);
                let _ = app.emit("player://state", snapshot);
                completed += size;
            }
            Ok(None)
        })();
        app.state::<HostState>()
            .audio_jobs
            .finish(&app, &job.id, result);
    });
    Ok(response)
}

#[tauri::command]
pub(super) fn start_audio_export(
    app: AppHandle,
    track_id: String,
    start: f64,
    end: f64,
    remove: bool,
    output_path: String,
) -> AppResult<AudioJob> {
    let state = app.state::<HostState>();
    let tool = tools(&state)?;
    let song = track(&state, &track_id)?;
    let (job, cancel) = state.audio_jobs.reserve("export")?;
    let response = job.clone();
    let _ = app.emit("player://audio-job", &job);
    tauri::async_runtime::spawn_blocking(move || {
        let mut last = Instant::now() - std::time::Duration::from_secs(1);
        let result = tool
            .export_selection(
                Path::new(&song.path),
                Path::new(&output_path),
                start,
                end,
                remove,
                song.duration,
                &cancel,
                |progress| {
                    if last.elapsed().as_millis() >= 150 {
                        app.state::<HostState>()
                            .audio_jobs
                            .update(&app, &job.id, |j| j.progress = progress.min(0.99));
                        last = Instant::now();
                    }
                },
            )
            .map_err(processing)
            .map(|_| Some(output_path));
        app.state::<HostState>()
            .audio_jobs
            .finish(&app, &job.id, result);
    });
    Ok(response)
}

#[tauri::command]
pub(super) async fn get_audio_waveform(app: AppHandle, track_id: String) -> AppResult<Vec<f64>> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<HostState>();
        let song = track(&state, &track_id)?;
        tools(&state)?
            .waveform(Path::new(&song.path), song.duration)
            .map_err(processing)
    })
    .await
    .map_err(|e| AppError::new("internal_error", e))?
}

#[tauri::command]
pub(super) async fn get_audio_preview(
    app: AppHandle,
    track_id: String,
    start: f64,
    end: f64,
    remove: bool,
) -> AppResult<String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<HostState>();
        let song = track(&state, &track_id)?;
        let tool = tools(&state)?;
        // A fixed small cache avoids accumulating previews. Unique filenames
        // prevent a stale WebView audio decoder from playing an older selection.
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let id = NEXT.fetch_add(1, Ordering::Relaxed);
        let output = state
            .preview_dir
            .join(format!("preview-{}-{id}.wav", std::process::id()));
        tool.preview_selection(
            Path::new(&song.path),
            &output,
            start,
            end,
            remove,
            song.duration,
        )
        .map_err(processing)?;
        if let Ok(files) = std::fs::read_dir(&state.preview_dir) {
            let now = SystemTime::now();
            let mut files = files
                .flatten()
                .filter_map(|f| {
                    f.metadata()
                        .ok()
                        .and_then(|m| m.modified().ok())
                        .map(|modified| (f.path(), modified))
                })
                .collect::<Vec<_>>();
            files.sort_by_key(|entry| std::cmp::Reverse(entry.1));
            for (index, (path, modified)) in files.into_iter().enumerate() {
                if path != output
                    && (index >= 8
                        || now
                            .duration_since(modified)
                            .is_ok_and(|d| d.as_secs() > 3600))
                {
                    let _ = std::fs::remove_file(path);
                }
            }
        }
        Ok(output.to_string_lossy().into_owned())
    })
    .await
    .map_err(|e| AppError::new("internal_error", e))?
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn folder_groups_are_path_based_and_albums_respect_album_artist() {
        let a = Track {
            path: "C:/Music/One/a.flac".into(),
            album: "Shared".into(),
            album_artist: "Ensemble".into(),
            artist: "Singer A".into(),
            ..Track::default()
        };
        let b = Track {
            path: "C:/Music/Two/b.flac".into(),
            artist: "Singer B".into(),
            ..a.clone()
        };
        assert_eq!(
            group_key(&a, GainGrouping::Album),
            group_key(&b, GainGrouping::Album)
        );
        assert_ne!(
            group_key(&a, GainGrouping::Folder),
            group_key(&b, GainGrouping::Folder)
        );
    }

    #[test]
    fn analysis_rejects_changed_group_members_files_and_fingerprints() {
        let original = Track {
            id: "one".into(),
            path: "C:/Music/Folder/one.flac".into(),
            content_fingerprint: Some("original-content".into()),
            ..Track::default()
        };
        let grouping = GainGrouping::Folder;
        let key = group_key(&original, grouping);
        assert!(validate_group(
            std::slice::from_ref(&original),
            std::slice::from_ref(&original),
            &key,
            grouping
        )
        .is_ok());
        let added = Track {
            id: "two".into(),
            path: "C:/Music/Folder/two.flac".into(),
            ..original.clone()
        };
        assert!(validate_group(
            std::slice::from_ref(&original),
            &[original.clone(), added],
            &key,
            grouping
        )
        .is_err());
        let changed = Track {
            content_fingerprint: Some("changed-content".into()),
            ..original.clone()
        };
        assert!(
            validate_group(std::slice::from_ref(&original), &[changed], &key, grouping).is_err()
        );
        let moved = Track {
            path: "C:/Music/Other/one.flac".into(),
            ..original.clone()
        };
        assert!(validate_group(&[original], &[moved], &key, grouping).is_err());
    }
}
