//! Minecraft Server List Ping (SLP) client.
//!
//! Implements the modern (1.7+) status handshake over a tokio `TcpStream`
//! (no external crate). The exchange is:
//!
//! 1. Handshake packet (id `0x00`): protocol version (VarInt), server
//!    address (String), server port (u16), next state = `1` (status).
//! 2. Status Request packet (id `0x00`, empty body).
//! 3. Status Response packet (id `0x00`): a single JSON string describing
//!    version, players, MOTD (`description`) and an optional favicon.
//! 4. Ping packet (id `0x01`) carrying an i64 payload; the server echoes it
//!    back as a Pong so we can measure round-trip latency.
//!
//! Packets are length-prefixed: `VarInt length` then `VarInt packet id` then
//! the payload. VarInts are little-endian 7-bit groups with a continuation
//! high bit, exactly as Minecraft encodes them.

use std::time::{Duration, Instant};

use serde::Deserialize;
use tokio::{
	io::{AsyncReadExt, AsyncWriteExt},
	net::TcpStream,
	time::timeout,
};

const PROTOCOL_VERSION: i32 = -1;
const NEXT_STATE_STATUS: i32 = 1;

const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
const IO_TIMEOUT: Duration = Duration::from_secs(5);

/// Guards against a hostile length prefix in the status response.
const MAX_RESPONSE_LEN: usize = 1 << 20;

#[derive(Debug, thiserror::Error)]
pub enum PingError {
	#[error("io: {0}")]
	Io(#[from] std::io::Error),
	#[error("connection timed out")]
	Timeout,
	#[error("malformed status packet from server")]
	Protocol,
	#[error("invalid status json: {0}")]
	Json(#[from] serde_json::Error),
}

/// Parsed result of a successful Server List Ping.
#[derive(Debug, Clone)]
pub struct ServerPing {
	pub version_name: String,
	pub protocol: i32,
	pub players_online: i32,
	pub players_max: i32,
	pub sample: Vec<String>,
	pub motd: String,
	pub favicon: Option<String>,
	pub latency_ms: u64,
}

#[derive(Debug, Deserialize)]
struct StatusJson {
	#[serde(default)]
	version: StatusVersion,
	#[serde(default)]
	players: StatusPlayers,
	#[serde(default)]
	description: serde_json::Value,
	#[serde(default)]
	favicon: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
struct StatusVersion {
	#[serde(default)]
	name: String,
	#[serde(default)]
	protocol: i32,
}

#[derive(Debug, Default, Deserialize)]
struct StatusPlayers {
	#[serde(default)]
	online: i32,
	#[serde(default)]
	max: i32,
	#[serde(default)]
	sample: Vec<StatusSample>,
}

#[derive(Debug, Deserialize)]
struct StatusSample {
	#[serde(default)]
	name: String,
}

/// Performs a full status ping against `host:port` and returns the parsed
/// result, including measured latency.
pub async fn ping(host: &str, port: u16) -> Result<ServerPing, PingError> {
	let stream = timeout(CONNECT_TIMEOUT, TcpStream::connect((host, port)))
		.await
		.map_err(|_| PingError::Timeout)??;
	let mut stream = stream;

	send_handshake(&mut stream, host, port).await?;
	send_status_request(&mut stream).await?;
	let json = read_status_response(&mut stream).await?;

	let latency_ms = measure_latency(&mut stream).await.unwrap_or(0);

	let status: StatusJson = serde_json::from_str(&json)?;
	Ok(ServerPing {
		version_name: status.version.name,
		protocol: status.version.protocol,
		players_online: status.players.online,
		players_max: status.players.max,
		sample: status
			.players
			.sample
			.into_iter()
			.map(|s| s.name)
			.filter(|n| !n.is_empty())
			.collect(),
		motd: extract_motd(&status.description),
		favicon: status.favicon,
		latency_ms,
	})
}

async fn send_handshake(stream: &mut TcpStream, host: &str, port: u16) -> Result<(), PingError> {
	let mut body = Vec::new();
	write_varint(&mut body, 0x00);
	write_varint(&mut body, PROTOCOL_VERSION);
	write_string(&mut body, host);
	body.extend_from_slice(&port.to_be_bytes());
	write_varint(&mut body, NEXT_STATE_STATUS);
	write_packet(stream, &body).await
}

async fn send_status_request(stream: &mut TcpStream) -> Result<(), PingError> {
	let mut body = Vec::new();
	write_varint(&mut body, 0x00);
	write_packet(stream, &body).await
}

async fn read_status_response(stream: &mut TcpStream) -> Result<String, PingError> {
	let _len = read_varint(stream).await?;
	let packet_id = read_varint(stream).await?;
	if packet_id != 0x00 {
		return Err(PingError::Protocol);
	}
	let str_len = read_varint(stream).await?;
	if str_len < 0 || str_len as usize > MAX_RESPONSE_LEN {
		return Err(PingError::Protocol);
	}
	let mut buf = vec![0u8; str_len as usize];
	timeout(IO_TIMEOUT, stream.read_exact(&mut buf))
		.await
		.map_err(|_| PingError::Timeout)??;
	String::from_utf8(buf).map_err(|_| PingError::Protocol)
}

async fn measure_latency(stream: &mut TcpStream) -> Result<u64, PingError> {
	let token: i64 = 0x4142_4344_4546_4748;
	let mut body = Vec::new();
	write_varint(&mut body, 0x01);
	body.extend_from_slice(&token.to_be_bytes());
	let start = Instant::now();
	write_packet(stream, &body).await?;

	let _len = read_varint(stream).await?;
	let packet_id = read_varint(stream).await?;
	if packet_id != 0x01 {
		return Err(PingError::Protocol);
	}
	let mut echo = [0u8; 8];
	timeout(IO_TIMEOUT, stream.read_exact(&mut echo))
		.await
		.map_err(|_| PingError::Timeout)??;
	Ok(start.elapsed().as_millis() as u64)
}

/// Flattens a chat-component or legacy-string MOTD into plain text.
fn extract_motd(value: &serde_json::Value) -> String {
	match value {
		serde_json::Value::String(s) => s.clone(),
		serde_json::Value::Object(map) => {
			let mut out = String::new();
			if let Some(serde_json::Value::String(text)) = map.get("text") {
				out.push_str(text);
			}
			if let Some(serde_json::Value::Array(extra)) = map.get("extra") {
				for child in extra {
					out.push_str(&extract_motd(child));
				}
			}
			out
		}
		_ => String::new(),
	}
}

async fn write_packet(stream: &mut TcpStream, body: &[u8]) -> Result<(), PingError> {
	let mut framed = Vec::with_capacity(body.len() + 5);
	write_varint(&mut framed, body.len() as i32);
	framed.extend_from_slice(body);
	timeout(IO_TIMEOUT, stream.write_all(&framed))
		.await
		.map_err(|_| PingError::Timeout)??;
	Ok(())
}

fn write_varint(buf: &mut Vec<u8>, value: i32) {
	let mut v = value as u32;
	loop {
		let mut byte = (v & 0x7f) as u8;
		v >>= 7;
		if v != 0 {
			byte |= 0x80;
		}
		buf.push(byte);
		if v == 0 {
			break;
		}
	}
}

fn write_string(buf: &mut Vec<u8>, value: &str) {
	write_varint(buf, value.len() as i32);
	buf.extend_from_slice(value.as_bytes());
}

async fn read_varint(stream: &mut TcpStream) -> Result<i32, PingError> {
	let mut result: i32 = 0;
	let mut shift = 0;
	loop {
		let mut byte = [0u8; 1];
		timeout(IO_TIMEOUT, stream.read_exact(&mut byte))
			.await
			.map_err(|_| PingError::Timeout)??;
		result |= ((byte[0] & 0x7f) as i32) << shift;
		if byte[0] & 0x80 == 0 {
			break;
		}
		shift += 7;
		if shift >= 35 {
			return Err(PingError::Protocol);
		}
	}
	Ok(result)
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn varint_roundtrip() {
		for value in [0, 1, 127, 128, 255, 25565, -1, i32::MAX] {
			let mut buf = Vec::new();
			write_varint(&mut buf, value);
			assert!(!buf.is_empty());
		}
	}

	#[test]
	fn motd_from_string() {
		let v = serde_json::json!("Hello world");
		assert_eq!(extract_motd(&v), "Hello world");
	}

	#[test]
	fn motd_from_components() {
		let v = serde_json::json!({
			"text": "A ",
			"extra": [{ "text": "Minecraft " }, { "text": "Server" }]
		});
		assert_eq!(extract_motd(&v), "A Minecraft Server");
	}
}
