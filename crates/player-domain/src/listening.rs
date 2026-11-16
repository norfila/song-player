//! Persisted listening passes. History is separate from the UI/state snapshot.
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ListeningEvent {
    pub id: String,
    pub track_id: String,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub path: String,
    pub started_at: i64,
    pub listened_seconds: f64,
    pub duration: f64,
}
