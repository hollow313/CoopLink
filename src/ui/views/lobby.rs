use crate::config::AppConfig;
use crate::network::tunnel::NetworkTunnel;
use crate::types::{AppState, ConnectionStatus, NodeRole};
use crate::ui::retro_theme::RetroTheme;
use egui::{Color32, RichText, Rounding, Stroke, Ui};
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
            // Bannière de statut d'ambiance rétro
            render_status_banner(ui, state);

            ui.add_space(10.0);

            // Panneau principal de contrôle
            match state.status {
                ConnectionStatus::Disconnected | ConnectionStatus::Error => {
                    render_connection_dashboard(ui, state, config, on_host_clicked, on_join_clicked);
                }
                ConnectionStatus::Connecting => {
                    render_connecting_screen(ui);
                }
                ConnectionStatus::Connected | ConnectionStatus::Hosting => {
                    render_active_session(ui, state, config, on_disconnect_clicked);
                }
            }

            ui.add_space(15.0);

            // Tableau des Joueurs (Scoreboard Arcade)
            render_peers_scoreboard(ui, state);
        });
    }
}

fn render_status_banner(ui: &mut Ui, state: &AppState) {
    let (bg_color, status_text, dot_color, sub_text) = match state.status {
        ConnectionStatus::Hosting => (
            Color32::from_rgb(12, 28, 22),
            "SALON VIRTUEL EN LIGNE (HÔTE ACTIF)",
            RetroTheme::NEON_GREEN,
            "En attente de vos amis. Vos jeux détecteront les parties comme en local.",
        ),
        ConnectionStatus::Connected => (
            Color32::from_rgb(12, 25, 38),
            "CONNECTÉ AU NŒUD VIRTUEL (CLIENT ACTIF)",
            RetroTheme::NEON_CYAN,
            "Réseau LAN actif. Prêt à rejoindre la partie de votre hôte.",
        ),
        ConnectionStatus::Connecting => (
            Color32::from_rgb(35, 28, 12),
            "NÉGOCIATION RÉSEAU & UPNP EN COURS...",
            RetroTheme::RETRO_AMBER,
            "Initialisation du tunnel chiffré et ouverture des ports...",
        ),
        ConnectionStatus::Disconnected => (
            Color32::from_rgb(18, 22, 34),
            "HORS-LIGNE // PRÊT POUR LE GAMING",
            Color32::from_rgb(140, 160, 190),
            "Créez une salle pour héberger vos amis, ou entrez l'adresse d'un hôte.",
        ),
        ConnectionStatus::Error => (
            Color32::from_rgb(38, 14, 20),
            "ERREUR DE CONNEXION RÉSEAU",
            RetroTheme::RETRO_RED,
            "Vérifiez l'adresse IP et assurez-vous que l'hôte a bien démarré.",
        ),
    };

    egui::Frame::none()
        .fill(bg_color)
        .stroke(Stroke::new(1.0_f32, dot_color))
        .inner_margin(10.0)
        .rounding(Rounding::same(6.0))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new("●").color(dot_color).size(18.0));
                ui.vertical(|ui| {
                    ui.label(RichText::new(status_text).color(Color32::WHITE).strong());
                    ui.label(RichText::new(sub_text).color(RetroTheme::TEXT_SECONDARY).small());
                });
            });
        });
}

fn render_connection_dashboard(
    ui: &mut Ui,
    state: &mut AppState,
    config: &mut AppConfig,
    on_host_clicked: &mut bool,
    on_join_clicked: &mut bool,
) {
    // Ligne de configuration générale (Pseudo & Code)
    egui::Frame::none()
        .fill(RetroTheme::PANEL_BG)
        .stroke(Stroke::new(1.0_f32, Color32::from_rgb(35, 45, 70)))
        .inner_margin(12.0)
        .rounding(Rounding::same(6.0))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new("👤 PSEUDO JOUEUR:").color(RetroTheme::NEON_CYAN).strong());
                ui.add(egui::TextEdit::singleline(&mut config.player_name).desired_width(140.0));

                ui.add_space(20.0);
                ui.label(RichText::new("🔑 CODE SALLE:").color(RetroTheme::NEON_GREEN).strong());
                ui.add(egui::TextEdit::singleline(&mut state.room_code).desired_width(120.0));
            });
        });

    ui.add_space(10.0);

    // Deux grandes colonnes d'actions : HÉBERGER ou REJOINDRE
    ui.columns(2, |columns| {
        // Colonne 1 : HÉBERGER
        egui::Frame::none()
            .fill(RetroTheme::CARD_BG)
            .stroke(Stroke::new(1.5_f32, RetroTheme::NEON_GREEN))
            .inner_margin(14.0)
            .rounding(Rounding::same(6.0))
            .show(&mut columns[0], |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new("👑").size(20.0));
                    ui.heading(RichText::new("HÉBERGER UN NŒUD").color(RetroTheme::NEON_GREEN).strong());
                });
                ui.add_space(6.0);
                ui.label(
                    RichText::new("Créez le salon local. Vos amis se connecteront sur votre nœud. UPnP ouvrira le port automatiquement.")
                        .color(RetroTheme::TEXT_SECONDARY)
                        .small(),
                );

                ui.add_space(14.0);
                let host_btn = egui::Button::new(
                    RichText::new("▶ CRÉER LA SALLE (HÔTE)")
                        .color(Color32::BLACK)
                        .strong()
                        .size(14.0),
                )
                .fill(RetroTheme::NEON_GREEN)
                .min_size(egui::vec2(ui.available_width(), 36.0));

                if ui.add(host_btn).clicked() {
                    *on_host_clicked = true;
                }
            });

        // Colonne 2 : REJOINDRE
        egui::Frame::none()
            .fill(RetroTheme::CARD_BG)
            .stroke(Stroke::new(1.5_f32, RetroTheme::NEON_CYAN))
            .inner_margin(14.0)
            .rounding(Rounding::same(6.0))
            .show(&mut columns[1], |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new("🚀").size(20.0));
                    ui.heading(RichText::new("REJOINDRE UN AMI").color(RetroTheme::NEON_CYAN).strong());
                });
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    ui.label(RichText::new("IP / Hôte:").color(Color32::WHITE).small());
                    ui.add(egui::TextEdit::singleline(&mut config.remote_server_address).desired_width(180.0));
                });

                ui.add_space(10.0);
                let join_btn = egui::Button::new(
                    RichText::new("▶ REJOINDRE LE NŒUD")
                        .color(Color32::BLACK)
                        .strong()
                        .size(14.0),
                )
                .fill(RetroTheme::NEON_CYAN)
                .min_size(egui::vec2(ui.available_width(), 36.0));

                if ui.add(join_btn).clicked() {
                    *on_join_clicked = true;
                }
            });
    });
}

fn render_connecting_screen(ui: &mut Ui) {
    egui::Frame::none()
        .fill(RetroTheme::PANEL_BG)
        .stroke(Stroke::new(1.0_f32, RetroTheme::RETRO_AMBER))
        .inner_margin(20.0)
        .rounding(Rounding::same(6.0))
        .show(ui, |ui| {
            ui.vertical_centered(|ui| {
                ui.spinner();
                ui.add_space(10.0);
                ui.heading(
                    RichText::new("NÉGOCIATION RÉSEAU EN COURS...")
                        .color(RetroTheme::RETRO_AMBER)
                        .strong(),
                );
                ui.label(
                    RichText::new("Configuration de la métrique basse, détection UPnP et synchronisation des pairs...")
                        .color(RetroTheme::TEXT_SECONDARY),
                );
            });
        });
}

fn render_active_session(
    ui: &mut Ui,
    state: &mut AppState,
    config: &mut AppConfig,
    on_disconnect_clicked: &mut bool,
) {
    egui::Frame::none()
        .fill(RetroTheme::PANEL_BG)
        .stroke(Stroke::new(1.5_f32, RetroTheme::NEON_GREEN))
        .inner_margin(14.0)
        .rounding(Rounding::same(6.0))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                let role_label = if state.role == NodeRole::Host {
                    RichText::new("👑 VOUS ÊTES L'HÔTE").color(RetroTheme::NEON_GREEN).strong()
                } else {
                    RichText::new("🎮 JOUEUR CONNECTÉ").color(RetroTheme::NEON_CYAN).strong()
                };
                ui.label(role_label);

                ui.add_space(15.0);
                ui.label(RichText::new("SALLE:").color(RetroTheme::TEXT_MUTED));
                ui.label(RichText::new(&state.room_code).color(Color32::WHITE).strong());

                if let Some(vip) = state.virtual_ip {
                    ui.add_space(20.0);
                    ui.label(RichText::new("IP VIRTUELLE:").color(RetroTheme::TEXT_MUTED));
                    ui.label(RichText::new(format!("{}", vip)).color(RetroTheme::NEON_GREEN).strong().monospace());
                    if ui.button("📋 Copier").clicked() {
                        ui.output_mut(|o| o.copied_text = vip.to_string());
                    }
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let disc_btn = egui::Button::new(
                        RichText::new("✖ QUITTER LE SALON")
                            .color(Color32::WHITE)
                            .strong(),
                    )
                    .fill(RetroTheme::RETRO_RED);

                    if ui.add(disc_btn).clicked() {
                        *on_disconnect_clicked = true;
                    }
                });
            });

            // Encart Hôte : IP Publique & UPnP
            if state.role == NodeRole::Host {
                ui.add_space(10.0);
                egui::Frame::none()
                    .fill(Color32::from_rgb(14, 22, 34))
                    .stroke(Stroke::new(1.0_f32, RetroTheme::NEON_CYAN))
                    .inner_margin(10.0)
                    .rounding(Rounding::same(4.0))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new("🌐 ADRESSE À DONNER À VOS AMIS:").color(Color32::WHITE).strong());
                            if let Some(pub_ip) = state.public_ip {
                                let full_addr = format!("{}:{}", pub_ip, config.listen_port);
                                ui.label(RichText::new(&full_addr).color(RetroTheme::NEON_CYAN).strong().monospace());
                                if ui.button("📋 Copier Adresse").clicked() {
                                    ui.output_mut(|o| o.copied_text = full_addr);
                                }
                            } else {
                                ui.label(RichText::new("Recherche UPnP sur la box...").color(RetroTheme::RETRO_AMBER));
                            }

                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                ui.label(RichText::new("✔ Port Box ouvert automatiquement via UPnP").color(RetroTheme::NEON_GREEN).small());
                            });
                        });
                    });
            }
        });
}

fn render_peers_scoreboard(ui: &mut Ui, state: &AppState) {
    egui::Frame::none()
        .fill(RetroTheme::PANEL_BG)
        .stroke(Stroke::new(1.0_f32, Color32::from_rgb(35, 45, 70)))
        .inner_margin(14.0)
        .rounding(Rounding::same(6.0))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.heading(
                    RichText::new(format!("👥 JOUEURS DANS LE NŒUD // SALLE ({})", state.peers.len()))
                        .color(Color32::WHITE)
                        .strong(),
                );
                ui.label(
                    RichText::new("(Multi-joueurs : 2 clients ou plus connectés ensemble)")
                        .color(RetroTheme::TEXT_MUTED)
                        .small(),
                );
            });
            ui.add_space(8.0);

            if state.peers.is_empty() {
                ui.label(
                    RichText::new("Aucun joueur connecté pour le moment. Lancez ou rejoignez une salle pour commencer !")
                        .color(RetroTheme::TEXT_MUTED)
                        .italics(),
                );
            } else {
                egui::Grid::new("peers_grid")
                    .num_columns(5)
                    .spacing([24.0, 10.0])
                    .striped(true)
                    .show(ui, |ui| {
                        ui.label(RichText::new("JOUEUR").color(RetroTheme::NEON_CYAN).strong());
                        ui.label(RichText::new("RÔLE").color(RetroTheme::NEON_CYAN).strong());
                        ui.label(RichText::new("IP VIRTUELLE").color(RetroTheme::NEON_CYAN).strong());
                        ui.label(RichText::new("QUALITÉ / PING").color(RetroTheme::NEON_CYAN).strong());
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
                                ui.label(RichText::new("★ HÔTE").color(RetroTheme::NEON_GREEN).strong());
                            } else {
                                ui.label(RichText::new("CLIENT").color(RetroTheme::NEON_CYAN));
                            }

                            // IP Virtuelle
                            ui.label(
                                RichText::new(format!("{}", peer.virtual_ip))
                                    .color(RetroTheme::NEON_GREEN)
                                    .monospace(),
                            );

                            // Ping RTT avec indicateur lumineux
                            let (dot, color) = if peer.ping_ms < 35 {
                                ("🟢", RetroTheme::NEON_GREEN)
                            } else if peer.ping_ms < 85 {
                                ("🟡", RetroTheme::RETRO_AMBER)
                            } else {
                                ("🔴", RetroTheme::RETRO_RED)
                            };
                            ui.horizontal(|ui| {
                                ui.label(dot);
                                ui.label(RichText::new(format!("{} ms", peer.ping_ms)).color(color).strong());
                            });

                            // Bouton Copier
                            if ui.button("📋 Copier IP").clicked() {
                                ui.output_mut(|o| o.copied_text = peer.virtual_ip.to_string());
                            }
                            ui.end_row();
                        }
                    });
            }
        });
}
