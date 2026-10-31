use anyhow::Result;
use igd_next::{search_gateway, Gateway, PortMappingProtocol, SearchOptions};
use std::net::{IpAddr, Ipv4Addr, SocketAddr, SocketAddrV4};
use std::time::Duration;
use tracing::{info, warn};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UpnpState {
    Disabled,
    Searching,
    Active {
        external_ip: Ipv4Addr,
        mapped_port: u16,
        gateway_name: String,
    },
    Failed {
        reason: String,
    },
}

pub struct UpnpManager {
    current_state: UpnpState,
    active_gateway: Option<Gateway>,
    mapped_port: Option<u16>,
}

impl UpnpManager {
    pub fn new() -> Self {
        Self {
            current_state: UpnpState::Disabled,
            active_gateway: None,
            mapped_port: None,
        }
    }

    pub fn state(&self) -> &UpnpState {
        &self.current_state
    }

    /// Tente d'ouvrir le port UDP sur la box internet via UPnP
    pub fn open_port(&mut self, port: u16) -> Result<Ipv4Addr> {
        info!("Recherche de la passerelle UPnP de la box internet...");
        self.current_state = UpnpState::Searching;

        let mut options = SearchOptions::default();
        options.timeout = Some(Duration::from_secs(3));

        let gateway = match search_gateway(options) {
            Ok(gw) => gw,
            Err(e) => {
                let err_msg = format!("Aucune box UPnP détectée ({})", e);
                warn!("{}", err_msg);
                self.current_state = UpnpState::Failed {
                    reason: err_msg.clone(),
                };
                return Err(anyhow::anyhow!(err_msg));
            }
        };

        // Récupère l'IP publique externe de la box
        let ext_ip = match gateway.get_external_ip() {
            Ok(IpAddr::V4(ipv4)) => ipv4,
            Ok(IpAddr::V6(_)) => {
                let err_msg = "IP externe IPv6 non supportée en UPnP IPv4".to_string();
                self.current_state = UpnpState::Failed {
                    reason: err_msg.clone(),
                };
                return Err(anyhow::anyhow!(err_msg));
            }
            Err(e) => {
                let err_msg = format!("Impossible d'obtenir l'IP externe de la box ({})", e);
                self.current_state = UpnpState::Failed {
                    reason: err_msg.clone(),
                };
                return Err(anyhow::anyhow!(err_msg));
            }
        };

        // Trouve l'adresse IP locale de notre PC sur le LAN
        let local_ip = get_local_lan_ip()?;
        let local_socket = SocketAddrV4::new(local_ip, port);

        info!(
            "Box UPnP détectée ! IP Externe: {}. Redirection du port UDP {} vers {}...",
            ext_ip, port, local_socket
        );

        // Enregistre la règle UPnP sur la box
        if let Err(e) = gateway.add_port(
            PortMappingProtocol::UDP,
            port,
            SocketAddr::V4(local_socket),
            3600 * 12, // Durée de bail : 12 heures
            "CoopLink Virtual LAN Gaming",
        ) {
            let err_msg = format!("Échec de l'ouverture du port sur la box: {}", e);
            warn!("{}", err_msg);
            self.current_state = UpnpState::Failed {
                reason: err_msg.clone(),
            };
            return Err(anyhow::anyhow!(err_msg));
        }

        info!("Port UDP {} ouvert avec succès sur la Box via UPnP !", port);

        self.current_state = UpnpState::Active {
            external_ip: ext_ip,
            mapped_port: port,
            gateway_name: "Box Internet (UPnP IGD)".to_string(),
        };
        self.active_gateway = Some(gateway);
        self.mapped_port = Some(port);

        Ok(ext_ip)
    }

    /// Ferme proprement le port sur la box
    pub fn close_port(&mut self) {
        if let (Some(gw), Some(port)) = (&self.active_gateway, self.mapped_port) {
            info!("Fermeture du port UPnP {} sur la box...", port);
            let _ = gw.remove_port(PortMappingProtocol::UDP, port);
        }
        self.active_gateway = None;
        self.mapped_port = None;
        self.current_state = UpnpState::Disabled;
    }
}

/// Trouve l'IP locale du PC utilisée pour communiquer sur le réseau local
fn get_local_lan_ip() -> Result<Ipv4Addr> {
    // Connexion UDP factice (sans émission réelle de paquets) pour identifier l'interface locale active
    let socket = std::net::UdpSocket::bind("0.0.0.0:0")?;
    socket.connect("8.8.8.8:80")?;
    if let SocketAddr::V4(addr) = socket.local_addr()? {
        Ok(*addr.ip())
    } else {
        Ok(Ipv4Addr::new(192, 168, 1, 100))
    }
}
