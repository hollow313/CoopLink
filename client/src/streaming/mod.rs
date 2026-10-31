pub mod moonlight_client;
pub mod pairing;
pub mod sunshine_host;

use moonlight_client::MoonlightClientManager;
use pairing::ZeroConfigPairingManager;
use serde::{Deserialize, Serialize};
use std::net::Ipv4Addr;
use sunshine_host::SunshineHostManager;

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
    pub sunshine_host: SunshineHostManager,
    pub moonlight_client: MoonlightClientManager,
    pub pairing: ZeroConfigPairingManager,
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
        let mut engine = Self {
            sunshine_host: SunshineHostManager::new(default_host_vip),
            moonlight_client: MoonlightClientManager::new(default_host_vip),
            pairing: ZeroConfigPairingManager::new(),
            role_tab: StreamingRoleTab::HostShareMyGame,
            coop_input_mode: CoopInputMode::MultiGamepad,
            selected_app_name: "Écran Complet (Bureau Windows)".to_string(),
            custom_exe_path: String::new(),
            detected_windows: vec![
                "Écran Complet (Bureau Windows)".to_string(),
            ],
            auto_pair_on_connect: true,
            show_advanced_settings: false,
        };
        engine.refresh_running_games();
        engine
    }

    /// Scanne les fenêtres et jeux ouverts sur le PC (sans ouvrir de fenêtre CMD)
    pub fn refresh_running_games(&mut self) {
        let mut list = vec!["Écran Complet (Bureau Windows)".to_string()];
        let ps_cmd = "Get-Process | Where-Object { $_.MainWindowTitle -ne '' } | Select-Object -ExpandProperty MainWindowTitle";
        if let Ok(out) = crate::platform::windows::process::run_hidden_powershell(ps_cmd) {
            if out.status.success() {
                let text = String::from_utf8_lossy(&out.stdout);
                for line in text.lines() {
                    let trimmed = line.trim();
                    if !trimmed.is_empty()
                        && !trimmed.contains("CoopLink")
                        && !trimmed.contains("Paramètres")
                        && !trimmed.contains("Settings")
                        && !list.contains(&trimmed.to_string())
                    {
                        list.push(trimmed.to_string());
                    }
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
$dlg.Filter = "Jeux & Applications (*.exe)|*.exe"
$dlg.Title = "Sélectionner le jeu à partager sur CoopLink"
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
                        let game_title = format!("Jeu : {}", fname.to_string_lossy());
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
