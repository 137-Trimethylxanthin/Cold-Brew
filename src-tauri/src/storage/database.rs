use std::convert::TryFrom;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64;
use lofty::file::{AudioFile, TaggedFileExt};
use lofty::prelude::Accessor;
use rusqlite::{Connection, params};
use serde::Serialize;
use tauri::{AppHandle, Manager};
use tracing::instrument;
use walkdir::WalkDir;

const AUDIO_EXTENSIONS: &[&str] = &[
    "aac", "aif", "aiff", "alac", "flac", "m4a", "mp3", "ogg", "opus", "wav",
];

#[derive(Clone, Debug, Serialize)]
pub struct LibraryTrack {
    pub path: String,
    pub title: String,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub genre: Option<String>,
    pub track_number: Option<u32>,
    pub duration_ms: Option<u64>,
    pub sample_rate: Option<u32>,
    pub bit_depth: Option<u8>,
    pub bitrate: Option<u32>,
    pub file_size: u64,
    pub modified_secs: Option<i64>,
    pub extension: String,
    pub has_artwork: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct ScanSummary {
    pub root: String,
    pub scanned_files: usize,
    pub indexed_tracks: usize,
    pub skipped_files: usize,
    pub tracks: Vec<LibraryTrack>,
}

#[instrument(skip(app))]
pub fn scan_library_path(app: &AppHandle, root: String) -> Result<ScanSummary, String> {
    tracing::info!("Scanning library at: {root}");
    let root_path = PathBuf::from(root.trim());
    if !root_path.is_dir() {
        return Err(format!(
            "Library path is not a readable directory: {}",
            root_path.display()
        ));
    }

    let connection = open_database(app)?;
    initialize_database(&connection)?;

    let mut summary = ScanSummary {
        root: root_path.to_string_lossy().to_string(),
        scanned_files: 0,
        indexed_tracks: 0,
        skipped_files: 0,
        tracks: Vec::new(),
    };

    for entry in WalkDir::new(&root_path).follow_links(false) {
        let entry = match entry {
            Ok(entry) => entry,
            Err(_) => {
                summary.skipped_files += 1;
                continue;
            }
        };
        let path = entry.path();

        if !path.is_file() || !is_audio_path(path) {
            continue;
        }

        summary.scanned_files += 1;
        match read_track(path) {
            Ok(track) => {
                upsert_track(&connection, &track)?;
                summary.indexed_tracks += 1;
                summary.tracks.push(track);
            }
            Err(_) => {
                summary.skipped_files += 1;
            }
        }
    }

    tracing::info!(
        scanned = summary.scanned_files,
        indexed = summary.indexed_tracks,
        skipped = summary.skipped_files,
        "Library scan complete"
    );
    Ok(summary)
}

#[instrument(skip(app))]
pub fn list_library_tracks(app: &AppHandle) -> Result<Vec<LibraryTrack>, String> {
    let connection = open_database(app)?;
    initialize_database(&connection)?;

    let mut statement = connection
        .prepare(
            "SELECT path, title, artist, album, genre, track_number, duration_ms,
                    sample_rate, bit_depth, bitrate, file_size, modified_secs, extension, has_artwork
             FROM tracks
             ORDER BY album COLLATE NOCASE, track_number, title COLLATE NOCASE",
        )
        .map_err(database_error)?;

    let rows = statement
        .query_map([], |row| {
            let duration_ms: Option<i64> = row.get(6)?;
            let file_size: i64 = row.get(10)?;
            let has_artwork: i64 = row.get(13)?;

            Ok(LibraryTrack {
                path: row.get(0)?,
                title: row.get(1)?,
                artist: row.get(2)?,
                album: row.get(3)?,
                genre: row.get(4)?,
                track_number: row.get(5)?,
                duration_ms: duration_ms.and_then(|value| u64::try_from(value).ok()),
                sample_rate: row.get(7)?,
                bit_depth: row.get(8)?,
                bitrate: row.get(9)?,
                file_size: u64::try_from(file_size).unwrap_or_default(),
                modified_secs: row.get(11)?,
                extension: row.get(12)?,
                has_artwork: has_artwork != 0,
            })
        })
        .map_err(database_error)?;

    let mut tracks = Vec::new();
    for row in rows {
        tracks.push(row.map_err(database_error)?);
    }
    Ok(tracks)
}

#[instrument(skip(app))]
pub fn get_library_track_by_path(
    app: &AppHandle,
    path: &str,
) -> Result<Option<LibraryTrack>, String> {
    let connection = open_database(app)?;
    initialize_database(&connection)?;

    let mut statement = connection
        .prepare(
            "SELECT path, title, artist, album, genre, track_number, duration_ms,
                    sample_rate, bit_depth, bitrate, file_size, modified_secs, extension, has_artwork
             FROM tracks
             WHERE path = ?1",
        )
        .map_err(database_error)?;
    let mut rows = statement.query(params![path]).map_err(database_error)?;
    let Some(row) = rows.next().map_err(database_error)? else {
        return Ok(None);
    };

    row_to_library_track(row).map(Some).map_err(database_error)
}

#[instrument]
fn read_track(path: &Path) -> Result<LibraryTrack, String> {
    let metadata = fs::metadata(path).map_err(|error| format!("{}: {error}", path.display()))?;
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    let fallback_title = path
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("Untitled")
        .to_string();

    let mut track = LibraryTrack {
        path: path.to_string_lossy().to_string(),
        title: fallback_title,
        artist: None,
        album: None,
        genre: None,
        track_number: None,
        duration_ms: None,
        sample_rate: None,
        bit_depth: None,
        bitrate: None,
        file_size: metadata.len(),
        modified_secs: metadata.modified().ok().and_then(|modified| {
            modified
                .duration_since(UNIX_EPOCH)
                .ok()
                .and_then(|duration| i64::try_from(duration.as_secs()).ok())
        }),
        extension,
        has_artwork: false,
    };

    if let Ok(tagged_file) = lofty::read_from_path(path) {
        let tag = tagged_file
            .primary_tag()
            .or_else(|| tagged_file.first_tag());
        if let Some(tag) = tag {
            if let Some(title) = tag.title() {
                track.title = title.to_string();
            }
            track.artist = tag.artist().map(|value| value.to_string());
            track.album = tag.album().map(|value| value.to_string());
            track.genre = tag.genre().map(|value| value.to_string());
            track.track_number = tag.track();
            track.has_artwork = !tag.pictures().is_empty();
        }

        let properties = tagged_file.properties();
        track.duration_ms = u64::try_from(properties.duration().as_millis()).ok();
        track.sample_rate = properties.sample_rate();
        track.bit_depth = properties.bit_depth();
        track.bitrate = properties
            .audio_bitrate()
            .or_else(|| properties.overall_bitrate());
    }

    Ok(track)
}

fn row_to_library_track(row: &rusqlite::Row<'_>) -> rusqlite::Result<LibraryTrack> {
    let duration_ms: Option<i64> = row.get(6)?;
    let file_size: i64 = row.get(10)?;
    let has_artwork: i64 = row.get(13)?;

    Ok(LibraryTrack {
        path: row.get(0)?,
        title: row.get(1)?,
        artist: row.get(2)?,
        album: row.get(3)?,
        genre: row.get(4)?,
        track_number: row.get(5)?,
        duration_ms: duration_ms.and_then(|value| u64::try_from(value).ok()),
        sample_rate: row.get(7)?,
        bit_depth: row.get(8)?,
        bitrate: row.get(9)?,
        file_size: u64::try_from(file_size).unwrap_or_default(),
        modified_secs: row.get(11)?,
        extension: row.get(12)?,
        has_artwork: has_artwork != 0,
    })
}

fn is_audio_path(path: &Path) -> bool {
    path.extension()
        .and_then(|value| value.to_str())
        .map(|extension| AUDIO_EXTENSIONS.contains(&extension.to_ascii_lowercase().as_str()))
        .unwrap_or(false)
}

fn open_database(app: &AppHandle) -> Result<Connection, String> {
    let data_dir = app
        .path()
        .app_data_dir()
        .map_err(|error| format!("Could not resolve app data directory: {error}"))?;
    fs::create_dir_all(&data_dir)
        .map_err(|error| format!("Could not create app data directory: {error}"))?;
    Connection::open(data_dir.join("library.sqlite")).map_err(database_error)
}

fn initialize_database(connection: &Connection) -> Result<(), String> {
    connection
        .execute_batch(
            "CREATE TABLE IF NOT EXISTS tracks (
                path TEXT PRIMARY KEY NOT NULL,
                title TEXT NOT NULL,
                artist TEXT,
                album TEXT,
                genre TEXT,
                track_number INTEGER,
                duration_ms INTEGER,
                sample_rate INTEGER,
                bit_depth INTEGER,
                bitrate INTEGER,
                file_size INTEGER NOT NULL,
                modified_secs INTEGER,
                extension TEXT NOT NULL,
                has_artwork INTEGER NOT NULL DEFAULT 0,
                source TEXT NOT NULL DEFAULT 'local',
                added_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
            );",
        )
        .map_err(database_error)
}

fn upsert_track(connection: &Connection, track: &LibraryTrack) -> Result<(), String> {
    connection
        .execute(
            "INSERT INTO tracks (
                path, title, artist, album, genre, track_number, duration_ms,
                sample_rate, bit_depth, bitrate, file_size, modified_secs, extension, has_artwork
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)
            ON CONFLICT(path) DO UPDATE SET
                title = excluded.title,
                artist = excluded.artist,
                album = excluded.album,
                genre = excluded.genre,
                track_number = excluded.track_number,
                duration_ms = excluded.duration_ms,
                sample_rate = excluded.sample_rate,
                bit_depth = excluded.bit_depth,
                bitrate = excluded.bitrate,
                file_size = excluded.file_size,
                modified_secs = excluded.modified_secs,
                extension = excluded.extension,
                has_artwork = excluded.has_artwork,
                updated_at = CURRENT_TIMESTAMP",
            params![
                track.path,
                track.title,
                track.artist,
                track.album,
                track.genre,
                track.track_number,
                track
                    .duration_ms
                    .and_then(|value| i64::try_from(value).ok()),
                track.sample_rate,
                track.bit_depth,
                track.bitrate,
                i64::try_from(track.file_size).unwrap_or(i64::MAX),
                track.modified_secs,
                track.extension,
                if track.has_artwork { 1 } else { 0 },
            ],
        )
        .map(|_| ())
        .map_err(database_error)
}

fn database_error(error: rusqlite::Error) -> String {
    format!("Library database error: {error}")
}

#[derive(Clone, Debug, Serialize)]
pub struct CoverArt {
    pub mime_type: String,
    pub data: String,
}

#[instrument]
pub fn get_track_cover_art(path: String) -> Result<CoverArt, String> {
    let file_path = Path::new(&path);
    if !file_path.is_file() {
        return Err(format!("File not found: {}", file_path.display()));
    }

    let tagged_file = lofty::read_from_path(file_path)
        .map_err(|error| format!("Could not read audio file: {error}"))?;

    let tag = tagged_file
        .primary_tag()
        .or_else(|| tagged_file.first_tag());

    let tag = tag.ok_or_else(|| "No metadata tags found in the audio file".to_string())?;

    let pictures = tag.pictures();
    let picture = pictures
        .first()
        .ok_or_else(|| "No embedded artwork found in the audio file".to_string())?;

    let mime_type = picture
        .mime_type()
        .map(|mt| mt.as_str().to_string())
        .unwrap_or_else(|| "image/jpeg".to_string());

    let b64 = BASE64.encode(picture.data());

    Ok(CoverArt {
        mime_type,
        data: b64,
    })
}

#[cfg(test)]
mod tests {
    use super::is_audio_path;
    use std::path::Path;

    #[test]
    fn recognizes_supported_audio_extensions() {
        assert!(is_audio_path(Path::new("album/song.FLAC")));
        assert!(is_audio_path(Path::new("album/song.m4a")));
        assert!(is_audio_path(Path::new("album/song.opus")));
        assert!(is_audio_path(Path::new("album/song.aiff")));
        assert!(is_audio_path(Path::new("album/song.aif")));
        assert!(!is_audio_path(Path::new("album/cover.jpg")));
    }
}



use crate::audio::player::Song;

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
    let connection = playlists_open_database(app)?;
    playlists_initialize_database(&connection)?;
    connection
        .execute("INSERT INTO playlists (name) VALUES (?1)", params![name])
        .map_err(database_error)?;
    let id = connection.last_insert_rowid();
    get_playlist(app, id)
}

#[instrument(skip(app))]
pub fn list_playlists(app: &AppHandle) -> Result<Vec<PlaylistSummary>, String> {
    let connection = playlists_open_database(app)?;
    playlists_initialize_database(&connection)?;
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
    let connection = playlists_open_database(app)?;
    playlists_initialize_database(&connection)?;
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
    let connection = playlists_open_database(app)?;
    playlists_initialize_database(&connection)?;
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

fn playlists_open_database(app: &AppHandle) -> Result<Connection, String> {
    let data_dir = app
        .path()
        .app_data_dir()
        .map_err(|error| format!("Could not resolve app data directory: {error}"))?;
    fs::create_dir_all(&data_dir)
        .map_err(|error| format!("Could not create app data directory: {error}"))?;
    Connection::open(data_dir.join("playlists.sqlite")).map_err(database_error)
}

fn playlists_initialize_database(connection: &Connection) -> Result<(), String> {
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

#[allow(dead_code)]
fn playlists_database_error(error: rusqlite::Error) -> String {
    format!("Playlist database error: {error}")
}

#[cfg(test)]
mod playlists_tests {
    use std::path::Path;
    use super::{parse_extinf, parse_m3u};

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



use crate::audio::player::PlaybackStatus;

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
    let connection = listening_open_database(app)?;
    listening_initialize_database(&connection)?;
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
    let connection = listening_open_database(app)?;
    listening_initialize_database(&connection)?;
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
    let connection = listening_open_database(app)?;
    listening_initialize_database(&connection)?;
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

fn listening_open_database(app: &AppHandle) -> Result<Connection, String> {
    let data_dir = app
        .path()
        .app_data_dir()
        .map_err(|error| format!("Could not resolve app data directory: {error}"))?;
    fs::create_dir_all(&data_dir)
        .map_err(|error| format!("Could not create app data directory: {error}"))?;
    Connection::open(data_dir.join("listening_history.sqlite")).map_err(database_error)
}

fn listening_initialize_database(connection: &Connection) -> Result<(), String> {
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
        Err(error) => Err(listening_database_error(error)),
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

    if position_ms < 5_000 { 0 } else { position_ms }
}

fn listening_database_error(error: rusqlite::Error) -> String {
    format!("Listening history database error: {error}")
}

#[cfg(test)]
mod listening_history_tests {
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
