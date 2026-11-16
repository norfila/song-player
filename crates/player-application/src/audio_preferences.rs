use crate::{AppResult, EqualizerSettings, GainGrouping, PlayerService, SleepMode};

const EQUALIZER_LABEL: &str = "@sonora-equalizer";
const GAIN_LABEL: &str = "@sonora-replaygain";

impl PlayerService {
    pub(crate) fn apply_equalizer(&mut self, value: &EqualizerSettings) -> AppResult<()> {
        value.validate()?;
        let Some(engine) = self.audio.as_mut() else {
            return Ok(());
        };
        if !value.enabled {
            // Removing an absent named filter is an mpv error: query first.
            if engine
                .get_property("af")
                .is_some_and(|s| s.contains("sonora-equalizer"))
            {
                engine.command(&["af", "remove", EQUALIZER_LABEL])?;
            }
            return Ok(());
        }
        let protection = value.bands.iter().copied().fold(0f64, f64::max);
        let preamp = value.preamp.min(-protection);
        let mut filters = vec![format!("volume={preamp:.4}dB")];
        for (frequency, gain) in [60, 250, 1000, 4000, 12000].into_iter().zip(value.bands) {
            filters.push(format!("equalizer=f={frequency}:t=o:w=2:g={gain:.4}"));
        }
        let filter = format!("{EQUALIZER_LABEL}:lavfi=[{}]", filters.join(","));
        engine.command(&["af", "add", &filter])
    }
    pub(crate) fn apply_current_gain(&mut self) -> AppResult<()> {
        self.apply_gain_preferences(&self.state.settings.clone())
    }
    pub(crate) fn apply_gain_preferences(&mut self, settings: &crate::Settings) -> AppResult<()> {
        let grouping = settings.gain_grouping;
        let current = self.active().current_entry().map(|e| &e.track);
        let analyzed = if settings.replay_gain {
            current
                .and_then(|track| {
                    self.state.loudness_values.iter().find(|v| {
                        v.track_id == track.id
                            && v.grouping == grouping
                            && (grouping == GainGrouping::Track
                                || v.group_key.as_ref().is_some_and(|key| {
                                    if cfg!(windows) && grouping == GainGrouping::Folder {
                                        key.eq_ignore_ascii_case(&crate::loudness_group_key(
                                            track, grouping,
                                        ))
                                    } else {
                                        key == &crate::loudness_group_key(track, grouping)
                                    }
                                }))
                    })
                })
                .cloned()
        } else {
            None
        };
        let native = if !settings.replay_gain || analyzed.is_some() {
            "no"
        } else {
            match grouping {
                GainGrouping::Track => "track",
                GainGrouping::Album => "album",
                GainGrouping::Folder => "no",
            }
        };
        let Some(engine) = self.audio.as_mut() else {
            return Ok(());
        };
        engine.set_property("replaygain", native)?;
        if let Some(value) = analyzed {
            // Clamp gain against the measured peak to avoid analysis-induced clipping.
            let gain = if value.peak > 0. {
                value.gain.min(-20. * value.peak.log10())
            } else {
                value.gain
            };
            engine.command(&[
                "af",
                "add",
                &format!("{GAIN_LABEL}:lavfi=[volume={gain:.4}dB]"),
            ])?;
        } else if engine
            .get_property("af")
            .is_some_and(|s| s.contains("sonora-replaygain"))
        {
            engine.command(&["af", "remove", GAIN_LABEL])?;
        }
        Ok(())
    }
    pub(crate) fn apply_sleep_loop(&mut self) -> AppResult<()> {
        let one = self.active().repeat == player_domain::RepeatMode::One
            && !self
                .sleep_timer
                .as_ref()
                .is_some_and(|t| matches!(t.mode, SleepMode::Track | SleepMode::Queue));
        if let Some(engine) = self.audio.as_mut() {
            engine.set_property("loop-file", if one { "inf" } else { "no" })?;
        }
        Ok(())
    }
    pub(crate) fn expire_sleep_timer(&mut self) -> bool {
        if self
            .sleep_deadline
            .is_some_and(|deadline| std::time::Instant::now() >= deadline)
        {
            self.finish_sleep_timer();
            return true;
        }
        false
    }
    pub(crate) fn finish_sleep_timer(&mut self) {
        if let Some(engine) = self.audio.as_mut() {
            if !self.engine_entries.is_empty() {
                let _ = engine.set_property("pause", "yes");
            }
        }
        if self.status == "playing" {
            self.status = "paused".into();
        }
        self.sleep_timer = None;
        self.sleep_deadline = None;
        let _ = self.apply_sleep_loop();
        self.revision += 1;
    }
}
