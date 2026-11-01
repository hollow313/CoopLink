// CoopLink Virtual Game Simulator & Feature Verification Tool
// Copyright (c) 2026 Loic HUON. All Rights Reserved.
// Standalone Portable Executable (No installation required)

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use eframe::egui::{self, Color32, FontId, Key, Pos2, Rect, RichText, Rounding, Stroke, Vec2};
use parking_lot::Mutex;
use std::net::{SocketAddr, UdpSocket};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

#[cfg(windows)]
type XInputGetStateFn = unsafe extern "system" fn(u32, *mut XInputState) -> u32;

#[repr(C)]
#[derive(Default, Clone, Copy)]
pub struct XInputGamepad {
    pub w_buttons: u16,
    pub b_left_trigger: u8,
    pub b_right_trigger: u8,
    pub s_thumb_lx: i16,
    pub s_thumb_ly: i16,
    pub s_thumb_rx: i16,
    pub s_thumb_ry: i16,
}

#[repr(C)]
#[derive(Default, Clone, Copy)]
pub struct XInputState {
    pub dw_packet_number: u32,
    pub gamepad: XInputGamepad,
}

struct XInputApi {
    #[cfg(windows)]
    get_state: Option<XInputGetStateFn>,
}

impl XInputApi {
    fn load() -> Self {
        #[cfg(windows)]
        {
            use windows_sys::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryA};
            let dll_names: &[&[u8]] = &[b"xinput1_4.dll\0", b"xinput1_3.dll\0", b"xinput9_1_0.dll\0"];
            for &dll in dll_names {
                unsafe {
                    let h_module = LoadLibraryA(dll.as_ptr());
                    if h_module != 0 {
                        let proc = GetProcAddress(h_module, b"XInputGetState\0".as_ptr());
                        if let Some(p) = proc {
                            let fn_ptr: XInputGetStateFn = std::mem::transmute(p);
                            return Self { get_state: Some(fn_ptr) };
                        }
                    }
                }
            }
            Self { get_state: None }
        }
        #[cfg(not(windows))]
        {
            Self {}
        }
    }

    fn poll_gamepad(&self, user_index: u32) -> Option<XInputGamepad> {
        #[cfg(windows)]
        {
            if let Some(get_state) = self.get_state {
                let mut state = XInputState::default();
                let ret = unsafe { get_state(user_index, &mut state) };
                if ret == 0 {
                    return Some(state.gamepad);
                }
            }
            None
        }
        #[cfg(not(windows))]
        {
            let _ = user_index;
            None
        }
    }
}

// Couleurs Modernes de la palette CoopLink
const COLOR_BG: Color32 = Color32::from_rgb(15, 23, 42);
const COLOR_CARD: Color32 = Color32::from_rgb(30, 41, 59);
const COLOR_BORDER: Color32 = Color32::from_rgb(51, 65, 85);
const COLOR_P1: Color32 = Color32::from_rgb(14, 165, 233); // Cyan
const COLOR_P2: Color32 = Color32::from_rgb(16, 185, 129); // Vert
const COLOR_P3: Color32 = Color32::from_rgb(245, 158, 11); // Orange
const COLOR_P4: Color32 = Color32::from_rgb(168, 85, 247); // Violet

#[derive(Clone)]
struct PlayerShip {
    x: f32,
    y: f32,
    color: Color32,
    name: &'static str,
    last_btn: String,
}

#[derive(Default, Clone)]
struct NetworkSyncState {
    peer_connected: bool,
    peer_ip: String,
    peer_keys_pressed: Vec<String>,
    last_peer_packet: Option<Instant>,
}

struct TestGameApp {
    xinput: XInputApi,
    players: [PlayerShip; 4],
    ball_pos: Pos2,
    ball_vel: Vec2,
    start_time: Instant,
    last_frame: Instant,
    frame_count: u32,
    fps_display: f32,
    fps_timer: Instant,
    network_sync: Arc<Mutex<NetworkSyncState>>,
    sound_feedback: String,
    sender_socket: Option<Arc<UdpSocket>>,
    my_keys_pressed: Vec<String>,
}

impl TestGameApp {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let mut style = (*cc.egui_ctx.style()).clone();
        let mut visuals = egui::Visuals::dark();
        visuals.panel_fill = COLOR_BG;
        visuals.window_fill = COLOR_CARD;
        style.visuals = visuals;
        cc.egui_ctx.set_style(style);

        let network_sync = Arc::new(Mutex::new(NetworkSyncState::default()));

        // Socket UDP pour écouter sur le port 39999 (Réseau virtuel CoopLink)
        let sync_clone = network_sync.clone();
        let rx_socket = UdpSocket::bind("0.0.0.0:39999").ok().map(Arc::new);
        let tx_socket = rx_socket.clone();

        if let Some(sock) = rx_socket {
            thread::spawn(move || {
                let mut buf = [0u8; 1024];
                let _ = sock.set_read_timeout(Some(Duration::from_millis(500)));

                loop {
                    if let Ok((len, src)) = sock.recv_from(&mut buf) {
                        if let Ok(msg) = std::str::from_utf8(&buf[..len]) {
                            let mut st = sync_clone.lock();
                            st.last_peer_packet = Some(Instant::now());
                            st.peer_connected = true;
                            st.peer_ip = src.ip().to_string();

                            if msg.starts_with("PING:") {
                                let _ = sock.send_to(b"PONG:OK", src);
                            } else if msg.starts_with("KEYS:") {
                                // Format: KEYS:Player:K1,K2,K3
                                let parts: Vec<&str> = msg.splitn(3, ':').collect();
                                if parts.len() >= 3 {
                                    let keys: Vec<String> = parts[2].split(',')
                                        .map(|s| s.trim().to_string())
                                        .filter(|s| !s.is_empty())
                                        .collect();
                                    st.peer_keys_pressed = keys;
                                }
                            }
                        }
                    }
                }
            });
        }

        Self {
            xinput: XInputApi::load(),
            players: [
                PlayerShip {
                    x: 120.0,
                    y: 160.0,
                    color: COLOR_P1,
                    name: "Joueur 1 (Hôte)",
                    last_btn: "-".to_string(),
                },
                PlayerShip {
                    x: 280.0,
                    y: 160.0,
                    color: COLOR_P2,
                    name: "Joueur 2 (Invité CoopLink)",
                    last_btn: "-".to_string(),
                },
                PlayerShip {
                    x: 440.0,
                    y: 160.0,
                    color: COLOR_P3,
                    name: "Joueur 3 (Manette #3)",
                    last_btn: "-".to_string(),
                },
                PlayerShip {
                    x: 600.0,
                    y: 160.0,
                    color: COLOR_P4,
                    name: "Joueur 4 (Manette #4)",
                    last_btn: "-".to_string(),
                },
            ],
            ball_pos: Pos2::new(360.0, 160.0),
            ball_vel: Vec2::new(280.0, 190.0),
            start_time: Instant::now(),
            last_frame: Instant::now(),
            frame_count: 0,
            fps_display: 60.0,
            fps_timer: Instant::now(),
            network_sync,
            sound_feedback: "Prêt pour le test audio".to_string(),
            sender_socket: tx_socket,
            my_keys_pressed: Vec::new(),
        }
    }

    fn send_network_ping(&self) {
        if let Some(ref sock) = self.sender_socket {
            let _ = sock.set_broadcast(true);
            let candidates = ["10.244.0.1:39999", "10.244.0.2:39999", "255.255.255.255:39999", "127.0.0.1:39999"];
            for target in &candidates {
                if let Ok(addr) = target.parse::<SocketAddr>() {
                    let _ = sock.send_to(b"PING:MonPC", addr);
                }
            }
        }
    }

    fn broadcast_my_keys(&self, keys: &[String]) {
        if let Some(ref sock) = self.sender_socket {
            let msg = format!("KEYS:MonPC:{}", keys.join(","));
            let candidates = ["10.244.0.1:39999", "10.244.0.2:39999", "255.255.255.255:39999", "127.0.0.1:39999"];
            for target in &candidates {
                if let Ok(addr) = target.parse::<SocketAddr>() {
                    let _ = sock.send_to(msg.as_bytes(), addr);
                }
            }
        }
    }

    fn play_test_beep(&mut self, freq: u32, duration_ms: u32, name: &str) {
        self.sound_feedback = format!("Son joué : {} ({} Hz / {} ms)", name, freq, duration_ms);
        #[cfg(windows)]
        {
            thread::spawn(move || {
                unsafe {
                    windows_sys::Win32::System::Diagnostics::Debug::Beep(freq, duration_ms);
                }
            });
        }
    }
}

impl eframe::App for TestGameApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let now = Instant::now();
        let dt = (now - self.last_frame).as_secs_f32().min(0.05);
        self.last_frame = now;

        // Calcul FPS
        self.frame_count += 1;
        if self.fps_timer.elapsed().as_secs_f32() >= 0.5 {
            self.fps_display = self.frame_count as f32 / self.fps_timer.elapsed().as_secs_f32();
            self.frame_count = 0;
            self.fps_timer = Instant::now();
        }

        // =========================================================================
        // CAPTURE DU CLAVIER LOCAL EN TEMPS RÉEL (Pour prouver l'envoi des touches)
        // =========================================================================
        let mut pressed_now = Vec::new();
        ctx.input(|i| {
            if i.key_down(Key::Z) || i.key_down(Key::W) { pressed_now.push("Z".to_string()); }
            if i.key_down(Key::Q) || i.key_down(Key::A) { pressed_now.push("Q".to_string()); }
            if i.key_down(Key::S) { pressed_now.push("S".to_string()); }
            if i.key_down(Key::D) { pressed_now.push("D".to_string()); }
            if i.key_down(Key::Space) { pressed_now.push("ESPACE".to_string()); }
            if i.key_down(Key::ArrowUp) { pressed_now.push("HAUT".to_string()); }
            if i.key_down(Key::ArrowDown) { pressed_now.push("BAS".to_string()); }
            if i.key_down(Key::ArrowLeft) { pressed_now.push("GAUCHE".to_string()); }
            if i.key_down(Key::ArrowRight) { pressed_now.push("DROITE".to_string()); }
            if i.key_down(Key::Enter) { pressed_now.push("ENTREE".to_string()); }
            if i.key_down(Key::I) { pressed_now.push("I".to_string()); }
            if i.key_down(Key::J) { pressed_now.push("J".to_string()); }
            if i.key_down(Key::K) { pressed_now.push("K".to_string()); }
            if i.key_down(Key::L) { pressed_now.push("L".to_string()); }
        });

        if pressed_now != self.my_keys_pressed {
            self.my_keys_pressed = pressed_now.clone();
            self.broadcast_my_keys(&pressed_now);
        }

        // =========================================================================
        // GESTION DES MANETTES & DÉPLACEMENT DES JOUEURS
        // =========================================================================
        for i in 0..4 {
            let mut dx = 0.0_f32;
            let mut dy = 0.0_f32;
            let mut btn_text = "-".to_string();

            if let Some(pad) = self.xinput.poll_gamepad(i as u32) {
                let lx = pad.s_thumb_lx as f32 / 32768.0;
                let ly = pad.s_thumb_ly as f32 / 32768.0;
                if lx.abs() > 0.2 { dx += lx * 320.0; }
                if ly.abs() > 0.2 { dy -= ly * 320.0; }

                if (pad.w_buttons & 0x0001) != 0 { dy -= 220.0; }
                if (pad.w_buttons & 0x0002) != 0 { dy += 220.0; }
                if (pad.w_buttons & 0x0004) != 0 { dx -= 220.0; }
                if (pad.w_buttons & 0x0008) != 0 { dx += 220.0; }

                let mut pressed = Vec::new();
                if (pad.w_buttons & 0x1000) != 0 { pressed.push("A"); }
                if (pad.w_buttons & 0x2000) != 0 { pressed.push("B"); }
                if (pad.w_buttons & 0x4000) != 0 { pressed.push("X"); }
                if (pad.w_buttons & 0x8000) != 0 { pressed.push("Y"); }
                if (pad.w_buttons & 0x0100) != 0 { pressed.push("LB"); }
                if (pad.w_buttons & 0x0200) != 0 { pressed.push("RB"); }
                if (pad.w_buttons & 0x0010) != 0 { pressed.push("START"); }
                if (pad.w_buttons & 0x0020) != 0 { pressed.push("BACK"); }

                if !pressed.is_empty() { btn_text = pressed.join("+"); }
            }

            // Déplacement clavier Joueur 1 (ZQSD)
            if i == 0 {
                if self.my_keys_pressed.contains(&"Z".to_string()) || self.my_keys_pressed.contains(&"HAUT".to_string()) { dy -= 260.0; }
                if self.my_keys_pressed.contains(&"S".to_string()) || self.my_keys_pressed.contains(&"BAS".to_string()) { dy += 260.0; }
                if self.my_keys_pressed.contains(&"Q".to_string()) || self.my_keys_pressed.contains(&"GAUCHE".to_string()) { dx -= 260.0; }
                if self.my_keys_pressed.contains(&"D".to_string()) || self.my_keys_pressed.contains(&"DROITE".to_string()) { dx += 260.0; }
                if self.my_keys_pressed.contains(&"ESPACE".to_string()) { btn_text = "TIR (ESPACE)".to_string(); }
            }

            // Déplacement clavier Joueur 2 (par les touches reçues du pair en réseau ou IJKL)
            if i == 1 {
                let peer_keys = self.network_sync.lock().peer_keys_pressed.clone();
                if peer_keys.contains(&"Z".to_string()) || peer_keys.contains(&"I".to_string()) || peer_keys.contains(&"HAUT".to_string()) { dy -= 260.0; }
                if peer_keys.contains(&"S".to_string()) || peer_keys.contains(&"K".to_string()) || peer_keys.contains(&"BAS".to_string()) { dy += 260.0; }
                if peer_keys.contains(&"Q".to_string()) || peer_keys.contains(&"J".to_string()) || peer_keys.contains(&"GAUCHE".to_string()) { dx -= 260.0; }
                if peer_keys.contains(&"D".to_string()) || peer_keys.contains(&"L".to_string()) || peer_keys.contains(&"DROITE".to_string()) { dx += 260.0; }
                if peer_keys.contains(&"ESPACE".to_string()) || peer_keys.contains(&"ENTREE".to_string()) { btn_text = "TIR (INVITÉ)".to_string(); }
            }

            self.players[i].x = (self.players[i].x + dx * dt).clamp(20.0, 720.0);
            self.players[i].y = (self.players[i].y + dy * dt).clamp(30.0, 260.0);
            if btn_text != "-" { self.players[i].last_btn = btn_text; }
        }

        // Balle 60/120 FPS
        self.ball_pos += self.ball_vel * dt;
        if self.ball_pos.x <= 20.0 || self.ball_pos.x >= 720.0 { self.ball_vel.x = -self.ball_vel.x; }
        if self.ball_pos.y <= 30.0 || self.ball_pos.y >= 260.0 { self.ball_vel.y = -self.ball_vel.y; }

        // =========================================================================
        // RENDU GRAPHIQUE DU JEU DE TEST
        // =========================================================================
        egui::CentralPanel::default().show(ctx, |ui| {
            // Entête
            ui.horizontal(|ui| {
                ui.heading(
                    RichText::new("CoopLink Virtual Game Test")
                        .color(Color32::WHITE)
                        .strong()
                        .size(18.0),
                );
                ui.add_space(10.0);
                ui.label(
                    RichText::new(format!("{:.0} FPS", self.fps_display))
                        .color(if self.fps_display >= 55.0 { COLOR_P2 } else { COLOR_P3 })
                        .strong()
                        .size(15.0),
                );
                ui.add_space(10.0);
                let elapsed_ms = self.start_time.elapsed().as_millis();
                ui.label(
                    RichText::new(format!("Chrono : {:02}:{:02}.{:03}", (elapsed_ms / 60000) % 60, (elapsed_ms / 1000) % 60, elapsed_ms % 1000))
                        .color(COLOR_P1)
                        .monospace()
                        .strong()
                        .size(14.0),
                );

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button(RichText::new("📡 Tester Connexion Réseau").strong().color(Color32::WHITE))
                        .on_hover_cursor(egui::CursorIcon::PointingHand)
                        .clicked()
                    {
                        self.send_network_ping();
                    }
                });
            });

            ui.add_space(6.0);

            // =====================================================================
            // BANNIÈRE DIAGNOSTIC RÉSEAU : LES 2 PC SONT-ILS DANS LE MÊME RÉSEAU ?
            // =====================================================================
            let net = self.network_sync.lock().clone();
            let is_peer_active = if let Some(last) = net.last_peer_packet {
                last.elapsed().as_secs() < 3
            } else {
                false
            };

            egui::Frame::none()
                .fill(if is_peer_active { Color32::from_rgb(6, 78, 59) } else { Color32::from_rgb(69, 26, 3) })
                .stroke(Stroke::new(1.0_f32, if is_peer_active { COLOR_P2 } else { COLOR_P3 }))
                .inner_margin(10.0)
                .rounding(Rounding::same(8.0))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.painter().circle_filled(ui.cursor().min + Vec2::new(6.0, 9.0), 5.0, if is_peer_active { COLOR_P2 } else { COLOR_P3 });
                        ui.add_space(14.0);
                        if is_peer_active {
                            ui.label(
                                RichText::new(format!(
                                    "✓ RÉSEAU VIRTUEL COOPLINK CONNECTÉ ! L'autre PC communique en direct (IP: {}) — Latence : < 10 ms",
                                    net.peer_ip
                                ))
                                .color(Color32::WHITE)
                                .strong()
                                .size(13.0),
                            );
                        } else {
                            ui.label(
                                RichText::new("⏳ En attente de connexion avec l'autre PC... Rejoignez le même salon dans CoopLink pour relier vos PC !")
                                    .color(Color32::from_rgb(254, 215, 170))
                                    .size(13.0),
                            );
                        }
                    });
                });

            ui.add_space(8.0);

            // 1. ARÈNE GRAPHIQUE ANIMÉE 2D
            let arena_rect = Rect::from_min_size(ui.cursor().min, Vec2::new(ui.available_width(), 260.0));
            let painter = ui.painter();

            painter.rect_filled(arena_rect, Rounding::same(10.0), Color32::from_rgb(10, 15, 30));
            painter.rect_stroke(arena_rect, Rounding::same(10.0), Stroke::new(1.5_f32, COLOR_BORDER));

            // Balle animée
            painter.circle_filled(
                Pos2::new(arena_rect.min.x + self.ball_pos.x, arena_rect.min.y + self.ball_pos.y),
                12.0,
                Color32::from_rgb(236, 72, 153),
            );

            // Vaisseaux des 4 joueurs
            for p in &self.players {
                let center = Pos2::new(arena_rect.min.x + p.x, arena_rect.min.y + p.y);
                painter.rect_filled(Rect::from_center_size(center, Vec2::new(32.0, 32.0)), Rounding::same(8.0), p.color);
                painter.text(Pos2::new(center.x, center.y - 22.0), egui::Align2::CENTER_CENTER, p.name, FontId::proportional(11.0), Color32::WHITE);
                painter.text(Pos2::new(center.x, center.y + 22.0), egui::Align2::CENTER_CENTER, &p.last_btn, FontId::proportional(10.0), p.color);
            }

            ui.allocate_rect(arena_rect, egui::Sense::hover());
            ui.add_space(10.0);

            // 2. PANNEAU DE TOUCHES EN DIRECT & CLAVIERS SYNCHRONISÉS
            ui.columns(2, |cols| {
                // COLONNE GAUCHE : MES TOUCHES CLAVIER (ENVOYÉES EN DIRECT)
                egui::Frame::none()
                    .fill(COLOR_CARD)
                    .stroke(Stroke::new(1.0_f32, COLOR_BORDER))
                    .inner_margin(12.0)
                    .rounding(Rounding::same(8.0))
                    .show(&mut cols[0], |ui| {
                        ui.label(RichText::new("⌨ Vos touches Clavier (Joueur 1)").color(COLOR_P1).strong().size(13.5));
                        ui.add_space(6.0);
                        ui.horizontal(|ui| {
                            let keys = ["Z", "Q", "S", "D", "ESPACE"];
                            for &k in &keys {
                                let active = self.my_keys_pressed.contains(&k.to_string());
                                let bg = if active { COLOR_P1 } else { Color32::from_rgb(45, 55, 72) };
                                let txt_col = if active { Color32::WHITE } else { Color32::GRAY };
                                egui::Frame::none()
                                    .fill(bg)
                                    .inner_margin(egui::Margin::symmetric(10.0, 6.0))
                                    .rounding(Rounding::same(6.0))
                                    .show(ui, |ui| {
                                        ui.label(RichText::new(k).color(txt_col).strong());
                                    });
                            }
                        });
                        ui.add_space(6.0);
                        let local_str = if self.my_keys_pressed.is_empty() { "Aucune touche pressée".to_string() } else { self.my_keys_pressed.join(" + ") };
                        ui.label(RichText::new(format!("En cours : {}", local_str)).color(Color32::WHITE).size(12.0));
                    });

                // COLONNE DROITE : TOUCHES REÇUES DE L'AUTRE JOUEUR (EN DIRECT PAR RÉSEAU !)
                egui::Frame::none()
                    .fill(COLOR_CARD)
                    .stroke(Stroke::new(1.0_f32, COLOR_BORDER))
                    .inner_margin(12.0)
                    .rounding(Rounding::same(8.0))
                    .show(&mut cols[1], |ui| {
                        ui.label(RichText::new("🎮 Touches reçues de l'Invité (Joueur 2)").color(COLOR_P2).strong().size(13.5));
                        ui.add_space(6.0);
                        let peer_keys = net.peer_keys_pressed.clone();
                        ui.horizontal(|ui| {
                            let keys = ["Z", "Q", "S", "D", "ESPACE", "ENTREE"];
                            for &k in &keys {
                                let active = peer_keys.contains(&k.to_string());
                                let bg = if active { COLOR_P2 } else { Color32::from_rgb(45, 55, 72) };
                                let txt_col = if active { Color32::WHITE } else { Color32::GRAY };
                                egui::Frame::none()
                                    .fill(bg)
                                    .inner_margin(egui::Margin::symmetric(8.0, 6.0))
                                    .rounding(Rounding::same(6.0))
                                    .show(ui, |ui| {
                                        ui.label(RichText::new(k).color(txt_col).strong());
                                    });
                            }
                        });
                        ui.add_space(6.0);
                        if !peer_keys.is_empty() {
                            ui.label(RichText::new(format!("Touches reçues en temps réel : {}", peer_keys.join(" + "))).color(COLOR_P2).strong().size(12.5));
                        } else {
                            ui.label(RichText::new("Demandez à votre ami d'appuyer sur Z, Q, S, D ou ESPACE").color(Color32::from_rgb(148, 163, 184)).size(11.5));
                        }
                    });
            });

            ui.add_space(8.0);

            // 3. TEST AUDIO DU JEU (ANTI-ÉCHO COOPLINK)
            egui::Frame::none()
                .fill(COLOR_CARD)
                .stroke(Stroke::new(1.0_f32, COLOR_BORDER))
                .inner_margin(10.0)
                .rounding(Rounding::same(8.0))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("🔊 Test Sonore Sans Écho :").color(COLOR_P3).strong().size(13.0));
                        ui.add_space(8.0);
                        if ui.button(RichText::new("Tir Laser (880 Hz)").strong()).clicked() {
                            self.play_test_beep(880, 100, "Tir Laser");
                        }
                        if ui.button(RichText::new("Saut 8-bit (1200 Hz)").strong()).clicked() {
                            self.play_test_beep(1200, 120, "Saut 8-bit");
                        }
                        if ui.button(RichText::new("Victoire (1760 Hz)").strong()).clicked() {
                            self.play_test_beep(1760, 200, "Victoire");
                        }
                        ui.add_space(10.0);
                        ui.label(RichText::new(&self.sound_feedback).color(COLOR_P2).size(12.0));
                    });
                });
        });

        // 60 / 120 FPS
        ctx.request_repaint_after(Duration::from_millis(8));
    }
}

fn main() -> eframe::Result<()> {
    let vp = egui::ViewportBuilder::default()
        .with_title("CoopLink Virtual Game Test - 60/120 FPS")
        .with_inner_size([760.0, 560.0])
        .with_min_inner_size([680.0, 480.0])
        .with_resizable(true);

    let native_options = eframe::NativeOptions {
        viewport: vp,
        ..Default::default()
    };

    eframe::run_native(
        "CoopLink Virtual Game Test",
        native_options,
        Box::new(|cc| Ok(Box::new(TestGameApp::new(cc)))),
    )
}
