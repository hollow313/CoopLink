# ------------------------------------------------------------------------------
# CoopLink Server - Multi-stage Production Dockerfile
# User: 568 (Non-root security standard)
# Ports: 30024/udp (Gaming Tunnel), 8080/tcp (Web Dashboard)
# ------------------------------------------------------------------------------

# --- Étape 1 : Build du binaire Rust ---
FROM rust:1.88-bullseye AS builder

WORKDIR /build

# Copie des manifestes pour mise en cache des dépendances
COPY Cargo.toml ./Cargo.toml
COPY server/Cargo.toml ./server/Cargo.toml
COPY client/Cargo.toml ./client/Cargo.toml

# Création de stubs temporaires pour compiler les dépendances
RUN mkdir -p server/src client/src && \
    echo "fn main() {}" > server/src/main.rs && \
    echo "fn main() {}" > client/src/main.rs && \
    cargo check --bin cooplink-server --release || true

# Copie du code source complet
COPY server ./server

# Compilation du binaire release optimisé
RUN cargo build --bin cooplink-server --release

# --- Étape 2 : Image Runtime légère & sécurisée ---
FROM debian:bullseye-slim

# Installation des certificats CA
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates && \
    rm -rf /var/lib/apt/lists/*

# Création du groupe et de l'utilisateur non-root 568 demandé
RUN groupadd -g 568 cooplink && \
    useradd -u 568 -g cooplink -m -s /bin/sh cooplink

# Création et attribution des droits sur le répertoire persistant /data
RUN mkdir -p /data && chown -R 568:568 /data

WORKDIR /home/cooplink

# Copie du binaire compilé depuis l'étape précédente
COPY --from=builder --chown=568:568 /build/target/release/cooplink-server /usr/local/bin/cooplink-server

# Définition de l'utilisateur 568
USER 568:568

# Variables d'environnement par défaut
ENV PORT_UDP=30024 \
    PORT_WEB=8080 \
    DATA_DIR=/data \
    SERVER_NAME="CoopLink Master Hub" \
    RUST_LOG=info

# Exposition des ports
# 30024/udp : Signalisation, coordination et tunnel UDP
# 8080/tcp  : Interface Web d'administration et métriques
EXPOSE 30024/udp
EXPOSE 8080/tcp

# Volume de persistance bindable
VOLUME ["/data"]

# Commande d'entrée
ENTRYPOINT ["/usr/local/bin/cooplink-server"]
