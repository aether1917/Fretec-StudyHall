//! 网易云登录 cookie 解析。
//!
//! 工程没有引入 `cookie_store`，这里直接从响应头的 `Set-Cookie` 中手工提取
//! 登录所需的 `MUSIC_U` 与 `__csrf`。

use reqwest::header::{HeaderMap, SET_COOKIE};

use crate::storage::NeteaseCookies;

/// 从一次响应中解析出来的登录 cookie。
#[derive(Debug, Clone, Default)]
pub(crate) struct CookieJar {
    pub(crate) music_u: Option<String>,
    pub(crate) csrf: Option<String>,
}

impl CookieJar {
    /// 遍历所有 `Set-Cookie` 头，忽略解析失败的条目。
    pub(crate) fn from_headers(headers: &HeaderMap) -> Self {
        let mut jar = Self::default();
        for value in headers.get_all(SET_COOKIE) {
            let raw = match value.to_str() {
                Ok(text) => text,
                Err(_) => continue,
            };
            jar.absorb(raw);
        }
        jar
    }

    /// 把解析结果写入已有的 [`NeteaseCookies`]，只覆盖拿到的字段。
    pub(crate) fn apply(&self, target: &mut NeteaseCookies) {
        if let Some(value) = &self.music_u {
            target.music_u = value.clone();
        }
        if let Some(value) = &self.csrf {
            target.csrf = value.clone();
        }
    }

    fn absorb(&mut self, raw: &str) {
        // 只关心 "name=value" 这一段，后面的 Path/Expires/Max-Age 全部忽略。
        let pair = raw.split(';').next().unwrap_or("");
        let (name, value) = match pair.split_once('=') {
            Some(parts) => parts,
            None => return,
        };
        let value = clean_value(value);
        if value.is_empty() {
            return;
        }
        match name.trim() {
            "MUSIC_U" => self.music_u = Some(value),
            "__csrf" => self.csrf = Some(value),
            _ => {}
        }
    }
}

/// 去掉首尾空白与可能存在的引号。
fn clean_value(value: &str) -> String {
    value.trim().trim_matches('"').to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use reqwest::header::HeaderValue;

    #[test]
    fn parses_music_u_and_csrf_from_set_cookie() {
        let mut headers = HeaderMap::new();
        headers.append(
            SET_COOKIE,
            HeaderValue::from_static("MUSIC_U=abc123; Max-Age=1296000; Path=/; HttpOnly"),
        );
        headers.append(
            SET_COOKIE,
            HeaderValue::from_static("__csrf=def456; Path=/"),
        );
        headers.append(
            SET_COOKIE,
            HeaderValue::from_static("NMTID=ignored; Path=/"),
        );

        let jar = CookieJar::from_headers(&headers);
        let mut cookies = NeteaseCookies::default();
        jar.apply(&mut cookies);
        assert_eq!(cookies.music_u, "abc123");
        assert_eq!(cookies.csrf, "def456");
    }

    #[test]
    fn ignores_malformed_set_cookie() {
        let mut headers = HeaderMap::new();
        headers.append(SET_COOKIE, HeaderValue::from_static("MUSIC_U"));
        headers.append(SET_COOKIE, HeaderValue::from_static("__csrf=; Path=/"));
        let jar = CookieJar::from_headers(&headers);
        assert!(jar.music_u.is_none());
        assert!(jar.csrf.is_none());
    }
}
