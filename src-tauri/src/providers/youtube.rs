use serde::{Deserialize, Serialize};

use crate::providers::remote::RemoteTrack;
use crate::web::auth;

const YOUTUBE_MUSIC_SEARCH_URL: &str =
    "https://www.googleapis.com/youtube/v3/search";
const YOUTUBE_MUSIC_BASE_URL: &str = "https://music.youtube.com";
const YOUTUBE_BASE_URL: &str = "https://www.youtube.com";
const USER_AGENT: &str = "Cold-Brew/0.1.0";

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct YoutubeTrack {
    pub video_id: String,
    pub title: String,
    pub artist: String,
    pub album: Option<String>,
    pub duration_ms: Option<u64>,
    pub external_url: Option<String>,
    pub thumbnail_url: Option<String>,
    pub is_music_content: bool,
}

#[derive(Debug, Default, Deserialize)]
struct YoutubeVideoSnippet {
    #[serde(default)]
    title: String,
    #[serde(default)]
    #[serde(rename = "channelTitle")]
    channel_title: String,
    #[serde(default)]
    thumbnails: YoutubeThumbnails,
}

#[derive(Debug, Default, Deserialize)]
struct YoutubeThumbnails {
    #[serde(default)]
    default: Option<YoutubeThumbnail>,
    #[serde(default)]
    medium: Option<YoutubeThumbnail>,
    #[serde(default)]
    high: Option<YoutubeThumbnail>,
}

#[derive(Debug, Deserialize)]
struct YoutubeThumbnail {
    #[serde(default)]
    url: String,
    #[serde(default)]
    width: u32,
    #[serde(default)]
    height: u32,
}

#[derive(Debug, Default, Deserialize)]
struct YoutubeContentDetails {
    #[serde(default)]
    duration: String,
}

#[derive(Debug, Deserialize)]
struct YoutubeVideoItem {
    #[serde(default)]
    id: String,
    #[serde(default)]
    snippet: YoutubeVideoSnippet,
    #[serde(default)]
    #[serde(rename = "contentDetails")]
    content_details: YoutubeContentDetails,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct YoutubeVideoListResponse {
    #[serde(default)]
    items: Vec<YoutubeVideoItem>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct YoutubeSearchItem {
    id: YoutubeSearchId,
    snippet: YoutubeSearchSnippet,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct YoutubeSearchId {
    #[serde(default)]
    #[serde(rename = "videoId")]
    video_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct YoutubeSearchSnippet {
    #[serde(default)]
    title: String,
    #[serde(default)]
    #[serde(rename = "channelTitle")]
    channel_title: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct YoutubeSearchResponse {
    #[serde(default)]
    items: Vec<YoutubeSearchItem>,
}

pub enum YoutubeCredentials {
    ApiKey(String),
    AccessToken(String),
}

impl YoutubeTrack {
    pub fn music_url(&self) -> String {
        format!("{}/watch?v={}", YOUTUBE_MUSIC_BASE_URL, self.video_id)
    }

    pub fn youtube_url(&self) -> String {
        format!("{}/watch?v={}", YOUTUBE_BASE_URL, self.video_id)
    }

    pub fn thumbnail_url(&self) -> Option<String> {
        self.thumbnail_url.clone().or_else(|| {
            Some(format!(
                "https://i.ytimg.com/vi/{}/hqdefault.jpg",
                self.video_id
            ))
        })
    }
}

pub fn youtube_credentials() -> Result<YoutubeCredentials, String> {
    let Some(secrets) = auth::load_provider_secrets("youtube")? else {
        return Err(
            "Save a YouTube Data API key or complete YouTube OAuth in Service Credentials first."
                .to_string(),
        );
    };
    if let Some(api_key) = secrets.api_key.map(|v| v.trim().to_string()).filter(|v| !v.is_empty()) {
        return Ok(YoutubeCredentials::ApiKey(api_key));
    }
    if let Some(access_token) = secrets.access_token.map(|v| v.trim().to_string()).filter(|v| !v.is_empty()) {
        return Ok(YoutubeCredentials::AccessToken(access_token));
    }
    Err(
        "Save a YouTube Data API key or complete YouTube OAuth in Service Credentials first."
            .to_string(),
    )
}

fn apply_youtube_credentials(
    request: reqwest::RequestBuilder,
    credentials: YoutubeCredentials,
) -> reqwest::RequestBuilder {
    match credentials {
        YoutubeCredentials::ApiKey(api_key) => request.query(&[("key", api_key)]),
        YoutubeCredentials::AccessToken(access_token) => request.bearer_auth(access_token),
    }
}

pub async fn search_youtube_music_tracks(
    query: String,
    limit: Option<u32>,
) -> Result<Vec<YoutubeTrack>, String> {
    let query = query.trim();
    if query.is_empty() {
        return Ok(Vec::new());
    }

    if auth::load_provider_secrets("youtube")?.is_none() {
        tracing::info!(
            "YouTube Music credentials not configured; returning empty results for query '{query}'"
        );
        return Ok(Vec::new());
    }

    let credentials = match youtube_credentials() {
        Ok(creds) => creds,
        Err(e) => {
            tracing::info!("YouTube credentials unavailable for YouTube Music search: {e}");
            return Ok(Vec::new());
        }
    };

    let max_results = limit.unwrap_or(10).clamp(1, 10);

    let music_query = format!("{query} music");
    let response = apply_youtube_credentials(
        reqwest::Client::new()
            .get(YOUTUBE_MUSIC_SEARCH_URL)
            .header("User-Agent", USER_AGENT)
            .query(&[
                ("part", "snippet"),
                ("type", "video"),
                ("q", &music_query),
                ("maxResults", &max_results.to_string()),
                ("videoCategoryId", "10"),
            ]),
        credentials,
    )
    .send()
    .await
    .map_err(|error| format!("YouTube Music search failed: {error}"))?;

    let status = response.status();
    if !status.is_success() {
        if status == reqwest::StatusCode::UNAUTHORIZED || status == reqwest::StatusCode::FORBIDDEN {
            auth::record_provider_auth_failure(
                "youtube",
                format!("YouTube Music search failed with HTTP {status}"),
            );
        }
        return Err(format!("YouTube Music search failed with HTTP {status}"));
    }
    auth::clear_provider_auth_failure("youtube");

    let search_response = response
        .json::<YoutubeSearchResponse>()
        .await
        .map_err(|error| format!("Could not parse YouTube Music search response: {error}"))?;

    let video_ids: Vec<String> = search_response
        .items
        .iter()
        .filter_map(|item| {
            let id = item.id.video_id.trim().to_string();
            if id.is_empty() {
                None
            } else {
                Some(id)
            }
        })
        .collect();

    let details = if video_ids.is_empty() {
        Vec::new()
    } else {
        fetch_youtube_video_details(video_ids.clone()).await
    };

    let results: Vec<YoutubeTrack> = search_response
        .items
        .into_iter()
        .filter_map(|item| {
            let video_id = item.id.video_id.trim().to_string();
            if video_id.is_empty() {
                return None;
            }

            let detail_item = details.iter().find(|d| d.id == video_id);

            let duration_ms = detail_item.and_then(|d| {
                parse_iso8601_duration_ms(&d.content_details.duration)
            });

            let thumbnail_url = detail_item
                .and_then(|d| {
                    d.snippet
                        .thumbnails
                        .high
                        .as_ref()
                        .map(|t| t.url.clone())
                        .or_else(|| d.snippet.thumbnails.medium.as_ref().map(|t| t.url.clone()))
                        .or_else(|| d.snippet.thumbnails.default.as_ref().map(|t| t.url.clone()))
                });

            let titles = item.snippet.title.splitn(2, " - ").collect::<Vec<_>>();
            let (artist, title) = if titles.len() == 2 {
                (titles[0].trim().to_string(), titles[1].trim().to_string())
            } else {
                (item.snippet.channel_title.trim().to_string(), item.snippet.title.trim().to_string())
            };

            Some(YoutubeTrack {
                video_id: video_id.clone(),
                title,
                artist,
                album: None,
                duration_ms,
                external_url: Some(format!("{}/watch?v={}", YOUTUBE_MUSIC_BASE_URL, video_id)),
                thumbnail_url,
                is_music_content: true,
            })
        })
        .collect();

    Ok(results)
}

async fn fetch_youtube_video_details(video_ids: Vec<String>) -> Vec<YoutubeVideoItem> {
    if video_ids.is_empty() {
        return Vec::new();
    }

    let credentials = match youtube_credentials() {
        Ok(c) => c,
        Err(_) => return Vec::new(),
    };

    let ids = video_ids.join(",");
    let response = match apply_youtube_credentials(
        reqwest::Client::new()
            .get("https://www.googleapis.com/youtube/v3/videos")
            .header("User-Agent", USER_AGENT)
            .query(&[
                ("part", "snippet,contentDetails"),
                ("id", &ids),
            ]),
        credentials,
    )
    .send()
    .await
    {
        Ok(resp) => resp,
        Err(_) => return Vec::new(),
    };

    match response.json::<YoutubeVideoListResponse>().await {
        Ok(video_list) => video_list.items,
        Err(_) => Vec::new(),
    }
}

fn parse_iso8601_duration_ms(duration: &str) -> Option<u64> {
    let body = duration.strip_prefix("PT")?;
    let mut number = String::new();
    let mut seconds = 0.0_f64;

    for character in body.chars() {
        if character.is_ascii_digit() || character == '.' {
            number.push(character);
            continue;
        }

        let value = number.parse::<f64>().ok()?;
        match character {
            'H' => seconds += value * 60.0 * 60.0,
            'M' => seconds += value * 60.0,
            'S' => seconds += value,
            _ => return None,
        }
        number.clear();
    }

    if number.is_empty() {
        Some((seconds * 1000.0).round() as u64)
    } else {
        None
    }
}

pub async fn search_youtube_music_as_remote(
    query: String,
    limit: Option<u32>,
) -> Result<Vec<RemoteTrack>, String> {
    let yt_tracks = search_youtube_music_tracks(query, limit).await?;
    let results = yt_tracks
        .into_iter()
        .map(|track| {
            let music_url = track.music_url();
            RemoteTrack {
                source: "youtube".to_string(),
                id: track.video_id,
                uri: music_url.clone(),
                title: track.title,
                artist: track.artist,
                album: track.album,
                duration_ms: track.duration_ms,
                external_url: Some(music_url),
                quality: Some("YouTube Music".to_string()),
                playable: false,
            }
        })
        .collect();
    Ok(results)
}

#[cfg(test)]
mod tests {
    use super::{parse_iso8601_duration_ms, YoutubeTrack};

    #[test]
    fn test_parse_iso8601_duration() {
        assert_eq!(parse_iso8601_duration_ms("PT3M30S"), Some(210000));
        assert_eq!(parse_iso8601_duration_ms("PT1H2M3S"), Some(3723000));
        assert_eq!(parse_iso8601_duration_ms(""), None);
        assert_eq!(parse_iso8601_duration_ms("not-a-duration"), None);
    }

    #[test]
    fn test_youtube_track_urls() {
        let track = YoutubeTrack {
            video_id: "dQw4w9WgXcQ".to_string(),
            title: "Never Gonna Give You Up".to_string(),
            artist: "Rick Astley".to_string(),
            album: None,
            duration_ms: Some(213000),
            external_url: None,
            thumbnail_url: None,
            is_music_content: true,
        };

        assert_eq!(
            track.music_url(),
            "https://music.youtube.com/watch?v=dQw4w9WgXcQ"
        );
        assert_eq!(
            track.youtube_url(),
            "https://www.youtube.com/watch?v=dQw4w9WgXcQ"
        );
        assert_eq!(
            track.thumbnail_url(),
            Some("https://i.ytimg.com/vi/dQw4w9WgXcQ/hqdefault.jpg".to_string())
        );
    }
}
