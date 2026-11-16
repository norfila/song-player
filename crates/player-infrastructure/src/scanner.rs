use crate::InfraError;
use lofty::{
    file::{AudioFile, TaggedFileExt},
    picture::{MimeType, PictureType},
    probe::Probe,
    tag::{Accessor, ItemKey},
};
use player_domain::Track;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::{HashMap, HashSet},
    fs,
    io::Read,
    path::Path,
    sync::{Mutex, OnceLock},
    time::{SystemTime, UNIX_EPOCH},
};
use uuid::Uuid;
use walkdir::WalkDir;

const MAX_COVER_BYTES: usize = 20 * 1024 * 1024;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanResult {
    pub tracks: Vec<Track>,
    pub warnings: Vec<String>,
    /// Explicitly selected roots, suitable for a later rescan/file watcher.
    pub folders: Vec<String>,
}

pub fn is_audio_path(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            matches!(
                extension.to_ascii_lowercase().as_str(),
                "aac"
                    | "ac3"
                    | "aif"
                    | "aiff"
                    | "aifc"
                    | "alac"
                    | "ape"
                    | "au"
                    | "caf"
                    | "dff"
                    | "dsf"
                    | "flac"
                    | "m4a"
                    | "m4b"
                    | "mka"
                    | "mp2"
                    | "mp3"
                    | "mp4"
                    | "mpc"
                    | "oga"
                    | "ogg"
                    | "opus"
                    | "spx"
                    | "tak"
                    | "tta"
                    | "wav"
                    | "wave"
                    | "wma"
                    | "wv"
            )
        })
}

/// Blocking scanner: callers run this outside the UI/coordinator lock.
/// A metadata parser failure retains a playable file with filename fallback;
/// libmpv supports additional formats that Lofty intentionally does not parse.
pub fn scan_paths(
    paths: &[String],
    recursive: bool,
    cover_dir: &Path,
) -> Result<ScanResult, InfraError> {
    let mut result = ScanResult::default();
    let mut seen = HashSet::new();
    let added_at = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;
    for path in paths {
        let input = Path::new(path);
        if !input.exists() {
            result
                .warnings
                .push(format!("File or folder does not exist: {path}"));
            continue;
        }
        if input.is_dir() {
            let root = readable_absolute(input)?;
            if !result.folders.contains(&root) {
                result.folders.push(root);
            }
            let depth = if recursive { usize::MAX } else { 1 };
            for entry in WalkDir::new(input)
                .follow_links(false)
                .max_depth(depth)
                .sort_by_file_name()
            {
                match entry {
                    Ok(entry) if entry.file_type().is_file() && is_audio_path(entry.path()) => {
                        add_track(entry.path(), cover_dir, added_at, &mut seen, &mut result);
                    }
                    Ok(_) => {}
                    Err(error) => result.warnings.push(error.to_string()),
                }
            }
        } else if input.is_file() && is_audio_path(input) {
            add_track(input, cover_dir, added_at, &mut seen, &mut result);
        } else {
            result
                .warnings
                .push(format!("Unsupported audio file extension: {path}"));
        }
    }
    Ok(result)
}

fn add_track(
    path: &Path,
    cover_dir: &Path,
    added_at: i64,
    seen: &mut HashSet<String>,
    result: &mut ScanResult,
) {
    let path_string = match readable_absolute(path) {
        Ok(path) => path,
        Err(error) => {
            result.warnings.push(format!("{}: {error}", path.display()));
            return;
        }
    };
    let identity = if cfg!(windows) {
        path_string.to_lowercase()
    } else {
        path_string.clone()
    };
    if !seen.insert(identity.clone()) {
        return;
    }
    let mut track = Track {
        id: Uuid::new_v5(&Uuid::NAMESPACE_URL, identity.as_bytes()).to_string(),
        path: path_string,
        title: path
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned(),
        format: path
            .extension()
            .unwrap_or_default()
            .to_string_lossy()
            .to_ascii_uppercase(),
        added_at,
        ..Track::default()
    };
    match content_fingerprint(path, &identity) {
        Ok(hash) => track.content_fingerprint = Some(hash),
        Err(error) => result
            .warnings
            .push(format!("Cannot fingerprint {}: {error}", path.display())),
    }
    let tagged_file = Probe::open(path)
        .and_then(|probe| Ok(probe.guess_file_type()?))
        .and_then(|probe| probe.read());
    match tagged_file {
        Ok(file) => {
            let properties = file.properties();
            track.duration = properties.duration().as_secs_f64();
            track.sample_rate = properties.sample_rate();
            track.channels = properties.channels().map(u16::from);
            track.bitrate = properties
                .audio_bitrate()
                .or_else(|| properties.overall_bitrate());
            // Preserve familiar extension names; MP4 can contain AAC or ALAC,
            // so the field denotes the container until engine probing occurs.
            if let Some(tag) = file.primary_tag().or_else(|| file.first_tag()) {
                if let Some(title) = tag.title().filter(|title| !title.trim().is_empty()) {
                    track.title = title.into_owned();
                }
                track.artist = tag
                    .artist()
                    .map(|value| value.into_owned())
                    .unwrap_or_default();
                track.album = tag
                    .album()
                    .map(|value| value.into_owned())
                    .unwrap_or_default();
                track.album_artist = tag
                    .get_string(ItemKey::AlbumArtist)
                    .unwrap_or_default()
                    .to_owned();
                track.genre = tag
                    .genre()
                    .map(|value| value.into_owned())
                    .unwrap_or_default();
                track.year = tag
                    .get_string(ItemKey::RecordingDate)
                    .and_then(|date| date.get(..4))
                    .and_then(|year| year.parse().ok());
                track.track_number = tag.track();
                track.disc_number = tag.disk();
            }
            let picture = file
                .tags()
                .iter()
                .flat_map(|tag| tag.pictures())
                .find(|picture| picture.pic_type() == PictureType::CoverFront)
                .or_else(|| file.tags().iter().flat_map(|tag| tag.pictures()).next());
            if let Some(picture) = picture {
                let extension = match picture.mime_type() {
                    Some(MimeType::Jpeg) => Some("jpg"),
                    Some(MimeType::Png) => Some("png"),
                    Some(MimeType::Gif) => Some("gif"),
                    Some(MimeType::Bmp) => Some("bmp"),
                    _ => None,
                };
                if let Some(extension) = extension {
                    match cache_cover(picture.data(), extension, cover_dir) {
                        Ok(path) => track.cover_path = Some(path),
                        Err(error) => result.warnings.push(format!("{}: {error}", path.display())),
                    }
                }
            }
        }
        Err(error) => result.warnings.push(format!(
            "Metadata unavailable for {}: {error}",
            path.display()
        )),
    }
    if track.cover_path.is_none() {
        track.cover_path = folder_cover(path, cover_dir);
    }
    result.tracks.push(track);
}

/// Full content hashes retain identity when a file is moved. Copies remain separate
/// tracks; the application only reconciles an unambiguous missing-file match.
/// Cache hashes by canonical path + file metadata so watcher rescans do not reread
/// an unchanged library. A changing file never receives a partial fingerprint.
fn content_fingerprint(path: &Path, identity: &str) -> Result<String, InfraError> {
    type Stamp = (u64, SystemTime, String);
    static CACHE: OnceLock<Mutex<HashMap<String, Stamp>>> = OnceLock::new();
    let metadata = fs::metadata(path)?;
    let modified = metadata.modified()?;
    let cache = CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    if let Ok(cache) = cache.lock() {
        if let Some((length, stamp, hash)) = cache.get(identity) {
            if *length == metadata.len() && *stamp == modified {
                return Ok(hash.clone());
            }
        }
    }
    let mut file = fs::File::open(path)?;
    let mut digest = Sha256::new();
    let mut buffer = vec![0_u8; 1024 * 1024];
    loop {
        let count = file.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        digest.update(&buffer[..count]);
    }
    let after = file.metadata()?;
    if after.len() != metadata.len() || after.modified()? != modified {
        return Err(InfraError::new(
            "file_changed",
            "File changed while being scanned; retry after writing finishes",
        ));
    }
    let hash = format!("{:x}", digest.finalize());
    if let Ok(mut cache) = cache.lock() {
        if cache.len() >= 16_384 {
            cache.clear();
        }
        cache.insert(
            identity.to_owned(),
            (metadata.len(), modified, hash.clone()),
        );
    }
    Ok(hash)
}
fn cache_cover(data: &[u8], extension: &str, cover_dir: &Path) -> Result<String, InfraError> {
    if data.is_empty() || data.len() > MAX_COVER_BYTES {
        return Err(InfraError::new(
            "cover_size",
            "Cover is empty or exceeds 20 MiB",
        ));
    }
    fs::create_dir_all(cover_dir)?;
    let digest = format!("{:x}", Sha256::digest(data));
    let path = cover_dir.join(format!("{digest}.{extension}"));
    if !path.exists() {
        fs::write(&path, data)?;
    }
    readable_absolute(&path)
}

fn folder_cover(audio_path: &Path, cover_dir: &Path) -> Option<String> {
    let folder = audio_path.parent()?;
    for name in [
        "cover.jpg",
        "cover.png",
        "folder.jpg",
        "folder.png",
        "front.jpg",
        "front.png",
    ] {
        let path = folder.join(name);
        let Ok(metadata) = fs::metadata(&path) else {
            continue;
        };
        if !metadata.is_file() || metadata.len() > MAX_COVER_BYTES as u64 {
            continue;
        }
        let Ok(data) = fs::read(&path) else {
            continue;
        };
        let extension = path.extension()?.to_str()?;
        if let Ok(cached) = cache_cover(&data, extension, cover_dir) {
            return Some(cached);
        }
    }
    None
}

fn readable_absolute(path: &Path) -> Result<String, InfraError> {
    let canonical = fs::canonicalize(path)?;
    let value = canonical.to_string_lossy();
    // Windows canonicalize returns verbatim paths; ordinary paths are friendlier
    // to libmpv, file associations, M3U8 consumers and the frontend asset URL.
    let value = if cfg!(windows) {
        if let Some(unc) = value.strip_prefix(r"\\?\UNC\") {
            format!(r"\\{unc}")
        } else {
            value.strip_prefix(r"\\?\").unwrap_or(&value).to_owned()
        }
    } else {
        value.into_owned()
    };
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wav_file(path: &Path) {
        // A valid one-second 8 kHz, 16-bit mono PCM WAV without tags.
        let length = 8_000_u32 * 2;
        let mut bytes = Vec::new();
        bytes.extend(b"RIFF");
        bytes.extend((36 + length).to_le_bytes());
        bytes.extend(b"WAVEfmt ");
        bytes.extend(16_u32.to_le_bytes());
        bytes.extend(1_u16.to_le_bytes());
        bytes.extend(1_u16.to_le_bytes());
        bytes.extend(8_000_u32.to_le_bytes());
        bytes.extend(16_000_u32.to_le_bytes());
        bytes.extend(2_u16.to_le_bytes());
        bytes.extend(16_u16.to_le_bytes());
        bytes.extend(b"data");
        bytes.extend(length.to_le_bytes());
        bytes.resize(44 + length as usize, 0);
        fs::write(path, bytes).unwrap();
    }

    #[test]
    fn scanner_deduplicates_paths_and_keeps_stable_ids() {
        let directory = tempfile::tempdir().unwrap();
        let audio = directory.path().join("Über 🎵.wav");
        wav_file(&audio);
        let cover = directory.path().join("covers");
        let first = scan_paths(
            &[
                directory.path().to_string_lossy().into_owned(),
                audio.to_string_lossy().into_owned(),
            ],
            false,
            &cover,
        )
        .unwrap();
        let second = scan_paths(&[audio.to_string_lossy().into_owned()], false, &cover).unwrap();
        assert_eq!(first.tracks.len(), 1);
        assert_eq!(first.tracks[0].id, second.tracks[0].id);
        assert_eq!(first.tracks[0].sample_rate, Some(8_000));
        assert_eq!(first.tracks[0].channels, Some(1));
        assert!((first.tracks[0].duration - 1.0).abs() < 0.01);
        assert_eq!(first.tracks[0].title, "Über 🎵");
    }

    #[test]
    fn recursion_is_explicit_and_metadata_failure_keeps_a_candidate() {
        let directory = tempfile::tempdir().unwrap();
        let nested = directory.path().join("nested");
        fs::create_dir(&nested).unwrap();
        wav_file(&nested.join("valid.wav"));
        fs::write(
            directory.path().join("fallback.wma"),
            b"not parsed by lofty",
        )
        .unwrap();
        let roots = vec![directory.path().to_string_lossy().into_owned()];
        let shallow = scan_paths(&roots, false, &directory.path().join("covers")).unwrap();
        assert_eq!(shallow.tracks.len(), 1);
        assert_eq!(shallow.tracks[0].format, "WMA");
        assert_eq!(shallow.warnings.len(), 1);
        let recursive = scan_paths(&roots, true, &directory.path().join("covers")).unwrap();
        assert_eq!(recursive.tracks.len(), 2);
    }

    #[test]
    fn fingerprints_match_copies_without_merging_and_change_when_bytes_change() {
        let directory = tempfile::tempdir().unwrap();
        let first = directory.path().join("first.wav");
        let copy = directory.path().join("copy.wav");
        wav_file(&first);
        fs::copy(&first, &copy).unwrap();
        let covers = directory.path().join("covers");
        let tracks = scan_paths(
            &[
                first.to_string_lossy().into_owned(),
                copy.to_string_lossy().into_owned(),
            ],
            false,
            &covers,
        )
        .unwrap()
        .tracks;
        assert_eq!(tracks.len(), 2);
        assert_ne!(tracks[0].id, tracks[1].id);
        assert_eq!(tracks[0].content_fingerprint, tracks[1].content_fingerprint);
        assert_eq!(tracks[0].content_fingerprint.as_ref().unwrap().len(), 64);
        let mut bytes = fs::read(&first).unwrap();
        bytes.extend([0, 0]);
        fs::write(&first, bytes).unwrap();
        let changed = scan_paths(&[first.to_string_lossy().into_owned()], false, &covers).unwrap();
        assert_ne!(
            changed.tracks[0].content_fingerprint,
            tracks[0].content_fingerprint
        );
        let moved = directory.path().join("moved.wav");
        fs::rename(&copy, &moved).unwrap();
        let moved = scan_paths(&[moved.to_string_lossy().into_owned()], false, &covers).unwrap();
        assert_eq!(
            moved.tracks[0].content_fingerprint,
            tracks[1].content_fingerprint
        );
    }
}
