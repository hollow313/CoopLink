use crate::config::AppConfig;
use crate::types::AppState;
use crate::ui::retro_theme::RetroTheme;
use egui::{Color32, RichText, Ui};

pub struct NetworkView;

impl NetworkView {
    pub fn render(
        ui: &mut Ui,
        state: &mut AppState,
        config: &mut AppConfig,
        on_apply_metric_clicked: &mut bool,
        on_refresh_interfaces_clicked: &mut bool,
    ) {
        ui.vertical(|ui| {
            // Carte principale Métrique Réseau (Dark Carbon 2026)
            egui::Frame::none()
                .fill(RetroTheme::PANEL_BG)
                .stroke(egui::Stroke::new(1.0_f32, RetroTheme::GREEN_VIVID))
                .inner_margin(14.0)
                .rounding(7.0)
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.heading(
                            RichText::new("⚡ OPTIMISATION DE LA MÉTRIQUE RÉSEAU (LAN OVERRIDE)")
                                .color(RetroTheme::GREEN_VIVID)
                                .strong(),
                        );
                    });
                    ui.add_space(6.0);

                    ui.label(
                        RichText::new(
                            "Pour faire croire aux jeux vidéo que vos amis sont sur votre box locale, les paquets de \
                             découverte UDP (255.255.255.255) doivent impérativement sortir par l'interface CoopLink \
                             AVANT la carte Wi-Fi ou Ethernet. Une métrique basse (ex: 5) donne une priorité absolue \
                             au réseau CoopLink.",
                        )
                        .color(RetroTheme::TEXT_MUTED),
                    );

                    ui.add_space(10.0);

                    ui.horizontal(|ui| {
                        ui.label(RichText::new("MÉTRIQUE CIBLE:").color(RetroTheme::TEXT_PRIMARY).strong());
                        ui.add(egui::Slider::new(&mut config.target_metric, 1..=50).text(""));

                        if config.target_metric <= 10 {
                            ui.label(
                                RichText::new("🚀 PRIORITÉ ABSOLUE (RECOMMANDÉ)")
                                    .color(RetroTheme::GREEN_VIVID)
                                    .strong(),
                            );
                        } else {
                            ui.label(
                                RichText::new("⚠ Risque de passer après le Wi-Fi")
                                    .color(RetroTheme::ORANGE_CORAL),
                            );
                        }
                    });

                    ui.add_space(10.0);

                    ui.horizontal(|ui| {
                        let btn = egui::Button::new(
                            RichText::new("APPLIQUER LA MÉTRIQUE PRIORITAIRE (5)")
                                .color(Color32::BLACK)
                                .strong(),
                        )
                        .fill(RetroTheme::GREEN_VIVID);

                        if ui.add(btn).clicked() {
                            *on_apply_metric_clicked = true;
                        }

                        if state.metric_applied {
                            ui.label(
                                RichText::new("✔ Priorité active : Vos paquets de jeu passent avant la box !")
                                    .color(RetroTheme::GREEN_VIVID)
                                    .strong(),
                            );
                        } else {
                            ui.label(
                                RichText::new("Métrique en attente d'application")
                                    .color(RetroTheme::TEXT_MUTED),
                            );
                        }
                    });
                });

            ui.add_space(15.0);

            // Carte Relais Broadcast UDP
            egui::Frame::none()
                .fill(RetroTheme::PANEL_BG)
                .stroke(egui::Stroke::new(1.0_f32, RetroTheme::CYAN_VIBRANT))
                .inner_margin(14.0)
                .rounding(7.0)
                .show(ui, |ui| {
                    ui.heading(
                        RichText::new("📡 DÉCOUVERTE BROADCAST LAN (JEUX VIDÉO)")
                            .color(RetroTheme::CYAN_VIBRANT)
                            .strong(),
                    );
                    ui.add_space(6.0);

                    ui.label(
                        RichText::new(
                            "Le module Broadcast Relay écoute les diffusions UDP sur les ports gaming standards \
                             (Source Engine, Warcraft III, Unreal, Minecraft, LAN DirectPlay...) et les injecte \
                             instantanément dans le tunnel CoopLink pour que la partie apparaisse dans la liste locale.",
                        )
                        .color(RetroTheme::TEXT_MUTED),
                    );

                    ui.add_space(8.0);

                    ui.label(
                        RichText::new(format!(
                            "Paquets de découverte relayés cette session: {}",
                            state.broadcast_packets_relayed
                        ))
                        .color(RetroTheme::CYAN_VIBRANT),
                    );
                });

            ui.add_space(15.0);

            // Liste des interfaces Windows détectées (Chargement 100% asynchrone sans gel UI)
            egui::Frame::none()
                .fill(RetroTheme::PANEL_BG)
                .stroke(egui::Stroke::new(1.0_f32, RetroTheme::BORDER_SUBTLE))
                .inner_margin(14.0)
                .rounding(7.0)
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.heading(
                            RichText::new("📋 INTERFACES RÉSEAU WINDOWS ACTIVES")
                                .color(RetroTheme::TEXT_PRIMARY)
                                .strong(),
                        );

                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if state.is_refreshing_interfaces {
                                ui.spinner();
                                ui.label(RichText::new("Scan en cours...").color(RetroTheme::CYAN_VIBRANT).small());
                            } else if ui.button("🔄 Actualiser les interfaces").clicked() {
                                *on_refresh_interfaces_clicked = true;
                            }
                        });
                    });
                    ui.add_space(8.0);

                    // Déclenchement automatique en arrière-plan au premier affichage si vide
                    if state.cached_interfaces.is_empty() && !state.is_refreshing_interfaces {
                        *on_refresh_interfaces_clicked = true;
                    }

                    if state.is_refreshing_interfaces && state.cached_interfaces.is_empty() {
                        ui.horizontal(|ui| {
                            ui.spinner();
                            ui.label(
                                RichText::new("Analyse des cartes réseau Windows en tâche de fond...")
                                    .color(RetroTheme::CYAN_VIBRANT)
                                    .italics(),
                            );
                        });
                    } else if state.cached_interfaces.is_empty() {
                        ui.label(
                            RichText::new("Aucune interface détectée pour le moment.")
                                .color(RetroTheme::TEXT_MUTED)
                                .italics(),
                        );
                    } else {
                        egui::Grid::new("interfaces_grid")
                            .num_columns(3)
                            .spacing([25.0, 8.0])
                            .striped(true)
                            .show(ui, |ui| {
                                ui.label(RichText::new("INTERFACE").color(RetroTheme::CYAN_VIBRANT).strong());
                                ui.label(RichText::new("MÉTRIQUE").color(RetroTheme::CYAN_VIBRANT).strong());
                                ui.label(RichText::new("PRIORITÉ").color(RetroTheme::CYAN_VIBRANT).strong());
                                ui.end_row();

                                for iface in &state.cached_interfaces {
                                    let is_coop = iface.name.contains("CoopLink");
                                    let name_text = if is_coop {
                                        RichText::new(&iface.name).color(RetroTheme::GREEN_VIVID).strong()
                                    } else {
                                        RichText::new(&iface.name).color(RetroTheme::TEXT_PRIMARY)
                                    };

                                    ui.label(name_text);
                                    ui.label(
                                        RichText::new(format!("{}", iface.metric))
                                            .color(if iface.metric <= 10 {
                                                RetroTheme::GREEN_VIVID
                                            } else {
                                                RetroTheme::TEXT_MUTED
                                            })
                                            .monospace(),
                                    );

                                    if iface.metric <= 10 {
                                        ui.label(RichText::new("HAUTE (AVANT BOX)").color(RetroTheme::GREEN_VIVID));
                                    } else if iface.metric <= 30 {
                                        ui.label(RichText::new("NORMALE (ETHERNET)").color(RetroTheme::ORANGE_CORAL));
                                    } else {
                                        ui.label(RichText::new("BASSE (WI-FI/DEFAULT)").color(RetroTheme::TEXT_MUTED));
                                    }
                                    ui.end_row();
                                }
                            });
                    }
                });
        });
    }
}
