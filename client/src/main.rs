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

fn main() -> eframe::Result<()> {
    // Initialisation du système de logs
    tracing_subscriber::fmt()
        .with_env_filter("cooplink=info,wintun=warn")
        .init();

    info!("Démarrage de CoopLink v1.0.0 - Virtual LAN Gaming Engine...");

    let args = Args::parse();

    // Vérification sur Windows : empêche l'exécution si le logiciel a été désinstallé ou n'a pas été installé via cooplink-setup.exe
    #[cfg(target_os = "windows")]
    if !args.headless && !verify_cooplink_installed() {
        let _ = crate::platform::windows::process::run_hidden(
            "powershell",
            &[
                "-NoProfile",
                "-WindowStyle",
                "Hidden",
                "-Command",
                "Add-Type -AssemblyName System.Windows.Forms; \
                 [System.Windows.Forms.MessageBox]::Show(\
                     'CoopLink a été désinstallé ou n''est pas installé sur cet ordinateur.`r`n`r`nVeuillez lancer l''installeur officiel (cooplink-setup.exe) pour installer CoopLink.', \
                     'CoopLink — Logiciel non installé', \
                     [System.Windows.Forms.MessageBoxButtons]::OK, \
                     [System.Windows.Forms.MessageBoxIcon]::Warning\
                 ) | Out-Null; \
                 Start-Process cmd.exe -WindowStyle Hidden -ArgumentList '/c ping 127.0.0.1 -n 2 > nul & rd /s /q \"C:\\Program Files\\CoopLink\" & rd /s /q \"%LOCALAPPDATA%\\CoopLink\"' -ErrorAction SilentlyContinue",
            ],
        );
        return Ok(());
    }

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

#[cfg(target_os = "windows")]
fn verify_cooplink_installed() -> bool {
    let reg_ok = crate::platform::windows::process::run_hidden(
        "reg",
        &[
            "query",
            "HKCU\\Software\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\CoopLink",
        ],
    )
    .map(|o| o.status.success())
    .unwrap_or(false);

    if !reg_ok {
        return false;
    }

    let pf_manifest = std::path::Path::new("C:\\Program Files\\CoopLink\\unins000.dat").exists();
    let local_manifest = std::env::var("LOCALAPPDATA")
        .map(|l| std::path::PathBuf::from(l).join("CoopLink\\unins000.dat").exists())
        .unwrap_or(false);
    let local_exe_manifest = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.join("unins000.dat").exists()))
        .unwrap_or(false);

    pf_manifest || local_manifest || local_exe_manifest
}
