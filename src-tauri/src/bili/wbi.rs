//! WBI 签名实现。
//!
//! 流程：从 nav 返回的 `wbi_img.img_url` / `sub_url` 取文件名得到
//! `img_key` / `sub_key`，拼接后按 `MIXIN_KEY_ENC_TAB` 重排取前 32 位
//! 得到 mixin key；参数排序编码后追加 `wts`，再取 MD5 作为 `w_rid`。

use percent_encoding::{utf8_percent_encode, AsciiSet, NON_ALPHANUMERIC};

/// 官方前端使用的字符重排表。
const MIXIN_KEY_ENC_TAB: [usize; 64] = [
    46, 47, 18, 2, 53, 8, 23, 32, 15, 50, 10, 31, 58, 3, 45, 35, 27, 43, 5, 49, 33, 9, 42, 19, 29,
    28, 14, 39, 12, 38, 41, 13, 37, 48, 7, 16, 24, 55, 40, 61, 26, 17, 0, 1, 60, 51, 30, 4, 22, 25,
    54, 21, 56, 59, 6, 63, 57, 62, 11, 36, 20, 34, 44, 52,
];

/// 与 JavaScript `encodeURIComponent` 一致的字符集。
const URI_COMPONENT_SET: &AsciiSet = &NON_ALPHANUMERIC
    .remove(b'-')
    .remove(b'_')
    .remove(b'.')
    .remove(b'~')
    .remove(b'!')
    .remove(b'*')
    .remove(b'\'')
    .remove(b'(')
    .remove(b')');

/// 从 `img_url` / `sub_url` 推导 mixin key。
///
/// 任一 URL 不含文件名或拼接后的长度不足以完成重排时返回 `None`。
pub(crate) fn mixin_key_from_urls(img_url: &str, sub_url: &str) -> Option<String> {
    let img_key = file_stem(img_url)?;
    let sub_key = file_stem(sub_url)?;
    let raw = format!("{img_key}{sub_key}");
    let mut mixin_key = String::with_capacity(32);
    for index in MIXIN_KEY_ENC_TAB.iter().take(32) {
        mixin_key.push(raw.chars().nth(*index)?);
    }
    Some(mixin_key)
}

fn file_stem(url: &str) -> Option<&str> {
    let path = url.split('?').next().unwrap_or(url);
    let path = path.split('#').next().unwrap_or(path);
    let name = path.rsplit('/').next()?;
    let stem = name.split('.').next()?;
    if stem.is_empty() {
        None
    } else {
        Some(stem)
    }
}

/// 与前端一致的 URL 编码（空格编码为 `%20`）。
pub(crate) fn encode_component(value: &str) -> String {
    utf8_percent_encode(value, URI_COMPONENT_SET).to_string()
}

/// 剔除 WBI 参数值中会导致签名不一致的字符。
fn sanitize(value: &str) -> String {
    value
        .chars()
        .filter(|character| !matches!(character, '!' | '\'' | '(' | ')' | '*'))
        .collect()
}

/// 生成带 `wts` 与 `w_rid` 的查询串。
///
/// `params` 中值为空的参数会被丢弃，所有参数（含 `wts`）按 key 升序排列，
/// 使用 URL 编码拼接后，对 `query + mixin_key` 取 MD5 得到 `w_rid`。
pub(crate) fn signed_query(mixin_key: &str, params: &[(&str, String)], wts: u64) -> String {
    let mut entries: Vec<(String, String)> = params
        .iter()
        .filter(|(_, value)| !value.is_empty())
        .map(|(key, value)| ((*key).to_string(), sanitize(value.as_str())))
        .collect();
    entries.push(("wts".to_string(), wts.to_string()));
    entries.sort_by(|left, right| left.0.cmp(&right.0));

    let query = entries
        .iter()
        .map(|(key, value)| format!("{}={}", encode_component(key), encode_component(value)))
        .collect::<Vec<String>>()
        .join("&");

    let digest = md5::compute(format!("{query}{mixin_key}").as_bytes());
    format!("{query}&w_rid={digest:x}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn derives_mixin_key_from_nav_urls() {
        let img_url = "https://i0.hdslb.com/bfs/wbi/7cd084941338484aae1ad9425b84077c.png";
        let sub_url = "https://i0.hdslb.com/bfs/wbi/4932caff0ff746eab6f01bf08b70ac45.png";
        assert_eq!(
            mixin_key_from_urls(img_url, sub_url).as_deref(),
            Some("ea1db124af3c7062474693fa704f4ff8")
        );
    }

    #[test]
    fn derives_mixin_key_without_query_or_dot() {
        let img_url = "https://i0.hdslb.com/bfs/wbi/7cd084941338484aae1ad9425b84077c";
        let sub_url = "https://i0.hdslb.com/bfs/wbi/4932caff0ff746eab6f01bf08b70ac45.png?v=1";
        assert_eq!(
            mixin_key_from_urls(img_url, sub_url).as_deref(),
            Some("ea1db124af3c7062474693fa704f4ff8")
        );
        assert!(mixin_key_from_urls("", sub_url).is_none());
    }

    #[test]
    fn signs_query_like_bilibili() {
        let params = vec![
            ("foo", "114".to_string()),
            ("bar", "514".to_string()),
            ("zab", "1919810".to_string()),
        ];
        assert_eq!(
            signed_query("ea1db124af3c7062474693fa704f4ff8", &params, 1702204169),
            "bar=514&foo=114&wts=1702204169&zab=1919810&w_rid=8f6f2b5b3d485fe1886cec6a0be8c5d4"
        );
    }

    #[test]
    fn strips_forbidden_characters_and_empty_values() {
        let params = vec![
            ("keyword", "Rust!(study)*'".to_string()),
            ("empty", String::new()),
        ];
        let signed = signed_query("key", &params, 1);
        assert!(signed.starts_with("keyword=Ruststudy&wts=1&w_rid="));
        assert_eq!(signed.len(), "keyword=Ruststudy&wts=1&w_rid=".len() + 32);
    }

    #[test]
    fn encodes_utf8_and_spaces() {
        assert_eq!(
            encode_component("中文 测试"),
            "%E4%B8%AD%E6%96%87%20%E6%B5%8B%E8%AF%95"
        );
    }
}
