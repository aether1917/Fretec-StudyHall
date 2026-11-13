use crate::error::{AppError, CmdResult};
use crate::state::AppState;
use crate::types::{
    AccountProfile, CaptchaConfig, QrLogin, QrPoll, SearchPage, SmsTicket,
};

#[derive(Default)]
pub struct BiliRuntime {
    pub buvid3: Option<String>,
    pub buvid4: Option<String>,
    pub wbi_mixin_key: Option<(String, u64)>,
}

#[derive(Debug, Clone, Default)]
pub struct BiliPlayInfo {
    pub title: String,
    pub author: String,
    pub cover: String,
    pub duration_sec: f64,
    pub video_url: Option<String>,
    pub audio_url: Option<String>,
    pub single_url: Option<String>,
    pub actual_qn: u32,
    pub fallback: bool,
    pub notice: Option<String>,
}

pub async fn search(_state: &AppState, _keyword: &str, _page: u32) -> CmdResult<SearchPage> {
    Err(AppError::msg("哔哩哔哩搜索暂不可用"))
}

pub async fn playurl(_state: &AppState, _bvid: &str, _qn: u32) -> CmdResult<BiliPlayInfo> {
    Err(AppError::msg("哔哩哔哩播放暂不可用"))
}

pub async fn qr_generate(_state: &AppState) -> CmdResult<QrLogin> {
    Err(AppError::msg("哔哩哔哩二维码登录暂不可用"))
}

pub async fn qr_poll(_state: &AppState, _key: &str) -> CmdResult<QrPoll> {
    Err(AppError::msg("哔哩哔哩二维码登录暂不可用"))
}

pub async fn sms_send(
    _state: &AppState,
    _tel: &str,
    _cid: &str,
    _challenge: &str,
    _validate: &str,
    _seccode: &str,
) -> CmdResult<SmsTicket> {
    Err(AppError::msg("哔哩哔哩短信登录暂不可用"))
}

pub async fn sms_login(
    _state: &AppState,
    _tel: &str,
    _cid: &str,
    _code: &str,
    _captcha_key: &str,
) -> CmdResult<AccountProfile> {
    Err(AppError::msg("哔哩哔哩短信登录暂不可用"))
}

pub async fn login_password(
    _state: &AppState,
    _username: &str,
    _password: &str,
    _captcha_token: Option<&str>,
) -> CmdResult<AccountProfile> {
    Err(AppError::msg("哔哩哔哩密码登录暂不可用"))
}

pub async fn captcha_config(_state: &AppState) -> CmdResult<CaptchaConfig> {
    Err(AppError::msg("哔哩哔哩验证码暂不可用"))
}

pub fn logout(state: &AppState) {
    let mut auth = state.auth.lock().expect("auth lock");
    auth.bili = None;
}
