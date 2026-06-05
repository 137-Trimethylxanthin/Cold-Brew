use std::convert::TryFrom;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine;
use lofty::file::{AudioFile, TaggedFileExt};
use lofty::prelude::Accessor;
use rusqlite::{params, Connection};
use serde::Serialize;
use tauri::{AppHandle, Manager};
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

pub fn scan_library_path(app: &AppHandle, root: String) -> Result<ScanSummary, String> {
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

    Ok(summary)
}

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

pub fn get_track_cover_art(path: String) -> Result<CoverArt, String> {
    let file_path = Path::new(&path);
    if !file_path.is_file() {
        return Err(format!("File not found: {}", file_path.display()));
    }

    let tagged_file =
        lofty::read_from_path(file_path).map_err(|error| format!("Could not read audio file: {error}"))?;

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
        assert!(!is_audio_path(Path::new("album/cover.jpg")));
    }
}
