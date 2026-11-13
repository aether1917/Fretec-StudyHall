//! 网易云搜索与播放地址。

use serde_json::{json, Value};

use super::{current_cookies, https_url, is_login_error, json_code, post_weapi, NeteasePlayInfo};
use crate::error::{AppError, CmdResult};
use crate::state::AppState;
use crate::storage::NeteaseCookies;
use crate::types::{SearchPage, SearchResultItem, Source};

const PAGE_SIZE: u32 = 20;
const CLOUDSEARCH_URL: &str = "https://music.163.com/weapi/cloudsearch/get/web";
const PLAYER_URL: &str = "https://music.163.com/weapi/song/enhance/player/url";
const SONG_DETAIL_URL: &str = "https://music.163.com/weapi/v3/song/detail";

/// 搜索单曲（type = 1），固定每页 20 条。
pub(crate) async fn run_search(
    state: &AppState,
    keyword: &str,
    page: u32,
) -> CmdResult<SearchPage> {
    let keyword = keyword.trim();
    if keyword.is_empty() {
        return Err(AppError::msg("请输入搜索关键词"));
    }
    let page = page.max(1);
    let cookies = current_cookies(state);
    let offset = u64::from(page - 1) * u64::from(PAGE_SIZE);
    if let Some(result) = fallback_public_search(state, keyword, page, offset).await {
        return Ok(result);
    }
    let payload = json!({
        "s": keyword,
        "type": 1,
        "limit": PAGE_SIZE,
        "offset": offset,
        "total": true,
    });
    let response = post_weapi(state, CLOUDSEARCH_URL, &payload, &cookies).await?;

    let code = json_code(&response.json);
    if is_login_error(code) {
        state.mark_expired(Source::Netease);
        return Err(AppError::msg("网易云登录状态已失效，请重新登录"));
    }
    if let Some(code) = code {
        if code != 200 && code != 0 {
            let detail = response
                .json
                .get("message")
                .and_then(Value::as_str)
                .unwrap_or("")
                .trim();
            return Err(AppError::msg(format!(
                "网易云音乐搜索失败（{code}）{}",
                if detail.is_empty() { String::new() } else { format!("：{detail}") }
            )));
        }
    }

    let result = match response.json.get("result") {
        Some(result) if result.is_object() => result,
        _ => return Ok(empty_page(page)),
    };
    let songs = result
        .get("songs")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let items: Vec<SearchResultItem> = songs.iter().filter_map(parse_song).collect();
    let total_raw = result
        .get("songCount")
        .and_then(Value::as_u64)
        .unwrap_or(items.len() as u64);
    let has_more = !items.is_empty() && u64::from(page) * u64::from(PAGE_SIZE) < total_raw;
    let total = total_raw.min(u64::from(u32::MAX)) as u32;
    Ok(SearchPage {
        items,
        page,
        has_more,
        total,
    })
}

/// 获取单曲播放地址，并尽量补齐标题 / 作者 / 封面 / 时长。
pub(crate) async fn run_song_url(state: &AppState, song_id: &str) -> CmdResult<NeteasePlayInfo> {
    let numeric: u64 = song_id
        .trim()
        .parse()
        .map_err(|_| AppError::msg("歌曲 ID 无效"))?;
    let cookies = current_cookies(state);
    let payload = json!({
        "ids": format!("[{}]", numeric),
        "br": 320_000,
    });
    let response = post_weapi(state, PLAYER_URL, &payload, &cookies).await?;

    let code = json_code(&response.json);
    if is_login_error(code) {
        state.mark_expired(Source::Netease);
        return Err(AppError::msg("网易云登录状态已失效，请重新登录"));
    }
    if let Some(code) = code {
        if code != 200 {
            return Err(AppError::msg("获取网易云播放地址失败，请稍后重试"));
        }
    }

    let entry = response
        .json
        .get("data")
        .and_then(Value::as_array)
        .and_then(|data| data.first())
        .cloned();
    let mut url = entry
        .as_ref()
        .and_then(|entry| entry.get("url"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_string());

    // 未登录 / 无版权时 weapi 会返回空 url，退回公开的外链播放接口再试一次。
    if url.is_none() {
        url = outer_song_url(state, numeric).await;
    }
    let url = url.ok_or_else(|| AppError::msg("该曲目需要登录或会员，换一首试试"))?;

    // 详情接口只是锦上添花，失败不影响播放地址返回。
    let meta = fetch_song_meta(state, numeric, &cookies).await;
    let size = entry
        .as_ref()
        .and_then(|entry| entry.get("size"))
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let bitrate = entry
        .as_ref()
        .and_then(|entry| entry.get("br"))
        .and_then(Value::as_u64)
        .unwrap_or(320_000)
        .max(1);
    let estimated_sec = if size > 0 {
        (size as f64 * 8.0) / bitrate as f64
    } else {
        0.0
    };
    let duration_sec = if meta.duration_sec > 0.0 {
        meta.duration_sec
    } else {
        estimated_sec
    };

    Ok(NeteasePlayInfo {
        url: Some(url),
        duration_sec,
        title: meta.title,
        author: meta.author,
        cover: meta.cover,
    })
}

/// 公开的外链播放地址：免费曲目会 302 到 CDN，付费曲目返回 404 或 HTML。
async fn outer_song_url(state: &AppState, song_id: u64) -> Option<String> {
    let url = format!("https://music.163.com/song/media/outer/url?id={song_id}.mp3");
    let response = state
        .http
        .get(url)
        .header(
            reqwest::header::USER_AGENT,
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36",
        )
        .header(reqwest::header::REFERER, "https://music.163.com")
        .send()
        .await
        .ok()?;
    if !response.status().is_success() {
        return None;
    }
    let content_type = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("")
        .to_ascii_lowercase();
    if !content_type.starts_with("audio") && !content_type.contains("octet-stream") {
        return None;
    }
    let final_url = response.url().to_string();
    if final_url.contains("/song/media/outer/url") {
        return None;
    }
    Some(final_url)
}

/// 未登录时用公开搜索接口兜底，避免 weapi 直接报错。
async fn fallback_public_search(
    state: &AppState,
    keyword: &str,
    page: u32,
    offset: u64,
) -> Option<SearchPage> {
    let url = format!(
        "https://music.163.com/api/search/get/web?csrf_token=&s={}&type=1&offset={}&limit={}",
        urlencoding::encode(keyword),
        offset,
        PAGE_SIZE
    );
    let response = state
        .http
        .get(url)
        .header(
            reqwest::header::USER_AGENT,
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36",
        )
        .header(reqwest::header::REFERER, "https://music.163.com")
        .send()
        .await
        .ok()?;
    if !response.status().is_success() {
        return None;
    }
    let json: Value = response.json().await.ok()?;
    let result = json.get("result")?;
    let songs = result.get("songs")?.as_array()?;
    let items: Vec<SearchResultItem> = songs.iter().filter_map(parse_song).collect();
    let total_raw = result
        .get("songCount")
        .and_then(Value::as_u64)
        .unwrap_or(items.len() as u64);
    let has_more = !items.is_empty() && u64::from(page) * u64::from(PAGE_SIZE) < total_raw;
    Some(SearchPage {
        items,
        page,
        has_more,
        total: total_raw.min(u64::from(u32::MAX)) as u32,
    })
}

fn empty_page(page: u32) -> SearchPage {
    SearchPage {
        items: Vec::new(),
        page,
        has_more: false,
        total: 0,
    }
}

#[derive(Default)]
struct SongMeta {
    title: String,
    author: String,
    cover: String,
    duration_sec: f64,
}

async fn fetch_song_meta(state: &AppState, song_id: u64, cookies: &NeteaseCookies) -> SongMeta {
    let payload = json!({ "c": format!("[{{\"id\":{}}}]", song_id) });
    let response = match post_weapi(state, SONG_DETAIL_URL, &payload, cookies).await {
        Ok(response) => response,
        Err(_) => return SongMeta::default(),
    };
    let song = match response
        .json
        .get("songs")
        .and_then(Value::as_array)
        .and_then(|songs| songs.first())
    {
        Some(song) => song,
        None => return SongMeta::default(),
    };
    SongMeta {
        title: song
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string(),
        author: artist_name(song),
        cover: album_cover(song),
        duration_sec: song_duration(song),
    }
}

fn parse_song(song: &Value) -> Option<SearchResultItem> {
    let id = song.get("id").and_then(id_text)?;
    Some(SearchResultItem {
        source: Source::Netease,
        source_id: id,
        title: song
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string(),
        author: artist_name(song),
        cover: album_cover(song),
        duration_sec: song_duration(song),
    })
}

fn id_text(value: &Value) -> Option<String> {
    match value {
        Value::Number(number) => Some(number.to_string()),
        Value::String(text) if !text.trim().is_empty() => Some(text.trim().to_string()),
        _ => None,
    }
}

fn artist_name(song: &Value) -> String {
    song.get("ar")
        .or_else(|| song.get("artists"))
        .and_then(Value::as_array)
        .and_then(|artists| artists.first())
        .and_then(|artist| artist.get("name"))
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string()
}

fn album_cover(song: &Value) -> String {
    song.get("al")
        .or_else(|| song.get("album"))
        .and_then(|album| album.get("picUrl"))
        .and_then(Value::as_str)
        .map(https_url)
        .unwrap_or_default()
}

fn song_duration(song: &Value) -> f64 {
    song.get("dt")
        .or_else(|| song.get("duration"))
        .and_then(Value::as_u64)
        .map(|millis| millis as f64 / 1000.0)
        .unwrap_or(0.0)
}

#[cfg(test)]
mod live_probe {
    use super::*;

    #[tokio::test]
    #[ignore]
    async fn live_netease_playback_probe() {
        let dir = std::env::temp_dir().join(format!("fretec-netease-{}", uuid::Uuid::new_v4()));
        let state = AppState::new(crate::storage::AppPaths::new(dir));
        let cookies = current_cookies(&state);
        let payload = json!({
            "s": "周杰伦",
            "type": 1,
            "limit": PAGE_SIZE,
            "offset": 0,
            "total": true,
        });
        match post_weapi(&state, CLOUDSEARCH_URL, &payload, &cookies).await {
            Ok(response) => eprintln!("weapi search json={}", response.json),
            Err(error) => eprintln!("weapi search error={error}"),
        }
        let page = run_search(&state, "周杰伦", 1).await.expect("search");
        eprintln!("search items={}", page.items.len());
        for item in page.items.iter().take(4) {
            match run_song_url(&state, &item.source_id).await {
                Ok(info) => eprintln!(
                    "OK {} url={:?}",
                    item.title,
                    info.url.as_deref().map(|url| &url[..url.len().min(60)])
                ),
                Err(error) => eprintln!("ERR {} => {}", item.title, error),
            }
        }
    }
}
