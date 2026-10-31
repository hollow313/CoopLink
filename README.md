# ⚡ CoopLink — Virtual LAN P2P & Co-op Streaming Engine

> **Jouez à vos jeux vidéo préférés en coopération locale (LAN & Remote Play Co-op), même à distance.**  
> *Moteur 100% Rust haute performance — Architecture Peer-to-Peer directe, Streaming Basse Latence (Sunshine / Moonlight) & Partage de Manettes (ViGEmBus).*

---

## 🎨 Charte Graphique & Identité Visuelle (Windows 11 Fluent Light)

CoopLink adopte un design moderne, lumineux et épuré aligné sur **Windows 11 Fluent Design** et les couleurs exactes de son logo officiel (`images/CoopLink.png`) :
- **Surfaces & Cartes** : Blanc pur (`#FFFFFF`) et fond Mica clair (`#F4F7FB`) avec bordures subtiles (`#E2E8F0`) — aucun thème sombre "bleu nuit" ni style rétro.
- **Accents du Logo** :
  - Bleu électrique principal : `#0066FF`
  - Cyan / Turquoise vibrant : `#00BCF2`
  - Vert émeraude / Menthe réseau : `#00BA88`
  - Violet / Indigo : `#7B3FE4`
  - Orange corail (sphère centrale) : `#FF7A28`
- **Installateur Windows 11 classique (`cooplink-setup.exe`)** : Assistant d'installation natif en 4 étapes (Bienvenue, Contrat de Licence, Dossier & Options système, Progression & Fin).

---

## 🎮 Module de Streaming Co-op Basse Latence (Sunshine + Moonlight + ViGEmBus)

CoopLink intègre directement une orchestration **Sunshine (Hôte)** et **Moonlight (Client)** sur le maillage virtuel P2P :

### 1. Architecture Hôte (Serveur Sunshine embarqué/orchestré)
- **Isolation Réseau Sécurisée** : L'écoute de Sunshine est liée exclusivement à l'interface virtuelle CoopLink (`bind_address = 10.244.0.1`, `upnp = disabled`), pilotée via l'API locale HTTPS/REST (`https://127.0.0.1:47990/api/`).
- **Capture & Encodage Matériel** : Détection automatique du GPU (`NVENC`, `AMF`, `QuickSync` avec fallback logiciel) et adaptation dynamique du débit (1080p @ 60 FPS ou 1440p @ 120 FPS selon la bande passante du tunnel P2P).
- **Partage de Manettes Multi-Joueurs (`ViGEmBus`)** :
  - Émulation de manettes virtuelles **Xbox 360 (XInput)** ou **DualShock 4**.
  - **Zéro conflit d'assignation** : Le **Joueur 1** est verrouillé sur l'Hôte local (Slot XInput `#0`), tandis que les amis distants sont automatiquement assignés aux **Joueur 2, Joueur 3 et Joueur 4** (Slots XInput `#1`, `#2`, `#3`).

### 2. Architecture Client (Renderer Moonlight)
- **Décodage Matériel Obligatoire (`< 5 ms`)** : Support `HEVC (H.265)`, `AV1` et `H.264` en mode **Fenêtré intégré** ou **Plein écran sans bordure**.
- **Capture Périphériques** : Hooking XInput / DirectInput / SDL2 à très basse latence, avec interrupteur configurable pour autoriser ou bloquer le partage Clavier/Souris.

### 3. Appairage Automatique « Zero-Config »
- **Zéro code PIN à 4 chiffres à saisir** : Le canal de contrôle chiffré P2P de CoopLink échange automatiquement les certificats SSL et valide le PIN sur l'API locale de l'Hôte.
- **Zéro copier-coller d'IP** : Le client découvre automatiquement l'IP virtuelle de l'Hôte (`10.244.0.1`) dès la connexion au salon.

---

## 🔗 Architecture 100% Peer-to-Peer (P2P) Directe

- **Aucun trafic de jeu ni flux vidéo ne transite par le serveur central** pendant vos sessions.
- Le serveur central (`server/`) intervient **uniquement au tout début** (signalisation UDP sur le port `30024`) pour :
  1. Créer ou référencer le salon et attribuer un `ClientID` unique.
  2. Échanger les endpoints publics (`IP:Port`) des pairs afin d'établir le **tunnel P2P direct (UDP Hole Punching)**.
- Le port `30025/tcp` expose le **Dashboard Web d'Administration** (supervision des salons et des `ClientID`s, utilisateur Docker non-root `568:568`, volume `/data`).

---

## 🔨 Compilation & Exécutables

```powershell
# Compiler le Client et l'Installateur Windows 11 en mode Release
cargo build --release --bin cooplink --bin cooplink-setup
```

Les exécutables générés se trouvent dans `target/release/` :
- `target/release/cooplink-setup.exe` : Assistant d'installation Windows 11
- `target/release/cooplink.exe` : Application Client CoopLink

---

## 📜 Licence Commerciale & Propriétaire

Ce logiciel est sous **licence commerciale propriétaire fermée**.  
Tous droits réservés © 2026 Loic HUON. Voir [LICENSE.md](LICENSE.md).
