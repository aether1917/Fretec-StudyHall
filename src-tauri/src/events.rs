use tauri::{AppHandle, Emitter, Wry};

use crate::types::{LanStatus, TimerSnapshot};

pub fn emit_timer(app: &AppHandle<Wry>, snapshot: &TimerSnapshot) {
    let _ = app.emit("timer://state", snapshot);
}

pub fn emit_lan(app: &AppHandle<Wry>, status: &LanStatus) {
    let _ = app.emit("lan://status", status);
}

pub fn emit_auth_expired(app: &AppHandle<Wry>, source: &str) {
    let _ = app.emit("auth://expired", serde_json::json!({ "source": source }));
}
