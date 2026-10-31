use super::VirtualAdapter;
use anyhow::Result;
use std::net::Ipv4Addr;

pub struct MacVirtualAdapter {
    adapter_name: String,
    metric: u32,
    connected: bool,
}

impl MacVirtualAdapter {
    pub fn new() -> Self {
        Self {
            adapter_name: "utun5".to_string(),
            metric: 5,
            connected: false,
        }
    }
}

impl VirtualAdapter for MacVirtualAdapter {
    fn initialize(
        &mut self,
        name: &str,
        _ip: Ipv4Addr,
        _subnet_mask: Ipv4Addr,
        metric: u32,
        _mtu: u32,
    ) -> Result<()> {
        self.adapter_name = name.to_string();
        self.metric = metric;
        // Prévu pour ouverture de utun (PF_SYSTEM socket)
        self.connected = true;
        Ok(())
    }

    fn read_packet(&mut self) -> Result<Option<Vec<u8>>> {
        Ok(None)
    }

    fn write_packet(&mut self, _packet: &[u8]) -> Result<()> {
        Ok(())
    }

    fn set_metric(&mut self, metric: u32) -> Result<()> {
        self.metric = metric;
        Ok(())
    }

    fn is_connected(&self) -> bool {
        self.connected
    }

    fn shutdown(&mut self) -> Result<()> {
        self.connected = false;
        Ok(())
    }
}
