use std::{
    net::{IpAddr, Ipv4Addr, SocketAddrV4},
    sync::Arc,
};

use serde::Serialize;

use crate::application::state::AppState;

#[derive(Debug, Serialize)]
pub struct NetworkStatus {
    pub direct_api_url: String,
    pub lan_api_url: Option<String>,
    pub public_api_url: String,
    pub upnp: UpnpStatus,
    pub cloudflare_tunnel: FallbackStatus,
    pub minecraft_exposure: MinecraftExposureStatus,
    pub playit: FallbackStatus,
}

#[derive(Debug, Serialize)]
pub struct UpnpStatus {
    pub state: String,
    pub external_ip: Option<String>,
    pub external_port: Option<u16>,
    pub error: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct FallbackStatus {
    pub state: String,
    pub url: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct MinecraftExposureStatus {
    pub state: String,
    pub cloudflare_tunnel: bool,
}

pub async fn network_status(state: &Arc<AppState>) -> NetworkStatus {
    let local_url = format!("http://127.0.0.1:{}", state.config.port);
    let lan_ip = local_ip_address::local_ip().ok();
    let lan_api_url =
        lan_ip.map(|ip| format!("http://{ip}:{}", state.config.port));
    let upnp = try_map_core_api(state.config.port, lan_ip).await;
    NetworkStatus {
        direct_api_url: local_url,
        lan_api_url,
        public_api_url: state.config.public_url.clone(),
        upnp,
        cloudflare_tunnel: FallbackStatus {
            state: "fallback_not_configured".to_string(),
            url: None,
        },
        minecraft_exposure: MinecraftExposureStatus {
            state: "direct_or_upnp_per_instance".to_string(),
            cloudflare_tunnel: false,
        },
        playit: FallbackStatus {
            state: "not_implemented".to_string(),
            url: None,
        },
    }
}

async fn try_map_core_api(port: u16, lan_ip: Option<IpAddr>) -> UpnpStatus {
    if port == 0 {
        return UpnpStatus {
            state: "disabled".to_string(),
            external_ip: None,
            external_port: None,
            error: Some("Core API port is ephemeral".to_string()),
        };
    }
    let Some(IpAddr::V4(local_ip)) = lan_ip else {
        return UpnpStatus {
            state: "unavailable".to_string(),
            external_ip: None,
            external_port: None,
            error: Some("no IPv4 LAN address detected".to_string()),
        };
    };
    tokio::task::spawn_blocking(move || map_port(port, local_ip))
        .await
        .unwrap_or_else(|error| UpnpStatus {
            state: "failed".to_string(),
            external_ip: None,
            external_port: None,
            error: Some(error.to_string()),
        })
}

fn map_port(port: u16, local_ip: Ipv4Addr) -> UpnpStatus {
    match igd::search_gateway(Default::default()) {
        Ok(gateway) => {
            let internal = SocketAddrV4::new(local_ip, port);
            match gateway.add_port(
                igd::PortMappingProtocol::TCP,
                port,
                internal,
                3600,
                "Amberite Core API",
            ) {
                Ok(()) => {
                    let external_ip =
                        gateway.get_external_ip().map(|ip| ip.to_string()).ok();
                    UpnpStatus {
                        state: "mapped".to_string(),
                        external_ip,
                        external_port: Some(port),
                        error: None,
                    }
                }
                Err(error) => UpnpStatus {
                    state: "failed".to_string(),
                    external_ip: None,
                    external_port: None,
                    error: Some(error.to_string()),
                },
            }
        }
        Err(error) => UpnpStatus {
            state: "unavailable".to_string(),
            external_ip: None,
            external_port: None,
            error: Some(error.to_string()),
        },
    }
}
