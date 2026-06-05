use std::process::Command;
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use rand::RngCore;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::time::timeout;

use crate::{credentials, scrobbling};

const SPOTIFY_AUTHORIZE_ENDPOINT: &str = "https://accounts.spotify.com/authorize";
const SPOTIFY_TOKEN_ENDPOINT: &str = "https://accounts.spotify.com/api/token";
const SPOTIFY_DEFAULT_SCOPE: &str =
    "playlist-read-private playlist-read-collaborative user-read-private user-read-email user-read-playback-state user-modify-playback-state streaming";
const TIDAL_AUTHORIZE_ENDPOINT: &str = "https://login.tidal.com/authorize";
const TIDAL_TOKEN_ENDPOINT: &str = "https://auth.tidal.com/v1/oauth2/token";
const TIDAL_DEFAULT_SCOPE: &str = "search.read playlists.read user.read";
const GOOGLE_AUTHORIZE_ENDPOINT: &str = "https://accounts.google.com/o/oauth2/v2/auth";
const GOOGLE_TOKEN_ENDPOINT: &str = "https://oauth2.googleapis.com/token";
const YOUTUBE_DEFAULT_SCOPE: &str = "https://www.googleapis.com/auth/youtube.readonly";
const LASTFM_ENDPOINT: &str = "https://ws.audioscrobbler.com/2.0/";
const LASTFM_AUTH_ENDPOINT: &str = "https://www.last.fm/api/auth/";

static SPOTIFY_PKCE_LOGIN: OnceLock<Mutex<Option<PkceLogin>>> = OnceLock::new();
static TIDAL_PKCE_LOGIN: OnceLock<Mutex<Option<PkceLogin>>> = OnceLock::new();
static YOUTUBE_PKCE_LOGIN: OnceLock<Mutex<Option<PkceLogin>>> = OnceLock::new();
static LASTFM_LOGIN: OnceLock<Mutex<Option<LastFmLogin>>> = OnceLock::new();

#[derive(Clone, Debug, Serialize)]
pub struct ProviderLoginStart {
    pub provider_id: String,
    pub authorization_url: String,
    pub state: Option<String>,
    pub message: String,
}

#[derive(Clone, Debug)]
struct PkceLogin {
    code_verifier: String,
    state: String,
    redirect_uri: String,
}

#[derive(Clone, Debug)]
struct LocalCallback {
    bind_address: String,
    path: String,
}

#[derive(Clone, Debug)]
struct OAuthCallbackCode {
    code: String,
    state: Option<String>,
}

#[derive(Clone, Debug)]
struct LastFmLogin {
    token: String,
}

#[derive(Debug, Deserialize)]
struct OAuthTokenResponse {
    access_token: String,
    refresh_token: Option<String>,
}

#[derive(Debug, Deserialize)]
struct LastFmTokenResponse {
    token: String,
}

#[derive(Debug, Deserialize)]
struct LastFmSessionResponse {
    session: LastFmSession,
}

#[derive(Debug, Deserialize)]
struct LastFmSession {
    name: String,
    key: String,
}

pub fn start_spotify_pkce_login(
    redirect_uri: String,
    scope: Option<String>,
) -> Result<ProviderLoginStart, String> {
    let client_id = required_provider_secret("spotify", |secrets| secrets.client_id)?
        .ok_or_else(|| "Save a Spotify Client ID in Service Credentials first.".to_string())?;
    let redirect_uri = non_empty(redirect_uri, "Spotify redirect URI")?;
    let scope = scope
        .and_then(non_empty_owned)
        .unwrap_or_else(|| SPOTIFY_DEFAULT_SCOPE.to_string());
    let code_verifier = random_urlsafe_token(64);
    let code_challenge = pkce_challenge(&code_verifier);
    let state = random_urlsafe_token(32);

    let mut url = reqwest::Url::parse(SPOTIFY_AUTHORIZE_ENDPOINT)
        .map_err(|error| format!("Could not build Spotify authorization URL: {error}"))?;
    url.query_pairs_mut()
        .append_pair("response_type", "code")
        .append_pair("client_id", &client_id)
        .append_pair("scope", &scope)
        .append_pair("code_challenge_method", "S256")
        .append_pair("code_challenge", &code_challenge)
        .append_pair("redirect_uri", &redirect_uri)
        .append_pair("state", &state);

    *spotify_login_state()
        .lock()
        .map_err(|_| "Spotify login state is unavailable.".to_string())? = Some(PkceLogin {
        code_verifier,
        state: state.clone(),
        redirect_uri,
    });

    Ok(ProviderLoginStart {
        provider_id: "spotify".to_string(),
        authorization_url: url.to_string(),
        state: Some(state),
        message: "Open the Spotify authorization URL and paste the returned code.".to_string(),
    })
}

pub async fn finish_spotify_pkce_login(
    code: String,
    state: Option<String>,
) -> Result<credentials::ProviderAccount, String> {
    let code = non_empty(code, "Spotify authorization code")?;
    let login = spotify_login_state()
        .lock()
        .map_err(|_| "Spotify login state is unavailable.".to_string())?
        .clone()
        .ok_or_else(|| "Start Spotify login before finishing it.".to_string())?;
    if let Some(state) = state.and_then(non_empty_owned) {
        if state != login.state {
            return Err("Spotify authorization state does not match the active login.".to_string());
        }
    }

    let client_id = required_provider_secret("spotify", |secrets| secrets.client_id)?
        .ok_or_else(|| "Save a Spotify Client ID in Service Credentials first.".to_string())?;
    let response = reqwest::Client::new()
        .post(SPOTIFY_TOKEN_ENDPOINT)
        .form(&[
            ("client_id", client_id.as_str()),
            ("grant_type", "authorization_code"),
            ("code", code.as_str()),
            ("redirect_uri", login.redirect_uri.as_str()),
            ("code_verifier", login.code_verifier.as_str()),
        ])
        .send()
        .await
        .map_err(|error| format!("Could not exchange Spotify authorization code: {error}"))?;
    let token = parse_oauth_token_response(response, "Spotify authorization", "spotify").await?;
    *spotify_login_state()
        .lock()
        .map_err(|_| "Spotify login state is unavailable.".to_string())? = None;

    credentials::save_provider_account(
        "spotify".to_string(),
        None,
        None,
        None,
        None,
        None,
        Some(token.access_token),
        token.refresh_token,
    )
}

pub async fn refresh_spotify_access_token() -> Result<credentials::ProviderAccount, String> {
    let secrets = credentials::load_provider_secrets("spotify")?
        .ok_or_else(|| "Save Spotify Client ID and refresh token first.".to_string())?;
    let client_id = secrets
        .client_id
        .and_then(non_empty_owned)
        .ok_or_else(|| "Save a Spotify Client ID first.".to_string())?;
    let refresh_token = secrets
        .refresh_token
        .and_then(non_empty_owned)
        .ok_or_else(|| "Save a Spotify refresh token first.".to_string())?;

    let response = reqwest::Client::new()
        .post(SPOTIFY_TOKEN_ENDPOINT)
        .form(&[
            ("client_id", client_id.as_str()),
            ("grant_type", "refresh_token"),
            ("refresh_token", refresh_token.as_str()),
        ])
        .send()
        .await
        .map_err(|error| format!("Could not refresh Spotify access token: {error}"))?;
    let token = parse_oauth_token_response(response, "Spotify token refresh", "spotify").await?;

    credentials::save_provider_account(
        "spotify".to_string(),
        None,
        None,
        None,
        None,
        None,
        Some(token.access_token),
        token.refresh_token.or(Some(refresh_token)),
    )
}

pub async fn complete_spotify_pkce_login_in_browser(
    redirect_uri: String,
    scope: Option<String>,
) -> Result<credentials::ProviderAccount, String> {
    let callback = local_callback_from_redirect_uri(&redirect_uri, "Spotify")?;
    let listener = TcpListener::bind(&callback.bind_address)
        .await
        .map_err(|error| {
            format!(
                "Could not listen for Spotify callback on {}: {error}",
                callback.bind_address
            )
        })?;
    let login = start_spotify_pkce_login(redirect_uri, scope)?;
    let expected_state = login
        .state
        .clone()
        .ok_or_else(|| "Spotify login did not create an authorization state.".to_string())?;

    open_authorization_url(&login.authorization_url)?;
    let callback_code = wait_for_oauth_callback(listener, &callback.path, &expected_state).await?;
    finish_spotify_pkce_login(callback_code.code, callback_code.state).await
}

pub fn get_spotify_web_playback_token() -> Result<String, String> {
    credentials::load_provider_secrets("spotify")?
        .and_then(|secrets| secrets.access_token)
        .and_then(non_empty_owned)
        .ok_or_else(|| "Complete Spotify OAuth before using Spotify Web Playback.".to_string())
}

pub fn start_tidal_pkce_login(
    redirect_uri: String,
    scope: Option<String>,
) -> Result<ProviderLoginStart, String> {
    let client_id = required_provider_secret("tidal", |secrets| secrets.client_id)?
        .ok_or_else(|| "Save a TIDAL Client ID in Service Credentials first.".to_string())?;
    let redirect_uri = non_empty(redirect_uri, "TIDAL redirect URI")?;
    let scope = scope
        .and_then(non_empty_owned)
        .unwrap_or_else(|| TIDAL_DEFAULT_SCOPE.to_string());
    let (authorization_url, login) = build_pkce_authorization_url(
        TIDAL_AUTHORIZE_ENDPOINT,
        &client_id,
        redirect_uri,
        scope,
        &[],
        "TIDAL",
    )?;

    let state = login.state.clone();
    *tidal_login_state()
        .lock()
        .map_err(|_| "TIDAL login state is unavailable.".to_string())? = Some(login);

    Ok(ProviderLoginStart {
        provider_id: "tidal".to_string(),
        authorization_url,
        state: Some(state),
        message: "Open the TIDAL authorization URL and paste the returned code.".to_string(),
    })
}

pub async fn finish_tidal_pkce_login(
    code: String,
    state: Option<String>,
) -> Result<credentials::ProviderAccount, String> {
    let code = non_empty(code, "TIDAL authorization code")?;
    let login = tidal_login_state()
        .lock()
        .map_err(|_| "TIDAL login state is unavailable.".to_string())?
        .clone()
        .ok_or_else(|| "Start TIDAL login before finishing it.".to_string())?;
    if let Some(state) = state.and_then(non_empty_owned) {
        if state != login.state {
            return Err("TIDAL authorization state does not match the active login.".to_string());
        }
    }

    let client_id = required_provider_secret("tidal", |secrets| secrets.client_id)?
        .ok_or_else(|| "Save a TIDAL Client ID in Service Credentials first.".to_string())?;
    let response = reqwest::Client::new()
        .post(TIDAL_TOKEN_ENDPOINT)
        .form(&[
            ("client_id", client_id.as_str()),
            ("grant_type", "authorization_code"),
            ("code", code.as_str()),
            ("redirect_uri", login.redirect_uri.as_str()),
            ("code_verifier", login.code_verifier.as_str()),
        ])
        .send()
        .await
        .map_err(|error| format!("Could not exchange TIDAL authorization code: {error}"))?;
    let token = parse_oauth_token_response(response, "TIDAL authorization", "tidal").await?;
    *tidal_login_state()
        .lock()
        .map_err(|_| "TIDAL login state is unavailable.".to_string())? = None;

    credentials::save_provider_account(
        "tidal".to_string(),
        None,
        None,
        None,
        None,
        None,
        Some(token.access_token),
        token.refresh_token,
    )
}

pub async fn refresh_tidal_access_token() -> Result<credentials::ProviderAccount, String> {
    let secrets = credentials::load_provider_secrets("tidal")?
        .ok_or_else(|| "Save a TIDAL refresh token first.".to_string())?;
    let refresh_token = secrets
        .refresh_token
        .and_then(non_empty_owned)
        .ok_or_else(|| "Save a TIDAL refresh token first.".to_string())?;

    let response = reqwest::Client::new()
        .post(TIDAL_TOKEN_ENDPOINT)
        .form(&[
            ("grant_type", "refresh_token"),
            ("refresh_token", refresh_token.as_str()),
        ])
        .send()
        .await
        .map_err(|error| format!("Could not refresh TIDAL access token: {error}"))?;
    let token = parse_oauth_token_response(response, "TIDAL token refresh", "tidal").await?;

    credentials::save_provider_account(
        "tidal".to_string(),
        None,
        None,
        None,
        None,
        None,
        Some(token.access_token),
        token.refresh_token.or(Some(refresh_token)),
    )
}

pub fn start_youtube_oauth_login(
    redirect_uri: String,
    scope: Option<String>,
) -> Result<ProviderLoginStart, String> {
    let client_id =
        required_provider_secret("youtube", |secrets| secrets.client_id)?.ok_or_else(|| {
            "Save a YouTube OAuth Client ID in Service Credentials first.".to_string()
        })?;
    let redirect_uri = non_empty(redirect_uri, "YouTube redirect URI")?;
    let scope = scope
        .and_then(non_empty_owned)
        .unwrap_or_else(|| YOUTUBE_DEFAULT_SCOPE.to_string());
    let (authorization_url, login) = build_pkce_authorization_url(
        GOOGLE_AUTHORIZE_ENDPOINT,
        &client_id,
        redirect_uri,
        scope,
        &[],
        "YouTube",
    )?;

    let state = login.state.clone();
    *youtube_login_state()
        .lock()
        .map_err(|_| "YouTube login state is unavailable.".to_string())? = Some(login);

    Ok(ProviderLoginStart {
        provider_id: "youtube".to_string(),
        authorization_url,
        state: Some(state),
        message: "Open the Google authorization URL and paste the returned code.".to_string(),
    })
}

pub async fn finish_youtube_oauth_login(
    code: String,
    state: Option<String>,
) -> Result<credentials::ProviderAccount, String> {
    let code = non_empty(code, "YouTube authorization code")?;
    let login = youtube_login_state()
        .lock()
        .map_err(|_| "YouTube login state is unavailable.".to_string())?
        .clone()
        .ok_or_else(|| "Start YouTube login before finishing it.".to_string())?;
    if let Some(state) = state.and_then(non_empty_owned) {
        if state != login.state {
            return Err("YouTube authorization state does not match the active login.".to_string());
        }
    }

    let secrets = credentials::load_provider_secrets("youtube")?.ok_or_else(|| {
        "Save a YouTube OAuth Client ID in Service Credentials first.".to_string()
    })?;
    let client_id = secrets
        .client_id
        .and_then(non_empty_owned)
        .ok_or_else(|| "Save a YouTube OAuth Client ID first.".to_string())?;
    let mut form = vec![
        ("client_id", client_id),
        ("grant_type", "authorization_code".to_string()),
        ("code", code),
        ("redirect_uri", login.redirect_uri),
        ("code_verifier", login.code_verifier),
    ];
    if let Some(client_secret) = secrets.client_secret.and_then(non_empty_owned) {
        form.push(("client_secret", client_secret));
    }

    let response = reqwest::Client::new()
        .post(GOOGLE_TOKEN_ENDPOINT)
        .form(&form)
        .send()
        .await
        .map_err(|error| format!("Could not exchange YouTube authorization code: {error}"))?;
    let token = parse_oauth_token_response(response, "YouTube authorization", "youtube").await?;
    *youtube_login_state()
        .lock()
        .map_err(|_| "YouTube login state is unavailable.".to_string())? = None;

    credentials::save_provider_account(
        "youtube".to_string(),
        None,
        None,
        None,
        None,
        None,
        Some(token.access_token),
        token.refresh_token,
    )
}

pub async fn refresh_youtube_access_token() -> Result<credentials::ProviderAccount, String> {
    let secrets = credentials::load_provider_secrets("youtube")?
        .ok_or_else(|| "Save YouTube OAuth Client ID and refresh token first.".to_string())?;
    let client_id = secrets
        .client_id
        .and_then(non_empty_owned)
        .ok_or_else(|| "Save a YouTube OAuth Client ID first.".to_string())?;
    let refresh_token = secrets
        .refresh_token
        .and_then(non_empty_owned)
        .ok_or_else(|| "Save a YouTube refresh token first.".to_string())?;
    let mut form = vec![
        ("client_id", client_id),
        ("refresh_token", refresh_token.clone()),
        ("grant_type", "refresh_token".to_string()),
    ];
    if let Some(client_secret) = secrets.client_secret.and_then(non_empty_owned) {
        form.push(("client_secret", client_secret));
    }

    let response = reqwest::Client::new()
        .post(GOOGLE_TOKEN_ENDPOINT)
        .form(&form)
        .send()
        .await
        .map_err(|error| format!("Could not refresh YouTube access token: {error}"))?;
    let token = parse_oauth_token_response(response, "YouTube token refresh", "youtube").await?;

    credentials::save_provider_account(
        "youtube".to_string(),
        None,
        None,
        None,
        None,
        None,
        Some(token.access_token),
        token.refresh_token.or(Some(refresh_token)),
    )
}

pub async fn start_lastfm_login() -> Result<ProviderLoginStart, String> {
    let (api_key, api_secret) = lastfm_api_key_and_secret()?;
    let mut params = vec![
        ("method".to_string(), "auth.getToken".to_string()),
        ("api_key".to_string(), api_key.clone()),
        ("format".to_string(), "json".to_string()),
    ];
    params.push((
        "api_sig".to_string(),
        scrobbling::lastfm_signature(&params, &api_secret),
    ));

    let response = reqwest::Client::new()
        .post(LASTFM_ENDPOINT)
        .form(&params)
        .send()
        .await
        .map_err(|error| format!("Could not request Last.fm auth token: {error}"))?;
    let status = response.status();
    let body = response
        .text()
        .await
        .map_err(|error| format!("Could not read Last.fm auth token response: {error}"))?;
    if !status.is_success() {
        let message = format!("Last.fm auth token HTTP {status}: {body}");
        credentials::record_provider_auth_failure("lastfm", message.clone());
        return Err(message);
    }
    let token_response = parse_lastfm_response::<LastFmTokenResponse>(&body, "auth token")?;
    let token = token_response.token;

    *lastfm_login_state()
        .lock()
        .map_err(|_| "Last.fm login state is unavailable.".to_string())? = Some(LastFmLogin {
        token: token.clone(),
    });

    let mut url = reqwest::Url::parse(LASTFM_AUTH_ENDPOINT)
        .map_err(|error| format!("Could not build Last.fm authorization URL: {error}"))?;
    url.query_pairs_mut()
        .append_pair("api_key", &api_key)
        .append_pair("token", &token);

    Ok(ProviderLoginStart {
        provider_id: "lastfm".to_string(),
        authorization_url: url.to_string(),
        state: None,
        message: "Open the Last.fm authorization URL, approve access, then finish login."
            .to_string(),
    })
}

pub async fn finish_lastfm_login() -> Result<credentials::ProviderAccount, String> {
    let (api_key, api_secret) = lastfm_api_key_and_secret()?;
    let token = lastfm_login_state()
        .lock()
        .map_err(|_| "Last.fm login state is unavailable.".to_string())?
        .clone()
        .ok_or_else(|| "Start Last.fm login before finishing it.".to_string())?
        .token;
    let mut params = vec![
        ("method".to_string(), "auth.getSession".to_string()),
        ("api_key".to_string(), api_key),
        ("token".to_string(), token),
        ("format".to_string(), "json".to_string()),
    ];
    params.push((
        "api_sig".to_string(),
        scrobbling::lastfm_signature(&params, &api_secret),
    ));

    let response = reqwest::Client::new()
        .post(LASTFM_ENDPOINT)
        .form(&params)
        .send()
        .await
        .map_err(|error| format!("Could not create Last.fm session: {error}"))?;
    let status = response.status();
    let body = response
        .text()
        .await
        .map_err(|error| format!("Could not read Last.fm session response: {error}"))?;
    if !status.is_success() {
        let message = format!("Last.fm session HTTP {status}: {body}");
        credentials::record_provider_auth_failure("lastfm", message.clone());
        return Err(message);
    }
    let session_response = parse_lastfm_response::<LastFmSessionResponse>(&body, "session")?;
    *lastfm_login_state()
        .lock()
        .map_err(|_| "Last.fm login state is unavailable.".to_string())? = None;

    credentials::save_provider_account(
        "lastfm".to_string(),
        Some(session_response.session.name),
        None,
        None,
        None,
        None,
        Some(session_response.session.key),
        None,
    )
}

fn build_pkce_authorization_url(
    authorize_endpoint: &str,
    client_id: &str,
    redirect_uri: String,
    scope: String,
    extra_params: &[(&str, &str)],
    label: &str,
) -> Result<(String, PkceLogin), String> {
    let code_verifier = random_urlsafe_token(64);
    let code_challenge = pkce_challenge(&code_verifier);
    let state = random_urlsafe_token(32);
    let mut url = reqwest::Url::parse(authorize_endpoint)
        .map_err(|error| format!("Could not build {label} authorization URL: {error}"))?;
    {
        let mut query = url.query_pairs_mut();
        query
            .append_pair("response_type", "code")
            .append_pair("client_id", client_id)
            .append_pair("scope", &scope)
            .append_pair("code_challenge_method", "S256")
            .append_pair("code_challenge", &code_challenge)
            .append_pair("redirect_uri", &redirect_uri)
            .append_pair("state", &state);
        for (name, value) in extra_params {
            query.append_pair(name, value);
        }
    }

    Ok((
        url.to_string(),
        PkceLogin {
            code_verifier,
            state,
            redirect_uri,
        },
    ))
}

fn local_callback_from_redirect_uri(
    redirect_uri: &str,
    label: &str,
) -> Result<LocalCallback, String> {
    let url = reqwest::Url::parse(redirect_uri)
        .map_err(|error| format!("{label} redirect URI is invalid: {error}"))?;
    if url.scheme() != "http" {
        return Err(format!(
            "{label} automated login requires an HTTP loopback redirect URI."
        ));
    }

    let host = url
        .host_str()
        .ok_or_else(|| format!("{label} redirect URI must include a loopback host."))?;
    if !matches!(host, "127.0.0.1" | "::1") {
        return Err(format!(
            "{label} automated login requires a loopback IP literal redirect URI such as http://127.0.0.1:9090/callback."
        ));
    }
    let port = url
        .port()
        .ok_or_else(|| format!("{label} redirect URI must include a local callback port."))?;
    if url.path().is_empty() || url.path() == "/" {
        return Err(format!(
            "{label} redirect URI must include a callback path such as /callback."
        ));
    }

    Ok(LocalCallback {
        bind_address: if host == "::1" {
            format!("[::1]:{port}")
        } else {
            format!("{host}:{port}")
        },
        path: url.path().to_string(),
    })
}

async fn wait_for_oauth_callback(
    listener: TcpListener,
    expected_path: &str,
    expected_state: &str,
) -> Result<OAuthCallbackCode, String> {
    let callback = timeout(Duration::from_secs(180), async {
        loop {
            let (mut stream, _) = listener.accept().await.map_err(|error| {
                format!("Could not accept Spotify callback connection: {error}")
            })?;
            let mut buffer = vec![0; 8192];
            let read = stream
                .read(&mut buffer)
                .await
                .map_err(|error| format!("Could not read Spotify callback request: {error}"))?;
            let request = String::from_utf8_lossy(&buffer[..read]).to_string();
            let result = parse_oauth_callback_request(&request, expected_path, expected_state);
            let response = match &result {
                Ok(_) => callback_http_response(
                    "Spotify login complete. You can close this browser tab.",
                    "Spotify login complete",
                ),
                Err(error) => callback_http_response(error, "Spotify login failed"),
            };
            let _ = stream.write_all(response.as_bytes()).await;
            stream.shutdown().await.ok();
            if result.is_ok() || !is_ignorable_callback_error(&result) {
                return result;
            }
        }
    })
    .await
    .map_err(|_| "Spotify login timed out waiting for the browser callback.".to_string())??;

    Ok(callback)
}

fn parse_oauth_callback_request(
    request: &str,
    expected_path: &str,
    expected_state: &str,
) -> Result<OAuthCallbackCode, String> {
    let request_line = request
        .lines()
        .next()
        .ok_or_else(|| "Spotify callback request was empty.".to_string())?;
    let mut parts = request_line.split_whitespace();
    let method = parts
        .next()
        .ok_or_else(|| "Spotify callback request method was missing.".to_string())?;
    let target = parts
        .next()
        .ok_or_else(|| "Spotify callback request target was missing.".to_string())?;
    if method != "GET" {
        return Err("Spotify callback must use GET.".to_string());
    }

    let url = reqwest::Url::parse(&format!("http://127.0.0.1{target}"))
        .map_err(|error| format!("Spotify callback URL is invalid: {error}"))?;
    if url.path() != expected_path {
        return Err(format!(
            "Ignoring request for {}; waiting for Spotify callback on {expected_path}.",
            url.path()
        ));
    }

    if let Some(error) = url
        .query_pairs()
        .find_map(|(name, value)| (name == "error").then(|| value.into_owned()))
    {
        return Err(format!("Spotify authorization failed: {error}"));
    }

    let code = url
        .query_pairs()
        .find_map(|(name, value)| (name == "code").then(|| value.into_owned()))
        .and_then(non_empty_owned)
        .ok_or_else(|| "Spotify callback did not include an authorization code.".to_string())?;
    let state = url
        .query_pairs()
        .find_map(|(name, value)| (name == "state").then(|| value.into_owned()))
        .and_then(non_empty_owned);
    if state.as_deref() != Some(expected_state) {
        return Err("Spotify authorization state does not match the active login.".to_string());
    }

    Ok(OAuthCallbackCode { code, state })
}

fn is_ignorable_callback_error(result: &Result<OAuthCallbackCode, String>) -> bool {
    result
        .as_ref()
        .err()
        .is_some_and(|error| error.starts_with("Ignoring request for "))
}

fn callback_http_response(body: &str, title: &str) -> String {
    let title = html_escape(title);
    let body = html_escape(body);
    let html = format!(
        "<!doctype html><html><head><meta charset=\"utf-8\"><title>{title}</title></head><body><h1>{title}</h1><p>{body}</p></body></html>"
    );
    format!(
        "HTTP/1.1 200 OK\r\ncontent-type: text/html; charset=utf-8\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",
        html.len(),
        html
    )
}

fn html_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

fn open_authorization_url(url: &str) -> Result<(), String> {
    let status = if cfg!(target_os = "windows") {
        Command::new("cmd").args(["/C", "start", "", url]).status()
    } else if cfg!(target_os = "macos") {
        Command::new("open").arg(url).status()
    } else {
        Command::new("xdg-open").arg(url).status()
    }
    .map_err(|error| format!("Could not open Spotify authorization URL in a browser: {error}"))?;

    if status.success() {
        Ok(())
    } else {
        Err(format!(
            "Could not open Spotify authorization URL in a browser; opener exited with {status}."
        ))
    }
}

async fn parse_oauth_token_response(
    response: reqwest::Response,
    label: &str,
    provider_id: &str,
) -> Result<OAuthTokenResponse, String> {
    let status = response.status();
    let body = response
        .text()
        .await
        .map_err(|error| format!("Could not read {label} response: {error}"))?;
    if !status.is_success() {
        let message = format!("{label} failed with HTTP {status}: {body}");
        credentials::record_provider_auth_failure(provider_id, message.clone());
        return Err(message);
    }
    let token = serde_json::from_str::<OAuthTokenResponse>(&body)
        .map_err(|error| format!("Could not parse {label} response: {error}; {body}"))?;
    credentials::clear_provider_auth_failure(provider_id);
    Ok(token)
}

fn parse_lastfm_response<T: for<'de> Deserialize<'de>>(
    body: &str,
    label: &str,
) -> Result<T, String> {
    let value = serde_json::from_str::<Value>(body)
        .map_err(|error| format!("Could not parse Last.fm {label} response: {error}; {body}"))?;
    if let Some(code) = value.get("error").and_then(Value::as_i64) {
        let message = value
            .get("message")
            .and_then(Value::as_str)
            .unwrap_or("Last.fm API error");
        let error = format!("Last.fm {label} failed with error {code}: {message}");
        credentials::record_provider_auth_failure("lastfm", error.clone());
        return Err(error);
    }
    credentials::clear_provider_auth_failure("lastfm");
    serde_json::from_value::<T>(value)
        .map_err(|error| format!("Could not decode Last.fm {label} response: {error}"))
}

fn required_provider_secret<F>(provider_id: &str, select: F) -> Result<Option<String>, String>
where
    F: FnOnce(credentials::ProviderSecrets) -> Option<String>,
{
    Ok(credentials::load_provider_secrets(provider_id)?
        .and_then(select)
        .and_then(non_empty_owned))
}

fn lastfm_api_key_and_secret() -> Result<(String, String), String> {
    let secrets = credentials::load_provider_secrets("lastfm")?
        .ok_or_else(|| "Save a Last.fm API key and API secret first.".to_string())?;
    let api_key = secrets
        .api_key
        .and_then(non_empty_owned)
        .ok_or_else(|| "Save a Last.fm API key first.".to_string())?;
    let api_secret = secrets
        .api_secret
        .and_then(non_empty_owned)
        .ok_or_else(|| "Save a Last.fm API secret first.".to_string())?;
    Ok((api_key, api_secret))
}

fn spotify_login_state() -> &'static Mutex<Option<PkceLogin>> {
    SPOTIFY_PKCE_LOGIN.get_or_init(|| Mutex::new(None))
}

fn tidal_login_state() -> &'static Mutex<Option<PkceLogin>> {
    TIDAL_PKCE_LOGIN.get_or_init(|| Mutex::new(None))
}

fn youtube_login_state() -> &'static Mutex<Option<PkceLogin>> {
    YOUTUBE_PKCE_LOGIN.get_or_init(|| Mutex::new(None))
}

fn lastfm_login_state() -> &'static Mutex<Option<LastFmLogin>> {
    LASTFM_LOGIN.get_or_init(|| Mutex::new(None))
}

fn pkce_challenge(code_verifier: &str) -> String {
    let digest = Sha256::digest(code_verifier.as_bytes());
    URL_SAFE_NO_PAD.encode(digest)
}

fn random_urlsafe_token(byte_count: usize) -> String {
    let mut bytes = vec![0; byte_count];
    rand::thread_rng().fill_bytes(&mut bytes);
    URL_SAFE_NO_PAD.encode(bytes)
}

fn non_empty(value: String, label: &str) -> Result<String, String> {
    non_empty_owned(value).ok_or_else(|| format!("{label} must not be empty."))
}

fn non_empty_owned(value: String) -> Option<String> {
    let trimmed = value.trim().to_string();
    (!trimmed.is_empty()).then_some(trimmed)
}

#[cfg(test)]
mod tests {
    use super::{
        local_callback_from_redirect_uri, parse_oauth_callback_request, pkce_challenge,
        random_urlsafe_token,
    };

    #[test]
    fn pkce_challenge_matches_known_vector() {
        assert_eq!(
            pkce_challenge("dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk"),
            "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"
        );
    }

    #[test]
    fn random_tokens_are_urlsafe_without_padding() {
        let token = random_urlsafe_token(32);
        assert!(!token.contains('='));
        assert!(token
            .chars()
            .all(|character| character.is_ascii_alphanumeric()
                || character == '-'
                || character == '_'));
    }

    #[test]
    fn spotify_loopback_callback_requires_ip_literal() {
        assert!(
            local_callback_from_redirect_uri("http://localhost:9090/callback", "Spotify").is_err()
        );

        let callback =
            local_callback_from_redirect_uri("http://127.0.0.1:9090/callback", "Spotify").unwrap();
        assert_eq!(callback.bind_address, "127.0.0.1:9090");
        assert_eq!(callback.path, "/callback");
    }

    #[test]
    fn spotify_callback_request_extracts_code_and_validates_state() {
        let callback = parse_oauth_callback_request(
            "GET /callback?code=abc123&state=state123 HTTP/1.1\r\nhost: 127.0.0.1\r\n\r\n",
            "/callback",
            "state123",
        )
        .unwrap();

        assert_eq!(callback.code, "abc123");
        assert_eq!(callback.state.as_deref(), Some("state123"));
        assert!(parse_oauth_callback_request(
            "GET /callback?code=abc123&state=bad HTTP/1.1\r\nhost: 127.0.0.1\r\n\r\n",
            "/callback",
            "state123",
        )
        .is_err());
    }
}
