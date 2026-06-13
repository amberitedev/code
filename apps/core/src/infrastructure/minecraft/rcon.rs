//! Minecraft Source RCON protocol client.
//!
//! Implements the Valve Source RCON wire protocol manually over a tokio
//! `TcpStream` (no external crate). A packet is framed as:
//!
//! ```text
//! int32 length   (LE) — number of bytes that follow this field
//! int32 id       (LE) — client-chosen request id, echoed in the response
//! int32 type     (LE) — SERVERDATA_* message type
//! body           — null-terminated ASCII/UTF-8 string
//! u8 0x00        — terminating null pad for the packet
//! ```
//!
//! Type values:
//! - `3` SERVERDATA_AUTH (client → server, login)
//! - `2` SERVERDATA_AUTH_RESPONSE / SERVERDATA_EXECCOMMAND (overloaded)
//! - `0` SERVERDATA_RESPONSE_VALUE (server → client, command output)
//!
//! Auth succeeds when the server replies with an `AUTH_RESPONSE` whose id
//! matches the request id. A reply id of `-1` means authentication failed.

use std::time::Duration;

use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
    time::timeout,
};

const TYPE_AUTH: i32 = 3;
const TYPE_AUTH_RESPONSE: i32 = 2;
const TYPE_EXECCOMMAND: i32 = 2;
const TYPE_RESPONSE_VALUE: i32 = 0;

const AUTH_FAILED_ID: i32 = -1;

/// Maximum packet body we will read, guarding against a hostile or corrupt
/// length prefix. 4 KiB header slack over the documented 4096-byte cap.
const MAX_PACKET_LEN: i32 = 8192;

const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
const IO_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Debug, thiserror::Error)]
pub enum RconError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("connection timed out")]
    Timeout,
    #[error("authentication failed: wrong rcon password")]
    AuthFailed,
    #[error("malformed rcon packet from server")]
    Protocol,
}

/// A connected, authenticated RCON session.
pub struct RconClient {
    stream: TcpStream,
    next_id: i32,
}

impl RconClient {
    /// Connect to `host:port`, authenticate with `password`, and return a
    /// ready-to-use session. Fails fast on a wrong password.
    pub async fn connect(
        host: &str,
        port: u16,
        password: &str,
    ) -> Result<Self, RconError> {
        let stream = timeout(CONNECT_TIMEOUT, TcpStream::connect((host, port)))
            .await
            .map_err(|_| RconError::Timeout)??;
        stream.set_nodelay(true).ok();

        let mut client = Self { stream, next_id: 1 };
        client.authenticate(password).await?;
        Ok(client)
    }

    async fn authenticate(&mut self, password: &str) -> Result<(), RconError> {
        let req_id = self.take_id();
        self.write_packet(req_id, TYPE_AUTH, password).await?;

        // Some servers emit an empty RESPONSE_VALUE before the AUTH_RESPONSE.
        // Loop until we see the AUTH_RESPONSE (or a definitive failure).
        loop {
            let (id, ty, _) = self.read_packet().await?;
            if ty == TYPE_AUTH_RESPONSE || ty == TYPE_RESPONSE_VALUE {
                if id == AUTH_FAILED_ID {
                    return Err(RconError::AuthFailed);
                }
                if ty == TYPE_AUTH_RESPONSE {
                    if id == req_id {
                        return Ok(());
                    }
                    return Err(RconError::AuthFailed);
                }
                // RESPONSE_VALUE pre-amble: keep reading.
            }
        }
    }

    /// Execute a single command and return the server's response body.
    pub async fn exec(&mut self, command: &str) -> Result<String, RconError> {
        let req_id = self.take_id();
        self.write_packet(req_id, TYPE_EXECCOMMAND, command).await?;

        let (_, _, body) = self.read_packet().await?;
        Ok(body)
    }

    fn take_id(&mut self) -> i32 {
        let id = self.next_id;
        self.next_id = self.next_id.wrapping_add(1).max(1);
        id
    }

    async fn write_packet(
        &mut self,
        id: i32,
        ty: i32,
        body: &str,
    ) -> Result<(), RconError> {
        let body_bytes = body.as_bytes();
        // length = id(4) + type(4) + body + null(1) + pad(1)
        let length = (4 + 4 + body_bytes.len() + 2) as i32;

        let mut buf = Vec::with_capacity(length as usize + 4);
        buf.extend_from_slice(&length.to_le_bytes());
        buf.extend_from_slice(&id.to_le_bytes());
        buf.extend_from_slice(&ty.to_le_bytes());
        buf.extend_from_slice(body_bytes);
        buf.push(0);
        buf.push(0);

        timeout(IO_TIMEOUT, self.stream.write_all(&buf))
            .await
            .map_err(|_| RconError::Timeout)??;
        Ok(())
    }

    /// Read one packet, returning `(id, type, body)` with both trailing nulls
    /// stripped from the body.
    async fn read_packet(&mut self) -> Result<(i32, i32, String), RconError> {
        let length = self.read_i32().await?;
        if !(10..=MAX_PACKET_LEN).contains(&length) {
            return Err(RconError::Protocol);
        }

        let mut payload = vec![0u8; length as usize];
        timeout(IO_TIMEOUT, self.stream.read_exact(&mut payload))
            .await
            .map_err(|_| RconError::Timeout)??;

        let id = i32::from_le_bytes(
            payload[0..4].try_into().map_err(|_| RconError::Protocol)?,
        );
        let ty = i32::from_le_bytes(
            payload[4..8].try_into().map_err(|_| RconError::Protocol)?,
        );

        // Body is everything after the two int32 headers, minus trailing nulls.
        let body_bytes = &payload[8..];
        let body_bytes = body_bytes
            .iter()
            .rposition(|&b| b != 0)
            .map(|end| &body_bytes[..=end])
            .unwrap_or(&[]);
        let body = String::from_utf8_lossy(body_bytes).into_owned();

        Ok((id, ty, body))
    }

    async fn read_i32(&mut self) -> Result<i32, RconError> {
        let mut buf = [0u8; 4];
        timeout(IO_TIMEOUT, self.stream.read_exact(&mut buf))
            .await
            .map_err(|_| RconError::Timeout)??;
        Ok(i32::from_le_bytes(buf))
    }
}
