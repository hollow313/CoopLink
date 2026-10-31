use crate::config::AppConfig;
use crate::types::AppState;
use crate::ui::retro_theme::ModernTheme;
use egui::{Color32, RichText, Rounding, Stroke, Ui};

pub struct NetworkView;

impl NetworkView {
    pub fn render(
        ui: &mut Ui,
        state: &mut AppState,
        config: &mut AppConfig,
        on_apply_metric_clicked: &mut bool,
        on_refresh_interfaces_clicked: &mut bool,
    ) {
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                // Carte principale Métrique Réseau
                egui::Frame::none()
                    .fill(ModernTheme::CARD_BG)
                    .stroke(Stroke::new(1.0_f32, ModernTheme::BORDER_SUBTLE))
                    .inner_margin(16.0)
                    .rounding(Rounding::same(10.0))
                    .show(ui, |ui| {
                        ui.label(
                            RichText::new("Priorité de l'interface réseau (Métrique basse)")
                                .color(ModernTheme::BLUE_ELECTRIC)
                                .strong()
                                .size(16.0),
                        );
                        ui.add_space(6.0);

                        ui.label(
                            RichText::new(
                                "Une métrique basse (valeur 5 recommandée) place l'interface virtuelle CoopLink en priorité \
                                 devant votre carte Wi-Fi ou votre box internet pour la découverte des jeux locaux.",
                            )
                            .color(ModernTheme::TEXT_SECONDARY),
                        );

                        ui.add_space(12.0);

                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new("Métrique cible :")
                                    .color(ModernTheme::TEXT_PRIMARY)
                                    .strong(),
                            );
                            ui.add(egui::Slider::new(&mut config.target_metric, 1..=50).text(""));

                            if config.target_metric <= 10 {
                                ui.label(
                                    RichText::new("✔ Priorité maximale (passe avant la box)")
                                        .color(ModernTheme::GREEN_VIVID)
                                        .strong(),
                                );
                            } else {
                                ui.label(
                                    RichText::new("⚠ Valeur élevée (risque de passer après le Wi-Fi)")
                                        .color(ModernTheme::ORANGE_CORAL),
                                );
                            }
                        });

                        ui.add_space(10.0);

                        ui.horizontal(|ui| {
                            let btn = egui::Button::new(
                                RichText::new("Appliquer la métrique prioritaire")
                                    .color(Color32::WHITE)
                                    .strong(),
                            )
                            .fill(ModernTheme::BLUE_ELECTRIC)
                            .rounding(Rounding::same(8.0));

                            if ui.add(btn).clicked() {
                                *on_apply_metric_clicked = true;
                            }

                            if state.metric_applied {
                                ui.label(
                                    RichText::new("✔ Métrique prioritaire active sur le système")
                                        .color(ModernTheme::GREEN_VIVID)
                                        .strong(),
                                );
                            }
                        });
                    });

                ui.add_space(12.0);

                // Carte Relais Broadcast UDP
                egui::Frame::none()
                    .fill(ModernTheme::CARD_BG)
                    .stroke(Stroke::new(1.0_f32, ModernTheme::BORDER_SUBTLE))
                    .inner_margin(16.0)
                    .rounding(Rounding::same(10.0))
                    .show(ui, |ui| {
                        ui.label(
                            RichText::new("Découverte automatique des parties LAN (Broadcast UDP P2P)")
                                .color(ModernTheme::GREEN_VIVID)
                                .strong()
                                .size(16.0),
                        );
                        ui.add_space(4.0);

                        ui.label(
                            RichText::new(
                                " Capture et achemine en P2P direct les annonces de salons locaux (255.255.255.255) vers vos amis connectés.",
                            )
                            .color(ModernTheme::TEXT_SECONDARY),
                        );

                        ui.add_space(8.0);
                        ui.label(
                            RichText::new(format!(
                                "Paquets de découverte LAN acheminés : {}",
                                state.broadcast_packets_relayed
                            ))
                            .color(ModernTheme::BLUE_ELECTRIC)
                            .strong(),
                        );
                    });

                ui.add_space(12.0);

                // Liste des interfaces Windows détectées (Asynchrone)
                egui::Frame::none()
                    .fill(ModernTheme::CARD_BG)
                    .stroke(Stroke::new(1.0_f32, ModernTheme::BORDER_SUBTLE))
                    .inner_margin(16.0)
                    .rounding(Rounding::same(10.0))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new("Cartes réseau Windows détectées")
                                    .color(ModernTheme::TEXT_PRIMARY)
                                    .strong()
                                    .size(16.0),
                            );

                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                if state.is_refreshing_interfaces {
                                    ui.spinner();
                                    ui.label(
                                        RichText::new("Actualisation...")
                                            .color(ModernTheme::BLUE_ELECTRIC)
                                            .small(),
                                    );
                                } else if ui.button("Actualiser la liste").clicked() {
                                    *on_refresh_interfaces_clicked = true;
                                }
                            });
                        });
                        ui.add_space(8.0);

                        if state.cached_interfaces.is_empty() && !state.is_refreshing_interfaces {
                            *on_refresh_interfaces_clicked = true;
                        }

                        if state.is_refreshing_interfaces && state.cached_interfaces.is_empty() {
                            ui.horizontal(|ui| {
                                ui.spinner();
                                ui.label(
                                    RichText::new("Analyse des interfaces réseau en tâche de fond...")
                                        .color(ModernTheme::TEXT_SECONDARY),
                                );
                            });
                        } else if state.cached_interfaces.is_empty() {
                            ui.label(
                                RichText::new("Aucune interface détectée.")
                                    .color(ModernTheme::TEXT_MUTED),
                            );
                        } else {
                            egui::Grid::new("interfaces_grid")
                                .num_columns(3)
                                .spacing([28.0, 8.0])
                                .striped(true)
                                .show(ui, |ui| {
                                    ui.label(RichText::new("INTERFACE").color(ModernTheme::BLUE_ELECTRIC).strong());
                                    ui.label(RichText::new("MÉTRIQUE").color(ModernTheme::BLUE_ELECTRIC).strong());
                                    ui.label(RichText::new("PRIORITÉ").color(ModernTheme::BLUE_ELECTRIC).strong());
                                    ui.end_row();

                                    for iface in &state.cached_interfaces {
                                        let is_coop = iface.name.contains("CoopLink");
                                        let name_text = if is_coop {
                                            RichText::new(&iface.name).color(ModernTheme::GREEN_VIVID).strong()
                                        } else {
                                            RichText::new(&iface.name).color(ModernTheme::TEXT_PRIMARY)
                                        };

                                        ui.label(name_text);
                                        ui.label(
                                            RichText::new(format!("{}", iface.metric))
                                                .color(if iface.metric <= 10 {
                                                    ModernTheme::GREEN_VIVID
                                                } else {
                                                    ModernTheme::TEXT_SECONDARY
                                                })
                                                .strong(),
                                        );

                                        if iface.metric <= 10 {
                                            ui.label(RichText::new("Haute (Prioritaire)").color(ModernTheme::GREEN_VIVID));
                                        } else if iface.metric <= 30 {
                                            ui.label(RichText::new("Normale (Ethernet)").color(ModernTheme::BLUE_ELECTRIC));
                                        } else {
                                            ui.label(RichText::new("Standard (Wi-Fi)").color(ModernTheme::TEXT_MUTED));
                                        }
                                        ui.end_row();
                                    }
                                });
                        }
                    });
            });
    }
}
