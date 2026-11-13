use tauri::{AppHandle, Wry};

use crate::types::TimerSnapshot;

/// 桌面端没有系统实时通知，保持空实现。
pub fn update(_app: &AppHandle<Wry>, _snapshot: &TimerSnapshot) {}

pub fn stop(_app: &AppHandle<Wry>) {}
