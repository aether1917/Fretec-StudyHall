//! 网易云音乐客户端：搜索、播放地址、二维码 / 短信 / 密码登录。
//!
//! 对外函数保持 `netease/mod.rs` 原有签名，供 lib 层的 command 直接调用。
//! 加密细节见 [`weapi`]，cookie 解析见 [`cookie`]。

mod cookie;
mod login;
mod search;
mod weapi;

use serde_json::Value;

use crate::error::{AppError, CmdResult};
use crate::state::AppState;
use crate::storage::{NeteaseAuth, NeteaseCookies};
use crate::types::{AccountProfile, QrLogin, QrPoll, SearchPage};

/// 网易云桌面端 Chrome UA。
const DESKTOP_USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) \
    AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36";
/// 网易云要求携带的 Referer。
const MUSIC_REFERER: &str = "https://music.163.com/";
/// 网易云要求携带的 Origin。
const MUSIC_ORIGIN: &str = "https://music.163.com";
/// 账号资料接口。
const NUSER_ACCOUNT_URL: &str = "https://music.163.com/api/nuser/account/get";

/// 网易云运行时缓存（csrf 等短期状态）。
#[derive(Default)]
pub struct NeteaseRuntime {
    pub csrf: String,
}

/// 网易云歌曲播放信息。
#[derive(Debug, Clone, Default)]
pub struct NeteasePlayInfo {
    pub url: Option<String>,
    pub duration_sec: f64,
    pub title: String,
    pub author: String,
    pub cover: String,
}

/// 搜索单曲（type = 1），每页 20 条。
pub async fn search(state: &AppState, keyword: &str, page: u32) -> CmdResult<SearchPage> {
    search::run_search(state, keyword, page).await
}

/// 获取单曲播放地址，并尽量补齐标题 / 作者 / 封面 / 时长。
pub async fn song_url(state: &AppState, song_id: &str) -> CmdResult<NeteasePlayInfo> {
    search::run_song_url(state, song_id).await
}

/// 生成扫码登录二维码。
pub async fn qr_generate(state: &AppState) -> CmdResult<QrLogin> {
    login::run_qr_generate(state).await
}

/// 轮询扫码状态。
pub async fn qr_poll(state: &AppState, key: &str) -> CmdResult<QrPoll> {
    login::run_qr_poll(state, key).await
}

/// 发送短信验证码。
pub async fn sms_send(state: &AppState, phone: &str) -> CmdResult<()> {
    login::run_sms_send(state, phone).await
}

/// 短信验证码登录。
pub async fn sms_login(state: &AppState, phone: &str, code: &str) -> CmdResult<AccountProfile> {
    login::run_sms_login(state, phone, code).await
}

/// 手机号 + 密码登录。
pub async fn login_password(
    state: &AppState,
    phone: &str,
    password: &str,
) -> CmdResult<AccountProfile> {
    login::run_login_password(state, phone, password).await
}

/// 一次网易云接口请求的解析结果。
pub(crate) struct NeteaseResponse {
    pub(crate) json: Value,
    pub(crate) cookies: cookie::CookieJar,
}

/// 当前登录 cookie；auth 里没有时回退到 runtime 缓存的 csrf。
pub(crate) fn current_cookies(state: &AppState) -> NeteaseCookies {
    let stored = match state.auth.lock() {
        Ok(auth) => auth.netease.as_ref().map(|entry| entry.cookies.clone()),
        Err(poisoned) => poisoned
            .into_inner()
            .netease
            .as_ref()
            .map(|entry| entry.cookies.clone()),
    };
    let mut cookies = stored.unwrap_or_default();
    if cookies.csrf.is_empty() {
        cookies.csrf = runtime_csrf(state);
    }
    cookies
}

fn runtime_csrf(state: &AppState) -> String {
    match state.netease.lock() {
        Ok(runtime) => runtime.csrf.clone(),
        Err(poisoned) => poisoned.into_inner().csrf.clone(),
    }
}

fn set_runtime_csrf(state: &AppState, csrf: &str) {
    if csrf.is_empty() {
        return;
    }
    let mut runtime = match state.netease.lock() {
        Ok(runtime) => runtime,
        Err(poisoned) => poisoned.into_inner(),
    };
    runtime.csrf = csrf.to_string();
}

/// 登录成功后持久化 cookie 与资料，并广播登录态。
pub(crate) fn save_login(state: &AppState, cookies: NeteaseCookies, profile: AccountProfile) {
    set_runtime_csrf(state, &cookies.csrf);
    {
        let mut auth = match state.auth.lock() {
            Ok(auth) => auth,
            Err(poisoned) => poisoned.into_inner(),
        };
        auth.netease = Some(NeteaseAuth { cookies, profile });
    }
    state.save_auth();
    state.publish_auth();
}

/// 读取 weapi 响应里的 `code`，容忍字符串形式。
pub(crate) fn json_code(value: &Value) -> Option<i64> {
    match value.get("code")? {
        Value::Number(number) => number.as_i64(),
        Value::String(text) => text.trim().parse::<i64>().ok(),
        _ => None,
    }
}

/// 301 / 302 / -462 都表示登录态失效，需要清理本地登录态。
pub(crate) fn is_login_error(code: Option<i64>) -> bool {
    matches!(code, Some(301) | Some(302) | Some(-462))
}

/// 网易云大部分资源地址是 `http://`，webview 下统一升级为 https。
pub(crate) fn https_url(url: &str) -> String {
    match url.strip_prefix("http://") {
        Some(rest) => format!("https://{rest}"),
        None => url.to_string(),
    }
}

/// 从 `{"profile":{"nickname","avatarUrl"}}` 中提取账号资料。
pub(crate) fn profile_from_json(value: &Value) -> Option<AccountProfile> {
    let profile = value.get("profile")?;
    if !profile.is_object() {
        return None;
    }
    Some(AccountProfile {
        name: profile
            .get("nickname")
            .and_then(Value::as_str)
            .unwrap_or("")
            .trim()
            .to_string(),
        avatar: profile
            .get("avatarUrl")
            .and_then(Value::as_str)
            .map(https_url)
            .unwrap_or_default(),
    })
}

/// 带 cookie 拉取账号资料，失败时返回 `None`（不阻断登录主流程）。
pub(crate) async fn fetch_account_profile(
    state: &AppState,
    cookies: &NeteaseCookies,
) -> Option<AccountProfile> {
    let response = get_json(state, NUSER_ACCOUNT_URL, cookies).await.ok()?;
    if let Some(code) = json_code(&response.json) {
        if code != 200 {
            return None;
        }
    }
    profile_from_json(&response.json)
}

fn network_error() -> AppError {
    AppError::msg("网络请求失败，请检查网络连接后重试")
}

fn service_error() -> AppError {
    AppError::msg("网易云音乐服务暂时不可用，请稍后重试")
}

fn cookie_header(cookies: &NeteaseCookies) -> Option<String> {
    let header = cookies.header();
    if header.is_empty() {
        None
    } else {
        Some(header)
    }
}

/// 统一处理响应：解析 cookie、校验状态码、解析 JSON。
async fn finish(response: reqwest::Response) -> CmdResult<NeteaseResponse> {
    let cookies = cookie::CookieJar::from_headers(response.headers());
    let status = response.status();
    let body = response.text().await.map_err(|_| network_error())?;
    let json: Value = match serde_json::from_str(&body) {
        Ok(json) => json,
        Err(_) => {
            return if status.is_success() {
                Err(AppError::msg("网易云接口返回了无法解析的数据"))
            } else {
                Err(service_error())
            };
        }
    };
    if status.as_u16() >= 500 {
        return Err(service_error());
    }
    Ok(NeteaseResponse { json, cookies })
}

/// 发送 weapi 请求（AES + RSA 加密的 form body）。
pub(crate) async fn post_weapi(
    state: &AppState,
    endpoint: &str,
    payload: &Value,
    cookies: &NeteaseCookies,
) -> CmdResult<NeteaseResponse> {
    let form = weapi::encrypt_payload(payload)?;
    let csrf = cookies.csrf.trim();
    let url = if csrf.is_empty() {
        format!("{endpoint}?csrf_token=")
    } else {
        format!("{endpoint}?csrf_token={}", urlencoding::encode(csrf))
    };
    let mut request = state
        .http
        .post(url)
        .header(reqwest::header::REFERER, MUSIC_REFERER)
        .header(reqwest::header::ORIGIN, MUSIC_ORIGIN)
        .header(reqwest::header::USER_AGENT, DESKTOP_USER_AGENT)
        .header(
            reqwest::header::CONTENT_TYPE,
            "application/x-www-form-urlencoded",
        )
        .body(form.body());
    if let Some(header) = cookie_header(cookies) {
        request = request.header(reqwest::header::COOKIE, header);
    }
    let response = request.send().await.map_err(|_| network_error())?;
    finish(response).await
}

/// 发送普通表单请求（二维码 / 短信验证码等 interface 接口）。
pub(crate) async fn post_form(
    state: &AppState,
    url: &str,
    body: String,
    cookies: Option<&NeteaseCookies>,
) -> CmdResult<NeteaseResponse> {
    let mut request = state
        .http
        .post(url)
        .header(reqwest::header::REFERER, MUSIC_REFERER)
        .header(reqwest::header::ORIGIN, MUSIC_ORIGIN)
        .header(reqwest::header::USER_AGENT, DESKTOP_USER_AGENT)
        .header(
            reqwest::header::CONTENT_TYPE,
            "application/x-www-form-urlencoded",
        )
        .body(body);
    if let Some(cookies) = cookies {
        if let Some(header) = cookie_header(cookies) {
            request = request.header(reqwest::header::COOKIE, header);
        }
    }
    let response = request.send().await.map_err(|_| network_error())?;
    finish(response).await
}

/// 发送带 cookie 的 GET 请求。
async fn get_json(
    state: &AppState,
    url: &str,
    cookies: &NeteaseCookies,
) -> CmdResult<NeteaseResponse> {
    let mut request = state
        .http
        .get(url)
        .header(reqwest::header::REFERER, MUSIC_REFERER)
        .header(reqwest::header::ORIGIN, MUSIC_ORIGIN)
        .header(reqwest::header::USER_AGENT, DESKTOP_USER_AGENT);
    if let Some(header) = cookie_header(cookies) {
        request = request.header(reqwest::header::COOKIE, header);
    }
    let response = request.send().await.map_err(|_| network_error())?;
    finish(response).await
}

pub fn logout(state: &AppState) {
    let mut auth = match state.auth.lock() {
        Ok(auth) => auth,
        Err(poisoned) => poisoned.into_inner(),
    };
    auth.netease = None;
}
