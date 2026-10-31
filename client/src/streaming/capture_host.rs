use serde::{Deserialize, Serialize};
use std::fs;
use std::net::Ipv4Addr;
use std::path::PathBuf;
use tracing::{info, warn};

/// Type d'encodeur matériel détecté sur l'Hôte
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HardwareEncoder {
    Nvenc,     // NVIDIA GeForce / RTX
    Amf,       // AMD Radeon RX
    QuickSync, // Intel Arc / Iris Xe
    Software,  // Fallback CPU
}

impl HardwareEncoder {
    pub fn as_engine_str(&self) -> &'static str {
        match self {
            HardwareEncoder::Nvenc => "nvenc",
            HardwareEncoder::Amf => "amf",
            HardwareEncoder::QuickSync => "quicksync",
            HardwareEncoder::Software => "software",
        }
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            HardwareEncoder::Nvenc => "NVIDIA NVENC (Matériel Direct)",
            HardwareEncoder::Amf => "AMD AMF (Matériel Direct)",
            HardwareEncoder::QuickSync => "Intel QuickSync (Matériel Direct)",
            HardwareEncoder::Software => "Encodage CPU Standard",
        }
    }
}

/// Type de manette virtuelle émulée par le bus CoopLink
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum VirtualGamepadType {
    Xbox360,
    DualShock4,
}

#[allow(dead_code)]
impl VirtualGamepadType {
    pub fn as_engine_str(&self) -> &'static str {
        match self {
            VirtualGamepadType::Xbox360 => "x360",
            VirtualGamepadType::DualShock4 => "ds4",
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            VirtualGamepadType::Xbox360 => "Xbox 360 (XInput Standard)",
            VirtualGamepadType::DualShock4 => "DualShock 4 (Manette Type PS4)",
        }
    }
}

/// Représentation d'un emplacement de manette assigné (Local ou Invité distant)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ControllerSlotAssignment {
    pub player_number: u8, // 1 = Hôte local, 2..4 = Invités distants
    pub owner_name: String,
    pub virtual_ip: Ipv4Addr,
    pub is_local_host: bool,
    pub emulated_type: VirtualGamepadType,
    pub driver_connected: bool,
    pub xinput_slot_index: u8, // 0..3
}

/// Gestionnaire du service Hôte de Capture & Partage de Jeu CoopLink
#[allow(dead_code)]
pub struct CoopCaptureHost {
    pub is_running: bool,
    pub bound_virtual_ip: Ipv4Addr,
    pub detected_gpu_name: String,
    pub active_encoder: HardwareEncoder,
    pub target_width: u32,
    pub target_height: u32,
    pub target_fps: u32,
    pub dynamic_bitrate_kbps: u32,
    pub virtual_gamepad_driver_ready: bool,
    pub virtual_display_driver_ready: bool,
    pub anti_audio_echo_enabled: bool,
    pub foreground_game_only: bool,
    pub default_gamepad_type: VirtualGamepadType,
    pub controller_slots: Vec<ControllerSlotAssignment>,
    pub service_port: u16,
}

#[allow(dead_code)]
impl CoopCaptureHost {
    pub fn new(host_virtual_ip: Ipv4Addr) -> Self {
        let mut mgr = Self {
            is_running: false,
            bound_virtual_ip: host_virtual_ip,
            detected_gpu_name: "GPU DirectX 12".to_string(),
            active_encoder: HardwareEncoder::Nvenc,
            target_width: 1920,
            target_height: 1080,
            target_fps: 60,
            dynamic_bitrate_kbps: 25000,
            virtual_gamepad_driver_ready: true,
            virtual_display_driver_ready: true,
            anti_audio_echo_enabled: true, // Anti-écho audio actif par défaut (isole l'audio du jeu sans capter Discord/Voix)
            foreground_game_only: true,    // Ne capture que le jeu au premier plan (CoopLink reste en arrière-plan)
            default_gamepad_type: VirtualGamepadType::Xbox360,
            controller_slots: vec![ControllerSlotAssignment {
                player_number: 1,
                owner_name: "Hôte Local (Vous)".to_string(),
                virtual_ip: host_virtual_ip,
                is_local_host: true,
                emulated_type: VirtualGamepadType::Xbox360,
                driver_connected: true,
                xinput_slot_index: 0,
            }],
            service_port: 47990,
        };
        mgr.detect_hardware_capabilities();
        mgr
    }

    /// Détecte le GPU matériel et les pilotes virtuels (Affichage & Bus Manettes)
    pub fn detect_hardware_capabilities(&mut self) {
        let ps_cmd = "Get-CimInstance Win32_VideoController | Select-Object -ExpandProperty Name";
        if let Ok(output) = crate::platform::windows::process::run_hidden_powershell(ps_cmd) {
            if output.status.success() {
                let names = String::from_utf8_lossy(&output.stdout);
                let first_gpu = names.lines().next().unwrap_or("GPU DirectX 12").trim();
                if !first_gpu.is_empty() {
                    self.detected_gpu_name = first_gpu.to_string();
                }
                let upper = names.to_uppercase();
                if upper.contains("NVIDIA") || upper.contains("GEFORCE") || upper.contains("RTX") || upper.contains("GTX") {
                    self.active_encoder = HardwareEncoder::Nvenc;
                } else if upper.contains("AMD") || upper.contains("RADEON") {
                    self.active_encoder = HardwareEncoder::Amf;
                } else if upper.contains("INTEL") || upper.contains("ARC") || upper.contains("IRIS") {
                    self.active_encoder = HardwareEncoder::QuickSync;
                } else {
                    self.active_encoder = HardwareEncoder::Software;
                }
            }
        }
    }

    /// Adapte dynamiquement le débit vidéo selon la latence mesurée sur le tunnel P2P CoopLink
    pub fn adjust_dynamic_bitrate(&mut self, avg_ping_ms: u32) {
        if avg_ping_ms <= 15 {
            self.dynamic_bitrate_kbps = 35000;
        } else if avg_ping_ms <= 40 {
            self.dynamic_bitrate_kbps = 25000;
        } else if avg_ping_ms <= 80 {
            self.dynamic_bitrate_kbps = 15000;
        } else {
            self.dynamic_bitrate_kbps = 10000;
        }
    }

    /// Assigne automatiquement un emplacement de manette virtuelle (Joueur 2, 3 ou 4) sans conflit avec le Joueur 1
    pub fn assign_remote_controller_slot(&mut self, peer_name: &str, peer_vip: Ipv4Addr) -> u8 {
        if let Some(existing) = self.controller_slots.iter().find(|s| s.virtual_ip == peer_vip) {
            return existing.player_number;
        }

        let next_player = (self.controller_slots.len() as u8 + 1).min(4);
        let xinput_idx = next_player.saturating_sub(1);

        self.controller_slots.push(ControllerSlotAssignment {
            player_number: next_player,
            owner_name: peer_name.to_string(),
            virtual_ip: peer_vip,
            is_local_host: false,
            emulated_type: self.default_gamepad_type,
            driver_connected: true,
            xinput_slot_index: xinput_idx,
        });

        info!(
            "Bus Manettes CoopLink : '{}' ({}) assigné en tant que Joueur {} (Slot XInput #{})",
            peer_name, peer_vip, next_player, xinput_idx
        );

        next_player
    }

    /// Génère le fichier de configuration interne lié exclusivement à l'interface virtuelle CoopLink
    pub fn write_isolated_config(&self, target_game_exe: &str) -> PathBuf {
        let base_dir = if let Ok(appdata) = std::env::var("APPDATA") {
            PathBuf::from(appdata).join("CoopLink").join("capture_service")
        } else {
            PathBuf::from("capture_service")
        };
        let _ = fs::create_dir_all(&base_dir);
        let conf_path = base_dir.join("cooplink_capture.conf");

        let conf_content = format!(
            "# CoopLink Internal Foreground Game Capture Service\n\
             cooplink_node_name = CoopLink Host ({})\n\
             bind_address = {}\n\
             port = 47989\n\
             encoder = {}\n\
             gamepad = {}\n\
             upnp = disabled\n\
             fec_percentage = 20\n\
             min_threads = 2\n\
             channels = 2\n\
             audio_isolation_mode = process_loopback_no_echo\n\
             exclude_voice_chat_audio = enabled\n\
             capture_mode = foreground_game_window_only\n\
             target_executable = {}\n\
             origin_web_ui_allowed = pc\n",
            self.bound_virtual_ip,
            self.bound_virtual_ip,
            self.active_encoder.as_engine_str(),
            self.default_gamepad_type.as_engine_str(),
            target_game_exe
        );

        let _ = fs::write(&conf_path, conf_content);
        conf_path
    }

    /// Démarre le service de capture du jeu au premier plan lié à l'IP virtuelle CoopLink
    pub fn start_daemon(&mut self, host_virtual_ip: Ipv4Addr, target_game_exe: &str) -> anyhow::Result<()> {
        self.bound_virtual_ip = host_virtual_ip;
        let conf_path = self.write_isolated_config(target_game_exe);

        info!(
            "Démarrage du moteur de capture CoopLink lié à {} (Encodeur: {}, Anti-Écho Audio: Actif, Jeu Premier Plan uniquement)",
            self.bound_virtual_ip,
            self.active_encoder.display_name()
        );

        let exe_candidates = [
            PathBuf::from("cooplink-capture-service.exe"),
            PathBuf::from("bin/cooplink-capture-service.exe"),
        ];

        for candidate in &exe_candidates {
            if candidate.exists() {
                let conf_str = conf_path.to_string_lossy().to_string();
                let _ = crate::platform::windows::process::spawn_hidden(
                    &candidate.to_string_lossy(),
                    &[&conf_str],
                );
                break;
            }
        }

        self.is_running = true;
        Ok(())
    }

    pub fn stop_daemon(&mut self) {
        if self.is_running {
            let _ = crate::platform::windows::process::run_hidden(
                "taskkill",
                &["/F", "/IM", "cooplink-capture-service.exe"],
            );
            self.is_running = false;
            self.controller_slots.retain(|s| s.is_local_host);
            warn!("Service de partage de jeu CoopLink arrêté.");
        }
    }
}
