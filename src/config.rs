use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::fs;
use std::net::Ipv4Addr;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub player_name: String,
    pub listen_port: u16,
    pub target_metric: u32,
    pub virtual_subnet_prefix: String,
    pub host_virtual_ip: Ipv4Addr,
    pub mtu: u32,
    pub auto_firewall_rule: bool,
    pub auto_metric_optimization: bool,
    pub enable_broadcast_reflector: bool,
    pub enable_audio_feedback: bool,
    pub remote_server_address: String,
    pub room_password: String,
    pub scanline_intensity: f32,
}

impl Default for AppConfig {
    fn default() -> Self {
        let random_suffix = rand::random::<u16>() % 900 + 100;
        Self {
            player_name: format!("Player_{}", random_suffix),
            listen_port: 42422,
            target_metric: 5, // Métrique ultra-basse : prend la priorité sur Wi-Fi (35-50) et Ethernet (25)
            virtual_subnet_prefix: "10.244.0".to_string(),
            host_virtual_ip: Ipv4Addr::new(10, 244, 0, 1),
            mtu: 1420,
            auto_firewall_rule: true,
            auto_metric_optimization: true,
            enable_broadcast_reflector: true,
            enable_audio_feedback: true,
            remote_server_address: "127.0.0.1:42422".to_string(),
            room_password: String::new(),
            scanline_intensity: 0.15,
        }
    }
}

impl AppConfig {
    pub fn config_path() -> PathBuf {
        let base_dir = dirs_fallback();
        base_dir.join("cooplink_config.json")
    }

    pub fn load() -> Self {
        let path = Self::config_path();
        if path.exists() {
            if let Ok(data) = fs::read_to_string(&path) {
                if let Ok(config) = serde_json::from_str::<AppConfig>(&data) {
                    return config;
                }
            }
        }
        let default_config = Self::default();
        let _ = default_config.save();
        default_config
    }

    pub fn save(&self) -> Result<()> {
        let path = Self::config_path();
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let data = serde_json::to_string_pretty(self)?;
        fs::write(path, data)?;
        Ok(())
    }
}

fn dirs_fallback() -> PathBuf {
    if let Ok(app_data) = std::env::var("APPDATA") {
        PathBuf::from(app_data).join("CoopLink")
    } else {
        PathBuf::from(".")
    }
}
