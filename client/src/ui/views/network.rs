use crate::config::AppConfig;
use crate::i18n::Language;
use crate::types::AppState;
use crate::ui::retro_theme::ModernTheme;
use egui::{Color32, RichText, Rounding, Stroke, Ui};

pub struct NetworkView;

impl NetworkView {
    pub fn render(
        ui: &mut Ui,
        state: &mut AppState,
        config: &mut AppConfig,
        on_apply_metric_clicked: &mut bool,
        on_refresh_interfaces_clicked: &mut bool,
    ) {
        let lang = config.language;
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                // Carte principale Métrique Réseau
                egui::Frame::none()
                    .fill(ModernTheme::CARD_BG)
                    .stroke(Stroke::new(1.0_f32, ModernTheme::BORDER_SUBTLE))
                    .inner_margin(16.0)
                    .rounding(Rounding::same(10.0))
                    .show(ui, |ui| {
                        ui.label(
                            RichText::new(match lang {
                                Language::Fr => "Priorité de l'interface réseau (Métrique basse)",
                                Language::En => "Network Adapter Priority (Low Interface Metric)",
                                Language::Es => "Prioridad de la interfaz de red (Métrica baja)",
                                Language::De => "Netzwerkadapter-Priorität (Niedrige Metrik)",
                                Language::Pt => "Prioridade da interface de rede (Métrica baixa)",
                            })
                            .color(ModernTheme::BLUE_ELECTRIC)
                            .strong()
                            .size(16.0),
                        );
                        ui.add_space(6.0);

                        ui.label(
                            RichText::new(match lang {
                                Language::Fr => "Une métrique basse (valeur 5 recommandée) place l'interface virtuelle CoopLink en priorité devant votre carte Wi-Fi ou votre box internet pour la découverte des jeux locaux.",
                                Language::En => "A low metric (recommended value: 5) places the CoopLink virtual adapter ahead of your Wi-Fi or router interface for instant local LAN game discovery.",
                                Language::Es => "Una métrica baja (valor 5 recomendado) coloca la interfaz virtual CoopLink por delante de tu tarjeta Wi-Fi o router para descubrir partidas LAN.",
                                Language::De => "Eine niedrige Metrik (Empfehlung: 5) priorisiert den virtuellen CoopLink-Adapter vor WLAN oder Router für die LAN-Spielsuche.",
                                Language::Pt => "Uma métrica baixa (valor 5 recomendado) coloca a interface virtual CoopLink à frente do Wi-Fi ou router para descoberta de jogos LAN.",
                            })
                            .color(ModernTheme::TEXT_SECONDARY),
                        );

                        ui.add_space(12.0);

                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new(match lang {
                                    Language::Fr => "Métrique cible :",
                                    Language::En => "Target Metric:",
                                    Language::Es => "Métrica objetivo:",
                                    Language::De => "Ziel-Metrik:",
                                    Language::Pt => "Métrica alvo:",
                                })
                                .color(ModernTheme::TEXT_PRIMARY)
                                .strong(),
                            );
                            ui.add(egui::Slider::new(&mut config.target_metric, 1..=50).text(""));

                            if config.target_metric <= 10 {
                                ModernTheme::status_pill(
                                    ui,
                                    ModernTheme::GREEN_VIVID,
                                    match lang {
                                        Language::Fr => "Priorité maximale (passe avant la box)",
                                        Language::En => "Highest Priority (passes ahead of router)",
                                        Language::Es => "Prioridad máxima (pasa antes del router)",
                                        Language::De => "Höchste Priorität (vor dem Router)",
                                        Language::Pt => "Prioridade máxima (antes do router)",
                                    },
                                );
                            } else {
                                ModernTheme::status_pill(
                                    ui,
                                    ModernTheme::ORANGE_CORAL,
                                    match lang {
                                        Language::Fr => "Valeur élevée (risque de passer après le Wi-Fi)",
                                        Language::En => "High value (may rank behind Wi-Fi)",
                                        _ => "High value",
                                    },
                                );
                            }
                        });

                        ui.add_space(10.0);

                        ui.horizontal(|ui| {
                            let btn = egui::Button::new(
                                RichText::new(match lang {
                                    Language::Fr => "Appliquer la métrique prioritaire",
                                    Language::En => "Apply Priority Metric",
                                    Language::Es => "Aplicar métrica prioritaria",
                                    Language::De => "Prioritätsmetrik anwenden",
                                    Language::Pt => "Aplicar métrica prioritária",
                                })
                                .color(Color32::WHITE)
                                .strong(),
                            )
                            .fill(ModernTheme::BLUE_ELECTRIC)
                            .rounding(Rounding::same(8.0));

                            if ui.add(btn).clicked() {
                                *on_apply_metric_clicked = true;
                            }

                            if state.metric_applied {
                                ModernTheme::status_pill(
                                    ui,
                                    ModernTheme::GREEN_VIVID,
                                    match lang {
                                        Language::Fr => "Métrique prioritaire active",
                                        Language::En => "Priority Metric Active",
                                        Language::Es => "Métrica prioritaria activa",
                                        Language::De => "Prioritätsmetrik aktiv",
                                        Language::Pt => "Métrica prioritária ativa",
                                    },
                                );
                            }
                        });
                    });

                ui.add_space(12.0);

                // Carte Relais Broadcast UDP
                egui::Frame::none()
                    .fill(ModernTheme::CARD_BG)
                    .stroke(Stroke::new(1.0_f32, ModernTheme::BORDER_SUBTLE))
                    .inner_margin(16.0)
                    .rounding(Rounding::same(10.0))
                    .show(ui, |ui| {
                        ui.label(
                            RichText::new(match lang {
                                Language::Fr => "Découverte automatique des parties LAN (Broadcast UDP P2P)",
                                Language::En => "Automatic LAN Game Discovery (P2P UDP Broadcast)",
                                Language::Es => "Descubrimiento automático de partidas LAN (Broadcast UDP P2P)",
                                Language::De => "Automatische LAN-Spielerkennung (P2P UDP Broadcast)",
                                Language::Pt => "Descoberta automática de jogos LAN (Broadcast UDP P2P)",
                            })
                            .color(ModernTheme::GREEN_VIVID)
                            .strong()
                            .size(16.0),
                        );
                        ui.add_space(4.0);

                        ui.label(
                            RichText::new(match lang {
                                Language::Fr => "Capture et achemine en P2P direct les annonces de salons locaux (255.255.255.255) vers vos amis connectés.",
                                Language::En => "Captures and forwards local LAN game broadcasts (255.255.255.255) directly over P2P to your connected friends.",
                                Language::Es => "Captura y encamina directamente en P2P los anuncios de salas locales (255.255.255.255) hacia tus amigos.",
                                Language::De => "Erfasst lokale LAN-Spielankündigungen (255.255.255.255) und leitet sie direkt per P2P an verbundene Freunde weiter.",
                                Language::Pt => "Captura e encaminha diretamente em P2P os anúncios de salas locais (255.255.255.255) para os seus amigos.",
                            })
                            .color(ModernTheme::TEXT_SECONDARY),
                        );

                        ui.add_space(8.0);
                        ui.label(
                            RichText::new(match lang {
                                Language::Fr => format!("Paquets de découverte LAN acheminés : {}", state.broadcast_packets_relayed),
                                Language::En => format!("LAN discovery packets forwarded: {}", state.broadcast_packets_relayed),
                                Language::Es => format!("Paquetes de descubrimiento LAN encaminados: {}", state.broadcast_packets_relayed),
                                Language::De => format!("Weitergeleitete LAN-Erkennungspakete: {}", state.broadcast_packets_relayed),
                                Language::Pt => format!("Pacotes de descoberta LAN encaminhados: {}", state.broadcast_packets_relayed),
                            })
                            .color(ModernTheme::BLUE_ELECTRIC)
                            .strong(),
                        );
                    });

                ui.add_space(12.0);

                // Liste des interfaces Windows détectées (Asynchrone)
                egui::Frame::none()
                    .fill(ModernTheme::CARD_BG)
                    .stroke(Stroke::new(1.0_f32, ModernTheme::BORDER_SUBTLE))
                    .inner_margin(16.0)
                    .rounding(Rounding::same(10.0))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new(match lang {
                                    Language::Fr => "Cartes réseau détectées sur le système",
                                    Language::En => "Detected System Network Adapters",
                                    Language::Es => "Tarjetas de red detectadas",
                                    Language::De => "Erkannte Netzwerkadapter",
                                    Language::Pt => "Placas de rede detetadas",
                                })
                                .color(ModernTheme::TEXT_PRIMARY)
                                .strong()
                                .size(16.0),
                            );

                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                if state.is_refreshing_interfaces {
                                    ui.spinner();
                                    ui.label(
                                        RichText::new(match lang {
                                            Language::Fr => "Actualisation...",
                                            Language::En => "Refreshing...",
                                            _ => "...",
                                        })
                                        .color(ModernTheme::BLUE_ELECTRIC)
                                        .small(),
                                    );
                                } else if ui
                                    .button(match lang {
                                        Language::Fr => "Actualiser la liste",
                                        Language::En => "Refresh List",
                                        Language::Es => "Actualizar lista",
                                        Language::De => "Liste aktualisieren",
                                        Language::Pt => "Atualizar lista",
                                    })
                                    .clicked()
                                {
                                    *on_refresh_interfaces_clicked = true;
                                }
                            });
                        });
                        ui.add_space(8.0);

                        if state.cached_interfaces.is_empty() && !state.is_refreshing_interfaces {
                            *on_refresh_interfaces_clicked = true;
                        }

                        if state.is_refreshing_interfaces && state.cached_interfaces.is_empty() {
                            ui.horizontal(|ui| {
                                ui.spinner();
                                ui.label(
                                    RichText::new(match lang {
                                        Language::Fr => "Analyse des interfaces réseau en tâche de fond...",
                                        Language::En => "Scanning network interfaces in background...",
                                        _ => "Scanning network interfaces...",
                                    })
                                    .color(ModernTheme::TEXT_SECONDARY),
                                );
                            });
                        } else if state.cached_interfaces.is_empty() {
                            ui.label(
                                RichText::new(match lang {
                                    Language::Fr => "Aucune interface détectée.",
                                    Language::En => "No network interface detected.",
                                    _ => "No interface detected.",
                                })
                                .color(ModernTheme::TEXT_MUTED),
                            );
                        } else {
                            egui::Grid::new("interfaces_grid")
                                .num_columns(3)
                                .spacing([28.0, 8.0])
                                .striped(true)
                                .show(ui, |ui| {
                                    ui.label(
                                        RichText::new(match lang {
                                            Language::Fr => "INTERFACE",
                                            Language::En => "ADAPTER",
                                            Language::Es => "INTERFAZ",
                                            Language::De => "ADAPTER",
                                            Language::Pt => "INTERFACE",
                                        })
                                        .color(ModernTheme::BLUE_ELECTRIC)
                                        .strong(),
                                    );
                                    ui.label(
                                        RichText::new(match lang {
                                            Language::Fr => "MÉTRIQUE",
                                            Language::En => "METRIC",
                                            Language::Es => "MÉTRICA",
                                            Language::De => "METRIK",
                                            Language::Pt => "MÉTRICA",
                                        })
                                        .color(ModernTheme::BLUE_ELECTRIC)
                                        .strong(),
                                    );
                                    ui.label(
                                        RichText::new(match lang {
                                            Language::Fr => "PRIORITÉ",
                                            Language::En => "PRIORITY",
                                            Language::Es => "PRIORIDAD",
                                            Language::De => "PRIORITÄT",
                                            Language::Pt => "PRIORIDADE",
                                        })
                                        .color(ModernTheme::BLUE_ELECTRIC)
                                        .strong(),
                                    );
                                    ui.end_row();

                                    for iface in &state.cached_interfaces {
                                        let is_coop = iface.name.contains("CoopLink");
                                        let name_text = if is_coop {
                                            RichText::new(&iface.name).color(ModernTheme::GREEN_VIVID).strong()
                                        } else {
                                            RichText::new(&iface.name).color(ModernTheme::TEXT_PRIMARY)
                                        };

                                        ui.label(name_text);
                                        ui.label(
                                            RichText::new(format!("{}", iface.metric))
                                                .color(if iface.metric <= 10 {
                                                    ModernTheme::GREEN_VIVID
                                                } else {
                                                    ModernTheme::TEXT_SECONDARY
                                                })
                                                .strong(),
                                        );

                                        if iface.metric <= 10 {
                                            ui.label(
                                                RichText::new(match lang {
                                                    Language::Fr => "Haute (Prioritaire)",
                                                    Language::En => "High (Priority)",
                                                    Language::Es => "Alta (Prioritaria)",
                                                    Language::De => "Hoch (Priorität)",
                                                    Language::Pt => "Alta (Prioritária)",
                                                })
                                                .color(ModernTheme::GREEN_VIVID),
                                            );
                                        } else if iface.metric <= 30 {
                                            ui.label(
                                                RichText::new(match lang {
                                                    Language::Fr => "Normale (Ethernet)",
                                                    Language::En => "Normal (Ethernet)",
                                                    Language::Es => "Normal (Ethernet)",
                                                    Language::De => "Normal (Ethernet)",
                                                    Language::Pt => "Normal (Ethernet)",
                                                })
                                                .color(ModernTheme::BLUE_ELECTRIC),
                                            );
                                        } else {
                                            ui.label(
                                                RichText::new(match lang {
                                                    Language::Fr => "Standard (Wi-Fi)",
                                                    Language::En => "Standard (Wi-Fi)",
                                                    _ => "Standard (Wi-Fi)",
                                                })
                                                .color(ModernTheme::TEXT_MUTED),
                                            );
                                        }
                                        ui.end_row();
                                    }
                                });
                        }
                    });
            });
    }
}
