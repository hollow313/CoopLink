use crate::i18n::Language;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::fs;
use std::net::Ipv4Addr;
use std::path::PathBuf;

fn default_central_server() -> String {
    "192.168.1.200:30026".to_string()
}

fn default_instance_uuid() -> u64 {
    rand::random::<u64>() % 900_000 + 100_000
}

fn default_true() -> bool {
    true
}

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
    pub enable_upnp: bool,
    pub remote_server_address: String,
    #[serde(default = "default_central_server")]
    pub central_server_address: String,
    #[serde(default = "default_instance_uuid")]
    pub instance_uuid: u64,
    #[serde(default)]
    pub language: Language,
    #[serde(default = "default_true")]
    pub close_to_background: bool,
    #[serde(default)]
    pub close_behavior_asked: bool,
    #[serde(default)]
    pub start_with_windows: bool,
    pub room_password: String,
    pub scanline_intensity: f32,
}

impl Default for AppConfig {
    fn default() -> Self {
        let random_suffix = rand::random::<u16>() % 900 + 100;
        Self {
            player_name: format!("Player_{}", random_suffix),
            listen_port: 42422,
            target_metric: 5,
            virtual_subnet_prefix: "10.244.0".to_string(),
            host_virtual_ip: Ipv4Addr::new(10, 244, 0, 1),
            mtu: 1420,
            auto_firewall_rule: true,
            auto_metric_optimization: true,
            enable_broadcast_reflector: true,
            enable_audio_feedback: true,
            enable_upnp: true,
            remote_server_address: "127.0.0.1:42422".to_string(),
            central_server_address: default_central_server(),
            instance_uuid: default_instance_uuid(),
            language: Language::from_os_locale(),
            close_to_background: true,
            close_behavior_asked: false,
            start_with_windows: false,
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
                if let Ok(mut config) = serde_json::from_str::<AppConfig>(&data) {
                    if config.central_server_address == "127.0.0.1:30024" {
                        config.central_server_address = default_central_server();
                    }
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
