pub mod windows;
pub mod linux;
pub mod macos;

/// Structure multi-plateforme (Windows / macOS / Linux) pour l'affichage des cartes réseau et de leur métrique
pub use windows::metric::InterfaceMetricInfo;

/// Vérifie l'état du pare-feu sur l'OS courant (Windows Defender, macOS Application Firewall/pfctl, Linux UFW/iptables/nftables)
pub fn check_os_firewall_status() -> bool {
    #[cfg(target_os = "windows")]
    {
        windows::firewall::check_firewall_status()
    }
    #[cfg(target_os = "linux")]
    {
        linux::check_firewall_status()
    }
    #[cfg(target_os = "macos")]
    {
        macos::check_firewall_status()
    }
}

/// Applique dès le premier lancement les règles de pare-feu sur Windows, macOS et Linux
pub fn setup_os_firewall_rules(listen_port: u16, subnet_cidr: &str) -> anyhow::Result<bool> {
    #[cfg(target_os = "windows")]
    {
        windows::firewall::setup_firewall_rules(listen_port, subnet_cidr)
    }
    #[cfg(target_os = "linux")]
    {
        linux::setup_firewall_rules(listen_port, subnet_cidr)
    }
    #[cfg(target_os = "macos")]
    {
        macos::setup_firewall_rules(listen_port, subnet_cidr)
    }
}

/// Applique la métrique basse prioritaire (ex: 5) sur Windows, macOS et Linux
pub fn set_os_adapter_metric(adapter_name: &str, target_metric: u32) -> anyhow::Result<()> {
    #[cfg(target_os = "windows")]
    {
        windows::metric::set_adapter_metric(adapter_name, target_metric)
    }
    #[cfg(target_os = "linux")]
    {
        linux::set_adapter_metric(adapter_name, target_metric)
    }
    #[cfg(target_os = "macos")]
    {
        macos::set_adapter_metric(adapter_name, target_metric)
    }
}

/// Liste les interfaces réseau et leurs métriques sur Windows, macOS et Linux
pub fn get_os_network_interfaces() -> Vec<InterfaceMetricInfo> {
    #[cfg(target_os = "windows")]
    {
        windows::metric::get_network_interfaces()
    }
    #[cfg(target_os = "linux")]
    {
        linux::get_network_interfaces()
    }
    #[cfg(target_os = "macos")]
    {
        macos::get_network_interfaces()
    }
}

/// Active ou désactive le démarrage automatique avec l'OS (Registre Windows, LaunchAgent macOS, XDG Autostart Linux)
pub fn set_os_autostart(enabled: bool) {
    #[cfg(target_os = "windows")]
    {
        windows::process::set_windows_autostart(enabled);
    }
    #[cfg(target_os = "linux")]
    {
        linux::set_linux_autostart(enabled);
    }
    #[cfg(target_os = "macos")]
    {
        macos::set_macos_autostart(enabled);
    }
}
