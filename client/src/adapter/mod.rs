use anyhow::Result;
use std::net::Ipv4Addr;

#[allow(dead_code)]
pub trait VirtualAdapter: Send + Sync {
    fn initialize(
        &mut self,
        name: &str,
        ip: Ipv4Addr,
        subnet_mask: Ipv4Addr,
        metric: u32,
        mtu: u32,
    ) -> Result<()>;

    fn read_packet(&mut self) -> Result<Option<Vec<u8>>>;
    fn write_packet(&mut self, packet: &[u8]) -> Result<()>;
    fn set_metric(&mut self, metric: u32) -> Result<()>;
    fn is_connected(&self) -> bool;
    fn shutdown(&mut self) -> Result<()>;
}

pub mod windows;
pub mod linux;
pub mod macos;

#[cfg(target_os = "windows")]
pub use windows::WindowsVirtualAdapter as NativeAdapter;

#[cfg(target_os = "linux")]
pub use linux::LinuxVirtualAdapter as NativeAdapter;

#[cfg(target_os = "macos")]
pub use macos::MacVirtualAdapter as NativeAdapter;
