use std::convert::TryFrom;
use std::fs;

use rusqlite::{Connection, params};
use serde::Serialize;
use tauri::{AppHandle, Manager};

const MIN_RESUME_MS: u64 = 5_000;
const END_CLEARANCE_MS: u64 = 5_000;

#[derive(Clone, Debug, Serialize)]
pub struct SavedPlaybackPosition {
    pub path: String,
    pub title: Option<String>,
    pub position_ms: u64,
    pub duration_ms: Option<u64>,
}

#[derive(Clone, Debug, Serialize)]
pub struct RestoredSession {
    pub last_track_path: Option<String>,
    pub last_track_title: Option<String>,
    pub position_ms: u64,
    pub duration_ms: Option<u64>,
    pub volume: f32,
    pub queue_song_ids: Vec<String>,
    pub queue_current_index: usize,
}

pub fn save_playback_position(
    app: &AppHandle,
    path: &str,
    title: Option<&str>,
    position_ms: u64,
    duration_ms: Option<u64>,
) -> Result<(), String> {
    let position_ms = resumable_position(position_ms, duration_ms).unwrap_or(0);
    let connection = open_database(app)?;
    initialize_database(&connection)?;
    connection
        .execute(
            "INSERT INTO track_positions (path, title, position_ms, duration_ms, updated_at)
             VALUES (?1, ?2, ?3, ?4, CURRENT_TIMESTAMP)
             ON CONFLICT(path) DO UPDATE SET
                title = excluded.title,
                position_ms = excluded.position_ms,
                duration_ms = excluded.duration_ms,
                updated_at = CURRENT_TIMESTAMP",
            params![
                path,
                title,
                i64::try_from(position_ms).unwrap_or(i64::MAX),
                duration_ms.and_then(|duration_ms| i64::try_from(duration_ms).ok())
            ],
        )
        .map_err(database_error)?;
    Ok(())
}

pub fn get_playback_position(
    app: &AppHandle,
    path: &str,
) -> Result<Option<SavedPlaybackPosition>, String> {
    let connection = open_database(app)?;
    initialize_database(&connection)?;
    let mut statement = connection
        .prepare(
            "SELECT path, title, position_ms, duration_ms
             FROM track_positions
             WHERE path = ?1",
        )
        .map_err(database_error)?;
    let mut rows = statement.query(params![path]).map_err(database_error)?;
    let Some(row) = rows.next().map_err(database_error)? else {
        return Ok(None);
    };

    let position_ms: i64 = row.get(2).map_err(database_error)?;
    let duration_ms: Option<i64> = row.get(3).map_err(database_error)?;
    Ok(Some(SavedPlaybackPosition {
        path: row.get(0).map_err(database_error)?,
        title: row.get(1).map_err(database_error)?,
        position_ms: u64::try_from(position_ms).unwrap_or_default(),
        duration_ms: duration_ms.and_then(|duration_ms| u64::try_from(duration_ms).ok()),
    }))
}

pub fn resumable_position(position_ms: u64, duration_ms: Option<u64>) -> Option<u64> {
    if position_ms < MIN_RESUME_MS {
        return None;
    }
    if duration_ms
        .and_then(|duration_ms| duration_ms.checked_sub(position_ms))
        .is_some_and(|remaining_ms| remaining_ms <= END_CLEARANCE_MS)
    {
        return None;
    }
    Some(position_ms)
}

fn open_database(app: &AppHandle) -> Result<Connection, String> {
    let data_dir = app
        .path()
        .app_data_dir()
        .map_err(|error| format!("Could not resolve app data directory: {error}"))?;
    fs::create_dir_all(&data_dir)
        .map_err(|error| format!("Could not create app data directory: {error}"))?;
    Connection::open(data_dir.join("playback.sqlite")).map_err(database_error)
}

fn initialize_database(connection: &Connection) -> Result<(), String> {
    connection
        .execute_batch(
            "PRAGMA journal_mode=WAL;
             CREATE TABLE IF NOT EXISTS track_positions (
                path TEXT PRIMARY KEY NOT NULL,
                title TEXT,
                position_ms INTEGER NOT NULL,
                duration_ms INTEGER,
                updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
            );
             CREATE TABLE IF NOT EXISTS playback_session (
                key TEXT PRIMARY KEY NOT NULL,
                last_track_path TEXT,
                last_track_title TEXT,
                position_ms INTEGER NOT NULL DEFAULT 0,
                duration_ms INTEGER,
                volume REAL NOT NULL DEFAULT 1.0,
                queue_song_ids TEXT NOT NULL DEFAULT '[]',
                queue_current_index INTEGER NOT NULL DEFAULT 0,
                updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
            );",
        )
        .map_err(database_error)
}

fn database_error(error: rusqlite::Error) -> String {
    format!("Playback position database error: {error}")
}

pub fn save_full_session(
    app: &AppHandle,
    last_path: Option<&str>,
    last_title: Option<&str>,
    position_ms: u64,
    duration_ms: Option<u64>,
    volume: f32,
    queue_ids: &[String],
    queue_index: usize,
) -> Result<(), String> {
    let connection = open_database(app)?;
    initialize_database(&connection)?;

    connection
        .execute(
            "INSERT INTO playback_session (
                key, last_track_path, last_track_title, position_ms,
                duration_ms, volume, queue_song_ids, queue_current_index, updated_at
             )
             VALUES ('current', ?1, ?2, ?3, ?4, ?5, ?6, ?7, CURRENT_TIMESTAMP)
             ON CONFLICT(key) DO UPDATE SET
                last_track_path = excluded.last_track_path,
                last_track_title = excluded.last_track_title,
                position_ms = excluded.position_ms,
                duration_ms = excluded.duration_ms,
                volume = excluded.volume,
                queue_song_ids = excluded.queue_song_ids,
                queue_current_index = excluded.queue_current_index,
                updated_at = CURRENT_TIMESTAMP",
            params![
                last_path,
                last_title,
                i64::try_from(position_ms).unwrap_or(i64::MAX),
                duration_ms.and_then(|v| i64::try_from(v).ok()),
                volume as f64,
                serde_json::to_string(queue_ids).unwrap_or_default(),
                i64::try_from(queue_index).unwrap_or(0),
            ],
        )
        .map_err(database_error)?;
    Ok(())
}

pub fn restore_playback_session(app: &AppHandle) -> Result<Option<RestoredSession>, String> {
    let connection = open_database(app)?;
    initialize_database(&connection)?;

    let mut statement = connection
        .prepare(
            "SELECT last_track_path, last_track_title, position_ms, duration_ms,
                    volume, queue_song_ids, queue_current_index
             FROM playback_session
             WHERE key = 'current'
             LIMIT 1",
        )
        .map_err(database_error)?;

    let mut rows = statement.query([]).map_err(database_error)?;
    let Some(row) = rows.next().map_err(database_error)? else {
        return Ok(None);
    };

    let last_track_path: Option<String> = row.get(0).map_err(database_error)?;
    let last_track_title: Option<String> = row.get(1).map_err(database_error)?;
    let position_ms: i64 = row.get(2).map_err(database_error)?;
    let duration_ms: Option<i64> = row.get(3).map_err(database_error)?;
    let volume: f64 = row.get(4).map_err(database_error)?;
    let queue_json: String = row.get(5).map_err(database_error)?;
    let queue_index: i64 = row.get(6).map_err(database_error)?;

    let queue_song_ids: Vec<String> =
        serde_json::from_str(&queue_json).unwrap_or_default();

    Ok(Some(RestoredSession {
        last_track_path,
        last_track_title,
        position_ms: u64::try_from(position_ms).unwrap_or_default(),
        duration_ms: duration_ms.and_then(|v| u64::try_from(v).ok()),
        volume: volume as f32,
        queue_song_ids,
        queue_current_index: usize::try_from(queue_index).unwrap_or(0),
    }))
}

#[cfg(test)]
mod tests {
    use super::resumable_position;

    #[test]
    fn short_positions_are_not_resumed() {
        assert_eq!(resumable_position(4_999, Some(60_000)), None);
    }

    #[test]
    fn positions_near_track_end_are_not_resumed() {
        assert_eq!(resumable_position(58_000, Some(60_000)), None);
    }

    #[test]
    fn middle_positions_are_resumed() {
        assert_eq!(resumable_position(20_000, Some(60_000)), Some(20_000));
    }
}
