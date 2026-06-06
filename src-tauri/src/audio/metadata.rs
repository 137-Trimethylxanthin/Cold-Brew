use reqwest::StatusCode;
use serde::{Deserialize, Serialize};

const MUSICBRAINZ_RECORDING_SEARCH: &str = "https://musicbrainz.org/ws/2/recording/";
const USER_AGENT: &str = "Cold-Brew/0.1.0 (at.maxsitter.coldbrew)";

#[derive(Clone, Debug, Serialize)]
pub struct MetadataSuggestion {
    pub source: String,
    pub recording_mbid: String,
    pub title: String,
    pub artist: String,
    pub album: Option<String>,
    pub first_release_date: Option<String>,
    pub length_ms: Option<u64>,
    pub score: Option<u32>,
    pub disambiguation: Option<String>,
}

#[derive(Debug, Deserialize)]
struct MusicBrainzSearchResponse {
    recordings: Vec<MusicBrainzRecording>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
struct MusicBrainzRecording {
    id: String,
    title: String,
    length: Option<u64>,
    score: Option<u32>,
    disambiguation: Option<String>,
    first_release_date: Option<String>,
    artist_credit: Option<Vec<MusicBrainzArtistCredit>>,
    releases: Option<Vec<MusicBrainzRelease>>,
}

#[derive(Debug, Deserialize)]
struct MusicBrainzArtistCredit {
    name: String,
}

#[derive(Debug, Deserialize)]
struct MusicBrainzRelease {
    title: String,
    date: Option<String>,
}

pub async fn search_metadata_suggestions(
    title: String,
    artist: Option<String>,
    album: Option<String>,
    duration_ms: Option<u64>,
) -> Result<Vec<MetadataSuggestion>, String> {
    let Some(title) = non_empty_string(&title).map(str::to_string) else {
        return Ok(Vec::new());
    };
    let artist = artist
        .as_deref()
        .and_then(non_empty_string)
        .map(str::to_string);
    let album = album
        .as_deref()
        .and_then(non_empty_string)
        .map(str::to_string);

    let query = recording_query(&title, artist.as_deref(), album.as_deref());
    let response = reqwest::Client::new()
        .get(MUSICBRAINZ_RECORDING_SEARCH)
        .header(reqwest::header::USER_AGENT, USER_AGENT)
        .query(&[
            ("query", query),
            ("fmt", "json".to_string()),
            ("limit", "5".to_string()),
        ])
        .send()
        .await
        .map_err(|error| format!("Could not query MusicBrainz metadata: {error}"))?;

    if response.status() == StatusCode::TOO_MANY_REQUESTS
        || response.status() == StatusCode::SERVICE_UNAVAILABLE
    {
        return Err("MusicBrainz rate limit reached. Try again later.".to_string());
    }
    if !response.status().is_success() {
        return Err(format!(
            "MusicBrainz metadata request failed with HTTP {}.",
            response.status()
        ));
    }

    let search_response = response
        .json::<MusicBrainzSearchResponse>()
        .await
        .map_err(|error| format!("Could not parse MusicBrainz response: {error}"))?;

    Ok(search_response
        .recordings
        .into_iter()
        .map(to_metadata_suggestion)
        .map(|mut suggestion| {
            if let (Some(length_ms), Some(duration_ms)) = (suggestion.length_ms, duration_ms) {
                let distance = length_ms.abs_diff(duration_ms);
                if distance <= 2_000 {
                    suggestion.score = suggestion.score.map(|score| (score + 5).min(100));
                }
            }
            suggestion
        })
        .collect())
}

fn to_metadata_suggestion(recording: MusicBrainzRecording) -> MetadataSuggestion {
    let first_release = recording
        .releases
        .as_ref()
        .and_then(|releases| releases.first());
    MetadataSuggestion {
        source: "musicbrainz".to_string(),
        recording_mbid: recording.id,
        title: recording.title,
        artist: recording
            .artist_credit
            .unwrap_or_default()
            .into_iter()
            .map(|credit| credit.name)
            .collect::<Vec<_>>()
            .join(""),
        album: first_release.map(|release| release.title.clone()),
        first_release_date: recording
            .first_release_date
            .or_else(|| first_release.and_then(|release| release.date.clone())),
        length_ms: recording.length,
        score: recording.score,
        disambiguation: recording.disambiguation.and_then(|value| {
            let value = value.trim().to_string();
            (!value.is_empty()).then_some(value)
        }),
    }
}

fn recording_query(title: &str, artist: Option<&str>, album: Option<&str>) -> String {
    let mut parts = vec![format!("recording:\"{}\"", lucene_escape(title))];
    if let Some(artist) = artist {
        parts.push(format!("artist:\"{}\"", lucene_escape(artist)));
    }
    if let Some(album) = album {
        parts.push(format!("release:\"{}\"", lucene_escape(album)));
    }
    parts.join(" AND ")
}

fn lucene_escape(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

fn non_empty_string(value: &str) -> Option<&str> {
    let value = value.trim();
    if value.is_empty() {
        None
    } else {
        Some(value)
    }
}

#[cfg(test)]
mod tests {
    use super::{lucene_escape, recording_query};

    #[test]
    fn lucene_values_escape_quotes_and_backslashes() {
        assert_eq!(lucene_escape(r#"A "B"\ C"#), r#"A \"B\"\\ C"#);
    }

    #[test]
    fn recording_query_uses_available_fields() {
        assert_eq!(
            recording_query("Track", Some("Artist"), Some("Album")),
            r#"recording:"Track" AND artist:"Artist" AND release:"Album""#
        );
        assert_eq!(recording_query("Track", None, None), r#"recording:"Track""#);
    }
}
