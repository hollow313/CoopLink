use crate::config::AppConfig;
use crate::types::AppState;
use crate::ui::retro_theme::RetroTheme;
use egui::{Color32, RichText, Ui};

pub struct FirewallView;

impl FirewallView {
    pub fn render(
        ui: &mut Ui,
        state: &mut AppState,
        config: &mut AppConfig,
        on_repair_firewall_clicked: &mut bool,
    ) {
        ui.vertical(|ui| {
            // Carte statut pare-feu
            egui::Frame::none()
                .fill(RetroTheme::PANEL_BG)
                .stroke(egui::Stroke::new(1.0_f32, RetroTheme::NEON_MAGENTA))
                .inner_margin(12.0)
                .rounding(4.0)
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.heading(
                            RichText::new("🛡 DIAGNOSTIC & PARE-FEU WINDOWS (FIREWALL)")
                                .color(RetroTheme::NEON_MAGENTA)
                                .strong(),
                        );
                    });
                    ui.add_space(6.0);

                    ui.label(
                        RichText::new(
                            "Le pare-feu Windows Defender bloque fréquemment les connexions entrantes des jeux vidéo \
                             et les paquets de découverte LAN provenant de réseaux distants. CoopLink peut enregistrer \
                             automatiquement les exceptions nécessaires pour garantir une stabilité à 100%.",
                        )
                        .color(RetroTheme::TEXT_MUTED),
                    );

                    ui.add_space(10.0);

                    // Statut détecté
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("STATUT DES RÈGLES PARE-FEU:").color(RetroTheme::TEXT_PRIMARY).strong());

                        if state.firewall_ok {
                            ui.label(
                                RichText::new("✔ ACTIF & SÉCURISÉ (RÈGLES DÉTECTÉES)")
                                    .color(RetroTheme::NEON_GREEN)
                                    .strong(),
                            );
                        } else {
                            ui.label(
                                RichText::new("⚠ NON CONFIGURÉ OU PARTIELLEMENT BLOQUÉ")
                                    .color(RetroTheme::RETRO_AMBER)
                                    .strong(),
                            );
                        }
                    });

                    ui.add_space(10.0);

                    ui.horizontal(|ui| {
                        let btn = egui::Button::new(
                            RichText::new("🛡 APPLIQUER / RÉPARER LES RÈGLES PARE-FEU EN 1 CLIC")
                                .color(Color32::WHITE)
                                .strong(),
                        )
                        .fill(RetroTheme::NEON_MAGENTA);

                        if ui.add(btn).clicked() {
                            *on_repair_firewall_clicked = true;
                        }

                        if state.firewall_ok {
                            ui.label(
                                RichText::new("Toutes les règles sont opérationnelles !")
                                    .color(RetroTheme::NEON_GREEN),
                            );
                        }
                    });
                });

            ui.add_space(15.0);

            // Détails des règles appliquées
            egui::Frame::none()
                .fill(RetroTheme::PANEL_BG)
                .stroke(egui::Stroke::new(1.0_f32, Color32::from_rgb(40, 50, 75)))
                .inner_margin(12.0)
                .rounding(4.0)
                .show(ui, |ui| {
                    ui.heading(
                        RichText::new("📝 DÉTAIL DES EXCEPTIONS PARE-FEU DE JEU")
                            .color(RetroTheme::TEXT_PRIMARY)
                            .strong(),
                    );
                    ui.add_space(8.0);

                    render_rule_item(
                        ui,
                        "CoopLink - Core Virtual LAN (In & Out)",
                        "Autorise les flux de données UDP/TCP de l'exécutable sans blocage UAC.",
                        true,
                    );
                    render_rule_item(
                        ui,
                        &format!("CoopLink - Gaming Virtual Subnet ({}/24)", config.virtual_subnet_prefix),
                        "Autorise tout échange direct entre les adresses IP virtuelles des joueurs connectés.",
                        true,
                    );
                    render_rule_item(
                        ui,
                        "CoopLink - UDP Gaming Broadcast Discovery",
                        "Autorise la réception des annonces de parties LAN hébergées (255.255.255.255).",
                        true,
                    );
                });
        });
    }
}

fn render_rule_item(ui: &mut Ui, title: &str, desc: &str, active: bool) {
    ui.horizontal(|ui| {
        let icon = if active {
            RichText::new("✔").color(RetroTheme::NEON_GREEN)
        } else {
            RichText::new("✖").color(RetroTheme::RETRO_RED)
        };
        ui.label(icon);
        ui.label(RichText::new(title).color(Color32::WHITE).strong());
    });
    ui.label(RichText::new(desc).color(RetroTheme::TEXT_MUTED).small());
    ui.add_space(6.0);
}
