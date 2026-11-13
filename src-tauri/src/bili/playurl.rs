//! 视频取流：先取 view 得到 cid 与元信息，再请求 WBI playurl。

use serde_json::Value;

use crate::error::{AppError, CmdResult};
use crate::state::AppState;

use super::{
    api_error, api_get, ensure_mixin_key, execute, is_logged_in, normalize_image_url,
    prepare_cookies, wbi, BiliPlayInfo,
};

const VIEW_URL: &str = "https://api.bilibili.com/x/web-interface/view";
const PLAYURL_URL: &str = "https://api.bilibili.com/x/player/wbi/playurl";

/// 请求播放地址并解析 DASH / FLV 流。
pub async fn playurl(state: &AppState, bvid: &str, qn: u32) -> CmdResult<BiliPlayInfo> {
    let bvid = bvid.trim();
    if bvid.is_empty() {
        return Err(AppError::msg("视频编号不能为空"));
    }
    let requested_qn = normalize_requested_qn(qn);
    let logged_in = is_logged_in(state);

    let cookies = prepare_cookies(state).await?;
    let view = fetch_view(state, bvid, &cookies).await?;

    let mut info = BiliPlayInfo {
        title: view.title,
        author: view.author,
        cover: view.cover,
        duration_sec: view.duration_sec,
        actual_qn: requested_qn,
        ..BiliPlayInfo::default()
    };

    // 1080P 需要登录，未登录时改为请求 720P，并给出提示。
    let mut effective_qn = requested_qn;
    if requested_qn == 80 && !logged_in {
        effective_qn = 64;
        info.fallback = true;
        info.notice = Some("该画质需登录，已切换到 720P".to_string());
    }

    let mixin_key = ensure_mixin_key(state).await?;
    let params: Vec<(&str, String)> = vec![
        ("bvid", bvid.to_string()),
        ("cid", view.cid.to_string()),
        ("qn", effective_qn.to_string()),
        ("fnval", "16".to_string()),
        ("fnver", "0".to_string()),
        ("fourk", "1".to_string()),
    ];
    let query = wbi::signed_query(&mixin_key, &params, super::now_secs());
    let url = format!("{PLAYURL_URL}?{query}");
    let response = execute(state, api_get(state, &url, &cookies)).await?;
    let code = response.code();
    if code != 0 {
        return Err(api_error(state, code, response.message()));
    }
    let data = response
        .data()
        .ok_or_else(|| AppError::msg("哔哩哔哩播放地址获取失败，请稍后再试"))?;

    let streams = parse_streams(data, effective_qn);
    info.video_url = streams.video_url;
    info.audio_url = streams.audio_url;
    info.single_url = streams.single_url;
    info.actual_qn = streams.actual_qn;

    let timelength = data
        .get("timelength")
        .and_then(Value::as_f64)
        .unwrap_or(0.0);
    if timelength > 0.0 {
        info.duration_sec = timelength / 1000.0;
    }

    if info.actual_qn != requested_qn {
        info.fallback = true;
        if info.notice.is_none() {
            info.notice = Some(if requested_qn == 80 && !logged_in {
                "该画质需登录，已自动切换".to_string()
            } else {
                "当前画质不可用，已自动切换".to_string()
            });
        }
    }

    if info.video_url.is_none() && info.single_url.is_none() {
        return Err(AppError::msg("没有获取到可播放的视频流，请稍后再试"));
    }
    Ok(info)
}

/// 只支持 16 / 64 / 80；其它值按 720P 处理，避免前端传入 0 时直接失败。
fn normalize_requested_qn(qn: u32) -> u32 {
    match qn {
        16 | 64 | 80 => qn,
        _ => 64,
    }
}

struct ViewInfo {
    cid: u64,
    title: String,
    author: String,
    cover: String,
    duration_sec: f64,
}

async fn fetch_view(state: &AppState, bvid: &str, cookies: &str) -> CmdResult<ViewInfo> {
    let url = format!("{VIEW_URL}?bvid={}", wbi::encode_component(bvid));
    let response = execute(state, api_get(state, &url, cookies)).await?;
    let code = response.code();
    if code != 0 {
        return Err(api_error(state, code, response.message()));
    }
    let data = response
        .data()
        .ok_or_else(|| AppError::msg("视频信息不存在或已下架"))?;

    let cid = data
        .get("pages")
        .and_then(Value::as_array)
        .and_then(|pages| pages.first())
        .and_then(|page| page.get("cid"))
        .and_then(Value::as_u64)
        .or_else(|| data.get("cid").and_then(Value::as_u64))
        .ok_or_else(|| AppError::msg("视频分页信息缺失，暂时无法播放"))?;
    let title = data
        .get("title")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim()
        .to_string();
    let author = data
        .get("owner")
        .and_then(|owner| owner.get("name"))
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim()
        .to_string();
    let cover = normalize_image_url(data.get("pic").and_then(Value::as_str).unwrap_or(""));
    let duration_sec = data.get("duration").and_then(Value::as_f64).unwrap_or(0.0);

    Ok(ViewInfo {
        cid,
        title: if title.is_empty() {
            format!("哔哩哔哩视频 {bvid}")
        } else {
            title
        },
        author,
        cover,
        duration_sec,
    })
}

struct StreamInfo {
    video_url: Option<String>,
    audio_url: Option<String>,
    single_url: Option<String>,
    actual_qn: u32,
}

fn parse_streams(data: &Value, requested_qn: u32) -> StreamInfo {
    let mut video_url = None;
    let mut audio_url = None;
    let mut actual_qn = requested_qn;

    if let Some(dash) = data.get("dash") {
        if let Some(streams) = dash.get("video").and_then(Value::as_array) {
            // 优先 AVC（codecid == 7），兼容性最好；否则取第一个流。
            let selected = streams
                .iter()
                .find(|stream| stream.get("codecid").and_then(Value::as_u64) == Some(7))
                .or_else(|| streams.first());
            if let Some(stream) = selected {
                video_url = stream_url(stream);
                if let Some(id) = stream.get("id").and_then(Value::as_u64) {
                    actual_qn = id as u32;
                }
            }
        }
        audio_url = dash
            .get("audio")
            .and_then(Value::as_array)
            .and_then(|streams| streams.first())
            .and_then(stream_url);
    }

    let single_url = data
        .get("durl")
        .and_then(Value::as_array)
        .and_then(|segments| segments.first())
        .and_then(|segment| segment.get("url"))
        .and_then(Value::as_str)
        .map(|url| url.to_string());

    // 没有 DASH 时以 FLV/MP4 直链播放，画质取接口返回的 quality。
    if video_url.is_none() && single_url.is_some() {
        actual_qn = data
            .get("quality")
            .and_then(Value::as_u64)
            .map(|quality| quality as u32)
            .unwrap_or(requested_qn);
    }

    StreamInfo {
        video_url,
        audio_url,
        single_url,
        actual_qn,
    }
}

fn stream_url(stream: &Value) -> Option<String> {
    stream
        .get("baseUrl")
        .or_else(|| stream.get("base_url"))
        .and_then(Value::as_str)
        .filter(|url| !url.is_empty())
        .map(|url| url.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prefers_avc_stream() {
        let data = serde_json::json!({
            "dash": {
                "video": [
                    {"id": 80, "codecid": 12, "baseUrl": "https://cdn/hevc.m4s"},
                    {"id": 64, "codecid": 7, "base_url": "https://cdn/avc.m4s"}
                ],
                "audio": [
                    {"baseUrl": "https://cdn/audio.m4s"}
                ]
            },
            "timelength": 125000
        });
        let streams = parse_streams(&data, 80);
        assert_eq!(streams.video_url.as_deref(), Some("https://cdn/avc.m4s"));
        assert_eq!(streams.audio_url.as_deref(), Some("https://cdn/audio.m4s"));
        assert_eq!(streams.actual_qn, 64);
        assert!(streams.single_url.is_none());
    }

    #[test]
    fn falls_back_to_durl_without_dash() {
        let data = serde_json::json!({
            "quality": 32,
            "durl": [
                {"url": "https://cdn/video.flv", "length": 1000}
            ]
        });
        let streams = parse_streams(&data, 64);
        assert!(streams.video_url.is_none());
        assert_eq!(streams.single_url.as_deref(), Some("https://cdn/video.flv"));
        assert_eq!(streams.actual_qn, 32);
    }

    #[test]
    fn normalizes_unknown_quality() {
        assert_eq!(normalize_requested_qn(0), 64);
        assert_eq!(normalize_requested_qn(80), 80);
        assert_eq!(normalize_requested_qn(16), 16);
    }
}
