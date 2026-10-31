use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpStream, UdpSocket};
use tokio::sync::Notify;

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
    pub discovered_http_endpoint: Arc<RwLock<Option<String>>>,
    pub state_snapshot: Arc<RwLock<TelemetryPayload>>,
    pub target_ips: Arc<RwLock<Vec<String>>>,
    pub configured_central_raw: Arc<RwLock<String>>,
    pub wake_notify: Arc<Notify>,
}

impl CentralTelemetryClient {
    pub fn new(instance_uuid: u64, player_name: String, central_addr: String) -> Arc<Self> {
        let fallback_id = format!("CL-{}", instance_uuid);
        let base_ip = extract_host_ip(&central_addr);
        let mut initial_ips = vec![base_ip, "127.0.0.1".to_string(), "192.168.1.200".to_string()];
        initial_ips.dedup();

        Arc::new(Self {
            assigned_id: Arc::new(RwLock::new(fallback_id)),
            server_connected: Arc::new(AtomicBool::new(false)),
            discovered_server: Arc::new(RwLock::new(None)),
            discovered_http_endpoint: Arc::new(RwLock::new(None)),
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
            target_ips: Arc::new(RwLock::new(initial_ips)),
            configured_central_raw: Arc::new(RwLock::new(central_addr)),
            wake_notify: Arc::new(Notify::new()),
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
        let mut changed = false;
        {
            let mut snap = self.state_snapshot.write();
            if snap.tunnel_mounted != tunnel_mounted
                || snap.is_host != is_host
                || snap.room_code != room_code
                || snap.peer_count != peer_count
                || snap.screen_sharing != screen_sharing
                || snap.player_name != player_name
            {
                changed = true;
            }
            snap.player_name = player_name.to_string();
            snap.tunnel_mounted = tunnel_mounted;
            snap.is_host = is_host;
            snap.room_code = room_code.to_string();
            snap.peer_count = peer_count;
            snap.screen_sharing = screen_sharing;
        }

        *self.configured_central_raw.write() = central_addr.to_string();

        let mut ips = Vec::new();
        let c_ip = extract_host_ip(central_addr);
        if !c_ip.is_empty() && !ips.contains(&c_ip) {
            ips.push(c_ip);
        }
        let r_ip = extract_host_ip(remote_peer_addr);
        if !r_ip.is_empty() && !ips.contains(&r_ip) {
            ips.push(r_ip);
        }
        for default_ip in ["192.168.1.200", "127.0.0.1"] {
            if !ips.contains(&default_ip.to_string()) {
                ips.push(default_ip.to_string());
            }
        }
        *self.target_ips.write() = ips;

        if changed {
            self.wake_notify.notify_one();
        }
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
            let mut auto_ips: Vec<String> = Vec::new();
            let mut auto_http_endpoints: Vec<String> = Vec::new();

            loop {
                if !me.server_connected.load(Ordering::Relaxed) && cycle % 6 == 0 {
                    if let Ok((found_ips, found_endpoints)) =
                        tokio::task::spawn_blocking(discover_hub_on_system).await
                    {
                        auto_ips = found_ips;
                        auto_http_endpoints = found_endpoints;
                    }
                }
                cycle = cycle.wrapping_add(1);

                let snap = me.state_snapshot.read().clone();
                let udp_pkt = encode_telemetry_packet(&snap);

                let mut candidate_ips = me.target_ips.read().clone();
                for ip in &auto_ips {
                    if !candidate_ips.contains(ip) {
                        candidate_ips.push(ip.clone());
                    }
                }

                // 1. Envoi UDP rapide en mémoire sur le port 30024 (Domaine OVH + IPs candidates, 0% CPU)
                let discovered_udp = *me.discovered_server.read();
                if let Some(srv) = discovered_udp {
                    let _ = socket.send_to(&udp_pkt, srv).await;
                }
                let _ = socket.send_to(&udp_pkt, "cooplink.home-lh.ovh:30024").await;
                for ip in &candidate_ips {
                    for udp_port in [30024u16, 30026u16] {
                        let target = format!("{}:{}", ip, udp_port);
                        let _ = socket.send_to(&udp_pkt, target.as_str()).await;
                    }
                }

                // 2. Envoi HTTP direct / LAN en mémoire (0 processus externe, 0% CPU)
                let raw_cfg = me.configured_central_raw.read().clone();
                let mut http_ok = false;
                let known_endpoint = me.discovered_http_endpoint.read().clone();
                if let Some(ref endpoint) = known_endpoint {
                    if let Some(ack) = send_http_telemetry_endpoint(endpoint, &snap).await {
                        *me.assigned_id.write() = ack.assigned_instance_id;
                        me.server_connected.store(true, Ordering::Relaxed);
                        http_ok = true;
                    } else {
                        *me.discovered_http_endpoint.write() = None;
                    }
                }

                if !http_ok {
                    let mut endpoints_to_try = Vec::new();
                    if let Some(ep) = extract_explicit_endpoint(&raw_cfg) {
                        endpoints_to_try.push(ep);
                    }
                    for ep in &auto_http_endpoints {
                        if !endpoints_to_try.contains(ep) {
                            endpoints_to_try.push(ep.clone());
                        }
                    }
                    for ip in &candidate_ips {
                        if !ip.ends_with(".255") {
                            for port in [30024u16, 30026u16, 30025u16] {
                                let ep = format!("{}:{}", ip, port);
                                if !endpoints_to_try.contains(&ep) {
                                    endpoints_to_try.push(ep);
                                }
                            }
                        }
                    }

                    for ep in endpoints_to_try {
                        if let Some(ack) = send_http_telemetry_endpoint(&ep, &snap).await {
                            *me.assigned_id.write() = ack.assigned_instance_id;
                            *me.discovered_http_endpoint.write() = Some(ep);
                            me.server_connected.store(true, Ordering::Relaxed);
                            http_ok = true;
                            break;
                        }
                    }
                }

                // 3. Envoi HTTPS vers https://cooplink.home-lh.ovh (Reverse Proxy -> port 30024)
                if !http_ok || cycle % 2 == 1 {
                    let mut https_urls = vec!["https://cooplink.home-lh.ovh".to_string()];
                    if raw_cfg.starts_with("https://") || raw_cfg.starts_with("http://") {
                        let clean_url = raw_cfg.trim_end_matches('/').to_string();
                        if !https_urls.contains(&clean_url) {
                            https_urls.insert(0, clean_url);
                        }
                    }
                    for base_url in &https_urls {
                        let url_clone = base_url.clone();
                        let snap_clone = snap.clone();
                        if let Ok(Some(ack)) = tokio::task::spawn_blocking(move || {
                            send_https_telemetry_curl(&url_clone, &snap_clone)
                        })
                        .await
                        {
                            *me.assigned_id.write() = ack.assigned_instance_id;
                            me.server_connected.store(true, Ordering::Relaxed);
                            break;
                        }
                    }
                }

                tokio::select! {
                    _ = tokio::time::sleep(Duration::from_secs(5)) => {}
                    _ = me.wake_notify.notified() => {}
                }
            }
        });
    }
}

fn extract_host_ip(addr: &str) -> String {
    let trimmed = addr
        .trim()
        .trim_start_matches("http://")
        .trim_start_matches("https://")
        .trim_end_matches('/');
    trimmed.split(':').next().unwrap_or("127.0.0.1").trim().to_string()
}

fn extract_explicit_endpoint(addr: &str) -> Option<String> {
    let trimmed = addr
        .trim()
        .trim_start_matches("http://")
        .trim_start_matches("https://")
        .trim_end_matches('/');
    if trimmed.contains(':') {
        Some(trimmed.to_string())
    } else {
        None
    }
}

/// Détecte automatiquement les IP de serveurs LAN (ex: NAS TrueNAS 192.168.1.200) et les ports Web actifs (30024..30050)
fn discover_hub_on_system() -> (Vec<String>, Vec<String>) {
    let mut ips = Vec::new();
    let mut endpoints = Vec::new();

    if let Ok(out) = crate::platform::windows::process::run_hidden("netstat", &["-n"]) {
        if out.status.success() {
            let text = String::from_utf8_lossy(&out.stdout);
            for line in text.lines() {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 3 {
                    let remote = parts[2];
                    if let Some((ip_part, port_part)) = remote.rsplit_once(':') {
                        let clean_ip = ip_part.trim_matches(|c| c == '[' || c == ']').to_string();
                        if let Ok(port_num) = port_part.parse::<u16>() {
                            // Si le PC (ex: navigateur Web) est connecté sur un port 30000..30100 (ex: 30025, 30026)
                            if (30000..=30100).contains(&port_num)
                                && !clean_ip.is_empty()
                                && clean_ip != "0.0.0.0"
                                && clean_ip != "::"
                            {
                                let ep = format!("{}:{}", clean_ip, port_num);
                                if !endpoints.contains(&ep) {
                                    endpoints.push(ep);
                                }
                                if !ips.contains(&clean_ip) {
                                    ips.push(clean_ip.clone());
                                }
                            }
                            // Collecte aussi les serveurs LAN locaux (ex: 192.168.1.200)
                            if (clean_ip.starts_with("192.168.") || clean_ip.starts_with("10."))
                                && !clean_ip.ends_with(".1")
                                && !clean_ip.ends_with(".255")
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

    (ips, endpoints)
}

async fn send_http_telemetry_endpoint(endpoint: &str, payload: &TelemetryPayload) -> Option<TelemetryAck> {
    let addr: SocketAddr = endpoint.parse().ok()?;
    let mut stream = tokio::time::timeout(Duration::from_millis(300), TcpStream::connect(addr))
        .await
        .ok()?
        .ok()?;

    let body = serde_json::to_string(payload).ok()?;
    let req = format!(
        "POST /api/telemetry HTTP/1.1\r\n\
         Host: {}\r\n\
         Content-Type: application/json\r\n\
         Content-Length: {}\r\n\
         Connection: close\r\n\r\n\
         {}",
        endpoint,
        body.len(),
        body
    );

    tokio::time::timeout(Duration::from_millis(300), stream.write_all(req.as_bytes()))
        .await
        .ok()?
        .ok()?;

    let mut resp_buf = Vec::with_capacity(1024);
    tokio::time::timeout(Duration::from_millis(300), stream.read_to_end(&mut resp_buf))
        .await
        .ok()?
        .ok()?;

    let resp_str = String::from_utf8_lossy(&resp_buf);
    if !resp_str.starts_with("HTTP/1.1 200") && !resp_str.starts_with("HTTP/1.0 200") {
        return None;
    }
    let json_start = resp_str.find('{')?;
    serde_json::from_str::<TelemetryAck>(&resp_str[json_start..]).ok()
}

fn send_https_telemetry_curl(base_url: &str, payload: &TelemetryPayload) -> Option<TelemetryAck> {
    let body = serde_json::to_string(payload).ok()?;
    let url = format!("{}/api/telemetry", base_url.trim_end_matches('/'));
    let curl_bin = if cfg!(target_os = "windows") {
        "curl.exe"
    } else {
        "curl"
    };
    let out = crate::platform::windows::process::run_hidden(
        curl_bin,
        &[
            "-s",
            "-L",
            "-k",
            "--connect-timeout",
            "2",
            "-m",
            "3",
            "-X",
            "POST",
            "-H",
            "Content-Type: application/json",
            "-d",
            &body,
            &url,
        ],
    )
    .ok()?;

    if !out.status.success() {
        return None;
    }
    let resp_str = String::from_utf8_lossy(&out.stdout);
    serde_json::from_str::<TelemetryAck>(resp_str.trim()).ok()
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
