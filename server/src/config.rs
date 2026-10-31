use serde::{Deserialize, Serialize};
use std::env;
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    pub udp_port: u16,
    pub web_port: u16,
    pub data_dir: String,
    pub server_name: String,
    pub admin_password: String,
    pub max_rooms: usize,
    pub max_clients_per_room: usize,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            udp_port: 30024,
            web_port: 8080,
            data_dir: "/data".to_string(),
            server_name: "CoopLink Central Server #1".to_string(),
            admin_password: "admin".to_string(),
            max_rooms: 500,
            max_clients_per_room: 32,
        }
    }
}

impl ServerConfig {
    pub fn load_from_env_or_file() -> Self {
        let mut config = Self::default();

        // Surcharge depuis les variables d'environnement (standard Docker)
        if let Ok(val) = env::var("PORT_UDP") {
            if let Ok(p) = val.parse::<u16>() {
                config.udp_port = p;
            }
        }
        if let Ok(val) = env::var("PORT_WEB") {
            if let Ok(p) = val.parse::<u16>() {
                config.web_port = p;
            }
        }
        if let Ok(val) = env::var("DATA_DIR") {
            config.data_dir = val;
        }
        if let Ok(val) = env::var("SERVER_NAME") {
            config.server_name = val;
        }
        if let Ok(val) = env::var("ADMIN_PASSWORD") {
            config.admin_password = val;
        }

        // Tentative de lecture du fichier persistant dans le data_dir bindé
        let config_file = PathBuf::from(&config.data_dir).join("server_config.json");
        if config_file.exists() {
            if let Ok(content) = fs::read_to_string(&config_file) {
                if let Ok(file_cfg) = serde_json::from_str::<ServerConfig>(&content) {
                    return file_cfg;
                }
            }
        } else {
            // Sauvegarde de la configuration initiale si possible
            let _ = fs::create_dir_all(&config.data_dir);
            if let Ok(json) = serde_json::to_string_pretty(&config) {
                let _ = fs::write(&config_file, json);
            }
        }

        config
    }
}
