use std::collections::HashMap;
use std::env;
use std::sync::{Mutex, OnceLock};

use keyring_core::{Entry, Error, set_default_store};
use serde::Serialize;

const SERVICE: &str = "cold-brew";
const JELLYFIN_BASE_URL: &str = "jellyfin.base_url";
const JELLYFIN_USER_NAME: &str = "jellyfin.user_name";
const JELLYFIN_PASSWORD: &str = "jellyfin.password";
const PROVIDER_ACCOUNT_IDS: &[&str] =
    &["spotify", "tidal", "qobuz", "youtube", "lastfm", "bandcamp"];
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
    let client_id = crate::secrets::get_credential(provider_id, "client_id");
    let client_secret = crate::secrets::get_credential(provider_id, "client_secret");
    let api_key = crate::secrets::get_credential(provider_id, "api_key");
    let api_secret = crate::secrets::get_credential(provider_id, "api_secret");
    let access_token = crate::secrets::get_credential(provider_id, "access_token");
    let refresh_token = crate::secrets::get_credential(provider_id, "refresh_token");

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
        "spotify" => spotify_login_state(account.as_ref()),
        "tidal" => token_login_state(
            account.as_ref(),
            "TIDAL access token saved; metadata search can run.",
            "Save a TIDAL access token or complete TIDAL OAuth before using TIDAL metadata search.",
            "TIDAL OAuth material is saved; finish login or refresh the token to save an access token.",
        ),
        "qobuz" => qobuz_login_state(account.as_ref()),
        "youtube" => youtube_login_state(account.as_ref()),
        "lastfm" => lastfm_login_state(account.as_ref()),
        "bandcamp" => (
            "link_out_only",
            "No supported Bandcamp login flow is implemented; use link-out or local downloads.",
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

fn spotify_login_state(account: Option<&ProviderAccount>) -> (&'static str, &'static str) {
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

fn qobuz_login_state(account: Option<&ProviderAccount>) -> (&'static str, &'static str) {
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

fn youtube_login_state(account: Option<&ProviderAccount>) -> (&'static str, &'static str) {
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

fn lastfm_login_state(account: Option<&ProviderAccount>) -> (&'static str, &'static str) {
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

fn non_empty(value: String, label: &str) -> Result<String, String> {
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
mod tests {
    use super::{
        ProviderAccount, non_empty_option, normalize_provider_id, provider_entry_name,
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
