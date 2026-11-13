//! 网易云登录：二维码、短信验证码、手机号密码。
//!
//! 登录成功后统一通过 [`save_login`] 保存 `MUSIC_U` / `__csrf` 与账号资料，
//! 并广播登录态；任何日志/错误里都不包含口令、验证码或 MUSIC_U。

use serde_json::{json, Value};

use super::{
    current_cookies, fetch_account_profile, is_login_error, json_code, post_form, post_weapi,
    profile_from_json, save_login, weapi,
};
use crate::error::{AppError, CmdResult};
use crate::state::AppState;
use crate::storage::NeteaseCookies;
use crate::types::{AccountProfile, QrLogin, QrPoll, QrState, Source};

const QR_UNIKEY_URL: &str = "https://interface.music.163.com/api/login/qrcode/unikey";
const QR_POLL_URL: &str = "https://interface.music.163.com/api/login/qrcode/client/login";
const SMS_SEND_URL: &str = "https://interface.music.163.com/api/sms/captcha/sent";
const CELLPHONE_LOGIN_URL: &str = "https://music.163.com/weapi/login/cellphone";

/// 生成扫码登录二维码（`unikey` + 登录链接）。
pub(crate) async fn run_qr_generate(state: &AppState) -> CmdResult<QrLogin> {
    let response = post_form(state, QR_UNIKEY_URL, "type=3".to_string(), None).await?;
    if let Some(code) = json_code(&response.json) {
        if code != 200 {
            return Err(AppError::msg("获取网易云登录二维码失败，请稍后重试"));
        }
    }
    let unikey = response
        .json
        .get("unikey")
        .and_then(Value::as_str)
        .or_else(|| {
            response
                .json
                .pointer("/response/unikey")
                .and_then(Value::as_str)
        })
        .unwrap_or("")
        .trim()
        .to_string();
    if unikey.is_empty() {
        return Err(AppError::msg("获取网易云登录二维码失败，请稍后重试"));
    }
    Ok(QrLogin {
        url: format!(
            "https://music.163.com/login?codekey={}",
            urlencoding::encode(&unikey)
        ),
        key: unikey,
        image: String::new(),
    })
}

/// 轮询扫码状态：800 过期 / 801 未扫码 / 802 待确认 / 803 成功。
pub(crate) async fn run_qr_poll(state: &AppState, key: &str) -> CmdResult<QrPoll> {
    let key = key.trim();
    if key.is_empty() {
        return Err(AppError::msg("登录二维码已失效，请重新获取"));
    }
    let body = format!("key={}&type=3", urlencoding::encode(key));
    let response = post_form(state, QR_POLL_URL, body, None).await?;
    let code = json_code(&response.json).unwrap_or(-1);
    match code {
        800 => Ok(qr_poll_state(QrState::Expired, "二维码已过期，请刷新重试")),
        801 => Ok(qr_poll_state(QrState::Waiting, "等待手机扫码")),
        802 => Ok(qr_poll_state(
            QrState::Scanned,
            "已扫码，请在手机上确认登录",
        )),
        803 => {
            let mut cookies = NeteaseCookies::default();
            response.cookies.apply(&mut cookies);
            if cookies.music_u.is_empty() {
                return Ok(qr_poll_state(
                    QrState::Error,
                    "登录失败：未获取到登录凭据，请重试",
                ));
            }
            let profile = fetch_account_profile(state, &cookies)
                .await
                .unwrap_or_default();
            save_login(state, cookies, profile.clone());
            Ok(QrPoll {
                state: QrState::Success,
                message: "登录成功".to_string(),
                profile: Some(profile),
            })
        }
        _ => Ok(qr_poll_state(
            QrState::Error,
            "二维码登录状态异常，请刷新重试",
        )),
    }
}

fn qr_poll_state(qr_state: QrState, message: &str) -> QrPoll {
    QrPoll {
        state: qr_state,
        message: message.to_string(),
        profile: None,
    }
}

/// 发送短信验证码。
pub(crate) async fn run_sms_send(state: &AppState, phone: &str) -> CmdResult<()> {
    let phone = normalize_phone(phone)?;
    let cookies = current_cookies(state);
    let body = format!(
        "cellphone={}&ctcode=86&csrf_token={}",
        urlencoding::encode(&phone),
        urlencoding::encode(&cookies.csrf)
    );
    let response = post_form(state, SMS_SEND_URL, body, Some(&cookies)).await?;
    let code = json_code(&response.json).unwrap_or(-1);
    if code == 200 {
        return Ok(());
    }
    Err(AppError::msg(sms_error_message(code)))
}

/// 短信验证码登录。
pub(crate) async fn run_sms_login(
    state: &AppState,
    phone: &str,
    code: &str,
) -> CmdResult<AccountProfile> {
    let phone = normalize_phone(phone)?;
    let code = code.trim();
    if code.is_empty() {
        return Err(AppError::msg("请输入验证码"));
    }
    let payload = json!({
        "phone": phone,
        "countrycode": "86",
        "captcha": code,
        "rememberLogin": "true",
    });
    login_with_payload(state, &payload, false).await
}

/// 手机号 + 密码登录；密码先做一次 MD5 再参与 weapi 加密。
pub(crate) async fn run_login_password(
    state: &AppState,
    phone: &str,
    password: &str,
) -> CmdResult<AccountProfile> {
    let phone = normalize_phone(phone)?;
    if password.is_empty() {
        return Err(AppError::msg("请输入密码"));
    }
    let payload = json!({
        "phone": phone,
        "countrycode": "86",
        "password": weapi::md5_hex(password),
        "rememberLogin": "true",
    });
    login_with_payload(state, &payload, true).await
}

async fn login_with_payload(
    state: &AppState,
    payload: &Value,
    password_mode: bool,
) -> CmdResult<AccountProfile> {
    let cookies = current_cookies(state);
    let response = post_weapi(state, CELLPHONE_LOGIN_URL, payload, &cookies).await?;

    let code = json_code(&response.json);
    if is_login_error(code) {
        state.mark_expired(Source::Netease);
        return Err(AppError::msg("网易云登录状态已失效，请重新登录"));
    }
    let code = code.unwrap_or(-1);
    if code != 200 {
        return Err(AppError::msg(login_error_message(code, password_mode)));
    }

    let mut cookies = NeteaseCookies::default();
    response.cookies.apply(&mut cookies);
    if cookies.music_u.is_empty() {
        return Err(AppError::msg("登录失败：未获取到登录凭据，请稍后重试"));
    }

    let profile = match profile_from_json(&response.json) {
        Some(profile) => profile,
        None => fetch_account_profile(state, &cookies)
            .await
            .unwrap_or_default(),
    };
    save_login(state, cookies, profile.clone());
    Ok(profile)
}

/// 去掉空格与连字符后校验中国大陆手机号（11 位数字）。
fn normalize_phone(phone: &str) -> CmdResult<String> {
    let digits: String = phone
        .chars()
        .filter(|ch| !ch.is_whitespace() && *ch != '-')
        .collect();
    if digits.is_empty() {
        return Err(AppError::msg("请输入手机号"));
    }
    if digits.len() != 11 || !digits.chars().all(|ch| ch.is_ascii_digit()) {
        return Err(AppError::msg("手机号格式不正确"));
    }
    Ok(digits)
}

fn sms_error_message(code: i64) -> &'static str {
    match code {
        400 => "手机号格式错误或请求参数不正确",
        503 => "验证码错误或已失效，请重新获取",
        -460 | 429 => "操作过于频繁，请稍后再试",
        _ => "短信发送失败，请稍后重试",
    }
}

fn login_error_message(code: i64, password_mode: bool) -> &'static str {
    if password_mode {
        match code {
            400 => "手机号格式错误或请求参数不正确",
            501 => "该手机号尚未注册网易云音乐账号",
            502 => "账号或密码不正确",
            -460 | 429 => "操作过于频繁，请稍后再试",
            _ => "网易云账号或密码登录失败，请稍后重试",
        }
    } else {
        match code {
            400 => "手机号格式错误或请求参数不正确",
            501 => "该手机号尚未注册网易云音乐账号",
            502 | 503 => "验证码错误或已失效，请重新获取",
            -460 | 429 => "操作过于频繁，请稍后再试",
            _ => "网易云短信登录失败，请稍后重试",
        }
    }
}
