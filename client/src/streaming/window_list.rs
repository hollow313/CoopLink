//! Read-only enumeration of visible top-level windows, used by the UI picker.
//!
//! The capture service has its own copy of this logic in its `engine` module
//! (it also needs geometry and process images at injection time). This module
//! only answers "what can I share right now?", with the process name attached so
//! the selection survives a title change.

use serde::{Deserialize, Serialize};

/// A shareable window: what the user sees, and what identifies it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WindowEntry {
    /// Window title, as shown in the task bar.
    pub title: String,
    /// Image name of the owning process (`Game.exe`), empty when not readable.
    pub process: String,
}

impl WindowEntry {
    /// `Cyberpunk 2077 [Cyberpunk2077.exe]`
    pub fn label(&self) -> String {
        if self.process.is_empty() {
            self.title.clone()
        } else {
            format!("{} [{}]", self.title, self.process)
        }
    }
}

/// Classes that belong to the Windows shell, never to a game.
#[cfg(windows)]
const SHELL_CLASSES: [&str; 6] = [
    "Progman",
    "WorkerW",
    "Shell_TrayWnd",
    "Shell_SecondaryTrayWnd",
    "Windows.UI.Core.CoreWindow",
    "ApplicationFrameWindow",
];

#[cfg(windows)]
pub fn visible_windows() -> Vec<WindowEntry> {
    use std::ffi::c_void;
    use windows_sys::Win32::Foundation::{BOOL, HANDLE, HWND, LPARAM};
    use windows_sys::Win32::System::Threading::{
        OpenProcess, QueryFullProcessImageNameW, PROCESS_QUERY_LIMITED_INFORMATION,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        EnumWindows, GetClassNameW, GetWindowTextW, GetWindowThreadProcessId, IsIconic,
        IsWindowVisible,
    };

    unsafe extern "system" fn collect(hwnd: HWND, lparam: LPARAM) -> BOOL {
        let out = &mut *(lparam as *mut Vec<WindowEntry>);
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
        let process = process_image_name(pid);
        if process.is_empty() || process.eq_ignore_ascii_case("cooplink.exe") {
            return 1;
        }

        out.push(WindowEntry { title, process });
        1
    }

    unsafe fn process_image_name(pid: u32) -> String {
        if pid == 0 {
            return String::new();
        }
        let handle: HANDLE = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if handle == 0 {
            return String::new();
        }
        let mut buf = [0u16; 1024];
        let mut size = buf.len() as u32;
        let ok = QueryFullProcessImageNameW(handle, 0, buf.as_mut_ptr(), &mut size);
        windows_sys::Win32::Foundation::CloseHandle(handle);
        if ok == 0 {
            return String::new();
        }
        let full = String::from_utf16_lossy(&buf[..size as usize]);
        full.rsplit(['\\', '/']).next().unwrap_or("").to_string()
    }

    let mut found: Vec<WindowEntry> = Vec::new();
    unsafe {
        EnumWindows(Some(collect), &mut found as *mut _ as LPARAM);
    }
    found.sort_by(|a, b| a.title.to_lowercase().cmp(&b.title.to_lowercase()));
    found.dedup_by(|a, b| a.process.eq_ignore_ascii_case(&b.process) && a.title == b.title);
    let _ = std::mem::size_of::<c_void>();
    found
}

#[cfg(not(windows))]
pub fn visible_windows() -> Vec<WindowEntry> {
    Vec::new()
}
