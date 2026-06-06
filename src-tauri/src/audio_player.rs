use std::fs::File;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard, OnceLock};
use std::time::Duration;

use lofty::file::TaggedFileExt;
use lofty::tag::ItemKey;
use rodio::cpal;
use rodio::cpal::traits::{DeviceTrait, HostTrait};
use rodio::{Decoder, DeviceSinkBuilder, MixerDeviceSink, Player, Source};
use serde::Serialize;
use tracing::instrument;

static AUDIO_PLAYER: OnceLock<Mutex<AudioPlayer>> = OnceLock::new();

#[derive(Clone, Debug, Serialize)]
pub struct PlaybackStatus {
    pub state: String,
    pub playing: bool,
    pub paused: bool,
    pub current_path: Option<String>,
    pub current_title: Option<String>,
    pub position_ms: u64,
    pub duration_ms: Option<u64>,
    pub volume: f32,
    pub source_format: Option<String>,
    pub source_is_lossless: Option<bool>,
    pub source_sample_rate: Option<u32>,
    pub source_channels: Option<u16>,
    pub output_sample_rate: Option<u32>,
    pub output_channels: Option<u16>,
    pub output_sample_format: Option<String>,
    pub output_device_id: Option<String>,
    pub output_device_name: Option<String>,
    pub quality_warnings: Vec<String>,
    pub replay_gain_mode: String,
    pub replay_gain_db: Option<f32>,
    pub replay_gain_source: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct AudioOutputDevice {
    pub id: String,
    pub name: String,
    pub selected: bool,
    pub is_default: bool,
    pub default_sample_rate: Option<u32>,
    pub default_channels: Option<u16>,
    pub default_sample_format: Option<String>,
}

#[derive(Clone, Debug)]
pub struct LocalPlaybackTrack {
    pub path: String,
    pub title: Option<String>,
}

#[derive(Clone, Debug)]
pub struct PlaybackTransition {
    pub event: String,
    pub status: PlaybackStatus,
}

struct AudioPlayer {
    sink: Option<MixerDeviceSink>,
    player: Option<Player>,
    current_path: Option<String>,
    current_title: Option<String>,
    duration_ms: Option<u64>,
    source_format: Option<String>,
    source_is_lossless: Option<bool>,
    source_sample_rate: Option<u32>,
    source_channels: Option<u16>,
    output_sample_rate: Option<u32>,
    output_channels: Option<u16>,
    output_sample_format: Option<String>,
    output_device_id: Option<String>,
    output_device_name: Option<String>,
    selected_output_device_id: Option<String>,
    replay_gain_mode: ReplayGainMode,
    replay_gain_db: Option<f32>,
    replay_gain_source: Option<String>,
    gapless_tracks: Vec<PreparedLocalTrack>,
    gapless_index: usize,
    gapless_total_sources: usize,
    gapless_events: Vec<PlaybackTransition>,
    volume: f32,
    stopped: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ReplayGainMode {
    Off,
    Track,
    Album,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct AppliedReplayGain {
    gain_db: Option<f32>,
    source: Option<&'static str>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct ReplayGainTags {
    track_gain_db: Option<f32>,
    album_gain_db: Option<f32>,
}

#[derive(Clone, Debug)]
struct PreparedLocalTrack {
    path: String,
    title: String,
    duration_ms: Option<u64>,
    source_format: Option<String>,
    source_is_lossless: Option<bool>,
    source_sample_rate: Option<u32>,
    source_channels: Option<u16>,
    replay_gain_db: Option<f32>,
    replay_gain_source: Option<String>,
}

impl Default for AudioPlayer {
    fn default() -> Self {
        Self {
            sink: None,
            player: None,
            current_path: None,
            current_title: None,
            duration_ms: None,
            source_format: None,
            source_is_lossless: None,
            source_sample_rate: None,
            source_channels: None,
            output_sample_rate: None,
            output_channels: None,
            output_sample_format: None,
            output_device_id: None,
            output_device_name: None,
            selected_output_device_id: None,
            replay_gain_mode: ReplayGainMode::Off,
            replay_gain_db: None,
            replay_gain_source: None,
            gapless_tracks: Vec::new(),
            gapless_index: 0,
            gapless_total_sources: 0,
            gapless_events: Vec::new(),
            volume: 1.0,
            stopped: false,
        }
    }
}

impl AudioPlayer {
    fn play_local_track(
        &mut self,
        path: String,
        title: Option<String>,
    ) -> Result<PlaybackStatus, String> {
        self.play_gapless_tracks(vec![LocalPlaybackTrack { path, title }])
    }

    fn play_gapless_tracks(
        &mut self,
        tracks: Vec<LocalPlaybackTrack>,
    ) -> Result<PlaybackStatus, String> {
        if tracks.is_empty() {
            return Err("No local tracks were provided for playback.".to_string());
        }

        let prepared_tracks = tracks
            .iter()
            .map(|track| prepare_local_track(track, self.replay_gain_mode))
            .collect::<Result<Vec<_>, _>>()?;

        self.ensure_output()?;

        let player = self
            .player
            .as_ref()
            .ok_or_else(|| "Audio output was not initialized.".to_string())?;
        if !player.empty() {
            player.clear();
        }
        player.set_volume(self.volume);
        let mut gapless_tracks = Vec::with_capacity(prepared_tracks.len());
        for (track, source) in prepared_tracks {
            player.append(source);
            gapless_tracks.push(track);
        }
        player.play();

        self.gapless_tracks = gapless_tracks;
        self.gapless_index = 0;
        self.gapless_total_sources = self.gapless_tracks.len();
        self.gapless_events.clear();
        self.apply_prepared_track(0);
        self.stopped = false;

        Ok(self.status())
    }

    fn pause(&mut self) -> PlaybackStatus {
        if let Some(player) = &self.player {
            player.pause();
        }
        self.status()
    }

    fn resume(&mut self) -> PlaybackStatus {
        if let Some(player) = &self.player {
            player.play();
            self.stopped = false;
        }
        self.status()
    }

    fn stop(&mut self) -> PlaybackStatus {
        if let Some(player) = &self.player {
            player.stop();
            player.pause();
        }
        self.current_path = None;
        self.current_title = None;
        self.duration_ms = None;
        self.source_format = None;
        self.source_is_lossless = None;
        self.source_sample_rate = None;
        self.source_channels = None;
        self.replay_gain_db = None;
        self.replay_gain_source = None;
        self.clear_gapless_state();
        self.stopped = true;
        self.status()
    }

    fn set_replay_gain_mode(&mut self, mode: String) -> Result<PlaybackStatus, String> {
        let replay_gain_mode = ReplayGainMode::from_string(&mode)?;
        if self.replay_gain_mode == replay_gain_mode {
            return Ok(self.status());
        }

        let previous_status = self.status();
        let current_path = self.current_path.clone();
        let current_title = self.current_title.clone();
        self.replay_gain_mode = replay_gain_mode;

        if let Some(path) = current_path {
            if matches!(previous_status.state.as_str(), "playing" | "paused") {
                let mut status = self.play_local_track(path, current_title)?;
                if previous_status.position_ms > 0 {
                    status = self.seek(previous_status.position_ms)?;
                }
                if previous_status.paused {
                    status = self.pause();
                }
                return Ok(status);
            }
        }

        Ok(self.status())
    }

    fn set_output_device(&mut self, device_id: Option<String>) -> Result<PlaybackStatus, String> {
        let selected_output_device_id = normalize_output_device_id(device_id);
        if let Some(device_id) = &selected_output_device_id {
            find_output_device(device_id)?;
        }

        let previous_status = self.status();
        let current_path = self.current_path.clone();
        let current_title = self.current_title.clone();

        if let Some(player) = &self.player {
            player.stop();
        }
        self.player = None;
        self.sink = None;
        self.output_sample_rate = None;
        self.output_channels = None;
        self.output_sample_format = None;
        self.output_device_id = None;
        self.output_device_name = None;
        self.selected_output_device_id = selected_output_device_id;

        if let Some(path) = current_path {
            if matches!(previous_status.state.as_str(), "playing" | "paused") {
                let mut status = self.play_local_track(path, current_title)?;
                if previous_status.position_ms > 0 {
                    status = self.seek(previous_status.position_ms)?;
                }
                if previous_status.paused {
                    status = self.pause();
                }
                return Ok(status);
            }
        }

        Ok(self.status())
    }

    fn seek(&mut self, position_ms: u64) -> Result<PlaybackStatus, String> {
        let player = self
            .player
            .as_ref()
            .ok_or_else(|| "No local track is loaded.".to_string())?;
        if self.current_path.is_none() {
            return Err("No local track is loaded.".to_string());
        }

        let target_ms = self
            .duration_ms
            .map(|duration_ms| position_ms.min(duration_ms))
            .unwrap_or(position_ms);
        player
            .try_seek(Duration::from_millis(target_ms))
            .map_err(|error| format!("Could not seek in the current track: {error}"))?;
        Ok(self.status())
    }

    fn set_volume(&mut self, volume: f32) -> Result<PlaybackStatus, String> {
        if !volume.is_finite() {
            return Err("Volume must be a finite number.".to_string());
        }

        self.volume = clamp_volume(volume);
        if let Some(player) = &self.player {
            player.set_volume(self.volume);
        }
        Ok(self.status())
    }

    fn ensure_output(&mut self) -> Result<(), String> {
        if self.sink.is_some() && self.player.is_some() {
            return Ok(());
        }

        let (mut sink, output_device_id, output_device_name) =
            open_selected_sink(self.selected_output_device_id.as_deref())?;
        sink.log_on_drop(false);
        let config = *sink.config();
        let player = Player::connect_new(sink.mixer());
        player.set_volume(self.volume);

        self.output_sample_rate = Some(config.sample_rate().get());
        self.output_channels = Some(config.channel_count().get());
        self.output_sample_format = Some(format!("{:?}", config.sample_format()));
        self.output_device_id = output_device_id;
        self.output_device_name = output_device_name;
        self.player = Some(player);
        self.sink = Some(sink);
        Ok(())
    }

    fn status(&mut self) -> PlaybackStatus {
        self.refresh_gapless_state();
        self.status_snapshot()
    }

    fn status_snapshot(&self) -> PlaybackStatus {
        let has_track = self.current_path.is_some();
        let player = self.player.as_ref();
        let player_empty = player.map(|player| player.empty()).unwrap_or(true);
        let player_paused = player.map(|player| player.is_paused()).unwrap_or(false);
        let state = if !has_track {
            "idle"
        } else if self.stopped {
            "stopped"
        } else if player_empty {
            "ended"
        } else if player_paused {
            "paused"
        } else {
            "playing"
        };
        let position_ms = match state {
            "idle" | "stopped" => 0,
            "ended" => self.duration_ms.unwrap_or_else(|| {
                player
                    .map(|player| duration_to_ms(player.get_pos()))
                    .unwrap_or_default()
            }),
            _ => player
                .map(|player| duration_to_ms(player.get_pos()))
                .unwrap_or_default(),
        };
        let position_ms = self
            .duration_ms
            .map(|duration_ms| position_ms.min(duration_ms))
            .unwrap_or(position_ms);

        PlaybackStatus {
            state: state.to_string(),
            playing: state == "playing",
            paused: state == "paused",
            current_path: self.current_path.clone(),
            current_title: self.current_title.clone(),
            position_ms,
            duration_ms: self.duration_ms,
            volume: self.volume,
            source_format: self.source_format.clone(),
            source_is_lossless: self.source_is_lossless,
            source_sample_rate: self.source_sample_rate,
            source_channels: self.source_channels,
            output_sample_rate: self.output_sample_rate,
            output_channels: self.output_channels,
            output_sample_format: self.output_sample_format.clone(),
            output_device_id: self.output_device_id.clone(),
            output_device_name: self.output_device_name.clone(),
            quality_warnings: self.quality_warnings(),
            replay_gain_mode: self.replay_gain_mode.as_str().to_string(),
            replay_gain_db: self.replay_gain_db,
            replay_gain_source: self.replay_gain_source.clone(),
        }
    }

    fn quality_warnings(&self) -> Vec<String> {
        quality_warnings_for(
            self.source_sample_rate,
            self.source_channels,
            self.output_sample_rate,
            self.output_channels,
        )
    }

    fn refresh_gapless_state(&mut self) {
        if self.gapless_tracks.is_empty() || self.gapless_total_sources == 0 {
            return;
        }

        let Some(player) = &self.player else {
            return;
        };
        let remaining_sources = player.len();
        let expected_index = self
            .gapless_total_sources
            .saturating_sub(remaining_sources)
            .min(self.gapless_tracks.len());
        if expected_index <= self.gapless_index {
            return;
        }

        let previous_index = self.gapless_index;
        for completed_index in previous_index..expected_index {
            if let Some(completed_track) = self.gapless_tracks.get(completed_index).cloned() {
                self.gapless_events.push(PlaybackTransition {
                    event: "ended".to_string(),
                    status: self.status_for_track(
                        &completed_track,
                        "ended",
                        completed_track.duration_ms.unwrap_or_default(),
                    ),
                });
            }

            let started_index = completed_index + 1;
            if started_index < self.gapless_tracks.len() {
                if let Some(started_track) = self.gapless_tracks.get(started_index).cloned() {
                    self.gapless_events.push(PlaybackTransition {
                        event: "started".to_string(),
                        status: self.status_for_track(&started_track, "playing", 0),
                    });
                }
            }
        }

        if expected_index < self.gapless_tracks.len() {
            self.gapless_index = expected_index;
            self.apply_prepared_track(expected_index);
        } else {
            self.gapless_index = self.gapless_tracks.len();
            if let Some(last_track) = self.gapless_tracks.last().cloned() {
                self.apply_track_metadata(&last_track);
            }
        }
    }

    fn apply_prepared_track(&mut self, index: usize) {
        if let Some(track) = self.gapless_tracks.get(index).cloned() {
            self.apply_track_metadata(&track);
        }
    }

    fn apply_track_metadata(&mut self, track: &PreparedLocalTrack) {
        self.current_title = Some(track.title.clone());
        self.current_path = Some(track.path.clone());
        self.duration_ms = track.duration_ms;
        self.source_format = track.source_format.clone();
        self.source_is_lossless = track.source_is_lossless;
        self.source_sample_rate = track.source_sample_rate;
        self.source_channels = track.source_channels;
        self.replay_gain_db = track.replay_gain_db;
        self.replay_gain_source = track.replay_gain_source.clone();
    }

    fn status_for_track(
        &self,
        track: &PreparedLocalTrack,
        state: &str,
        position_ms: u64,
    ) -> PlaybackStatus {
        PlaybackStatus {
            state: state.to_string(),
            playing: state == "playing",
            paused: state == "paused",
            current_path: Some(track.path.clone()),
            current_title: Some(track.title.clone()),
            position_ms,
            duration_ms: track.duration_ms,
            volume: self.volume,
            source_format: track.source_format.clone(),
            source_is_lossless: track.source_is_lossless,
            source_sample_rate: track.source_sample_rate,
            source_channels: track.source_channels,
            output_sample_rate: self.output_sample_rate,
            output_channels: self.output_channels,
            output_sample_format: self.output_sample_format.clone(),
            output_device_id: self.output_device_id.clone(),
            output_device_name: self.output_device_name.clone(),
            quality_warnings: quality_warnings_for(
                track.source_sample_rate,
                track.source_channels,
                self.output_sample_rate,
                self.output_channels,
            ),
            replay_gain_mode: self.replay_gain_mode.as_str().to_string(),
            replay_gain_db: track.replay_gain_db,
            replay_gain_source: track.replay_gain_source.clone(),
        }
    }

    fn drain_gapless_events(&mut self) -> Vec<PlaybackTransition> {
        std::mem::take(&mut self.gapless_events)
    }

    fn clear_gapless_state(&mut self) {
        self.gapless_tracks.clear();
        self.gapless_index = 0;
        self.gapless_total_sources = 0;
        self.gapless_events.clear();
    }
}

impl ReplayGainMode {
    fn from_string(mode: &str) -> Result<Self, String> {
        match mode.trim().to_ascii_lowercase().as_str() {
            "off" => Ok(Self::Off),
            "track" => Ok(Self::Track),
            "album" => Ok(Self::Album),
            _ => Err("ReplayGain mode must be off, track, or album.".to_string()),
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::Track => "track",
            Self::Album => "album",
        }
    }

    fn applied_gain(self, tags: ReplayGainTags) -> AppliedReplayGain {
        match self {
            Self::Off => AppliedReplayGain::default(),
            Self::Track => AppliedReplayGain {
                gain_db: tags.track_gain_db.map(clamp_replay_gain_db),
                source: tags.track_gain_db.map(|_| "track"),
            },
            Self::Album => {
                if let Some(album_gain_db) = tags.album_gain_db {
                    AppliedReplayGain {
                        gain_db: Some(clamp_replay_gain_db(album_gain_db)),
                        source: Some("album"),
                    }
                } else {
                    AppliedReplayGain {
                        gain_db: tags.track_gain_db.map(clamp_replay_gain_db),
                        source: tags.track_gain_db.map(|_| "track"),
                    }
                }
            }
        }
    }
}

#[instrument]
pub fn play_gapless_local_tracks(
    tracks: Vec<LocalPlaybackTrack>,
) -> Result<PlaybackStatus, String> {
    tracing::info!(track_count = tracks.len(), "Starting gapless playback");
    let mut player = lock_player()?;
    player.play_gapless_tracks(tracks)
}

#[instrument]
pub fn playback_pause() -> Result<PlaybackStatus, String> {
    let mut player = lock_player()?;
    Ok(player.pause())
}

#[instrument]
pub fn playback_resume() -> Result<PlaybackStatus, String> {
    let mut player = lock_player()?;
    Ok(player.resume())
}

#[instrument]
pub fn playback_stop() -> Result<PlaybackStatus, String> {
    let mut player = lock_player()?;
    Ok(player.stop())
}

#[instrument]
pub fn playback_seek(position_ms: u64) -> Result<PlaybackStatus, String> {
    let mut player = lock_player()?;
    player.seek(position_ms)
}

pub fn set_playback_volume(volume: f32) -> Result<PlaybackStatus, String> {
    let mut player = lock_player()?;
    player.set_volume(volume)
}

pub fn set_audio_output_device(device_id: Option<String>) -> Result<PlaybackStatus, String> {
    let mut player = lock_player()?;
    player.set_output_device(device_id)
}

pub fn set_replay_gain_mode(mode: String) -> Result<PlaybackStatus, String> {
    let mut player = lock_player()?;
    player.set_replay_gain_mode(mode)
}

#[instrument]
pub fn get_playback_status() -> Result<PlaybackStatus, String> {
    let mut player = lock_player()?;
    Ok(player.status())
}

pub fn drain_playback_transitions() -> Result<Vec<PlaybackTransition>, String> {
    let mut player = lock_player()?;
    Ok(player.drain_gapless_events())
}

pub fn list_audio_output_devices() -> Result<Vec<AudioOutputDevice>, String> {
    let selected_output_device_id = {
        let player = lock_player()?;
        player.selected_output_device_id.clone()
    };
    output_devices(selected_output_device_id.as_deref())
}

fn lock_player() -> Result<MutexGuard<'static, AudioPlayer>, String> {
    AUDIO_PLAYER
        .get_or_init(|| Mutex::new(AudioPlayer::default()))
        .lock()
        .map_err(|_| "Audio player state is unavailable.".to_string())
}

fn normalize_audio_path(path: &str) -> Result<PathBuf, String> {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return Err("Track path is empty.".to_string());
    }

    let path = PathBuf::from(trimmed);
    if !path.is_file() {
        return Err(format!("Track path is not a file: {}", path.display()));
    }
    Ok(path)
}

fn prepare_local_track(
    track: &LocalPlaybackTrack,
    replay_gain_mode: ReplayGainMode,
) -> Result<(PreparedLocalTrack, Box<dyn Source + Send + 'static>), String> {
    let path = normalize_audio_path(&track.path)?;
    let file =
        File::open(&path).map_err(|error| format!("Could not open {}: {error}", path.display()))?;
    let source =
        Decoder::try_from(file).map_err(|error| format!("Could not decode audio file: {error}"))?;
    let duration_ms = source.total_duration().map(duration_to_ms);
    let source_format = source_format_from_path(&path);
    let source_is_lossless = source_format.as_deref().map(is_lossless_format);
    let source_sample_rate = Some(source.sample_rate().get());
    let source_channels = Some(source.channels().get());
    let replay_gain = replay_gain_mode.applied_gain(replay_gain_tags_from_path(&path));
    let replay_gain_factor = replay_gain
        .gain_db
        .map(replay_gain_db_to_linear)
        .unwrap_or(1.0);
    let title = track
        .title
        .as_deref()
        .and_then(|title| non_empty_string(title.trim()))
        .unwrap_or_else(|| title_from_path(&path));

    Ok((
        PreparedLocalTrack {
            path: path.to_string_lossy().to_string(),
            title,
            duration_ms,
            source_format,
            source_is_lossless,
            source_sample_rate,
            source_channels,
            replay_gain_db: replay_gain.gain_db,
            replay_gain_source: replay_gain.source.map(str::to_string),
        },
        Box::new(source.amplify(replay_gain_factor)),
    ))
}

fn title_from_path(path: &Path) -> String {
    path.file_stem()
        .and_then(|value| value.to_str())
        .filter(|value| !value.is_empty())
        .unwrap_or("Untitled")
        .to_string()
}

fn source_format_from_path(path: &Path) -> Option<String> {
    path.extension()
        .and_then(|extension| extension.to_str())
        .map(|extension| extension.to_ascii_uppercase())
}

fn is_lossless_format(format: &str) -> bool {
    matches!(format, "AIF" | "AIFF" | "ALAC" | "FLAC" | "WAV")
}

fn replay_gain_tags_from_path(path: &Path) -> ReplayGainTags {
    let Ok(tagged_file) = lofty::read_from_path(path) else {
        return ReplayGainTags::default();
    };
    let tag = tagged_file
        .primary_tag()
        .or_else(|| tagged_file.first_tag());
    let Some(tag) = tag else {
        return ReplayGainTags::default();
    };

    ReplayGainTags {
        track_gain_db: tag
            .get_string(ItemKey::ReplayGainTrackGain)
            .and_then(parse_replay_gain_db),
        album_gain_db: tag
            .get_string(ItemKey::ReplayGainAlbumGain)
            .and_then(parse_replay_gain_db),
    }
}

fn parse_replay_gain_db(value: &str) -> Option<f32> {
    let normalized = value
        .trim()
        .trim_end_matches("dB")
        .trim_end_matches("db")
        .trim();
    normalized.parse::<f32>().ok()
}

fn clamp_replay_gain_db(gain_db: f32) -> f32 {
    gain_db.clamp(-24.0, 12.0)
}

fn replay_gain_db_to_linear(gain_db: f32) -> f32 {
    10_f32.powf(gain_db / 20.0)
}

fn non_empty_string(value: &str) -> Option<String> {
    if value.is_empty() {
        None
    } else {
        Some(value.to_string())
    }
}

fn duration_to_ms(duration: Duration) -> u64 {
    u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
}

fn clamp_volume(volume: f32) -> f32 {
    volume.clamp(0.0, 2.0)
}

fn format_sample_rate(sample_rate: u32) -> String {
    if sample_rate % 1000 == 0 {
        format!("{} kHz", sample_rate / 1000)
    } else {
        format!("{:.1} kHz", sample_rate as f32 / 1000.0)
    }
}

fn quality_warnings_for(
    source_sample_rate: Option<u32>,
    source_channels: Option<u16>,
    output_sample_rate: Option<u32>,
    output_channels: Option<u16>,
) -> Vec<String> {
    let mut warnings = Vec::new();
    if let (Some(source_rate), Some(output_rate)) = (source_sample_rate, output_sample_rate) {
        if source_rate != output_rate {
            warnings.push(format!(
                "Output sample rate differs from source: {} -> {}",
                format_sample_rate(source_rate),
                format_sample_rate(output_rate)
            ));
        }
    }

    if let (Some(source_channels), Some(output_channels)) = (source_channels, output_channels) {
        if source_channels > output_channels {
            warnings.push(format!(
                "Output has fewer channels than source: {source_channels} -> {output_channels}"
            ));
        }
    }

    warnings
}

fn normalize_output_device_id(device_id: Option<String>) -> Option<String> {
    device_id
        .map(|device_id| device_id.trim().to_string())
        .filter(|device_id| !device_id.is_empty() && device_id != "default")
}

fn open_selected_sink(
    selected_output_device_id: Option<&str>,
) -> Result<(MixerDeviceSink, Option<String>, Option<String>), String> {
    if let Some(device_id) = selected_output_device_id {
        let (device, name) = find_output_device(device_id)?;
        let sink = DeviceSinkBuilder::from_device(device)
            .and_then(|builder| builder.open_stream())
            .map_err(|error| format!("Could not open selected audio output: {error}"))?;
        return Ok((sink, Some(device_id.to_string()), Some(name)));
    }

    let sink = DeviceSinkBuilder::open_default_sink()
        .map_err(|error| format!("Could not open the default audio output: {error}"))?;
    Ok((sink, None, default_output_device_name()))
}

fn output_devices(
    selected_output_device_id: Option<&str>,
) -> Result<Vec<AudioOutputDevice>, String> {
    let host = cpal::default_host();
    let mut devices = Vec::new();
    let default_config = host
        .default_output_device()
        .and_then(|device| device.default_output_config().ok());

    devices.push(AudioOutputDevice {
        id: "default".to_string(),
        name: default_output_device_name()
            .map(|name| format!("System default ({name})"))
            .unwrap_or_else(|| "System default".to_string()),
        selected: selected_output_device_id.is_none(),
        is_default: true,
        default_sample_rate: default_config.as_ref().map(|config| config.sample_rate()),
        default_channels: default_config.as_ref().map(|config| config.channels()),
        default_sample_format: default_config
            .as_ref()
            .map(|config| format!("{:?}", config.sample_format())),
    });

    let output_devices = host
        .output_devices()
        .map_err(|error| format!("Could not list audio output devices: {error}"))?;
    for (index, device) in output_devices.enumerate() {
        let name = device_name(&device, index);
        let id = output_device_key(&device, index, &name);
        let default_config = device.default_output_config().ok();
        devices.push(AudioOutputDevice {
            selected: selected_output_device_id == Some(id.as_str()),
            id,
            name,
            is_default: false,
            default_sample_rate: default_config.as_ref().map(|config| config.sample_rate()),
            default_channels: default_config.as_ref().map(|config| config.channels()),
            default_sample_format: default_config
                .as_ref()
                .map(|config| format!("{:?}", config.sample_format())),
        });
    }

    Ok(devices)
}

fn find_output_device(device_id: &str) -> Result<(cpal::Device, String), String> {
    let host = cpal::default_host();
    if let Ok(parsed_id) = device_id.parse::<cpal::DeviceId>() {
        if let Some(device) = host.device_by_id(&parsed_id) {
            if device.supports_output() {
                let name = device_name(&device, 0);
                return Ok((device, name));
            }
        }
    }

    let output_devices = host
        .output_devices()
        .map_err(|error| format!("Could not list audio output devices: {error}"))?;
    for (index, device) in output_devices.enumerate() {
        let name = device_name(&device, index);
        if output_device_key(&device, index, &name) == device_id {
            return Ok((device, name));
        }
    }

    Err(format!(
        "Audio output device is no longer available: {device_id}"
    ))
}

fn default_output_device_name() -> Option<String> {
    cpal::default_host()
        .default_output_device()
        .and_then(|device| {
            device
                .description()
                .ok()
                .map(|description| description.name().to_string())
        })
}

fn device_name(device: &cpal::Device, index: usize) -> String {
    device
        .description()
        .map(|description| description.name().to_string())
        .ok()
        .filter(|name| !name.trim().is_empty())
        .unwrap_or_else(|| format!("Output device {}", index + 1))
}

fn output_device_key(device: &cpal::Device, index: usize, name: &str) -> String {
    device
        .id()
        .map(|id| id.to_string())
        .unwrap_or_else(|_| output_device_id(index, name))
}

fn output_device_id(index: usize, name: &str) -> String {
    format!("{index}:{name}")
}

#[cfg(test)]
mod tests {
    use super::{
        ReplayGainMode, ReplayGainTags, clamp_volume, format_sample_rate, is_lossless_format,
        normalize_output_device_id, output_device_id, parse_replay_gain_db,
        replay_gain_db_to_linear, source_format_from_path, title_from_path,
    };
    use std::path::Path;

    #[test]
    fn volume_is_clamped_to_safe_range() {
        assert_eq!(clamp_volume(-0.5), 0.0);
        assert_eq!(clamp_volume(0.75), 0.75);
        assert_eq!(clamp_volume(3.0), 2.0);
    }

    #[test]
    fn title_uses_file_stem() {
        assert_eq!(
            title_from_path(Path::new("/music/album/Track 01.flac")),
            "Track 01"
        );
    }

    #[test]
    fn default_output_device_id_is_not_stored() {
        assert_eq!(
            normalize_output_device_id(Some("default".to_string())),
            None
        );
        assert_eq!(normalize_output_device_id(Some("".to_string())), None);
    }

    #[test]
    fn output_device_id_keeps_index_and_name() {
        assert_eq!(output_device_id(2, "USB DAC"), "2:USB DAC");
    }

    #[test]
    fn source_format_is_read_from_extension() {
        assert_eq!(
            source_format_from_path(Path::new("/music/album/Track 01.flac")).unwrap(),
            "FLAC"
        );
        assert!(is_lossless_format("FLAC"));
        assert!(!is_lossless_format("MP3"));
    }

    #[test]
    fn sample_rate_label_keeps_fractional_khz() {
        assert_eq!(format_sample_rate(44_100), "44.1 kHz");
        assert_eq!(format_sample_rate(48_000), "48 kHz");
    }

    #[test]
    fn replay_gain_values_parse_with_db_suffix() {
        assert_eq!(parse_replay_gain_db("+3.25 dB"), Some(3.25));
        assert_eq!(parse_replay_gain_db("-7.00 db"), Some(-7.0));
        assert_eq!(parse_replay_gain_db("not gain"), None);
    }

    #[test]
    fn replay_gain_db_converts_to_linear_factor() {
        assert!((replay_gain_db_to_linear(6.0) - 1.995).abs() < 0.01);
        assert!((replay_gain_db_to_linear(-6.0) - 0.501).abs() < 0.01);
    }

    #[test]
    fn album_replay_gain_falls_back_to_track_gain() {
        let applied = ReplayGainMode::Album.applied_gain(ReplayGainTags {
            track_gain_db: Some(-3.0),
            album_gain_db: None,
        });

        assert_eq!(applied.gain_db, Some(-3.0));
        assert_eq!(applied.source, Some("track"));
    }
}
