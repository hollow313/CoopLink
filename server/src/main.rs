// CoopLink - Central Coordination & Relay Server
// Copyright (c) 2026 Loic HUON. All Rights Reserved.
// Proprietary Commercial Software.

mod config;
mod hub;
mod web;

use config::ServerConfig;
use hub::ServerHub;
use std::sync::Arc;
use tracing::info;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialisation du logging
    tracing_subscriber::fmt()
        .with_env_filter("cooplink_server=info,tower_http=info")
        .init();

    info!("Démarrage de CoopLink Central Server v1.0.0...");

    let config = ServerConfig::load_from_env_or_file();
    info!("Configuration chargée : Nom='{}', Port UDP={}, Port Web={}, Répertoire Data='{}'",
        config.server_name, config.udp_port, config.web_port, config.data_dir
    );

    // Initialisation du Hub central UDP (port 30024 par défaut)
    let hub = Arc::new(ServerHub::new(config.udp_port));
    hub.clone().start().await?;

    // Lancement du Serveur Web d'Administration (port 8080 par défaut)
    let hub_for_web = hub.clone();
    let config_for_web = config.clone();
    tokio::spawn(async move {
        web::start_web_server(hub_for_web, config_for_web).await;
    });

    info!("Serveur CoopLink opérationnel ! En attente de connexions clients sur le port UDP {}...", config.udp_port);

    // Maintien en vie et gestion du signal d'arrêt propre (Docker SIGTERM)
    tokio::signal::ctrl_c().await?;
    info!("Arrêt gracieux du serveur CoopLink demandé. Au revoir !");

    Ok(())
}
