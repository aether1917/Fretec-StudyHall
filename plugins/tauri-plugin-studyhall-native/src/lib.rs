use serde::{Deserialize, Serialize};
use tauri::plugin::{Builder, PluginHandle, TauriPlugin};
use tauri::{Manager, Runtime};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TimerPayload {
    pub phase: String,
    pub total_ms: u64,
    pub started_at_ms: Option<u64>,
    pub pause_offset_ms: u64,
    pub paused_at_ms: Option<u64>,
    pub status: String,
}

pub struct StudyHallHandle<R: Runtime>(pub PluginHandle<R>);

impl<R: Runtime> StudyHallHandle<R> {
    pub fn start(&self, payload: &TimerPayload) {
        let _ = self.0.run_mobile_plugin::<()>("startTimer", payload);
    }

    pub fn update(&self, payload: &TimerPayload) {
        let _ = self.0.run_mobile_plugin::<()>("updateTimer", payload);
    }

    pub fn stop(&self) {
        let _ = self.0.run_mobile_plugin::<()>("stopTimer", ());
    }

    pub fn request_permission(&self) {
        let _ = self
            .0
            .run_mobile_plugin::<()>("requestNotificationPermission", ());
    }

    pub fn permission_status(&self) -> bool {
        self.0
            .run_mobile_plugin::<bool>("notificationPermissionStatus", ())
            .unwrap_or(false)
    }
}

pub fn init<R: Runtime>() -> TauriPlugin<R> {
    Builder::new("studyhall-native")
        .setup(|app, api| {
            #[cfg(target_os = "android")]
            {
                let handle = api.register_android_plugin(
                    "com.fretec.studyhall.nativeplugin",
                    "StudyHallNativePlugin",
                )?;
                app.manage(StudyHallHandle(handle));
            }
            #[cfg(not(target_os = "android"))]
            {
                let _ = (app, api);
            }
            Ok(())
        })
        .build()
}
