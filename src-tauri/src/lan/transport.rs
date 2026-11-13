//! TCP 传输：监听随机端口，处理 hello 握手、心跳、名字与计时同步。
//!
//! 每条连接拆成读半端（当前任务内联运行）与写半端（独立任务，每 1 秒发 ping）。
//! 连接取消通过 `watch` 广播，不跨 `await` 持有任何 `std::sync::Mutex`。

use std::io;
use std::net::{Ipv4Addr, SocketAddr};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use tokio::io::AsyncWriteExt;
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{mpsc, watch};

use super::protocol::{encode_frame, read_frame, write_frame, WireMessage};
use super::{
    emit_status, is_timer_busy, note_message, Peer, Shared, CONNECT_TIMEOUT, MAX_PEERS, PEER_TIMEOUT,
    PING_INTERVAL, UNAVAILABLE_MESSAGE, WRITE_TIMEOUT,
};

static NEXT_TOKEN: AtomicU64 = AtomicU64::new(1);

fn next_token() -> u64 {
    NEXT_TOKEN.fetch_add(1, Ordering::Relaxed)
}

/// 确保 TCP 监听任务存在；返回的端口会写入 `LanInner::tcp_port` 并随 UDP 广播发出。
pub(super) fn ensure_listener(shared: &Arc<Shared>) -> Result<(), String> {
    {
        let inner = shared.inner.lock().expect("lan lock");
        if inner.listener.is_some() {
            return Ok(());
        }
    }

    let std_listener = std::net::TcpListener::bind((Ipv4Addr::UNSPECIFIED, 0))
        .map_err(|error| error.to_string())?;
    std_listener
        .set_nonblocking(true)
        .map_err(|error| error.to_string())?;
    let port = std_listener
        .local_addr()
        .map_err(|error| error.to_string())?
        .port();
    if port == 0 {
        return Err("操作系统未分配有效 TCP 端口".to_string());
    }

    let (cancel, cancel_rx) = watch::channel(false);
    {
        let mut inner = shared.inner.lock().expect("lan lock");
        // 竞态兜底：另一个线程已经启动监听，丢弃本端刚绑定的 socket。
        if inner.listener.is_some() {
            return Ok(());
        }
        inner.tcp_port = Some(port);
        inner.accepting = true;
        inner.listener = Some(super::ListenerSession { cancel });
    }

    let shared = shared.clone();
    drop(tauri::async_runtime::spawn(async move {
        accept_loop(shared, std_listener, port, cancel_rx).await;
    }));
    Ok(())
}

async fn accept_loop(
    shared: Arc<Shared>,
    std_listener: std::net::TcpListener,
    port: u16,
    mut cancel: watch::Receiver<bool>,
) {
    let listener = match TcpListener::from_std(std_listener) {
        Ok(listener) => listener,
        Err(error) => {
            eprintln!("lan accept loop failed: {error}");
            {
                let mut inner = shared.inner.lock().expect("lan lock");
                if inner.tcp_port == Some(port) {
                    inner.tcp_port = None;
                    inner.listener = None;
                    inner.accepting = false;
                    inner.available = false;
                    inner.error = Some(UNAVAILABLE_MESSAGE.to_string());
                    inner.message = UNAVAILABLE_MESSAGE.to_string();
                }
            }
            emit_status(&shared);
            return;
        }
    };

    loop {
        tokio::select! {
            _ = cancel.changed() => break,
            accepted = listener.accept() => match accepted {
                Ok((stream, address)) => match rejection_reason(&shared) {
                    Some(message) => {
                        drop(tokio::spawn(async move {
                            reject_inbound(stream, message).await;
                        }));
                    }
                    None => {
                        let shared = shared.clone();
                        drop(tokio::spawn(async move {
                            serve_inbound(shared, stream, address).await;
                        }));
                    }
                },
                Err(error) => {
                    if error.kind() != io::ErrorKind::Interrupted {
                        tokio::time::sleep(Duration::from_millis(100)).await;
                    }
                }
            }
        }
    }
}

fn rejection_reason(shared: &Shared) -> Option<&'static str> {
    if is_timer_busy(shared) {
        return Some("计时进行中，暂不可连接");
    }
    let inner = shared.inner.lock().expect("lan lock");
    if !inner.available {
        return Some("当前网络不可用");
    }
    if inner.peers.len() >= MAX_PEERS {
        return Some("连接已满");
    }
    None
}

async fn serve_inbound(shared: Arc<Shared>, stream: TcpStream, address: SocketAddr) {
    if let Some(message) = rejection_reason(&shared) {
        reject_inbound(stream, message).await;
        return;
    }
    run_connection(shared, stream, address, false, None).await;
}

async fn reject_inbound(mut stream: TcpStream, message: &'static str) {
    let rejection = WireMessage::Error {
        message: message.to_string(),
    };
    let _ = tokio::time::timeout(WRITE_TIMEOUT, write_frame(&mut stream, &rejection)).await;
    let _ = stream.shutdown().await;
}

/// 对已发现的地址发起 TCP 连接（自动发现与手动重连共用）。
pub(super) fn spawn_dial(shared: Arc<Shared>, expected_id: String, address: SocketAddr) {
    drop(tauri::async_runtime::spawn(async move {
        dial(shared, expected_id, address).await;
    }));
}

async fn dial(shared: Arc<Shared>, expected_id: String, address: SocketAddr) {
    if is_timer_busy(&shared) {
        return;
    }
    {
        let inner = shared.inner.lock().expect("lan lock");
        if !inner.accepting
            || !inner.available
            || inner.peers.contains_key(&expected_id)
            || inner.peers.len() >= MAX_PEERS
        {
            return;
        }
    }

    let stream = match tokio::time::timeout(CONNECT_TIMEOUT, TcpStream::connect(address)).await {
        Ok(Ok(stream)) => stream,
        Ok(Err(error)) => {
            note_connect_failure(&shared, &expected_id, &format!("无法连接：{error}"));
            return;
        }
        Err(_) => {
            note_connect_failure(&shared, &expected_id, "连接超时");
            return;
        }
    };
    let _ = stream.set_nodelay(true);
    run_connection(shared, stream, address, true, Some(expected_id)).await;
}

fn note_connect_failure(shared: &Shared, peer_id: &str, reason: &str) {
    let display = {
        let inner = shared.inner.lock().expect("lan lock");
        inner
            .discovered
            .get(peer_id)
            .map(|record| record.name.clone())
            .unwrap_or_else(|| peer_id.to_string())
    };
    note_message(shared, format!("连接 {display} 失败（{reason}）"));
}

async fn run_connection(
    shared: Arc<Shared>,
    stream: TcpStream,
    address: SocketAddr,
    outbound: bool,
    expected_id: Option<String>,
) {
    let token = next_token();
    let _ = stream.set_nodelay(true);
    let hello = {
        let inner = shared.inner.lock().expect("lan lock");
        WireMessage::Hello {
            id: inner.device_id.clone(),
            name: inner.device_name.clone(),
        }
    };
    let (mut read_half, mut write_half) = stream.into_split();

    let hello_written =
        tokio::time::timeout(WRITE_TIMEOUT, write_frame(&mut write_half, &hello)).await;
    if !matches!(hello_written, Ok(Ok(()))) {
        note_connection_closed(&shared, outbound, expected_id.as_deref());
        return;
    }

    let (frames, frames_rx) = mpsc::unbounded_channel::<Vec<u8>>();
    let (cancel, cancel_rx) = watch::channel(false);
    let cleanup = cancel.clone();
    let writer_cancel = cancel_rx.clone();
    drop(tokio::spawn(async move {
        writer_loop(write_half, frames_rx, writer_cancel).await;
    }));

    reader_loop(
        shared,
        &mut read_half,
        address,
        outbound,
        expected_id,
        token,
        frames,
        cancel,
        cancel_rx,
    )
    .await;

    let _ = cleanup.send(true);
}

async fn writer_loop(
    mut writer: OwnedWriteHalf,
    mut frames: mpsc::UnboundedReceiver<Vec<u8>>,
    mut cancel: watch::Receiver<bool>,
) {
    let ping = encode_frame(&WireMessage::Ping);
    let mut ticker = tokio::time::interval(PING_INTERVAL);
    ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

    loop {
        tokio::select! {
            _ = cancel.changed() => break,
            _ = ticker.tick() => {
                if writer.write_all(&ping).await.is_err() {
                    break;
                }
            }
            frame = frames.recv() => match frame {
                Some(frame) => {
                    if writer.write_all(&frame).await.is_err() {
                        break;
                    }
                }
                None => break,
            }
        }
    }
    let _ = writer.shutdown().await;
}

enum Handshake {
    Hello { id: String, name: String },
    Rejected { message: String },
    Closed,
}

async fn wait_for_hello(reader: &mut OwnedReadHalf) -> Handshake {
    match tokio::time::timeout(PEER_TIMEOUT, read_frame(reader)).await {
        Ok(Ok(Some(WireMessage::Hello { id, name }))) => Handshake::Hello { id, name },
        Ok(Ok(Some(WireMessage::Error { message }))) => Handshake::Rejected { message },
        _ => Handshake::Closed,
    }
}

fn note_connection_closed(shared: &Shared, outbound: bool, expected_id: Option<&str>) {
    if !outbound {
        return;
    }
    let Some(peer_id) = expected_id else {
        return;
    };
    let already_connected = {
        let inner = shared.inner.lock().expect("lan lock");
        inner.peers.contains_key(peer_id)
    };
    if !already_connected {
        note_connect_failure(shared, peer_id, "握手失败");
    }
}

#[allow(clippy::too_many_arguments)]
async fn reader_loop(
    shared: Arc<Shared>,
    reader: &mut OwnedReadHalf,
    address: SocketAddr,
    outbound: bool,
    expected_id: Option<String>,
    token: u64,
    frames: mpsc::UnboundedSender<Vec<u8>>,
    cancel: watch::Sender<bool>,
    mut cancel_rx: watch::Receiver<bool>,
) {
    let (peer_id, peer_name) = match wait_for_hello(reader).await {
        Handshake::Hello { id, name } => (id, name),
        Handshake::Rejected { message } => {
            note_message(&shared, format!("连接被拒绝：{message}"));
            let _ = cancel.send(true);
            return;
        }
        Handshake::Closed => {
            note_connection_closed(&shared, outbound, expected_id.as_deref());
            let _ = cancel.send(true);
            return;
        }
    };

    if peer_id.trim().is_empty() {
        let _ = cancel.send(true);
        return;
    }
    let is_self = {
        let inner = shared.inner.lock().expect("lan lock");
        peer_id.as_str() == inner.device_id.as_str()
    };
    if is_self {
        let _ = cancel.send(true);
        return;
    }

    // 握手期间本地计时可能已启动：此时按“不接受新连接”处理。
    if is_timer_busy(&shared) {
        let _ = frames.send(encode_frame(&WireMessage::Error {
            message: "计时进行中，暂不可连接".to_string(),
        }));
        tokio::time::sleep(Duration::from_millis(50)).await;
        let _ = cancel.send(true);
        return;
    }

    if let Err(error) = register_peer(
        &shared,
        &peer_id,
        &peer_name,
        address,
        outbound,
        token,
        frames.clone(),
        cancel.clone(),
    ) {
        match error {
            RegisterError::Full => {
                let _ = frames.send(encode_frame(&WireMessage::Error {
                    message: "连接已满".to_string(),
                }));
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
            RegisterError::Duplicate | RegisterError::Invalid => {}
        }
        let _ = cancel.send(true);
        return;
    }

    loop {
        let outcome = tokio::select! {
            _ = cancel_rx.changed() => break,
            result = tokio::time::timeout(PEER_TIMEOUT, read_frame(reader)) => result,
        };

        match outcome {
            Err(_) => {
                remove_peer(&shared, &peer_id, token, "设备已离线");
                break;
            }
            Ok(Ok(Some(WireMessage::Timer { snapshot }))) => {
                let applied = shared.timer.apply_remote(snapshot);
                if let Some(app) = shared.app() {
                    crate::events::emit_timer(app, &applied);
                    crate::tray::update_tooltip(app, &applied);
                }
                // 本地计时被远端同步后 can_connect 会变化；仅刷新状态，不回广播。
                emit_status(&shared);
            }
            Ok(Ok(Some(WireMessage::Name { name }))) => {
                update_peer_name(&shared, &peer_id, &name);
            }
            Ok(Ok(Some(WireMessage::Ping))) | Ok(Ok(Some(WireMessage::Hello { .. }))) => {}
            Ok(Ok(Some(WireMessage::Bye))) => {
                remove_peer(&shared, &peer_id, token, "对方已断开连接");
                break;
            }
            Ok(Ok(Some(WireMessage::Error { message }))) => {
                remove_peer(&shared, &peer_id, token, &format!("连接异常：{message}"));
                break;
            }
            Ok(Ok(None)) => {
                remove_peer(&shared, &peer_id, token, "对方已断开连接");
                break;
            }
            Ok(Err(_)) => {
                remove_peer(&shared, &peer_id, token, "连接已断开");
                break;
            }
        }
    }

    let _ = cancel.send(true);
}

enum RegisterError {
    Duplicate,
    Full,
    Invalid,
}

#[allow(clippy::too_many_arguments)]
fn register_peer(
    shared: &Shared,
    peer_id: &str,
    peer_name: &str,
    address: SocketAddr,
    outbound: bool,
    token: u64,
    frames: mpsc::UnboundedSender<Vec<u8>>,
    cancel: watch::Sender<bool>,
) -> Result<(), RegisterError> {
    let mut stale_cancel: Option<watch::Sender<bool>> = None;
    {
        let mut inner = shared.inner.lock().expect("lan lock");
        if peer_id.is_empty() || peer_id == inner.device_id.as_str() {
            return Err(RegisterError::Invalid);
        }
        if !inner.accepting {
            return Err(RegisterError::Invalid);
        }

        if inner.peers.contains_key(peer_id) {
            // 双方同时互连时保留同一条连接：按 id 较小者发起的连接为准。
            let keep_new = if outbound {
                inner.device_id.as_str() < peer_id
            } else {
                inner.device_id.as_str() > peer_id
            };
            if !keep_new {
                return Err(RegisterError::Duplicate);
            }
            stale_cancel = inner.peers.remove(peer_id).map(|peer| peer.cancel);
        }

        if inner.peers.len() >= MAX_PEERS {
            return Err(RegisterError::Full);
        }

        let display_name = if peer_name.trim().is_empty() {
            peer_id.to_string()
        } else {
            peer_name.to_string()
        };
        inner.peers.insert(
            peer_id.to_string(),
            Peer {
                name: display_name.clone(),
                token,
                cancel,
                frames,
            },
        );
        {
            let record = inner
                .discovered
                .entry(peer_id.to_string())
                .or_insert_with(|| super::Discovered {
                    name: display_name.clone(),
                    addr: address,
                    auto_attempted: true,
                });
            record.name = display_name.clone();
            record.addr = address;
            record.auto_attempted = true;
        }
        inner.message = format!("已连接 {display_name}");
    }

    if let Some(cancel) = stale_cancel {
        let _ = cancel.send(true);
    }
    emit_status(shared);
    Ok(())
}

fn update_peer_name(shared: &Shared, peer_id: &str, name: &str) {
    let name = name.trim();
    if name.is_empty() {
        return;
    }
    let changed = {
        let mut inner = shared.inner.lock().expect("lan lock");
        let mut changed = false;
        if let Some(peer) = inner.peers.get_mut(peer_id) {
            if peer.name.as_str() != name {
                peer.name = name.to_string();
                changed = true;
            }
        }
        if let Some(record) = inner.discovered.get_mut(peer_id) {
            record.name = name.to_string();
        }
        changed
    };
    if changed {
        emit_status(shared);
    }
}

fn remove_peer(shared: &Shared, peer_id: &str, token: u64, message: &str) {
    let removed = {
        let mut inner = shared.inner.lock().expect("lan lock");
        let is_current = inner.peers.get(peer_id).map(|peer| peer.token) == Some(token);
        if !is_current {
            None
        } else {
            let peer = inner.peers.remove(peer_id);
            if peer.is_some() {
                inner.message = message.to_string();
            }
            peer
        }
    };

    if let Some(peer) = removed {
        let _ = peer.cancel.send(true);
        emit_status(shared);
    }
}

/// 向所有已连接 peer 投递一条消息（非阻塞）。
pub(super) fn send_to_peers(shared: &Shared, message: &WireMessage) {
    let frame = encode_frame(message);
    let inner = shared.inner.lock().expect("lan lock");
    for peer in inner.peers.values() {
        let _ = peer.frames.send(frame.clone());
    }
}
