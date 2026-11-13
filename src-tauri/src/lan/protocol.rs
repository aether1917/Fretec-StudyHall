//! 局域网线协议：4 字节大端长度前缀 + UTF-8 JSON 帧。
//!
//! 帧格式：
//! ```text
//! +----------------+---------------------------+
//! | u32 big endian | JSON body (UTF-8)         |
//! | body length    |                           |
//! +----------------+---------------------------+
//! ```
//!
//! 本模块只做纯数据编解码，不触碰网络、状态与事件，便于单元测试。

use std::io;

use serde::{Deserialize, Serialize};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

use crate::types::TimerSnapshot;

/// 单帧 JSON 上限，避免异常长度导致超大内存分配。
pub(super) const MAX_FRAME_LEN: usize = 64 * 1024;

/// TCP 报文。`type` 为内部标签，其余字段随类型变化。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub(super) enum WireMessage {
    /// 连接建立后的首个报文，用于交换设备身份。
    Hello { id: String, name: String },
    /// 倒计时快照；`snapshot` 内部字段由 [`TimerSnapshot`] 序列化为 camelCase。
    Timer { snapshot: TimerSnapshot },
    /// 设备改名通知。
    Name { name: String },
    /// 心跳，每 1 秒一次。
    Ping,
    /// 主动离开。
    Bye,
    /// 拒绝/异常通知，例如服务端连接已满。
    Error { message: String },
}

/// UDP 发现报文：`{"v":1,"id":...,"name":...,"port":...}`。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(super) struct DiscoveryPacket {
    pub(super) v: u8,
    pub(super) id: String,
    pub(super) name: String,
    /// 对端 TCP 监听端口（随机端口）。
    pub(super) port: u16,
}

#[derive(Debug, thiserror::Error)]
pub(super) enum FrameError {
    #[error("LAN 帧内容为空")]
    Empty,
    #[error("LAN 帧长度 {0} 超出上限")]
    TooLarge(usize),
    #[error("LAN 帧不是合法 JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("LAN 帧读写失败: {0}")]
    Io(#[from] io::Error),
}

/// 把消息编码为一个完整帧（长度前缀 + JSON body）。
pub(super) fn encode_frame(message: &WireMessage) -> Vec<u8> {
    let body = serde_json::to_vec(message).expect("LAN 消息序列化不应失败");
    let mut frame = Vec::with_capacity(4 + body.len());
    frame.extend_from_slice(&(body.len() as u32).to_be_bytes());
    frame.extend_from_slice(&body);
    frame
}

/// 写入一个完整帧并 flush。
pub(super) async fn write_frame<W>(writer: &mut W, message: &WireMessage) -> io::Result<()>
where
    W: AsyncWrite + Unpin,
{
    writer.write_all(&encode_frame(message)).await?;
    writer.flush().await
}

/// 读取一个完整帧；返回 `Ok(None)` 表示对端在帧边界处正常关闭。
pub(super) async fn read_frame<R>(reader: &mut R) -> Result<Option<WireMessage>, FrameError>
where
    R: AsyncRead + Unpin,
{
    let mut header = [0u8; 4];
    match reader.read_exact(&mut header).await {
        Ok(_) => {}
        Err(error) if error.kind() == io::ErrorKind::UnexpectedEof => return Ok(None),
        Err(error) => return Err(FrameError::Io(error)),
    }

    let length = u32::from_be_bytes(header) as usize;
    validate_length(length)?;

    let mut body = vec![0u8; length];
    reader.read_exact(&mut body).await.map_err(FrameError::Io)?;
    let message = decode_body(&body)?;
    Ok(Some(message))
}

fn validate_length(length: usize) -> Result<(), FrameError> {
    if length == 0 {
        return Err(FrameError::Empty);
    }
    if length > MAX_FRAME_LEN {
        return Err(FrameError::TooLarge(length));
    }
    Ok(())
}

fn decode_body(body: &[u8]) -> Result<WireMessage, FrameError> {
    serde_json::from_slice(body).map_err(FrameError::from)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{Phase, TimerSnapshot, TimerStatus};
    use serde_json::json;
    use tokio::io::AsyncWriteExt;

    /// 纯内存帧解析，仅测试使用；线上读取走 [`read_frame`]。
    fn parse_frame(bytes: &[u8]) -> Result<Option<(WireMessage, usize)>, FrameError> {
        if bytes.len() < 4 {
            return Ok(None);
        }
        let length = u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]) as usize;
        validate_length(length)?;
        let end = 4 + length;
        if bytes.len() < end {
            return Ok(None);
        }
        let message = decode_body(&bytes[4..end])?;
        Ok(Some((message, end)))
    }

    #[test]
    fn frame_uses_big_endian_length_prefix() {
        let message = WireMessage::Ping;
        let body = serde_json::to_vec(&message).expect("serialize ping");
        let frame = encode_frame(&message);

        assert_eq!(u32::from_be_bytes([frame[0], frame[1], frame[2], frame[3]]), body.len() as u32);
        assert_eq!(&frame[4..], body.as_slice());
        assert_eq!(frame.len(), 4 + body.len());
    }

    #[test]
    fn hello_frame_roundtrips() {
        let message = WireMessage::Hello {
            id: "peer-a".to_string(),
            name: "书桌".to_string(),
        };
        let frame = encode_frame(&message);
        let (decoded, consumed) = parse_frame(&frame)
            .expect("frame must parse")
            .expect("frame must be complete");

        assert_eq!(consumed, frame.len());
        match decoded {
            WireMessage::Hello { id, name } => {
                assert_eq!(id, "peer-a");
                assert_eq!(name, "书桌");
            }
            other => panic!("unexpected message: {other:?}"),
        }
    }

    #[test]
    fn parser_handles_partial_and_concatenated_frames() {
        let first = encode_frame(&WireMessage::Ping);
        let second = encode_frame(&WireMessage::Name {
            name: "新名字".to_string(),
        });

        let mut stream = first.clone();
        stream.extend_from_slice(&second);

        let (message, consumed) = parse_frame(&stream)
            .expect("first frame must parse")
            .expect("first frame must be complete");
        assert!(matches!(message, WireMessage::Ping));
        assert_eq!(consumed, first.len());

        let (message, second_consumed) = parse_frame(&stream[consumed..])
            .expect("second frame must parse")
            .expect("second frame must be complete");
        match message {
            WireMessage::Name { name } => assert_eq!(name, "新名字"),
            other => panic!("unexpected message: {other:?}"),
        }
        assert_eq!(stream.len(), consumed + second_consumed);

        assert!(parse_frame(&stream[..3]).expect("partial header is fine").is_none());
        assert!(parse_frame(&first[..first.len() - 1])
            .expect("partial body is fine")
            .is_none());
    }

    #[test]
    fn parser_rejects_empty_and_oversized_frames() {
        assert!(matches!(parse_frame(&[0, 0, 0, 0]), Err(FrameError::Empty)));

        let oversized = (MAX_FRAME_LEN as u32 + 1).to_be_bytes();
        assert!(matches!(
            parse_frame(&oversized),
            Err(FrameError::TooLarge(_))
        ));
    }

    #[test]
    fn timer_snapshot_roundtrips_as_payload() {
        let snapshot = TimerSnapshot {
            phase: Phase::Break,
            total_ms: 300_000,
            started_at_ms: Some(1_700_000_000_000),
            pause_offset_ms: 500,
            paused_at_ms: Some(1_700_000_010_000),
            status: TimerStatus::Paused,
        };
        let frame = encode_frame(&WireMessage::Timer {
            snapshot: snapshot.clone(),
        });
        let (decoded, consumed) = parse_frame(&frame)
            .expect("timer frame must parse")
            .expect("timer frame must be complete");
        assert_eq!(consumed, frame.len());
        let message = serde_json::to_value(&WireMessage::Timer {
            snapshot: snapshot.clone(),
        })
        .expect("serialize timer message");
        assert_eq!(message["type"], json!("timer"));
        assert_eq!(message["snapshot"]["totalMs"], json!(300_000));

        match decoded {
            WireMessage::Timer { snapshot: decoded } => {
                assert_eq!(decoded.phase, snapshot.phase);
                assert_eq!(decoded.total_ms, snapshot.total_ms);
                assert_eq!(decoded.started_at_ms, snapshot.started_at_ms);
                assert_eq!(decoded.pause_offset_ms, snapshot.pause_offset_ms);
                assert_eq!(decoded.paused_at_ms, snapshot.paused_at_ms);
                assert_eq!(decoded.status, snapshot.status);

                let value = serde_json::to_value(&decoded).expect("serialize snapshot");
                assert_eq!(value["phase"], json!("break"));
                assert_eq!(value["totalMs"], json!(300_000));
                assert_eq!(value["startedAtMs"], json!(1_700_000_000_000u64));
                assert_eq!(value["pauseOffsetMs"], json!(500));
                assert_eq!(value["pausedAtMs"], json!(1_700_000_010_000u64));
                assert_eq!(value["status"], json!("paused"));
            }
            other => panic!("unexpected message: {other:?}"),
        }
    }

    #[test]
    fn discovery_packet_serializes_expected_fields() {
        let packet = DiscoveryPacket {
            v: 1,
            id: "device-a".to_string(),
            name: "书桌".to_string(),
            port: 51_234,
        };
        let value = serde_json::to_value(&packet).expect("serialize discovery packet");
        assert_eq!(value["v"], json!(1));
        assert_eq!(value["id"], json!("device-a"));
        assert_eq!(value["name"], json!("书桌"));
        assert_eq!(value["port"], json!(51_234));

        let decoded: DiscoveryPacket =
            serde_json::from_value(value).expect("deserialize discovery packet");
        assert_eq!(decoded, packet);
    }

    #[test]
    fn wire_messages_use_expected_type_tags() {
        let cases = [
            (
                WireMessage::Hello {
                    id: "a".to_string(),
                    name: "A".to_string(),
                },
                "hello",
            ),
            (WireMessage::Ping, "ping"),
            (WireMessage::Bye, "bye"),
            (
                WireMessage::Error {
                    message: "连接已满".to_string(),
                },
                "error",
            ),
        ];
        for (message, expected) in cases {
            let value = serde_json::to_value(&message).expect("serialize message");
            assert_eq!(value["type"], json!(expected));
        }
    }

    #[test]
    fn ping_and_full_error_have_exact_wire_shape() {
        let ping = encode_frame(&WireMessage::Ping);
        assert_eq!(
            std::str::from_utf8(&ping[4..]).expect("ping body is utf-8"),
            r#"{"type":"ping"}"#
        );
        let decoded: WireMessage = serde_json::from_slice(&ping[4..]).expect("parse ping");
        assert!(matches!(decoded, WireMessage::Ping));

        let error = WireMessage::Error {
            message: "连接已满".to_string(),
        };
        let value = serde_json::to_value(&error).expect("serialize error");
        assert_eq!(
            value,
            json!({"type": "error", "message": "连接已满"})
        );
        let decoded: WireMessage = serde_json::from_value(value).expect("parse error");
        assert!(matches!(decoded, WireMessage::Error { message } if message == "连接已满"));
    }

    #[tokio::test]
    async fn read_frame_handles_stream_and_clean_eof() {
        let (mut client, mut server) = tokio::io::duplex(256);
        let frame = encode_frame(&WireMessage::Ping);
        client
            .write_all(&frame)
            .await
            .expect("write ping frame");

        let decoded = read_frame(&mut server).await.expect("read ping frame");
        assert!(matches!(decoded, Some(WireMessage::Ping)));

        drop(client);
        assert!(read_frame(&mut server)
            .await
            .expect("clean eof is not an error")
            .is_none());
    }

    #[test]
    fn decodes_literal_timer_message_with_camel_case_fields() {
        let literal = br#"{"type":"timer","snapshot":{"phase":"focus","totalMs":1500000,"startedAtMs":1700000000000,"pauseOffsetMs":0,"pausedAtMs":null,"status":"running"}}"#;
        let message: WireMessage = serde_json::from_slice(literal).expect("parse timer message");
        match message {
            WireMessage::Timer { snapshot } => {
                assert_eq!(snapshot.phase, Phase::Focus);
                assert_eq!(snapshot.total_ms, 1_500_000);
                assert_eq!(snapshot.started_at_ms, Some(1_700_000_000_000));
                assert_eq!(snapshot.pause_offset_ms, 0);
                assert_eq!(snapshot.paused_at_ms, None);
                assert_eq!(snapshot.status, TimerStatus::Running);
            }
            other => panic!("unexpected message: {other:?}"),
        }
    }
}
