use crate::config::AppConfig;
use crate::network::tunnel::NetworkTunnel;
use crate::types::{AppState, ConnectionStatus, NodeRole};
use crate::ui::retro_theme::ModernTheme;
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
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                // Bannière de statut moderne Windows 11
                render_status_banner(ui, state);

                ui.add_space(12.0);

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

                ui.add_space(14.0);

                // Liste des Joueurs connectés en P2P direct
                render_peers_scoreboard(ui, state);
            });
    }
}

fn render_status_banner(ui: &mut Ui, state: &AppState) {
    let (bg_color, status_text, dot_color, sub_text) = match state.status {
        ConnectionStatus::Hosting => (
            Color32::from_rgb(236, 253, 245), // Vert menthe très doux
            "Salon actif — Vous hébergez la session P2P",
            ModernTheme::GREEN_VIVID,
            "Le serveur central a initialisé le salon. Vos amis se connectent désormais en P2P direct à votre nœud.",
        ),
        ConnectionStatus::Connected => (
            Color32::from_rgb(239, 246, 255), // Bleu clair très doux
            "Connecté au maillage P2P CoopLink",
            ModernTheme::BLUE_ELECTRIC,
            "Connexion directe établie avec l'hôte (zéro transit par le serveur central). Prêt pour le jeu ou le streaming.",
        ),
        ConnectionStatus::Connecting => (
            Color32::from_rgb(255, 247, 237), // Orange pastel doux
            "Négociation du salon et ouverture UPnP en cours...",
            ModernTheme::ORANGE_CORAL,
            "Échange des clés et découverte directe des pairs...",
        ),
        ConnectionStatus::Disconnected => (
            ModernTheme::CARD_BG,
            "Prêt à créer ou rejoindre un salon",
            ModernTheme::BLUE_ELECTRIC,
            "Créez un salon pour héberger vos amis en P2P direct ou rejoignez un code de salon existant.",
        ),
        ConnectionStatus::Error => (
            Color32::from_rgb(254, 242, 242),
            "Impossible de joindre le salon",
            ModernTheme::RED_ERROR,
            "Vérifiez l'adresse ou le code du salon et réessayez.",
        ),
    };

    egui::Frame::none()
        .fill(bg_color)
        .stroke(Stroke::new(1.0_f32, dot_color.gamma_multiply(0.45)))
        .inner_margin(14.0)
        .rounding(Rounding::same(10.0))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new("●").color(dot_color).size(18.0));
                ui.vertical(|ui| {
                    ui.label(
                        RichText::new(status_text)
                            .color(ModernTheme::TEXT_PRIMARY)
                            .strong()
                            .size(15.0),
                    );
                    ui.label(RichText::new(sub_text).color(ModernTheme::TEXT_SECONDARY).small());
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
    // Barre de profil joueur et code de salon
    egui::Frame::none()
        .fill(ModernTheme::CARD_BG)
        .stroke(Stroke::new(1.0_f32, ModernTheme::BORDER_SUBTLE))
        .inner_margin(14.0)
        .rounding(Rounding::same(10.0))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new("Votre pseudo :")
                        .color(ModernTheme::TEXT_PRIMARY)
                        .strong(),
                );
                ui.add(egui::TextEdit::singleline(&mut config.player_name).desired_width(160.0));

                ui.add_space(24.0);
                ui.label(
                    RichText::new("Code du salon :")
                        .color(ModernTheme::TEXT_PRIMARY)
                        .strong(),
                );
                ui.add(egui::TextEdit::singleline(&mut state.room_code).desired_width(130.0));
            });
        });

    ui.add_space(12.0);

    // Deux cartes modernes côte à côte : Héberger ou Rejoindre
    ui.columns(2, |columns| {
        // Colonne 1 : HÉBERGER
        egui::Frame::none()
            .fill(ModernTheme::CARD_BG)
            .stroke(Stroke::new(1.5_f32, ModernTheme::BORDER_SUBTLE))
            .inner_margin(18.0)
            .rounding(Rounding::same(10.0))
            .show(&mut columns[0], |ui| {
                ui.label(
                    RichText::new("Héberger un salon")
                        .color(ModernTheme::BLUE_ELECTRIC)
                        .strong()
                        .size(17.0),
                );
                ui.add_space(6.0);
                ui.label(
                    RichText::new(
                        "Créez un salon virtuel et partagez votre session avec vos amis en P2P direct (ouverture UPnP et Sunshine intégrés).",
                    )
                    .color(ModernTheme::TEXT_SECONDARY),
                );

                ui.add_space(18.0);
                let host_btn = egui::Button::new(
                    RichText::new("Créer et héberger le salon")
                        .color(Color32::WHITE)
                        .strong()
                        .size(14.5),
                )
                .fill(ModernTheme::BLUE_ELECTRIC)
                .rounding(Rounding::same(8.0))
                .min_size(egui::vec2(ui.available_width(), 38.0));

                if ui.add(host_btn).clicked() {
                    *on_host_clicked = true;
                }
            });

        // Colonne 2 : REJOINDRE
        egui::Frame::none()
            .fill(ModernTheme::CARD_BG)
            .stroke(Stroke::new(1.5_f32, ModernTheme::BORDER_SUBTLE))
            .inner_margin(18.0)
            .rounding(Rounding::same(10.0))
            .show(&mut columns[1], |ui| {
                ui.label(
                    RichText::new("Rejoindre un ami")
                        .color(ModernTheme::GREEN_VIVID)
                        .strong()
                        .size(17.0),
                );
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new("Serveur / Hôte :")
                            .color(ModernTheme::TEXT_SECONDARY),
                    );
                    ui.add(
                        egui::TextEdit::singleline(&mut config.remote_server_address)
                            .desired_width(190.0),
                    );
                });

                ui.add_space(12.0);
                let join_btn = egui::Button::new(
                    RichText::new("Rejoindre le salon")
                        .color(Color32::WHITE)
                        .strong()
                        .size(14.5),
                )
                .fill(ModernTheme::GREEN_VIVID)
                .rounding(Rounding::same(8.0))
                .min_size(egui::vec2(ui.available_width(), 38.0));

                if ui.add(join_btn).clicked() {
                    *on_join_clicked = true;
                }
            });
    });
}

fn render_connecting_screen(ui: &mut Ui) {
    egui::Frame::none()
        .fill(ModernTheme::CARD_BG)
        .stroke(Stroke::new(1.0_f32, ModernTheme::BORDER_SUBTLE))
        .inner_margin(24.0)
        .rounding(Rounding::same(10.0))
        .show(ui, |ui| {
            ui.vertical_centered(|ui| {
                ui.spinner();
                ui.add_space(10.0);
                ui.heading(
                    RichText::new("Connexion et synchronisation P2P en cours...")
                        .color(ModernTheme::BLUE_ELECTRIC)
                        .strong(),
                );
                ui.label(
                    RichText::new(
                        "Configuration de la métrique prioritaire, ouverture UPnP et jumelage automatique...",
                    )
                    .color(ModernTheme::TEXT_SECONDARY),
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
        .fill(ModernTheme::CARD_BG)
        .stroke(Stroke::new(1.5_f32, ModernTheme::GREEN_VIVID))
        .inner_margin(16.0)
        .rounding(Rounding::same(10.0))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                let role_label = if state.role == NodeRole::Host {
                    RichText::new("● Hôte du salon")
                        .color(ModernTheme::GREEN_VIVID)
                        .strong()
                } else {
                    RichText::new("● Client connecté en P2P")
                        .color(ModernTheme::BLUE_ELECTRIC)
                        .strong()
                };
                ui.label(role_label);

                ui.add_space(16.0);
                ui.label(RichText::new("Salon :").color(ModernTheme::TEXT_SECONDARY));
                ui.label(RichText::new(&state.room_code).color(ModernTheme::TEXT_PRIMARY).strong());

                if let Some(vip) = state.virtual_ip {
                    ui.add_space(16.0);
                    ui.label(RichText::new("IP Virtuelle :").color(ModernTheme::TEXT_SECONDARY));
                    ui.label(
                        RichText::new(format!("{}", vip))
                            .color(ModernTheme::BLUE_ELECTRIC)
                            .strong(),
                    );
                    if ui.button("Copier").clicked() {
                        ui.output_mut(|o| o.copied_text = vip.to_string());
                    }
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let disc_btn = egui::Button::new(
                        RichText::new("Quitter le salon")
                            .color(Color32::WHITE)
                            .strong(),
                    )
                    .fill(ModernTheme::RED_ERROR)
                    .rounding(Rounding::same(8.0));

                    if ui.add(disc_btn).clicked() {
                        *on_disconnect_clicked = true;
                    }
                });
            });

            if state.role == NodeRole::Host {
                ui.add_space(10.0);
                egui::Frame::none()
                    .fill(ModernTheme::SUBTLE_BG)
                    .stroke(Stroke::new(1.0_f32, ModernTheme::BORDER_SUBTLE))
                    .inner_margin(12.0)
                    .rounding(Rounding::same(8.0))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new("Adresse directe P2P :")
                                    .color(ModernTheme::TEXT_PRIMARY)
                                    .strong(),
                            );
                            if let Some(pub_ip) = state.public_ip {
                                let full_addr = format!("{}:{}", pub_ip, config.listen_port);
                                ui.label(
                                    RichText::new(&full_addr)
                                        .color(ModernTheme::BLUE_ELECTRIC)
                                        .strong(),
                                );
                                if ui.button("Copier l'adresse").clicked() {
                                    ui.output_mut(|o| o.copied_text = full_addr);
                                }
                            } else {
                                ui.label(
                                    RichText::new("Ouverture UPnP automatique...")
                                        .color(ModernTheme::ORANGE_CORAL),
                                );
                            }

                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                ui.label(
                                    RichText::new("✔ Trafic 100% P2P direct (aucun transit serveur)")
                                        .color(ModernTheme::GREEN_VIVID)
                                        .small(),
                                );
                            });
                        });
                    });
            }
        });
}

fn render_peers_scoreboard(ui: &mut Ui, state: &AppState) {
    egui::Frame::none()
        .fill(ModernTheme::CARD_BG)
        .stroke(Stroke::new(1.0_f32, ModernTheme::BORDER_SUBTLE))
        .inner_margin(16.0)
        .rounding(Rounding::same(10.0))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(format!("Participants connectés au salon ({})", state.peers.len()))
                        .color(ModernTheme::TEXT_PRIMARY)
                        .strong()
                        .size(15.5),
                );
                ui.label(
                    RichText::new("— Connexion directe pair-à-pair")
                        .color(ModernTheme::TEXT_MUTED)
                        .small(),
                );
            });

            ui.add_space(10.0);

            if state.peers.is_empty() {
                ui.label(
                    RichText::new("Aucun joueur connecté pour le moment. Créez ou rejoignez un salon pour commencer.")
                        .color(ModernTheme::TEXT_MUTED),
                );
            } else {
                egui::Grid::new("peers_grid")
                    .num_columns(5)
                    .spacing([28.0, 8.0])
                    .striped(true)
                    .show(ui, |ui| {
                        ui.label(RichText::new("JOUEUR").color(ModernTheme::BLUE_ELECTRIC).strong());
                        ui.label(RichText::new("RÔLE").color(ModernTheme::BLUE_ELECTRIC).strong());
                        ui.label(RichText::new("IP VIRTUELLE").color(ModernTheme::BLUE_ELECTRIC).strong());
                        ui.label(RichText::new("LATENCE P2P").color(ModernTheme::BLUE_ELECTRIC).strong());
                        ui.label(RichText::new("ACTION").color(ModernTheme::BLUE_ELECTRIC).strong());
                        ui.end_row();

                        for peer in &state.peers {
                            ui.label(RichText::new(&peer.name).color(ModernTheme::TEXT_PRIMARY).strong());

                            if peer.is_host {
                                ui.label(RichText::new("Hôte").color(ModernTheme::GREEN_VIVID).strong());
                            } else {
                                ui.label(RichText::new("Invité").color(ModernTheme::BLUE_ELECTRIC));
                            }

                            ui.label(
                                RichText::new(format!("{}", peer.virtual_ip))
                                    .color(ModernTheme::TEXT_PRIMARY),
                            );

                            let color = if peer.ping_ms < 35 {
                                ModernTheme::GREEN_VIVID
                            } else if peer.ping_ms < 85 {
                                ModernTheme::ORANGE_CORAL
                            } else {
                                ModernTheme::RED_ERROR
                            };
                            ui.label(RichText::new(format!("{} ms", peer.ping_ms)).color(color).strong());

                            if ui.button("Copier IP").clicked() {
                                ui.output_mut(|o| o.copied_text = peer.virtual_ip.to_string());
                            }
                            ui.end_row();
                        }
                    });
            }
        });
}
