//! Library annotations and preferences independent from scanner metadata.
use crate::{now, valid_name, AppError, AppResult, PlayerService};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum GainGrouping {
    #[default]
    Track,
    Album,
    Folder,
}

/// Shared host/core grouping rules, including a folder fallback for untagged albums.
pub fn loudness_group_key(track: &player_domain::Track, grouping: GainGrouping) -> String {
    let parent = std::path::Path::new(&track.path)
        .parent()
        .unwrap_or(std::path::Path::new(""));
    let path = std::fs::canonicalize(parent)
        .unwrap_or_else(|_| parent.to_path_buf())
        .to_string_lossy()
        .into_owned();
    let folder = if cfg!(windows) {
        if let Some(unc) = path.strip_prefix(r"\\?\UNC\") {
            format!(r"\\{unc}").to_lowercase()
        } else {
            path.strip_prefix(r"\\?\").unwrap_or(&path).to_lowercase()
        }
    } else {
        path
    };
    match grouping {
        GainGrouping::Track => track.id.clone(),
        GainGrouping::Folder => folder,
        GainGrouping::Album => {
            if track.album.trim().is_empty() {
                format!("folder:{folder}")
            } else {
                format!(
                    "{}\u{1f}{}",
                    if track.album_artist.trim().is_empty() {
                        &track.artist
                    } else {
                        &track.album_artist
                    }
                    .to_lowercase(),
                    track.album.to_lowercase()
                )
            }
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default, deny_unknown_fields)]
pub struct StatisticsSettings {
    pub enabled: bool,
    pub percent: f64,
    pub seconds: f64,
}
impl Default for StatisticsSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            percent: 50.,
            seconds: 240.,
        }
    }
}
impl StatisticsSettings {
    pub fn validate(&self) -> AppResult<()> {
        if !self.percent.is_finite()
            || !(1. ..=100.).contains(&self.percent)
            || !self.seconds.is_finite()
            || !(0. ..=3600.).contains(&self.seconds)
        {
            return Err(AppError::new(
                "invalid_input",
                "Invalid listening threshold",
            ));
        }
        Ok(())
    }
    pub fn qualifies(&self, listened: f64, duration: f64) -> bool {
        (duration > 0. && listened >= duration * self.percent / 100.)
            || (self.seconds > 0. && listened >= self.seconds)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default, deny_unknown_fields)]
pub struct EqualizerSettings {
    pub enabled: bool,
    pub bands: [f64; 5],
    pub preamp: f64,
}
impl Default for EqualizerSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            bands: [0.; 5],
            preamp: 0.,
        }
    }
}
impl EqualizerSettings {
    pub fn validate(&self) -> AppResult<()> {
        if self
            .bands
            .iter()
            .any(|n| !n.is_finite() || !(-12. ..=12.).contains(n))
            || !self.preamp.is_finite()
            || !(-24. ..=0.).contains(&self.preamp)
        {
            return Err(AppError::new("invalid_input", "Invalid equalizer values"));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default, deny_unknown_fields)]
pub struct GlobalSearchSettings {
    pub enabled: bool,
    pub binding: String,
}
impl Default for GlobalSearchSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            binding: "Ctrl+Alt+Space".into(),
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PlaylistIcon {
    pub kind: String,
    pub value: String,
}
impl Default for PlaylistIcon {
    fn default() -> Self {
        Self {
            kind: "builtin".into(),
            value: "music".into(),
        }
    }
}
impl PlaylistIcon {
    pub fn validate(&self) -> AppResult<()> {
        let valid = match self.kind.as_str() {
            "builtin" => [
                "music",
                "headphones",
                "heart",
                "star",
                "disc",
                "folder",
                "moon",
                "sun",
                "zap",
                "coffee",
            ]
            .contains(&self.value.as_str()),
            // The host copies/imports verified image files and grants asset access.
            "image" => {
                self.value.len() <= 32768
                    && !self.value.chars().any(char::is_control)
                    && std::path::Path::new(&self.value).is_absolute()
                    && std::path::Path::new(&self.value)
                        .extension()
                        .and_then(|s| s.to_str())
                        .is_some_and(|s| {
                            ["png", "jpg", "jpeg", "webp"]
                                .contains(&s.to_ascii_lowercase().as_str())
                        })
            }
            _ => false,
        };
        if valid {
            Ok(())
        } else {
            Err(AppError::new("invalid_input", "Invalid playlist icon"))
        }
    }
}
pub(crate) fn legacy_pinned() -> bool {
    true
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LoudnessValue {
    pub track_id: String,
    pub gain: f64,
    pub peak: f64,
    pub grouping: GainGrouping,
    pub group_key: Option<String>,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SleepMode {
    Minutes,
    Track,
    Queue,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SleepTimer {
    pub mode: SleepMode,
    pub ends_at: Option<i64>,
}

impl PlayerService {
    pub(crate) fn relink_scanned_tracks(&mut self, tracks: &mut [player_domain::Track]) {
        let mut incoming = std::collections::HashMap::<String, usize>::new();
        for t in tracks.iter() {
            if let Some(hash) = &t.content_fingerprint {
                *incoming.entry(hash.clone()).or_default() += 1;
            }
        }
        let incoming_ids: HashSet<_> = tracks.iter().map(|t| t.id.clone()).collect();
        for track in tracks {
            // IDs deliberately survive a move; later path-based scanner IDs
            // therefore need to resolve through the remembered current path.
            let same_path: Vec<_> = self
                .state
                .tracks
                .iter()
                .filter(|old| {
                    if cfg!(windows) {
                        old.path.eq_ignore_ascii_case(&track.path)
                    } else {
                        old.path == track.path
                    }
                })
                .collect();
            if same_path.len() == 1 {
                track.id = same_path[0].id.clone();
            }
            if !self.state.tracks.iter().any(|old| old.id == track.id) {
                if let Some(hash) = track
                    .content_fingerprint
                    .as_ref()
                    .filter(|h| incoming.get(*h) == Some(&1))
                {
                    let candidates: Vec<_> = self
                        .state
                        .tracks
                        .iter()
                        .filter(|old| {
                            !incoming_ids.contains(&old.id)
                                && old.content_fingerprint.as_ref() == Some(hash)
                                && !std::path::Path::new(&old.path).exists()
                        })
                        .collect();
                    if candidates.len() == 1 {
                        track.id = candidates[0].id.clone();
                        track.added_at = candidates[0].added_at;
                    }
                }
            }
            if let Some(old) = self
                .state
                .tracks
                .iter()
                .find(|old| old.id == track.id)
                .cloned()
            {
                track.added_at = old.added_at;
                // Existing analysis is invalid if bytes at a path changed.
                let changed = old.content_fingerprint.is_some()
                    && track.content_fingerprint.is_some()
                    && old.content_fingerprint != track.content_fingerprint;
                if changed {
                    self.state
                        .loudness_values
                        .retain(|v| v.track_id != track.id);
                }
                for grouping in [GainGrouping::Album, GainGrouping::Folder] {
                    let previous_key = loudness_group_key(&old, grouping);
                    let next_key = loudness_group_key(track, grouping);
                    if changed || previous_key != next_key {
                        self.state.loudness_values.retain(|v| {
                            v.grouping != grouping
                                || v.group_key
                                    .as_ref()
                                    .is_none_or(|key| key != &previous_key && key != &next_key)
                        });
                    }
                }
            } else {
                // A group result describes its complete membership. A newly
                // discovered member makes that cached aggregate stale.
                for grouping in [GainGrouping::Album, GainGrouping::Folder] {
                    let key = loudness_group_key(track, grouping);
                    self.state
                        .loudness_values
                        .retain(|v| v.grouping != grouping || v.group_key.as_ref() != Some(&key));
                }
            }
            for session in &mut self.state.sessions {
                session.refresh_track(track);
            }
        }
    }
    /// Host-only metadata refresh after writing optional tags. This neither
    /// changes library membership nor starts/replaces the playback queue.
    pub fn refresh_scanned_metadata(
        &mut self,
        mut tracks: Vec<player_domain::Track>,
    ) -> AppResult<()> {
        let previous = self.state.clone();
        self.relink_scanned_tracks(&mut tracks);
        for track in tracks {
            if let Some(old) = self.state.tracks.iter_mut().find(|old| old.id == track.id) {
                *old = track;
            }
        }
        if let Err(error) = self.apply_current_gain().and_then(|_| self.persist()) {
            self.state = previous;
            let _ = self.apply_current_gain();
            return Err(error);
        }
        self.revision += 1;
        Ok(())
    }
    pub(crate) fn library_mutation(
        &mut self,
        operation: impl FnOnce(&mut crate::SavedState) -> AppResult<()>,
    ) -> AppResult<()> {
        let previous = self.state.clone();
        if let Err(error) = operation(&mut self.state) {
            self.state = previous;
            return Err(error);
        }
        if let Err(error) = self.persist() {
            self.state = previous;
            return Err(error);
        }
        Ok(())
    }
    pub(crate) fn create_playlist(&mut self, name: String) -> AppResult<()> {
        let name = valid_name(name)?;
        self.library_mutation(|state| {
            state.playlists.push(crate::Playlist {
                id: uuid::Uuid::new_v4().to_string(),
                name,
                track_ids: vec![],
                created_at: now(),
                pinned: false,
                icon: PlaylistIcon::default(),
            });
            Ok(())
        })
    }
    pub(crate) fn set_playlist_membership(
        &mut self,
        track_ids: Vec<String>,
        playlist_ids: Vec<String>,
        preserve_playlist_ids: Vec<String>,
    ) -> AppResult<()> {
        let tracks: HashSet<_> = track_ids.into_iter().collect();
        let playlists: HashSet<_> = playlist_ids.into_iter().collect();
        let preserve: HashSet<_> = preserve_playlist_ids.into_iter().collect();
        if tracks.is_empty()
            || tracks
                .iter()
                .any(|id| !self.state.tracks.iter().any(|t| &t.id == id))
            || playlists
                .union(&preserve)
                .any(|id| !self.state.playlists.iter().any(|p| &p.id == id))
            || !playlists.is_disjoint(&preserve)
        {
            return Err(AppError::new("not_found", "Unknown track or playlist"));
        }
        // Preserve existing order; append new selected members in library order.
        let ordered: Vec<_> = self
            .state
            .tracks
            .iter()
            .filter(|t| tracks.contains(&t.id))
            .map(|t| t.id.clone())
            .collect();
        self.library_mutation(|state| {
            for p in &mut state.playlists {
                if preserve.contains(&p.id) {
                    continue;
                }
                if playlists.contains(&p.id) {
                    let mut seen = HashSet::new();
                    p.track_ids
                        .retain(|id| !tracks.contains(id) || seen.insert(id.clone()));
                    for id in &ordered {
                        if !p.track_ids.contains(id) {
                            p.track_ids.push(id.clone());
                        }
                    }
                } else {
                    p.track_ids.retain(|id| !tracks.contains(id));
                }
            }
            Ok(())
        })
    }
    pub(crate) fn set_favorite(&mut self, track_ids: Vec<String>, favorite: bool) -> AppResult<()> {
        if track_ids.is_empty()
            || track_ids
                .iter()
                .any(|id| !self.state.tracks.iter().any(|t| &t.id == id))
        {
            return Err(AppError::new("not_found", "Unknown track"));
        }
        self.library_mutation(|state| {
            for id in track_ids {
                if favorite {
                    if !state.favorite_track_ids.contains(&id) {
                        state.favorite_track_ids.push(id);
                    }
                } else {
                    state.favorite_track_ids.retain(|t| t != &id);
                }
            }
            Ok(())
        })
    }
    pub(crate) fn update_loudness(&mut self, values: Vec<LoudnessValue>) -> AppResult<()> {
        let mut seen = HashSet::new();
        if values.is_empty()
            || values.iter().any(|v| {
                !v.gain.is_finite()
                    || !(-80. ..=80.).contains(&v.gain)
                    || !v.peak.is_finite()
                    || !(0. ..=100.).contains(&v.peak)
                    || v.group_key.as_ref().is_some_and(|s| {
                        s.len() > 32768 || s.chars().any(|c| c.is_control() && c != '\u{1f}')
                    })
                    || !seen.insert((v.track_id.clone(), v.grouping as u8))
                    || !self.state.tracks.iter().any(|t| t.id == v.track_id)
            })
        {
            return Err(AppError::new(
                "invalid_input",
                "Invalid loudness analysis results",
            ));
        }
        let previous = self.state.loudness_values.clone();
        for value in values {
            self.state
                .loudness_values
                .retain(|v| v.track_id != value.track_id || v.grouping != value.grouping);
            self.state.loudness_values.push(value);
        }
        if let Err(error) = self.apply_current_gain().and_then(|_| self.persist()) {
            self.state.loudness_values = previous;
            let _ = self.apply_current_gain();
            return Err(error);
        }
        Ok(())
    }
}
