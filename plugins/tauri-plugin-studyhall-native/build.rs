const COMMANDS: &[&str] = &[
    "startTimer",
    "updateTimer",
    "stopTimer",
    "requestNotificationPermission",
    "notificationPermissionStatus",
];

fn main() {
    tauri_plugin::Builder::new(COMMANDS)
        .android_path("android")
        .build();
}
