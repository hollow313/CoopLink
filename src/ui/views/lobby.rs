use crate::config::AppConfig;
use crate::network::tunnel::NetworkTunnel;
use crate::types::{AppState, ConnectionStatus, NodeRole};
use crate::ui::retro_theme::RetroTheme;
use egui::{Color32, RichText, Ui};
use std::sync::Arc;

pub struct LobbyView;

impl LobbyView {
    pub fn render(
        ui: &mut Ui,
        state: &mut AppState,
        config: &mut AppConfig,
        _tunnel: &Arc<NetworkTunnel>,
        on_host_clicked: &mut bool,
        on_join_clicked: &mut bool,
        on_disconnect_clicked: &mut bool,
    ) {
        ui.vertical(|ui| {
            // Bannière de statut rétro
            render_status_banner(ui, state);

            ui.add_space(10.0);

            // Panneau de contrôle : Héberger ou Rejoindre
            egui::Frame::none()
                .fill(RetroTheme::PANEL_BG)
                .stroke(egui::Stroke::new(1.0_f32, RetroTheme::NEON_CYAN))
                .inner_margin(12.0)
                .rounding(4.0)
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.heading(RichText::new("🕹 CONTRÔLE DE SALLE (NOEUD LAN)").color(RetroTheme::NEON_CYAN).strong());
                    });
                    ui.add_space(8.0);

                    match state.status {
                        ConnectionStatus::Disconnected | ConnectionStatus::Error => {
                            ui.horizontal(|ui| {
                                ui.label(RichText::new("PSEUDO:").color(RetroTheme::TEXT_MUTED).strong());
                                ui.text_edit_singleline(&mut config.player_name);

                                ui.add_space(15.0);
                                ui.label(RichText::new("CODE SALLE:").color(RetroTheme::TEXT_MUTED).strong());
                                ui.text_edit_singleline(&mut state.room_code);
                            });

                            ui.add_space(8.0);
                            ui.horizontal(|ui| {
                                ui.label(RichText::new("IP / SERVEUR HÔTE:").color(RetroTheme::TEXT_MUTED).strong());
                                ui.text_edit_singleline(&mut config.remote_server_address);
                            });

                            ui.add_space(12.0);
                            ui.horizontal(|ui| {
                                let host_btn = egui::Button::new(
                                    RichText::new("👑 CRÉER & HÉBERGER UN NŒUD (HOST)")
                                        .color(Color32::BLACK)
                                        .strong(),
                                )
                                .fill(RetroTheme::NEON_GREEN);

                                if ui.add(host_btn).clicked() {
                                    *on_host_clicked = true;
                                }

                                ui.add_space(10.0);

                                let join_btn = egui::Button::new(
                                    RichText::new("🚀 REJOINDRE LE NŒUD (CLIENT)")
                                        .color(Color32::BLACK)
                                        .strong(),
                                )
                                .fill(RetroTheme::NEON_CYAN);

                                if ui.add(join_btn).clicked() {
                                    *on_join_clicked = true;
                                }
                            });
                        }
                        ConnectionStatus::Connecting => {
                            ui.horizontal(|ui| {
                                ui.spinner();
                                ui.label(
                                    RichText::new("CONNEXION ET NEGOCIATION AU NOEUD EN COURS...")
                                        .color(RetroTheme::RETRO_AMBER)
                                        .strong(),
                                );
                            });
                        }
                        ConnectionStatus::Connected | ConnectionStatus::Hosting => {
                            ui.horizontal(|ui| {
                                let role_badge = if state.role == NodeRole::Host {
                                    RichText::new("[HÔTE DU NŒUD]").color(RetroTheme::NEON_GREEN).strong()
                                } else {
                                    RichText::new("[CLIENT CONNECTÉ]").color(RetroTheme::NEON_CYAN).strong()
                                };
                                ui.label(role_badge);

                                ui.label(RichText::new("SALLE:").color(RetroTheme::TEXT_MUTED));
                                ui.label(RichText::new(&state.room_code).color(Color32::WHITE).strong());

                                if let Some(vip) = state.virtual_ip {
                                    ui.add_space(15.0);
                                    ui.label(RichText::new("MON IP VIRTUELLE:").color(RetroTheme::TEXT_MUTED));
                                    ui.label(
                                        RichText::new(format!("{}", vip))
                                            .color(RetroTheme::NEON_GREEN)
                                            .strong(),
                                    );
                                    if ui.button("📋 Copier IP").clicked() {
                                        ui.output_mut(|o| o.copied_text = vip.to_string());
                                    }
                                }

                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    let disc_btn = egui::Button::new(
                                        RichText::new("✖ QUITTER LA SALLE")
                                            .color(Color32::WHITE)
                                            .strong(),
                                    )
                                    .fill(RetroTheme::RETRO_RED);

                                    if ui.add(disc_btn).clicked() {
                                        *on_disconnect_clicked = true;
                                    }
                                });
                            });

                            // Encart UPnP & IP Publique pour l'Hôte
                            if state.role == NodeRole::Host {
                                ui.add_space(8.0);
                                egui::Frame::none()
                                    .fill(Color32::from_rgb(12, 18, 30))
                                    .stroke(egui::Stroke::new(1.0_f32, RetroTheme::NEON_GREEN))
                                    .inner_margin(8.0)
                                    .rounding(4.0)
                                    .show(ui, |ui| {
                                        ui.horizontal(|ui| {
                                            ui.label(RichText::new("🌐 MON IP PUBLIQUE (À ENVOYER AUX AMIS):").color(RetroTheme::TEXT_PRIMARY).strong());
                                            if let Some(pub_ip) = state.public_ip {
                                                let full_addr = format!("{}:{}", pub_ip, config.listen_port);
                                                ui.label(RichText::new(&full_addr).color(RetroTheme::NEON_GREEN).strong().monospace());
                                                if ui.button("📋 Copier Adresse Hôte").clicked() {
                                                    ui.output_mut(|o| o.copied_text = full_addr);
                                                }
                                            } else {
                                                ui.label(RichText::new("Détection automatique en cours...").color(RetroTheme::RETRO_AMBER));
                                            }
                                        });

                                        ui.horizontal(|ui| {
                                            ui.label(RichText::new("BOX INTERNET (UPNP):").color(RetroTheme::TEXT_MUTED).small());
                                            ui.label(RichText::new(&state.upnp_status_text).color(RetroTheme::NEON_CYAN).small());
                                            ui.label(RichText::new("✔ Zéro configuration manuelle de box requise").color(RetroTheme::NEON_GREEN).small());
                                        });
                                    });
                            }
                        }
                    }
                });

            ui.add_space(15.0);

            // Liste des joueurs connectés (2 ou plus)
            render_peers_list(ui, state);
        });
    }
}

fn render_status_banner(ui: &mut Ui, state: &AppState) {
    let (bg_color, status_text, dot_color) = match state.status {
        ConnectionStatus::Hosting => (
            Color32::from_rgb(15, 35, 25),
            "NŒUD HÉBERGÉ EN LIGNE - EN ATTENTE D'AMIS OU PRÊT POUR LE JEU",
            RetroTheme::NEON_GREEN,
        ),
        ConnectionStatus::Connected => (
            Color32::from_rgb(15, 30, 45),
            "CONNECTÉ AU NŒUD VIRTUEL - PRÊT POUR LES PARTIES MULTIJOUEURS",
            RetroTheme::NEON_CYAN,
        ),
        ConnectionStatus::Connecting => (
            Color32::from_rgb(45, 35, 15),
            "NÉGOCIATION RÉSEAU EN COURS...",
            RetroTheme::RETRO_AMBER,
        ),
        ConnectionStatus::Disconnected => (
            Color32::from_rgb(25, 25, 35),
            "HORS-LIGNE - CRÉEZ UNE SALLE OU REJOIGNEZ CELLE D'UN AMI",
            Color32::GRAY,
        ),
        ConnectionStatus::Error => (
            Color32::from_rgb(45, 15, 20),
            "ERREUR DE CONNEXION - VÉRIFIEZ L'ADRESSE ET LE PARE-FEU",
            RetroTheme::RETRO_RED,
        ),
    };

    egui::Frame::none()
        .fill(bg_color)
        .stroke(egui::Stroke::new(1.0_f32, dot_color))
        .inner_margin(8.0)
        .rounding(4.0)
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new("●").color(dot_color).size(16.0));
                ui.label(RichText::new(status_text).color(RetroTheme::TEXT_PRIMARY).strong());
            });
        });
}

fn render_peers_list(ui: &mut Ui, state: &AppState) {
    egui::Frame::none()
        .fill(RetroTheme::PANEL_BG)
        .stroke(egui::Stroke::new(1.0_f32, Color32::from_rgb(40, 50, 75)))
        .inner_margin(12.0)
        .rounding(4.0)
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.heading(
                    RichText::new(format!("👥 JOUEURS DANS LE NŒUD ({})", state.peers.len()))
                        .color(RetroTheme::TEXT_PRIMARY)
                        .strong(),
                );
                ui.label(
                    RichText::new("(Supporte 2 joueurs ou plus simultanément)")
                        .color(RetroTheme::TEXT_MUTED)
                        .small(),
                );
            });
            ui.add_space(8.0);

            if state.peers.is_empty() {
                ui.label(
                    RichText::new("Aucun joueur pour le moment. Lancez ou rejoignez une salle pour commencer !")
                        .color(RetroTheme::TEXT_MUTED)
                        .italics(),
                );
            } else {
                egui::Grid::new("peers_grid")
                    .num_columns(5)
                    .spacing([20.0, 10.0])
                    .striped(true)
                    .show(ui, |ui| {
                        ui.label(RichText::new("JOUEUR").color(RetroTheme::NEON_CYAN).strong());
                        ui.label(RichText::new("RÔLE").color(RetroTheme::NEON_CYAN).strong());
                        ui.label(RichText::new("IP VIRTUELLE").color(RetroTheme::NEON_CYAN).strong());
                        ui.label(RichText::new("LATENCE (PING)").color(RetroTheme::NEON_CYAN).strong());
                        ui.label(RichText::new("ACTION").color(RetroTheme::NEON_CYAN).strong());
                        ui.end_row();

                        for peer in &state.peers {
                            // Nom joueur
                            ui.horizontal(|ui| {
                                ui.label(RichText::new("🎮").size(14.0));
                                ui.label(RichText::new(&peer.name).color(Color32::WHITE).strong());
                            });

                            // Rôle
                            if peer.is_host {
                                ui.label(RichText::new("HÔTE (P1)").color(RetroTheme::NEON_GREEN).strong());
                            } else {
                                ui.label(RichText::new("CLIENT").color(RetroTheme::NEON_CYAN));
                            }

                            // IP Virtuelle
                            ui.label(
                                RichText::new(format!("{}", peer.virtual_ip))
                                    .color(RetroTheme::NEON_GREEN)
                                    .monospace(),
                            );

                            // Ping RTT
                            let ping_color = if peer.ping_ms < 35 {
                                RetroTheme::NEON_GREEN
                            } else if peer.ping_ms < 90 {
                                RetroTheme::RETRO_AMBER
                            } else {
                                RetroTheme::RETRO_RED
                            };
                            ui.label(RichText::new(format!("{} ms", peer.ping_ms)).color(ping_color).strong());

                            // Bouton Copier
                            if ui.button("Copier IP").clicked() {
                                ui.output_mut(|o| o.copied_text = peer.virtual_ip.to_string());
                            }
                            ui.end_row();
                        }
                    });
            }
        });
}
