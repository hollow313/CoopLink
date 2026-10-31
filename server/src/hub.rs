use anyhow::Result;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::{Ipv4Addr, SocketAddr};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio::net::UdpSocket;
use tracing::{debug, info};

// Import du protocole réseau commun
const PROTOCOL_MAGIC: &[u8; 4] = b"COOP";
const PROTOCOL_VERSION: u8 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectedClient {
    pub client_id: u64,
    pub player_name: String,
    pub room_code: String,
    pub endpoint: SocketAddr,
    pub virtual_ip: Ipv4Addr,
    pub is_host: bool,
    pub ping_ms: u32,
    pub connected_at_secs: u64,
    pub last_seen_secs: u64,
    pub bytes_tx: u64,
    pub bytes_rx: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActiveRoom {
    pub room_code: String,
    pub host_id: u64,
    pub host_name: String,
    pub created_at_secs: u64,
    pub client_count: usize,
    pub clients: Vec<ConnectedClient>,
}

pub struct ServerHub {
    pub udp_port: u16,
    pub next_client_id: AtomicU64,
    pub clients_by_id: Arc<RwLock<HashMap<u64, ConnectedClient>>>,
    pub endpoint_to_id: Arc<RwLock<HashMap<SocketAddr, u64>>>,
    pub rooms: Arc<RwLock<HashMap<String, ActiveRoom>>>,
    pub total_bytes_tx: Arc<AtomicU64>,
    pub total_bytes_rx: Arc<AtomicU64>,
    pub total_packets_relayed: Arc<AtomicU64>,
    pub start_time: Instant,
}

impl ServerHub {
    pub fn new(udp_port: u16) -> Self {
        Self {
            udp_port,
            next_client_id: AtomicU64::new(1001), // IDs uniques commençant à 1001
            clients_by_id: Arc::new(RwLock::new(HashMap::new())),
            endpoint_to_id: Arc::new(RwLock::new(HashMap::new())),
            rooms: Arc::new(RwLock::new(HashMap::new())),
            total_bytes_tx: Arc::new(AtomicU64::new(0)),
            total_bytes_rx: Arc::new(AtomicU64::new(0)),
            total_packets_relayed: Arc::new(AtomicU64::new(0)),
            start_time: Instant::now(),
        }
    }

    /// Démarre le serveur UDP centralisé sur le port 30024
    pub async fn start(self: Arc<Self>) -> Result<()> {
        let bind_addr = format!("0.0.0.0:{}", self.udp_port);
        let socket = Arc::new(UdpSocket::bind(&bind_addr).await?);
        info!("CoopLink Central Server UDP opérationnel sur {}", bind_addr);

        let hub = self.clone();
        let socket_rx = socket.clone();

        // Tâche de réception et de traitement des paquets
        tokio::spawn(async move {
            let mut buf = vec![0u8; 65535];
            while let Ok((len, src_endpoint)) = socket_rx.recv_from(&mut buf).await {
                hub.total_bytes_rx.fetch_add(len as u64, Ordering::Relaxed);
                hub.process_incoming_packet(&buf[..len], src_endpoint, &socket_rx).await;
            }
        });

        // Tâche de nettoyage des clients inactifs (Timeout après 30 secondes sans ping)
        let hub_cleanup = self.clone();
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(Duration::from_secs(10)).await;
                hub_cleanup.cleanup_inactive_clients();
            }
        });

        Ok(())
    }

    async fn process_incoming_packet(
        &self,
        buf: &[u8],
        src_endpoint: SocketAddr,
        socket: &Arc<UdpSocket>,
    ) {
        if buf.len() < 9 || &buf[0..4] != PROTOCOL_MAGIC {
            return;
        }

        // Vérification CRC32
        let payload_len = buf.len() - 4;
        let expected_crc = u32::from_be_bytes([
            buf[payload_len],
            buf[payload_len + 1],
            buf[payload_len + 2],
            buf[payload_len + 3],
        ]);
        if crc32fast::hash(&buf[..payload_len]) != expected_crc {
            return;
        }

        let ptype = buf[5];
        match ptype {
            // Handshake Request
            0x01 => {
                let name_len = buf[7] as usize;
                let client_name = String::from_utf8_lossy(&buf[8..8 + name_len]).to_string();
                let code_offset = 8 + name_len;
                let code_len = buf[code_offset] as usize;
                let room_code = String::from_utf8_lossy(&buf[code_offset + 1..code_offset + 1 + code_len]).to_string();

                self.handle_handshake(src_endpoint, client_name, room_code, socket).await;
            }
            // Ping
            0x03 => {
                // Echo Pong immédiat
                let mut pong = buf.to_vec();
                pong[5] = 0x04; // PacketType::Pong
                // Recalcul CRC32
                let pl = pong.len() - 4;
                let new_crc = crc32fast::hash(&pong[..pl]);
                pong[pl..pl + 4].copy_from_slice(&new_crc.to_be_bytes());

                self.total_bytes_tx.fetch_add(pong.len() as u64, Ordering::Relaxed);
                let _ = socket.send_to(&pong, src_endpoint).await;

                // Mise à jour de la dernière activité
                if let Some(&cid) = self.endpoint_to_id.read().get(&src_endpoint) {
                    if let Some(client) = self.clients_by_id.write().get_mut(&cid) {
                        client.last_seen_secs = current_timestamp_secs();
                    }
                }
            }
            // Pong
            0x04 => {
                if let Some(&cid) = self.endpoint_to_id.read().get(&src_endpoint) {
                    if let Some(client) = self.clients_by_id.write().get_mut(&cid) {
                        client.last_seen_secs = current_timestamp_secs();
                    }
                }
            }
            // TunnelData ou BroadcastGaming : Relais direct
            0x05 | 0x06 => {
                self.total_packets_relayed.fetch_add(1, Ordering::Relaxed);
                self.relay_room_packet(buf, src_endpoint, socket).await;
            }
            _ => {}
        }
    }

    async fn handle_handshake(
        &self,
        src_endpoint: SocketAddr,
        player_name: String,
        room_code: String,
        socket: &Arc<UdpSocket>,
    ) {
        // Attribution de l'identifiant client unique
        let client_id = self.next_client_id.fetch_add(1, Ordering::SeqCst);

        let (assigned_ip, host_name) = {
            let mut rooms = self.rooms.write();
            let room = rooms.entry(room_code.clone()).or_insert_with(|| ActiveRoom {
                room_code: room_code.clone(),
                host_id: client_id,
                host_name: player_name.clone(),
                created_at_secs: current_timestamp_secs(),
                client_count: 0,
                clients: Vec::new(),
            });

            let is_host = room.host_id == client_id;
            let suffix = (room.clients.len() as u8) + 1; // 10.244.0.1 pour hôte, .2, .3 etc.
            let assigned_ip = Ipv4Addr::new(10, 244, 0, suffix);

            let client = ConnectedClient {
                client_id,
                player_name: player_name.clone(),
                room_code: room_code.clone(),
                endpoint: src_endpoint,
                virtual_ip: assigned_ip,
                is_host,
                ping_ms: 12,
                connected_at_secs: current_timestamp_secs(),
                last_seen_secs: current_timestamp_secs(),
                bytes_tx: 0,
                bytes_rx: 0,
            };

            let host_name = room.host_name.clone();
            room.clients.push(client.clone());
            room.client_count = room.clients.len();

            self.clients_by_id.write().insert(client_id, client);
            self.endpoint_to_id.write().insert(src_endpoint, client_id);

            (assigned_ip, host_name)
        };

        info!(
            "Nouveau Client #{} '{}' connecté sur le port 30024 [Salle: {}] -> IP Virtuelle: {}",
            client_id, player_name, room_code, assigned_ip
        );

        // Construction de la réponse Handshake
        let resp = encode_handshake_response(true, assigned_ip, client_id, &host_name, "");
        self.total_bytes_tx.fetch_add(resp.len() as u64, Ordering::Relaxed);
        let _ = socket.send_to(&resp, src_endpoint).await;
    }

    async fn relay_room_packet(
        &self,
        buf: &[u8],
        src_endpoint: SocketAddr,
        socket: &Arc<UdpSocket>,
    ) {
        let cid = match self.endpoint_to_id.read().get(&src_endpoint) {
            Some(&id) => id,
            None => return,
        };

        let room_code = match self.clients_by_id.read().get(&cid) {
            Some(c) => c.room_code.clone(),
            None => return,
        };

        let target_endpoints: Vec<SocketAddr> = {
            let rooms = self.rooms.read();
            if let Some(r) = rooms.get(&room_code) {
                r.clients
                    .iter()
                    .filter(|c| c.endpoint != src_endpoint)
                    .map(|c| c.endpoint)
                    .collect()
            } else {
                Vec::new()
            }
        };

        for target in target_endpoints {
            self.total_bytes_tx.fetch_add(buf.len() as u64, Ordering::Relaxed);
            let _ = socket.send_to(buf, target).await;
        }
    }

    fn cleanup_inactive_clients(&self) {
        let now = current_timestamp_secs();
        let timeout_secs = 35;

        let mut to_remove = Vec::new();
        {
            let clients = self.clients_by_id.read();
            for (&id, client) in clients.iter() {
                if now.saturating_sub(client.last_seen_secs) > timeout_secs {
                    to_remove.push((id, client.endpoint, client.room_code.clone()));
                }
            }
        }

        for (id, ep, room_code) in to_remove {
            self.clients_by_id.write().remove(&id);
            self.endpoint_to_id.write().remove(&ep);

            let mut rooms = self.rooms.write();
            if let Some(r) = rooms.get_mut(&room_code) {
                r.clients.retain(|c| c.client_id != id);
                r.client_count = r.clients.len();
                if r.clients.is_empty() {
                    rooms.remove(&room_code);
                    info!("Salle {} fermée (aucun client actif)", room_code);
                }
            }
            debug!("Client #{} déconnecté pour inactivité", id);
        }
    }

    pub fn get_active_rooms(&self) -> Vec<ActiveRoom> {
        self.rooms.read().values().cloned().collect()
    }

    pub fn get_total_clients(&self) -> usize {
        self.clients_by_id.read().len()
    }
}

fn encode_handshake_response(
    success: bool,
    assigned_ip: Ipv4Addr,
    assigned_id: u64,
    host_name: &str,
    error_msg: &str,
) -> Vec<u8> {
    let mut buf = Vec::with_capacity(64);
    buf.extend_from_slice(PROTOCOL_MAGIC);
    buf.push(PROTOCOL_VERSION);
    buf.push(0x02); // PacketType::HandshakeResponse
    buf.push(if success { 1 } else { 0 });
    buf.extend_from_slice(&assigned_ip.octets());
    buf.extend_from_slice(&assigned_id.to_be_bytes());

    let host_bytes = host_name.as_bytes();
    buf.push(host_bytes.len().min(255) as u8);
    buf.extend_from_slice(&host_bytes[..host_bytes.len().min(255)]);

    let err_bytes = error_msg.as_bytes();
    buf.push(err_bytes.len().min(255) as u8);
    buf.extend_from_slice(&err_bytes[..err_bytes.len().min(255)]);

    let crc = crc32fast::hash(&buf);
    buf.extend_from_slice(&crc.to_be_bytes());
    buf
}

fn current_timestamp_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
