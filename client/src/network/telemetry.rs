use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpStream, UdpSocket};

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
    pub discovered_http_host: Arc<RwLock<Option<String>>>,
    pub state_snapshot: Arc<RwLock<TelemetryPayload>>,
    pub target_ips: Arc<RwLock<Vec<String>>>,
}

impl CentralTelemetryClient {
    pub fn new(instance_uuid: u64, player_name: String, central_addr: String) -> Arc<Self> {
        let fallback_id = format!("CL-{}", instance_uuid);
        let base_ip = extract_host_ip(&central_addr);
        Arc::new(Self {
            assigned_id: Arc::new(RwLock::new(fallback_id)),
            server_connected: Arc::new(AtomicBool::new(false)),
            discovered_server: Arc::new(RwLock::new(None)),
            discovered_http_host: Arc::new(RwLock::new(None)),
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
            target_ips: Arc::new(RwLock::new(vec![base_ip, "127.0.0.1".to_string()])),
        })
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

        let mut ips = Vec::new();
        let c_ip = extract_host_ip(central_addr);
        if !c_ip.is_empty() && !ips.contains(&c_ip) {
            ips.push(c_ip);
        }
        let r_ip = extract_host_ip(remote_peer_addr);
        if !r_ip.is_empty() && !ips.contains(&r_ip) {
            ips.push(r_ip);
        }
        if !ips.contains(&"127.0.0.1".to_string()) {
            ips.push("127.0.0.1".to_string());
        }
        *self.target_ips.write() = ips;
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

            // Tâche d'écoute des réponses UDP du serveur central (attribution de l'ID unique)
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

            let mut cycle: u32 = 0;
            let mut auto_detected_ips: Vec<String> = Vec::new();

            loop {
                // Toutes les 3 itérations (~6s), scanne les connexions actives vers le port 30025 (ex: navigateur ouvert sur le Hub)
                // ainsi que les sous-réseaux locaux physiques pour trouver automatiquement le container Docker
                if cycle % 3 == 0 {
                    if let Ok(found) = tokio::task::spawn_blocking(discover_hub_ips_on_system).await {
                        auto_detected_ips = found;
                    }
                }
                cycle = cycle.wrapping_add(1);

                let snap = me.state_snapshot.read().clone();
                let udp_pkt = encode_telemetry_packet(&snap);

                let mut candidate_ips = me.target_ips.read().clone();
                for ip in &auto_detected_ips {
                    if !candidate_ips.contains(ip) {
                        candidate_ips.push(ip.clone());
                    }
                }

                // 1. Envoi UDP sur le port 30024
                let discovered_udp = *me.discovered_server.read();
                if let Some(srv) = discovered_udp {
                    let _ = socket.send_to(&udp_pkt, srv).await;
                }
                for ip in &candidate_ips {
                    if let Ok(addr) = format!("{}:30024", ip).parse::<SocketAddr>() {
                        let _ = socket.send_to(&udp_pkt, addr).await;
                    }
                }

                // 2. Envoi HTTP POST /api/telemetry sur le port 30025 (garantit la remontée même si l'UDP 30024 est filtré)
                let discovered_http = me.discovered_http_host.read().clone();
                if let Some(ref host_ip) = discovered_http {
                    if let Some(ack) = send_http_telemetry(host_ip, &snap).await {
                        *me.assigned_id.write() = ack.assigned_instance_id;
                        me.server_connected.store(true, Ordering::Relaxed);
                    }
                } else {
                    for ip in &candidate_ips {
                        if !ip.ends_with(".255") {
                            if let Some(ack) = send_http_telemetry(ip, &snap).await {
                                *me.assigned_id.write() = ack.assigned_instance_id;
                                *me.discovered_http_host.write() = Some(ip.clone());
                                me.server_connected.store(true, Ordering::Relaxed);
                                break;
                            }
                        }
                    }
                }

                tokio::time::sleep(Duration::from_secs(2)).await;
            }
        });
    }
}

fn extract_host_ip(addr: &str) -> String {
    let trimmed = addr
        .trim()
        .trim_start_matches("http://")
        .trim_start_matches("https://");
    trimmed.split(':').next().unwrap_or("127.0.0.1").trim().to_string()
}

/// Détecte automatiquement l'IP du container Hub CoopLink (via les connexions TCP au port 30025 ou les interfaces LAN physiques)
fn discover_hub_ips_on_system() -> Vec<String> {
    let mut ips = Vec::new();

    // 1. Vérifie via netstat si le PC (ex: le navigateur Web de l'utilisateur) est connecté à un serveur sur le port 30025 ou 30024
    if let Ok(out) = crate::platform::windows::process::run_hidden("netstat", &["-n"]) {
        if out.status.success() {
            let text = String::from_utf8_lossy(&out.stdout);
            for line in text.lines() {
                if line.contains(":30025") || line.contains(":30024") {
                    for part in line.split_whitespace() {
                        if let Some((ip_part, port_part)) = part.rsplit_once(':') {
                            if port_part == "30025" || port_part == "30024" {
                                let clean_ip = ip_part.trim_matches(|c| c == '[' || c == ']').to_string();
                                if !clean_ip.is_empty()
                                    && clean_ip != "0.0.0.0"
                                    && clean_ip != "::"
                                    && !ips.contains(&clean_ip)
                                {
                                    ips.push(clean_ip);
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // 2. Ajoute les adresses de broadcast dirigées des interfaces physiques (ex: 192.168.1.255) pour le port UDP 30024
    if let Ok(out) = crate::platform::windows::process::run_hidden("ipconfig", &[]) {
        if out.status.success() {
            let text = String::from_utf8_lossy(&out.stdout);
            for line in text.lines() {
                if line.contains("IPv4") {
                    if let Some(ip_str) = line.split(':').nth(1) {
                        let ip = ip_str.trim();
                        let octets: Vec<&str> = ip.split('.').collect();
                        if octets.len() == 4 && octets[0] != "10" && octets[0] != "127" {
                            let bcast = format!("{}.{}.{}.255", octets[0], octets[1], octets[2]);
                            if !ips.contains(&bcast) {
                                ips.push(bcast);
                            }
                        }
                    }
                }
            }
        }
    }

    ips
}

async fn send_http_telemetry(host_ip: &str, payload: &TelemetryPayload) -> Option<TelemetryAck> {
    let addr: SocketAddr = format!("{}:30025", host_ip).parse().ok()?;
    let mut stream = tokio::time::timeout(Duration::from_millis(350), TcpStream::connect(addr))
        .await
        .ok()?
        .ok()?;

    let body = serde_json::to_string(payload).ok()?;
    let req = format!(
        "POST /api/telemetry HTTP/1.1\r\n\
         Host: {}:30025\r\n\
         Content-Type: application/json\r\n\
         Content-Length: {}\r\n\
         Connection: close\r\n\r\n\
         {}",
        host_ip,
        body.len(),
        body
    );

    tokio::time::timeout(Duration::from_millis(350), stream.write_all(req.as_bytes()))
        .await
        .ok()?
        .ok()?;

    let mut resp_buf = Vec::with_capacity(1024);
    tokio::time::timeout(Duration::from_millis(350), stream.read_to_end(&mut resp_buf))
        .await
        .ok()?
        .ok()?;

    let resp_str = String::from_utf8_lossy(&resp_buf);
    let json_start = resp_str.find('{')?;
    serde_json::from_str::<TelemetryAck>(&resp_str[json_start..]).ok()
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
