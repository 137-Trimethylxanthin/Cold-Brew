use std::sync::Mutex;
use tauri::AppHandle;
use tracing::instrument;

static NOTIFICATION_ENABLED: Mutex<bool> = Mutex::new(true);
static LAST_NOTIFIED_TRACK: Mutex<Option<String>> = Mutex::new(None);

pub fn is_enabled() -> bool {
    *NOTIFICATION_ENABLED.lock().unwrap_or_else(|e| e.into_inner())
}

pub fn set_enabled(enabled: bool) {
    if let Ok(mut guard) = NOTIFICATION_ENABLED.lock() {
        *guard = enabled;
    }
}

#[instrument(skip(app))]
pub fn show_now_playing(app: &AppHandle, title: &str, artist: &str, album: &str) {
    if !is_enabled() {
        return;
    }

    let dedup_key = format!("{}|{}", title, artist);
    {
        let mut last = LAST_NOTIFIED_TRACK.lock().unwrap_or_else(|e| e.into_inner());
        if last.as_deref() == Some(&dedup_key) {
            return;
        }
        *last = Some(dedup_key.clone());
    }

    let body = if artist.is_empty() {
        album.to_string()
    } else if album.is_empty() {
        format!("by {}", artist)
    } else {
        format!("by {} — {}", artist, album)
    };

    #[cfg(any(target_os = "linux", target_os = "windows", target_os = "macos"))]
    {
        use tauri_plugin_notification::NotificationExt;

        let title = title.to_string();
        let body = body.to_string();
        let app = app.clone();

        tauri::async_runtime::spawn(async move {
            if let Err(e) = app
                .notification()
                .builder()
                .title(&title)
                .body(&body)
                .show()
            {
                tracing::warn!("Failed to show notification: {e}");
            }
        });
    }

    #[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
    {
        tracing::info!("Now playing: {title} - {body}");
        let _ = app;
        let _ = body;
    }
}

#[tauri::command(rename_all = "snake_case")]
pub fn get_notification_setting() -> bool {
    is_enabled()
}

#[tauri::command(rename_all = "snake_case")]
pub fn set_notification_setting(enabled: bool) {
    set_enabled(enabled);
}
