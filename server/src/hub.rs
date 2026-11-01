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

const PROTOCOL_MAGIC: &[u8; 4] = b"COOP";
const PROTOCOL_VERSION: u8 = 1;
const PKT_INSTANCE_HEARTBEAT: u8 = 0x10;
const PKT_INSTANCE_ID_ASSIGNED: u8 = 0x11;
pub const PKT_RELAY_REGISTER: u8 = 0x20;
pub const PKT_RELAY_DATA: u8 = 0x21;

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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisteredInstance {
    pub assigned_id: String,
    pub instance_uuid: u64,
    pub player_name: String,
    pub tunnel_mounted: bool,
    pub is_host: bool,
    pub room_code: String,
    pub peer_count: usize,
    pub screen_sharing: bool,
    pub version: String,
    pub last_seen_secs: u64,
}

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
    pub host_public_ip: String,
    pub host_port: u16,
    pub created_at_secs: u64,
    pub client_count: usize,
    pub clients: Vec<ConnectedClient>,
}

pub struct ServerHub {
    pub udp_port: u16,
    pub next_client_id: AtomicU64,
    pub total_tunnels_mounted_ever: AtomicU64,
    pub instances_by_uuid: Arc<RwLock<HashMap<u64, RegisteredInstance>>>,
    pub clients_by_id: Arc<RwLock<HashMap<u64, ConnectedClient>>>,
    pub endpoint_to_id: Arc<RwLock<HashMap<SocketAddr, u64>>>,
    pub rooms: Arc<RwLock<HashMap<String, ActiveRoom>>>,
    pub relay_rooms: Arc<RwLock<HashMap<String, Vec<(SocketAddr, u64)>>>>,
    pub total_bytes_tx: Arc<AtomicU64>,
    pub total_bytes_rx: Arc<AtomicU64>,
    pub total_packets_relayed: Arc<AtomicU64>,
    pub start_time: Instant,
}

impl ServerHub {
    pub fn new(udp_port: u16) -> Self {
        Self {
            udp_port,
            next_client_id: AtomicU64::new(1001), // IDs uniques commençant à CL-1001
            total_tunnels_mounted_ever: AtomicU64::new(0),
            instances_by_uuid: Arc::new(RwLock::new(HashMap::new())),
            clients_by_id: Arc::new(RwLock::new(HashMap::new())),
            endpoint_to_id: Arc::new(RwLock::new(HashMap::new())),
            rooms: Arc::new(RwLock::new(HashMap::new())),
            relay_rooms: Arc::new(RwLock::new(HashMap::new())),
            total_bytes_tx: Arc::new(AtomicU64::new(0)),
            total_bytes_rx: Arc::new(AtomicU64::new(0)),
            total_packets_relayed: Arc::new(AtomicU64::new(0)),
            start_time: Instant::now(),
        }
    }

    pub async fn start(self: Arc<Self>) -> Result<()> {
        let bind_addr = format!("0.0.0.0:{}", self.udp_port);
        let socket = Arc::new(UdpSocket::bind(&bind_addr).await?);
        info!("CoopLink Central Metrics & ID Server UDP opérationnel sur {}", bind_addr);

        let hub = self.clone();
        let socket_rx = socket.clone();

        tokio::spawn(async move {
            let mut buf = vec![0u8; 65535];
            while let Ok((len, src_endpoint)) = socket_rx.recv_from(&mut buf).await {
                hub.total_bytes_rx.fetch_add(len as u64, Ordering::Relaxed);
                hub.process_incoming_packet(&buf[..len], src_endpoint, &socket_rx).await;
            }
        });

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
            PKT_INSTANCE_HEARTBEAT => {
                if let Ok(payload) = serde_json::from_slice::<TelemetryPayload>(&buf[6..payload_len]) {
                    self.handle_instance_heartbeat(src_endpoint, payload, socket).await;
                }
            }
            PKT_RELAY_REGISTER => {
                let Some(&code_len_u8) = buf.get(6) else { return; };
                let code_len = code_len_u8 as usize;
                if 7 + code_len > payload_len { return; }
                let Some(code_slice) = buf.get(7..7 + code_len) else { return; };
                let room_code = String::from_utf8_lossy(code_slice).to_string();
                let now = current_timestamp_secs();

                let mut relay = self.relay_rooms.write();
                let endpoints = relay.entry(room_code).or_default();
                if let Some(entry) = endpoints.iter_mut().find(|(ep, _)| *ep == src_endpoint) {
                    entry.1 = now;
                } else {
                    endpoints.push((src_endpoint, now));
                }
            }
            PKT_RELAY_DATA => {
                let Some(&code_len_u8) = buf.get(6) else { return; };
                let code_len = code_len_u8 as usize;
                if 7 + code_len > payload_len { return; }
                let Some(code_slice) = buf.get(7..7 + code_len) else { return; };
                let room_code = String::from_utf8_lossy(code_slice).to_string();
                let now = current_timestamp_secs();

                let targets: Vec<SocketAddr> = {
                    let mut relay = self.relay_rooms.write();
                    let endpoints = relay.entry(room_code).or_default();
                    if let Some(entry) = endpoints.iter_mut().find(|(ep, _)| *ep == src_endpoint) {
                        entry.1 = now;
                    } else {
                        endpoints.push((src_endpoint, now));
                    }
                    endpoints
                        .iter()
                        .filter(|(ep, _)| *ep != src_endpoint)
                        .map(|(ep, _)| *ep)
                        .collect()
                };

                for target in targets {
                    self.total_packets_relayed.fetch_add(1, Ordering::Relaxed);
                    self.total_bytes_tx.fetch_add(buf.len() as u64, Ordering::Relaxed);
                    let _ = socket.send_to(buf, target).await;
                }
            }
            // Handshake de salon P2P (avec vérification stricte des bornes pour empêcher tout crash distant)
            0x01 => {
                let Some(&name_len_u8) = buf.get(7) else { return; };
                let name_len = name_len_u8 as usize;
                let Some(name_end) = 8usize.checked_add(name_len) else { return; };
                if name_end >= payload_len {
                    return;
                }
                let Some(name_slice) = buf.get(8..name_end) else { return; };
                let client_name = String::from_utf8_lossy(name_slice).to_string();

                let code_offset = name_end;
                let Some(&code_len_u8) = buf.get(code_offset) else { return; };
                let code_len = code_len_u8 as usize;
                let Some(code_start) = code_offset.checked_add(1) else { return; };
                let Some(code_end) = code_start.checked_add(code_len) else { return; };
                if code_end > payload_len {
                    return;
                }
                let Some(code_slice) = buf.get(code_start..code_end) else { return; };
                let room_code = String::from_utf8_lossy(code_slice).to_string();

                self.handle_handshake(src_endpoint, client_name, room_code, socket).await;
            }
            0x03 => {
                let mut pong = buf.to_vec();
                pong[5] = 0x04;
                let pl = pong.len() - 4;
                let new_crc = crc32fast::hash(&pong[..pl]);
                pong[pl..pl + 4].copy_from_slice(&new_crc.to_be_bytes());

                self.total_bytes_tx.fetch_add(pong.len() as u64, Ordering::Relaxed);
                let _ = socket.send_to(&pong, src_endpoint).await;

                if let Some(&cid) = self.endpoint_to_id.read().get(&src_endpoint) {
                    if let Some(client) = self.clients_by_id.write().get_mut(&cid) {
                        client.last_seen_secs = current_timestamp_secs();
                    }
                }
            }
            0x04 => {
                if let Some(&cid) = self.endpoint_to_id.read().get(&src_endpoint) {
                    if let Some(client) = self.clients_by_id.write().get_mut(&cid) {
                        client.last_seen_secs = current_timestamp_secs();
                    }
                }
            }
            _ => {}
        }
    }

    pub fn process_telemetry(&self, payload: TelemetryPayload) -> TelemetryAck {
        let now = current_timestamp_secs();
        let assigned_id = {
            let mut instances = self.instances_by_uuid.write();
            if let Some(existing) = instances.get_mut(&payload.instance_uuid) {
                if !existing.tunnel_mounted && payload.tunnel_mounted && payload.is_host {
                    self.total_tunnels_mounted_ever.fetch_add(1, Ordering::Relaxed);
                }
                existing.player_name = payload.player_name.clone();
                existing.tunnel_mounted = payload.tunnel_mounted;
                existing.is_host = payload.is_host;
                existing.room_code = payload.room_code.clone();
                existing.peer_count = payload.peer_count;
                existing.screen_sharing = payload.screen_sharing;
                existing.last_seen_secs = now;
                existing.assigned_id.clone()
            } else {
                let seq_id = self.next_client_id.fetch_add(1, Ordering::SeqCst);
                let id_str = format!("CL-{}", seq_id);
                if payload.tunnel_mounted && payload.is_host {
                    self.total_tunnels_mounted_ever.fetch_add(1, Ordering::Relaxed);
                }
                info!(
                    "Nouvelle instance CoopLink détectée -> Attribution de l'ID Unique {}",
                    id_str
                );
                instances.insert(
                    payload.instance_uuid,
                    RegisteredInstance {
                        assigned_id: id_str.clone(),
                        instance_uuid: payload.instance_uuid,
                        player_name: payload.player_name,
                        tunnel_mounted: payload.tunnel_mounted,
                        is_host: payload.is_host,
                        room_code: payload.room_code,
                        peer_count: payload.peer_count,
                        screen_sharing: payload.screen_sharing,
                        version: payload.version,
                        last_seen_secs: now,
                    },
                );
                id_str
            }
        };

        TelemetryAck {
            assigned_instance_id: assigned_id,
            active_tunnels: self.get_active_tunnels_count(),
            active_instances: self.get_active_instances_count(),
        }
    }

    async fn handle_instance_heartbeat(
        &self,
        src_endpoint: SocketAddr,
        payload: TelemetryPayload,
        socket: &Arc<UdpSocket>,
    ) {
        let ack = self.process_telemetry(payload);

        if let Ok(json_bytes) = serde_json::to_vec(&ack) {
            let mut pkt = Vec::with_capacity(10 + json_bytes.len());
            pkt.extend_from_slice(PROTOCOL_MAGIC);
            pkt.push(PROTOCOL_VERSION);
            pkt.push(PKT_INSTANCE_ID_ASSIGNED);
            pkt.extend_from_slice(&json_bytes);
            let crc = crc32fast::hash(&pkt);
            pkt.extend_from_slice(&crc.to_be_bytes());
            self.total_bytes_tx.fetch_add(pkt.len() as u64, Ordering::Relaxed);
            let _ = socket.send_to(&pkt, src_endpoint).await;
        }
    }

    async fn handle_handshake(
        &self,
        src_endpoint: SocketAddr,
        player_name: String,
        room_code: String,
        socket: &Arc<UdpSocket>,
    ) {
        let client_id = self.next_client_id.fetch_add(1, Ordering::SeqCst);

        let (assigned_ip, host_name, room_peers) = {
            let mut rooms = self.rooms.write();
            let is_new_room = !rooms.contains_key(&room_code);
            let room = rooms.entry(room_code.clone()).or_insert_with(|| ActiveRoom {
                room_code: room_code.clone(),
                host_id: client_id,
                host_name: player_name.clone(),
                host_public_ip: src_endpoint.ip().to_string(),
                host_port: src_endpoint.port(),
                created_at_secs: current_timestamp_secs(),
                client_count: 0,
                clients: Vec::new(),
            });

            if is_new_room {
                self.total_tunnels_mounted_ever.fetch_add(1, Ordering::Relaxed);
            }

            let is_host = room.host_id == client_id;
            let suffix = (room.clients.len() as u8) + 1;
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
            let peers_snapshot = room.clients.clone();

            self.clients_by_id.write().insert(client_id, client);
            self.endpoint_to_id.write().insert(src_endpoint, client_id);

            (assigned_ip, host_name, peers_snapshot)
        };

        let resp = encode_handshake_response(true, assigned_ip, client_id, &host_name, "");
        self.total_bytes_tx.fetch_add(resp.len() as u64, Ordering::Relaxed);
        let _ = socket.send_to(&resp, src_endpoint).await;

        self.broadcast_peer_list_for_direct_p2p(&room_peers, socket).await;
    }

    async fn broadcast_peer_list_for_direct_p2p(
        &self,
        peers: &[ConnectedClient],
        socket: &Arc<UdpSocket>,
    ) {
        if let Ok(json_bytes) = serde_json::to_vec(peers) {
            let mut pkt = Vec::with_capacity(10 + json_bytes.len());
            pkt.extend_from_slice(PROTOCOL_MAGIC);
            pkt.push(PROTOCOL_VERSION);
            pkt.push(0x07);
            pkt.extend_from_slice(&(json_bytes.len() as u16).to_be_bytes());
            pkt.extend_from_slice(&json_bytes);
            let crc = crc32fast::hash(&pkt);
            pkt.extend_from_slice(&crc.to_be_bytes());

            self.total_packets_relayed.fetch_add(1, Ordering::Relaxed);
            for peer in peers {
                self.total_bytes_tx.fetch_add(pkt.len() as u64, Ordering::Relaxed);
                let _ = socket.send_to(&pkt, peer.endpoint).await;
            }
        }
    }

    fn cleanup_inactive_clients(&self) {
        let now = current_timestamp_secs();
        let timeout_secs = 25;

        // Nettoyage des instances inactives
        self.instances_by_uuid
            .write()
            .retain(|_, inst| now.saturating_sub(inst.last_seen_secs) <= timeout_secs);

        self.relay_rooms.write().retain(|_, eps| {
            eps.retain(|(_, last)| now.saturating_sub(*last) <= 120);
            !eps.is_empty()
        });

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
                }
            }
            debug!("Client #{} déconnecté pour inactivité", id);
        }
    }

    pub fn get_active_instances(&self) -> Vec<RegisteredInstance> {
        let mut list: Vec<RegisteredInstance> = self.instances_by_uuid.read().values().cloned().collect();
        list.sort_by(|a, b| a.assigned_id.cmp(&b.assigned_id));
        list
    }

    pub fn get_active_instances_count(&self) -> usize {
        self.instances_by_uuid.read().len().max(self.clients_by_id.read().len())
    }

    pub fn get_active_tunnels_count(&self) -> usize {
        let from_instances = self
            .instances_by_uuid
            .read()
            .values()
            .filter(|i| i.tunnel_mounted && i.is_host)
            .count();
        from_instances.max(self.rooms.read().len())
    }

    pub fn get_active_screen_shares_count(&self) -> usize {
        self.instances_by_uuid
            .read()
            .values()
            .filter(|i| i.screen_sharing)
            .count()
    }

    pub fn get_total_ids_issued(&self) -> u64 {
        self.next_client_id.load(Ordering::Relaxed).saturating_sub(1001)
    }

    pub fn register_web_room(&self, room_code: &str, host_name: &str, public_ip: &str, port: u16) {
        let mut rooms = self.rooms.write();
        let client_id = self.next_client_id.fetch_add(1, Ordering::SeqCst);
        let room = rooms.entry(room_code.to_string()).or_insert_with(|| ActiveRoom {
            room_code: room_code.to_string(),
            host_id: client_id,
            host_name: host_name.to_string(),
            host_public_ip: public_ip.to_string(),
            host_port: port,
            created_at_secs: current_timestamp_secs(),
            client_count: 1,
            clients: Vec::new(),
        });
        room.host_public_ip = public_ip.to_string();
        room.host_port = port;
        room.host_name = host_name.to_string();
        self.total_tunnels_mounted_ever.fetch_add(1, Ordering::Relaxed);
    }

    pub fn lookup_room(&self, room_code: &str) -> Option<ActiveRoom> {
        self.rooms.read().get(room_code).cloned()
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
    buf.push(0x02);
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
