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
            integration_state: "researched",
            auth_model: "OAuth",
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
                "Official docs say the SDK Player module is the only allowed playback path for third-party apps.",
                "The public third-party Player module is documented for TIDAL previews, not unrestricted full-track native playback.",
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
            integration_state: "researched",
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
                "The official YouTube Data API supports YouTube resources such as videos, playlists, and channels.",
                "No official public YouTube Music full-track playback API was confirmed.",
                "Use link-out or official embedded/player surfaces instead of scraping or bypassing playback restrictions.",
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
