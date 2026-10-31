use crate::config::AppConfig;
use crate::i18n::Language;
use crate::platform::windows::process::set_windows_autostart;
use crate::ui::retro_theme::ModernTheme;
use egui::{Color32, CursorIcon, RichText, Rounding, Stroke, Ui};

pub struct SettingsView;

impl SettingsView {
    pub fn render(ui: &mut Ui, config: &mut AppConfig, saved_notice: &mut bool) {
        let lang = config.language;
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
                            RichText::new(match lang {
                                Language::Fr => "Paramètres de l'application & Comportement Windows",
                                Language::En => "Application Settings & Windows Behavior",
                                Language::Es => "Ajustes de la Aplicación y Windows",
                                Language::De => "Anwendungseinstellungen & Windows-Verhalten",
                                Language::Pt => "Definições da Aplicação e Windows",
                            })
                            .color(ModernTheme::BLUE_ELECTRIC)
                            .strong()
                            .size(16.5),
                        );
                        ui.add_space(4.0);
                        ui.label(
                            RichText::new(match lang {
                                Language::Fr => "Configurez la langue, le lancement au démarrage de Windows, la réduction en arrière-plan et le serveur Hub.",
                                Language::En => "Configure language, Windows startup launch, minimize-on-close behavior and the Hub server.",
                                Language::Es => "Configura el idioma, inicio con Windows, reducción en segundo plano y el servidor Hub.",
                                Language::De => "Sprache, Autostart mit Windows, Minimieren beim Schließen und Hub-Server konfigurieren.",
                                Language::Pt => "Configure o idioma, arranque com o Windows, redução em segundo plano e o servidor Hub.",
                            })
                            .color(ModernTheme::TEXT_SECONDARY),
                        );

                        ui.add_space(14.0);

                        egui::Grid::new("settings_grid")
                            .num_columns(2)
                            .spacing([24.0, 12.0])
                            .show(ui, |ui| {
                                ui.label(
                                    RichText::new(match lang {
                                        Language::Fr => "Langue de l'interface (OS auto) :",
                                        Language::En => "Interface Language (OS auto):",
                                        Language::Es => "Idioma de la interfaz:",
                                        Language::De => "Sprache der Oberfläche:",
                                        Language::Pt => "Idioma da interface:",
                                    })
                                    .color(ModernTheme::TEXT_PRIMARY)
                                    .strong(),
                                );
                                egui::ComboBox::from_id_salt("settings_lang_combo")
                                    .selected_text(config.language.display_name())
                                    .show_ui(ui, |ui| {
                                        for &l in Language::all() {
                                            ui.selectable_value(&mut config.language, l, l.display_name());
                                        }
                                    });
                                ui.end_row();

                                ui.label(
                                    RichText::new(match lang {
                                        Language::Fr => "Pseudo par défaut :",
                                        Language::En => "Default Nickname:",
                                        Language::Es => "Apodo predeterminado:",
                                        Language::De => "Standard-Spielername:",
                                        Language::Pt => "Nome predefinido:",
                                    })
                                    .color(ModernTheme::TEXT_PRIMARY)
                                    .strong(),
                                );
                                ui.text_edit_singleline(&mut config.player_name);
                                ui.end_row();

                                ui.label(
                                    RichText::new(match lang {
                                        Language::Fr => "Adresse du Hub CoopLink (IP ou Domaine) :",
                                        Language::En => "CoopLink Hub Address (IP or Domain):",
                                        Language::Es => "Dirección del Hub CoopLink:",
                                        Language::De => "CoopLink Hub-Adresse:",
                                        Language::Pt => "Endereço do Hub CoopLink:",
                                    })
                                    .color(ModernTheme::TEXT_PRIMARY)
                                    .strong(),
                                );
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

                        if ui
                            .checkbox(
                                &mut config.start_with_windows,
                                RichText::new(match lang {
                                    Language::Fr => "Lancer automatiquement CoopLink au démarrage de Windows",
                                    Language::En => "Launch CoopLink automatically when Windows starts",
                                    Language::Es => "Iniciar CoopLink automáticamente al arrancar Windows",
                                    Language::De => "CoopLink automatisch beim Windows-Start ausführen",
                                    Language::Pt => "Iniciar o CoopLink automaticamente com o Windows",
                                })
                                .color(ModernTheme::TEXT_PRIMARY)
                                .strong(),
                            )
                            .changed()
                        {
                            set_windows_autostart(config.start_with_windows);
                            let _ = config.save();
                        }

                        ui.checkbox(
                            &mut config.close_to_background,
                            RichText::new(match lang {
                                Language::Fr => "Réduire en arrière-plan lorsque je clique sur la croix (X) pour ne pas couper le jeu",
                                Language::En => "Minimize to background when clicking the close button (X) to keep games running",
                                Language::Es => "Minimizar en segundo plano al cerrar con la cruz (X) para no cortar el juego",
                                Language::De => "Beim Klick auf Schließen (X) im Hintergrund weiterlaufen lassen",
                                Language::Pt => "Minimizar para segundo plano ao clicar no botão fechar (X)",
                            })
                            .color(ModernTheme::TEXT_PRIMARY)
                            .strong(),
                        );

                        ui.checkbox(
                            &mut config.auto_metric_optimization,
                            RichText::new(match lang {
                                Language::Fr => "Appliquer automatiquement la métrique basse prioritaire (5) au lancement",
                                Language::En => "Automatically apply high-priority low interface metric (5) on startup",
                                Language::Es => "Aplicar automáticamente métrica prioritaria baja (5)",
                                Language::De => "Niedrige Prioritätsmetrik (5) automatisch anwenden",
                                Language::Pt => "Aplicar automaticamente métrica prioritária baixa (5)",
                            })
                            .color(ModernTheme::TEXT_PRIMARY),
                        );

                        ui.checkbox(
                            &mut config.auto_firewall_rule,
                            RichText::new(match lang {
                                Language::Fr => "Autoriser automatiquement CoopLink dans le pare-feu Windows",
                                Language::En => "Automatically allow CoopLink in Windows Defender Firewall",
                                Language::Es => "Permitir automáticamente CoopLink en el cortafuegos de Windows",
                                Language::De => "CoopLink automatisch in der Windows-Firewall zulassen",
                                Language::Pt => "Permitir automaticamente o CoopLink na firewall do Windows",
                            })
                            .color(ModernTheme::TEXT_PRIMARY),
                        );

                        ui.checkbox(
                            &mut config.enable_broadcast_reflector,
                            RichText::new(match lang {
                                Language::Fr => "Activer la découverte automatique des salons LAN (Broadcast UDP)",
                                Language::En => "Enable automatic LAN room discovery (UDP Broadcast)",
                                Language::Es => "Activar descubrimiento automático de salas LAN (Broadcast UDP)",
                                Language::De => "Automatische LAN-Raumerkennung aktivieren (UDP Broadcast)",
                                Language::Pt => "Ativar descoberta automática de salas LAN (Broadcast UDP)",
                            })
                            .color(ModernTheme::TEXT_PRIMARY),
                        );

                        ui.checkbox(
                            &mut config.enable_upnp,
                            RichText::new(match lang {
                                Language::Fr => "Activer l'ouverture de port automatique UPnP sur la box",
                                Language::En => "Enable automatic UPnP port forwarding on router",
                                Language::Es => "Activar apertura de puertos automática UPnP en el router",
                                Language::De => "Automatische UPnP-Portfreigabe am Router aktivieren",
                                Language::Pt => "Ativar abertura automática de portas UPnP no router",
                            })
                            .color(ModernTheme::TEXT_PRIMARY),
                        );

                        ui.add_space(16.0);
                        ui.horizontal(|ui| {
                            let save_btn = egui::Button::new(
                                RichText::new(match lang {
                                    Language::Fr => "Enregistrer les paramètres",
                                    Language::En => "Save Settings",
                                    Language::Es => "Guardar ajustes",
                                    Language::De => "Einstellungen speichern",
                                    Language::Pt => "Guardar definições",
                                })
                                .color(Color32::WHITE)
                                .strong(),
                            )
                            .fill(ModernTheme::BLUE_ELECTRIC)
                            .rounding(Rounding::same(8.0));

                            if ui.add(save_btn).on_hover_cursor(CursorIcon::PointingHand).clicked() {
                                set_windows_autostart(config.start_with_windows);
                                let _ = config.save();
                                *saved_notice = true;
                            }

                            if *saved_notice {
                                ModernTheme::status_pill(
                                    ui,
                                    ModernTheme::GREEN_VIVID,
                                    match lang {
                                        Language::Fr => "Paramètres enregistrés",
                                        Language::En => "Settings saved",
                                        Language::Es => "Ajustes guardados",
                                        Language::De => "Gespeichert",
                                        Language::Pt => "Definições guardadas",
                                    },
                                );
                            }

                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                let quit_btn = egui::Button::new(
                                    RichText::new(match lang {
                                        Language::Fr => "Quitter complètement CoopLink",
                                        Language::En => "Quit CoopLink Completely",
                                        Language::Es => "Salir completamente de CoopLink",
                                        Language::De => "CoopLink vollständig beenden",
                                        Language::Pt => "Sair completamente do CoopLink",
                                    })
                                    .color(Color32::WHITE)
                                    .strong(),
                                )
                                .fill(ModernTheme::RED_ERROR)
                                .rounding(Rounding::same(8.0));

                                if ui.add(quit_btn).on_hover_cursor(CursorIcon::PointingHand).clicked() {
                                    std::process::exit(0);
                                }
                            });
                        });
                    });
            });
    }
}
