use anyhow::Result;
use socket2::{Domain, Protocol, Socket, Type};
use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use tokio::net::UdpSocket;
use tokio::sync::mpsc;
use tracing::{debug, info, warn};

/// Ports de découverte LAN gaming standards fréquents
pub const GAMING_BROADCAST_PORTS: &[u16] = &[
    6112,  // Blizzard (Warcraft III, Diablo, StarCraft)
    27015, // Source Engine / Valve (CS, Half-Life, L4D, TF2)
    7777,  // Unreal Engine games
    19132, // Minecraft Bedrock LAN
    25565, // Minecraft Java LAN discovery
    11155, // Terraria LAN
    3724,  // World of Warcraft / Blizzard
];

pub struct BroadcastRelay {
    running: Arc<AtomicBool>,
    relayed_counter: Arc<AtomicU64>,
}

impl BroadcastRelay {
    pub fn new() -> Self {
        Self {
            running: Arc::new(AtomicBool::new(false)),
            relayed_counter: Arc::new(AtomicU64::new(0)),
        }
    }

    pub fn relayed_count(&self) -> u64 {
        self.relayed_counter.load(Ordering::Relaxed)
    }

    /// Démarre l'écoute et le relais des broadcasts gaming
    pub async fn start(
        &self,
        virtual_ip: Ipv4Addr,
        to_tunnel_tx: mpsc::Sender<(u16, Vec<u8>)>,
        mut from_tunnel_rx: mpsc::Receiver<(u16, Vec<u8>)>,
    ) -> Result<()> {
        self.running.store(true, Ordering::SeqCst);
        let running_flag = self.running.clone();
        let counter = self.relayed_counter.clone();

        info!(
            "Démarrage du Gaming Broadcast Relay pour l'IP virtuelle {}...",
            virtual_ip
        );

        // Tâche d'injection : réinjecte en local les broadcasts reçus des amis distants
        let running_inject = running_flag.clone();
        tokio::spawn(async move {
            let socket = match create_broadcast_socket(0) {
                Ok(s) => s,
                Err(e) => {
                    warn!("Impossible de créer le socket d'injection broadcast: {}", e);
                    return;
                }
            };
            let tokio_socket = match UdpSocket::from_std(socket.into()) {
                Ok(s) => s,
                Err(e) => {
                    warn!("Impossible de convertir le socket d'injection: {}", e);
                    return;
                }
            };

            while running_inject.load(Ordering::Relaxed) {
                if let Some((port, payload)) = from_tunnel_rx.recv().await {
                    let target_addr = SocketAddrV4::new(Ipv4Addr::BROADCAST, port);
                    if let Err(e) = tokio_socket.send_to(&payload, target_addr).await {
                        debug!("Erreur d'envoi du broadcast gaming vers {}: {}", target_addr, e);
                    }
                } else {
                    break;
                }
            }
        });

        // Tâche de capture : écoute sur les ports gaming pour capturer les annonces de parties locales
        for &port in GAMING_BROADCAST_PORTS {
            let tx_clone = to_tunnel_tx.clone();
            let running_port = running_flag.clone();
            let counter_clone = counter.clone();

            tokio::spawn(async move {
                if let Ok(std_sock) = create_broadcast_socket(port) {
                    if let Ok(async_sock) = UdpSocket::from_std(std_sock.into()) {
                        let mut buf = vec![0u8; 4096];
                        while running_port.load(Ordering::Relaxed) {
                            match async_sock.recv_from(&mut buf).await {
                                Ok((len, src)) => {
                                    // Ignore les paquets provenant de notre propre IP virtuelle pour éviter les boucles
                                    if let SocketAddr::V4(v4) = src {
                                        if *v4.ip() == virtual_ip {
                                            continue;
                                        }
                                    }
                                    counter_clone.fetch_add(1, Ordering::Relaxed);
                                    let _ = tx_clone.send((port, buf[..len].to_vec())).await;
                                }
                                Err(_) => {
                                    tokio::time::sleep(std::time::Duration::from_millis(250)).await;
                                }
                            }
                        }
                    }
                }
            });
        }

        Ok(())
    }

    pub fn stop(&self) {
        self.running.store(false, Ordering::SeqCst);
    }
}

/// Crée un socket UDP avec option SO_BROADCAST et SO_REUSEADDR / SO_REUSEPORT
fn create_broadcast_socket(port: u16) -> Result<Socket> {
    let socket = Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::UDP))?;
    socket.set_broadcast(true)?;
    socket.set_reuse_address(true)?;

    #[cfg(not(windows))]
    let _ = socket.set_reuse_port(true);

    let bind_addr = SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, port);
    socket.bind(&bind_addr.into())?;
    socket.set_nonblocking(true)?;

    Ok(socket)
}
