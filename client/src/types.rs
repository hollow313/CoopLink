use serde::{Deserialize, Serialize};
use std::net::{Ipv4Addr, SocketAddr};

/// Rôle du nœud actuel
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NodeRole {
    Host,
    Client,
    Idle,
}

/// Statut de connexion
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConnectionStatus {
    Disconnected,
    Connecting,
    Connected,
    Hosting,
    Error,
}

/// Représentation d'un pair connecté dans le salon
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerInfo {
    pub id: u64,
    pub name: String,
    pub virtual_ip: Ipv4Addr,
    pub endpoint: SocketAddr,
    pub ping_ms: u32,
    pub last_seen_secs: u64,
    pub is_host: bool,
    pub bytes_sent: u64,
    pub bytes_recv: u64,
}

/// Événements de logs pour la console rétro
#[derive(Debug, Clone)]
pub struct LogEntry {
    pub timestamp: String,
    pub level: LogLevel,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogLevel {
    Info,
    Warning,
    Error,
    Success,
    Game,
}

/// État global partagé de l'application
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct AppState {
    pub status: ConnectionStatus,
    pub role: NodeRole,
    pub room_code: String,
    pub virtual_ip: Option<Ipv4Addr>,
    pub peers: Vec<PeerInfo>,
    pub interface_metric: u32,
    pub metric_applied: bool,
    pub firewall_ok: bool,
    pub total_bytes_tx: u64,
    pub total_bytes_rx: u64,
    pub broadcast_packets_relayed: u64,
    pub adapter_name: String,
    pub adapter_status: String,
    pub upnp_enabled: bool,
    pub upnp_status_text: String,
    pub public_ip: Option<Ipv4Addr>,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            status: ConnectionStatus::Disconnected,
            role: NodeRole::Idle,
            room_code: "COOP-7701".to_string(),
            virtual_ip: None,
            peers: Vec::new(),
            interface_metric: 5,
            metric_applied: false,
            firewall_ok: false,
            total_bytes_tx: 0,
            total_bytes_rx: 0,
            broadcast_packets_relayed: 0,
            adapter_name: "CoopLink Virtual Adapter".to_string(),
            adapter_status: "Prêt / En attente".to_string(),
            upnp_enabled: true,
            upnp_status_text: "En attente".to_string(),
            public_ip: None,
        }
    }
}
