//! The local socket: how the CLI on this machine reaches Core. A Unix socket on Linux and macOS,
//! a named pipe on Windows. The OS tells who connects, so there is no token to steal, and
//! browsers cannot reach it. Only `copal` and the owner recorded at install get in, and a request
//! that arrives this way is the owner's. The same Axum router serves it and HTTPS.
//!
//! [`Listener`] is also what a shim listens on for Core's commands.

use std::{io, path::Path};

use axum::{Extension, Router};
use color_eyre::eyre::Result;
#[cfg(unix)]
use color_eyre::eyre::WrapErr;
use tokio::io::{AsyncRead, AsyncWrite};
use tracing::{debug, warn};

use crate::daemon::paths::Paths;

/// Marks a request that came over the local socket. `access::Caller` reads it as the owner.
#[derive(Debug, Clone, Copy)]
pub struct Local;

/// A connection the local socket or a shim accepted.
pub trait Stream: AsyncRead + AsyncWrite + Unpin + Send + 'static {}
impl<T: AsyncRead + AsyncWrite + Unpin + Send + 'static> Stream for T {}

/// Serve `router` on the local socket until the process ends. `owner` is the OS id the install
/// recorded: a uid on Linux and macOS, a SID on Windows.
pub async fn serve(paths: &Paths, owner: &str, router: Router) -> Result<()> {
    let router = router.layer(Extension(Local));
    let path = paths.local_socket();
    #[cfg(unix)]
    let mut listener = {
        let owner: u32 =
            owner.parse().wrap_err("the recorded owner is not a uid")?;
        // Anyone may connect; `accept` drops everyone but `copal` and the owner.
        let listener = Listener::bind(&path, 0o666)?;
        let me = nix::unistd::geteuid().as_raw();
        listener.allow(move |uid| peer_allowed(uid, me, owner))
    };
    #[cfg(windows)]
    let mut listener = {
        let me = crate::daemon::win::current_sid()?;
        Listener::bind(&path, &format!("D:P(A;;GA;;;{me})(A;;GRGW;;;{owner})"))?
    };
    debug!(path = %path.display(), "local socket ready");
    loop {
        match listener.accept().await {
            Ok(stream) => {
                let router = router.clone();
                tokio::spawn(async move {
                    if let Err(error) =
                        crate::api::serve_connection(stream, router).await
                    {
                        debug!(%error, "local connection closed");
                    }
                });
            }
            Err(error) => {
                warn!(%error, "local socket");
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            }
        }
    }
}

/// Whether a Unix peer may use the local socket: `copal` itself, or the owner.
#[cfg_attr(windows, allow(dead_code))]
fn peer_allowed(peer: u32, core: u32, owner: u32) -> bool {
    peer == core || peer == owner
}

/// Connect to Core's local socket, for the CLI.
pub async fn connect(paths: &Paths) -> io::Result<Box<dyn Stream>> {
    connect_to(&paths.local_socket()).await
}

#[cfg(unix)]
pub async fn connect_to(path: &Path) -> io::Result<Box<dyn Stream>> {
    Ok(Box::new(tokio::net::UnixStream::connect(path).await?))
}

#[cfg(windows)]
pub async fn connect_to(path: &Path) -> io::Result<Box<dyn Stream>> {
    use tokio::net::windows::named_pipe::ClientOptions;
    use windows_sys::Win32::Foundation::ERROR_PIPE_BUSY;

    // Every instance is busy only for the moment the server takes to make the next one.
    for _ in 0..40 {
        match ClientOptions::new().open(path) {
            Err(error)
                if error.raw_os_error() == Some(ERROR_PIPE_BUSY as i32) =>
            {
                tokio::time::sleep(std::time::Duration::from_millis(25)).await;
            }
            result => return Ok(Box::new(result?)),
        }
    }
    Err(io::Error::from(io::ErrorKind::TimedOut))
}

/// A Unix socket listener. The folder it is in, or its mode, decides who reaches it.
#[cfg(unix)]
pub struct Listener {
    listener: tokio::net::UnixListener,
    allow: Option<Box<dyn Fn(u32) -> bool + Send>>,
}

#[cfg(unix)]
impl Listener {
    /// Listen at `path`, replacing a socket left by an earlier process. The caller holds the
    /// lock that makes it the only one listening there.
    pub fn bind(path: &Path, mode: u32) -> io::Result<Self> {
        use std::os::unix::fs::PermissionsExt;
        match std::fs::remove_file(path) {
            Err(error) if error.kind() != io::ErrorKind::NotFound => {
                return Err(error);
            }
            _ => {}
        }
        let listener = tokio::net::UnixListener::bind(path)?;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode))?;
        Ok(Self {
            listener,
            allow: None,
        })
    }

    /// Drop connections from uids `allow` refuses.
    pub fn allow(self, allow: impl Fn(u32) -> bool + Send + 'static) -> Self {
        Self {
            allow: Some(Box::new(allow)),
            ..self
        }
    }

    pub async fn accept(&mut self) -> io::Result<Box<dyn Stream>> {
        loop {
            let (stream, _) = self.listener.accept().await?;
            let uid = stream.peer_cred()?.uid();
            if self.allow.as_ref().is_none_or(|allow| allow(uid)) {
                return Ok(Box::new(stream));
            }
            debug!(uid, "refused a local connection");
        }
    }
}

/// A named pipe listener. Its DACL decides who may open it; remote clients never can.
#[cfg(windows)]
pub struct Listener {
    name: std::path::PathBuf,
    security: crate::daemon::win::SecurityDescriptor,
    next: tokio::net::windows::named_pipe::NamedPipeServer,
}

#[cfg(windows)]
impl Listener {
    /// Create the pipe with the permissions in `sddl`. Being its first instance proves no
    /// other process squats the name.
    pub fn bind(name: &Path, sddl: &str) -> io::Result<Self> {
        let security = crate::daemon::win::SecurityDescriptor::from_sddl(sddl)?;
        let next = Self::instance(name, &security, true)?;
        Ok(Self {
            name: name.to_owned(),
            security,
            next,
        })
    }

    fn instance(
        name: &Path,
        security: &crate::daemon::win::SecurityDescriptor,
        first: bool,
    ) -> io::Result<tokio::net::windows::named_pipe::NamedPipeServer> {
        let mut attributes = security.attributes();
        // SAFETY: the attributes point at a descriptor that outlives the call.
        unsafe {
            tokio::net::windows::named_pipe::ServerOptions::new()
                .first_pipe_instance(first)
                .reject_remote_clients(true)
                .create_with_security_attributes_raw(
                    name,
                    (&raw mut attributes).cast(),
                )
        }
    }

    pub async fn accept(&mut self) -> io::Result<Box<dyn Stream>> {
        self.next.connect().await?;
        let next = Self::instance(&self.name, &self.security, false)?;
        Ok(Box::new(std::mem::replace(&mut self.next, next)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_core_and_the_owner_may_use_the_socket() {
        assert!(peer_allowed(990, 990, 1000));
        assert!(peer_allowed(1000, 990, 1000));
        // copal-servers, or any other account on the machine.
        assert!(!peer_allowed(991, 990, 1000));
        assert!(!peer_allowed(0, 990, 1000));
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn the_socket_drops_a_refused_peer() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.sock");
        let me = nix::unistd::geteuid().as_raw();
        let mut listener = Listener::bind(&path, 0o666)
            .unwrap()
            .allow(move |uid| uid != me);
        let _client = connect_to(&path).await.unwrap();
        let accepted = tokio::time::timeout(
            std::time::Duration::from_millis(200),
            listener.accept(),
        )
        .await;
        assert!(accepted.is_err(), "a refused peer must never be accepted");
    }

    #[cfg(windows)]
    #[tokio::test]
    async fn the_pipe_refuses_an_account_its_dacl_leaves_out() {
        let name = std::path::PathBuf::from(format!(
            r"\\.\pipe\copal-test-{}",
            uuid::Uuid::new_v4()
        ));
        // Only SYSTEM may open it; the test runs as someone else.
        let _listener = Listener::bind(&name, "D:P(A;;GA;;;SY)").unwrap();
        let error = connect_to(&name).await.err().unwrap();
        assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);
    }
}
