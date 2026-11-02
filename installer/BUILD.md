# Construire l'installateur CoopLink (Windows)

## Prérequis

- Windows 10/11 x64
- Rust stable, toolchain MSVC (https://rustup.rs)
- Inno Setup 6.2 ou plus récent (https://jrsoftware.org/isdl.php) — installe `ISCC.exe`

## 1. Construire les binaires

```powershell
cargo build --release --workspace
```

Vérifie que `target\release\` contient bien :
`cooplink.exe`, `cooplink-viewer.exe`, `cooplink-capture-service.exe`, `cooplink-setup.exe`.

> Important : `client\build.rs` réembarque les `.exe` présents dans `target\release\` (sinon `bin\`).
> Si tu compiles l'installateur sans avoir rebuild les binaires, tu embarqueras les anciens.

## 2. Compiler l'installateur

Interface graphique : ouvrir `installer\CoopLink.iss`, puis **Build → Compile** (F9).

Ligne de commande :

```powershell
& "C:\Program Files (x86)\Inno Setup 6\ISCC.exe" installer\CoopLink.iss
```

Sortie : `dist\CoopLink-Setup-v1.0.0.exe`.

`client\installer\CoopLink_Setup.iss` produit le même installeur dans `target\installer\` (script historique).

## 3. Ce que fait l'installateur

- copie `cooplink.exe`, `README.md`, `LICENSE.md` dans `{app}` ;
- copie `installer\redist\ViGEmBus_1.22.0_x64_x86_arm64.exe` dans `{app}\redist` ;
- tâche **« Installer le pilote de manette virtuelle ViGEmBus »**, cochée par défaut :
  exécute l'installateur en `/quiet /norestart`, **uniquement si** la clé
  `HKLM\SYSTEM\CurrentControlSet\Services\ViGEmBus` est absente ;
- tâche pare-feu : règles entrantes pour `cooplink.exe` et l'UDP 42422 ;
- demande l'élévation administrateur (`PrivilegesRequired=admin`), indispensable pour un pilote noyau.

## 4. Vérifier après installation

1. `services.msc` → le service **ViGEmBus** est présent et démarré.
2. Lancer CoopLink → onglet de partage → la ligne doit afficher
   « Manette virtuelle : pilote ViGEmBus détecté ».
3. Si elle affiche « pilote absent », relancer l'installateur en cochant la tâche, ou exécuter
   manuellement `{app}\redist\ViGEmBus_1.22.0_x64_x86_arm64.exe`.

## 5. Signature (recommandé)

```powershell
signtool sign /fd SHA256 /tr http://timestamp.digicert.com /td SHA256 /a "dist\CoopLink-Setup-v1.0.0.exe"
```

Sans signature, SmartScreen affichera un avertissement sur un exécutable qui installe un pilote.

## 6. Déploiement du hub (relais)

Le relais TCP écoute sur `PORT_RELAY` (47510 par défaut). Côté Docker, `docker-compose.yml`
publie `47510:47510`, `30024:30024/udp`, `30024:30024/tcp` et `30025:30025/tcp`.
