mod audio_player;
mod auth_flows;
mod credentials;
pub mod error;
mod jellyfin;
mod library;
mod listening_history;
mod lyrics;
mod metadata;
mod music_player;
mod playback_store;
mod playlists;
mod providers;
mod remote_providers;
mod scrobbling;
pub mod secrets;
mod spotify_native;

use std::path::Path;

use credentials::{JellyfinAccount, ProviderAccount, ProviderLoginState};
use serde::Serialize;
use serde_json::Value;
use tauri::AppHandle;
use tracing::instrument;

use crate::jellyfin::Api;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|_app| {
            secrets::init_default_credentials();
            tauri::async_runtime::spawn(async move {
                music_player::run().await;
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            display_song_list,
            get_jellyfin_account,
            save_jellyfin_account,
            clear_jellyfin_account,
            list_provider_accounts,
            list_provider_login_states,
            start_spotify_pkce_login,
            finish_spotify_pkce_login,
            refresh_spotify_access_token,
            complete_spotify_pkce_login_in_browser,
            get_spotify_web_playback_token,
            start_tidal_pkce_login,
            finish_tidal_pkce_login,
            refresh_tidal_access_token,
            start_youtube_oauth_login,
            finish_youtube_oauth_login,
            refresh_youtube_access_token,
            start_lastfm_login,
            finish_lastfm_login,
            save_provider_account,
            clear_provider_account,
            get_lastfm_scrobble_status,
            retry_lastfm_scrobbles,
            search_spotify_tracks,
            list_spotify_playlists,
            get_spotify_playlist_tracks,
            search_tidal_tracks,
            list_tidal_playlists,
            search_tidal_playlists,
            get_tidal_playlist_tracks,
            search_qobuz_tracks,
            search_youtube_tracks,
            search_youtube_playlists,
            get_youtube_playlist_tracks,
            search_lastfm_tracks,
            scan_library_path,
            list_library_tracks,
            play_local_track,
            playback_pause,
            playback_resume,
            playback_stop,
            playback_seek,
            set_playback_volume,
            get_playback_status,
            list_audio_output_devices,
            set_audio_output_device,
            set_replay_gain_mode,
            queue_song,
            remove_queued_song,
            move_queued_song,
            get_queue_snapshot,
            advance_queue_to_song_id,
            play_track_now,
            play_current_queue_song,
            play_next_queue_song,
            play_previous_queue_song,
            create_playlist,
            list_playlists,
            get_playlist,
            add_song_to_playlist,
            import_m3u_playlist,
            export_m3u_playlist,
            get_track_cover_art,
            get_local_lyrics,
            get_track_lyrics,
            search_metadata_suggestions,
            list_listening_history,
            list_listening_history_summary,
            list_service_capabilities,
            get_provider_credentials,
            set_provider_credentials,
            reset_provider_credentials,
            get_all_provider_statuses,
            spotify_native_status,
            connect_spotify_native,
            disconnect_spotify_native,
            start_spotify_native_playback,
            spotify_native_pause,
            spotify_native_resume,
            spotify_native_stop
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[derive(Serialize)]
struct QueuePlaybackResult {
    queue: music_player::QueueSnapshot,
    playback_status: Option<audio_player::PlaybackStatus>,
    message: Option<String>,
}

#[instrument]
#[tauri::command(rename_all = "snake_case")]
async fn display_song_list() -> Result<Value, String> {
    let credentials = credentials::load_jellyfin_credentials()?;
    let api = Api::new(
        credentials.base_url,
        credentials.user_name,
        credentials.password,
    )
    .await?;
    let songs = api.get_all_songs().await?;
    Ok(songs
        .get("Items")
        .cloned()
        .unwrap_or_else(|| Value::Array(Vec::new())))
}

#[tauri::command(rename_all = "snake_case")]
fn get_jellyfin_account() -> Result<Option<JellyfinAccount>, String> {
    credentials::get_jellyfin_account()
}

#[tauri::command(rename_all = "snake_case")]
fn save_jellyfin_account(
    base_url: String,
    user_name: String,
    password: String,
) -> Result<JellyfinAccount, String> {
    credentials::save_jellyfin_account(base_url, user_name, password)
}

#[tauri::command(rename_all = "snake_case")]
fn clear_jellyfin_account() -> Result<(), String> {
    credentials::clear_jellyfin_account()
}

#[tauri::command(rename_all = "snake_case")]
fn list_provider_accounts() -> Result<Vec<ProviderAccount>, String> {
    credentials::list_provider_accounts()
}

#[tauri::command(rename_all = "snake_case")]
fn list_provider_login_states() -> Result<Vec<ProviderLoginState>, String> {
    credentials::list_provider_login_states()
}

#[tauri::command(rename_all = "snake_case")]
#[allow(clippy::too_many_arguments)]
fn save_provider_account(
    provider_id: String,
    display_name: Option<String>,
    client_id: Option<String>,
    client_secret: Option<String>,
    api_key: Option<String>,
    api_secret: Option<String>,
    access_token: Option<String>,
    refresh_token: Option<String>,
) -> Result<ProviderAccount, String> {
    credentials::save_provider_account(
        provider_id,
        display_name,
        client_id,
        client_secret,
        api_key,
        api_secret,
        access_token,
        refresh_token,
    )
}

#[tauri::command(rename_all = "snake_case")]
fn clear_provider_account(provider_id: String) -> Result<(), String> {
    credentials::clear_provider_account(provider_id)
}

#[tauri::command(rename_all = "snake_case")]
fn start_spotify_pkce_login(
    redirect_uri: String,
    scope: Option<String>,
) -> Result<auth_flows::ProviderLoginStart, String> {
    auth_flows::start_spotify_pkce_login(redirect_uri, scope)
}

#[tauri::command(rename_all = "snake_case")]
async fn finish_spotify_pkce_login(
    code: String,
    state: Option<String>,
) -> Result<ProviderAccount, String> {
    auth_flows::finish_spotify_pkce_login(code, state).await
}

#[tauri::command(rename_all = "snake_case")]
async fn refresh_spotify_access_token() -> Result<ProviderAccount, String> {
    auth_flows::refresh_spotify_access_token().await
}

#[tauri::command(rename_all = "snake_case")]
async fn complete_spotify_pkce_login_in_browser(
    redirect_uri: String,
    scope: Option<String>,
) -> Result<ProviderAccount, String> {
    auth_flows::complete_spotify_pkce_login_in_browser(redirect_uri, scope).await
}

#[instrument]
#[tauri::command(rename_all = "snake_case")]
fn get_spotify_web_playback_token() -> Result<String, String> {
    auth_flows::get_spotify_web_playback_token()
}

#[instrument]
#[tauri::command(rename_all = "snake_case")]
fn start_tidal_pkce_login(
    redirect_uri: String,
    scope: Option<String>,
) -> Result<auth_flows::ProviderLoginStart, String> {
    auth_flows::start_tidal_pkce_login(redirect_uri, scope)
}

#[tauri::command(rename_all = "snake_case")]
async fn finish_tidal_pkce_login(
    code: String,
    state: Option<String>,
) -> Result<ProviderAccount, String> {
    auth_flows::finish_tidal_pkce_login(code, state).await
}

#[tauri::command(rename_all = "snake_case")]
async fn refresh_tidal_access_token() -> Result<ProviderAccount, String> {
    auth_flows::refresh_tidal_access_token().await
}

#[instrument]
#[tauri::command(rename_all = "snake_case")]
fn start_youtube_oauth_login(
    redirect_uri: String,
    scope: Option<String>,
) -> Result<auth_flows::ProviderLoginStart, String> {
    auth_flows::start_youtube_oauth_login(redirect_uri, scope)
}

#[tauri::command(rename_all = "snake_case")]
async fn finish_youtube_oauth_login(
    code: String,
    state: Option<String>,
) -> Result<ProviderAccount, String> {
    auth_flows::finish_youtube_oauth_login(code, state).await
}

#[tauri::command(rename_all = "snake_case")]
async fn refresh_youtube_access_token() -> Result<ProviderAccount, String> {
    auth_flows::refresh_youtube_access_token().await
}

#[tauri::command(rename_all = "snake_case")]
async fn start_lastfm_login() -> Result<auth_flows::ProviderLoginStart, String> {
    auth_flows::start_lastfm_login().await
}

#[tauri::command(rename_all = "snake_case")]
async fn finish_lastfm_login() -> Result<ProviderAccount, String> {
    auth_flows::finish_lastfm_login().await
}

#[tauri::command(rename_all = "snake_case")]
fn get_lastfm_scrobble_status(app: AppHandle) -> Result<scrobbling::LastFmScrobbleStatus, String> {
    scrobbling::get_lastfm_scrobble_status(&app)
}

#[tauri::command(rename_all = "snake_case")]
async fn retry_lastfm_scrobbles(
    app: AppHandle,
) -> Result<scrobbling::LastFmScrobbleStatus, String> {
    scrobbling::retry_lastfm_scrobbles(&app).await
}

#[tauri::command(rename_all = "snake_case")]
async fn search_spotify_tracks(
    query: String,
    limit: Option<u32>,
) -> Result<Vec<remote_providers::RemoteTrack>, String> {
    remote_providers::search_spotify_tracks(query, limit).await
}

#[tauri::command(rename_all = "snake_case")]
async fn list_spotify_playlists(
    limit: Option<u32>,
) -> Result<Vec<remote_providers::RemotePlaylist>, String> {
    remote_providers::list_spotify_playlists(limit).await
}

#[tauri::command(rename_all = "snake_case")]
async fn get_spotify_playlist_tracks(
    playlist_id: String,
    limit: Option<u32>,
) -> Result<Vec<remote_providers::RemoteTrack>, String> {
    remote_providers::get_spotify_playlist_tracks(playlist_id, limit).await
}

#[tauri::command(rename_all = "snake_case")]
async fn search_tidal_tracks(
    query: String,
    country_code: Option<String>,
    limit: Option<u32>,
) -> Result<Vec<remote_providers::RemoteTrack>, String> {
    remote_providers::search_tidal_tracks(query, country_code, limit).await
}

#[tauri::command(rename_all = "snake_case")]
async fn list_tidal_playlists(
    country_code: Option<String>,
    limit: Option<u32>,
) -> Result<Vec<remote_providers::RemotePlaylist>, String> {
    remote_providers::list_tidal_playlists(country_code, limit).await
}

#[tauri::command(rename_all = "snake_case")]
async fn search_tidal_playlists(
    query: String,
    country_code: Option<String>,
    limit: Option<u32>,
) -> Result<Vec<remote_providers::RemotePlaylist>, String> {
    remote_providers::search_tidal_playlists(query, country_code, limit).await
}

#[tauri::command(rename_all = "snake_case")]
async fn get_tidal_playlist_tracks(
    playlist_id: String,
    country_code: Option<String>,
    limit: Option<u32>,
) -> Result<Vec<remote_providers::RemoteTrack>, String> {
    remote_providers::get_tidal_playlist_tracks(playlist_id, country_code, limit).await
}

#[tauri::command(rename_all = "snake_case")]
async fn search_qobuz_tracks(
    query: String,
    limit: Option<u32>,
) -> Result<Vec<remote_providers::RemoteTrack>, String> {
    remote_providers::search_qobuz_tracks(query, limit).await
}

#[tauri::command(rename_all = "snake_case")]
async fn search_youtube_tracks(
    query: String,
    limit: Option<u32>,
) -> Result<Vec<remote_providers::RemoteTrack>, String> {
    remote_providers::search_youtube_tracks(query, limit).await
}

#[tauri::command(rename_all = "snake_case")]
async fn search_youtube_playlists(
    query: String,
    limit: Option<u32>,
) -> Result<Vec<remote_providers::RemotePlaylist>, String> {
    remote_providers::search_youtube_playlists(query, limit).await
}

#[tauri::command(rename_all = "snake_case")]
async fn get_youtube_playlist_tracks(
    playlist_id: String,
    limit: Option<u32>,
) -> Result<Vec<remote_providers::RemoteTrack>, String> {
    remote_providers::get_youtube_playlist_tracks(playlist_id, limit).await
}

#[tauri::command(rename_all = "snake_case")]
async fn search_lastfm_tracks(
    query: String,
    limit: Option<u32>,
) -> Result<Vec<remote_providers::RemoteTrack>, String> {
    remote_providers::search_lastfm_tracks(query, limit).await
}

#[instrument(skip(app))]
#[tauri::command(rename_all = "snake_case")]
fn scan_library_path(app: AppHandle, path: String) -> Result<library::ScanSummary, String> {
    tracing::info!("Starting library scan: {path}");
    library::scan_library_path(&app, path)
}

#[instrument(skip(app))]
#[tauri::command(rename_all = "snake_case")]
fn list_library_tracks(app: AppHandle) -> Result<Vec<library::LibraryTrack>, String> {
    library::list_library_tracks(&app)
}

#[instrument]
#[tauri::command(rename_all = "snake_case")]
fn get_track_cover_art(path: String) -> Result<library::CoverArt, String> {
    library::get_track_cover_art(path)
}

#[tauri::command(rename_all = "snake_case")]
fn get_local_lyrics(path: String) -> Result<Option<lyrics::LyricsResult>, String> {
    lyrics::get_local_lyrics(path)
}

#[instrument]
#[tauri::command(rename_all = "snake_case")]
async fn get_track_lyrics(
    path: String,
    title: String,
    artist: Option<String>,
    album: Option<String>,
    duration_ms: Option<u64>,
) -> Result<Option<lyrics::LyricsResult>, String> {
    lyrics::get_track_lyrics(path, title, artist, album, duration_ms).await
}

#[instrument]
#[tauri::command(rename_all = "snake_case")]
async fn search_metadata_suggestions(
    title: String,
    artist: Option<String>,
    album: Option<String>,
    duration_ms: Option<u64>,
) -> Result<Vec<metadata::MetadataSuggestion>, String> {
    metadata::search_metadata_suggestions(title, artist, album, duration_ms).await
}

#[tauri::command(rename_all = "snake_case")]
fn list_listening_history(
    app: AppHandle,
    limit: Option<u32>,
) -> Result<Vec<listening_history::ListeningHistoryEntry>, String> {
    listening_history::list_listening_history(&app, limit)
}

#[tauri::command(rename_all = "snake_case")]
fn list_listening_history_summary(
    app: AppHandle,
    limit: Option<u32>,
) -> Result<Vec<listening_history::ListeningHistorySummary>, String> {
    listening_history::list_listening_history_summary(&app, limit)
}

#[tauri::command(rename_all = "snake_case")]
fn list_service_capabilities() -> Vec<providers::ProviderCapability> {
    providers::list_service_capabilities()
}

#[instrument(skip(app))]
#[tauri::command(rename_all = "snake_case")]
fn play_local_track(
    app: AppHandle,
    path: String,
    title: Option<String>,
) -> Result<audio_player::PlaybackStatus, String> {
    play_local_track_with_restore(&app, path, title)
}

#[instrument(skip(app))]
#[tauri::command(rename_all = "snake_case")]
fn playback_pause(app: AppHandle) -> Result<audio_player::PlaybackStatus, String> {
    let status = audio_player::playback_pause()?;
    save_status_position(&app, &status)?;
    record_playback_event(&app, &status, "paused")?;
    Ok(status)
}

#[instrument(skip(app))]
#[tauri::command(rename_all = "snake_case")]
fn playback_resume(app: AppHandle) -> Result<audio_player::PlaybackStatus, String> {
    let status = audio_player::playback_resume()?;
    record_playback_event(&app, &status, "resumed")?;
    Ok(status)
}

#[instrument(skip(app))]
#[tauri::command(rename_all = "snake_case")]
fn playback_stop(app: AppHandle) -> Result<audio_player::PlaybackStatus, String> {
    let status = audio_player::get_playback_status()?;
    if let Some(path) = status.current_path.as_deref() {
        record_playback_event(&app, &status, "stopped")?;
        playback_store::save_playback_position(
            &app,
            path,
            status.current_title.as_deref(),
            0,
            status.duration_ms,
        )?;
    }
    audio_player::playback_stop()
}

#[instrument(skip(app))]
#[tauri::command(rename_all = "snake_case")]
fn playback_seek(app: AppHandle, position_ms: u64) -> Result<audio_player::PlaybackStatus, String> {
    let status = audio_player::playback_seek(position_ms)?;
    save_status_position(&app, &status)?;
    record_playback_event(&app, &status, "seeked")?;
    Ok(status)
}

#[instrument]
#[tauri::command(rename_all = "snake_case")]
fn set_playback_volume(volume: f32) -> Result<audio_player::PlaybackStatus, String> {
    audio_player::set_playback_volume(volume)
}

#[instrument(skip(app))]
#[tauri::command(rename_all = "snake_case")]
fn get_playback_status(app: AppHandle) -> Result<audio_player::PlaybackStatus, String> {
    let status = audio_player::get_playback_status()?;
    handle_playback_transitions(&app)?;
    save_status_position(&app, &status)?;
    Ok(status)
}

#[tauri::command(rename_all = "snake_case")]
fn list_audio_output_devices() -> Result<Vec<audio_player::AudioOutputDevice>, String> {
    audio_player::list_audio_output_devices()
}

#[tauri::command(rename_all = "snake_case")]
fn set_audio_output_device(
    device_id: Option<String>,
) -> Result<audio_player::PlaybackStatus, String> {
    audio_player::set_audio_output_device(device_id)
}

#[tauri::command(rename_all = "snake_case")]
fn set_replay_gain_mode(mode: String) -> Result<audio_player::PlaybackStatus, String> {
    audio_player::set_replay_gain_mode(mode)
}

#[instrument]
#[tauri::command(rename_all = "snake_case")]
fn queue_song(song: music_player::Song) -> Result<music_player::QueueSnapshot, String> {
    music_player::queue_song(song)
}

#[tauri::command(rename_all = "snake_case")]
fn remove_queued_song(song: music_player::Song) -> Result<music_player::QueueSnapshot, String> {
    music_player::remove_song(song)
}

#[tauri::command(rename_all = "snake_case")]
fn move_queued_song(
    from_index: usize,
    to_index: usize,
) -> Result<music_player::QueueSnapshot, String> {
    music_player::move_song(from_index, to_index)
}

#[tauri::command(rename_all = "snake_case")]
fn get_queue_snapshot() -> Result<music_player::QueueSnapshot, String> {
    music_player::get_queue_snapshot()
}

#[tauri::command(rename_all = "snake_case")]
fn advance_queue_to_song_id(song_id: String) -> Result<music_player::QueueSnapshot, String> {
    music_player::advance_to_song_id(&song_id)
}

#[instrument(skip(app))]
#[tauri::command(rename_all = "snake_case")]
fn play_track_now(app: AppHandle, song: music_player::Song) -> Result<QueuePlaybackResult, String> {
    let queue = music_player::play_track_now(song)?;
    play_queue_snapshot(&app, queue)
}

#[instrument(skip(app))]
#[tauri::command(rename_all = "snake_case")]
fn play_current_queue_song(app: AppHandle) -> Result<QueuePlaybackResult, String> {
    let queue = music_player::get_queue_snapshot()?;
    if queue.current_song.is_some() {
        return play_queue_snapshot(&app, queue);
    }
    play_queue_snapshot(&app, music_player::next_queue_song()?)
}

#[instrument(skip(app))]
#[tauri::command(rename_all = "snake_case")]
fn play_next_queue_song(app: AppHandle) -> Result<QueuePlaybackResult, String> {
    play_queue_snapshot(&app, music_player::next_queue_song()?)
}

#[instrument(skip(app))]
#[tauri::command(rename_all = "snake_case")]
fn play_previous_queue_song(app: AppHandle) -> Result<QueuePlaybackResult, String> {
    play_queue_snapshot(&app, music_player::previous_queue_song()?)
}

#[instrument(skip(app))]
fn play_queue_snapshot(
    app: &AppHandle,
    queue: music_player::QueueSnapshot,
) -> Result<QueuePlaybackResult, String> {
    if queue.current_song.is_none() {
        return Ok(QueuePlaybackResult {
            queue,
            playback_status: None,
            message: Some("Queue is empty.".to_string()),
        });
    }

    let local_tracks = local_gapless_tracks(&queue);
    if !local_tracks.is_empty() {
        let playback_status = play_local_tracks_with_restore(app, local_tracks)?;
        return Ok(QueuePlaybackResult {
            queue,
            playback_status: Some(playback_status),
            message: None,
        });
    }

    Ok(QueuePlaybackResult {
        queue,
        playback_status: None,
        message: Some("The selected queued track is not a local playable file.".to_string()),
    })
}

#[instrument(skip(app))]
fn play_local_track_with_restore(
    app: &AppHandle,
    path: String,
    title: Option<String>,
) -> Result<audio_player::PlaybackStatus, String> {
    play_local_tracks_with_restore(app, vec![audio_player::LocalPlaybackTrack { path, title }])
}

#[instrument(skip(app))]
fn play_local_tracks_with_restore(
    app: &AppHandle,
    tracks: Vec<audio_player::LocalPlaybackTrack>,
) -> Result<audio_player::PlaybackStatus, String> {
    let Some(first_track) = tracks.first() else {
        return Err("No local tracks were provided for playback.".to_string());
    };

    audio_player::get_playback_status()?;
    handle_playback_transitions(app)?;
    record_current_track_transition(app, &first_track.path)?;
    let resume_position =
        playback_store::get_playback_position(app, &first_track.path)?.and_then(|position| {
            playback_store::resumable_position(position.position_ms, position.duration_ms)
        });
    let mut status = audio_player::play_gapless_local_tracks(tracks)?;
    if let Some(position_ms) = resume_position {
        status = audio_player::playback_seek(position_ms)?;
    }
    save_status_position(app, &status)?;
    record_playback_event(app, &status, "started")?;
    Ok(status)
}

fn local_gapless_tracks(
    queue: &music_player::QueueSnapshot,
) -> Vec<audio_player::LocalPlaybackTrack> {
    let mut tracks = Vec::new();
    let Some(current_song) = queue.current_song.as_ref() else {
        return tracks;
    };
    if !Path::new(&current_song.id).is_file() {
        return tracks;
    }

    tracks.push(audio_player::LocalPlaybackTrack {
        path: current_song.id.clone(),
        title: Some(current_song.title.clone()),
    });
    for song in &queue.upcoming {
        if !Path::new(&song.id).is_file() {
            break;
        }
        tracks.push(audio_player::LocalPlaybackTrack {
            path: song.id.clone(),
            title: Some(song.title.clone()),
        });
    }
    tracks
}

#[instrument(skip(app))]
fn record_current_track_transition(app: &AppHandle, next_path: &str) -> Result<(), String> {
    let status = audio_player::get_playback_status()?;
    if status
        .current_path
        .as_deref()
        .is_some_and(|current_path| current_path != next_path)
        && matches!(status.state.as_str(), "playing" | "paused" | "ended")
    {
        record_playback_event(app, &status, "changed")?;
    }
    save_status_position(app, &status)
}

#[instrument(skip(app, status))]
fn record_playback_event(
    app: &AppHandle,
    status: &audio_player::PlaybackStatus,
    event: &str,
) -> Result<(), String> {
    tracing::info!(event, path = ?status.current_path, "Recording playback event");
    listening_history::record_playback_event(app, status, event)?;
    if event == "started" {
        let app = app.clone();
        let status = status.clone();
        tauri::async_runtime::spawn(async move {
            if let Err(error) =
                scrobbling::update_lastfm_now_playing_from_status(&app, &status).await
            {
                eprintln!("Last.fm now-playing update failed: {error}");
            }
        });
    }
    if matches!(event, "stopped" | "changed" | "ended") {
        scrobbling::queue_lastfm_scrobble_from_status(app, status)?;
    }
    Ok(())
}

#[instrument(skip(app))]
fn handle_playback_transitions(app: &AppHandle) -> Result<(), String> {
    for transition in audio_player::drain_playback_transitions()? {
        if transition.event == "started" {
            if let Some(path) = transition.status.current_path.as_deref() {
                music_player::advance_to_song_id(path)?;
            }
        }
        save_status_position(app, &transition.status)?;
        record_playback_event(app, &transition.status, &transition.event)?;
    }
    Ok(())
}

#[instrument(skip(app, status))]
fn save_status_position(
    app: &AppHandle,
    status: &audio_player::PlaybackStatus,
) -> Result<(), String> {
    if let Some(path) = status.current_path.as_deref() {
        playback_store::save_playback_position(
            app,
            path,
            status.current_title.as_deref(),
            status.position_ms,
            status.duration_ms,
        )?;
    }
    Ok(())
}

#[instrument(skip(app))]
#[tauri::command(rename_all = "snake_case")]
fn create_playlist(app: AppHandle, name: String) -> Result<playlists::PlaylistDetail, String> {
    playlists::create_playlist(&app, name)
}

#[instrument(skip(app))]
#[tauri::command(rename_all = "snake_case")]
fn list_playlists(app: AppHandle) -> Result<Vec<playlists::PlaylistSummary>, String> {
    playlists::list_playlists(&app)
}

#[instrument(skip(app))]
#[tauri::command(rename_all = "snake_case")]
fn get_playlist(app: AppHandle, playlist_id: i64) -> Result<playlists::PlaylistDetail, String> {
    playlists::get_playlist(&app, playlist_id)
}

#[instrument(skip(app))]
#[tauri::command(rename_all = "snake_case")]
fn add_song_to_playlist(
    app: AppHandle,
    playlist_id: i64,
    song: music_player::Song,
) -> Result<playlists::PlaylistDetail, String> {
    playlists::add_song_to_playlist(&app, playlist_id, song)
}

#[instrument(skip(app))]
#[tauri::command(rename_all = "snake_case")]
fn import_m3u_playlist(
    app: AppHandle,
    path: String,
    name: Option<String>,
) -> Result<playlists::PlaylistDetail, String> {
    playlists::import_m3u_playlist(&app, path, name)
}

#[tauri::command(rename_all = "snake_case")]
fn export_m3u_playlist(app: AppHandle, playlist_id: i64, path: String) -> Result<(), String> {
    playlists::export_m3u_playlist(&app, playlist_id, path)
}

#[tauri::command(rename_all = "snake_case")]
fn get_provider_credentials(provider: String) -> Result<serde_json::Value, String> {
    let has_client_id = secrets::get_credential(&provider, "client_id").is_some();
    let has_client_secret = secrets::get_credential(&provider, "client_secret").is_some();
    let has_api_key = secrets::get_credential(&provider, "api_key").is_some();
    let has_api_secret = secrets::get_credential(&provider, "api_secret").is_some();
    let has_redirect_uri = secrets::get_credential(&provider, "redirect_uri").is_some();
    let has_app_id = secrets::get_credential(&provider, "app_id").is_some();
    let has_app_secret = secrets::get_credential(&provider, "app_secret").is_some();
    let has_creds = has_client_id
        || has_client_secret
        || has_api_key
        || has_api_secret
        || has_app_id
        || has_app_secret
        || has_redirect_uri;
    let is_default = secrets::is_default_credential(&provider);

    Ok(serde_json::json!({
        "provider": provider,
        "has_creds": has_creds,
        "client_id": has_client_id,
        "client_secret": has_client_secret,
        "api_key": has_api_key,
        "api_secret": has_api_secret,
        "redirect_uri": has_redirect_uri,
        "app_id": has_app_id,
        "app_secret": has_app_secret,
        "is_default": is_default
    }))
}

#[tauri::command(rename_all = "snake_case")]
fn set_provider_credentials(provider: String, key: String, value: String) -> Result<(), String> {
    secrets::set_credential(&provider, &key, &value)?;
    secrets::mark_custom(&provider)?;
    Ok(())
}

#[tauri::command(rename_all = "snake_case")]
fn reset_provider_credentials(provider: String) -> Result<(), String> {
    secrets::reset_to_default(&provider)
}

#[tauri::command(rename_all = "snake_case")]
fn get_all_provider_statuses() -> Result<serde_json::Value, String> {
    let providers = [
        ("spotify", "Spotify", "music"),
        ("tidal", "TIDAL", "radio"),
        ("qobuz", "Qobuz", "disc-3"),
        ("youtube", "YouTube Music", "youtube"),
        ("lastfm", "Last.fm", "radio-tower"),
        ("bandcamp", "Bandcamp", "shopping-bag"),
    ];

    let mut results = Vec::new();
    for (id, name, icon) in &providers {
        let has_creds = secrets::has_credentials(id);
        let is_default = secrets::is_default_credential(id);

        let has_access_token = secrets::get_credential(id, "access_token").is_some()
            || crate::credentials::list_provider_accounts()
                .ok()
                .and_then(|accounts| {
                    accounts
                        .into_iter()
                        .find(|a| a.provider_id == *id)
                        .map(|a| a.has_access_token)
                })
                .unwrap_or(false);

        let is_connected = has_access_token;

        results.push(serde_json::json!({
            "id": id,
            "name": name,
            "icon": icon,
            "has_creds": has_creds,
            "is_default": is_default,
            "is_connected": is_connected
        }));
    }

    Ok(serde_json::Value::Array(results))
}

// --- Native Spotify Playback Commands ---

#[tauri::command(rename_all = "snake_case")]
fn spotify_native_status() -> Result<spotify_native::SpotifyNativeStatus, String> {
    spotify_native::spotify_native_status()
}

#[instrument]
#[tauri::command(rename_all = "snake_case")]
async fn connect_spotify_native(
    access_token: String,
) -> Result<spotify_native::SpotifyNativeStatus, String> {
    tracing::info!("Connecting native Spotify player");
    spotify_native::connect_spotify_native(access_token).await
}

#[instrument]
#[tauri::command(rename_all = "snake_case")]
async fn disconnect_spotify_native() -> Result<spotify_native::SpotifyNativeStatus, String> {
    tracing::info!("Disconnecting native Spotify player");
    spotify_native::disconnect_spotify_native().await
}

#[instrument]
#[tauri::command(rename_all = "snake_case")]
fn start_spotify_native_playback(
    track_uri: String,
    device_id: Option<String>,
) -> Result<spotify_native::SpotifyNativeStatus, String> {
    tracing::info!(track_uri, "Starting native Spotify playback");
    spotify_native::start_spotify_native_playback(track_uri, device_id)
}

#[instrument]
#[tauri::command(rename_all = "snake_case")]
fn spotify_native_pause() -> Result<spotify_native::SpotifyNativeStatus, String> {
    spotify_native::spotify_native_pause()
}

#[instrument]
#[tauri::command(rename_all = "snake_case")]
fn spotify_native_resume() -> Result<spotify_native::SpotifyNativeStatus, String> {
    spotify_native::spotify_native_resume()
}

#[instrument]
#[tauri::command(rename_all = "snake_case")]
fn spotify_native_stop() -> Result<spotify_native::SpotifyNativeStatus, String> {
    spotify_native::spotify_native_stop()
}
