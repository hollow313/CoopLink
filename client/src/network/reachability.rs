//! Détection de la joignabilité de l'hôte : IPv4 directe, IPv4 mutualisée (CGNAT) ou IPv6.
//!
//! Beaucoup de connexions (fibre mutualisée, 4G/5G, certains opérateurs) placent plusieurs
//! abonnés derrière une même adresse IPv4 publique : aucune redirection de port n'est alors
//! possible, et l'invité ne peut pas joindre l'hôte. L'IPv6, lui, est routé de bout en bout
//! sans NAT : c'est le repli transparent quand il est disponible.

use std::net::{Ipv4Addr, Ipv6Addr, SocketAddr, UdpSocket};
use tracing::info;

/// Comment l'hôte est joignable depuis Internet.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reachability {
    /// IPv4 publique propre + redirection de port : le chemin actuel fonctionne.
    DirectV4,
    /// IPv4 mutualisée (CGNAT / double NAT) : la redirection ne peut pas aboutir.
    SharedV4,
    /// Aucune IPv4 exploitable, mais une IPv6 globale est disponible.
    Ipv6Only,
    /// Rien de concluant (pas de réseau, détection impossible).
    Unknown,
}

#[derive(Debug, Clone)]
pub struct ReachabilityReport {
    pub mode: Reachability,
    /// Adresse vue par Internet.
    pub public_v4: Option<Ipv4Addr>,
    /// Adresse externe annoncée par la box (UPnP), si le mapping a réussi.
    pub upnp_v4: Option<Ipv4Addr>,
    /// Première IPv6 globale de la machine.
    pub global_v6: Option<Ipv6Addr>,
}

impl Reachability {
    /// Même message que [`ReachabilityReport::summary`], à partir du seul mode.
    pub fn summary_for(&self, global_v6: Option<Ipv6Addr>) -> String {
        match self {
            Reachability::DirectV4 => "IPv4 directe : la redirection de port fonctionne.".to_string(),
            Reachability::SharedV4 => match global_v6 {
                Some(_) => "IPv4 mutualisée (CGNAT) : repli automatique sur IPv6.".to_string(),
                None => "IPv4 mutualisée (CGNAT) et aucune IPv6 globale : l'invité ne pourra pas se connecter sans relais.".to_string(),
            },
            Reachability::Ipv6Only => "Pas d'IPv4 publique exploitable : connexion par IPv6.".to_string(),
            Reachability::Unknown => "Joignabilité indéterminée.".to_string(),
        }
    }
}

impl ReachabilityReport {
    /// Adresse à communiquer à l'invité, port compris.
    pub fn endpoint(&self, port: u16) -> String {
        match (self.mode, self.public_v4, self.global_v6) {
            (Reachability::SharedV4, _, Some(v6)) | (Reachability::Ipv6Only, _, Some(v6)) => {
                format!("[{}]:{}", v6, port)
            }
            (_, Some(v4), _) => format!("{}:{}", v4, port),
            (_, _, Some(v6)) => format!("[{}]:{}", v6, port),
            _ => format!("adresse inconnue:{}", port),
        }
    }

    /// Message court pour l'interface.
    pub fn summary(&self) -> String {
        match self.mode {
            Reachability::DirectV4 => {
                "IPv4 directe : la redirection de port fonctionne, l'invité se connecte normalement."
                    .to_string()
            }
            Reachability::SharedV4 => match self.global_v6 {
                Some(_) => "IPv4 mutualisée (CGNAT) : la redirection de port est impossible. Repli automatique sur IPv6.".to_string(),
                None => "IPv4 mutualisée (CGNAT) et aucune IPv6 globale : l'invité ne pourra pas se connecter sans relais.".to_string(),
            },
            Reachability::Ipv6Only => {
                "Pas d'IPv4 publique exploitable : connexion par IPv6.".to_string()
            }
            Reachability::Unknown => "Joignabilité indéterminée (réseau indisponible).".to_string(),
        }
    }

    /// L'invité doit-il viser l'adresse IPv6 ?
    pub fn uses_ipv6(&self) -> bool {
        matches!(self.mode, Reachability::SharedV4 | Reachability::Ipv6Only)
            && self.global_v6.is_some()
    }
}

/// `100.64.0.0/10` : plage réservée au CGNAT (RFC 6598).
pub fn is_cgnat(ip: Ipv4Addr) -> bool {
    let o = ip.octets();
    o[0] == 100 && (64..=127).contains(&o[1])
}

/// `10/8`, `172.16/12`, `192.168/16` : adresse privée, donc jamais joignable de l'extérieur.
pub fn is_private_v4(ip: Ipv4Addr) -> bool {
    ip.is_private() || ip.is_loopback() || ip.is_link_local()
}

/// Détecte le mode de joignabilité à partir de l'état UPnP et de l'IP vue par Internet.
pub fn detect(upnp_external: Option<Ipv4Addr>, upnp_active: bool) -> ReachabilityReport {
    let public_v4 = crate::network::public_ip::fetch_public_ip_sync();
    let global_v6 = first_global_ipv6();

    let mut report = ReachabilityReport {
        mode: Reachability::Unknown,
        public_v4,
        upnp_v4: upnp_external,
        global_v6,
    };

    report.mode = match (public_v4, upnp_external, upnp_active) {
        // La box annonce une adresse CGNAT, ou une adresse différente de celle vue dehors :
        // on est derrière un NAT opérateur, aucune redirection ne peut aboutir.
        (_, Some(upnp), _) if is_cgnat(upnp) || is_private_v4(upnp) => Reachability::SharedV4,
        (Some(public), Some(upnp), true) if public != upnp => Reachability::SharedV4,
        (Some(public), _, true) if is_cgnat(public) => Reachability::SharedV4,
        (Some(public), _, true) if !is_private_v4(public) => Reachability::DirectV4,
        // Pas de redirection UPnP : sans IPv6, on ne peut rien promettre.
        (_, _, false) => {
            if report.global_v6.is_some() {
                Reachability::Ipv6Only
            } else {
                Reachability::Unknown
            }
        }
        _ => {
            if report.global_v6.is_some() {
                Reachability::Ipv6Only
            } else {
                Reachability::SharedV4
            }
        }
    };

    info!(
        "Joignabilité : {:?} (IPv4 publique {:?}, UPnP {:?}, IPv6 {:?})",
        report.mode, report.public_v4, report.upnp_v4, report.global_v6
    );
    report
}

/// Première adresse IPv6 globale (2000::/3), hors lien-local et hors loopback.
pub fn first_global_ipv6() -> Option<Ipv6Addr> {
    // 1. L'adresse que le système choisirait réellement pour sortir vers Internet
    //    (celle de l'interface qui porte la route par défaut). C'est la seule qui soit
    //    joignable de l'extérieur : un Get-NetIPAddress brut peut renvoyer l'adresse
    //    d'un adaptateur virtuel (Hyper-V, VPN, Tailscale) que le pair ne peut pas joindre.
    if let Ok(socket) = UdpSocket::bind("[::]:0") {
        if socket.connect("[2001:4860:4860::8888]:53").is_ok() {
            if let Ok(SocketAddr::V6(addr)) = socket.local_addr() {
                let ip = *addr.ip();
                if is_global_v6(ip) {
                    return Some(ip);
                }
            }
        }
    }

    #[cfg(windows)]
    {
        let ps = "Get-NetIPAddress -AddressFamily IPv6 -ErrorAction SilentlyContinue | \
                  Where-Object { $_.IPAddress -match '^[23]' } | \
                  Select-Object -ExpandProperty IPAddress";
        if let Ok(out) = crate::platform::windows::process::run_hidden_powershell(ps) {
            if out.status.success() {
                for line in String::from_utf8_lossy(&out.stdout).lines() {
                    let candidate = line.trim().trim_end_matches('%');
                    if let Ok(ip) = candidate.parse::<Ipv6Addr>() {
                        if is_global_v6(ip) {
                            return Some(ip);
                        }
                    }
                }
            }
        }
    }

    None
}

fn is_global_v6(ip: Ipv6Addr) -> bool {
    let first = ip.segments()[0];
    // 2000::/3 = adresses globales unicast.
    (0x2000..=0x3fff).contains(&first) && !ip.is_loopback()
}

/// Formate une cible `hôte:port` en acceptant les littéraux IPv6 nus.
pub fn format_endpoint(host: &str, port: u16) -> String {
    let trimmed = host.trim();
    if trimmed.starts_with('[') {
        return if trimmed.contains("]:") {
            trimmed.to_string()
        } else {
            format!("{}:{}", trimmed, port)
        };
    }
    if trimmed.matches(':').count() >= 2 {
        // IPv6 sans crochets.
        format!("[{}]:{}", trimmed, port)
    } else if trimmed.contains(':') {
        trimmed.to_string()
    } else {
        format!("{}:{}", trimmed, port)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cgnat_range_is_detected() {
        assert!(is_cgnat(Ipv4Addr::new(100, 64, 0, 1)));
        assert!(is_cgnat(Ipv4Addr::new(100, 127, 255, 254)));
        assert!(!is_cgnat(Ipv4Addr::new(100, 128, 0, 1)));
        assert!(!is_cgnat(Ipv4Addr::new(192, 168, 1, 1)));
        assert!(!is_cgnat(Ipv4Addr::new(81, 2, 3, 4)));
    }

    #[test]
    fn endpoint_prefers_ipv6_when_ipv4_is_shared() {
        let report = ReachabilityReport {
            mode: Reachability::SharedV4,
            public_v4: Some(Ipv4Addr::new(81, 2, 3, 4)),
            upnp_v4: Some(Ipv4Addr::new(100, 64, 1, 2)),
            global_v6: Some("2a01:cb00::1".parse().unwrap()),
        };
        assert!(report.uses_ipv6());
        assert_eq!(report.endpoint(47500), "[2a01:cb00::1]:47500");

        let direct = ReachabilityReport {
            mode: Reachability::DirectV4,
            public_v4: Some(Ipv4Addr::new(81, 2, 3, 4)),
            upnp_v4: Some(Ipv4Addr::new(81, 2, 3, 4)),
            global_v6: Some("2a01:cb00::1".parse().unwrap()),
        };
        assert!(!direct.uses_ipv6());
        assert_eq!(direct.endpoint(47500), "81.2.3.4:47500");
    }

    #[test]
    fn endpoint_formats_bare_ipv6() {
        assert_eq!(format_endpoint("2a01:cb00::1", 47500), "[2a01:cb00::1]:47500");
        assert_eq!(format_endpoint("[2a01:cb00::1]", 47500), "[2a01:cb00::1]:47500");
        assert_eq!(format_endpoint("1.2.3.4", 47500), "1.2.3.4:47500");
        assert_eq!(format_endpoint("1.2.3.4:5000", 47500), "1.2.3.4:5000");
    }
}
