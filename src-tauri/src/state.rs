use std::sync::{Arc, Mutex, OnceLock};

use tauri::{AppHandle, Wry};

use crate::bili::BiliRuntime;
use crate::lan::LanService;
use crate::media::MediaService;
use crate::netease::NeteaseRuntime;
use crate::storage::{AppPaths, AppSettings, AuthStore, DeviceInfo, FlowStore};
use crate::types::{CaptchaAnswer, Source, TimerSnapshot};
use crate::timer::TimerEngine;

pub struct AppState {
    app: OnceLock<AppHandle<Wry>>,
    pub paths: AppPaths,
    pub flow: Mutex<FlowStore>,
    pub auth: Mutex<AuthStore>,
    pub device: Mutex<DeviceInfo>,
    pub settings: Mutex<AppSettings>,
    pub http: reqwest::Client,
    pub timer: Arc<TimerEngine>,
    pub media: MediaService,
    pub lan: Arc<LanService>,
    pub bili: Mutex<BiliRuntime>,
    pub netease: Mutex<NeteaseRuntime>,
    pub captcha: Mutex<Option<CaptchaAnswer>>,
}

impl AppState {
    pub fn new(paths: AppPaths) -> Self {
        let http = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(20))
            .build()
            .expect("http client");
        let flow = paths.load_flow();
        let auth = paths.load_auth();
        let device = paths.load_device();
        let settings = paths.load_settings();
        let timer = Arc::new(TimerEngine::new());
        Self {
            app: OnceLock::new(),
            paths: paths.clone(),
            flow: Mutex::new(flow),
            auth: Mutex::new(auth),
            device: Mutex::new(device),
            settings: Mutex::new(settings),
            http,
            timer: timer.clone(),
            media: MediaService::new(),
            lan: Arc::new(LanService::new(timer)),
            bili: Mutex::new(BiliRuntime::default()),
            netease: Mutex::new(NeteaseRuntime::default()),
            captcha: Mutex::new(None),
        }
    }

    pub fn attach(&self, app: AppHandle<Wry>) {
        let _ = self.app.set(app);
    }

    pub fn app(&self) -> Option<&AppHandle<Wry>> {
        self.app.get()
    }

    pub fn save_flow(&self) {
        let snapshot = self.flow.lock().expect("flow lock").clone();
        if let Err(error) = self.paths.save_flow(&snapshot) {
            eprintln!("save flow failed: {error}");
        }
    }

    pub fn save_auth(&self) {
        let snapshot = self.auth.lock().expect("auth lock").clone();
        if let Err(error) = self.paths.save_auth(&snapshot) {
            eprintln!("save auth failed: {error}");
        }
    }

    pub fn save_device(&self) {
        let snapshot = self.device.lock().expect("device lock").clone();
        if let Err(error) = self.paths.save_device(&snapshot) {
            eprintln!("save device failed: {error}");
        }
    }

    pub fn save_settings(&self) {
        let snapshot = self.settings.lock().expect("settings lock").clone();
        if let Err(error) = self.paths.save_settings(&snapshot) {
            eprintln!("save settings failed: {error}");
        }
    }

    pub fn auth_status(&self) -> crate::types::AuthStatus {
        let auth = self.auth.lock().expect("auth lock");
        crate::types::AuthStatus {
            bili: match &auth.bili {
                Some(entry) => crate::types::AccountStatus {
                    logged_in: true,
                    name: entry.profile.name.clone(),
                    avatar: entry.profile.avatar.clone(),
                },
                None => crate::types::AccountStatus::default(),
            },
            netease: match &auth.netease {
                Some(entry) => crate::types::AccountStatus {
                    logged_in: true,
                    name: entry.profile.name.clone(),
                    avatar: entry.profile.avatar.clone(),
                },
                None => crate::types::AccountStatus::default(),
            },
        }
    }

    pub fn publish_auth(&self) {
        if let Some(app) = self.app() {
            let status = self.auth_status();
            let _ = tauri::Emitter::emit(app, "auth://state", status);
        }
    }

    pub fn mark_expired(&self, source: Source) {
        {
            let mut auth = self.auth.lock().expect("auth lock");
            match source {
                Source::Bili => auth.bili = None,
                Source::Netease => auth.netease = None,
            }
        }
        self.save_auth();
        if let Some(app) = self.app() {
            let _ = tauri::Emitter::emit(
                app,
                "auth://expired",
                serde_json::json!({ "source": source }),
            );
            let status = self.auth_status();
            let _ = tauri::Emitter::emit(app, "auth://state", status);
        }
    }

    pub fn publish_timer(&self, snapshot: &TimerSnapshot) {
        if let Some(app) = self.app() {
            crate::events::emit_timer(app, snapshot);
            #[cfg(desktop)]
            crate::tray::update_tooltip(app, snapshot);
            crate::notification::update(app, snapshot);
        }
        self.lan.broadcast_timer(snapshot);
    }

    pub fn publish_lan_status(&self) {
        if let Some(app) = self.app() {
            crate::events::emit_lan(app, &self.lan.status());
        }
    }
}
