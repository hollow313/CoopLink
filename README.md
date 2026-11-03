# ⚡ CoopLink — Virtual LAN P2P & Co-op Game Sharing Engine

> **Jouez à vos jeux vidéo préférés en coopération locale (LAN & Partage de Jeu Co-op au Premier Plan), même à distance.**  
> *Moteur 100% Rust haute performance — Architecture Peer-to-Peer chiffrée de bout en bout, Partage de Jeu Basse Latence (Anti-Écho Audio) & Bus Manettes Multi-Joueurs.*

---

## 🎨 Charte Graphique & Identité Visuelle (Windows 11 Fluent Light)

CoopLink adopte un design moderne, lumineux et épuré aligné sur **Windows 11 Fluent Design** et les couleurs exactes de son logo officiel (`images/CoopLink.png`) :
- **Surfaces & Cartes** : Blanc pur (`#FFFFFF`) et fond Mica clair (`#F4F7FB`) avec bordures subtiles (`#E2E8F0`).
- **Accents du Logo** :
  - Bleu électrique principal : `#0066FF`
  - Cyan / Turquoise vibrant : `#00BCF2`
  - Vert émeraude / Menthe réseau : `#00BA88`
  - Violet / Indigo : `#7B3FE4`
  - Orange corail (sphère centrale) : `#FF7A28`
- **Multilingue International (Détection OS automatique)** : Français (`FR`), English (`US/EN`), Español (`ES`), Deutsch (`DE`), Português (`PT`).

---

## 🎮 Partage de Jeu au Premier Plan & Manettes Co-op

CoopLink intègre son propre moteur d'orchestration de capture vidéo matérielle et de partage de périphériques sur le maillage virtuel P2P :

### 1. Architecture Hôte (`CoopCaptureHost`)
- **Capture Exclusive du Jeu au Premier Plan** : CoopLink passe en arrière-plan (ou se réduit dans la barre des tâches lorsque l'on clique sur la croix `X`) et ne diffuse que la fenêtre du jeu actif au premier plan.
- **Anti-Écho Audio Intelligent** : Isole la piste sonore du jeu au premier plan et exclut automatiquement les logiciels de discussion vocale (Discord, TeamSpeak, micro) pour éviter tout écho.
- **Partage de Manettes & Clavier/Souris Multi-Joueurs** :
  - **Mode A (2 à 4 Manettes)** : Joueur 1 = Hôte local (Slot `#0`), Joueurs 2/3/4 = Amis distants (Slots `#1`, `#2`, `#3`).
  - **Mode B (2 Claviers/Souris)** : L'Hôte joue au Clavier/Souris (J1) et le Clavier/Souris de l'invité est converti en Manette Virtuelle Joueur 2 sans conflit de curseur.
  - **Mode C (Clavier/Souris Partagé)** : Pour les jeux au tour par tour ou sur un même clavier.

### 2. Pilote manette virtuelle embarqué (ViGEmBus)
- L'installateur CoopLink déploie **ViGEmBus 1.22.0** (binaire signé, BSD-3-Clause, redistribué tel quel dans `installer/redist/`) et l'installe en silencieux (`/quiet /norestart`) si le service `ViGEmBus` est absent.
- Il est aussi embarqué dans `cooplink-setup.exe` (installateur intégré), donc disponible hors ligne.
- L'application vérifie le pilote au démarrage et affiche son état dans l'onglet de partage ; sans lui, la manette d'un joueur distant ne peut pas être transmise au jeu.
- Le pilote nécessite l'élévation : l'installateur Inno demande l'élévation administrateur.

### 3. Test de Débit & Score de Capacité Hôte (`SpeedtestView`)
- Mesure le **Débit Montant (Upload Mbps)**, le **Débit Descendant (Download Mbps)**, la **Latence / Stabilité P2P** et l'**Encodeur Matériel GPU**.
- Attribue une **Note Hôte sur 100** et un verdict clair (**`OK`**, **`MOYEN`**, **`PAS OK`**) permettant à deux amis possédant le jeu de choisir immédiatement celui qui a la meilleure connexion pour héberger.

---

## 🔗 Architecture 100% Peer-to-Peer (P2P) & Hub de Métriques (`server/`)

- **Aucun trafic de jeu ni flux vidéo ne transite par le serveur central** pendant vos sessions.
- Le container Hub (`server/`) :
  - Attribue un **ID Unique d'instance (`CL-1001`, `CL-1002`...)** à chaque logiciel CoopLink lancé (`30024/udp` et `/api/telemetry` sur `30026/tcp`).
  - Expose une **Plateforme Web Moderne de Métriques** sur le port `30026/tcp` (Instances actives, Tunnels P2P montés actifs et cumulés, Sessions de partage de jeu actives, IDs uniques attribués).

---

## 🔨 Compilation & Exécutables

```powershell
# Compiler l'ensemble du Workspace (Client, Installateur Windows 11 & Serveur Hub)
cargo build --release --workspace
```

Les exécutables générés se trouvent dans `target/release/` :
- `target/release/cooplink-setup.exe` : Assistant d'installation Windows 11 (avec CLUF complet, vérification GPU et installation automatique des pilotes virtuels)
- `target/release/cooplink.exe` : Application Client CoopLink
- `target/release/cooplink-server.exe` : Serveur Hub de Métriques & Attribution d'IDs

---

## 📜 Licence Commerciale & Propriétaire

Ce logiciel est sous **licence commerciale propriétaire fermée** avec clause complète d'exonération de responsabilité.  
Tous droits réservés © 2026 Loic HUON. Voir [LICENSE.md](LICENSE.md).
