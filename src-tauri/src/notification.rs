use tauri::{AppHandle, Wry};

use crate::types::TimerSnapshot;

/// 桌面端没有系统实时通知，保持空实现。
#[cfg(not(target_os = "android"))]
pub fn update(_app: &AppHandle<Wry>, _snapshot: &TimerSnapshot) {}

#[cfg(not(target_os = "android"))]
pub fn stop(_app: &AppHandle<Wry>) {}

#[cfg(target_os = "android")]
mod android {
    use std::sync::atomic::{AtomicU8, Ordering};

    use tauri::{AppHandle, Manager, Wry};

    use crate::types::{Phase, TimerSnapshot, TimerStatus};
    use tauri_plugin_studyhall_native::{StudyHallHandle, TimerPayload};

    static LAST_ACTIVE: AtomicU8 = AtomicU8::new(0);

    fn payload(snapshot: &TimerSnapshot) -> TimerPayload {
        TimerPayload {
            phase: match snapshot.phase {
                Phase::Focus => "focus".to_string(),
                Phase::Break => "break".to_string(),
            },
            total_ms: snapshot.total_ms,
            started_at_ms: snapshot.started_at_ms,
            pause_offset_ms: snapshot.pause_offset_ms,
            paused_at_ms: snapshot.paused_at_ms,
            status: match snapshot.status {
                TimerStatus::Idle => "idle".to_string(),
                TimerStatus::Running => "running".to_string(),
                TimerStatus::Paused => "paused".to_string(),
            },
        }
    }

    pub fn update(app: &AppHandle<Wry>, snapshot: &TimerSnapshot) {
        let Some(handle) = app.try_state::<StudyHallHandle<Wry>>() else {
            return;
        };
        let active = snapshot.status != TimerStatus::Idle;
        let was_active = LAST_ACTIVE.swap(u8::from(active), Ordering::Relaxed) == 1;
        if !active {
            if was_active {
                handle.stop();
            }
            return;
        }
        let payload = payload(snapshot);
        if was_active {
            handle.update(&payload);
        } else {
            handle.start(&payload);
        }
    }

    pub fn stop(app: &AppHandle<Wry>) {
        LAST_ACTIVE.store(0, Ordering::Relaxed);
        if let Some(handle) = app.try_state::<StudyHallHandle<Wry>>() {
            handle.stop();
        }
    }
}

#[cfg(target_os = "android")]
pub use android::{stop, update};
