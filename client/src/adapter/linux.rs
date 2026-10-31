use super::VirtualAdapter;
use crate::platform::linux::set_adapter_metric;
use anyhow::Result;
use std::net::Ipv4Addr;
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::info;

#[allow(dead_code)]
pub struct LinuxVirtualAdapter {
    adapter_name: String,
    assigned_ip: Option<Ipv4Addr>,
    metric: u32,
    connected: Arc<AtomicBool>,
    tx_queue: mpsc::Sender<Vec<u8>>,
    rx_queue: mpsc::Receiver<Vec<u8>>,
}

#[allow(dead_code)]
impl LinuxVirtualAdapter {
    pub fn new() -> (Self, mpsc::Sender<Vec<u8>>, mpsc::Receiver<Vec<u8>>) {
        let (to_adapter_tx, from_net_rx) = mpsc::channel(1000);
        let (to_net_tx, rx_queue) = mpsc::channel(1000);

        let adapter = Self {
            adapter_name: "cooplink0".to_string(),
            assigned_ip: None,
            metric: 5,
            connected: Arc::new(AtomicBool::new(false)),
            tx_queue: to_net_tx,
            rx_queue,
        };

        (adapter, to_adapter_tx, from_net_rx)
    }
}

impl VirtualAdapter for LinuxVirtualAdapter {
    fn initialize(
        &mut self,
        name: &str,
        ip: Ipv4Addr,
        _subnet_mask: Ipv4Addr,
        metric: u32,
        mtu: u32,
    ) -> Result<()> {
        let iface = if name.is_empty() || name.contains(' ') {
            "cooplink0"
        } else {
            name
        };
        self.adapter_name = iface.to_string();
        self.assigned_ip = Some(ip);
        self.metric = metric;

        info!(
            "Initialisation de l'interface TUN Linux '{}' avec IP {}/24, MTU {}, Métrique {}...",
            iface, ip, mtu, metric
        );

        let _ = Command::new("ip")
            .args(["tuntap", "add", "dev", iface, "mode", "tun"])
            .output();
        let _ = Command::new("ip")
            .args(["addr", "add", &format!("{}/24", ip), "dev", iface])
            .output();
        let _ = Command::new("ip")
            .args(["link", "set", "dev", iface, "mtu", &mtu.to_string(), "up"])
            .output();
        let _ = set_adapter_metric(iface, metric);

        self.connected.store(true, Ordering::SeqCst);
        Ok(())
    }

    fn read_packet(&mut self) -> Result<Option<Vec<u8>>> {
        match self.rx_queue.try_recv() {
            Ok(pkt) => Ok(Some(pkt)),
            Err(_) => Ok(None),
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
        let _ = Command::new("ip")
            .args(["link", "set", "dev", &self.adapter_name, "down"])
            .output();
        Ok(())
    }
}
