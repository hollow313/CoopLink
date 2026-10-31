use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::net::UdpSocket;

const PROTOCOL_MAGIC: &[u8; 4] = b"COOP";
const PROTOCOL_VERSION: u8 = 1;
const PKT_INSTANCE_HEARTBEAT: u8 = 0x10;
const PKT_INSTANCE_ID_ASSIGNED: u8 = 0x11;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelemetryPayload {
    pub instance_uuid: u64,
    pub player_name: String,
    pub tunnel_mounted: bool,
    pub is_host: bool,
    pub room_code: String,
    pub peer_count: usize,
    pub screen_sharing: bool,
    pub version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelemetryAck {
    pub assigned_instance_id: String,
    pub active_tunnels: usize,
    pub active_instances: usize,
}

pub struct CentralTelemetryClient {
    pub assigned_id: Arc<RwLock<String>>,
    pub server_connected: Arc<AtomicBool>,
    pub discovered_server: Arc<RwLock<Option<SocketAddr>>>,
    pub state_snapshot: Arc<RwLock<TelemetryPayload>>,
    pub target_addresses: Arc<RwLock<Vec<String>>>,
}

impl CentralTelemetryClient {
    pub fn new(instance_uuid: u64, player_name: String, central_addr: String) -> Arc<Self> {
        let fallback_id = format!("CL-{}", instance_uuid);
        let client = Arc::new(Self {
            assigned_id: Arc::new(RwLock::new(fallback_id)),
            server_connected: Arc::new(AtomicBool::new(false)),
            discovered_server: Arc::new(RwLock::new(None)),
            state_snapshot: Arc::new(RwLock::new(TelemetryPayload {
                instance_uuid,
                player_name,
                tunnel_mounted: false,
                is_host: false,
                room_code: String::new(),
                peer_count: 0,
                screen_sharing: false,
                version: "1.0.0".to_string(),
            })),
            target_addresses: Arc::new(RwLock::new(vec![central_addr])),
        });
        client
    }

    pub fn update_state(
        &self,
        player_name: &str,
        tunnel_mounted: bool,
        is_host: bool,
        room_code: &str,
        peer_count: usize,
        screen_sharing: bool,
        central_addr: &str,
        remote_peer_addr: &str,
    ) {
        {
            let mut snap = self.state_snapshot.write();
            snap.player_name = player_name.to_string();
            snap.tunnel_mounted = tunnel_mounted;
            snap.is_host = is_host;
            snap.room_code = room_code.to_string();
            snap.peer_count = peer_count;
            snap.screen_sharing = screen_sharing;
        }

        let mut addrs = Vec::new();
        if !central_addr.trim().is_empty() {
            if central_addr.contains(':') {
                addrs.push(central_addr.trim().to_string());
            } else {
                addrs.push(format!("{}:30024", central_addr.trim()));
            }
        }
        // Si l'utilisateur a renseigné une IP dans remote_server_address, on teste aussi le port 30024 dessus
        if let Some(ip_part) = remote_peer_addr.split(':').next() {
            let candidate = format!("{}:30024", ip_part.trim());
            if !addrs.contains(&candidate) {
                addrs.push(candidate);
            }
        }
        for default_bcast in [
            "127.0.0.1:30024",
            "255.255.255.255:30024",
            "192.168.1.255:30024",
            "192.168.0.255:30024",
        ] {
            let s = default_bcast.to_string();
            if !addrs.contains(&s) {
                addrs.push(s);
            }
        }
        *self.target_addresses.write() = addrs;
    }

    pub fn start_background_loop(self: &Arc<Self>, rt: &tokio::runtime::Runtime) {
        let me = self.clone();
        rt.spawn(async move {
            let socket = match UdpSocket::bind("0.0.0.0:0").await {
                Ok(s) => {
                    let _ = s.set_broadcast(true);
                    Arc::new(s)
                }
                Err(_) => return,
            };

            // Tâche d'écoute des réponses du serveur central (attribution de l'ID unique)
            let sock_rx = socket.clone();
            let me_rx = me.clone();
            tokio::spawn(async move {
                let mut buf = vec![0u8; 4096];
                while let Ok((len, src)) = sock_rx.recv_from(&mut buf).await {
                    if len > 10 && &buf[0..4] == PROTOCOL_MAGIC && buf[5] == PKT_INSTANCE_ID_ASSIGNED {
                        let payload_end = len - 4;
                        if let Ok(ack) = serde_json::from_slice::<TelemetryAck>(&buf[6..payload_end]) {
                            *me_rx.assigned_id.write() = ack.assigned_instance_id;
                            *me_rx.discovered_server.write() = Some(src);
                            me_rx.server_connected.store(true, Ordering::Relaxed);
                        }
                    }
                }
            });

            // Boucle d'envoi périodique (toutes les 3 secondes)
            loop {
                let pkt = {
                    let snap = me.state_snapshot.read().clone();
                    encode_telemetry_packet(&snap)
                };

                let targets = me.target_addresses.read().clone();
                let discovered_opt = *me.discovered_server.read();
                if let Some(discovered) = discovered_opt {
                    let _ = socket.send_to(&pkt, discovered).await;
                }
                for t in targets {
                    if let Ok(addr) = t.parse::<SocketAddr>() {
                        let _ = socket.send_to(&pkt, addr).await;
                    }
                }

                tokio::time::sleep(Duration::from_secs(3)).await;
            }
        });
    }
}

fn encode_telemetry_packet(payload: &TelemetryPayload) -> Vec<u8> {
    let json_bytes = serde_json::to_vec(payload).unwrap_or_default();
    let mut buf = Vec::with_capacity(6 + json_bytes.len() + 4);
    buf.extend_from_slice(PROTOCOL_MAGIC);
    buf.push(PROTOCOL_VERSION);
    buf.push(PKT_INSTANCE_HEARTBEAT);
    buf.extend_from_slice(&json_bytes);
    let crc = crc32fast::hash(&buf);
    buf.extend_from_slice(&crc.to_be_bytes());
    buf
}
