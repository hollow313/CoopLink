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

    /// Tente d'ouvrir dynamiquement le port UDP sur la box internet via UPnP IGD
    pub fn open_port(&mut self, port: u16) -> Result<Ipv4Addr> {
        info!("Recherche de la passerelle UPnP IGD de la box internet...");
        self.current_state = UpnpState::Searching;

        // Méthode 1 : API native Windows NATUPnP COM (HNetCfg.NATUPnP - supporte 100% Freebox, Livebox, Bbox, SFR)
        #[cfg(windows)]
        {
            if let Ok(local_ip) = get_local_lan_ip() {
                let stream_port = crate::streaming::settings::DEFAULT_STREAM_PORT;
                let ps_cmd = format!(
                    r#"try {{ $nat = New-Object -ComObject HNetCfg.NATUPnP; $maps = $nat.StaticPortMappingCollection; if ($maps) {{ if (-not ($maps | Where-Object {{ $_.ExternalPort -eq {0} -and $_.Protocol -eq 'UDP' }})) {{ $maps.Add({0}, 'UDP', {0}, '{1}', $true, 'CoopLink P2P Tunnel'); }} if (-not ($maps | Where-Object {{ $_.ExternalPort -eq {2} -and $_.Protocol -eq 'TCP' }})) {{ $maps.Add({2}, 'TCP', {2}, '{1}', $true, 'CoopLink Stream TCP'); }} $ip = ($maps | Where-Object {{ $_.ExternalPort -eq {0} }}).ExternalIPAddress; if ($ip) {{ Write-Output $ip; exit 0 }} }} }} catch {{ }} exit 1"#,
                    port, local_ip, stream_port
                );
                if let Ok(out) = crate::platform::windows::process::run_hidden_powershell(&ps_cmd) {
                    if out.status.success() {
                        let stdout = String::from_utf8_lossy(&out.stdout).trim().to_string();
                        if let Ok(ext_ip) = stdout.parse::<Ipv4Addr>() {
                            info!("Box UPnP (Windows NATUPnP) : Port UDP {} ouvert avec succès vers {} ! IP Externe: {}", port, local_ip, ext_ip);
                            self.current_state = UpnpState::Active {
                                external_ip: ext_ip,
                                mapped_port: port,
                                gateway_name: "Box Internet (UPnP IGD Windows)".to_string(),
                            };
                            self.mapped_port = Some(port);
                            return Ok(ext_ip);
                        }
                    }
                }
            }
        }

        // Méthode 2 : Bibliothèque igd_next (Multiplateforme Linux/macOS ou fallback Windows)
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
        #[cfg(windows)]
        if let Some(port) = self.mapped_port {
            let ps_cmd = format!(
                r#"try {{ $nat = New-Object -ComObject HNetCfg.NATUPnP; $maps = $nat.StaticPortMappingCollection; if ($maps) {{ $maps.Remove({0}, 'UDP'); $maps.Remove({1}, 'TCP') }} }} catch {{ }}"#,
                port,
                crate::streaming::settings::DEFAULT_STREAM_PORT
            );
            let _ = crate::platform::windows::process::run_hidden_powershell(&ps_cmd);
        }

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
    let socket = std::net::UdpSocket::bind("0.0.0.0:0")?;
    socket.connect("8.8.8.8:80")?;
    if let SocketAddr::V4(addr) = socket.local_addr()? {
        Ok(*addr.ip())
    } else {
        Ok(Ipv4Addr::new(192, 168, 1, 100))
    }
}
