; Script d'installation professionnel Inno Setup pour CoopLink
; Permet de générer un installeur autonome Windows .exe sans aucune console CMD.

#define MyAppName "CoopLink"
#define MyAppVersion "1.0.0"
#define MyAppPublisher "Loic HUON"
#define MyAppURL "https://github.com/hollow313/CoopLink"
#define MyAppExeName "cooplink.exe"

[Setup]
AppId={{8B1D4E3A-9F2C-4A7B-8E15-6C4D0A1E2F3B}
AppName={#MyAppName}
AppVersion={#MyAppVersion}
AppPublisher={#MyAppPublisher}
AppPublisherURL={#MyAppURL}
AppSupportURL={#MyAppURL}
AppUpdatesURL={#MyAppURL}
DefaultDirName={autopf}\{#MyAppName}
DisableProgramGroupPage=yes
LicenseFile=..\..\LICENSE.md
OutputDir=..\..\target\installer
OutputBaseFilename=CoopLink_Setup_v1.0.0
Compression=lzma2/ultra64
SolidCompression=yes
WizardStyle=modern
PrivilegesRequired=admin
ArchitecturesInstallIn64BitMode=x64

[Languages]
Name: "french"; MessagesFile: "compiler:Languages\French.isl"
Name: "english"; MessagesFile: "compiler:Default.isl"

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"
Name: "firewallrule"; Description: "Autoriser automatiquement CoopLink dans le Pare-feu Windows Defender"; GroupDescription: "Réseau et Sécurité:"
Name: "vigembus"; Description: "Installer le pilote de manette virtuelle ViGEmBus (manettes des invités)"; GroupDescription: "Pilotes et périphériques"; Flags: checkedonce

[Files]
Source: "..\..\target\release\{#MyAppExeName}"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\..\README.md"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\..\LICENSE.md"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\..\installer\redist\ViGEmBus_1.22.0_x64_x86_arm64.exe"; DestDir: "{app}\redist"; Flags: ignoreversion

[Icons]
Name: "{autoprograms}\{#MyAppName}"; Filename: "{app}\{#MyAppExeName}"
Name: "{autodesktop}\{#MyAppName}"; Filename: "{app}\{#MyAppExeName}"; Tasks: desktopicon

[Run]
Filename: "{app}\redist\ViGEmBus_1.22.0_x64_x86_arm64.exe"; Parameters: "/quiet /norestart"; StatusMsg: "Installation du pilote de manette virtuelle ViGEmBus..."; Flags: runhidden waituntilterminated; Check: (not ViGEmBusInstalled) and WizardIsTaskSelected('vigembus')
Filename: "{app}\{#MyAppExeName}"; Description: "{cm:LaunchProgram,{#StringChange(MyAppName, '&', '&&')}}"; Flags: nowait postinstall skipifsilent

[Code]
// Le pilote ViGEmBus est nécessaire pour les manettes des joueurs distants.
function ViGEmBusInstalled: Boolean;
begin
  Result := RegKeyExists(HKLM, 'SYSTEM\CurrentControlSet\Services\ViGEmBus');
end;
