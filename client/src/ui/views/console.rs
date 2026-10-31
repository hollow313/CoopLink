use crate::types::{LogEntry, LogLevel};
use crate::ui::retro_theme::RetroTheme;
use egui::{Color32, RichText, ScrollArea, Ui};

pub struct ConsoleView;

impl ConsoleView {
    pub fn render(ui: &mut Ui, logs: &[LogEntry], on_clear_clicked: &mut bool) {
        ui.vertical(|ui| {
            egui::Frame::none()
                .fill(RetroTheme::BG_DARK)
                .stroke(egui::Stroke::new(1.0_f32, RetroTheme::GREEN_VIVID))
                .inner_margin(12.0)
                .rounding(7.0)
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.heading(
                            RichText::new("📟 TERMINAL SYSTÈME & ÉVÉNEMENTS RÉSEAU")
                                .color(RetroTheme::GREEN_VIVID)
                                .monospace()
                                .strong(),
                        );

                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.button("🗑 Effacer").clicked() {
                                *on_clear_clicked = true;
                            }
                        });
                    });

                    ui.add_space(8.0);
                    ui.separator();
                    ui.add_space(6.0);

                    ScrollArea::vertical()
                        .max_height(350.0)
                        .auto_shrink([false, false])
                        .stick_to_bottom(true)
                        .show(ui, |ui| {
                            if logs.is_empty() {
                                ui.label(
                                    RichText::new("[SYSTEM] Prêt. En attente d'événements réseau...")
                                        .color(RetroTheme::TEXT_MUTED)
                                        .monospace(),
                                );
                            }

                            for entry in logs {
                                ui.horizontal(|ui| {
                                    // Timestamp
                                    ui.label(
                                        RichText::new(format!("[{}]", entry.timestamp))
                                            .color(RetroTheme::TEXT_MUTED)
                                            .monospace(),
                                    );

                                    // Badge & Message
                                    let (tag, color) = match entry.level {
                                        LogLevel::Info => ("[INFO]", RetroTheme::NEON_CYAN),
                                        LogLevel::Warning => ("[WARN]", RetroTheme::RETRO_AMBER),
                                        LogLevel::Error => ("[FAIL]", RetroTheme::RETRO_RED),
                                        LogLevel::Success => ("[OK]", RetroTheme::NEON_GREEN),
                                        LogLevel::Game => ("[GAME]", RetroTheme::NEON_MAGENTA),
                                    };

                                    ui.label(RichText::new(tag).color(color).monospace().strong());
                                    ui.label(RichText::new(&entry.message).color(Color32::WHITE).monospace());
                                });
                            }
                        });
                });
        });
    }
}
