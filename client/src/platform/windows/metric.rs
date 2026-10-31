use super::process::{run_hidden, run_hidden_powershell};
use anyhow::Result;
use tracing::{info, warn};

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct InterfaceMetricInfo {
    pub name: String,
    pub index: u32,
    pub metric: u32,
    pub ip_address: Option<String>,
}

/// Applique une métrique basse personnalisée à l'interface CoopLink (100% invisible en arrière-plan)
pub fn set_adapter_metric(adapter_name: &str, target_metric: u32) -> Result<()> {
    info!(
        "Application silencieuse de la métrique basse ({}) sur l'interface '{}'...",
        target_metric, adapter_name
    );

    // Méthode 1 : PowerShell Set-NetIPInterface discret
    let ps_cmd = format!(
        "Get-NetIPInterface | Where-Object {{ $_.InterfaceAlias -eq '{0}' -or $_.InterfaceDescription -like '*{0}*' }} | ForEach-Object {{ Set-NetIPInterface -InterfaceIndex $_.InterfaceIndex -InterfaceMetric {1} }}",
        adapter_name, target_metric
    );

    let output = run_hidden_powershell(&ps_cmd);

    let ps_success = match output {
        Ok(out) => {
            if out.status.success() {
                info!("Métrique appliquée avec succès via PowerShell discret !");
                true
            } else {
                let stderr = String::from_utf8_lossy(&out.stderr);
                warn!("PowerShell discret a retourné: {}", stderr);
                false
            }
        }
        Err(e) => {
            warn!("Impossible d'exécuter PowerShell: {}", e);
            false
        }
    };

    // Méthode 2 : Repli avec netsh sans fenêtre si nécessaire
    if !ps_success {
        info!("Tentative de repli via netsh discret...");
        let netsh_status = run_hidden(
            "netsh",
            &[
                "interface",
                "ipv4",
                "set",
                "interface",
                &format!("\"{}\"", adapter_name),
                &format!("metric={}", target_metric),
            ],
        );

        if let Ok(st) = netsh_status {
            if st.status.success() {
                info!("Métrique appliquée avec succès via netsh discret !");
                return Ok(());
            }
        }
    }

    Ok(())
}

/// Récupère la liste des interfaces réseau avec leurs métriques (100% silencieux)
pub fn get_network_interfaces() -> Vec<InterfaceMetricInfo> {
    let mut interfaces = Vec::new();

    let ps_cmd = "Get-NetIPInterface -AddressFamily IPv4 | Select-Object InterfaceAlias, InterfaceIndex, InterfaceMetric | ConvertTo-Json";
    if let Ok(output) = run_hidden_powershell(ps_cmd) {
        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            if let Ok(json_val) = serde_json::from_str::<serde_json::Value>(&stdout) {
                if let Some(arr) = json_val.as_array() {
                    for item in arr {
                        if let (Some(name), Some(idx), Some(met)) = (
                            item.get("InterfaceAlias").and_then(|v| v.as_str()),
                            item.get("InterfaceIndex").and_then(|v| v.as_u64()),
                            item.get("InterfaceMetric").and_then(|v| v.as_u64()),
                        ) {
                            interfaces.push(InterfaceMetricInfo {
                                name: name.to_string(),
                                index: idx as u32,
                                metric: met as u32,
                                ip_address: None,
                            });
                        }
                    }
                } else if let (Some(name), Some(idx), Some(met)) = (
                    json_val.get("InterfaceAlias").and_then(|v| v.as_str()),
                    json_val.get("InterfaceIndex").and_then(|v| v.as_u64()),
                    json_val.get("InterfaceMetric").and_then(|v| v.as_u64()),
                ) {
                    interfaces.push(InterfaceMetricInfo {
                        name: name.to_string(),
                        index: idx as u32,
                        metric: met as u32,
                        ip_address: None,
                    });
                }
            }
        }
    }

    interfaces
}

/// Vérifie si l'interface spécifiée a actuellement une métrique prioritaire (<= 10)
pub fn is_metric_priority_active(adapter_name: &str, target_metric: u32) -> bool {
    let ifaces = get_network_interfaces();
    for iface in ifaces {
        if iface.name.contains(adapter_name) {
            return iface.metric <= target_metric;
        }
    }
    false
}
