use crate::types::{LogEntry, LogLevel};
use crate::ui::retro_theme::ModernTheme;
use egui::{RichText, Rounding, ScrollArea, Stroke, Ui};

pub struct ConsoleView;

impl ConsoleView {
    pub fn render(ui: &mut Ui, logs: &[LogEntry], on_clear_clicked: &mut bool) {
        ui.vertical(|ui| {
            egui::Frame::none()
                .fill(ModernTheme::CARD_BG)
                .stroke(Stroke::new(1.0_f32, ModernTheme::BORDER_SUBTLE))
                .inner_margin(16.0)
                .rounding(Rounding::same(10.0))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new("Journal d'activité réseau & streaming")
                                .color(ModernTheme::BLUE_ELECTRIC)
                                .strong()
                                .size(16.0),
                        );

                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.button("Effacer l'historique").clicked() {
                                *on_clear_clicked = true;
                            }
                        });
                    });

                    ui.add_space(8.0);
                    ui.separator();
                    ui.add_space(6.0);

                    ScrollArea::vertical()
                        .max_height(370.0)
                        .auto_shrink([false, false])
                        .stick_to_bottom(true)
                        .show(ui, |ui| {
                            if logs.is_empty() {
                                ui.label(
                                    RichText::new("Aucun événement enregistré pour le moment.")
                                        .color(ModernTheme::TEXT_MUTED),
                                );
                            }

                            for entry in logs {
                                ui.horizontal(|ui| {
                                    ui.label(
                                        RichText::new(&entry.timestamp)
                                            .color(ModernTheme::TEXT_MUTED)
                                            .small(),
                                    );

                                    let (tag, color) = match entry.level {
                                        LogLevel::Info => ("Info", ModernTheme::BLUE_ELECTRIC),
                                        LogLevel::Warning => ("Alerte", ModernTheme::ORANGE_CORAL),
                                        LogLevel::Error => ("Erreur", ModernTheme::RED_ERROR),
                                        LogLevel::Success => ("Succès", ModernTheme::GREEN_VIVID),
                                        LogLevel::Game => ("Session", ModernTheme::INDIGO_ACCENT),
                                    };

                                    ui.label(RichText::new(tag).color(color).strong().small());
                                    ui.label(
                                        RichText::new(&entry.message)
                                            .color(ModernTheme::TEXT_PRIMARY),
                                    );
                                });
                                ui.add_space(2.0);
                            }
                        });
                });
        });
    }
}
