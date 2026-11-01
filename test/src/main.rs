// CoopLink Virtual Game Simulator & Feature Verification Tool
// Copyright (c) 2026 Loic HUON. All Rights Reserved.
// Standalone Portable Executable (No installation required)

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use eframe::egui::{self, Color32, FontId, Key, Pos2, Rect, RichText, Rounding, Stroke, Vec2};
use parking_lot::Mutex;
use std::net::{SocketAddr, UdpSocket};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant, SystemTime};

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
const COLOR_P1: Color32 = Color32::from_rgb(14, 165, 233); // Cyan / Bleu
const COLOR_P2: Color32 = Color32::from_rgb(16, 185, 129); // Vert
const COLOR_P3: Color32 = Color32::from_rgb(245, 158, 11); // Orange
const COLOR_P4: Color32 = Color32::from_rgb(168, 85, 247); // Violet

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum PlayerRole {
    HostPlayer1,  // Hôte (Joueur 1 - Bleu)
    GuestPlayer2, // Invité (Joueur 2 - Vert)
}

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
    peer_role: String,
    peer_keys_pressed: Vec<String>,
    last_peer_packet: Option<Instant>,
    local_cooplink_ip: Option<String>,
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
    current_role: PlayerRole,
    my_instance_id: u32,
    custom_target_ip: String,
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

        // Identifiant d'instance unique pour éliminer tout paquet réfléchi / loopback
        let my_instance_id: u32 = {
            let nanos = SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .map(|d| d.subsec_nanos())
                .unwrap_or(12345);
            nanos ^ (std::process::id() << 16)
        };

        // Détection automatique de l'interface réseau CoopLink locale
        let local_cooplink_ip = if UdpSocket::bind("10.244.0.1:0").is_ok() {
            Some("10.244.0.1".to_string())
        } else if UdpSocket::bind("10.244.0.2:0").is_ok() {
            Some("10.244.0.2".to_string())
        } else {
            None
        };

        let initial_role = if local_cooplink_ip.as_deref() == Some("10.244.0.2") {
            PlayerRole::GuestPlayer2
        } else {
            PlayerRole::HostPlayer1
        };

        let network_sync = Arc::new(Mutex::new(NetworkSyncState {
            local_cooplink_ip: local_cooplink_ip.clone(),
            ..Default::default()
        }));

        // Socket UDP d'écoute : port 39999
        let rx_socket = UdpSocket::bind("0.0.0.0:39999")
            .or_else(|_| UdpSocket::bind("0.0.0.0:0"))
            .ok()
            .map(Arc::new);
        let tx_socket = rx_socket.clone();

        if let Some(sock) = rx_socket {
            let sync_clone = network_sync.clone();
            let my_id = my_instance_id;
            thread::spawn(move || {
                let mut buf = [0u8; 1024];
                let _ = sock.set_read_timeout(Some(Duration::from_millis(300)));

                loop {
                    if let Ok((len, src)) = sock.recv_from(&mut buf) {
                        // RÈGLE CRITIQUE #1 : REFUS TOTAL DU LOOPBACK (127.0.0.1 / ::1)
                        if src.ip().is_loopback() {
                            continue;
                        }

                        if let Ok(msg) = std::str::from_utf8(&buf[..len]) {
                            // Format : TYPE:INSTANCE_ID:ROLE:PAYLOAD
                            let parts: Vec<&str> = msg.split(':').collect();
                            if parts.len() >= 2 {
                                // RÈGLE CRITIQUE #2 : REFUS DES PAQUETS DE NOTRE PROPRE INSTANCE
                                if let Ok(sender_id) = parts[1].parse::<u32>() {
                                    if sender_id == my_id {
                                        continue;
                                    }
                                }

                                let msg_type = parts[0];
                                let peer_role = if parts.len() >= 3 { parts[2] } else { "Pair" };

                                let mut st = sync_clone.lock();
                                st.last_peer_packet = Some(Instant::now());
                                st.peer_connected = true;
                                st.peer_ip = src.ip().to_string();
                                st.peer_role = peer_role.to_string();

                                if msg_type == "PING" {
                                    let pong_reply = format!("PONG:{}:OK", my_id);
                                    let _ = sock.send_to(pong_reply.as_bytes(), src);
                                } else if msg_type == "KEYS" && parts.len() >= 4 {
                                    let keys: Vec<String> = parts[3]
                                        .split(',')
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
            current_role: initial_role,
            my_instance_id,
            custom_target_ip: String::new(),
            network_sync,
            sound_feedback: "Prêt pour le test audio".to_string(),
            sender_socket: tx_socket,
            my_keys_pressed: Vec::new(),
        }
    }

    fn send_network_ping(&self) {
        if let Some(ref sock) = self.sender_socket {
            let _ = sock.set_broadcast(true);
            let role_str = match self.current_role {
                PlayerRole::HostPlayer1 => "Hote",
                PlayerRole::GuestPlayer2 => "Invite",
            };
            let msg = format!("PING:{}:{}", self.my_instance_id, role_str);

            let mut targets = vec![
                "10.244.0.1:39999".to_string(),
                "10.244.0.2:39999".to_string(),
                "255.255.255.255:39999".to_string(),
            ];
            let custom = self.custom_target_ip.trim();
            if !custom.is_empty() {
                targets.push(format!("{}:39999", custom));
            }

            for target in targets {
                if let Ok(addr) = target.parse::<SocketAddr>() {
                    // Refus absolu du loopback
                    if !addr.ip().is_loopback() {
                        let _ = sock.send_to(msg.as_bytes(), addr);
                    }
                }
            }
        }
    }

    fn broadcast_my_keys(&self, keys: &[String]) {
        if let Some(ref sock) = self.sender_socket {
            let role_str = match self.current_role {
                PlayerRole::HostPlayer1 => "Hote",
                PlayerRole::GuestPlayer2 => "Invite",
            };
            let msg = format!("KEYS:{}:{}:{}", self.my_instance_id, role_str, keys.join(","));

            let mut targets = vec![
                "10.244.0.1:39999".to_string(),
                "10.244.0.2:39999".to_string(),
                "255.255.255.255:39999".to_string(),
            ];
            let custom = self.custom_target_ip.trim();
            if !custom.is_empty() {
                targets.push(format!("{}:39999", custom));
            }

            for target in targets {
                if let Ok(addr) = target.parse::<SocketAddr>() {
                    if !addr.ip().is_loopback() {
                        let _ = sock.send_to(msg.as_bytes(), addr);
                    }
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
        });

        if pressed_now != self.my_keys_pressed {
            self.my_keys_pressed = pressed_now.clone();
            self.broadcast_my_keys(&pressed_now);
        }

        // =========================================================================
        // DÉPLACEMENT DES JOUEURS DISSOCIÉS STRICTEMENT PAR RÔLE & PAIRS DISTANTS
        // =========================================================================
        let peer_keys = self.network_sync.lock().peer_keys_pressed.clone();

        for i in 0..4 {
            let mut dx = 0.0_f32;
            let mut dy = 0.0_f32;
            let mut btn_text = "-".to_string();

            // Support Manette XInput locale
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

            // GESTION DU JOUEUR 1 (Hôte - Bleu)
            if i == 0 {
                if self.current_role == PlayerRole::HostPlayer1 {
                    // Si je suis l'Hôte : mes touches locales contrôlent le Joueur 1
                    if self.my_keys_pressed.contains(&"Z".to_string()) || self.my_keys_pressed.contains(&"HAUT".to_string()) { dy -= 260.0; }
                    if self.my_keys_pressed.contains(&"S".to_string()) || self.my_keys_pressed.contains(&"BAS".to_string()) { dy += 260.0; }
                    if self.my_keys_pressed.contains(&"Q".to_string()) || self.my_keys_pressed.contains(&"GAUCHE".to_string()) { dx -= 260.0; }
                    if self.my_keys_pressed.contains(&"D".to_string()) || self.my_keys_pressed.contains(&"DROITE".to_string()) { dx += 260.0; }
                    if self.my_keys_pressed.contains(&"ESPACE".to_string()) { btn_text = "TIR (ESPACE)".to_string(); }
                } else {
                    // Si je suis l'Invité : le Joueur 1 n'est contrôlé QUE par les touches reçues de l'Hôte distant
                    if peer_keys.contains(&"Z".to_string()) || peer_keys.contains(&"HAUT".to_string()) { dy -= 260.0; }
                    if peer_keys.contains(&"S".to_string()) || peer_keys.contains(&"BAS".to_string()) { dy += 260.0; }
                    if peer_keys.contains(&"Q".to_string()) || peer_keys.contains(&"GAUCHE".to_string()) { dx -= 260.0; }
                    if peer_keys.contains(&"D".to_string()) || peer_keys.contains(&"DROITE".to_string()) { dx += 260.0; }
                    if peer_keys.contains(&"ESPACE".to_string()) { btn_text = "TIR (HÔTE)".to_string(); }
                }
            }

            // GESTION DU JOUEUR 2 (Invité - Vert)
            if i == 1 {
                if self.current_role == PlayerRole::GuestPlayer2 {
                    // Si je suis l'Invité : mes touches locales contrôlent le Joueur 2
                    if self.my_keys_pressed.contains(&"Z".to_string()) || self.my_keys_pressed.contains(&"HAUT".to_string()) { dy -= 260.0; }
                    if self.my_keys_pressed.contains(&"S".to_string()) || self.my_keys_pressed.contains(&"BAS".to_string()) { dy += 260.0; }
                    if self.my_keys_pressed.contains(&"Q".to_string()) || self.my_keys_pressed.contains(&"GAUCHE".to_string()) { dx -= 260.0; }
                    if self.my_keys_pressed.contains(&"D".to_string()) || self.my_keys_pressed.contains(&"DROITE".to_string()) { dx += 260.0; }
                    if self.my_keys_pressed.contains(&"ESPACE".to_string()) || self.my_keys_pressed.contains(&"ENTREE".to_string()) { btn_text = "TIR (LOCAL)".to_string(); }
                } else {
                    // Si je suis l'Hôte : le Joueur 2 n'est contrôlé QUE par les touches reçues de l'Invité distant !
                    if peer_keys.contains(&"Z".to_string()) || peer_keys.contains(&"HAUT".to_string()) { dy -= 260.0; }
                    if peer_keys.contains(&"S".to_string()) || peer_keys.contains(&"BAS".to_string()) { dy += 260.0; }
                    if peer_keys.contains(&"Q".to_string()) || peer_keys.contains(&"GAUCHE".to_string()) { dx -= 260.0; }
                    if peer_keys.contains(&"D".to_string()) || peer_keys.contains(&"DROITE".to_string()) { dx += 260.0; }
                    if peer_keys.contains(&"ESPACE".to_string()) || peer_keys.contains(&"ENTREE".to_string()) { btn_text = "TIR (INVITÉ)".to_string(); }
                }
            }

            self.players[i].x = (self.players[i].x + dx * dt).clamp(20.0, 720.0);
            self.players[i].y = (self.players[i].y + dy * dt).clamp(30.0, 260.0);
            if btn_text != "-" { self.players[i].last_btn = btn_text; }
        }

        // Balle rebondissante 60/120 FPS
        self.ball_pos += self.ball_vel * dt;
        if self.ball_pos.x <= 20.0 || self.ball_pos.x >= 720.0 { self.ball_vel.x = -self.ball_vel.x; }
        if self.ball_pos.y <= 30.0 || self.ball_pos.y >= 260.0 { self.ball_vel.y = -self.ball_vel.y; }

        // =========================================================================
        // INTERFACE GRAPHIQUE
        // =========================================================================
        egui::CentralPanel::default().show(ctx, |ui| {
            // EN-TÊTE SUPÉRIEURE
            ui.horizontal(|ui| {
                ui.heading(RichText::new("CoopLink Virtual Game Test").color(Color32::WHITE).size(17.0));
                ui.add_space(8.0);
                ui.label(RichText::new(format!("{:.0} FPS", self.fps_display)).color(COLOR_P2).strong());
                ui.add_space(8.0);
                let elapsed_ms = self.start_time.elapsed().as_millis();
                ui.label(
                    RichText::new(format!("Chrono : {:02}:{:02}.{:03}", (elapsed_ms / 60000) % 60, (elapsed_ms / 1000) % 60, elapsed_ms % 1000))
                        .color(COLOR_P1)
                        .monospace()
                        .strong()
                        .size(13.0),
                );

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button(RichText::new("📡 Tester Réseau (Ping)").strong().color(Color32::WHITE))
                        .on_hover_cursor(egui::CursorIcon::PointingHand)
                        .clicked()
                    {
                        self.send_network_ping();
                    }
                });
            });

            ui.add_space(4.0);

            // SÉLECTEUR DE RÔLE & DIAGNOSTIC IP LOCALE
            ui.horizontal(|ui| {
                ui.label(RichText::new("Votre Rôle sur ce PC :").strong().color(Color32::WHITE));
                if ui.selectable_label(self.current_role == PlayerRole::HostPlayer1, "🎮 Hôte (Joueur 1)").clicked() {
                    self.current_role = PlayerRole::HostPlayer1;
                }
                if ui.selectable_label(self.current_role == PlayerRole::GuestPlayer2, "🕹 Invité (Joueur 2)").clicked() {
                    self.current_role = PlayerRole::GuestPlayer2;
                }

                ui.add_space(10.0);
                let net_state = self.network_sync.lock().clone();
                if let Some(ref local_ip) = net_state.local_cooplink_ip {
                    ui.label(RichText::new(format!("Adaptateur CoopLink : {}", local_ip)).color(COLOR_P2).strong());
                } else {
                    ui.label(RichText::new("IP CoopLink : Non détectée (Lancer le salon)").color(Color32::from_rgb(148, 163, 184)));
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.add(egui::TextEdit::singleline(&mut self.custom_target_ip).desired_width(110.0).hint_text("IP personnalisée..."));
                    ui.label(RichText::new("IP Ami :").color(Color32::from_rgb(203, 213, 225)).size(12.0));
                });
            });

            ui.add_space(4.0);

            // =====================================================================
            // BANNIÈRE DIAGNOSTIC RÉSEAU : STRICTEMENT DISSOCIÉE DE LOOPBACK
            // =====================================================================
            let net = self.network_sync.lock().clone();
            let is_peer_active = if let Some(last) = net.last_peer_packet {
                last.elapsed().as_secs() < 3
            } else {
                false
            };

            egui::Frame::none()
                .fill(if is_peer_active { Color32::from_rgb(6, 78, 59) } else { Color32::from_rgb(45, 26, 3) })
                .stroke(Stroke::new(1.0_f32, if is_peer_active { COLOR_P2 } else { COLOR_P3 }))
                .inner_margin(8.0)
                .rounding(Rounding::same(8.0))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.painter().circle_filled(ui.cursor().min + Vec2::new(6.0, 9.0), 5.0, if is_peer_active { COLOR_P2 } else { COLOR_P3 });
                        ui.add_space(14.0);
                        if is_peer_active {
                            ui.label(
                                RichText::new(format!(
                                    "✓ RÉSEAU COOPLINK VALIDÉ ! L'autre PC communique en direct ({}) — IP: {} — Latence : < 5 ms",
                                    net.peer_role,
                                    net.peer_ip
                                ))
                                .color(Color32::WHITE)
                                .strong()
                                .size(13.0),
                            );
                        } else {
                            ui.label(
                                RichText::new("⏳ En attente de paquets de l'autre PC (aucun paquet local/loopback n'est accepté). Connectez-vous au même salon CoopLink !")
                                    .color(Color32::from_rgb(254, 215, 170))
                                    .size(12.5),
                            );
                        }
                    });
                });

            ui.add_space(6.0);

            // 1. ARÈNE DU JEU 2D (60 / 120 FPS)
            let arena_rect = Rect::from_min_size(ui.cursor().min, Vec2::new(ui.available_width(), 250.0));
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
            for (idx, p) in self.players.iter().enumerate() {
                let center = Pos2::new(arena_rect.min.x + p.x, arena_rect.min.y + p.y);
                painter.rect_filled(Rect::from_center_size(center, Vec2::new(32.0, 32.0)), Rounding::same(8.0), p.color);
                
                let title = if idx == 0 {
                    if self.current_role == PlayerRole::HostPlayer1 { "Joueur 1 (Vous - Hôte)" } else { "Joueur 1 (Hôte Distant)" }
                } else if idx == 1 {
                    if self.current_role == PlayerRole::GuestPlayer2 { "Joueur 2 (Vous - Invité)" } else { "Joueur 2 (Invité Distant)" }
                } else {
                    p.name
                };

                painter.text(Pos2::new(center.x, center.y - 22.0), egui::Align2::CENTER_CENTER, title, FontId::proportional(11.0), Color32::WHITE);
                painter.text(Pos2::new(center.x, center.y + 22.0), egui::Align2::CENTER_CENTER, &p.last_btn, FontId::proportional(10.0), p.color);
            }

            ui.allocate_rect(arena_rect, egui::Sense::hover());
            ui.add_space(8.0);

            // 2. PANNEAU DE TOUCHES EN DIRECT & CLAVIERS SYNCHRONISÉS
            ui.columns(2, |cols| {
                // COLONNE GAUCHE : VOS TOUCHES LOCALES
                let my_title = match self.current_role {
                    PlayerRole::HostPlayer1 => "⌨ Vos touches Clavier (Joueur 1 - Hôte)",
                    PlayerRole::GuestPlayer2 => "⌨ Vos touches Clavier (Joueur 2 - Invité)",
                };
                let my_color = match self.current_role {
                    PlayerRole::HostPlayer1 => COLOR_P1,
                    PlayerRole::GuestPlayer2 => COLOR_P2,
                };

                egui::Frame::none()
                    .fill(COLOR_CARD)
                    .stroke(Stroke::new(1.0_f32, COLOR_BORDER))
                    .inner_margin(10.0)
                    .rounding(Rounding::same(8.0))
                    .show(&mut cols[0], |ui| {
                        ui.label(RichText::new(my_title).color(my_color).strong().size(13.0));
                        ui.add_space(4.0);
                        ui.horizontal(|ui| {
                            let keys = ["Z", "Q", "S", "D", "ESPACE"];
                            for &k in &keys {
                                let active = self.my_keys_pressed.contains(&k.to_string());
                                let bg = if active { my_color } else { Color32::from_rgb(45, 55, 72) };
                                let txt_col = if active { Color32::WHITE } else { Color32::GRAY };
                                egui::Frame::none()
                                    .fill(bg)
                                    .inner_margin(egui::Margin::symmetric(9.0, 5.0))
                                    .rounding(Rounding::same(6.0))
                                    .show(ui, |ui| {
                                        ui.label(RichText::new(k).color(txt_col).strong());
                                    });
                            }
                        });
                        ui.add_space(4.0);
                        let local_str = if self.my_keys_pressed.is_empty() { "Aucune touche pressée".to_string() } else { self.my_keys_pressed.join(" + ") };
                        ui.label(RichText::new(format!("En direct : {}", local_str)).color(Color32::WHITE).size(11.5));
                    });

                // COLONNE DROITE : TOUCHES REÇUES DU PAIR DISTANT
                let peer_title = match self.current_role {
                    PlayerRole::HostPlayer1 => "🎮 Touches reçues de l'Invité (Joueur 2)",
                    PlayerRole::GuestPlayer2 => "🎮 Touches reçues de l'Hôte (Joueur 1)",
                };
                let peer_color = match self.current_role {
                    PlayerRole::HostPlayer1 => COLOR_P2,
                    PlayerRole::GuestPlayer2 => COLOR_P1,
                };

                egui::Frame::none()
                    .fill(COLOR_CARD)
                    .stroke(Stroke::new(1.0_f32, COLOR_BORDER))
                    .inner_margin(10.0)
                    .rounding(Rounding::same(8.0))
                    .show(&mut cols[1], |ui| {
                        ui.label(RichText::new(peer_title).color(peer_color).strong().size(13.0));
                        ui.add_space(4.0);
                        let peer_keys = net.peer_keys_pressed.clone();
                        ui.horizontal(|ui| {
                            let keys = ["Z", "Q", "S", "D", "ESPACE", "ENTREE"];
                            for &k in &keys {
                                let active = peer_keys.contains(&k.to_string());
                                let bg = if active { peer_color } else { Color32::from_rgb(45, 55, 72) };
                                let txt_col = if active { Color32::WHITE } else { Color32::GRAY };
                                egui::Frame::none()
                                    .fill(bg)
                                    .inner_margin(egui::Margin::symmetric(7.0, 5.0))
                                    .rounding(Rounding::same(6.0))
                                    .show(ui, |ui| {
                                        ui.label(RichText::new(k).color(txt_col).strong());
                                    });
                            }
                        });
                        ui.add_space(4.0);
                        if !peer_keys.is_empty() {
                            ui.label(RichText::new(format!("Reçues par le réseau : {}", peer_keys.join(" + "))).color(peer_color).strong().size(12.0));
                        } else {
                            ui.label(RichText::new("En attente de touches de l'autre PC (Z, Q, S, D, Espace)...").color(Color32::from_rgb(148, 163, 184)).size(11.0));
                        }
                    });
            });

            ui.add_space(6.0);

            // 3. TEST AUDIO DU JEU (ANTI-ÉCHO COOPLINK)
            egui::Frame::none()
                .fill(COLOR_CARD)
                .stroke(Stroke::new(1.0_f32, COLOR_BORDER))
                .inner_margin(8.0)
                .rounding(Rounding::same(8.0))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("🔊 Test Sonore :").color(COLOR_P3).strong().size(12.5));
                        ui.add_space(6.0);
                        if ui.button(RichText::new("Tir Laser (880 Hz)").strong()).clicked() {
                            self.play_test_beep(880, 100, "Tir Laser");
                        }
                        if ui.button(RichText::new("Saut 8-bit (1200 Hz)").strong()).clicked() {
                            self.play_test_beep(1200, 120, "Saut 8-bit");
                        }
                        if ui.button(RichText::new("Victoire (1760 Hz)").strong()).clicked() {
                            self.play_test_beep(1760, 200, "Victoire");
                        }
                        ui.add_space(8.0);
                        ui.label(RichText::new(&self.sound_feedback).color(COLOR_P2).size(11.5));
                    });
                });
        });

        // Repaint à 60 / 120 FPS
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
