use crate::InfraError;
use player_domain::Track;
use std::{fs, path::Path};

/// Local UTF-8 M3U/M3U8. Relative entries are relative to the playlist, not cwd.
/// Duplicate entries are deliberately preserved.
pub fn read_m3u8(path: &Path) -> Result<Vec<String>, InfraError> {
    let contents = fs::read_to_string(path)?;
    let base = path.parent().unwrap_or_else(|| Path::new("."));
    let mut paths = Vec::new();
    for line in contents.trim_start_matches('\u{feff}').lines() {
        let entry = line.trim();
        if entry.is_empty() || entry.starts_with('#') {
            continue;
        }
        if entry.contains("://") {
            return Err(InfraError::new(
                "playlist_remote",
                "Only local playlist entries are supported",
            ));
        }
        let entry_path = Path::new(entry);
        let resolved = if entry_path.is_absolute() {
            entry_path.to_path_buf()
        } else {
            base.join(entry_path)
        };
        paths.push(resolved.to_string_lossy().into_owned());
    }
    Ok(paths)
}

pub fn write_m3u8(path: &Path, tracks: &[Track]) -> Result<(), InfraError> {
    let mut contents = String::from("#EXTM3U\n");
    for track in tracks {
        if track.path.contains(['\n', '\r']) || track.path.starts_with('#') {
            return Err(InfraError::new(
                "playlist_path",
                "A path cannot be represented as an M3U8 line",
            ));
        }
        let label = if track.artist.is_empty() {
            track.title.clone()
        } else {
            format!("{} - {}", track.artist, track.title)
        };
        let label = label.replace(['\n', '\r'], " ");
        let duration = if track.duration.is_finite() && track.duration > 0.0 {
            track.duration.round() as i64
        } else {
            -1
        };
        contents.push_str(&format!("#EXTINF:{duration},{label}\n{}\n", track.path));
    }
    fs::write(path, contents)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn imports_bom_relative_paths_and_duplicate_entries() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("list.m3u8");
        fs::write(
            &path,
            "\u{feff}#EXTM3U\r\n#EXTINF:3,Title\r\nMusik/Über.mp3\r\nMusik/Über.mp3\r\n",
        )
        .unwrap();
        let tracks = read_m3u8(&path).unwrap();
        assert_eq!(tracks.len(), 2);
        assert_eq!(
            tracks[0],
            directory.path().join("Musik/Über.mp3").to_string_lossy()
        );
        assert_eq!(tracks[0], tracks[1]);
    }

    #[test]
    fn export_roundtrips_unicode_and_rejects_path_line_injection() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("export.m3u8");
        let mut track = Track {
            path: directory
                .path()
                .join("Über 🎵.flac")
                .to_string_lossy()
                .into_owned(),
            title: "Track\nName".into(),
            duration: 2.1,
            ..Track::default()
        };
        write_m3u8(&path, &[track.clone(), track.clone()]).unwrap();
        assert_eq!(
            read_m3u8(&path).unwrap(),
            vec![track.path.clone(), track.path.clone()]
        );
        track.path = "bad\nfile.mp3".into();
        assert!(write_m3u8(&path, &[track]).is_err());
    }
}
