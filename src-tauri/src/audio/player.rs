use std::fs::File;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard, OnceLock};
use std::time::Duration;

use lofty::file::TaggedFileExt;
use lofty::tag::ItemKey;
use rand::seq::SliceRandom;
use rand::thread_rng;
use rodio::cpal;
use rodio::cpal::traits::{DeviceTrait, HostTrait};
use rodio::{Decoder, DeviceSinkBuilder, MixerDeviceSink, Player, Source};
use serde::{Deserialize, Serialize};
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
pub struct PlaybackSettings {
    pub crossfade_duration_ms: Option<u64>,
    pub playback_speed: f32,
    pub mono_downmix: bool,
    pub preamp_gain_db: f32,
    pub replay_gain_mode: String,
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
    crossfade_duration_ms: Option<u64>,
    crossfade_old_player: Option<Player>,
    crossfade_old_sink: Option<MixerDeviceSink>,
    crossfade_start: Option<std::time::Instant>,
    crossfade_total_ms: u64,
    playback_speed: f32,
    mono_downmix: bool,
    preamp_gain_db: f32,
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
            crossfade_duration_ms: None,
            crossfade_old_player: None,
            crossfade_old_sink: None,
            crossfade_start: None,
            crossfade_total_ms: 0,
            playback_speed: 1.0,
            mono_downmix: false,
            preamp_gain_db: 0.0,
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

        let settings = self.playback_settings_snapshot();
        let mut prepared_tracks: Vec<_> = Vec::new();
        for track in &tracks {
            match prepare_local_track(track, self.replay_gain_mode, &settings) {
                Ok(pt) => prepared_tracks.push(pt),
                Err(err) => {
                    tracing::warn!(
                        path = %track.path,
                        error = %err,
                        "Skipping corrupted audio file"
                    );
                }
            }
        }

        if prepared_tracks.is_empty() {
            return Err(
                "No playable tracks found — all files in the batch failed to decode.".to_string(),
            );
        }

        let crossfade_ms = self.crossfade_duration_ms.filter(|&d| d > 0);
        let has_current =
            self.current_path.is_some() && self.player.as_ref().is_some_and(|p| !p.empty());

        if let Some(duration_ms) = crossfade_ms {
            if has_current {
                return self.crossfade_transition(prepared_tracks, duration_ms);
            }
        }

        self.crossfade_old_player = None;
        self.crossfade_old_sink = None;
        self.crossfade_start = None;
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

    fn crossfade_transition(
        &mut self,
        prepared_tracks: Vec<(
            PreparedLocalTrack,
            Box<dyn Source<Item = f32> + Send + 'static>,
        )>,
        duration_ms: u64,
    ) -> Result<PlaybackStatus, String> {
        let old_sink = self.sink.take();
        let old_player = self.player.take();

        if let (Some(old_sink), Some(old_player)) = (old_sink, old_player) {
            self.crossfade_old_player = Some(old_player);
            self.crossfade_old_sink = Some(old_sink);
            self.crossfade_start = Some(std::time::Instant::now());
            self.crossfade_total_ms = duration_ms;
        }

        self.output_sample_rate = None;
        self.output_channels = None;
        self.output_sample_format = None;
        self.output_device_id = None;
        self.output_device_name = None;

        self.ensure_output()?;
        let new_player = self
            .player
            .as_ref()
            .ok_or_else(|| "Audio output was not initialized.".to_string())?;
        new_player.set_volume(if self.crossfade_old_player.is_some() {
            0.0
        } else {
            self.volume
        });
        let mut gapless_tracks = Vec::with_capacity(prepared_tracks.len());
        for (track, source) in prepared_tracks {
            new_player.append(source);
            gapless_tracks.push(track);
        }
        new_player.play();

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
        self.crossfade_old_player = None;
        self.crossfade_old_sink = None;
        self.crossfade_start = None;
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
        self.update_crossfade();
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

    fn update_crossfade(&mut self) {
        let Some(start) = self.crossfade_start else {
            return;
        };
        let elapsed = start.elapsed();
        let total = Duration::from_millis(self.crossfade_total_ms);

        if elapsed >= total {
            if let Some(old_player) = self.crossfade_old_player.take() {
                old_player.set_volume(0.0);
                old_player.stop();
            }
            self.crossfade_old_sink = None;
            self.crossfade_start = None;
            if let Some(player) = &self.player {
                player.set_volume(self.volume);
            }
            return;
        }

        let progress = elapsed.as_secs_f32() / total.as_secs_f32();
        let old_vol = self.volume * (1.0 - progress);
        let new_vol = self.volume * progress;

        if let Some(old_player) = &self.crossfade_old_player {
            old_player.set_volume(old_vol);
        }
        if let Some(player) = &self.player {
            player.set_volume(new_vol);
        }
    }

    fn playback_settings_snapshot(&self) -> PlaybackSettings {
        PlaybackSettings {
            crossfade_duration_ms: self.crossfade_duration_ms,
            playback_speed: self.playback_speed,
            mono_downmix: self.mono_downmix,
            preamp_gain_db: self.preamp_gain_db,
            replay_gain_mode: self.replay_gain_mode.as_str().to_string(),
        }
    }

    fn set_crossfade(&mut self, duration_ms: Option<u64>) -> PlaybackStatus {
        self.crossfade_duration_ms = duration_ms;
        self.status()
    }

    fn set_playback_speed_inner(&mut self, speed: f32) -> Result<PlaybackStatus, String> {
        if !speed.is_finite() || !(0.5..=2.0).contains(&speed) {
            return Err("Playback speed must be between 0.5 and 2.0.".to_string());
        }

        let previous_status = self.status();
        let current_path = self.current_path.clone();
        let current_title = self.current_title.clone();
        self.playback_speed = speed;

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

    fn set_mono_downmix_inner(&mut self, enabled: bool) -> Result<PlaybackStatus, String> {
        if self.mono_downmix == enabled {
            return Ok(self.status());
        }

        let previous_status = self.status();
        let current_path = self.current_path.clone();
        let current_title = self.current_title.clone();
        self.mono_downmix = enabled;

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

    fn set_preamp_gain_inner(&mut self, db: f32) -> Result<PlaybackStatus, String> {
        if !db.is_finite() || !(-12.0..=12.0).contains(&db) {
            return Err("Preamp gain must be between -12.0 and 12.0 dB.".to_string());
        }

        let previous_status = self.status();
        let current_path = self.current_path.clone();
        let current_title = self.current_title.clone();
        self.preamp_gain_db = db;

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

pub fn set_crossfade(duration_ms: Option<u64>) -> Result<PlaybackStatus, String> {
    let mut player = lock_player()?;
    let capped = duration_ms.map(|d| d.min(12_000));
    Ok(player.set_crossfade(capped))
}

pub fn get_playback_settings() -> Result<PlaybackSettings, String> {
    let player = lock_player()?;
    Ok(player.playback_settings_snapshot())
}

pub fn set_playback_speed(speed: f32) -> Result<PlaybackStatus, String> {
    let mut player = lock_player()?;
    player.set_playback_speed_inner(speed)
}

pub fn set_mono_downmix(enabled: bool) -> Result<PlaybackStatus, String> {
    let mut player = lock_player()?;
    player.set_mono_downmix_inner(enabled)
}

pub fn set_preamp_gain(db: f32) -> Result<PlaybackStatus, String> {
    let mut player = lock_player()?;
    player.set_preamp_gain_inner(db)
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
    settings: &PlaybackSettings,
) -> Result<
    (
        PreparedLocalTrack,
        Box<dyn Source<Item = f32> + Send + 'static>,
    ),
    String,
> {
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

    let mut source: Box<dyn Source<Item = f32> + Send + 'static> =
        Box::new(source.amplify(replay_gain_factor));

    if settings.preamp_gain_db != 0.0 {
        let preamp_factor = 10_f32.powf(settings.preamp_gain_db / 20.0);
        source = Box::new(source.amplify(preamp_factor));
    }

    if settings.mono_downmix && source.channels().get() > 1 {
        source = Box::new(MonoDownmix::new(source));
    }

    if (settings.playback_speed - 1.0).abs() > f32::EPSILON {
        source = Box::new(source.speed(settings.playback_speed));
    }

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
        source,
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

struct MonoDownmix<I> {
    input: I,
}

impl<I> MonoDownmix<I> {
    fn new(input: I) -> Self {
        Self { input }
    }
}

impl<I> Iterator for MonoDownmix<I>
where
    I: Iterator<Item = f32> + Send + 'static,
{
    type Item = f32;

    fn next(&mut self) -> Option<Self::Item> {
        let left = self.input.next()?;
        let right = self.input.next().unwrap_or(left);
        Some((left + right) / 2.0)
    }
}

impl<I> Source for MonoDownmix<I>
where
    I: Source<Item = f32> + Send + 'static,
{
    fn current_span_len(&self) -> Option<usize> {
        self.input.current_span_len()
    }

    fn channels(&self) -> std::num::NonZero<u16> {
        std::num::NonZero::new(1).unwrap()
    }

    fn sample_rate(&self) -> std::num::NonZero<u32> {
        self.input.sample_rate()
    }

    fn total_duration(&self) -> Option<Duration> {
        self.input.total_duration()
    }

    fn try_seek(&mut self, pos: Duration) -> Result<(), rodio::source::SeekError> {
        self.input.try_seek(pos)
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
        clamp_volume, format_sample_rate, is_lossless_format, normalize_output_device_id,
        output_device_id, parse_replay_gain_db, replay_gain_db_to_linear, source_format_from_path,
        title_from_path, ReplayGainMode, ReplayGainTags,
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

use std::collections::{HashMap, VecDeque};

use async_trait::async_trait;
use ezsockets::CloseFrame;
use ezsockets::Error;
use ezsockets::Server;
use lazy_static::lazy_static;
use serde_json::{json, Value};
use std::net::SocketAddr;

lazy_static! {
    static ref QUEUE_MANAGER: Mutex<QueueManager> = Mutex::new(QueueManager::new());
}

const DEFAULT_QUEUE_ID: &str = "test";

const SKIP_HISTORY_MAX: usize = 20;
const PLAYED_HISTORY_MAX: usize = 50;

#[derive(Clone, Debug, Serialize)]
pub struct QueueHistoryEntry {
    pub song: Song,
    pub played_at: String,
}

struct Queue {
    current_song: Song,
    old: VecDeque<Song>,
    upcoming: VecDeque<Song>,
    skip_history: VecDeque<Song>,
    played_history: VecDeque<QueueHistoryEntry>,
}

impl Queue {
    fn new() -> Self {
        Self {
            current_song: Song {
                id: "".to_string(),
                title: "".to_string(),
                artist: "".to_string(),
                album: "".to_string(),
                duration: 0,
                source: None,
                uri: None,
                external_url: None,
                quality: None,
                playable: None,
            },
            old: VecDeque::new(),
            upcoming: VecDeque::new(),
            skip_history: VecDeque::new(),
            played_history: VecDeque::new(),
        }
    }

    fn has_current_song(&self) -> bool {
        !self.current_song.id.is_empty()
    }

    fn add_song(&mut self, song: Song) {
        self.upcoming.push_back(song);
    }

    fn remove_song(&mut self, song: Song) {
        self.upcoming.retain(|x| x.id != song.id);
    }

    fn move_upcoming_song(&mut self, from_index: usize, to_index: usize) -> Result<(), String> {
        let length = self.upcoming.len();
        if from_index >= length || to_index >= length {
            return Err(format!(
                "Queue move is out of range: {from_index} to {to_index} for {length} upcoming tracks."
            ));
        }
        if from_index == to_index {
            return Ok(());
        }

        let song = self
            .upcoming
            .remove(from_index)
            .ok_or_else(|| "Queued track could not be moved.".to_string())?;
        self.upcoming.insert(to_index, song);
        Ok(())
    }

    fn next_song(&mut self) {
        if self.upcoming.is_empty() {
            return;
        }
        if self.has_current_song() {
            let skipped = self.current_song.clone();
            self.push_skip_history(skipped);
            self.old.push_back(self.current_song.clone());
        }
        self.current_song = self.upcoming.pop_front().unwrap();
        self.record_now_playing();
    }

    fn get_current_song(&self) -> Song {
        self.current_song.clone()
    }

    fn previous_song(&mut self) {
        if self.old.is_empty() {
            return;
        }
        self.upcoming.push_front(self.current_song.clone());
        self.current_song = self.old.pop_back().unwrap();
    }

    fn advance_to_song_id(&mut self, song_id: &str) {
        if self.current_song.id == song_id {
            return;
        }

        while let Some(next_song) = self.upcoming.pop_front() {
            if self.has_current_song() {
                self.old.push_back(self.current_song.clone());
            }
            let matched = next_song.id == song_id;
            self.current_song = next_song;
            if matched {
                return;
            }
        }
    }

    fn push_skip_history(&mut self, song: Song) {
        if song.id.is_empty() {
            return;
        }
        self.skip_history.push_back(song);
        if self.skip_history.len() > SKIP_HISTORY_MAX {
            self.skip_history.pop_front();
        }
    }

    fn undo_last_skip(&mut self) -> Option<Song> {
        let song = self.skip_history.pop_back()?;
        self.upcoming.push_front(song.clone());
        Some(song)
    }

    fn record_now_playing(&mut self) {
        let song = self.current_song.clone();
        if song.id.is_empty() {
            return;
        }
        let now = chrono_now_iso();
        self.played_history.push_back(QueueHistoryEntry {
            song,
            played_at: now,
        });
        if self.played_history.len() > PLAYED_HISTORY_MAX {
            self.played_history.pop_front();
        }
    }

    fn shuffle_upcoming(&mut self) {
        let mut upcoming: Vec<Song> = self.upcoming.drain(..).collect();
        let mut rng = thread_rng();
        upcoming.shuffle(&mut rng);
        for song in upcoming {
            self.upcoming.push_back(song);
        }
    }

    fn played_history(&self) -> Vec<QueueHistoryEntry> {
        self.played_history.iter().cloned().collect()
    }
}

struct QueueManager {
    queues: HashMap<String, Queue>,
}

impl QueueManager {
    fn new() -> Self {
        Self {
            queues: HashMap::new(),
        }
    }

    fn queue_exists(&self, id: &str) -> bool {
        self.queues.contains_key(id)
    }

    fn create_queue(&mut self, id: &str) {
        self.queues.insert(id.to_string(), Queue::new());
    }

    fn get_queue(&mut self, id: &str) -> &mut Queue {
        if !self.queue_exists(id) {
            self.create_queue(id);
        }
        self.queues.get_mut(id).unwrap()
    }

    fn add_song_to_queue(&mut self, id: &str, song: Song) {
        if !self.queue_exists(id) {
            self.create_queue(id);
        }
        self.queues.get_mut(id).unwrap().add_song(song);
    }

    fn remove_song_from_queue(&mut self, id: &str, song: Song) {
        if self.queue_exists(id) {
            self.queues.get_mut(id).unwrap().remove_song(song);
        }
    }

    fn move_song_in_queue(
        &mut self,
        id: &str,
        from_index: usize,
        to_index: usize,
    ) -> Result<(), String> {
        self.get_queue(id).move_upcoming_song(from_index, to_index)
    }

    fn undo_last_skip(&mut self, id: &str) -> Option<Song> {
        self.get_queue(id).undo_last_skip()
    }

    fn shuffle_queue(&mut self, id: &str) {
        self.get_queue(id).shuffle_upcoming();
    }

    fn played_history(&self, id: &str) -> Vec<QueueHistoryEntry> {
        if let Some(queue) = self.queues.get(id) {
            queue.played_history()
        } else {
            Vec::new()
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
pub struct Song {
    pub id: String,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub duration: usize,
    #[serde(default)]
    pub source: Option<String>,
    #[serde(default)]
    pub uri: Option<String>,
    #[serde(default)]
    pub external_url: Option<String>,
    #[serde(default)]
    pub quality: Option<String>,
    #[serde(default)]
    pub playable: Option<bool>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct QueueSnapshot {
    pub current_song: Option<Song>,
    pub old: Vec<Song>,
    pub upcoming: Vec<Song>,
}

impl Queue {
    fn snapshot(&self) -> QueueSnapshot {
        QueueSnapshot {
            current_song: self.has_current_song().then(|| self.current_song.clone()),
            old: self.old.iter().cloned().collect(),
            upcoming: self.upcoming.iter().cloned().collect(),
        }
    }
}

#[instrument]
pub fn queue_song(song: Song) -> Result<QueueSnapshot, String> {
    let mut queue_manager = lock_queue_manager()?;
    queue_manager.add_song_to_queue(DEFAULT_QUEUE_ID, song);
    Ok(queue_manager.get_queue(DEFAULT_QUEUE_ID).snapshot())
}

#[instrument]
pub fn remove_song(song: Song) -> Result<QueueSnapshot, String> {
    let mut queue_manager = lock_queue_manager()?;
    queue_manager.remove_song_from_queue(DEFAULT_QUEUE_ID, song);
    Ok(queue_manager.get_queue(DEFAULT_QUEUE_ID).snapshot())
}

#[instrument]
pub fn move_song(from_index: usize, to_index: usize) -> Result<QueueSnapshot, String> {
    let mut queue_manager = lock_queue_manager()?;
    queue_manager.move_song_in_queue(DEFAULT_QUEUE_ID, from_index, to_index)?;
    Ok(queue_manager.get_queue(DEFAULT_QUEUE_ID).snapshot())
}

pub fn get_queue_snapshot() -> Result<QueueSnapshot, String> {
    let mut queue_manager = lock_queue_manager()?;
    Ok(queue_manager.get_queue(DEFAULT_QUEUE_ID).snapshot())
}

#[instrument]
pub fn next_queue_song() -> Result<QueueSnapshot, String> {
    let mut queue_manager = lock_queue_manager()?;
    let queue = queue_manager.get_queue(DEFAULT_QUEUE_ID);
    queue.next_song();
    Ok(queue.snapshot())
}

#[instrument]
pub fn previous_queue_song() -> Result<QueueSnapshot, String> {
    let mut queue_manager = lock_queue_manager()?;
    let queue = queue_manager.get_queue(DEFAULT_QUEUE_ID);
    queue.previous_song();
    Ok(queue.snapshot())
}

#[instrument]
pub fn advance_to_song_id(song_id: &str) -> Result<QueueSnapshot, String> {
    let mut queue_manager = lock_queue_manager()?;
    let queue = queue_manager.get_queue(DEFAULT_QUEUE_ID);
    queue.advance_to_song_id(song_id);
    Ok(queue.snapshot())
}

#[instrument]
pub fn undo_last_skip() -> Result<QueueSnapshot, String> {
    let mut queue_manager = lock_queue_manager()?;
    let restored = queue_manager.undo_last_skip(DEFAULT_QUEUE_ID);
    if restored.is_none() {
        return Err("No skipped tracks to restore.".to_string());
    }
    Ok(queue_manager.get_queue(DEFAULT_QUEUE_ID).snapshot())
}

pub fn queue_history() -> Result<Vec<QueueHistoryEntry>, String> {
    let queue_manager = lock_queue_manager()?;
    Ok(queue_manager.played_history(DEFAULT_QUEUE_ID))
}

#[instrument]
pub fn shuffle_queue() -> Result<QueueSnapshot, String> {
    let mut queue_manager = lock_queue_manager()?;
    queue_manager.shuffle_queue(DEFAULT_QUEUE_ID);
    Ok(queue_manager.get_queue(DEFAULT_QUEUE_ID).snapshot())
}

#[instrument]
pub fn move_queue_item(from: usize, to: usize) -> Result<QueueSnapshot, String> {
    let mut queue_manager = lock_queue_manager()?;
    queue_manager.move_song_in_queue(DEFAULT_QUEUE_ID, from, to)?;
    Ok(queue_manager.get_queue(DEFAULT_QUEUE_ID).snapshot())
}

#[instrument]
pub fn play_track_now(song: Song) -> Result<QueueSnapshot, String> {
    let mut queue_manager = lock_queue_manager()?;
    let queue = queue_manager.get_queue(DEFAULT_QUEUE_ID);

    if queue.has_current_song() && queue.current_song.id != song.id {
        let current = queue.current_song.clone();
        queue.old.push_back(current);
    }

    queue.upcoming.retain(|s| s.id != song.id);
    queue.current_song = song;

    Ok(queue.snapshot())
}

fn lock_queue_manager() -> Result<MutexGuard<'static, QueueManager>, String> {
    QUEUE_MANAGER
        .lock()
        .map_err(|_| "Queue state is unavailable.".to_string())
}

// Web socket start
type SessionID = u16;
type Session = ezsockets::Session<SessionID, ()>;

//server
struct MusicServer {}
#[async_trait]
impl ezsockets::ServerExt for MusicServer {
    type Session = MusicSession;
    type Call = ();

    async fn on_connect(
        &mut self,
        socket: ezsockets::Socket,
        _request: ezsockets::Request,
        address: SocketAddr,
    ) -> Result<Session, Option<CloseFrame>> {
        let id = address.port();
        let session = Session::create(|handle| MusicSession { id, handle }, id, socket);
        Ok(session)
    }

    async fn on_disconnect(
        &mut self,
        _id: <Self::Session as ezsockets::SessionExt>::ID,
        _reason: Result<Option<CloseFrame>, Error>,
    ) -> Result<(), Error> {
        Ok(())
    }

    async fn on_call(&mut self, call: Self::Call) -> Result<(), Error> {
        let () = call;
        Ok(())
    }
}

//Session
struct MusicSession {
    handle: Session,
    id: SessionID,
}

#[async_trait]
impl ezsockets::SessionExt for MusicSession {
    type ID = SessionID;
    type Call = ();

    fn id(&self) -> &Self::ID {
        &self.id
    }

    async fn on_text(&mut self, text: String) -> Result<(), Error> {
        //parse the json
        let jason: Value = serde_json::from_str(&text).unwrap();
        println!("Received text: {}", jason);
        //best way to handle rquest like play, pause would be with a if and then a match statement
        if !jason["command"].is_null() && !jason["song"].is_null() {
            let command = jason["command"].as_str().unwrap();
            println!("Command: {}", command);
            if command == "/add" {
                let song = value_to_song(jason["song"].clone());
                let _ = self
                    .handle
                    .text(format!("{} added to queue", song.title))
                    .unwrap();
                let mut queue_manager = QUEUE_MANAGER.lock().unwrap();
                queue_manager.add_song_to_queue(DEFAULT_QUEUE_ID, song);
                let _ = self.handle.text(get_queue(&mut queue_manager)).unwrap();
                drop(queue_manager);
            } else if command == "/remove" {
                let song = value_to_song(jason["song"].clone());
                let mut queue_manager = QUEUE_MANAGER.lock().unwrap();
                queue_manager.remove_song_from_queue(DEFAULT_QUEUE_ID, song);
                let _ = self.handle.text(get_queue(&mut queue_manager)).unwrap();
                drop(queue_manager);
            } else if command == "/next" {
                let mut queue_manager = QUEUE_MANAGER.lock().unwrap();
                let queue = queue_manager.get_queue(DEFAULT_QUEUE_ID);
                queue.next_song();
                let _ = self.handle.text(get_queue(&mut queue_manager)).unwrap();
                drop(queue_manager);
            } else if command == "/previous" {
                let mut queue_manager = QUEUE_MANAGER.lock().unwrap();
                let queue = queue_manager.get_queue(DEFAULT_QUEUE_ID);
                queue.previous_song();
                let _ = self.handle.text(get_queue(&mut queue_manager)).unwrap();
                drop(queue_manager);
            } else if command == "/get_queue" {
                let mut queue_manager = QUEUE_MANAGER.lock().unwrap();
                let queue = queue_manager.get_queue(DEFAULT_QUEUE_ID);
                let current_song = queue.get_current_song();
                let _ = self
                    .handle
                    .text(
                        json!({
                            "current_song": current_song,
                            "upcoming": queue.upcoming,
                            "old": queue.old
                        })
                        .to_string(),
                    )
                    .unwrap();
                drop(queue_manager);
            } else {
                let _ = self.handle.text("Invalid command").unwrap();
            }
        } else {
            let _ = self.handle.text("Invalid command").unwrap();
        }
        Ok(())
    }

    async fn on_binary(&mut self, _bytes: Vec<u8>) -> Result<(), Error> {
        unimplemented!()
    }

    async fn on_call(&mut self, _call: Self::Call) -> Result<(), Error> {
        Ok(())
    }
}

fn get_queue(queue_manager: &mut MutexGuard<QueueManager>) -> String {
    let queue = queue_manager.get_queue(DEFAULT_QUEUE_ID);
    let current_song = queue.get_current_song();
    json!({
        "current_song": current_song,
        "upcoming": queue.upcoming,
        "old": queue.old
    })
    .to_string()
}

//WS end :)
pub async fn run() {
    //start a new async thread that does not block the main thread
    let (server, _) = Server::create(|_server| MusicServer {});
    ezsockets::tungstenite::run(server, "127.0.0.1:6969")
        .await
        .unwrap();
}

fn value_to_song(value: Value) -> Song {
    let id = value["id"].as_str().unwrap_or("None");
    let title = value["title"].as_str().unwrap_or("NoTitle");
    let artist = value["artist"].as_str().unwrap_or("NoArtist");
    let album = value["album"].as_str().unwrap_or("NoAlbum");
    let duration = value["duration"].as_u64().unwrap_or(0) as usize;
    Song {
        id: id.to_string(),
        title: title.to_string(),
        artist: artist.to_string(),
        album: album.to_string(),
        duration,
        source: optional_string(&value["source"]),
        uri: optional_string(&value["uri"]),
        external_url: optional_string(&value["external_url"]),
        quality: optional_string(&value["quality"]),
        playable: value["playable"].as_bool(),
    }
}

fn optional_string(value: &Value) -> Option<String> {
    value
        .as_str()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn chrono_now_iso() -> String {
    use std::time::SystemTime;
    let dur = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default();
    let secs = dur.as_secs();
    let days_since_epoch = secs / 86400;
    let time_of_day = secs % 86400;
    let hours = time_of_day / 3600;
    let minutes = (time_of_day % 3600) / 60;
    let seconds = time_of_day % 60;
    let year = 1970 + (days_since_epoch / 365);
    let month = ((days_since_epoch % 365) / 30) + 1;
    let day = (days_since_epoch % 365) % 30 + 1;
    format!("{year:04}-{month:02}-{day:02}T{hours:02}:{minutes:02}:{seconds:02}Z")
}

//3, 2, 4, 6, 2, 1 >=< 18, 9==4,

#[cfg(test)]
mod queue_tests {
    use super::{Queue, Song};

    fn song(id: &str) -> Song {
        Song {
            id: id.to_string(),
            title: format!("Song {id}"),
            artist: "Artist".to_string(),
            album: "Album".to_string(),
            duration: 100,
            source: None,
            uri: None,
            external_url: None,
            quality: None,
            playable: None,
        }
    }

    #[test]
    fn previous_song_returns_the_immediate_history_item() {
        let mut queue = Queue::new();
        queue.add_song(song("1"));
        queue.add_song(song("2"));

        queue.next_song();
        queue.next_song();
        queue.previous_song();

        assert_eq!(queue.current_song.id, "1");
        assert_eq!(queue.upcoming.front().unwrap().id, "2");
    }

    #[test]
    fn remove_song_deletes_matching_upcoming_id() {
        let mut queue = Queue::new();
        queue.add_song(song("1"));
        queue.add_song(song("2"));

        queue.remove_song(song("1"));

        assert_eq!(queue.upcoming.len(), 1);
        assert_eq!(queue.upcoming.front().unwrap().id, "2");
    }

    #[test]
    fn move_upcoming_song_reorders_by_index() {
        let mut queue = Queue::new();
        queue.add_song(song("1"));
        queue.add_song(song("2"));
        queue.add_song(song("3"));

        queue.move_upcoming_song(2, 0).unwrap();

        let ids = queue
            .upcoming
            .iter()
            .map(|queued_song| queued_song.id.as_str())
            .collect::<Vec<_>>();
        assert_eq!(ids, vec!["3", "1", "2"]);
    }

    #[test]
    fn move_upcoming_song_rejects_out_of_range_indexes() {
        let mut queue = Queue::new();
        queue.add_song(song("1"));

        let result = queue.move_upcoming_song(0, 1);

        assert!(result.is_err());
        assert_eq!(queue.upcoming.front().unwrap().id, "1");
    }

    #[test]
    fn snapshot_exposes_current_history_and_upcoming() {
        let mut queue = Queue::new();
        queue.add_song(song("1"));
        queue.add_song(song("2"));
        queue.next_song();

        let snapshot = queue.snapshot();

        assert_eq!(snapshot.current_song.unwrap().id, "1");
        assert!(snapshot.old.is_empty());
        assert_eq!(snapshot.upcoming.len(), 1);
        assert_eq!(snapshot.upcoming[0].id, "2");
    }

    #[test]
    fn advance_to_song_id_moves_upcoming_tracks_into_history() {
        let mut queue = Queue::new();
        queue.add_song(song("1"));
        queue.add_song(song("2"));
        queue.add_song(song("3"));

        queue.next_song();
        queue.advance_to_song_id("3");

        assert_eq!(queue.current_song.id, "3");
        assert_eq!(
            queue
                .old
                .iter()
                .map(|queued_song| queued_song.id.as_str())
                .collect::<Vec<_>>(),
            vec!["1", "2"]
        );
        assert!(queue.upcoming.is_empty());
    }
}
