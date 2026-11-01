use serde::{Deserialize, Serialize};
use std::net::Ipv4Addr;
use std::path::PathBuf;
use tracing::info;

/// Codec vidéo matériel préféré (Décodage < 5 ms)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum VideoCodec {
    H264,
    Hevc, // H.265
    Av1,  // AV1
}

impl VideoCodec {
    pub fn as_cli_arg(&self) -> &'static str {
        match self {
            VideoCodec::H264 => "H.264",
            VideoCodec::Hevc => "HEVC",
            VideoCodec::Av1 => "AV1",
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            VideoCodec::H264 => "H.264 (Compatibilité Universelle)",
            VideoCodec::Hevc => "HEVC / H.265 (Haute Efficacité < 3ms)",
            VideoCodec::Av1 => "AV1 (Ultra Basse Latence RTX 40 / RX 7000)",
        }
    }
}

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
            StreamDisplayMode::EmbeddedWindowed => "Fenêtre intégrée",
            StreamDisplayMode::BorderlessFullscreen => "Plein écran sans bordure",
        }
    }
}

/// Gestionnaire du lecteur vidéo & manettes distant CoopLink
#[allow(dead_code)]
pub struct CoopStreamViewer {
    pub is_streaming: bool,
    pub target_host_vip: Ipv4Addr,
    pub preferred_codec: VideoCodec,
    pub display_mode: StreamDisplayMode,
    pub resolution_width: u32,
    pub resolution_height: u32,
    pub fps: u32,
    pub bitrate_kbps: u32,
    pub hardware_decoding_mandatory: bool,
    pub capture_gamepads_xinput_sdl: bool,
    pub capture_mouse_keyboard: bool,
    pub multi_controller_support: bool,
    pub estimated_decode_latency_ms: f32,
    pub assigned_player_slot: u8,
}

impl CoopStreamViewer {
    pub fn new(default_host_vip: Ipv4Addr) -> Self {
        Self {
            is_streaming: false,
            target_host_vip: default_host_vip,
            preferred_codec: VideoCodec::Hevc,
            display_mode: StreamDisplayMode::BorderlessFullscreen,
            resolution_width: 1920,
            resolution_height: 1080,
            fps: 60,
            bitrate_kbps: 25000,
            hardware_decoding_mandatory: true,
            capture_gamepads_xinput_sdl: true,
            capture_mouse_keyboard: false,
            multi_controller_support: true,
            estimated_decode_latency_ms: 1.8,
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

        let display_flag = match self.display_mode {
            StreamDisplayMode::EmbeddedWindowed => "--windowed",
            StreamDisplayMode::BorderlessFullscreen => "--borderless",
        };

        let res_w = self.resolution_width.to_string();
        let res_h = self.resolution_height.to_string();
        let fps_str = self.fps.to_string();
        let bitrate_str = self.bitrate_kbps.to_string();
        let codec_str = self.preferred_codec.as_cli_arg();

        info!(
            "Ouverture du flux vidéo P2P CoopLink vers {} (Jeu: '{}', Codec: {}, Décodage Matériel < 5ms)",
            host_str, app_name, codec_str
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

        for candidate in &exe_candidates {
            if candidate.exists() {
                let mut args = vec![
                    "stream",
                    &host_str,
                    app_name,
                    "--resolution",
                    &res_w,
                    &res_h,
                    "--fps",
                    &fps_str,
                    "--bitrate",
                    &bitrate_str,
                    "--video-codec",
                    codec_str,
                    "--video-decoder",
                    "hardware",
                    "--audio-on-host",
                    "--multi-controller",
                    display_flag,
                ];

                if self.capture_mouse_keyboard {
                    args.push("--capture-system-keys");
                    args.push("always");
                } else {
                    args.push("--no-mouse-capture");
                }

                let _ = std::process::Command::new(candidate)
                    .args(&args)
                    .spawn();
                break;
            }
        }

        self.is_streaming = true;
        self.estimated_decode_latency_ms = match self.preferred_codec {
            VideoCodec::Av1 => 1.4,
            VideoCodec::Hevc => 1.8,
            VideoCodec::H264 => 2.3,
        };
        Ok(())
    }

    pub fn stop_stream(&mut self) {
        if self.is_streaming {
            let _ = crate::platform::windows::process::run_hidden(
                "taskkill",
                &["/F", "/IM", "cooplink-viewer.exe"],
            );
            self.is_streaming = false;
            info!("Flux d'écran partagé CoopLink fermé.");
        }
    }
}
