use std::sync::{Arc, Mutex, OnceLock};

use tauri::{AppHandle, Wry};

use crate::timer::TimerEngine;
use crate::types::{LanPeer, LanSelf, LanStatus, TimerSnapshot};

#[derive(Default)]
struct LanInner {
    device_id: String,
    device_name: String,
    searching: bool,
    available: bool,
    peers: Vec<LanPeer>,
    message: String,
    error: Option<String>,
}

pub struct LanService {
    inner: Mutex<LanInner>,
    timer: Arc<TimerEngine>,
    app: OnceLock<AppHandle<Wry>>,
}

impl LanService {
    pub fn new(timer: Arc<TimerEngine>) -> Self {
        Self {
            inner: Mutex::new(LanInner {
                device_id: String::new(),
                device_name: "Fretec StudyHall".to_string(),
                searching: false,
                available: true,
                peers: Vec::new(),
                message: "未开始搜索".to_string(),
                error: None,
            }),
            timer,
            app: OnceLock::new(),
        }
    }

    pub fn attach(&self, app: AppHandle<Wry>) {
        let _ = self.app.set(app);
    }

    pub fn configure(&self, id: String, name: String) {
        let mut inner = self.inner.lock().expect("lan lock");
        inner.device_id = id;
        inner.device_name = name;
    }

    pub fn update_name(&self, name: String) {
        let mut inner = self.inner.lock().expect("lan lock");
        inner.device_name = name;
    }

    pub fn status(&self) -> LanStatus {
        let inner = self.inner.lock().expect("lan lock");
        let running = self.timer.snapshot().status != crate::types::TimerStatus::Idle;
        LanStatus {
            available: inner.available,
            searching: inner.searching,
            peers: inner.peers.clone(),
            self_info: LanSelf {
                id: inner.device_id.clone(),
                name: inner.device_name.clone(),
            },
            message: inner.message.clone(),
            can_connect: !running,
            error: inner.error.clone(),
        }
    }

    pub fn start_search(&self) {}

    pub fn stop(&self) {}

    pub fn connect(&self, _peer_id: &str) {}

    pub fn disconnect(&self, _peer_id: &str) {}

    pub fn broadcast_timer(&self, _snapshot: &TimerSnapshot) {}
}
