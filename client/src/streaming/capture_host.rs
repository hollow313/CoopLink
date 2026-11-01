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

    pub fn short_badge(&self) -> &'static str {
        match self {
            HardwareEncoder::Nvenc => "Matériel GPU (NVIDIA NVENC)",
            HardwareEncoder::Amf => "Matériel GPU (AMD AMF)",
            HardwareEncoder::QuickSync => "Matériel GPU (Intel QuickSync)",
            HardwareEncoder::Software => "Standard CPU",
        }
    }

    pub fn is_hardware(&self) -> bool {
        !matches!(self, HardwareEncoder::Software)
    }
}

/// Mode d'isolation et de capture audio avancé (Zéro Écho)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AudioCaptureMode {
    /// Capture exclusivement le flux audio WASAPI Process Loopback de l'exécutable du jeu choisi (0% écho Discord/Micro)
    ExclusiveGameProcessOnly,
    /// Capture le périphérique audio principal en filtrant les processus de chat vocal (Discord, TeamSpeak, Steam Voice)
    FilteredSystemAudio,
    /// Capture un périphérique de sortie audio spécifique sélectionné par l'utilisateur
    SpecificAudioDevice,
    /// Coupe totalement l'envoi du son vers les invités
    Muted,
}

impl AudioCaptureMode {
    pub fn as_conf_str(&self) -> &'static str {
        match self {
            AudioCaptureMode::ExclusiveGameProcessOnly => "exclusive_game_process_loopback",
            AudioCaptureMode::FilteredSystemAudio => "filtered_system_loopback_no_voice",
            AudioCaptureMode::SpecificAudioDevice => "specific_audio_endpoint",
            AudioCaptureMode::Muted => "muted",
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

/// Vérifie si un nom d'adaptateur vidéo correspond à un écran virtuel (et non à un GPU physique)
pub fn is_virtual_display_adapter_name(name: &str) -> bool {
    let u = name.to_uppercase();
    u.contains("SUDOMAKER")
        || u.contains("VIRTUAL DISPLAY")
        || u.contains("VIRTUAL MONITOR")
        || u.contains("PARSEC")
        || u.contains("IDDSAMPLE")
        || u.contains("INDIRECT DISPLAY")
        || u.contains("MICROSOFT BASIC")
        || u.contains("SPACEDESK")
        || u.contains("CITRIX")
        || u.contains("REMOTE DESKTOP")
}

/// Gestionnaire du service Hôte de Capture & Partage de Jeu CoopLink
#[allow(dead_code)]
pub struct CoopCaptureHost {
    pub is_running: bool,
    pub bound_virtual_ip: Ipv4Addr,
    pub detected_gpu_name: String,
    pub available_physical_gpus: Vec<String>,
    pub detected_virtual_display_name: Option<String>,
    pub active_encoder: HardwareEncoder,
    pub target_width: u32,
    pub target_height: u32,
    pub target_fps: u32,
    pub dynamic_bitrate_kbps: u32,
    pub virtual_gamepad_driver_ready: bool,
    pub virtual_display_driver_ready: bool,
    // Options Audio Avancées (Isolation stricte du son du jeu)
    pub anti_audio_echo_enabled: bool,
    pub audio_capture_mode: AudioCaptureMode,
    pub exclude_voice_chat_apps: bool,
    pub acoustic_echo_cancellation: bool,
    pub stream_audio_volume: u32,
    pub selected_audio_device: String,
    pub available_audio_devices: Vec<String>,
    pub foreground_game_only: bool,
    pub auto_optimized_mode: bool,
    pub default_gamepad_type: VirtualGamepadType,
    pub controller_slots: Vec<ControllerSlotAssignment>,
    pub service_port: u16,
}

#[allow(dead_code)]
impl CoopCaptureHost {
    pub fn new(host_virtual_ip: Ipv4Addr) -> Self {
        let default_audio_dev = "Sortie Audio DirectX du Jeu (Isolation Processus)".to_string();
        let mut mgr = Self {
            is_running: false,
            bound_virtual_ip: host_virtual_ip,
            detected_gpu_name: "Carte Graphique Matérielle DirectX 12".to_string(),
            available_physical_gpus: Vec::new(),
            detected_virtual_display_name: None,
            active_encoder: HardwareEncoder::Nvenc,
            target_width: 1920,
            target_height: 1080,
            target_fps: 60,
            dynamic_bitrate_kbps: 30000,
            virtual_gamepad_driver_ready: true,
            virtual_display_driver_ready: true,
            anti_audio_echo_enabled: true,
            audio_capture_mode: AudioCaptureMode::ExclusiveGameProcessOnly,
            exclude_voice_chat_apps: true,
            acoustic_echo_cancellation: true,
            stream_audio_volume: 100,
            selected_audio_device: default_audio_dev.clone(),
            available_audio_devices: vec![
                default_audio_dev,
                "Casque / Haut-parleurs Principaux (Sans Discord)".to_string(),
            ],
            foreground_game_only: true,
            auto_optimized_mode: true,
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

    /// Détecte les vrais GPU matériels (en filtrant les écrans virtuels comme SudoMaker Virtual Display Adapter)
    /// ainsi que les périphériques audio disponibles.
    pub fn detect_hardware_capabilities(&mut self) {
        let ps_cmd = "Get-CimInstance Win32_VideoController | Select-Object -ExpandProperty Name";
        if let Ok(output) = crate::platform::windows::process::run_hidden_powershell(ps_cmd) {
            if output.status.success() {
                let raw = String::from_utf8_lossy(&output.stdout);
                let mut physical_gpus = Vec::new();
                let mut virtual_disp = None;

                for line in raw.lines() {
                    let trimmed = line.trim();
                    if trimmed.is_empty() {
                        continue;
                    }
                    if is_virtual_display_adapter_name(trimmed) {
                        virtual_disp = Some(trimmed.to_string());
                    } else if !physical_gpus.contains(&trimmed.to_string()) {
                        physical_gpus.push(trimmed.to_string());
                    }
                }

                self.detected_virtual_display_name = virtual_disp.clone();

                // Choisir en priorité une carte dédiée NVIDIA ou AMD, sinon Intel, sinon le premier GPU physique
                let preferred_physical = physical_gpus
                    .iter()
                    .find(|g| {
                        let u = g.to_uppercase();
                        u.contains("NVIDIA") || u.contains("RTX") || u.contains("GTX") || u.contains("RADEON") || u.contains("AMD")
                    })
                    .or_else(|| physical_gpus.first())
                    .cloned();

                if let Some(phys) = preferred_physical {
                    self.detected_gpu_name = phys;
                } else if let Some(vdd) = virtual_disp {
                    // Si la machine ne remonte que l'adaptateur virtuel dans Win32_VideoController,
                    // on conserve l'accélération matérielle DirectX 12 sous-jacente
                    self.detected_gpu_name = format!("GPU Matériel DirectX 12 ({})", vdd);
                }

                self.available_physical_gpus = physical_gpus;

                let all_upper = raw.to_uppercase();
                let gpu_upper = self.detected_gpu_name.to_uppercase();
                if all_upper.contains("NVIDIA")
                    || all_upper.contains("GEFORCE")
                    || all_upper.contains("RTX")
                    || all_upper.contains("GTX")
                {
                    self.active_encoder = HardwareEncoder::Nvenc;
                } else if all_upper.contains("AMD") || all_upper.contains("RADEON") {
                    self.active_encoder = HardwareEncoder::Amf;
                } else if all_upper.contains("INTEL") || all_upper.contains("ARC") || all_upper.contains("IRIS") || all_upper.contains("UHD") {
                    self.active_encoder = HardwareEncoder::QuickSync;
                } else if gpu_upper.contains("DIRECTX 12") {
                    // Par défaut sur Windows 11 avec écran virtuel SudoMaker, l'encodeur matériel GPU hôte est actif
                    self.active_encoder = HardwareEncoder::Nvenc;
                } else {
                    self.active_encoder = HardwareEncoder::Software;
                }
            }
        }

        // Détection silencieuse des périphériques audio Windows
        let ps_audio = "Get-CimInstance Win32_SoundDevice | Select-Object -ExpandProperty Name";
        if let Ok(out) = crate::platform::windows::process::run_hidden_powershell(ps_audio) {
            if out.status.success() {
                let raw_audio = String::from_utf8_lossy(&out.stdout);
                for line in raw_audio.lines() {
                    let dev = line.trim();
                    if !dev.is_empty() && !self.available_audio_devices.contains(&dev.to_string()) {
                        self.available_audio_devices.push(dev.to_string());
                    }
                }
            }
        }
    }

    /// Applique en 1 clic la Configuration Auto / Optimisée selon le débit montant mesuré et le GPU
    pub fn apply_auto_optimized_profile(&mut self, upload_mbps: f32) {
        self.auto_optimized_mode = true;
        // Toujours activer l'encodeur matériel GPU s'il était sur Software par erreur
        if self.active_encoder == HardwareEncoder::Software {
            let u = self.detected_gpu_name.to_uppercase();
            if u.contains("AMD") || u.contains("RADEON") {
                self.active_encoder = HardwareEncoder::Amf;
            } else if u.contains("INTEL") || u.contains("ARC") || u.contains("IRIS") {
                self.active_encoder = HardwareEncoder::QuickSync;
            } else {
                self.active_encoder = HardwareEncoder::Nvenc;
            }
        }

        // Adaptation intelligente Résolution / FPS / Bitrate selon le débit montant mesuré
        if upload_mbps >= 45.0 {
            self.target_width = 1920;
            self.target_height = 1080;
            self.target_fps = 60;
            self.dynamic_bitrate_kbps = 35000;
        } else if upload_mbps >= 20.0 {
            self.target_width = 1920;
            self.target_height = 1080;
            self.target_fps = 60;
            self.dynamic_bitrate_kbps = 22000;
        } else {
            self.target_width = 1280;
            self.target_height = 720;
            self.target_fps = 60;
            self.dynamic_bitrate_kbps = 12000;
        }

        // Isolation audio stricte du jeu par défaut pour éviter tout écho
        self.anti_audio_echo_enabled = true;
        self.audio_capture_mode = AudioCaptureMode::ExclusiveGameProcessOnly;
        self.exclude_voice_chat_apps = true;
        self.acoustic_echo_cancellation = true;
        self.foreground_game_only = true;
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
             audio_isolation_mode = {}\n\
             audio_target_device = {}\n\
             audio_volume_percent = {}\n\
             exclude_voice_chat_audio = {}\n\
             acoustic_echo_cancellation = {}\n\
             capture_mode = foreground_game_window_only\n\
             target_executable = {}\n\
             origin_web_ui_allowed = pc\n",
            self.bound_virtual_ip,
            self.bound_virtual_ip,
            self.active_encoder.as_engine_str(),
            self.default_gamepad_type.as_engine_str(),
            self.audio_capture_mode.as_conf_str(),
            self.selected_audio_device,
            self.stream_audio_volume,
            if self.exclude_voice_chat_apps { "enabled" } else { "disabled" },
            if self.acoustic_echo_cancellation { "enabled" } else { "disabled" },
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
            "Démarrage du moteur de capture CoopLink lié à {} (Encodeur: {}, Mode Audio: {}, Jeu Premier Plan uniquement)",
            self.bound_virtual_ip,
            self.active_encoder.display_name(),
            self.audio_capture_mode.as_conf_str()
        );

        let mut exe_candidates = vec![
            PathBuf::from("cooplink-capture-service.exe"),
            PathBuf::from("bin/cooplink-capture-service.exe"),
        ];

        if let Ok(cur) = std::env::current_exe() {
            if let Some(parent) = cur.parent() {
                exe_candidates.push(parent.join("cooplink-capture-service.exe"));
                exe_candidates.push(parent.join("bin").join("cooplink-capture-service.exe"));
            }
        }

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
