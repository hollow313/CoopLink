use crate::streaming::settings::{RateControl, StreamSettings, DEFAULT_CONTROL_PORT, DEFAULT_STREAM_PORT};
use serde::{Deserialize, Serialize};
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

/// True when the ViGEmBus driver answers, i.e. a virtual pad can actually be plugged.
pub fn vigembus_available() -> bool {
    #[cfg(windows)]
    {
        vigem_client::Client::connect().is_ok()
    }
    #[cfg(not(windows))]
    {
        false
    }
}

/// Label shown in the UI when no explicit game has been picked.
pub const FOLLOW_FOREGROUND_LABEL: &str =
    "Jeu actif au premier plan (Détection automatique DirectX / Vulkan)";

/// Splits a UI selection into `(process, title fragment)`.
///
/// `"Jeu (.exe) : Cyberpunk2077"` -> `("Cyberpunk2077.exe", "")`
/// `"Cyberpunk 2077"`            -> `("", "Cyberpunk 2077")`
/// the "follow foreground" label  -> `("", "")`
pub fn selection_to_target(selection: &str) -> (String, String) {
    let trimmed = selection.trim();
    if trimmed.is_empty() || trimmed == FOLLOW_FOREGROUND_LABEL {
        return (String::new(), String::new());
    }

    let candidate = trimmed
        .split(" : ")
        .nth(1)
        .unwrap_or(trimmed)
        .trim()
        .to_string();

    if candidate.to_lowercase().ends_with(".exe") {
        (candidate, String::new())
    } else if trimmed.to_lowercase().starts_with("jeu (.exe)") {
        // The .exe picker keeps only the file stem: match the window title instead.
        (String::new(), candidate)
    } else {
        (String::new(), candidate)
    }
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
    pub quality_min: u8,
    pub quality_max: u8,
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
    pub stream_port: u16,
    /// Relay fallback used when the guest cannot reach the host directly.
    pub relay_enabled: bool,
    pub relay_addr: String,
    pub relay_room: String,
    /// Clé de diffusion (hex) : dérivée du salon côté UI, vérifiée par le service.
    pub auth_key_hex: String,
    /// Process image of the game to capture (`Game.exe`). Empty = follow the foreground window.
    pub target_process: String,
    /// Window-title fragment used when `target_process` is empty.
    pub target_title: String,
    /// Settings actually handed to the capture service.
    pub last_settings_path: Option<PathBuf>,
    /// Signature of the last written session file, to avoid rewriting it every frame.
    pub last_settings_signature: String,
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
            dynamic_bitrate_kbps: 30_000,
            quality_min: 42,
            quality_max: 90,
            virtual_gamepad_driver_ready: vigembus_available(),
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
            service_port: DEFAULT_CONTROL_PORT,
            stream_port: DEFAULT_STREAM_PORT,
            relay_enabled: false,
            relay_addr: String::new(),
            relay_room: String::new(),
            auth_key_hex: String::new(),
            target_process: String::new(),
            target_title: String::new(),
            last_settings_path: None,
            last_settings_signature: String::new(),
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
                        u.contains("NVIDIA")
                            || u.contains("RTX")
                            || u.contains("GTX")
                            || u.contains("RADEON")
                            || u.contains("AMD")
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
                } else if all_upper.contains("INTEL")
                    || all_upper.contains("ARC")
                    || all_upper.contains("IRIS")
                    || all_upper.contains("UHD")
                {
                    self.active_encoder = HardwareEncoder::QuickSync;
                } else if gpu_upper.contains("DIRECTX 12") {
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

    /// Applique en 1 clic la Configuration Auto / Optimisée selon le débit montant mesuré et le GPU.
    ///
    /// The bitrate is derived from the measured upload (60 % of it) and clamped to
    /// what the stream engine can actually sustain, instead of being a fixed number.
    pub fn apply_auto_optimized_profile(&mut self, upload_mbps: f32) {
        self.auto_optimized_mode = true;

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

        let usable_mbps = (upload_mbps.max(5.0) * 0.6).clamp(6.0, 60.0);

        if usable_mbps >= 30.0 {
            self.target_width = 1920;
            self.target_height = 1080;
            self.target_fps = 60;
        } else if usable_mbps >= 15.0 {
            self.target_width = 1920;
            self.target_height = 1080;
            self.target_fps = 45;
        } else {
            self.target_width = 1280;
            self.target_height = 720;
            self.target_fps = 60;
        }

        self.dynamic_bitrate_kbps = (usable_mbps * 1000.0) as u32;

        // Isolation audio stricte du jeu par défaut pour éviter tout écho
        self.anti_audio_echo_enabled = true;
        self.audio_capture_mode = AudioCaptureMode::ExclusiveGameProcessOnly;
        self.exclude_voice_chat_apps = true;
        self.acoustic_echo_cancellation = true;
        self.foreground_game_only = true;
    }

    /// Adapte le débit vidéo selon la latence mesurée sur le tunnel P2P CoopLink.
    ///
    /// A latency spike must never collapse the picture: the floor stays at 60 % of
    /// the configured bitrate.
    pub fn adjust_dynamic_bitrate(&mut self, avg_ping_ms: u32) {
        let floor = (self.dynamic_bitrate_kbps as f32 * 0.6) as u32;
        let factor = if avg_ping_ms <= 20 {
            1.0
        } else if avg_ping_ms <= 45 {
            0.9
        } else if avg_ping_ms <= 80 {
            0.8
        } else {
            0.65
        };
        self.dynamic_bitrate_kbps = ((self.dynamic_bitrate_kbps as f32 * factor) as u32).max(floor);
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

    /// Settings actually sent to the capture service (JSON, one file per session).
    pub fn build_stream_settings(&self, game_selection: &str) -> StreamSettings {
        // Explicit picker selection wins; the label is only a fallback.
        let (process, title) = if self.target_process.trim().is_empty()
            && self.target_title.trim().is_empty()
        {
            selection_to_target(game_selection)
        } else {
            (self.target_process.clone(), self.target_title.clone())
        };

        let mut settings = StreamSettings {
            target_process: process,
            target_title: title,
            width: self.target_width,
            height: self.target_height,
            fps: self.target_fps,
            bitrate_kbps: self.dynamic_bitrate_kbps,
            rate_control: RateControl::Target,
            quality_min: self.quality_min,
            quality_max: self.quality_max,
            audio_enabled: !matches!(self.audio_capture_mode, AudioCaptureMode::Muted),
            audio_volume: self.stream_audio_volume.min(100) as u8,
            encoder: self.active_encoder.as_engine_str().to_string(),
            stream_port: self.stream_port,
            relay_enabled: self.relay_enabled,
            relay_addr: self.relay_addr.clone(),
            relay_room: self.relay_room.clone(),
            auth_key_hex: self.auth_key_hex.clone(),
            ..StreamSettings::default()
        };
        settings.clamp();
        settings
    }

    /// Écrit le fichier de session JSON consommé par le service de capture.
    pub fn write_stream_settings(&self, game_selection: &str) -> PathBuf {
        let settings = self.build_stream_settings(game_selection);
        let path = StreamSettings::default_path();
        if let Err(e) = settings.save_to(&path) {
            warn!("Impossible d'écrire les réglages de diffusion : {}", e);
        }
        path
    }

    /// Rewrites the session file when the user changed resolution, FPS, bitrate or
    /// target while a session is running. The service reloads it on mtime change.
    pub fn persist_if_running(&mut self) {
        if !self.is_running {
            return;
        }
        let settings = self.build_stream_settings("");
        let signature = format!(
            "{}|{}|{}x{}@{}|{}|{}|{}|{}",
            settings.target_process,
            settings.target_title,
            settings.width,
            settings.height,
            settings.fps,
            settings.bitrate_kbps,
            settings.quality_min,
            settings.quality_max,
            settings.audio_volume
        );
        if signature == self.last_settings_signature {
            return;
        }
        let path = StreamSettings::default_path();
        if settings.save_to(&path).is_ok() {
            self.last_settings_signature = signature;
            self.last_settings_path = Some(path);
            info!(
                "Réglages de diffusion mis à jour à chaud : {}x{} @ {} FPS, {:.1} Mbps",
                settings.width,
                settings.height,
                settings.fps,
                settings.bitrate_kbps as f32 / 1000.0
            );
        }
    }

    /// Démarre le service de capture du jeu ciblé.
    pub fn start_daemon(
        &mut self,
        host_virtual_ip: Ipv4Addr,
        game_selection: &str,
    ) -> anyhow::Result<()> {
        if self.is_running {
            return Ok(());
        }
        self.bound_virtual_ip = host_virtual_ip;
        let settings_path = self.write_stream_settings(game_selection);
        self.last_settings_path = Some(settings_path.clone());

        let settings = self.build_stream_settings(game_selection);
        info!(
            "Démarrage de la diffusion CoopLink sur {} : {}x{} @ {} FPS, cible {:.1} Mbps, cible de capture '{}' (encodeur {})",
            self.bound_virtual_ip,
            settings.width,
            settings.height,
            settings.fps,
            settings.bitrate_kbps as f32 / 1000.0,
            if settings.target_process.is_empty() {
                if settings.target_title.is_empty() {
                    "fenêtre au premier plan".to_string()
                } else {
                    settings.target_title.clone()
                }
            } else {
                settings.target_process.clone()
            },
            self.active_encoder.display_name(),
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

        let mut launched = false;
        for candidate in &exe_candidates {
            if candidate.exists() {
                let settings_str = settings_path.to_string_lossy().to_string();
                match crate::platform::windows::process::spawn_hidden(
                    &candidate.to_string_lossy(),
                    &[&settings_str],
                ) {
                    Ok(_) => {
                        info!("Service de capture vidéo démarré : {}", candidate.display());
                        launched = true;
                        break;
                    }
                    Err(e) => {
                        warn!(
                            "Erreur lors du démarrage du service de capture sur {} : {}",
                            candidate.display(),
                            e
                        );
                    }
                }
            }
        }

        if !launched {
            warn!("Aucun exécutable cooplink-capture-service.exe trouvé dans les chemins candidats");
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
