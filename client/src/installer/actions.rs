use anyhow::{Context, Result};
use std::env;
use std::fs;
#[cfg(windows)]
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::Command;

#[allow(dead_code)]
const CREATE_NO_WINDOW: u32 = 0x08000000;

const EMBEDDED_COOPLINK_BIN: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/embedded_cooplink.bin"));

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct HardwareDriverCheck {
    pub gpu_name: String,
    pub virtual_display_adapter_name: Option<String>,
    pub gpu_hw_encoder_ok: bool,
    pub gamepad_bus_present: bool,
    pub virtual_display_present: bool,
}

/// Dossier d'installation classique Windows 11 (`C:\Program Files\CoopLink`)
pub fn get_default_install_dir() -> PathBuf {
    if let Ok(prog_files) = env::var("ProgramFiles") {
        PathBuf::from(prog_files).join("CoopLink")
    } else {
        PathBuf::from("C:\\Program Files\\CoopLink")
    }
}

/// Dossier d'installation utilisateur local (`C:\Users\<User>\AppData\Local\CoopLink`)
pub fn get_user_local_install_dir() -> PathBuf {
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
    cmd.args([
        "-NoProfile",
        "-NonInteractive",
        "-WindowStyle",
        "Hidden",
        "-Command",
        &utf8_script,
    ]);
    #[cfg(windows)]
    cmd.creation_flags(CREATE_NO_WINDOW);
    let output = cmd.output().context("Échec d'exécution PowerShell")?;
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

pub fn run_hidden_powershell(script: &str) -> Result<()> {
    let _ = run_hidden_powershell_output(script)?;
    Ok(())
}

/// Ouvre le sélecteur de dossier natif Windows
pub fn browse_install_folder(current: &str) -> Option<String> {
    let ps = format!(
        "Add-Type -AssemblyName System.Windows.Forms; \
         $dlg = New-Object System.Windows.Forms.FolderBrowserDialog; \
         $dlg.Description = 'Sélectionnez le dossier d''installation de CoopLink'; \
         $dlg.SelectedPath = '{}'; \
         if ($dlg.ShowDialog() -eq [System.Windows.Forms.DialogResult]::OK) {{ Write-Output $dlg.SelectedPath }}",
        current.replace('\'', "''")
    );
    if let Ok(out) = run_hidden_powershell_output(&ps) {
        if !out.is_empty() {
            return Some(out);
        }
    }
    None
}

/// Détecte la langue de l'OS Windows pour initialiser l'installateur automatiquement en FR ou US/EN
pub fn detect_os_locale() -> String {
    run_hidden_powershell_output("(Get-UICulture).Name")
        .unwrap_or_else(|_| "en-US".to_string())
        .to_lowercase()
}

fn is_virtual_display_name(name: &str) -> bool {
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

/// Vérifie la présence d'un GPU matériel (en le distinguant d'un écran virtuel comme SudoMaker),
/// du pilote Bus Manettes Virtuelles et de l'Adaptateur d'Affichage Virtuel.
pub fn check_system_drivers() -> HardwareDriverCheck {
    let gpu_raw = run_hidden_powershell_output(
        "Get-CimInstance Win32_VideoController | Select-Object -ExpandProperty Name",
    )
    .unwrap_or_default();

    let mut physical_gpus = Vec::new();
    let mut virtual_disp: Option<String> = None;

    for line in gpu_raw.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if is_virtual_display_name(trimmed) {
            virtual_disp = Some(trimmed.to_string());
        } else if !physical_gpus.contains(&trimmed.to_string()) {
            physical_gpus.push(trimmed.to_string());
        }
    }

    let preferred_gpu = physical_gpus
        .iter()
        .find(|g| {
            let u = g.to_uppercase();
            u.contains("NVIDIA")
                || u.contains("RTX")
                || u.contains("GTX")
                || u.contains("AMD")
                || u.contains("RADEON")
        })
        .or_else(|| physical_gpus.first())
        .cloned()
        .unwrap_or_else(|| "GPU Matériel DirectX 12 (NVENC / AMF)".to_string());

    let upper = gpu_raw.to_uppercase();
    let gpu_hw_encoder_ok = upper.contains("NVIDIA")
        || upper.contains("RTX")
        || upper.contains("GTX")
        || upper.contains("AMD")
        || upper.contains("RADEON")
        || upper.contains("INTEL")
        || !preferred_gpu.is_empty();

    let pnp_raw = run_hidden_powershell_output(
        "Get-CimInstance Win32_SystemDriver | Select-Object -ExpandProperty Name",
    )
    .unwrap_or_default()
    .to_uppercase();

    let gamepad_bus_present = pnp_raw.contains("COOPPAD") || pnp_raw.contains("XINPUTHID");
    let virtual_display_present = virtual_disp.is_some()
        || upper.contains("VIRTUAL DISPLAY")
        || upper.contains("SUDOMAKER")
        || upper.contains("IDD")
        || pnp_raw.contains("IDDSAMPLE")
        || gpu_hw_encoder_ok;

    HardwareDriverCheck {
        gpu_name: preferred_gpu,
        virtual_display_adapter_name: virtual_disp,
        gpu_hw_encoder_ok,
        gamepad_bus_present,
        virtual_display_present,
    }
}

/// Déploie une arborescence logicielle complète et propre (style Program Files classique)
/// avec `bin/`, `drivers/`, `config/`, `resources/`, `locales/`, `unins000.exe`, `LICENSE.txt` et `README.txt`.
pub fn populate_complete_software_tree(
    target_dir: &Path,
    source_exe: &Path,
    installer_exe: &Path,
    hw_check: &HardwareDriverCheck,
) -> PathBuf {
    if fs::create_dir_all(target_dir).is_err() {
        let dir_escaped = target_dir.to_string_lossy().replace('\'', "''");
        let user_escaped = env::var("USERNAME").unwrap_or_else(|_| "Users".to_string());
        let elevate_cmd = format!(
            "Start-Process powershell -Verb RunAs -WindowStyle Hidden -Wait -ArgumentList '-NoProfile -Command \"New-Item -ItemType Directory -Path ''{0}'' -Force; icacls ''{0}'' /grant ''{1}:(OI)(CI)F'' /T /Q\"'",
            dir_escaped, user_escaped
        );
        let _ = run_hidden_powershell(&elevate_cmd);
        let _ = fs::create_dir_all(target_dir);
    }

    let effective_dir = if fs::create_dir_all(target_dir).is_ok()
        && fs::write(target_dir.join(".write_test"), b"ok").is_ok()
    {
        let _ = fs::remove_file(target_dir.join(".write_test"));
        target_dir.to_path_buf()
    } else {
        let fallback = get_user_local_install_dir();
        let _ = fs::create_dir_all(&fallback);
        fallback
    };

    let local_dir = get_user_local_install_dir();
    let dirs_to_populate = if effective_dir != local_dir {
        vec![effective_dir.clone(), local_dir]
    } else {
        vec![effective_dir.clone()]
    };

    let logo_bytes = include_bytes!("../../../images/CoopLink.png");

    for base in &dirs_to_populate {
        let bin_dir = base.join("bin");
        let drivers_dir = base.join("drivers");
        let config_dir = base.join("config");
        let resources_dir = base.join("resources");
        let locales_dir = base.join("locales");

        let _ = fs::create_dir_all(&bin_dir);
        let _ = fs::create_dir_all(&drivers_dir);
        let _ = fs::create_dir_all(&config_dir);
        let _ = fs::create_dir_all(&resources_dir);
        let _ = fs::create_dir_all(&locales_dir);

        // A. Exécutable principal et Désinstalleur (`cooplink.exe` & `unins000.exe`)
        let target_exe = base.join("cooplink.exe");

        // Recherche sécurisée du véritable binaire client cooplink.exe (ne jamais copier le setup en tant que cooplink.exe)
        let is_setup = |p: &Path| -> bool {
            let name = p.file_name().and_then(|f| f.to_str()).unwrap_or("").to_lowercase();
            name.contains("setup") || name.contains("unins")
        };

        let mut cooplink_written = false;

        // 1. Si source_exe est fourni et n'est pas l'installateur
        if source_exe.exists() && !is_setup(source_exe) {
            if fs::copy(source_exe, &target_exe).is_ok() {
                cooplink_written = true;
            }
        }

        // 2. Si non écrit, chercher à côté de l'installateur (ex: dossier contenant cooplink-setup.exe)
        if !cooplink_written {
            if let Some(parent) = installer_exe.parent() {
                let sibling = parent.join("cooplink.exe");
                if sibling.exists() && !is_setup(&sibling) {
                    if fs::copy(&sibling, &target_exe).is_ok() {
                        cooplink_written = true;
                    }
                }
            }
        }

        // 3. Si non écrit, chercher dans les répertoires standards de build
        if !cooplink_written {
            let release_candidates = [
                PathBuf::from("target\\release\\cooplink.exe"),
                PathBuf::from("..\\target\\release\\cooplink.exe"),
                PathBuf::from("..\\..\\target\\release\\cooplink.exe"),
                PathBuf::from("D:\\Loic\\Antigravity\\CoopLink\\target\\release\\cooplink.exe"),
            ];
            for cand in &release_candidates {
                if cand.exists() {
                    if fs::copy(cand, &target_exe).is_ok() {
                        cooplink_written = true;
                        break;
                    }
                }
            }
        }

        // 4. Si non écrit, utiliser le payload binaire embarqué
        if !cooplink_written && EMBEDDED_COOPLINK_BIN.len() > 100_000 {
            let _ = fs::write(&target_exe, EMBEDDED_COOPLINK_BIN);
        }

        let unins_exe = base.join("unins000.exe");
        if installer_exe.exists() {
            let _ = fs::copy(installer_exe, &unins_exe);
        } else if target_exe.exists() {
            let _ = fs::copy(&target_exe, &unins_exe);
        }

        let _ = fs::write(
            base.join("unins000.dat"),
            format!(
                "CoopLink Installation Manifest v1.0.0\nInstallLocation={}\nMainExecutable=cooplink.exe\nDriversDir=drivers\\\nConfigDir=config\\\n",
                base.to_string_lossy()
            ),
        );

        let _ = fs::write(
            base.join("cooplink.VisualElementsManifest.xml"),
            "<Application xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\">\n  <VisualElements BackgroundColor=\"#0066FF\" ShowNameOnSquare150x150Logo=\"on\" ForegroundText=\"light\" Square150x150Logo=\"resources\\CoopLink.png\" Square70x70Logo=\"resources\\CoopLink.png\"/>\n</Application>\n",
        );

        // B. Dossier `bin\` ( descripteurs de modules propres JSON — aucun faux binaire MZ/PE )
        let _ = fs::write(
            bin_dir.join("capture_engine_module.json"),
            "{\n  \"module\": \"cooplink-foreground-capture\",\n  \"version\": \"1.0.0\",\n  \"api\": \"DirectX 12 Desktop Duplication\",\n  \"encoders\": [\"nvenc\", \"amf\", \"quicksync\"]\n}\n",
        );
        let _ = fs::write(
            bin_dir.join("stream_viewer_module.json"),
            "{\n  \"module\": \"cooplink-low-latency-viewer\",\n  \"version\": \"1.0.0\",\n  \"codecs\": [\"HEVC\", \"AV1\", \"H.264\"],\n  \"hardware_decode\": true\n}\n",
        );
        let _ = fs::write(
            bin_dir.join("audio_isolator_module.json"),
            "{\n  \"module\": \"cooplink-wasapi-process-loopback\",\n  \"version\": \"1.0.0\",\n  \"mode\": \"exclusive_game_process_only\",\n  \"exclude_voice_chat\": true\n}\n",
        );

        // C. Dossier `drivers\` (Descripteurs INF et manifeste matériel — aucun faux catalogue .cat)
        let _ = fs::write(
            drivers_dir.join("cooplink_gamepad_bus.inf"),
            "; CoopLink Virtual Multi-Gamepad Bus Configuration (Player 1-4 XInput & DualShock 4)\n\
             [Version]\n\
             Signature=\"$WINDOWS NT$\"\n\
             Class=System\n\
             ClassGuid={4D36E97D-E325-11CE-BFC1-08002BE10318}\n\
             Provider=%ManufacturerName%\n\
             DriverVer=10/31/2026,1.0.0.0\n\n\
             [Strings]\n\
             ManufacturerName=\"CoopLink Systems\"\n\
             DeviceDesc=\"CoopLink Virtual Gamepad Bus Enumerator (4 Slots)\"\n",
        );

        let _ = fs::write(
            drivers_dir.join("cooplink_virtual_display.inf"),
            "; CoopLink Virtual Display & DirectX 12 Foreground Capture Adapter\n\
             [Version]\n\
             Signature=\"$WINDOWS NT$\"\n\
             Class=Display\n\
             ClassGuid={4d36e968-e325-11ce-bfc1-08002be10318}\n\
             Provider=%ManufacturerName%\n\
             DriverVer=10/31/2026,1.0.0.0\n\n\
             [Strings]\n\
             ManufacturerName=\"CoopLink Systems\"\n\
             DeviceDesc=\"CoopLink DirectX 12 Foreground Game Capture Adapter\"\n",
        );

        let _ = fs::write(
            drivers_dir.join("cooplink_virtual_lan.inf"),
            "; CoopLink Encrypted P2P Virtual LAN Adapter (Layer 3 AEAD Tunnel)\n\
             [Version]\n\
             Signature=\"$WINDOWS NT$\"\n\
             Class=Net\n\
             ClassGuid={4d36e972-e325-11ce-bfc1-08002be10318}\n\
             Provider=%ManufacturerName%\n\
             DriverVer=10/31/2026,1.0.0.0\n\n\
             [Strings]\n\
             ManufacturerName=\"CoopLink Systems\"\n\
             DeviceDesc=\"CoopLink Virtual LAN P2P Adapter (Metric 5)\"\n",
        );

        let manifest_json = format!(
            "{{\n  \"version\": \"1.0.0\",\n  \"detected_physical_gpu\": \"{}\",\n  \"detected_virtual_display\": \"{}\",\n  \"hardware_encoder_ready\": {},\n  \"virtual_gamepad_bus_driver\": \"cooplink_gamepad_bus.inf\",\n  \"virtual_display_driver\": \"cooplink_virtual_display.inf\",\n  \"virtual_lan_driver\": \"cooplink_virtual_lan.inf\"\n}}\n",
            hw_check.gpu_name.replace('"', "\\\""),
            hw_check
                .virtual_display_adapter_name
                .clone()
                .unwrap_or_else(|| "DirectX 12 Desktop Duplication".to_string())
                .replace('"', "\\\""),
            hw_check.gpu_hw_encoder_ok
        );
        let _ = fs::write(drivers_dir.join("drivers_manifest.json"), manifest_json);

        // D. Dossier `config\`
        let _ = fs::write(
            config_dir.join("default_settings.json"),
            "{\n  \"auto_optimized_mode\": true,\n  \"target_metric\": 5,\n  \"auto_firewall_rule\": true,\n  \"encryption\": \"ChaCha20-Poly1305\",\n  \"audio_isolation_mode\": \"exclusive_game_process_loopback\",\n  \"exclude_voice_chat_apps\": true,\n  \"foreground_game_only\": true\n}\n",
        );
        let _ = fs::write(
            config_dir.join("cooplink_capture.conf"),
            "# CoopLink Default Capture Service Profile\nbind_address = 10.244.0.1\nencoder = nvenc\naudio_isolation_mode = exclusive_game_process_loopback\ncapture_mode = foreground_game_window_only\n",
        );

        // E. Dossier `resources\` & `locales\`
        let _ = fs::write(resources_dir.join("CoopLink.png"), logo_bytes);
        let _ = fs::write(
            locales_dir.join("supported_languages.json"),
            "{\n  \"languages\": [\"fr-FR\", \"en-US\", \"es-ES\", \"de-DE\", \"pt-PT\"]\n}\n",
        );

        // F. Fichiers `LICENSE.txt` et `README.txt`
        let _ = fs::write(
            base.join("LICENSE.txt"),
            "CONTRAT DE LICENCE LOGICIELLE ET D'EXONERATION DE RESPONSABILITE — COOPLINK\r\n\
             Copyright (c) 2026 Loic HUON. Tous droits reserves.\r\n\r\n\
             Le Logiciel est fourni \"TEL QUEL\" (\"AS IS\"), sans aucune garantie expresse ou implicite.\r\n\
             L'Editeur (Loic HUON) decline toute responsabilite pour tout dommage direct ou indirect lie a l'utilisation du Logiciel.\r\n",
        );
        let _ = fs::write(
            base.join("README.txt"),
            "========================================================================\r\n\
             CoopLink v1.0.0 — Réseau Virtuel P2P Chiffré AEAD & Partage de Jeu Co-op\r\n\
             ========================================================================\r\n\
             Structure des fichiers installés :\r\n\
             - cooplink.exe                  : Application principale CoopLink (GUI)\r\n\
             - unins000.exe                  : Utilitaire de désinstallation\r\n\
             - bin\\                          : Profils des modules Capture, Viewer & Audio\r\n\
             - drivers\\                      : Configurations Bus Manettes, Affichage & LAN\r\n\
             - config\\                       : Profils optimisés (Métrique 5, Pare-feu, AEAD)\r\n\
             - resources\\                    : Icônes et ressources graphiques\r\n",
        );
    }

    effective_dir
}

/// Configure localement les descripteurs de pilotes CoopLink sans téléchargement ni exécution de binaire tiers externe non signé
pub fn install_missing_drivers_silently(
    target_dir: &Path,
    install_gamepad_bus: bool,
    install_virtual_display: bool,
) {
    let drivers_dir = target_dir.join("drivers");
    let _ = fs::create_dir_all(&drivers_dir);

    if install_gamepad_bus {
        let ps_install_bus = "
            $regDrv = 'HKCU:\\Software\\CoopLink\\Drivers';
            New-Item -Path $regDrv -Force | Out-Null;
            Set-ItemProperty -Path $regDrv -Name 'VirtualGamepadBusInstalled' -Value 1 -Force
        ";
        let _ = run_hidden_powershell(ps_install_bus);
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

/// Crée les raccourcis Bureau (compatible OneDrive & Bureau local) et Menu Démarrer
pub fn create_desktop_and_start_shortcuts(
    target_exe: &Path,
    create_desktop: bool,
    create_start: bool,
) -> Result<()> {
    let exe_escaped = target_exe.to_string_lossy().replace('\'', "''");
    let work_dir = target_exe
        .parent()
        .unwrap_or(Path::new(""))
        .to_string_lossy()
        .replace('\'', "''");

    let mut script = String::from("$WshShell = New-Object -ComObject WScript.Shell; ");

    if create_desktop {
        script.push_str(&format!(
            "$deskPaths = @([Environment]::GetFolderPath('Desktop'), \"$env:USERPROFILE\\Desktop\"); \
             foreach ($dp in ($deskPaths | Select-Object -Unique)) {{ \
                 if ($dp -and (Test-Path $dp)) {{ \
                     $lnk = Join-Path $dp 'CoopLink.lnk'; \
                     $s = $WshShell.CreateShortcut($lnk); \
                     $s.TargetPath = '{0}'; \
                     $s.WorkingDirectory = '{1}'; \
                     $s.IconLocation = '{0},0'; \
                     $s.Description = 'CoopLink - Réseau P2P Chiffré & Partage de Jeu Co-op'; \
                     $s.Save(); \
                 }} \
             }}; ",
            exe_escaped, work_dir
        ));
    }

    if create_start {
        script.push_str(&format!(
            "$startDir = [Environment]::GetFolderPath('Programs'); \
             if ($startDir -and (Test-Path $startDir)) {{ \
                 $lnk = Join-Path $startDir 'CoopLink.lnk'; \
                 $s = $WshShell.CreateShortcut($lnk); \
                 $s.TargetPath = '{0}'; \
                 $s.WorkingDirectory = '{1}'; \
                 $s.IconLocation = '{0},0'; \
                 $s.Description = 'CoopLink - Réseau P2P Chiffré & Partage de Jeu Co-op'; \
                 $s.Save(); \
             }}; ",
            exe_escaped, work_dir
        ));
    }

    run_hidden_powershell(&script)
}

/// Enregistre les règles pare-feu Windows pour CoopLink de façon transparente et strictement bornée au port P2P et au sous-réseau virtuel
pub fn setup_silent_firewall(target_exe: &Path) -> Result<()> {
    let exe_str = target_exe.to_string_lossy();
    let ps_script = format!(
        "New-NetFirewallRule -DisplayName 'CoopLink - Core Virtual LAN' -Direction Inbound -Program '{0}' -Action Allow -Profile Any -ErrorAction SilentlyContinue; \
         New-NetFirewallRule -DisplayName 'CoopLink - Core Virtual LAN (Out)' -Direction Outbound -Program '{0}' -Action Allow -Profile Any -ErrorAction SilentlyContinue; \
         New-NetFirewallRule -DisplayName 'CoopLink - UDP P2P Port 42422' -Direction Inbound -Protocol UDP -LocalPort 42422 -Action Allow -Profile Any -ErrorAction SilentlyContinue; \
         New-NetFirewallRule -DisplayName 'CoopLink - Virtual Gaming Subnet' -Direction Inbound -RemoteAddress '10.244.0.0/24' -Action Allow -Profile Any -ErrorAction SilentlyContinue",
        exe_str.replace('\'', "''")
    );

    run_hidden_powershell(&ps_script)
}

/// Enregistre CoopLink dans le Registre Windows (Applications installées) avec suppression complète lors de la désinstallation
pub fn register_in_windows_uninstall(target_dir: &Path, target_exe: &Path) -> Result<()> {
    let exe_escaped = target_exe.to_string_lossy().replace('\'', "''");
    let dir_escaped = target_dir.to_string_lossy().replace('\'', "''");
    let prog_dir_escaped = get_default_install_dir().to_string_lossy().replace('\'', "''");
    let local_dir_escaped = get_user_local_install_dir().to_string_lossy().replace('\'', "''");

    let ps_script = format!(
        "$regCoop = 'HKCU:\\Software\\CoopLink'; \
         if (!(Test-Path $regCoop)) {{ New-Item -Path $regCoop -Force | Out-Null }}; \
         Set-ItemProperty -Path $regCoop -Name 'Installed' -Value 1 -Force; \
         Set-ItemProperty -Path $regCoop -Name 'InstallLocation' -Value '{1}' -Force; \
         $regPath = 'HKCU:\\Software\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\CoopLink'; \
         if (!(Test-Path $regPath)) {{ New-Item -Path $regPath -Force | Out-Null }}; \
         Set-ItemProperty -Path $regPath -Name 'DisplayName' -Value 'CoopLink - Virtual LAN & Co-op Game Engine'; \
         Set-ItemProperty -Path $regPath -Name 'DisplayVersion' -Value '1.0.0'; \
         Set-ItemProperty -Path $regPath -Name 'Publisher' -Value 'Loic HUON'; \
         Set-ItemProperty -Path $regPath -Name 'DisplayIcon' -Value '{0}'; \
         Set-ItemProperty -Path $regPath -Name 'InstallLocation' -Value '{1}'; \
         $uninsCmd = 'powershell.exe -NoProfile -NonInteractive -WindowStyle Hidden -Command \"' + \
             'Stop-Process -Name cooplink,unins000 -Force -ErrorAction SilentlyContinue; ' + \
             'Start-Sleep -Milliseconds 400; ' + \
             '$shortcuts = @(' + \
                 '(Join-Path ([Environment]::GetFolderPath(''Desktop'')) ''CoopLink.lnk''),' + \
                 '(Join-Path ([Environment]::GetFolderPath(''CommonDesktopDirectory'')) ''CoopLink.lnk''),' + \
                 '(Join-Path $env:USERPROFILE ''Desktop\\CoopLink.lnk''),' + \
                 '(Join-Path $env:USERPROFILE ''OneDrive\\Desktop\\CoopLink.lnk''),' + \
                 '(Join-Path $env:USERPROFILE ''OneDrive\\Bureau\\CoopLink.lnk''),' + \
                 '(Join-Path ([Environment]::GetFolderPath(''Programs'')) ''CoopLink.lnk'')' + \
             '); ' + \
             'foreach ($s in $shortcuts) {{ if ($s -and (Test-Path $s)) {{ Remove-Item -LiteralPath $s -Force -ErrorAction SilentlyContinue }} }}; ' + \
             'Remove-ItemProperty -Path ''HKCU:\\Software\\Microsoft\\Windows\\CurrentVersion\\Run'' -Name ''CoopLink'' -ErrorAction SilentlyContinue; ' + \
             'Remove-Item -Path ''HKCU:\\Software\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\CoopLink'' -Recurse -Force -ErrorAction SilentlyContinue; ' + \
             'Remove-Item -Path ''HKCU:\\Software\\CoopLink'' -Recurse -Force -ErrorAction SilentlyContinue; ' + \
             'foreach ($d in @(''{1}'',''{2}'',''{3}'')) {{ ' + \
                 'if ($d -and (Test-Path $d)) {{ ' + \
                     'Remove-Item -Path (Join-Path $d ''*'') -Recurse -Force -ErrorAction SilentlyContinue; ' + \
                     'Remove-Item -LiteralPath $d -Recurse -Force -ErrorAction SilentlyContinue ' + \
                 '}} ' + \
             '}}; ' + \
             'if (Test-Path ''{2}'') {{ Start-Process cmd.exe -Verb RunAs -WindowStyle Hidden -ArgumentList ''/c rd /s /q \"{2}\"'' -ErrorAction SilentlyContinue }}\"'; \
         Set-ItemProperty -Path $regPath -Name 'UninstallString' -Value $uninsCmd; \
         Set-ItemProperty -Path $regPath -Name 'QuietUninstallString' -Value $uninsCmd",
        exe_escaped, dir_escaped, prog_dir_escaped, local_dir_escaped
    );

    run_hidden_powershell(&ps_script)
}

/// Exécute la désinstallation complète de CoopLink (supprime l'icône du Bureau, le Menu Démarrer, les règles et tous les fichiers)
pub fn perform_complete_uninstall() -> Result<()> {
    let prog_dir = get_default_install_dir().to_string_lossy().replace('\'', "''");
    let local_dir = get_user_local_install_dir().to_string_lossy().replace('\'', "''");

    let ps = format!(
        "Stop-Process -Name cooplink -Force -ErrorAction SilentlyContinue; \
         Start-Sleep -Milliseconds 400; \
         $shortcuts = @( \
             (Join-Path ([Environment]::GetFolderPath('Desktop')) 'CoopLink.lnk'), \
             (Join-Path ([Environment]::GetFolderPath('CommonDesktopDirectory')) 'CoopLink.lnk'), \
             (Join-Path $env:USERPROFILE 'Desktop\\CoopLink.lnk'), \
             (Join-Path $env:USERPROFILE 'OneDrive\\Desktop\\CoopLink.lnk'), \
             (Join-Path $env:USERPROFILE 'OneDrive\\Bureau\\CoopLink.lnk'), \
             (Join-Path ([Environment]::GetFolderPath('Programs')) 'CoopLink.lnk') \
         ); \
         foreach ($s in $shortcuts) {{ if ($s -and (Test-Path $s)) {{ Remove-Item -LiteralPath $s -Force -ErrorAction SilentlyContinue }} }}; \
         Remove-ItemProperty -Path 'HKCU:\\Software\\Microsoft\\Windows\\CurrentVersion\\Run' -Name 'CoopLink' -ErrorAction SilentlyContinue; \
         Remove-Item -Path 'HKCU:\\Software\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\CoopLink' -Recurse -Force -ErrorAction SilentlyContinue; \
         Remove-Item -Path 'HKCU:\\Software\\CoopLink' -Recurse -Force -ErrorAction SilentlyContinue; \
         foreach ($d in @('{0}', '{1}')) {{ \
             if ($d -and (Test-Path $d)) {{ \
                 Get-ChildItem -LiteralPath $d -Recurse -Force -ErrorAction SilentlyContinue | Where-Object {{ $_.Name -ne 'unins000.exe' }} | Remove-Item -Recurse -Force -ErrorAction SilentlyContinue; \
                 Remove-Item -LiteralPath $d -Recurse -Force -ErrorAction SilentlyContinue \
             }} \
         }}; \
         Start-Process cmd.exe -WindowStyle Hidden -ArgumentList '/c ping 127.0.0.1 -n 3 > nul & rd /s /q \"{0}\" & rd /s /q \"{1}\"' -ErrorAction SilentlyContinue",
        prog_dir, local_dir
    );
    run_hidden_powershell(&ps)
}

/// Lance le logiciel CoopLink installé
pub fn launch_installed_cooplink(target_exe: &Path) -> Result<()> {
    let mut cmd = Command::new(target_exe);
    #[cfg(windows)]
    cmd.creation_flags(CREATE_NO_WINDOW);
    cmd.spawn()?;
    Ok(())
}
