pub mod capture_host;
pub mod pairing;
pub mod settings;
pub mod stream_viewer;
pub mod window_list;

use capture_host::CoopCaptureHost;
use pairing::CoopSessionPairing;
use serde::{Deserialize, Serialize};
use std::net::Ipv4Addr;
use stream_viewer::CoopStreamViewer;
use window_list::WindowEntry;

/// Mode de gestion des contrôles quand 2 à 4 joueurs jouent sur le même écran
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CoopInputMode {
    /// Chacun joue avec sa manette : l'hôte branche ses manettes, l'invité utilise la sienne.
    MultiGamepad,
    /// Deux claviers : l'invité joue sur un jeu de touches dédié (flèches + pavé numérique),
    /// sans conflit avec le ZQSD de l'hôte. Aucun pilote virtuel requis.
    TwoKeyboards,
    /// Les deux joueurs partagent le même clavier/souris (jeux au tour par tour).
    SharedKeyboardMouse,
}

impl CoopInputMode {
    pub fn label(&self) -> &'static str {
        match self {
            CoopInputMode::MultiGamepad => "Manettes (1 à 4 joueurs)",
            CoopInputMode::TwoKeyboards => "2 claviers (écran partagé)",
            CoopInputMode::SharedKeyboardMouse => "Clavier/souris partagé (tour par tour)",
        }
    }

    /// Which control set the guest's viewer must emit.
    pub fn guest_player_index(&self) -> u8 {
        match self {
            CoopInputMode::TwoKeyboards => 2,
            _ => 1,
        }
    }

    /// In two-keyboard mode the guest has no separate cursor, so its mouse is off.
    pub fn guest_mouse_enabled(&self) -> bool {
        !matches!(self, CoopInputMode::TwoKeyboards)
    }
}

pub use capture_host::FOLLOW_FOREGROUND_LABEL;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamingRoleTab {
    HostShareMyGame,
    GuestJoinFriendGame,
}

#[allow(dead_code)]
pub struct StreamingEngine {
    pub capture_host: CoopCaptureHost,
    pub stream_viewer: CoopStreamViewer,
    pub pairing: CoopSessionPairing,
    pub role_tab: StreamingRoleTab,
    pub coop_input_mode: CoopInputMode,
    pub selected_app_name: String,
    pub custom_exe_path: String,
    /// Labels shown in the picker (foreground option first).
    pub detected_windows: Vec<String>,
    /// Full entries, so a selection keeps the process name and survives a title change.
    pub detected_entries: Vec<WindowEntry>,
    pub auto_pair_on_connect: bool,
    pub show_advanced_settings: bool,
}

impl StreamingEngine {
    pub fn new(default_host_vip: Ipv4Addr) -> Self {
        let mut engine = Self {
            capture_host: CoopCaptureHost::new(default_host_vip),
            stream_viewer: CoopStreamViewer::new(default_host_vip),
            pairing: CoopSessionPairing::new(),
            role_tab: StreamingRoleTab::HostShareMyGame,
            coop_input_mode: CoopInputMode::MultiGamepad,
            selected_app_name: FOLLOW_FOREGROUND_LABEL.to_string(),
            custom_exe_path: String::new(),
            detected_windows: vec![FOLLOW_FOREGROUND_LABEL.to_string()],
            detected_entries: Vec::new(),
            auto_pair_on_connect: true,
            show_advanced_settings: false,
        };
        engine.refresh_running_games();
        engine
    }

    /// Liste les fenêtres réellement partageables (titre + processus), sans passer par PowerShell.
    pub fn refresh_running_games(&mut self) {
        let mut list = vec![FOLLOW_FOREGROUND_LABEL.to_string()];
        let mut entries = Vec::new();

        // Fenêtres qui ne sont jamais un jeu.
        let noise = [
            "paramètres",
            "parametres",
            "settings",
            "configuration",
            "overlay",
            "textinputhost",
            "nvidia geforce",
            "task manager",
            "gestionnaire des tâches",
            "file explorer",
            "explorateur",
        ];

        for entry in window_list::visible_windows() {
            let lower = entry.title.to_lowercase();
            if lower.contains('u') && entry.title.contains('\u{FFFD}') {
                continue; // encoding damage, unreadable title
            }
            if noise.iter().any(|n| lower.contains(n)) {
                continue;
            }
            let label = entry.label();
            if list.contains(&label) {
                continue;
            }
            list.push(label);
            entries.push(entry);
        }

        if let Some(entry) = self
            .detected_entries
            .iter()
            .find(|e| e.title == self.capture_host.target_title && e.process == self.capture_host.target_process)
        {
            if let Some(name) = entries
                .iter()
                .find(|e| e.process.eq_ignore_ascii_case(&entry.process))
                .map(|e| e.label())
            {
                self.selected_app_name = name;
            }
        }

        self.detected_windows = list;
        self.detected_entries = entries;
    }

    /// Applique une sélection du picker (libellé « Titre [process.exe] »).
    pub fn select_window(&mut self, label: &str) {
        self.selected_app_name = label.to_string();

        if label == FOLLOW_FOREGROUND_LABEL {
            self.capture_host.target_process.clear();
            self.capture_host.target_title.clear();
            return;
        }

        if let Some(entry) = self.detected_entries.iter().find(|e| e.label() == label) {
            self.capture_host.target_process = entry.process.clone();
            self.capture_host.target_title = entry.title.clone();
            return;
        }

        // Custom .exe entries: "Jeu (.exe) : Nom"
        if let Some(stem) = label.strip_prefix("Jeu (.exe) : ") {
            self.capture_host.target_process = format!("{}.exe", stem);
            self.capture_host.target_title.clear();
            return;
        }

        self.capture_host.target_process.clear();
        self.capture_host.target_title = label.to_string();
    }

    /// Ouvre la boîte de dialogue native Windows 11 pour sélectionner l'exécutable (.exe) d'un jeu
    pub fn browse_game_executable(&mut self) {
        let ps_cmd = r#"
Add-Type -AssemblyName System.Windows.Forms
$dlg = New-Object System.Windows.Forms.OpenFileDialog
$dlg.Filter = "Jeux (*.exe)|*.exe"
$dlg.Title = "Sélectionner le jeu (.exe) à partager au premier plan"
if ($dlg.ShowDialog() -eq [System.Windows.Forms.DialogResult]::OK) {
    Write-Output $dlg.FileName
}
"#;
        if let Ok(out) = crate::platform::windows::process::run_hidden_powershell(ps_cmd) {
            if out.status.success() {
                let path = String::from_utf8_lossy(&out.stdout).trim().to_string();
                if !path.is_empty() {
                    self.custom_exe_path = path.clone();
                    if let Some(fname) = std::path::Path::new(&path).file_name() {
                        let game_title = format!("Jeu (.exe) : {}", fname.to_string_lossy());
                        if !self.detected_windows.contains(&game_title) {
                            self.detected_windows.push(game_title.clone());
                        }
                        self.selected_app_name = game_title;
                        self.capture_host.target_process = fname.to_string_lossy().to_string();
                        self.capture_host.target_title.clear();
                    }
                }
            }
        }
    }
}
