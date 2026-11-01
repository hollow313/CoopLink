// CoopLink - Windows 11 Graphical Setup Installer
// Copyright (c) 2026 Loic HUON. All Rights Reserved.
// Proprietary Commercial Software.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod actions;
mod wizard;

use eframe::egui;
use wizard::InstallerApp;

fn main() -> eframe::Result<()> {
    let mut vp = egui::ViewportBuilder::default()
        .with_title("Installation de CoopLink")
        .with_inner_size([720.0, 560.0])
        .with_min_inner_size([720.0, 560.0])
        .with_resizable(false)
        .with_maximize_button(false)
        .with_active(true)
        .with_decorations(true);

    if let Ok(img) = image::load_from_memory(include_bytes!("../../../images/CoopLink.png")) {
        let rgba = img.into_rgba8();
        let (width, height) = rgba.dimensions();
        vp = vp.with_icon(egui::IconData {
            rgba: rgba.into_raw(),
            width,
            height,
        });
    }

    let native_options = eframe::NativeOptions {
        viewport: vp,
        ..Default::default()
    };

    eframe::run_native(
        "CoopLink Installer",
        native_options,
        Box::new(|cc| Ok(Box::new(InstallerApp::new(cc)))),
    )
}
