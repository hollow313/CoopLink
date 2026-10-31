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
    #[allow(dead_code)]
    pub discovered_http_endpoint: Arc<RwLock<Option<String>>>,
    pub state_snapshot: Arc<RwLock<TelemetryPayload>>,
    pub configured_central_raw: Arc<RwLock<String>>,
    pub wake_notify: Arc<Notify>,
}

impl CentralTelemetryClient {
    pub fn new(instance_uuid: u64, player_name: String, central_addr: String) -> Arc<Self> {
        let fallback_id = format!("CL-{}", instance_uuid);

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
            configured_central_raw: Arc::new(RwLock::new(central_addr)),
            wake_notify: Arc::new(Notify::new()),
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub fn update_state(
        &self,
        player_name: &str,
        tunnel_mounted: bool,
        is_host: bool,
        room_code: &str,
        peer_count: usize,
        screen_sharing: bool,
        central_addr: &str,
        _remote_peer_addr: &str,
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

        if changed {
            self.wake_notify.notify_one();
        }
    }

    pub fn start_background_loop(self: &Arc<Self>, rt: &tokio::runtime::Runtime) {
        let me = self.clone();
        rt.spawn(async move {
            let socket = match UdpSocket::bind("0.0.0.0:0").await {
                Ok(s) => Arc::new(s),
                Err(_) => return,
            };

            // Tâche d'écoute des réponses UDP du serveur central (attribution de l'ID unique)
            let sock_rx = socket.clone();
            let me_rx = me.clone();
            tokio::spawn(async move {
                let mut buf = vec![0u8; 4096];
                while let Ok((len, src)) = sock_rx.recv_from(&mut buf).await {
                    if len > 10 && buf.get(0..4) == Some(PROTOCOL_MAGIC.as_slice()) && buf[5] == PKT_INSTANCE_ID_ASSIGNED {
                        let payload_end = len - 4;
                        if let Some(json_slice) = buf.get(6..payload_end) {
                            if let Ok(ack) = serde_json::from_slice::<TelemetryAck>(json_slice) {
                                *me_rx.assigned_id.write() = ack.assigned_instance_id;
                                *me_rx.discovered_server.write() = Some(src);
                                me_rx.server_connected.store(true, Ordering::Relaxed);
                            }
                        }
                    }
                }
            });

            loop {
                let snap = me.state_snapshot.read().clone();
                let udp_pkt = encode_telemetry_packet(&snap);
                let raw_cfg = me.configured_central_raw.read().clone();
                let host_only = extract_host_name(&raw_cfg);

                // 1. Envoi UDP vers le serveur central configuré (ex: cooplink.home-lh.ovh:30024)
                let discovered_udp = *me.discovered_server.read();
                if let Some(srv) = discovered_udp {
                    let _ = socket.send_to(&udp_pkt, srv).await;
                }
                if !host_only.is_empty() {
                    let udp_target = format!("{}:30024", host_only);
                    let _ = socket.send_to(&udp_pkt, udp_target.as_str()).await;
                }

                // 2. Envoi HTTPS / HTTP vers l'URL configurée (ex: https://cooplink.home-lh.ovh)
                let mut delivered = false;
                if let Some(ep) = extract_explicit_socket_endpoint(&raw_cfg) {
                    if let Some(ack) = send_http_telemetry_endpoint(&ep, &snap).await {
                        *me.assigned_id.write() = ack.assigned_instance_id;
                        me.server_connected.store(true, Ordering::Relaxed);
                        delivered = true;
                    }
                }

                if !delivered {
                    let base_url = if raw_cfg.starts_with("https://") || raw_cfg.starts_with("http://") {
                        raw_cfg.trim_end_matches('/').to_string()
                    } else {
                        "https://cooplink.home-lh.ovh".to_string()
                    };
                    let snap_clone = snap.clone();
                    if let Ok(Some(ack)) = tokio::task::spawn_blocking(move || {
                        send_https_telemetry_curl(&base_url, &snap_clone)
                    })
                    .await
                    {
                        *me.assigned_id.write() = ack.assigned_instance_id;
                        me.server_connected.store(true, Ordering::Relaxed);
                    }
                }

                tokio::select! {
                    _ = tokio::time::sleep(Duration::from_secs(6)) => {}
                    _ = me.wake_notify.notified() => {}
                }
            }
        });
    }
}

fn extract_host_name(addr: &str) -> String {
    let trimmed = addr
        .trim()
        .trim_start_matches("http://")
        .trim_start_matches("https://")
        .trim_end_matches('/');
    trimmed
        .split(':')
        .next()
        .unwrap_or("cooplink.home-lh.ovh")
        .trim()
        .to_string()
}

fn extract_explicit_socket_endpoint(addr: &str) -> Option<String> {
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

async fn send_http_telemetry_endpoint(endpoint: &str, payload: &TelemetryPayload) -> Option<TelemetryAck> {
    let addr: SocketAddr = endpoint.parse().ok()?;
    let mut stream = tokio::time::timeout(Duration::from_millis(400), TcpStream::connect(addr))
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

    tokio::time::timeout(Duration::from_millis(400), stream.write_all(req.as_bytes()))
        .await
        .ok()?
        .ok()?;

    let mut resp_buf = Vec::with_capacity(1024);
    tokio::time::timeout(Duration::from_millis(400), stream.read_to_end(&mut resp_buf))
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
