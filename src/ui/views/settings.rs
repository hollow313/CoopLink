use crate::config::AppConfig;
use crate::ui::retro_theme::RetroTheme;
use egui::{Color32, RichText, Ui};

pub struct SettingsView;

impl SettingsView {
    pub fn render(ui: &mut Ui, config: &mut AppConfig, saved_notice: &mut bool) {
        ui.vertical(|ui| {
            egui::Frame::none()
                .fill(RetroTheme::PANEL_BG)
                .stroke(egui::Stroke::new(1.0_f32, RetroTheme::RETRO_AMBER))
                .inner_margin(12.0)
                .rounding(4.0)
                .show(ui, |ui| {
                    ui.heading(
                        RichText::new("⚙ ADMINISTRATION & PARAMÈTRES AVANCÉS")
                            .color(RetroTheme::RETRO_AMBER)
                            .strong(),
                    );
                    ui.add_space(4.0);
                    ui.label(
                        RichText::new("Configurez le comportement réseau, les ports et les optimisations de CoopLink.")
                            .color(RetroTheme::TEXT_MUTED),
                    );

                    ui.add_space(12.0);

                    egui::Grid::new("settings_grid")
                        .num_columns(2)
                        .spacing([20.0, 10.0])
                        .show(ui, |ui| {
                            // Pseudo
                            ui.label(RichText::new("PSEUDO JOUEUR:").color(RetroTheme::TEXT_PRIMARY).strong());
                            ui.text_edit_singleline(&mut config.player_name);
                            ui.end_row();

                            // Port UDP
                            ui.label(RichText::new("PORT UDP D'ÉCOUTE:").color(RetroTheme::TEXT_PRIMARY).strong());
                            let mut port_str = config.listen_port.to_string();
                            if ui.text_edit_singleline(&mut port_str).changed() {
                                if let Ok(val) = port_str.parse::<u16>() {
                                    config.listen_port = val;
                                }
                            }
                            ui.end_row();

                            // Préfixe IP Virtuelle
                            ui.label(RichText::new("SOUS-RÉSEAU VIRTUEL (CIDR /24):").color(RetroTheme::TEXT_PRIMARY).strong());
                            ui.text_edit_singleline(&mut config.virtual_subnet_prefix);
                            ui.end_row();

                            // MTU
                            ui.label(RichText::new("TAILLE MTU DU TUNNEL:").color(RetroTheme::TEXT_PRIMARY).strong());
                            let mut mtu_str = config.mtu.to_string();
                            if ui.text_edit_singleline(&mut mtu_str).changed() {
                                if let Ok(val) = mtu_str.parse::<u32>() {
                                    config.mtu = val;
                                }
                            }
                            ui.end_row();

                            // Métrique par défaut
                            ui.label(RichText::new("MÉTRIQUE PAR DÉFAUT:").color(RetroTheme::TEXT_PRIMARY).strong());
                            ui.add(egui::Slider::new(&mut config.target_metric, 1..=50));
                            ui.end_row();

                            // Mot de passe de salon
                            ui.label(RichText::new("MOT DE PASSE SALON (OPTIONNEL):").color(RetroTheme::TEXT_PRIMARY).strong());
                            ui.text_edit_singleline(&mut config.room_password);
                            ui.end_row();
                        });

                    ui.add_space(12.0);
                    ui.separator();
                    ui.add_space(8.0);

                    // Options automatiques
                    ui.checkbox(
                        &mut config.auto_metric_optimization,
                        RichText::new("Appliquer automatiquement la métrique basse au démarrage")
                            .color(Color32::WHITE),
                    );
                    ui.checkbox(
                        &mut config.auto_firewall_rule,
                        RichText::new("Vérifier et injecter automatiquement les règles pare-feu au lancement")
                            .color(Color32::WHITE),
                    );
                    ui.checkbox(
                        &mut config.enable_broadcast_reflector,
                        RichText::new("Activer le réflecteur de broadcast pour les jeux Blizzard/Valve/Unreal")
                            .color(Color32::WHITE),
                    );

                    ui.add_space(15.0);
                    ui.horizontal(|ui| {
                        let save_btn = egui::Button::new(
                            RichText::new("💾 ENREGISTRER LES PARAMÈTRES")
                                .color(Color32::BLACK)
                                .strong(),
                        )
                        .fill(RetroTheme::RETRO_AMBER);

                        if ui.add(save_btn).clicked() {
                            let _ = config.save();
                            *saved_notice = true;
                        }

                        if *saved_notice {
                            ui.label(
                                RichText::new("✔ Paramètres enregistrés avec succès !")
                                    .color(RetroTheme::NEON_GREEN)
                                    .strong(),
                            );
                        }
                    });
                });
        });
    }
}
