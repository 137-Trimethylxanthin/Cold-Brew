use std::convert::TryFrom;
use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};

use md5::{Digest, Md5};
use reqwest::StatusCode;
use rusqlite::{Connection, params};
use serde::Serialize;
use serde_json::Value;
use tauri::{AppHandle, Manager};

use crate::audio_player::PlaybackStatus;
use crate::{credentials, library};

const LASTFM_ENDPOINT: &str = "https://ws.audioscrobbler.com/2.0/";
const LASTFM_BATCH_LIMIT: u32 = 50;

#[derive(Clone, Debug, Serialize)]
pub struct LastFmScrobbleStatus {
    pub pending_count: u64,
    pub submitted_count: u64,
    pub failed_count: u64,
    pub last_error: Option<String>,
}

#[derive(Clone, Debug)]
struct PendingScrobble {
    id: i64,
    artist: String,
    track: String,
    album: Option<String>,
    duration_seconds: Option<u64>,
    timestamp: i64,
}

struct LastFmTrack {
    path: String,
    artist: String,
    track: String,
    album: Option<String>,
    duration_ms: Option<u64>,
}

struct LastFmAuth {
    api_key: String,
    api_secret: String,
    session_key: String,
}

struct SubmissionError {
    message: String,
    retriable: bool,
}

pub fn queue_lastfm_scrobble_from_status(
    app: &AppHandle,
    status: &PlaybackStatus,
) -> Result<bool, String> {
    let Some(track) = lastfm_track_from_status(app, status)? else {
        return Ok(false);
    };
    if !should_scrobble(status.position_ms, track.duration_ms) {
        return Ok(false);
    };

    let timestamp = scrobble_started_at(current_unix_timestamp(), status.position_ms);
    enqueue_lastfm_scrobble(
        app,
        &track.path,
        &track.artist,
        &track.track,
        track.album.as_deref(),
        track.duration_ms.map(duration_ms_to_seconds),
        timestamp,
    )
}

pub async fn update_lastfm_now_playing_from_status(
    app: &AppHandle,
    status: &PlaybackStatus,
) -> Result<(), String> {
    let Some(track) = lastfm_track_from_status(app, status)? else {
        return Ok(());
    };
    let Some(auth) = optional_lastfm_auth()? else {
        return Ok(());
    };

    let client = reqwest::Client::new();
    submit_lastfm_now_playing(
        &client,
        &track,
        &auth.api_key,
        &auth.api_secret,
        &auth.session_key,
    )
    .await
    .map_err(|error| error.message)
}

pub fn get_lastfm_scrobble_status(app: &AppHandle) -> Result<LastFmScrobbleStatus, String> {
    let connection = open_database(app)?;
    initialize_database(&connection)?;
    scrobble_status_from_connection(&connection)
}

pub async fn retry_lastfm_scrobbles(app: &AppHandle) -> Result<LastFmScrobbleStatus, String> {
    let secrets = credentials::load_provider_secrets("lastfm")?
        .ok_or_else(|| "Save Last.fm API key, API secret, and session key first.".to_string())?;
    let api_key = secrets
        .api_key
        .as_deref()
        .and_then(non_empty_string)
        .ok_or_else(|| "Save a Last.fm API key first.".to_string())?;
    let api_secret = secrets
        .api_secret
        .as_deref()
        .and_then(non_empty_string)
        .ok_or_else(|| "Save a Last.fm API secret first.".to_string())?;
    let session_key = secrets
        .access_token
        .as_deref()
        .and_then(non_empty_string)
        .ok_or_else(|| "Save a Last.fm session key in the access token field first.".to_string())?;

    let scrobbles = pending_scrobbles(app, LASTFM_BATCH_LIMIT)?;
    let client = reqwest::Client::new();
    for scrobble in scrobbles {
        match submit_lastfm_scrobble(&client, &scrobble, api_key, api_secret, session_key).await {
            Ok(()) => mark_scrobble_submitted(app, scrobble.id)?,
            Err(error) => {
                mark_scrobble_error(app, scrobble.id, &error.message, error.retriable)?;
                if !error.retriable {
                    continue;
                }
            }
        }
    }

    get_lastfm_scrobble_status(app)
}

fn lastfm_track_from_status(
    app: &AppHandle,
    status: &PlaybackStatus,
) -> Result<Option<LastFmTrack>, String> {
    let Some(path) = status.current_path.as_deref() else {
        return Ok(None);
    };
    let Some(track) = library::get_library_track_by_path(app, path)? else {
        return Ok(None);
    };
    let Some(artist) = track.artist.as_deref().and_then(non_empty_string) else {
        return Ok(None);
    };
    let title = non_empty_string(&track.title)
        .or_else(|| status.current_title.as_deref().and_then(non_empty_string));
    let Some(title) = title else {
        return Ok(None);
    };

    Ok(Some(LastFmTrack {
        path: path.to_string(),
        artist: artist.to_string(),
        track: title.to_string(),
        album: track
            .album
            .as_deref()
            .and_then(non_empty_string)
            .map(str::to_string),
        duration_ms: status.duration_ms.or(track.duration_ms),
    }))
}

fn optional_lastfm_auth() -> Result<Option<LastFmAuth>, String> {
    let Some(secrets) = credentials::load_provider_secrets("lastfm")? else {
        return Ok(None);
    };
    let Some(api_key) = secrets.api_key.and_then(non_empty_owned) else {
        return Ok(None);
    };
    let Some(api_secret) = secrets.api_secret.and_then(non_empty_owned) else {
        return Ok(None);
    };
    let Some(session_key) = secrets.access_token.and_then(non_empty_owned) else {
        return Ok(None);
    };

    Ok(Some(LastFmAuth {
        api_key,
        api_secret,
        session_key,
    }))
}

fn enqueue_lastfm_scrobble(
    app: &AppHandle,
    path: &str,
    artist: &str,
    track: &str,
    album: Option<&str>,
    duration_seconds: Option<u64>,
    timestamp: i64,
) -> Result<bool, String> {
    let connection = open_database(app)?;
    initialize_database(&connection)?;
    let inserted = connection
        .execute(
            "INSERT OR IGNORE INTO lastfm_scrobbles (
                path, artist, track, album, duration_seconds, timestamp, status
             )
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'pending')",
            params![
                path,
                artist,
                track,
                album,
                duration_seconds.and_then(|duration_seconds| i64::try_from(duration_seconds).ok()),
                timestamp
            ],
        )
        .map_err(database_error)?;
    Ok(inserted > 0)
}

fn pending_scrobbles(app: &AppHandle, limit: u32) -> Result<Vec<PendingScrobble>, String> {
    let connection = open_database(app)?;
    initialize_database(&connection)?;
    let mut statement = connection
        .prepare(
            "SELECT id, artist, track, album, duration_seconds, timestamp
             FROM lastfm_scrobbles
             WHERE status = 'pending'
             ORDER BY timestamp ASC, id ASC
             LIMIT ?1",
        )
        .map_err(database_error)?;
    let rows = statement
        .query_map(params![limit], |row| {
            let duration_seconds: Option<i64> = row.get(4)?;
            Ok(PendingScrobble {
                id: row.get(0)?,
                artist: row.get(1)?,
                track: row.get(2)?,
                album: row.get(3)?,
                duration_seconds: duration_seconds
                    .and_then(|duration_seconds| u64::try_from(duration_seconds).ok()),
                timestamp: row.get(5)?,
            })
        })
        .map_err(database_error)?;

    let mut scrobbles = Vec::new();
    for row in rows {
        scrobbles.push(row.map_err(database_error)?);
    }
    Ok(scrobbles)
}

async fn submit_lastfm_scrobble(
    client: &reqwest::Client,
    scrobble: &PendingScrobble,
    api_key: &str,
    api_secret: &str,
    session_key: &str,
) -> Result<(), SubmissionError> {
    let mut params = vec![
        ("method".to_string(), "track.scrobble".to_string()),
        ("artist".to_string(), scrobble.artist.clone()),
        ("track".to_string(), scrobble.track.clone()),
        ("timestamp".to_string(), scrobble.timestamp.to_string()),
        ("api_key".to_string(), api_key.to_string()),
        ("sk".to_string(), session_key.to_string()),
        ("format".to_string(), "json".to_string()),
    ];
    if let Some(album) = &scrobble.album {
        params.push(("album".to_string(), album.clone()));
    }
    if let Some(duration_seconds) = scrobble.duration_seconds {
        params.push(("duration".to_string(), duration_seconds.to_string()));
    }
    params.push(("api_sig".to_string(), lastfm_signature(&params, api_secret)));

    let response = client
        .post(LASTFM_ENDPOINT)
        .form(&params)
        .send()
        .await
        .map_err(|error| SubmissionError {
            message: format!("Last.fm scrobble request failed: {error}"),
            retriable: true,
        })?;

    let status = response.status();
    let body = response.text().await.map_err(|error| SubmissionError {
        message: format!("Could not read Last.fm scrobble response: {error}"),
        retriable: true,
    })?;

    if !status.is_success() {
        return Err(SubmissionError {
            message: format!("Last.fm scrobble HTTP {status}: {body}"),
            retriable: is_retriable_http_status(status),
        });
    }

    let value = serde_json::from_str::<Value>(&body).map_err(|error| SubmissionError {
        message: format!("Could not parse Last.fm scrobble response: {error}; {body}"),
        retriable: false,
    })?;
    if let Some(code) = value.get("error").and_then(Value::as_i64) {
        return Err(SubmissionError {
            message: value
                .get("message")
                .and_then(Value::as_str)
                .map(|message| format!("Last.fm error {code}: {message}"))
                .unwrap_or_else(|| format!("Last.fm error {code}")),
            retriable: matches!(code, 11 | 16 | 29),
        });
    }

    Ok(())
}

async fn submit_lastfm_now_playing(
    client: &reqwest::Client,
    track: &LastFmTrack,
    api_key: &str,
    api_secret: &str,
    session_key: &str,
) -> Result<(), SubmissionError> {
    let mut params = vec![
        ("method".to_string(), "track.updateNowPlaying".to_string()),
        ("artist".to_string(), track.artist.clone()),
        ("track".to_string(), track.track.clone()),
        ("api_key".to_string(), api_key.to_string()),
        ("sk".to_string(), session_key.to_string()),
        ("format".to_string(), "json".to_string()),
    ];
    if let Some(album) = &track.album {
        params.push(("album".to_string(), album.clone()));
    }
    if let Some(duration_ms) = track.duration_ms {
        params.push((
            "duration".to_string(),
            duration_ms_to_seconds(duration_ms).to_string(),
        ));
    }
    params.push(("api_sig".to_string(), lastfm_signature(&params, api_secret)));

    let response = client
        .post(LASTFM_ENDPOINT)
        .form(&params)
        .send()
        .await
        .map_err(|error| SubmissionError {
            message: format!("Last.fm now-playing request failed: {error}"),
            retriable: false,
        })?;

    let status = response.status();
    let body = response.text().await.map_err(|error| SubmissionError {
        message: format!("Could not read Last.fm now-playing response: {error}"),
        retriable: false,
    })?;

    if !status.is_success() {
        return Err(SubmissionError {
            message: format!("Last.fm now-playing HTTP {status}: {body}"),
            retriable: false,
        });
    }

    let value = serde_json::from_str::<Value>(&body).map_err(|error| SubmissionError {
        message: format!("Could not parse Last.fm now-playing response: {error}; {body}"),
        retriable: false,
    })?;
    if let Some(code) = value.get("error").and_then(Value::as_i64) {
        return Err(SubmissionError {
            message: value
                .get("message")
                .and_then(Value::as_str)
                .map(|message| format!("Last.fm error {code}: {message}"))
                .unwrap_or_else(|| format!("Last.fm error {code}")),
            retriable: false,
        });
    }

    Ok(())
}

fn mark_scrobble_submitted(app: &AppHandle, id: i64) -> Result<(), String> {
    let connection = open_database(app)?;
    initialize_database(&connection)?;
    connection
        .execute(
            "UPDATE lastfm_scrobbles
             SET status = 'submitted',
                 attempts = attempts + 1,
                 last_error = NULL,
                 updated_at = CURRENT_TIMESTAMP
             WHERE id = ?1",
            params![id],
        )
        .map_err(database_error)?;
    Ok(())
}

fn mark_scrobble_error(
    app: &AppHandle,
    id: i64,
    error: &str,
    retriable: bool,
) -> Result<(), String> {
    let connection = open_database(app)?;
    initialize_database(&connection)?;
    connection
        .execute(
            "UPDATE lastfm_scrobbles
             SET status = ?1,
                 attempts = attempts + 1,
                 last_error = ?2,
                 updated_at = CURRENT_TIMESTAMP
             WHERE id = ?3",
            params![if retriable { "pending" } else { "failed" }, error, id],
        )
        .map_err(database_error)?;
    Ok(())
}

fn scrobble_status_from_connection(
    connection: &Connection,
) -> Result<LastFmScrobbleStatus, String> {
    let pending_count = count_scrobbles(connection, "pending")?;
    let submitted_count = count_scrobbles(connection, "submitted")?;
    let failed_count = count_scrobbles(connection, "failed")?;
    let last_error = connection
        .query_row(
            "SELECT last_error
             FROM lastfm_scrobbles
             WHERE last_error IS NOT NULL
             ORDER BY updated_at DESC, id DESC
             LIMIT 1",
            [],
            |row| row.get(0),
        )
        .or_else(|error| match error {
            rusqlite::Error::QueryReturnedNoRows => Ok(None),
            error => Err(error),
        })
        .map_err(database_error)?;

    Ok(LastFmScrobbleStatus {
        pending_count,
        submitted_count,
        failed_count,
        last_error,
    })
}

fn count_scrobbles(connection: &Connection, status: &str) -> Result<u64, String> {
    let count: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM lastfm_scrobbles WHERE status = ?1",
            params![status],
            |row| row.get(0),
        )
        .map_err(database_error)?;
    Ok(u64::try_from(count).unwrap_or_default())
}

fn open_database(app: &AppHandle) -> Result<Connection, String> {
    let data_dir = app
        .path()
        .app_data_dir()
        .map_err(|error| format!("Could not resolve app data directory: {error}"))?;
    fs::create_dir_all(&data_dir)
        .map_err(|error| format!("Could not create app data directory: {error}"))?;
    Connection::open(data_dir.join("scrobbling.sqlite")).map_err(database_error)
}

fn initialize_database(connection: &Connection) -> Result<(), String> {
    connection
        .execute_batch(
            "CREATE TABLE IF NOT EXISTS lastfm_scrobbles (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                path TEXT NOT NULL,
                artist TEXT NOT NULL,
                track TEXT NOT NULL,
                album TEXT,
                duration_seconds INTEGER,
                timestamp INTEGER NOT NULL,
                status TEXT NOT NULL DEFAULT 'pending',
                attempts INTEGER NOT NULL DEFAULT 0,
                last_error TEXT,
                created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                UNIQUE(path, timestamp)
            );
            CREATE INDEX IF NOT EXISTS lastfm_scrobbles_status_idx
                ON lastfm_scrobbles(status, timestamp, id);",
        )
        .map_err(database_error)
}

fn should_scrobble(played_ms: u64, duration_ms: Option<u64>) -> bool {
    let Some(duration_ms) = duration_ms else {
        return false;
    };
    if duration_ms <= 30_000 {
        return false;
    }
    let threshold_ms = (duration_ms / 2).min(240_000);
    played_ms >= threshold_ms
}

fn scrobble_started_at(now_seconds: i64, played_ms: u64) -> i64 {
    now_seconds.saturating_sub(i64::try_from(played_ms / 1000).unwrap_or(i64::MAX))
}

fn current_unix_timestamp() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .and_then(|duration| i64::try_from(duration.as_secs()).ok())
        .unwrap_or_default()
}

fn duration_ms_to_seconds(duration_ms: u64) -> u64 {
    (duration_ms + 500) / 1000
}

pub(crate) fn lastfm_signature(params: &[(String, String)], secret: &str) -> String {
    let mut signable = params
        .iter()
        .filter(|(name, _)| name != "format" && name != "callback" && name != "api_sig")
        .collect::<Vec<_>>();
    signable.sort_by(|(left_name, _), (right_name, _)| left_name.cmp(right_name));

    let mut payload = String::new();
    for (name, value) in signable {
        payload.push_str(name);
        payload.push_str(value);
    }
    payload.push_str(secret);

    let mut hasher = Md5::new();
    hasher.update(payload.as_bytes());
    format!("{:x}", hasher.finalize())
}

fn is_retriable_http_status(status: StatusCode) -> bool {
    status.is_server_error() || status == StatusCode::TOO_MANY_REQUESTS
}

fn non_empty_string(value: &str) -> Option<&str> {
    let value = value.trim();
    if value.is_empty() { None } else { Some(value) }
}

fn non_empty_owned(value: String) -> Option<String> {
    let value = value.trim().to_string();
    if value.is_empty() { None } else { Some(value) }
}

fn database_error(error: rusqlite::Error) -> String {
    format!("Scrobbling database error: {error}")
}

#[cfg(test)]
mod tests {
    use super::{duration_ms_to_seconds, lastfm_signature, scrobble_started_at, should_scrobble};

    #[test]
    fn scrobble_threshold_rejects_short_tracks() {
        assert!(!should_scrobble(20_000, Some(30_000)));
        assert!(!should_scrobble(20_000, None));
    }

    #[test]
    fn scrobble_threshold_accepts_half_or_four_minutes() {
        assert!(should_scrobble(90_000, Some(180_000)));
        assert!(should_scrobble(240_000, Some(900_000)));
        assert!(!should_scrobble(239_999, Some(900_000)));
    }

    #[test]
    fn scrobble_timestamp_uses_play_start_time() {
        assert_eq!(scrobble_started_at(1_700_000_000, 90_000), 1_699_999_910);
    }

    #[test]
    fn duration_milliseconds_round_to_seconds() {
        assert_eq!(duration_ms_to_seconds(240_499), 240);
        assert_eq!(duration_ms_to_seconds(240_500), 241);
    }

    #[test]
    fn lastfm_signature_ignores_format_and_signature_params() {
        let params = vec![
            ("method".to_string(), "track.scrobble".to_string()),
            ("artist".to_string(), "Artist".to_string()),
            ("track".to_string(), "Track".to_string()),
            ("timestamp".to_string(), "100".to_string()),
            ("api_key".to_string(), "key".to_string()),
            ("sk".to_string(), "session".to_string()),
        ];
        let mut with_format = params.clone();
        with_format.push(("format".to_string(), "json".to_string()));
        with_format.push(("api_sig".to_string(), "ignored".to_string()));

        assert_eq!(
            lastfm_signature(&params, "secret"),
            lastfm_signature(&with_format, "secret")
        );
    }
}
