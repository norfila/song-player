//! Replace a user-selected JSON export only after its complete contents are durable.
use std::{
    fs::{self, File, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

static NEXT_FILE: AtomicU64 = AtomicU64::new(0);

struct TemporaryFile {
    path: PathBuf,
    published: bool,
}
impl Drop for TemporaryFile {
    fn drop(&mut self) {
        if !self.published {
            let _ = fs::remove_file(&self.path);
        }
    }
}

fn create_temporary(directory: &Path) -> io::Result<(TemporaryFile, File)> {
    for _ in 0..32 {
        let nonce = NEXT_FILE.fetch_add(1, Ordering::Relaxed);
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let path = directory.join(format!(
            ".song-player-config-{}-{timestamp}-{nonce}.tmp",
            std::process::id()
        ));
        match OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(file) => {
                return Ok((
                    TemporaryFile {
                        path,
                        published: false,
                    },
                    file,
                ))
            }
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error),
        }
    }
    Err(io::Error::new(
        io::ErrorKind::AlreadyExists,
        "Could not create a temporary export file",
    ))
}

pub(super) fn write_atomic(path: &Path, contents: &[u8]) -> io::Result<()> {
    write_with(path, |file| file.write_all(contents))
}

/// History is not a settings document. Stream it into the same durable atomic
/// replacement path without applying the settings import/export size limit.
pub(super) fn write_json_atomic(path: &Path, document: &impl serde::Serialize) -> io::Result<()> {
    write_with(path, |file| {
        let mut writer = io::BufWriter::new(file);
        serde_json::to_writer_pretty(&mut writer, document).map_err(io::Error::other)?;
        writer.flush()
    })
}

fn write_with(path: &Path, write: impl FnOnce(&mut File) -> io::Result<()>) -> io::Result<()> {
    if path.file_name().is_none() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Export path must name a file",
        ));
    }
    let directory = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let (mut temporary, mut file) = create_temporary(directory)?;
    let result = write(&mut file)
        .and_then(|_| file.flush())
        .and_then(|_| file.sync_all());
    // Windows replacement and cleanup require our temporary handle to be closed.
    drop(file);
    result?;
    // A sibling keeps the rename on the same filesystem. Rust's Windows rename
    // uses replacement semantics, so the prior export survives a failed rename.
    fs::rename(&temporary.path, path)?;
    temporary.published = true;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestDirectory(PathBuf);
    impl TestDirectory {
        fn new() -> Self {
            let nonce = NEXT_FILE.fetch_add(1, Ordering::Relaxed);
            let timestamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let path = std::env::temp_dir().join(format!(
                "SongPlayer-config-test-{}-{timestamp}-{nonce}",
                std::process::id()
            ));
            fs::create_dir(&path).unwrap();
            Self(path)
        }
        fn assert_only(&self, filename: &str) {
            let entries: Vec<_> = fs::read_dir(&self.0)
                .unwrap()
                .map(|entry| entry.unwrap().file_name())
                .collect();
            assert_eq!(entries, vec![std::ffi::OsString::from(filename)]);
        }
    }
    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = fs::remove_dir(&self.0);
        }
    }

    #[test]
    fn atomic_config_writer_creates_and_overwrites_unicode_exports() {
        let directory = TestDirectory::new();
        let filename = "Eigene Designs – Grüße 🎵.json";
        let path = directory.0.join(filename);
        write_atomic(&path, br#"{"version":1}"#).unwrap();
        write_atomic(&path, br#"{"version":2,"complete":true}"#).unwrap();
        assert_eq!(
            fs::read(&path).unwrap(),
            br#"{"version":2,"complete":true}"#
        );
        directory.assert_only(filename);
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn history_writer_supports_documents_larger_than_settings_limit() {
        let directory = TestDirectory::new();
        let path = directory.0.join("history.json");
        let document = serde_json::json!({ "history": "x".repeat(3 * 1024 * 1024) });
        write_json_atomic(&path, &document).unwrap();
        assert!(fs::metadata(&path).unwrap().len() > 2 * 1024 * 1024);
        let actual: serde_json::Value =
            serde_json::from_reader(File::open(&path).unwrap()).unwrap();
        assert_eq!(actual, document);
        directory.assert_only("history.json");
    }

    #[test]
    fn atomic_config_writer_preserves_previous_export_after_partial_write_failure() {
        let directory = TestDirectory::new();
        let path = directory.0.join("settings.json");
        fs::write(&path, b"previous complete backup").unwrap();
        let error = write_with(&path, |file| {
            file.write_all(b"partial replacement")?;
            Err(io::Error::other("Simulated disk full"))
        })
        .unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::Other);
        assert_eq!(fs::read(&path).unwrap(), b"previous complete backup");
        directory.assert_only("settings.json");
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn atomic_config_writer_cleans_up_when_replacement_fails() {
        let directory = TestDirectory::new();
        let path = directory.0.join("settings.json");
        fs::create_dir(&path).unwrap();
        assert!(write_atomic(&path, b"complete replacement").is_err());
        assert!(path.is_dir());
        directory.assert_only("settings.json");
        fs::remove_dir(path).unwrap();
    }

    #[cfg(windows)]
    #[test]
    fn atomic_config_writer_preserves_locked_windows_export() {
        use std::os::windows::fs::OpenOptionsExt;
        let directory = TestDirectory::new();
        let path = directory.0.join("settings.json");
        fs::write(&path, b"previous locked backup").unwrap();
        let lock = OpenOptions::new()
            .read(true)
            .share_mode(0)
            .open(&path)
            .unwrap();
        assert!(write_atomic(&path, b"new backup").is_err());
        drop(lock);
        assert_eq!(fs::read(&path).unwrap(), b"previous locked backup");
        directory.assert_only("settings.json");
        fs::remove_file(path).unwrap();
    }
}
