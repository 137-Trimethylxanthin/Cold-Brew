use std::collections::HashMap;

use reqwest::{StatusCode, Url};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::credentials;

const USER_AGENT: &str = "Cold-Brew/0.1.0";
const SPOTIFY_SEARCH_ENDPOINT: &str = "https://api.spotify.com/v1/search";
const SPOTIFY_PLAYLISTS_ENDPOINT: &str = "https://api.spotify.com/v1/me/playlists";
const SPOTIFY_PLAYLIST_TRACK_LIMIT: u32 = 50;
const TIDAL_API_BASE: &str = "https://openapi.tidal.com/v2";
const QOBUZ_SEARCH_ENDPOINT: &str = "https://www.qobuz.com/api.json/0.2/track/search";
const YOUTUBE_SEARCH_ENDPOINT: &str = "https://www.googleapis.com/youtube/v3/search";
const YOUTUBE_PLAYLIST_ITEMS_ENDPOINT: &str = "https://www.googleapis.com/youtube/v3/playlistItems";
const LASTFM_API_ENDPOINT: &str = "https://ws.audioscrobbler.com/2.0/";

enum YoutubeCredentials {
    ApiKey(String),
    AccessToken(String),
}

#[derive(Clone, Debug, Serialize)]
pub struct RemoteTrack {
    pub source: String,
    pub id: String,
    pub uri: String,
    pub title: String,
    pub artist: String,
    pub album: Option<String>,
    pub duration_ms: Option<u64>,
    pub external_url: Option<String>,
    pub quality: Option<String>,
    pub playable: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct RemotePlaylist {
    pub source: String,
    pub id: String,
    pub name: String,
    pub track_count: u64,
    pub external_url: Option<String>,
}

#[derive(Debug, Deserialize)]
struct SpotifySearchResponse {
    tracks: SpotifyTrackPage,
}

#[derive(Debug, Deserialize)]
struct SpotifyTrackPage {
    items: Vec<SpotifyTrack>,
}

#[derive(Debug, Deserialize)]
struct SpotifyPlaylistPage {
    items: Vec<SpotifyPlaylist>,
}

#[derive(Debug, Deserialize)]
struct SpotifyPlaylist {
    id: String,
    name: String,
    external_urls: HashMap<String, String>,
    tracks: SpotifyPlaylistTrackCount,
}

#[derive(Debug, Deserialize)]
struct SpotifyPlaylistTrackCount {
    total: u64,
}

#[derive(Debug, Deserialize)]
struct SpotifyPlaylistItemsPage {
    items: Vec<SpotifyPlaylistItem>,
}

#[derive(Debug, Deserialize)]
struct SpotifyPlaylistItem {
    track: Option<SpotifyTrack>,
}

#[derive(Debug, Deserialize)]
struct SpotifyTrack {
    id: String,
    name: String,
    uri: String,
    duration_ms: Option<u64>,
    is_playable: Option<bool>,
    external_urls: HashMap<String, String>,
    artists: Vec<SpotifyArtist>,
    album: SpotifyAlbum,
}

#[derive(Debug, Deserialize)]
struct SpotifyArtist {
    name: String,
}

#[derive(Debug, Deserialize)]
struct SpotifyAlbum {
    name: String,
}

#[derive(Debug, Deserialize)]
struct TidalSearchRelationshipResponse {
    #[serde(default)]
    data: Vec<TidalResourceIdentifier>,
    #[serde(default)]
    included: Vec<TidalResourceObject>,
}

#[derive(Debug, Deserialize)]
struct TidalResourceListResponse {
    #[serde(default)]
    data: Vec<TidalResourceObject>,
}

#[derive(Clone, Debug, Deserialize)]
struct TidalResourceIdentifier {
    id: String,
    #[serde(rename = "type")]
    resource_type: String,
}

#[derive(Clone, Debug, Deserialize)]
struct TidalResourceObject {
    id: String,
    #[serde(rename = "type")]
    resource_type: String,
    attributes: Option<TidalAttributes>,
    relationships: Option<TidalTrackRelationships>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TidalAttributes {
    title: Option<String>,
    name: Option<String>,
    duration: Option<String>,
    number_of_items: Option<u64>,
    media_tags: Option<Vec<String>>,
    external_links: Option<Vec<TidalExternalLink>>,
}

#[derive(Clone, Debug, Deserialize)]
struct TidalExternalLink {
    href: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
struct TidalTrackRelationships {
    artists: Option<TidalRelationship>,
    albums: Option<TidalRelationship>,
}

#[derive(Clone, Debug, Deserialize)]
struct TidalRelationship {
    #[serde(default)]
    data: Vec<TidalResourceIdentifier>,
}

#[derive(Debug, Deserialize)]
struct QobuzSearchResponse {
    tracks: Option<QobuzTrackPage>,
}

#[derive(Debug, Deserialize)]
struct QobuzTrackPage {
    #[serde(default)]
    items: Vec<QobuzTrack>,
}

#[derive(Debug, Deserialize)]
struct QobuzTrack {
    id: Value,
    title: Option<String>,
    version: Option<String>,
    duration: Option<Value>,
    is_lossless: Option<bool>,
    is_high_res: Option<bool>,
    is_super_high_res: Option<bool>,
    performer: Option<QobuzNamedEntity>,
    album: Option<QobuzAlbum>,
    url: Option<String>,
}

#[derive(Debug, Deserialize)]
struct QobuzAlbum {
    title: Option<String>,
    artist: Option<QobuzNamedEntity>,
}

#[derive(Debug, Deserialize)]
struct QobuzNamedEntity {
    name: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct YoutubeSearchResponse {
    #[serde(default)]
    items: Vec<YoutubeSearchItem>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct YoutubeSearchItem {
    id: YoutubeSearchId,
    snippet: YoutubeSnippet,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct YoutubeSearchId {
    video_id: Option<String>,
    playlist_id: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct YoutubeSnippet {
    title: String,
    channel_title: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct YoutubePlaylistItemsResponse {
    #[serde(default)]
    items: Vec<YoutubePlaylistItem>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct YoutubePlaylistItem {
    snippet: YoutubePlaylistItemSnippet,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct YoutubePlaylistItemSnippet {
    title: String,
    video_owner_channel_title: Option<String>,
    channel_title: String,
    resource_id: YoutubeResourceId,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct YoutubeResourceId {
    kind: String,
    video_id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct LastFmSearchResponse {
    results: LastFmResults,
}

#[derive(Debug, Deserialize)]
struct LastFmResults {
    trackmatches: LastFmTrackMatches,
}

#[derive(Debug, Deserialize)]
struct LastFmTrackMatches {
    track: Option<OneOrMany<LastFmTrack>>,
}

#[derive(Debug, Deserialize)]
struct LastFmTrack {
    name: String,
    artist: String,
    url: Option<String>,
    mbid: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum OneOrMany<T> {
    One(T),
    Many(Vec<T>),
}

impl<T> OneOrMany<T> {
    fn into_vec(self) -> Vec<T> {
        match self {
            Self::One(value) => vec![value],
            Self::Many(values) => values,
        }
    }
}

pub async fn search_spotify_tracks(
    query: String,
    limit: Option<u32>,
) -> Result<Vec<RemoteTrack>, String> {
    let query = query.trim();
    if query.is_empty() {
        return Ok(Vec::new());
    }

    let access_token = spotify_access_token()?;

    let limit = limit.unwrap_or(10).clamp(1, 10).to_string();
    let response = reqwest::Client::new()
        .get(SPOTIFY_SEARCH_ENDPOINT)
        .bearer_auth(access_token)
        .query(&[
            ("q", query.to_string()),
            ("type", "track".to_string()),
            ("limit", limit),
        ])
        .send()
        .await
        .map_err(|error| format!("Could not search Spotify: {error}"))?;

    ensure_provider_success(response, "Spotify search", "spotify")
        .await?
        .json::<SpotifySearchResponse>()
        .await
        .map(|search_response| {
            search_response
                .tracks
                .items
                .into_iter()
                .map(to_remote_track)
                .collect()
        })
        .map_err(|error| format!("Could not parse Spotify search response: {error}"))
}

pub async fn list_spotify_playlists(limit: Option<u32>) -> Result<Vec<RemotePlaylist>, String> {
    let access_token = spotify_access_token()?;
    let limit = limit.unwrap_or(20).clamp(1, 50).to_string();
    let response = reqwest::Client::new()
        .get(SPOTIFY_PLAYLISTS_ENDPOINT)
        .bearer_auth(access_token)
        .query(&[("limit", limit)])
        .send()
        .await
        .map_err(|error| format!("Could not load Spotify playlists: {error}"))?;

    ensure_provider_success(response, "Spotify playlists", "spotify")
        .await?
        .json::<SpotifyPlaylistPage>()
        .await
        .map(|page| page.items.into_iter().map(to_remote_playlist).collect())
        .map_err(|error| format!("Could not parse Spotify playlists response: {error}"))
}

pub async fn get_spotify_playlist_tracks(
    playlist_id: String,
    limit: Option<u32>,
) -> Result<Vec<RemoteTrack>, String> {
    let playlist_id = normalize_spotify_id(&playlist_id)?;
    let access_token = spotify_access_token()?;
    let limit = limit
        .unwrap_or(SPOTIFY_PLAYLIST_TRACK_LIMIT)
        .clamp(1, SPOTIFY_PLAYLIST_TRACK_LIMIT)
        .to_string();
    let response = reqwest::Client::new()
        .get(format!(
            "https://api.spotify.com/v1/playlists/{playlist_id}/tracks"
        ))
        .bearer_auth(access_token)
        .query(&[("limit", limit)])
        .send()
        .await
        .map_err(|error| format!("Could not load Spotify playlist tracks: {error}"))?;

    ensure_provider_success(response, "Spotify playlist tracks", "spotify")
        .await?
        .json::<SpotifyPlaylistItemsPage>()
        .await
        .map(|page| {
            page.items
                .into_iter()
                .filter_map(|item| item.track)
                .map(to_remote_track)
                .collect()
        })
        .map_err(|error| format!("Could not parse Spotify playlist tracks response: {error}"))
}

pub async fn search_tidal_tracks(
    query: String,
    country_code: Option<String>,
    limit: Option<u32>,
) -> Result<Vec<RemoteTrack>, String> {
    let query = query.trim();
    if query.is_empty() {
        return Ok(Vec::new());
    }

    let access_token = tidal_access_token()?;
    let country_code = normalize_country_code(country_code)?;
    let mut url = Url::parse(TIDAL_API_BASE)
        .map_err(|error| format!("Could not construct TIDAL search URL: {error}"))?;
    {
        let mut segments = url
            .path_segments_mut()
            .map_err(|_| "Could not construct TIDAL search URL.".to_string())?;
        segments
            .push("searchResults")
            .push(query)
            .push("relationships")
            .push("tracks");
    }
    url.query_pairs_mut()
        .append_pair("countryCode", &country_code)
        .append_pair("explicitFilter", "INCLUDE")
        .append_pair("include", "tracks");

    let response = reqwest::Client::new()
        .get(url)
        .header("Accept", "application/vnd.api+json")
        .bearer_auth(access_token)
        .send()
        .await
        .map_err(|error| format!("Could not search TIDAL: {error}"))?;

    ensure_provider_success(response, "TIDAL search", "tidal")
        .await?
        .json::<TidalSearchRelationshipResponse>()
        .await
        .map(|search_response| normalize_tidal_tracks(search_response, limit.unwrap_or(10)))
        .map_err(|error| format!("Could not parse TIDAL search response: {error}"))
}

pub async fn list_tidal_playlists(
    country_code: Option<String>,
    limit: Option<u32>,
) -> Result<Vec<RemotePlaylist>, String> {
    let access_token = tidal_access_token()?;
    let country_code = normalize_country_code(country_code)?;
    let mut url = Url::parse(TIDAL_API_BASE)
        .map_err(|error| format!("Could not construct TIDAL playlists URL: {error}"))?;
    {
        let mut segments = url
            .path_segments_mut()
            .map_err(|_| "Could not construct TIDAL playlists URL.".to_string())?;
        segments.push("playlists");
    }
    url.query_pairs_mut()
        .append_pair("countryCode", &country_code)
        .append_pair("filter[owners.id]", "me");

    let response = reqwest::Client::new()
        .get(url)
        .header("Accept", "application/vnd.api+json")
        .bearer_auth(access_token)
        .send()
        .await
        .map_err(|error| format!("Could not load TIDAL playlists: {error}"))?;

    ensure_provider_success(response, "TIDAL playlists", "tidal")
        .await?
        .json::<TidalResourceListResponse>()
        .await
        .map(|playlist_response| normalize_tidal_playlists(playlist_response, limit.unwrap_or(20)))
        .map_err(|error| format!("Could not parse TIDAL playlists response: {error}"))
}

pub async fn search_tidal_playlists(
    query: String,
    country_code: Option<String>,
    limit: Option<u32>,
) -> Result<Vec<RemotePlaylist>, String> {
    let query = query.trim();
    if query.is_empty() {
        return Ok(Vec::new());
    }

    let access_token = tidal_access_token()?;
    let country_code = normalize_country_code(country_code)?;
    let mut url = Url::parse(TIDAL_API_BASE)
        .map_err(|error| format!("Could not construct TIDAL playlist search URL: {error}"))?;
    {
        let mut segments = url
            .path_segments_mut()
            .map_err(|_| "Could not construct TIDAL playlist search URL.".to_string())?;
        segments
            .push("searchResults")
            .push(query)
            .push("relationships")
            .push("playlists");
    }
    url.query_pairs_mut()
        .append_pair("countryCode", &country_code)
        .append_pair("include", "playlists");

    let response = reqwest::Client::new()
        .get(url)
        .header("Accept", "application/vnd.api+json")
        .bearer_auth(access_token)
        .send()
        .await
        .map_err(|error| format!("Could not search TIDAL playlists: {error}"))?;

    ensure_provider_success(response, "TIDAL playlist search", "tidal")
        .await?
        .json::<TidalSearchRelationshipResponse>()
        .await
        .map(|search_response| {
            normalize_tidal_playlist_search(search_response, limit.unwrap_or(10))
        })
        .map_err(|error| format!("Could not parse TIDAL playlist search response: {error}"))
}

pub async fn get_tidal_playlist_tracks(
    playlist_id: String,
    country_code: Option<String>,
    limit: Option<u32>,
) -> Result<Vec<RemoteTrack>, String> {
    let playlist_id = normalize_remote_id(&playlist_id, "TIDAL playlist")?;
    let access_token = tidal_access_token()?;
    let country_code = normalize_country_code(country_code)?;
    let mut url = Url::parse(TIDAL_API_BASE)
        .map_err(|error| format!("Could not construct TIDAL playlist tracks URL: {error}"))?;
    {
        let mut segments = url
            .path_segments_mut()
            .map_err(|_| "Could not construct TIDAL playlist tracks URL.".to_string())?;
        segments
            .push("playlists")
            .push(&playlist_id)
            .push("relationships")
            .push("items");
    }
    url.query_pairs_mut()
        .append_pair("countryCode", &country_code)
        .append_pair("include", "items");

    let response = reqwest::Client::new()
        .get(url)
        .header("Accept", "application/vnd.api+json")
        .bearer_auth(access_token)
        .send()
        .await
        .map_err(|error| format!("Could not load TIDAL playlist tracks: {error}"))?;

    ensure_provider_success(response, "TIDAL playlist tracks", "tidal")
        .await?
        .json::<TidalSearchRelationshipResponse>()
        .await
        .map(|playlist_response| normalize_tidal_tracks(playlist_response, limit.unwrap_or(50)))
        .map_err(|error| format!("Could not parse TIDAL playlist tracks response: {error}"))
}

pub async fn search_qobuz_tracks(
    query: String,
    limit: Option<u32>,
) -> Result<Vec<RemoteTrack>, String> {
    let query = query.trim();
    if query.is_empty() {
        return Ok(Vec::new());
    }

    let app_id = required_provider_value("qobuz", |secrets| secrets.client_id.or(secrets.api_key))?
        .ok_or_else(|| "Save a Qobuz-issued app id in Service Credentials first.".to_string())?;
    let limit = limit.unwrap_or(10).clamp(1, 10).to_string();
    let response = reqwest::Client::new()
        .get(QOBUZ_SEARCH_ENDPOINT)
        .header("User-Agent", USER_AGENT)
        .query(&[
            ("app_id", app_id),
            ("query", query.to_string()),
            ("limit", limit),
        ])
        .send()
        .await
        .map_err(|error| format!("Could not search Qobuz: {error}"))?;

    ensure_provider_success(response, "Qobuz search", "qobuz")
        .await?
        .json::<QobuzSearchResponse>()
        .await
        .map(|search_response| {
            search_response
                .tracks
                .map(|tracks| tracks.items)
                .unwrap_or_default()
                .into_iter()
                .filter_map(to_qobuz_remote_track)
                .collect()
        })
        .map_err(|error| format!("Could not parse Qobuz search response: {error}"))
}

pub async fn search_youtube_tracks(
    query: String,
    limit: Option<u32>,
) -> Result<Vec<RemoteTrack>, String> {
    let query = query.trim();
    if query.is_empty() {
        return Ok(Vec::new());
    }

    let credentials = youtube_credentials()?;
    let max_results = limit.unwrap_or(10).clamp(1, 10).to_string();
    let request = reqwest::Client::new()
        .get(YOUTUBE_SEARCH_ENDPOINT)
        .header("User-Agent", USER_AGENT)
        .query(&[
            ("part", "snippet".to_string()),
            ("type", "video".to_string()),
            ("q", query.to_string()),
            ("maxResults", max_results),
        ]);
    let response = apply_youtube_credentials(request, credentials)
        .send()
        .await
        .map_err(|error| format!("Could not search YouTube: {error}"))?;

    ensure_provider_success(response, "YouTube search", "youtube")
        .await?
        .json::<YoutubeSearchResponse>()
        .await
        .map(|search_response| {
            search_response
                .items
                .into_iter()
                .filter_map(to_youtube_remote_track)
                .collect()
        })
        .map_err(|error| format!("Could not parse YouTube search response: {error}"))
}

pub async fn search_youtube_playlists(
    query: String,
    limit: Option<u32>,
) -> Result<Vec<RemotePlaylist>, String> {
    let query = query.trim();
    if query.is_empty() {
        return Ok(Vec::new());
    }

    let credentials = youtube_credentials()?;
    let max_results = limit.unwrap_or(10).clamp(1, 10).to_string();
    let request = reqwest::Client::new()
        .get(YOUTUBE_SEARCH_ENDPOINT)
        .header("User-Agent", USER_AGENT)
        .query(&[
            ("part", "snippet".to_string()),
            ("type", "playlist".to_string()),
            ("q", query.to_string()),
            ("maxResults", max_results),
        ]);
    let response = apply_youtube_credentials(request, credentials)
        .send()
        .await
        .map_err(|error| format!("Could not search YouTube playlists: {error}"))?;

    ensure_provider_success(response, "YouTube playlist search", "youtube")
        .await?
        .json::<YoutubeSearchResponse>()
        .await
        .map(|search_response| {
            search_response
                .items
                .into_iter()
                .filter_map(to_youtube_remote_playlist)
                .collect()
        })
        .map_err(|error| format!("Could not parse YouTube playlist search response: {error}"))
}

pub async fn get_youtube_playlist_tracks(
    playlist_id: String,
    limit: Option<u32>,
) -> Result<Vec<RemoteTrack>, String> {
    let playlist_id = normalize_remote_id(&playlist_id, "YouTube playlist")?;
    let credentials = youtube_credentials()?;
    let max_results = limit.unwrap_or(50).clamp(1, 50).to_string();
    let request = reqwest::Client::new()
        .get(YOUTUBE_PLAYLIST_ITEMS_ENDPOINT)
        .header("User-Agent", USER_AGENT)
        .query(&[
            ("part", "snippet".to_string()),
            ("playlistId", playlist_id),
            ("maxResults", max_results),
        ]);
    let response = apply_youtube_credentials(request, credentials)
        .send()
        .await
        .map_err(|error| format!("Could not load YouTube playlist items: {error}"))?;

    ensure_provider_success(response, "YouTube playlist items", "youtube")
        .await?
        .json::<YoutubePlaylistItemsResponse>()
        .await
        .map(|playlist_response| {
            playlist_response
                .items
                .into_iter()
                .filter_map(to_youtube_playlist_track)
                .collect()
        })
        .map_err(|error| format!("Could not parse YouTube playlist items response: {error}"))
}

pub async fn search_lastfm_tracks(
    query: String,
    limit: Option<u32>,
) -> Result<Vec<RemoteTrack>, String> {
    let query = query.trim();
    if query.is_empty() {
        return Ok(Vec::new());
    }

    let api_key = required_provider_value("lastfm", |secrets| secrets.api_key)?
        .ok_or_else(|| "Save a Last.fm API key in Service Credentials first.".to_string())?;
    let limit = limit.unwrap_or(10).clamp(1, 10).to_string();
    let response = reqwest::Client::new()
        .get(LASTFM_API_ENDPOINT)
        .header("User-Agent", USER_AGENT)
        .query(&[
            ("method", "track.search".to_string()),
            ("track", query.to_string()),
            ("api_key", api_key),
            ("format", "json".to_string()),
            ("limit", limit),
        ])
        .send()
        .await
        .map_err(|error| format!("Could not search Last.fm: {error}"))?;

    let response = ensure_provider_success(response, "Last.fm search", "lastfm").await?;
    let value = response
        .json::<Value>()
        .await
        .map_err(|error| format!("Could not parse Last.fm search response: {error}"))?;

    if let Some(error_code) = value.get("error").and_then(Value::as_i64) {
        let message = value
            .get("message")
            .and_then(Value::as_str)
            .unwrap_or("Last.fm API error");
        let error = format!("Last.fm search failed with error {error_code}: {message}");
        credentials::record_provider_auth_failure("lastfm", error.clone());
        return Err(error);
    }

    credentials::clear_provider_auth_failure("lastfm");

    serde_json::from_value::<LastFmSearchResponse>(value)
        .map(|search_response| {
            search_response
                .results
                .trackmatches
                .track
                .map(OneOrMany::into_vec)
                .unwrap_or_default()
                .into_iter()
                .map(to_lastfm_remote_track)
                .collect()
        })
        .map_err(|error| format!("Could not parse Last.fm search response: {error}"))
}

async fn ensure_provider_success(
    response: reqwest::Response,
    label: &str,
    provider_id: &str,
) -> Result<reqwest::Response, String> {
    let status = response.status();
    if status == StatusCode::UNAUTHORIZED {
        let message = format!("{label} credentials are invalid or expired.");
        credentials::record_provider_auth_failure(provider_id, message.clone());
        return Err(message);
    }
    if status == StatusCode::FORBIDDEN {
        let message = format!("{label} is not allowed for the saved credentials.");
        credentials::record_provider_auth_failure(provider_id, message.clone());
        return Err(message);
    }
    if status == StatusCode::TOO_MANY_REQUESTS {
        return Err(format!("{label} rate limit reached. Try again later."));
    }
    if !status.is_success() {
        return Err(format!("{label} failed with HTTP {status}."));
    }
    credentials::clear_provider_auth_failure(provider_id);
    Ok(response)
}

fn to_remote_track(track: SpotifyTrack) -> RemoteTrack {
    RemoteTrack {
        source: "spotify".to_string(),
        id: track.id,
        uri: track.uri,
        title: track.name,
        artist: track
            .artists
            .into_iter()
            .map(|artist| artist.name)
            .collect::<Vec<_>>()
            .join(", "),
        album: Some(track.album.name).filter(|album| !album.is_empty()),
        duration_ms: track.duration_ms,
        external_url: track.external_urls.get("spotify").cloned(),
        quality: Some("Spotify Connect".to_string()),
        playable: track.is_playable.unwrap_or(true),
    }
}

fn to_remote_playlist(playlist: SpotifyPlaylist) -> RemotePlaylist {
    RemotePlaylist {
        source: "spotify".to_string(),
        id: playlist.id,
        name: playlist.name,
        track_count: playlist.tracks.total,
        external_url: playlist.external_urls.get("spotify").cloned(),
    }
}

fn normalize_tidal_tracks(
    search_response: TidalSearchRelationshipResponse,
    limit: u32,
) -> Vec<RemoteTrack> {
    let mut included = HashMap::new();
    for resource in search_response.included {
        included.insert(
            (resource.resource_type.clone(), resource.id.clone()),
            resource,
        );
    }

    search_response
        .data
        .into_iter()
        .filter(|identifier| identifier.resource_type == "tracks")
        .take(limit.clamp(1, 10) as usize)
        .filter_map(|identifier| {
            let track = included
                .get(&("tracks".to_string(), identifier.id.clone()))
                .cloned()
                .unwrap_or_else(|| TidalResourceObject {
                    id: identifier.id,
                    resource_type: "tracks".to_string(),
                    attributes: None,
                    relationships: None,
                });
            to_tidal_remote_track(track, &included)
        })
        .collect()
}

fn normalize_tidal_playlists(
    playlist_response: TidalResourceListResponse,
    limit: u32,
) -> Vec<RemotePlaylist> {
    playlist_response
        .data
        .into_iter()
        .filter(|resource| resource.resource_type == "playlists")
        .take(limit.clamp(1, 50) as usize)
        .filter_map(to_tidal_remote_playlist)
        .collect()
}

fn normalize_tidal_playlist_search(
    search_response: TidalSearchRelationshipResponse,
    limit: u32,
) -> Vec<RemotePlaylist> {
    let mut included = HashMap::new();
    for resource in search_response.included {
        included.insert(
            (resource.resource_type.clone(), resource.id.clone()),
            resource,
        );
    }

    search_response
        .data
        .into_iter()
        .filter(|identifier| identifier.resource_type == "playlists")
        .take(limit.clamp(1, 10) as usize)
        .filter_map(|identifier| {
            included
                .get(&("playlists".to_string(), identifier.id.clone()))
                .cloned()
                .or_else(|| {
                    Some(TidalResourceObject {
                        id: identifier.id,
                        resource_type: "playlists".to_string(),
                        attributes: None,
                        relationships: None,
                    })
                })
                .and_then(to_tidal_remote_playlist)
        })
        .collect()
}

fn to_tidal_remote_track(
    track: TidalResourceObject,
    included: &HashMap<(String, String), TidalResourceObject>,
) -> Option<RemoteTrack> {
    let title = track
        .attributes
        .as_ref()
        .and_then(|attributes| trimmed_option(attributes.title.clone()))
        .unwrap_or_else(|| format!("TIDAL track {}", track.id));
    let artist =
        relationship_names(&track, "artists", included).unwrap_or_else(|| "TIDAL".to_string());
    let album = relationship_names(&track, "albums", included);
    let duration_ms = track
        .attributes
        .as_ref()
        .and_then(|attributes| attributes.duration.as_deref())
        .and_then(parse_iso8601_duration_ms);
    let external_url = track
        .attributes
        .as_ref()
        .and_then(|attributes| attributes.external_links.as_ref())
        .and_then(|links| links.iter().filter_map(|link| link.href.clone()).next())
        .or_else(|| Some(format!("https://tidal.com/browse/track/{}", track.id)));
    let quality = track
        .attributes
        .as_ref()
        .and_then(|attributes| quality_from_media_tags(attributes.media_tags.as_deref()));

    Some(RemoteTrack {
        source: "tidal".to_string(),
        id: track.id.clone(),
        uri: format!("tidal:track:{}", track.id),
        title,
        artist,
        album,
        duration_ms,
        external_url,
        quality,
        playable: false,
    })
}

fn to_tidal_remote_playlist(playlist: TidalResourceObject) -> Option<RemotePlaylist> {
    let name = playlist
        .attributes
        .as_ref()
        .and_then(|attributes| {
            trimmed_option(attributes.name.clone())
                .or_else(|| trimmed_option(attributes.title.clone()))
        })
        .unwrap_or_else(|| format!("TIDAL playlist {}", playlist.id));
    let track_count = playlist
        .attributes
        .as_ref()
        .and_then(|attributes| attributes.number_of_items)
        .unwrap_or(0);
    let external_url = playlist
        .attributes
        .as_ref()
        .and_then(|attributes| attributes.external_links.as_ref())
        .and_then(|links| links.iter().filter_map(|link| link.href.clone()).next())
        .or_else(|| Some(format!("https://tidal.com/browse/playlist/{}", playlist.id)));

    Some(RemotePlaylist {
        source: "tidal".to_string(),
        id: playlist.id,
        name,
        track_count,
        external_url,
    })
}

fn relationship_names(
    track: &TidalResourceObject,
    relationship_name: &str,
    included: &HashMap<(String, String), TidalResourceObject>,
) -> Option<String> {
    let relationship = match relationship_name {
        "artists" => track
            .relationships
            .as_ref()
            .and_then(|relationships| relationships.artists.as_ref()),
        "albums" => track
            .relationships
            .as_ref()
            .and_then(|relationships| relationships.albums.as_ref()),
        _ => None,
    }?;
    let names = relationship
        .data
        .iter()
        .filter_map(|identifier| {
            included
                .get(&(identifier.resource_type.clone(), identifier.id.clone()))
                .and_then(|resource| resource.attributes.as_ref())
                .and_then(|attributes| {
                    trimmed_option(attributes.name.clone())
                        .or_else(|| trimmed_option(attributes.title.clone()))
                })
        })
        .collect::<Vec<_>>();

    if names.is_empty() {
        None
    } else {
        Some(names.join(", "))
    }
}

fn to_qobuz_remote_track(track: QobuzTrack) -> Option<RemoteTrack> {
    let id = value_to_string(&track.id)?;
    let mut title = trimmed_option(track.title)?;
    if let Some(version) = trimmed_option(track.version) {
        title = format!("{title} ({version})");
    }
    let album = track.album.as_ref().and_then(|album| album.title.clone());
    let artist = track
        .performer
        .and_then(|performer| performer.name)
        .or_else(|| {
            track
                .album
                .and_then(|album| album.artist)
                .and_then(|artist| artist.name)
        })
        .unwrap_or_else(|| "Qobuz".to_string());
    let duration_ms = track
        .duration
        .and_then(value_to_u64)
        .map(|seconds| seconds * 1000);
    let external_url =
        trimmed_option(track.url).or_else(|| Some(format!("https://www.qobuz.com/track/{id}")));
    let quality = qobuz_quality_label(
        track.is_lossless.unwrap_or(false),
        track.is_high_res.unwrap_or(false),
        track.is_super_high_res.unwrap_or(false),
    );

    Some(RemoteTrack {
        source: "qobuz".to_string(),
        id: id.clone(),
        uri: format!("qobuz:track:{id}"),
        title,
        artist,
        album,
        duration_ms,
        external_url,
        quality,
        playable: false,
    })
}

fn to_youtube_remote_track(item: YoutubeSearchItem) -> Option<RemoteTrack> {
    let video_id = item.id.video_id?;
    let external_url = format!("https://www.youtube.com/watch?v={video_id}");

    Some(RemoteTrack {
        source: "youtube".to_string(),
        id: video_id.clone(),
        uri: external_url.clone(),
        title: item.snippet.title,
        artist: item.snippet.channel_title,
        album: None,
        duration_ms: None,
        external_url: Some(external_url),
        quality: None,
        playable: false,
    })
}

fn to_youtube_remote_playlist(item: YoutubeSearchItem) -> Option<RemotePlaylist> {
    let playlist_id = item.id.playlist_id?;
    Some(RemotePlaylist {
        source: "youtube".to_string(),
        id: playlist_id.clone(),
        name: item.snippet.title,
        track_count: 0,
        external_url: Some(format!(
            "https://www.youtube.com/playlist?list={playlist_id}"
        )),
    })
}

fn to_youtube_playlist_track(item: YoutubePlaylistItem) -> Option<RemoteTrack> {
    if item.snippet.resource_id.kind != "youtube#video" {
        return None;
    }
    let video_id = item.snippet.resource_id.video_id?;
    let external_url = format!("https://www.youtube.com/watch?v={video_id}");
    let artist = item
        .snippet
        .video_owner_channel_title
        .unwrap_or(item.snippet.channel_title);

    Some(RemoteTrack {
        source: "youtube".to_string(),
        id: video_id.clone(),
        uri: external_url.clone(),
        title: item.snippet.title,
        artist,
        album: None,
        duration_ms: None,
        external_url: Some(external_url),
        quality: Some("playlist video".to_string()),
        playable: false,
    })
}

fn to_lastfm_remote_track(track: LastFmTrack) -> RemoteTrack {
    let id = trimmed_option(track.mbid)
        .or_else(|| track.url.clone())
        .unwrap_or_else(|| fallback_id(&[&track.artist, &track.name]));
    let external_url = trimmed_option(track.url);

    RemoteTrack {
        source: "lastfm".to_string(),
        id,
        uri: format!(
            "lastfm:track:{}",
            fallback_id(&[&track.artist, &track.name])
        ),
        title: track.name,
        artist: track.artist,
        album: None,
        duration_ms: None,
        external_url,
        quality: Some("metadata only".to_string()),
        playable: false,
    }
}

fn quality_from_media_tags(media_tags: Option<&[String]>) -> Option<String> {
    let tags = media_tags?;
    if tags.is_empty() {
        None
    } else {
        Some(tags.join(", "))
    }
}

fn qobuz_quality_label(
    is_lossless: bool,
    is_high_res: bool,
    is_super_high_res: bool,
) -> Option<String> {
    if is_super_high_res {
        Some("hi-res lossless".to_string())
    } else if is_high_res {
        Some("hi-res".to_string())
    } else if is_lossless {
        Some("lossless".to_string())
    } else {
        None
    }
}

fn spotify_access_token() -> Result<String, String> {
    credentials::load_provider_secrets("spotify")?
        .and_then(|secrets| secrets.access_token)
        .map(|token| token.trim().to_string())
        .filter(|token| !token.is_empty())
        .ok_or_else(|| "Save a Spotify access token in Service Credentials first.".to_string())
}

fn tidal_access_token() -> Result<String, String> {
    required_provider_value("tidal", |secrets| secrets.access_token)?
        .ok_or_else(|| "Save a TIDAL access token in Service Credentials first.".to_string())
}

fn youtube_credentials() -> Result<YoutubeCredentials, String> {
    let Some(secrets) = credentials::load_provider_secrets("youtube")? else {
        return Err(
            "Save a YouTube Data API key or complete YouTube OAuth in Service Credentials first."
                .to_string(),
        );
    };
    if let Some(api_key) = non_empty_owned(secrets.api_key) {
        return Ok(YoutubeCredentials::ApiKey(api_key));
    }
    if let Some(access_token) = non_empty_owned(secrets.access_token) {
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

fn required_provider_value<F>(provider_id: &str, select: F) -> Result<Option<String>, String>
where
    F: FnOnce(credentials::ProviderSecrets) -> Option<String>,
{
    Ok(credentials::load_provider_secrets(provider_id)?
        .and_then(select)
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty()))
}

fn non_empty_owned(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn normalize_spotify_id(id: &str) -> Result<String, String> {
    normalize_remote_id(id, "Spotify")
}

fn normalize_remote_id(id: &str, label: &str) -> Result<String, String> {
    let id = id.trim();
    if id.is_empty() || id.contains('/') {
        Err(format!("{label} id is invalid."))
    } else {
        Ok(id.to_string())
    }
}

fn normalize_country_code(country_code: Option<String>) -> Result<String, String> {
    let country_code = country_code
        .unwrap_or_else(|| "US".to_string())
        .trim()
        .to_ascii_uppercase();
    if country_code.len() == 2
        && country_code
            .chars()
            .all(|character| character.is_ascii_uppercase())
    {
        Ok(country_code)
    } else {
        Err("Country code must be a two-letter ISO 3166-1 alpha-2 value.".to_string())
    }
}

fn parse_iso8601_duration_ms(duration: &str) -> Option<u64> {
    let body = duration.strip_prefix("PT")?;
    let mut number = String::new();
    let mut seconds = 0.0;

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

fn value_to_string(value: &Value) -> Option<String> {
    match value {
        Value::String(value) => trimmed_option(Some(value.clone())),
        Value::Number(value) => Some(value.to_string()),
        _ => None,
    }
}

fn value_to_u64(value: Value) -> Option<u64> {
    match value {
        Value::Number(number) => number.as_u64(),
        Value::String(value) => value.trim().parse::<u64>().ok(),
        _ => None,
    }
}

fn trimmed_option(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn fallback_id(parts: &[&str]) -> String {
    parts
        .iter()
        .map(|part| part.trim())
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join(":")
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use serde_json::json;

    use super::{
        QobuzAlbum, QobuzNamedEntity, QobuzTrack, SpotifyAlbum, SpotifyArtist, SpotifyPlaylist,
        SpotifyPlaylistTrackCount, SpotifyTrack, TidalAttributes, TidalExternalLink,
        TidalResourceIdentifier, TidalResourceListResponse, TidalResourceObject,
        TidalSearchRelationshipResponse, YoutubePlaylistItem, YoutubePlaylistItemSnippet,
        YoutubeResourceId, YoutubeSearchId, YoutubeSearchItem, YoutubeSnippet,
        normalize_country_code, normalize_spotify_id, normalize_tidal_playlists,
        normalize_tidal_tracks, parse_iso8601_duration_ms, to_qobuz_remote_track,
        to_remote_playlist, to_remote_track, to_youtube_playlist_track, to_youtube_remote_playlist,
        value_to_string,
    };

    #[test]
    fn spotify_tracks_are_normalized_for_queueing() {
        let remote = to_remote_track(SpotifyTrack {
            id: "123".to_string(),
            name: "Track".to_string(),
            uri: "spotify:track:123".to_string(),
            duration_ms: Some(180_000),
            is_playable: Some(false),
            external_urls: HashMap::from([(
                "spotify".to_string(),
                "https://open.spotify.com/track/123".to_string(),
            )]),
            artists: vec![
                SpotifyArtist {
                    name: "One".to_string(),
                },
                SpotifyArtist {
                    name: "Two".to_string(),
                },
            ],
            album: SpotifyAlbum {
                name: "Album".to_string(),
            },
        });

        assert_eq!(remote.source, "spotify");
        assert_eq!(remote.artist, "One, Two");
        assert_eq!(remote.album.as_deref(), Some("Album"));
        assert_eq!(remote.quality.as_deref(), Some("Spotify Connect"));
        assert_eq!(remote.playable, false);
    }

    #[test]
    fn spotify_playlists_are_normalized() {
        let playlist = to_remote_playlist(SpotifyPlaylist {
            id: "playlist".to_string(),
            name: "Favorites".to_string(),
            external_urls: HashMap::from([(
                "spotify".to_string(),
                "https://open.spotify.com/playlist/playlist".to_string(),
            )]),
            tracks: SpotifyPlaylistTrackCount { total: 42 },
        });

        assert_eq!(playlist.source, "spotify");
        assert_eq!(playlist.track_count, 42);
        assert_eq!(
            playlist.external_url.as_deref(),
            Some("https://open.spotify.com/playlist/playlist")
        );
    }

    #[test]
    fn tidal_playlists_are_normalized() {
        let playlists = normalize_tidal_playlists(
            TidalResourceListResponse {
                data: vec![TidalResourceObject {
                    id: "playlist-id".to_string(),
                    resource_type: "playlists".to_string(),
                    attributes: Some(TidalAttributes {
                        title: None,
                        name: Some("Favorites".to_string()),
                        duration: None,
                        number_of_items: Some(12),
                        media_tags: None,
                        external_links: Some(vec![TidalExternalLink {
                            href: Some("https://tidal.com/browse/playlist/playlist-id".to_string()),
                        }]),
                    }),
                    relationships: None,
                }],
            },
            20,
        );

        assert_eq!(playlists.len(), 1);
        assert_eq!(playlists[0].source, "tidal");
        assert_eq!(playlists[0].name, "Favorites");
        assert_eq!(playlists[0].track_count, 12);
        assert_eq!(
            playlists[0].external_url.as_deref(),
            Some("https://tidal.com/browse/playlist/playlist-id")
        );
    }

    #[test]
    fn tidal_playlist_items_are_normalized_as_tracks() {
        let tracks = normalize_tidal_tracks(
            TidalSearchRelationshipResponse {
                data: vec![TidalResourceIdentifier {
                    id: "track-id".to_string(),
                    resource_type: "tracks".to_string(),
                }],
                included: vec![TidalResourceObject {
                    id: "track-id".to_string(),
                    resource_type: "tracks".to_string(),
                    attributes: Some(TidalAttributes {
                        title: Some("Song".to_string()),
                        name: None,
                        duration: Some("PT1M2S".to_string()),
                        number_of_items: None,
                        media_tags: Some(vec!["LOSSLESS".to_string()]),
                        external_links: None,
                    }),
                    relationships: None,
                }],
            },
            50,
        );

        assert_eq!(tracks.len(), 1);
        assert_eq!(tracks[0].source, "tidal");
        assert_eq!(tracks[0].id, "track-id");
        assert_eq!(tracks[0].title, "Song");
        assert_eq!(tracks[0].duration_ms, Some(62_000));
        assert_eq!(tracks[0].quality.as_deref(), Some("LOSSLESS"));
        assert!(!tracks[0].playable);
    }

    #[test]
    fn spotify_ids_reject_path_segments() {
        assert_eq!(normalize_spotify_id(" abc ").unwrap(), "abc");
        assert!(normalize_spotify_id("abc/def").is_err());
    }

    #[test]
    fn country_codes_are_normalized() {
        assert_eq!(
            normalize_country_code(Some(" at ".to_string())).unwrap(),
            "AT"
        );
        assert!(normalize_country_code(Some("austria".to_string())).is_err());
    }

    #[test]
    fn iso8601_durations_are_converted_to_milliseconds() {
        assert_eq!(parse_iso8601_duration_ms("PT2M58S"), Some(178_000));
        assert_eq!(parse_iso8601_duration_ms("PT1H2M3.5S"), Some(3_723_500));
        assert_eq!(parse_iso8601_duration_ms("P1D"), None);
    }

    #[test]
    fn qobuz_tracks_are_normalized_for_queueing() {
        let remote = to_qobuz_remote_track(QobuzTrack {
            id: json!(12345),
            title: Some("Track".to_string()),
            version: Some("Live".to_string()),
            duration: Some(json!("181")),
            is_lossless: Some(true),
            is_high_res: Some(false),
            is_super_high_res: Some(false),
            performer: Some(QobuzNamedEntity {
                name: Some("Artist".to_string()),
            }),
            album: Some(QobuzAlbum {
                title: Some("Album".to_string()),
                artist: None,
            }),
            url: Some("https://www.qobuz.com/track/12345".to_string()),
        })
        .unwrap();

        assert_eq!(remote.source, "qobuz");
        assert_eq!(remote.id, "12345");
        assert_eq!(remote.title, "Track (Live)");
        assert_eq!(remote.artist, "Artist");
        assert_eq!(remote.album.as_deref(), Some("Album"));
        assert_eq!(remote.duration_ms, Some(181_000));
        assert_eq!(remote.quality.as_deref(), Some("lossless"));
        assert_eq!(remote.playable, false);
    }

    #[test]
    fn youtube_playlists_are_normalized() {
        let playlist = to_youtube_remote_playlist(YoutubeSearchItem {
            id: YoutubeSearchId {
                video_id: None,
                playlist_id: Some("PL123".to_string()),
            },
            snippet: YoutubeSnippet {
                title: "Playlist".to_string(),
                channel_title: "Channel".to_string(),
            },
        })
        .unwrap();

        assert_eq!(playlist.source, "youtube");
        assert_eq!(playlist.id, "PL123");
        assert_eq!(
            playlist.external_url.as_deref(),
            Some("https://www.youtube.com/playlist?list=PL123")
        );
    }

    #[test]
    fn youtube_playlist_items_are_normalized_as_metadata_tracks() {
        let track = to_youtube_playlist_track(YoutubePlaylistItem {
            snippet: YoutubePlaylistItemSnippet {
                title: "Video".to_string(),
                video_owner_channel_title: Some("Owner".to_string()),
                channel_title: "Playlist Channel".to_string(),
                resource_id: YoutubeResourceId {
                    kind: "youtube#video".to_string(),
                    video_id: Some("abc".to_string()),
                },
            },
        })
        .unwrap();

        assert_eq!(track.source, "youtube");
        assert_eq!(track.id, "abc");
        assert_eq!(track.artist, "Owner");
        assert_eq!(track.quality.as_deref(), Some("playlist video"));
        assert!(!track.playable);
    }

    #[test]
    fn json_values_convert_to_remote_ids() {
        assert_eq!(value_to_string(&json!("abc")).as_deref(), Some("abc"));
        assert_eq!(value_to_string(&json!(123)).as_deref(), Some("123"));
        assert_eq!(value_to_string(&json!(null)), None);
    }
}
