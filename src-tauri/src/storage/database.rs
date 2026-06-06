use std::convert::TryFrom;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64;
use lofty::file::{AudioFile, TaggedFileExt};
use lofty::prelude::Accessor;
use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};
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
    let mut zero_byte_count: usize = 0;

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

        let meta = match fs::metadata(path) {
            Ok(m) => m,
            Err(_) => {
                summary.skipped_files += 1;
                continue;
            }
        };
        if meta.len() == 0 {
            zero_byte_count += 1;
            summary.skipped_files += 1;
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

    if zero_byte_count > 0 {
        tracing::warn!(
            zero_byte_count,
            "Skipped zero-byte audio files during library scan"
        );
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
            "PRAGMA journal_mode=WAL;
             PRAGMA cache_size=-8000;
             CREATE TABLE IF NOT EXISTS tracks (
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
            );
             CREATE INDEX IF NOT EXISTS idx_tracks_artist ON tracks(artist);
             CREATE INDEX IF NOT EXISTS idx_tracks_album ON tracks(album);
             CREATE INDEX IF NOT EXISTS idx_tracks_title ON tracks(title);
             CREATE INDEX IF NOT EXISTS idx_tracks_file_path ON tracks(path);",
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

#[instrument(skip(app))]
pub fn get_tracks_page(
    app: &AppHandle,
    page: usize,
    per_page: usize,
) -> Result<Vec<LibraryTrack>, String> {
    let connection = open_database(app)?;
    initialize_database(&connection)?;
    let offset = page.saturating_mul(per_page);

    let mut statement = connection
        .prepare(
            "SELECT path, title, artist, album, genre, track_number, duration_ms,
                    sample_rate, bit_depth, bitrate, file_size, modified_secs, extension, has_artwork
             FROM tracks
             ORDER BY album COLLATE NOCASE, track_number, title COLLATE NOCASE
             LIMIT ?1 OFFSET ?2",
        )
        .map_err(database_error)?;

    let rows = statement
        .query_map(params![per_page as i64, offset as i64], |row| {
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

// ── Library Stats ──

use std::collections::HashMap;

#[derive(Clone, Debug, Serialize)]
pub struct LibraryStats {
    pub total_tracks: usize,
    pub total_albums: usize,
    pub total_artists: usize,
    pub total_duration_secs: u64,
    pub format_breakdown: HashMap<String, usize>,
    pub top_artists: Vec<ArtistStat>,
    pub top_albums: Vec<AlbumStat>,
    pub forgotten_count: usize,
}

#[derive(Clone, Debug, Serialize)]
pub struct ArtistStat {
    pub name: String,
    pub play_count: u64,
}

#[derive(Clone, Debug, Serialize)]
pub struct AlbumStat {
    pub name: String,
    pub artist: String,
    pub play_count: u64,
}

#[instrument(skip(app))]
pub fn get_library_stats(app: &AppHandle) -> Result<LibraryStats, String> {
    let tracks = list_library_tracks(app)?;

    let total_tracks = tracks.len();
    let total_duration_secs: u64 = tracks
        .iter()
        .filter_map(|t| t.duration_ms)
        .sum::<u64>()
        / 1000;

    let mut albums_set: HashMap<String, String> = HashMap::new();
    let mut artists_set: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut format_counts: HashMap<String, usize> = HashMap::new();

    for track in &tracks {
        let album_key = track
            .album
            .as_deref()
            .unwrap_or("Unknown Album")
            .to_string();
        albums_set.entry(album_key.clone()).or_insert(
            track
                .artist
                .as_deref()
                .unwrap_or("Unknown Artist")
                .to_string(),
        );

        if let Some(artist) = &track.artist {
            artists_set.insert(artist.to_lowercase());
        }

        *format_counts
            .entry(track.extension.to_uppercase())
            .or_insert(0) += 1;
    }

    let total_albums = albums_set.len();
    let total_artists = artists_set.len();

    // Top artists by play count from listening history
    let mut top_artists = Vec::new();
    if let Ok(history_connection) = listening_open_database(app) {
        let _ = listening_initialize_database(&history_connection);
        let mut stmt = history_connection
            .prepare(
                "SELECT t.artist, COUNT(*) as cnt
                 FROM listening_history lh
                 JOIN tracks t ON lh.path = t.path
                 WHERE lh.event = 'started' AND t.artist IS NOT NULL
                 GROUP BY t.artist
                 ORDER BY cnt DESC
                 LIMIT 5",
            )
            .map_err(database_error)?;
        let rows = stmt
            .query_map([], |row| {
                let count: i64 = row.get(1)?;
                Ok(ArtistStat {
                    name: row.get(0)?,
                    play_count: u64::try_from(count).unwrap_or_default(),
                })
            })
            .map_err(database_error)?;
        for row in rows {
            top_artists.push(row.map_err(database_error)?);
        }

        // Top albums by play count
        let mut top_albums = Vec::new();
        let mut album_stmt = history_connection
            .prepare(
                "SELECT t.album, t.artist, COUNT(*) as cnt
                 FROM listening_history lh
                 JOIN tracks t ON lh.path = t.path
                 WHERE lh.event = 'started' AND t.album IS NOT NULL
                 GROUP BY t.album, t.artist
                 ORDER BY cnt DESC
                 LIMIT 5",
            )
            .map_err(database_error)?;
        let album_rows = album_stmt
            .query_map([], |row| {
                let count: i64 = row.get(2)?;
                Ok(AlbumStat {
                    name: row.get(0)?,
                    artist: row.get::<_, Option<String>>(1)?.unwrap_or_default(),
                    play_count: u64::try_from(count).unwrap_or_default(),
                })
            })
            .map_err(database_error)?;
        for row in album_rows {
            top_albums.push(row.map_err(database_error)?);
        }

        // Forgotten tracks: tracks in library that have never been played or not played in 30 days
        let forgotten: i64 = history_connection
            .query_row(
                "SELECT COUNT(*)
                 FROM (
                     SELECT t.path
                     FROM tracks t
                     LEFT JOIN (
                         SELECT path, MAX(created_at) AS last_played
                         FROM listening_history
                         WHERE event = 'started'
                         GROUP BY path
                     ) lh ON t.path = lh.path
                     WHERE lh.last_played IS NULL
                        OR lh.last_played < datetime('now', '-30 days')
                 )",
                [],
                |row| row.get(0),
            )
            .unwrap_or(0);

        Ok(LibraryStats {
            total_tracks,
            total_albums,
            total_artists,
            total_duration_secs,
            format_breakdown: format_counts,
            top_artists,
            top_albums,
            forgotten_count: usize::try_from(forgotten).unwrap_or_default(),
        })
    } else {
        Ok(LibraryStats {
            total_tracks,
            total_albums,
            total_artists,
            total_duration_secs,
            format_breakdown: format_counts,
            top_artists: Vec::new(),
            top_albums: Vec::new(),
            forgotten_count: 0,
        })
    }
}

// ── Duplicate Finder ──

#[instrument(skip(app))]
pub fn find_duplicates(app: &AppHandle) -> Result<Vec<Vec<LibraryTrack>>, String> {
    let connection = open_database(app)?;
    initialize_database(&connection)?;

    let mut stmt = connection
        .prepare(
            "SELECT artist, album, title
             FROM tracks
             GROUP BY artist, album, title
             HAVING COUNT(*) > 1",
        )
        .map_err(database_error)?;

    let duplicate_keys: Vec<(Option<String>, Option<String>, String)> = stmt
        .query_map([], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?))
        })
        .map_err(database_error)?
        .filter_map(|r| r.ok())
        .collect();

    let mut groups = Vec::new();
    for (artist, album, title) in duplicate_keys {
        let mut detail_stmt = connection
            .prepare(
                "SELECT path, title, artist, album, genre, track_number, duration_ms,
                        sample_rate, bit_depth, bitrate, file_size, modified_secs, extension, has_artwork
                 FROM tracks
                 WHERE artist IS ?1 AND album IS ?2 AND LOWER(title) = LOWER(?3)
                 ORDER BY path",
            )
            .map_err(database_error)?;

        let rows = detail_stmt
            .query_map(
                rusqlite::params![artist.as_deref(), album.as_deref(), title],
                |row| {
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
                        duration_ms: duration_ms.and_then(|v| u64::try_from(v).ok()),
                        sample_rate: row.get(7)?,
                        bit_depth: row.get(8)?,
                        bitrate: row.get(9)?,
                        file_size: u64::try_from(file_size).unwrap_or_default(),
                        modified_secs: row.get(11)?,
                        extension: row.get(12)?,
                        has_artwork: has_artwork != 0,
                    })
                },
            )
            .map_err(database_error)?;

        let group: Vec<LibraryTrack> = rows.filter_map(|r| r.ok()).collect();
        if group.len() > 1 {
            groups.push(group);
        }
    }

    Ok(groups)
}

// ── Watch Folders ──

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

static WATCHER_RUNNING: AtomicBool = AtomicBool::new(false);

pub fn start_watcher(app: AppHandle, root: String) -> Result<(), String> {
    if WATCHER_RUNNING.swap(true, Ordering::SeqCst) {
        return Ok(());
    }

    let root_path = PathBuf::from(root.trim());
    if !root_path.is_dir() {
        WATCHER_RUNNING.store(false, Ordering::SeqCst);
        return Err(format!(
            "Watch path is not a directory: {}",
            root_path.display()
        ));
    }

    let root_clone = root_path.clone();
    tauri::async_runtime::spawn(async move {
        use notify::RecursiveMode;
        use notify::Watcher;

        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();

        let mut watcher = match notify::recommended_watcher(move |event| {
            let _ = tx.send(event);
        }) {
            Ok(w) => w,
            Err(e) => {
                tracing::error!("Failed to create file watcher: {e}");
                WATCHER_RUNNING.store(false, Ordering::SeqCst);
                return;
            }
        };

        if let Err(e) = watcher.watch(&root_clone, RecursiveMode::Recursive) {
            tracing::error!("Failed to watch directory: {e}");
            WATCHER_RUNNING.store(false, Ordering::SeqCst);
            return;
        }

        tracing::info!("File watcher started for: {}", root_clone.display());

        let mut debounce_timer: Option<tokio::time::Instant> = None;

        loop {
            match tokio::time::timeout(Duration::from_secs(1), rx.recv()).await {
                Ok(Some(_event)) => {
                    if let Ok(event) =
                        _event.map_err(|e| tracing::warn!("Watcher event error: {e}"))
                    {
                        if event.kind.is_create() || event.kind.is_modify() {
                            for path in &event.paths {
                                if path.is_file() && is_audio_path(path) {
                                    debounce_timer = Some(tokio::time::Instant::now());
                                    break;
                                }
                            }
                        }
                    }
                }
                Ok(None) => break,
                Err(_) => {
                    // Timeout: check if we need to debounce-scan
                }
            }

            if let Some(timer) = debounce_timer {
                if timer.elapsed() >= Duration::from_secs(2) {
                    tracing::info!(
                        "Watcher: new/modified audio files detected, rescanning library"
                    );
                    if let Err(e) = scan_library_path(
                        &app,
                        root_clone.to_string_lossy().to_string(),
                    ) {
                        tracing::error!("Watcher library scan failed: {e}");
                    }
                    debounce_timer = None;
                }
            }
        }

        WATCHER_RUNNING.store(false, Ordering::SeqCst);
    });

    Ok(())
}

pub fn is_watcher_running() -> bool {
    WATCHER_RUNNING.load(Ordering::SeqCst)
}

pub fn stop_watcher() {
    WATCHER_RUNNING.store(false, Ordering::SeqCst);
}

// ── Smart Playlists ──

#[derive(Clone, Debug, Serialize)]
pub struct SmartPlaylistSummary {
    pub id: i64,
    pub name: String,
    pub rules_json: String,
    pub is_template: bool,
    pub track_count: usize,
}

#[allow(dead_code)]
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SmartPlaylistRule {
    pub field: String,
    pub op: String,
    pub value: serde_json::Value,
}

#[allow(dead_code)]
#[derive(Clone, Debug, Serialize, Deserialize)]
struct SmartPlaylistRules {
    #[serde(default)]
    rules: Vec<SmartPlaylistRule>,
    #[serde(default)]
    combination: Option<Vec<SmartPlaylistRules>>,
    #[serde(default)]
    logic: Option<String>,
}

fn smart_playlists_open_database(app: &AppHandle) -> Result<Connection, String> {
    let data_dir = app
        .path()
        .app_data_dir()
        .map_err(|error| format!("Could not resolve app data directory: {error}"))?;
    fs::create_dir_all(&data_dir)
        .map_err(|error| format!("Could not create app data directory: {error}"))?;
    Connection::open(data_dir.join("smart_playlists.sqlite")).map_err(database_error)
}

fn smart_playlists_initialize_database(connection: &Connection) -> Result<(), String> {
    connection
        .execute_batch(
            "CREATE TABLE IF NOT EXISTS smart_playlists (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL,
                rules_json TEXT NOT NULL,
                is_template INTEGER NOT NULL DEFAULT 0,
                created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
            );"
        )
        .map_err(database_error)
}

#[instrument(skip(app))]
pub fn create_smart_playlist(app: &AppHandle, name: String, rules_json: String) -> Result<SmartPlaylistSummary, String> {
    let name = name.trim().to_string();
    if name.is_empty() {
        return Err("Smart playlist name is required.".to_string());
    }
    serde_json::from_str::<serde_json::Value>(&rules_json)
        .map_err(|e| format!("Invalid rules JSON: {e}"))?;
    let connection = smart_playlists_open_database(app)?;
    smart_playlists_initialize_database(&connection)?;
    connection
        .execute(
            "INSERT INTO smart_playlists (name, rules_json) VALUES (?1, ?2)",
            params![name, rules_json],
        )
        .map_err(database_error)?;
    let id = connection.last_insert_rowid();
    let tracks = evaluate_smart_playlist_rules(app, &rules_json)?;
    Ok(SmartPlaylistSummary {
        id,
        name,
        rules_json,
        is_template: false,
        track_count: tracks.len(),
    })
}

#[instrument(skip(app))]
pub fn list_smart_playlists(app: &AppHandle) -> Result<Vec<SmartPlaylistSummary>, String> {
    let _ = create_template_smart_playlists(app);
    let connection = smart_playlists_open_database(app)?;
    smart_playlists_initialize_database(&connection)?;
    let mut statement = connection
        .prepare(
            "SELECT id, name, rules_json, is_template FROM smart_playlists ORDER BY name COLLATE NOCASE"
        )
        .map_err(database_error)?;
    let rows = statement
        .query_map([], |row| {
            let is_template: i64 = row.get(3)?;
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                is_template != 0,
            ))
        })
        .map_err(database_error)?;

    let mut playlists = Vec::new();
    for row in rows {
        let (id, name, rules_json, is_template) = row.map_err(database_error)?;
        let tracks = evaluate_smart_playlist_rules(app, &rules_json).unwrap_or_default();
        playlists.push(SmartPlaylistSummary {
            id,
            name,
            rules_json,
            is_template,
            track_count: tracks.len(),
        });
    }
    Ok(playlists)
}

#[instrument(skip(app))]
pub fn get_smart_playlist_tracks(app: &AppHandle, playlist_id: i64) -> Result<Vec<LibraryTrack>, String> {
    let connection = smart_playlists_open_database(app)?;
    smart_playlists_initialize_database(&connection)?;
    let rules_json: String = connection
        .query_row(
            "SELECT rules_json FROM smart_playlists WHERE id = ?1",
            params![playlist_id],
            |row| row.get(0),
        )
        .map_err(|_| format!("Smart playlist {playlist_id} not found."))?;
    evaluate_smart_playlist_rules(app, &rules_json)
}

#[instrument(skip(app))]
pub fn delete_smart_playlist(app: &AppHandle, playlist_id: i64) -> Result<(), String> {
    let connection = smart_playlists_open_database(app)?;
    smart_playlists_initialize_database(&connection)?;
    connection
        .execute("DELETE FROM smart_playlists WHERE id = ?1 AND is_template = 0", params![playlist_id])
        .map_err(database_error)?;
    Ok(())
}

#[instrument(skip(app))]
pub fn create_template_smart_playlists(app: &AppHandle) -> Result<(), String> {
    let connection = smart_playlists_open_database(app)?;
    smart_playlists_initialize_database(&connection)?;
    let count: i64 = connection
        .query_row("SELECT COUNT(*) FROM smart_playlists WHERE is_template = 1", [], |r| r.get(0))
        .unwrap_or(0);
    if count > 0 {
        return Ok(());
    }

    let templates: &[(&str, &str)] = &[
        ("Recently Added", r#"{"rules":[{"field":"added_at","op":"gte","value":"now-30d"}]}"#),
        ("Never Played", r#"{"rules":[{"field":"play_count","op":"equals","value":0}]}"#),
        ("Top Rated", r#"{"rules":[{"field":"last_played","op":"gte","value":"now-90d"}]}"#),
        ("Forgotten", r#"{"rules":[{"field":"last_played","op":"lt","value":"now-30d"}]}"#),
    ];

    for (name, rules) in templates {
        connection
            .execute(
                "INSERT INTO smart_playlists (name, rules_json, is_template) VALUES (?1, ?2, 1)",
                params![name, rules],
            )
            .map_err(database_error)?;
    }
    Ok(())
}

fn evaluate_smart_playlist_rules(app: &AppHandle, rules_json: &str) -> Result<Vec<LibraryTrack>, String> {
    let parsed: serde_json::Value = serde_json::from_str(rules_json)
        .map_err(|e| format!("Invalid rules JSON: {e}"))?;
    let (where_clause, bind_values) = build_smart_playlist_sql(&parsed)?;
    let connection = open_database(app)?;
    initialize_database(&connection)?;

    let sql = format!(
        "SELECT t.path, t.title, t.artist, t.album, t.genre, t.track_number, t.duration_ms,
                t.sample_rate, t.bit_depth, t.bitrate, t.file_size, t.modified_secs, t.extension, t.has_artwork
         FROM tracks t
         LEFT JOIN (
             SELECT path, MAX(created_at) AS last_played, COUNT(*) AS play_count
             FROM listening_history
             WHERE event = 'started'
             GROUP BY path
         ) lh ON t.path = lh.path
         WHERE {}
         ORDER BY t.album COLLATE NOCASE, t.track_number, t.title COLLATE NOCASE
         LIMIT 500",
        where_clause
    );

    let mut statement = connection.prepare(&sql).map_err(database_error)?;
    let params_refs: Vec<&dyn rusqlite::types::ToSql> = bind_values
        .iter()
        .map(|v| v as &dyn rusqlite::types::ToSql)
        .collect();
    let rows = statement
        .query_map(params_refs.as_slice(), |row| {
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
                duration_ms: duration_ms.and_then(|v| u64::try_from(v).ok()),
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

fn build_smart_playlist_sql(value: &serde_json::Value) -> Result<(String, Vec<String>), String> {
    if let Some(rules) = value.get("rules").and_then(|v| v.as_array()) {
        let mut conditions = Vec::new();
        let mut bind_values = Vec::new();
        for rule in rules {
            let (cond, val) = build_rule_condition(rule)?;
            conditions.push(cond);
            if let Some(v) = val {
                bind_values.push(v);
            }
        }
        let logic = value
            .get("logic")
            .and_then(|v| v.as_str())
            .unwrap_or("AND");
        let clause = conditions.join(&format!(" {logic} "));
        return Ok((clause, bind_values));
    }

    if let Some(combination) = value.get("combination").and_then(|v| v.as_array()) {
        let mut conditions = Vec::new();
        let mut bind_values = Vec::new();
        for nested in combination {
            let (cond, vals) = build_smart_playlist_sql(nested)?;
            conditions.push(format!("({cond})"));
            bind_values.extend(vals);
        }
        let logic = value
            .get("logic")
            .and_then(|v| v.as_str())
            .unwrap_or("OR");
        let clause = conditions.join(&format!(" {logic} "));
        return Ok((clause, bind_values));
    }

    Err("Invalid smart playlist rules format.".to_string())
}

fn build_rule_condition(rule: &serde_json::Value) -> Result<(String, Option<String>), String> {
    let field = rule
        .get("field")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "Rule missing 'field'.".to_string())?;
    let op = rule
        .get("op")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "Rule missing 'op'.".to_string())?;
    let value = rule.get("value").ok_or_else(|| "Rule missing 'value'.".to_string())?;

    let db_field = match field {
        "genre" => "t.genre".to_string(),
        "artist" => "t.artist".to_string(),
        "album" => "t.album".to_string(),
        "title" => "t.title".to_string(),
        "year" => "t.modified_secs".to_string(),
        "play_count" => "COALESCE(lh.play_count, 0)".to_string(),
        "last_played" => "lh.last_played".to_string(),
        "duration" => "t.duration_ms".to_string(),
        "format" => "t.extension".to_string(),
        "added_at" => "t.added_at".to_string(),
        _ => return Err(format!("Unsupported field: {field}")),
    };

    match op {
        "equals" => {
            let v = value_to_sql_string(value);
            Ok((format!("{db_field} = ?1"), Some(v)))
        }
        "contains" => {
            let v = format!("%{}%", value_to_sql_string(value).trim_matches('\''));
            Ok((format!("{db_field} LIKE ?1"), Some(v)))
        }
        "gt" => {
            let v = value_to_sql_string(value);
            Ok((format!("{db_field} > ?1"), Some(v)))
        }
        "lt" => {
            let v = value_to_sql_string(value);
            Ok((format!("{db_field} < ?1"), Some(v)))
        }
        "gte" => {
            let v = value_to_sql_string(value);
            Ok((format!("{db_field} >= ?1"), Some(v)))
        }
        "lte" => {
            let v = value_to_sql_string(value);
            Ok((format!("{db_field} <= ?1"), Some(v)))
        }
        "between" => {
            let arr = value.as_array().ok_or("between requires array of two values")?;
            let v1 = value_to_sql_string(&arr[0]);
            let v2 = value_to_sql_string(&arr.get(1).unwrap_or(&arr[0]));
            Ok((format!("{db_field} BETWEEN {v1} AND {v2}"), None))
        }
        _ => Err(format!("Unsupported operator: {op}")),
    }
}

fn value_to_sql_string(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::String(s) => {
            let now_special = s.strip_prefix("now-");
            if let Some(rest) = now_special {
                let days: i64 = rest.trim_end_matches('d').parse().unwrap_or(30);
                format!("datetime('now', '-{days} days')")
            } else {
                format!("'{}'", s.replace('\'', "''"))
            }
        }
        serde_json::Value::Number(n) => n.to_string(),
        serde_json::Value::Bool(b) => (if *b { 1 } else { 0 }).to_string(),
        _ => value.to_string(),
    }
}

// ── Discovery Dashboard ──

#[derive(Clone, Debug, Serialize)]
pub struct DiscoverySection {
    pub label: String,
    pub tracks: Vec<LibraryTrack>,
}

#[derive(Clone, Debug, Serialize)]
pub struct DiscoveryResult {
    pub you_might_like: DiscoverySection,
    pub deep_cuts: DiscoverySection,
    pub new_additions: DiscoverySection,
}

#[instrument(skip(app))]
pub fn get_discovery_dashboard(app: &AppHandle) -> Result<DiscoveryResult, String> {
    let you_might_like = get_discover_you_might_like(app)?;
    let deep_cuts = get_discover_deep_cuts(app)?;
    let new_additions = get_discover_new_additions(app)?;
    Ok(DiscoveryResult {
        you_might_like,
        deep_cuts,
        new_additions,
    })
}

#[instrument(skip(app))]
pub fn get_discover_you_might_like(app: &AppHandle) -> Result<DiscoverySection, String> {
    let connection = open_database(app)?;
    initialize_database(&connection)?;
    let mut statement = connection
        .prepare(
            "SELECT DISTINCT t2.path, t2.title, t2.artist, t2.album, t2.genre, t2.track_number,
                    t2.duration_ms, t2.sample_rate, t2.bit_depth, t2.bitrate, t2.file_size,
                    t2.modified_secs, t2.extension, t2.has_artwork
             FROM listening_history lh
             JOIN tracks t ON lh.path = t.path AND t.artist IS NOT NULL
             JOIN tracks t2 ON t2.artist = t.artist AND t2.path != t.path
             WHERE lh.event = 'started'
             GROUP BY t2.path
             ORDER BY RANDOM()
             LIMIT 20"
        )
        .map_err(database_error)?;
    tracks_from_statement(&mut statement)
        .map(|tracks| DiscoverySection {
            label: "You Might Like".to_string(),
            tracks,
        })
}

#[instrument(skip(app))]
pub fn get_discover_deep_cuts(app: &AppHandle) -> Result<DiscoverySection, String> {
    let connection = open_database(app)?;
    initialize_database(&connection)?;
    let mut statement = connection
        .prepare(
            "SELECT t.path, t.title, t.artist, t.album, t.genre, t.track_number,
                    t.duration_ms, t.sample_rate, t.bit_depth, t.bitrate, t.file_size,
                    t.modified_secs, t.extension, t.has_artwork
             FROM tracks t
             WHERE t.artist IN (
                 SELECT DISTINCT t2.artist FROM listening_history lh2
                 JOIN tracks t2 ON lh2.path = t2.path AND t2.artist IS NOT NULL
                 WHERE lh2.event = 'started'
             )
             AND t.path NOT IN (
                 SELECT DISTINCT lh.path FROM listening_history lh WHERE lh.event = 'started'
             )
             ORDER BY RANDOM()
             LIMIT 20"
        )
        .map_err(database_error)?;
    tracks_from_statement(&mut statement)
        .map(|tracks| DiscoverySection {
            label: "Deep Cuts".to_string(),
            tracks,
        })
}

#[instrument(skip(app))]
pub fn get_discover_new_additions(app: &AppHandle) -> Result<DiscoverySection, String> {
    let connection = open_database(app)?;
    initialize_database(&connection)?;
    let mut statement = connection
        .prepare(
            "SELECT path, title, artist, album, genre, track_number, duration_ms,
                    sample_rate, bit_depth, bitrate, file_size, modified_secs, extension, has_artwork
             FROM tracks
             WHERE added_at IS NOT NULL
             ORDER BY added_at DESC
             LIMIT 20"
        )
        .map_err(database_error)?;
    tracks_from_statement(&mut statement)
        .map(|tracks| DiscoverySection {
            label: "New Additions".to_string(),
            tracks,
        })
}

fn tracks_from_statement(statement: &mut rusqlite::Statement) -> Result<Vec<LibraryTrack>, String> {
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
                duration_ms: duration_ms.and_then(|v| u64::try_from(v).ok()),
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

// ── Genre Radio ──

#[instrument(skip(app))]
pub fn get_random_tracks_by_genre(app: &AppHandle, genre: &str, limit: usize) -> Result<Vec<LibraryTrack>, String> {
    let connection = open_database(app)?;
    initialize_database(&connection)?;
    let mut statement = connection
        .prepare(
            "SELECT path, title, artist, album, genre, track_number, duration_ms,
                    sample_rate, bit_depth, bitrate, file_size, modified_secs, extension, has_artwork
             FROM tracks
             WHERE genre IS NOT NULL AND LOWER(genre) = LOWER(?1)
             ORDER BY RANDOM()
             LIMIT ?2"
        )
        .map_err(database_error)?;
    let rows = statement
        .query_map(params![genre, limit as i64], |row| {
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
                duration_ms: duration_ms.and_then(|v| u64::try_from(v).ok()),
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
