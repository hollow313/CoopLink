use crate::platform::windows::metric::InterfaceMetricInfo;
use anyhow::Result;
use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use tracing::info;

/// Vérifie l'état du pare-feu applicatif macOS (`socketfilterfw` / `pfctl`)
#[allow(dead_code)]
pub fn check_firewall_status() -> bool {
    if let Ok(exe) = env::current_exe() {
        if let Ok(out) = Command::new("/usr/libexec/ApplicationFirewall/socketfilterfw")
            .args(["--getappblocked", &exe.to_string_lossy()])
            .output()
        {
            let s = String::from_utf8_lossy(&out.stdout).to_lowercase();
            return !s.contains("blocked");
        }
    }
    true
}

/// Configure automatiquement le pare-feu macOS (`socketfilterfw` + `pfctl`) dès le premier lancement
#[allow(dead_code)]
pub fn setup_firewall_rules(listen_port: u16, subnet_cidr: &str) -> Result<bool> {
    info!("Application des règles de pare-feu macOS (socketfilterfw & pfctl) pour CoopLink...");
    if let Ok(exe) = env::current_exe() {
        let exe_str = exe.to_string_lossy().to_string();
        let _ = Command::new("/usr/libexec/ApplicationFirewall/socketfilterfw")
            .args(["--add", &exe_str])
            .output();
        let _ = Command::new("/usr/libexec/ApplicationFirewall/socketfilterfw")
            .args(["--unblockapp", &exe_str])
            .output();
    }

    // Autorisation du sous-réseau virtuel et du port UDP P2P via pfctl
    let pf_rule = format!(
        "pass in quick proto udp from any to any port {}\npass in quick from {} to any\n",
        listen_port, subnet_cidr
    );
    let _ = fs::write("/tmp/cooplink_pf.conf", pf_rule);
    let _ = Command::new("pfctl")
        .args(["-a", "com.cooplink.rules", "-f", "/tmp/cooplink_pf.conf"])
        .output();

    Ok(true)
}

/// Applique la priorité de routage (métrique basse) sur macOS (`utun5` / `route` / `networksetup`)
#[allow(dead_code)]
pub fn set_adapter_metric(adapter_name: &str, _target_metric: u32) -> Result<()> {
    let iface = if adapter_name.is_empty() || adapter_name.contains(' ') {
        "utun5"
    } else {
        adapter_name
    };

    let _ = Command::new("route")
        .args(["-n", "change", "-net", "10.244.0.0/24", "-interface", iface])
        .output();

    Ok(())
}

/// Récupère la liste des interfaces réseau macOS
#[allow(dead_code)]
pub fn get_network_interfaces() -> Vec<InterfaceMetricInfo> {
    let mut list = Vec::new();
    if let Ok(out) = Command::new("ifconfig").arg("-l").output() {
        let stdout = String::from_utf8_lossy(&out.stdout);
        for (idx, name) in stdout.split_whitespace().enumerate() {
            let metric = if name.starts_with("utun") { 5 } else { 20 };
            list.push(InterfaceMetricInfo {
                name: name.to_string(),
                index: (idx + 1) as u32,
                metric,
                ip_address: None,
            });
        }
    }
    list
}

/// Configure le démarrage automatique sous macOS via LaunchAgents (`~/Library/LaunchAgents/com.cooplink.client.plist`)
#[allow(dead_code)]
pub fn set_macos_autostart(enabled: bool) {
    if let Ok(home) = env::var("HOME") {
        let agents_dir = PathBuf::from(home).join("Library").join("LaunchAgents");
        let plist_path = agents_dir.join("com.cooplink.client.plist");
        if enabled {
            let _ = fs::create_dir_all(&agents_dir);
            let exe = env::current_exe().unwrap_or_else(|_| PathBuf::from("/Applications/CoopLink.app/Contents/MacOS/cooplink"));
            let plist = format!(
                "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
                 <!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n\
                 <plist version=\"1.0\">\n\
                 <dict>\n\
                     <key>Label</key><string>com.cooplink.client</string>\n\
                     <key>ProgramArguments</key>\n\
                     <array>\n\
                         <string>{}</string>\n\
                         <string>--minimized</string>\n\
                     </array>\n\
                     <key>RunAtLoad</key><true/>\n\
                 </dict>\n\
                 </plist>\n",
                exe.to_string_lossy()
            );
            let _ = fs::write(plist_path, plist);
        } else {
            let _ = fs::remove_file(plist_path);
        }
    }
}
