use serde::{Deserialize, Serialize};

use crate::web::auth;

const SOUNDCLOUD_API_BASE: &str = "https://api-v2.soundcloud.com";
const SOUNDCLOUD_WELL_KNOWN_CLIENT_ID: &str = "a3e059563d7fd3372b49b37f00a00bcf";
const SOUNDCLOUD_BASE_URL: &str = "https://soundcloud.com";
const USER_AGENT: &str = "Cold-Brew/0.1.0";

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SoundCloudTrack {
    pub id: u64,
    pub title: String,
    pub artist: String,
    pub artist_url: Option<String>,
    pub album: Option<String>,
    pub duration_ms: u64,
    pub external_url: String,
    pub permalink_url: Option<String>,
    pub preview_url: Option<String>,
    pub streamable: bool,
    pub artwork_url: Option<String>,
    pub waveform_url: Option<String>,
    pub genre: Option<String>,
    pub play_count: Option<u64>,
    pub likes_count: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct SoundCloudSearchResponse {
    #[serde(default)]
    collection: Vec<SoundCloudTrackRaw>,
}

#[derive(Debug, Deserialize)]
struct SoundCloudTrackRaw {
    #[serde(default)]
    id: u64,
    #[serde(default)]
    title: String,
    #[serde(default)]
    permalink_url: Option<String>,
    #[serde(default)]
    duration: u64,
    #[serde(default)]
    streamable: Option<bool>,
    #[serde(default)]
    stream_url: Option<String>,
    #[serde(default)]
    artwork_url: Option<String>,
    #[serde(default)]
    waveform_url: Option<String>,
    #[serde(default)]
    genre: Option<String>,
    #[serde(default)]
    playback_count: Option<u64>,
    #[serde(default)]
    likes_count: Option<u64>,
    #[serde(default)]
    user: Option<SoundCloudUserRaw>,
    #[serde(rename = "media")]
    #[serde(default)]
    media: Option<SoundCloudMedia>,
}

#[derive(Debug, Deserialize)]
struct SoundCloudUserRaw {
    #[serde(default)]
    id: u64,
    #[serde(default)]
    username: String,
    #[serde(default)]
    permalink_url: Option<String>,
    #[serde(default)]
    avatar_url: Option<String>,
}

#[derive(Debug, Deserialize)]
struct SoundCloudMedia {
    #[serde(default)]
    transcodings: Vec<SoundCloudTranscoding>,
}

#[derive(Debug, Deserialize)]
struct SoundCloudTranscoding {
    #[serde(default)]
    url: String,
    #[serde(default)]
    preset: Option<String>,
    #[serde(default)]
    format: Option<SoundCloudFormat>,
}

#[derive(Debug, Deserialize)]
struct SoundCloudFormat {
    #[serde(default)]
    protocol: String,
    #[serde(default)]
    mime_type: String,
}

impl SoundCloudTrack {
    pub fn open_url(&self) -> &str {
        self.permalink_url
            .as_deref()
            .unwrap_or(&self.external_url)
    }

    pub fn artwork_url_500(&self) -> Option<String> {
        self.artwork_url
            .as_ref()
            .map(|url| url.replace("-large", "-t500x500"))
    }
}

fn soundcloud_client_id() -> Result<String, String> {
    if let Some(secrets) = auth::load_provider_secrets("soundcloud")? {
        if let Some(api_key) = secrets
            .api_key
            .map(|v| v.trim().to_string())
            .filter(|v| !v.is_empty())
        {
            return Ok(api_key);
        }
        if let Some(client_id) = secrets
            .client_id
            .map(|v| v.trim().to_string())
            .filter(|v| !v.is_empty())
        {
            return Ok(client_id);
        }
    }

    Ok(SOUNDCLOUD_WELL_KNOWN_CLIENT_ID.to_string())
}

pub async fn search_soundcloud_tracks(
    query: String,
    limit: Option<u32>,
) -> Result<Vec<SoundCloudTrack>, String> {
    let query = query.trim();
    if query.is_empty() {
        return Ok(Vec::new());
    }

    let client_id = soundcloud_client_id()?;
    let limit = limit.unwrap_or(10).clamp(1, 20);

    let url = format!(
        "{}/search/tracks?q={}&client_id={}&limit={}&offset=0",
        SOUNDCLOUD_API_BASE,
        urlencoding(&query),
        client_id,
        limit
    );

    let response = reqwest::Client::new()
        .get(&url)
        .header("User-Agent", USER_AGENT)
        .header("Accept", "application/json")
        .send()
        .await
        .map_err(|error| format!("SoundCloud search failed: {error}"))?;

    let status = response.status();
    if !status.is_success() {
        auth::record_provider_auth_failure(
            "soundcloud",
            format!("SoundCloud search failed with HTTP {status}"),
        );
        return Err(format!("SoundCloud search failed with HTTP {status}"));
    }
    auth::clear_provider_auth_failure("soundcloud");

    let search_response = response
        .json::<SoundCloudSearchResponse>()
        .await
        .map_err(|error| format!("Could not parse SoundCloud search response: {error}"))?;

    let tracks: Vec<SoundCloudTrack> = search_response
        .collection
        .into_iter()
        .filter_map(|raw| {
            if raw.id == 0 {
                return None;
            }

            let artist = raw
                .user
                .as_ref()
                .map(|u| u.username.clone())
                .unwrap_or_else(|| "SoundCloud Artist".to_string());

            let artist_url = raw
                .user
                .as_ref()
                .and_then(|u| u.permalink_url.clone());

            let preview_url = raw
                .media
                .as_ref()
                .and_then(|media| {
                    media
                        .transcodings
                        .iter()
                        .find(|t| {
                            t.format
                                .as_ref()
                                .map_or(false, |f| f.protocol == "progressive")
                        })
                        .map(|t| format!("{}?client_id={}", t.url, client_id))
                });

            let external_url = raw
                .permalink_url
                .clone()
                .unwrap_or_else(|| format!("{}/{}/{}", SOUNDCLOUD_BASE_URL, artist, raw.title));

            let title = if raw.title.is_empty() {
                "Untitled Track".to_string()
            } else {
                raw.title
            };

            Some(SoundCloudTrack {
                id: raw.id,
                title,
                artist,
                artist_url,
                album: None,
                duration_ms: raw.duration,
                external_url: external_url.clone(),
                permalink_url: raw.permalink_url,
                preview_url,
                streamable: raw.streamable.unwrap_or(false),
                artwork_url: raw.artwork_url,
                waveform_url: raw.waveform_url,
                genre: raw.genre,
                play_count: raw.playback_count,
                likes_count: raw.likes_count,
            })
        })
        .collect();

    Ok(tracks)
}

pub async fn get_soundcloud_preview_url(track_id: u64) -> Result<Option<String>, String> {
    let client_id = soundcloud_client_id()?;
    let url = format!(
        "{}/tracks/{}?client_id={}",
        SOUNDCLOUD_API_BASE, track_id, client_id
    );

    let response = reqwest::Client::new()
        .get(&url)
        .header("User-Agent", USER_AGENT)
        .send()
        .await
        .map_err(|error| format!("SoundCloud track lookup failed: {error}"))?;

    if !response.status().is_success() {
        return Ok(None);
    }

    let track: SoundCloudTrackRaw = response
        .json()
        .await
        .map_err(|error| format!("Could not parse SoundCloud track: {error}"))?;

    let preview_url = track.media.as_ref().and_then(|media| {
        media
            .transcodings
            .iter()
            .find(|t| {
                t.format
                    .as_ref()
                    .map_or(false, |f| f.protocol == "progressive")
            })
            .map(|t| format!("{}?client_id={}", t.url, client_id))
    });

    Ok(preview_url)
}

fn urlencoding(input: &str) -> String {
    let mut result = String::with_capacity(input.len() * 3);
    for byte in input.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                result.push(byte as char);
            }
            b' ' => result.push_str("%20"),
            _ => {
                result.push_str(&format!("%{:02X}", byte));
            }
        }
    }
    result
}

pub async fn search_soundcloud_as_remote(
    query: String,
    limit: Option<u32>,
) -> Result<Vec<crate::providers::remote::RemoteTrack>, String> {
    let sc_tracks = search_soundcloud_tracks(query, limit).await?;
    let results = sc_tracks
        .into_iter()
        .map(|track| crate::providers::remote::RemoteTrack {
            source: "soundcloud".to_string(),
            id: track.id.to_string(),
            uri: track.external_url.clone(),
            title: track.title,
            artist: track.artist,
            album: track.album,
            duration_ms: Some(track.duration_ms),
            external_url: Some(track.external_url),
            quality: track.preview_url.as_ref().map(|_| "30s preview".to_string()),
            playable: track.preview_url.is_some(),
        })
        .collect();
    Ok(results)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_urlencoding() {
        assert_eq!(urlencoding("hello world"), "hello%20world");
        assert_eq!(urlencoding("abc123"), "abc123");
        assert_eq!(
            urlencoding("test & query"),
            "test%20%26%20query"
        );
    }

    #[test]
    fn test_soundcloud_track_open_url() {
        let track = SoundCloudTrack {
            id: 12345,
            title: "Test Track".to_string(),
            artist: "Test Artist".to_string(),
            artist_url: None,
            album: None,
            duration_ms: 180000,
            external_url: "https://soundcloud.com/test-artist/test-track"
                .to_string(),
            permalink_url: Some(
                "https://soundcloud.com/test-artist/test-track".to_string(),
            ),
            preview_url: Some("https://api.soundcloud.com/tracks/12345/stream".to_string()),
            streamable: true,
            artwork_url: None,
            waveform_url: None,
            genre: None,
            play_count: None,
            likes_count: None,
        };
        assert_eq!(
            track.open_url(),
            "https://soundcloud.com/test-artist/test-track"
        );
    }

    #[test]
    fn test_artwork_url_resize() {
        let track = SoundCloudTrack {
            id: 12345,
            title: "Test".to_string(),
            artist: "Artist".to_string(),
            artist_url: None,
            album: None,
            duration_ms: 1000,
            external_url: "https://soundcloud.com/a/t".to_string(),
            permalink_url: None,
            preview_url: None,
            streamable: false,
            artwork_url: Some(
                "https://i1.sndcdn.com/artworks-0001-large.jpg".to_string(),
            ),
            waveform_url: None,
            genre: None,
            play_count: None,
            likes_count: None,
        };
        assert_eq!(
            track.artwork_url_500(),
            Some("https://i1.sndcdn.com/artworks-0001-t500x500.jpg".to_string())
        );
    }
}
