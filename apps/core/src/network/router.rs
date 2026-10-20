//! Asking the home router to open ports, over PCP, NAT-PMP and UPnP.
//!
//! PCP and NAT-PMP are tried first because they are simpler and report clear errors; UPnP is the
//! most common and is tried last. Every mapping is TCP and has a finite lease, so one that is never
//! removed (a crash) still goes away by itself.

use std::{
    fmt::{self, Write as _},
    net::{IpAddr, Ipv4Addr, SocketAddr},
    num::NonZeroU16,
    time::Duration,
};

use crab_nat::{
    GatewayAddress, InternetProtocol, PortMappingOptions, TimeoutConfig, natpmp,
};
use igd_next::{
    AddPortError, PortMappingProtocol, SearchOptions,
    aio::Gateway,
    aio::tokio::{Tokio, search_gateway},
};

/// How long the router keeps a mapping without hearing from Core again.
const LEASE_SECONDS: u32 = 3600;
/// Shown in the router's own list of forwarded ports.
const DESCRIPTION: &str = "Amberite Core";
/// PCP and NAT-PMP are UDP: about four seconds of retries before giving up.
const PMP_TIMEOUT: TimeoutConfig = TimeoutConfig {
    initial_timeout: Duration::from_millis(250),
    max_retries: 3,
    max_retry_timeout: None,
};
const UPNP_SEARCH_TIMEOUT: Duration = Duration::from_secs(3);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Protocol {
    Pcp,
    NatPmp,
    Upnp,
}

impl fmt::Display for Protocol {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Pcp => "PCP",
            Self::NatPmp => "NAT-PMP",
            Self::Upnp => "UPnP",
        })
    }
}

/// A router that answered at least one forwarding protocol.
pub struct Router {
    local_ip: Ipv4Addr,
    /// Set when the gateway answered PCP or NAT-PMP.
    pmp: Option<(GatewayAddress, Protocol)>,
    upnp: Option<Gateway<Tokio>>,
}

/// One port the router agreed to forward to this machine.
pub struct Mapping {
    pub internal: u16,
    pub external: u16,
    /// Kept for PCP and NAT-PMP, which renew and delete through it.
    pmp: Option<crab_nat::PortMapping>,
}

/// The gateway this machine sends internet traffic through, if it can be found.
pub fn default_gateway() -> Option<Ipv4Addr> {
    netdev::get_default_gateway().ok()?.ipv4.into_iter().next()
}

impl Router {
    /// Look for a router that speaks any forwarding protocol. `None` when nothing answers.
    pub async fn discover(local_ip: Ipv4Addr) -> Option<Self> {
        let pmp = async {
            let gateway = GatewayAddress::from(default_gateway()?);
            match natpmp::external_address(gateway, Some(PMP_TIMEOUT)).await {
                Ok(_) => Some((gateway, Protocol::NatPmp)),
                // A PCP-only router answers a NAT-PMP question with "wrong version".
                Err(natpmp::Failure::UnsupportedVersion(_)) => {
                    Some((gateway, Protocol::Pcp))
                }
                Err(_) => None,
            }
        };
        let upnp = async {
            search_gateway(SearchOptions {
                bind_addr: SocketAddr::new(IpAddr::V4(local_ip), 0),
                timeout: Some(UPNP_SEARCH_TIMEOUT),
                ..Default::default()
            })
            .await
            .ok()
        };
        let (pmp, upnp) = tokio::join!(pmp, upnp);
        (pmp.is_some() || upnp.is_some()).then_some(Self {
            local_ip,
            pmp,
            upnp,
        })
    }

    /// The protocols this router answered, for display.
    pub fn protocols(&self) -> String {
        let mut names = Vec::new();
        if let Some((_, protocol)) = self.pmp {
            names.push(protocol.to_string());
        }
        if self.upnp.is_some() {
            names.push(Protocol::Upnp.to_string());
        }
        names.join(" and ")
    }

    /// The address the router has on its internet side. A private or shared address here means
    /// the router is not the last hop to the internet.
    pub async fn wan_ip(&self) -> Option<IpAddr> {
        if let Some((gateway, Protocol::NatPmp)) = self.pmp
            && let Ok(ip) =
                natpmp::external_address(gateway, Some(PMP_TIMEOUT)).await
        {
            return Some(IpAddr::V4(ip));
        }
        match &self.upnp {
            Some(upnp) => upnp.get_external_ip().await.ok(),
            None => None,
        }
    }

    /// Ask for `external` to be forwarded to `internal` on this machine. PCP and NAT-PMP treat
    /// `external` as a wish and may grant another port; the granted one is in the result.
    /// The error is the router's refusal in words.
    pub async fn map(
        &self,
        internal: u16,
        external: u16,
    ) -> Result<Mapping, String> {
        let mut refusal = String::new();
        if let (Some((gateway, protocol)), Some(port)) =
            (self.pmp, NonZeroU16::new(internal))
        {
            match crab_nat::PortMapping::new(
                gateway,
                IpAddr::V4(self.local_ip),
                InternetProtocol::Tcp,
                port,
                PortMappingOptions {
                    external_port: NonZeroU16::new(external),
                    lifetime_seconds: Some(LEASE_SECONDS),
                    timeout_config: Some(PMP_TIMEOUT),
                },
            )
            .await
            {
                Ok(mapping) => {
                    return Ok(Mapping {
                        internal,
                        external: mapping.external_port().get(),
                        pmp: Some(mapping),
                    });
                }
                Err(error) => refusal = format!("{protocol}: {error}"),
            }
        }
        if let Some(upnp) = &self.upnp {
            match self.upnp_add(upnp, internal, external).await {
                Ok(()) => {
                    return Ok(Mapping {
                        internal,
                        external,
                        pmp: None,
                    });
                }
                Err(error) => {
                    if !refusal.is_empty() {
                        refusal.push_str("; ");
                    }
                    let _ = write!(refusal, "UPnP: {error}");
                }
            }
        }
        Err(refusal)
    }

    async fn upnp_add(
        &self,
        upnp: &Gateway<Tokio>,
        internal: u16,
        external: u16,
    ) -> Result<(), AddPortError> {
        let local = SocketAddr::new(IpAddr::V4(self.local_ip), internal);
        let add = |lease| {
            upnp.add_port(
                PortMappingProtocol::TCP,
                external,
                local,
                lease,
                DESCRIPTION,
            )
        };
        match add(LEASE_SECONDS).await {
            // Some routers only accept mappings that never expire. Core removes those when their
            // server stops.
            Err(AddPortError::OnlyPermanentLeasesSupported) => add(0).await,
            result => result,
        }
    }

    /// Extend a mapping's lease. Also restores it when the router restarted and forgot it.
    pub async fn renew(&self, mapping: &mut Mapping) -> Result<(), String> {
        match (&mut mapping.pmp, &self.upnp) {
            (Some(pmp), _) => {
                pmp.renew().await.map_err(|error| error.to_string())?;
                mapping.external = pmp.external_port().get();
                Ok(())
            }
            (None, Some(upnp)) => self
                .upnp_add(upnp, mapping.internal, mapping.external)
                .await
                .map_err(|error| error.to_string()),
            (None, None) => Err("the router no longer answers".into()),
        }
    }

    /// Ask the router to stop forwarding. Failure is ignored: the lease ends by itself.
    pub async fn remove(&self, mapping: Mapping) {
        match (mapping.pmp, &self.upnp) {
            (Some(pmp), _) => {
                let _ = pmp.try_drop().await;
            }
            (None, Some(upnp)) => {
                let _ = upnp
                    .remove_port(PortMappingProtocol::TCP, mapping.external)
                    .await;
            }
            (None, None) => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{
        collections::HashMap,
        sync::{Arc, Mutex},
    };

    use tokio::net::UdpSocket;

    use super::*;

    /// A NAT-PMP router on this computer. It grants the outside port one above the one asked for,
    /// and records what is mapped: internal port to (outside port, times granted).
    async fn fake_router() -> Arc<Mutex<HashMap<u16, (u16, u32)>>> {
        let socket =
            UdpSocket::bind((Ipv4Addr::LOCALHOST, crab_nat::GATEWAY_PORT))
                .await
                .expect("port 5351 is free on this computer");
        let mapped = Arc::new(Mutex::new(HashMap::new()));
        let seen = Arc::clone(&mapped);
        tokio::spawn(async move {
            let mut request = [0u8; 64];
            loop {
                let Ok((_, from)) = socket.recv_from(&mut request).await else {
                    return;
                };
                let opcode = request[1];
                let mut reply = vec![0, 0x80 | opcode, 0, 0, 0, 0, 0, 1];
                if request[0] != 0 {
                    // PCP: answer the way a NAT-PMP-only router does, "unsupported version".
                    reply[3] = 1;
                } else if opcode == 0 {
                    reply.extend([203, 0, 113, 9]);
                } else {
                    let internal = u16::from_be_bytes([request[4], request[5]]);
                    let wanted = u16::from_be_bytes([request[6], request[7]]);
                    let lifetime = &request[8..12];
                    let external = if lifetime == [0; 4] {
                        seen.lock().unwrap().remove(&internal);
                        0
                    } else {
                        let mut seen = seen.lock().unwrap();
                        let entry =
                            seen.entry(internal).or_insert((wanted + 1, 0));
                        entry.1 += 1;
                        entry.0
                    };
                    reply.extend(internal.to_be_bytes());
                    reply.extend(external.to_be_bytes());
                    reply.extend(lifetime);
                }
                let _ = socket.send_to(&reply, from).await;
            }
        });
        mapped
    }

    #[tokio::test]
    async fn a_nat_pmp_router_grants_renews_and_removes_a_mapping() {
        let mapped = fake_router().await;
        let router = Router {
            local_ip: Ipv4Addr::LOCALHOST,
            pmp: Some((
                GatewayAddress::from(Ipv4Addr::LOCALHOST),
                Protocol::NatPmp,
            )),
            upnp: None,
        };
        assert_eq!(router.protocols(), "NAT-PMP");
        assert_eq!(
            router.wan_ip().await,
            Some(IpAddr::V4(Ipv4Addr::new(203, 0, 113, 9)))
        );

        // The router grants a different outside port than the one asked for; Core takes it.
        let mut mapping = router.map(25565, 25565).await.unwrap();
        assert_eq!((mapping.internal, mapping.external), (25565, 25566));
        assert_eq!(mapped.lock().unwrap().get(&25565), Some(&(25566, 1)));

        router.renew(&mut mapping).await.unwrap();
        assert_eq!(mapping.external, 25566);
        assert_eq!(mapped.lock().unwrap().get(&25565), Some(&(25566, 2)));

        router.remove(mapping).await;
        assert!(mapped.lock().unwrap().is_empty());
    }
}
