use std::convert::TryFrom;
use std::fs;

use rusqlite::{params, Connection};
use serde::Serialize;
use tauri::{AppHandle, Manager};

use crate::audio_player::PlaybackStatus;

#[derive(Clone, Debug, Serialize)]
pub struct ListeningHistoryEntry {
    pub id: i64,
    pub path: String,
    pub title: Option<String>,
    pub source: String,
    pub event: String,
    pub classification: Option<String>,
    pub position_ms: u64,
    pub duration_ms: Option<u64>,
    pub listened_ms: u64,
    pub created_at: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct ListeningHistorySummary {
    pub path: String,
    pub title: Option<String>,
    pub source: String,
    pub play_count: u64,
    pub completion_count: u64,
    pub skip_count: u64,
    pub partial_count: u64,
    pub total_listened_ms: u64,
    pub duration_ms: Option<u64>,
    pub last_played_at: String,
}

pub fn record_playback_event(
    app: &AppHandle,
    status: &PlaybackStatus,
    event: &str,
) -> Result<(), String> {
    let Some(path) = status.current_path.as_deref() else {
        return Ok(());
    };
    let connection = open_database(app)?;
    initialize_database(&connection)?;
    let event = normalize_event(event);
    let classification = classify_event(&event, status.position_ms, status.duration_ms);
    let listened_ms = listened_duration_ms(&event, status.position_ms, status.duration_ms);
    connection
        .execute(
            "INSERT INTO listening_history (
                path, title, source, event, classification, position_ms, duration_ms, listened_ms
             )
             VALUES (?1, ?2, 'local', ?3, ?4, ?5, ?6, ?7)",
            params![
                path,
                status.current_title.as_deref(),
                event,
                classification,
                i64::try_from(status.position_ms).unwrap_or(i64::MAX),
                status
                    .duration_ms
                    .and_then(|duration_ms| i64::try_from(duration_ms).ok()),
                i64::try_from(listened_ms).unwrap_or(i64::MAX)
            ],
        )
        .map_err(database_error)?;
    Ok(())
}

pub fn list_listening_history(
    app: &AppHandle,
    limit: Option<u32>,
) -> Result<Vec<ListeningHistoryEntry>, String> {
    let connection = open_database(app)?;
    initialize_database(&connection)?;
    let limit = limit.unwrap_or(50).clamp(1, 200);
    let mut statement = connection
        .prepare(
            "SELECT id, path, title, source, event, classification, position_ms, duration_ms,
                    listened_ms, created_at
             FROM listening_history
             ORDER BY id DESC
             LIMIT ?1",
        )
        .map_err(database_error)?;
    let rows = statement
        .query_map(params![limit], |row| {
            let position_ms: i64 = row.get(6)?;
            let duration_ms: Option<i64> = row.get(7)?;
            let listened_ms: i64 = row.get(8)?;
            Ok(ListeningHistoryEntry {
                id: row.get(0)?,
                path: row.get(1)?,
                title: row.get(2)?,
                source: row.get(3)?,
                event: row.get(4)?,
                classification: row.get(5)?,
                position_ms: u64::try_from(position_ms).unwrap_or_default(),
                duration_ms: duration_ms.and_then(|duration_ms| u64::try_from(duration_ms).ok()),
                listened_ms: u64::try_from(listened_ms).unwrap_or_default(),
                created_at: row.get(9)?,
            })
        })
        .map_err(database_error)?;

    let mut entries = Vec::new();
    for row in rows {
        entries.push(row.map_err(database_error)?);
    }
    Ok(entries)
}

pub fn list_listening_history_summary(
    app: &AppHandle,
    limit: Option<u32>,
) -> Result<Vec<ListeningHistorySummary>, String> {
    let connection = open_database(app)?;
    initialize_database(&connection)?;
    let limit = limit.unwrap_or(25).clamp(1, 100);
    let mut statement = connection
        .prepare(
            "SELECT
                path,
                MAX(title),
                source,
                SUM(CASE WHEN event = 'started' THEN 1 ELSE 0 END),
                SUM(CASE WHEN classification = 'completed' THEN 1 ELSE 0 END),
                SUM(CASE WHEN classification = 'skipped' THEN 1 ELSE 0 END),
                SUM(CASE WHEN classification = 'partial' THEN 1 ELSE 0 END),
                SUM(listened_ms),
                MAX(duration_ms),
                MAX(created_at),
                MAX(id)
             FROM listening_history
             GROUP BY path, source
             ORDER BY MAX(id) DESC
             LIMIT ?1",
        )
        .map_err(database_error)?;
    let rows = statement
        .query_map(params![limit], |row| {
            let play_count: i64 = row.get(3)?;
            let completion_count: i64 = row.get(4)?;
            let skip_count: i64 = row.get(5)?;
            let partial_count: i64 = row.get(6)?;
            let total_listened_ms: i64 = row.get(7)?;
            let duration_ms: Option<i64> = row.get(8)?;
            Ok(ListeningHistorySummary {
                path: row.get(0)?,
                title: row.get(1)?,
                source: row.get(2)?,
                play_count: u64::try_from(play_count).unwrap_or_default(),
                completion_count: u64::try_from(completion_count).unwrap_or_default(),
                skip_count: u64::try_from(skip_count).unwrap_or_default(),
                partial_count: u64::try_from(partial_count).unwrap_or_default(),
                total_listened_ms: u64::try_from(total_listened_ms).unwrap_or_default(),
                duration_ms: duration_ms.and_then(|duration_ms| u64::try_from(duration_ms).ok()),
                last_played_at: row.get(9)?,
            })
        })
        .map_err(database_error)?;

    let mut summaries = Vec::new();
    for row in rows {
        summaries.push(row.map_err(database_error)?);
    }
    Ok(summaries)
}

fn open_database(app: &AppHandle) -> Result<Connection, String> {
    let data_dir = app
        .path()
        .app_data_dir()
        .map_err(|error| format!("Could not resolve app data directory: {error}"))?;
    fs::create_dir_all(&data_dir)
        .map_err(|error| format!("Could not create app data directory: {error}"))?;
    Connection::open(data_dir.join("listening_history.sqlite")).map_err(database_error)
}

fn initialize_database(connection: &Connection) -> Result<(), String> {
    connection
        .execute_batch(
            "CREATE TABLE IF NOT EXISTS listening_history (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                path TEXT NOT NULL,
                title TEXT,
                source TEXT NOT NULL,
                event TEXT NOT NULL,
                classification TEXT,
                position_ms INTEGER NOT NULL DEFAULT 0,
                duration_ms INTEGER,
                listened_ms INTEGER NOT NULL DEFAULT 0,
                created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
            );
            CREATE INDEX IF NOT EXISTS listening_history_path_idx
                ON listening_history(path, source, id);",
        )
        .map_err(database_error)?;
    add_column_if_missing(
        connection,
        "ALTER TABLE listening_history ADD COLUMN classification TEXT",
    )?;
    add_column_if_missing(
        connection,
        "ALTER TABLE listening_history ADD COLUMN listened_ms INTEGER NOT NULL DEFAULT 0",
    )
}

fn add_column_if_missing(connection: &Connection, statement: &str) -> Result<(), String> {
    match connection.execute(statement, []) {
        Ok(_) => Ok(()),
        Err(error) if error.to_string().contains("duplicate column name") => Ok(()),
        Err(error) => Err(database_error(error)),
    }
}

fn normalize_event(event: &str) -> String {
    match event.trim().to_ascii_lowercase().as_str() {
        "started" | "paused" | "resumed" | "seeked" | "stopped" | "changed" | "ended" => {
            event.trim().to_ascii_lowercase()
        }
        _ => "event".to_string(),
    }
}

fn classify_event(event: &str, position_ms: u64, duration_ms: Option<u64>) -> Option<&'static str> {
    match event {
        "started" | "resumed" | "seeked" => None,
        "paused" => (position_ms >= 30_000).then_some("partial"),
        "stopped" | "changed" | "ended" => Some(terminal_classification(position_ms, duration_ms)),
        _ => None,
    }
}

fn terminal_classification(position_ms: u64, duration_ms: Option<u64>) -> &'static str {
    let Some(duration_ms) = duration_ms.filter(|duration_ms| *duration_ms > 0) else {
        return if position_ms >= 30_000 {
            "partial"
        } else {
            "skipped"
        };
    };

    let remaining_ms = duration_ms.saturating_sub(position_ms);
    if position_ms.saturating_mul(10) >= duration_ms.saturating_mul(9) || remaining_ms <= 30_000 {
        "completed"
    } else if position_ms < 30_000 || position_ms.saturating_mul(2) < duration_ms {
        "skipped"
    } else {
        "partial"
    }
}

fn listened_duration_ms(event: &str, position_ms: u64, duration_ms: Option<u64>) -> u64 {
    if !matches!(event, "paused" | "stopped" | "changed" | "ended") {
        return 0;
    }

    let position_ms = duration_ms
        .map(|duration_ms| position_ms.min(duration_ms))
        .unwrap_or(position_ms);

    if position_ms < 5_000 {
        0
    } else {
        position_ms
    }
}

fn database_error(error: rusqlite::Error) -> String {
    format!("Listening history database error: {error}")
}

#[cfg(test)]
mod tests {
    use super::{classify_event, listened_duration_ms, terminal_classification};

    #[test]
    fn terminal_events_near_the_end_are_completed() {
        assert_eq!(terminal_classification(181_000, Some(200_000)), "completed");
        assert_eq!(terminal_classification(172_000, Some(200_000)), "completed");
    }

    #[test]
    fn terminal_events_before_halfway_are_skipped() {
        assert_eq!(terminal_classification(50_000, Some(200_000)), "skipped");
    }

    #[test]
    fn terminal_events_after_halfway_are_partial() {
        assert_eq!(terminal_classification(130_000, Some(200_000)), "partial");
    }

    #[test]
    fn started_and_seeked_events_do_not_classify_listens() {
        assert_eq!(classify_event("started", 60_000, Some(200_000)), None);
        assert_eq!(classify_event("seeked", 60_000, Some(200_000)), None);
    }

    #[test]
    fn listened_duration_ignores_short_or_non_terminal_events() {
        assert_eq!(listened_duration_ms("started", 60_000, Some(200_000)), 0);
        assert_eq!(listened_duration_ms("stopped", 4_999, Some(200_000)), 0);
        assert_eq!(
            listened_duration_ms("stopped", 240_000, Some(200_000)),
            200_000
        );
    }
}
