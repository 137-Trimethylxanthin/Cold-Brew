use std::fs;
use std::path::{Path, PathBuf};

use lofty::file::TaggedFileExt;
use lofty::tag::ItemKey;
use reqwest::StatusCode;
use serde::{Deserialize, Serialize};

const LRCLIB_API_BASE: &str = "https://lrclib.net/api";
const USER_AGENT: &str = "Cold-Brew/0.1.0";

#[derive(Clone, Debug, Serialize)]
pub struct LyricsResult {
    pub source: String,
    pub synced: bool,
    pub content: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LrcLibRecord {
    track_name: String,
    artist_name: String,
    duration: u64,
    instrumental: bool,
    plain_lyrics: Option<String>,
    synced_lyrics: Option<String>,
}

pub fn get_local_lyrics(path: String) -> Result<Option<LyricsResult>, String> {
    let path = PathBuf::from(path.trim());
    if !path.is_file() {
        return Err(format!("Track path is not a file: {}", path.display()));
    }

    if let Some(lyrics) = embedded_lyrics(&path) {
        return Ok(Some(lyrics));
    }
    if let Some(lyrics) = sibling_lyrics(&path, "lrc", true)? {
        return Ok(Some(lyrics));
    }
    if let Some(lyrics) = sibling_lyrics(&path, "txt", false)? {
        return Ok(Some(lyrics));
    }

    Ok(None)
}

pub async fn get_track_lyrics(
    path: String,
    title: String,
    artist: Option<String>,
    album: Option<String>,
    duration_ms: Option<u64>,
) -> Result<Option<LyricsResult>, String> {
    if let Some(local_lyrics) = get_local_lyrics(path)? {
        return Ok(Some(local_lyrics));
    }

    get_lrclib_lyrics(title, artist, album, duration_ms).await
}

fn embedded_lyrics(path: &Path) -> Option<LyricsResult> {
    let tagged_file = lofty::read_from_path(path).ok()?;
    let tag = tagged_file
        .primary_tag()
        .or_else(|| tagged_file.first_tag())?;
    let content = tag
        .get_string(ItemKey::Lyrics)
        .or_else(|| tag.get_string(ItemKey::UnsyncLyrics))?;
    non_empty_string(content.trim()).map(|content| LyricsResult {
        source: "embedded".to_string(),
        synced: looks_synced(&content),
        content,
    })
}

fn sibling_lyrics(
    path: &Path,
    extension: &str,
    synced: bool,
) -> Result<Option<LyricsResult>, String> {
    let lyrics_path = path.with_extension(extension);
    if !lyrics_path.is_file() {
        return Ok(None);
    }
    let content = fs::read_to_string(&lyrics_path).map_err(|error| {
        format!(
            "Could not read lyrics file {}: {error}",
            lyrics_path.display()
        )
    })?;
    Ok(
        non_empty_string(content.trim()).map(|content| LyricsResult {
            source: lyrics_path.to_string_lossy().to_string(),
            synced,
            content,
        }),
    )
}

async fn get_lrclib_lyrics(
    title: String,
    artist: Option<String>,
    album: Option<String>,
    duration_ms: Option<u64>,
) -> Result<Option<LyricsResult>, String> {
    let title = non_empty_string(title.trim());
    let artist = artist
        .as_deref()
        .and_then(|value| non_empty_string(value.trim()));
    let album = album
        .as_deref()
        .and_then(|value| non_empty_string(value.trim()));
    let duration_seconds = duration_ms.map(duration_ms_to_seconds);

    let Some(title) = title else {
        return Ok(None);
    };

    let client = reqwest::Client::new();
    if let (Some(artist), Some(album), Some(duration_seconds)) =
        (artist.as_deref(), album.as_deref(), duration_seconds)
    {
        if let Some(record) =
            request_lrclib_exact(&client, &title, artist, album, duration_seconds).await?
        {
            return Ok(lyrics_from_lrclib_record(record, "lrclib"));
        }
    }

    let Some(artist) = artist.as_deref() else {
        return Ok(None);
    };
    let records = request_lrclib_search(&client, &title, artist).await?;
    Ok(
        select_best_lrclib_record(records, &title, artist, duration_seconds)
            .and_then(|record| lyrics_from_lrclib_record(record, "lrclib search")),
    )
}

async fn request_lrclib_exact(
    client: &reqwest::Client,
    title: &str,
    artist: &str,
    album: &str,
    duration_seconds: u64,
) -> Result<Option<LrcLibRecord>, String> {
    let response = client
        .get(format!("{LRCLIB_API_BASE}/get"))
        .header(reqwest::header::USER_AGENT, USER_AGENT)
        .query(&[
            ("track_name", title.to_string()),
            ("artist_name", artist.to_string()),
            ("album_name", album.to_string()),
            ("duration", duration_seconds.to_string()),
        ])
        .send()
        .await
        .map_err(|error| format!("Could not query LRCLIB lyrics: {error}"))?;

    if response.status() == StatusCode::NOT_FOUND {
        return Ok(None);
    }
    if !response.status().is_success() {
        return Err(format!(
            "LRCLIB lyrics request failed with HTTP {}.",
            response.status()
        ));
    }

    response
        .json::<LrcLibRecord>()
        .await
        .map(Some)
        .map_err(|error| format!("Could not read LRCLIB lyrics response: {error}"))
}

async fn request_lrclib_search(
    client: &reqwest::Client,
    title: &str,
    artist: &str,
) -> Result<Vec<LrcLibRecord>, String> {
    let response = client
        .get(format!("{LRCLIB_API_BASE}/search"))
        .header(reqwest::header::USER_AGENT, USER_AGENT)
        .query(&[
            ("track_name", title.to_string()),
            ("artist_name", artist.to_string()),
        ])
        .send()
        .await
        .map_err(|error| format!("Could not search LRCLIB lyrics: {error}"))?;

    if !response.status().is_success() {
        return Err(format!(
            "LRCLIB lyrics search failed with HTTP {}.",
            response.status()
        ));
    }

    response
        .json::<Vec<LrcLibRecord>>()
        .await
        .map_err(|error| format!("Could not read LRCLIB search response: {error}"))
}

fn select_best_lrclib_record(
    records: Vec<LrcLibRecord>,
    title: &str,
    artist: &str,
    duration_seconds: Option<u64>,
) -> Option<LrcLibRecord> {
    records
        .into_iter()
        .filter(has_lyrics)
        .max_by_key(|record| lrclib_match_score(record, title, artist, duration_seconds))
}

fn lrclib_match_score(
    record: &LrcLibRecord,
    title: &str,
    artist: &str,
    duration_seconds: Option<u64>,
) -> i64 {
    let mut score = 0;
    let record_title = normalized_text(&record.track_name);
    let record_artist = normalized_text(&record.artist_name);
    let title = normalized_text(title);
    let artist = normalized_text(artist);

    if record_title == title {
        score += 50;
    } else if record_title.contains(&title) || title.contains(&record_title) {
        score += 20;
    }

    if record_artist == artist {
        score += 35;
    } else if record_artist.contains(&artist) || artist.contains(&record_artist) {
        score += 15;
    }

    if let Some(duration_seconds) = duration_seconds {
        let difference = record.duration.abs_diff(duration_seconds);
        if difference <= 2 {
            score += 30;
        } else if difference <= 5 {
            score += 12;
        } else {
            score -= 25;
        }
    }

    if record
        .synced_lyrics
        .as_deref()
        .and_then(|content| non_empty_string(content.trim()))
        .is_some()
    {
        score += 4;
    } else if record
        .plain_lyrics
        .as_deref()
        .and_then(|content| non_empty_string(content.trim()))
        .is_some()
    {
        score += 1;
    }

    score
}

fn lyrics_from_lrclib_record(record: LrcLibRecord, source: &str) -> Option<LyricsResult> {
    if let Some(content) = record
        .synced_lyrics
        .as_deref()
        .and_then(|content| non_empty_string(content.trim()))
    {
        return Some(LyricsResult {
            source: source.to_string(),
            synced: true,
            content,
        });
    }
    if let Some(content) = record
        .plain_lyrics
        .as_deref()
        .and_then(|content| non_empty_string(content.trim()))
    {
        return Some(LyricsResult {
            source: source.to_string(),
            synced: false,
            content,
        });
    }
    record.instrumental.then(|| LyricsResult {
        source: source.to_string(),
        synced: false,
        content: "Instrumental".to_string(),
    })
}

fn has_lyrics(record: &LrcLibRecord) -> bool {
    record.instrumental
        || record
            .synced_lyrics
            .as_deref()
            .and_then(|content| non_empty_string(content.trim()))
            .is_some()
        || record
            .plain_lyrics
            .as_deref()
            .and_then(|content| non_empty_string(content.trim()))
            .is_some()
}

fn duration_ms_to_seconds(duration_ms: u64) -> u64 {
    (duration_ms + 500) / 1000
}

fn normalized_text(value: &str) -> String {
    value
        .chars()
        .filter_map(|character| {
            if character.is_ascii_alphanumeric() {
                Some(character.to_ascii_lowercase())
            } else if character.is_whitespace() {
                Some(' ')
            } else {
                None
            }
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn looks_synced(content: &str) -> bool {
    content.lines().any(|line| {
        let line = line.trim_start();
        line.starts_with('[')
            && line
                .chars()
                .skip(1)
                .take(5)
                .any(|character| character == ':')
    })
}

fn non_empty_string(value: &str) -> Option<String> {
    if value.is_empty() {
        None
    } else {
        Some(value.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::{duration_ms_to_seconds, looks_synced, lrclib_match_score, LrcLibRecord};

    #[test]
    fn synced_lyrics_detect_timestamp_lines() {
        assert!(looks_synced("[00:12.34] lyric line"));
        assert!(!looks_synced("plain lyric line"));
    }

    #[test]
    fn duration_milliseconds_are_rounded_to_seconds() {
        assert_eq!(duration_ms_to_seconds(232_499), 232);
        assert_eq!(duration_ms_to_seconds(232_500), 233);
    }

    #[test]
    fn lrclib_score_prefers_exact_duration_matches() {
        let exact = LrcLibRecord {
            track_name: "Track".to_string(),
            artist_name: "Artist".to_string(),
            duration: 200,
            instrumental: false,
            plain_lyrics: None,
            synced_lyrics: Some("[00:01.00] line".to_string()),
        };
        let wrong_duration = LrcLibRecord {
            duration: 260,
            ..exact.clone()
        };

        assert!(
            lrclib_match_score(&exact, "Track", "Artist", Some(200))
                > lrclib_match_score(&wrong_duration, "Track", "Artist", Some(200))
        );
    }
}
