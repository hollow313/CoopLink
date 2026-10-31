use crate::config::AppConfig;
use crate::i18n::Language;
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
        let lang = config.language;
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                // Bannière de statut moderne Windows 11 avec vraie pastille vectorielle
                render_status_banner(ui, lang, state);

                ui.add_space(12.0);

                // Panneau principal de contrôle
                match state.status {
                    ConnectionStatus::Disconnected | ConnectionStatus::Error => {
                        render_connection_dashboard(ui, lang, state, config, on_host_clicked, on_join_clicked);
                    }
                    ConnectionStatus::Connecting => {
                        render_connecting_screen(ui, lang);
                    }
                    ConnectionStatus::Connected | ConnectionStatus::Hosting => {
                        render_active_session(ui, lang, state, config, on_disconnect_clicked);
                    }
                }

                ui.add_space(14.0);

                // Liste des Joueurs connectés en P2P direct
                render_peers_scoreboard(ui, lang, state);
            });
    }
}

fn render_status_banner(ui: &mut Ui, lang: Language, state: &AppState) {
    let (bg_color, status_text, dot_color, sub_text) = match state.status {
        ConnectionStatus::Hosting => (
            Color32::from_rgb(236, 253, 245),
            match lang {
                Language::Fr => "Salon actif — Vous hébergez la session P2P",
                Language::En => "Active Room — You are hosting the P2P session",
                Language::Es => "Sala activa — Estás alojando la sesión P2P",
                Language::De => "Aktiver Raum — Du hostest die P2P-Sitzung",
                Language::Pt => "Sala ativa — Está a alojar a sessão P2P",
            },
            ModernTheme::GREEN_VIVID,
            match lang {
                Language::Fr => "Vos amis se connectent directement en P2P à votre PC (aucun transit par le serveur).",
                Language::En => "Your friends connect directly via P2P to your PC (zero traffic through central server).",
                Language::Es => "Tus amigos se conectan directamente por P2P a tu PC (cero tránsito por el servidor).",
                Language::De => "Deine Freunde verbinden sich direkt per P2P mit deinem PC.",
                Language::Pt => "Os seus amigos ligam-se diretamente por P2P ao seu PC.",
            },
        ),
        ConnectionStatus::Connected => (
            Color32::from_rgb(239, 246, 255),
            match lang {
                Language::Fr => "Connecté au tunnel P2P CoopLink",
                Language::En => "Connected to CoopLink P2P Tunnel",
                Language::Es => "Conectado al túnel P2P de CoopLink",
                Language::De => "Mit CoopLink P2P-Tunnel verbunden",
                Language::Pt => "Conectado ao túnel P2P CoopLink",
            },
            ModernTheme::BLUE_ELECTRIC,
            match lang {
                Language::Fr => "Connexion directe établie avec l'hôte. Lancez votre jeu LAN ou rejoignez l'écran partagé.",
                Language::En => "Direct connection established with host. Launch your LAN game or join the shared screen.",
                Language::Es => "Conexión directa establecida con el anfitrión. Inicia tu juego LAN o únete a la pantalla.",
                Language::De => "Direkte Verbindung mit dem Host hergestellt. Starte dein LAN-Spiel oder teile den Bildschirm.",
                Language::Pt => "Ligação direta estabelecida com o anfitrião. Inicie o jogo LAN ou junte-se ao ecrã.",
            },
        ),
        ConnectionStatus::Connecting => (
            Color32::from_rgb(255, 247, 237),
            match lang {
                Language::Fr => "Ouverture du tunnel P2P et configuration UPnP en cours...",
                Language::En => "Opening P2P tunnel and configuring UPnP...",
                Language::Es => "Abriendo túnel P2P y configurando UPnP...",
                Language::De => "P2P-Tunnel wird geöffnet und UPnP konfiguriert...",
                Language::Pt => "A abrir túnel P2P e a configurar UPnP...",
            },
            ModernTheme::ORANGE_CORAL,
            match lang {
                Language::Fr => "Échange des clés et négociation directe entre pairs...",
                Language::En => "Exchanging keys and direct peer negotiation...",
                Language::Es => "Intercambiando claves y negociación directa entre pares...",
                Language::De => "Schlüsselaustausch und direkte Peer-Verhandlung...",
                Language::Pt => "Troca de chaves e negociação direta entre pares...",
            },
        ),
        ConnectionStatus::Disconnected => (
            ModernTheme::CARD_BG,
            match lang {
                Language::Fr => "Prêt à créer ou rejoindre un salon",
                Language::En => "Ready to host or join a room",
                Language::Es => "Listo para crear o unirse a una sala",
                Language::De => "Bereit, einen Raum zu erstellen oder beizutreten",
                Language::Pt => "Pronto para criar ou entrar numa sala",
            },
            ModernTheme::GREEN_VIVID,
            match lang {
                Language::Fr => "Créez un salon pour héberger vos amis en P2P direct ou rejoignez le salon d'un ami.",
                Language::En => "Create a room to host your friends over direct P2P or join a friend's room.",
                Language::Es => "Crea una sala para alojar a tus amigos en P2P directo o únete a la sala de un amigo.",
                Language::De => "Erstelle einen Raum für deine Freunde über direktes P2P oder tritt einem Raum bei.",
                Language::Pt => "Crie uma sala para receber os seus amigos em P2P direto ou junte-se a um amigo.",
            },
        ),
        ConnectionStatus::Error => (
            Color32::from_rgb(254, 242, 242),
            match lang {
                Language::Fr => "Impossible de joindre le salon",
                Language::En => "Unable to reach the room",
                Language::Es => "No se pudo conectar a la sala",
                Language::De => "Raum konnte nicht erreicht werden",
                Language::Pt => "Não foi possível ligar à sala",
            },
            ModernTheme::RED_ERROR,
            match lang {
                Language::Fr => "Vérifiez l'adresse ou le code du salon et réessayez.",
                Language::En => "Check the room address or code and try again.",
                Language::Es => "Verifica la dirección o el código de la sala e inténtalo de nuevo.",
                Language::De => "Überprüfe die Adresse oder den Raumcode und versuche es erneut.",
                Language::Pt => "Verifique o endereço ou código da sala e tente novamente.",
            },
        ),
    };

    egui::Frame::none()
        .fill(bg_color)
        .stroke(Stroke::new(1.0_f32, dot_color.gamma_multiply(0.45)))
        .inner_margin(14.0)
        .rounding(Rounding::same(10.0))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ModernTheme::draw_dot(ui, dot_color, 5.5);
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
    lang: Language,
    state: &mut AppState,
    config: &mut AppConfig,
    on_host_clicked: &mut bool,
    on_join_clicked: &mut bool,
) {
    egui::Frame::none()
        .fill(ModernTheme::CARD_BG)
        .stroke(Stroke::new(1.0_f32, ModernTheme::BORDER_SUBTLE))
        .inner_margin(14.0)
        .rounding(Rounding::same(10.0))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(match lang {
                        Language::Fr => "Votre pseudo :",
                        Language::En => "Your nickname:",
                        Language::Es => "Tu apodo:",
                        Language::De => "Dein Spielername:",
                        Language::Pt => "O seu nome:",
                    })
                    .color(ModernTheme::TEXT_PRIMARY)
                    .strong(),
                );
                ui.add(egui::TextEdit::singleline(&mut config.player_name).desired_width(160.0));

                ui.add_space(24.0);
                ui.label(
                    RichText::new(match lang {
                        Language::Fr => "Code du salon :",
                        Language::En => "Room Code:",
                        Language::Es => "Código de sala:",
                        Language::De => "Raumcode:",
                        Language::Pt => "Código da sala:",
                    })
                    .color(ModernTheme::TEXT_PRIMARY)
                    .strong(),
                );
                ui.add(egui::TextEdit::singleline(&mut state.room_code).desired_width(130.0));
            });
        });

    ui.add_space(12.0);

    ui.columns(2, |columns| {
        // Colonne 1 : HÉBERGER
        egui::Frame::none()
            .fill(ModernTheme::CARD_BG)
            .stroke(Stroke::new(1.5_f32, ModernTheme::BORDER_SUBTLE))
            .inner_margin(18.0)
            .rounding(Rounding::same(10.0))
            .show(&mut columns[0], |ui| {
                ui.horizontal(|ui| {
                    ModernTheme::draw_dot(ui, ModernTheme::BLUE_ELECTRIC, 4.5);
                    ui.label(
                        RichText::new(match lang {
                            Language::Fr => "Héberger un salon",
                            Language::En => "Host a Room",
                            Language::Es => "Crear una Sala",
                            Language::De => "Raum hosten",
                            Language::Pt => "Criar uma Sala",
                        })
                        .color(ModernTheme::BLUE_ELECTRIC)
                        .strong()
                        .size(17.0),
                    );
                });
                ui.add_space(6.0);
                ui.label(
                    RichText::new(match lang {
                        Language::Fr => "Créez un salon virtuel pour jouer en LAN ou partager votre écran et vos manettes avec vos amis en P2P direct.",
                        Language::En => "Create a virtual room to play LAN games or share your screen and controllers with friends over direct P2P.",
                        Language::Es => "Crea una sala virtual para jugar en LAN o compartir tu pantalla y mandos con tus amigos en P2P directo.",
                        Language::De => "Erstelle einen virtuellen Raum für LAN-Spiele oder teile deinen Bildschirm und Controller per P2P.",
                        Language::Pt => "Crie uma sala virtual para jogar em LAN ou partilhar o seu ecrã e comandos com amigos em P2P direto.",
                    })
                    .color(ModernTheme::TEXT_SECONDARY),
                );

                ui.add_space(18.0);
                let host_btn = egui::Button::new(
                    RichText::new(match lang {
                        Language::Fr => "Créer et héberger le salon",
                        Language::En => "Create & Host Room",
                        Language::Es => "Crear y Alojar Sala",
                        Language::De => "Raum erstellen & hosten",
                        Language::Pt => "Criar e Alojar Sala",
                    })
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
                ui.horizontal(|ui| {
                    ModernTheme::draw_dot(ui, ModernTheme::GREEN_VIVID, 4.5);
                    ui.label(
                        RichText::new(match lang {
                            Language::Fr => "Rejoindre un ami",
                            Language::En => "Join a Friend",
                            Language::Es => "Unirse a un Amigo",
                            Language::De => "Einem Freund beitreten",
                            Language::Pt => "Juntar-se a um Amigo",
                        })
                        .color(ModernTheme::GREEN_VIVID)
                        .strong()
                        .size(17.0),
                    );
                });
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(match lang {
                            Language::Fr => "Adresse de l'Hôte :",
                            Language::En => "Host Address:",
                            Language::Es => "Dirección del Anfitrión:",
                            Language::De => "Host-Adresse:",
                            Language::Pt => "Endereço do Anfitrião:",
                        })
                        .color(ModernTheme::TEXT_SECONDARY),
                    );
                    ui.add(
                        egui::TextEdit::singleline(&mut config.remote_server_address)
                            .desired_width(190.0),
                    );
                });

                ui.add_space(12.0);
                let join_btn = egui::Button::new(
                    RichText::new(match lang {
                        Language::Fr => "Rejoindre le salon",
                        Language::En => "Join Room",
                        Language::Es => "Unirse a la Sala",
                        Language::De => "Raum beitreten",
                        Language::Pt => "Entrar na Sala",
                    })
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

fn render_connecting_screen(ui: &mut Ui, lang: Language) {
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
                    RichText::new(match lang {
                        Language::Fr => "Connexion et synchronisation P2P en cours...",
                        Language::En => "Connecting and synchronizing P2P tunnel...",
                        Language::Es => "Conectando y sincronizando túnel P2P...",
                        Language::De => "Verbindung und P2P-Synchronisierung läuft...",
                        Language::Pt => "A ligar e sincronizar túnel P2P...",
                    })
                    .color(ModernTheme::BLUE_ELECTRIC)
                    .strong(),
                );
            });
        });
}

fn render_active_session(
    ui: &mut Ui,
    lang: Language,
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
                let (dot_col, role_text) = if state.role == NodeRole::Host {
                    (
                        ModernTheme::GREEN_VIVID,
                        match lang {
                            Language::Fr => "Hôte du salon",
                            Language::En => "Room Host",
                            Language::Es => "Anfitrión de la sala",
                            Language::De => "Raum-Host",
                            Language::Pt => "Anfitrião da sala",
                        },
                    )
                } else {
                    (
                        ModernTheme::BLUE_ELECTRIC,
                        match lang {
                            Language::Fr => "Client connecté en P2P",
                            Language::En => "P2P Connected Client",
                            Language::Es => "Cliente conectado en P2P",
                            Language::De => "Verbunden per P2P",
                            Language::Pt => "Cliente ligado em P2P",
                        },
                    )
                };

                ModernTheme::draw_dot(ui, dot_col, 4.5);
                ui.label(RichText::new(role_text).color(dot_col).strong());

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
                        RichText::new(match lang {
                            Language::Fr => "Quitter le salon",
                            Language::En => "Leave Room",
                            Language::Es => "Salir de la sala",
                            Language::De => "Raum verlassen",
                            Language::Pt => "Sair da sala",
                        })
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
                                RichText::new(match lang {
                                    Language::Fr => "Adresse directe P2P :",
                                    Language::En => "Direct P2P Address:",
                                    Language::Es => "Dirección P2P directa:",
                                    Language::De => "Direkte P2P-Adresse:",
                                    Language::Pt => "Endereço P2P direto:",
                                })
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
                                if ui
                                    .button(match lang {
                                        Language::Fr => "Copier l'adresse",
                                        Language::En => "Copy Address",
                                        Language::Es => "Copiar dirección",
                                        Language::De => "Adresse kopieren",
                                        Language::Pt => "Copiar endereço",
                                    })
                                    .clicked()
                                {
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
                                    RichText::new(match lang {
                                        Language::Fr => "Trafic 100% P2P direct (aucun transit serveur)",
                                        Language::En => "100% Direct P2P Traffic (zero server relay)",
                                        Language::Es => "Tráfico 100% P2P directo (sin tránsito servidor)",
                                        Language::De => "100% Direkter P2P-Verkehr",
                                        Language::Pt => "Tráfego 100% P2P direto",
                                    })
                                    .color(ModernTheme::GREEN_VIVID)
                                    .small(),
                                );
                                ModernTheme::draw_dot(ui, ModernTheme::GREEN_VIVID, 3.5);
                            });
                        });
                    });
            }
        });
}

fn render_peers_scoreboard(ui: &mut Ui, lang: Language, state: &AppState) {
    egui::Frame::none()
        .fill(ModernTheme::CARD_BG)
        .stroke(Stroke::new(1.0_f32, ModernTheme::BORDER_SUBTLE))
        .inner_margin(16.0)
        .rounding(Rounding::same(10.0))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(match lang {
                        Language::Fr => format!("Participants connectés au salon ({})", state.peers.len()),
                        Language::En => format!("Connected Room Players ({})", state.peers.len()),
                        Language::Es => format!("Jugadores conectados a la sala ({})", state.peers.len()),
                        Language::De => format!("Verbundene Spieler im Raum ({})", state.peers.len()),
                        Language::Pt => format!("Jogadores ligados à sala ({})", state.peers.len()),
                    })
                    .color(ModernTheme::TEXT_PRIMARY)
                    .strong()
                    .size(15.5),
                );
                ui.label(
                    RichText::new("— P2P Direct")
                        .color(ModernTheme::TEXT_MUTED)
                        .small(),
                );
            });

            ui.add_space(10.0);

            if state.peers.is_empty() {
                ui.label(
                    RichText::new(match lang {
                        Language::Fr => "Aucun joueur connecté pour le moment. Créez ou rejoignez un salon pour commencer.",
                        Language::En => "No players connected yet. Create or join a room to start.",
                        Language::Es => "Ningún jugador conectado por el momento. Crea o únete a una sala para empezar.",
                        Language::De => "Noch keine Spieler verbunden. Erstelle einen Raum oder tritt einem bei.",
                        Language::Pt => "Nenhum jogador ligado de momento. Crie ou entre numa sala para começar.",
                    })
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
