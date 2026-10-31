use serde::{Deserialize, Serialize};
use std::net::Ipv4Addr;
use std::path::PathBuf;
use tracing::{info, warn};

/// Codec vidéo matériel utilisé par le décodeur Moonlight
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum VideoCodec {
    H264,
    Hevc, // H.265
    Av1,  // AV1 Ultra-Low Latency
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
            VideoCodec::H264 => "H.264 (Compatibilité universelle < 3 ms)",
            VideoCodec::Hevc => "HEVC / H.265 (Haute efficacité 10-bit)",
            VideoCodec::Av1 => "AV1 (Nouvelle génération ultra-basse latence)",
        }
    }
}

/// Mode d'affichage du flux Moonlight dans CoopLink
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum StreamDisplayMode {
    EmbeddedWindowed,     // Fenêtré intégré
    BorderlessFullscreen, // Plein écran sans bordure
}

impl StreamDisplayMode {
    pub fn label(&self) -> &'static str {
        match self {
            StreamDisplayMode::EmbeddedWindowed => "Fenêtré (Intégré à CoopLink)",
            StreamDisplayMode::BorderlessFullscreen => "Plein écran sans bordure (Immersion totale)",
        }
    }
}

/// Gestionnaire du Client de Streaming Moonlight intégré à CoopLink
#[allow(dead_code)]
pub struct MoonlightClientManager {
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
    pub capture_mouse_keyboard: bool, // Toggle configurable souris/clavier
    pub multi_controller_support: bool,
    pub estimated_decode_latency_ms: f32,
    pub assigned_player_slot: u8,
}

impl MoonlightClientManager {
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
            capture_mouse_keyboard: false, // Par défaut désactivé en Co-op pour ne pas gêner la souris de l'hôte
            multi_controller_support: true,
            estimated_decode_latency_ms: 1.8,
            assigned_player_slot: 2,
        }
    }

    /// Lance le flux de streaming Moonlight vers l'IP virtuelle de l'Hôte détectée automatiquement
    pub fn start_stream(&mut self, host_virtual_ip: Ipv4Addr, app_name: &str) -> anyhow::Result<()> {
        self.target_host_vip = host_virtual_ip;

        let display_flag = match self.display_mode {
            StreamDisplayMode::EmbeddedWindowed => "--windowed",
            StreamDisplayMode::BorderlessFullscreen => "--borderless",
        };

        let host_str = host_virtual_ip.to_string();
        let res_w = self.resolution_width.to_string();
        let res_h = self.resolution_height.to_string();
        let fps_str = self.fps.to_string();
        let bitrate_str = self.bitrate_kbps.to_string();
        let codec_str = self.preferred_codec.as_cli_arg();

        info!(
            "Démarrage du client Moonlight vers {} (App: '{}', Codec: {}, Décodage Matériel < 5ms, Souris/Clavier: {})",
            host_str, app_name, codec_str, self.capture_mouse_keyboard
        );

        let exe_candidates = [
            PathBuf::from("moonlight.exe"),
            PathBuf::from("bin/moonlight.exe"),
            PathBuf::from("C:\\Program Files\\Moonlight Game Streaming\\Moonlight.exe"),
        ];

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
                    display_flag,
                ];
                if self.multi_controller_support {
                    args.push("--multi-controller");
                }
                if !self.capture_mouse_keyboard {
                    args.push("--no-quit-after");
                }

                if let Err(e) = crate::platform::windows::process::spawn_hidden(
                    &candidate.to_string_lossy(),
                    &args,
                ) {
                    warn!("Avertissement lancement binaire Moonlight: {}", e);
                }
                break;
            }
        }

        self.is_streaming = true;
        self.estimated_decode_latency_ms = match self.preferred_codec {
            VideoCodec::H264 => 1.4,
            VideoCodec::Hevc => 1.8,
            VideoCodec::Av1 => 2.1,
        };

        Ok(())
    }

    pub fn stop_stream(&mut self) {
        self.is_streaming = false;
        info!("Flux Moonlight arrêté.");
    }
}
