use lazy_static::lazy_static;
use serde::{Deserialize, Serialize};
use std::sync::Mutex;

lazy_static! {
    static ref CROSSFEED_STATE: Mutex<CrossfeedState> = Mutex::new(CrossfeedState::default());
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CrossfeedState {
    pub level: String,
}

impl Default for CrossfeedState {
    fn default() -> Self {
        Self {
            level: "off".to_string(),
        }
    }
}

#[tauri::command(rename_all = "snake_case")]
pub fn get_crossfeed() -> Result<CrossfeedState, String> {
    CROSSFEED_STATE
        .lock()
        .map(|state| state.clone())
        .map_err(|e| format!("Crossfeed state lock error: {e}"))
}

#[tauri::command(rename_all = "snake_case")]
pub fn set_crossfeed(level: String) -> Result<CrossfeedState, String> {
    let normalized = level.trim().to_ascii_lowercase();
    if !matches!(normalized.as_str(), "off" | "light" | "strong") {
        return Err(format!(
            "Invalid crossfeed level '{}'. Must be 'off', 'light', or 'strong'.",
            level
        ));
    }
    let mut state = CROSSFEED_STATE
        .lock()
        .map_err(|e| format!("Crossfeed state lock error: {e}"))?;
    state.level = normalized;
    Ok(state.clone())
}
