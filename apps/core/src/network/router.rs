//! Asking the home router to open ports, over PCP, NAT-PMP and UPnP.
//!
//! PCP and NAT-PMP are tried first because they are simpler and report clear errors; UPnP is the
//! most common and is tried last. Every mapping is TCP and has a finite lease, so one that is never
//! removed (a crash) still goes away by itself.

use std::{
    fmt,
    net::{IpAddr, Ipv4Addr, SocketAddr},
    num::NonZeroU16,
    time::Duration,
};

use crab_nat::{
    natpmp, GatewayAddress, InternetProtocol, PortMappingOptions,
    PortMappingType, TimeoutConfig,
};
use igd_next::{
    aio::tokio::{search_gateway, Tokio},
    aio::Gateway,
    AddPortError, PortMappingProtocol, SearchOptions,
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
    pub protocol: Protocol,
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
        if let Some((gateway, Protocol::NatPmp)) = self.pmp {
            if let Ok(ip) =
                natpmp::external_address(gateway, Some(PMP_TIMEOUT)).await
            {
                return Some(IpAddr::V4(ip));
            }
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
                        protocol: match mapping.mapping_type() {
                            PortMappingType::NatPmp => Protocol::NatPmp,
                            PortMappingType::Pcp { .. } => Protocol::Pcp,
                        },
                        pmp: Some(mapping),
                    })
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
                        protocol: Protocol::Upnp,
                        pmp: None,
                    })
                }
                Err(error) => {
                    if !refusal.is_empty() {
                        refusal.push_str("; ");
                    }
                    refusal.push_str(&format!("UPnP: {error}"));
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
            // Some routers only accept mappings that never expire. Core removes those on exit.
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
