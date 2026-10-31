use anyhow::Result;
use std::env;
use std::process::Command;
use tracing::info;

const RULE_NAME_APP: &str = "CoopLink - Core Virtual LAN";
const RULE_NAME_SUBNET: &str = "CoopLink - Gaming Virtual Subnet";
const RULE_NAME_BROADCAST: &str = "CoopLink - UDP Gaming Broadcast Discovery";

/// Vérifie si les règles pare-feu de CoopLink sont déjà configurées
pub fn check_firewall_status() -> bool {
    let check_cmd = format!("netsh advfirewall firewall show rule name=\"{}\"", RULE_NAME_APP);
    let output = Command::new("cmd")
        .args(&["/C", &check_cmd])
        .output();

    if let Ok(out) = output {
        if out.status.success() {
            let stdout = String::from_utf8_lossy(&out.stdout);
            return stdout.contains(RULE_NAME_APP);
        }
    }
    false
}

/// Ajoute ou répare automatiquement les règles de pare-feu Windows pour CoopLink
pub fn setup_firewall_rules(listen_port: u16, subnet_cidr: &str) -> Result<bool> {
    info!("Configuration des règles de pare-feu Windows pour CoopLink...");

    let current_exe = env::current_exe().unwrap_or_default();
    let exe_path_str = current_exe.to_string_lossy();

    // 1. Règle pour l'application CoopLink (Inbound / Entrant)
    let add_app_in = format!(
        "netsh advfirewall firewall add rule name=\"{}\" dir=in action=allow program=\"{}\" enable=yes profile=any",
        RULE_NAME_APP, exe_path_str
    );
    let _ = Command::new("cmd").args(&["/C", &add_app_in]).output();

    // 2. Règle pour l'application CoopLink (Outbound / Sortant)
    let add_app_out = format!(
        "netsh advfirewall firewall add rule name=\"{} (Out)\" dir=out action=allow program=\"{}\" enable=yes profile=any",
        RULE_NAME_APP, exe_path_str
    );
    let _ = Command::new("cmd").args(&["/C", &add_app_out]).output();

    // 3. Règle pour le port UDP de tunneling/signalisation
    let add_port_udp = format!(
        "netsh advfirewall firewall add rule name=\"{} - UDP Port {}\" dir=in action=allow protocol=UDP localport={} enable=yes profile=any",
        RULE_NAME_APP, listen_port, listen_port
    );
    let _ = Command::new("cmd").args(&["/C", &add_port_udp]).output();

    // 4. Règle pour le sous-réseau virtuel (autorise tout le trafic entre les amis dans le sous-réseau virtuel)
    let add_subnet = format!(
        "netsh advfirewall firewall add rule name=\"{}\" dir=in action=allow protocol=ANY remoteip={} enable=yes profile=any",
        RULE_NAME_SUBNET, subnet_cidr
    );
    let _ = Command::new("cmd").args(&["/C", &add_subnet]).output();

    // 5. Règle de broadcast UDP universel pour la découverte de parties LAN
    let add_bcast = format!(
        "netsh advfirewall firewall add rule name=\"{}\" dir=in action=allow protocol=UDP localport=any remoteip=localsubnet enable=yes profile=any",
        RULE_NAME_BROADCAST
    );
    let _ = Command::new("cmd").args(&["/C", &add_bcast]).output();

    info!("Règles de pare-feu Windows appliquées avec succès !");
    Ok(true)
}

/// Supprime les règles CoopLink si nécessaire
#[allow(dead_code)]
pub fn remove_firewall_rules() -> Result<()> {
    let del_rules = [
        RULE_NAME_APP,
        &format!("{} (Out)", RULE_NAME_APP),
        RULE_NAME_SUBNET,
        RULE_NAME_BROADCAST,
    ];
    for name in &del_rules {
        let cmd = format!("netsh advfirewall firewall delete rule name=\"{}\"", name);
        let _ = Command::new("cmd").args(&["/C", &cmd]).output();
    }
    Ok(())
}
