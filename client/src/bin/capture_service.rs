// CoopLink Capture & Streaming Service Daemon
// Copyright (c) 2026 Loic HUON. All Rights Reserved.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::net::UdpSocket;
use std::time::Duration;
use tracing::info;

fn main() {
    tracing_subscriber::fmt::init();
    info!("CoopLink Capture Service démarré...");

    // Écoute sur le port 47989 pour les entrées manettes et pings des invités
    let socket = match UdpSocket::bind("0.0.0.0:47989") {
        Ok(s) => s,
        Err(_) => match UdpSocket::bind("0.0.0.0:0") {
            Ok(s) => s,
            Err(e) => {
                eprintln!("Erreur bind UDP: {}", e);
                return;
            }
        },
    };

    let _ = socket.set_read_timeout(Some(Duration::from_millis(500)));
    let mut buf = [0u8; 2048];

    loop {
        if let Ok((len, src)) = socket.recv_from(&mut buf) {
            if let Ok(msg) = std::str::from_utf8(&buf[..len]) {
                if msg.starts_with("VIEWER:HELLO") {
                    let _ = socket.send_to(b"HOST:ACK:READY", src);
                } else if msg.starts_with("INPUT:") {
                    // Réception des touches/manettes d'un invité
                    // Format: INPUT:SLOT2:Z,Q,S,D
                }
            }
        }
    }
}
