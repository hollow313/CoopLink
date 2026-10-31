# ⚡ CoopLink — Virtual LAN Gaming Engine

> **Jouez à vos jeux vidéo préférés en coopération locale (LAN), même à des centaines de kilomètres de distance.**  
> *Développé en Rust haute performance pour Windows 11 (Architecture prête pour macOS et Linux).*

---

## 🕹 Présentation

**CoopLink** est une solution logicielle propriétaire ultra-légère et ultra-stable conçue spécifiquement pour les joueurs de jeux vidéo.  
Son objectif : **faire croire aux moteurs de jeux vidéo que vous et vos amis êtes physiquement branchés sur le même switch réseau local (LAN)**, même si vous résidez dans des villes ou pays différents.

Que ce soit pour les jeux rétro des années 90/2000 (Warcraft III, Age of Empires II, StarCraft, Doom, Heroes of Might & Magic III, Counter-Strike 1.6) ou les jeux coopératifs modernes (Minecraft LAN, Borderlands, Unreal Engine, Left 4 Dead), **CoopLink élimine les tracas de configuration NAT, de box internet et de ports bloqués**.

---

## 🚀 Fonctionnalités Clés

### 1. 🌐 Ouverture Automatique de Port UPnP (Zéro Configuration Box)
Pour l'Hôte de la partie, **plus besoin d'ouvrir manuellement sa box internet** :
- CoopLink détecte la passerelle UPnP IGD de la box (Livebox, Freebox, Bbox, SFR Box, routeurs ASUS/TP-Link).
- Dès que vous créez une salle, le port UDP `42422` est ouvert automatiquement sur la box.
- Votre adresse IP publique est automatiquement détectée et affichée dans le Lobby avec un bouton de copie en un clic.
- Lorsque vous quittez la salle ou fermez CoopLink, le port est refermé proprement.

### 2. ⚡ Métrique Réseau Basse Prioritaire (Wi-Fi & Box Override)
Dans Windows, chaque carte réseau possède une **Métrique d'Interface** qui définit sa priorité dans la table de routage :
- Le Wi-Fi a généralement une métrique de **35 à 50+**.
- L'Ethernet filaire a généralement une métrique de **25**.
- Si un adaptateur virtuel possède une métrique trop élevée, Windows envoie les requêtes de découverte locale (`255.255.255.255`) sur votre carte Wi-Fi physique vers votre box internet, qui les rejette immédiatement !

👉 **CoopLink configure automatiquement une métrique basse (valeur 5)** sur son interface réseau. Résultat : **les paquets de découverte LAN de vos jeux vidéo passent en priorité absolue sur le tunnel CoopLink avant votre box ou votre Wi-Fi.**

### 2. 🛡 Gestion Automatique du Pare-Feu Windows Defender
Windows Defender bloque par défaut les connexions non sollicitées et les annonces UDP distantes.  
CoopLink intègre un gestionnaire de pare-feu qui injecte et vérifie automatiquement :
- L'autorisation complète de l'application en entrée/sortie.
- L'ouverture du sous-réseau virtuel de jeu (`10.244.0.0/24`).
- L'autorisation du broadcast de découverte UDP sur les ports de jeux.
- Un bouton de **réparation en 1 clic** directement dans l'interface.

### 3. 👥 Multijoueur Multi-Clients (2 joueurs ou plus par Nœud)
CoopLink n'est pas limité à un simple tunnel 1-vers-1 :
- **Mode Hôte (Nœud Maître)** : Un joueur crée un salon (ex: `COOP-7701`). Il reçoit automatiquement l'IP virtuelle `10.244.0.1`.
- **Mode Client (Rejoindre)** : Plusieurs amis (2, 4, 8, 16 joueurs ou plus) rejoignent le même nœud avec l'adresse ou le code de salon.
- Chaque joueur reçoit une IP dédiée unique (`10.244.0.2`, `10.244.0.3`, etc.).
- Mesure en temps réel du **ping (latence en millisecondes)** entre tous les membres.

### 4. 📡 Répéteur de Broadcast Gaming Universel
Certains jeux ne routent pas nativement les requêtes `255.255.255.255`. CoopLink intègre un socket de réflexion multi-ports qui capture et renvoie instantanément les trames de découverte :
- **Port 6112 UDP** : Blizzard (Warcraft III, Diablo, StarCraft)
- **Port 27015 UDP** : Valve / Source Engine (Half-Life, Counter-Strike, L4D, TF2)
- **Port 7777 UDP** : Unreal Engine
- **Ports 19132 / 25565 UDP** : Minecraft Bedrock & Java LAN
- **Ports 11155 UDP** : Terraria
- **Ports 1900 / 5353 UDP** : SSDP & mDNS Discovery

### 5. 👾 Design Rétro Gaming Arcade Immersif
L'interface graphique a été conçue avec un thème néon arcade rétro soigné :
- Palette Dark Space (`#0a0b12`), vert phosphore CRT (`#00ff8c`), cyan cyber (`#00e5ff`) et ambre arcade.
- Indicateurs LED de statut en direct.
- Boutons cartouches arcade avec survol lumineux.
- Terminal de logs façon borne d'arcade des années 80/90.

### 6. ⚙ Administration et Paramètres Intégrés
Réglez tous les paramètres directement dans le logiciel :
- Port UDP configurable (défaut : `42422`).
- Plage IP virtuelle ajustable (`10.244.0.0/24`).
- MTU personnalisé (défaut : `1420` sans fragmentation).
- Automatisation de la métrique et du pare-feu.
- Mode headless dédié pour les serveurs sans interface graphique (`cooplink --headless --host`).

### 7. 🌐 Architecture Multiplateforme (Windows 11 immédiat, Mac & Linux prêts)
Le code est structuré autour du trait modulaire `VirtualAdapter` :
- `WindowsVirtualAdapter` : Cible prioritaire Windows 11 avec métrique et pare-feu Win32.
- `LinuxVirtualAdapter` : Stub prêt pour `/dev/net/tun` (prévu pour la version ultérieure).
- `MacVirtualAdapter` : Stub prêt pour `utun` macOS (prévu pour la version ultérieure).

---

## 📦 Installation & Mire d'Installation Professionnelle

Pour une expérience utilisateur commerciale et soignée, **aucune fenêtre de commande noire (CMD) n'est affichée**. CoopLink propose deux modes d'installation :

### 1. Mire d'installation graphique native (`cooplink-setup.exe`)
Lancez simplement `cooplink-setup.exe` :
- Assistant graphique pas-à-pas (Bienvenue, Contrat de Licence, Choix du répertoire).
- Options de création des raccourcis (Bureau et Menu Démarrer).
- Configuration automatique et transparente du Pare-feu Windows Defender sans invite de commande.
- Enregistrement propre dans le système Windows 11 pour la désinstallation.
- Case à cocher pour lancer le logiciel immédiatement à la fin.

### 2. Script Inno Setup (`installer/CoopLink_Setup.iss`)
Un script Inno Setup standard de l'industrie est également fourni pour packager un installeur Windows officiel autonome compressé en LZMA2.
- **Système d'exploitation** : Windows 11 (ou Windows 10 version 2004+) 64-bit.
- **Droits Administrateur** : Recommandés pour l'application automatique de la métrique basse et des règles pare-feu Windows.
- **Compilateur Rust** (si compilation depuis les sources) : Rust 1.80+ avec Cargo.

---

## 🔨 Compilation depuis les sources

```powershell
# Cloner le dépôt
git clone https://github.com/hollow313/CoopLink.git
cd CoopLink

# Compilation en mode Release optimisé
cargo build --release

# L'exécutable autonome est généré dans target/release/cooplink.exe
```

Le binaire `cooplink.exe` est 100% autonome et pèse environ **5 Mo** sans runtime lourd.

---

## 🎮 Guide d'utilisation Rapide

### Scénario 1 : Vous êtes l'Hôte de la partie
1. Lancez `cooplink.exe`.
2. Choisissez votre pseudo dans l'onglet **Lobby**.
3. Choisissez ou conservez le code de salon (ex: `COOP-7701`).
4. Cliquez sur **👑 CRÉER & HÉBERGER UN NŒUD (HOST)**.
5. Votre IP virtuelle s'affiche (`10.244.0.1`).
6. Donnez votre IP publique ou IP locale à vos amis avec le code de salon.
7. Lancez votre jeu vidéo et hébergez une partie en réseau local (LAN).

### Scénario 2 : Vous rejoignez un ami
1. Lancez `cooplink.exe`.
2. Entrez l'IP de votre ami et le code de salon.
3. Cliquez sur **🚀 REJOINDRE LE NŒUD (CLIENT)**.
4. Une fois connecté, vous recevrez une IP virtuelle unique (ex: `10.244.0.2`).
5. Ouvrez le jeu vidéo : la partie de votre ami apparaît immédiatement dans la liste des parties locales (LAN) ! Si le jeu requiert une IP directe, entrez simplement l'IP virtuelle de l'hôte (`10.244.0.1`).

---

## 💻 Mode Serveur Dédié (Headless)

Pour héberger un nœud CoopLink en arrière-plan ou sur une machine dédiée sans interface graphique :

```powershell
cooplink.exe --headless --host --port 42422 --room COOP-7701
```

---

## 📜 Licence Commerciale & Propriétaire

Ce logiciel est sous **licence commerciale propriétaire fermée**.  
Tous droits réservés © 2026 Loic HUON.

Toute reproduction, redistribution, décompilation ou revente sans autorisation écrite préalable est strictement interdite. Voir le fichier [LICENSE.md](LICENSE.md) pour les conditions complètes.
