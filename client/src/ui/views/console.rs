use crate::i18n::Language;
use crate::types::{LogEntry, LogLevel};
use crate::ui::retro_theme::ModernTheme;
use egui::{RichText, Rounding, ScrollArea, Stroke, Ui};

pub struct ConsoleView;

impl ConsoleView {
    pub fn render(ui: &mut Ui, lang: Language, logs: &[LogEntry], on_clear_clicked: &mut bool) {
        ui.vertical(|ui| {
            egui::Frame::none()
                .fill(ModernTheme::CARD_BG)
                .stroke(Stroke::new(1.0_f32, ModernTheme::BORDER_SUBTLE))
                .inner_margin(16.0)
                .rounding(Rounding::same(10.0))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new(match lang {
                                Language::Fr => "Journal d'activité réseau & partage de jeu",
                                Language::En => "Network & Game Sharing Activity Log",
                                Language::Es => "Registro de actividad de red y juego",
                                Language::De => "Netzwerk- & Spiel-Aktivitätsprotokoll",
                                Language::Pt => "Registo de atividade de rede e jogo",
                            })
                            .color(ModernTheme::BLUE_ELECTRIC)
                            .strong()
                            .size(16.0),
                        );

                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui
                                .button(match lang {
                                    Language::Fr => "Effacer l'historique",
                                    Language::En => "Clear Log",
                                    Language::Es => "Borrar historial",
                                    Language::De => "Verlauf löschen",
                                    Language::Pt => "Limpar histórico",
                                })
                                .clicked()
                            {
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
                                    RichText::new(match lang {
                                        Language::Fr => "Aucun événement enregistré pour le moment.",
                                        Language::En => "No activity events recorded yet.",
                                        Language::Es => "Ningún evento registrado por el momento.",
                                        Language::De => "Noch keine Ereignisse aufgezeichnet.",
                                        Language::Pt => "Nenhum evento registado de momento.",
                                    })
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
                                        LogLevel::Warning => (
                                            match lang {
                                                Language::Fr => "Alerte",
                                                Language::En => "Warn",
                                                _ => "Warn",
                                            },
                                            ModernTheme::ORANGE_CORAL,
                                        ),
                                        LogLevel::Error => (
                                            match lang {
                                                Language::Fr => "Erreur",
                                                Language::En => "Error",
                                                _ => "Error",
                                            },
                                            ModernTheme::RED_ERROR,
                                        ),
                                        LogLevel::Success => (
                                            match lang {
                                                Language::Fr => "Succès",
                                                Language::En => "Ready",
                                                _ => "OK",
                                            },
                                            ModernTheme::GREEN_VIVID,
                                        ),
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
