use anyhow::{Context, Result};
use std::env;
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::Command;

const CREATE_NO_WINDOW: u32 = 0x08000000;

/// Dossier d'installation par défaut dans le profil utilisateur local
pub fn get_default_install_dir() -> PathBuf {
    if let Ok(local_app_data) = env::var("LOCALAPPDATA") {
        PathBuf::from(local_app_data).join("CoopLink")
    } else {
        PathBuf::from("C:\\Program Files\\CoopLink")
    }
}

/// Exécute une commande PowerShell en arrière-plan sans JAMAIS afficher de fenêtre CMD/PowerShell
pub fn run_hidden_powershell(script: &str) -> Result<()> {
    let mut cmd = Command::new("powershell");
    cmd.args(&[
        "-NoProfile",
        "-NonInteractive",
        "-WindowStyle",
        "Hidden",
        "-Command",
        script,
    ]);
    cmd.creation_flags(CREATE_NO_WINDOW);
    let output = cmd.output().context("Échec d'exécution PowerShell")?;
    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr);
        tracing::warn!("PowerShell discret a retourné: {}", err);
    }
    Ok(())
}

/// Crée un raccourci Windows .lnk de façon totalement transparente
pub fn create_windows_shortcut(target_exe: &Path, shortcut_path: &Path, description: &str) -> Result<()> {
    let ps_script = format!(
        "$WshShell = New-Object -ComObject WScript.Shell; \
         $Shortcut = $WshShell.CreateShortcut('{0}'); \
         $Shortcut.TargetPath = '{1}'; \
         $Shortcut.Description = '{2}'; \
         $Shortcut.WorkingDirectory = '{3}'; \
         $Shortcut.Save()",
        shortcut_path.to_string_lossy().replace("'", "''"),
        target_exe.to_string_lossy().replace("'", "''"),
        description.replace("'", "''"),
        target_exe.parent().unwrap_or(Path::new("")).to_string_lossy().replace("'", "''")
    );

    run_hidden_powershell(&ps_script)
}

/// Enregistre les règles pare-feu Windows pour CoopLink de façon transparente
pub fn setup_silent_firewall(target_exe: &Path) -> Result<()> {
    let exe_str = target_exe.to_string_lossy();
    let ps_script = format!(
        "New-NetFirewallRule -DisplayName 'CoopLink - Core Virtual LAN' -Direction Inbound -Program '{0}' -Action Allow -Profile Any -ErrorAction SilentlyContinue; \
         New-NetFirewallRule -DisplayName 'CoopLink - Core Virtual LAN (Out)' -Direction Outbound -Program '{0}' -Action Allow -Profile Any -ErrorAction SilentlyContinue; \
         New-NetFirewallRule -DisplayName 'CoopLink - UDP Gaming Discovery' -Direction Inbound -Protocol UDP -LocalPort 42422 -Action Allow -Profile Any -ErrorAction SilentlyContinue; \
         New-NetFirewallRule -DisplayName 'CoopLink - Virtual Gaming Subnet' -Direction Inbound -RemoteAddress '10.244.0.0/24' -Action Allow -Profile Any -ErrorAction SilentlyContinue",
        exe_str.replace("'", "''")
    );

    run_hidden_powershell(&ps_script)
}

/// Enregistre CoopLink dans le Registre Windows pour la gestion des applications
pub fn register_in_windows_uninstall(target_dir: &Path, target_exe: &Path) -> Result<()> {
    let ps_script = format!(
        "$regPath = 'HKCU:\\Software\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\CoopLink'; \
         if (!(Test-Path $regPath)) {{ New-Item -Path $regPath -Force | Out-Null }}; \
         Set-ItemProperty -Path $regPath -Name 'DisplayName' -Value 'CoopLink - Virtual LAN Gaming Engine'; \
         Set-ItemProperty -Path $regPath -Name 'DisplayVersion' -Value '1.0.0'; \
         Set-ItemProperty -Path $regPath -Name 'Publisher' -Value 'Loic HUON'; \
         Set-ItemProperty -Path $regPath -Name 'DisplayIcon' -Value '{0}'; \
         Set-ItemProperty -Path $regPath -Name 'InstallLocation' -Value '{1}'; \
         Set-ItemProperty -Path $regPath -Name 'UninstallString' -Value 'powershell -NoProfile -Command \"Remove-Item -Recurse -Force ''{1}''; Remove-Item -Path ''HKCU:\\Software\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\CoopLink'' -Force\"'",
        target_exe.to_string_lossy().replace("'", "''"),
        target_dir.to_string_lossy().replace("'", "''")
    );

    run_hidden_powershell(&ps_script)
}

/// Lance le jeu/logiciel CoopLink installé
pub fn launch_installed_cooplink(target_exe: &Path) -> Result<()> {
    let mut cmd = Command::new(target_exe);
    cmd.creation_flags(CREATE_NO_WINDOW);
    cmd.spawn()?;
    Ok(())
}
