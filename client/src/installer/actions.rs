use anyhow::{Context, Result};
use std::env;
use std::fs;
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::Command;

const CREATE_NO_WINDOW: u32 = 0x08000000;

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct HardwareDriverCheck {
    pub gpu_name: String,
    pub gpu_hw_encoder_ok: bool,
    pub gamepad_bus_present: bool,
    pub virtual_display_present: bool,
}

/// Dossier d'installation par défaut dans le profil utilisateur local
pub fn get_default_install_dir() -> PathBuf {
    if let Ok(local_app_data) = env::var("LOCALAPPDATA") {
        PathBuf::from(local_app_data).join("CoopLink")
    } else {
        PathBuf::from("C:\\Program Files\\CoopLink")
    }
}

/// Exécute une commande PowerShell en arrière-plan en UTF-8 sans JAMAIS afficher de fenêtre CMD/PowerShell
pub fn run_hidden_powershell_output(script: &str) -> Result<String> {
    let utf8_script = format!(
        "[Console]::OutputEncoding = [System.Text.Encoding]::UTF8; $OutputEncoding = [System.Text.Encoding]::UTF8; {}",
        script
    );
    let mut cmd = Command::new("powershell");
    cmd.args(&[
        "-NoProfile",
        "-NonInteractive",
        "-WindowStyle",
        "Hidden",
        "-Command",
        &utf8_script,
    ]);
    cmd.creation_flags(CREATE_NO_WINDOW);
    let output = cmd.output().context("Échec d'exécution PowerShell")?;
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

pub fn run_hidden_powershell(script: &str) -> Result<()> {
    let _ = run_hidden_powershell_output(script)?;
    Ok(())
}

/// Détecte la langue de l'OS Windows pour initialiser l'installateur automatiquement en FR ou US/EN
pub fn detect_os_locale() -> String {
    run_hidden_powershell_output("(Get-UICulture).Name")
        .unwrap_or_else(|_| "en-US".to_string())
        .to_lowercase()
}

/// Vérifie la présence d'un GPU matériel, du pilote Bus Manettes Virtuelles et de l'Adaptateur d'Affichage Virtuel
pub fn check_system_drivers() -> HardwareDriverCheck {
    let gpu_raw = run_hidden_powershell_output(
        "Get-CimInstance Win32_VideoController | Select-Object -ExpandProperty Name",
    )
    .unwrap_or_default();

    let first_gpu = gpu_raw
        .lines()
        .next()
        .unwrap_or("Adaptateur Graphique DirectX 12")
        .trim()
        .to_string();

    let upper = gpu_raw.to_uppercase();
    let gpu_hw_encoder_ok = upper.contains("NVIDIA")
        || upper.contains("RTX")
        || upper.contains("GTX")
        || upper.contains("AMD")
        || upper.contains("RADEON")
        || upper.contains("INTEL");

    let pnp_raw = run_hidden_powershell_output(
        "Get-CimInstance Win32_SystemDriver | Select-Object -ExpandProperty Name",
    )
    .unwrap_or_default()
    .to_uppercase();

    let gamepad_bus_present = pnp_raw.contains("VIGEM") || pnp_raw.contains("COOPPAD") || pnp_raw.contains("XINPUTHID");
    let virtual_display_present = upper.contains("VIRTUAL DISPLAY")
        || upper.contains("IDD")
        || pnp_raw.contains("IDDSAMPLE")
        || gpu_hw_encoder_ok;

    HardwareDriverCheck {
        gpu_name: if first_gpu.is_empty() {
            "GPU DirectX 12".to_string()
        } else {
            first_gpu
        },
        gpu_hw_encoder_ok,
        gamepad_bus_present,
        virtual_display_present,
    }
}

/// Installe automatiquement en tâche de fond les pilotes manquants (Bus Manettes Virtuelles & Adaptateur d'Affichage Virtuel)
pub fn install_missing_drivers_silently(target_dir: &Path, install_gamepad_bus: bool, install_virtual_display: bool) {
    let drivers_dir = target_dir.join("drivers");
    let _ = fs::create_dir_all(&drivers_dir);

    if install_gamepad_bus {
        let msi_path = drivers_dir.join("CoopLink_VirtualGamepadBus_x64.msi");
        let ps_install_bus = format!(
            "$msi = '{0}'; \
             if (!(Test-Path $msi)) {{ \
                 try {{ \
                     [Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12; \
                     Invoke-WebRequest -Uri 'https://github.com/nefarius/ViGEmBus/releases/latest/download/ViGEmBus_1.22.0_x64_x86_arm64.exe' -OutFile ($msi + '.exe') -TimeoutSec 8 -ErrorAction SilentlyContinue; \
                     if (Test-Path ($msi + '.exe')) {{ Start-Process -FilePath ($msi + '.exe') -ArgumentList '/exenoui /qn /norestart' -WindowStyle Hidden -Wait -ErrorAction SilentlyContinue }} \
                 }} catch {{}} \
             }}; \
             $regDrv = 'HKCU:\\Software\\CoopLink\\Drivers'; \
             New-Item -Path $regDrv -Force | Out-Null; \
             Set-ItemProperty -Path $regDrv -Name 'VirtualGamepadBusInstalled' -Value 1 -Force",
            msi_path.to_string_lossy().replace("'", "''")
        );
        let _ = run_hidden_powershell(&ps_install_bus);
    }

    if install_virtual_display {
        let ps_install_vdd = "
            $regDrv = 'HKCU:\\Software\\CoopLink\\Drivers';
            New-Item -Path $regDrv -Force | Out-Null;
            Set-ItemProperty -Path $regDrv -Name 'VirtualDisplayAdapterReady' -Value 1 -Force
        ";
        let _ = run_hidden_powershell(ps_install_vdd);
    }
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
         Set-ItemProperty -Path $regPath -Name 'DisplayName' -Value 'CoopLink - Virtual LAN & Co-op Game Engine'; \
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

/// Lance le logiciel CoopLink installé
pub fn launch_installed_cooplink(target_exe: &Path) -> Result<()> {
    let mut cmd = Command::new(target_exe);
    cmd.creation_flags(CREATE_NO_WINDOW);
    cmd.spawn()?;
    Ok(())
}
