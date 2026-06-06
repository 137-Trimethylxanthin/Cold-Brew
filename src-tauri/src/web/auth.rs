use std::process::Command;
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use rand::RngCore;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::time::timeout;


const SPOTIFY_AUTHORIZE_ENDPOINT: &str = "https://accounts.spotify.com/authorize";
const SPOTIFY_TOKEN_ENDPOINT: &str = "https://accounts.spotify.com/api/token";
const SPOTIFY_DEFAULT_SCOPE: &str = "playlist-read-private playlist-read-collaborative user-read-private user-read-email user-read-playback-state user-modify-playback-state streaming";
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
) -> Result<ProviderAccount, String> {
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

    save_provider_account(
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

pub async fn refresh_spotify_access_token() -> Result<ProviderAccount, String> {
    let secrets = load_provider_secrets("spotify")?
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

    save_provider_account(
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
) -> Result<ProviderAccount, String> {
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
    load_provider_secrets("spotify")?
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
) -> Result<ProviderAccount, String> {
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

    save_provider_account(
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

pub async fn refresh_tidal_access_token() -> Result<ProviderAccount, String> {
    let secrets = load_provider_secrets("tidal")?
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

    save_provider_account(
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
) -> Result<ProviderAccount, String> {
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

    let secrets = load_provider_secrets("youtube")?.ok_or_else(|| {
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

    save_provider_account(
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

pub async fn refresh_youtube_access_token() -> Result<ProviderAccount, String> {
    let secrets = load_provider_secrets("youtube")?
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

    save_provider_account(
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
        crate::providers::lastfm::lastfm_signature(&params, &api_secret),
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
        record_provider_auth_failure("lastfm", message.clone());
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

pub async fn finish_lastfm_login() -> Result<ProviderAccount, String> {
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
        crate::providers::lastfm::lastfm_signature(&params, &api_secret),
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
        record_provider_auth_failure("lastfm", message.clone());
        return Err(message);
    }
    let session_response = parse_lastfm_response::<LastFmSessionResponse>(&body, "session")?;
    *lastfm_login_state()
        .lock()
        .map_err(|_| "Last.fm login state is unavailable.".to_string())? = None;

    save_provider_account(
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
        record_provider_auth_failure(provider_id, message.clone());
        return Err(message);
    }
    let token = serde_json::from_str::<OAuthTokenResponse>(&body)
        .map_err(|error| format!("Could not parse {label} response: {error}; {body}"))?;
    clear_provider_auth_failure(provider_id);
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
        record_provider_auth_failure("lastfm", error.clone());
        return Err(error);
    }
    clear_provider_auth_failure("lastfm");
    serde_json::from_value::<T>(value)
        .map_err(|error| format!("Could not decode Last.fm {label} response: {error}"))
}

fn required_provider_secret<F>(provider_id: &str, select: F) -> Result<Option<String>, String>
where
    F: FnOnce(ProviderSecrets) -> Option<String>,
{
    Ok(load_provider_secrets(provider_id)?
        .and_then(select)
        .and_then(non_empty_owned))
}

fn lastfm_api_key_and_secret() -> Result<(String, String), String> {
    let secrets = load_provider_secrets("lastfm")?
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
        assert!(
            token
                .chars()
                .all(|character| character.is_ascii_alphanumeric()
                    || character == '-'
                    || character == '_')
        );
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
        assert!(
            parse_oauth_callback_request(
                "GET /callback?code=abc123&state=bad HTTP/1.1\r\nhost: 127.0.0.1\r\n\r\n",
                "/callback",
                "state123",
            )
            .is_err()
        );
    }
}

use std::collections::HashMap;
use std::env;

use keyring_core::{Entry, Error, set_default_store};

const SERVICE: &str = "cold-brew";
const JELLYFIN_BASE_URL: &str = "jellyfin.base_url";
const JELLYFIN_USER_NAME: &str = "jellyfin.user_name";
const JELLYFIN_PASSWORD: &str = "jellyfin.password";
const PROVIDER_ACCOUNT_IDS: &[&str] =
    &["spotify", "tidal", "qobuz", "youtube", "lastfm", "bandcamp", "soundcloud"];
const PROVIDER_ACCOUNT_FIELDS: &[&str] = &[
    "display_name",
    "client_id",
    "client_secret",
    "api_key",
    "api_secret",
    "access_token",
    "refresh_token",
];

static KEYRING_INIT: OnceLock<Result<(), String>> = OnceLock::new();
static PROVIDER_AUTH_FAILURES: OnceLock<Mutex<HashMap<String, String>>> = OnceLock::new();

#[derive(Clone)]
pub struct JellyfinCredentials {
    pub base_url: String,
    pub user_name: String,
    pub password: String,
}

#[derive(Clone, Serialize)]
pub struct JellyfinAccount {
    pub base_url: String,
    pub user_name: String,
    pub has_password: bool,
    pub source: String,
}

#[derive(Clone, Serialize)]
pub struct ProviderAccount {
    pub provider_id: String,
    pub display_name: Option<String>,
    pub has_client_id: bool,
    pub has_client_secret: bool,
    pub has_api_key: bool,
    pub has_api_secret: bool,
    pub has_access_token: bool,
    pub has_refresh_token: bool,
    pub source: String,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct ProviderLoginState {
    pub provider_id: String,
    pub status: String,
    pub message: String,
    pub last_error: Option<String>,
}

#[derive(Clone)]
pub struct ProviderSecrets {
    pub client_id: Option<String>,
    pub client_secret: Option<String>,
    pub api_key: Option<String>,
    pub api_secret: Option<String>,
    pub access_token: Option<String>,
    pub refresh_token: Option<String>,
}

impl JellyfinCredentials {
    pub fn from_env() -> Result<Self, String> {
        Ok(Self {
            base_url: required_env("COLD_BREW_JELLYFIN_URL")?,
            user_name: required_env("COLD_BREW_JELLYFIN_USER")?,
            password: required_env("COLD_BREW_JELLYFIN_PASSWORD")?,
        })
    }
}

pub fn load_jellyfin_credentials() -> Result<JellyfinCredentials, String> {
    if let Some(saved) = load_jellyfin_credentials_from_keyring()? {
        return Ok(saved);
    }

    JellyfinCredentials::from_env()
}

pub fn get_jellyfin_account() -> Result<Option<JellyfinAccount>, String> {
    if let Some(saved) = load_jellyfin_credentials_from_keyring()? {
        return Ok(Some(JellyfinAccount {
            base_url: saved.base_url,
            user_name: saved.user_name,
            has_password: !saved.password.is_empty(),
            source: "keyring".to_string(),
        }));
    }

    match JellyfinCredentials::from_env() {
        Ok(env_account) => Ok(Some(JellyfinAccount {
            base_url: env_account.base_url,
            user_name: env_account.user_name,
            has_password: !env_account.password.is_empty(),
            source: "environment".to_string(),
        })),
        Err(_) => Ok(None),
    }
}

pub fn save_jellyfin_account(
    base_url: String,
    user_name: String,
    password: String,
) -> Result<JellyfinAccount, String> {
    let credentials = JellyfinCredentials {
        base_url: normalize_base_url(&base_url)?,
        user_name: non_empty(user_name, "Jellyfin username")?,
        password: non_empty(password, "Jellyfin password")?,
    };

    write_entry(JELLYFIN_BASE_URL, &credentials.base_url)?;
    write_entry(JELLYFIN_USER_NAME, &credentials.user_name)?;
    write_entry(JELLYFIN_PASSWORD, &credentials.password)?;

    Ok(JellyfinAccount {
        base_url: credentials.base_url,
        user_name: credentials.user_name,
        has_password: true,
        source: "keyring".to_string(),
    })
}

pub fn clear_jellyfin_account() -> Result<(), String> {
    delete_entry(JELLYFIN_BASE_URL)?;
    delete_entry(JELLYFIN_USER_NAME)?;
    delete_entry(JELLYFIN_PASSWORD)?;
    Ok(())
}

pub fn list_provider_accounts() -> Result<Vec<ProviderAccount>, String> {
    let mut accounts = Vec::new();
    for provider_id in PROVIDER_ACCOUNT_IDS {
        if let Some(account) = get_provider_account((*provider_id).to_string())? {
            accounts.push(account);
        }
    }
    Ok(accounts)
}

pub fn list_provider_login_states() -> Result<Vec<ProviderLoginState>, String> {
    let mut states = Vec::new();
    for provider_id in PROVIDER_ACCOUNT_IDS {
        let account = get_provider_account((*provider_id).to_string())?;
        let last_error = provider_auth_failure(provider_id);
        states.push(provider_login_state_from_account(
            provider_id,
            account,
            last_error,
        ));
    }
    Ok(states)
}

pub fn get_provider_account(provider_id: String) -> Result<Option<ProviderAccount>, String> {
    let provider_id = normalize_provider_id(&provider_id)?;
    let display_name = read_provider_entry(&provider_id, "display_name")?;
    let has_client_id = read_provider_entry(&provider_id, "client_id")?.is_some();
    let has_client_secret = read_provider_entry(&provider_id, "client_secret")?.is_some();
    let has_api_key = read_provider_entry(&provider_id, "api_key")?.is_some();
    let has_api_secret = read_provider_entry(&provider_id, "api_secret")?.is_some();
    let has_access_token = read_provider_entry(&provider_id, "access_token")?.is_some();
    let has_refresh_token = read_provider_entry(&provider_id, "refresh_token")?.is_some();

    if display_name.is_none()
        && !has_client_id
        && !has_client_secret
        && !has_api_key
        && !has_api_secret
        && !has_access_token
        && !has_refresh_token
    {
        return Ok(None);
    }

    Ok(Some(ProviderAccount {
        provider_id,
        display_name,
        has_client_id,
        has_client_secret,
        has_api_key,
        has_api_secret,
        has_access_token,
        has_refresh_token,
        source: "keyring".to_string(),
    }))
}

#[allow(clippy::too_many_arguments)]
pub fn save_provider_account(
    provider_id: String,
    display_name: Option<String>,
    client_id: Option<String>,
    client_secret: Option<String>,
    api_key: Option<String>,
    api_secret: Option<String>,
    access_token: Option<String>,
    refresh_token: Option<String>,
) -> Result<ProviderAccount, String> {
    let provider_id = normalize_provider_id(&provider_id)?;
    let mut wrote_value = false;

    for (field, value) in [
        ("display_name", display_name),
        ("client_id", client_id),
        ("client_secret", client_secret),
        ("api_key", api_key),
        ("api_secret", api_secret),
        ("access_token", access_token),
        ("refresh_token", refresh_token),
    ] {
        if let Some(value) = non_empty_option(value) {
            write_provider_entry(&provider_id, field, &value)?;
            wrote_value = true;
        }
    }

    let account = get_provider_account(provider_id.clone())?;
    match (account, wrote_value) {
        (Some(account), _) => {
            clear_provider_auth_failure(&provider_id);
            Ok(account)
        }
        (None, false) => Err("Enter at least one provider credential value to save.".to_string()),
        (None, true) => Err("Saved provider account could not be read back.".to_string()),
    }
}

pub fn clear_provider_account(provider_id: String) -> Result<(), String> {
    let provider_id = normalize_provider_id(&provider_id)?;
    for field in PROVIDER_ACCOUNT_FIELDS {
        delete_provider_entry(&provider_id, field)?;
    }
    clear_provider_auth_failure(&provider_id);
    Ok(())
}

pub fn record_provider_auth_failure(provider_id: &str, message: String) {
    let Ok(provider_id) = normalize_provider_id(provider_id) else {
        return;
    };
    let Ok(mut failures) = provider_auth_failures().lock() else {
        return;
    };
    failures.insert(provider_id, message);
}

pub fn clear_provider_auth_failure(provider_id: &str) {
    let Ok(provider_id) = normalize_provider_id(provider_id) else {
        return;
    };
    let Ok(mut failures) = provider_auth_failures().lock() else {
        return;
    };
    failures.remove(&provider_id);
}

pub fn load_provider_secrets(provider_id: &str) -> Result<Option<ProviderSecrets>, String> {
    let provider_id = normalize_provider_id(provider_id)?;

    if let Some(secrets) = load_provider_secrets_from_secrets_module(&provider_id) {
        if secrets.client_id.is_some()
            || secrets.client_secret.is_some()
            || secrets.api_key.is_some()
        {
            return Ok(Some(secrets));
        }
    }

    let client_id = read_provider_entry(&provider_id, "client_id")?;
    let client_secret = read_provider_entry(&provider_id, "client_secret")?;
    let api_key = read_provider_entry(&provider_id, "api_key")?;
    let api_secret = read_provider_entry(&provider_id, "api_secret")?;
    let access_token = read_provider_entry(&provider_id, "access_token")?;
    let refresh_token = read_provider_entry(&provider_id, "refresh_token")?;

    if client_id.is_none()
        && client_secret.is_none()
        && api_key.is_none()
        && api_secret.is_none()
        && access_token.is_none()
        && refresh_token.is_none()
    {
        return Ok(None);
    }

    Ok(Some(ProviderSecrets {
        client_id,
        client_secret,
        api_key,
        api_secret,
        access_token,
        refresh_token,
    }))
}

fn load_provider_secrets_from_secrets_module(provider_id: &str) -> Option<ProviderSecrets> {
    let client_id = crate::storage::keyring::get_credential(provider_id, "client_id");
    let client_secret = crate::storage::keyring::get_credential(provider_id, "client_secret");
    let api_key = crate::storage::keyring::get_credential(provider_id, "api_key");
    let api_secret = crate::storage::keyring::get_credential(provider_id, "api_secret");
    let access_token = crate::storage::keyring::get_credential(provider_id, "access_token");
    let refresh_token = crate::storage::keyring::get_credential(provider_id, "refresh_token");

    if client_id.is_none()
        && client_secret.is_none()
        && api_key.is_none()
        && api_secret.is_none()
        && access_token.is_none()
        && refresh_token.is_none()
    {
        return None;
    }

    Some(ProviderSecrets {
        client_id,
        client_secret,
        api_key,
        api_secret,
        access_token,
        refresh_token,
    })
}

fn load_jellyfin_credentials_from_keyring() -> Result<Option<JellyfinCredentials>, String> {
    let base_url = read_entry(JELLYFIN_BASE_URL)?;
    let user_name = read_entry(JELLYFIN_USER_NAME)?;
    let password = read_entry(JELLYFIN_PASSWORD)?;

    match (base_url, user_name, password) {
        (None, None, None) => Ok(None),
        (Some(base_url), Some(user_name), Some(password)) => Ok(Some(JellyfinCredentials {
            base_url,
            user_name,
            password,
        })),
        _ => Err("Saved Jellyfin account is incomplete. Clear and save it again.".to_string()),
    }
}

fn provider_login_state_from_account(
    provider_id: &str,
    account: Option<ProviderAccount>,
    last_error: Option<String>,
) -> ProviderLoginState {
    if let Some(last_error) = last_error {
        return ProviderLoginState {
            provider_id: provider_id.to_string(),
            status: "failed".to_string(),
            message: "The last provider request failed with the saved credentials.".to_string(),
            last_error: Some(last_error),
        };
    }

    let state = match provider_id {
        "spotify" => credentials_spotify_login_state(account.as_ref()),
        "tidal" => token_login_state(
            account.as_ref(),
            "TIDAL access token saved; metadata search can run.",
            "Save a TIDAL access token or complete TIDAL OAuth before using TIDAL metadata search.",
            "TIDAL OAuth material is saved; finish login or refresh the token to save an access token.",
        ),
        "qobuz" => credentials_qobuz_login_state(account.as_ref()),
        "youtube" => credentials_credentials_youtube_login_state(account.as_ref()),
        "lastfm" => credentials_credentials_lastfm_login_state(account.as_ref()),
        "bandcamp" => (
            "link_out_only",
            "No supported Bandcamp login flow is implemented; use link-out or local downloads.",
        ),
        "soundcloud" => api_key_login_state(
            account.as_ref(),
            "SoundCloud API key saved; metadata search and previews can run.",
            "Save a SoundCloud API key or use the built-in default client ID before using SoundCloud search.",
        ),
        _ => (
            "missing",
            "No credential status is available for this provider.",
        ),
    };

    ProviderLoginState {
        provider_id: provider_id.to_string(),
        status: state.0.to_string(),
        message: state.1.to_string(),
        last_error: None,
    }
}

fn credentials_spotify_login_state(account: Option<&ProviderAccount>) -> (&'static str, &'static str) {
    let Some(account) = account else {
        return (
            "missing",
            "Save a Spotify Client ID and complete Spotify OAuth before using Spotify search or playlist loading.",
        );
    };
    if account.has_access_token {
        (
            "ready",
            "Spotify access token saved; search, playlist loading, and Web Playback can run for Premium accounts.",
        )
    } else if account.has_client_id || account.has_client_secret || account.has_refresh_token {
        (
            "partial",
            "Spotify OAuth material is saved; finish login or refresh the token to save an access token.",
        )
    } else {
        (
            "missing",
            "Save a Spotify Client ID and complete Spotify OAuth before using Spotify search or playlist loading.",
        )
    }
}

fn token_login_state(
    account: Option<&ProviderAccount>,
    ready_message: &'static str,
    missing_message: &'static str,
    partial_message: &'static str,
) -> (&'static str, &'static str) {
    let Some(account) = account else {
        return ("missing", missing_message);
    };
    if account.has_access_token {
        ("ready", ready_message)
    } else if account.has_client_id
        || account.has_client_secret
        || account.has_api_key
        || account.has_api_secret
        || account.has_refresh_token
    {
        ("partial", partial_message)
    } else {
        ("missing", missing_message)
    }
}

fn credentials_qobuz_login_state(account: Option<&ProviderAccount>) -> (&'static str, &'static str) {
    let Some(account) = account else {
        return (
            "missing",
            "Save a Qobuz app id in the Client ID/App ID or API key field before using Qobuz search.",
        );
    };
    if account.has_client_id || account.has_api_key {
        (
            "ready",
            "Qobuz app id saved; metadata search can run with the current credentials.",
        )
    } else {
        (
            "partial",
            "Qobuz credentials are incomplete; an app id is required.",
        )
    }
}

fn credentials_credentials_youtube_login_state(account: Option<&ProviderAccount>) -> (&'static str, &'static str) {
    let Some(account) = account else {
        return (
            "missing",
            "Save a YouTube Data API key or complete YouTube OAuth before using YouTube search or playlist loading.",
        );
    };
    if account.has_api_key || account.has_access_token {
        (
            "ready",
            "YouTube Data API key or OAuth access token saved; search and playlist loading can run.",
        )
    } else if account.has_client_id || account.has_client_secret || account.has_refresh_token {
        (
            "partial",
            "YouTube OAuth material is saved; finish login or refresh the token to save an access token.",
        )
    } else {
        (
            "partial",
            "YouTube credentials are incomplete; a Data API key or OAuth access token is required.",
        )
    }
}

fn api_key_login_state(
    account: Option<&ProviderAccount>,
    ready_message: &'static str,
    missing_message: &'static str,
) -> (&'static str, &'static str) {
    let Some(account) = account else {
        return ("missing", missing_message);
    };
    if account.has_api_key {
        ("ready", ready_message)
    } else {
        ("partial", missing_message)
    }
}

fn credentials_credentials_lastfm_login_state(account: Option<&ProviderAccount>) -> (&'static str, &'static str) {
    let Some(account) = account else {
        return (
            "missing",
            "Save a Last.fm API key before using Last.fm search or scrobbling.",
        );
    };
    if account.has_api_key && account.has_api_secret && account.has_access_token {
        (
            "ready",
            "Last.fm API key, API secret, and session key saved; scrobbling can run.",
        )
    } else if account.has_api_key {
        (
            "partial",
            "Last.fm search can run; scrobbling still needs an API secret and session key.",
        )
    } else {
        (
            "missing",
            "Save a Last.fm API key before using Last.fm search or scrobbling.",
        )
    }
}

fn read_provider_entry(provider_id: &str, field: &str) -> Result<Option<String>, String> {
    read_entry(&provider_entry_name(provider_id, field))
}

fn write_provider_entry(provider_id: &str, field: &str, value: &str) -> Result<(), String> {
    write_entry(&provider_entry_name(provider_id, field), value)
}

fn delete_provider_entry(provider_id: &str, field: &str) -> Result<(), String> {
    delete_entry(&provider_entry_name(provider_id, field))
}

fn provider_entry_name(provider_id: &str, field: &str) -> String {
    format!("provider.{provider_id}.{field}")
}

fn read_entry(name: &str) -> Result<Option<String>, String> {
    init_keyring()?;
    let entry = Entry::new(SERVICE, name).map_err(keyring_error)?;
    match entry.get_password() {
        Ok(value) => Ok(Some(value)),
        Err(Error::NoEntry) => Ok(None),
        Err(error) => Err(keyring_error(error)),
    }
}

fn write_entry(name: &str, value: &str) -> Result<(), String> {
    init_keyring()?;
    let entry = Entry::new(SERVICE, name).map_err(keyring_error)?;
    entry.set_password(value).map_err(keyring_error)
}

fn delete_entry(name: &str) -> Result<(), String> {
    init_keyring()?;
    let entry = Entry::new(SERVICE, name).map_err(keyring_error)?;
    match entry.delete_credential() {
        Ok(()) | Err(Error::NoEntry) => Ok(()),
        Err(error) => Err(keyring_error(error)),
    }
}

fn init_keyring() -> Result<(), String> {
    KEYRING_INIT.get_or_init(install_native_store).clone()
}

fn provider_auth_failures() -> &'static Mutex<HashMap<String, String>> {
    PROVIDER_AUTH_FAILURES.get_or_init(|| Mutex::new(HashMap::new()))
}

fn provider_auth_failure(provider_id: &str) -> Option<String> {
    provider_auth_failures()
        .lock()
        .ok()
        .and_then(|failures| failures.get(provider_id).cloned())
}

#[cfg(target_os = "linux")]
fn install_native_store() -> Result<(), String> {
    let config = HashMap::new();
    match dbus_secret_service_keyring_store::Store::new_with_configuration(&config) {
        Ok(store) => {
            set_default_store(store);
            Ok(())
        }
        Err(secret_service_error) => {
            match linux_keyutils_keyring_store::Store::new_with_configuration(&config) {
                Ok(store) => {
                    set_default_store(store);
                    Ok(())
                }
                Err(keyutils_error) => Err(format!(
                    "Could not initialize Linux credential storage. Secret Service: {secret_service_error}; keyutils: {keyutils_error}"
                )),
            }
        }
    }
}

#[cfg(target_os = "macos")]
fn install_native_store() -> Result<(), String> {
    let config = HashMap::new();
    let store = apple_native_keyring_store::keychain::Store::new_with_configuration(&config)
        .map_err(keyring_error)?;
    set_default_store(store);
    Ok(())
}

#[cfg(target_os = "windows")]
fn install_native_store() -> Result<(), String> {
    let config = HashMap::new();
    let store = windows_native_keyring_store::Store::new_with_configuration(&config)
        .map_err(keyring_error)?;
    set_default_store(store);
    Ok(())
}

#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
fn install_native_store() -> Result<(), String> {
    Err("Cold-Brew does not have a native credential store for this OS yet.".to_string())
}

fn keyring_error(error: Error) -> String {
    format!("Credential storage error: {error}")
}

fn required_env(name: &str) -> Result<String, String> {
    env::var(name)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| format!("Set {name} to enable Jellyfin library loading."))
}

fn normalize_base_url(base_url: &str) -> Result<String, String> {
    let value = non_empty(base_url.to_string(), "Jellyfin URL")?
        .trim_end_matches('/')
        .to_string();

    if !value.starts_with("http://") && !value.starts_with("https://") {
        return Err("Jellyfin URL must start with http:// or https://.".to_string());
    }

    Ok(value)
}

#[allow(dead_code)]
fn credentials_non_empty(value: String, label: &str) -> Result<String, String> {
    let trimmed = value.trim().to_string();
    if trimmed.is_empty() {
        Err(format!("{label} must not be empty."))
    } else {
        Ok(trimmed)
    }
}

fn non_empty_option(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn normalize_provider_id(provider_id: &str) -> Result<String, String> {
    let normalized = provider_id.trim().to_ascii_lowercase();
    if PROVIDER_ACCOUNT_IDS.contains(&normalized.as_str()) {
        Ok(normalized)
    } else {
        Err(format!("Unsupported provider account id: {provider_id}"))
    }
}

#[cfg(test)]
mod credentials_tests {
    use super::{ProviderAccount, non_empty_option, normalize_provider_id, provider_entry_name,
        provider_login_state_from_account,
    };

    #[test]
    fn provider_entry_names_are_namespaced() {
        assert_eq!(
            provider_entry_name("spotify", "client_id"),
            "provider.spotify.client_id"
        );
    }

    #[test]
    fn provider_ids_are_normalized_and_validated() {
        assert_eq!(normalize_provider_id(" Spotify ").unwrap(), "spotify");
        assert!(normalize_provider_id("unsupported").is_err());
    }

    #[test]
    fn optional_provider_values_are_trimmed() {
        assert_eq!(
            non_empty_option(Some("  value  ".to_string())).as_deref(),
            Some("value")
        );
        assert_eq!(non_empty_option(Some("   ".to_string())), None);
        assert_eq!(non_empty_option(None), None);
    }

    #[test]
    fn spotify_login_state_reports_partial_credentials() {
        let state = provider_login_state_from_account(
            "spotify",
            Some(ProviderAccount {
                provider_id: "spotify".to_string(),
                display_name: None,
                has_client_id: true,
                has_client_secret: false,
                has_api_key: false,
                has_api_secret: false,
                has_access_token: false,
                has_refresh_token: false,
                source: "keyring".to_string(),
            }),
            None,
        );

        assert_eq!(state.status, "partial");
        assert!(state.message.contains("access token"));
    }

    #[test]
    fn login_state_prefers_recorded_failure() {
        let state = provider_login_state_from_account(
            "youtube",
            None,
            Some("YouTube search credentials are invalid or expired.".to_string()),
        );

        assert_eq!(state.status, "failed");
        assert_eq!(
            state.last_error.as_deref(),
            Some("YouTube search credentials are invalid or expired.")
        );
    }
}
