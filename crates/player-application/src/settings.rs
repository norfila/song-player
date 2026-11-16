//! Validated, portable appearance and settings documents.
use crate::{
    now, validate_shortcuts, AppError, AppResult, EqualizerSettings, GainGrouping,
    GlobalSearchSettings, PanelWidths, PlayerService, SeekbarStyle, Settings, StatisticsSettings,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{BTreeMap, HashSet};

pub const MAX_CONFIG_BYTES: usize = 2 * 1024 * 1024;
pub const SETTINGS_SECTIONS: [&str; 7] = [
    "general",
    "appearance",
    "audio",
    "shortcuts",
    "layout",
    "device",
    "library",
];

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ThemeColors {
    pub background: String,
    pub panel: String,
    pub surface: String,
    pub surface_hover: String,
    pub border: String,
    pub text: String,
    pub secondary: String,
    pub muted: String,
    pub accent: String,
    pub accent_text: String,
    pub sidebar: String,
    pub player: String,
    pub selection: String,
    pub success: String,
    pub error: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ThemeDefinition {
    pub version: u32,
    pub id: String,
    pub name: String,
    pub base: String,
    pub colors: ThemeColors,
    pub font_family: String,
    pub font_scale: f64,
    pub radius: f64,
    pub spacing: f64,
    pub shadows: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AppearanceSettings {
    pub active_theme_id: String,
    pub custom_themes: Vec<ThemeDefinition>,
}
impl Default for AppearanceSettings {
    fn default() -> Self {
        Self {
            active_theme_id: "dark".into(),
            custom_themes: vec![],
        }
    }
}

fn invalid(details: impl ToString) -> AppError {
    AppError::new("invalid_input", details)
}
fn finite_range(value: f64, min: f64, max: f64) -> bool {
    value.is_finite() && (min..=max).contains(&value)
}
pub fn validate_appearance(value: AppearanceSettings) -> AppResult<AppearanceSettings> {
    if value.custom_themes.len() > 32 {
        return Err(invalid("At most 32 custom themes are supported"));
    }
    let mut ids: HashSet<&str> = ["light", "dark", "gold"].into_iter().collect();
    for theme in &value.custom_themes {
        if theme.version != 1
            || theme.id.is_empty()
            || theme.id.len() > 64
            || !theme
                .id
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
            || !ids.insert(&theme.id)
            || theme.name.trim().is_empty()
            || theme.name.chars().count() > 80
            || theme.name.chars().any(char::is_control)
            || !["light", "dark", "gold"].contains(&theme.base.as_str())
            || !["system", "sans", "serif", "mono"].contains(&theme.font_family.as_str())
            || !finite_range(theme.font_scale, 0.85, 1.3)
            || !finite_range(theme.radius, 0., 20.)
            || !finite_range(theme.spacing, 0.8, 1.3)
            || !finite_range(theme.shadows, 0., 1.)
        {
            return Err(invalid("Invalid custom theme"));
        }
        let colors = &theme.colors;
        for color in [
            &colors.background,
            &colors.panel,
            &colors.surface,
            &colors.surface_hover,
            &colors.border,
            &colors.text,
            &colors.secondary,
            &colors.muted,
            &colors.accent,
            &colors.accent_text,
            &colors.sidebar,
            &colors.player,
            &colors.selection,
            &colors.success,
            &colors.error,
        ] {
            if color.len() != 7
                || !color.starts_with('#')
                || !color.as_bytes()[1..].iter().all(u8::is_ascii_hexdigit)
            {
                return Err(invalid("Theme colors must use #RRGGBB"));
            }
        }
    }
    if !ids.contains(value.active_theme_id.as_str()) {
        return Err(invalid("Unknown active theme"));
    }
    Ok(value)
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsImportUndo {
    pub settings: Settings,
    pub library_folders: Vec<String>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct General {
    language: String,
    close_to_tray: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    open_external_now_playing: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    suggest_external_folders: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    statistics: Option<StatisticsSettings>,
}
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Audio {
    volume: f64,
    muted: bool,
    replay_gain: bool,
    loudness_normalization: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    equalizer: Option<EqualizerSettings>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    gain_grouping: Option<GainGrouping>,
}
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Layout {
    panel_widths: PanelWidths,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    seekbar_style: Option<SeekbarStyle>,
}
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Device {
    output_device: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    global_search: Option<GlobalSearchSettings>,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Library {
    folders: Vec<String>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Document {
    format: String,
    version: u32,
    exported_at: i64,
    sections: BTreeMap<String, Value>,
}

fn selected_sections(sections: &[String]) -> AppResult<()> {
    let mut unique = HashSet::new();
    if sections.is_empty()
        || sections.len() > SETTINGS_SECTIONS.len()
        || sections
            .iter()
            .any(|s| !SETTINGS_SECTIONS.contains(&s.as_str()) || !unique.insert(s))
    {
        return Err(invalid("Select one or more supported settings sections"));
    }
    Ok(())
}
fn decode<T: serde::de::DeserializeOwned>(value: &Value) -> AppResult<T> {
    serde_json::from_value(value.clone()).map_err(invalid)
}

impl PlayerService {
    pub fn export_settings_document(&self, selected: Vec<String>) -> AppResult<Value> {
        selected_sections(&selected)?;
        let s = &self.state.settings;
        let mut sections = serde_json::Map::new();
        for section in selected {
            let value = match section.as_str() {
                "general" => json!(General {
                    language: s.language.clone(),
                    close_to_tray: s.close_to_tray,
                    open_external_now_playing: Some(s.open_external_now_playing),
                    suggest_external_folders: Some(s.suggest_external_folders),
                    statistics: Some(s.statistics.clone())
                }),
                "appearance" => json!(s.appearance),
                "audio" => json!(Audio {
                    volume: s.volume,
                    muted: s.muted,
                    replay_gain: s.replay_gain,
                    loudness_normalization: s.loudness_normalization,
                    equalizer: Some(s.equalizer.clone()),
                    gain_grouping: Some(s.gain_grouping)
                }),
                "shortcuts" => json!(s.shortcuts),
                "layout" => json!(Layout {
                    panel_widths: s.panel_widths.clone(),
                    seekbar_style: Some(s.seekbar_style),
                }),
                "device" => json!(Device {
                    output_device: s.output_device.clone(),
                    global_search: Some(s.global_search.clone())
                }),
                "library" => json!(Library {
                    folders: self.state.library_folders.clone()
                }),
                _ => unreachable!(),
            };
            sections.insert(section, value);
        }
        Ok(
            json!({ "format": "song-player-settings", "version": 1, "exportedAt": now() * 1000, "sections": sections }),
        )
    }

    pub(crate) fn import_settings(
        &mut self,
        document: Value,
        selected: Vec<String>,
    ) -> AppResult<()> {
        selected_sections(&selected)?;
        if serde_json::to_vec(&document).map_err(invalid)?.len() > MAX_CONFIG_BYTES {
            return Err(invalid("Settings document exceeds 2 MiB"));
        }
        let document: Document = decode(&document)?;
        if document.format != "song-player-settings"
            || document.version != 1
            || document.exported_at < 0
            || document
                .sections
                .keys()
                .any(|s| !SETTINGS_SECTIONS.contains(&s.as_str()))
        {
            return Err(invalid("Unsupported settings document"));
        }
        let mut settings = self.state.settings.clone();
        let mut folders = self.state.library_folders.clone();
        for section in selected {
            let value = document
                .sections
                .get(&section)
                .ok_or_else(|| invalid("Selected section is missing"))?;
            match section.as_str() {
                "general" => {
                    let value: General = decode(value)?;
                    if !["de", "en"].contains(&value.language.as_str()) {
                        return Err(invalid("Unsupported language"));
                    }
                    settings.language = value.language;
                    settings.close_to_tray = value.close_to_tray;
                    if let Some(v) = value.open_external_now_playing {
                        settings.open_external_now_playing = v;
                    }
                    if let Some(v) = value.suggest_external_folders {
                        settings.suggest_external_folders = v;
                    }
                    if let Some(v) = value.statistics {
                        v.validate()?;
                        settings.statistics = v;
                    }
                }
                "appearance" => settings.appearance = validate_appearance(decode(value)?)?,
                "audio" => {
                    let value: Audio = decode(value)?;
                    if !finite_range(value.volume, 0., 100.) {
                        return Err(invalid("Volume must be between 0 and 100"));
                    }
                    settings.volume = value.volume;
                    settings.muted = value.muted;
                    settings.replay_gain = value.replay_gain;
                    settings.loudness_normalization = value.loudness_normalization;
                    if let Some(v) = value.equalizer {
                        v.validate()?;
                        settings.equalizer = v;
                    }
                    if let Some(v) = value.gain_grouping {
                        settings.gain_grouping = v;
                    }
                }
                "shortcuts" => settings.shortcuts = validate_shortcuts(decode(value)?)?,
                "layout" => {
                    let value: Layout = decode(value)?;
                    if !finite_range(value.panel_widths.sidebar, 160., 360.)
                        || !finite_range(value.panel_widths.queue, 240., 560.)
                    {
                        return Err(invalid("Invalid panel widths"));
                    }
                    settings.panel_widths = value.panel_widths;
                    if let Some(style) = value.seekbar_style {
                        settings.seekbar_style = style;
                    }
                }
                "device" => {
                    let value: Device = decode(value)?;
                    if value.output_device.trim().is_empty()
                        || value.output_device.len() > 1024
                        || value.output_device.chars().any(char::is_control)
                    {
                        return Err(invalid("Invalid audio device"));
                    }
                    settings.output_device = value.output_device;
                    if let Some(v) = value.global_search {
                        if !crate::valid_shortcut(&v.binding)
                            || v.binding.is_empty()
                            || v.binding == "Ctrl+KeyF"
                            || !["Ctrl+", "Alt+"].iter().any(|m| v.binding.contains(m))
                        {
                            return Err(invalid("Invalid global search shortcut"));
                        }
                        settings.global_search = v;
                    }
                }
                "library" => {
                    let value: Library = decode(value)?;
                    if value.folders.len() > 256
                        || value.folders.iter().any(|path| {
                            path.trim().is_empty()
                                || path.len() > 32768
                                || path.chars().any(char::is_control)
                        })
                    {
                        return Err(invalid("Invalid library folder definitions"));
                    }
                    for folder in value.folders {
                        if !folders.contains(&folder) {
                            folders.push(folder);
                        }
                    }
                    if folders.len() > 256 {
                        return Err(invalid("At most 256 library folders are supported"));
                    }
                }
                _ => unreachable!(),
            }
        }
        self.replace_imported_settings(settings, folders, false)
    }

    pub(crate) fn undo_settings_import(&mut self) -> AppResult<()> {
        let previous = self
            .state
            .settings_import_undo
            .clone()
            .ok_or_else(|| invalid("No settings import to undo"))?;
        self.replace_imported_settings(previous.settings, previous.library_folders, true)
    }

    fn replace_imported_settings(
        &mut self,
        settings: Settings,
        folders: Vec<String>,
        undo: bool,
    ) -> AppResult<()> {
        let previous = SettingsImportUndo {
            settings: self.state.settings.clone(),
            library_folders: self.state.library_folders.clone(),
        };
        let previous_undo = self.state.settings_import_undo.clone();
        let collection_changed =
            settings.statistics.enabled != previous.settings.statistics.enabled;
        if let Err(error) = self.apply_audio_preferences(&settings, &previous.settings) {
            let _ = self.apply_audio_preferences(&previous.settings, &settings);
            return Err(error);
        }
        self.state.settings = settings;
        self.state.library_folders = folders;
        self.state.settings_import_undo = if undo { None } else { Some(previous.clone()) };
        if let Err(error) = self.persist() {
            let _ = self.apply_audio_preferences(&previous.settings, &self.state.settings.clone());
            self.state.settings = previous.settings;
            self.state.library_folders = previous.library_folders;
            self.state.settings_import_undo = previous_undo;
            return Err(error);
        }
        if collection_changed {
            self.finish_listening();
        }
        Ok(())
    }

    fn apply_audio_preferences(&mut self, target: &Settings, previous: &Settings) -> AppResult<()> {
        let Some(engine) = self.audio.as_mut() else {
            return Ok(());
        };
        if target.volume != previous.volume {
            engine.set_property("volume", &target.volume.to_string())?;
        }
        if target.muted != previous.muted {
            engine.set_property("mute", if target.muted { "yes" } else { "no" })?;
        }
        if target.output_device != previous.output_device {
            engine.set_property("audio-device", &target.output_device)?;
        }
        if target.loudness_normalization != previous.loudness_normalization {
            engine.command(if target.loudness_normalization {
                &["af", "add", crate::NORMALIZATION_FILTER]
            } else {
                &["af", "remove", crate::NORMALIZATION_LABEL]
            })?;
        }
        if target.equalizer != previous.equalizer {
            self.apply_equalizer(&target.equalizer)?;
        }
        if target.replay_gain != previous.replay_gain
            || target.gain_grouping != previous.gain_grouping
        {
            self.apply_gain_preferences(target)?;
        }
        Ok(())
    }
}
