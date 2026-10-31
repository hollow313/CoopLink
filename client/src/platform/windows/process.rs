use std::io::Result;
#[cfg(windows)]
use std::os::windows::process::CommandExt;
use std::process::{Child, Command, Output};

pub const CREATE_NO_WINDOW: u32 = 0x08000000;

/// Exécute une commande de façon 100% invisible en arrière-plan (aucun terminal, aucune console CMD)
pub fn run_hidden(program: &str, args: &[&str]) -> Result<Output> {
    let mut cmd = Command::new(program);
    cmd.args(args);
    #[cfg(windows)]
    cmd.creation_flags(CREATE_NO_WINDOW);
    cmd.output()
}

/// Lance un processus démon en tâche de fond sans bloquer et sans fenêtre CMD
pub fn spawn_hidden(program: &str, args: &[&str]) -> Result<Child> {
    let mut cmd = Command::new(program);
    cmd.args(args);
    #[cfg(windows)]
    cmd.creation_flags(CREATE_NO_WINDOW);
    cmd.spawn()
}

/// Exécute une commande PowerShell en forçant l'encodage UTF-8 pour un affichage parfait des accents (é, è, ê, à...)
pub fn run_hidden_powershell(script: &str) -> Result<Output> {
    #[cfg(windows)]
    {
        let utf8_script = format!(
            "[Console]::OutputEncoding = [System.Text.Encoding]::UTF8; $OutputEncoding = [System.Text.Encoding]::UTF8; {}",
            script
        );
        run_hidden(
            "powershell",
            &[
                "-NoProfile",
                "-NonInteractive",
                "-WindowStyle",
                "Hidden",
                "-Command",
                &utf8_script,
            ],
        )
    }
    #[cfg(not(windows))]
    {
        let _ = script;
        Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "PowerShell only on Windows",
        ))
    }
}

/// Détecte la langue de l'OS (Windows / macOS / Linux : ex: "fr-FR", "en-US", "es-ES", "de-DE", "pt-PT")
pub fn detect_windows_os_locale() -> String {
    #[cfg(windows)]
    {
        if let Ok(out) = run_hidden_powershell("(Get-UICulture).Name") {
            if out.status.success() {
                let s = String::from_utf8_lossy(&out.stdout).trim().to_lowercase();
                if !s.is_empty() {
                    return s;
                }
            }
        }
    }
    if let Ok(lang) = std::env::var("LANG") {
        let s = lang.trim().to_lowercase();
        if !s.is_empty() {
            return s;
        }
    }
    "en-us".to_string()
}

/// Active ou désactive le lancement automatique de CoopLink au démarrage de Windows 11
pub fn set_windows_autostart(enabled: bool) {
    if let Ok(exe_path) = std::env::current_exe() {
        let exe_str = exe_path.to_string_lossy().replace('\'', "''");
        let ps = if enabled {
            format!(
                "Set-ItemProperty -Path 'HKCU:\\Software\\Microsoft\\Windows\\CurrentVersion\\Run' -Name 'CoopLink' -Value '\"{}\" --minimized' -Force",
                exe_str
            )
        } else {
            "Remove-ItemProperty -Path 'HKCU:\\Software\\Microsoft\\Windows\\CurrentVersion\\Run' -Name 'CoopLink' -ErrorAction SilentlyContinue".to_string()
        };
        let _ = run_hidden_powershell(&ps);
    }
}
