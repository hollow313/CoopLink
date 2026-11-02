use crate::network::protocol::{
    compute_handshake_auth_tag, constant_time_eq, derive_session_key, CoopPacket, PROTOCOL_VERSION,
};
use crate::types::{ConnectionStatus, NodeRole, PeerInfo};
use anyhow::Result;
use parking_lot::RwLock;
use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::net::UdpSocket;
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};

const MAX_AUTH_FAILURES_PER_WINDOW: u32 = 5;
const AUTH_RATE_LIMIT_WINDOW_SECS: u64 = 30;

pub struct NetworkTunnel {
    pub running: Arc<AtomicBool>,
    pub role: Arc<RwLock<NodeRole>>,
    pub conn_status: Arc<RwLock<ConnectionStatus>>,
    pub last_error: Arc<RwLock<Option<String>>>,
    pub my_id: u64,
    pub my_virtual_ip: Arc<RwLock<Option<Ipv4Addr>>>,
    pub peers: Arc<RwLock<HashMap<u64, PeerInfo>>>,
    pub endpoint_to_id: Arc<RwLock<HashMap<SocketAddr, u64>>>,
    pub ip_to_id: Arc<RwLock<HashMap<Ipv4Addr, u64>>>,
    pub bytes_tx: Arc<AtomicU64>,
    pub bytes_rx: Arc<AtomicU64>,
    pub host_socket: Arc<RwLock<Option<Arc<UdpSocket>>>>,
    pub active_session_key: Arc<RwLock<Option<[u8; 32]>>>,
}

impl NetworkTunnel {
    pub fn new() -> Self {
        let my_id = rand::random::<u64>();
        Self {
            running: Arc::new(AtomicBool::new(false)),
            role: Arc::new(RwLock::new(NodeRole::Idle)),
            conn_status: Arc::new(RwLock::new(ConnectionStatus::Disconnected)),
            last_error: Arc::new(RwLock::new(None)),
            my_id,
            my_virtual_ip: Arc::new(RwLock::new(None)),
            peers: Arc::new(RwLock::new(HashMap::new())),
            endpoint_to_id: Arc::new(RwLock::new(HashMap::new())),
            ip_to_id: Arc::new(RwLock::new(HashMap::new())),
            bytes_tx: Arc::new(AtomicU64::new(0)),
            bytes_rx: Arc::new(AtomicU64::new(0)),
            host_socket: Arc::new(RwLock::new(None)),
            active_session_key: Arc::new(RwLock::new(None)),
        }
    }

    /// Démarre en mode Hôte (Room Master / Nœud Maître) avec chiffrement AEAD ChaCha20-Poly1305
    #[allow(clippy::too_many_arguments)]
    pub async fn start_host(
        &self,
        port: u16,
        player_name: String,
        room_code: String,
        room_password: String,
        host_virtual_ip: Ipv4Addr,
        to_adapter_tx: mpsc::Sender<Vec<u8>>,
        mut from_adapter_rx: mpsc::Receiver<Vec<u8>>,
        to_bcast_tx: mpsc::Sender<(u16, Vec<u8>)>,
        mut from_bcast_rx: mpsc::Receiver<(u16, Vec<u8>)>,
    ) -> Result<()> {
        *self.role.write() = NodeRole::Host;
        *self.conn_status.write() = ConnectionStatus::Hosting;
        *self.last_error.write() = None;
        *self.my_virtual_ip.write() = Some(host_virtual_ip);
        self.running.store(true, Ordering::SeqCst);

        let session_key = derive_session_key(&room_code, &room_password);

        // Dual-stack: an IPv6 socket also accepts IPv4-mapped peers on Windows, which
        // keeps the tunnel usable when the public IPv4 is shared (CGNAT).
        let bind_addr = format!("[::]:{}", port);
        let socket = Arc::new(match UdpSocket::bind(&bind_addr).await {
            Ok(s) => s,
            Err(_) => {
                let fallback = format!("0.0.0.0:{}", port);
                let s = UdpSocket::bind(&fallback).await?;
                info!("Tunnel UDP en IPv4 seul ({} indisponible)", bind_addr);
                s
            }
        });
        *self.host_socket.write() = Some(socket.clone());
        *self.active_session_key.write() = Some(session_key);
        info!(
            "CoopLink Nœud Maître AEAD démarré sur UDP {} (Chiffrement ChaCha20-Poly1305 actif)",
            bind_addr
        );

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
                    last_seen_secs: current_timestamp_secs(),
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

        // Tâche de réception UDP principale (avec anti-spoofing, rate-limiting et AEAD)
        tokio::spawn(async move {
            let mut buf = vec![0u8; 65535];
            let mut next_client_ip_suffix: u8 = 2; // Host = .1, Clients = .2, .3, .4 ...
            let mut auth_failures: HashMap<IpAddr, (u32, u64)> = HashMap::new();

            while running_flag.load(Ordering::Relaxed) {
                match socket_rx.recv_from(&mut buf).await {
                    Ok((len, src_endpoint)) => {
                        rx_counter.fetch_add(len as u64, Ordering::Relaxed);
                        let now_secs = current_timestamp_secs();
                        let src_ip_addr = src_endpoint.ip();

                        // Vérification du Rate-Limiting anti brute-force par IP
                        if let Some((fails, window_start)) = auth_failures.get_mut(&src_ip_addr) {
                            if now_secs.saturating_sub(*window_start) > AUTH_RATE_LIMIT_WINDOW_SECS {
                                *fails = 0;
                                *window_start = now_secs;
                            } else if *fails >= MAX_AUTH_FAILURES_PER_WINDOW {
                                debug!("Paquet ignoré depuis {} (rate-limit anti brute-force actif)", src_ip_addr);
                                continue;
                            }
                        }

                        let decoded = CoopPacket::decode_encrypted(&buf[..len], &session_key);
                        let packet = match decoded {
                            Ok(pkt) => pkt,
                            Err(_) => {
                                let entry = auth_failures.entry(src_ip_addr).or_insert((0, now_secs));
                                entry.0 = entry.0.saturating_add(1);
                                continue;
                            }
                        };

                        match packet {
                            CoopPacket::HandshakeRequest {
                                player_name: client_name,
                                room_code: req_auth_hex,
                                ..
                            } => {
                                let expected_tag = compute_handshake_auth_tag(
                                    &room_code,
                                    &room_password,
                                    &client_name,
                                );
                                let expected_hex = hex_encode(&expected_tag);

                                if !constant_time_eq(req_auth_hex.as_bytes(), expected_hex.as_bytes()) {
                                    let entry = auth_failures.entry(src_ip_addr).or_insert((0, now_secs));
                                    entry.0 = entry.0.saturating_add(1);
                                    warn!(
                                        "Tentative de Handshake avec code/mot de passe invalide depuis {}",
                                        src_endpoint
                                    );
                                    let resp = CoopPacket::HandshakeResponse {
                                        success: false,
                                        assigned_ip: Ipv4Addr::UNSPECIFIED,
                                        assigned_id: 0,
                                        host_name: player_name.clone(),
                                        error_msg: "Code de salon ou mot de passe incorrect".to_string(),
                                    };
                                    let encoded = resp.encode_encrypted(&session_key);
                                    let _ = socket_rx.send_to(&encoded, src_endpoint).await;
                                    continue;
                                }

                                // Si ce client était déjà enregistré sur ce même endpoint, réutiliser son IP/ID
                                let (assigned_id, assigned_ip) =
                                    if let Some(&existing_id) = ep_map.read().get(&src_endpoint) {
                                        if let Some(existing_peer) = peers_map.read().get(&existing_id) {
                                            (existing_id, existing_peer.virtual_ip)
                                        } else {
                                            let nid = rand::random::<u64>();
                                            let ip = Ipv4Addr::new(
                                                host_virtual_ip.octets()[0],
                                                host_virtual_ip.octets()[1],
                                                host_virtual_ip.octets()[2],
                                                next_client_ip_suffix,
                                            );
                                            next_client_ip_suffix = next_client_ip_suffix.saturating_add(1);
                                            (nid, ip)
                                        }
                                    } else {
                                        let nid = rand::random::<u64>();
                                        let ip = Ipv4Addr::new(
                                            host_virtual_ip.octets()[0],
                                            host_virtual_ip.octets()[1],
                                            host_virtual_ip.octets()[2],
                                            next_client_ip_suffix,
                                        );
                                        next_client_ip_suffix = next_client_ip_suffix.saturating_add(1);
                                        (nid, ip)
                                    };

                                let peer = PeerInfo {
                                    id: assigned_id,
                                    name: client_name.clone(),
                                    virtual_ip: assigned_ip,
                                    endpoint: src_endpoint,
                                    ping_ms: 10,
                                    last_seen_secs: now_secs,
                                    is_host: false,
                                    bytes_sent: 0,
                                    bytes_recv: len as u64,
                                };

                                peers_map.write().insert(assigned_id, peer);
                                ep_map.write().insert(src_endpoint, assigned_id);
                                ip_map.write().insert(assigned_ip, assigned_id);

                                info!(
                                    "Client authentifié connecté au salon: {} [{}] -> {}",
                                    client_name, src_endpoint, assigned_ip
                                );

                                let resp = CoopPacket::HandshakeResponse {
                                    success: true,
                                    assigned_ip,
                                    assigned_id,
                                    host_name: player_name.clone(),
                                    error_msg: String::new(),
                                };
                                let encoded = resp.encode_encrypted(&session_key);
                                tx_counter.fetch_add(encoded.len() as u64, Ordering::Relaxed);
                                let _ = socket_rx.send_to(&encoded, src_endpoint).await;

                                // Diffuse la mise à jour de la liste des pairs à tous les clients authentifiés
                                let peer_list_json =
                                    serde_json::to_string(&*peers_map.read()).unwrap_or_default();
                                let update_pkt = CoopPacket::PeerListUpdate {
                                    raw_json: peer_list_json,
                                }
                                .encode_encrypted(&session_key);

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
                                // Ne répond qu'aux pairs authentifiés
                                if ep_map.read().contains_key(&src_endpoint) {
                                    let pong = CoopPacket::Pong { seq, timestamp_ms }
                                        .encode_encrypted(&session_key);
                                    let _ = socket_rx.send_to(&pong, src_endpoint).await;
                                }
                            }
                            CoopPacket::Pong { timestamp_ms, .. } => {
                                let now_ms = current_timestamp_ms();
                                let rtt = (now_ms.saturating_sub(timestamp_ms)).min(999) as u32;
                                if let Some(&peer_id) = ep_map.read().get(&src_endpoint) {
                                    if let Some(peer) = peers_map.write().get_mut(&peer_id) {
                                        peer.ping_ms = rtt;
                                        peer.last_seen_secs = now_secs;
                                    }
                                }
                            }
                            CoopPacket::TunnelData {
                                src_ip,
                                dst_ip,
                                payload,
                            } => {
                                // Vérification anti-usurpation d'IP (src_ip doit correspondre au pair enregistré pour src_endpoint)
                                let authorized = {
                                    let ep_m = ep_map.read();
                                    let p_m = peers_map.read();
                                    ep_m.get(&src_endpoint)
                                        .and_then(|pid| p_m.get(pid))
                                        .map(|peer| peer.virtual_ip == src_ip)
                                        .unwrap_or(false)
                                };
                                if !authorized {
                                    warn!(
                                        "Tentative d'usurpation IP (TunnelData src_ip={}) rejetée depuis {}",
                                        src_ip, src_endpoint
                                    );
                                    continue;
                                }

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
                                        .encode_encrypted(&session_key);
                                        let _ = socket_rx.send_to(&fwd, ep).await;
                                    }
                                }
                            }
                            CoopPacket::BroadcastGaming {
                                src_ip,
                                port,
                                payload,
                            } => {
                                // Vérification anti-usurpation d'IP sur les broadcasts
                                let authorized = {
                                    let ep_m = ep_map.read();
                                    let p_m = peers_map.read();
                                    ep_m.get(&src_endpoint)
                                        .and_then(|pid| p_m.get(pid))
                                        .map(|peer| peer.virtual_ip == src_ip)
                                        .unwrap_or(false)
                                };
                                if !authorized {
                                    continue;
                                }

                                let _ = to_bcast_tx.send((port, payload.clone())).await;

                                let bcast_pkt = CoopPacket::BroadcastGaming {
                                    src_ip,
                                    port,
                                    payload,
                                }
                                .encode_encrypted(&session_key);

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
                                // Vérifie que l'expéditeur ne peut déconnecter que lui-même (jamais un autre joueur)
                                let is_owner = ep_map.read().get(&src_endpoint) == Some(&peer_id);
                                if is_owner {
                                    ep_map.write().remove(&src_endpoint);
                                    if let Some(removed) = peers_map.write().remove(&peer_id) {
                                        ip_map.write().remove(&removed.virtual_ip);
                                    }
                                    info!("Client {} s'est déconnecté proprement du salon", peer_id);
                                } else {
                                    warn!(
                                        "Tentative de Disconnect non autorisée pour peer_id={} depuis {}",
                                        peer_id, src_endpoint
                                    );
                                }
                            }
                            _ => {}
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
                        let dst_ip = Ipv4Addr::new(data[16], data[17], data[18], data[19]);
                        if dst_ip == Ipv4Addr::BROADCAST || dst_ip.octets()[3] == 255 {
                            let bcast_pkt = CoopPacket::BroadcastGaming {
                                src_ip: my_vip,
                                port: 0,
                                payload: data,
                            }
                            .encode_encrypted(&session_key);

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
                                .encode_encrypted(&session_key);
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
                    .encode_encrypted(&session_key);

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
                .encode_encrypted(&session_key);

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

    /// Démarre en mode Client (Rejoindre un salon / Nœud distant) avec chiffrement AEAD ChaCha20-Poly1305
    #[allow(clippy::too_many_arguments)]
    pub async fn start_client(
        &self,
        server_addr: SocketAddr,
        player_name: String,
        room_code: String,
        room_password: String,
        to_adapter_tx: mpsc::Sender<Vec<u8>>,
        mut from_adapter_rx: mpsc::Receiver<Vec<u8>>,
        to_bcast_tx: mpsc::Sender<(u16, Vec<u8>)>,
        mut from_bcast_rx: mpsc::Receiver<(u16, Vec<u8>)>,
    ) -> Result<()> {
        *self.role.write() = NodeRole::Client;
        *self.conn_status.write() = ConnectionStatus::Connecting;
        *self.last_error.write() = None;
        self.running.store(true, Ordering::SeqCst);

        let session_key = derive_session_key(&room_code, &room_password);
        let auth_tag = compute_handshake_auth_tag(&room_code, &room_password, &player_name);
        let auth_hex = hex_encode(&auth_tag);

        let (socket, dual_stack) = match UdpSocket::bind("[::]:0").await {
            Ok(s) => (s, true),
            Err(_) => (UdpSocket::bind("0.0.0.0:0").await?, false),
        };
        let socket = Arc::new(socket);
        info!("CoopLink Client AEAD initialisé sur {}", socket.local_addr()?);

        // Une socket double pile n'accepte pas une cible IPv4 nue : il faut l'adresse
        // mappée (::ffff:a.b.c.d), sinon send_to échoue en EAFNOSUPPORT et le handshake
        // n'est jamais émis. La réponse de l'hôte arrive sous la même forme mappée,
        // donc la comparaison src_endpoint == server_addr reste valide.
        let mut server_addr = server_addr;
        if dual_stack {
            if let SocketAddr::V4(v4) = server_addr {
                server_addr =
                    SocketAddr::new(IpAddr::V6(v4.ip().to_ipv6_mapped()), v4.port());
            }
        }

        // Endpoint courant de l'hôte. Il est re-verrouillé sur la première réponse AEAD
        // valide : sur une machine multi-hébergée (plusieurs IPv6), le noyau peut répondre
        // depuis une autre adresse que celle visée par le client.
        let host_endpoint = Arc::new(RwLock::new(server_addr));

        // Envoi du Handshake Request chiffré AEAD
        let req = CoopPacket::HandshakeRequest {
            player_name: player_name.clone(),
            room_code: auth_hex.clone(),
            client_version: PROTOCOL_VERSION,
        }
        .encode_encrypted(&session_key);
        socket.send_to(&req, server_addr).await?;

        // Tâche de relance du Handshake si perte UDP et gestion du timeout (passe en Error si pas de réponse)
        let running_retry = self.running.clone();
        let status_retry = self.conn_status.clone();
        let err_retry = self.last_error.clone();
        let sock_retry = socket.clone();
        let player_retry = player_name.clone();
        let auth_retry = auth_hex.clone();

        tokio::spawn(async move {
            for attempt in 1..=8 {
                tokio::time::sleep(Duration::from_millis(1800)).await;
                if !running_retry.load(Ordering::Relaxed) {
                    return;
                }
                if *status_retry.read() != ConnectionStatus::Connecting {
                    return;
                }
                if attempt < 8 {
                    let retry_pkt = CoopPacket::HandshakeRequest {
                        player_name: player_retry.clone(),
                        room_code: auth_retry.clone(),
                        client_version: PROTOCOL_VERSION,
                    }
                    .encode_encrypted(&session_key);
                    let _ = sock_retry.send_to(&retry_pkt, server_addr).await;
                } else {
                    *status_retry.write() = ConnectionStatus::Error;
                    *err_retry.write() = Some(
                        "Délai d'attente dépassé ou code/mot de passe de salon incorrect.".to_string(),
                    );
                }
            }
        });

        let running_flag = self.running.clone();
        let socket_rx = socket.clone();
        let peers_map = self.peers.clone();
        let my_vip_holder = self.my_virtual_ip.clone();
        let conn_status_holder = self.conn_status.clone();
        let last_err_holder = self.last_error.clone();
        let rx_counter = self.bytes_rx.clone();
        let host_ep_rx = host_endpoint.clone();

        // Tâche de réception client : AEAD obligatoire, et endpoint de l'hôte verrouillé
        // sur la première réponse valide (une machine multi-hébergée peut répondre depuis
        // une autre de ses adresses que celle visée).
        tokio::spawn(async move {
            let mut buf = vec![0u8; 65535];
            while running_flag.load(Ordering::Relaxed) {
                match socket_rx.recv_from(&mut buf).await {
                    Ok((len, src_endpoint)) => {
                        rx_counter.fetch_add(len as u64, Ordering::Relaxed);
                        let decoded = CoopPacket::decode_encrypted(&buf[..len], &session_key);

                        let locked = *host_ep_rx.read();
                        if src_endpoint != locked {
                            let adoptable = matches!(
                                &decoded,
                                Ok(CoopPacket::HandshakeResponse { success: true, .. })
                            ) && *conn_status_holder.read() == ConnectionStatus::Connecting;
                            if adoptable {
                                *host_ep_rx.write() = src_endpoint;
                                info!(
                                    "Réponse de l'hôte reçue depuis {} (adresse visée : {}) : endpoint verrouillé.",
                                    src_endpoint, locked
                                );
                            } else {
                                // Un paquet qui se déchiffre mais vient d'une autre adresse :
                                // c'est le symptôme d'un hôte multi-hébergé. On le signale au
                                // lieu de rester silencieux (le pair, lui, nous voit connecté).
                                if decoded.is_ok() {
                                    warn!(
                                        "Paquet AEAD valide ignoré : émis depuis {} alors que l'hôte est {}",
                                        src_endpoint, locked
                                    );
                                } else {
                                    debug!(
                                        "Paquet ignoré depuis une source tierce: {} (attendu: {})",
                                        src_endpoint, locked
                                    );
                                }
                                continue;
                            }
                        }

                        if let Ok(packet) = decoded {
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
                                        *conn_status_holder.write() = ConnectionStatus::Connected;
                                        *last_err_holder.write() = None;
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
                                            ping_ms: 15,
                                            last_seen_secs: current_timestamp_secs(),
                                            is_host: true,
                                            bytes_sent: 0,
                                            bytes_recv: len as u64,
                                        };
                                        peers_map.write().insert(1, host_peer);
                                    } else {
                                        *conn_status_holder.write() = ConnectionStatus::Error;
                                        *last_err_holder.write() = Some(error_msg.clone());
                                        error!("Échec de connexion au salon: {}", error_msg);
                                    }
                                }
                                CoopPacket::Ping { seq, timestamp_ms } => {
                                    let pong = CoopPacket::Pong { seq, timestamp_ms }
                                        .encode_encrypted(&session_key);
                                    let _ = socket_rx.send_to(&pong, src_endpoint).await;
                                }
                                CoopPacket::Pong { timestamp_ms, .. } => {
                                    let now_ms = current_timestamp_ms();
                                    let rtt = (now_ms.saturating_sub(timestamp_ms)).min(999) as u32;
                                    let mut p_map = peers_map.write();
                                    for peer in p_map.values_mut() {
                                        if peer.is_host {
                                            peer.ping_ms = rtt;
                                            peer.last_seen_secs = current_timestamp_secs();
                                        }
                                    }
                                }
                                CoopPacket::TunnelData { payload, .. } => {
                                    let _ = to_adapter_tx.send(payload).await;
                                }
                                CoopPacket::BroadcastGaming { port, payload, .. } => {
                                    let _ = to_bcast_tx.send((port, payload)).await;
                                }
                                CoopPacket::PeerListUpdate { raw_json } => {
                                    if let Ok(mut updated_peers) =
                                        serde_json::from_str::<HashMap<u64, PeerInfo>>(&raw_json)
                                    {
                                        let current_ping = peers_map.read().values().find(|p| p.is_host).map(|p| p.ping_ms).unwrap_or(15);
                                        for p in updated_peers.values_mut() {
                                            if p.is_host && current_ping > 0 {
                                                p.ping_ms = current_ping;
                                            }
                                        }
                                        *peers_map.write() = updated_peers;
                                    }
                                }
                                CoopPacket::Disconnect { reason, .. } => {
                                    warn!("L'hôte s'est déconnecté du salon: {}", reason);
                                    *conn_status_holder.write() = ConnectionStatus::Disconnected;
                                    *last_err_holder.write() = Some(format!("Session fermée par l'hôte: {}", reason));
                                    peers_map.write().clear();
                                    *my_vip_holder.write() = None;
                                    break;
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
        let host_ep_fwd = host_endpoint.clone();
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
                        .encode_encrypted(&session_key);
                        let host_ep = *host_ep_fwd.read();
                        let _ = socket_fwd.send_to(&pkt, host_ep).await;
                    }
                } else {
                    break;
                }
            }
        });

        // Relais des broadcasts de jeux émis localement vers le serveur
        let running_bcast = self.running.clone();
        let socket_bcast = socket.clone();
        let host_ep_bcast = host_endpoint.clone();
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
                        .encode_encrypted(&session_key);
                        let host_ep = *host_ep_bcast.read();
                        let _ = socket_bcast.send_to(&pkt, host_ep).await;
                    }
                } else {
                    break;
                }
            }
        });

        // Ping périodique client -> serveur avec surveillance Heartbeat Timeout
        let running_ping = self.running.clone();
        let status_ping = self.conn_status.clone();
        let socket_ping = socket.clone();
        let host_ep_ping = host_endpoint.clone();
        let peers_ping = self.peers.clone();
        let last_err_ping = self.last_error.clone();
        let my_vip_ping = self.my_virtual_ip.clone();

        tokio::spawn(async move {
            let mut seq = 0u32;
            let mut missed_pings = 0u32;

            while running_ping.load(Ordering::Relaxed) {
                tokio::time::sleep(Duration::from_millis(1500)).await;
                if *status_ping.read() == ConnectionStatus::Connected {
                    seq = seq.wrapping_add(1);
                    let now_ms = current_timestamp_ms();
                    let ping_pkt = CoopPacket::Ping {
                        seq,
                        timestamp_ms: now_ms,
                    }
                    .encode_encrypted(&session_key);
                    let host_ep = *host_ep_ping.read();
                    let _ = socket_ping.send_to(&ping_pkt, host_ep).await;

                    // Surveillance Heartbeat : si aucun paquet de l'Hôte n'a été reçu depuis plus de 4.5s
                    let last_seen = peers_ping.read().get(&1).map(|p| p.last_seen_secs).unwrap_or(0);
                    let now_secs = current_timestamp_secs();
                    if last_seen > 0 && now_secs.saturating_sub(last_seen) > 4 {
                        missed_pings += 1;
                        if missed_pings >= 2 {
                            warn!("Perte de connexion avec l'hôte (Timeout 4s) ! Déconnexion automatique de la session.");
                            *status_ping.write() = ConnectionStatus::Disconnected;
                            *last_err_ping.write() = Some("L'hôte s'est déconnecté du salon.".to_string());
                            peers_ping.write().clear();
                            *my_vip_ping.write() = None;
                            break;
                        }
                    } else {
                        missed_pings = 0;
                    }
                }
            }
        });

        Ok(())
    }

    pub fn stop(&self) {
        if *self.role.read() == NodeRole::Host {
            if let Some(sock) = &*self.host_socket.read() {
                if let Some(key) = &*self.active_session_key.read() {
                    let disc = CoopPacket::Disconnect {
                        peer_id: self.my_id,
                        reason: "L'hôte a fermé la session".to_string(),
                    }
                    .encode_encrypted(key);

                    for peer in self.peers.read().values() {
                        if !peer.is_host {
                            let _ = sock.try_send_to(&disc, peer.endpoint);
                        }
                    }
                }
            }
        }

        self.running.store(false, Ordering::SeqCst);
        *self.role.write() = NodeRole::Idle;
        *self.conn_status.write() = ConnectionStatus::Disconnected;
        *self.last_error.write() = None;
        *self.my_virtual_ip.write() = None;
        self.peers.write().clear();
        self.endpoint_to_id.write().clear();
        self.ip_to_id.write().clear();
        *self.host_socket.write() = None;
        *self.active_session_key.write() = None;
    }
}

fn hex_encode(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        use std::fmt::Write;
        let _ = write!(&mut s, "{:02x}", b);
    }
    s
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
