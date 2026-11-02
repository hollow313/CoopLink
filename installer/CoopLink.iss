; ------------------------------------------------------------------------------
; CoopLink - Windows 11 Official Professional Inno Setup Script
; Generates standalone CoopLink-Setup-v1.0.0.exe
; (c) 2026 Loic HUON. All Rights Reserved.
; ------------------------------------------------------------------------------

#define MyAppName "CoopLink"
#define MyAppVersion "1.0.0"
#define MyAppPublisher "Loic HUON"
#define MyAppURL "https://github.com/hollow313/CoopLink"
#define MyAppExeName "cooplink.exe"

[Setup]
AppId={{E8B3A824-77A1-48C0-8E0E-2F3B66024AA1}
AppName={#MyAppName}
AppVersion={#MyAppVersion}
AppPublisher={#MyAppPublisher}
AppPublisherURL={#MyAppURL}
AppSupportURL={#MyAppURL}
AppUpdatesURL={#MyAppURL}
DefaultDirName={autopf}\{#MyAppName}
DisableProgramGroupPage=yes
LicenseFile=..\LICENSE.md
OutputDir=..\dist
OutputBaseFilename=CoopLink-Setup-v1.0.0
Compression=lzma2/ultra64
SolidCompression=yes
WizardStyle=modern
PrivilegesRequired=admin
PrivilegesRequiredOverridesAllowed=dialog
ArchitecturesInstallIn64BitMode=x64
ArchitecturesAllowed=x64compatible
MinVersion=10.0.17763

[Languages]
Name: "french"; MessagesFile: "compiler:Languages\French.isl"
Name: "english"; MessagesFile: "compiler:Default.isl"

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked
Name: "firewall"; Description: "Autoriser automatiquement CoopLink dans le Pare-feu Windows Defender"; GroupDescription: "Réseau & Gaming"; Flags: checkedonce
Name: "vigembus"; Description: "Installer le pilote de manette virtuelle ViGEmBus (manettes des invités)"; GroupDescription: "Pilotes et périphériques"; Flags: checkedonce

[Files]
Source: "..\target\release\{#MyAppExeName}"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\README.md"; DestDir: "{app}"; Flags: isreadme
Source: "..\LICENSE.md"; DestDir: "{app}"; Flags: ignoreversion
Source: "redist\ViGEmBus_1.22.0_x64_x86_arm64.exe"; DestDir: "{app}\redist"; Flags: ignoreversion

[Icons]
Name: "{autoprograms}\{#MyAppName}"; Filename: "{app}\{#MyAppExeName}"
Name: "{autodesktop}\{#MyAppName}"; Filename: "{app}\{#MyAppExeName}"; Tasks: desktopicon

[Run]
Filename: "{app}\redist\ViGEmBus_1.22.0_x64_x86_arm64.exe"; Parameters: "/quiet /norestart"; StatusMsg: "Installation du pilote de manette virtuelle ViGEmBus..."; Flags: runhidden waituntilterminated; Check: (not ViGEmBusInstalled) and WizardIsTaskSelected('vigembus')
Filename: "powershell.exe"; Parameters: "-NoProfile -NonInteractive -WindowStyle Hidden -Command ""New-NetFirewallRule -DisplayName 'CoopLink - Core Virtual LAN' -Direction Inbound -Program '{app}\{#MyAppExeName}' -Action Allow -Profile Any -ErrorAction SilentlyContinue; New-NetFirewallRule -DisplayName 'CoopLink - UDP Gaming Discovery' -Direction Inbound -Protocol UDP -LocalPort 42422 -Action Allow -Profile Any -ErrorAction SilentlyContinue; New-NetFirewallRule -DisplayName 'CoopLink - Stream TCP' -Direction Inbound -Protocol TCP -LocalPort 47500 -Action Allow -Profile Any -ErrorAction SilentlyContinue; if (Test-Path '{app}\cooplink-capture-service.exe') {{ New-NetFirewallRule -DisplayName 'CoopLink - Capture Service' -Direction Inbound -Program '{app}\cooplink-capture-service.exe' -Action Allow -Profile Any -ErrorAction SilentlyContinue }}"""; Flags: runhidden; Tasks: firewall
Filename: "{app}\{#MyAppExeName}"; Description: "{cm:LaunchProgram,{#StringChange(MyAppName, '&', '&&')}}"; Flags: nowait postinstall skipifsilent

[Code]
// Le pilote ViGEmBus est nécessaire pour les manettes des joueurs distants.
function ViGEmBusInstalled: Boolean;
begin
  Result := RegKeyExists(HKLM, 'SYSTEM\CurrentControlSet\Services\ViGEmBus');
end;
