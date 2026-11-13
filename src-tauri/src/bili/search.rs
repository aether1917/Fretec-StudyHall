//! 视频搜索。

use serde_json::Value;

use crate::error::{AppError, CmdResult};
use crate::state::AppState;
use crate::types::{SearchPage, SearchResultItem, Source};

use super::{
    api_error, api_get, ensure_mixin_key, execute, normalize_image_url, prepare_cookies,
    refresh_buvid, wbi, BiliResponse,
};

const SEARCH_URL: &str = "https://api.bilibili.com/x/web-interface/wbi/search/type";
const PAGE_SIZE: u32 = 20;

/// 搜索视频，失败时返回中文可读错误。
pub async fn search(state: &AppState, keyword: &str, page: u32) -> CmdResult<SearchPage> {
    let keyword = keyword.trim();
    if keyword.is_empty() {
        return Err(AppError::msg("请输入搜索关键词"));
    }
    let page = page.max(1);

    // -412 通常表示匿名 cookie 不足，补取 buvid3 后重试一次。
    let mut refreshed = false;
    loop {
        let response = search_request(state, keyword, page).await?;
        let code = response.code();
        if code == 0 {
            return Ok(parse_page(&response, page));
        }
        if code == -412 && !refreshed {
            refreshed = true;
            refresh_buvid(state).await?;
            continue;
        }
        return Err(api_error(state, code, response.message()));
    }
}

async fn search_request(state: &AppState, keyword: &str, page: u32) -> CmdResult<BiliResponse> {
    let cookies = prepare_cookies(state).await?;
    let mixin_key = ensure_mixin_key(state).await?;
    let params: Vec<(&str, String)> = vec![
        ("search_type", "video".to_string()),
        ("keyword", keyword.to_string()),
        ("page", page.to_string()),
        ("page_size", PAGE_SIZE.to_string()),
    ];
    let query = wbi::signed_query(&mixin_key, &params, super::now_secs());
    let url = format!("{SEARCH_URL}?{query}");
    execute(state, api_get(state, &url, &cookies)).await
}

fn parse_page(response: &BiliResponse, page: u32) -> SearchPage {
    let Some(data) = response.data() else {
        return SearchPage {
            items: Vec::new(),
            page,
            has_more: false,
            total: 0,
        };
    };

    let items: Vec<SearchResultItem> = data
        .get("result")
        .and_then(Value::as_array)
        .map(|rows| rows.iter().filter_map(parse_item).collect())
        .unwrap_or_default();
    let total = data
        .get("total")
        .or_else(|| data.get("numResults"))
        .or_else(|| data.get("num_results"))
        .and_then(Value::as_u64)
        .unwrap_or(0)
        .min(u32::MAX as u64) as u32;
    let num_pages = data
        .get("numPages")
        .or_else(|| data.get("num_pages"))
        .and_then(Value::as_u64)
        .unwrap_or(0);

    let has_more = if items.is_empty() {
        false
    } else if num_pages > 0 {
        (page as u64) < num_pages
    } else if total > 0 {
        (page as u64) * (PAGE_SIZE as u64) < total as u64
    } else {
        items.len() >= PAGE_SIZE as usize
    };

    SearchPage {
        items,
        page,
        has_more,
        total,
    }
}

fn parse_item(raw: &Value) -> Option<SearchResultItem> {
    let bvid = raw.get("bvid").and_then(Value::as_str).unwrap_or("").trim();
    if bvid.is_empty() {
        return None;
    }
    let title = clean_text(raw.get("title").and_then(Value::as_str).unwrap_or(""));
    let author = raw
        .get("author")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim()
        .to_string();
    let cover = normalize_image_url(raw.get("pic").and_then(Value::as_str).unwrap_or(""));
    let duration_sec = raw.get("duration").map(duration_seconds).unwrap_or(0.0);

    Some(SearchResultItem {
        source: Source::Bili,
        source_id: bvid.to_string(),
        title,
        author,
        cover,
        duration_sec,
    })
}

/// 搜索接口的 `duration` 可能是 "mm:ss" / "hh:mm:ss" 字符串，也可能是数字秒。
fn duration_seconds(value: &Value) -> f64 {
    if let Some(text) = value.as_str() {
        parse_duration_sec(text)
    } else {
        value.as_f64().unwrap_or(0.0)
    }
}

fn parse_duration_sec(raw: &str) -> f64 {
    let text = raw.trim();
    if text.is_empty() {
        return 0.0;
    }
    let mut total: u64 = 0;
    let mut parts: u32 = 0;
    for part in text.split(':') {
        let value = match part.trim().parse::<u64>() {
            Ok(value) => value,
            Err(_) => return 0.0,
        };
        if parts == 3 {
            return 0.0;
        }
        total = total.saturating_mul(60).saturating_add(value);
        parts += 1;
    }
    if parts == 0 {
        0.0
    } else {
        total as f64
    }
}

/// 去掉搜索结果标题里的高亮标签并反转义 HTML 实体。
fn clean_text(raw: &str) -> String {
    decode_entities(&strip_tags(raw)).trim().to_string()
}

fn strip_tags(raw: &str) -> String {
    let mut output = String::with_capacity(raw.len());
    let mut inside_tag = false;
    for character in raw.chars() {
        match character {
            '<' => inside_tag = true,
            '>' if inside_tag => inside_tag = false,
            _ if !inside_tag => output.push(character),
            _ => {}
        }
    }
    output
}

fn decode_entities(raw: &str) -> String {
    let mut output = String::with_capacity(raw.len());
    let mut rest = raw;
    while let Some(index) = rest.find('&') {
        output.push_str(&rest[..index]);
        let after = &rest[index + 1..];
        match after.find(';') {
            Some(end) if end > 0 && end <= 10 => match decode_entity(&after[..end]) {
                Some(character) => {
                    output.push(character);
                    rest = &after[end + 1..];
                }
                None => {
                    output.push('&');
                    rest = after;
                }
            },
            _ => {
                output.push('&');
                rest = after;
            }
        }
    }
    output.push_str(rest);
    output
}

fn decode_entity(entity: &str) -> Option<char> {
    match entity {
        "amp" => Some('&'),
        "lt" => Some('<'),
        "gt" => Some('>'),
        "quot" => Some('"'),
        "apos" => Some('\''),
        "nbsp" => Some(' '),
        _ => {
            if let Some(hex) = entity
                .strip_prefix("#x")
                .or_else(|| entity.strip_prefix("#X"))
            {
                u32::from_str_radix(hex, 16).ok().and_then(char::from_u32)
            } else if let Some(decimal) = entity.strip_prefix('#') {
                decimal.parse::<u32>().ok().and_then(char::from_u32)
            } else {
                None
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_minute_second_duration() {
        assert_eq!(parse_duration_sec("12:34"), 754.0);
        assert_eq!(parse_duration_sec("00:05"), 5.0);
        assert_eq!(parse_duration_sec(" 3:07 "), 187.0);
    }

    #[test]
    fn parses_hour_duration() {
        assert_eq!(parse_duration_sec("1:02:03"), 3723.0);
        assert_eq!(parse_duration_sec("0:00:09"), 9.0);
    }

    #[test]
    fn rejects_invalid_duration() {
        assert_eq!(parse_duration_sec(""), 0.0);
        assert_eq!(parse_duration_sec("abc"), 0.0);
        assert_eq!(parse_duration_sec("1:2:3:4"), 0.0);
        assert_eq!(parse_duration_sec("12:ab"), 0.0);
    }

    #[test]
    fn strips_highlight_tags_and_entities() {
        assert_eq!(
            clean_text(r#"<em class="keyword">Rust</em> &amp; Tauri"#),
            "Rust & Tauri"
        );
        assert_eq!(clean_text("A &lt; B &#38; C"), "A < B & C");
        assert_eq!(clean_text("&unknown; &#x1F600;"), "&unknown; 😀");
    }

    #[test]
    fn builds_search_item_from_json() {
        let raw = serde_json::json!({
            "bvid": "BV1xx411c7mD",
            "title": "<em class=\"keyword\">线性代数</em> 01",
            "author": "某 UP 主",
            "pic": "//i0.hdslb.com/bfs/archive/cover.jpg",
            "duration": "10:25"
        });
        let item = parse_item(&raw).expect("item");
        assert_eq!(item.source, Source::Bili);
        assert_eq!(item.source_id, "BV1xx411c7mD");
        assert_eq!(item.title, "线性代数 01");
        assert_eq!(item.author, "某 UP 主");
        assert_eq!(item.cover, "https://i0.hdslb.com/bfs/archive/cover.jpg");
        assert_eq!(item.duration_sec, 625.0);
    }
}
