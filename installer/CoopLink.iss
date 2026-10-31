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
PrivilegesRequired=lowest
PrivilegesRequiredOverridesAllowed=dialog
ArchitecturesInstallIn64BitMode=x64

[Languages]
Name: "french"; MessagesFile: "compiler:Languages\French.isl"
Name: "english"; MessagesFile: "compiler:Default.isl"

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked
Name: "firewall"; Description: "Autoriser automatiquement CoopLink dans le Pare-feu Windows Defender"; GroupDescription: "Réseau & Gaming"; Flags: checkedonce

[Files]
Source: "..\target\release\{#MyAppExeName}"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\README.md"; DestDir: "{app}"; Flags: isreadme
Source: "..\LICENSE.md"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{autoprograms}\{#MyAppName}"; Filename: "{app}\{#MyAppExeName}"
Name: "{autodesktop}\{#MyAppName}"; Filename: "{app}\{#MyAppExeName}"; Tasks: desktopicon

[Run]
Filename: "powershell.exe"; Parameters: "-NoProfile -NonInteractive -WindowStyle Hidden -Command ""New-NetFirewallRule -DisplayName 'CoopLink - Core Virtual LAN' -Direction Inbound -Program '{app}\{#MyAppExeName}' -Action Allow -Profile Any -ErrorAction SilentlyContinue; New-NetFirewallRule -DisplayName 'CoopLink - UDP Gaming Discovery' -Direction Inbound -Protocol UDP -LocalPort 42422 -Action Allow -Profile Any -ErrorAction SilentlyContinue"""; Flags: runhidden; Tasks: firewall
Filename: "{app}\{#MyAppExeName}"; Description: "{cm:LaunchProgram,{#StringChange(MyAppName, '&', '&&')}}"; Flags: nowait postinstall skipifsilent
