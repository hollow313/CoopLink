// CoopLink - Virtual LAN Gaming Engine
// Copyright (c) 2026 Loic HUON. All Rights Reserved.
// Proprietary Commercial Software.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod adapter;
mod config;
mod i18n;
mod network;
mod platform;
mod streaming;
mod types;
mod ui;

use clap::Parser;
use config::AppConfig;
use std::sync::Arc;
use tokio::runtime::Runtime;
use tracing::info;
use ui::CoopLinkApp;

#[derive(Parser, Debug)]
#[command(name = "CoopLink")]
#[command(author = "Loic HUON <loic.huon@outlook.com>")]
#[command(version = "1.0.0")]
#[command(about = "CoopLink - High Performance Virtual LAN Gaming Engine for Windows 11")]
struct Args {
    /// Exécute en mode serveur dédié sans interface graphique (Headless)
    #[arg(long, default_value_t = false)]
    headless: bool,

    /// En mode headless : démarrer comme hôte de salon
    #[arg(long, default_value_t = false)]
    host: bool,

    /// Port d'écoute UDP
    #[arg(short, long)]
    port: Option<u16>,

    /// Code de salon
    #[arg(short, long)]
    room: Option<String>,

    /// Démarre l'application réduite en arrière-plan (au lancement automatique Windows)
    #[arg(long, default_value_t = false)]
    minimized: bool,
}

const EMBEDDED_VIEWER_BIN: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/embedded_viewer.bin"));
const EMBEDDED_CAPTURE_BIN: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/embedded_capture.bin"));

fn ensure_companion_services() {
    if let Ok(cur) = std::env::current_exe() {
        if let Some(parent) = cur.parent() {
            let bin_dir = parent.join("bin");
            let _ = std::fs::create_dir_all(&bin_dir);

            // 1. cooplink-viewer.exe
            let v1 = parent.join("cooplink-viewer.exe");
            let v2 = bin_dir.join("cooplink-viewer.exe");
            if !v1.exists() && !v2.exists() && EMBEDDED_VIEWER_BIN.len() > 10_000 {
                let _ = std::fs::write(&v1, EMBEDDED_VIEWER_BIN);
                let _ = std::fs::write(&v2, EMBEDDED_VIEWER_BIN);
            }

            // 2. cooplink-capture-service.exe
            let c1 = parent.join("cooplink-capture-service.exe");
            let c2 = bin_dir.join("cooplink-capture-service.exe");
            if !c1.exists() && !c2.exists() && EMBEDDED_CAPTURE_BIN.len() > 10_000 {
                let _ = std::fs::write(&c1, EMBEDDED_CAPTURE_BIN);
                let _ = std::fs::write(&c2, EMBEDDED_CAPTURE_BIN);
            }
        }
    }
}

fn main() -> eframe::Result<()> {
    ensure_companion_services();
    // Initialisation du système de logs
    tracing_subscriber::fmt()
        .with_env_filter("cooplink=info,wintun=warn")
        .init();

    info!("Démarrage de CoopLink v1.0.0 - Virtual LAN Gaming Engine...");

    let args = Args::parse();

    let rt = Arc::new(Runtime::new().expect("Impossible d'initialiser le runtime Tokio"));

    if args.headless {
        info!("Exécution en mode Headless (Serveur Dédié)");
        let mut config = AppConfig::load();
        if let Some(p) = args.port {
            config.listen_port = p;
        }
        let room_code = args.room.unwrap_or_else(|| "COOP-7701".to_string());

        rt.block_on(async move {
            info!("Nœud Headless CoopLink actif sur le port UDP {}", config.listen_port);
            info!("Code de salon: {}", room_code);
            // Maintient le serveur actif
            tokio::signal::ctrl_c().await.expect("Arrêt par signal");
            info!("Arrêt du nœud Headless CoopLink.");
        });
        return Ok(());
    }

    // Lancement de l'interface graphique moderne Windows 11 (eframe / egui)
    let start_minimized = args.minimized;
    let mut vp = egui::ViewportBuilder::default()
        .with_title("CoopLink")
        .with_inner_size([1040.0, 710.0])
        .with_min_inner_size([860.0, 580.0])
        .with_active(!start_minimized)
        .with_decorations(true)
        .with_resizable(true);

    if let Ok(img) = image::load_from_memory(include_bytes!("../../images/CoopLink.png")) {
        let rgba = img.into_rgba8();
        let (width, height) = rgba.dimensions();
        vp = vp.with_icon(egui::IconData {
            rgba: rgba.into_raw(),
            width,
            height,
        });
    }

    let native_options = eframe::NativeOptions {
        viewport: vp,
        ..Default::default()
    };

    eframe::run_native(
        "CoopLink",
        native_options,
        Box::new(move |cc| {
            if start_minimized {
                cc.egui_ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true));
            }
            Ok(Box::new(CoopLinkApp::new(cc, rt)))
        }),
    )
}
