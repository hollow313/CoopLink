use super::process::run_hidden;
use anyhow::Result;
use std::env;
use tracing::info;

const RULE_NAME_APP: &str = "CoopLink - Core Virtual LAN";
const RULE_NAME_SUBNET: &str = "CoopLink - Gaming Virtual Subnet";

/// Vérifie si les règles pare-feu de CoopLink sont déjà configurées (sans aucune fenêtre CMD)
pub fn check_firewall_status() -> bool {
    let output = run_hidden(
        "netsh",
        &[
            "advfirewall",
            "firewall",
            "show",
            "rule",
            &format!("name={}", RULE_NAME_APP),
        ],
    );

    if let Ok(out) = output {
        if out.status.success() {
            let stdout = String::from_utf8_lossy(&out.stdout);
            return stdout.contains(RULE_NAME_APP);
        }
    }
    false
}

/// Ajoute ou répare automatiquement les règles de pare-feu Windows pour CoopLink (strictement bornées à CoopLink et au sous-réseau virtuel)
pub fn setup_firewall_rules(listen_port: u16, subnet_cidr: &str) -> Result<bool> {
    info!("Configuration silencieuse des règles de pare-feu Windows pour CoopLink...");

    let current_exe = env::current_exe().unwrap_or_default();
    let exe_path_str = current_exe.to_string_lossy();
    let port_str = listen_port.to_string();

    // Supprime toute ancienne règle générique éventuelle
    let _ = run_hidden(
        "netsh",
        &[
            "advfirewall",
            "firewall",
            "delete",
            "rule",
            "name=CoopLink - UDP Gaming Broadcast Discovery",
        ],
    );

    // 1. Règle pour l'application CoopLink (Inbound / Entrant)
    let _ = run_hidden(
        "netsh",
        &[
            "advfirewall",
            "firewall",
            "add",
            "rule",
            &format!("name={}", RULE_NAME_APP),
            "dir=in",
            "action=allow",
            &format!("program={}", exe_path_str),
            "enable=yes",
            "profile=any",
        ],
    );

    // 2. Règle pour l'application CoopLink (Outbound / Sortant)
    let _ = run_hidden(
        "netsh",
        &[
            "advfirewall",
            "firewall",
            "add",
            "rule",
            &format!("name={} (Out)", RULE_NAME_APP),
            "dir=out",
            "action=allow",
            &format!("program={}", exe_path_str),
            "enable=yes",
            "profile=any",
        ],
    );

    // 3. Règle strictement limitée au port UDP P2P de CoopLink (ex: 42422)
    let _ = run_hidden(
        "netsh",
        &[
            "advfirewall",
            "firewall",
            "add",
            "rule",
            &format!("name={} - UDP Port {}", RULE_NAME_APP, port_str),
            "dir=in",
            "action=allow",
            "protocol=UDP",
            &format!("localport={}", port_str),
            "enable=yes",
            "profile=any",
        ],
    );

    // 4. Règle strictement limitée au sous-réseau virtuel chiffré (10.244.0.0/24)
    let _ = run_hidden(
        "netsh",
        &[
            "advfirewall",
            "firewall",
            "add",
            "rule",
            &format!("name={}", RULE_NAME_SUBNET),
            "dir=in",
            "action=allow",
            "protocol=ANY",
            &format!("remoteip={}", subnet_cidr),
            "enable=yes",
            "profile=any",
        ],
    );

    info!("Règles de pare-feu Windows appliquées en arrière-plan (bornées au port UDP {} et au sous-réseau {}) !", port_str, subnet_cidr);
    Ok(true)
}

/// Supprime les règles CoopLink si nécessaire
#[allow(dead_code)]
pub fn remove_firewall_rules() -> Result<()> {
    let del_rules = [
        RULE_NAME_APP,
        &format!("{} (Out)", RULE_NAME_APP),
        RULE_NAME_SUBNET,
    ];
    for name in &del_rules {
        let _ = run_hidden(
            "netsh",
            &[
                "advfirewall",
                "firewall",
                "delete",
                "rule",
                &format!("name={}", name),
            ],
        );
    }
    Ok(())
}
