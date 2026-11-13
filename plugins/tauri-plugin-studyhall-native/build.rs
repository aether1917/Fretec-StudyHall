const COMMANDS: &[&str] = &[
    "startTimer",
    "updateTimer",
    "stopTimer",
    "requestNotificationPermission",
    "notificationPermissionStatus",
    "exitApp",
];

fn main() {
    tauri_plugin::Builder::new(COMMANDS)
        .android_path("android")
        .build();
}
