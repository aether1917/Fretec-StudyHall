use tauri::{AppHandle, Emitter, State};

use crate::error::{AppError, CmdResult};
use crate::state::AppState;
use crate::types::{
    now_ms, AccountProfile, AuthStatus, CaptchaAnswer, CaptchaConfig, FlowItem, LanStatus,
    MediaPayload, Progress, QrLogin, QrPoll, SearchPage, SearchResultItem, SmsTicket, Source,
    TimerSnapshot,
};

/* ---------------- 学习流 ---------------- */

#[tauri::command]
pub fn flow_list(state: State<'_, AppState>) -> Vec<FlowItem> {
    state.flow.lock().expect("flow lock").items.clone()
}

#[tauri::command]
pub fn flow_add(state: State<'_, AppState>, item: SearchResultItem) -> CmdResult<FlowItem> {
    let mut flow = state.flow.lock().expect("flow lock");
    if let Some(existing) = flow
        .items
        .iter()
        .find(|entry| entry.source == item.source && entry.source_id == item.source_id)
    {
        return Ok(existing.clone());
    }
    let created = FlowItem {
        id: uuid::Uuid::new_v4().to_string(),
        source: item.source,
        source_id: item.source_id,
        title: item.title,
        author: item.author,
        cover: item.cover,
        duration_sec: item.duration_sec,
        added_at: now_ms(),
        progress: Progress::default(),
    };
    flow.items.push(created.clone());
    drop(flow);
    state.save_flow();
    Ok(created)
}

#[tauri::command]
pub fn flow_remove(state: State<'_, AppState>, id: String) -> CmdResult<()> {
    let mut flow = state.flow.lock().expect("flow lock");
    let before = flow.items.len();
    flow.items.retain(|entry| entry.id != id);
    if flow.items.len() == before {
        return Err(AppError::msg("条目不存在"));
    }
    drop(flow);
    state.save_flow();
    Ok(())
}

#[tauri::command]
pub fn flow_reorder(state: State<'_, AppState>, ids: Vec<String>) -> CmdResult<Vec<FlowItem>> {
    let mut flow = state.flow.lock().expect("flow lock");
    let mut ordered: Vec<FlowItem> = Vec::with_capacity(flow.items.len());
    for id in &ids {
        if let Some(position) = flow.items.iter().position(|entry| &entry.id == id) {
            ordered.push(flow.items.remove(position));
        }
    }
    ordered.append(&mut flow.items);
    flow.items = ordered;
    let snapshot = flow.items.clone();
    drop(flow);
    state.save_flow();
    Ok(snapshot)
}

#[tauri::command]
pub fn flow_progress_save(
    state: State<'_, AppState>,
    id: String,
    position_sec: f64,
    duration_sec: f64,
) -> CmdResult<()> {
    let mut flow = state.flow.lock().expect("flow lock");
    let Some(item) = flow.items.iter_mut().find(|entry| entry.id == id) else {
        return Ok(());
    };
    item.progress.position_sec = position_sec.max(0.0);
    if duration_sec > 0.0 {
        item.duration_sec = duration_sec;
    }
    item.progress.updated_at = now_ms();
    if item.progress.position_sec > item.duration_sec {
        item.progress.position_sec = item.duration_sec;
    }
    drop(flow);
    state.save_flow();
    Ok(())
}

/* ---------------- 设备 ---------------- */

#[tauri::command]
pub fn device_get_name(state: State<'_, AppState>) -> String {
    state.device.lock().expect("device lock").name.clone()
}

#[tauri::command]
pub fn device_set_name(state: State<'_, AppState>, name: String) -> CmdResult<String> {
    let trimmed = name.trim().to_string();
    if trimmed.is_empty() {
        return Err(AppError::msg("设备名称不能为空"));
    }
    if trimmed.chars().count() > 40 {
        return Err(AppError::msg("设备名称最多 40 个字符"));
    }
    {
        let mut device = state.device.lock().expect("device lock");
        device.name = trimmed.clone();
    }
    state.save_device();
    state.lan.update_name(trimmed.clone());
    state.publish_lan_status();
    Ok(trimmed)
}

/* ---------------- 计时 ---------------- */

#[tauri::command]
pub fn timer_get(state: State<'_, AppState>) -> TimerSnapshot {
    state.timer.snapshot()
}

fn publish_timer(state: &AppState, snapshot: TimerSnapshot) -> TimerSnapshot {
    state.publish_timer(&snapshot);
    snapshot
}

#[tauri::command]
pub fn timer_start(state: State<'_, AppState>) -> TimerSnapshot {
    publish_timer(&state, state.timer.start())
}

#[tauri::command]
pub fn timer_pause(state: State<'_, AppState>) -> TimerSnapshot {
    publish_timer(&state, state.timer.pause())
}

#[tauri::command]
pub fn timer_resume(state: State<'_, AppState>) -> TimerSnapshot {
    publish_timer(&state, state.timer.resume())
}

#[tauri::command]
pub fn timer_stop(state: State<'_, AppState>) -> TimerSnapshot {
    publish_timer(&state, state.timer.stop())
}

/* ---------------- 账号 ---------------- */

#[tauri::command]
pub fn auth_status(state: State<'_, AppState>) -> AuthStatus {
    state.auth_status()
}

#[tauri::command]
pub fn auth_logout(state: State<'_, AppState>, source: Source) -> AuthStatus {
    match source {
        Source::Bili => crate::bili::logout(&state),
        Source::Netease => crate::netease::logout(&state),
    }
    state.save_auth();
    state.publish_auth();
    state.auth_status()
}

#[tauri::command]
pub async fn bili_qr_generate(state: State<'_, AppState>) -> CmdResult<QrLogin> {
    crate::bili::qr_generate(&state).await
}

#[tauri::command]
pub async fn bili_qr_poll(state: State<'_, AppState>, key: String) -> CmdResult<QrPoll> {
    crate::bili::qr_poll(&state, &key).await
}

#[tauri::command]
pub async fn netease_qr_generate(state: State<'_, AppState>) -> CmdResult<QrLogin> {
    crate::netease::qr_generate(&state).await
}

#[tauri::command]
pub async fn netease_qr_poll(state: State<'_, AppState>, key: String) -> CmdResult<QrPoll> {
    crate::netease::qr_poll(&state, &key).await
}

#[tauri::command]
pub async fn bili_captcha_config(state: State<'_, AppState>) -> CmdResult<CaptchaConfig> {
    crate::bili::captcha_config(&state).await
}

#[tauri::command]
pub fn bili_captcha_result(
    state: State<'_, AppState>,
    app: AppHandle,
    challenge: String,
    validate: String,
    seccode: String,
) -> CmdResult<()> {
    let answer = CaptchaAnswer {
        challenge,
        validate,
        seccode,
    };
    *state.captcha.lock().expect("captcha lock") = Some(answer.clone());
    let _ = app.emit("captcha://result", answer);
    Ok(())
}

#[tauri::command]
pub async fn bili_sms_send(
    state: State<'_, AppState>,
    tel: String,
    cid: String,
    challenge: String,
    validate: String,
    seccode: String,
) -> CmdResult<SmsTicket> {
    crate::bili::sms_send(&state, &tel, &cid, &challenge, &validate, &seccode).await
}

#[tauri::command]
pub async fn bili_sms_login(
    state: State<'_, AppState>,
    tel: String,
    cid: String,
    code: String,
    captcha_key: String,
) -> CmdResult<AccountProfile> {
    crate::bili::sms_login(&state, &tel, &cid, &code, &captcha_key).await
}

#[tauri::command]
pub async fn bili_login_password(
    state: State<'_, AppState>,
    username: String,
    password: String,
    captcha_token: Option<String>,
) -> CmdResult<AccountProfile> {
    crate::bili::login_password(&state, &username, &password, captcha_token.as_deref()).await
}

#[tauri::command]
pub async fn netease_sms_send(state: State<'_, AppState>, phone: String) -> CmdResult<()> {
    crate::netease::sms_send(&state, &phone).await
}

#[tauri::command]
pub async fn netease_sms_login(
    state: State<'_, AppState>,
    phone: String,
    code: String,
) -> CmdResult<AccountProfile> {
    crate::netease::sms_login(&state, &phone, &code).await
}

#[tauri::command]
pub async fn netease_login_password(
    state: State<'_, AppState>,
    phone: String,
    password: String,
) -> CmdResult<AccountProfile> {
    crate::netease::login_password(&state, &phone, &password).await
}

/* ---------------- 搜索 ---------------- */

#[tauri::command]
pub async fn search_bili(
    state: State<'_, AppState>,
    keyword: String,
    page: u32,
) -> CmdResult<SearchPage> {
    crate::bili::search(&state, &keyword, page).await
}

#[tauri::command]
pub async fn search_netease(
    state: State<'_, AppState>,
    keyword: String,
    page: u32,
) -> CmdResult<SearchPage> {
    crate::netease::search(&state, &keyword, page).await
}

/* ---------------- 媒体 ---------------- */

#[tauri::command]
pub async fn media_resolve(
    state: State<'_, AppState>,
    item_id: String,
    quality: u32,
) -> CmdResult<MediaPayload> {
    state.media.resolve(state.inner(), &item_id, quality).await
}

#[tauri::command]
pub fn media_close(state: State<'_, AppState>) -> CmdResult<()> {
    state.media.close();
    Ok(())
}

/* ---------------- 局域网 ---------------- */

#[tauri::command]
pub fn lan_status(state: State<'_, AppState>) -> LanStatus {
    state.lan.status()
}

#[tauri::command]
pub fn lan_start(state: State<'_, AppState>) -> LanStatus {
    state.lan.start_search();
    let status = state.lan.status();
    state.publish_lan_status();
    status
}

#[tauri::command]
pub fn lan_stop(state: State<'_, AppState>) {
    state.lan.stop();
    state.publish_lan_status();
}

#[tauri::command]
pub fn lan_connect(state: State<'_, AppState>, peer_id: String) -> CmdResult<()> {
    state.lan.connect(&peer_id);
    Ok(())
}

#[tauri::command]
pub fn lan_disconnect(state: State<'_, AppState>, peer_id: String) -> CmdResult<()> {
    state.lan.disconnect(&peer_id);
    Ok(())
}
