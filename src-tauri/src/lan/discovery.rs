//! UDP 发现：广播本机信息并在同一端口监听其它设备。
//!
//! 每轮搜索持续 [`SEARCH_DURATION`]（10 秒），每秒广播一次。所有轮次由同一个
//! 常驻 `discovery_manager` 任务串行驱动，避免重复绑定端口以及多轮之间的竞态。
//! manager 只持有 UDP socket，网络状态变化仍写回 [`super::Shared`] 并统一发事件。

use std::net::{Ipv4Addr, SocketAddr};
use std::sync::Arc;
use std::time::Duration;

use tokio::net::UdpSocket;
use tokio::sync::mpsc;

use super::protocol::DiscoveryPacket;
use super::transport;
use super::{
    emit_status, finish_search, is_timer_busy, mark_available, set_unavailable, Shared,
    DEFAULT_DISCOVERY_PORT, MAX_PEERS, SEARCH_DURATION,
};

/// 发给 discovery manager 的控制指令。
pub(super) enum DiscoveryCommand {
    /// 开始新一轮 10 秒搜索。
    Search { generation: u64, port: u16 },
    /// 停止搜索（状态已由调用方写入，manager 只需要中断当前轮次）。
    Stop,
}

enum RoundOutcome {
    Deadline,
    Command(DiscoveryCommand),
    Closed,
}

struct DiscoverySocket {
    port: u16,
    socket: Arc<UdpSocket>,
}

/// 请求开始一轮搜索。manager 尚未创建时懒加载创建。
pub(super) fn start(shared: &Arc<Shared>, generation: u64, port: u16) {
    send(shared, DiscoveryCommand::Search { generation, port });
}

/// 请求停止当前搜索轮次。manager 不存在时无需创建。
pub(super) fn stop(shared: &Arc<Shared>) {
    let sender = {
        let inner = shared.inner.lock().expect("lan lock");
        inner.discovery_tx.clone()
    };
    if let Some(sender) = sender {
        let _ = sender.send(DiscoveryCommand::Stop);
    }
}

fn send(shared: &Arc<Shared>, command: DiscoveryCommand) {
    let mut command = command;
    // manager 只会在服务彻底销毁时退出；若发现 channel 关闭则重建一次。
    for _ in 0..2 {
        let Some(sender) = ensure_manager(shared) else {
            return;
        };
        match sender.send(command) {
            Ok(()) => return,
            Err(error) => {
                command = error.0;
                let mut inner = shared.inner.lock().expect("lan lock");
                inner.discovery_tx = None;
            }
        }
    }
}

fn ensure_manager(shared: &Arc<Shared>) -> Option<mpsc::UnboundedSender<DiscoveryCommand>> {
    {
        let inner = shared.inner.lock().expect("lan lock");
        if let Some(sender) = &inner.discovery_tx {
            return Some(sender.clone());
        }
    }

    let (sender, receiver) = mpsc::unbounded_channel();
    {
        let mut inner = shared.inner.lock().expect("lan lock");
        if let Some(existing) = &inner.discovery_tx {
            return Some(existing.clone());
        }
        inner.discovery_tx = Some(sender.clone());
    }

    let shared = shared.clone();
    let _ = tauri::async_runtime::spawn(async move {
        manager(shared, receiver).await;
    });
    Some(sender)
}

async fn manager(shared: Arc<Shared>, mut commands: mpsc::UnboundedReceiver<DiscoveryCommand>) {
    let mut socket: Option<DiscoverySocket> = None;
    let mut pending: Option<DiscoveryCommand> = None;

    loop {
        let command = match pending.take() {
            Some(command) => command,
            None => match commands.recv().await {
                Some(command) => command,
                None => return,
            },
        };

        match command {
            DiscoveryCommand::Stop => {}
            DiscoveryCommand::Search { generation, port } => {
                if !is_current(&shared, generation) {
                    continue;
                }
                if let Err(error) = ensure_socket(&mut socket, port).await {
                    eprintln!("lan discovery socket unavailable: {error}");
                    set_unavailable(&shared, generation);
                    continue;
                }
                mark_available(&shared, generation);
                match run_round(&shared, generation, port, &socket, &mut commands).await {
                    RoundOutcome::Deadline => finish_search(&shared, generation),
                    RoundOutcome::Command(command) => pending = Some(command),
                    RoundOutcome::Closed => return,
                }
            }
        }
    }
}

async fn run_round(
    shared: &Arc<Shared>,
    generation: u64,
    port: u16,
    socket: &Option<DiscoverySocket>,
    commands: &mut mpsc::UnboundedReceiver<DiscoveryCommand>,
) -> RoundOutcome {
    let Some(socket) = socket.as_ref().map(|current| current.socket.clone()) else {
        return RoundOutcome::Deadline;
    };

    let mut ticker = tokio::time::interval(Duration::from_secs(1));
    ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    let deadline = tokio::time::sleep(SEARCH_DURATION);
    tokio::pin!(deadline);

    let mut buffer = vec![0u8; 4096];
    loop {
        if !is_current(shared, generation) {
            return RoundOutcome::Deadline;
        }
        tokio::select! {
            command = commands.recv() => {
                return match command {
                    Some(command) => RoundOutcome::Command(command),
                    None => RoundOutcome::Closed,
                };
            }
            _ = &mut deadline => return RoundOutcome::Deadline,
            _ = ticker.tick() => broadcast_once(shared, socket.as_ref(), port, generation).await,
            received = socket.recv_from(&mut buffer) => {
                if let Ok((length, source)) = received {
                    handle_datagram(shared, &buffer[..length], source);
                }
            }
        }
    }
}

async fn broadcast_once(shared: &Shared, socket: &UdpSocket, port: u16, generation: u64) {
    if !is_current(shared, generation) {
        return;
    }
    let packet = {
        let inner = shared.inner.lock().expect("lan lock");
        let Some(tcp_port) = inner.tcp_port else {
            return;
        };
        DiscoveryPacket {
            v: 1,
            id: inner.device_id.clone(),
            name: inner.device_name.clone(),
            port: tcp_port,
        }
    };

    let payload = match serde_json::to_vec(&packet) {
        Ok(payload) => payload,
        Err(error) => {
            eprintln!("lan discovery serialize failed: {error}");
            return;
        }
    };

    let target = SocketAddr::from((Ipv4Addr::BROADCAST, port));
    if let Err(error) = socket.send_to(&payload, target).await {
        eprintln!("lan broadcast failed: {error}");
    }

    // 多实例测试：当监听端口被 STUDYHALL_LAN_PORT 覆盖时，额外向固定的 47821
    // 与本机回环发送一份，方便同机另一个默认端口的实例收到发现报文。
    for target in extra_targets(port) {
        if let Err(error) = socket.send_to(&payload, target).await {
            eprintln!("lan broadcast failed: {error}");
        }
    }
}

fn extra_targets(port: u16) -> Vec<SocketAddr> {
    if port == DEFAULT_DISCOVERY_PORT {
        return Vec::new();
    }
    vec![
        SocketAddr::from((Ipv4Addr::BROADCAST, DEFAULT_DISCOVERY_PORT)),
        SocketAddr::from((Ipv4Addr::LOCALHOST, DEFAULT_DISCOVERY_PORT)),
    ]
}

fn handle_datagram(shared: &Arc<Shared>, payload: &[u8], source: SocketAddr) {
    let packet: DiscoveryPacket = match serde_json::from_slice(payload) {
        Ok(packet) => packet,
        Err(_) => return,
    };
    if packet.v != 1 || packet.port == 0 || packet.id.trim().is_empty() {
        return;
    }

    let address = SocketAddr::new(source.ip(), packet.port);
    let timer_busy = is_timer_busy(shared);
    let mut discovered_new = false;
    let mut state_changed = false;
    let mut dial: Option<(String, SocketAddr)> = None;

    {
        let mut inner = shared.inner.lock().expect("lan lock");
        if inner.device_id == packet.id {
            return;
        }

        match inner.discovered.get_mut(&packet.id) {
            Some(record) => {
                record.name = packet.name.clone();
                record.addr = address;
            }
            None => {
                inner.discovered.insert(
                    packet.id.clone(),
                    super::Discovered {
                        name: packet.name.clone(),
                        addr: address,
                        auto_attempted: false,
                    },
                );
                discovered_new = true;
            }
        }

        if let Some(peer) = inner.peers.get_mut(&packet.id) {
            if peer.name.as_str() != packet.name {
                peer.name = packet.name.clone();
                state_changed = true;
            }
        } else if !timer_busy && inner.available && inner.peers.len() < MAX_PEERS {
            let should_dial = match inner.discovered.get_mut(&packet.id) {
                Some(record) => {
                    if record.auto_attempted {
                        false
                    } else {
                        record.auto_attempted = true;
                        true
                    }
                }
                None => false,
            };
            if should_dial {
                dial = Some((packet.id.clone(), address));
            }
        }

        if discovered_new {
            inner.message = format!("发现设备 {}", packet.name);
            state_changed = true;
        }
    }

    if state_changed {
        emit_status(shared);
    }
    if let Some((peer_id, address)) = dial {
        transport::spawn_dial(shared.clone(), peer_id, address);
    }
}

async fn ensure_socket(
    current: &mut Option<DiscoverySocket>,
    port: u16,
) -> Result<(), std::io::Error> {
    if let Some(socket) = current {
        if socket.port == port {
            return Ok(());
        }
    }
    *current = None;

    let std_socket = std::net::UdpSocket::bind((Ipv4Addr::UNSPECIFIED, port))?;
    std_socket.set_broadcast(true)?;
    std_socket.set_nonblocking(true)?;
    let socket = UdpSocket::from_std(std_socket)?;
    *current = Some(DiscoverySocket {
        port,
        socket: Arc::new(socket),
    });
    Ok(())
}

fn is_current(shared: &Shared, generation: u64) -> bool {
    shared.inner.lock().expect("lan lock").generation == generation
}
