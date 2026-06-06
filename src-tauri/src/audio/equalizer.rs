use lazy_static::lazy_static;
use serde::{Deserialize, Serialize};
use std::sync::Mutex;

const EQ_BANDS: usize = 10;

lazy_static! {
    static ref EQ_STATE: Mutex<EqualizerState> = Mutex::new(EqualizerState::default());
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EqualizerState {
    pub bands: Vec<f32>,
    pub preset_name: String,
}

impl Default for EqualizerState {
    fn default() -> Self {
        Self {
            bands: vec![0.0; EQ_BANDS],
            preset_name: "Flat".to_string(),
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct EqPreset {
    pub name: String,
    pub bands: Vec<f32>,
}

pub fn list_presets() -> Vec<EqPreset> {
    vec![
        EqPreset {
            name: "Flat".to_string(),
            bands: vec![0.0; EQ_BANDS],
        },
        EqPreset {
            name: "Rock".to_string(),
            bands: vec![3.0, 2.0, 1.0, 0.0, -1.0, 0.0, 1.0, 2.0, 2.0, 2.0],
        },
        EqPreset {
            name: "Jazz".to_string(),
            bands: vec![2.0, 1.0, 0.0, -1.0, -1.0, 0.0, 1.0, 1.0, 0.0, -1.0],
        },
        EqPreset {
            name: "Classical".to_string(),
            bands: vec![-2.0, -1.0, 0.0, 1.0, 1.0, 0.0, -1.0, 0.0, 1.0, 1.0],
        },
        EqPreset {
            name: "Vocal".to_string(),
            bands: vec![0.0, 0.0, 1.0, 3.0, 3.0, 2.0, 1.0, 0.0, 0.0, 0.0],
        },
        EqPreset {
            name: "Bass Boost".to_string(),
            bands: vec![6.0, 5.0, 3.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
        },
        EqPreset {
            name: "Treble Boost".to_string(),
            bands: vec![0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 2.0, 4.0, 5.0, 6.0],
        },
    ]
}

#[tauri::command(rename_all = "snake_case")]
pub fn get_eq_state() -> Result<EqualizerState, String> {
    EQ_STATE
        .lock()
        .map(|state| state.clone())
        .map_err(|e| format!("EQ state lock error: {e}"))
}

#[tauri::command(rename_all = "snake_case")]
pub fn set_eq_band(index: usize, gain_db: f32) -> Result<EqualizerState, String> {
    if index >= EQ_BANDS {
        return Err(format!("EQ band index {index} is out of range (0-{})", EQ_BANDS - 1));
    }
    let clamped = gain_db.clamp(-12.0, 12.0);
    let mut state = EQ_STATE
        .lock()
        .map_err(|e| format!("EQ state lock error: {e}"))?;
    state.bands[index] = clamped;

    // TODO: Apply biquad filters to audio output.
    // For now, EQ state is stored but not applied to the audio pipeline.
    // Future implementation: compute biquad coefficients from band gains,
    // apply via a series of Biquad<f32> filters chained on the rodio source.

    Ok(state.clone())
}

#[tauri::command(rename_all = "snake_case")]
pub fn set_eq_preset(preset_name: String) -> Result<EqualizerState, String> {
    let presets = list_presets();
    let preset = presets
        .iter()
        .find(|p| p.name.eq_ignore_ascii_case(&preset_name))
        .ok_or_else(|| {
            let names: Vec<_> = presets.iter().map(|p| p.name.as_str()).collect();
            format!("Unknown EQ preset '{}'. Available: {}", preset_name, names.join(", "))
        })?;

    let mut state = EQ_STATE
        .lock()
        .map_err(|e| format!("EQ state lock error: {e}"))?;
    state.bands = preset.bands.clone();
    state.preset_name = preset.name.clone();

    Ok(state.clone())
}

#[tauri::command(rename_all = "snake_case")]
pub fn list_eq_presets() -> Result<Vec<EqPreset>, String> {
    Ok(list_presets())
}
