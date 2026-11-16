use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Track {
    pub id: String,
    pub path: String,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub album_artist: String,
    pub genre: String,
    pub year: Option<u32>,
    pub track_number: Option<u32>,
    pub disc_number: Option<u32>,
    pub duration: f64,
    pub sample_rate: Option<u32>,
    pub channels: Option<u16>,
    pub bitrate: Option<u32>,
    pub format: String,
    pub cover_path: Option<String>,
    pub added_at: i64,
    /// Content-derived identity evidence used only for unambiguous moved-file
    /// relinking. Never use editable metadata to merge distinct recordings.
    pub content_fingerprint: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct QueueEntry {
    pub id: String,
    pub track: Track,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RepeatMode {
    #[default]
    Off,
    One,
    All,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PlayOrder {
    #[default]
    Manual,
    ShuffleTracks,
    ShuffleAlbums,
    ShuffleFolders,
    Title,
    Artist,
    Album,
    Filename,
    Year,
    Duration,
    Added,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SessionSnapshot {
    pub id: String,
    pub name: String,
    pub entries: Vec<QueueEntry>,
    pub current_entry_id: Option<String>,
    pub position: f64,
    pub repeat: RepeatMode,
    pub order_mode: PlayOrder,
}
