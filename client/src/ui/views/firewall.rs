use crate::config::AppConfig;
use crate::types::AppState;
use crate::ui::retro_theme::ModernTheme;
use egui::{Color32, RichText, Rounding, Stroke, Ui};

pub struct FirewallView;

impl FirewallView {
    pub fn render(
        ui: &mut Ui,
        state: &mut AppState,
        config: &mut AppConfig,
        on_repair_firewall_clicked: &mut bool,
    ) {
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                egui::Frame::none()
                    .fill(ModernTheme::CARD_BG)
                    .stroke(Stroke::new(1.0_f32, ModernTheme::BORDER_SUBTLE))
                    .inner_margin(16.0)
                    .rounding(Rounding::same(10.0))
                    .show(ui, |ui| {
                        ui.label(
                            RichText::new("Sécurité & Pare-feu Windows Defender")
                                .color(ModernTheme::BLUE_ELECTRIC)
                                .strong()
                                .size(16.0),
                        );
                        ui.add_space(6.0);

                        ui.label(
                            RichText::new(
                                "CoopLink configure automatiquement les règles d'autorisation dans le pare-feu Windows \
                                 afin d'autoriser le maillage P2P (10.244.0.0/24) et le partage d'écran coopératif sans coupure.",
                            )
                            .color(ModernTheme::TEXT_SECONDARY),
                        );

                        ui.add_space(12.0);

                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new("État des règles :")
                                    .color(ModernTheme::TEXT_PRIMARY)
                                    .strong(),
                            );

                            if state.firewall_ok {
                                ModernTheme::status_pill(ui, ModernTheme::GREEN_VIVID, "Configuré et opérationnel");
                            } else {
                                ModernTheme::status_pill(ui, ModernTheme::ORANGE_CORAL, "Configuration recommandée");
                            }
                        });

                        ui.add_space(12.0);

                        ui.horizontal(|ui| {
                            let btn = egui::Button::new(
                                RichText::new("Appliquer / Réparer les règles pare-feu")
                                    .color(Color32::WHITE)
                                    .strong(),
                            )
                            .fill(ModernTheme::BLUE_ELECTRIC)
                            .rounding(Rounding::same(8.0));

                            if ui.add(btn).clicked() {
                                *on_repair_firewall_clicked = true;
                            }
                        });
                    });

                ui.add_space(12.0);

                egui::Frame::none()
                    .fill(ModernTheme::CARD_BG)
                    .stroke(Stroke::new(1.0_f32, ModernTheme::BORDER_SUBTLE))
                    .inner_margin(16.0)
                    .rounding(Rounding::same(10.0))
                    .show(ui, |ui| {
                        ui.label(
                            RichText::new("Règles réseau enregistrées")
                                .color(ModernTheme::TEXT_PRIMARY)
                                .strong()
                                .size(15.5),
                        );
                        ui.add_space(8.0);

                        render_rule_item(
                            ui,
                            "CoopLink — Tunnel P2P Direct (UDP/TCP)",
                            "Autorise les flux directs entre pairs sans passer par le serveur central.",
                        );
                        render_rule_item(
                            ui,
                            &format!("CoopLink — Sous-réseau Virtuel ({}.0/24)", config.virtual_subnet_prefix),
                            "Autorise le trafic LAN et le partage d'écran Co-op strictement sur l'interface virtuelle.",
                        );
                        render_rule_item(
                            ui,
                            "CoopLink — Découverte LAN & Manettes Multi-Joueurs",
                            "Autorise la découverte des salons locaux et la synchronisation basse latence des manettes.",
                        );
                    });
            });
    }
}

fn render_rule_item(ui: &mut Ui, title: &str, desc: &str) {
    ui.horizontal(|ui| {
        ModernTheme::draw_dot(ui, ModernTheme::GREEN_VIVID, 4.0);
        ui.vertical(|ui| {
            ui.label(RichText::new(title).color(ModernTheme::TEXT_PRIMARY).strong());
            ui.label(RichText::new(desc).color(ModernTheme::TEXT_SECONDARY).small());
        });
    });
    ui.add_space(6.0);
}
