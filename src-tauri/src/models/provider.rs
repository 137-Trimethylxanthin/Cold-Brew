use serde::Serialize;

#[derive(Clone, Debug, Serialize)]
pub struct ProviderCapability {
    pub id: &'static str,
    pub name: &'static str,
    pub integration_state: &'static str,
    pub auth_model: &'static str,
    pub documentation_url: &'static str,
    pub can_search: bool,
    pub can_list_playlists: bool,
    pub can_stream_full_tracks: bool,
    pub can_stream_previews: bool,
    pub can_link_out: bool,
    pub can_scrobble: bool,
    pub requires_oauth: bool,
    pub requires_partner_access: bool,
    pub notes: Vec<&'static str>,
}

pub fn list_service_capabilities() -> Vec<ProviderCapability> {
    vec![
        ProviderCapability {
            id: "local",
            name: "Local Files",
            integration_state: "implemented",
            auth_model: "none",
            documentation_url: "",
            can_search: false,
            can_list_playlists: true,
            can_stream_full_tracks: true,
            can_stream_previews: false,
            can_link_out: false,
            can_scrobble: false,
            requires_oauth: false,
            requires_partner_access: false,
            notes: vec![
                "Recursive scanning, metadata indexing, local playlists, and Rodio playback are implemented.",
                "Gapless playback for consecutive local queue tracks is implemented.",
            ],
        },
        ProviderCapability {
            id: "jellyfin",
            name: "Jellyfin",
            integration_state: "partial",
            auth_model: "server credentials",
            documentation_url: "https://api.jellyfin.org/",
            can_search: true,
            can_list_playlists: true,
            can_stream_full_tracks: true,
            can_stream_previews: false,
            can_link_out: true,
            can_scrobble: false,
            requires_oauth: false,
            requires_partner_access: false,
            notes: vec![
                "Cold-Brew currently stores Jellyfin credentials securely and can load tracks.",
                "Search, playlist loading, and streaming through the native player are still pending.",
            ],
        },
        ProviderCapability {
            id: "spotify",
            name: "Spotify",
            integration_state: "implemented",
            auth_model: "OAuth PKCE",
            documentation_url: "https://developer.spotify.com/documentation/web-api",
            can_search: true,
            can_list_playlists: true,
            can_stream_full_tracks: true,
            can_stream_previews: false,
            can_link_out: true,
            can_scrobble: false,
            requires_oauth: true,
            requires_partner_access: false,
            notes: vec![
                "Search, playlist loading, OAuth PKCE, and playback through Spotify Web Playback SDK / Spotify Connect are implemented.",
                "Full playback is constrained to Spotify's playback surfaces such as Web Playback SDK and requires a valid Premium user.",
                "Spotify policy forbids commercial streaming integrations and altering Spotify content.",
            ],
        },
        ProviderCapability {
            id: "tidal",
            name: "TIDAL",
            integration_state: "implemented",
            auth_model: "OAuth PKCE",
            documentation_url: "https://developer.tidal.com/documentation/api-sdk/api-sdk-overview",
            can_search: true,
            can_list_playlists: true,
            can_stream_full_tracks: false,
            can_stream_previews: true,
            can_link_out: true,
            can_scrobble: false,
            requires_oauth: true,
            requires_partner_access: false,
            notes: vec![
                "TIDAL search, playlist loading, and OAuth PKCE login are implemented via the public Web API (openapi.tidal.com/v2).",
                "Full-track streaming is only available through the official TIDAL SDK Player module for approved partners.",
                "Preview clips may be available via the SDK; link-out to tidal.com for full playback.",
            ],
        },
        ProviderCapability {
            id: "qobuz",
            name: "Qobuz",
            integration_state: "researched",
            auth_model: "Qobuz-issued app id and secret",
            documentation_url: "https://static.qobuz.com/apps/api/QobuzAPI-TermsofUse.pdf",
            can_search: true,
            can_list_playlists: true,
            can_stream_full_tracks: false,
            can_stream_previews: false,
            can_link_out: true,
            can_scrobble: false,
            requires_oauth: false,
            requires_partner_access: true,
            notes: vec![
                "Qobuz API terms require a Qobuz-issued application id and secret.",
                "The app secret must not be shared and Qobuz may limit calls, access, and geoblocked metadata.",
                "Full-track playback is treated as partner-gated until a current approved API contract confirms it.",
            ],
        },
        ProviderCapability {
            id: "youtube",
            name: "YouTube / YouTube Music",
            integration_state: "implemented",
            auth_model: "Google OAuth / API key",
            documentation_url: "https://developers.google.com/youtube/v3/getting-started",
            can_search: true,
            can_list_playlists: true,
            can_stream_full_tracks: false,
            can_stream_previews: false,
            can_link_out: true,
            can_scrobble: false,
            requires_oauth: true,
            requires_partner_access: false,
            notes: vec![
                "YouTube Music metadata search uses the YouTube Data API with category filtering for music content.",
                "Link-out to music.youtube.com for each track; no streaming (YouTube ToS prohibits unauthorized playback).",
                "Search returns video IDs, titles, channel names, durations, and thumbnails.",
            ],
        },
        ProviderCapability {
            id: "soundcloud",
            name: "SoundCloud",
            integration_state: "implemented",
            auth_model: "API key / client ID",
            documentation_url: "https://developers.soundcloud.com/",
            can_search: true,
            can_list_playlists: false,
            can_stream_full_tracks: false,
            can_stream_previews: true,
            can_link_out: true,
            can_scrobble: false,
            requires_oauth: false,
            requires_partner_access: false,
            notes: vec![
                "SoundCloud search uses api-v2.soundcloud.com with a well-known client ID or user-provided API key.",
                "30-second MP3 preview streams are available for most tracks via progressive transcoding URLs.",
                "Full-track streaming requires a SoundCloud Go+ subscription and official SDK integration.",
                "Link-out to soundcloud.com for full track playback and artist pages.",
            ],
        },
        ProviderCapability {
            id: "bandcamp",
            name: "Bandcamp",
            integration_state: "researched",
            auth_model: "not available for music playback",
            documentation_url: "https://bandcamp.com/developer",
            can_search: false,
            can_list_playlists: false,
            can_stream_full_tracks: false,
            can_stream_previews: false,
            can_link_out: true,
            can_scrobble: false,
            requires_oauth: false,
            requires_partner_access: true,
            notes: vec![
                "Official Bandcamp developer docs expose account and merch/order APIs, not a public music-library playback API.",
                "Treat Bandcamp as link-out or user-owned local downloads until a supported music API is confirmed.",
            ],
        },
        ProviderCapability {
            id: "lastfm",
            name: "Last.fm",
            integration_state: "researched",
            auth_model: "API key plus authenticated session",
            documentation_url: "https://www.last.fm/api/scrobbling",
            can_search: true,
            can_list_playlists: false,
            can_stream_full_tracks: false,
            can_stream_previews: false,
            can_link_out: true,
            can_scrobble: true,
            requires_oauth: true,
            requires_partner_access: false,
            notes: vec![
                "Scrobbling uses track.updateNowPlaying and track.scrobble POST requests.",
                "Scrobble only after tracks longer than 30 seconds are played for half their duration or 4 minutes, whichever comes first.",
                "Failed scrobbles should be cached locally and retried in order.",
            ],
        },
        ProviderCapability {
            id: "lrclib",
            name: "LRCLIB",
            integration_state: "implemented",
            auth_model: "none",
            documentation_url: "https://lrclib.net/docs",
            can_search: true,
            can_list_playlists: false,
            can_stream_full_tracks: false,
            can_stream_previews: false,
            can_link_out: true,
            can_scrobble: false,
            requires_oauth: false,
            requires_partner_access: false,
            notes: vec![
                "Cold-Brew uses LRCLIB for online synced/plain lyrics after checking embedded and sibling lyrics.",
                "LRCLIB does not require an API key and recommends a client User-Agent.",
            ],
        },
    ]
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::list_service_capabilities;

    #[test]
    fn provider_ids_are_unique() {
        let providers = list_service_capabilities();
        let mut ids = HashSet::new();

        for provider in providers {
            assert!(ids.insert(provider.id));
        }
    }

    #[test]
    fn researched_streaming_services_include_documentation() {
        for provider in list_service_capabilities()
            .into_iter()
            .filter(|provider| provider.integration_state == "researched")
        {
            assert!(!provider.documentation_url.is_empty());
            assert!(!provider.notes.is_empty());
        }
    }
}
