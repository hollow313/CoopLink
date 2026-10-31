# ⚡ CoopLink — Virtual LAN Gaming Engine

> **Jouez à vos jeux vidéo préférés en coopération locale (LAN), même à distance.**  
> *Moteur 100% Rust haute performance, architecture modulaire Client & Serveur.*

---

## 🎨 Charte Graphique & Identité Visuelle

CoopLink intègre une identité visuelle moderne inspirée de son logo officiel :
- **Nuances dominantes** : Bleu électrique (`#0057FF`), Cyan vibrant (`#00D2FF`), Bleu nuit profond (`#0038A8`).
- **Nuances secondaires** : Vert menthe (`#00BFA5`), Vert vif (`#00E599`), Indigo (`#651FFF`), Orange corail (`#FF6D00`).
- **Fonds & Contrastes** : Dark Blue Space (`#090D16`), Blanc pur (`#FFFFFF`), Gris neutre (`#8E8E93`).

---

## 📁 Architecture du Projet

Le projet est structuré en **Workspace Cargo** :
```
CoopLink/
├── client/                     # Application Client Gaming & Mire d'installation Windows 11
│   ├── src/                    # Code source du client (GUI moderne, Wintun, UPnP, Firewall)
│   ├── installer/              # Script Inno Setup & code de l'assistant d'installation
│   └── Cargo.toml
│
├── server/                     # Serveur centralisé de coordination, relais & Web Admin
│   ├── src/
│   │   ├── hub.rs              # Hub UDP 30024 : attribution des ClientIDs, STUN, relais
│   │   ├── web.rs              # Dashboard Web d'administration en temps réel
│   │   ├── config.rs           # Configuration et variables d'environnement
│   │   └── main.rs
│   ├── Dockerfile              # Container Docker multi-stage (Utilisateur non-root 568)
│   └── Cargo.toml
│
├── .github/workflows/          # CI/CD GitHub Actions
│   └── docker-build.yml        # Build & publication automatique du container Docker
│
├── images/                     # Logos et ressources visuelles
├── docker-compose.yml          # Déploiement en 1 commande du serveur
├── Cargo.toml                  # Workspace racine
└── LICENSE.md                  # Licence Commerciale Propriétaire
```

---

## 🖥️ 1. Le Client Gaming (`client/`)

- **Binaire autonome** : `cooplink.exe` (sans runtime externe lourd, zéro fenêtre noire CMD).
- **Mire d'installation graphique** : `cooplink-setup.exe` (assistant Windows complet avec logo, licence, raccourcis, pare-feu automatique et désinstallation propre).
- **Ouverture de port automatique UPnP** : aucune configuration manuelle de box internet requise pour l'hôte.
- **Métrique basse prioritaire (valeur 5)** : garantit que Windows achemine les paquets de découverte LAN sur CoopLink avant le Wi-Fi physique ou la box.
- **Répéteur de Broadcast Gaming** : compatible Blizzard (6112), Valve Source (27015), Unreal Engine (7777), Minecraft LAN, etc.
- **Multi-joueurs** : supporte 2 clients ou plus connectés ensemble sur le même salon.

---

## 🐳 2. Le Serveur Centralisé & Container Docker (`server/`)

Le serveur centralise la signalisation, l'attribution des identifiants uniques (`ClientID`), la traversée de NAT (UDP hole punching) et le relais de secours.

### Caractéristiques du Container :
- **Port UDP `30024`** : Tunnel gaming, coordination et paquets de découverte.
- **Port TCP `8080`** : Dashboard Web d'administration en direct.
- **Utilisateur de sécurité** : Exécuté sous l'UID/GID **`568:568`** (non-root).
- **Volume persistant à binder** : **`/data`** (stocke la configuration `server_config.json`, les données et l'historique).
- **Build automatique GitHub Actions** : Déclenché à chaque push sur `main`, publie l'image sur GitHub Packages (`ghcr.io/hollow313/cooplink-server:latest`).

### 🚀 Lancement avec Docker Compose

```yaml
services:
  cooplink-server:
    image: ghcr.io/hollow313/cooplink-server:latest
    container_name: cooplink-server
    restart: unless-stopped
    user: "568:568"
    ports:
      - "30024:30024/udp"   # Port de signalisation gaming
      - "8080:8080/tcp"     # Dashboard Web d'administration
    environment:
      - PORT_UDP=30024
      - PORT_WEB=8080
      - DATA_DIR=/data
      - SERVER_NAME=CoopLink Official Master Hub
    volumes:
      - ./data:/data        # Dossier bindé pour la persistance
```

Pour lancer le serveur :
```bash
docker compose up -d
```

### 🌐 Dashboard Web d'Administration
Ouvrez simplement votre navigateur sur `http://<IP-SERVEUR>:8080` :
- Vue d'ensemble des **Salons Actifs** et du nombre de joueurs par salon.
- Liste complète de **tous les Clients Connectés avec leur ClientID unique** (ex: `#1001`, `#1002`...), leur IP virtuelle, leur endpoint public et leur latence (Ping).
- Statistiques de débit réseau (Mo relayés en direct).
- Bouton d'expulsion de salon en 1 clic pour les administrateurs.
- API REST intégrée : `/api/stats`, `/api/rooms`.

---

## 🔨 Compilation depuis les sources

```powershell
# Cloner le dépôt
git clone https://github.com/hollow313/CoopLink.git
cd CoopLink

# Compiler tout le workspace (Client + Installateur + Serveur)
cargo build --release --workspace

# Les exécutables sont générés dans target/release/ :
# - cooplink.exe (Client Gaming)
# - cooplink-setup.exe (Installateur Client Windows)
# - cooplink-server.exe (Serveur autonome)
```

---

## 📜 Licence Commerciale & Propriétaire

Ce logiciel est sous **licence commerciale propriétaire fermée**.  
Tous droits réservés © 2026 Loic HUON. Voir [LICENSE.md](LICENSE.md).
