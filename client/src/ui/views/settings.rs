use crate::config::AppConfig;
use crate::i18n::Language;
use crate::ui::retro_theme::ModernTheme;
use egui::{Color32, RichText, Rounding, Stroke, Ui};

pub struct SettingsView;

impl SettingsView {
    pub fn render(ui: &mut Ui, config: &mut AppConfig, saved_notice: &mut bool) {
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                egui::Frame::none()
                    .fill(ModernTheme::CARD_BG)
                    .stroke(Stroke::new(1.0_f32, ModernTheme::BORDER_SUBTLE))
                    .inner_margin(18.0)
                    .rounding(Rounding::same(10.0))
                    .show(ui, |ui| {
                        ui.label(
                            RichText::new("Paramètres de l'application & Langue")
                                .color(ModernTheme::BLUE_ELECTRIC)
                                .strong()
                                .size(16.5),
                        );
                        ui.add_space(4.0);
                        ui.label(
                            RichText::new(
                                "Personnalisez la langue, le serveur de métriques (attribution d'ID unique) et les automatisations Windows.",
                            )
                            .color(ModernTheme::TEXT_SECONDARY),
                        );

                        ui.add_space(14.0);

                        egui::Grid::new("settings_grid")
                            .num_columns(2)
                            .spacing([24.0, 12.0])
                            .show(ui, |ui| {
                                ui.label(RichText::new("Langue de l'interface :").color(ModernTheme::TEXT_PRIMARY).strong());
                                egui::ComboBox::from_id_salt("settings_lang_combo")
                                    .selected_text(format!("{} ({})", config.language.display_name(), config.language.code()))
                                    .show_ui(ui, |ui| {
                                        for &lang in Language::all() {
                                            ui.selectable_value(
                                                &mut config.language,
                                                lang,
                                                format!("{} ({})", lang.display_name(), lang.code()),
                                            );
                                        }
                                    });
                                ui.end_row();

                                ui.label(RichText::new("Pseudo par défaut :").color(ModernTheme::TEXT_PRIMARY).strong());
                                ui.text_edit_singleline(&mut config.player_name);
                                ui.end_row();

                                ui.label(RichText::new("Serveur Central (ID & Métriques 30024) :").color(ModernTheme::TEXT_PRIMARY).strong());
                                ui.text_edit_singleline(&mut config.central_server_address);
                                ui.end_row();

                                ui.label(RichText::new("Port UDP P2P :").color(ModernTheme::TEXT_PRIMARY).strong());
                                let mut port_str = config.listen_port.to_string();
                                if ui.text_edit_singleline(&mut port_str).changed() {
                                    if let Ok(val) = port_str.parse::<u16>() {
                                        config.listen_port = val;
                                    }
                                }
                                ui.end_row();

                                ui.label(RichText::new("Sous-réseau virtuel :").color(ModernTheme::TEXT_PRIMARY).strong());
                                ui.text_edit_singleline(&mut config.virtual_subnet_prefix);
                                ui.end_row();

                                ui.label(RichText::new("Taille MTU :").color(ModernTheme::TEXT_PRIMARY).strong());
                                let mut mtu_str = config.mtu.to_string();
                                if ui.text_edit_singleline(&mut mtu_str).changed() {
                                    if let Ok(val) = mtu_str.parse::<u32>() {
                                        config.mtu = val;
                                    }
                                }
                                ui.end_row();
                            });

                        ui.add_space(14.0);
                        ui.separator();
                        ui.add_space(10.0);

                        ui.checkbox(
                            &mut config.auto_metric_optimization,
                            RichText::new("Appliquer automatiquement la métrique basse prioritaire (5) au lancement")
                                .color(ModernTheme::TEXT_PRIMARY),
                        );
                        ui.checkbox(
                            &mut config.auto_firewall_rule,
                            RichText::new("Autoriser automatiquement CoopLink dans le pare-feu Windows")
                                .color(ModernTheme::TEXT_PRIMARY),
                        );
                        ui.checkbox(
                            &mut config.enable_broadcast_reflector,
                            RichText::new("Activer la découverte automatique des salons LAN (Broadcast UDP)")
                                .color(ModernTheme::TEXT_PRIMARY),
                        );
                        ui.checkbox(
                            &mut config.enable_upnp,
                            RichText::new("Activer l'ouverture de port automatique UPnP sur la box")
                                .color(ModernTheme::TEXT_PRIMARY),
                        );

                        ui.add_space(16.0);
                        ui.horizontal(|ui| {
                            let save_btn = egui::Button::new(
                                RichText::new("Enregistrer les paramètres")
                                    .color(Color32::WHITE)
                                    .strong(),
                            )
                            .fill(ModernTheme::BLUE_ELECTRIC)
                            .rounding(Rounding::same(8.0));

                            if ui.add(save_btn).clicked() {
                                let _ = config.save();
                                *saved_notice = true;
                            }

                            if *saved_notice {
                                ModernTheme::status_pill(ui, ModernTheme::GREEN_VIVID, "Paramètres enregistrés");
                            }
                        });
                    });
            });
    }
}
