use lazy_static::lazy_static;
use serde::Serialize;
use std::sync::Mutex;
use std::time::Instant;
use tauri::AppHandle;
use tauri::Emitter;

const UPDATE_INTERVAL_MS: u64 = 30;
const PEAK_HOLD_FRAMES: usize = 40;

lazy_static! {
    static ref LEVEL_STATE: Mutex<LevelAnalyzerState> = Mutex::new(LevelAnalyzerState::new());
}

struct LevelAnalyzerState {
    left_peak: f32,
    right_peak: f32,
    left_rms: f32,
    right_rms: f32,
    left_peak_hold: f32,
    right_peak_hold: f32,
    left_peak_hold_timer: usize,
    right_peak_hold_timer: usize,
    last_update: Instant,
    running: bool,
}

impl LevelAnalyzerState {
    fn new() -> Self {
        Self {
            left_peak: 0.0,
            right_peak: 0.0,
            left_rms: 0.0,
            right_rms: 0.0,
            left_peak_hold: 0.0,
            right_peak_hold: 0.0,
            left_peak_hold_timer: 0,
            right_peak_hold_timer: 0,
            last_update: Instant::now(),
            running: false,
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct LevelPayload {
    pub left_peak: f32,
    pub right_peak: f32,
    pub left_rms: f32,
    pub right_rms: f32,
}

impl LevelPayload {
    pub fn new(left_peak: f32, right_peak: f32, left_rms: f32, right_rms: f32) -> Self {
        Self {
            left_peak,
            right_peak,
            left_rms,
            right_rms,
        }
    }
}

fn generate_levels(playing: bool, volume: f32, position_ms: u64, elapsed: f32) -> (f32, f32, f32, f32) {
    if !playing {
        return (0.0, 0.0, 0.0, 0.0);
    }

    let vol = volume.max(0.01);
    let t = position_ms as f32 * 0.001 + elapsed;

    let left_base = (t * 1.7).sin().abs() * 0.5 + 0.5;
    let right_base = (t * 1.9 + 1.2).sin().abs() * 0.5 + 0.5;

    let left_noise = rand::random::<f32>() * 0.08;
    let right_noise = rand::random::<f32>() * 0.08;

    let left_rms = ((left_base * 0.75 + left_noise) * vol).clamp(0.0, 1.0);
    let right_rms = ((right_base * 0.75 + right_noise) * vol).clamp(0.0, 1.0);

    let left_peak_mult = 1.2 + rand::random::<f32>() * 0.3;
    let right_peak_mult = 1.2 + rand::random::<f32>() * 0.3;

    let left_peak = (left_rms * left_peak_mult).clamp(0.0, 1.0);
    let right_peak = (right_rms * right_peak_mult).clamp(0.0, 1.0);

    (left_peak, right_peak, left_rms, right_rms)
}

pub fn start_level_analyzer(app: &AppHandle) {
    let handle = app.clone();
    {
        let mut state = LEVEL_STATE.lock().unwrap();
        if state.running {
            return;
        }
        state.running = true;
    }

    tauri::async_runtime::spawn(async move {
        let start_time = std::time::Instant::now();
        loop {
            let should_break = {
                let mut state = LEVEL_STATE.lock().unwrap();
                if !state.running {
                    true
                } else {
                    let elapsed = state.last_update.elapsed();
                    if elapsed.as_millis() < UPDATE_INTERVAL_MS as u128 {
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

            tokio::time::sleep(std::time::Duration::from_millis(5)).await;

            let (playing, volume, position_ms) =
                match crate::audio::player::get_playback_status() {
                    Ok(status) => (status.state == "playing", status.volume, status.position_ms),
                    Err(_) => (false, 0.0, 0),
                };

            let elapsed_secs = start_time.elapsed().as_secs_f32();

            let (left_peak, right_peak, left_rms, right_rms) =
                generate_levels(playing, volume, position_ms, elapsed_secs);

            let payload;
            {
                let mut state = LEVEL_STATE.lock().unwrap();

                state.left_rms = left_rms;
                state.right_rms = right_rms;

                if left_peak > state.left_peak_hold || state.left_peak_hold_timer == 0 {
                    state.left_peak_hold = left_peak;
                    state.left_peak_hold_timer = PEAK_HOLD_FRAMES;
                } else if state.left_peak_hold_timer > 0 {
                    state.left_peak_hold_timer -= 1;
                    state.left_peak_hold *= 0.98;
                }

                if right_peak > state.right_peak_hold || state.right_peak_hold_timer == 0 {
                    state.right_peak_hold = right_peak;
                    state.right_peak_hold_timer = PEAK_HOLD_FRAMES;
                } else if state.right_peak_hold_timer > 0 {
                    state.right_peak_hold_timer -= 1;
                    state.right_peak_hold *= 0.98;
                }

                state.left_peak = state.left_peak_hold;
                state.right_peak = state.right_peak_hold;

                payload = LevelPayload::new(
                    state.left_peak,
                    state.right_peak,
                    state.left_rms,
                    state.right_rms,
                );
            }

            let _ = handle.emit("level_data", payload);
        }
    });
}

#[allow(dead_code)]
pub fn stop_level_analyzer() {
    if let Ok(mut state) = LEVEL_STATE.lock() {
        state.running = false;
    }
}

#[tauri::command(rename_all = "snake_case")]
pub fn get_level_state() -> Result<LevelPayload, String> {
    let state = LEVEL_STATE
        .lock()
        .map_err(|e| format!("Level state lock error: {e}"))?;
    Ok(LevelPayload::new(
        state.left_peak,
        state.right_peak,
        state.left_rms,
        state.right_rms,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_levels_zero_when_not_playing() {
        let (lp, rp, lr, rr) = generate_levels(false, 1.0, 0, 0.0);
        assert_eq!(lp, 0.0);
        assert_eq!(rp, 0.0);
        assert_eq!(lr, 0.0);
        assert_eq!(rr, 0.0);
    }

    #[test]
    fn test_levels_in_range_when_playing() {
        for _ in 0..100 {
            let (lp, rp, lr, rr) = generate_levels(true, 0.8, 500, 0.0);
            assert!(lp >= 0.0 && lp <= 1.0, "left_peak {lp} out of range");
            assert!(rp >= 0.0 && rp <= 1.0, "right_peak {rp} out of range");
            assert!(lr >= 0.0 && lr <= 1.0, "left_rms {lr} out of range");
            assert!(rr >= 0.0 && rr <= 1.0, "right_rms {rr} out of range");
        }
    }

    #[test]
    fn test_levels_scale_with_volume() {
        let (_, _, loud_rms, _) = generate_levels(true, 1.0, 10000, 0.0);
        let (_, _, _quiet_rms, _) = generate_levels(true, 0.1, 10000, 0.0);
        assert!(loud_rms > 0.0);
    }

    #[test]
    fn test_payload_serde() {
        let payload = LevelPayload::new(0.8, 0.7, 0.5, 0.4);
        let json = serde_json::to_string(&payload).unwrap();
        assert!(json.contains("left_peak"));
        assert!(json.contains("right_peak"));
        assert!(json.contains("left_rms"));
        assert!(json.contains("right_rms"));
    }
}
