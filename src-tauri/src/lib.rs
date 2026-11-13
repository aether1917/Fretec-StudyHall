mod bili;
mod commands;
mod error;
mod events;
mod lan;
mod media;
mod netease;
mod notification;
mod proxy;
mod state;
mod storage;
mod timer;
mod types;

#[cfg(desktop)]
mod tray;

use std::time::Duration;

use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let builder = tauri::Builder::default();
    #[cfg(target_os = "android")]
    let builder = builder.plugin(tauri_plugin_studyhall_native::init());
    builder
        .setup(|app| {
            let data_dir = app.path().app_data_dir()?;
            let paths = storage::AppPaths::new(data_dir);
            let state = state::AppState::new(paths);
            {
                let device = state.device.lock().expect("device lock").clone();
                state.lan.configure(device.id, device.name);
            }
            let handle = app.handle().clone();
            state.attach(handle.clone());
            state.lan.attach(handle.clone());
            app.manage(state);

            #[cfg(desktop)]
            tray::create(&handle)?;

            let loop_handle = handle.clone();
            tauri::async_runtime::spawn(async move {
                let (timer, lan) = {
                    let guard = loop_handle.state::<state::AppState>();
                    (guard.timer.clone(), guard.lan.clone())
                };
                lan.start_search();
                let mut last_slow_update = std::time::Instant::now();
                loop {
                    tokio::time::sleep(Duration::from_millis(250)).await;
                    if let Some(snapshot) = timer.tick() {
                        events::emit_timer(&loop_handle, &snapshot);
                        #[cfg(desktop)]
                        tray::update_tooltip(&loop_handle, &snapshot);
                        notification::update(&loop_handle, &snapshot);
                        lan.broadcast_timer(&snapshot);
                        last_slow_update = std::time::Instant::now();
                    } else if last_slow_update.elapsed() >= Duration::from_secs(5) {
                        let snapshot = timer.snapshot();
                        #[cfg(desktop)]
                        tray::update_tooltip(&loop_handle, &snapshot);
                        notification::update(&loop_handle, &snapshot);
                        last_slow_update = std::time::Instant::now();
                    }
                }
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::flow_list,
            commands::flow_add,
            commands::flow_remove,
            commands::flow_reorder,
            commands::flow_progress_save,
            commands::device_get_name,
            commands::device_set_name,
            commands::timer_get,
            commands::timer_start,
            commands::timer_pause,
            commands::timer_resume,
            commands::timer_stop,
            commands::auth_status,
            commands::auth_logout,
            commands::bili_qr_generate,
            commands::bili_qr_poll,
            commands::netease_qr_generate,
            commands::netease_qr_poll,
            commands::bili_captcha_config,
            commands::bili_captcha_result,
            commands::bili_sms_send,
            commands::bili_sms_login,
            commands::bili_login_password,
            commands::netease_sms_send,
            commands::netease_sms_login,
            commands::netease_login_password,
            commands::search_bili,
            commands::search_netease,
            commands::media_resolve,
            commands::media_close,
            commands::lan_status,
            commands::lan_start,
            commands::lan_stop,
            commands::lan_connect,
            commands::lan_disconnect,
        ])
        .on_window_event(|window, event| {
            #[cfg(desktop)]
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.hide();
            }
            #[cfg(not(desktop))]
            {
                let _ = (window, event);
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running Fretec StudyHall");
}
