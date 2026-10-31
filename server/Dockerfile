# ------------------------------------------------------------------------------
# CoopLink Server - Production Dockerfile (Alpine Linux)
# User: 568 (Non-root security)
# Ports: 30024/udp (Gaming Tunnel Relay), 30025/tcp (Web Admin Dashboard)
# ------------------------------------------------------------------------------

# --- Étape 1 : Build du binaire Rust statique ---
FROM rust:alpine AS builder

RUN apk add --no-cache musl-dev gcc make

WORKDIR /build

# Copie autonome du manifeste serveur
COPY server/Cargo.toml ./Cargo.toml

# Création d'un stub pour la mise en cache des dépendances
RUN mkdir -p src && \
    echo "fn main() {}" > src/main.rs && \
    cargo check --release || true

# Copie du code source complet du serveur
COPY server/src ./src

# Compilation optimisée du serveur
RUN cargo build --release

# --- Étape 2 : Image Runtime minimale & sécurisée ---
FROM alpine:3.20

RUN apk add --no-cache ca-certificates tzdata shadow && \
    addgroup -g 568 -S cooplink && \
    adduser -u 568 -S cooplink -G cooplink -s /bin/sh -D && \
    mkdir -p /data && chown -R 568:568 /data

WORKDIR /home/cooplink

COPY --from=builder --chown=568:568 /build/target/release/cooplink-server /usr/local/bin/cooplink-server

USER 568:568

ENV PORT_UDP=30024 \
    PORT_WEB=30025 \
    DATA_DIR=/data \
    SERVER_NAME="CoopLink Master Hub" \
    RUST_LOG=info

# Exposition des ports : 30024/udp (Tunnel P2P), 30024/tcp & 30025/tcp (Web Hub & Reverse Proxy)
EXPOSE 30024/udp
EXPOSE 30024/tcp
EXPOSE 30025/tcp

VOLUME ["/data"]

ENTRYPOINT ["/usr/local/bin/cooplink-server"]
