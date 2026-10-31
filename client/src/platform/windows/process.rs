use std::io::Result;
use std::os::windows::process::CommandExt;
use std::process::{Command, Output};

pub const CREATE_NO_WINDOW: u32 = 0x08000000;

/// Exécute une commande de façon 100% invisible en arrière-plan (aucun terminal, aucune console CMD)
pub fn run_hidden(program: &str, args: &[&str]) -> Result<Output> {
    let mut cmd = Command::new(program);
    cmd.args(args);
    cmd.creation_flags(CREATE_NO_WINDOW);
    cmd.output()
}

/// Exécute une commande PowerShell sans aucune fenêtre visible
pub fn run_hidden_powershell(script: &str) -> Result<Output> {
    run_hidden(
        "powershell",
        &["-NoProfile", "-NonInteractive", "-WindowStyle", "Hidden", "-Command", script],
    )
}
