# Déploiement & test — CoopLink 1.0.0

## Ce qu'il y a dans ce dossier

| Élément | Rôle |
|---|---|
| `dist/CoopLink-Setup-v1.0.0.exe` | Installateur complet (app + encodeur + pilote manettes), 35 Mo |
| `build/` | Binaires Windows non empaquetés, pour test rapide |
| `installer/redist/ffmpeg.exe` | Encodeur H.264 embarqué par l'installateur |

SHA-256 de l'installateur : `15d66cc5aef91ca77449ad6687ca5f518158672c28f487cd3e0a9f9256780abc`

## Installer sur un poste

1. Lancer `dist/CoopLink-Setup-v1.0.0.exe` (accepter l'élévation admin).
   Non signé : « Informations complémentaires → Exécuter quand même ».
   Cocher « ViGEmBus » (manettes des invités) et « Pare-feu ».
2. Vérifier que `C:\Program Files\CoopLink\` contient `cooplink.exe` et `ffmpeg.exe`.
3. Au premier lancement, `cooplink-viewer.exe` et `cooplink-capture-service.exe`
   sont extraits automatiquement à côté de `cooplink.exe`.
4. Onglet Partage : la ligne doit afficher « Manette virtuelle : pilote ViGEmBus détecté ».

## Configurer une session

- Réglages → « serveur central » = `82.66.152.133` (relais de repli, port 47510).
- Même **code de salon** et même **mot de passe** sur les deux postes.
- Hôte : créer/héberger le salon → lancer le jeu → « Actualiser les jeux ouverts » →
  sélectionner la fenêtre → « Démarrer le Partage du Jeu ».
- Invité : rejoindre le salon → onglet « Je rejoins un ami » → « Rejoindre le Jeu de l'Hôte ».

L'ordre compte : la clé de diffusion est dérivée du code + mot de passe du salon.
Partager avant d'héberger produit un flux que personne ne peut déchiffrer.

## Côté NAS (hub + relais)

```bash
docker compose up -d --force-recreate   # recréation obligatoire (PORT_WEB change)
ss -lnt | grep -E '30024|30026|47510'
curl -s http://127.0.0.1:30026/api/stats
```
Attendu : hub sur `30024` (UDP + TCP), web admin sur `30026`, relais `47510`.
NAT sur la box : UDP 30024 et TCP 47510 → NAS.

## Limites connues

- Pas de carte réseau virtuelle : la découverte LAN des jeux est relayée par broadcast
  (6112, 27015, 7777, 19132, 25565…), mais un jeu qui rappelle une IP LAN échouera.
- Seule la fenêtre au premier plan est diffusée.
- Le chemin direct dépend de l'UPnP (UDP 42422 + TCP 47500) ; sinon repli relais.
