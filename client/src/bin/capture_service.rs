// CoopLink Capture & Streaming Service Daemon
// Real-time Screen & Game Capture + Audio Streaming + Input Injection
// Copyright (c) 2026 Loic HUON. All Rights Reserved.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::collections::HashSet;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream, UdpSocket};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};
use parking_lot::Mutex;
use tracing::{error, info, warn};

// Win32 Input Injection (Clavier & Souris)
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

#[cfg(windows)]
fn send_mouse_input(dx: i32, dy: i32, left_down: bool, right_down: bool, prev_l: bool, prev_r: bool) {
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
        mouse_event, MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP, MOUSEEVENTF_MOVE,
        MOUSEEVENTF_RIGHTDOWN, MOUSEEVENTF_RIGHTUP,
    };
    unsafe {
        let mut flags = 0u32;
        if dx != 0 || dy != 0 {
            flags |= MOUSEEVENTF_MOVE;
        }
        if left_down && !prev_l {
            flags |= MOUSEEVENTF_LEFTDOWN;
        } else if !left_down && prev_l {
            flags |= MOUSEEVENTF_LEFTUP;
        }
        if right_down && !prev_r {
            flags |= MOUSEEVENTF_RIGHTDOWN;
        } else if !right_down && prev_r {
            flags |= MOUSEEVENTF_RIGHTUP;
        }
        if flags != 0 {
            mouse_event(flags, dx, dy, 0, 0);
        }
    }
}

fn map_key_to_vk(k: &str) -> Option<u16> {
    match k.trim().to_uppercase().as_str() {
        "Z" | "W" => Some(0x5A), // VK_Z
        "Q" | "A" => Some(0x51), // VK_Q
        "S" => Some(0x53),       // VK_S
        "D" => Some(0x44),       // VK_D
        "E" => Some(0x45),       // VK_E (Interact)
        "F" => Some(0x46),       // VK_F (Action Cyberpunk)
        "R" => Some(0x52),       // VK_R (Recharger)
        "C" => Some(0x43),       // VK_C (Accroupi)
        "X" => Some(0x58),       // VK_X
        "V" => Some(0x56),       // VK_V
        "SHIFT" => Some(0x10),   // VK_SHIFT (Sprint)
        "CTRL" => Some(0x11),    // VK_CONTROL (S'accroupir)
        "TAB" => Some(0x09),     // VK_TAB (Scanner Cyberpunk)
        "ECHAP" | "ESC" => Some(0x1B), // VK_ESCAPE
        "1" => Some(0x31),       // Arme 1
        "2" => Some(0x32),       // Arme 2
        "3" => Some(0x33),       // Arme 3
        "4" => Some(0x34),       // Arme 4
        "HAUT" | "UP" => Some(0x26),    // VK_UP
        "BAS" | "DOWN" => Some(0x28),   // VK_DOWN
        "GAUCHE" | "LEFT" => Some(0x25),// VK_LEFT
        "DROITE" | "RIGHT" => Some(0x27), // VK_RIGHT
        "ESPACE" | "SPACE" => Some(0x20), // VK_SPACE
        "ENTREE" | "ENTER" => Some(0x0D), // VK_RETURN
        _ => None,
    }
}

// Gestionnaire d'état des entrées utilisateur pour maintenir les touches enfoncées (ZQSD)
struct InputState {
    held_keys: HashSet<u16>,
    mouse_l: bool,
    mouse_r: bool,
    last_packet: Instant,
}

impl InputState {
    fn new() -> Self {
        Self {
            held_keys: HashSet::new(),
            mouse_l: false,
            mouse_r: false,
            last_packet: Instant::now(),
        }
    }

    #[cfg(windows)]
    fn update_keys(&mut self, new_keys: &[u16]) {
        self.last_packet = Instant::now();
        let new_set: HashSet<u16> = new_keys.iter().copied().collect();

        // Enfoncer les nouvelles touches
        for &vk in &new_set {
            if !self.held_keys.contains(&vk) {
                send_vk(vk, false); // KEY DOWN
            }
        }

        // Relâcher les touches qui ne sont plus actives
        for &vk in &self.held_keys {
            if !new_set.contains(&vk) {
                send_vk(vk, true); // KEY UP
            }
        }

        self.held_keys = new_set;
    }

    #[cfg(not(windows))]
    fn update_keys(&mut self, _new_keys: &[u16]) {}

    #[cfg(windows)]
    fn release_all(&mut self) {
        for &vk in &self.held_keys {
            send_vk(vk, true);
        }
        self.held_keys.clear();
        if self.mouse_l {
            send_mouse_input(0, 0, false, self.mouse_r, true, self.mouse_r);
            self.mouse_l = false;
        }
        if self.mouse_r {
            send_mouse_input(0, 0, self.mouse_l, false, self.mouse_l, true);
            self.mouse_r = false;
        }
    }

    #[cfg(not(windows))]
    fn release_all(&mut self) {}
}

// Recherche et focalisation automatique de la fenêtre du jeu cible
#[cfg(windows)]
fn find_and_focus_window(target_hint: &str) -> (i32, i32, i32, i32) {
    use windows_sys::Win32::Foundation::{BOOL, HWND, LPARAM, RECT};
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        EnumWindows, GetForegroundWindow, GetSystemMetrics, GetWindowRect, GetWindowTextW,
        IsWindowVisible, SetForegroundWindow, ShowWindow, SM_CXSCREEN, SM_CYSCREEN, SW_RESTORE,
    };

    let screen_w = unsafe { GetSystemMetrics(SM_CXSCREEN) };
    let screen_h = unsafe { GetSystemMetrics(SM_CYSCREEN) };
    let default_rect = (0, 0, screen_w, screen_h);

    if target_hint.is_empty() || target_hint.starts_with("CoopLink") {
        return default_rect;
    }

    struct SearchCtx {
        target: String,
        found_hwnd: HWND,
    }

    unsafe extern "system" fn enum_proc(hwnd: HWND, lparam: LPARAM) -> BOOL {
        let ctx = &mut *(lparam as *mut SearchCtx);
        if IsWindowVisible(hwnd) == 0 {
            return 1;
        }
        let mut title_buf = [0u16; 512];
        let len = GetWindowTextW(hwnd, title_buf.as_mut_ptr(), 512);
        if len > 0 {
            let title = String::from_utf16_lossy(&title_buf[..len as usize]);
            let title_lower = title.to_lowercase();
            let target_lower = ctx.target.to_lowercase();

            // Ignore les utilitaires système et fenêtres invisibles
            if !title_lower.contains("cooplink")
                && (title_lower.contains(&target_lower)
                    || (target_lower.contains("cyberpunk") && title_lower.contains("cyberpunk")))
            {
                ctx.found_hwnd = hwnd;
                return 0; // Trouvé, arrêt de l'énumération
            }
        }
        1
    }

    let clean_target = if target_hint.contains(" : ") {
        target_hint.split(" : ").nth(1).unwrap_or(target_hint)
    } else {
        target_hint
    };

    let mut ctx = SearchCtx {
        target: clean_target.to_string(),
        found_hwnd: 0,
    };

    unsafe {
        EnumWindows(Some(enum_proc), &mut ctx as *mut _ as LPARAM);
        let target_hwnd = if ctx.found_hwnd != 0 {
            ctx.found_hwnd
        } else {
            GetForegroundWindow()
        };

        if target_hwnd != 0 {
            ShowWindow(target_hwnd, SW_RESTORE);
            SetForegroundWindow(target_hwnd);

            let mut r: RECT = std::mem::zeroed();
            if GetWindowRect(target_hwnd, &mut r) != 0 {
                let w = r.right - r.left;
                let h = r.bottom - r.top;
                if w > 200 && h > 200 {
                    return (r.left.max(0), r.top.max(0), w, h);
                }
            }
        }
    }

    default_rect
}

#[cfg(not(windows))]
fn find_and_focus_window(_target_hint: &str) -> (i32, i32, i32, i32) {
    (0, 0, 1920, 1080)
}

#[cfg(windows)]
fn capture_screen_rgb(crop_x: i32, crop_y: i32, crop_w: i32, crop_h: i32, out_w: i32, out_h: i32) -> Option<Vec<u8>> {
    use windows_sys::Win32::Graphics::Gdi::{
        CreateCompatibleBitmap, CreateCompatibleDC, DeleteDC, DeleteObject, GetDC,
        GetDIBits, ReleaseDC, SelectObject, SetStretchBltMode, StretchBlt, BITMAPINFO,
        BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS, SRCCOPY,
    };

    unsafe {
        let hdc_screen = GetDC(0);
        if hdc_screen == 0 {
            return None;
        }
        let hdc_mem = CreateCompatibleDC(hdc_screen);
        if hdc_mem == 0 {
            ReleaseDC(0, hdc_screen);
            return None;
        }

        let hbm = CreateCompatibleBitmap(hdc_screen, out_w, out_h);
        if hbm == 0 {
            DeleteDC(hdc_mem);
            ReleaseDC(0, hdc_screen);
            return None;
        }

        let old_bm = SelectObject(hdc_mem, hbm);

        // COLORONCOLOR (3) est jusqu'à 8x plus rapide que HALFTONE (4), garantissant 60 FPS
        const COLORONCOLOR: i32 = 3;
        SetStretchBltMode(hdc_mem, COLORONCOLOR);

        StretchBlt(
            hdc_mem, 0, 0, out_w, out_h,
            hdc_screen, crop_x, crop_y, crop_w, crop_h,
            SRCCOPY,
        );

        let mut bmi: BITMAPINFO = std::mem::zeroed();
        bmi.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
        bmi.bmiHeader.biWidth = out_w;
        bmi.bmiHeader.biHeight = -out_h; // Top-down DIB
        bmi.bmiHeader.biPlanes = 1;
        bmi.bmiHeader.biBitCount = 32;
        bmi.bmiHeader.biCompression = BI_RGB;

        let mut bgra = vec![0u8; (out_w * out_h * 4) as usize];
        let res = GetDIBits(
            hdc_mem,
            hbm,
            0,
            out_h as u32,
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

        let mut rgb = vec![0u8; (out_w * out_h * 3) as usize];
        for (i, chunk) in bgra.chunks_exact(4).enumerate() {
            rgb[i * 3] = chunk[2];     // R
            rgb[i * 3 + 1] = chunk[1]; // G
            rgb[i * 3 + 2] = chunk[0]; // B
        }

        Some(rgb)
    }
}

#[cfg(not(windows))]
fn capture_screen_rgb(_x: i32, _y: i32, _cw: i32, _ch: i32, _w: i32, _h: i32) -> Option<Vec<u8>> {
    None
}

// Encodage JPEG haute fidélité (Qualité 82 pour une netteté cristalline et 0 pixelisation)
fn encode_frame_jpeg(rgb: &[u8], width: u32, height: u32) -> Option<Vec<u8>> {
    let mut jpeg_buf = Vec::with_capacity(60_000);
    let mut encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut jpeg_buf, 82);
    if encoder.encode(rgb, width, height, image::ExtendedColorType::Rgb8).is_ok() {
        Some(jpeg_buf)
    } else {
        None
    }
}

// Démarrage de la capture audio système WASAPI Loopback via cpal
fn start_audio_capture_thread(
    audio_queue: Arc<Mutex<Vec<u8>>>,
    running: Arc<AtomicBool>,
) {
    thread::spawn(move || {
        use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

        let host = cpal::default_host();
        let device = match host.default_output_device() {
            Some(d) => d,
            None => {
                warn!("Aucun périphérique audio de sortie trouvé pour la capture loopback.");
                return;
            }
        };

        // Sur Windows WASAPI, créer un input stream sur le default_output_device effectue la capture loopback du son joué
        let supported_configs = match device.supported_input_configs() {
            Ok(configs) => configs,
            Err(e) => {
                warn!("Impossible d'énumérer les configurations audio loopback : {}", e);
                return;
            }
        };

        let config = match supported_configs.into_iter().next() {
            Some(c) => c.with_max_sample_rate().config(),
            None => {
                warn!("Aucune configuration audio loopback supportée.");
                return;
            }
        };

        let sample_rate = config.sample_rate;
        let channels = config.channels;
        info!("Capture audio système loopback active : {} Hz, {} canaux", sample_rate, channels);

        let q = audio_queue.clone();
        let stream = device.build_input_stream(
            config,
            move |data: &[f32], _| {
                // Conversion f32 -> i16 PCM
                let mut pcm = Vec::with_capacity(data.len() * 2);
                for &s in data {
                    let clamped = s.clamp(-1.0, 1.0);
                    let val = (clamped * 32767.0) as i16;
                    pcm.extend_from_slice(&val.to_le_bytes());
                }
                let mut lock = q.lock();
                if lock.len() < 48_000 {
                    lock.extend_from_slice(&pcm);
                }
            },
            |err| {
                warn!("Erreur flux audio loopback : {}", err);
            },
            None,
        );

        if let Ok(st) = stream {
            let _ = st.play();
            while running.load(Ordering::Relaxed) {
                thread::sleep(Duration::from_millis(50));
            }
        }
    });
}

fn handle_client_stream(
    mut stream: TcpStream,
    target_hint: String,
    audio_queue: Arc<Mutex<Vec<u8>>>,
    udp_fwd: Arc<UdpSocket>,
    running: Arc<AtomicBool>,
) {
    let _ = stream.set_nodelay(true);
    let _ = stream.set_read_timeout(Some(Duration::from_millis(50)));

    let mut stream_read = match stream.try_clone() {
        Ok(s) => s,
        Err(_) => return,
    };

    let input_state = Arc::new(Mutex::new(InputState::new()));

    // Thread de réception des entrées (Clavier ZQSD continu + Souris 3D / Tirs)
    let input_clone = input_state.clone();
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
                                    let vks: Vec<u16> = keys_csv
                                        .split(',')
                                        .map(|s| s.trim())
                                        .filter_map(map_key_to_vk)
                                        .collect();

                                    input_clone.lock().update_keys(&vks);

                                    let test_msg = format!("KEYS:999999:Invite:{}", keys_csv);
                                    let _ = udp_fwd_clone.send_to(test_msg.as_bytes(), "127.0.0.1:39999");
                                }
                            } else if msg.starts_with("MOUSE:") {
                                // MOUSE:dx:dy:left_down:right_down
                                let parts: Vec<&str> = msg.split(':').collect();
                                if parts.len() >= 5 {
                                    let dx: i32 = parts[1].parse().unwrap_or(0);
                                    let dy: i32 = parts[2].parse().unwrap_or(0);
                                    let left: bool = parts[3] == "1";
                                    let right: bool = parts[4] == "1";

                                    #[cfg(windows)]
                                    {
                                        let mut st = input_clone.lock();
                                        send_mouse_input(dx, dy, left, right, st.mouse_l, st.mouse_r);
                                        st.mouse_l = left;
                                        st.mouse_r = right;
                                    }
                                }
                            }
                        }
                    }
                }
                Err(ref e) if e.kind() == std::io::ErrorKind::TimedOut || e.kind() == std::io::ErrorKind::WouldBlock => {
                    // Watchdog de sécurité : relâche les touches si inactif depuis 400ms
                    let mut st = input_clone.lock();
                    if !st.held_keys.is_empty() && st.last_packet.elapsed() > Duration::from_millis(400) {
                        st.release_all();
                    }
                    thread::sleep(Duration::from_millis(5));
                }
                Err(_) => break,
            }
        }
        input_clone.lock().release_all();
    });

    // Détermination de la zone de capture (Jeu cible vs Plein écran)
    let (mut crop_x, mut crop_y, mut crop_w, mut crop_h) = find_and_focus_window(&target_hint);
    info!(
        "Zone de capture initialisée pour '{}' : pos=({}, {}), taille={}x{}",
        target_hint, crop_x, crop_y, crop_w, crop_h
    );

    // Résolution de diffusion haute qualité : 1920x1080 (ou taille native du jeu)
    let stream_w = 1920i32;
    let stream_h = 1080i32;
    let frame_interval = Duration::from_millis(16); // 60 FPS ultra fluide

    let mut last_window_check = Instant::now();

    while running.load(Ordering::Relaxed) {
        let start = Instant::now();

        // Réajuste la position de la fenêtre toutes les 2 secondes
        if last_window_check.elapsed() > Duration::from_secs(2) {
            let (nx, ny, nw, nh) = find_and_focus_window(&target_hint);
            crop_x = nx;
            crop_y = ny;
            crop_w = nw;
            crop_h = nh;
            last_window_check = Instant::now();
        }

        // 1. Transmission de l'audio si présent dans la file (b"CLAU")
        {
            let mut audio_data = Vec::new();
            {
                let mut q = audio_queue.lock();
                if !q.is_empty() {
                    let take_len = q.len().min(4096);
                    audio_data = q.drain(..take_len).collect();
                }
            }
            if !audio_data.is_empty() {
                let mut audio_hdr = [0u8; 12];
                audio_hdr[0..4].copy_from_slice(b"CLAU");
                audio_hdr[4..8].copy_from_slice(&(audio_data.len() as u32).to_be_bytes());
                audio_hdr[8..10].copy_from_slice(&(48000u16).to_be_bytes()); // 48kHz
                audio_hdr[10] = 2; // Stéréo
                audio_hdr[11] = 16; // 16 bits PCM

                if stream.write_all(&audio_hdr).is_err() || stream.write_all(&audio_data).is_err() {
                    break;
                }
            }
        }

        // 2. Capture et Envoi de la trame vidéo (b"CLFR")
        if let Some(rgb) = capture_screen_rgb(crop_x, crop_y, crop_w, crop_h, stream_w, stream_h) {
            if let Some(jpeg) = encode_frame_jpeg(&rgb, stream_w as u32, stream_h as u32) {
                let mut header = [0u8; 12];
                header[0..4].copy_from_slice(b"CLFR");
                header[4..8].copy_from_slice(&(jpeg.len() as u32).to_be_bytes());
                header[8..10].copy_from_slice(&(stream_w as u16).to_be_bytes());
                header[10..12].copy_from_slice(&(stream_h as u16).to_be_bytes());

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

    input_state.lock().release_all();
}

fn main() {
    tracing_subscriber::fmt::init();
    info!("CoopLink Real-Time Screen Capture & Audio Service démarré...");

    let args: Vec<String> = std::env::args().collect();
    let target_hint = if args.len() > 2 {
        args[2].clone()
    } else if args.len() > 1 && !args[1].ends_with(".conf") {
        args[1].clone()
    } else {
        String::new()
    };

    let running = Arc::new(AtomicBool::new(true));
    let audio_queue = Arc::new(Mutex::new(Vec::new()));

    // Démarrage du thread de capture audio système
    start_audio_capture_thread(audio_queue.clone(), running.clone());

    let udp_socket = Arc::new(
        UdpSocket::bind("0.0.0.0:47989")
            .or_else(|_| UdpSocket::bind("0.0.0.0:0"))
            .expect("Bind UDP capture"),
    );

    // Écoute des paquets UDP legacy
    let udp_clone = udp_socket.clone();
    let running_udp = running.clone();
    thread::spawn(move || {
        let mut buf = [0u8; 2048];
        while running_udp.load(Ordering::Relaxed) {
            if let Ok((len, src)) = udp_clone.recv_from(&mut buf) {
                if let Ok(msg) = std::str::from_utf8(&buf[..len]) {
                    if msg.starts_with("VIEWER:HELLO") {
                        let _ = udp_clone.send_to(b"HOST:ACK:READY", src);
                    }
                }
            }
        }
    });

    // Serveur TCP de streaming vidéo et audio haute performance (Port 47989)
    let listener = match TcpListener::bind("0.0.0.0:47989") {
        Ok(l) => l,
        Err(e) => {
            error!("Erreur bind TCP 47989: {}", e);
            return;
        }
    };

    info!("Serveur de flux direct (1080p 60 FPS + Audio) actif sur TCP 0.0.0.0:47989");

    for stream in listener.incoming() {
        match stream {
            Ok(s) => {
                info!("Client viewer connecté pour le streaming : {:?}", s.peer_addr());
                let running_client = running.clone();
                let audio_q = audio_queue.clone();
                let udp_fwd = udp_socket.clone();
                let hint = target_hint.clone();
                thread::spawn(move || {
                    handle_client_stream(s, hint, audio_q, udp_fwd, running_client);
                });
            }
            Err(e) => {
                error!("Erreur incoming TCP: {}", e);
            }
        }
    }
}
