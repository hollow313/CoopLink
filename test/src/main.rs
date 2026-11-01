// CoopLink Virtual Game Simulator & Feature Verification Tool
// Copyright (c) 2026 Loic HUON. All Rights Reserved.
// Portable Standalone Test Executable (No installation required)

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use eframe::egui::{self, Color32, FontId, Key, Pos2, Rect, RichText, Rounding, Stroke, Vec2};
use parking_lot::Mutex;
use std::net::{SocketAddr, UdpSocket};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
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
const COLOR_BG: Color32 = Color32::from_rgb(15, 23, 42); // Bleu nuit sombre style jeu vidéo
const COLOR_CARD: Color32 = Color32::from_rgb(30, 41, 59);
const COLOR_BORDER: Color32 = Color32::from_rgb(51, 65, 85);
const COLOR_P1: Color32 = Color32::from_rgb(14, 165, 233); // Bleu Cyan
const COLOR_P2: Color32 = Color32::from_rgb(16, 185, 129); // Vert Émeraude
const COLOR_P3: Color32 = Color32::from_rgb(245, 158, 11); // Orange Ambre
const COLOR_P4: Color32 = Color32::from_rgb(168, 85, 247); // Violet

#[derive(Clone)]
struct PlayerShip {
    x: f32,
    y: f32,
    vx: f32,
    vy: f32,
    color: Color32,
    name: &'static str,
    score: u32,
    last_btn: String,
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
    lan_server_active: Arc<AtomicBool>,
    lan_packets_sent: Arc<AtomicU32>,
    discovered_lan_servers: Arc<Mutex<Vec<(String, String, Instant)>>>,
    sound_feedback: String,
}

impl TestGameApp {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let mut style = (*cc.egui_ctx.style()).clone();
        let mut visuals = egui::Visuals::dark();
        visuals.panel_fill = COLOR_BG;
        visuals.window_fill = COLOR_CARD;
        style.visuals = visuals;
        cc.egui_ctx.set_style(style);

        let lan_server_active = Arc::new(AtomicBool::new(false));
        let lan_packets_sent = Arc::new(AtomicU32::new(0));
        let discovered_lan_servers = Arc::new(Mutex::new(Vec::new()));

        let app = Self {
            xinput: XInputApi::load(),
            players: [
                PlayerShip {
                    x: 100.0,
                    y: 180.0,
                    vx: 0.0,
                    vy: 0.0,
                    color: COLOR_P1,
                    name: "Joueur 1 (Hôte)",
                    score: 0,
                    last_btn: "-".to_string(),
                },
                PlayerShip {
                    x: 250.0,
                    y: 180.0,
                    vx: 0.0,
                    vy: 0.0,
                    color: COLOR_P2,
                    name: "Joueur 2 (Invité CoopLink)",
                    score: 0,
                    last_btn: "-".to_string(),
                },
                PlayerShip {
                    x: 400.0,
                    y: 180.0,
                    vx: 0.0,
                    vy: 0.0,
                    color: COLOR_P3,
                    name: "Joueur 3 (Manette #3)",
                    score: 0,
                    last_btn: "-".to_string(),
                },
                PlayerShip {
                    x: 550.0,
                    y: 180.0,
                    vx: 0.0,
                    vy: 0.0,
                    color: COLOR_P4,
                    name: "Joueur 4 (Manette #4)",
                    score: 0,
                    last_btn: "-".to_string(),
                },
            ],
            ball_pos: Pos2::new(320.0, 160.0),
            ball_vel: Vec2::new(260.0, 180.0),
            start_time: Instant::now(),
            last_frame: Instant::now(),
            frame_count: 0,
            fps_display: 60.0,
            fps_timer: Instant::now(),
            lan_server_active,
            lan_packets_sent,
            discovered_lan_servers,
            sound_feedback: "Prêt pour le test audio".to_string(),
        };

        // Démarre l'écouteur LAN Broadcast UDP pour tester la découverte P2P CoopLink
        let discovered_clone = app.discovered_lan_servers.clone();
        thread::spawn(move || {
            let socket = match UdpSocket::bind("0.0.0.0:38888") {
                Ok(s) => s,
                Err(_) => return,
            };
            let _ = socket.set_read_timeout(Some(Duration::from_millis(500)));
            let mut buf = [0u8; 512];

            loop {
                if let Ok((len, src)) = socket.recv_from(&mut buf) {
                    if let Ok(msg) = std::str::from_utf8(&buf[..len]) {
                        if msg.starts_with("COOPLINK_LAN_DISCOVERY:") {
                            let parts: Vec<&str> = msg.split(':').collect();
                            let room_name = parts.get(1).unwrap_or(&"Jeu LAN").to_string();
                            let mut list = discovered_clone.lock();
                            let entry = (room_name, src.to_string(), Instant::now());
                            list.retain(|(_, addr, time)| addr != &src.to_string() && time.elapsed().as_secs() < 10);
                            list.push(entry);
                        }
                    }
                }
            }
        });

        app
    }

    fn toggle_lan_server(&self) {
        let is_running = self.lan_server_active.load(Ordering::Relaxed);
        if is_running {
            self.lan_server_active.store(false, Ordering::Relaxed);
        } else {
            self.lan_server_active.store(true, Ordering::Relaxed);
            let active = self.lan_server_active.clone();
            let sent_counter = self.lan_packets_sent.clone();

            thread::spawn(move || {
                let socket = match UdpSocket::bind("0.0.0.0:0") {
                    Ok(s) => s,
                    Err(_) => return,
                };
                let _ = socket.set_broadcast(true);
                let target: SocketAddr = "255.255.255.255:38888".parse().unwrap();
                let message = b"COOPLINK_LAN_DISCOVERY:Arena-60FPS-LAN";

                while active.load(Ordering::Relaxed) {
                    let _ = socket.send_to(message, target);
                    sent_counter.fetch_add(1, Ordering::Relaxed);
                    thread::sleep(Duration::from_millis(1000));
                }
            });
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

        // Calcul des FPS en temps réel
        self.frame_count += 1;
        if self.fps_timer.elapsed().as_secs_f32() >= 0.5 {
            self.fps_display = self.frame_count as f32 / self.fps_timer.elapsed().as_secs_f32();
            self.frame_count = 0;
            self.fps_timer = Instant::now();
        }

        // =========================================================================
        // MISE À JOUR DES ENTRÉES : XINPUT (MANETTES 1 À 4) + CLAVIER / SOURIS
        // =========================================================================
        for i in 0..4 {
            let mut dx = 0.0_f32;
            let mut dy = 0.0_f32;
            let mut btn_text = "-".to_string();

            if let Some(pad) = self.xinput.poll_gamepad(i as u32) {
                // Sticks analogiques (-32768..32767 avec zone morte)
                let lx = pad.s_thumb_lx as f32 / 32768.0;
                let ly = pad.s_thumb_ly as f32 / 32768.0;
                if lx.abs() > 0.2 {
                    dx += lx * 320.0;
                }
                if ly.abs() > 0.2 {
                    dy -= ly * 320.0;
                }

                // D-Pad
                if (pad.w_buttons & 0x0001) != 0 { dy -= 220.0; } // Up
                if (pad.w_buttons & 0x0002) != 0 { dy += 220.0; } // Down
                if (pad.w_buttons & 0x0004) != 0 { dx -= 220.0; } // Left
                if (pad.w_buttons & 0x0008) != 0 { dx += 220.0; } // Right

                // Boutons A, B, X, Y
                let mut pressed = Vec::new();
                if (pad.w_buttons & 0x1000) != 0 { pressed.push("A"); }
                if (pad.w_buttons & 0x2000) != 0 { pressed.push("B"); }
                if (pad.w_buttons & 0x4000) != 0 { pressed.push("X"); }
                if (pad.w_buttons & 0x8000) != 0 { pressed.push("Y"); }
                if (pad.w_buttons & 0x0100) != 0 { pressed.push("LB"); }
                if (pad.w_buttons & 0x0200) != 0 { pressed.push("RB"); }
                if (pad.w_buttons & 0x0010) != 0 { pressed.push("START"); }
                if (pad.w_buttons & 0x0020) != 0 { pressed.push("BACK"); }

                if !pressed.is_empty() {
                    btn_text = pressed.join("+");
                }
            }

            // Fallback Clavier pour tester sans manette physique
            ctx.input(|input| {
                if i == 0 {
                    // Joueur 1 : ZQSD ou Flèches
                    if input.key_down(Key::Z) || input.key_down(Key::W) || input.key_down(Key::ArrowUp) { dy -= 260.0; }
                    if input.key_down(Key::S) || input.key_down(Key::ArrowDown) { dy += 260.0; }
                    if input.key_down(Key::Q) || input.key_down(Key::A) || input.key_down(Key::ArrowLeft) { dx -= 260.0; }
                    if input.key_down(Key::D) || input.key_down(Key::ArrowRight) { dx += 260.0; }
                    if input.key_down(Key::Space) { btn_text = "ESPACE (Tir)".to_string(); }
                } else if i == 1 {
                    // Joueur 2 : IJKL ou Pavé numérique (pour tester l'émulation Joueur 2 par clavier)
                    if input.key_down(Key::I) || input.key_down(Key::Num8) { dy -= 260.0; }
                    if input.key_down(Key::K) || input.key_down(Key::Num2) { dy += 260.0; }
                    if input.key_down(Key::J) || input.key_down(Key::Num4) { dx -= 260.0; }
                    if input.key_down(Key::L) || input.key_down(Key::Num6) { dx += 260.0; }
                    if input.key_down(Key::Enter) { btn_text = "ENTRÉE (Tir J2)".to_string(); }
                }
            });

            self.players[i].vx = dx;
            self.players[i].vy = dy;
            self.players[i].x = (self.players[i].x + dx * dt).clamp(20.0, 680.0);
            self.players[i].y = (self.players[i].y + dy * dt).clamp(30.0, 310.0);
            self.players[i].last_btn = btn_text;
        }

        // Balle dynamique pour tester la fluidité 60/120 FPS
        self.ball_pos += self.ball_vel * dt;
        if self.ball_pos.x <= 20.0 || self.ball_pos.x >= 680.0 {
            self.ball_vel.x = -self.ball_vel.x;
        }
        if self.ball_pos.y <= 30.0 || self.ball_pos.y >= 310.0 {
            self.ball_vel.y = -self.ball_vel.y;
        }

        // =========================================================================
        // RENDU DE L'INTERFACE GRAPHIQUE DU JEU DE TEST
        // =========================================================================
        egui::CentralPanel::default().show(ctx, |ui| {
            // Bandeau supérieur de statut & métriques temps réel
            ui.horizontal(|ui| {
                ui.heading(
                    RichText::new("CoopLink Virtual Game Test")
                        .color(Color32::WHITE)
                        .strong()
                        .size(18.0),
                );
                ui.add_space(8.0);
                ui.label(
                    RichText::new(format!("{:.0} FPS", self.fps_display))
                        .color(if self.fps_display >= 55.0 { COLOR_P2 } else { COLOR_P3 })
                        .strong()
                        .size(15.0),
                );
                ui.add_space(8.0);
                let elapsed_ms = self.start_time.elapsed().as_millis();
                ui.label(
                    RichText::new(format!("Chrono : {:02}:{:02}.{:03}", (elapsed_ms / 60000) % 60, (elapsed_ms / 1000) % 60, elapsed_ms % 1000))
                        .color(COLOR_P1)
                        .monospace()
                        .strong()
                        .size(14.0),
                );

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(
                        RichText::new("Prêt pour capture CoopLink (DirectX / Vulkan / Bureau)")
                            .color(Color32::from_rgb(148, 163, 184))
                            .small(),
                    );
                });
            });

            ui.add_space(8.0);

            // 1. ZONE DE JEU 2D (Arène graphique animée)
            let arena_rect = Rect::from_min_size(ui.cursor().min, Vec2::new(ui.available_width(), 320.0));
            let painter = ui.painter();

            // Fond de l'arène avec grille subtile
            painter.rect_filled(arena_rect, Rounding::same(10.0), Color32::from_rgb(10, 15, 30));
            painter.rect_stroke(arena_rect, Rounding::same(10.0), Stroke::new(1.5_f32, COLOR_BORDER));

            // Balle rebondissante haute vitesse (pour vérifier le stutter / frame drops)
            painter.circle_filled(
                Pos2::new(arena_rect.min.x + self.ball_pos.x, arena_rect.min.y + self.ball_pos.y),
                12.0,
                Color32::from_rgb(236, 72, 153), // Rose fluo
            );

            // Vaisseaux des 4 joueurs
            for p in &self.players {
                let center = Pos2::new(arena_rect.min.x + p.x, arena_rect.min.y + p.y);
                // Corps du vaisseau
                painter.rect_filled(
                    Rect::from_center_size(center, Vec2::new(34.0, 34.0)),
                    Rounding::same(8.0),
                    p.color,
                );
                // Nom du joueur
                painter.text(
                    Pos2::new(center.x, center.y - 24.0),
                    egui::Align2::CENTER_CENTER,
                    p.name,
                    FontId::proportional(11.0),
                    Color32::WHITE,
                );
                // Dernier bouton pressé
                painter.text(
                    Pos2::new(center.x, center.y + 24.0),
                    egui::Align2::CENTER_CENTER,
                    &p.last_btn,
                    FontId::proportional(10.5),
                    p.color,
                );
            }

            ui.allocate_rect(arena_rect, egui::Sense::hover());
            ui.add_space(10.0);

            // 2. PANNEAU DE CONTRÔLE MULTI-FEATURE : MANETTES, AUDIO & RÉSEAU LAN
            ui.columns(3, |cols| {
                // Colonne 1 : STATUT DES 4 MANETTES VIRTUELLES COOPLINK
                egui::Frame::none()
                    .fill(COLOR_CARD)
                    .stroke(Stroke::new(1.0_f32, COLOR_BORDER))
                    .inner_margin(12.0)
                    .rounding(Rounding::same(8.0))
                    .show(&mut cols[0], |ui| {
                        ui.label(RichText::new("🎮 Test Manettes Co-op (ViGEm)").color(COLOR_P1).strong().size(13.5));
                        ui.add_space(4.0);
                        for i in 0..4 {
                            let pad_opt = self.xinput.poll_gamepad(i as u32);
                            let is_connected = pad_opt.is_some();
                            ui.horizontal(|ui| {
                                let dot_col = if is_connected { COLOR_P2 } else { Color32::from_rgb(100, 116, 139) };
                                ui.painter().circle_filled(ui.cursor().min + Vec2::new(5.0, 8.0), 4.0, dot_col);
                                ui.add_space(12.0);
                                ui.label(RichText::new(format!("Slot #{}: {}", i, if is_connected { "Connecté (XInput)" } else { "En attente..." })).color(if is_connected { Color32::WHITE } else { Color32::GRAY }).size(12.0));
                            });
                        }
                        ui.add_space(4.0);
                        ui.label(RichText::new("Contrôles : ZQSD / Flèches (J1) | IJKL / Pavé num (J2)").color(Color32::from_rgb(148, 163, 184)).small());
                    });

                // Colonne 2 : TEST AUDIO DU JEU (ANTI-ÉCHO COOPLINK)
                egui::Frame::none()
                    .fill(COLOR_CARD)
                    .stroke(Stroke::new(1.0_f32, COLOR_BORDER))
                    .inner_margin(12.0)
                    .rounding(Rounding::same(8.0))
                    .show(&mut cols[1], |ui| {
                        ui.label(RichText::new("🔊 Test Audio Sans Écho").color(COLOR_P2).strong().size(13.5));
                        ui.add_space(4.0);
                        ui.horizontal_wrapped(|ui| {
                            if ui.button(RichText::new("Tir Laser").strong()).clicked() {
                                self.play_test_beep(880, 100, "Tir Laser (880 Hz)");
                            }
                            if ui.button(RichText::new("Saut 8-bit").strong()).clicked() {
                                self.play_test_beep(1200, 120, "Saut (1200 Hz)");
                            }
                            if ui.button(RichText::new("Victoire").strong()).clicked() {
                                self.play_test_beep(1760, 200, "Victoire (1760 Hz)");
                            }
                        });
                        ui.add_space(6.0);
                        ui.label(RichText::new(&self.sound_feedback).color(COLOR_P2).size(11.5));
                        ui.label(RichText::new("Vérifie que l'invité reçoit le son du jeu sans retour micro").color(Color32::from_rgb(148, 163, 184)).small());
                    });

                // Colonne 3 : TEST DÉCOUVERTE RÉSEAU LAN P2P
                egui::Frame::none()
                    .fill(COLOR_CARD)
                    .stroke(Stroke::new(1.0_f32, COLOR_BORDER))
                    .inner_margin(12.0)
                    .rounding(Rounding::same(8.0))
                    .show(&mut cols[2], |ui| {
                        ui.label(RichText::new("🌐 Test Parties LAN P2P").color(COLOR_P3).strong().size(13.5));
                        ui.add_space(4.0);
                        let is_server = self.lan_server_active.load(Ordering::Relaxed);
                        let srv_btn = egui::Button::new(RichText::new(if is_server { "■ Stopper Annonce LAN" } else { "▶ Émettre Annonce LAN (Port 38888)" }).color(Color32::WHITE).strong())
                            .fill(if is_server { Color32::from_rgb(239, 68, 68) } else { COLOR_P3 });
                        if ui.add(srv_btn).clicked() {
                            self.toggle_lan_server();
                        }

                        let count = self.lan_packets_sent.load(Ordering::Relaxed);
                        if is_server {
                            ui.label(RichText::new(format!("Paquets Broadcast envoyés : {}", count)).color(COLOR_P2).size(11.5));
                        }

                        let list = self.discovered_lan_servers.lock();
                        ui.add_space(4.0);
                        ui.label(RichText::new(format!("Parties LAN détectées : {}", list.len())).color(Color32::WHITE).size(12.0));
                        for (name, addr, _) in list.iter().take(2) {
                            ui.label(RichText::new(format!("• {} ({})", name, addr)).color(COLOR_P2).small());
                        }
                    });
            });
        });

        // Repaint permanent pour un affichage 60/120 FPS ultra fluide
        ctx.request_repaint_after(Duration::from_millis(8));
    }
}

fn main() -> eframe::Result<()> {
    let vp = egui::ViewportBuilder::default()
        .with_title("CoopLink Virtual Game Test - 60/120 FPS")
        .with_inner_size([740.0, 530.0])
        .with_min_inner_size([640.0, 460.0])
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
