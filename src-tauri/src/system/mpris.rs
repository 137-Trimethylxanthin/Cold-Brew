use crate::audio::player::PlaybackStatus;

#[cfg(feature = "mpris")]
mod linux {
    use std::sync::{Mutex, OnceLock};
    use mpris_server::{
        LoopStatus, Metadata, PlaybackRate, PlaybackStatus as MprisPlaybackStatus,
        PlayerInterface, Property, RootInterface, Server, Time, TrackId, Volume,
    };

    static MPRIS_SERVER: OnceLock<Mutex<Option<Server<ColdBrewPlayer>>>> = OnceLock::new();

    pub fn get_server() -> &'static OnceLock<Mutex<Option<Server<ColdBrewPlayer>>>> {
        &MPRIS_SERVER
    }

    struct ColdBrewPlayer;

    impl RootInterface for ColdBrewPlayer {
        async fn raise(&self) {}
        async fn quit(&self) {}
        async fn can_quit(&self) -> bool { true }
        async fn fullscreen(&self) -> bool { false }
        async fn set_fullscreen(&self, _fullscreen: bool) {}
        async fn can_set_fullscreen(&self) -> bool { false }
        async fn can_raise(&self) -> bool { false }
        async fn has_track_list(&self) -> bool { false }
        async fn identity(&self) -> String { "Cold-Brew".to_string() }
        async fn desktop_entry(&self) -> String { "cold-brew".to_string() }
        async fn supported_uri_schemes(&self) -> Vec<String> { vec!["file".to_string()] }
        async fn supported_mime_types(&self) -> Vec<String> {
            vec!["audio/mpeg".into(), "audio/flac".into(), "audio/wav".into(), "audio/ogg".into()]
        }
    }

    impl PlayerInterface for ColdBrewPlayer {
        async fn next(&self) {
            if let Ok(queue) = crate::audio::player::next_queue_song() {
                super::play_gapless_from_snapshot(&queue);
            }
        }
        async fn previous(&self) {
            if let Ok(queue) = crate::audio::player::previous_queue_song() {
                super::play_gapless_from_snapshot(&queue);
            }
        }
        async fn pause(&self) { let _ = crate::audio::player::playback_pause(); }
        async fn play_pause(&self) {
            if let Ok(status) = crate::audio::player::get_playback_status() {
                if status.playing {
                    let _ = crate::audio::player::playback_pause();
                } else {
                    let _ = crate::audio::player::playback_resume();
                }
            }
        }
        async fn stop(&self) { let _ = crate::audio::player::playback_stop(); }
        async fn play(&self) { let _ = crate::audio::player::playback_resume(); }
        async fn seek(&self, offset: Time) {
            if let Ok(status) = crate::audio::player::get_playback_status() {
                let offset_ms = offset.as_millis();
                let target_ms = ((status.position_ms as i64) + offset_ms).max(0) as u64;
                if let Ok(new_status) = crate::audio::player::playback_seek(target_ms) {
                    super::update_metadata(&new_status);
                }
            }
        }
        async fn set_position(&self, _track_id: TrackId, position: Time) {
            let pos_ms = position.as_millis().max(0) as u64;
            if let Ok(new_status) = crate::audio::player::playback_seek(pos_ms) {
                super::update_metadata(&new_status);
            }
        }
        async fn open_uri(&self, _uri: String) {}
        async fn playback_status(&self) -> MprisPlaybackStatus {
            match crate::audio::player::get_playback_status() {
                Ok(s) if s.state == "playing" => MprisPlaybackStatus::Playing,
                Ok(s) if s.state == "paused" => MprisPlaybackStatus::Paused,
                _ => MprisPlaybackStatus::Stopped,
            }
        }
        async fn loop_status(&self) -> LoopStatus { LoopStatus::None }
        async fn set_loop_status(&self, _loop_status: LoopStatus) {}
        async fn rate(&self) -> PlaybackRate {
            crate::audio::player::get_playback_settings()
                .map(|s| PlaybackRate::from(s.playback_speed as f64))
                .unwrap_or(PlaybackRate::from(1.0))
        }
        async fn set_rate(&self, rate: PlaybackRate) {
            let _ = crate::audio::player::set_playback_speed(rate as f32);
        }
        async fn shuffle(&self) -> bool { false }
        async fn set_shuffle(&self, _shuffle: bool) {}
        async fn metadata(&self) -> Metadata {
            crate::audio::player::get_playback_status()
                .map(|s| super::build_metadata(&s))
                .unwrap_or_default()
        }
        async fn volume(&self) -> Volume {
            crate::audio::player::get_playback_status()
                .map(|s| s.volume as f64)
                .unwrap_or(1.0)
        }
        async fn set_volume(&self, volume: Volume) {
            let _ = crate::audio::player::set_playback_volume(volume as f32);
        }
        async fn position(&self) -> Time {
            crate::audio::player::get_playback_status()
                .map(|s| Time::from_millis(s.position_ms as i64))
                .unwrap_or(Time::from_millis(0))
        }
        async fn minimum_rate(&self) -> PlaybackRate { PlaybackRate::from(0.5) }
        async fn maximum_rate(&self) -> PlaybackRate { PlaybackRate::from(2.0) }
        async fn can_go_next(&self) -> bool { true }
        async fn can_go_previous(&self) -> bool { true }
        async fn can_play(&self) -> bool { true }
        async fn can_pause(&self) -> bool { true }
        async fn can_seek(&self) -> bool { true }
        async fn can_control(&self) -> bool { true }
    }

    pub fn start_mpris_server() {
        tauri::async_runtime::spawn(async {
            match Server::new("Cold-Brew", ColdBrewPlayer).await {
                Ok(server) => {
                    tracing::info!("MPRIS server started successfully");
                    MPRIS_SERVER.get_or_init(|| Mutex::new(None))
                        .lock().ok()
                        .map(|mut g| *g = Some(server));
                }
                Err(e) => {
                    tracing::warn!("Failed to start MPRIS server: {e}");
                }
            }
        });
    }
}

pub fn init_mpris() {
    #[cfg(feature = "mpris")]
    linux::start_mpris_server();
    #[cfg(not(feature = "mpris"))]
    tracing::info!("MPRIS not available (enable 'mpris' feature to activate)");
}

#[cfg(feature = "mpris")]
pub fn build_metadata(status: &PlaybackStatus) -> mpris_server::Metadata {
    use mpris_server::{Metadata, Time, TrackId};

    let mut builder = Metadata::builder();

    if let Some(duration_ms) = status.duration_ms {
        builder = builder.length(Time::from_millis(duration_ms as i64));
    }
    if let Some(ref title) = status.current_title {
        builder = builder.title(title.clone());
    }
    if let Some(ref path) = status.current_path {
        let track_id = format!("/org/mpris/MediaPlayer2/TrackId/{}", path);
        if let Ok(id) = TrackId::try_from(track_id.as_str()) {
            builder = builder.trackid(id);
        }
    }

    builder.build()
}

#[cfg(not(feature = "mpris"))]
#[allow(dead_code)]
pub fn build_metadata(_status: &PlaybackStatus) {}

pub fn update_metadata(status: &PlaybackStatus) {
    #[cfg(feature = "mpris")]
    {
        if let Some(server_lock) = linux::get_server().get() {
            if let Ok(mut guard) = server_lock.lock() {
                if let Some(ref srv) = *guard {
                    let props: [Property; 0] = [];
                    let _ = srv.properties_changed(props);
                }
            }
        }
    }
    #[cfg(not(feature = "mpris"))]
    let _ = status;
}

pub fn play_gapless_from_snapshot(queue: &crate::audio::player::QueueSnapshot) {
    if let Some(song) = &queue.current_song {
        let path = song.id.clone();
        let title = Some(song.title.clone());
        if let Ok(status) = crate::audio::player::play_gapless_local_tracks(vec![
            crate::audio::player::LocalPlaybackTrack { path, title },
        ]) {
            update_metadata(&status);
        }
    }
}
