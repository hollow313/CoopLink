// CoopLink Capture & Streaming Service Daemon
// Copyright (c) 2026 Loic HUON. All Rights Reserved.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::net::UdpSocket;
use std::time::Duration;
use tracing::info;

#[cfg(windows)]
fn send_vk(vk: u16, key_up: bool) {
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
        SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP,
    };
    let flags = if key_up { KEYEVENTF_KEYUP } else { 0 };
    let input = INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: vk,
                wScan: 0,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: 0,
            },
        },
    };
    unsafe {
        SendInput(1, &input, std::mem::size_of::<INPUT>() as i32);
    }
}

fn map_key_to_vk(k: &str) -> Option<u16> {
    match k.trim().to_uppercase().as_str() {
        "Z" | "W" => Some(0x5A), // VK_Z
        "Q" | "A" => Some(0x51), // VK_Q
        "S" => Some(0x53),       // VK_S
        "D" => Some(0x44),       // VK_D
        "HAUT" | "UP" => Some(0x26),    // VK_UP
        "BAS" | "DOWN" => Some(0x28),   // VK_DOWN
        "GAUCHE" | "LEFT" => Some(0x25),// VK_LEFT
        "DROITE" | "RIGHT" => Some(0x27), // VK_RIGHT
        "ESPACE" | "SPACE" => Some(0x20), // VK_SPACE
        "ENTREE" | "ENTER" => Some(0x0D), // VK_RETURN
        _ => None,
    }
}

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
                    // Format: INPUT:SLOT2:Z,Q,S,D
                    let parts: Vec<&str> = msg.split(':').collect();
                    if parts.len() >= 3 {
                        let keys_csv = parts[2];
                        let keys: Vec<&str> = keys_csv.split(',').map(|s| s.trim()).filter(|s| !s.is_empty()).collect();

                        // 1. Injection Windows pour tout jeu en avant-plan
                        #[cfg(windows)]
                        {
                            for &k in &keys {
                                if let Some(vk) = map_key_to_vk(k) {
                                    send_vk(vk, false);
                                    send_vk(vk, true);
                                }
                            }
                        }

                        // 2. Relais vers le simulateur de test s'il tourne sur la machine hôte
                        let test_game_msg = format!("KEYS:999999:Invite:{}", keys_csv);
                        let _ = socket.send_to(test_game_msg.as_bytes(), "127.0.0.1:39999");
                    }
                }
            }
        }
    }
}
