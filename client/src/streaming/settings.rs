#![allow(dead_code)]
//! CoopLink stream settings, status file and closed-loop bitrate control.
//!
//! This module is shared by three consumers:
//!   * the desktop app, which writes the settings before spawning the service,
//!   * the capture service (`cooplink-capture-service`), which reads them,
//!   * the display client (`cooplink-viewer`), which reads the display half.
//!
//! It only depends on `serde` and `std` so the two binaries can include it with
//! `#[path = "../streaming/settings.rs"] mod stream_settings;`.

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Port that carries the encoded video and audio.
pub const DEFAULT_STREAM_PORT: u16 = 47500;
/// Local port used for service health/status nudges.
pub const DEFAULT_CONTROL_PORT: u16 = 47501;
/// Hard ceiling on a single encoded frame, protects the reader from a corrupt header.
pub const MAX_FRAME_BYTES: usize = 4 * 1024 * 1024;
/// Fixed header size of the CoopLink frame protocol (`CLK3`).
pub const FRAME_HEADER_LEN: usize = 16;
pub const FRAME_MAGIC: [u8; 4] = *b"CLK3";
pub const FRAME_KIND_VIDEO: u8 = 1;
pub const FRAME_KIND_AUDIO: u8 = 2;
/// Chunk de flux H.264 élémentaire (encodeur inter-images).
pub const FRAME_KIND_HEVC: u8 = 3;
/// Métadonnées de flux (largeur, hauteur, codec) envoyées une fois.
pub const FRAME_KIND_INFO: u8 = 4;

fn default_stream_port() -> u16 {
    DEFAULT_STREAM_PORT
}

fn daemon_true() -> bool {
    true
}

fn default_width() -> u32 {
    1920
}

fn default_height() -> u32 {
    1080
}

fn default_fps() -> u32 {
    60
}

fn default_bitrate() -> u32 {
    30_000
}

fn default_quality_min() -> u8 {
    42
}

fn default_quality_max() -> u8 {
    90
}

fn default_audio_volume() -> u8 {
    100
}

/// Whether the encoder aims for a flat bitrate or lets the quality drive it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RateControl {
    /// Close the loop on the bitrate: quality floats inside the configured bounds.
    Target,
    /// Keep the quality fixed and let the bitrate float (used for LAN / tests).
    Quality,
}

impl Default for RateControl {
    fn default() -> Self {
        RateControl::Target
    }
}

/// Everything the capture service needs to know about the session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamSettings {
    #[serde(default = "default_stream_port")]
    pub stream_port: u16,
    /// Process image of the game to capture (`Game.exe`). Empty = follow the foreground window.
    #[serde(default)]
    pub target_process: String,
    /// Window-title fragment used when `target_process` is empty.
    #[serde(default)]
    pub target_title: String,
    #[serde(default = "default_width")]
    pub width: u32,
    #[serde(default = "default_height")]
    pub height: u32,
    #[serde(default = "default_fps")]
    pub fps: u32,
    #[serde(default = "default_bitrate")]
    pub bitrate_kbps: u32,
    #[serde(default)]
    pub rate_control: RateControl,
    #[serde(default = "default_quality_min")]
    pub quality_min: u8,
    #[serde(default = "default_quality_max")]
    pub quality_max: u8,
    #[serde(default = "daemon_true")]
    pub audio_enabled: bool,
    #[serde(default = "default_audio_volume")]
    pub audio_volume: u8,
    /// Output device to capture (empty = system default).
    #[serde(default)]
    pub audio_device: String,
    /// When false the service refuses to stream the desktop and waits for a real window.
    #[serde(default)]
    pub allow_desktop_fallback: bool,
    /// Keep the captured window in the foreground so injected inputs reach the game.
    #[serde(default = "daemon_true")]
    pub keep_target_focused: bool,
    /// Free-form label of the detected encoder, shown in the UI.
    #[serde(default)]
    pub encoder: String,
    /// Access token shared with the display client (empty = accept any peer).
    #[serde(default)]
    pub access_token: String,
    /// Fallback path when the host cannot be reached directly (CGNAT, no IPv6).
    #[serde(default)]
    pub relay_enabled: bool,
    /// Relay endpoint (`hub.example.com:47510`).
    #[serde(default)]
    pub relay_addr: String,
    /// Room code used to pair host and guest on the relay.
    #[serde(default)]
    pub relay_room: String,
    /// Clé de diffusion dérivée du salon (hex, 32 octets). Vide = session non
    /// authentifiable : le service génère une clé aléatoire et refuse tout le monde.
    #[serde(default)]
    pub auth_key_hex: String,
}

impl Default for StreamSettings {
    fn default() -> Self {
        Self {
            stream_port: DEFAULT_STREAM_PORT,
            target_process: String::new(),
            target_title: String::new(),
            width: 1920,
            height: 1080,
            fps: 60,
            bitrate_kbps: 30_000,
            rate_control: RateControl::Target,
            quality_min: 42,
            quality_max: 90,
            audio_enabled: true,
            audio_volume: 100,
            audio_device: String::new(),
            allow_desktop_fallback: false,
            keep_target_focused: true,
            encoder: String::new(),
            access_token: String::new(),
            relay_enabled: false,
            relay_addr: String::new(),
            relay_room: String::new(),
            auth_key_hex: String::new(),
        }
    }
}

impl StreamSettings {
    /// Clamp every value into a range the engine can honour.
    pub fn clamp(&mut self) {
        self.width = (self.width.clamp(320, 3840) / 2) * 2;
        self.height = (self.height.clamp(240, 2160) / 2) * 2;
        self.fps = self.fps.clamp(15, 144);
        self.bitrate_kbps = self.bitrate_kbps.clamp(2_000, 120_000);
        self.quality_min = self.quality_min.clamp(25, 80);
        self.quality_max = self.quality_max.clamp(self.quality_min.max(50), 95);
        self.audio_volume = self.audio_volume.min(100);
        if self.stream_port < 1024 {
            self.stream_port = DEFAULT_STREAM_PORT;
        }
    }

    /// `%APPDATA%/CoopLink/stream/session.json`
    pub fn default_path() -> PathBuf {
        session_dir().join("session.json")
    }

    pub fn load_from(path: &PathBuf) -> Option<Self> {
        let raw = fs::read_to_string(path).ok()?;
        let mut parsed: StreamSettings = serde_json::from_str(&raw).ok()?;
        parsed.clamp();
        Some(parsed)
    }

    pub fn load_or_default(path: &PathBuf) -> Self {
        Self::load_from(path).unwrap_or_default()
    }

    pub fn save_to(&self, path: &PathBuf) -> std::io::Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let body = serde_json::to_string_pretty(self)?;
        fs::write(path, body)
    }

    /// Target payload bits per second.
    pub fn target_bps(&self) -> u64 {
        self.bitrate_kbps as u64 * 1000
    }

    pub fn frame_interval(&self) -> Duration {
        Duration::from_micros(1_000_000 / self.fps.max(1) as u64)
    }
}

/// Live counters written by the capture service and read by the UI.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct StreamStats {
    pub running: bool,
    pub target_title: String,
    pub target_process: String,
    pub source_width: u32,
    pub source_height: u32,
    pub width: u32,
    pub height: u32,
    pub fps: u32,
    pub measured_fps: f32,
    pub measured_kbps: u32,
    pub quality: u8,
    pub dropped_frames: u64,
    pub clients: u32,
    /// True when the loopback stream is actually running.
    #[serde(default)]
    pub audio_active: bool,
    #[serde(default)]
    pub audio_device: String,
    #[serde(default)]
    pub audio_error: String,
    pub message: String,
    pub updated_ms: u64,
}

impl StreamStats {
    pub fn status_path() -> PathBuf {
        session_dir().join("status.json")
    }

    pub fn publish(&self) {
        let mut copy = self.clone();
        copy.updated_ms = now_ms();
        if let Ok(body) = serde_json::to_string(&copy) {
            let path = Self::status_path();
            if let Some(parent) = path.parent() {
                let _ = fs::create_dir_all(parent);
            }
            let _ = fs::write(path, body);
        }
    }

    pub fn read() -> Option<Self> {
        let raw = fs::read_to_string(Self::status_path()).ok()?;
        serde_json::from_str(&raw).ok()
    }

    /// True when the file was refreshed by a live service.
    pub fn is_fresh(&self) -> bool {
        let now = now_ms();
        self.updated_ms > 0 && now.saturating_sub(self.updated_ms) < 3_000
    }
}

/// `hôte:port`, en acceptant les littéraux IPv6 nus ou déjà crochetés.
pub fn format_endpoint(host: &str, port: u16) -> String {
    let trimmed = host.trim();
    if trimmed.starts_with('[') {
        return if trimmed.contains("]:") {
            trimmed.to_string()
        } else {
            format!("{}:{}", trimmed, port)
        };
    }
    if trimmed.matches(':').count() >= 2 {
        format!("[{}]:{}", trimmed, port)
    } else if trimmed.contains(':') {
        trimmed.to_string()
    } else {
        format!("{}:{}", trimmed, port)
    }
}

pub fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn session_dir() -> PathBuf {
    match std::env::var("APPDATA") {
        Ok(appdata) if !appdata.trim().is_empty() => PathBuf::from(appdata)
            .join("CoopLink")
            .join("stream"),
        _ => PathBuf::from("cooplink-stream"),
    }
}

/// Closed-loop quality controller.
///
/// The encoder is intra-frame only, so the payload size grows roughly with the
/// square of the quantiser quality. Each review window measures what was really
/// pushed on the socket and moves the quality multiplicatively inside the bounds.
#[derive(Debug, Clone)]
pub struct RateController {
    target_bps: f64,
    quality: f64,
    min_quality: f64,
    max_quality: f64,
}

impl RateController {
    pub fn new(target_kbps: u32, min_quality: u8, max_quality: u8) -> Self {
        let min_q = min_quality.clamp(20, 95) as f64;
        let max_q = (max_quality as f64).max(min_q);
        Self {
            target_bps: (target_kbps.max(500) as f64) * 1000.0,
            quality: (min_q + max_q) / 2.0,
            min_quality: min_q,
            max_quality: max_q,
        }
    }

    pub fn set_target_kbps(&mut self, kbps: u32) {
        self.target_bps = (kbps.max(500) as f64) * 1000.0;
    }

    pub fn target_bps(&self) -> u64 {
        self.target_bps as u64
    }

    pub fn quality(&self) -> u8 {
        self.quality.round().clamp(1.0, 100.0) as u8
    }

    pub fn at_floor(&self) -> bool {
        self.quality <= self.min_quality + 0.5
    }

    pub fn at_ceiling(&self) -> bool {
        self.quality >= self.max_quality - 0.5
    }

    /// Feed the bits actually sent during the last window, get the quality to use next.
    pub fn review(&mut self, bits_sent: u64, elapsed: Duration) -> u8 {
        let secs = elapsed.as_secs_f64().max(0.05);
        let measured = bits_sent as f64 / secs;
        let ratio = (measured / self.target_bps.max(1.0)).max(0.01);

        // sqrt because bits ~ quality^2 for a DCT quantiser.
        let factor = (1.0 / ratio).sqrt().clamp(0.5, 1.6);
        self.quality = (self.quality * factor).clamp(self.min_quality, self.max_quality);
        self.quality()
    }
}

/// Frame-rate ladder used when the quality controller stays pinned at its floor.
#[derive(Debug, Clone)]
pub struct FpsLadder {
    rungs: Vec<u32>,
    index: usize,
    slow_windows: u32,
    fast_windows: u32,
}

impl FpsLadder {
    pub fn new(configured_fps: u32) -> Self {
        let configured = configured_fps.clamp(15, 144);
        let mut rungs = vec![configured];
        for candidate in [120, 90, 60, 48, 30] {
            if candidate < configured && rungs.last().copied() != Some(candidate) {
                rungs.push(candidate);
            }
        }
        if *rungs.last().unwrap() > 24 {
            rungs.push(24);
        }
        Self {
            rungs,
            index: 0,
            slow_windows: 0,
            fast_windows: 0,
        }
    }

    pub fn current(&self) -> u32 {
        self.rungs[self.index]
    }

    pub fn max(&self) -> u32 {
        self.rungs[0]
    }

    /// Returns true when the rung changed.
    pub fn observe(&mut self, at_floor: bool, at_ceiling: bool) -> bool {
        if at_floor {
            self.slow_windows += 1;
            self.fast_windows = 0;
        } else if at_ceiling {
            self.fast_windows += 1;
            self.slow_windows = 0;
        } else {
            self.slow_windows = 0;
            self.fast_windows = 0;
            return false;
        }

        if self.slow_windows >= 3 && self.index + 1 < self.rungs.len() {
            self.index += 1;
            self.slow_windows = 0;
            return true;
        }
        if self.fast_windows >= 6 && self.index > 0 {
            self.index -= 1;
            self.fast_windows = 0;
            return true;
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Emulates an intra encoder: payload grows with quality^2.
    fn frame_bytes(quality: u8, at_q82: f64) -> f64 {
        at_q82 * (quality as f64 / 82.0).powi(2)
    }

    #[test]
    fn quality_moves_down_on_overshoot_and_up_on_undershoot() {
        let mut ctl = RateController::new(30_000, 40, 90);
        let start = ctl.quality();
        let bits = (60u64) * 400_000 * 8; // far above 30 Mbps for a 1 s window
        let after = ctl.review(bits, Duration::from_secs(1));
        assert!(after < start, "quality should drop: {start} -> {after}");
        assert!(after >= 40);

        let low = ctl.review(2_000_000, Duration::from_secs(1));
        assert!(low > after, "quality should climb back: {after} -> {low}");
    }

    #[test]
    fn quality_stays_inside_bounds() {
        let mut ctl = RateController::new(20_000, 45, 88);
        for _ in 0..200 {
            ctl.review(u64::MAX / 2, Duration::from_secs(1));
        }
        assert_eq!(ctl.quality(), 45, "must clamp on the floor");
        for _ in 0..200 {
            ctl.review(1, Duration::from_secs(1));
        }
        assert_eq!(ctl.quality(), 88, "must clamp on the ceiling");
    }

    #[test]
    fn loop_converges_on_the_target_bitrate() {
        // 1080p60 intra payload measured at Q82 for a representative game frame.
        let per_frame_at_q82 = 330_000.0;
        let target_kbps = 30_000u32;
        let fps = 60u64;
        let window = Duration::from_millis(500);
        let frames_per_window = (fps as f64 * window.as_secs_f64()).round() as u64;

        let mut ctl = RateController::new(target_kbps, 40, 90);
        let mut measured_kbps = 0.0;
        for _ in 0..80 {
            let q = ctl.quality();
            let bytes = frame_bytes(q, per_frame_at_q82);
            let bits = (bytes as u64) * frames_per_window * 8;
            measured_kbps = bits as f64 / window.as_secs_f64() / 1000.0;
            ctl.review(bits, window);
        }

        let ratio = measured_kbps / target_kbps as f64;
        assert!(
            (0.6..=1.6).contains(&ratio),
            "converged to {measured_kbps:.0} kbps for a {target_kbps} kbps target (ratio {ratio:.2}), quality {}",
            ctl.quality()
        );
    }

    #[test]
    fn ladder_walks_down_when_pinned_at_the_floor() {
        let mut ladder = FpsLadder::new(60);
        assert_eq!(ladder.current(), 60);
        for _ in 0..2 {
            assert!(!ladder.observe(true, false));
        }
        assert!(ladder.observe(true, false));
        assert!(ladder.current() < 60);
    }

    #[test]
    fn ladder_walks_back_up_and_never_overflows() {
        let mut ladder = FpsLadder::new(60);
        for _ in 0..40 {
            ladder.observe(true, false);
        }
        assert_eq!(ladder.current(), 24, "floor rung");
        for _ in 0..40 {
            ladder.observe(false, true);
        }
        assert_eq!(ladder.current(), 60, "back to the configured rate");
    }

    #[test]
    fn settings_are_clamped_and_round_trip() {
        let mut s = StreamSettings::default();
        s.width = 1919;
        s.height = 10;
        s.fps = 400;
        s.bitrate_kbps = 1;
        s.quality_min = 80;
        s.quality_max = 50;
        s.clamp();
        assert_eq!(s.width % 2, 0);
        assert!(s.height >= 240);
        assert_eq!(s.fps, 144);
        assert!(s.bitrate_kbps >= 2_000);
        assert!(s.quality_max >= s.quality_min);

        let dir = std::env::temp_dir().join("cooplink-settings-test");
        let path = dir.join("session.json");
        s.save_to(&path).expect("save");
        let back = StreamSettings::load_from(&path).expect("load");
        assert_eq!(back.width, s.width);
        assert_eq!(back.fps, s.fps);
        let _ = std::fs::remove_dir_all(dir);
    }
}
