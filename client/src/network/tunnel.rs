use crate::network::protocol::{CoopPacket, PROTOCOL_VERSION};
use crate::types::{NodeRole, PeerInfo};
use anyhow::Result;
use parking_lot::RwLock;
use std::collections::HashMap;
use std::net::{Ipv4Addr, SocketAddr};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::net::UdpSocket;
use tokio::sync::mpsc;
use tracing::{debug, error, info};

pub struct NetworkTunnel {
    pub running: Arc<AtomicBool>,
    pub role: Arc<RwLock<NodeRole>>,
    pub my_id: u64,
    pub my_virtual_ip: Arc<RwLock<Option<Ipv4Addr>>>,
    pub peers: Arc<RwLock<HashMap<u64, PeerInfo>>>,
    pub endpoint_to_id: Arc<RwLock<HashMap<SocketAddr, u64>>>,
    pub ip_to_id: Arc<RwLock<HashMap<Ipv4Addr, u64>>>,
    pub bytes_tx: Arc<AtomicU64>,
    pub bytes_rx: Arc<AtomicU64>,
}

impl NetworkTunnel {
    pub fn new() -> Self {
        let my_id = rand::random::<u64>();
        Self {
            running: Arc::new(AtomicBool::new(false)),
            role: Arc::new(RwLock::new(NodeRole::Idle)),
            my_id,
            my_virtual_ip: Arc::new(RwLock::new(None)),
            peers: Arc::new(RwLock::new(HashMap::new())),
            endpoint_to_id: Arc::new(RwLock::new(HashMap::new())),
            ip_to_id: Arc::new(RwLock::new(HashMap::new())),
            bytes_tx: Arc::new(AtomicU64::new(0)),
            bytes_rx: Arc::new(AtomicU64::new(0)),
        }
    }

    /// Démarre en mode Hôte (Room Master / Nœud Maître)
    pub async fn start_host(
        &self,
        port: u16,
        player_name: String,
        room_code: String,
        host_virtual_ip: Ipv4Addr,
        to_adapter_tx: mpsc::Sender<Vec<u8>>,
        mut from_adapter_rx: mpsc::Receiver<Vec<u8>>,
        to_bcast_tx: mpsc::Sender<(u16, Vec<u8>)>,
        mut from_bcast_rx: mpsc::Receiver<(u16, Vec<u8>)>,
    ) -> Result<()> {
        *self.role.write() = NodeRole::Host;
        *self.my_virtual_ip.write() = Some(host_virtual_ip);
        self.running.store(true, Ordering::SeqCst);

        let bind_addr = format!("0.0.0.0:{}", port);
        let socket = Arc::new(UdpSocket::bind(&bind_addr).await?);
        info!("CoopLink Nœud Maître démarré sur UDP {}", bind_addr);

        // Ajoute l'hôte lui-même à la liste des pairs
        {
            let mut peers = self.peers.write();
            peers.insert(
                self.my_id,
                PeerInfo {
                    id: self.my_id,
                    name: format!("{} (Hôte)", player_name),
                    virtual_ip: host_virtual_ip,
                    endpoint: socket.local_addr()?,
                    ping_ms: 0,
                    last_seen_secs: 0,
                    is_host: true,
                    bytes_sent: 0,
                    bytes_recv: 0,
                },
            );
        }

        let running_flag = self.running.clone();
        let socket_rx = socket.clone();
        let peers_map = self.peers.clone();
        let ep_map = self.endpoint_to_id.clone();
        let ip_map = self.ip_to_id.clone();
        let rx_counter = self.bytes_rx.clone();
        let tx_counter = self.bytes_tx.clone();

        // Tâche de réception UDP principale
        tokio::spawn(async move {
            let mut buf = vec![0u8; 65535];
            let mut next_client_ip_suffix: u8 = 2; // Host = .1, Clients = .2, .3, .4 ...

            while running_flag.load(Ordering::Relaxed) {
                match socket_rx.recv_from(&mut buf).await {
                    Ok((len, src_endpoint)) => {
                        rx_counter.fetch_add(len as u64, Ordering::Relaxed);
                        if let Ok(packet) = CoopPacket::decode(&buf[..len]) {
                            match packet {
                                CoopPacket::HandshakeRequest {
                                    player_name: client_name,
                                    room_code: req_room_code,
                                    ..
                                } => {
                                    if req_room_code != room_code {
                                        let resp = CoopPacket::HandshakeResponse {
                                            success: false,
                                            assigned_ip: Ipv4Addr::UNSPECIFIED,
                                            assigned_id: 0,
                                            host_name: player_name.clone(),
                                            error_msg: "Code de salle incorrect".to_string(),
                                        };
                                        let encoded = resp.encode();
                                        let _ = socket_rx.send_to(&encoded, src_endpoint).await;
                                        continue;
                                    }

                                    let new_id = rand::random::<u64>();
                                    let assigned_ip = Ipv4Addr::new(
                                        host_virtual_ip.octets()[0],
                                        host_virtual_ip.octets()[1],
                                        host_virtual_ip.octets()[2],
                                        next_client_ip_suffix,
                                    );
                                    next_client_ip_suffix = next_client_ip_suffix.saturating_add(1);

                                    let peer = PeerInfo {
                                        id: new_id,
                                        name: client_name.clone(),
                                        virtual_ip: assigned_ip,
                                        endpoint: src_endpoint,
                                        ping_ms: 10,
                                        last_seen_secs: current_timestamp_secs(),
                                        is_host: false,
                                        bytes_sent: 0,
                                        bytes_recv: len as u64,
                                    };

                                    peers_map.write().insert(new_id, peer.clone());
                                    ep_map.write().insert(src_endpoint, new_id);
                                    ip_map.write().insert(assigned_ip, new_id);

                                    info!(
                                        "Nouveau client connecté au salon: {} [{}] assigné à {}",
                                        client_name, src_endpoint, assigned_ip
                                    );

                                    let resp = CoopPacket::HandshakeResponse {
                                        success: true,
                                        assigned_ip,
                                        assigned_id: new_id,
                                        host_name: player_name.clone(),
                                        error_msg: String::new(),
                                    };
                                    let encoded = resp.encode();
                                    tx_counter.fetch_add(encoded.len() as u64, Ordering::Relaxed);
                                    let _ = socket_rx.send_to(&encoded, src_endpoint).await;

                                    // Diffuse la mise à jour de la liste des pairs à tous les clients
                                    let peer_list_json = serde_json::to_string(&*peers_map.read()).unwrap_or_default();
                                    let update_pkt = CoopPacket::PeerListUpdate {
                                        raw_json: peer_list_json,
                                    }
                                    .encode();

                                    let client_endpoints: Vec<SocketAddr> = peers_map
                                        .read()
                                        .values()
                                        .filter(|p| !p.is_host)
                                        .map(|p| p.endpoint)
                                        .collect();

                                    for ep in client_endpoints {
                                        let _ = socket_rx.send_to(&update_pkt, ep).await;
                                    }
                                }
                                CoopPacket::Ping { seq, timestamp_ms } => {
                                    let pong = CoopPacket::Pong { seq, timestamp_ms }.encode();
                                    let _ = socket_rx.send_to(&pong, src_endpoint).await;
                                }
                                CoopPacket::Pong { timestamp_ms, .. } => {
                                    let now_ms = current_timestamp_ms();
                                    let rtt = (now_ms.saturating_sub(timestamp_ms)).min(999) as u32;
                                    if let Some(&peer_id) = ep_map.read().get(&src_endpoint) {
                                        if let Some(peer) = peers_map.write().get_mut(&peer_id) {
                                            peer.ping_ms = rtt;
                                            peer.last_seen_secs = current_timestamp_secs();
                                        }
                                    }
                                }
                                CoopPacket::TunnelData {
                                    src_ip,
                                    dst_ip,
                                    payload,
                                } => {
                                    if dst_ip == host_virtual_ip {
                                        let _ = to_adapter_tx.send(payload).await;
                                    } else {
                                        let target_ep = {
                                            let ip_m = ip_map.read();
                                            let p_m = peers_map.read();
                                            ip_m.get(&dst_ip)
                                                .and_then(|tid| p_m.get(tid))
                                                .map(|p| p.endpoint)
                                        };

                                        if let Some(ep) = target_ep {
                                            let fwd = CoopPacket::TunnelData {
                                                src_ip,
                                                dst_ip,
                                                payload,
                                            }
                                            .encode();
                                            let _ = socket_rx.send_to(&fwd, ep).await;
                                        }
                                    }
                                }
                                CoopPacket::BroadcastGaming {
                                    src_ip,
                                    port,
                                    payload,
                                } => {
                                    // Relaye à l'adaptateur local
                                    let _ = to_bcast_tx.send((port, payload.clone())).await;

                                    // Relaye à tous les autres pairs du salon
                                    let bcast_pkt = CoopPacket::BroadcastGaming {
                                        src_ip,
                                        port,
                                        payload,
                                    }
                                    .encode();

                                    let other_endpoints: Vec<SocketAddr> = peers_map
                                        .read()
                                        .values()
                                        .filter(|p| !p.is_host && p.endpoint != src_endpoint)
                                        .map(|p| p.endpoint)
                                        .collect();

                                    for ep in other_endpoints {
                                        let _ = socket_rx.send_to(&bcast_pkt, ep).await;
                                    }
                                }
                                CoopPacket::Disconnect { peer_id, .. } => {
                                    peers_map.write().remove(&peer_id);
                                    info!("Client {} s'est déconnecté du salon", peer_id);
                                }
                                _ => {}
                            }
                        }
                    }
                    Err(e) => {
                        debug!("Erreur de lecture socket: {}", e);
                        tokio::time::sleep(Duration::from_millis(50)).await;
                    }
                }
            }
        });

        // Tâche de relais depuis l'adaptateur local vers les clients du nœud
        let running_fwd = self.running.clone();
        let socket_tx_data = socket.clone();
        let ip_map_fwd = self.ip_to_id.clone();
        let peers_fwd = self.peers.clone();
        let my_vip = host_virtual_ip;

        tokio::spawn(async move {
            while running_fwd.load(Ordering::Relaxed) {
                if let Some(data) = from_adapter_rx.recv().await {
                    if data.len() >= 20 {
                        // Décode l'IP de destination du paquet IPv4 (octets 16..20)
                        let dst_ip = Ipv4Addr::new(data[16], data[17], data[18], data[19]);
                        if dst_ip == Ipv4Addr::BROADCAST || dst_ip.octets()[3] == 255 {
                            // Paquet Broadcast IPv4
                            let bcast_pkt = CoopPacket::BroadcastGaming {
                                src_ip: my_vip,
                                port: 0,
                                payload: data,
                            }
                            .encode();

                            let client_eps: Vec<SocketAddr> = peers_fwd
                                .read()
                                .values()
                                .filter(|p| !p.is_host)
                                .map(|p| p.endpoint)
                                .collect();

                            for ep in client_eps {
                                let _ = socket_tx_data.send_to(&bcast_pkt, ep).await;
                            }
                        } else {
                            let target_ep = {
                                let ip_m = ip_map_fwd.read();
                                let p_m = peers_fwd.read();
                                ip_m.get(&dst_ip)
                                    .and_then(|tid| p_m.get(tid))
                                    .map(|p| p.endpoint)
                            };

                            if let Some(ep) = target_ep {
                                let fwd = CoopPacket::TunnelData {
                                    src_ip: my_vip,
                                    dst_ip,
                                    payload: data,
                                }
                                .encode();
                                let _ = socket_tx_data.send_to(&fwd, ep).await;
                            }
                        }
                    }
                } else {
                    break;
                }
            }
        });

        // Tâche de broadcast local émis vers le salon
        let running_bcast = self.running.clone();
        let socket_bcast = socket.clone();
        let peers_bcast = self.peers.clone();

        tokio::spawn(async move {
            while running_bcast.load(Ordering::Relaxed) {
                if let Some((port, payload)) = from_bcast_rx.recv().await {
                    let pkt = CoopPacket::BroadcastGaming {
                        src_ip: host_virtual_ip,
                        port,
                        payload,
                    }
                    .encode();

                    let client_eps: Vec<SocketAddr> = peers_bcast
                        .read()
                        .values()
                        .filter(|p| !p.is_host)
                        .map(|p| p.endpoint)
                        .collect();

                    for ep in client_eps {
                        let _ = socket_bcast.send_to(&pkt, ep).await;
                    }
                } else {
                    break;
                }
            }
        });

        // Tâche périodique de Ping / Keepalive vers tous les clients
        let running_ping = self.running.clone();
        let socket_ping = socket.clone();
        let peers_ping = self.peers.clone();

        tokio::spawn(async move {
            let mut seq = 0u32;
            while running_ping.load(Ordering::Relaxed) {
                tokio::time::sleep(Duration::from_millis(1500)).await;
                seq = seq.wrapping_add(1);
                let now_ms = current_timestamp_ms();
                let ping_pkt = CoopPacket::Ping {
                    seq,
                    timestamp_ms: now_ms,
                }
                .encode();

                let client_eps: Vec<SocketAddr> = peers_ping
                    .read()
                    .values()
                    .filter(|p| !p.is_host)
                    .map(|p| p.endpoint)
                    .collect();

                for ep in client_eps {
                    let _ = socket_ping.send_to(&ping_pkt, ep).await;
                }
            }
        });

        Ok(())
    }

    /// Démarre en mode Client (Rejoindre un salon / Nœud distant)
    pub async fn start_client(
        &self,
        server_addr: SocketAddr,
        player_name: String,
        room_code: String,
        to_adapter_tx: mpsc::Sender<Vec<u8>>,
        mut from_adapter_rx: mpsc::Receiver<Vec<u8>>,
        to_bcast_tx: mpsc::Sender<(u16, Vec<u8>)>,
        mut from_bcast_rx: mpsc::Receiver<(u16, Vec<u8>)>,
    ) -> Result<()> {
        *self.role.write() = NodeRole::Client;
        self.running.store(true, Ordering::SeqCst);

        let socket = Arc::new(UdpSocket::bind("0.0.0.0:0").await?);
        info!("CoopLink Client connecté via {}", socket.local_addr()?);

        // Envoi du Handshake Request
        let req = CoopPacket::HandshakeRequest {
            player_name: player_name.clone(),
            room_code: room_code.clone(),
            client_version: PROTOCOL_VERSION,
        }
        .encode();
        socket.send_to(&req, server_addr).await?;

        let running_flag = self.running.clone();
        let socket_rx = socket.clone();
        let peers_map = self.peers.clone();
        let my_vip_holder = self.my_virtual_ip.clone();
        let rx_counter = self.bytes_rx.clone();

        // Tâche de réception client
        tokio::spawn(async move {
            let mut buf = vec![0u8; 65535];
            while running_flag.load(Ordering::Relaxed) {
                match socket_rx.recv_from(&mut buf).await {
                    Ok((len, src_endpoint)) => {
                        rx_counter.fetch_add(len as u64, Ordering::Relaxed);
                        if let Ok(packet) = CoopPacket::decode(&buf[..len]) {
                            match packet {
                                CoopPacket::HandshakeResponse {
                                    success,
                                    assigned_ip,
                                    assigned_id: _,
                                    host_name,
                                    error_msg,
                                } => {
                                    if success {
                                        *my_vip_holder.write() = Some(assigned_ip);
                                        info!(
                                            "Salon rejoint avec succès ! IP Virtuelle attribuée: {} par l'Hôte {}",
                                            assigned_ip, host_name
                                        );
                                        let host_peer = PeerInfo {
                                            id: 1,
                                            name: host_name,
                                            virtual_ip: Ipv4Addr::new(
                                                assigned_ip.octets()[0],
                                                assigned_ip.octets()[1],
                                                assigned_ip.octets()[2],
                                                1,
                                            ),
                                            endpoint: src_endpoint,
                                            ping_ms: 20,
                                            last_seen_secs: current_timestamp_secs(),
                                            is_host: true,
                                            bytes_sent: 0,
                                            bytes_recv: len as u64,
                                        };
                                        peers_map.write().insert(1, host_peer);
                                    } else {
                                        error!("Échec de connexion au salon: {}", error_msg);
                                    }
                                }
                                CoopPacket::Ping { seq, timestamp_ms } => {
                                    let pong = CoopPacket::Pong { seq, timestamp_ms }.encode();
                                    let _ = socket_rx.send_to(&pong, src_endpoint).await;
                                }
                                CoopPacket::Pong { timestamp_ms, .. } => {
                                    let now_ms = current_timestamp_ms();
                                    let rtt = (now_ms.saturating_sub(timestamp_ms)).min(999) as u32;
                                    if let Some(host_peer) = peers_map.write().get_mut(&1) {
                                        host_peer.ping_ms = rtt;
                                        host_peer.last_seen_secs = current_timestamp_secs();
                                    }
                                }
                                CoopPacket::TunnelData { payload, .. } => {
                                    let _ = to_adapter_tx.send(payload).await;
                                }
                                CoopPacket::BroadcastGaming { port, payload, .. } => {
                                    let _ = to_bcast_tx.send((port, payload)).await;
                                }
                                CoopPacket::PeerListUpdate { raw_json } => {
                                    if let Ok(updated_peers) =
                                        serde_json::from_str::<HashMap<u64, PeerInfo>>(&raw_json)
                                    {
                                        *peers_map.write() = updated_peers;
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
                    Err(e) => {
                        debug!("Erreur de lecture client: {}", e);
                        tokio::time::sleep(Duration::from_millis(50)).await;
                    }
                }
            }
        });

        // Envoi des paquets IP locaux vers le serveur
        let running_fwd = self.running.clone();
        let socket_fwd = socket.clone();
        let my_vip_fwd = self.my_virtual_ip.clone();

        tokio::spawn(async move {
            while running_fwd.load(Ordering::Relaxed) {
                if let Some(data) = from_adapter_rx.recv().await {
                    let maybe_vip = *my_vip_fwd.read();
                    if let Some(vip) = maybe_vip {
                        let dst_ip = if data.len() >= 20 {
                            Ipv4Addr::new(data[16], data[17], data[18], data[19])
                        } else {
                            Ipv4Addr::BROADCAST
                        };

                        let pkt = CoopPacket::TunnelData {
                            src_ip: vip,
                            dst_ip,
                            payload: data,
                        }
                        .encode();
                        let _ = socket_fwd.send_to(&pkt, server_addr).await;
                    }
                } else {
                    break;
                }
            }
        });

        // Relais des broadcasts de jeux émis localement vers le serveur
        let running_bcast = self.running.clone();
        let socket_bcast = socket.clone();
        let my_vip_bcast = self.my_virtual_ip.clone();

        tokio::spawn(async move {
            while running_bcast.load(Ordering::Relaxed) {
                if let Some((port, payload)) = from_bcast_rx.recv().await {
                    let maybe_vip = *my_vip_bcast.read();
                    if let Some(vip) = maybe_vip {
                        let pkt = CoopPacket::BroadcastGaming {
                            src_ip: vip,
                            port,
                            payload,
                        }
                        .encode();
                        let _ = socket_bcast.send_to(&pkt, server_addr).await;
                    }
                } else {
                    break;
                }
            }
        });

        // Ping périodique client -> serveur
        let running_ping = self.running.clone();
        let socket_ping = socket.clone();

        tokio::spawn(async move {
            let mut seq = 0u32;
            while running_ping.load(Ordering::Relaxed) {
                tokio::time::sleep(Duration::from_millis(1500)).await;
                seq = seq.wrapping_add(1);
                let now_ms = current_timestamp_ms();
                let ping_pkt = CoopPacket::Ping {
                    seq,
                    timestamp_ms: now_ms,
                }
                .encode();
                let _ = socket_ping.send_to(&ping_pkt, server_addr).await;
            }
        });

        Ok(())
    }

    pub fn stop(&self) {
        self.running.store(false, Ordering::SeqCst);
        *self.role.write() = NodeRole::Idle;
        *self.my_virtual_ip.write() = None;
        self.peers.write().clear();
        self.endpoint_to_id.write().clear();
        self.ip_to_id.write().clear();
    }
}

fn current_timestamp_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

fn current_timestamp_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
