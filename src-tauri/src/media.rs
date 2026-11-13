use std::sync::{Arc, Mutex};

use crate::error::{AppError, CmdResult};
use crate::proxy::{ProxyServer, ProxyTarget};
use crate::state::AppState;
use crate::types::{MediaPayload, QualityInfo, Source};

const BILI_REFERER: &str = "https://www.bilibili.com";
const NETEASE_REFERER: &str = "https://music.163.com";
const DESKTOP_UA: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36";

pub struct MediaService {
    proxy: Mutex<Option<Arc<ProxyServer>>>,
}

impl MediaService {
    pub fn new() -> Self {
        Self {
            proxy: Mutex::new(None),
        }
    }

    async fn ensure_proxy(&self) -> CmdResult<Arc<ProxyServer>> {
        if let Some(existing) = self.proxy.lock().expect("proxy lock").clone() {
            return Ok(existing);
        }
        let server = Arc::new(ProxyServer::start().await?);
        *self.proxy.lock().expect("proxy lock") = Some(server.clone());
        Ok(server)
    }

    pub fn close(&self) {
        if let Some(proxy) = self.proxy.lock().expect("proxy lock").take() {
            proxy.stop();
        }
    }

    pub async fn resolve(
        &self,
        state: &AppState,
        item_id: &str,
        quality: u32,
    ) -> CmdResult<MediaPayload> {
        let item = {
            let flow = state.flow.lock().expect("flow lock");
            flow.items
                .iter()
                .find(|entry| entry.id == item_id)
                .cloned()
                .ok_or_else(|| AppError::msg("该条目已不在学习流中"))?
        };

        let requested = match quality {
            16 | 64 | 80 => quality,
            _ => 64,
        };
        let proxy = self.ensure_proxy().await?;
        let base = proxy.base();

        match item.source {
            Source::Bili => {
                let info = crate::bili::playurl(state, &item.source_id, requested).await?;
                let mut video_url = None;
                let mut audio_url = None;
                let mut single_url = None;
                if let Some(url) = info.video_url.clone() {
                    video_url = Some(
                        proxy
                            .register(ProxyTarget {
                                url,
                                referer: Some(BILI_REFERER.to_string()),
                                user_agent: Some(DESKTOP_UA.to_string()),
                            })
                            .await,
                    );
                }
                if let Some(url) = info.audio_url.clone() {
                    audio_url = Some(
                        proxy
                            .register(ProxyTarget {
                                url,
                                referer: Some(BILI_REFERER.to_string()),
                                user_agent: Some(DESKTOP_UA.to_string()),
                            })
                            .await,
                    );
                }
                if let Some(url) = info.single_url.clone() {
                    single_url = Some(
                        proxy
                            .register(ProxyTarget {
                                url,
                                referer: Some(BILI_REFERER.to_string()),
                                user_agent: Some(DESKTOP_UA.to_string()),
                            })
                            .await,
                    );
                }
                if single_url.is_none() && video_url.is_none() && audio_url.is_none() {
                    return Err(AppError::msg("未取到可播放的视频流"));
                }
                let duration = if info.duration_sec > 0.0 {
                    info.duration_sec
                } else {
                    item.duration_sec
                };
                Ok(MediaPayload {
                    item_id: item.id.clone(),
                    kind: "video".to_string(),
                    title: if info.title.is_empty() {
                        item.title.clone()
                    } else {
                        info.title
                    },
                    author: if info.author.is_empty() {
                        item.author.clone()
                    } else {
                        info.author
                    },
                    cover: if info.cover.is_empty() {
                        item.cover.clone()
                    } else {
                        info.cover
                    },
                    duration_sec: duration,
                    video_url,
                    audio_url,
                    single_url,
                    position_sec: resume_position(item.progress.position_sec, duration),
                    quality: QualityInfo {
                        requested,
                        actual: info.actual_qn,
                        fallback: info.fallback,
                    },
                    proxy_base: base,
                    notice: info.notice,
                })
            }
            Source::Netease => {
                let info = crate::netease::song_url(state, &item.source_id).await?;
                let Some(url) = info.url.clone() else {
                    return Err(AppError::msg("该曲目需登录或为会员可用"));
                };
                let single_url = proxy
                    .register(ProxyTarget {
                        url,
                        referer: Some(NETEASE_REFERER.to_string()),
                        user_agent: Some(DESKTOP_UA.to_string()),
                    })
                    .await;
                let duration = if info.duration_sec > 0.0 {
                    info.duration_sec
                } else {
                    item.duration_sec
                };
                Ok(MediaPayload {
                    item_id: item.id.clone(),
                    kind: "audio".to_string(),
                    title: if info.title.is_empty() {
                        item.title.clone()
                    } else {
                        info.title
                    },
                    author: if info.author.is_empty() {
                        item.author.clone()
                    } else {
                        info.author
                    },
                    cover: if info.cover.is_empty() {
                        item.cover.clone()
                    } else {
                        info.cover
                    },
                    duration_sec: duration,
                    video_url: None,
                    audio_url: None,
                    single_url: Some(single_url),
                    position_sec: resume_position(item.progress.position_sec, duration),
                    quality: QualityInfo {
                        requested,
                        actual: requested,
                        fallback: false,
                    },
                    proxy_base: base,
                    notice: None,
                })
            }
        }
    }
}

fn resume_position(position: f64, duration: f64) -> f64 {
    if position <= 1.0 {
        return 0.0;
    }
    if duration > 3.0 && position >= duration - 2.0 {
        return 0.0;
    }
    position
}
