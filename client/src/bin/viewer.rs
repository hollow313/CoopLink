// CoopLink Stream Viewer
// Displays the shared game, plays the host audio and forwards keyboard/mouse input.
// Copyright (c) 2026 Loic HUON. All Rights Reserved.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

#[path = "../streaming/settings.rs"]
mod stream_settings;

use eframe::egui::{
    self, Color32, FontId, Key, Pos2, Rect, RichText, Rounding, TextureHandle, TextureOptions,
    Vec2,
};
use parking_lot::Mutex;
use std::collections::VecDeque;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};
use stream_settings::{
    FRAME_HEADER_LEN, FRAME_KIND_AUDIO, FRAME_KIND_VIDEO, FRAME_MAGIC, MAX_FRAME_BYTES,
};

const COLOR_BG: Color32 = Color32::from_rgb(10, 15, 30);
const COLOR_CARD: Color32 = Color32::from_rgb(20, 28, 48);
const COLOR_ACCENT: Color32 = Color32::from_rgb(0, 186, 136);
const COLOR_BLUE: Color32 = Color32::from_rgb(0, 102, 255);
const COLOR_MUTED: Color32 = Color32::from_rgb(148, 163, 184);

// ---------------------------------------------------------------------------
// Command line
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
struct ViewerArgs {
    host: String,
    port: u16,
    title: String,
    width: u32,
    height: u32,
    fps: u32,
    bitrate_kbps: u32,
    fullscreen: bool,
    audio: bool,
    send_input: bool,
    /// 1 = host key set, 2 = guest key set (arrows + numpad).
    player: u8,
    /// Two-keyboard mode disables the guest mouse so it cannot steal the host cursor.
    mouse: bool,
    /// Forward the guest's XInput pad to the host's virtual controller.
    gamepad: bool,
    /// Relay endpoint used when the host is not directly reachable.
    relay: Option<String>,
    /// Room code that pairs us with the host on the relay.
    room: String,
}

impl Default for ViewerArgs {
    fn default() -> Self {
        Self {
            host: "10.244.0.1".to_string(),
            port: stream_settings::DEFAULT_STREAM_PORT,
            title: "Session partagée".to_string(),
            width: 1920,
            height: 1080,
            fps: 60,
            bitrate_kbps: 30_000,
            fullscreen: false,
            audio: true,
            send_input: true,
            player: 1,
            mouse: true,
            gamepad: true,
            relay: None,
            room: String::new(),
        }
    }
}

fn parse_args() -> ViewerArgs {
    let argv: Vec<String> = std::env::args().collect();
    let mut args = ViewerArgs::default();
    let mut positional: Vec<String> = Vec::new();
    let mut i = 1;

    while i < argv.len() {
        let token = argv[i].as_str();
        let mut next = |i: &mut usize| -> Option<String> {
            *i += 1;
            argv.get(*i).cloned()
        };
        match token {
            "stream" | "display" => {}
            "--host" => {
                if let Some(v) = next(&mut i) {
                    args.host = v;
                }
            }
            "--port" => {
                if let Some(v) = next(&mut i) {
                    if let Ok(p) = v.parse() {
                        args.port = p;
                    }
                }
            }
            "--title" | "--game" => {
                if let Some(v) = next(&mut i) {
                    args.title = v;
                }
            }
            "--width" => {
                if let Some(v) = next(&mut i) {
                    args.width = v.parse().unwrap_or(args.width);
                }
            }
            "--height" => {
                if let Some(v) = next(&mut i) {
                    args.height = v.parse().unwrap_or(args.height);
                }
            }
            "--fps" => {
                if let Some(v) = next(&mut i) {
                    args.fps = v.parse().unwrap_or(args.fps);
                }
            }
            "--bitrate" => {
                if let Some(v) = next(&mut i) {
                    args.bitrate_kbps = v.parse().unwrap_or(args.bitrate_kbps);
                }
            }
            "--fullscreen" | "--borderless" => args.fullscreen = true,
            "--windowed" => args.fullscreen = false,
            "--mute" => args.audio = false,
            "--view-only" => args.send_input = false,
            "--no-mouse" => args.mouse = false,
            "--no-gamepad" => args.gamepad = false,
            "--relay" => {
                if let Some(v) = next(&mut i) {
                    args.relay = Some(v);
                }
            }
            "--room" => {
                if let Some(v) = next(&mut i) {
                    args.room = v;
                }
            }
            "--player" => {
                if let Some(v) = next(&mut i) {
                    args.player = v.parse().unwrap_or(1);
                }
            }
            other if !other.starts_with('-') => positional.push(other.to_string()),
            _ => {}
        }
        i += 1;
    }

    if let Some(host) = positional.first() {
        if !host.is_empty() && !host.eq_ignore_ascii_case("stream") {
            args.host = host.clone();
        }
    }
    if let Some(title) = positional.get(1) {
        args.title = title.clone();
    }
    args
}

// ---------------------------------------------------------------------------
// Audio playback
// ---------------------------------------------------------------------------

#[derive(Default)]
struct AudioSink {
    samples: VecDeque<f32>,
    input_rate: u32,
    input_channels: usize,
}

impl AudioSink {
    fn push(&mut self, pcm: &[u8], rate: u32, channels: usize) {
        self.input_rate = rate.max(8_000);
        self.input_channels = channels.clamp(1, 2);
        for chunk in pcm.chunks_exact(2) {
            let v = i16::from_le_bytes([chunk[0], chunk[1]]) as f32 / 32768.0;
            self.samples.push_back(v);
        }
        // ~250 ms of stereo headroom, drop the oldest on overflow.
        while self.samples.len() > 48_000 {
            self.samples.pop_front();
        }
    }
}

/// Plays the received PCM at 48 kHz stereo, resampling linearly if the host is not at 48 kHz.
fn start_audio_playback(sink: Arc<Mutex<AudioSink>>, running: Arc<AtomicBool>) {
    thread::spawn(move || {
        use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

        let host = cpal::default_host();
        let device = match host.default_output_device() {
            Some(d) => d,
            None => return,
        };

        let mut config = None;
        if let Ok(configs) = device.supported_output_configs() {
            for cfg in configs {
                if cfg.channels() == 2 && cfg.min_sample_rate() <= 48_000 && cfg.max_sample_rate() >= 48_000 {
                    config = Some(cfg.with_sample_rate(48_000).config());
                    break;
                }
            }
        }
        let config = config.or_else(|| device.default_output_config().ok().map(|c| c.config()));
        let config = match config {
            Some(c) => c,
            None => return,
        };
        let out_rate = config.sample_rate as f64;
        let out_channels = config.channels as usize;
        let sink_clone = sink.clone();

        let stream = device.build_output_stream(
            config,
            move |output: &mut [f32], _| {
                let mut state = sink_clone.lock();
                let in_channels = state.input_channels.max(1);
                let step = state.input_rate as f64 / out_rate;
                let mut pos = 0.0f64;

                if state.samples.len() < in_channels {
                    for sample in output.iter_mut() {
                        *sample = 0.0;
                    }
                    return;
                }

                for frame in output.chunks_mut(out_channels) {
                    let idx = (pos as usize) * in_channels;
                    if idx + in_channels > state.samples.len() {
                        for s in frame.iter_mut() {
                            *s = 0.0;
                        }
                        continue;
                    }
                    let left = state.samples[idx];
                    let right = if in_channels > 1 {
                        state.samples[idx + 1]
                    } else {
                        left
                    };
                    for (ch, s) in frame.iter_mut().enumerate() {
                        // Mono downmix for a 1-channel sink, swap-free stereo otherwise.
                        *s = if ch == 0 { left } else { right };
                    }
                    pos += step;
                    if pos as usize * in_channels + in_channels > state.samples.len() {
                        pos = 0.0;
                    }
                }

                // Drop what has been consumed so the queue does not drift.
                let consumed = (pos as usize) * in_channels;
                for _ in 0..consumed.min(state.samples.len()) {
                    state.samples.pop_front();
                }
            },
            |_err| {},
            None,
        );

        if let Ok(st) = stream {
            let _ = st.play();
            while running.load(Ordering::Relaxed) {
                thread::sleep(Duration::from_millis(100));
            }
        }
    });
}

// ---------------------------------------------------------------------------
// Runtime state
// ---------------------------------------------------------------------------

#[derive(Default)]
struct ViewerState {
    connected: bool,
    host_addr: String,
    game_title: String,
    received_fps: f32,
    received_kbps: f32,
    width: u32,
    height: u32,
    audio_active: bool,
    received_frames: u64,
    dropped: u64,
    status: String,
}

struct ViewerApp {
    state: Arc<Mutex<ViewerState>>,
    frame_image: Arc<Mutex<Option<egui::ColorImage>>>,
    frame_size: Vec2,
    texture: Option<TextureHandle>,
    tcp_tx: Arc<Mutex<Option<TcpStream>>>,
    active_keys: Vec<u16>,
    last_input_time: Instant,
    fullscreen: bool,
    wants_input: bool,
    player: u8,
    mouse_enabled: bool,
}

/// Keyboard map: egui key -> Windows virtual-key code.
const KEY_MAP: &[(Key, u16)] = &[
    (Key::A, 0x41),
    (Key::B, 0x42),
    (Key::C, 0x43),
    (Key::D, 0x44),
    (Key::E, 0x45),
    (Key::F, 0x46),
    (Key::G, 0x47),
    (Key::H, 0x48),
    (Key::I, 0x49),
    (Key::J, 0x4A),
    (Key::K, 0x4B),
    (Key::L, 0x4C),
    (Key::M, 0x4D),
    (Key::N, 0x4E),
    (Key::O, 0x4F),
    (Key::P, 0x50),
    (Key::Q, 0x51),
    (Key::R, 0x52),
    (Key::S, 0x53),
    (Key::T, 0x54),
    (Key::U, 0x55),
    (Key::V, 0x56),
    (Key::W, 0x57),
    (Key::X, 0x58),
    (Key::Y, 0x59),
    (Key::Z, 0x5A),
    (Key::Num0, 0x30),
    (Key::Num1, 0x31),
    (Key::Num2, 0x32),
    (Key::Num3, 0x33),
    (Key::Num4, 0x34),
    (Key::Num5, 0x35),
    (Key::Num6, 0x36),
    (Key::Num7, 0x37),
    (Key::Num8, 0x38),
    (Key::Num9, 0x39),
    (Key::Space, 0x20),
    (Key::Enter, 0x0D),
    (Key::Escape, 0x1B),
    (Key::Tab, 0x09),
    (Key::Backspace, 0x08),
    (Key::Delete, 0x2E),
    (Key::Insert, 0x2D),
    (Key::Home, 0x24),
    (Key::End, 0x23),
    (Key::PageUp, 0x21),
    (Key::PageDown, 0x22),
    (Key::ArrowLeft, 0x25),
    (Key::ArrowUp, 0x26),
    (Key::ArrowRight, 0x27),
    (Key::ArrowDown, 0x28),
    (Key::F1, 0x70),
    (Key::F2, 0x71),
    (Key::F3, 0x72),
    (Key::F4, 0x73),
    (Key::F5, 0x74),
    (Key::F6, 0x75),
    (Key::F7, 0x76),
    (Key::F8, 0x77),
];

/// Second keyboard -> dedicated Player 2 key set, so two players can share one game
/// without a virtual gamepad driver. Arrows to move, numpad for actions.
fn translate_player2(vk: u16) -> u16 {
    match vk {
        0x57 => 0x26, // W -> Up
        0x53 => 0x28, // S -> Down
        0x41 => 0x25, // A -> Left
        0x44 => 0x27, // D -> Right
        0x20 => 0x60, // Space -> Numpad 0 (action / jump)
        0x45 => 0x61, // E -> Numpad 1 (interact)
        0x46 => 0x62, // F -> Numpad 2 (secondary action)
        0x51 => 0x63, // Q -> Numpad 3
        0x52 => 0x64, // R -> Numpad 4 (reload)
        0x43 => 0x65, // C -> Numpad 5 (crouch)
        0x09 => 0x66, // Tab -> Numpad 6 (scoreboard)
        0x10 => 0xA1, // Shift -> Right Shift (sprint)
        0x11 => 0xA3, // Ctrl -> Right Ctrl (crouch)
        0x12 => 0xA5, // Alt -> Right Alt
        other => other,
    }
}

/// Reads the guest's XInput pad and streams its state to the host, which feeds it
/// into a virtual Xbox 360 controller (ViGEmBus).
#[cfg(windows)]
fn start_gamepad_forwarding(tcp_tx: Arc<Mutex<Option<TcpStream>>>, running: Arc<AtomicBool>) {
    use windows_sys::Win32::UI::Input::XboxController::{XInputGetState, XINPUT_STATE};

    thread::spawn(move || {
        let mut state: XINPUT_STATE = unsafe { std::mem::zeroed() };
        let mut was_connected = false;
        while running.load(Ordering::Relaxed) {
            let rc = unsafe { XInputGetState(0, &mut state) };
            if rc == 0 {
                was_connected = true;
                let g = &state.Gamepad;
                let line = format!(
                    "G {:04X} {} {} {} {} {} {}\n",
                    g.wButtons,
                    g.bLeftTrigger,
                    g.bRightTrigger,
                    g.sThumbLX,
                    g.sThumbLY,
                    g.sThumbRX,
                    g.sThumbRY
                );
                if let Some(sock) = tcp_tx.lock().as_mut() {
                    let _ = sock.write_all(line.as_bytes());
                }
            } else if was_connected {
                // Pad unplugged: release everything on the host side.
                was_connected = false;
                if let Some(sock) = tcp_tx.lock().as_mut() {
                    let _ = sock.write_all(b"G 0000 0 0 0 0 0 0\n");
                }
            }
            thread::sleep(Duration::from_millis(16));
        }
    });
}

#[cfg(not(windows))]
fn start_gamepad_forwarding(_tx: Arc<Mutex<Option<TcpStream>>>, _running: Arc<AtomicBool>) {}

/// Envoie `HELLO guest <room>` et attend `OK` : le relais confirme qu'un hôte attend.
fn relay_handshake(stream: &mut TcpStream, room: &str) -> bool {
    if stream
        .write_all(format!("HELLO guest {}\n", room).as_bytes())
        .is_err()
    {
        return false;
    }
    let mut line = Vec::new();
    let mut byte = [0u8; 1];
    while line.len() < 16 {
        match stream.read(&mut byte) {
            Ok(0) => break,
            Ok(_) => {
                if byte[0] == b'\n' {
                    break;
                }
                line.push(byte[0]);
            }
            Err(_) => return false,
        }
    }
    String::from_utf8_lossy(&line).trim() == "OK"
}

impl ViewerApp {
    fn new(args: ViewerArgs) -> Self {
        let state = Arc::new(Mutex::new(ViewerState {
            connected: false,
            host_addr: format!("{}:{}", args.host, args.port),
            game_title: args.title.clone(),
            status: "connexion...".to_string(),
            ..Default::default()
        }));

        let frame_image = Arc::new(Mutex::new(None));
        let tcp_tx = Arc::new(Mutex::new(None));
        let sink = Arc::new(Mutex::new(AudioSink::default()));
        let running = Arc::new(AtomicBool::new(true));

        if args.audio {
            start_audio_playback(sink.clone(), running.clone());
        }
        if args.gamepad {
            start_gamepad_forwarding(tcp_tx.clone(), running.clone());
        }

        let state_stream = state.clone();
        let frame_clone = frame_image.clone();
        let tx_clone = tcp_tx.clone();
        let sink_clone = sink.clone();
        let running_conn = running.clone();
        let host = args.host.clone();
        let port = args.port;
        let requested_kbps = args.bitrate_kbps;

        thread::spawn(move || {
            // The host may be an IPv4 literal, a bare IPv6 literal or an already
            // bracketed IPv6 endpoint. format_endpoint keeps both cases valid.
            // Direct first (IPv4 or IPv6), relay only as a fallback.
            let primary = stream_settings::format_endpoint(&host, port);
            let mut candidates: Vec<(String, bool)> = vec![
                (primary, false),
                (format!("10.244.0.1:{}", port), false),
                (format!("127.0.0.1:{}", port), false),
            ];
            if let Some(relay) = args.relay.clone() {
                if !relay.trim().is_empty() {
                    candidates.push((stream_settings::format_endpoint(&relay, 47510), true));
                }
            }
            let relay_room = args.room.clone();

            while running_conn.load(Ordering::Relaxed) {
                let mut connected = None;
                for (candidate, is_relay) in &candidates {
                    if let Ok(addr) = candidate.parse::<SocketAddr>() {
                        if let Ok(mut s) =
                            TcpStream::connect_timeout(&addr, Duration::from_millis(1200))
                        {
                            if *is_relay {
                                let _ = s.set_read_timeout(Some(Duration::from_millis(2000)));
                                if !relay_handshake(&mut s, &relay_room) {
                                    continue;
                                }
                            }
                            connected = Some((s, candidate.clone()));
                            break;
                        }
                    }
                }

                let Some((mut stream, addr)) = connected else {
                    state_stream.lock().status = "hôte injoignable".to_string();
                    thread::sleep(Duration::from_millis(1000));
                    continue;
                };

                let _ = stream.set_nodelay(true);
                let _ = stream.set_read_timeout(Some(Duration::from_millis(2000)));
                {
                    let mut st = state_stream.lock();
                    st.connected = true;
                    st.host_addr = addr.clone();
                    st.status = "connecté".to_string();
                }
                if let Ok(tx) = stream.try_clone() {
                    *tx_clone.lock() = Some(tx);
                    let mut guard = tx_clone.lock();
                    if let Some(sock) = guard.as_mut() {
                        // Ask the host to focus the game and honour our bitrate request.
                        let _ = sock.write_all(b"F\n");
                        let _ = sock.write_all(format!("B {}\n", requested_kbps).as_bytes());
                    }
                }

                let mut header = [0u8; FRAME_HEADER_LEN];
                let mut window_start = Instant::now();
                let mut window_bytes: u64 = 0;
                let mut window_frames: u32 = 0;

                loop {
                    if !running_conn.load(Ordering::Relaxed) {
                        break;
                    }
                    if stream.read_exact(&mut header).is_err() {
                        break;
                    }
                    if header[0..4] != FRAME_MAGIC {
                        break;
                    }
                    let kind = header[4];
                    let width = u16::from_le_bytes([header[6], header[7]]) as usize;
                    let height = u16::from_le_bytes([header[8], header[9]]) as usize;
                    let len = u32::from_le_bytes([header[12], header[13], header[14], header[15]])
                        as usize;
                    if len == 0 || len > MAX_FRAME_BYTES {
                        break;
                    }

                    let mut payload = vec![0u8; len];
                    if stream.read_exact(&mut payload).is_err() {
                        break;
                    }

                    if kind == FRAME_KIND_VIDEO {
                        if let Ok(img) = image::load_from_memory(&payload) {
                            let rgba = img.to_rgba8();
                            let size = [rgba.width() as usize, rgba.height() as usize];
                            let (w, h) = (rgba.width(), rgba.height());
                            let color_img =
                                egui::ColorImage::from_rgba_unmultiplied(size, &rgba.into_raw());
                            *frame_clone.lock() = Some(color_img);
                            let mut st = state_stream.lock();
                            st.received_frames += 1;
                            st.width = w;
                            st.height = h;
                        }
                    } else if kind == FRAME_KIND_AUDIO {
                        let rate = (height as u32) * 1000;
                        let channels = width.max(1);
                        sink_clone
                            .lock()
                            .push(&payload, rate, channels);
                        state_stream.lock().audio_active = true;
                    }

                    window_bytes += (len + FRAME_HEADER_LEN) as u64;
                    window_frames += 1;
                    let elapsed = window_start.elapsed();
                    if elapsed >= Duration::from_millis(500) {
                        let secs = elapsed.as_secs_f32();
                        let mut st = state_stream.lock();
                        st.received_fps = window_frames as f32 / secs;
                        st.received_kbps = (window_bytes as f32 * 8.0) / secs / 1000.0;
                        drop(st);
                        window_bytes = 0;
                        window_frames = 0;
                        window_start = Instant::now();
                    }
                }

                {
                    let mut st = state_stream.lock();
                    st.connected = false;
                    st.status = "flux interrompu".to_string();
                    st.received_fps = 0.0;
                    st.received_kbps = 0.0;
                }
                *tx_clone.lock() = None;
                thread::sleep(Duration::from_millis(500));
            }
        });

        Self {
            state,
            frame_image,
            frame_size: Vec2::new(args.width as f32, args.height as f32),
            texture: None,
            tcp_tx,
            active_keys: Vec::new(),
            last_input_time: Instant::now(),
            fullscreen: args.fullscreen,
            wants_input: args.send_input,
            player: args.player,
            mouse_enabled: args.mouse,
        }
    }

    fn send_input(&self, line: String) {
        if let Some(stream) = self.tcp_tx.lock().as_mut() {
            let _ = stream.write_all(line.as_bytes());
        }
    }
}

impl eframe::App for ViewerApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if let Some(new_img) = self.frame_image.lock().take() {
            self.frame_size = Vec2::new(new_img.size[0] as f32, new_img.size[1] as f32);
            match self.texture.as_mut() {
                // Reuse the GPU texture instead of allocating one per frame.
                Some(tex) => tex.set(new_img, TextureOptions::LINEAR),
                None => {
                    self.texture = Some(ctx.load_texture(
                        "stream_frame",
                        new_img,
                        TextureOptions::LINEAR,
                    ))
                }
            }
        }

        if self.wants_input {
            let mut keys: Vec<u16> = Vec::new();
            ctx.input(|i| {
                if i.modifiers.shift {
                    keys.push(0x10);
                }
                if i.modifiers.ctrl {
                    keys.push(0x11);
                }
                if i.modifiers.alt {
                    keys.push(0x12);
                }
                for (key, vk) in KEY_MAP {
                    if i.key_down(*key) {
                        keys.push(*vk);
                    }
                }
            });
            keys.sort_unstable();
            keys.dedup();
            if self.player == 2 {
                for vk in keys.iter_mut() {
                    *vk = translate_player2(*vk);
                }
                keys.sort_unstable();
                keys.dedup();
            }

            let changed = keys != self.active_keys;
            let heartbeat = self.last_input_time.elapsed() > Duration::from_millis(50);
            if changed || (heartbeat && !keys.is_empty()) {
                self.active_keys = keys.clone();
                self.last_input_time = Instant::now();
                let payload: Vec<String> = keys.iter().map(|vk| format!("{:X}", vk)).collect();
                // Full state every time: a lost packet can never leave a key stuck.
                self.send_input(format!("K {}\n", payload.join(",")));
            } else if heartbeat {
                self.last_input_time = Instant::now();
                self.send_input("K \n".to_string());
            }

            let (delta, buttons) = ctx.input(|i| {
                let mut mask = 0u8;
                if i.pointer.primary_down() {
                    mask |= 1;
                }
                if i.pointer.secondary_down() {
                    mask |= 2;
                }
                (i.pointer.delta(), mask)
            });
            if self.mouse_enabled && (delta.length() > 0.05 || buttons != 0) {
                self.send_input(format!(
                    "M {} {} {}\n",
                    delta.x.round() as i32,
                    delta.y.round() as i32,
                    buttons
                ));
            }
        }

        egui::TopBottomPanel::top("hud")
            .frame(egui::Frame::none().fill(COLOR_CARD).inner_margin(egui::Margin::symmetric(14.0, 8.0)))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    let st = self.state.lock();
                    let dot = if st.connected { COLOR_ACCENT } else { Color32::from_rgb(255, 170, 0) };
                    ui.painter()
                        .circle_filled(ui.cursor().min + Vec2::new(8.0, 10.0), 5.0, dot);
                    ui.add_space(18.0);
                    ui.label(RichText::new("CoopLink").color(Color32::WHITE).strong());
                    ui.label(RichText::new(format!("· {}", st.game_title)).color(COLOR_ACCENT));
                    ui.label(RichText::new(format!("· {}", st.host_addr)).color(COLOR_MUTED).small());

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(
                            RichText::new(format!(
                                "{:.1} Mbps · {:.0} fps · {}x{} · {}",
                                st.received_kbps / 1000.0,
                                st.received_fps,
                                st.width,
                                st.height,
                                st.status
                            ))
                            .color(COLOR_BLUE)
                            .monospace()
                            .strong(),
                        );
                        if st.audio_active {
                            ui.label(RichText::new("son").color(COLOR_ACCENT).small());
                        }
                    });
                });
            });

        egui::CentralPanel::default()
            .frame(egui::Frame::none().fill(COLOR_BG))
            .show(ctx, |ui| {
                let rect = ui.available_rect_before_wrap();
                let painter = ui.painter();
                painter.rect_filled(rect, Rounding::same(0.0), Color32::from_rgb(6, 10, 20));

                if let Some(ref tex) = self.texture {
                    // Letterbox: keep the source aspect ratio, never stretch the image.
                    let src = self.frame_size;
                    let scale = (rect.width() / src.x).min(rect.height() / src.y);
                    let size = Vec2::new(src.x * scale, src.y * scale);
                    let target = Rect::from_center_size(rect.center(), size);
                    painter.image(
                        tex.id(),
                        target,
                        Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                        Color32::WHITE,
                    );
                } else {
                    painter.text(
                        rect.center(),
                        egui::Align2::CENTER_CENTER,
                        "En attente du flux de l'hôte...",
                        FontId::proportional(20.0),
                        Color32::WHITE,
                    );
                }

                if !self.active_keys.is_empty() {
                    let labels: Vec<String> =
                        self.active_keys.iter().map(|vk| format!("{:X}", vk)).collect();
                    let pos = Pos2::new(rect.min.x + 16.0, rect.max.y - 36.0);
                    painter.rect_filled(
                        Rect::from_min_size(pos, Vec2::new(240.0, 26.0)),
                        Rounding::same(6.0),
                        Color32::from_black_alpha(160),
                    );
                    painter.text(
                        pos + Vec2::new(10.0, 13.0),
                        egui::Align2::LEFT_CENTER,
                        labels.join(" + "),
                        FontId::proportional(12.0),
                        COLOR_ACCENT,
                    );
                }

                // Keep the surface clickable so mouse input reaches the host.
                ui.allocate_rect(rect, egui::Sense::click_and_drag());
            });

        ctx.request_repaint_after(Duration::from_millis(8));
    }
}

fn main() -> eframe::Result<()> {
    let args = parse_args();

    let mut viewport = egui::ViewportBuilder::default()
        .with_title(format!("CoopLink — {}", args.title))
        .with_inner_size([1280.0, 750.0])
        .with_min_inner_size([640.0, 400.0]);

    if args.fullscreen {
        viewport = viewport.with_fullscreen(true);
    }

    let app_args = args.clone();
    eframe::run_native(
        "CoopLink Viewer",
        eframe::NativeOptions {
            viewport,
            ..Default::default()
        },
        Box::new(move |_cc| Ok(Box::new(ViewerApp::new(app_args)))),
    )
}
