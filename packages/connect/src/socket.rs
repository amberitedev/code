//! The one UDP socket of an endpoint, under QUIC. It keeps what QUIC must not see away from it:
//! packets from addresses nobody introduced, STUN answers, and the other side's punches.

use std::{
    collections::HashMap,
    fmt,
    io::{self, IoSliceMut},
    net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr},
    pin::Pin,
    sync::{Arc, Mutex},
    task::{Context, Poll, ready},
    time::{Duration, Instant},
};

use quinn::{
    AsyncUdpSocket, Runtime, TokioRuntime, UdpPoller,
    udp::{RecvMeta, Transmit},
};
use tokio::sync::oneshot;

use crate::{ip::is_private, stun, tls::PeerId};

/// How long an introduced address may take to connect.
const ADMITTED_FOR: Duration = Duration::from_secs(60);
/// How long a STUN server may take to answer.
const STUN_TIMEOUT: Duration = Duration::from_millis(1500);
/// What a punch carries. Any packet opens the router; this one reads as neither QUIC nor STUN.
const PUNCH: [u8; 1] = [0];
/// Receives that may drop everything before the task yields, so a flood of refused packets
/// cannot hold the runtime.
const DROPS_PER_POLL: usize = 32;

/// Who may send to a listener: the addresses the backend introduced, for a while, and those with
/// a connection open.
#[derive(Default)]
struct Admitted {
    peers: HashMap<IpAddr, Admission>,
    /// The dialers the backend introduced, until when they may connect.
    ids: HashMap<PeerId, Instant>,
    /// Private addresses get in with no introduction.
    local: bool,
}

struct Admission {
    until: Instant,
    /// Open connections from this address.
    held: usize,
}

impl Admitted {
    fn admits(&self, ip: IpAddr) -> bool {
        (self.local && is_private(ip))
            || self.peers.get(&ip).is_some_and(|admission| {
                admission.held > 0 || admission.until > Instant::now()
            })
    }
}

pub struct Socket {
    inner: Arc<dyn AsyncUdpSocket>,
    v6: bool,
    /// `None` on a socket that only dials: it has no listener to keep strangers from.
    admitted: Option<Mutex<Admitted>>,
    asked: Mutex<HashMap<stun::Transaction, oneshot::Sender<SocketAddr>>>,
}

/// Keeps an address admitted while its connection is open.
pub struct Hold {
    socket: Arc<Socket>,
    ip: IpAddr,
}

impl Drop for Hold {
    fn drop(&mut self) {
        if let Some(admitted) = &self.socket.admitted
            && let Some(admission) = admitted
                .lock()
                .expect("not poisoned")
                .peers
                .get_mut(&self.ip)
        {
            admission.held -= 1;
        }
    }
}

/// How a listener treats addresses nobody introduced.
#[derive(Clone, Copy)]
pub enum Gate {
    /// Only introduced addresses get in.
    Introduced,
    /// Private addresses (this computer and networks like it) get in too, so a Core is reached
    /// at home with the internet down.
    IntroducedOrLocal,
}

impl Socket {
    /// Bind `address`. Every IPv4 address also means every IPv6 one, on one socket, where the
    /// machine has IPv6. `gate` makes it a listener's socket.
    pub fn bind(
        address: SocketAddr,
        gate: Option<Gate>,
    ) -> io::Result<Arc<Self>> {
        let socket = if address.ip() == IpAddr::V4(Ipv4Addr::UNSPECIFIED) {
            dual_stack(address.port())
                .or_else(|_| std::net::UdpSocket::bind(address))?
        } else {
            std::net::UdpSocket::bind(address)?
        };
        let v6 = socket.local_addr()?.is_ipv6();
        Ok(Arc::new(Self {
            inner: TokioRuntime.wrap_udp_socket(socket)?,
            v6,
            admitted: gate.map(|gate| {
                Mutex::new(Admitted {
                    local: matches!(gate, Gate::IntroducedOrLocal),
                    ..Admitted::default()
                })
            }),
            asked: Mutex::default(),
        }))
    }

    pub fn local_addr(&self) -> io::Result<SocketAddr> {
        self.inner.local_addr()
    }

    pub fn is_v6(&self) -> bool {
        self.v6
    }

    /// Let the dialer `id` in from `ips` for [`ADMITTED_FOR`].
    pub fn admit(&self, id: PeerId, ips: impl IntoIterator<Item = IpAddr>) {
        let Some(admitted) = &self.admitted else {
            return;
        };
        let mut admitted = admitted.lock().expect("not poisoned");
        let now = Instant::now();
        let until = now + ADMITTED_FOR;
        admitted
            .peers
            .retain(|_, admission| admission.held > 0 || admission.until > now);
        admitted.ids.retain(|_, until| *until > now);
        admitted.ids.insert(id, until);
        for ip in ips {
            admitted
                .peers
                .entry(ip.to_canonical())
                .or_insert(Admission { until, held: 0 })
                .until = until;
        }
    }

    /// Whether the dialer `id`, at `ip`, may connect: it was introduced, or the gate lets `ip`
    /// in with no introduction. Anyone else at an introduced address, as behind the same
    /// carrier's router, is not.
    pub fn vouches(&self, id: PeerId, ip: IpAddr) -> bool {
        self.admitted.as_ref().is_none_or(|admitted| {
            let admitted = admitted.lock().expect("not poisoned");
            (admitted.local && is_private(ip.to_canonical()))
                || admitted
                    .ids
                    .get(&id)
                    .is_some_and(|until| *until > Instant::now())
        })
    }

    /// Keep `ip` admitted until the hold drops.
    pub fn hold(self: &Arc<Self>, ip: IpAddr) -> Hold {
        let ip = ip.to_canonical();
        if let Some(admitted) = &self.admitted {
            admitted
                .lock()
                .expect("not poisoned")
                .peers
                .entry(ip)
                .or_insert(Admission {
                    until: Instant::now(),
                    held: 0,
                })
                .held += 1;
        }
        Hold {
            socket: Arc::clone(self),
            ip,
        }
    }

    /// Send one packet toward `to`, so the router in front of this socket lets `to` answer.
    pub fn punch(&self, to: SocketAddr) {
        self.send(to, &PUNCH);
    }

    /// The address a STUN server sees this socket's packets come from: the router's, with the
    /// port it gave this socket. `None` when the server does not answer in time.
    pub async fn reflexive(&self, server: &str) -> Option<SocketAddr> {
        tokio::time::timeout(STUN_TIMEOUT, self.ask(server))
            .await
            .ok()
            .flatten()
    }

    async fn ask(&self, server: &str) -> Option<SocketAddr> {
        let server = tokio::net::lookup_host(server)
            .await
            .ok()?
            .find(SocketAddr::is_ipv4)?;
        let id = stun::transaction();
        let (answer, mut answered) = oneshot::channel();
        self.asked.lock().expect("not poisoned").insert(id, answer);
        // Also when the caller stops waiting, as for the server that lost the race.
        let _forget = Forget {
            asked: &self.asked,
            id,
        };
        // UDP: ask again while there is no answer.
        let mut again = tokio::time::interval(STUN_TIMEOUT / 3);
        loop {
            tokio::select! {
                _ = again.tick() => self.send(server, &stun::request(&id)),
                address = &mut answered => return address.ok(),
            }
        }
    }

    /// One packet outside QUIC. Lost when the socket is busy; every caller sends again.
    fn send(&self, to: SocketAddr, contents: &[u8]) {
        let destination = match to {
            SocketAddr::V4(v4) if self.v6 => {
                SocketAddr::new(v4.ip().to_ipv6_mapped().into(), v4.port())
            }
            to => to,
        };
        let _ = self.inner.try_send(&Transmit {
            destination,
            ecn: None,
            contents,
            segment_size: None,
            src_ip: None,
        });
    }

    /// Whether QUIC gets a packet. QUIC's first byte has one of its top two bits set and STUN's
    /// has neither (RFC 7983), which a punch follows. It holds only while no endpoint greases
    /// QUIC's fixed bit (see `Endpoint::new`).
    fn for_quic(&self, packet: &[u8], from: SocketAddr) -> bool {
        if packet.first().is_some_and(|first| first & 0xC0 != 0) {
            return self.admitted.as_ref().is_none_or(|admitted| {
                admitted
                    .lock()
                    .expect("not poisoned")
                    .admits(from.ip().to_canonical())
            });
        }
        if let Some((id, address)) = stun::mapped_address(packet)
            && let Some(answer) =
                self.asked.lock().expect("not poisoned").remove(&id)
        {
            let _ = answer.send(address);
        }
        false
    }
}

struct Forget<'a> {
    asked: &'a Mutex<HashMap<stun::Transaction, oneshot::Sender<SocketAddr>>>,
    id: stun::Transaction,
}

impl Drop for Forget<'_> {
    fn drop(&mut self) {
        self.asked.lock().expect("not poisoned").remove(&self.id);
    }
}

/// One socket on every IPv6 and IPv4 address.
fn dual_stack(port: u16) -> io::Result<std::net::UdpSocket> {
    let socket = socket2::Socket::new(
        socket2::Domain::IPV6,
        socket2::Type::DGRAM,
        Some(socket2::Protocol::UDP),
    )?;
    socket.set_only_v6(false)?;
    socket.bind(&SocketAddr::new(Ipv6Addr::UNSPECIFIED.into(), port).into())?;
    Ok(socket.into())
}

impl fmt::Debug for Socket {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Socket")
            .field("inner", &self.inner)
            .finish()
    }
}

impl AsyncUdpSocket for Socket {
    fn create_io_poller(self: Arc<Self>) -> Pin<Box<dyn UdpPoller>> {
        Arc::clone(&self.inner).create_io_poller()
    }

    fn try_send(&self, transmit: &Transmit) -> io::Result<()> {
        self.inner.try_send(transmit)
    }

    /// The inner socket's packets with those QUIC must not see taken out.
    fn poll_recv(
        &self,
        cx: &mut Context,
        bufs: &mut [IoSliceMut<'_>],
        meta: &mut [RecvMeta],
    ) -> Poll<io::Result<usize>> {
        for _ in 0..DROPS_PER_POLL {
            let received = ready!(self.inner.poll_recv(cx, bufs, meta))?;
            let mut kept = 0;
            for at in 0..received {
                let len = meta[at].len;
                if !self.for_quic(&bufs[at][..len], meta[at].addr) {
                    continue;
                }
                if kept != at {
                    let (before, rest) = bufs.split_at_mut(at);
                    before[kept][..len].copy_from_slice(&rest[0][..len]);
                    meta[kept] = meta[at];
                }
                kept += 1;
            }
            if kept > 0 {
                return Poll::Ready(Ok(kept));
            }
        }
        cx.waker().wake_by_ref();
        Poll::Pending
    }

    fn local_addr(&self) -> io::Result<SocketAddr> {
        self.inner.local_addr()
    }

    fn max_transmit_segments(&self) -> usize {
        self.inner.max_transmit_segments()
    }

    fn max_receive_segments(&self) -> usize {
        self.inner.max_receive_segments()
    }

    fn may_fragment(&self) -> bool {
        self.inner.may_fragment()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tls::Identity;

    #[tokio::test]
    async fn quic_reaches_a_listener_only_from_addresses_let_in() {
        let quic = [0xC0];
        let stranger: SocketAddr =
            (Ipv4Addr::new(203, 0, 113, 9), 40000).into();
        let home: SocketAddr = (Ipv4Addr::new(192, 168, 1, 5), 40000).into();
        let bind = (Ipv4Addr::LOCALHOST, 0).into();

        let introduced = Socket::bind(bind, Some(Gate::Introduced)).unwrap();
        assert!(!introduced.for_quic(&quic, stranger));
        assert!(!introduced.for_quic(&quic, home));
        introduced.admit(Identity::generate().unwrap().id(), [stranger.ip()]);
        assert!(introduced.for_quic(&quic, stranger));

        let local = Socket::bind(bind, Some(Gate::IntroducedOrLocal)).unwrap();
        assert!(local.for_quic(&quic, home));
        assert!(!local.for_quic(&quic, stranger));
    }
}
