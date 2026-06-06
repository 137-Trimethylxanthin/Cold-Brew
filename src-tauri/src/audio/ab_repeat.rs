use lazy_static::lazy_static;
use serde::Serialize;
use std::sync::Mutex;

lazy_static! {
    static ref AB_REPEAT_STATE: Mutex<AbRepeatState> = Mutex::new(AbRepeatState::default());
}

#[derive(Clone, Debug, Serialize, Default)]
pub struct AbRepeatState {
    pub active: bool,
    pub loop_start_secs: Option<f64>,
    pub loop_end_secs: Option<f64>,
}

#[tauri::command(rename_all = "snake_case")]
pub fn set_ab_repeat_a(position_secs: Option<f64>) -> Result<AbRepeatState, String> {
    let mut state = AB_REPEAT_STATE
        .lock()
        .map_err(|e| format!("AB Repeat lock error: {e}"))?;

    if let Some(secs) = position_secs {
        state.loop_start_secs = Some(secs.max(0.0));
    } else {
        // Use current playback position
        if let Ok(status) = crate::audio::player::get_playback_status() {
            state.loop_start_secs = Some(status.position_ms as f64 / 1000.0);
        }
    }

    // If both A and B are set, activate looping
    if state.loop_start_secs.is_some() && state.loop_end_secs.is_some() {
        state.active = true;
    }

    Ok(state.clone())
}

#[tauri::command(rename_all = "snake_case")]
pub fn set_ab_repeat_b(position_secs: Option<f64>) -> Result<AbRepeatState, String> {
    let mut state = AB_REPEAT_STATE
        .lock()
        .map_err(|e| format!("AB Repeat lock error: {e}"))?;

    if let Some(secs) = position_secs {
        state.loop_end_secs = Some(secs.max(0.0));
    } else if let Ok(status) = crate::audio::player::get_playback_status() {
        state.loop_end_secs = Some(status.position_ms as f64 / 1000.0);
    }

    // Validate: end must be after start
    if let (Some(start), Some(end)) = (state.loop_start_secs, state.loop_end_secs) {
        if end <= start {
            state.loop_end_secs = None;
            return Err("B marker must be after A marker.".to_string());
        }
        state.active = true;
    }

    Ok(state.clone())
}

#[tauri::command(rename_all = "snake_case")]
pub fn clear_ab_repeat() -> Result<AbRepeatState, String> {
    let mut state = AB_REPEAT_STATE
        .lock()
        .map_err(|e| format!("AB Repeat lock error: {e}"))?;
    state.active = false;
    state.loop_start_secs = None;
    state.loop_end_secs = None;
    Ok(state.clone())
}

#[tauri::command(rename_all = "snake_case")]
pub fn get_ab_repeat() -> Result<AbRepeatState, String> {
    AB_REPEAT_STATE
        .lock()
        .map(|state| state.clone())
        .map_err(|e| format!("AB Repeat lock error: {e}"))
}

/// Called from the playback loop to check if we need to seek back to A.
/// Returns true if a seek-back was performed.
pub fn check_and_handle_ab_repeat() -> Result<bool, String> {
    let state = AB_REPEAT_STATE
        .lock()
        .map_err(|e| format!("AB Repeat lock error: {e}"))?;

    if !state.active {
        return Ok(false);
    }

    let (Some(start), Some(end)) = (state.loop_start_secs, state.loop_end_secs) else {
        return Ok(false);
    };

    if end <= start {
        return Ok(false);
    }

    drop(state);

    let status = crate::audio::player::get_playback_status()?;
    let position_secs = status.position_ms as f64 / 1000.0;

    if position_secs >= end - 0.15 {
        let seek_target = (start * 1000.0) as u64;
        let _ = crate::audio::player::playback_seek(seek_target);
        return Ok(true);
    }

    Ok(false)
}
