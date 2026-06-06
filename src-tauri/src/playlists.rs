use std::convert::TryFrom;
use std::fs;
use std::path::{Path, PathBuf};

use rusqlite::{Connection, params};
use serde::Serialize;
use tauri::{AppHandle, Manager};
use tracing::instrument;

use crate::music_player::Song;

#[derive(Clone, Debug, Serialize)]
pub struct PlaylistSummary {
    pub id: i64,
    pub name: String,
    pub track_count: usize,
}

#[derive(Clone, Debug, Serialize)]
pub struct PlaylistDetail {
    pub id: i64,
    pub name: String,
    pub tracks: Vec<Song>,
}

#[instrument(skip(app))]
pub fn create_playlist(app: &AppHandle, name: String) -> Result<PlaylistDetail, String> {
    let name = normalize_playlist_name(&name)?;
    let connection = open_database(app)?;
    initialize_database(&connection)?;
    connection
        .execute("INSERT INTO playlists (name) VALUES (?1)", params![name])
        .map_err(database_error)?;
    let id = connection.last_insert_rowid();
    get_playlist(app, id)
}

#[instrument(skip(app))]
pub fn list_playlists(app: &AppHandle) -> Result<Vec<PlaylistSummary>, String> {
    let connection = open_database(app)?;
    initialize_database(&connection)?;
    let mut statement = connection
        .prepare(
            "SELECT playlists.id, playlists.name, COUNT(playlist_tracks.id) AS track_count
             FROM playlists
             LEFT JOIN playlist_tracks ON playlist_tracks.playlist_id = playlists.id
             GROUP BY playlists.id
             ORDER BY playlists.name COLLATE NOCASE",
        )
        .map_err(database_error)?;
    let rows = statement
        .query_map([], |row| {
            let track_count: i64 = row.get(2)?;
            Ok(PlaylistSummary {
                id: row.get(0)?,
                name: row.get(1)?,
                track_count: usize::try_from(track_count).unwrap_or_default(),
            })
        })
        .map_err(database_error)?;

    let mut playlists = Vec::new();
    for row in rows {
        playlists.push(row.map_err(database_error)?);
    }
    Ok(playlists)
}

#[instrument(skip(app))]
pub fn get_playlist(app: &AppHandle, playlist_id: i64) -> Result<PlaylistDetail, String> {
    let connection = open_database(app)?;
    initialize_database(&connection)?;
    let name = connection
        .query_row(
            "SELECT name FROM playlists WHERE id = ?1",
            params![playlist_id],
            |row| row.get::<_, String>(0),
        )
        .map_err(database_error)?;

    Ok(PlaylistDetail {
        id: playlist_id,
        name,
        tracks: playlist_tracks(&connection, playlist_id)?,
    })
}

#[instrument(skip(app))]
pub fn add_song_to_playlist(
    app: &AppHandle,
    playlist_id: i64,
    song: Song,
) -> Result<PlaylistDetail, String> {
    let connection = open_database(app)?;
    initialize_database(&connection)?;
    let next_position: i64 = connection
        .query_row(
            "SELECT COALESCE(MAX(position), -1) + 1 FROM playlist_tracks WHERE playlist_id = ?1",
            params![playlist_id],
            |row| row.get(0),
        )
        .map_err(database_error)?;
    connection
        .execute(
            "INSERT INTO playlist_tracks (
                playlist_id, position, path, title, artist, album, duration
             )
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                playlist_id,
                next_position,
                song.id,
                song.title,
                song.artist,
                song.album,
                i64::try_from(song.duration).unwrap_or(i64::MAX)
            ],
        )
        .map_err(database_error)?;
    get_playlist(app, playlist_id)
}

#[instrument(skip(app))]
pub fn import_m3u_playlist(
    app: &AppHandle,
    path: String,
    name: Option<String>,
) -> Result<PlaylistDetail, String> {
    let path = PathBuf::from(path.trim());
    if !path.is_file() {
        return Err(format!("Playlist file is not readable: {}", path.display()));
    }
    let content =
        fs::read_to_string(&path).map_err(|error| format!("Could not read playlist: {error}"))?;
    let playlist_name = name
        .and_then(|name| non_empty_string(name.trim()))
        .or_else(|| {
            path.file_stem()
                .and_then(|value| value.to_str())
                .map(str::to_string)
        })
        .ok_or_else(|| "Playlist name is required.".to_string())?;
    let detail = create_playlist(app, playlist_name)?;
    let base_dir = path.parent().unwrap_or_else(|| Path::new(""));

    for song in parse_m3u(&content, base_dir) {
        let _ = add_song_to_playlist(app, detail.id, song)?;
    }

    get_playlist(app, detail.id)
}

#[instrument(skip(app))]
pub fn export_m3u_playlist(app: &AppHandle, playlist_id: i64, path: String) -> Result<(), String> {
    let detail = get_playlist(app, playlist_id)?;
    let mut content = String::from("#EXTM3U\n");
    for song in detail.tracks {
        let duration_seconds = song.duration / 10_000_000;
        content.push_str(&format!("#EXTINF:{duration_seconds},{}\n", song.title));
        content.push_str(&song.id);
        content.push('\n');
    }

    fs::write(path.trim(), content).map_err(|error| format!("Could not write playlist: {error}"))
}

fn playlist_tracks(connection: &Connection, playlist_id: i64) -> Result<Vec<Song>, String> {
    let mut statement = connection
        .prepare(
            "SELECT path, title, artist, album, duration
             FROM playlist_tracks
             WHERE playlist_id = ?1
             ORDER BY position",
        )
        .map_err(database_error)?;
    let rows = statement
        .query_map(params![playlist_id], |row| {
            let duration: i64 = row.get(4)?;
            Ok(Song {
                id: row.get(0)?,
                title: row.get(1)?,
                artist: row.get(2)?,
                album: row.get(3)?,
                duration: usize::try_from(duration).unwrap_or_default(),
                source: None,
                uri: None,
                external_url: None,
                quality: None,
                playable: None,
            })
        })
        .map_err(database_error)?;

    let mut tracks = Vec::new();
    for row in rows {
        tracks.push(row.map_err(database_error)?);
    }
    Ok(tracks)
}

fn parse_m3u(content: &str, base_dir: &Path) -> Vec<Song> {
    let mut songs = Vec::new();
    let mut pending_title: Option<String> = None;
    let mut pending_duration: usize = 0;

    for line in content.lines().map(str::trim) {
        if line.is_empty() || line == "#EXTM3U" {
            continue;
        }
        if let Some(extinf) = line.strip_prefix("#EXTINF:") {
            let (duration, title) = parse_extinf(extinf);
            pending_duration = duration;
            pending_title = title;
            continue;
        }
        if line.starts_with('#') {
            continue;
        }

        let path = resolve_playlist_path(base_dir, line);
        let title = pending_title
            .take()
            .unwrap_or_else(|| title_from_path(&path));
        songs.push(Song {
            id: path.to_string_lossy().to_string(),
            title,
            artist: "Unknown artist".to_string(),
            album: String::new(),
            duration: pending_duration.saturating_mul(10_000_000),
            source: None,
            uri: None,
            external_url: None,
            quality: None,
            playable: None,
        });
        pending_duration = 0;
    }

    songs
}

fn parse_extinf(extinf: &str) -> (usize, Option<String>) {
    let (duration, title) = extinf.split_once(',').unwrap_or((extinf, ""));
    (
        duration
            .trim()
            .parse::<isize>()
            .ok()
            .and_then(|duration| usize::try_from(duration).ok())
            .unwrap_or_default(),
        non_empty_string(title.trim()),
    )
}

fn resolve_playlist_path(base_dir: &Path, path: &str) -> PathBuf {
    let path = PathBuf::from(path);
    if path.is_absolute() {
        path
    } else {
        base_dir.join(path)
    }
}

fn title_from_path(path: &Path) -> String {
    path.file_stem()
        .and_then(|value| value.to_str())
        .filter(|value| !value.is_empty())
        .unwrap_or("Untitled")
        .to_string()
}

fn normalize_playlist_name(name: &str) -> Result<String, String> {
    non_empty_string(name.trim()).ok_or_else(|| "Playlist name is required.".to_string())
}

fn non_empty_string(value: &str) -> Option<String> {
    if value.is_empty() {
        None
    } else {
        Some(value.to_string())
    }
}

fn open_database(app: &AppHandle) -> Result<Connection, String> {
    let data_dir = app
        .path()
        .app_data_dir()
        .map_err(|error| format!("Could not resolve app data directory: {error}"))?;
    fs::create_dir_all(&data_dir)
        .map_err(|error| format!("Could not create app data directory: {error}"))?;
    Connection::open(data_dir.join("playlists.sqlite")).map_err(database_error)
}

fn initialize_database(connection: &Connection) -> Result<(), String> {
    connection
        .execute_batch(
            "CREATE TABLE IF NOT EXISTS playlists (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL,
                created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
            );
            CREATE TABLE IF NOT EXISTS playlist_tracks (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                playlist_id INTEGER NOT NULL,
                position INTEGER NOT NULL,
                path TEXT NOT NULL,
                title TEXT NOT NULL,
                artist TEXT NOT NULL,
                album TEXT NOT NULL,
                duration INTEGER NOT NULL DEFAULT 0,
                added_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                FOREIGN KEY (playlist_id) REFERENCES playlists(id) ON DELETE CASCADE
            );",
        )
        .map_err(database_error)
}

fn database_error(error: rusqlite::Error) -> String {
    format!("Playlist database error: {error}")
}

#[cfg(test)]
mod tests {
    use super::{parse_extinf, parse_m3u};
    use std::path::Path;

    #[test]
    fn extinf_parses_duration_and_title() {
        assert_eq!(
            parse_extinf("123,Artist - Title"),
            (123, Some("Artist - Title".to_string()))
        );
    }

    #[test]
    fn m3u_parser_resolves_relative_paths() {
        let songs = parse_m3u(
            "#EXTM3U\n#EXTINF:5,Track One\nAlbum/track.flac\n",
            Path::new("/music"),
        );

        assert_eq!(songs.len(), 1);
        assert_eq!(songs[0].title, "Track One");
        assert_eq!(songs[0].id, "/music/Album/track.flac");
        assert_eq!(songs[0].duration, 50_000_000);
    }
}
