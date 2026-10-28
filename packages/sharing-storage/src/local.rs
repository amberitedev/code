//! The local API between Copal and its store, over the store's stdin and stdout, one JSON object
//! a line. Only Copal, the parent, holds the pipes, so nothing else can call it and no token is
//! needed.
//!
//! - The store's first line says where its HTTP listener is: `{"listening": <port>}`.
//! - Copal calls with `{"id": n, "call": "status"}`, `{"id": n, "call": "drop", "roots": [R…]}`,
//!   or `{"id": n, "call": "config", "cap": bytes, "egress_cap": bytes}`.
//! - The store answers each with `{"id": n, "ok": …}` ([`Status`] for `status`, `null` for the
//!   others) or `{"id": n, "error": "…"}`.
//!
//! Copal's side is [`Client`]; the store's is [`serve`].

use std::{io, sync::Arc, time::Duration};

use serde::{Deserialize, Serialize, de::DeserializeOwned};
use tokio::io::{
    AsyncBufReadExt, AsyncRead, AsyncWrite, AsyncWriteExt, BufReader, Lines,
};
use tracing::warn;
use ts_rs::TS;

use crate::{Hash, Store};

/// How long a call may take before Copal gives up on it.
const CALL_TIMEOUT: Duration = Duration::from_secs(10);

/// The store's caps, in bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Caps {
    /// The most it may hold.
    pub cap: u64,
    /// The most it may serve a day.
    pub egress_cap: u64,
}

/// What the store holds and has room for, in bytes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Status {
    pub cap: u64,
    /// What the stored, pending, and dropped lists name, counting each object once.
    pub used: u64,
    /// Free on the store's disk.
    pub free: u64,
    pub egress_cap: u64,
    /// Served today, UTC.
    pub egress_today: u64,
    /// Every version the store holds whole.
    pub versions: Vec<HeldVersion>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct HeldVersion {
    /// R, the version's root hash.
    pub root: String,
    /// The bytes of its objects.
    pub bytes: u64,
}

#[derive(Serialize, Deserialize)]
struct Listening {
    listening: u16,
}

#[derive(Serialize, Deserialize)]
struct Request {
    id: u64,
    #[serde(flatten)]
    call: Call,
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "call", rename_all = "snake_case")]
enum Call {
    Status,
    Drop { roots: Vec<String> },
    Config(Caps),
}

/// Just the id of a call the store could not read, so it can still answer it.
#[derive(Deserialize)]
struct Id {
    id: u64,
}

#[derive(Serialize, Deserialize)]
struct Reply {
    id: u64,
    #[serde(flatten)]
    outcome: Outcome,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum Outcome {
    Ok(serde_json::Value),
    Error(String),
}

/// Copal's end of the pipes to one running store. Calls go one at a time.
pub struct Client<W, R> {
    pipes: tokio::sync::Mutex<Pipes<W, R>>,
}

struct Pipes<W, R> {
    /// `None` once a call could not be written whole: anything sent after it would be garbled.
    stdin: Option<W>,
    stdout: Lines<BufReader<R>>,
    last_id: u64,
}

impl<W, R> Client<W, R>
where
    W: AsyncWrite + Unpin + Send,
    R: AsyncRead + Unpin + Send,
{
    /// Read the store's first line, and keep the pipes for calls. Returns the store's port.
    pub async fn connect(stdin: W, stdout: R) -> io::Result<(Self, u16)> {
        let mut stdout = BufReader::new(stdout).lines();
        let line = stdout.next_line().await?.ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "the store stopped before it said where it listens",
            )
        })?;
        let Listening { listening } =
            serde_json::from_str(&line).map_err(|_| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!(
                        "the store said {line:?} instead of where it listens"
                    ),
                )
            })?;
        let pipes = Pipes {
            stdin: Some(stdin),
            stdout,
            last_id: 0,
        };
        Ok((
            Self {
                pipes: tokio::sync::Mutex::new(pipes),
            },
            listening,
        ))
    }

    pub async fn status(&self) -> Result<Status, String> {
        self.call(Call::Status).await
    }

    /// Stop holding `roots`. Fetch tickets already out keep working through the grace period.
    pub async fn drop_versions(
        &self,
        roots: Vec<String>,
    ) -> Result<(), String> {
        self.call(Call::Drop { roots }).await
    }

    pub async fn config(&self, caps: Caps) -> Result<(), String> {
        self.call(Call::Config(caps)).await
    }

    /// Send `call` and wait for its answer. The error is a description.
    async fn call<T: DeserializeOwned>(&self, call: Call) -> Result<T, String> {
        let mut pipes = self.pipes.lock().await;
        let pipes = &mut *pipes;
        pipes.last_id += 1;
        let id = pipes.last_id;
        let stdin = pipes
            .stdin
            .as_mut()
            .ok_or("the store stopped taking calls")?;
        let mut line = serde_json::to_string(&Request { id, call })
            .expect("calls serialize");
        line.push('\n');
        let written = tokio::time::timeout(CALL_TIMEOUT, async {
            stdin.write_all(line.as_bytes()).await?;
            stdin.flush().await
        })
        .await;
        if !matches!(written, Ok(Ok(()))) {
            // Closing stdin asks the store to stop; Copal starts it again.
            pipes.stdin = None;
            return Err("the store did not take the call".into());
        }
        let outcome = tokio::time::timeout(CALL_TIMEOUT, async {
            loop {
                let line = pipes
                    .stdout
                    .next_line()
                    .await
                    .map_err(|error| error.to_string())?
                    .ok_or("the store stopped")?;
                let reply: Reply = serde_json::from_str(&line)
                    .map_err(|_| format!("the store answered {line:?}"))?;
                // An answer to a call that timed out arrives late; it is skipped here.
                if reply.id == id {
                    return Ok::<_, String>(reply.outcome);
                }
            }
        })
        .await
        .map_err(|_| "the store did not answer in time".to_string())??;
        match outcome {
            Outcome::Ok(value) => {
                serde_json::from_value(value).map_err(|error| error.to_string())
            }
            Outcome::Error(error) => Err(error),
        }
    }
}

/// Say where the store listens, then answer Copal's calls in order until stdin closes.
pub(crate) async fn serve(
    store: Arc<Store>,
    port: u16,
    stdin: impl AsyncRead + Unpin,
    mut stdout: impl AsyncWrite + Unpin,
) -> io::Result<()> {
    write_line(&mut stdout, &Listening { listening: port }).await?;
    let mut lines = BufReader::new(stdin).lines();
    while let Some(line) = lines.next_line().await? {
        let reply = match serde_json::from_str::<Request>(&line) {
            Ok(Request { id, call }) => Reply {
                id,
                outcome: answer(&store, call).await,
            },
            Err(error) => {
                warn!(%error, "Copal sent a call the store cannot read");
                let Ok(Id { id }) = serde_json::from_str(&line) else {
                    continue;
                };
                Reply {
                    id,
                    outcome: Outcome::Error(format!(
                        "the store cannot read that call: {error}"
                    )),
                }
            }
        };
        write_line(&mut stdout, &reply).await?;
    }
    Ok(())
}

async fn answer(store: &Store, call: Call) -> Outcome {
    let done = match call {
        Call::Status => store.status().map(|status| {
            serde_json::to_value(status).expect("a status serializes")
        }),
        Call::Drop { roots } => {
            let roots: Vec<_> =
                roots.iter().filter_map(|root| Hash::parse(root)).collect();
            store
                .drop_versions(&roots)
                .await
                .map(|()| serde_json::Value::Null)
        }
        Call::Config(caps) => {
            store.set_caps(caps);
            Ok(serde_json::Value::Null)
        }
    };
    match done {
        Ok(value) => Outcome::Ok(value),
        Err(error) => {
            warn!(%error, "a call from Copal failed");
            Outcome::Error(error.to_string())
        }
    }
}

async fn write_line(
    out: &mut (impl AsyncWrite + Unpin),
    value: &impl Serialize,
) -> io::Result<()> {
    let mut line = serde_json::to_vec(value).expect("answers serialize");
    line.push(b'\n');
    out.write_all(&line).await?;
    out.flush().await
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::{objects::list_of, routes::tests::fixture};

    /// A client talking to `store` over in-memory pipes, as Copal does to `copal store`.
    async fn connect(
        store: Arc<Store>,
    ) -> (
        Client<tokio::io::DuplexStream, tokio::io::DuplexStream>,
        tokio::task::JoinHandle<io::Result<()>>,
    ) {
        let (core_stdin, store_stdin) = tokio::io::duplex(4096);
        let (store_stdout, core_stdout) = tokio::io::duplex(4096);
        let served =
            tokio::spawn(serve(store, 4242, store_stdin, store_stdout));
        let (client, port) =
            Client::connect(core_stdin, core_stdout).await.unwrap();
        assert_eq!(port, 4242);
        (client, served)
    }

    #[tokio::test]
    async fn status_config_and_drop_go_over_the_pipes() {
        let f = fixture("core", 1 << 30, 1 << 30).await;
        let list = list_of(&[b"one", b"two"]);
        let (_, answer) = f.upload(&list, &[b"one", b"two"], 100).await;
        assert_eq!(answer, json!({ "complete": true }));
        let (client, _) = connect(Arc::clone(&f.store)).await;

        let status = client.status().await.unwrap();
        assert_eq!(status.used, 6);
        assert_eq!(
            status.versions,
            [HeldVersion {
                root: list.root().to_string(),
                bytes: 6
            }]
        );
        client
            .config(Caps {
                cap: 5,
                egress_cap: 7,
            })
            .await
            .unwrap();
        client
            .drop_versions(vec![list.root().to_string(), "nonsense".into()])
            .await
            .unwrap();
        let status = client.status().await.unwrap();
        assert_eq!((status.cap, status.egress_cap), (5, 7));
        assert!(status.versions.is_empty());
    }

    #[tokio::test]
    async fn an_unreadable_call_is_answered_and_closing_stdin_stops_the_store()
    {
        let f = fixture("core", 1 << 30, 1 << 30).await;
        let (mut core_stdin, store_stdin) = tokio::io::duplex(4096);
        let (store_stdout, core_stdout) = tokio::io::duplex(4096);
        let served = tokio::spawn(serve(
            Arc::clone(&f.store),
            1,
            store_stdin,
            store_stdout,
        ));
        let mut lines = BufReader::new(core_stdout).lines();
        assert_eq!(
            lines.next_line().await.unwrap().unwrap(),
            r#"{"listening":1}"#
        );
        core_stdin
            .write_all(b"{\"id\":3,\"call\":\"explode\"}\n")
            .await
            .unwrap();
        let reply: serde_json::Value =
            serde_json::from_str(&lines.next_line().await.unwrap().unwrap())
                .unwrap();
        assert_eq!(reply["id"], 3);
        assert!(reply["error"].is_string(), "{reply}");
        drop(core_stdin);
        served.await.unwrap().unwrap();
    }

    #[tokio::test]
    async fn calls_to_a_store_that_stopped_fail() {
        let f = fixture("core", 1 << 30, 1 << 30).await;
        let (client, served) = connect(Arc::clone(&f.store)).await;
        served.abort();
        let _ = served.await;
        assert!(client.status().await.is_err());
    }
}
