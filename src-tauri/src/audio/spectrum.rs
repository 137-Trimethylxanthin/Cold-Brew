use lazy_static::lazy_static;
use num_complex::Complex;
use rustfft::FftPlanner;
use serde::Serialize;
use std::sync::Mutex;
use tauri::AppHandle;
use tauri::Emitter;

use crate::audio::spectrum_tap::AudioRingBuffer;

const BIN_COUNT: usize = 64;
const MIN_FREQ: f32 = 20.0;
const MAX_FREQ: f32 = 20_000.0;
const FFT_SIZE: usize = 2048;
const UPDATE_INTERVAL_MS: u64 = 40;
const SMOOTHING_ALPHA: f32 = 0.45;

lazy_static! {
    pub static ref AUDIO_RING: std::sync::Arc<AudioRingBuffer> = AudioRingBuffer::new();
    static ref SPECTRUM_STATE: Mutex<SpectrumState> = Mutex::new(SpectrumState::new());
}

struct SpectrumState {
    prev_bins: Vec<f32>,
    running: bool,
}

impl SpectrumState {
    fn new() -> Self {
        Self {
            prev_bins: vec![0.0; BIN_COUNT],
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

fn bin_center_frequencies() -> Vec<f32> {
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

fn hann_window(size: usize) -> Vec<f32> {
    (0..size)
        .map(|i| 0.5 * (1.0 - (2.0 * std::f32::consts::PI * i as f32 / (size - 1) as f32).cos()))
        .collect()
}

fn compute_real_fft(samples: &[f32]) -> Vec<f32> {
    let n = samples.len();
    if n == 0 {
        return vec![];
    }

    let window = hann_window(n);

    let mut planner = FftPlanner::new();
    let fft = planner.plan_fft_forward(n);

    let mut complex: Vec<Complex<f32>> = samples
        .iter()
        .enumerate()
        .map(|(i, &s)| Complex::new(s * window[i], 0.0))
        .collect();

    fft.process(&mut complex);

    let half = n / 2;
    complex[..half]
        .iter()
        .map(|c| c.norm() / n as f32)
        .collect()
}

fn map_magnitudes_to_bins(magnitudes: &[f32], sample_rate: u32) -> Vec<f32> {
    let bin_freqs = bin_center_frequencies();
    let fft_bin_count = magnitudes.len();
    let nyquist = sample_rate as f32 / 2.0;
    let fft_resolution = nyquist / fft_bin_count as f32;

    let mut bins = vec![0.0f32; BIN_COUNT];

    for (mag_idx, mag) in magnitudes.iter().enumerate() {
        let freq = mag_idx as f32 * fft_resolution;
        for (bin_idx, &center_freq) in bin_freqs.iter().enumerate() {
            let half_range = if bin_idx == 0 {
                (bin_freqs[1] - center_freq) / 2.0
            } else if bin_idx == BIN_COUNT - 1 {
                (center_freq - bin_freqs[bin_idx - 1]) / 2.0
            } else {
                let lower = (center_freq - bin_freqs[bin_idx - 1]) / 2.0;
                let upper = (bin_freqs[bin_idx + 1] - center_freq) / 2.0;
                lower.min(upper)
            };
            if freq >= center_freq - half_range && freq < center_freq + half_range {
                bins[bin_idx] += mag;
            }
        }
    }

    bins
}

fn normalize_bins(bins: &[f32]) -> Vec<f32> {
    let max = bins.iter().cloned().fold(0.0f32, f32::max);
    if max < 1e-10 {
        return vec![0.0; bins.len()];
    }

    let dynamic_max = max.max(0.005);
    bins.iter()
        .map(|&v| {
            let normalized = v / dynamic_max;
            (normalized * normalized.sqrt().powf(0.4)).clamp(0.0, 1.0)
        })
        .collect()
}

fn apply_smoothing(current: &[f32], previous: &[f32]) -> Vec<f32> {
    current
        .iter()
        .zip(previous.iter())
        .map(|(&c, &p)| p * (1.0 - SMOOTHING_ALPHA) + c * SMOOTHING_ALPHA)
        .collect()
}

fn analyze_spectrum(samples: &[f32], sample_rate: u32) -> Vec<f32> {
    if samples.len() < FFT_SIZE {
        return vec![];
    }

    let fft_input: Vec<f32> = samples[samples.len() - FFT_SIZE..].to_vec();
    let magnitudes = compute_real_fft(&fft_input);
    let raw_bins = map_magnitudes_to_bins(&magnitudes, sample_rate);
    let normalized = normalize_bins(&raw_bins);

    let mut state = SPECTRUM_STATE.lock().unwrap();
    let smoothed = apply_smoothing(&normalized, &state.prev_bins);
    state.prev_bins = smoothed.clone();

    smoothed
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

    AUDIO_RING.clear();

    tauri::async_runtime::spawn(async move {
        loop {
            tokio::time::sleep(std::time::Duration::from_millis(UPDATE_INTERVAL_MS)).await;

            let should_break = {
                let state = SPECTRUM_STATE.lock().unwrap();
                !state.running
            };
            if should_break {
                break;
            }

            let sample_rate = match AUDIO_RING.sample_rate() {
                Some(sr) => sr,
                None => continue,
            };

            let samples = AUDIO_RING.drain(FFT_SIZE * 2);
            if samples.len() < FFT_SIZE {
                continue;
            }

            let bins = analyze_spectrum(&samples, sample_rate);
            if bins.is_empty() {
                continue;
            }

            let _ = handle.emit("spectrum_data", SpectrumPayload::new(bins));
        }
    });
}

#[allow(dead_code)]
pub fn stop_spectrum_analyzer() {
    if let Ok(mut state) = SPECTRUM_STATE.lock() {
        state.running = false;
        state.prev_bins = vec![0.0; BIN_COUNT];
    }
    AUDIO_RING.clear();
}

#[tauri::command(rename_all = "snake_case")]
pub fn get_spectrum_state() -> Result<SpectrumPayload, String> {
    Ok(SpectrumPayload::new(vec![0.0; BIN_COUNT]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bin_count() {
        assert_eq!(bin_center_frequencies().len(), BIN_COUNT);
    }

    #[test]
    fn test_bin_frequencies_are_ascending() {
        let freqs = bin_center_frequencies();
        for w in freqs.windows(2) {
            assert!(w[0] < w[1]);
        }
    }

    #[test]
    fn test_bin_frequencies_in_range() {
        for freq in bin_center_frequencies() {
            assert!(freq >= MIN_FREQ - 1.0);
            assert!(freq <= MAX_FREQ + 1.0, "freq {freq} exceeds limit");
        }
    }

    #[test]
    fn test_hann_window_length() {
        assert_eq!(hann_window(FFT_SIZE).len(), FFT_SIZE);
    }

    #[test]
    fn test_hann_window_endpoints_zero() {
        let w = hann_window(FFT_SIZE);
        assert!(w[0] < 1e-6);
        assert!(w[FFT_SIZE - 1] < 1e-6);
    }

    #[test]
    fn test_fft_sine_wave() {
        let sample_rate = 44100u32;
        let freq = 440.0;
        let samples: Vec<f32> = (0..FFT_SIZE)
            .map(|i| (2.0 * std::f32::consts::PI * freq * i as f32 / sample_rate as f32).sin())
            .collect();

        let magnitudes = compute_real_fft(&samples);
        assert_eq!(magnitudes.len(), FFT_SIZE / 2);

        let peak_idx = (freq * FFT_SIZE as f32 / sample_rate as f32).round() as usize;
        let peak_mag = magnitudes[peak_idx];
        let surrounding: Vec<f32> = magnitudes
            .iter()
            .enumerate()
            .filter(|(i, _)| i.saturating_sub(peak_idx) > 5)
            .map(|(_, &m)| m)
            .collect();
        let avg_surrounding: f32 = surrounding.iter().sum::<f32>() / surrounding.len() as f32;
        assert!(
            peak_mag > avg_surrounding * 5.0,
            "peak {peak_mag} should dominate surrounding avg {avg_surrounding}"
        );
    }

    #[test]
    fn test_normalize_empty() {
        let result = normalize_bins(&vec![0.0; 64]);
        assert!(result.iter().all(|&v| v < 0.001));
    }

    #[test]
    fn test_normalize_preserves_relative() {
        let input: Vec<f32> = (0..64).map(|i| i as f32 / 63.0).collect();
        let result = normalize_bins(&input);
        assert!(result[0] < result[63]);
        assert!(result.iter().all(|&v| v >= 0.0 && v <= 1.0));
    }

    #[test]
    fn test_smoothing_identity() {
        let bins = vec![0.5; 64];
        let smoothed = apply_smoothing(&bins, &bins);
        for (i, &v) in smoothed.iter().enumerate() {
            assert!((v - 0.5).abs() < 0.001, "bin {i} drifted to {v}");
        }
    }
}
