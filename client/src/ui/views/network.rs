use crate::config::AppConfig;
use crate::platform::windows::metric::get_network_interfaces;
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
    ) {
        ui.vertical(|ui| {
            // Carte principale Métrique Réseau
            egui::Frame::none()
                .fill(RetroTheme::PANEL_BG)
                .stroke(egui::Stroke::new(1.0_f32, RetroTheme::NEON_GREEN))
                .inner_margin(12.0)
                .rounding(4.0)
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.heading(
                            RichText::new("⚡ OPTIMISATION DE LA MÉTRIQUE RÉSEAU (LAN OVERRIDE)")
                                .color(RetroTheme::NEON_GREEN)
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
                                RichText::new("★ PRIORITÉ ULTRA-ÉLEVÉE (RECOMMANDÉ)")
                                    .color(RetroTheme::NEON_GREEN)
                                    .strong(),
                            );
                        } else {
                            ui.label(
                                RichText::new("ATTENTION: Métrique élevée, risque de conflit Wi-Fi")
                                    .color(RetroTheme::RETRO_AMBER),
                            );
                        }
                    });

                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        let btn = egui::Button::new(
                            RichText::new("🔧 APPLIQUER LA MÉTRIQUE BASSE IMMÉDIATEMENT")
                                .color(Color32::BLACK)
                                .strong(),
                        )
                        .fill(RetroTheme::NEON_GREEN);

                        if ui.add(btn).clicked() {
                            *on_apply_metric_clicked = true;
                        }

                        if state.metric_applied {
                            ui.label(
                                RichText::new("✔ Métrique appliquée avec succès !")
                                    .color(RetroTheme::NEON_GREEN)
                                    .strong(),
                            );
                        }
                    });
                });

            ui.add_space(15.0);

            // Relais de broadcast des jeux vidéo
            egui::Frame::none()
                .fill(RetroTheme::PANEL_BG)
                .stroke(egui::Stroke::new(1.0_f32, RetroTheme::NEON_CYAN))
                .inner_margin(12.0)
                .rounding(4.0)
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.heading(
                            RichText::new("📡 RÉPÉTEUR DE BROADCAST GAMING (UDP LAN)")
                                .color(RetroTheme::NEON_CYAN)
                                .strong(),
                        );
                    });
                    ui.add_space(6.0);

                    ui.label(
                        RichText::new(
                            "Intercepte et réplique en temps réel les trames de recherche de parties des moteurs \
                             de jeux (Blizzard 6112, Valve Source 27015, Unreal Engine 7777, Minecraft, etc.).",
                        )
                        .color(RetroTheme::TEXT_MUTED),
                    );

                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        ui.checkbox(
                            &mut config.enable_broadcast_reflector,
                            RichText::new("Activer le relais de découverte LAN automatique").color(Color32::WHITE),
                        );
                    });
                    ui.label(
                        RichText::new(format!(
                            "Paquets de découverte relayés cette session: {}",
                            state.broadcast_packets_relayed
                        ))
                        .color(RetroTheme::NEON_CYAN),
                    );
                });

            ui.add_space(15.0);

            // Liste des interfaces Windows détectées
            egui::Frame::none()
                .fill(RetroTheme::PANEL_BG)
                .stroke(egui::Stroke::new(1.0_f32, Color32::from_rgb(40, 50, 75)))
                .inner_margin(12.0)
                .rounding(4.0)
                .show(ui, |ui| {
                    ui.heading(
                        RichText::new("📋 INTERFACES RÉSEAU WINDOWS ACTIVES")
                            .color(RetroTheme::TEXT_PRIMARY)
                            .strong(),
                    );
                    ui.add_space(6.0);

                    let ifaces = get_network_interfaces();
                    if ifaces.is_empty() {
                        ui.label(
                            RichText::new("Analyse des interfaces réseau en cours...")
                                .color(RetroTheme::TEXT_MUTED)
                                .italics(),
                        );
                    } else {
                        egui::Grid::new("interfaces_grid")
                            .num_columns(3)
                            .spacing([25.0, 6.0])
                            .striped(true)
                            .show(ui, |ui| {
                                ui.label(RichText::new("INTERFACE").color(RetroTheme::NEON_CYAN).strong());
                                ui.label(RichText::new("MÉTRIQUE").color(RetroTheme::NEON_CYAN).strong());
                                ui.label(RichText::new("PRIORITÉ").color(RetroTheme::NEON_CYAN).strong());
                                ui.end_row();

                                for iface in ifaces {
                                    let is_coop = iface.name.contains("CoopLink");
                                    let name_text = if is_coop {
                                        RichText::new(&iface.name).color(RetroTheme::NEON_GREEN).strong()
                                    } else {
                                        RichText::new(&iface.name).color(Color32::WHITE)
                                    };

                                    ui.label(name_text);
                                    ui.label(
                                        RichText::new(format!("{}", iface.metric))
                                            .color(if iface.metric <= 10 {
                                                RetroTheme::NEON_GREEN
                                            } else {
                                                RetroTheme::TEXT_MUTED
                                            })
                                            .monospace(),
                                    );

                                    if iface.metric <= 10 {
                                        ui.label(RichText::new("HAUTE (AVANT BOX)").color(RetroTheme::NEON_GREEN));
                                    } else if iface.metric <= 30 {
                                        ui.label(RichText::new("NORMALE (ETHERNET)").color(RetroTheme::RETRO_AMBER));
                                    } else {
                                        ui.label(RichText::new("BASSE (WI-FI/DEFAULT)").color(Color32::GRAY));
                                    }
                                    ui.end_row();
                                }
                            });
                    }
                });
        });
    }
}
