#![allow(dead_code)]
use serde::{Deserialize, Serialize};

use crate::web::auth;

const TIDAL_API_BASE: &str = "https://openapi.tidal.com/v2";

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TidalTrack {
    pub id: String,
    pub title: String,
    pub artist: String,
    pub album: Option<String>,
    pub duration_ms: Option<u64>,
    pub external_url: Option<String>,
    pub quality: Option<String>,
    pub explicit: bool,
    pub isrc: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TidalPlaylist {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub track_count: u64,
    pub external_url: Option<String>,
    pub image_url: Option<String>,
}

impl TidalTrack {
    pub fn open_url(&self) -> String {
        self.external_url
            .clone()
            .unwrap_or_else(|| format!("https://tidal.com/browse/track/{}", self.id))
    }

    pub fn artist_url(&self) -> String {
        format!("https://tidal.com/browse/artist/{}", self.id)
    }

    pub fn album_url(&self) -> Option<String> {
        self.album
            .as_ref()
            .map(|_album| format!("https://tidal.com/browse/album/{}", self.id))
    }
}

pub async fn tidal_access_token() -> Result<String, String> {
    let secrets = auth::load_provider_secrets("tidal")?
        .ok_or_else(|| "Save a TIDAL access token in Service Credentials first.".to_string())?;
    secrets
        .access_token
        .map(|token| token.trim().to_string())
        .filter(|token| !token.is_empty())
        .ok_or_else(|| "Save a TIDAL access token in Service Credentials first.".to_string())
}

pub fn tidal_api_base() -> &'static str {
    TIDAL_API_BASE
}

pub fn quality_from_media_tags(media_tags: Option<&[String]>) -> Option<String> {
    let tags = media_tags?;
    if tags.is_empty() {
        None
    } else {
        Some(tags.join(", "))
    }
}
