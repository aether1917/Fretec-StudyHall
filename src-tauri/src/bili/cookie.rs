//! Cookie 解析与合并。
//!
//! 不使用 cookie_store：哔哩哔哩登录接口把凭据放在多个 `Set-Cookie`
//! 响应头里，这里手工解析名称与值，值保持原样（不做 percent decode）。

use reqwest::header::{HeaderMap, SET_COOKIE};

use crate::storage::BiliCookies;

/// 收集响应中的全部 `Set-Cookie` 头（保留原始行）。
pub(crate) fn collect_set_cookie(headers: &HeaderMap) -> Vec<String> {
    headers
        .get_all(SET_COOKIE)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .map(|value| value.to_string())
        .collect()
}

/// 解析单行 `name=value; Path=/; ...` 的 name/value。
pub(crate) fn parse_pair(raw: &str) -> Option<(&str, &str)> {
    let line = raw.split(';').next()?.trim();
    let (name, value) = line.split_once('=')?;
    let name = name.trim();
    if name.is_empty() {
        return None;
    }
    Some((name, value.trim()))
}

/// 按名称查找非空的 cookie 值。
pub(crate) fn find_value<'a>(headers: &'a [String], name: &str) -> Option<&'a str> {
    headers.iter().find_map(|raw| {
        let (key, value) = parse_pair(raw.as_str())?;
        if key.eq_ignore_ascii_case(name) && !value.is_empty() {
            Some(value)
        } else {
            None
        }
    })
}

/// 从 `Set-Cookie` 头构造登录凭据。
///
/// `DedeUserID__ckMd5` 也在服务端返回中，但本项目后续请求不需要它，
/// `BiliCookies` 也没有对应字段，因此这里只取实际会用到的 cookie。
pub(crate) fn cookies_from_set_cookie(headers: &[String]) -> BiliCookies {
    BiliCookies {
        sessdata: find_value(headers, "SESSDATA")
            .unwrap_or_default()
            .to_string(),
        bili_jct: find_value(headers, "bili_jct")
            .unwrap_or_default()
            .to_string(),
        dede_user_id: find_value(headers, "DedeUserID")
            .unwrap_or_default()
            .to_string(),
        buvid3: find_value(headers, "buvid3")
            .unwrap_or_default()
            .to_string(),
        buvid4: find_value(headers, "buvid4")
            .unwrap_or_default()
            .to_string(),
    }
}

/// 登录成功后合并 cookie：会话字段以新值为准，buvid 保留已有的。
pub(crate) fn merge_login_cookies(existing: &BiliCookies, incoming: &BiliCookies) -> BiliCookies {
    BiliCookies {
        sessdata: prefer(&incoming.sessdata, &existing.sessdata),
        bili_jct: prefer(&incoming.bili_jct, &existing.bili_jct),
        dede_user_id: prefer(&incoming.dede_user_id, &existing.dede_user_id),
        buvid3: prefer(&existing.buvid3, &incoming.buvid3),
        buvid4: prefer(&existing.buvid4, &incoming.buvid4),
    }
}

fn prefer(primary: &str, fallback: &str) -> String {
    if primary.is_empty() {
        fallback.to_string()
    } else {
        primary.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use reqwest::header::HeaderValue;

    #[test]
    fn parses_multiple_set_cookie_headers() {
        let mut headers = HeaderMap::new();
        headers.append(
            SET_COOKIE,
            HeaderValue::from_static(
                "SESSDATA=abc%2C123; Path=/; Domain=.bilibili.com; Secure; HttpOnly",
            ),
        );
        headers.append(
            SET_COOKIE,
            HeaderValue::from_static("bili_jct=deadbeef; Path=/"),
        );
        headers.append(
            SET_COOKIE,
            HeaderValue::from_static("DedeUserID=42; Path=/"),
        );
        headers.append(
            SET_COOKIE,
            HeaderValue::from_static("buvid3=xyz%3Dinfoc; Path=/; Expires=Wed, 21 Oct 2026"),
        );

        let pairs = collect_set_cookie(&headers);
        assert_eq!(pairs.len(), 4);
        assert_eq!(find_value(&pairs, "SESSDATA"), Some("abc%2C123"));
        assert_eq!(find_value(&pairs, "bili_jct"), Some("deadbeef"));

        let cookies = cookies_from_set_cookie(&pairs);
        assert_eq!(cookies.dede_user_id, "42");
        assert_eq!(cookies.buvid3, "xyz%3Dinfoc");
    }

    #[test]
    fn ignores_empty_and_malformed_cookie_values() {
        let pairs = vec![
            "SESSDATA=; Path=/".to_string(),
            "broken".to_string(),
            "=novalue; Path=/".to_string(),
        ];
        assert!(find_value(&pairs, "SESSDATA").is_none());
        assert!(find_value(&pairs, "broken").is_none());
    }

    #[test]
    fn keeps_existing_buvid_on_login() {
        let existing = BiliCookies {
            buvid3: "old-buvid3".to_string(),
            ..BiliCookies::default()
        };
        let incoming = BiliCookies {
            sessdata: "new-sessdata".to_string(),
            bili_jct: "new-jct".to_string(),
            dede_user_id: "7".to_string(),
            buvid3: "new-buvid3".to_string(),
            buvid4: "new-buvid4".to_string(),
        };

        let merged = merge_login_cookies(&existing, &incoming);
        assert_eq!(merged.sessdata, "new-sessdata");
        assert_eq!(merged.bili_jct, "new-jct");
        assert_eq!(merged.dede_user_id, "7");
        assert_eq!(merged.buvid3, "old-buvid3");
        assert_eq!(merged.buvid4, "new-buvid4");
    }
}
