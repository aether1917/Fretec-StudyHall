use crate::error::{AppError, CmdResult};
use crate::state::AppState;
use crate::types::{AccountProfile, QrLogin, QrPoll, SearchPage};

#[derive(Default)]
pub struct NeteaseRuntime {
    pub csrf: String,
}

#[derive(Debug, Clone, Default)]
pub struct NeteasePlayInfo {
    pub url: Option<String>,
    pub duration_sec: f64,
    pub title: String,
    pub author: String,
    pub cover: String,
}

pub async fn search(_state: &AppState, _keyword: &str, _page: u32) -> CmdResult<SearchPage> {
    Err(AppError::msg("网易云音乐搜索暂不可用"))
}

pub async fn song_url(_state: &AppState, _song_id: &str) -> CmdResult<NeteasePlayInfo> {
    Err(AppError::msg("网易云音乐播放暂不可用"))
}

pub async fn qr_generate(_state: &AppState) -> CmdResult<QrLogin> {
    Err(AppError::msg("网易云音乐二维码登录暂不可用"))
}

pub async fn qr_poll(_state: &AppState, _key: &str) -> CmdResult<QrPoll> {
    Err(AppError::msg("网易云音乐二维码登录暂不可用"))
}

pub async fn sms_send(_state: &AppState, _phone: &str) -> CmdResult<()> {
    Err(AppError::msg("网易云音乐短信登录暂不可用"))
}

pub async fn sms_login(
    _state: &AppState,
    _phone: &str,
    _code: &str,
) -> CmdResult<AccountProfile> {
    Err(AppError::msg("网易云音乐短信登录暂不可用"))
}

pub async fn login_password(
    _state: &AppState,
    _phone: &str,
    _password: &str,
) -> CmdResult<AccountProfile> {
    Err(AppError::msg("网易云音乐密码登录暂不可用"))
}

pub fn logout(state: &AppState) {
    let mut auth = state.auth.lock().expect("auth lock");
    auth.netease = None;
}
