use std::sync::{Arc, Mutex, MutexGuard, OnceLock};

use serde::Serialize;

static SPOTIFY_NATIVE: OnceLock<Mutex<SpotifyNative>> = OnceLock::new();

#[derive(Clone, Debug, Serialize)]
pub struct SpotifyNativeStatus {
    pub connected: bool,
    pub username: Option<String>,
    pub device_name: Option<String>,
    pub error: Option<String>,
}

pub struct SpotifyNative {
    state: SpotifyNativeState,
    username: Option<String>,
    device_name: Option<String>,
    last_error: Option<String>,
}

enum SpotifyNativeState {
    Disconnected,
    Connecting,
    #[cfg(feature = "native_spotify")]
    Connected {
        session: librespot_core::session::Session,
        player: Arc<librespot_playback::player::Player>,
    },
    #[cfg(not(feature = "native_spotify"))]
    Connected,
}

impl SpotifyNativeState {
    fn is_connected(&self) -> bool {
        matches!(self, Self::Connected { .. })
    }
}

impl std::fmt::Debug for SpotifyNativeState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Disconnected => f.write_str("Disconnected"),
            Self::Connecting => f.write_str("Connecting"),
            Self::Connected { .. } => f.write_str("Connected"),
        }
    }
}

impl SpotifyNative {
    fn new() -> Self {
        Self {
            state: SpotifyNativeState::Disconnected,
            username: None,
            device_name: None,
            last_error: None,
        }
    }

    fn status(&self) -> SpotifyNativeStatus {
        SpotifyNativeStatus {
            connected: self.state.is_connected(),
            username: self.username.clone(),
            device_name: self.device_name.clone(),
            error: self.last_error.clone(),
        }
    }
}

#[cfg(feature = "native_spotify")]
async fn do_connect(
    access_token: String,
) -> Result<
    (
        librespot_core::session::Session,
        Arc<librespot_playback::player::Player>,
        String,
    ),
    String,
> {
    let session_config = librespot_core::config::SessionConfig {
        device_id: "cold-brew-native".to_string(),
        ..Default::default()
    };

    let session = librespot_core::session::Session::new(session_config, None);

    let credentials = librespot_core::authentication::Credentials::with_access_token(access_token);

    let mixer_builder =
        librespot_playback::mixer::find(None).ok_or("No audio mixer backend found")?;
    let mixer = mixer_builder(librespot_playback::mixer::MixerConfig::default())
        .map_err(|error| format!("Failed to create mixer: {error}"))?;

    let player_config = librespot_playback::config::PlayerConfig {
        normalisation: true,
        ..Default::default()
    };

    let player = librespot_playback::player::Player::new(
        player_config,
        session.clone(),
        mixer.get_soft_volume(),
        move || {
            let backend =
                librespot_playback::audio_backend::find(None).expect("No audio backend available");
            backend(None, librespot_playback::config::AudioFormat::default())
        },
    );

    let (spirc, spirc_task) = librespot_connect::Spirc::new(
        librespot_connect::ConnectConfig::default(),
        session.clone(),
        credentials,
        player.clone(),
        mixer,
    )
    .await
    .map_err(|error| format!("Failed to create Spotify Connect device: {error}"))?;

    spirc
        .activate()
        .map_err(|error| format!("Failed to activate Spotify Connect: {error}"))?;

    let username = session.username().to_string();

    tokio::spawn(spirc_task);

    Ok((session, player, username))
}

fn lock_spotify_native() -> Result<MutexGuard<'static, SpotifyNative>, String> {
    SPOTIFY_NATIVE
        .get_or_init(|| Mutex::new(SpotifyNative::new()))
        .lock()
        .map_err(|_| "Spotify native player state is unavailable.".to_string())
}

#[cfg(feature = "native_spotify")]
pub async fn connect_spotify_native(access_token: String) -> Result<SpotifyNativeStatus, String> {
    {
        let player = lock_spotify_native()?;
        if player.state.is_connected() {
            return Ok(player.status());
        }
    }

    {
        let mut player = lock_spotify_native()?;
        player.last_error = None;
        player.state = SpotifyNativeState::Connecting;
    }

    match do_connect(access_token).await {
        Ok((session, player, username)) => {
            let mut p = lock_spotify_native()?;
            p.state = SpotifyNativeState::Connected { session, player };
            p.username = Some(username);
            p.device_name = Some("Cold-Brew".to_string());
            Ok(p.status())
        }
        Err(error) => {
            let mut p = lock_spotify_native()?;
            p.last_error = Some(error.clone());
            p.state = SpotifyNativeState::Disconnected;
            Err(error)
        }
    }
}

#[cfg(not(feature = "native_spotify"))]
pub async fn connect_spotify_native(_access_token: String) -> Result<SpotifyNativeStatus, String> {
    Err("Native Spotify playback is not available (feature not compiled).".to_string())
}

#[cfg(feature = "native_spotify")]
pub async fn disconnect_spotify_native() -> Result<SpotifyNativeStatus, String> {
    let mut player = lock_spotify_native()?;
    if let SpotifyNativeState::Connected { session, .. } =
        std::mem::replace(&mut player.state, SpotifyNativeState::Disconnected)
    {
        session.shutdown();
    }
    player.username = None;
    player.device_name = None;
    Ok(player.status())
}

#[cfg(not(feature = "native_spotify"))]
pub async fn disconnect_spotify_native() -> Result<SpotifyNativeStatus, String> {
    Ok(SpotifyNativeStatus {
        connected: false,
        username: None,
        device_name: None,
        error: Some("Native Spotify playback not compiled.".to_string()),
    })
}

pub fn spotify_native_status() -> Result<SpotifyNativeStatus, String> {
    let player = lock_spotify_native()?;
    Ok(player.status())
}

#[cfg(feature = "native_spotify")]
pub fn start_spotify_native_playback(
    track_uri: String,
    _device_id: Option<String>,
) -> Result<SpotifyNativeStatus, String> {
    let player = lock_spotify_native()?;
    match &player.state {
        SpotifyNativeState::Connected { player: p, .. } => {
            let uri = librespot_core::spotify_uri::SpotifyUri::from_uri(&track_uri)
                .map_err(|error| format!("Invalid Spotify URI: {error}"))?;
            p.load(uri, true, 0);
            p.play();
            Ok(player.status())
        }
        _ => Err("Spotify native player is not connected.".to_string()),
    }
}

#[cfg(not(feature = "native_spotify"))]
pub fn start_spotify_native_playback(
    _track_uri: String,
    _device_id: Option<String>,
) -> Result<SpotifyNativeStatus, String> {
    Err("Native Spotify playback is not available (feature not compiled).".to_string())
}

#[cfg(feature = "native_spotify")]
pub fn spotify_native_pause() -> Result<SpotifyNativeStatus, String> {
    let player = lock_spotify_native()?;
    match &player.state {
        SpotifyNativeState::Connected { player: p, .. } => {
            p.pause();
            Ok(player.status())
        }
        _ => Err("Spotify native player is not connected.".to_string()),
    }
}

#[cfg(not(feature = "native_spotify"))]
pub fn spotify_native_pause() -> Result<SpotifyNativeStatus, String> {
    Err("Native Spotify playback is not available (feature not compiled).".to_string())
}

#[cfg(feature = "native_spotify")]
pub fn spotify_native_resume() -> Result<SpotifyNativeStatus, String> {
    let player = lock_spotify_native()?;
    match &player.state {
        SpotifyNativeState::Connected { player: p, .. } => {
            p.play();
            Ok(player.status())
        }
        _ => Err("Spotify native player is not connected.".to_string()),
    }
}

#[cfg(not(feature = "native_spotify"))]
pub fn spotify_native_resume() -> Result<SpotifyNativeStatus, String> {
    Err("Native Spotify playback is not available (feature not compiled).".to_string())
}

#[cfg(feature = "native_spotify")]
pub fn spotify_native_stop() -> Result<SpotifyNativeStatus, String> {
    let player = lock_spotify_native()?;
    match &player.state {
        SpotifyNativeState::Connected { player: p, .. } => {
            p.stop();
            Ok(player.status())
        }
        _ => Err("Spotify native player is not connected.".to_string()),
    }
}

#[cfg(not(feature = "native_spotify"))]
pub fn spotify_native_stop() -> Result<SpotifyNativeStatus, String> {
    Err("Native Spotify playback is not available (feature not compiled).".to_string())
}
