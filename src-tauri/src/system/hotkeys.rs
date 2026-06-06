use tauri::AppHandle;

pub fn register_media_hotkeys(app: &AppHandle) {
    #[cfg(any(target_os = "linux", target_os = "windows", target_os = "macos"))]
    {
        use tauri_plugin_global_shortcut::{
            Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutEvent, ShortcutState,
        };

        let gs = app.global_shortcut();

        let register = |shortcut: Shortcut, label: &str,
                        action: fn(&AppHandle)| {
            let app = app.clone();
            let label = label.to_string();
            if let Err(e) = gs.on_shortcut(
                shortcut,
                move |_: &AppHandle, _: &Shortcut, event: ShortcutEvent| {
                    if event.state() == ShortcutState::Pressed {
                        action(&app);
                    }
                },
            ) {
                tracing::warn!("Failed to register {label}: {e}");
            } else {
                tracing::info!("Registered media hotkey: {label}");
            }
        };

        register(
            Shortcut::new(Some(Modifiers::empty()), Code::MediaPlayPause),
            "PlayPause",
            handle_play_pause,
        );
        register(
            Shortcut::new(Some(Modifiers::empty()), Code::MediaTrackNext),
            "NextTrack",
            handle_next_track,
        );
        register(
            Shortcut::new(Some(Modifiers::empty()), Code::MediaTrackPrevious),
            "PreviousTrack",
            handle_prev_track,
        );
        register(
            Shortcut::new(Some(Modifiers::empty()), Code::AudioVolumeUp),
            "VolumeUp",
            handle_vol_up,
        );
        register(
            Shortcut::new(Some(Modifiers::empty()), Code::AudioVolumeDown),
            "VolumeDown",
            handle_vol_down,
        );
    }

    #[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
    {
        tracing::info!("Global media hotkeys not available on this platform");
        let _ = app;
    }
}

fn handle_play_pause(app: &AppHandle) {
    let _app = app.clone();
    tauri::async_runtime::spawn(async move {
        match crate::audio::player::get_playback_status() {
            Ok(status) => {
                let result = if status.playing {
                    crate::audio::player::playback_pause()
                } else {
                    crate::audio::player::playback_resume()
                };
                if let Err(e) = result {
                    tracing::warn!("PlayPause failed: {e}");
                }
            }
            Err(e) => tracing::warn!("PlayPause status error: {e}"),
        }
    });
}

fn handle_next_track(app: &AppHandle) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        match crate::audio::player::next_queue_song() {
            Ok(queue) => {
                crate::system::mpris::play_gapless_from_snapshot(&queue);
            }
            Err(e) => tracing::warn!("NextTrack failed: {e}"),
        }
        let _ = app;
    });
}

fn handle_prev_track(app: &AppHandle) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        match crate::audio::player::previous_queue_song() {
            Ok(queue) => {
                crate::system::mpris::play_gapless_from_snapshot(&queue);
            }
            Err(e) => tracing::warn!("PreviousTrack failed: {e}"),
        }
        let _ = app;
    });
}

fn handle_vol_up(app: &AppHandle) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        match crate::audio::player::get_playback_status() {
            Ok(status) => {
                let new_vol = (status.volume + 0.05).min(1.0);
                if let Err(e) = crate::audio::player::set_playback_volume(new_vol) {
                    tracing::warn!("VolumeUp failed: {e}");
                }
            }
            Err(e) => tracing::warn!("VolumeUp status error: {e}"),
        }
        let _ = app;
    });
}

fn handle_vol_down(app: &AppHandle) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        match crate::audio::player::get_playback_status() {
            Ok(status) => {
                let new_vol = (status.volume - 0.05).max(0.0);
                if let Err(e) = crate::audio::player::set_playback_volume(new_vol) {
                    tracing::warn!("VolumeDown failed: {e}");
                }
            }
            Err(e) => tracing::warn!("VolumeDown status error: {e}"),
        }
        let _ = app;
    });
}
