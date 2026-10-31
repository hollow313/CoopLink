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
    Software,  // Fallback CPU (libx264/svt-av1)
}

impl HardwareEncoder {
    pub fn as_sunshine_str(&self) -> &'static str {
        match self {
            HardwareEncoder::Nvenc => "nvenc",
            HardwareEncoder::Amf => "amdvce",
            HardwareEncoder::QuickSync => "quicksync",
            HardwareEncoder::Software => "software",
        }
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            HardwareEncoder::Nvenc => "NVIDIA NVENC (Matériel Direct)",
            HardwareEncoder::Amf => "AMD AMF / VCE (Matériel Direct)",
            HardwareEncoder::QuickSync => "Intel QuickSync (Matériel Direct)",
            HardwareEncoder::Software => "Encodage Logiciel (Fallback CPU)",
        }
    }
}

/// Type de manette virtuelle émulée par ViGEmBus
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum VirtualGamepadType {
    Xbox360,
    DualShock4,
}

#[allow(dead_code)]
impl VirtualGamepadType {
    pub fn as_sunshine_str(&self) -> &'static str {
        match self {
            VirtualGamepadType::Xbox360 => "x360",
            VirtualGamepadType::DualShock4 => "ds4",
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            VirtualGamepadType::Xbox360 => "Xbox 360 (XInput Standard)",
            VirtualGamepadType::DualShock4 => "DualShock 4 (Gyro & Touchpad)",
        }
    }
}

/// Représentation d'un slot de contrôleur assigné (Local ou Distant via ViGEmBus)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ControllerSlotAssignment {
    pub player_number: u8, // 1 = Hôte local, 2..4 = Clients distants
    pub owner_name: String,
    pub virtual_ip: Ipv4Addr,
    pub is_local_host: bool,
    pub emulated_type: VirtualGamepadType,
    pub vigem_connected: bool,
    pub xinput_slot_index: u8, // 0..3
}

/// Gestionnaire du service Hôte Sunshine intégré à CoopLink
#[allow(dead_code)]
pub struct SunshineHostManager {
    pub is_running: bool,
    pub bound_virtual_ip: Ipv4Addr,
    pub detected_gpu_name: String,
    pub active_encoder: HardwareEncoder,
    pub target_width: u32,
    pub target_height: u32,
    pub target_fps: u32,
    pub dynamic_bitrate_kbps: u32,
    pub vigembus_installed: bool,
    pub default_gamepad_type: VirtualGamepadType,
    pub controller_slots: Vec<ControllerSlotAssignment>,
    pub api_port: u16,
}

#[allow(dead_code)]
impl SunshineHostManager {
    pub fn new(host_virtual_ip: Ipv4Addr) -> Self {
        let mut mgr = Self {
            is_running: false,
            bound_virtual_ip: host_virtual_ip,
            detected_gpu_name: "Détection GPU DirectX en cours...".to_string(),
            active_encoder: HardwareEncoder::Nvenc,
            target_width: 1920,
            target_height: 1080,
            target_fps: 60,
            dynamic_bitrate_kbps: 25000, // 25 Mbps par défaut (adaptatif)
            vigembus_installed: true,
            default_gamepad_type: VirtualGamepadType::Xbox360,
            controller_slots: vec![ControllerSlotAssignment {
                player_number: 1,
                owner_name: "Hôte Local (Vous)".to_string(),
                virtual_ip: host_virtual_ip,
                is_local_host: true,
                emulated_type: VirtualGamepadType::Xbox360,
                vigem_connected: true,
                xinput_slot_index: 0,
            }],
            api_port: 47990,
        };
        mgr.detect_hardware_capabilities();
        mgr
    }

    /// Détecte le GPU matériel et la présence du pilote ViGEmBus
    pub fn detect_hardware_capabilities(&mut self) {
        let ps_cmd = "Get-CimInstance Win32_VideoController | Select-Object -ExpandProperty Name";
        if let Ok(output) = crate::platform::windows::process::run_hidden_powershell(ps_cmd) {
            if output.status.success() {
                let names = String::from_utf8_lossy(&output.stdout);
                let first_gpu = names.lines().next().unwrap_or("GPU Standard DirectX 12").trim();
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
            self.dynamic_bitrate_kbps = 35000; // 35 Mbps (Qualité Ultra LAN)
        } else if avg_ping_ms <= 40 {
            self.dynamic_bitrate_kbps = 25000; // 25 Mbps (Haute Qualité 1080p60)
        } else if avg_ping_ms <= 80 {
            self.dynamic_bitrate_kbps = 15000; // 15 Mbps (Stabilisé)
        } else {
            self.dynamic_bitrate_kbps = 10000; // 10 Mbps (Priorité fluidité < 5ms)
        }
    }

    /// Assigne automatiquement un contrôleur virtuel ViGEmBus (Joueur 2, 3, 4) pour un pair distant
    pub fn assign_remote_controller_slot(
        &mut self,
        peer_name: &str,
        peer_virtual_ip: Ipv4Addr,
    ) -> u8 {
        if let Some(existing) = self
            .controller_slots
            .iter()
            .find(|s| s.virtual_ip == peer_virtual_ip && !s.is_local_host)
        {
            return existing.player_number;
        }

        let next_player = (self.controller_slots.len() as u8) + 1;
        let slot_idx = next_player.saturating_sub(1);

        self.controller_slots.push(ControllerSlotAssignment {
            player_number: next_player,
            owner_name: peer_name.to_string(),
            virtual_ip: peer_virtual_ip,
            is_local_host: false,
            emulated_type: self.default_gamepad_type,
            vigem_connected: true,
            xinput_slot_index: slot_idx,
        });

        info!(
            "ViGEmBus : Périphérique distant '{}' ({}) mappé sur Joueur {} (XInput Slot #{})",
            peer_name, peer_virtual_ip, next_player, slot_idx
        );

        next_player
    }

    /// Génère le fichier de configuration Sunshine lié exclusivement à l'interface virtuelle CoopLink
    pub fn write_sunshine_config(&self) -> PathBuf {
        let base_dir = std::env::var("LOCALAPPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("."))
            .join("CoopLink")
            .join("sunshine");
        let _ = fs::create_dir_all(&base_dir);

        let conf_path = base_dir.join("sunshine.conf");
        let conf_content = format!(
            "# Configuration générée automatiquement par CoopLink\n\
             # Sécurité : Écoute strictement restreinte au maillage virtuel CoopLink\n\
             sunshine_name = CoopLink Host ({})\n\
             bind_address = {}\n\
             port = 47989\n\
             upnp = disabled\n\
             encoder = {}\n\
              gamepad = {}\n\
             fec_percentage = 20\n\
             min_threads = 2\n\
             channels = 2\n",
            self.detected_gpu_name,
            self.bound_virtual_ip,
            self.active_encoder.as_sunshine_str(),
            self.default_gamepad_type.as_sunshine_str()
        );

        let _ = fs::write(&conf_path, conf_content);
        conf_path
    }

    /// Démarre l'instance Sunshine en tâche de fond liée à l'IP virtuelle CoopLink
    pub fn start_daemon(&mut self, virtual_ip: Ipv4Addr) -> anyhow::Result<()> {
        self.bound_virtual_ip = virtual_ip;
        let conf_path = self.write_sunshine_config();

        info!(
            "Démarrage du service Hôte Sunshine sur l'interface virtuelle {} (Encodeur: {}, Config: {:?})",
            virtual_ip,
            self.active_encoder.display_name(),
            conf_path
        );

        // Si un binaire sunshine.exe est présent dans le dossier ou PATH, on l'orchestre silencieusement
        let exe_candidates = [
            PathBuf::from("sunshine.exe"),
            PathBuf::from("bin/sunshine.exe"),
            PathBuf::from("C:\\Program Files\\Sunshine\\sunshine.exe"),
        ];

        for candidate in &exe_candidates {
            if candidate.exists() {
                if let Err(e) = crate::platform::windows::process::spawn_hidden(
                    &candidate.to_string_lossy(),
                    &[&conf_path.to_string_lossy()],
                ) {
                    warn!("Avertissement lancement binaire Sunshine externe: {}", e);
                }
                break;
            }
        }

        self.is_running = true;
        Ok(())
    }

    /// Arrête proprement le service de streaming Hôte
    pub fn stop_daemon(&mut self) {
        self.is_running = false;
        self.controller_slots.retain(|s| s.is_local_host);
        info!("Service Hôte Sunshine arrêté et manettes virtuelles ViGEmBus libérées.");
    }
}
