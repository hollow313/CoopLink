#[cfg(target_os = "windows")]
pub mod windows;

#[cfg(target_os = "linux")]
pub mod linux {
    // Stub préparatoire pour Linux dans une feature future
    pub fn setup_metric(_name: &str, _metric: u32) -> anyhow::Result<()> {
        Ok(())
    }
}

#[cfg(target_os = "macos")]
pub mod macos {
    // Stub préparatoire pour macOS dans une feature future
    pub fn setup_metric(_name: &str, _metric: u32) -> anyhow::Result<()> {
        Ok(())
    }
}
