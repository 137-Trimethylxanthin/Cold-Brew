use musicbrainz_rs::entity::release::Release;
use musicbrainz_rs::prelude::*;
use serde::Serialize;

#[derive(Clone, Debug, Serialize)]
pub struct MusicBrainzRelease {
    pub mbid: String,
    pub title: String,
    pub artist: String,
    pub date: Option<String>,
    pub country: Option<String>,
    pub format: Option<String>,
    pub cover_art_url: Option<String>,
}

pub async fn search_release(artist: &str, title: &str) -> Result<Vec<MusicBrainzRelease>, String> {
    let query = format!(
        r#"release:"{}" AND artist:"{}""#,
        escape_lucene(title),
        escape_lucene(artist)
    );

    let mut search_query = Release::search(query);
    let result = search_query
        .execute_async()
        .await
        .map_err(|e| format!("MusicBrainz search error: {e}"))?;

    let releases: Vec<MusicBrainzRelease> = result
        .entities
        .into_iter()
        .filter_map(|release| {
            let mbid = release.id.to_string();
            let title = release.title;
            let artist = release
                .artist_credit
                .as_ref()
                .and_then(|ac| ac.first())
                .map(|ac| ac.artist.name.clone())
                .unwrap_or_else(|| "Unknown Artist".to_string());
            let date = release.date.map(String::from);
            let country = release.country;
            let format = release
                .media
                .as_ref()
                .and_then(|m| m.first())
                .and_then(|m| m.format.clone());
            let cover_art_url = Some(format!("https://coverartarchive.org/release/{mbid}"));

            Some(MusicBrainzRelease {
                mbid,
                title,
                artist,
                date,
                country,
                format,
                cover_art_url,
            })
        })
        .collect();

    Ok(releases)
}

pub async fn get_cover_art(mbid: &str) -> Result<Option<String>, String> {
    let url = format!("https://coverartarchive.org/release/{mbid}");
    let client = reqwest::Client::builder()
        .user_agent("Cold-Brew/0.1 (maxi@coldbrew.app)")
        .build()
        .map_err(|e| format!("HTTP client error: {e}"))?;

    let response = client
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("Cover Art Archive request failed: {e}"))?;

    if !response.status().is_success() {
        return Ok(None);
    }

    let json: serde_json::Value = response
        .json()
        .await
        .map_err(|e| format!("Cover Art Archive JSON error: {e}"))?;

    let image_url = json["images"]
        .as_array()
        .and_then(|images| images.first())
        .and_then(|img| img["thumbnails"]["large"].as_str())
        .or_else(|| {
            json["images"]
                .as_array()
                .and_then(|images| images.first())
                .and_then(|img| img["image"].as_str())
        })
        .map(|s| s.to_string());

    Ok(image_url)
}

fn escape_lucene(input: &str) -> String {
    let special = [
        '+', '-', '&', '|', '!', '(', ')', '{', '}', '[', ']', '^', '"', '~', '*', '?', ':', '\\',
    ];
    let mut result = String::with_capacity(input.len());
    for ch in input.chars() {
        if special.contains(&ch) {
            result.push('\\');
        }
        result.push(ch);
    }
    result
}
