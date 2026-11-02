use crate::streaming::settings::{StreamSettings, DEFAULT_STREAM_PORT};
use serde::{Deserialize, Serialize};
use std::net::Ipv4Addr;
use std::path::PathBuf;
use tracing::info;

/// Mode d'affichage du flux distant CoopLink
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum StreamDisplayMode {
    EmbeddedWindowed,     // Mode fenêtré
    BorderlessFullscreen, // Plein écran sans bordure
}

#[allow(dead_code)]
impl StreamDisplayMode {
    pub fn label(&self) -> &'static str {
        match self {
            StreamDisplayMode::EmbeddedWindowed => "Fenêtre",
            StreamDisplayMode::BorderlessFullscreen => "Plein écran",
        }
    }
}

/// Gestionnaire du lecteur vidéo & périphériques distants CoopLink
#[allow(dead_code)]
pub struct CoopStreamViewer {
    pub is_streaming: bool,
    pub target_host_vip: Ipv4Addr,
    pub display_mode: StreamDisplayMode,
    /// What the client asks for; the host decides what it can actually send.
    pub resolution_width: u32,
    pub resolution_height: u32,
    pub fps: u32,
    pub bitrate_kbps: u32,
    pub capture_mouse_keyboard: bool,
    pub send_input: bool,
    pub multi_controller_support: bool,
    pub assigned_player_slot: u8,
}

impl CoopStreamViewer {
    pub fn new(default_host_vip: Ipv4Addr) -> Self {
        Self {
            is_streaming: false,
            target_host_vip: default_host_vip,
            display_mode: StreamDisplayMode::BorderlessFullscreen,
            resolution_width: 1920,
            resolution_height: 1080,
            fps: 60,
            bitrate_kbps: 30_000,
            capture_mouse_keyboard: false,
            send_input: true,
            multi_controller_support: true,
            assigned_player_slot: 2,
        }
    }

    /// Lance le lecteur de flux vidéo P2P CoopLink vers l'IP virtuelle de l'Hôte
    #[allow(dead_code)]
    pub fn start_stream(&mut self, host_virtual_ip: Ipv4Addr, app_name: &str) -> anyhow::Result<()> {
        self.start_stream_str(&host_virtual_ip.to_string(), app_name)
    }

    /// Lance le lecteur de flux vers n'importe quelle adresse IP cible (virtuelle, LAN ou publique)
    pub fn start_stream_str(&mut self, host_str: &str, app_name: &str) -> anyhow::Result<()> {
        if let Ok(ip) = host_str.parse::<Ipv4Addr>() {
            self.target_host_vip = ip;
        }

        let port = StreamSettings::default_path()
            .exists()
            .then(|| StreamSettings::load_or_default(&StreamSettings::default_path()).stream_port)
            .unwrap_or(DEFAULT_STREAM_PORT);

        info!(
            "Ouverture du flux CoopLink vers {}:{} ({}x{} @ {} FPS, cible {:.1} Mbps)",
            host_str,
            port,
            self.resolution_width,
            self.resolution_height,
            self.fps,
            self.bitrate_kbps as f32 / 1000.0
        );

        let mut exe_candidates = vec![
            PathBuf::from("cooplink-viewer.exe"),
            PathBuf::from("bin/cooplink-viewer.exe"),
        ];

        if let Ok(cur) = std::env::current_exe() {
            if let Some(parent) = cur.parent() {
                exe_candidates.push(parent.join("cooplink-viewer.exe"));
                exe_candidates.push(parent.join("bin").join("cooplink-viewer.exe"));
            }
        }

        let width = self.resolution_width.to_string();
        let height = self.resolution_height.to_string();
        let fps = self.fps.to_string();
        let bitrate = self.bitrate_kbps.to_string();
        let port_str = port.to_string();
        let display_flag = match self.display_mode {
            StreamDisplayMode::EmbeddedWindowed => "--windowed",
            StreamDisplayMode::BorderlessFullscreen => "--fullscreen",
        };

        let mut args: Vec<&str> = vec![
            "stream",
            "--host",
            host_str,
            "--port",
            &port_str,
            "--title",
            app_name,
            "--width",
            &width,
            "--height",
            &height,
            "--fps",
            &fps,
            "--bitrate",
            &bitrate,
            display_flag,
        ];
        if !self.send_input {
            args.push("--view-only");
        }

        for candidate in &exe_candidates {
            if candidate.exists() {
                let _ = std::process::Command::new(candidate).args(&args).spawn();
                break;
            }
        }

        self.is_streaming = true;
        Ok(())
    }

    pub fn stop_stream(&mut self) {
        if self.is_streaming {
            let _ = crate::platform::windows::process::run_hidden(
                "taskkill",
                &["/F", "/IM", "cooplink-viewer.exe"],
            );
            self.is_streaming = false;
            info!("Flux CoopLink fermé.");
        }
    }
}
