use std::env;
use std::sync::OnceLock;

use keyring_core::{set_default_store, Entry, Error};

const SERVICE: &str = "cold-brew-secrets";
const SPOTIFY_CLIENT_ID_ENV: &str = "SPOTIFY_CLIENT_ID";
const SPOTIFY_CLIENT_SECRET_ENV: &str = "SPOTIFY_CLIENT_SECRET";
const SPOTIFY_REDIRECT_URI_ENV: &str = "SPOTIFY_REDIRECT_URI";

static KEYRING_INIT: OnceLock<Result<(), String>> = OnceLock::new();

pub fn init_default_credentials() {
    let _ = ensure_spotify_defaults();
}

fn ensure_spotify_defaults() -> Result<(), String> {
    if has_credentials("spotify") {
        return Ok(());
    }

    let client_id = env_var(SPOTIFY_CLIENT_ID_ENV);
    let client_secret = env_var(SPOTIFY_CLIENT_SECRET_ENV);
    let redirect_uri = env_var(SPOTIFY_REDIRECT_URI_ENV);

    if client_id.is_some() || client_secret.is_some() || redirect_uri.is_some() {
        if let Some(v) = client_id {
            let _ = set_credential("spotify", "client_id", &v);
        }
        if let Some(v) = client_secret {
            let _ = set_credential("spotify", "client_secret", &v);
        }
        if let Some(v) = redirect_uri {
            let _ = set_credential("spotify", "redirect_uri", &v);
        }
    }

    Ok(())
}

pub fn get_credential(provider: &str, key: &str) -> Option<String> {
    let entry_name = format!("secrets.{provider}.{key}");
    read_entry(&entry_name).ok().flatten()
}

pub fn set_credential(provider: &str, key: &str, value: &str) -> Result<(), String> {
    let entry_name = format!("secrets.{provider}.{key}");
    write_entry(&entry_name, value)
}

pub fn delete_credential(provider: &str, key: &str) -> Result<(), String> {
    let entry_name = format!("secrets.{provider}.{key}");
    delete_entry(&entry_name)
}

pub fn has_credentials(provider: &str) -> bool {
    get_credential(provider, "client_id").is_some()
        || get_credential(provider, "client_secret").is_some()
        || get_credential(provider, "api_key").is_some()
}

pub fn is_default_credential(provider: &str) -> bool {
    read_entry(&format!("secrets.{provider}.is_custom"))
        .ok()
        .flatten()
        .is_none_or(|v| v != "true")
}

pub fn mark_custom(provider: &str) -> Result<(), String> {
    write_entry(&format!("secrets.{provider}.is_custom"), "true")
}

pub fn mark_default(provider: &str) -> Result<(), String> {
    delete_entry(&format!("secrets.{provider}.is_custom"))
}

pub fn reset_to_default(provider: &str) -> Result<(), String> {
    let keys = match provider {
        "spotify" => vec!["client_id", "client_secret", "redirect_uri"],
        "tidal" => vec!["client_id", "client_secret"],
        "qobuz" => vec!["app_id", "app_secret"],
        "youtube" => vec!["api_key"],
        "lastfm" => vec!["api_key", "api_secret"],
        "bandcamp" => vec![],
        "soundcloud" => vec!["api_key"],
        _ => return Err(format!("Unknown provider: {provider}")),
    };

    for key in keys {
        let _ = delete_credential(provider, key);
    }
    mark_default(provider)?;

    ensure_spotify_defaults()?;

    Ok(())
}

fn env_var(name: &str) -> Option<String> {
    env::var(name)
        .ok()
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
}

fn read_entry(name: &str) -> Result<Option<String>, String> {
    init_keyring()?;
    let entry = Entry::new(SERVICE, name).map_err(|e| format!("Keyring error: {e}"))?;
    match entry.get_password() {
        Ok(value) => Ok(Some(value)),
        Err(Error::NoEntry) => Ok(None),
        Err(error) => Err(format!("Keyring error: {error}")),
    }
}

fn write_entry(name: &str, value: &str) -> Result<(), String> {
    init_keyring()?;
    let entry = Entry::new(SERVICE, name).map_err(|e| format!("Keyring error: {e}"))?;
    entry
        .set_password(value)
        .map_err(|e| format!("Keyring error: {e}"))
}

fn delete_entry(name: &str) -> Result<(), String> {
    init_keyring()?;
    let entry = Entry::new(SERVICE, name).map_err(|e| format!("Keyring error: {e}"))?;
    match entry.delete_credential() {
        Ok(()) | Err(Error::NoEntry) => Ok(()),
        Err(error) => Err(format!("Keyring error: {error}")),
    }
}

fn init_keyring() -> Result<(), String> {
    KEYRING_INIT.get_or_init(install_native_store).clone()
}

#[cfg(target_os = "linux")]
fn install_native_store() -> Result<(), String> {
    let config = std::collections::HashMap::new();
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
    let config = std::collections::HashMap::new();
    let store = apple_native_keyring_store::keychain::Store::new_with_configuration(&config)
        .map_err(|e| format!("Keyring error: {e}"))?;
    set_default_store(store);
    Ok(())
}

#[cfg(target_os = "windows")]
fn install_native_store() -> Result<(), String> {
    let config = std::collections::HashMap::new();
    let store = windows_native_keyring_store::Store::new_with_configuration(&config)
        .map_err(|e| format!("Keyring error: {e}"))?;
    set_default_store(store);
    Ok(())
}

#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
fn install_native_store() -> Result<(), String> {
    Err("Cold-Brew does not have a native credential store for this OS yet.".to_string())
}
