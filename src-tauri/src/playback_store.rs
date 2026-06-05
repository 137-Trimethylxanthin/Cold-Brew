use std::convert::TryFrom;
use std::fs;

use rusqlite::{params, Connection};
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
            "CREATE TABLE IF NOT EXISTS track_positions (
                path TEXT PRIMARY KEY NOT NULL,
                title TEXT,
                position_ms INTEGER NOT NULL,
                duration_ms INTEGER,
                updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
            );",
        )
        .map_err(database_error)
}

fn database_error(error: rusqlite::Error) -> String {
    format!("Playback position database error: {error}")
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
