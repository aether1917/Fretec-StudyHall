use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Source {
    Bili,
    Netease,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Progress {
    pub position_sec: f64,
    pub updated_at: u64,
}

impl Default for Progress {
    fn default() -> Self {
        Self {
            position_sec: 0.0,
            updated_at: 0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FlowItem {
    pub id: String,
    pub source: Source,
    pub source_id: String,
    pub title: String,
    pub author: String,
    pub cover: String,
    pub duration_sec: f64,
    pub added_at: u64,
    #[serde(default)]
    pub progress: Progress,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchResultItem {
    pub source: Source,
    pub source_id: String,
    pub title: String,
    pub author: String,
    pub cover: String,
    pub duration_sec: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchPage {
    pub items: Vec<SearchResultItem>,
    pub page: u32,
    pub has_more: bool,
    pub total: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QualityInfo {
    pub requested: u32,
    pub actual: u32,
    pub fallback: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaPayload {
    pub item_id: String,
    pub kind: String,
    pub title: String,
    pub author: String,
    pub cover: String,
    pub duration_sec: f64,
    pub video_url: Option<String>,
    pub audio_url: Option<String>,
    pub single_url: Option<String>,
    pub position_sec: f64,
    pub quality: QualityInfo,
    pub proxy_base: String,
    pub notice: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountProfile {
    pub name: String,
    pub avatar: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountStatus {
    pub logged_in: bool,
    pub name: String,
    pub avatar: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthStatus {
    pub bili: AccountStatus,
    pub netease: AccountStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QrLogin {
    pub key: String,
    pub url: String,
    #[serde(default)]
    pub image: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum QrState {
    Waiting,
    Scanned,
    Expired,
    Success,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QrPoll {
    pub state: QrState,
    pub message: String,
    pub profile: Option<AccountProfile>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CaptchaConfig {
    #[serde(rename = "type")]
    pub kind: String,
    pub gt: String,
    pub challenge: String,
    pub success: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SmsTicket {
    pub captcha_key: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CaptchaAnswer {
    pub challenge: String,
    pub validate: String,
    pub seccode: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LanSelf {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LanPeer {
    pub id: String,
    pub name: String,
    pub online: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LanStatus {
    pub available: bool,
    pub searching: bool,
    pub peers: Vec<LanPeer>,
    #[serde(rename = "self")]
    pub self_info: LanSelf,
    pub message: String,
    pub can_connect: bool,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Phase {
    Focus,
    Break,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TimerStatus {
    Idle,
    Running,
    Paused,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TimerSnapshot {
    pub phase: Phase,
    pub total_ms: u64,
    pub started_at_ms: Option<u64>,
    pub pause_offset_ms: u64,
    pub paused_at_ms: Option<u64>,
    pub status: TimerStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TimerWire {
    pub phase: Phase,
    pub total_ms: u64,
    pub started_at_ms: Option<u64>,
    pub pause_offset_ms: u64,
    pub paused_at_ms: Option<u64>,
    pub status: TimerStatus,
}

pub const FOCUS_TOTAL_MS: u64 = 25 * 60 * 1000;
pub const BREAK_TOTAL_MS: u64 = 5 * 60 * 1000;

pub fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|value| value.as_millis() as u64)
        .unwrap_or(0)
}

pub fn remaining_ms(snapshot: &TimerSnapshot) -> u64 {
    match (snapshot.status, snapshot.started_at_ms) {
        (TimerStatus::Running, Some(started)) | (TimerStatus::Paused, Some(started)) => {
            let anchor = snapshot.paused_at_ms.unwrap_or_else(now_ms);
            let elapsed = anchor.saturating_sub(started).saturating_sub(snapshot.pause_offset_ms);
            snapshot.total_ms.saturating_sub(elapsed)
        }
        _ => snapshot.total_ms,
    }
}
