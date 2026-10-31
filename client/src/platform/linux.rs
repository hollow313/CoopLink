use crate::platform::windows::metric::InterfaceMetricInfo;
use anyhow::Result;
use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use tracing::info;

/// Vérifie si les règles de pare-feu Linux (UFW / iptables / nftables) autorisent CoopLink
#[allow(dead_code)]
pub fn check_firewall_status() -> bool {
    if let Ok(out) = Command::new("iptables").args(["-L", "INPUT", "-n"]).output() {
        if out.status.success() {
            let s = String::from_utf8_lossy(&out.stdout);
            if s.contains("42422") || s.contains("10.244.0.0/24") || s.contains("cooplink0") {
                return true;
            }
        }
    }
    if let Ok(out) = Command::new("ufw").arg("status").output() {
        if out.status.success() {
            let s = String::from_utf8_lossy(&out.stdout);
            if s.contains("42422") || s.contains("10.244.0.0/24") || s.contains("inactive") {
                return true;
            }
        }
    }
    if let Ok(out) = Command::new("nft").args(["list", "ruleset"]).output() {
        if out.status.success() {
            let s = String::from_utf8_lossy(&out.stdout);
            if s.contains("42422") || s.contains("10.244.0.0/24") {
                return true;
            }
        }
    }
    false
}

/// Configure automatiquement les règles de pare-feu sur Linux (UFW + iptables + nftables) dès le premier lancement
#[allow(dead_code)]
pub fn setup_firewall_rules(listen_port: u16, subnet_cidr: &str) -> Result<bool> {
    info!("Application des règles de pare-feu Linux (UFW / iptables / nftables) pour CoopLink...");
    let port_str = listen_port.to_string();

    // 1. UFW (Ubuntu / Debian / Mint)
    let _ = Command::new("ufw")
        .args(["allow", &format!("{}/udp", port_str), "comment", "CoopLink P2P Tunnel"])
        .output();
    let _ = Command::new("ufw")
        .args(["allow", "from", subnet_cidr, "comment", "CoopLink Virtual LAN"])
        .output();
    let _ = Command::new("ufw")
        .args(["allow", "in", "on", "cooplink0", "comment", "CoopLink Interface"])
        .output();

    // 2. iptables (Standard Linux)
    let _ = Command::new("iptables")
        .args(["-I", "INPUT", "-p", "udp", "--dport", &port_str, "-j", "ACCEPT"])
        .output();
    let _ = Command::new("iptables")
        .args(["-I", "INPUT", "-s", subnet_cidr, "-j", "ACCEPT"])
        .output();
    let _ = Command::new("iptables")
        .args(["-I", "INPUT", "-i", "cooplink0", "-j", "ACCEPT"])
        .output();

    // 3. nftables (Fedora / Arch / Kernel moderne)
    let _ = Command::new("nft")
        .args([
            "add",
            "rule",
            "inet",
            "filter",
            "input",
            "udp",
            "dport",
            &port_str,
            "accept",
        ])
        .output();

    Ok(true)
}

/// Applique la métrique basse prioritaire (ex: 5) sur l'interface TUN Linux (ip route / nmcli)
#[allow(dead_code)]
pub fn set_adapter_metric(adapter_name: &str, target_metric: u32) -> Result<()> {
    let iface = if adapter_name.is_empty() || adapter_name.contains(' ') {
        "cooplink0"
    } else {
        adapter_name
    };
    let metric_str = target_metric.to_string();

    let _ = Command::new("ip")
        .args([
            "route",
            "replace",
            "10.244.0.0/24",
            "dev",
            iface,
            "metric",
            &metric_str,
        ])
        .output();

    let _ = Command::new("nmcli")
        .args([
            "connection",
            "modify",
            iface,
            "ipv4.route-metric",
            &metric_str,
        ])
        .output();

    Ok(())
}

/// Récupère les interfaces réseau et leurs métriques sous Linux via `ip route`
#[allow(dead_code)]
pub fn get_network_interfaces() -> Vec<InterfaceMetricInfo> {
    let mut list = Vec::new();
    if let Ok(out) = Command::new("ip").args(["-o", "-4", "route", "show"]).output() {
        let stdout = String::from_utf8_lossy(&out.stdout);
        for (idx, line) in stdout.lines().enumerate() {
            let parts: Vec<&str> = line.split_whitespace().collect();
            let mut dev_name = None;
            let mut metric_val = 100u32;
            for i in 0..parts.len() {
                if parts[i] == "dev" && i + 1 < parts.len() {
                    dev_name = Some(parts[i + 1].to_string());
                }
                if parts[i] == "metric" && i + 1 < parts.len() {
                    metric_val = parts[i + 1].parse().unwrap_or(100);
                }
            }
            if let Some(name) = dev_name {
                if !list.iter().any(| existing: &InterfaceMetricInfo | existing.name == name) {
                    list.push(InterfaceMetricInfo {
                        name,
                        index: (idx + 1) as u32,
                        metric: metric_val,
                        ip_address: None,
                    });
                }
            }
        }
    }
    list
}

/// Configure le démarrage automatique sous Linux via XDG Autostart (~/.config/autostart/cooplink.desktop)
#[allow(dead_code)]
pub fn set_linux_autostart(enabled: bool) {
    if let Ok(home) = env::var("HOME") {
        let autostart_dir = PathBuf::from(home).join(".config").join("autostart");
        let desktop_file = autostart_dir.join("cooplink.desktop");
        if enabled {
            let _ = fs::create_dir_all(&autostart_dir);
            let exe = env::current_exe().unwrap_or_else(|_| PathBuf::from("/usr/bin/cooplink"));
            let content = format!(
                "[Desktop Entry]\nType=Application\nName=CoopLink\nComment=Encrypted P2P Virtual LAN & Co-op Game Sharing\nExec=\"{}\" --minimized\nTerminal=false\nStartupNotify=false\n",
                exe.to_string_lossy()
            );
            let _ = fs::write(desktop_file, content);
        } else {
            let _ = fs::remove_file(desktop_file);
        }
    }
}
