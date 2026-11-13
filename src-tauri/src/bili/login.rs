//! 扫码、短信、密码登录与人机验证配置。
//!
//! 登录成功后统一走 [`complete_login`]：合并 cookie、从 nav 取昵称头像、
//! 写入 `auth.json` 并广播 `auth://state`。

use base64::Engine as _;
use rsa::pkcs8::DecodePublicKey;
use rsa::{Oaep, RsaPublicKey};
use serde_json::Value;

use crate::error::{AppError, CmdResult};
use crate::state::AppState;
use crate::storage::BiliCookies;
use crate::types::{AccountProfile, CaptchaConfig, QrLogin, QrPoll, QrState, SmsTicket};

use super::cookie;
use super::{
    api_get, execute, fallback_profile, fetch_profile_with_cookies, form_body, form_request,
    merge_for_login, persist_bili_auth, prepare_cookies, wbi,
};

const QR_GENERATE_URL: &str = "https://passport.bilibili.com/x/passport-login/web/qrcode/generate";
const QR_POLL_URL: &str = "https://passport.bilibili.com/x/passport-login/web/qrcode/poll";
const SMS_SEND_URL: &str = "https://passport.bilibili.com/x/passport-login/web/sms/send";
const SMS_LOGIN_URL: &str = "https://passport.bilibili.com/x/passport-login/web/login/sms";
const PASSWORD_KEY_URL: &str = "https://passport.bilibili.com/x/passport-login/web/key";
const PASSWORD_LOGIN_URL: &str = "https://passport.bilibili.com/x/passport-login/web/login";
const CAPTCHA_URL: &str = "https://passport.bilibili.com/x/passport-login/captcha";

/// 生成扫码登录二维码。
pub async fn qr_generate(state: &AppState) -> CmdResult<QrLogin> {
    let cookies = prepare_cookies(state).await?;
    let response = execute(state, api_get(state, QR_GENERATE_URL, &cookies)).await?;
    let code = response.code();
    if code != 0 {
        return Err(login_api_error("二维码生成", code, response.message()));
    }
    let data = response
        .data()
        .ok_or_else(|| AppError::msg("二维码生成失败，请稍后再试"))?;
    let url = data
        .get("url")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim()
        .to_string();
    let key = data
        .get("qrcode_key")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim()
        .to_string();
    if url.is_empty() || key.is_empty() {
        return Err(AppError::msg("二维码生成失败，请稍后再试"));
    }
    Ok(QrLogin {
        key,
        url,
        image: String::new(),
    })
}

/// 轮询扫码状态；成功时保存登录态并返回用户资料。
pub async fn qr_poll(state: &AppState, key: &str) -> CmdResult<QrPoll> {
    let key = key.trim();
    if key.is_empty() {
        return Err(AppError::msg("二维码已失效，请重新生成"));
    }
    let cookies = prepare_cookies(state).await?;
    let url = format!("{}?qrcode_key={}", QR_POLL_URL, wbi::encode_component(key));
    let response = execute(state, api_get(state, &url, &cookies)).await?;
    let code = response.code();
    if code != 0 {
        return Err(login_api_error("二维码状态查询", code, response.message()));
    }
    let data = response
        .data()
        .ok_or_else(|| AppError::msg("二维码状态查询失败，请重新生成二维码"))?;
    let device_code = data.get("code").and_then(Value::as_i64).unwrap_or(-1);

    match device_code {
        86101 => Ok(qr_state(QrState::Waiting, "等待扫码")),
        86090 => Ok(qr_state(QrState::Scanned, "已扫码，请在手机上确认")),
        86038 => Ok(qr_state(QrState::Expired, "二维码已过期，请刷新")),
        0 => {
            let incoming = cookie::cookies_from_set_cookie(response.set_cookies());
            if incoming.sessdata.is_empty() {
                return Err(AppError::msg("登录失败，未获取到登录凭据，请重试"));
            }
            let profile = complete_login(state, &incoming).await;
            Ok(QrPoll {
                state: QrState::Success,
                message: "登录成功".to_string(),
                profile: Some(profile),
            })
        }
        _ => Ok(qr_state(QrState::Error, "二维码状态异常，请刷新重试")),
    }
}

/// 发送短信验证码。
pub async fn sms_send(
    state: &AppState,
    tel: &str,
    cid: &str,
    challenge: &str,
    validate: &str,
    seccode: &str,
) -> CmdResult<SmsTicket> {
    let tel = tel.trim();
    if tel.is_empty() {
        return Err(AppError::msg("请输入手机号"));
    }
    let cid = default_cid(cid);
    let challenge = challenge.trim();
    let validate = validate.trim();
    let seccode = seccode.trim();
    let cookies = prepare_cookies(state).await?;
    let body = form_body(&[
        ("cid", cid),
        ("tel", tel),
        ("source", "main_web"),
        ("token", validate),
        ("challenge", challenge),
        ("validate", validate),
        ("seccode", seccode),
    ]);
    let response = execute(state, form_request(state, SMS_SEND_URL, &cookies, body)).await?;
    let code = response.code();
    if code != 0 {
        if code == -105 {
            return Err(AppError::msg("验证码错误，请重试"));
        }
        return Err(login_api_error("验证码发送", code, response.message()));
    }
    let data = response
        .data()
        .ok_or_else(|| AppError::msg("验证码发送失败，请稍后再试"))?;
    let captcha_key = data
        .get("captcha_key")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    Ok(SmsTicket {
        captcha_key,
        message: "验证码已发送".to_string(),
    })
}

/// 短信验证码登录。
pub async fn sms_login(
    state: &AppState,
    tel: &str,
    cid: &str,
    code: &str,
    captcha_key: &str,
) -> CmdResult<AccountProfile> {
    let tel = tel.trim();
    if tel.is_empty() {
        return Err(AppError::msg("请输入手机号"));
    }
    let code = code.trim();
    if code.is_empty() {
        return Err(AppError::msg("请输入短信验证码"));
    }
    let cid = default_cid(cid);
    let captcha_key = captcha_key.trim();
    let cookies = prepare_cookies(state).await?;
    let body = form_body(&[
        ("cid", cid),
        ("tel", tel),
        ("code", code),
        ("captcha_key", captcha_key),
        ("source", "main_web"),
    ]);
    let response = execute(state, form_request(state, SMS_LOGIN_URL, &cookies, body)).await?;
    let api_code = response.code();
    if api_code != 0 {
        return Err(sms_login_error(api_code, response.message()));
    }
    let (incoming, status) = {
        let incoming = cookie::cookies_from_set_cookie(response.set_cookies());
        let status = data_status(&response);
        (incoming, status)
    };
    if status != 0 {
        return Err(AppError::msg("短信登录失败，请稍后重试"));
    }
    if incoming.sessdata.is_empty() {
        return Err(AppError::msg("登录失败，未获取到登录凭据，请重试"));
    }
    Ok(complete_login(state, &incoming).await)
}

/// 账号密码登录；需要人机验证时返回 `NEED_CAPTCHA`。
pub async fn login_password(
    state: &AppState,
    username: &str,
    password: &str,
    captcha_token: Option<&str>,
) -> CmdResult<AccountProfile> {
    let username = username.trim();
    if username.is_empty() {
        return Err(AppError::msg("请输入手机号或邮箱"));
    }
    if password.is_empty() {
        return Err(AppError::msg("请输入密码"));
    }

    let cookies = prepare_cookies(state).await?;
    let response = execute(state, api_get(state, PASSWORD_KEY_URL, &cookies)).await?;
    let code = response.code();
    if code != 0 {
        return Err(login_api_error("登录密钥获取", code, response.message()));
    }
    let data = response
        .data()
        .ok_or_else(|| AppError::msg("登录密钥获取失败，请稍后再试"))?;
    let hash = data
        .get("hash")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim();
    let public_key = data.get("key").and_then(Value::as_str).unwrap_or("").trim();
    if hash.is_empty() || public_key.is_empty() {
        return Err(AppError::msg("登录密钥获取失败，请稍后再试"));
    }

    let encrypted = encrypt_password(public_key, &format!("{password}{hash}"))?;

    // captcha_token 的格式为 "challenge|validate|seccode"。
    let mut challenge = "";
    let mut validate = "";
    let mut seccode = "";
    if let Some(token) = captcha_token {
        let token = token.trim();
        if !token.is_empty() {
            let mut parts = token.splitn(3, '|');
            challenge = parts.next().unwrap_or("");
            validate = parts.next().unwrap_or("");
            seccode = parts.next().unwrap_or("");
            if validate.is_empty() {
                return Err(AppError::msg("人机验证参数无效，请重新完成验证"));
            }
        }
    }

    let mut form: Vec<(&str, &str)> = vec![
        ("username", username),
        ("password", encrypted.as_str()),
        ("keep", "true"),
        ("source", "main_web"),
    ];
    if !validate.is_empty() {
        form.push(("token", validate));
        form.push(("challenge", challenge));
        form.push(("validate", validate));
        form.push(("seccode", seccode));
    }

    let body = form_body(&form);
    let response = execute(
        state,
        form_request(state, PASSWORD_LOGIN_URL, &cookies, body),
    )
    .await?;
    let api_code = response.code();
    let message = response.message();
    let status = data_status(&response);
    if is_need_captcha(api_code, status, message) {
        return Err(AppError::msg("NEED_CAPTCHA"));
    }
    if api_code != 0 {
        return Err(password_login_error(api_code, message));
    }
    if status != 0 {
        return Err(AppError::msg("登录失败，请稍后重试"));
    }

    let incoming = cookie::cookies_from_set_cookie(response.set_cookies());
    if incoming.sessdata.is_empty() {
        return Err(AppError::msg("登录失败，未获取到登录凭据，请重试"));
    }
    Ok(complete_login(state, &incoming).await)
}

/// 获取极验人机验证配置。
pub async fn captcha_config(state: &AppState) -> CmdResult<CaptchaConfig> {
    let cookies = prepare_cookies(state).await?;
    let url = format!("{CAPTCHA_URL}?source=main_web");
    let response = execute(state, api_get(state, &url, &cookies)).await?;
    let code = response.code();
    if code != 0 {
        return Err(login_api_error("验证码配置获取", code, response.message()));
    }
    let data = response
        .data()
        .ok_or_else(|| AppError::msg("验证码配置获取失败，请稍后再试"))?;
    let kind = data
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim();
    let gt = data
        .get("gt")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim()
        .to_string();
    let challenge = data
        .get("challenge")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim()
        .to_string();
    let success = data.get("success").map(as_u32).unwrap_or(0);
    Ok(CaptchaConfig {
        kind: if kind.is_empty() {
            "geetest".to_string()
        } else {
            kind.to_string()
        },
        gt,
        challenge,
        success,
    })
}

/// 合并 cookie、拉取资料、保存并广播登录态。
async fn complete_login(state: &AppState, incoming: &BiliCookies) -> AccountProfile {
    let merged = merge_for_login(state, incoming);
    let profile = match fetch_profile_with_cookies(state, &merged.header()).await {
        Ok(profile) => profile,
        Err(_) => fallback_profile(),
    };
    persist_bili_auth(state, merged, profile.clone());
    profile
}

fn qr_state(state: QrState, message: &str) -> QrPoll {
    QrPoll {
        state,
        message: message.to_string(),
        profile: None,
    }
}

fn default_cid(cid: &str) -> &str {
    let cid = cid.trim();
    if cid.is_empty() {
        "86"
    } else {
        cid
    }
}

fn data_status(response: &super::BiliResponse) -> i64 {
    response
        .data()
        .and_then(|data| data.get("status"))
        .and_then(Value::as_i64)
        .unwrap_or(0)
}

fn as_u32(value: &Value) -> u32 {
    if let Some(number) = value.as_u64() {
        number.min(u32::MAX as u64) as u32
    } else if let Some(flag) = value.as_bool() {
        if flag {
            1
        } else {
            0
        }
    } else {
        0
    }
}

fn is_need_captcha(code: i64, status: i64, message: &str) -> bool {
    if status == 2 || code == -105 {
        return true;
    }
    let message = message.trim();
    message.contains("验证")
        || message.contains("风控")
        || message.to_ascii_lowercase().contains("captcha")
}

fn login_api_error(action: &str, code: i64, message: &str) -> AppError {
    let detail = message.trim();
    match code {
        -105 => AppError::msg("验证码错误，请重试"),
        -629 => AppError::msg("账号或密码错误"),
        -627 => AppError::msg("账号状态异常，暂时无法登录"),
        -352 => AppError::msg("请求触发风控，请稍后再试"),
        _ => {
            if detail.is_empty() || detail == "0" {
                AppError::msg(format!("{action}失败，请稍后再试"))
            } else {
                AppError::msg(format!("{action}失败：{detail}"))
            }
        }
    }
}

fn sms_login_error(code: i64, message: &str) -> AppError {
    match code {
        86203 | -105 => AppError::msg("验证码错误或已过期，请重新获取"),
        86202 => AppError::msg("验证码已过期，请重新获取"),
        86201 => AppError::msg("手机号格式不正确，请检查后重试"),
        _ => login_api_error("短信登录", code, message),
    }
}

fn password_login_error(code: i64, message: &str) -> AppError {
    match code {
        -629 => AppError::msg("账号或密码错误"),
        -627 => AppError::msg("账号状态异常，暂时无法登录"),
        _ => login_api_error("登录", code, message),
    }
}

fn encrypt_password(public_key_pem: &str, plaintext: &str) -> CmdResult<String> {
    let public_key = RsaPublicKey::from_public_key_pem(public_key_pem.trim())
        .map_err(|_| AppError::msg("登录密钥解析失败，请稍后再试"))?;
    let mut rng = rand::thread_rng();
    let padding = Oaep::new::<sha2::Sha256>();
    let encrypted = public_key
        .encrypt(&mut rng, padding, plaintext.as_bytes())
        .map_err(|_| AppError::msg("密码加密失败，请稍后再试"))?;
    Ok(encode_url_safe_base64(&encrypted))
}

/// 标准 base64 后做 URL-safe 替换并去掉 `=` 填充。
fn encode_url_safe_base64(bytes: &[u8]) -> String {
    let encoded = base64::engine::general_purpose::STANDARD.encode(bytes);
    encoded
        .replace('+', "-")
        .replace('/', "_")
        .trim_end_matches('=')
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_password_payload_as_url_safe_base64() {
        assert_eq!(encode_url_safe_base64(&[0xfb, 0xff]), "-_8");
        assert_eq!(encode_url_safe_base64(b"secret"), "c2VjcmV0");
    }

    #[test]
    fn detects_captcha_requirement() {
        assert!(is_need_captcha(0, 2, ""));
        assert!(is_need_captcha(-105, 0, "验证码错误"));
        assert!(is_need_captcha(0, 0, "需要完成验证后登录"));
        assert!(!is_need_captcha(-629, 0, "账号或密码错误"));
        assert!(!is_need_captcha(0, 0, ""));
    }

    #[test]
    fn normalizes_captcha_success_flag() {
        assert_eq!(as_u32(&serde_json::json!(1)), 1);
        assert_eq!(as_u32(&serde_json::json!(true)), 1);
        assert_eq!(as_u32(&serde_json::json!("x")), 0);
    }
}
