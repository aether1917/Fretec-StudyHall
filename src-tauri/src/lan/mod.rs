//! 局域网发现与倒计时同步。
//!
//! 发现使用 UDP 47821（可用 `STUDYHALL_LAN_PORT` 覆盖）广播本机信息并同时监听；
//! 传输使用 TCP 随机端口，报文为 4 字节大端长度前缀 + UTF-8 JSON。
//! 所有对前端可见的状态变化都通过 `attach` 进来的 `AppHandle` 走
//! [`crate::events::emit_lan`] 推送。
//!
//! 环境变量（只读消费，仅用于同机多实例测试）：
//! - `STUDYHALL_LAN_ID`：覆盖设备 id；
//! - `STUDYHALL_LAN_PORT`：覆盖 UDP 发现端口（监听与广播目标端口同时覆盖）；
//! - `STUDYHALL_DATA_DIR`：优先从该目录的 `device.json` 读取设备 id；目录里还没有
//!   `device.json` 时使用进程内稳定的临时 id，便于同机多实例使用不同身份。
//!   设备名不读取环境变量。

mod discovery;
mod protocol;
mod transport;

use std::collections::HashMap;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use tauri::{AppHandle, Wry};
use tokio::sync::{mpsc, watch};

use crate::timer::TimerEngine;
use crate::types::{LanPeer, LanSelf, LanStatus, TimerSnapshot, TimerStatus};

use discovery::DiscoveryCommand;
use protocol::WireMessage;

const DEFAULT_DEVICE_NAME: &str = "Fretec StudyHall";
const DEFAULT_DISCOVERY_PORT: u16 = 47_821;
const SEARCH_DURATION: Duration = Duration::from_secs(10);
const PING_INTERVAL: Duration = Duration::from_secs(1);
const PEER_TIMEOUT: Duration = Duration::from_secs(5);
const CONNECT_TIMEOUT: Duration = Duration::from_secs(3);
const WRITE_TIMEOUT: Duration = Duration::from_secs(3);
const MAX_PEERS: usize = 4;
const UNAVAILABLE_MESSAGE: &str = "当前网络不可用，多设备互联已降级";

/// 一条已建立的 TCP 连接。写半端由独立的 writer 任务持有，通过 `frames` 投递数据；
/// `cancel` 用于通知读写任务退出（关闭 socket）。
pub(super) struct Peer {
    pub(super) name: String,
    pub(super) token: u64,
    pub(super) cancel: watch::Sender<bool>,
    pub(super) frames: mpsc::UnboundedSender<Vec<u8>>,
}

/// 收到过广播、但当前未连接的设备；保留用于手动重连。
pub(super) struct Discovered {
    pub(super) name: String,
    pub(super) addr: SocketAddr,
    pub(super) auto_attempted: bool,
}

pub(super) struct ListenerSession {
    pub(super) cancel: watch::Sender<bool>,
}

pub(super) struct LanInner {
    pub(super) device_id: String,
    pub(super) device_name: String,
    pub(super) available: bool,
    /// 监听任务是否处于“服务开启”状态；stop 后为 false，阻止存量任务再注册连接。
    pub(super) accepting: bool,
    pub(super) searching: bool,
    pub(super) message: String,
    pub(super) error: Option<String>,
    pub(super) tcp_port: Option<u16>,
    pub(super) listener: Option<ListenerSession>,
    pub(super) discovery_tx: Option<mpsc::UnboundedSender<DiscoveryCommand>>,
    pub(super) peers: HashMap<String, Peer>,
    pub(super) discovered: HashMap<String, Discovered>,
    pub(super) generation: u64,
}

pub(super) struct Shared {
    pub(super) inner: Mutex<LanInner>,
    pub(super) timer: Arc<TimerEngine>,
    pub(super) app: OnceLock<AppHandle<Wry>>,
}

impl Shared {
    pub(super) fn app(&self) -> Option<&AppHandle<Wry>> {
        self.app.get()
    }
}

pub struct LanService {
    shared: Arc<Shared>,
    /// 串行化 start_search / stop，避免两者交错把已关闭的监听重新打开。
    lifecycle: Mutex<()>,
}

impl LanService {
    pub fn new(timer: Arc<TimerEngine>) -> Self {
        let shared = Shared {
            inner: Mutex::new(LanInner {
                device_id: resolve_device_id(""),
                device_name: DEFAULT_DEVICE_NAME.to_string(),
                available: true,
                accepting: false,
                searching: false,
                message: "未开始搜索".to_string(),
                error: None,
                tcp_port: None,
                listener: None,
                discovery_tx: None,
                peers: HashMap::new(),
                discovered: HashMap::new(),
                generation: 0,
            }),
            timer,
            app: OnceLock::new(),
        };
        Self {
            shared: Arc::new(shared),
            lifecycle: Mutex::new(()),
        }
    }

    pub fn attach(&self, app: AppHandle<Wry>) {
        let _ = self.shared.app.set(app);
        emit_status(&self.shared);
    }

    /// lib.rs 启动时注入持久化的设备 id 与名字。`STUDYHALL_LAN_ID` 优先级最高。
    pub fn configure(&self, id: String, name: String) {
        let resolved = resolve_device_id(&id);
        {
            let mut inner = self.shared.inner.lock().expect("lan lock");
            inner.device_id = resolved;
            let name = name.trim();
            if !name.is_empty() {
                inner.device_name = name.to_string();
            }
        }
        emit_status(&self.shared);
    }

    /// 修改本机显示名并广播给所有已连接设备。
    pub fn update_name(&self, name: String) {
        let name = name.trim().to_string();
        if name.is_empty() {
            return;
        }
        {
            let mut inner = self.shared.inner.lock().expect("lan lock");
            inner.device_name = name.clone();
        }
        transport::send_to_peers(&self.shared, &WireMessage::Name { name });
        emit_status(&self.shared);
    }

    pub fn status(&self) -> LanStatus {
        status_of(&self.shared)
    }

    /// 开始（或重新开始）一轮 10 秒搜索。
    pub fn start_search(&self) {
        let shared = self.shared.clone();
        {
            // 与 stop 串行，避免交错把刚关闭的监听重新打开。
            let _lifecycle = self.lifecycle.lock().expect("lan lifecycle");
            let port = discovery_port();
            let generation = {
                let mut inner = shared.inner.lock().expect("lan lock");
                inner.generation = inner.generation.wrapping_add(1);
                inner.searching = true;
                inner.message = "正在搜索附近的设备…".to_string();
                for record in inner.discovered.values_mut() {
                    record.auto_attempted = false;
                }
                inner.generation
            };

            if let Err(error) = transport::ensure_listener(&shared) {
                eprintln!("lan tcp listener failed: {error}");
                let mut inner = shared.inner.lock().expect("lan lock");
                if inner.generation == generation {
                    inner.available = false;
                    inner.accepting = false;
                    inner.searching = false;
                    inner.error = Some(UNAVAILABLE_MESSAGE.to_string());
                    inner.message = UNAVAILABLE_MESSAGE.to_string();
                }
            } else {
                discovery::start(&shared, generation, port);
            }
        }
        emit_status(&shared);
    }

    /// 停止发现搜索，但保留已建立的连接和监听端口。
    pub fn stop_search(&self) {
        {
            let _lifecycle = self.lifecycle.lock().expect("lan lifecycle");
            let mut inner = self.shared.inner.lock().expect("lan lock");
            inner.generation = inner.generation.wrapping_add(1);
            inner.searching = false;
            inner.message = "已停止搜索".to_string();
        }
        discovery::stop(&self.shared);
        emit_status(&self.shared);
    }

    /// 停止搜索与所有连接；保留可用性与已发现设备记录（便于重新搜索后手动重连）。
    pub fn stop(&self) {
        {
            // 与 start_search 串行，避免交错。
            let _lifecycle = self.lifecycle.lock().expect("lan lifecycle");
            let removed = {
                let mut inner = self.shared.inner.lock().expect("lan lock");
                inner.generation = inner.generation.wrapping_add(1);
                inner.accepting = false;
                inner.searching = false;
                inner.message = "已停止搜索".to_string();
                inner.tcp_port = None;
                if let Some(listener) = inner.listener.take() {
                    let _ = listener.cancel.send(true);
                }
                std::mem::take(&mut inner.peers)
            };

            discovery::stop(&self.shared);
            for peer in removed.into_values() {
                let _ = peer.cancel.send(true);
            }
        }
        emit_status(&self.shared);
    }

    /// 对已发现但未连接的设备手动发起连接。
    pub fn connect(&self, peer_id: &str) {
        let peer_id = peer_id.trim().to_string();
        if peer_id.is_empty() {
            return;
        }
        if is_timer_busy(&self.shared) {
            note_message(&self.shared, "计时进行中，暂不可连接".to_string());
            return;
        }

        let action = {
            let mut inner = self.shared.inner.lock().expect("lan lock");
            if inner.peers.contains_key(&peer_id) {
                None
            } else if !inner.accepting {
                inner.message = "请先重新搜索附近设备".to_string();
                None
            } else if !inner.available {
                inner.message = inner
                    .error
                    .clone()
                    .unwrap_or_else(|| UNAVAILABLE_MESSAGE.to_string());
                None
            } else if inner.peers.len() >= MAX_PEERS {
                inner.message = "连接已满（最多 4 台）".to_string();
                None
            } else if let Some(record) = inner.discovered.get_mut(&peer_id) {
                record.auto_attempted = true;
                Some((record.addr, record.name.clone()))
            } else {
                inner.message = "未发现该设备，请先重新搜索".to_string();
                None
            }
        };

        let Some((address, name)) = action else {
            emit_status(&self.shared);
            return;
        };

        note_message(&self.shared, format!("正在连接 {name}…"));
        transport::spawn_dial(self.shared.clone(), peer_id, address);
    }

    /// 断开指定设备；只关闭 socket，不发送 bye。
    pub fn disconnect(&self, peer_id: &str) {
        let removed = {
            let mut inner = self.shared.inner.lock().expect("lan lock");
            let removed = inner.peers.remove(peer_id);
            if let Some(peer) = &removed {
                inner.message = format!("已断开与 {} 的连接", peer.name);
            }
            removed
        };

        if let Some(peer) = removed {
            let _ = peer.cancel.send(true);
            emit_status(&self.shared);
        }
    }

    /// 向所有已连接设备广播当前倒计时快照。
    pub fn broadcast_timer(&self, snapshot: &TimerSnapshot) {
        transport::send_to_peers(
            &self.shared,
            &WireMessage::Timer {
                snapshot: snapshot.clone(),
            },
        );
        // 计时状态会改变 can_connect，同步刷新一次局域网状态。
        emit_status(&self.shared);
    }
}

fn status_of(shared: &Shared) -> LanStatus {
    let timer_busy = is_timer_busy(shared);
    let inner = shared.inner.lock().expect("lan lock");

    let mut peers: Vec<LanPeer> = inner
        .peers
        .iter()
        .map(|(id, peer)| LanPeer {
            id: id.clone(),
            name: peer.name.clone(),
            online: true,
        })
        .collect();
    peers.sort_by(|left, right| {
        left.name
            .cmp(&right.name)
            .then_with(|| left.id.cmp(&right.id))
    });

    LanStatus {
        available: inner.available,
        searching: inner.searching,
        peers,
        self_info: LanSelf {
            id: inner.device_id.clone(),
            name: inner.device_name.clone(),
        },
        message: inner.message.clone(),
        // 与占位实现保持一致：只反映「计时是否允许新建连接」。容量在 connect/自动
        // 连接内部单独校验；网络可用性通过 `available` / `error` 暴露。
        can_connect: !timer_busy,
        error: inner.error.clone(),
    }
}

pub(super) fn emit_status(shared: &Shared) {
    if let Some(app) = shared.app() {
        let status = status_of(shared);
        crate::events::emit_lan(app, &status);
    }
}

pub(super) fn is_timer_busy(shared: &Shared) -> bool {
    shared.timer.snapshot().status != TimerStatus::Idle
}

/// 更新提示文案并推送状态。
pub(super) fn note_message(shared: &Shared, message: String) {
    {
        let mut inner = shared.inner.lock().expect("lan lock");
        inner.message = message;
    }
    emit_status(shared);
}

/// 搜索轮次自然结束（10 秒到期）。
pub(super) fn finish_search(shared: &Shared, generation: u64) {
    let changed = {
        let mut inner = shared.inner.lock().expect("lan lock");
        if inner.generation != generation || !inner.searching {
            false
        } else {
            inner.searching = false;
            if inner.peers.is_empty() {
                inner.message = "未发现设备".to_string();
            }
            true
        }
    };
    if changed {
        emit_status(shared);
    }
}

pub(super) fn mark_available(shared: &Shared, generation: u64) {
    let changed = {
        let mut inner = shared.inner.lock().expect("lan lock");
        if inner.generation != generation {
            false
        } else {
            let changed = !inner.available || inner.error.is_some();
            inner.available = true;
            inner.error = None;
            changed
        }
    };
    if changed {
        emit_status(shared);
    }
}

pub(super) fn set_unavailable(shared: &Shared, generation: u64) {
    let changed = {
        let mut inner = shared.inner.lock().expect("lan lock");
        if inner.generation != generation {
            false
        } else {
            inner.available = false;
            inner.searching = false;
            inner.error = Some(UNAVAILABLE_MESSAGE.to_string());
            inner.message = UNAVAILABLE_MESSAGE.to_string();
            true
        }
    };
    if changed {
        emit_status(shared);
    }
}

fn env_non_empty(key: &str) -> Option<String> {
    std::env::var(key)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn discovery_port() -> u16 {
    env_non_empty("STUDYHALL_LAN_PORT")
        .and_then(|value| value.parse::<u16>().ok())
        .filter(|port| *port != 0)
        .unwrap_or(DEFAULT_DISCOVERY_PORT)
}

fn resolve_device_id(configured: &str) -> String {
    if let Some(id) = env_non_empty("STUDYHALL_LAN_ID") {
        return id;
    }
    // 多实例测试：独立 data dir 里已有持久化 id 时直接使用。
    if let Some(id) = device_id_from_data_dir() {
        return id;
    }
    // 指定了独立 data dir 但还没有 device.json 时，生成本进程内稳定的临时 id，
    // 保证同机多实例不会因为共用持久化 id 而互相忽略。
    if env_non_empty("STUDYHALL_DATA_DIR").is_some() {
        return instance_fallback_id().to_string();
    }
    let configured = configured.trim();
    if !configured.is_empty() {
        return configured.to_string();
    }
    instance_fallback_id().to_string()
}

fn instance_fallback_id() -> &'static str {
    static FALLBACK: OnceLock<String> = OnceLock::new();
    FALLBACK
        .get_or_init(|| uuid::Uuid::new_v4().to_string())
        .as_str()
}

fn device_id_from_data_dir() -> Option<String> {
    let directory = env_non_empty("STUDYHALL_DATA_DIR")?;
    let content = std::fs::read_to_string(PathBuf::from(directory).join("device.json")).ok()?;
    let value: serde_json::Value = serde_json::from_str(&content).ok()?;
    value
        .get("id")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|id| !id.is_empty())
        .map(str::to_string)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::timer::TimerEngine;
    use std::sync::Arc;

    #[test]
    fn status_serializes_expected_fields() {
        let status = LanStatus {
            available: true,
            searching: false,
            peers: vec![LanPeer {
                id: "peer-1".to_string(),
                name: "同桌".to_string(),
                online: true,
            }],
            self_info: LanSelf {
                id: "device-self".to_string(),
                name: "本机".to_string(),
            },
            message: "未发现设备".to_string(),
            can_connect: true,
            error: None,
        };

        let value = serde_json::to_value(&status).expect("serialize lan status");
        assert_eq!(value["available"], serde_json::json!(true));
        assert_eq!(value["searching"], serde_json::json!(false));
        assert_eq!(value["message"], serde_json::json!("未发现设备"));
        assert_eq!(value["canConnect"], serde_json::json!(true));
        assert_eq!(value["error"], serde_json::Value::Null);
        assert_eq!(value["self"]["id"], serde_json::json!("device-self"));
        assert_eq!(value["self"]["name"], serde_json::json!("本机"));
        assert_eq!(value["peers"][0]["id"], serde_json::json!("peer-1"));
        assert_eq!(value["peers"][0]["name"], serde_json::json!("同桌"));
        assert_eq!(value["peers"][0]["online"], serde_json::json!(true));
    }

    #[test]
    fn discovery_port_is_never_zero() {
        assert_ne!(discovery_port(), 0);
    }

    #[test]
    fn device_id_is_never_empty() {
        assert!(!resolve_device_id("").is_empty());
    }

    #[test]
    fn lan_service_is_send_and_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<LanService>();
    }

    #[test]
    fn initial_status_is_idle_and_connectable() {
        let service = LanService::new(Arc::new(TimerEngine::new()));
        let status = service.status();
        assert!(status.available);
        assert!(!status.searching);
        assert!(status.peers.is_empty());
        assert!(status.can_connect);
        assert!(!status.self_info.id.is_empty());
        assert_eq!(status.message, "未开始搜索");
    }

    #[test]
    fn can_connect_is_false_while_timer_is_running() {
        let timer = Arc::new(TimerEngine::new());
        let service = LanService::new(timer.clone());
        timer.start();
        assert!(!service.status().can_connect);
        timer.stop();
        assert!(service.status().can_connect);
    }

    #[test]
    fn update_name_is_kept_in_status() {
        let service = LanService::new(Arc::new(TimerEngine::new()));
        service.update_name("  自习室  ".to_string());
        assert_eq!(service.status().self_info.name, "自习室");
    }
}
