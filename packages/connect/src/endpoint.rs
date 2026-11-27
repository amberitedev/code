//! An endpoint: one UDP socket that dials other programs, and for a listener, takes their
//! connections.

use std::{
    fmt, io,
    net::{IpAddr, SocketAddr},
    sync::Arc,
    time::Duration,
};

use futures::{StreamExt, stream::FuturesUnordered};
use quinn::{
    ClientConfig, EndpointConfig, ServerConfig, TokioRuntime, TransportConfig,
    crypto::rustls::{HandshakeData, QuicClientConfig, QuicServerConfig},
    rustls::pki_types::CertificateDer,
};

use crate::{
    ip::is_public,
    socket::{Gate, Hold, Socket},
    stun,
    tls::{self, Identity, PeerId},
};

/// The most addresses one side offers, and the most the other side tries.
pub const MAX_CANDIDATES: usize = 8;
/// How long a listener keeps sending toward a dialer it was introduced to.
const PUNCH_FOR: Duration = Duration::from_secs(8);
const PUNCH_EVERY: Duration = Duration::from_millis(250);
/// A dialer sends this often on a quiet connection, so the routers on the way keep its path
/// open and a dead one is noticed.
const KEEP_ALIVE: Duration = Duration::from_secs(5);
/// A connection that hears nothing for this long is over.
const IDLE: Duration = Duration::from_secs(20);
/// Streams one connection may have open at once.
const MAX_STREAMS: u32 = 64;

/// One two-way stream of a connection.
pub type Stream = tokio::io::Join<quinn::RecvStream, quinn::SendStream>;

/// No candidate answered in time: there is no direct path, or the other side is off.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NoPath;

impl fmt::Display for NoPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("no address answered")
    }
}

impl std::error::Error for NoPath {}

pub struct Endpoint {
    quinn: quinn::Endpoint,
    socket: Arc<Socket>,
    identity: Identity,
}

impl Endpoint {
    /// An endpoint that only dials. Call it inside a Tokio runtime.
    pub fn dialer(bind: SocketAddr, identity: Identity) -> io::Result<Self> {
        Self::new(Socket::bind(bind, None)?, identity, None)
    }

    /// An endpoint that takes connections speaking one of `alpns`, from the addresses `gate`
    /// lets in. Call it inside a Tokio runtime.
    pub fn listener(
        bind: SocketAddr,
        identity: Identity,
        alpns: &[&[u8]],
        gate: Gate,
    ) -> io::Result<Self> {
        let crypto = QuicServerConfig::try_from(
            tls::listener_config(&identity, alpns).map_err(io::Error::other)?,
        )
        .map_err(io::Error::other)?;
        let mut server = ServerConfig::with_crypto(Arc::new(crypto));
        server.transport_config(transport(false));
        Self::new(Socket::bind(bind, Some(gate))?, identity, Some(server))
    }

    fn new(
        socket: Arc<Socket>,
        identity: Identity,
        server: Option<ServerConfig>,
    ) -> io::Result<Self> {
        let mut config = EndpointConfig::new(Arc::new(identity.reset_key()));
        // The socket tells QUIC from STUN and punches by the bit this would clear at random.
        config.grease_quic_bit(false);
        let quinn = quinn::Endpoint::new_with_abstract_socket(
            config,
            server,
            Arc::clone(&socket) as Arc<dyn quinn::AsyncUdpSocket>,
            Arc::new(TokioRuntime),
        )?;
        Ok(Self {
            quinn,
            socket,
            identity,
        })
    }

    pub fn id(&self) -> PeerId {
        self.identity.id()
    }

    pub fn local_addr(&self) -> io::Result<SocketAddr> {
        self.socket.local_addr()
    }

    /// Where this endpoint might be reached, best first: the router's address with each port in
    /// `mapped` (ports the router forwards to this socket), the router's address with the port
    /// it gave this socket, then this computer's own addresses. An endpoint bound to loopback
    /// has only that address.
    pub async fn candidates(&self, mapped: &[u16]) -> Vec<SocketAddr> {
        let Ok(local) = self.socket.local_addr() else {
            return Vec::new();
        };
        if local.ip().is_loopback() {
            return vec![local];
        }
        let mut found = Vec::new();
        let mut add = |address: SocketAddr| {
            if !found.contains(&address) {
                found.push(address);
            }
        };
        let [first, second] =
            stun::SERVERS.map(|server| Box::pin(self.socket.reflexive(server)));
        // Either server's answer will do.
        let outside = tokio::select! {
            Some(address) = first => Some(address),
            Some(address) = second => Some(address),
            else => None,
        };
        if let Some(outside) = outside {
            for port in mapped {
                add(SocketAddr::new(outside.ip(), *port));
            }
            add(outside);
        }
        if local.ip().is_unspecified() {
            for ip in own_addresses(self.socket.is_v6()) {
                add(SocketAddr::new(ip, local.port()));
            }
        } else {
            add(SocketAddr::new(local.ip().to_canonical(), local.port()));
        }
        found.truncate(MAX_CANDIDATES);
        found
    }

    /// The dialer `id` at `candidates` was introduced: let it in for a while, and send toward
    /// those addresses so the router in front of this endpoint lets their packets in.
    pub fn expect(&self, id: PeerId, candidates: &[SocketAddr]) {
        let candidates: Vec<SocketAddr> =
            candidates.iter().take(MAX_CANDIDATES).copied().collect();
        self.socket
            .admit(id, candidates.iter().map(|candidate| candidate.ip()));
        let socket = Arc::clone(&self.socket);
        tokio::spawn(async move {
            let mut every = tokio::time::interval(PUNCH_EVERY);
            for _ in 0..PUNCH_FOR.as_millis() / PUNCH_EVERY.as_millis() {
                every.tick().await;
                for candidate in &candidates {
                    socket.punch(*candidate);
                }
            }
        });
    }

    /// Connect to `peer` at whichever of `candidates` answers first, speaking `alpn`. Only the
    /// program holding `peer`'s certificate can answer. Each attempt's packets also open this
    /// side's router for the answer.
    pub async fn dial(
        &self,
        peer: PeerId,
        candidates: &[SocketAddr],
        alpn: &[u8],
        within: Duration,
    ) -> Result<Connection, NoPath> {
        let crypto = tls::dialer_config(&self.identity, peer, alpn)
            .ok()
            .and_then(|config| QuicClientConfig::try_from(config).ok())
            .ok_or(NoPath)?;
        let mut config = ClientConfig::new(Arc::new(crypto));
        config.transport_config(transport(true));
        let mut attempts: FuturesUnordered<_> = candidates
            .iter()
            .take(MAX_CANDIDATES)
            // An IPv6 address on an IPv4 socket is refused here.
            .filter_map(|candidate| {
                self.quinn
                    .connect_with(config.clone(), *candidate, tls::SERVER_NAME)
                    .ok()
            })
            .collect();
        let first = async {
            while let Some(attempt) = attempts.next().await {
                if let Ok(connection) = attempt {
                    return Some(connection);
                }
            }
            None
        };
        let quinn = tokio::time::timeout(within, first)
            .await
            .ok()
            .flatten()
            .ok_or(NoPath)?;
        Ok(Connection {
            quinn,
            peer,
            _hold: None,
        })
    }

    /// Tell every connection this endpoint is going, and give the word a moment to leave.
    pub async fn close(&self) {
        self.quinn.close(0u32.into(), b"");
        let _ = tokio::time::timeout(
            Duration::from_millis(250),
            self.quinn.wait_idle(),
        )
        .await;
    }

    /// The next dialer the gate let through. `None` once the endpoint is closed.
    pub async fn accept(&self) -> Option<Incoming> {
        Some(Incoming {
            quinn: self.quinn.accept().await?,
            socket: Arc::clone(&self.socket),
        })
    }
}

/// Every address of this computer's networks that another computer could send to, public ones
/// first: a global IPv6 address may be the only way in, and virtual adapters add many private ones.
fn own_addresses(v6: bool) -> Vec<IpAddr> {
    let mut addresses = Vec::new();
    for interface in netdev::get_interfaces() {
        if !interface.is_up() || interface.is_loopback() {
            continue;
        }
        addresses.extend(
            interface
                .ipv4
                .iter()
                .map(|net| net.addr())
                .filter(|ip| !ip.is_link_local())
                .map(IpAddr::V4),
        );
        if v6 {
            addresses.extend(
                interface
                    .ipv6
                    .iter()
                    .map(|net| net.addr())
                    .filter(|ip| !ip.is_unicast_link_local())
                    .map(IpAddr::V6),
            );
        }
    }
    addresses.sort_by_key(|ip| !is_public(*ip));
    addresses
}

fn transport(keep_alive: bool) -> Arc<TransportConfig> {
    let mut transport = TransportConfig::default();
    transport
        .max_idle_timeout(Some(IDLE.try_into().expect("a short time")))
        .max_concurrent_bidi_streams(MAX_STREAMS.into())
        .max_concurrent_uni_streams(0u32.into())
        .keep_alive_interval(keep_alive.then_some(KEEP_ALIVE));
    Arc::new(transport)
}

/// A dialer the gate let through, before the handshake.
pub struct Incoming {
    quinn: quinn::Incoming,
    socket: Arc<Socket>,
}

impl Incoming {
    /// Turn the dialer away, as when too many connections are open.
    pub fn refuse(self) {
        self.quinn.refuse();
    }

    /// Finish the handshake, and keep the dialer only when it is who was introduced. Its
    /// address stays admitted for as long as the connection is kept.
    pub async fn accept(self) -> io::Result<Connection> {
        let ip = self.quinn.remote_address().ip();
        let hold = self.socket.hold(ip);
        let quinn = self.quinn.await?;
        let peer = quinn
            .peer_identity()
            .and_then(|identity| {
                identity.downcast::<Vec<CertificateDer<'static>>>().ok()
            })
            .and_then(|chain| chain.first().map(|leaf| PeerId::of(leaf)))
            .ok_or_else(|| {
                io::Error::other("the dialer showed no certificate")
            })?;
        if !self.socket.vouches(peer, ip) {
            quinn.close(0u32.into(), b"not introduced");
            return Err(io::Error::other("the dialer was not introduced"));
        }
        Ok(Connection {
            quinn,
            peer,
            _hold: Some(hold),
        })
    }
}

/// A connection to `peer`, open until it is dropped or either side closes it.
pub struct Connection {
    quinn: quinn::Connection,
    peer: PeerId,
    _hold: Option<Hold>,
}

impl Connection {
    /// Who is on the other end, proven by the handshake.
    pub fn peer(&self) -> PeerId {
        self.peer
    }

    /// The address the other end is reached at.
    pub fn remote(&self) -> SocketAddr {
        let remote = self.quinn.remote_address();
        SocketAddr::new(remote.ip().to_canonical(), remote.port())
    }

    /// The protocol both sides agreed on.
    pub fn alpn(&self) -> Option<Vec<u8>> {
        self.quinn
            .handshake_data()?
            .downcast::<HandshakeData>()
            .ok()?
            .protocol
    }

    pub fn is_open(&self) -> bool {
        self.quinn.close_reason().is_none()
    }

    /// Open a stream. The other side sees it once something is written to it.
    pub async fn open(&self) -> io::Result<Stream> {
        let (send, receive) = self.quinn.open_bi().await?;
        Ok(tokio::io::join(receive, send))
    }

    /// The next stream the other side opened. `None` once the connection is over.
    pub async fn accept(&self) -> Option<Stream> {
        let (send, receive) = self.quinn.accept_bi().await.ok()?;
        Some(tokio::io::join(receive, send))
    }
}

/// Whether `ip` is this computer, or on a network this computer is plugged into, so it can be
/// tried with no introduction.
pub fn on_own_network(ip: IpAddr) -> bool {
    ip.is_loopback()
        || netdev::get_interfaces()
            .iter()
            .filter(|interface| interface.is_up())
            .any(|interface| match ip {
                IpAddr::V4(ip) => {
                    interface.ipv4.iter().any(|net| net.contains(&ip))
                }
                IpAddr::V6(ip) => {
                    interface.ipv6.iter().any(|net| net.contains(&ip))
                }
            })
}
