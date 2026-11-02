// CoopLink Capture & Stream Service
// Game-window capture, intra-frame encoding, audio loopback and remote input injection.
// Copyright (c) 2026 Loic HUON. All Rights Reserved.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

#[path = "../streaming/settings.rs"]
mod stream_settings;
#[path = "../streaming/stream_crypto.rs"]
mod stream_crypto;
#[path = "../streaming/video_codec.rs"]
mod video_codec;

use parking_lot::Mutex;
use std::collections::{HashSet, VecDeque};
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, AtomicU8, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};
use stream_settings::{
    FpsLadder, RateControl, RateController, StreamSettings, StreamStats, FRAME_HEADER_LEN,
    FRAME_KIND_AUDIO, FRAME_KIND_HEVC, FRAME_KIND_INFO, FRAME_KIND_VIDEO, FRAME_MAGIC,
    MAX_FRAME_BYTES,
};
use stream_crypto::{auth_token, constant_time_eq, from_hex, StreamCipher};
use video_codec::{find_ffmpeg, probe_encoder, Codec, VideoEncoder};
use tracing::{debug, error, info, warn};

// ---------------------------------------------------------------------------
// Shared state
// ---------------------------------------------------------------------------

/// One captured screen frame, already converted to RGB8.
struct VideoFrame {
    rgb: Vec<u8>,
    width: u32,
    height: u32,
}

/// Latest frame, written once by the capture thread and read by every client.
struct FrameSlot {
    inner: Mutex<Option<(u64, Arc<VideoFrame>)>>,
}

impl FrameSlot {
    fn new() -> Self {
        Self {
            inner: Mutex::new(None),
        }
    }

    fn publish(&self, seq: u64, frame: VideoFrame) {
        *self.inner.lock() = Some((seq, Arc::new(frame)));
    }

    fn latest_after(&self, last_seq: u64) -> Option<(u64, Arc<VideoFrame>)> {
        let guard = self.inner.lock();
        match guard.as_ref() {
            Some((seq, frame)) if *seq > last_seq => Some((*seq, frame.clone())),
            _ => None,
        }
    }
}

/// PCM chunks broadcast to every connected client.
struct AudioBus {
    chunks: Mutex<VecDeque<(u64, Arc<Vec<u8>>)>>,
    next_id: AtomicU64,
    sample_rate: AtomicU32,
    channels: AtomicU8,
}

impl AudioBus {
    fn new() -> Self {
        Self {
            chunks: Mutex::new(VecDeque::new()),
            next_id: AtomicU64::new(1),
            sample_rate: AtomicU32::new(48_000),
            channels: AtomicU8::new(2),
        }
    }

    fn push(&self, pcm: Vec<u8>, sample_rate: u32, channels: u8) {
        if pcm.is_empty() {
            return;
        }
        self.sample_rate.store(sample_rate, Ordering::Relaxed);
        self.channels.store(channels, Ordering::Relaxed);
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let mut q = self.chunks.lock();
        q.push_back((id, Arc::new(pcm)));
        while q.len() > 150 {
            q.pop_front();
        }
    }

    /// Chunks newer than `cursor`; what the reader missed is dropped, not queued.
    fn take_since(&self, cursor: &mut u64) -> (Vec<Arc<Vec<u8>>>, u32, u8) {
        let rate = self.sample_rate.load(Ordering::Relaxed);
        let channels = self.channels.load(Ordering::Relaxed);
        let q = self.chunks.lock();
        let mut out = Vec::new();
        for (id, chunk) in q.iter() {
            if *id > *cursor {
                out.push(chunk.clone());
                *cursor = *id;
            }
        }
        (out, rate, channels)
    }
}

/// Counters shared between the client threads and the status publisher.
#[derive(Default)]
struct LiveStats {
    frames_sent: AtomicU64,
    bytes_sent: AtomicU64,
    dropped: AtomicU64,
    quality: AtomicU8,
    fps: AtomicU32,
    width: AtomicU32,
    height: AtomicU32,
    clients: AtomicU32,
    audio_ok: AtomicBool,
    audio_error: Mutex<String>,
    audio_device_name: Mutex<String>,
}

// ---------------------------------------------------------------------------
// Windows engine
// ---------------------------------------------------------------------------

#[cfg(windows)]
mod engine {
    use super::*;
    use std::ffi::c_void;
    use windows_sys::Win32::Foundation::{CloseHandle, BOOL, HANDLE, HWND, LPARAM, POINT, RECT};
    use windows_sys::Win32::Graphics::Gdi::{
        BitBlt, ClientToScreen, CreateCompatibleBitmap, CreateCompatibleDC, DeleteDC, DeleteObject,
        GetDC, GetDIBits, ReleaseDC, SelectObject, SetStretchBltMode, StretchBlt, BITMAPINFO,
        BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS, HALFTONE, SRCCOPY,
    };
    use windows_sys::Win32::System::Threading::{
        OpenProcess, QueryFullProcessImageNameW, PROCESS_QUERY_LIMITED_INFORMATION,
    };
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
        MapVirtualKeyW, SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, INPUT_MOUSE, KEYBDINPUT,
        KEYEVENTF_EXTENDEDKEY, KEYEVENTF_KEYUP, KEYEVENTF_SCANCODE, MOUSEEVENTF_LEFTDOWN,
        MOUSEEVENTF_LEFTUP, MOUSEEVENTF_MOVE, MOUSEEVENTF_RIGHTDOWN, MOUSEEVENTF_RIGHTUP,
        MOUSEINPUT,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        EnumWindows, GetClassNameW, GetClientRect, GetForegroundWindow,
        GetSystemMetrics, GetWindowTextW, GetWindowThreadProcessId, IsIconic, IsWindow,
        IsWindowVisible, SetForegroundWindow, ShowWindow, SM_CXSCREEN, SM_CYSCREEN, SW_RESTORE,
    };

    #[derive(Debug, Clone)]
    pub struct WindowRef {
        /// Raw `HWND`. Kept as `isize` so the struct stays `Send` across threads.
        pub hwnd: isize,
        pub pid: u32,
        pub title: String,
        pub class: String,
        pub process: String,
    }

    /// What the service is currently capturing.
    pub enum CaptureTarget {
        Window(WindowRef),
        Desktop,
        None,
    }

    const SHELL_CLASSES: [&str; 6] = [
        "Progman",
        "WorkerW",
        "Shell_TrayWnd",
        "Shell_SecondaryTrayWnd",
        "Windows.UI.Core.CoreWindow",
        "ApplicationFrameWindow",
    ];

    pub fn own_pid() -> u32 {
        std::process::id()
    }

    unsafe extern "system" fn enum_proc(hwnd: HWND, lparam: LPARAM) -> BOOL {
        let out = &mut *(lparam as *mut Vec<WindowRef>);
        if IsWindowVisible(hwnd) == 0 || IsIconic(hwnd) != 0 {
            return 1;
        }

        let mut class_buf = [0u16; 256];
        let class_len = GetClassNameW(hwnd, class_buf.as_mut_ptr(), class_buf.len() as i32);
        let class = String::from_utf16_lossy(&class_buf[..class_len.max(0) as usize]);
        if SHELL_CLASSES.iter().any(|c| c.eq_ignore_ascii_case(&class)) {
            return 1;
        }

        let mut title_buf = [0u16; 512];
        let title_len = GetWindowTextW(hwnd, title_buf.as_mut_ptr(), title_buf.len() as i32);
        let title = String::from_utf16_lossy(&title_buf[..title_len.max(0) as usize]);
        if title.trim().is_empty() {
            return 1;
        }

        let mut pid = 0u32;
        GetWindowThreadProcessId(hwnd, &mut pid);
        if pid == 0 {
            return 1;
        }

        out.push(WindowRef {
            hwnd: hwnd as isize,
            pid,
            title,
            class,
            process: process_image_name(pid),
        });
        1
    }

    /// `Game.exe` for a pid (empty when the process cannot be queried).
    pub fn process_image_name(pid: u32) -> String {
        unsafe {
            let handle: HANDLE = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
            if handle == 0 {
                return String::new();
            }
            let mut buf = [0u16; 1024];
            let mut size = buf.len() as u32;
            let ok = QueryFullProcessImageNameW(handle, 0, buf.as_mut_ptr(), &mut size);
            CloseHandle(handle);
            if ok == 0 {
                return String::new();
            }
            let full = String::from_utf16_lossy(&buf[..size as usize]);
            full.rsplit(['\\', '/']).next().unwrap_or("").to_string()
        }
    }

    pub fn list_windows() -> Vec<WindowRef> {
        let mut found: Vec<WindowRef> = Vec::new();
        unsafe {
            EnumWindows(Some(enum_proc), &mut found as *mut _ as LPARAM);
        }
        found
    }

    pub fn foreground_window() -> Option<WindowRef> {
        unsafe {
            let hwnd = GetForegroundWindow();
            if hwnd == 0 {
                return None;
            }
            let mut pid = 0u32;
            GetWindowThreadProcessId(hwnd, &mut pid);
            let mut title_buf = [0u16; 512];
            let len = GetWindowTextW(hwnd, title_buf.as_mut_ptr(), title_buf.len() as i32);
            let title = String::from_utf16_lossy(&title_buf[..len.max(0) as usize]);
            let mut class_buf = [0u16; 256];
            let clen = GetClassNameW(hwnd, class_buf.as_mut_ptr(), class_buf.len() as i32);
            let class = String::from_utf16_lossy(&class_buf[..clen.max(0) as usize]);
            Some(WindowRef {
                hwnd: hwnd as isize,
                pid,
                title,
                class,
                process: process_image_name(pid),
            })
        }
    }

    fn is_usable(window: &WindowRef) -> bool {
        !window.process.is_empty()
            && !window.process.eq_ignore_ascii_case("cooplink.exe")
            && window.pid != own_pid()
            && !window.class.eq_ignore_ascii_case("CoopLink")
    }

    /// Pick the window to capture.
    ///
    /// Order: explicit process name, title fragment, foreground window, largest
    /// visible window. Shell windows and CoopLink itself are never returned, and
    /// the whole desktop is only used when explicitly allowed.
    pub fn resolve_target(settings: &StreamSettings, sticky: Option<&WindowRef>) -> CaptureTarget {
        let windows = list_windows();

        if !settings.target_process.trim().is_empty() {
            let wanted = settings.target_process.trim().to_ascii_lowercase();
            if let Some(w) = windows
                .iter()
                .find(|w| w.process.to_ascii_lowercase() == wanted)
            {
                return CaptureTarget::Window(w.clone());
            }
        }

        if !settings.target_title.trim().is_empty() {
            let wanted = settings.target_title.trim().to_ascii_lowercase();
            if let Some(w) = windows
                .iter()
                .find(|w| w.title.to_ascii_lowercase().contains(&wanted) && is_usable(w))
            {
                return CaptureTarget::Window(w.clone());
            }
        }

        // Follow the foreground game, but keep the previous game selected while
        // the host is clicking around inside CoopLink.
        if let Some(fg) = foreground_window() {
            if is_usable(&fg) {
                return CaptureTarget::Window(fg);
            }
        }

        if let Some(prev) = sticky {
            if windows.iter().any(|w| w.hwnd == prev.hwnd && is_usable(w)) {
                return CaptureTarget::Window(prev.clone());
            }
        }

        let candidate = windows
            .iter()
            .filter(|w| is_usable(w))
            .max_by_key(|w| source_size_of(w.hwnd as HWND).map(|(a, b)| a * b).unwrap_or(0));

        if let Some(w) = candidate {
            return CaptureTarget::Window(w.clone());
        }

        if settings.allow_desktop_fallback {
            CaptureTarget::Desktop
        } else {
            CaptureTarget::None
        }
    }

    /// Client area of a window: `(screen_x, screen_y, width, height)`.
    fn client_geometry(hwnd: HWND) -> Option<(i32, i32, u32, u32)> {
        unsafe {
            let mut rc: RECT = std::mem::zeroed();
            if GetClientRect(hwnd, &mut rc) == 0 {
                return None;
            }
            let width = (rc.right - rc.left).max(0) as u32;
            let height = (rc.bottom - rc.top).max(0) as u32;
            if width < 16 || height < 16 {
                return None;
            }
            let mut origin = POINT { x: 0, y: 0 };
            if ClientToScreen(hwnd, &mut origin) == 0 {
                return None;
            }
            Some((origin.x, origin.y, width, height))
        }
    }

    fn source_size_of(hwnd: HWND) -> Option<(u32, u32)> {
        client_geometry(hwnd).map(|(_, _, w, h)| (w, h))
    }

    /// Native size of the captured surface.
    pub fn source_size(target: &CaptureTarget) -> Option<(u32, u32)> {
        match target {
            CaptureTarget::Window(w) => source_size_of(w.hwnd as HWND),
            CaptureTarget::Desktop => unsafe {
                Some((
                    GetSystemMetrics(SM_CXSCREEN) as u32,
                    GetSystemMetrics(SM_CYSCREEN) as u32,
                ))
            },
            CaptureTarget::None => None,
        }
    }

    /// `(title, process, message)` for the status file and the UI.
    pub fn describe(target: &CaptureTarget) -> (String, String, String) {
        match target {
            CaptureTarget::Window(w) => (
                w.title.clone(),
                w.process.clone(),
                "capture active".to_string(),
            ),
            CaptureTarget::Desktop => (
                "Écran complet".to_string(),
                String::new(),
                "aucune fenêtre ciblée".to_string(),
            ),
            CaptureTarget::None => (
                String::new(),
                String::new(),
                "en attente d'une fenêtre de jeu".to_string(),
            ),
        }
    }

    /// Grab `out_w x out_h` pixels from the target (1:1 when the sizes match).
    pub fn grab(target: &CaptureTarget, out_w: u32, out_h: u32) -> Option<(Vec<u8>, u32, u32)> {
        let (src_x, src_y, src_w, src_h) = match target {
            CaptureTarget::Window(w) => client_geometry(w.hwnd as HWND)?,
            CaptureTarget::Desktop => unsafe {
                (
                    0,
                    0,
                    GetSystemMetrics(SM_CXSCREEN) as u32,
                    GetSystemMetrics(SM_CYSCREEN) as u32,
                )
            },
            CaptureTarget::None => return None,
        };
        if src_w < 16 || src_h < 16 {
            return None;
        }

        unsafe {
            let screen_dc = GetDC(0);
            if screen_dc == 0 {
                return None;
            }
            let mem_dc = CreateCompatibleDC(screen_dc);
            if mem_dc == 0 {
                ReleaseDC(0, screen_dc);
                return None;
            }
            let bitmap = CreateCompatibleBitmap(screen_dc, out_w as i32, out_h as i32);
            if bitmap == 0 {
                DeleteDC(mem_dc);
                ReleaseDC(0, screen_dc);
                return None;
            }
            let previous = SelectObject(mem_dc, bitmap);

            let copied = if src_w == out_w && src_h == out_h {
                // 1:1 copy: no resampling, no aliasing, sharpest possible capture.
                BitBlt(
                    mem_dc,
                    0,
                    0,
                    out_w as i32,
                    out_h as i32,
                    screen_dc,
                    src_x,
                    src_y,
                    SRCCOPY,
                )
            } else {
                // Only when the requested size differs from the source size.
                SetStretchBltMode(mem_dc, HALFTONE);
                StretchBlt(
                    mem_dc,
                    0,
                    0,
                    out_w as i32,
                    out_h as i32,
                    screen_dc,
                    src_x,
                    src_y,
                    src_w as i32,
                    src_h as i32,
                    SRCCOPY,
                )
            };

            let mut info: BITMAPINFO = std::mem::zeroed();
            info.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
            info.bmiHeader.biWidth = out_w as i32;
            info.bmiHeader.biHeight = -(out_h as i32); // top-down
            info.bmiHeader.biPlanes = 1;
            info.bmiHeader.biBitCount = 32;
            info.bmiHeader.biCompression = BI_RGB;

            let mut bgra = vec![0u8; (out_w * out_h * 4) as usize];
            let lines = GetDIBits(
                mem_dc,
                bitmap,
                0,
                out_h,
                bgra.as_mut_ptr() as *mut c_void,
                &mut info,
                DIB_RGB_COLORS,
            );

            SelectObject(mem_dc, previous);
            DeleteObject(bitmap);
            DeleteDC(mem_dc);
            ReleaseDC(0, screen_dc);

            if copied == 0 || lines == 0 {
                return None;
            }

            let pixels = (out_w * out_h) as usize;
            let mut rgb = vec![0u8; pixels * 3];
            for (i, chunk) in bgra.chunks_exact(4).enumerate() {
                let dst = i * 3;
                rgb[dst] = chunk[2];
                rgb[dst + 1] = chunk[1];
                rgb[dst + 2] = chunk[0];
            }

            Some((rgb, out_w, out_h))
        }
    }

    fn is_extended_key(vk: u16) -> bool {
        matches!(
            vk,
            0x21..=0x28 | 0x2C | 0x2D | 0x2E | 0x6F | 0x90 | 0xA3 | 0xA5
        )
    }

    /// Press or release a key by scan code (works with DirectInput / Raw Input games).
    pub fn inject_key(vk: u16, down: bool) {
        let scan = unsafe { MapVirtualKeyW(vk as u32, 0) } as u16;
        let mut flags = KEYEVENTF_SCANCODE;
        if !down {
            flags |= KEYEVENTF_KEYUP;
        }
        if is_extended_key(vk) {
            flags |= KEYEVENTF_EXTENDEDKEY;
        }
        let input = INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: vk,
                    wScan: scan,
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

    /// Relative mouse motion plus the pressed-button state.
    pub fn inject_mouse(dx: i32, dy: i32, buttons: u8) {
        let mut flags: u32 = MOUSEEVENTF_MOVE;
        if buttons & 1 != 0 {
            flags |= MOUSEEVENTF_LEFTDOWN;
        }
        if buttons & 2 != 0 {
            flags |= MOUSEEVENTF_RIGHTDOWN;
        }
        let input = INPUT {
            r#type: INPUT_MOUSE,
            Anonymous: INPUT_0 {
                mi: MOUSEINPUT {
                    dx,
                    dy,
                    mouseData: 0,
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

    pub fn inject_mouse_buttons(held_left: bool, held_right: bool, want_left: bool, want_right: bool) {
        let mut flags: u32 = 0;
        if want_left && !held_left {
            flags |= MOUSEEVENTF_LEFTDOWN;
        }
        if !want_left && held_left {
            flags |= MOUSEEVENTF_LEFTUP;
        }
        if want_right && !held_right {
            flags |= MOUSEEVENTF_RIGHTDOWN;
        }
        if !want_right && held_right {
            flags |= MOUSEEVENTF_RIGHTUP;
        }
        if flags == 0 {
            return;
        }
        let input = INPUT {
            r#type: INPUT_MOUSE,
            Anonymous: INPUT_0 {
                mi: MOUSEINPUT {
                    dx: 0,
                    dy: 0,
                    mouseData: 0,
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

    pub fn release_mouse() {
        inject_mouse_buttons(true, true, false, false);
    }

    /// Bring the captured game back to the foreground before injecting input.
    pub fn ensure_foreground(target: &CaptureTarget) {
        let hwnd = match target {
            CaptureTarget::Window(w) => w.hwnd as HWND,
            _ => return,
        };
        unsafe {
            if GetForegroundWindow() == hwnd {
                return;
            }
            ShowWindow(hwnd, SW_RESTORE);
            SetForegroundWindow(hwnd);
        }
    }

    pub fn window_alive(target: &CaptureTarget) -> bool {
        match target {
            CaptureTarget::Window(w) => unsafe { IsWindow(w.hwnd as HWND) != 0 },
            CaptureTarget::Desktop => true,
            CaptureTarget::None => false,
        }
    }
}

#[cfg(not(windows))]
mod engine {
    use super::*;

    #[derive(Debug, Clone)]
    pub struct WindowRef {
        pub pid: u32,
        pub title: String,
        pub class: String,
        pub process: String,
    }

    pub enum CaptureTarget {
        Window(WindowRef),
        Desktop,
        None,
    }

    pub fn resolve_target(_s: &StreamSettings, _sticky: Option<&WindowRef>) -> CaptureTarget {
        CaptureTarget::None
    }
    pub fn source_size(_t: &CaptureTarget) -> Option<(u32, u32)> {
        None
    }
    pub fn describe(_t: &CaptureTarget) -> (String, String, String) {
        (String::new(), String::new(), "capture indisponible".to_string())
    }
    pub fn grab(_t: &CaptureTarget, _w: u32, _h: u32) -> Option<(Vec<u8>, u32, u32)> {
        None
    }
    pub fn inject_key(_vk: u16, _down: bool) {}
    pub fn inject_mouse(_dx: i32, _dy: i32, _buttons: u8) {}
    pub fn inject_mouse_buttons(_a: bool, _b: bool, _c: bool, _d: bool) {}
    pub fn release_mouse() {}
    pub fn ensure_foreground(_t: &CaptureTarget) {}
    pub fn window_alive(_t: &CaptureTarget) -> bool {
        false
    }
    pub fn list_windows() -> Vec<WindowRef> {
        Vec::new()
    }
}

// ---------------------------------------------------------------------------
// Virtual gamepad (guest controller -> host)
// ---------------------------------------------------------------------------

/// Guest pad state, exactly what the viewer reads from XInput.
#[derive(Debug, Clone, Copy, Default)]
struct PadState {
    buttons: u16,
    left_trigger: u8,
    right_trigger: u8,
    thumb_lx: i16,
    thumb_ly: i16,
    thumb_rx: i16,
    thumb_ry: i16,
}

impl PadState {
    fn is_neutral(&self) -> bool {
        self.buttons == 0
            && self.left_trigger == 0
            && self.right_trigger == 0
            && self.thumb_lx.abs() < 1500
            && self.thumb_ly.abs() < 1500
            && self.thumb_rx.abs() < 1500
            && self.thumb_ry.abs() < 1500
    }

    /// `G <hex buttons> <lt> <rt> <lx> <ly> <rx> <ry>`
    fn parse(payload: &str) -> Option<Self> {
        let parts: Vec<&str> = payload.split_whitespace().collect();
        if parts.len() < 7 {
            return None;
        }
        Some(Self {
            buttons: u16::from_str_radix(parts[0].trim_start_matches("0x"), 16).ok()?,
            left_trigger: parts[1].parse().ok()?,
            right_trigger: parts[2].parse().ok()?,
            thumb_lx: parts[3].parse().ok()?,
            thumb_ly: parts[4].parse().ok()?,
            thumb_rx: parts[5].parse().ok()?,
            thumb_ry: parts[6].parse().ok()?,
        })
    }
}

/// Virtual Xbox 360 pad backed by ViGEmBus. Guest controllers are forwarded into it.
struct VirtualPad {
    #[cfg(windows)]
    inner: Option<vigem_client::Xbox360Wired<vigem_client::Client>>,
    last: PadState,
}

impl VirtualPad {
    fn open() -> Self {
        #[cfg(windows)]
        {
            match vigem_client::Client::connect() {
                Ok(client) => {
                    let mut target = vigem_client::Xbox360Wired::new(
                        client,
                        vigem_client::TargetId::XBOX360_WIRED,
                    );
                    match target.plugin() {
                        Ok(()) => {
                            info!("Manette virtuelle Xbox 360 branchée (ViGEmBus).");
                            return Self {
                                inner: Some(target),
                                last: PadState::default(),
                            };
                        }
                        Err(e) => warn!("ViGEmBus : branchement de la manette virtuelle impossible ({:?}).", e),
                    }
                }
                Err(e) => warn!(
                    "ViGEmBus introuvable ({:?}) : les manettes des invités ne seront pas transmises. Installez le pilote ViGEmBus.",
                    e
                ),
            }
        }
        Self {
            inner: None,
            last: PadState::default(),
        }
    }

    fn is_ready(&self) -> bool {
        #[cfg(windows)]
        {
            self.inner.is_some()
        }
        #[cfg(not(windows))]
        {
            false
        }
    }

    fn update(&mut self, state: PadState) {
        #[cfg(windows)]
        if let Some(target) = self.inner.as_mut() {
            let report = vigem_client::XGamepad {
                buttons: vigem_client::XButtons(state.buttons),
                left_trigger: state.left_trigger,
                right_trigger: state.right_trigger,
                thumb_lx: state.thumb_lx,
                thumb_ly: state.thumb_ly,
                thumb_rx: state.thumb_rx,
                thumb_ry: state.thumb_ry,
            };
            if let Err(e) = target.update(&report) {
                warn!("Manette virtuelle : mise à jour refusée ({:?}).", e);
            }
        }
        self.last = state;
    }

    fn neutralize(&mut self) {
        if !self.last.is_neutral() {
            self.update(PadState::default());
        }
    }
}

impl Drop for VirtualPad {
    fn drop(&mut self) {
        self.neutralize();
    }
}

// ---------------------------------------------------------------------------
// Encoding
// ---------------------------------------------------------------------------

fn encode_frame(rgb: &[u8], width: u32, height: u32, quality: u8) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(96_000);
    let mut encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, quality);
    if encoder
        .encode(rgb, width, height, image::ExtendedColorType::Rgb8)
        .is_ok()
    {
        Some(out)
    } else {
        None
    }
}

fn write_header(buf: &mut [u8; FRAME_HEADER_LEN], kind: u8, a: u16, b: u16, len: u32) {
    buf[0..4].copy_from_slice(&FRAME_MAGIC);
    buf[4] = kind;
    buf[5] = 0;
    buf[6..8].copy_from_slice(&a.to_le_bytes()); // video: width   / audio: channels
    buf[8..10].copy_from_slice(&b.to_le_bytes()); // video: height  / audio: kHz
    buf[10..12].copy_from_slice(&0u16.to_le_bytes());
    buf[12..16].copy_from_slice(&len.to_le_bytes());
}

// ---------------------------------------------------------------------------
// Audio loopback
// ---------------------------------------------------------------------------

fn start_audio_capture(
    bus: Arc<AudioBus>,
    volume: u8,
    wanted_device: String,
    stats: Arc<LiveStats>,
    running: Arc<AtomicBool>,
) {
    thread::spawn(move || {
        use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

        let host = cpal::default_host();
        // The selected game plays on the system default output; capture that one.
        let _ = &wanted_device;
        let device = match host.default_output_device() {
            Some(d) => d,
            None => {
                let msg = "Aucune sortie audio disponible".to_string();
                warn!("{} : diffusion vidéo sans son.", msg);
                *stats.audio_error.lock() = msg;
                return;
            }
        };
        let device_name = format!("{:?}", device.description());

        let configs: Vec<_> = match device.supported_input_configs() {
            Ok(c) => c.collect(),
            Err(e) => {
                let msg = format!("Loopback indisponible sur '{}' : {}", device_name, e);
                warn!("{}", msg);
                *stats.audio_error.lock() = msg;
                return;
            }
        };

        // Prefer stereo 48 kHz so the client never has to resample at 44.1 kHz.
        let mut chosen = None;
        for cfg in &configs {
            if cfg.channels() == 2 && cfg.max_sample_rate() >= 48_000 {
                chosen = Some(cfg.clone().with_sample_rate(48_000).config());
                break;
            }
        }
        if chosen.is_none() {
            chosen = configs
                .into_iter()
                .next()
                .map(|c| c.with_max_sample_rate().config());
        }
        let stream_config = match chosen {
            Some(c) => c,
            None => {
                let msg = "Aucune configuration audio exploitable".to_string();
                warn!("{}.", msg);
                *stats.audio_error.lock() = msg;
                return;
            }
        };

        let sample_rate = stream_config.sample_rate;
        let channels = stream_config.channels;
        info!(
            "Capture audio système : {} Hz, {} canal(aux), volume {}%",
            sample_rate, channels, volume
        );

        let gain = (volume as f32 / 100.0).clamp(0.0, 1.0);
        let bus_clone = bus.clone();
        let chunk_bytes = ((sample_rate as usize * channels as usize * 20) / 1000) * 2;
        let mut pending: Vec<u8> = Vec::with_capacity(chunk_bytes * 2);

        let stream = device.build_input_stream(
            stream_config,
            move |data: &[f32], _| {
                for &sample in data {
                    let v = (sample.clamp(-1.0, 1.0) * gain * 32767.0) as i16;
                    pending.extend_from_slice(&v.to_le_bytes());
                }
                while chunk_bytes > 0 && pending.len() >= chunk_bytes {
                    let rest = pending.split_off(chunk_bytes);
                    let chunk = std::mem::replace(&mut pending, rest);
                    bus_clone.push(chunk, sample_rate, channels as u8);
                }
            },
            |err| warn!("Erreur du flux audio : {}", err),
            None,
        );

        match stream {
            Ok(st) => match st.play() {
                Ok(()) => {
                    stats.audio_ok.store(true, Ordering::Relaxed);
                    *stats.audio_error.lock() = String::new();
                    *stats.audio_device_name.lock() = device_name.clone();
                    info!("Son du jeu capturé sur '{}' ({} Hz)", device_name, sample_rate);
                    while running.load(Ordering::Relaxed) {
                        thread::sleep(Duration::from_millis(100));
                    }
                }
                Err(e) => {
                    let msg = format!("Lecture loopback impossible sur '{}' : {}", device_name, e);
                    warn!("{}", msg);
                    *stats.audio_error.lock() = msg;
                }
            },
            Err(e) => {
                let msg = format!("Capture audio impossible sur '{}' : {}", device_name, e);
                warn!("{}", msg);
                *stats.audio_error.lock() = msg;
            }
        }
    });
}

// ---------------------------------------------------------------------------
// Client connection
// ---------------------------------------------------------------------------

#[derive(Debug, Default)]
struct InputState {
    held: HashSet<u16>,
    buttons: u8,
    last_activity: Option<Instant>,
}

fn apply_key_state(
    state: &mut InputState,
    target: &engine::CaptureTarget,
    wanted: &[u16],
    focus: bool,
) {
    let wanted: HashSet<u16> = wanted.iter().copied().filter(|v| *v > 0).collect();
    if focus || !wanted.is_empty() {
        engine::ensure_foreground(target);
    }
    for vk in wanted.iter() {
        if !state.held.contains(vk) {
            engine::inject_key(*vk, true);
        }
    }
    for vk in state.held.iter() {
        if !wanted.contains(vk) {
            engine::inject_key(*vk, false);
        }
    }
    state.held = wanted;
    state.last_activity = Some(Instant::now());
}

fn release_all(state: &mut InputState) {
    for vk in state.held.drain() {
        engine::inject_key(vk, false);
    }
    if state.buttons != 0 {
        engine::inject_mouse_buttons(true, true, false, false);
        state.buttons = 0;
    }
}

/// Buffered line reader: a TCP read that splits a command can never lose it.
struct LineReader {
    pending: Vec<u8>,
}

impl LineReader {
    fn new() -> Self {
        Self {
            pending: Vec::with_capacity(4096),
        }
    }

    fn feed(&mut self, data: &[u8]) -> Vec<String> {
        self.pending.extend_from_slice(data);
        if self.pending.len() > 16 * 1024 {
            self.pending.clear();
            return Vec::new();
        }
        let mut lines = Vec::new();
        while let Some(pos) = self.pending.iter().position(|b| *b == b'\n') {
            let raw: Vec<u8> = self.pending.drain(..=pos).collect();
            let text = String::from_utf8_lossy(&raw);
            let trimmed = text.trim();
            if !trimmed.is_empty() {
                lines.push(trimmed.to_string());
            }
        }
        lines
    }
}

fn handle_client(
    mut stream: TcpStream,
    settings: Arc<StreamSettings>,
    frames: Arc<FrameSlot>,
    audio: Arc<AudioBus>,
    stats: Arc<LiveStats>,
    running: Arc<AtomicBool>,
    target: Arc<Mutex<engine::CaptureTarget>>,
    auth_timeout: Duration,
    codec: Codec,
) {
    let peer = stream
        .peer_addr()
        .map(|a| a.to_string())
        .unwrap_or_else(|_| "inconnu".to_string());
    let _ = stream.set_nodelay(true);
    // A slow peer must never freeze the encoder.
    let _ = stream.set_write_timeout(Some(Duration::from_millis(80)));

    // --- clé de session + authentification ------------------------------
    let key: [u8; 32] = match from_hex(&settings.auth_key_hex) {
        Some(raw) if raw.len() == 32 => {
            let mut k = [0u8; 32];
            k.copy_from_slice(&raw);
            k
        }
        _ => {
            warn!(
                "Aucune clé de session fournie : le service génère une clé aléatoire, \
                 aucun invité ne pourra s'authentifier."
            );
            rand::random()
        }
    };
    let cipher = StreamCipher::new(&key);

    // Une connexion directe doit s'authentifier tout de suite ; via le relais,
    // l'hôte peut attendre longtemps avant qu'un invité n'arrive.
    let _ = stream.set_read_timeout(Some(auth_timeout));
    // Le relais envoie d'abord son propre accusé (`OK`) : on ignore toute ligne
    // qui n'est pas le jeton, plutôt que de rejeter la connexion.
    let mut presented: Vec<u8> = Vec::new();
    for _ in 0..6 {
        let mut line: Vec<u8> = Vec::new();
        let mut byte = [0u8; 1];
        let mut closed = false;
        while line.len() < 80 {
            match stream.read(&mut byte) {
                Ok(0) => {
                    closed = true;
                    break;
                }
                Ok(_) => {
                    if byte[0] == b'\n' {
                        break;
                    }
                    line.push(byte[0]);
                }
                Err(_) => {
                    closed = true;
                    break;
                }
            }
        }
        let text = String::from_utf8_lossy(&line).trim().to_string();
        if let Some(token) = text.strip_prefix("AUTH ") {
            presented = from_hex(token).unwrap_or_default();
            break;
        }
        if closed {
            break;
        }
    }
    if presented.is_empty() {
        debug!("Aucun jeton reçu de {} (délai d'authentification écoulé)", peer);
        return;
    }
    if !constant_time_eq(&presented, &auth_token(&key)) {
        warn!("Connexion refusée : jeton de session invalide ({})", peer);
        let _ = stream.write_all(b"DENY\n");
        return;
    }
    if stream.write_all(b"OK\n").is_err() {
        return;
    }
    info!("Client de diffusion authentifié : {}", peer);
    let _ = stream.set_read_timeout(None);

    // ---- encodeur vidéo -------------------------------------------------
    let mut encoder = if codec != Codec::Jpeg {
        find_ffmpeg().and_then(|ffmpeg| {
            VideoEncoder::start(
                &ffmpeg,
                codec,
                settings.width,
                settings.height,
                settings.fps,
                settings.bitrate_kbps,
            )
        })
    } else {
        None
    };
    if encoder.is_none() && codec != Codec::Jpeg {
        warn!("Encodeur H.264 indisponible (ffmpeg absent ou codec refusé) : repli JPEG.");
    }
    if let Some(enc) = encoder.as_ref() {
        info!(
            "Diffusion H.264 ({:?}) {}x{} @ {} FPS, cible {} kbps",
            enc.codec, enc.width, enc.height, settings.fps, settings.bitrate_kbps
        );
        // Dimensions annoncées une fois : le client démarre son décodeur.
        let mut info_payload = Vec::with_capacity(5);
        info_payload.extend_from_slice(&(enc.width as u16).to_le_bytes());
        info_payload.extend_from_slice(&(enc.height as u16).to_le_bytes());
        info_payload.push(1);
        if let Some(sealed) = cipher.seal(&info_payload) {
            let mut header = [0u8; FRAME_HEADER_LEN];
            write_header(
                &mut header,
                FRAME_KIND_INFO,
                enc.width as u16,
                enc.height as u16,
                sealed.len() as u32,
            );
            header[5] = 1;
            let _ = stream
                .write_all(&header)
                .and_then(|_| stream.write_all(&sealed))
                .and_then(|_| stream.flush());
        }
    }

    stats.clients.fetch_add(1, Ordering::Relaxed);
    let mut pad = VirtualPad::open();
    // Bitrate the client asked for (it can lower it without renegotiating).
    let requested_kbps = Arc::new(AtomicU32::new(0));

    // ---- input reader (client -> host) ----------------------------------
    let input_running = running.clone();
    let mut input_stream = match stream.try_clone() {
        Ok(s) => s,
        Err(_) => return,
    };
    let input_target = target.clone();
    let input_stats = stats.clone();
    let input_bitrate = requested_kbps.clone();
    let peer_for_input = peer.clone();
    thread::spawn(move || {
        let mut reader = LineReader::new();
        let mut state = InputState::default();
        let mut buf = [0u8; 4096];
        let _ = input_stream.set_read_timeout(Some(Duration::from_millis(200)));
        while input_running.load(Ordering::Relaxed) {
            match input_stream.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => {
                    for line in reader.feed(&buf[..n]) {
                        if let Some(rest) = line.strip_prefix("K ") {
                            let keys: Vec<u16> = rest
                                .split(',')
                                .filter_map(|t| u16::from_str_radix(t.trim(), 16).ok())
                                .collect();
                            let guard = input_target.lock();
                            apply_key_state(&mut state, &guard, &keys, false);
                        } else if let Some(rest) = line.strip_prefix("M ") {
                            let parts: Vec<i32> = rest
                                .split_whitespace()
                                .filter_map(|t| t.parse::<i32>().ok())
                                .collect();
                            if parts.len() >= 3 {
                                let guard = input_target.lock();
                                engine::ensure_foreground(&guard);
                                engine::inject_mouse(parts[0], parts[1], parts[2] as u8);
                                state.buttons = parts[2] as u8;
                            }
                        } else if line == "F" {
                            let guard = input_target.lock();
                            engine::ensure_foreground(&guard);
                        } else if let Some(rest) = line.strip_prefix("G ") {
                            if let Some(state) = PadState::parse(rest) {
                                // The pad lives in this thread, no lock needed.
                                pad.update(state);
                            }
                        } else if let Some(rest) = line.strip_prefix("B ") {
                            if let Ok(kbps) = rest.trim().parse::<u32>() {
                                input_bitrate.store(kbps.clamp(2_000, 120_000), Ordering::Relaxed);
                            }
                        }
                    }
                    if let Some(last) = state.last_activity {
                        if last.elapsed() > Duration::from_millis(700) && !state.held.is_empty() {
                            release_all(&mut state);
                        }
                    }
                }
                Err(ref e)
                    if e.kind() == std::io::ErrorKind::WouldBlock
                        || e.kind() == std::io::ErrorKind::TimedOut =>
                {
                    if let Some(last) = state.last_activity {
                        if last.elapsed() > Duration::from_millis(700) {
                            release_all(&mut state);
                        }
                    }
                }
                Err(_) => break,
            }
        }
        release_all(&mut state);
        input_stats.clients.fetch_sub(1, Ordering::Relaxed);
        info!("Client de diffusion déconnecté : {}", peer_for_input);
    });

    // ---- video + audio writer -------------------------------------------
    let mut controller = RateController::new(
        settings.bitrate_kbps,
        settings.quality_min,
        settings.quality_max,
    );
    let mut ladder = FpsLadder::new(settings.fps);
    let mut last_seq = 0u64;
    let mut applied_kbps = settings.bitrate_kbps;
    let mut audio_cursor = 0u64;
    let mut window_start = Instant::now();
    let mut window_bits: u64 = 0;
    let mut next_frame = Instant::now();

    while running.load(Ordering::Relaxed) {
        let interval = Duration::from_micros(1_000_000 / ladder.current().max(1) as u64);

        if let Some((seq, frame)) = frames.latest_after(last_seq) {
            last_seq = seq;

            if let Some(enc) = encoder.as_mut() {
                // Chemin H.264 : l'encodeur gère le débit, on pousse l'image brute.
                if !enc.write_frame(&frame.rgb) {
                    warn!("Encodeur vidéo interrompu, fin de la session {}", peer);
                    break;
                }
                stats.width.store(frame.width, Ordering::Relaxed);
                stats.height.store(frame.height, Ordering::Relaxed);
            } else {
                let quality = if settings.rate_control == RateControl::Quality {
                    settings.quality_max
                } else {
                    controller.quality()
                };

                if let Some(jpeg) = encode_frame(&frame.rgb, frame.width, frame.height, quality) {
                    if let Some(payload) = cipher.seal(&jpeg) {
                        if payload.len() <= MAX_FRAME_BYTES {
                            let mut header = [0u8; FRAME_HEADER_LEN];
                            write_header(
                                &mut header,
                                FRAME_KIND_VIDEO,
                                frame.width as u16,
                                frame.height as u16,
                                payload.len() as u32,
                            );
                            header[5] = 1; // chiffré
                            let sent = stream
                                .write_all(&header)
                                .and_then(|_| stream.write_all(&payload))
                                .and_then(|_| stream.flush());
                            match sent {
                                Ok(()) => {
                                    window_bits +=
                                        (payload.len() as u64 + FRAME_HEADER_LEN as u64) * 8;
                                    stats.frames_sent.fetch_add(1, Ordering::Relaxed);
                                    stats
                                        .bytes_sent
                                        .fetch_add(payload.len() as u64, Ordering::Relaxed);
                                    stats.quality.store(quality, Ordering::Relaxed);
                                }
                                Err(e) => {
                                    stats.dropped.fetch_add(1, Ordering::Relaxed);
                                    if e.kind() == std::io::ErrorKind::BrokenPipe
                                        || e.kind() == std::io::ErrorKind::ConnectionReset
                                    {
                                        break;
                                    }
                                }
                            }
                        }
                    }
                }
            }

            if settings.audio_enabled {
                let (chunks, rate, channels) = audio.take_since(&mut audio_cursor);
                for chunk in chunks {
                    let Some(payload) = cipher.seal(&chunk) else {
                        continue;
                    };
                    let mut header = [0u8; FRAME_HEADER_LEN];
                    write_header(
                        &mut header,
                        FRAME_KIND_AUDIO,
                        channels as u16,
                        (rate / 1000) as u16,
                        payload.len() as u32,
                    );
                    header[5] = 1; // chiffré
                    if stream
                        .write_all(&header)
                        .and_then(|_| stream.write_all(&payload))
                        .is_err()
                    {
                        break;
                    }
                }
            }

            // Chunks H.264 déjà encodés (indépendants de la cadence d'images).
            if let Some(enc) = encoder.as_mut() {
                for chunk in enc.drain() {
                    if let Some(sealed) = cipher.seal(&chunk) {
                        let mut header = [0u8; FRAME_HEADER_LEN];
                        write_header(
                            &mut header,
                            FRAME_KIND_HEVC,
                            enc.width as u16,
                            enc.height as u16,
                            sealed.len() as u32,
                        );
                        header[5] = 1;
                        window_bits += (sealed.len() as u64 + FRAME_HEADER_LEN as u64) * 8;
                        if stream
                            .write_all(&header)
                            .and_then(|_| stream.write_all(&sealed))
                            .is_err()
                        {
                            break;
                        }
                    }
                }
            }

            let requested = requested_kbps.load(Ordering::Relaxed);
            if requested > 0 && requested != applied_kbps {
                controller.set_target_kbps(requested);
                applied_kbps = requested;
                info!("Débit cible ajusté à la demande du client : {} kbps", requested);
            }

            // Fenêtre courte : réaction à un pic de congestion en 400 ms.
            let elapsed = window_start.elapsed();
            if elapsed >= Duration::from_millis(400) {
                let quality = controller.review(window_bits, elapsed);
                if ladder.observe(controller.at_floor(), controller.at_ceiling()) {
                    info!(
                        "Débit insuffisant : cadence ajustée à {} FPS (qualité {})",
                        ladder.current(),
                        quality
                    );
                }
                stats.fps.store(ladder.current(), Ordering::Relaxed);
                window_bits = 0;
                window_start = Instant::now();
            }
        }

        next_frame += interval;
        let now = Instant::now();
        if next_frame > now {
            thread::sleep(next_frame - now);
        } else if now.duration_since(next_frame) > interval * 3 {
            next_frame = now; // never burst to catch up
        }
    }

    let _ = stream.flush();
    info!("Session de diffusion terminée pour {}", peer);
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

fn main() {
    tracing_subscriber::fmt().with_env_filter("info").init();

    let args: Vec<String> = std::env::args().collect();
    let settings_path = args
        .get(1)
        .filter(|a| a.ends_with(".json"))
        .map(PathBuf::from)
        .unwrap_or_else(StreamSettings::default_path);

    let mut settings = StreamSettings::load_or_default(&settings_path);
    // Legacy positional argument: a window title to capture.
    if let Some(title) = args.get(2).filter(|t| !t.trim().is_empty()) {
        if settings.target_title.trim().is_empty() && settings.target_process.trim().is_empty() {
            settings.target_title = title.clone();
        }
    }
    settings.clamp();

    info!(
        "Service de capture prêt : {}x{} @ {} FPS, cible {:.1} Mbps, encodeur {}",
        settings.width,
        settings.height,
        settings.fps,
        settings.bitrate_kbps as f32 / 1000.0,
        if settings.encoder.is_empty() {
            "auto"
        } else {
            &settings.encoder
        }
    );

    // Choix de l'encodeur : NVENC si ffmpeg + GPU le permettent, sinon libx264,
    // sinon repli JPEG (intra-image).
    let codec = {
        match find_ffmpeg() {
            Some(ffmpeg) => {
                if probe_encoder(&ffmpeg, "h264_nvenc") {
                    info!("Encodeur vidéo : H.264 NVENC ({}).", ffmpeg.display());
                    Codec::Nvenc
                } else if probe_encoder(&ffmpeg, "libx264") {
                    info!(
                        "Encodeur vidéo : H.264 logiciel libx264 ({}).",
                        ffmpeg.display()
                    );
                    Codec::Software
                } else {
                    warn!("ffmpeg ne fournit aucun encodeur H.264 : repli JPEG.");
                    Codec::Jpeg
                }
            }
            None => {
                warn!("ffmpeg introuvable : repli JPEG (qualité limitée). Placez ffmpeg.exe à côté de cooplink.exe.");
                Codec::Jpeg
            }
        }
    };

    let running = Arc::new(AtomicBool::new(true));
    let frames = Arc::new(FrameSlot::new());
    let audio = Arc::new(AudioBus::new());
    let stats = Arc::new(LiveStats::default());
    let target = Arc::new(Mutex::new(engine::resolve_target(&settings, None)));
    let settings = Arc::new(settings);

    if settings.audio_enabled {
        {
            start_audio_capture(
                audio.clone(),
                settings.audio_volume,
                settings.audio_device.clone(),
                stats.clone(),
                running.clone(),
            );
        }
    }

    // ---- capture thread --------------------------------------------------
    {
        let frames = frames.clone();
        let running = running.clone();
        let settings = settings.clone();
        let target = target.clone();
        let stats = stats.clone();
        let settings_path = settings_path.clone();

        thread::spawn(move || {
            let mut seq = 0u64;
            let mut last_refresh = Instant::now();
            let mut sticky: Option<engine::WindowRef> = None;
            let mut last_message = String::new();
            // Hot reload: the UI rewrites the session file when the user changes
            // resolution, FPS, bitrate or target while the session is running.
            let mut live = (*settings).clone();
            let mut last_mtime = std::fs::metadata(&settings_path)
                .and_then(|m| m.modified())
                .ok();
            let mut last_file_check = Instant::now();

            while running.load(Ordering::Relaxed) {
                let started = Instant::now();

                // Personne ne regarde : inutile de capturer et d'allouer 8 Mo par image.
                if stats.clients.load(Ordering::Relaxed) == 0 {
                    thread::sleep(Duration::from_millis(50));
                    continue;
                }

                if last_file_check.elapsed() > Duration::from_millis(500) {
                    last_file_check = Instant::now();
                    let mtime = std::fs::metadata(&settings_path)
                        .and_then(|m| m.modified())
                        .ok();
                    if mtime.is_some() && mtime != last_mtime {
                        last_mtime = mtime;
                        if let Some(mut reloaded) = StreamSettings::load_from(&settings_path) {
                            reloaded.clamp();
                            info!(
                                "Session rechargée : {}x{} @ {} FPS, {:.1} Mbps, cible '{}'",
                                reloaded.width,
                                reloaded.height,
                                reloaded.fps,
                                reloaded.bitrate_kbps as f32 / 1000.0,
                                if reloaded.target_process.is_empty() {
                                    reloaded.target_title.clone()
                                } else {
                                    reloaded.target_process.clone()
                                }
                            );
                            live = reloaded;
                            last_refresh = Instant::now() - Duration::from_secs(2);
                        }
                    }
                }

                let needs_refresh = last_refresh.elapsed() > Duration::from_secs(1)
                    || !engine::window_alive(&target.lock());
                if needs_refresh {
                    let resolved = engine::resolve_target(&live, sticky.as_ref());
                    let message = engine::describe(&resolved).2;
                    if message != last_message {
                        match &resolved {
                            engine::CaptureTarget::Window(w) => {
                                info!("Capture de la fenêtre '{}' [{}]", w.title, w.process)
                            }
                            engine::CaptureTarget::Desktop => {
                                warn!("Aucune fenêtre ciblée : capture de l'écran complet (mode secours).")
                            }
                            engine::CaptureTarget::None => warn!(
                                "Aucune fenêtre de jeu détectée : capture en attente, le bureau n'est pas diffusé."
                            ),
                        }
                        last_message = message;
                    }
                    if let engine::CaptureTarget::Window(w) = &resolved {
                        sticky = Some(w.clone());
                    }
                    *target.lock() = resolved;
                    last_refresh = Instant::now();
                }

                let (out_w, out_h) = {
                    let guard = target.lock();
                    match engine::source_size(&guard) {
                        // Never upscale above the native window size.
                        Some((sw, sh)) => (
                            (live.width.min(sw) / 2) * 2,
                            (live.height.min(sh) / 2) * 2,
                        ),
                        None => (live.width, live.height),
                    }
                };

                let grabbed = {
                    let guard = target.lock();
                    engine::grab(&guard, out_w, out_h)
                };

                if let Some((rgb, w, h)) = grabbed {
                    seq += 1;
                    frames.publish(
                        seq,
                        VideoFrame {
                            rgb,
                            width: w,
                            height: h,
                        },
                    );
                    stats.width.store(w, Ordering::Relaxed);
                    stats.height.store(h, Ordering::Relaxed);
                }

                let elapsed = started.elapsed();
                let budget = live.frame_interval();
                if elapsed < budget {
                    thread::sleep(budget - elapsed);
                }
            }
        });
    }

    // ---- status publisher ------------------------------------------------
    {
        let stats = stats.clone();
        let target = target.clone();
        let running = running.clone();
        thread::spawn(move || {
            let mut last_frames = 0u64;
            let mut last_bytes = 0u64;
            let mut last_at = Instant::now();
            while running.load(Ordering::Relaxed) {
                thread::sleep(Duration::from_millis(500));
                let frames_now = stats.frames_sent.load(Ordering::Relaxed);
                let bytes_now = stats.bytes_sent.load(Ordering::Relaxed);
                let elapsed = last_at.elapsed().as_secs_f64().max(0.2);
                let fps = (frames_now - last_frames) as f32 / elapsed as f32;
                let kbps = ((bytes_now - last_bytes) * 8) as f32 / elapsed as f32 / 1000.0;

                let (title, process, message, source) = {
                    let guard = target.lock();
                    let (title, process, message) = engine::describe(&guard);
                    (title, process, message, engine::source_size(&guard))
                };

                StreamStats {
                    running: true,
                    target_title: title,
                    target_process: process,
                    source_width: source.map(|(w, _)| w).unwrap_or(0),
                    source_height: source.map(|(_, h)| h).unwrap_or(0),
                    width: stats.width.load(Ordering::Relaxed),
                    height: stats.height.load(Ordering::Relaxed),
                    fps: stats.fps.load(Ordering::Relaxed).max(1),
                    measured_fps: fps,
                    measured_kbps: kbps as u32,
                    quality: stats.quality.load(Ordering::Relaxed),
                    dropped_frames: stats.dropped.load(Ordering::Relaxed),
                    clients: stats.clients.load(Ordering::Relaxed),
                    audio_active: stats.audio_ok.load(Ordering::Relaxed),
                    audio_device: stats.audio_device_name.lock().clone(),
                    audio_error: stats.audio_error.lock().clone(),
                    message: if codec == Codec::Jpeg {
                        format!("{} (JPEG)", message)
                    } else {
                        message
                    },
                    updated_ms: 0,
                }
                .publish();

                last_frames = frames_now;
                last_bytes = bytes_now;
                last_at = Instant::now();
            }
        });
    }

    // ---- stream listener -------------------------------------------------
    // Dual-stack first: on Windows a socket bound to [::] accepts IPv4 and IPv6, which is
    // what makes the IPv6 fallback work for hosts behind a shared (CGNAT) IPv4.
    let v6_addr = format!("[::]:{}", settings.stream_port);
    let v4_addr = format!("0.0.0.0:{}", settings.stream_port);
    let listener = match TcpListener::bind(&v6_addr) {
        Ok(l) => {
            info!(
                "Diffusion CoopLink en écoute sur {} (TCP, IPv4 + IPv6)",
                v6_addr
            );
            l
        }
        Err(e6) => match TcpListener::bind(&v4_addr) {
            Ok(l) => {
                info!("Diffusion CoopLink en écoute sur {} (TCP, IPv4 seul : {})", v4_addr, e6);
                l
            }
            Err(e) => {
                error!("Impossible d'écouter sur {} ni sur {} : {}", v6_addr, v4_addr, e);
                return;
            }
        },
    };

    // ---- relay fallback ---------------------------------------------------
    // When the guest cannot reach us directly (shared IPv4, no IPv6), we dial the
    // relay ourselves and serve that connection exactly like an inbound client.
    if settings.relay_enabled
        && !settings.relay_addr.trim().is_empty()
        && !settings.relay_room.trim().is_empty()
    {
        let relay_addr = settings.relay_addr.clone();
        let relay_room = settings.relay_room.clone();
        let settings_for_relay = settings.clone();
        let frames_r = frames.clone();
        let audio_r = audio.clone();
        let stats_r = stats.clone();
        let running_r = running.clone();
        let target_r = target.clone();
        info!(
            "Relais actif : {} (salon {}) — repli si la connexion directe échoue.",
            relay_addr, relay_room
        );
        thread::spawn(move || loop {
            if !running_r.load(Ordering::Relaxed) {
                break;
            }
            match TcpStream::connect(&relay_addr) {
                Ok(mut stream) => {
                    // Empêche le NAT de couper une socket muette pendant l'attente.
                    {
                        use socket2::{SockRef, TcpKeepalive};
                        let _ = SockRef::from(&stream).set_tcp_keepalive(
                            &TcpKeepalive::new()
                                .with_time(Duration::from_secs(30))
                                .with_interval(Duration::from_secs(10)),
                        );
                    }
                    let hello = format!("HELLO host {}\n", relay_room);
                    if stream.write_all(hello.as_bytes()).is_ok() {
                        info!("Relais connecté ({}), en attente d'un invité.", relay_addr);
                        handle_client(
                            stream,
                            settings_for_relay.clone(),
                            frames_r.clone(),
                            audio_r.clone(),
                            stats_r.clone(),
                            running_r.clone(),
                            target_r.clone(),
                            Duration::from_secs(1800),
                            codec,
                        );
                    }
                }
                Err(e) => warn!("Relais {} injoignable : {}", relay_addr, e),
            }
            thread::sleep(Duration::from_secs(10));
        });
    }

    for incoming in listener.incoming() {
        match incoming {
            Ok(stream) => {
                let settings = settings.clone();
                let frames = frames.clone();
                let audio = audio.clone();
                let stats = stats.clone();
                let running = running.clone();
                let target = target.clone();
                thread::spawn(move || {
                    handle_client(
                        stream,
                        settings,
                        frames,
                        audio,
                        stats,
                        running,
                        target,
                        Duration::from_secs(5),
                        codec,
                    )
                });
            }
            Err(e) => warn!("Connexion entrante refusée : {}", e),
        }
    }
}
