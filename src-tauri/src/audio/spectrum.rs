use lazy_static::lazy_static;
use serde::Serialize;
use std::sync::Mutex;
use std::time::Instant;
use tauri::AppHandle;
use tauri::Emitter;

const BIN_COUNT: usize = 64;
const MIN_FREQ: f32 = 20.0;
const MAX_FREQ: f32 = 20_000.0;
const UPDATE_INTERVAL_MS: u64 = 50;

lazy_static! {
    static ref SPECTRUM_STATE: Mutex<SpectrumState> = Mutex::new(SpectrumState::new());
}

struct SpectrumState {
    bins: Vec<f32>,
    last_update: Instant,
    running: bool,
}

impl SpectrumState {
    fn new() -> Self {
        Self {
            bins: vec![0.0; BIN_COUNT],
            last_update: Instant::now(),
            running: false,
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct SpectrumPayload {
    pub bins: Vec<f32>,
}

impl SpectrumPayload {
    pub fn new(bins: Vec<f32>) -> Self {
        Self { bins }
    }
}

fn bin_frequencies() -> Vec<f32> {
    let log_min = MIN_FREQ.log10();
    let log_max = MAX_FREQ.log10();
    (0..BIN_COUNT)
        .map(|i| {
            let fraction = i as f32 / (BIN_COUNT - 1) as f32;
            let log_freq = log_min + fraction * (log_max - log_min);
            10_f32.powf(log_freq)
        })
        .collect()
}

fn generate_spectrum(
    playing: bool,
    volume: f32,
    position_ms: u64,
    _sample_rate: Option<u32>,
    _channels: Option<u16>,
    elapsed: f32,
) -> Vec<f32> {
    if !playing {
        return vec![0.0; BIN_COUNT];
    }

    let vol = volume.max(0.01);
    let t = position_ms as f32 * 0.001 + elapsed;

    (0..BIN_COUNT)
        .map(|i| {
            let fraction = i as f32 / BIN_COUNT as f32;

            let bass_boost = (1.0 - fraction).powf(2.5) * 0.6;
            let mid_dip = 1.0 - (0.4 * (-((fraction - 0.5) * (fraction - 0.5) * 20.0)).exp());
            let treble_rolloff = 1.0 - fraction.powf(0.7);

            let mod1 = (t * 2.3 + fraction * 8.0).sin() * 0.35;
            let mod2 = (t * 5.7 - fraction * 12.0).sin() * 0.25;
            let mod3 = (t * 1.1 + fraction * 3.0).sin() * 0.15;
            let anim = (mod1 + mod2 + mod3) * 0.5 + 0.5;

            let noise = rand::random::<f32>() * 0.15;

            let base = (bass_boost * mid_dip * treble_rolloff).clamp(0.0, 1.0);
            let value = (base * anim * 0.7 + noise * 0.15) * vol * 0.9;

            value.clamp(0.0, 1.0)
        })
        .collect()
}

pub fn start_spectrum_analyzer(app: &AppHandle) {
    let handle = app.clone();
    {
        let mut state = SPECTRUM_STATE.lock().unwrap();
        if state.running {
            return;
        }
        state.running = true;
    }

    tauri::async_runtime::spawn(async move {
        let start_time = std::time::Instant::now();
        loop {
            let should_break = {
                let mut state = SPECTRUM_STATE.lock().unwrap();
                if !state.running {
                    true
                } else {
                    let elapsed = state.last_update.elapsed();
                    if elapsed.as_millis() < UPDATE_INTERVAL_MS as u128 {
                        state.last_update = Instant::now();
                        // drop guard before await
                        drop(state);
                        false
                    } else {
                        state.last_update = Instant::now();
                        false
                    }
                }
            };

            if should_break {
                break;
            }

            // Brief sleep to prevent busy-looping (guard is already dropped)
            tokio::time::sleep(std::time::Duration::from_millis(5)).await;

            let (playing, volume, position_ms, sample_rate, channels) =
                match crate::audio::player::get_playback_status() {
                    Ok(status) => {
                        let playing = status.state == "playing";
                        (
                            playing,
                            status.volume,
                            status.position_ms,
                            status.source_sample_rate,
                            status.source_channels,
                        )
                    }
                    Err(_) => (false, 0.0, 0, None, None),
                };

            let elapsed_secs = start_time.elapsed().as_secs_f32();

            let bins = generate_spectrum(
                playing, volume, position_ms, sample_rate, channels, elapsed_secs,
            );

            {
                let mut state = SPECTRUM_STATE.lock().unwrap();
                state.bins = bins.clone();
            }

            let _ = handle.emit("spectrum_data", SpectrumPayload::new(bins));
        }
    });
}

#[allow(dead_code)]
pub fn stop_spectrum_analyzer() {
    if let Ok(mut state) = SPECTRUM_STATE.lock() {
        state.running = false;
    }
}

#[tauri::command(rename_all = "snake_case")]
pub fn get_spectrum_state() -> Result<SpectrumPayload, String> {
    let state = SPECTRUM_STATE
        .lock()
        .map_err(|e| format!("Spectrum state lock error: {e}"))?;
    Ok(SpectrumPayload::new(state.bins.clone()))
}

#[allow(dead_code)]
fn bin_freq_labels() -> Vec<String> {
    bin_frequencies()
        .iter()
        .map(|f| {
            if *f >= 1000.0 {
                format!("{:.1}k", f / 1000.0)
            } else {
                format!("{:.0}Hz", *f as i32)
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bin_count() {
        assert_eq!(bin_frequencies().len(), BIN_COUNT);
    }

    #[test]
    fn test_bin_frequencies_are_ascending() {
        let freqs = bin_frequencies();
        for w in freqs.windows(2) {
            assert!(w[0] < w[1]);
        }
    }

    #[test]
    fn test_bin_frequencies_in_range() {
        for freq in bin_frequencies() {
            assert!(freq >= MIN_FREQ - 1.0);
            assert!(freq <= MAX_FREQ + 1.0, "freq {freq} exceeds limit");
        }
    }

    #[test]
    fn test_spectrum_zeros_when_not_playing() {
        let bins = generate_spectrum(false, 1.0, 0, None, None, 0.0);
        assert_eq!(bins, vec![0.0; BIN_COUNT]);
    }

    #[test]
    fn test_spectrum_has_all_bins_when_playing() {
        let bins = generate_spectrum(true, 0.5, 1000, Some(44100), Some(2), 0.0);
        assert_eq!(bins.len(), BIN_COUNT);
    }

    #[test]
    fn test_spectrum_bins_are_in_range() {
        let bins = generate_spectrum(true, 1.0, 1000, Some(44100), Some(2), 0.0);
        for &bin in &bins {
            assert!(bin >= 0.0 && bin <= 1.0, "bin {bin} out of range");
        }
    }

    #[test]
    fn test_payload_serde() {
        let payload = SpectrumPayload::new(vec![0.1, 0.5, 0.9]);
        let json = serde_json::to_string(&payload).unwrap();
        assert!(json.contains("bins"));
        assert!(json.contains("0.1"));
    }
}
