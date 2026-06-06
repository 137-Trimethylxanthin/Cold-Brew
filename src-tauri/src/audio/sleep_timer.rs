use lazy_static::lazy_static;
use serde::Serialize;
use std::sync::Mutex;
use tokio::sync::watch;

lazy_static! {
    static ref SLEEP_TIMER_STATE: Mutex<SleepTimerState> = Mutex::new(SleepTimerState::default());
    static ref SLEEP_TIMER_CANCEL: Mutex<Option<watch::Sender<bool>>> = Mutex::new(None);
}

#[derive(Clone, Debug, Serialize)]
pub struct SleepTimerState {
    pub active: bool,
    pub remaining_seconds: Option<u64>,
    pub total_seconds: Option<u64>,
}

impl Default for SleepTimerState {
    fn default() -> Self {
        Self {
            active: false,
            remaining_seconds: None,
            total_seconds: None,
        }
    }
}

#[tauri::command(rename_all = "snake_case")]
pub async fn set_sleep_timer(minutes: Option<u32>) -> Result<SleepTimerState, String> {
    // Cancel any existing timer
    {
        let mut cancel = SLEEP_TIMER_CANCEL
            .lock()
            .map_err(|e| format!("Sleep timer lock error: {e}"))?;
        if let Some(tx) = cancel.take() {
            let _ = tx.send(true);
        }
    }

    let Some(minutes) = minutes.filter(|&m| m > 0) else {
        let mut state = SLEEP_TIMER_STATE
            .lock()
            .map_err(|e| format!("Sleep timer lock error: {e}"))?;
        state.active = false;
        state.remaining_seconds = None;
        state.total_seconds = None;
        return Ok(state.clone());
    };

    let total_seconds = u64::from(minutes) * 60;
    let (cancel_tx, mut cancel_rx) = watch::channel(false);

    {
        let mut cancel = SLEEP_TIMER_CANCEL
            .lock()
            .map_err(|e| format!("Sleep timer lock error: {e}"))?;
        *cancel = Some(cancel_tx);
    }

    {
        let mut state = SLEEP_TIMER_STATE
            .lock()
            .map_err(|e| format!("Sleep timer lock error: {e}"))?;
        state.active = true;
        state.remaining_seconds = Some(total_seconds);
        state.total_seconds = Some(total_seconds);
    }

    tauri::async_runtime::spawn(async move {
        let fade_duration = 30u64.min(total_seconds);
        let pre_fade = total_seconds.saturating_sub(fade_duration);

        if pre_fade > 0 {
            let remaining = tokio::time::timeout(
                std::time::Duration::from_secs(pre_fade),
                cancel_rx.changed(),
            )
            .await;

            if remaining.is_err() {
                // Timeout expired normally, continue to fade
            } else {
                // Cancelled
                return;
            }
        }

        // Check cancellation again
        if *cancel_rx.borrow() {
            return;
        }

        // Save original volume
        let original_volume = crate::audio::player::get_playback_status()
            .map(|s| s.volume)
            .unwrap_or(1.0);

        let fade_steps: u64 = fade_duration.min(30);
        let step_duration = std::time::Duration::from_secs_f64(
            fade_duration as f64 / fade_steps as f64,
        );

        for step in 0..=fade_steps {
            if *cancel_rx.borrow() {
                let _ = crate::audio::player::set_playback_volume(original_volume);
                return;
            }

            let progress = step as f32 / fade_steps as f32;
            let target_volume = original_volume * (1.0 - progress);
            let _ = crate::audio::player::set_playback_volume(target_volume);

            {
                let elapsed = pre_fade + step * (fade_duration / fade_steps);
                let mut state = SLEEP_TIMER_STATE.lock().unwrap();
                state.remaining_seconds = Some(total_seconds.saturating_sub(elapsed));
            }

            if step < fade_steps {
                tokio::time::sleep(step_duration).await;
            }
        }

        // Stop playback
        let _ = crate::audio::player::playback_stop();

        // Restore original volume
        let _ = crate::audio::player::set_playback_volume(original_volume);

        let mut state = SLEEP_TIMER_STATE.lock().unwrap();
        state.active = false;
        state.remaining_seconds = None;
    });

    SLEEP_TIMER_STATE
        .lock()
        .map(|state| state.clone())
        .map_err(|e| format!("Sleep timer lock error: {e}"))
}

#[tauri::command(rename_all = "snake_case")]
pub fn get_sleep_timer_remaining() -> Result<SleepTimerState, String> {
    SLEEP_TIMER_STATE
        .lock()
        .map(|state| state.clone())
        .map_err(|e| format!("Sleep timer lock error: {e}"))
}
