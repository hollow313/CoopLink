pub mod capture_host;
pub mod pairing;
pub mod settings;
pub mod stream_viewer;

use capture_host::CoopCaptureHost;
use pairing::CoopSessionPairing;
use serde::{Deserialize, Serialize};
use std::net::Ipv4Addr;
use stream_viewer::CoopStreamViewer;

/// Mode de gestion des contrôles quand 2 à 4 joueurs jouent sur le même écran
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CoopInputMode {
    /// Chacun joue avec sa manette : Hôte = Joueur 1, Amis = Joueur 2 / 3 / 4
    MultiGamepad,
    /// Vous êtes tous les 2 au Clavier/Souris : l'Hôte garde le Clavier/Souris (J1) et le Clavier/Souris de l'ami est converti en Manette Virtuelle (J2)
    KeyboardToVirtualGamepad,
    /// Les deux joueurs partagent directement le même Clavier et la même Souris (jeux au tour par tour ou J1=ZQSD / J2=Flèches)
    SharedKeyboardMouse,
}

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
    pub detected_windows: Vec<String>,
    pub auto_pair_on_connect: bool,
    pub show_advanced_settings: bool,
}

impl StreamingEngine {
    pub fn new(default_host_vip: Ipv4Addr) -> Self {
        let default_game = "Jeu actif au premier plan (Détection automatique DirectX / Vulkan)".to_string();
        let mut engine = Self {
            capture_host: CoopCaptureHost::new(default_host_vip),
            stream_viewer: CoopStreamViewer::new(default_host_vip),
            pairing: CoopSessionPairing::new(),
            role_tab: StreamingRoleTab::HostShareMyGame,
            coop_input_mode: CoopInputMode::MultiGamepad,
            selected_app_name: default_game.clone(),
            custom_exe_path: String::new(),
            detected_windows: vec![default_game],
            auto_pair_on_connect: true,
            show_advanced_settings: false,
        };
        engine.refresh_running_games();
        engine
    }

    /// Scanne uniquement les jeux et applications de premier plan ouverts sur le PC (en UTF-8, en filtrant les processus système)
    pub fn refresh_running_games(&mut self) {
        let default_opt = "Jeu actif au premier plan (Détection automatique DirectX / Vulkan)".to_string();
        let mut list = vec![default_opt];

        let ps_cmd = "Get-Process | Where-Object { $_.MainWindowTitle -ne '' } | Select-Object -ExpandProperty MainWindowTitle";
        if let Ok(out) = crate::platform::windows::process::run_hidden_powershell(ps_cmd) {
            if out.status.success() {
                let text = String::from_utf8_lossy(&out.stdout);
                let ignored_keywords = [
                    "param",
                    "settings",
                    "configur",
                    "cua.agent",
                    "cursoroverlay",
                    "overlay",
                    "textinputhost",
                    "applicationframehost",
                    "shellexperiencehost",
                    "program manager",
                    "windows input",
                    "microsoft text",
                    "explorateur",
                    "file explorer",
                    "nvidia geforce",
                    "task manager",
                    "gestionnaire des",
                ];

                for line in text.lines() {
                    let trimmed = line.trim();
                    let lower = trimmed.to_lowercase();
                    // On filtre la fenêtre principale CoopLink elle-même, mais on conserve les jeux de test et applications de jeux
                    let is_cooplink_self = lower == "cooplink" || lower == "installation de cooplink" || lower.starts_with("cooplink v1.");
                    let is_ignored = is_cooplink_self || ignored_keywords.iter().any(|k| lower.contains(k));

                    if !trimmed.is_empty()
                        && !is_ignored
                        && !trimmed.contains('\u{FFFD}')
                        && !list.contains(&trimmed.to_string())
                    {
                        list.push(trimmed.to_string());
                    }
                }
            }
        }
        if !self.custom_exe_path.is_empty() {
            if let Some(fname) = std::path::Path::new(&self.custom_exe_path).file_stem() {
                let custom_title = format!("Jeu (.exe) : {}", fname.to_string_lossy());
                if !list.contains(&custom_title) {
                    list.push(custom_title);
                }
            }
        }
        self.detected_windows = list;
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
                    if let Some(fname) = std::path::Path::new(&path).file_stem() {
                        let game_title = format!("Jeu (.exe) : {}", fname.to_string_lossy());
                        if !self.detected_windows.contains(&game_title) {
                            self.detected_windows.push(game_title.clone());
                        }
                        self.selected_app_name = game_title;
                    }
                }
            }
        }
    }
}
