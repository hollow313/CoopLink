// CoopLink Capture & Streaming Service Daemon
// Real-time Screen & Game Capture + Input Injection
// Copyright (c) 2026 Loic HUON. All Rights Reserved.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream, UdpSocket};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};
use tracing::{error, info};

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

#[cfg(windows)]
fn capture_screen_rgb(width: i32, height: i32) -> Option<Vec<u8>> {
    use windows_sys::Win32::Graphics::Gdi::{
        CreateCompatibleBitmap, CreateCompatibleDC, DeleteDC, DeleteObject, GetDC,
        GetDIBits, ReleaseDC, SelectObject, SetStretchBltMode, StretchBlt, BITMAPINFO,
        BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS, SRCCOPY,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::{GetSystemMetrics, SM_CXSCREEN, SM_CYSCREEN};

    unsafe {
        let screen_w = GetSystemMetrics(SM_CXSCREEN);
        let screen_h = GetSystemMetrics(SM_CYSCREEN);
        if screen_w <= 0 || screen_h <= 0 {
            return None;
        }

        let hdc_screen = GetDC(0);
        if hdc_screen == 0 {
            return None;
        }
        let hdc_mem = CreateCompatibleDC(hdc_screen);
        if hdc_mem == 0 {
            ReleaseDC(0, hdc_screen);
            return None;
        }

        let hbm = CreateCompatibleBitmap(hdc_screen, width, height);
        if hbm == 0 {
            DeleteDC(hdc_mem);
            ReleaseDC(0, hdc_screen);
            return None;
        }

        let old_bm = SelectObject(hdc_mem, hbm);

        const HALFTONE: i32 = 4;
        SetStretchBltMode(hdc_mem, HALFTONE);

        StretchBlt(
            hdc_mem, 0, 0, width, height,
            hdc_screen, 0, 0, screen_w, screen_h,
            SRCCOPY,
        );

        let mut bmi: BITMAPINFO = std::mem::zeroed();
        bmi.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
        bmi.bmiHeader.biWidth = width;
        bmi.bmiHeader.biHeight = -height; // Top-down DIB
        bmi.bmiHeader.biPlanes = 1;
        bmi.bmiHeader.biBitCount = 32;
        bmi.bmiHeader.biCompression = BI_RGB;

        let mut bgra = vec![0u8; (width * height * 4) as usize];
        let res = GetDIBits(
            hdc_mem,
            hbm,
            0,
            height as u32,
            bgra.as_mut_ptr() as *mut _,
            &mut bmi,
            DIB_RGB_COLORS,
        );

        SelectObject(hdc_mem, old_bm);
        DeleteObject(hbm);
        DeleteDC(hdc_mem);
        ReleaseDC(0, hdc_screen);

        if res == 0 {
            return None;
        }

        let mut rgb = vec![0u8; (width * height * 3) as usize];
        for (i, chunk) in bgra.chunks_exact(4).enumerate() {
            rgb[i * 3] = chunk[2];     // R
            rgb[i * 3 + 1] = chunk[1]; // G
            rgb[i * 3 + 2] = chunk[0]; // B
        }

        Some(rgb)
    }
}

#[cfg(not(windows))]
fn capture_screen_rgb(_width: i32, _height: i32) -> Option<Vec<u8>> {
    None
}

fn encode_frame_jpeg(rgb: &[u8], width: u32, height: u32) -> Option<Vec<u8>> {
    let mut jpeg_buf = Vec::with_capacity(30_000);
    let mut encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut jpeg_buf, 65);
    if encoder.encode(rgb, width, height, image::ExtendedColorType::Rgb8).is_ok() {
        Some(jpeg_buf)
    } else {
        None
    }
}

fn handle_client_stream(mut stream: TcpStream, udp_fwd: Arc<UdpSocket>, running: Arc<AtomicBool>) {
    let _ = stream.set_nodelay(true);
    let _ = stream.set_read_timeout(Some(Duration::from_millis(50)));

    let mut stream_read = match stream.try_clone() {
        Ok(s) => s,
        Err(_) => return,
    };

    // Thread de réception des entrées du viewer
    let udp_fwd_clone = udp_fwd.clone();
    let running_reader = running.clone();
    thread::spawn(move || {
        let mut buf = [0u8; 1024];
        while running_reader.load(Ordering::Relaxed) {
            match stream_read.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => {
                    if let Ok(text) = std::str::from_utf8(&buf[..n]) {
                        for line in text.lines() {
                            let msg = line.trim();
                            if msg.starts_with("INPUT:") {
                                let parts: Vec<&str> = msg.split(':').collect();
                                if parts.len() >= 3 {
                                    let keys_csv = parts[2];
                                    let keys: Vec<&str> = keys_csv.split(',').map(|s| s.trim()).filter(|s| !s.is_empty()).collect();

                                    #[cfg(windows)]
                                    for &k in &keys {
                                        if let Some(vk) = map_key_to_vk(k) {
                                            send_vk(vk, false);
                                            send_vk(vk, true);
                                        }
                                    }

                                    let test_msg = format!("KEYS:999999:Invite:{}", keys_csv);
                                    let _ = udp_fwd_clone.send_to(test_msg.as_bytes(), "127.0.0.1:39999");
                                }
                            }
                        }
                    }
                }
                Err(ref e) if e.kind() == std::io::ErrorKind::TimedOut || e.kind() == std::io::ErrorKind::WouldBlock => {
                    thread::sleep(Duration::from_millis(10));
                }
                Err(_) => break,
            }
        }
    });

    // Boucle d'envoi vidéo temps réel (~45-60 FPS)
    let width = 1280i32;
    let height = 720i32;
    let frame_interval = Duration::from_millis(22); // ~45 FPS ultra fluide

    while running.load(Ordering::Relaxed) {
        let start = Instant::now();
        if let Some(rgb) = capture_screen_rgb(width, height) {
            if let Some(jpeg) = encode_frame_jpeg(&rgb, width as u32, height as u32) {
                let mut header = [0u8; 12];
                header[0..4].copy_from_slice(b"CLFR");
                header[4..8].copy_from_slice(&(jpeg.len() as u32).to_be_bytes());
                header[8..10].copy_from_slice(&(width as u16).to_be_bytes());
                header[10..12].copy_from_slice(&(height as u16).to_be_bytes());

                if stream.write_all(&header).is_err() || stream.write_all(&jpeg).is_err() || stream.flush().is_err() {
                    break;
                }
            }
        }

        let elapsed = start.elapsed();
        if elapsed < frame_interval {
            thread::sleep(frame_interval - elapsed);
        }
    }
}

fn main() {
    tracing_subscriber::fmt::init();
    info!("CoopLink Real-Time Screen Capture Service démarré...");

    let running = Arc::new(AtomicBool::new(true));

    let udp_socket = Arc::new(
        UdpSocket::bind("0.0.0.0:47989")
            .or_else(|_| UdpSocket::bind("0.0.0.0:0"))
            .expect("Bind UDP capture"),
    );

    // Écoute des paquets UDP legacy (manettes/pings)
    let udp_clone = udp_socket.clone();
    let running_udp = running.clone();
    thread::spawn(move || {
        let mut buf = [0u8; 2048];
        while running_udp.load(Ordering::Relaxed) {
            if let Ok((len, src)) = udp_clone.recv_from(&mut buf) {
                if let Ok(msg) = std::str::from_utf8(&buf[..len]) {
                    if msg.starts_with("VIEWER:HELLO") {
                        let _ = udp_clone.send_to(b"HOST:ACK:READY", src);
                    } else if msg.starts_with("INPUT:") {
                        let parts: Vec<&str> = msg.split(':').collect();
                        if parts.len() >= 3 {
                            let keys_csv = parts[2];
                            let keys: Vec<&str> = keys_csv.split(',').map(|s| s.trim()).filter(|s| !s.is_empty()).collect();

                            #[cfg(windows)]
                            for &k in &keys {
                                if let Some(vk) = map_key_to_vk(k) {
                                    send_vk(vk, false);
                                    send_vk(vk, true);
                                }
                            }

                            let test_game_msg = format!("KEYS:999999:Invite:{}", keys_csv);
                            let _ = udp_clone.send_to(test_game_msg.as_bytes(), "127.0.0.1:39999");
                        }
                    }
                }
            }
        }
    });

    // Serveur TCP de streaming vidéo en direct (Port 47989)
    let listener = match TcpListener::bind("0.0.0.0:47989") {
        Ok(l) => l,
        Err(e) => {
            error!("Erreur bind TCP 47989: {}", e);
            return;
        }
    };

    info!("Serveur de flux vidéo en direct actif sur TCP 0.0.0.0:47989");

    for stream in listener.incoming() {
        match stream {
            Ok(s) => {
                info!("Client viewer connecté pour le streaming vidéo : {:?}", s.peer_addr());
                let running_client = running.clone();
                let udp_fwd = udp_socket.clone();
                thread::spawn(move || {
                    handle_client_stream(s, udp_fwd, running_client);
                });
            }
            Err(e) => {
                error!("Erreur incoming TCP: {}", e);
            }
        }
    }
}
