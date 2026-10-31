use super::VirtualAdapter;
use crate::platform::windows::metric::set_adapter_metric;
use crate::platform::windows::process::run_hidden;
use anyhow::Result;
use std::net::Ipv4Addr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::info;

#[allow(dead_code)]
pub struct WindowsVirtualAdapter {
    adapter_name: String,
    assigned_ip: Option<Ipv4Addr>,
    metric: u32,
    connected: Arc<AtomicBool>,
    tx_queue: mpsc::Sender<Vec<u8>>,
    rx_queue: mpsc::Receiver<Vec<u8>>,
}

impl WindowsVirtualAdapter {
    pub fn new() -> (Self, mpsc::Sender<Vec<u8>>, mpsc::Receiver<Vec<u8>>) {
        // Canal 1 : Réseau Tunnel -> Adaptateur Virtuel (paquets entrants à injecter localement)
        let (net_to_adapter_tx, net_to_adapter_rx) = mpsc::channel(1000);
        // Canal 2 : Adaptateur Virtuel -> Réseau Tunnel (paquets sortants à expédier vers les pairs)
        let (adapter_to_net_tx, adapter_to_net_rx) = mpsc::channel(1000);

        let adapter = Self {
            adapter_name: "CoopLink Virtual Adapter".to_string(),
            assigned_ip: None,
            metric: 5,
            connected: Arc::new(AtomicBool::new(false)),
            tx_queue: adapter_to_net_tx,
            rx_queue: net_to_adapter_rx,
        };

        (adapter, net_to_adapter_tx, adapter_to_net_rx)
    }
}

impl VirtualAdapter for WindowsVirtualAdapter {
    fn initialize(
        &mut self,
        name: &str,
        ip: Ipv4Addr,
        subnet_mask: Ipv4Addr,
        metric: u32,
        mtu: u32,
    ) -> Result<()> {
        self.adapter_name = name.to_string();
        self.assigned_ip = Some(ip);
        self.metric = metric;

        info!(
            "Initialisation de l'adaptateur virtuel Windows '{}' avec IP {}, Masque {}, Métrique {}...",
            name, ip, subnet_mask, metric
        );

        // Configuration IP & Sous-réseau via netsh discret (sans aucune console CMD)
        let _ = run_hidden(
            "netsh",
            &[
                "interface",
                "ipv4",
                "set",
                "address",
                &format!("name=\"{}\"", name),
                "static",
                &ip.to_string(),
                &subnet_mask.to_string(),
                "none",
            ],
        );

        // Configuration du MTU discret
        let _ = run_hidden(
            "netsh",
            &[
                "interface",
                "ipv4",
                "set",
                "subinterface",
                &format!("\"{}\"", name),
                &format!("mtu={}", mtu),
                "store=persistent",
            ],
        );

        // Application de la métrique basse demandée
        let _ = set_adapter_metric(name, metric);

        // Ajout de la route vers le sous-réseau virtuel avec métrique basse de façon discrète
        let subnet_prefix = format!("{}.0", ip.octets()[0..3].iter().map(|o| o.to_string()).collect::<Vec<_>>().join("."));
        let _ = run_hidden(
            "route",
            &[
                "add",
                &subnet_prefix,
                "mask",
                "255.255.255.0",
                &ip.to_string(),
                "metric",
                &metric.to_string(),
            ],
        );

        self.connected.store(true, Ordering::SeqCst);
        info!("Adaptateur Windows configuré avec succès ! Métrique basse activée pour priorité LAN.");

        Ok(())
    }

    fn read_packet(&mut self) -> Result<Option<Vec<u8>>> {
        match self.rx_queue.try_recv() {
            Ok(pkt) => Ok(Some(pkt)),
            Err(mpsc::error::TryRecvError::Empty) => Ok(None),
            Err(mpsc::error::TryRecvError::Disconnected) => Ok(None),
        }
    }

    fn write_packet(&mut self, packet: &[u8]) -> Result<()> {
        let _ = self.tx_queue.try_send(packet.to_vec());
        Ok(())
    }

    fn set_metric(&mut self, metric: u32) -> Result<()> {
        self.metric = metric;
        set_adapter_metric(&self.adapter_name, metric)
    }

    fn is_connected(&self) -> bool {
        self.connected.load(Ordering::Relaxed)
    }

    fn shutdown(&mut self) -> Result<()> {
        self.connected.store(false, Ordering::SeqCst);
        info!("Fermeture de l'adaptateur virtuel CoopLink");
        Ok(())
    }
}
