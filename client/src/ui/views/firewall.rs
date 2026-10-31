use crate::config::AppConfig;
use crate::i18n::Language;
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
        let lang = config.language;
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
                            RichText::new(match lang {
                                Language::Fr => "Sécurité & Pare-feu Système (Windows Defender / macOS / Linux)",
                                Language::En => "System Security & Firewall (Windows Defender / macOS / Linux)",
                                Language::Es => "Seguridad y Cortafuegos del Sistema",
                                Language::De => "Systemsicherheit & Firewall",
                                Language::Pt => "Segurança e Firewall do Sistema",
                            })
                            .color(ModernTheme::BLUE_ELECTRIC)
                            .strong()
                            .size(16.0),
                        );
                        ui.add_space(6.0);

                        ui.label(
                            RichText::new(match lang {
                                Language::Fr => "CoopLink configure automatiquement les règles d'autorisation dans le pare-feu afin d'autoriser le maillage P2P (10.244.0.0/24) et le partage de jeu coopératif sans coupure.",
                                Language::En => "CoopLink automatically configures firewall rules on startup to allow the encrypted P2P mesh (10.244.0.0/24) and low-latency co-op game sharing.",
                                Language::Es => "CoopLink configura automáticamente las reglas del cortafuegos para permitir la malla P2P (10.244.0.0/24) y la compartición de juego cooperativo.",
                                Language::De => "CoopLink konfiguriert automatisch die Firewall-Regeln, um das P2P-Netzwerk (10.244.0.0/24) und die Co-op-Spielfreigabe zuzulassen.",
                                Language::Pt => "O CoopLink configura automaticamente as regras da firewall para permitir a rede P2P (10.244.0.0/24) e a partilha de jogo cooperativo.",
                            })
                            .color(ModernTheme::TEXT_SECONDARY),
                        );

                        ui.add_space(12.0);

                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new(match lang {
                                    Language::Fr => "État des règles :",
                                    Language::En => "Firewall Rules Status:",
                                    Language::Es => "Estado de las reglas:",
                                    Language::De => "Regelstatus:",
                                    Language::Pt => "Estado das regras:",
                                })
                                .color(ModernTheme::TEXT_PRIMARY)
                                .strong(),
                            );

                            if state.firewall_ok {
                                ModernTheme::status_pill(
                                    ui,
                                    ModernTheme::GREEN_VIVID,
                                    match lang {
                                        Language::Fr => "Configuré et opérationnel",
                                        Language::En => "Configured & Operational",
                                        Language::Es => "Configurado y operativo",
                                        Language::De => "Konfiguriert & Aktiv",
                                        Language::Pt => "Configurado e operacional",
                                    },
                                );
                            } else {
                                ModernTheme::status_pill(
                                    ui,
                                    ModernTheme::ORANGE_CORAL,
                                    match lang {
                                        Language::Fr => "Configuration recommandée",
                                        Language::En => "Configuration Recommended",
                                        _ => "Configuration Recommended",
                                    },
                                );
                            }
                        });

                        ui.add_space(12.0);

                        ui.horizontal(|ui| {
                            let btn = egui::Button::new(
                                RichText::new(match lang {
                                    Language::Fr => "Appliquer / Réparer les règles pare-feu",
                                    Language::En => "Apply / Repair Firewall Rules",
                                    Language::Es => "Aplicar / Reparar reglas del cortafuegos",
                                    Language::De => "Firewall-Regeln anwenden / reparieren",
                                    Language::Pt => "Aplicar / Reparar regras da firewall",
                                })
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
                            RichText::new(match lang {
                                Language::Fr => "Règles réseau enregistrées",
                                Language::En => "Registered Network Rules",
                                Language::Es => "Reglas de red registradas",
                                Language::De => "Registrierte Netzwerkregeln",
                                Language::Pt => "Regras de rede registadas",
                            })
                            .color(ModernTheme::TEXT_PRIMARY)
                            .strong()
                            .size(15.5),
                        );
                        ui.add_space(8.0);

                        render_rule_item(
                            ui,
                            match lang {
                                Language::Fr => "CoopLink — Tunnel P2P Direct (UDP/TCP)",
                                Language::En => "CoopLink — Direct P2P Tunnel (UDP/TCP)",
                                _ => "CoopLink — Direct P2P Tunnel (UDP/TCP)",
                            },
                            match lang {
                                Language::Fr => "Autorise les flux directs entre pairs sans passer par le serveur central.",
                                Language::En => "Allows direct encrypted peer-to-peer streams without passing through the central server.",
                                Language::Es => "Permite flujos directos entre pares sin pasar por el servidor central.",
                                Language::De => "Erlaubt direkte P2P-Verbindungen ohne zentralen Server.",
                                Language::Pt => "Permite fluxos diretos entre pares sem passar pelo servidor central.",
                            },
                        );
                        let sub_title = match lang {
                            Language::Fr => format!("CoopLink — Sous-réseau Virtuel ({}.0/24)", config.virtual_subnet_prefix),
                            Language::En => format!("CoopLink — Virtual Subnet ({}.0/24)", config.virtual_subnet_prefix),
                            _ => format!("CoopLink — Virtual Subnet ({}.0/24)", config.virtual_subnet_prefix),
                        };
                        render_rule_item(
                            ui,
                            &sub_title,
                            match lang {
                                Language::Fr => "Autorise le trafic LAN et le partage d'écran Co-op strictement sur l'interface virtuelle.",
                                Language::En => "Allows LAN game traffic and Co-op screen sharing strictly over the virtual interface.",
                                Language::Es => "Permite el tráfico LAN y la pantalla compartida estrictamente en la interfaz virtual.",
                                Language::De => "Erlaubt LAN-Verkehr und Bildschirmfreigabe ausschließlich über das virtuelle Interface.",
                                Language::Pt => "Permite o tráfego LAN e a partilha de ecrã estritamente na interface virtual.",
                            },
                        );
                        render_rule_item(
                            ui,
                            match lang {
                                Language::Fr => "CoopLink — Découverte LAN & Manettes Multi-Joueurs",
                                Language::En => "CoopLink — LAN Discovery & Multi-Player Controllers",
                                _ => "CoopLink — LAN Discovery & Controllers",
                            },
                            match lang {
                                Language::Fr => "Autorise la découverte des salons locaux et la synchronisation basse latence des manettes.",
                                Language::En => "Allows local room discovery and low-latency multi-player controller synchronization.",
                                Language::Es => "Permite el descubrimiento de salas locales y la sincronización de mandos.",
                                Language::De => "Erlaubt die lokale Raumerkennung und Controller-Synchronisierung mit niedriger Latenz.",
                                Language::Pt => "Permite a descoberta de salas locais e a sincronização de comandos.",
                            },
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
