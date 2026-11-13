//! 哔哩哔哩客户端模块。
//!
//! 模块只负责协议细节：cookie 引导、WBI 签名、搜索、取流与登录。
//! 凭据只保存在内存 / `auth.json`，本模块不会向标准输出打印任何凭据。

mod cookie;
mod login;
mod playurl;
mod search;
mod wbi;

use std::time::{SystemTime, UNIX_EPOCH};

use reqwest::header::{ACCEPT, ACCEPT_LANGUAGE, CONTENT_TYPE, COOKIE, REFERER, USER_AGENT};
use reqwest::RequestBuilder;
use serde_json::Value;

use crate::error::{AppError, CmdResult};
use crate::state::AppState;
use crate::storage::{BiliAuth, BiliCookies};
use crate::types::{AccountProfile, Source};

pub use login::{captcha_config, login_password, qr_generate, qr_poll, sms_login, sms_send};
pub use playurl::playurl;
pub use search::search;

pub(crate) const UA: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) \
AppleWebKit/537.36 (KHTML, like Gecko) Chrome/125.0.0.0 Safari/537.36";
pub(crate) const REFERER_VALUE: &str = "https://www.bilibili.com";
pub(crate) const HOMEPAGE_URL: &str = "https://www.bilibili.com/";
pub(crate) const NAV_URL: &str = "https://api.bilibili.com/x/web-interface/nav";

/// WBI 密钥缓存时长（秒）。
const WBI_TTL_SECS: u64 = 60 * 60;

/// 哔哩哔哩匿名访问与签名相关的运行时状态。
#[derive(Default, Clone)]
pub struct BiliRuntime {
    pub buvid3: Option<String>,
    pub buvid4: Option<String>,
    /// `(mixin_key, 过期时间戳秒)`，缓存一小时。
    pub wbi_mixin_key: Option<(String, u64)>,
}

/// `playurl` 聚合后的播放信息，供上层组装 `MediaPayload`。
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

pub fn logout(state: &AppState) {
    let mut auth = state.auth.lock().expect("auth lock");
    auth.bili = None;
}

/// 哔哩哔哩接口响应的最小解析结果。
pub(crate) struct BiliResponse {
    body: Value,
    set_cookie_headers: Vec<String>,
}

impl BiliResponse {
    pub(crate) fn code(&self) -> i64 {
        self.body.get("code").and_then(Value::as_i64).unwrap_or(-1)
    }

    pub(crate) fn message(&self) -> &str {
        self.body
            .get("message")
            .and_then(Value::as_str)
            .unwrap_or("")
    }

    /// `data` 为空或 `null` 时返回 `None`。
    pub(crate) fn data(&self) -> Option<&Value> {
        self.body.get("data").filter(|value| !value.is_null())
    }

    pub(crate) fn set_cookies(&self) -> &[String] {
        &self.set_cookie_headers
    }
}

pub(crate) fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

fn network_error(error: reqwest::Error) -> AppError {
    if error.is_timeout() {
        AppError::msg("网络请求超时，请稍后再试")
    } else if error.is_connect() {
        AppError::msg("网络连接失败，请检查网络后重试")
    } else {
        AppError::msg("网络请求失败，请稍后再试")
    }
}

/// 发送请求并把响应体解析为 JSON；解析失败时给出中文错误。
pub(crate) async fn execute(_state: &AppState, request: RequestBuilder) -> CmdResult<BiliResponse> {
    let response = request.send().await.map_err(network_error)?;
    let status = response.status();
    let blocked = status.as_u16() == 412;
    let set_cookie_headers = cookie::collect_set_cookie(response.headers());
    let text = response.text().await.map_err(network_error)?;
    let body = if text.trim().is_empty() {
        if blocked {
            blocked_body()
        } else {
            return Err(AppError::msg("哔哩哔哩没有返回数据，请稍后再试"));
        }
    } else {
        match serde_json::from_str::<Value>(&text) {
            Ok(body) => body,
            // HTTP 412 可能是纯文本拦截页，这里合成 -412 以便上层按 cookie 不足重试。
            Err(_) if blocked => blocked_body(),
            Err(_) => {
                return Err(AppError::msg(format!(
                    "哔哩哔哩返回数据异常（HTTP {}）",
                    status.as_u16()
                )));
            }
        }
    };
    Ok(BiliResponse {
        body,
        set_cookie_headers,
    })
}

/// 风控拦截响应的合成结果，仅保留错误码与文案。
fn blocked_body() -> Value {
    serde_json::json!({ "code": -412, "message": "请求被拦截" })
}

/// 构造带桌面 UA、Referer 与 Cookie 的 GET 请求。
pub(crate) fn api_get(state: &AppState, url: &str, cookies: &str) -> RequestBuilder {
    with_common_headers(state.http.get(url), cookies)
}

/// 构造带桌面 UA、Referer 与 Cookie 的 POST 请求。
pub(crate) fn api_post(state: &AppState, url: &str, cookies: &str) -> RequestBuilder {
    with_common_headers(state.http.post(url), cookies)
}

/// 构造 `application/x-www-form-urlencoded` 的 POST 请求。
pub(crate) fn form_request(
    state: &AppState,
    url: &str,
    cookies: &str,
    body: String,
) -> RequestBuilder {
    api_post(state, url, cookies)
        .header(CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(body)
}

/// 编码 `application/x-www-form-urlencoded` 表单。
///
/// 复用 WBI 的 `encodeURIComponent` 编码器：`&`、`=`、`+`、`%` 等分隔符都会被
/// 转义，空格编码为 `%20`，服务端按表单解析可正确还原。
pub(crate) fn form_body(pairs: &[(&str, &str)]) -> String {
    pairs
        .iter()
        .map(|&(key, value)| {
            format!(
                "{}={}",
                wbi::encode_component(key),
                wbi::encode_component(value)
            )
        })
        .collect::<Vec<String>>()
        .join("&")
}

fn with_common_headers(mut request: RequestBuilder, cookies: &str) -> RequestBuilder {
    request = request
        .header(USER_AGENT, UA)
        .header(REFERER, REFERER_VALUE)
        .header(ACCEPT, "application/json, text/plain, */*")
        .header(ACCEPT_LANGUAGE, "zh-CN,zh;q=0.9");
    if !cookies.is_empty() {
        request = request.header(COOKIE, cookies);
    }
    request
}

pub(crate) fn auth_cookies(state: &AppState) -> BiliCookies {
    let auth = state.auth.lock().expect("auth lock");
    auth.bili
        .as_ref()
        .map(|entry| entry.cookies.clone())
        .unwrap_or_default()
}

pub(crate) fn runtime_snapshot(state: &AppState) -> BiliRuntime {
    state.bili.lock().expect("bili lock").clone()
}

pub(crate) fn is_logged_in(state: &AppState) -> bool {
    let auth = state.auth.lock().expect("auth lock");
    auth.bili
        .as_ref()
        .map(|entry| entry.cookies.is_logged_in())
        .unwrap_or(false)
}

/// 合并已登录 cookie 与运行时匿名 buvid；运行时的 buvid 更新，优先使用。
pub(crate) fn merged_cookies(auth: &BiliCookies, runtime: &BiliRuntime) -> BiliCookies {
    let mut merged = auth.clone();
    if let Some(value) = runtime.buvid3.as_deref().filter(|value| !value.is_empty()) {
        merged.buvid3 = value.to_string();
    }
    if let Some(value) = runtime.buvid4.as_deref().filter(|value| !value.is_empty()) {
        merged.buvid4 = value.to_string();
    }
    merged
}

/// 确保已取得 buvid3 / buvid4。首次请求前访问首页引导匿名 cookie。
pub(crate) async fn ensure_buvid(state: &AppState) -> CmdResult<()> {
    let has_runtime = runtime_snapshot(state)
        .buvid3
        .as_deref()
        .map(|value| !value.is_empty())
        .unwrap_or(false);
    if has_runtime {
        return Ok(());
    }
    fetch_buvid(state).await
}

/// 强制重新获取 buvid3，用于 -412 时补 cookie。
pub(crate) async fn refresh_buvid(state: &AppState) -> CmdResult<()> {
    {
        let mut runtime = state.bili.lock().expect("bili lock");
        runtime.buvid3 = None;
        runtime.buvid4 = None;
    }
    fetch_buvid(state).await
}

/// 访问首页，从 `Set-Cookie` 中读取 buvid3 / buvid4。
async fn fetch_buvid(state: &AppState) -> CmdResult<()> {
    let response = state
        .http
        .get(HOMEPAGE_URL)
        .header(USER_AGENT, UA)
        .header(
            ACCEPT,
            "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8",
        )
        .header(ACCEPT_LANGUAGE, "zh-CN,zh;q=0.9")
        .send()
        .await
        .map_err(network_error)?;
    let headers = cookie::collect_set_cookie(response.headers());
    let buvid3 = cookie::find_value(&headers, "buvid3")
        .map(|value| value.to_string())
        .unwrap_or_default();
    let buvid4 = cookie::find_value(&headers, "buvid4")
        .map(|value| value.to_string())
        .unwrap_or_default();
    if buvid3.is_empty() {
        return Err(AppError::msg("未能获取哔哩哔哩访问凭据，请稍后再试"));
    }
    {
        let mut runtime = state.bili.lock().expect("bili lock");
        runtime.buvid3 = Some(buvid3);
        if !buvid4.is_empty() {
            runtime.buvid4 = Some(buvid4);
        }
    }
    Ok(())
}

/// 取 WBI mixin key，命中缓存则直接返回。
pub(crate) async fn ensure_mixin_key(state: &AppState) -> CmdResult<String> {
    {
        let runtime = state.bili.lock().expect("bili lock");
        if let Some((key, expires_at)) = runtime.wbi_mixin_key.as_ref() {
            if !key.is_empty() && now_secs() < *expires_at {
                return Ok(key.clone());
            }
        }
    }

    let cookies = prepare_cookies(state).await?;
    let response = execute(state, api_get(state, NAV_URL, &cookies)).await?;
    let code = response.code();
    if code != 0 {
        return Err(api_error(state, code, response.message()));
    }
    let data = response
        .data()
        .ok_or_else(|| AppError::msg("哔哩哔哩签名信息获取失败，请稍后再试"))?;
    let img_url = data
        .get("wbi_img")
        .and_then(|value| value.get("img_url"))
        .and_then(Value::as_str)
        .unwrap_or("");
    let sub_url = data
        .get("wbi_img")
        .and_then(|value| value.get("sub_url"))
        .and_then(Value::as_str)
        .unwrap_or("");
    let mixin_key = wbi::mixin_key_from_urls(img_url, sub_url)
        .ok_or_else(|| AppError::msg("哔哩哔哩签名密钥解析失败，请稍后再试"))?;
    {
        let mut runtime = state.bili.lock().expect("bili lock");
        runtime.wbi_mixin_key = Some((mixin_key.clone(), now_secs() + WBI_TTL_SECS));
    }
    Ok(mixin_key)
}

/// 确保取得 Cookie 请求头（含匿名 buvid3）。
pub(crate) async fn prepare_cookies(state: &AppState) -> CmdResult<String> {
    ensure_buvid(state).await?;
    let auth = auth_cookies(state);
    let runtime = runtime_snapshot(state);
    Ok(merged_cookies(&auth, &runtime).header())
}

/// 使用指定 Cookie 请求 nav，读取昵称与头像。
pub(crate) async fn fetch_profile_with_cookies(
    state: &AppState,
    cookies: &str,
) -> CmdResult<AccountProfile> {
    let response = execute(state, api_get(state, NAV_URL, cookies)).await?;
    let code = response.code();
    if code != 0 {
        return Err(api_error(state, code, response.message()));
    }
    let data = response
        .data()
        .ok_or_else(|| AppError::msg("哔哩哔哩账号信息获取失败，请稍后再试"))?;
    let logged_in = data
        .get("isLogin")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let name = data
        .get("uname")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim()
        .to_string();
    let avatar = data.get("face").and_then(Value::as_str).unwrap_or("");
    if !logged_in && name.is_empty() {
        return Err(AppError::msg("哔哩哔哩登录状态校验失败，请重试"));
    }
    Ok(AccountProfile {
        name: if name.is_empty() {
            "哔哩哔哩用户".to_string()
        } else {
            name
        },
        avatar: normalize_image_url(avatar),
    })
}

pub(crate) fn fallback_profile() -> AccountProfile {
    AccountProfile {
        name: "哔哩哔哩用户".to_string(),
        avatar: String::new(),
    }
}

/// 登录成功后把响应 cookie 与已有 cookie 合并（保留已有 buvid3 / buvid4）。
pub(crate) fn merge_for_login(state: &AppState, incoming: &BiliCookies) -> BiliCookies {
    let auth = auth_cookies(state);
    let runtime = runtime_snapshot(state);
    let base = merged_cookies(&auth, &runtime);
    cookie::merge_login_cookies(&base, incoming)
}

/// 持久化登录态并广播 `auth://state`。
pub(crate) fn persist_bili_auth(state: &AppState, cookies: BiliCookies, profile: AccountProfile) {
    {
        let mut auth = state.auth.lock().expect("auth lock");
        auth.bili = Some(BiliAuth {
            cookies: cookies.clone(),
            profile,
        });
    }
    {
        let mut runtime = state.bili.lock().expect("bili lock");
        if !cookies.buvid3.is_empty() {
            runtime.buvid3 = Some(cookies.buvid3.clone());
        }
        if !cookies.buvid4.is_empty() {
            runtime.buvid4 = Some(cookies.buvid4.clone());
        }
    }
    state.save_auth();
    state.publish_auth();
}

/// 常见接口错误码到中文文案的映射；-101 会同步标记登录过期。
pub(crate) fn api_error(state: &AppState, code: i64, message: &str) -> AppError {
    match code {
        -101 => {
            if is_logged_in(state) {
                state.mark_expired(Source::Bili);
            }
            AppError::msg("哔哩哔哩需要登录")
        }
        -352 => AppError::msg("请求触发风控，请稍后再试"),
        -412 => AppError::msg("请求被拦截，请稍后再试"),
        -799 => AppError::msg("请求过于频繁，请稍后再试"),
        -400 => AppError::msg("请求参数有误，请稍后再试"),
        -404 => AppError::msg("请求的内容不存在或已下架"),
        _ => {
            let detail = message.trim();
            if detail.is_empty() {
                AppError::msg(format!("哔哩哔哩请求失败（错误码 {code}）"))
            } else {
                AppError::msg(format!("哔哩哔哩请求失败：{detail}"))
            }
        }
    }
}

/// 把封面 / 头像等图片地址补全为可直接加载的 https 地址。
pub(crate) fn normalize_image_url(raw: &str) -> String {
    let raw = raw.trim();
    if let Some(rest) = raw.strip_prefix("//") {
        format!("https://{rest}")
    } else if let Some(rest) = raw.strip_prefix("http://") {
        format!("https://{rest}")
    } else {
        raw.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_image_urls() {
        assert_eq!(
            normalize_image_url("//i0.hdslb.com/bfs/archive/a.png"),
            "https://i0.hdslb.com/bfs/archive/a.png"
        );
        assert_eq!(
            normalize_image_url("http://i0.hdslb.com/bfs/archive/a.png"),
            "https://i0.hdslb.com/bfs/archive/a.png"
        );
        assert_eq!(
            normalize_image_url("https://i0.hdslb.com/bfs/archive/a.png"),
            "https://i0.hdslb.com/bfs/archive/a.png"
        );
        assert_eq!(normalize_image_url(""), "");
    }

    #[test]
    fn encodes_form_pairs() {
        assert_eq!(
            form_body(&[("keyword", "Rust & Tauri"), ("page", "1")]),
            "keyword=Rust%20%26%20Tauri&page=1"
        );
        assert_eq!(
            form_body(&[("tel", "13800138000"), ("code", "123456")]),
            "tel=13800138000&code=123456"
        );
    }
}
