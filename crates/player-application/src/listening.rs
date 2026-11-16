//! Actual playback accounting: monotonic elapsed time capped by audio clock
//! progress; seeks reset the sample, pauses add nothing. One row per pass.
use crate::{now, AppResult, PlayerService};
use player_domain::{ListeningEvent, Track};
use serde::Serialize;
use std::{collections::BTreeMap, time::Instant};

#[derive(Default)]
pub(crate) struct ListeningTracker {
    event: Option<ListeningEvent>,
    sample: Option<(Instant, f64, bool)>,
    last_saved: f64,
    pending_elapsed: f64,
    began_at_start: bool,
    had_seek: bool,
}
impl ListeningTracker {
    pub(crate) fn reset_sample(&mut self) {
        self.sample = None;
        self.pending_elapsed = 0.;
    }
    pub(crate) fn mark_seek(&mut self) {
        self.had_seek = true;
        self.reset_sample();
    }
    pub(crate) fn begin_pass(&mut self) {
        self.began_at_start = true;
    }
    /// Native EOF/automatic advance proves a full pass only when the app started
    /// that pass at zero and no seek occurred. mpv can report decoder EOF before
    /// its final audio buffer drains, so a wall-clock cutoff here would miss
    /// valid completions. Manual Next/replacement never calls this method.
    fn complete_naturally(&mut self) {
        if let Some(event) = self.event.as_mut() {
            if !self.had_seek
                && self.began_at_start
                && event.duration > 0.
                && event.listened_seconds < event.duration
            {
                event.listened_seconds = event.duration;
            }
        }
    }
    fn observe(
        &mut self,
        track: &Track,
        position: f64,
        playing: bool,
        duration: f64,
        loop_one: bool,
    ) -> Option<ListeningEvent> {
        let clock = Instant::now();
        let changed = self.event.as_ref().is_some_and(|e| e.track_id != track.id);
        let wrapped = loop_one
            && duration > 0.
            && self.sample.as_ref().is_some_and(|(_, old, running)| {
                *running
                    && playing
                    && position + 0.02 < *old
                    && position <= duration * 0.25
                    && *old >= duration * 0.75
            });
        let finished = if changed || wrapped {
            if wrapped {
                self.complete_naturally();
            }
            let event = self.event.take();
            *self = Self::default();
            if wrapped {
                self.begin_pass();
            }
            event
        } else {
            None
        };
        let event = self.event.get_or_insert_with(|| ListeningEvent {
            id: uuid::Uuid::new_v4().to_string(),
            track_id: track.id.clone(),
            title: track.title.clone(),
            artist: track.artist.clone(),
            album: track.album.clone(),
            path: track.path.clone(),
            started_at: now(),
            listened_seconds: 0.,
            duration,
        });
        event.duration = duration;
        if let Some((last, previous, running)) = self.sample {
            let elapsed = clock.duration_since(last).as_secs_f64();
            let progress = position - previous;
            if running && playing {
                // mpv's audio clock advances in chunks. Carry the intervening
                // wall time until the next clock update instead of counting
                // only the one polling interval in which that update arrives.
                self.pending_elapsed = (self.pending_elapsed + elapsed).min(30.);
                if progress >= 0. && progress <= self.pending_elapsed + 0.5 {
                    let heard = progress.min(self.pending_elapsed);
                    event.listened_seconds += heard;
                    self.pending_elapsed -= heard;
                } else if progress > 0. {
                    self.had_seek = true;
                    self.pending_elapsed = 0.;
                }
            } else {
                self.pending_elapsed = 0.;
            }
        }
        self.sample = Some((clock, position, playing));
        finished
    }
}

#[derive(Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Statistics {
    pub plays: u64,
    pub listened_seconds: f64,
    pub tracks: Vec<TrackStatistics>,
    pub artists: Vec<ArtistStatistics>,
}
#[derive(Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TrackStatistics {
    pub track_id: String,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub path: String,
    pub plays: u64,
    pub listened_seconds: f64,
}
#[derive(Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtistStatistics {
    pub artist: String,
    pub plays: u64,
    pub listened_seconds: f64,
}

impl PlayerService {
    pub(crate) fn observe_listening(&mut self) {
        if !self.state.settings.statistics.enabled {
            self.listening.reset_sample();
            return;
        }
        let Some(track) = self.active().current_entry().map(|e| e.track.clone()) else {
            return;
        };
        let loop_one = self.active().repeat == player_domain::RepeatMode::One;
        if let Some(finished) = self.listening.observe(
            &track,
            self.position,
            self.status == "playing" && !self.awaiting_load,
            self.duration,
            loop_one,
        ) {
            if let Err(error) = self.store.record_listening(&finished) {
                self.warning(error.to_string());
            }
        }
        let should_save = self
            .listening
            .event
            .as_ref()
            .is_some_and(|e| e.listened_seconds - self.listening.last_saved >= 5.);
        if should_save {
            let _ = self.flush_listening();
        }
    }
    pub(crate) fn flush_listening(&mut self) -> AppResult<()> {
        if let Some(event) = self
            .listening
            .event
            .as_ref()
            .filter(|e| e.listened_seconds > 0.)
        {
            self.store.record_listening(event)?;
            self.listening.last_saved = event.listened_seconds;
        }
        Ok(())
    }
    pub(crate) fn finish_listening(&mut self) {
        if let Err(error) = self.flush_listening() {
            self.warning(error.to_string());
        }
        self.listening = ListeningTracker::default();
    }
    pub(crate) fn finish_listening_naturally(&mut self) {
        self.listening.complete_naturally();
        self.finish_listening();
    }
    pub fn statistics(&mut self, from: Option<i64>, to: Option<i64>) -> AppResult<Statistics> {
        if from.zip(to).is_some_and(|(a, b)| a >= b) {
            return Err(crate::AppError::new(
                "invalid_input",
                "Invalid statistics range",
            ));
        }
        self.flush_listening()?;
        let events = self.store.listening_history(from, to)?;
        let mut tracks: BTreeMap<String, TrackStatistics> = BTreeMap::new();
        let mut artists: BTreeMap<String, ArtistStatistics> = BTreeMap::new();
        let mut total = Statistics::default();
        for event in events {
            let plays = u64::from(
                self.state
                    .settings
                    .statistics
                    .qualifies(event.listened_seconds, event.duration),
            );
            total.plays += plays;
            total.listened_seconds += event.listened_seconds;
            let track = tracks.entry(event.track_id.clone()).or_insert_with(|| {
                let current = self.state.tracks.iter().find(|t| t.id == event.track_id);
                TrackStatistics {
                    track_id: event.track_id.clone(),
                    title: current
                        .map(|t| t.title.clone())
                        .unwrap_or_else(|| event.title.clone()),
                    artist: current
                        .map(|t| t.artist.clone())
                        .unwrap_or_else(|| event.artist.clone()),
                    album: current
                        .map(|t| t.album.clone())
                        .unwrap_or_else(|| event.album.clone()),
                    path: current
                        .map(|t| t.path.clone())
                        .unwrap_or_else(|| event.path.clone()),
                    ..Default::default()
                }
            });
            track.plays += plays;
            track.listened_seconds += event.listened_seconds;
            let artist = artists
                .entry(track.artist.clone())
                .or_insert_with(|| ArtistStatistics {
                    artist: track.artist.clone(),
                    ..Default::default()
                });
            artist.plays += plays;
            artist.listened_seconds += event.listened_seconds;
        }
        total.tracks = tracks.into_values().collect();
        total.artists = artists.into_values().collect();
        total.tracks.sort_by(|a, b| {
            b.plays
                .cmp(&a.plays)
                .then_with(|| b.listened_seconds.total_cmp(&a.listened_seconds))
                .then_with(|| a.title.cmp(&b.title))
        });
        total.artists.sort_by(|a, b| {
            b.plays
                .cmp(&a.plays)
                .then_with(|| b.listened_seconds.total_cmp(&a.listened_seconds))
                .then_with(|| a.artist.cmp(&b.artist))
        });
        Ok(total)
    }
    pub fn statistics_years(&mut self) -> AppResult<Vec<i32>> {
        self.flush_listening()?;
        self.store.listening_years()
    }
    pub fn clear_statistics(&mut self) -> AppResult<()> {
        self.store.clear_listening()?;
        self.listening = ListeningTracker::default();
        Ok(())
    }
    pub fn export_history(&mut self) -> AppResult<serde_json::Value> {
        self.flush_listening()?;
        Ok(
            serde_json::json!({"format":"song-player-listening-history","version":1,"exportedAt":now()*1000,
            "threshold":self.state.settings.statistics,"events":self.store.listening_history(None,None)?}),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn pause_seek_repeat_and_elapsed_accounting() {
        let track = Track {
            id: "a".into(),
            duration: 120.,
            ..Default::default()
        };
        let mut tracker = ListeningTracker::default();
        tracker.observe(&track, 0., true, 120., true);
        tracker.sample = Some((
            Instant::now() - std::time::Duration::from_secs(10),
            0.,
            true,
        ));
        tracker.observe(&track, 10., true, 120., true);
        assert!((tracker.event.as_ref().unwrap().listened_seconds - 10.).abs() < 0.01);
        tracker.sample = Some((
            Instant::now() - std::time::Duration::from_secs(10),
            10.,
            false,
        ));
        tracker.observe(&track, 10., false, 120., true);
        assert_eq!(tracker.event.as_ref().unwrap().listened_seconds, 10.);
        tracker.reset_sample();
        tracker.observe(&track, 90., true, 120., true);
        assert_eq!(tracker.event.as_ref().unwrap().listened_seconds, 10.);
        let finished = tracker.observe(&track, 0., true, 120., true).unwrap();
        assert_eq!(finished.listened_seconds, 10.);
        assert_ne!(tracker.event.as_ref().unwrap().id, finished.id);
    }
    #[test]
    fn natural_completion_requires_a_known_full_pass_and_rejects_seeks_or_partial_recording() {
        let track = Track {
            id: "a".into(),
            duration: 2.,
            ..Default::default()
        };
        let mut tracker = ListeningTracker::default();
        tracker.begin_pass();
        tracker.observe(&track, 0.02, true, 2., false);
        tracker.event.as_mut().unwrap().listened_seconds = 1.95;
        tracker.sample = Some((
            Instant::now() - std::time::Duration::from_millis(30),
            1.98,
            true,
        ));
        tracker.complete_naturally();
        assert_eq!(tracker.event.as_ref().unwrap().listened_seconds, 2.);
        tracker.event.as_mut().unwrap().listened_seconds = 0.1;
        tracker.mark_seek();
        tracker.complete_naturally();
        assert_eq!(tracker.event.as_ref().unwrap().listened_seconds, 0.1);
        tracker.had_seek = false;
        tracker.began_at_start = false;
        tracker.sample = Some((
            Instant::now() - std::time::Duration::from_secs(10),
            0.1,
            false,
        ));
        tracker.complete_naturally();
        assert_eq!(tracker.event.as_ref().unwrap().listened_seconds, 0.1);
    }
}
