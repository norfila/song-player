//! Native adapters. No Tauri or application dependency belongs in this crate.

mod audio_tools;
mod error;
mod mpv;
mod playlist;
mod scanner;
mod store;

pub use audio_tools::{write_gain_tags, AudioTools, GainResult, FFMPEG_WINDOWS_SHA256};
pub use error::InfraError;
pub use mpv::{AudioDevice, EngineEvent, MpvEngine};
pub use playlist::{read_m3u8, write_m3u8};
pub use scanner::{is_audio_path, scan_paths, ScanResult};
pub use store::Store;

/// Stable names for app-owned media assets; no paths from the source filename
/// are used when importing an image.
pub fn content_sha256(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    format!("{:x}", Sha256::digest(bytes))
}
