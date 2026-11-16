use crate::InfraError;
use player_domain::ListeningEvent;
use rusqlite::{params, Connection, OptionalExtension};
use std::{path::Path, time::Duration};

/// A single transactional SQLite snapshot keeps queue identities, history and
/// settings consistent. Its schema can later grow normalized library indexes.
pub struct Store {
    connection: Connection,
}

impl Store {
    pub fn open(path: &Path) -> Result<Self, InfraError> {
        if let Some(parent) = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        {
            std::fs::create_dir_all(parent)?;
        }
        let connection = Connection::open(path)?;
        connection.busy_timeout(Duration::from_secs(5))?;
        connection.execute_batch(
            "PRAGMA journal_mode=WAL;
             PRAGMA synchronous=NORMAL;
             PRAGMA foreign_keys=ON;",
        )?;
        let version: i64 = connection.pragma_query_value(None, "user_version", |row| row.get(0))?;
        if version > 2 {
            return Err(InfraError::new(
                "database_version",
                format!("Unsupported schema version {version}"),
            ));
        }
        if version < 1 {
            connection.execute_batch(
                "BEGIN IMMEDIATE;
                 CREATE TABLE IF NOT EXISTS app_state (
                     id INTEGER PRIMARY KEY CHECK (id = 1),
                     json TEXT NOT NULL,
                     updated_at INTEGER NOT NULL
                 );
                 PRAGMA user_version=1;
                 COMMIT;",
            )?;
        }
        if version < 2 {
            connection.execute_batch(
                "BEGIN IMMEDIATE;
                 CREATE TABLE IF NOT EXISTS listening_history (
                   id TEXT PRIMARY KEY, track_id TEXT NOT NULL, started_at INTEGER NOT NULL,
                   listened_seconds REAL NOT NULL, duration REAL NOT NULL,
                   title TEXT NOT NULL, artist TEXT NOT NULL, album TEXT NOT NULL, path TEXT NOT NULL
                 );
                 CREATE INDEX IF NOT EXISTS history_started ON listening_history(started_at);
                 CREATE INDEX IF NOT EXISTS history_track ON listening_history(track_id,started_at);
                 PRAGMA user_version=2;
                 COMMIT;",
            )?;
        }
        Ok(Self { connection })
    }

    pub fn load_json(&self) -> Result<Option<String>, InfraError> {
        Ok(self
            .connection
            .query_row("SELECT json FROM app_state WHERE id=1", [], |row| {
                row.get(0)
            })
            .optional()?)
    }

    pub fn save_json(&mut self, json: &str) -> Result<(), InfraError> {
        // Reject invalid state before replacing the last known good snapshot.
        serde_json::from_str::<serde_json::Value>(json)
            .map_err(|error| InfraError::new("invalid_state", error.to_string()))?;
        let transaction = self.connection.transaction()?;
        transaction.execute(
            "INSERT INTO app_state(id, json, updated_at) VALUES(1, ?1, unixepoch())
             ON CONFLICT(id) DO UPDATE SET json=excluded.json, updated_at=excluded.updated_at",
            params![json],
        )?;
        transaction.commit()?;
        Ok(())
    }

    pub fn record_listening(&mut self, event: &ListeningEvent) -> Result<(), InfraError> {
        if event.id.is_empty()
            || event.track_id.is_empty()
            || !event.listened_seconds.is_finite()
            || event.listened_seconds < 0.
            || !event.duration.is_finite()
            || event.duration < 0.
        {
            return Err(InfraError::new("invalid_history", "Invalid listening pass"));
        }
        self.connection.execute(
            "INSERT INTO listening_history(id,track_id,started_at,listened_seconds,duration,title,artist,album,path)
             VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9)
             ON CONFLICT(id) DO UPDATE SET listened_seconds=excluded.listened_seconds,duration=excluded.duration",
            params![event.id,event.track_id,event.started_at,event.listened_seconds,event.duration,
                event.title,event.artist,event.album,event.path],
        )?;
        Ok(())
    }

    pub fn listening_history(
        &mut self,
        from: Option<i64>,
        to: Option<i64>,
    ) -> Result<Vec<ListeningEvent>, InfraError> {
        let mut query = self.connection.prepare(
            "SELECT id,track_id,title,artist,album,path,started_at,listened_seconds,duration
             FROM listening_history WHERE started_at>=?1 AND started_at<?2 ORDER BY started_at,id",
        )?;
        let rows = query
            .query_map(
                params![from.unwrap_or(i64::MIN), to.unwrap_or(i64::MAX)],
                |row| {
                    Ok(ListeningEvent {
                        id: row.get(0)?,
                        track_id: row.get(1)?,
                        title: row.get(2)?,
                        artist: row.get(3)?,
                        album: row.get(4)?,
                        path: row.get(5)?,
                        started_at: row.get(6)?,
                        listened_seconds: row.get(7)?,
                        duration: row.get(8)?,
                    })
                },
            )?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    pub fn listening_years(&mut self) -> Result<Vec<i32>, InfraError> {
        // Match the local calendar boundaries used by the statistics date picker.
        // This reads only history columns, rather than deserializing every pass.
        let mut query = self.connection.prepare(
            "SELECT DISTINCT CAST(strftime('%Y',started_at,'unixepoch','localtime') AS INTEGER)
             FROM listening_history WHERE listened_seconds>0 ORDER BY 1 DESC",
        )?;
        let years = query
            .query_map([], |row| row.get::<_, Option<i32>>(0))?
            .collect::<Result<Vec<_>, _>>()?;
        // SQLite returns NULL for timestamps beyond its supported date range.
        Ok(years.into_iter().flatten().collect())
    }

    pub fn clear_listening(&mut self) -> Result<(), InfraError> {
        self.connection
            .execute("DELETE FROM listening_history", [])?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn persists_unicode_and_keeps_last_good_snapshot() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("state.sqlite3");
        let mut store = Store::open(&path).unwrap();
        assert!(store.load_json().unwrap().is_none());
        let state = r#"{"playlist":"Übergrößen 🎵","queue":["a","a"]}"#;
        store.save_json(state).unwrap();
        assert!(store.save_json("not json").is_err());
        drop(store);
        let store = Store::open(&path).unwrap();
        assert_eq!(store.load_json().unwrap().as_deref(), Some(state));
    }

    #[test]
    fn indexed_history_migrates_updates_pass_once_and_filters_time_ranges() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("state.sqlite3");
        let connection = Connection::open(&path).unwrap();
        connection.execute_batch("CREATE TABLE app_state(id INTEGER PRIMARY KEY,json TEXT NOT NULL,updated_at INTEGER NOT NULL); INSERT INTO app_state VALUES(1,'{}',0); PRAGMA user_version=1;").unwrap();
        drop(connection);
        let mut store = Store::open(&path).unwrap();
        let mut event = ListeningEvent {
            id: "pass".into(),
            track_id: "song".into(),
            title: "Grüße 🎵".into(),
            artist: "日本語".into(),
            album: "Album".into(),
            path: "C:/music/song.flac".into(),
            started_at: 100,
            listened_seconds: 30.,
            duration: 120.,
        };
        store.record_listening(&event).unwrap();
        event.listened_seconds = 65.;
        store.record_listening(&event).unwrap();
        assert_eq!(
            store.listening_history(None, None).unwrap(),
            vec![event.clone()]
        );
        assert!(store.listening_history(Some(101), None).unwrap().is_empty());
        assert!(store.listening_history(None, Some(100)).unwrap().is_empty());
        drop(store);
        let mut store = Store::open(&path).unwrap();
        assert_eq!(store.load_json().unwrap(), Some("{}".into()));
        assert_eq!(
            store.listening_history(Some(100), Some(101)).unwrap(),
            vec![event]
        );
        store.clear_listening().unwrap();
        assert!(store.listening_history(None, None).unwrap().is_empty());
        assert!(store.load_json().unwrap().is_some());
    }

    #[test]
    fn listening_years_include_partial_passes_without_filling_gaps_or_empty_years() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("state.sqlite3");
        let mut store = Store::open(&path).unwrap();
        assert!(store.listening_years().unwrap().is_empty());
        for (id, date, listened_seconds) in [
            ("older", "2022-06-15 12:00:00", 0.5),
            ("newer", "2026-06-15 12:00:00", 1.),
            ("same_year", "2026-07-15 12:00:00", 60.),
            ("not_heard", "2024-06-15 12:00:00", 0.),
        ] {
            let started_at = store
                .connection
                .query_row("SELECT unixepoch(?1)", [date], |row| row.get(0))
                .unwrap();
            store
                .record_listening(&ListeningEvent {
                    id: id.into(),
                    track_id: "song".into(),
                    title: "Song".into(),
                    artist: "Artist".into(),
                    album: "Album".into(),
                    path: "C:/music/song.flac".into(),
                    started_at,
                    listened_seconds,
                    duration: 120.,
                })
                .unwrap();
        }
        drop(store);
        let mut store = Store::open(&path).unwrap();
        assert_eq!(store.listening_years().unwrap(), vec![2026, 2022]);
        store.clear_listening().unwrap();
        assert!(store.listening_years().unwrap().is_empty());
    }
}
