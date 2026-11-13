const COMMANDS: &[&str] = &[
    "startTimer",
    "updateTimer",
    "stopTimer",
    "requestNotificationPermission",
];

fn main() {
    tauri_plugin::Builder::new(COMMANDS)
        .android_path("android")
        .build();
}
