use super::actions::*;
use eframe::egui;
use egui::{Color32, FontFamily, FontId, RichText, Rounding, ScrollArea, Stroke, TextStyle, Ui};
use std::env;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

// Palette Windows 11 Fluent & Couleurs de l'icône CoopLink.png
const WIN11_BG: Color32 = Color32::from_rgb(244, 247, 251);         // #F4F7FB
const WIN11_CARD: Color32 = Color32::from_rgb(255, 255, 255);       // #FFFFFF
const WIN11_SUBTLE: Color32 = Color32::from_rgb(248, 250, 252);     // #F8FAFC
const WIN11_BORDER: Color32 = Color32::from_rgb(226, 232, 240);     // #E2E8F0
const WIN11_BLUE: Color32 = Color32::from_rgb(0, 102, 255);         // #0066FF
const WIN11_GREEN: Color32 = Color32::from_rgb(0, 186, 136);        // #00BA88
const WIN11_TEXT_MAIN: Color32 = Color32::from_rgb(15, 23, 42);     // #0F172A
const WIN11_TEXT_SUB: Color32 = Color32::from_rgb(71, 85, 105);     // #475569
const WIN11_TEXT_MUTED: Color32 = Color32::from_rgb(100, 116, 139); // #64748B

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstallerLang {
    Fr,
    En,
    Es,
    De,
    Pt,
}

impl InstallerLang {
    pub fn all() -> &'static [InstallerLang] {
        &[
            InstallerLang::Fr,
            InstallerLang::En,
            InstallerLang::Es,
            InstallerLang::De,
            InstallerLang::Pt,
        ]
    }

    pub fn code(&self) -> &'static str {
        match self {
            InstallerLang::Fr => "FR",
            InstallerLang::En => "EN",
            InstallerLang::Es => "ES",
            InstallerLang::De => "DE",
            InstallerLang::Pt => "PT",
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            InstallerLang::Fr => "Français",
            InstallerLang::En => "English",
            InstallerLang::Es => "Español",
            InstallerLang::De => "Deutsch",
            InstallerLang::Pt => "Português",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstallStep {
    Welcome,
    License,
    Destination,
    Options,
    Installing,
    Finished,
}

pub struct InstallerApp {
    lang: InstallerLang,
    step: InstallStep,
    install_dir: String,
    license_accepted: bool,
    create_desktop_shortcut: bool,
    create_start_menu_shortcut: bool,
    configure_firewall: bool,
    launch_on_finish: bool,
    progress: f32,
    status_label: String,
    install_started: Arc<AtomicBool>,
    install_complete: Arc<AtomicBool>,
    error_msg: Option<String>,
    logo_texture: Option<egui::TextureHandle>,
}

impl InstallerApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        setup_installer_theme(&cc.egui_ctx);
        let default_dir = get_default_install_dir();
        let logo_texture = load_installer_logo(&cc.egui_ctx);

        Self {
            lang: InstallerLang::Fr,
            step: InstallStep::Welcome,
            install_dir: default_dir.to_string_lossy().to_string(),
            license_accepted: false,
            create_desktop_shortcut: true,
            create_start_menu_shortcut: true,
            configure_firewall: true,
            launch_on_finish: true,
            progress: 0.0,
            status_label: "Prêt pour l'installation...".to_string(),
            install_started: Arc::new(AtomicBool::new(false)),
            install_complete: Arc::new(AtomicBool::new(false)),
            error_msg: None,
            logo_texture,
        }
    }
}

fn draw_dot(ui: &mut Ui, color: Color32, radius: f32) {
    let size = egui::vec2(radius * 2.8, radius * 2.8);
    let (rect, _) = ui.allocate_exact_size(size, egui::Sense::hover());
    if ui.is_rect_visible(rect) {
        let center = rect.center();
        let halo = Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), 45);
        ui.painter().circle_filled(center, radius * 1.45, halo);
        ui.painter().circle_filled(center, radius, color);
    }
}

impl eframe::App for InstallerApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let lang = self.lang;

        // En-tête Classique Moderne Windows 11 (Fond Blanc Pur)
        egui::TopBottomPanel::top("installer_header")
            .frame(
                egui::Frame::none()
                    .fill(WIN11_CARD)
                    .inner_margin(egui::Margin::symmetric(22.0, 14.0))
                    .stroke(Stroke::new(1.0_f32, WIN11_BORDER)),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    if let Some(ref logo) = self.logo_texture {
                        ui.image((logo.id(), egui::vec2(40.0, 40.0)));
                    }
                    ui.add_space(6.0);
                    ui.vertical(|ui| {
                        ui.label(
                            RichText::new(match lang {
                                InstallerLang::Fr => "Installation de CoopLink",
                                InstallerLang::En => "CoopLink Setup",
                                InstallerLang::Es => "Instalación de CoopLink",
                                InstallerLang::De => "CoopLink Installation",
                                InstallerLang::Pt => "Instalação do CoopLink",
                            })
                            .color(WIN11_TEXT_MAIN)
                            .strong()
                            .size(18.0),
                        );
                        let step_subtitle = match self.step {
                            InstallStep::Welcome => match lang {
                                InstallerLang::Fr => "Étape 1 sur 5 — Présentation & Bienvenue",
                                InstallerLang::En => "Step 1 of 5 — Overview & Welcome",
                                InstallerLang::Es => "Paso 1 de 5 — Presentación y Bienvenida",
                                InstallerLang::De => "Schritt 1 von 5 — Übersicht & Willkommen",
                                InstallerLang::Pt => "Passo 1 de 5 — Apresentação e Boas-vindas",
                            },
                            InstallStep::License => match lang {
                                InstallerLang::Fr => "Étape 2 sur 5 — Contrat de licence logicielle",
                                InstallerLang::En => "Step 2 of 5 — Software License Agreement",
                                InstallerLang::Es => "Paso 2 de 5 — Acuerdo de licencia",
                                InstallerLang::De => "Schritt 2 von 5 — Lizenzvereinbarung",
                                InstallerLang::Pt => "Passo 2 de 5 — Contrato de licença",
                            },
                            InstallStep::Destination => match lang {
                                InstallerLang::Fr => "Étape 3 sur 5 — Dossier de destination",
                                InstallerLang::En => "Step 3 of 5 — Destination Folder",
                                InstallerLang::Es => "Paso 3 de 5 — Carpeta de destino",
                                InstallerLang::De => "Schritt 3 von 5 — Zielordner",
                                InstallerLang::Pt => "Passo 3 de 5 — Pasta de destino",
                            },
                            InstallStep::Options => match lang {
                                InstallerLang::Fr => "Étape 4 sur 5 — Raccourcis et pare-feu Windows",
                                InstallerLang::En => "Step 4 of 5 — Shortcuts & Windows Firewall",
                                InstallerLang::Es => "Paso 4 de 5 — Accesos directos y Cortafuegos",
                                InstallerLang::De => "Schritt 4 von 5 — Verknüpfungen & Firewall",
                                InstallerLang::Pt => "Passo 4 de 5 — Atalhos e Firewall do Windows",
                            },
                            InstallStep::Installing => match lang {
                                InstallerLang::Fr => "Étape 5 sur 5 — Installation en cours...",
                                InstallerLang::En => "Step 5 of 5 — Installing components...",
                                InstallerLang::Es => "Paso 5 de 5 — Instalando...",
                                InstallerLang::De => "Schritt 5 von 5 — Installation läuft...",
                                InstallerLang::Pt => "Passo 5 de 5 — A instalar...",
                            },
                            InstallStep::Finished => match lang {
                                InstallerLang::Fr => "Installation terminée avec succès",
                                InstallerLang::En => "Installation completed successfully",
                                InstallerLang::Es => "Instalación completada con éxito",
                                InstallerLang::De => "Installation erfolgreich abgeschlossen",
                                InstallerLang::Pt => "Instalação concluída com sucesso",
                            },
                        };
                        ui.label(
                            RichText::new(step_subtitle)
                                .color(WIN11_TEXT_SUB)
                                .size(13.0),
                        );
                    });

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        egui::ComboBox::from_id_salt("installer_lang_combo")
                            .selected_text(RichText::new(self.lang.code()).color(WIN11_BLUE).strong())
                            .width(56.0)
                            .show_ui(ui, |ui| {
                                for &l in InstallerLang::all() {
                                    ui.selectable_value(&mut self.lang, l, format!("{} — {}", l.code(), l.label()));
                                }
                            });
                    });
                });
            });

        // Barre inférieure de navigation Windows 11
        egui::TopBottomPanel::bottom("installer_footer")
            .frame(
                egui::Frame::none()
                    .fill(WIN11_CARD)
                    .inner_margin(egui::Margin::symmetric(22.0, 12.0))
                    .stroke(Stroke::new(1.0_f32, WIN11_BORDER)),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    draw_dot(ui, WIN11_GREEN, 4.0);
                    ui.label(
                        RichText::new("CoopLink v1.0.0 — Windows 11")
                            .color(WIN11_TEXT_MUTED)
                            .small(),
                    );

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let next_lbl = match lang {
                            InstallerLang::Fr => "Suivant",
                            InstallerLang::En => "Next",
                            InstallerLang::Es => "Siguiente",
                            InstallerLang::De => "Weiter",
                            InstallerLang::Pt => "Seguinte",
                        };
                        let prev_lbl = match lang {
                            InstallerLang::Fr => "Précédent",
                            InstallerLang::En => "Back",
                            InstallerLang::Es => "Atrás",
                            InstallerLang::De => "Zurück",
                            InstallerLang::Pt => "Anterior",
                        };

                        match self.step {
                            InstallStep::Welcome => {
                                let btn = egui::Button::new(
                                    RichText::new(next_lbl).color(Color32::WHITE).strong(),
                                )
                                .fill(WIN11_BLUE)
                                .rounding(Rounding::same(6.0))
                                .min_size(egui::vec2(105.0, 34.0));

                                if ui.add(btn).clicked() {
                                    self.step = InstallStep::License;
                                }
                            }
                            InstallStep::License => {
                                let next_btn = egui::Button::new(
                                    RichText::new(next_lbl)
                                        .color(if self.license_accepted {
                                            Color32::WHITE
                                        } else {
                                            WIN11_TEXT_MUTED
                                        })
                                        .strong(),
                                )
                                .fill(if self.license_accepted {
                                    WIN11_BLUE
                                } else {
                                    Color32::from_rgb(226, 232, 240)
                                })
                                .rounding(Rounding::same(6.0))
                                .min_size(egui::vec2(105.0, 34.0));

                                if ui.add_enabled(self.license_accepted, next_btn).clicked() {
                                    self.step = InstallStep::Destination;
                                }

                                if ui.add_sized([100.0, 34.0], egui::Button::new(prev_lbl)).clicked() {
                                    self.step = InstallStep::Welcome;
                                }
                            }
                            InstallStep::Destination => {
                                let btn = egui::Button::new(
                                    RichText::new(next_lbl).color(Color32::WHITE).strong(),
                                )
                                .fill(WIN11_BLUE)
                                .rounding(Rounding::same(6.0))
                                .min_size(egui::vec2(105.0, 34.0));

                                if ui.add(btn).clicked() {
                                    self.step = InstallStep::Options;
                                }

                                if ui.add_sized([100.0, 34.0], egui::Button::new(prev_lbl)).clicked() {
                                    self.step = InstallStep::License;
                                }
                            }
                            InstallStep::Options => {
                                let install_lbl = match lang {
                                    InstallerLang::Fr => "Installer",
                                    InstallerLang::En => "Install",
                                    InstallerLang::Es => "Instalar",
                                    InstallerLang::De => "Installieren",
                                    InstallerLang::Pt => "Instalar",
                                };
                                let install_btn = egui::Button::new(
                                    RichText::new(install_lbl).color(Color32::WHITE).strong(),
                                )
                                .fill(WIN11_BLUE)
                                .rounding(Rounding::same(6.0))
                                .min_size(egui::vec2(120.0, 34.0));

                                if ui.add(install_btn).clicked() {
                                    self.step = InstallStep::Installing;
                                    self.start_installation();
                                }

                                if ui.add_sized([100.0, 34.0], egui::Button::new(prev_lbl)).clicked() {
                                    self.step = InstallStep::Destination;
                                }
                            }
                            InstallStep::Installing => {
                                ui.label(
                                    RichText::new("Installation...")
                                        .color(WIN11_BLUE)
                                        .strong(),
                                );
                            }
                            InstallStep::Finished => {
                                let finish_lbl = match lang {
                                    InstallerLang::Fr => "Terminer",
                                    InstallerLang::En => "Finish",
                                    InstallerLang::Es => "Finalizar",
                                    InstallerLang::De => "Fertigstellen",
                                    InstallerLang::Pt => "Concluir",
                                };
                                let finish_btn = egui::Button::new(
                                    RichText::new(finish_lbl).color(Color32::WHITE).strong(),
                                )
                                .fill(WIN11_GREEN)
                                .rounding(Rounding::same(6.0))
                                .min_size(egui::vec2(115.0, 34.0));

                                if ui.add(finish_btn).clicked() {
                                    if self.launch_on_finish {
                                        let exe_path = PathBuf::from(&self.install_dir).join("cooplink.exe");
                                        let _ = launch_installed_cooplink(&exe_path);
                                    }
                                    std::process::exit(0);
                                }
                            }
                        }
                    });
                });
            });

        // Contenu central style Windows 11
        egui::CentralPanel::default()
            .frame(
                egui::Frame::none()
                    .fill(WIN11_BG)
                    .inner_margin(22.0),
            )
            .show(ctx, |ui| {
                egui::Frame::none()
                    .fill(WIN11_CARD)
                    .stroke(Stroke::new(1.0_f32, WIN11_BORDER))
                    .inner_margin(20.0)
                    .rounding(Rounding::same(10.0))
                    .show(ui, |ui| match self.step {
                        InstallStep::Welcome => self.render_welcome(ui),
                        InstallStep::License => self.render_license(ui),
                        InstallStep::Destination => self.render_destination(ui),
                        InstallStep::Options => self.render_options(ui),
                        InstallStep::Installing => self.render_installing(ui),
                        InstallStep::Finished => self.render_finished(ui),
                    });
            });

        if self.step == InstallStep::Installing && self.install_complete.load(Ordering::Relaxed) {
            self.step = InstallStep::Finished;
        }

        ctx.request_repaint_after(Duration::from_millis(100));
    }
}

impl InstallerApp {
    fn render_welcome(&self, ui: &mut Ui) {
        let lang = self.lang;
        ui.vertical(|ui| {
            ui.label(
                RichText::new(match lang {
                    InstallerLang::Fr => "Jouez avec vos amis à distance comme si vous étiez dans le même salon",
                    InstallerLang::En => "Play with friends remotely as if you were in the same living room",
                    InstallerLang::Es => "Juega con tus amigos a distancia como si estuvieran en la misma sala",
                    InstallerLang::De => "Spiele mit Freunden aus der Ferne wie im selben Wohnzimmer",
                    InstallerLang::Pt => "Jogue com amigos à distância como se estivessem na mesma sala",
                })
                .color(WIN11_TEXT_MAIN)
                .strong()
                .size(16.5),
            );
            ui.add_space(6.0);

            ui.label(
                RichText::new(match lang {
                    InstallerLang::Fr => "CoopLink réunit deux technologies complémentaires dans une seule application simple et sans configuration :",
                    InstallerLang::En => "CoopLink combines two complementary gaming technologies in a single zero-configuration application:",
                    InstallerLang::Es => "CoopLink reúne dos tecnologías en una sola aplicación sencilla y sin configuración:",
                    InstallerLang::De => "CoopLink vereint zwei Technologien in einer einzigen, einfachen Anwendung ohne Konfiguration:",
                    InstallerLang::Pt => "O CoopLink reúne duas tecnologias numa única aplicação simples e sem configuração:",
                })
                .color(WIN11_TEXT_SUB),
            );

            ui.add_space(10.0);

            egui::Frame::none()
                .fill(WIN11_SUBTLE)
                .stroke(Stroke::new(1.0_f32, WIN11_BORDER))
                .inner_margin(12.0)
                .rounding(Rounding::same(8.0))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        draw_dot(ui, WIN11_BLUE, 4.5);
                        ui.vertical(|ui| {
                            ui.label(
                                RichText::new(match lang {
                                    InstallerLang::Fr => "1. Réseau Local Virtuel (LAN P2P Direct)",
                                    InstallerLang::En => "1. Virtual Local Network (Direct P2P LAN)",
                                    InstallerLang::Es => "1. Red Local Virtual (LAN P2P Directo)",
                                    InstallerLang::De => "1. Virtuelles Lokales Netzwerk (Direktes P2P-LAN)",
                                    InstallerLang::Pt => "1. Rede Local Virtual (LAN P2P Direto)",
                                })
                                .color(WIN11_BLUE)
                                .strong(),
                            );
                            ui.label(
                                RichText::new(match lang {
                                    InstallerLang::Fr => "Reliez vos PC en connexion directe (100% P2P) pour jouer à tous vos jeux multijoueurs LAN sans ouvrir manuellement les ports de votre box.",
                                    InstallerLang::En => "Connect your PCs directly (100% P2P) to play any LAN multiplayer game without manual router port forwarding.",
                                    InstallerLang::Es => "Conecta tus PCs directamente (100% P2P) para jugar a cualquier juego LAN sin abrir puertos manualmente.",
                                    InstallerLang::De => "Verbinde eure PCs direkt (100% P2P) für alle LAN-Multiplayer-Spiele ohne manuelle Portfreigabe.",
                                    InstallerLang::Pt => "Ligue os vossos PCs diretamente (100% P2P) para jogar qualquer jogo LAN sem abrir portas manualmente.",
                                })
                                .color(WIN11_TEXT_SUB)
                                .size(12.5),
                            );
                        });
                    });

                    ui.add_space(8.0);

                    ui.horizontal(|ui| {
                        draw_dot(ui, WIN11_GREEN, 4.5);
                        ui.vertical(|ui| {
                            ui.label(
                                RichText::new(match lang {
                                    InstallerLang::Fr => "2. Écran Partagé & Manettes Co-op (1 seul jeu requis)",
                                    InstallerLang::En => "2. Shared Screen & Co-op Controllers (Only 1 game copy needed)",
                                    InstallerLang::Es => "2. Pantalla Compartida y Mandos Co-op (Solo 1 copia del juego)",
                                    InstallerLang::De => "2. Geteilter Bildschirm & Co-op Controller (Nur 1 Spiel nötig)",
                                    InstallerLang::Pt => "2. Ecrã Partilhado e Comandos Co-op (Apenas 1 jogo necessário)",
                                })
                                .color(WIN11_GREEN)
                                .strong(),
                            );
                            ui.label(
                                RichText::new(match lang {
                                    InstallerLang::Fr => "Partagez votre jeu en temps réel : vos amis rejoignent votre écran avec leurs manettes (Joueur 2, 3, 4) ou leur clavier/souris converti en manette virtuelle.",
                                    InstallerLang::En => "Share your game in real time: friends join your screen with their gamepads (Player 2, 3, 4) or their keyboard/mouse converted into a virtual gamepad.",
                                    InstallerLang::Es => "Comparte tu juego en tiempo real: tus amigos se unen con sus mandos (Jugador 2, 3, 4) o su teclado/ratón convertido en mando.",
                                    InstallerLang::De => "Teile dein Spiel in Echtzeit: Freunde treten mit ihren Controllern (Spieler 2, 3, 4) oder per Tastatur/Maus bei.",
                                    InstallerLang::Pt => "Partilhe o seu jogo em tempo real: os amigos juntam-se com os seus comandos (Jogador 2, 3, 4) ou teclado/rato.",
                                })
                                .color(WIN11_TEXT_SUB)
                                .size(12.5),
                            );
                        });
                    });
                });

            ui.add_space(10.0);
            ui.label(
                RichText::new(match lang {
                    InstallerLang::Fr => "Cliquez sur « Suivant » pour démarrer l'installation.",
                    InstallerLang::En => "Click 'Next' to continue setup.",
                    InstallerLang::Es => "Haz clic en « Siguiente » para continuar.",
                    InstallerLang::De => "Klicke auf „Weiter“, um fortzufahren.",
                    InstallerLang::Pt => "Clique em « Seguinte » para continuar.",
                })
                .color(WIN11_TEXT_MUTED)
                .small(),
            );
        });
    }

    fn render_license(&mut self, ui: &mut Ui) {
        ui.vertical(|ui| {
            ui.label(
                RichText::new("Contrat de Licence Logicielle")
                    .color(WIN11_TEXT_MAIN)
                    .strong()
                    .size(16.5),
            );
            ui.label(
                RichText::new("Veuillez prendre connaissance des termes de la licence avant de continuer.")
                    .color(WIN11_TEXT_SUB)
                    .small(),
            );
            ui.add_space(8.0);

            ScrollArea::vertical()
                .max_height(175.0)
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    egui::Frame::none()
                        .fill(WIN11_SUBTLE)
                        .stroke(Stroke::new(1.0_f32, WIN11_BORDER))
                        .inner_margin(12.0)
                        .rounding(Rounding::same(6.0))
                        .show(ui, |ui| {
                            ui.label(
                                RichText::new(
                                    "COOPLINK — LICENCE LOGICIELLE PROPRIÉTAIRE\n\
                                     Copyright (c) 2026 Loic HUON. Tous droits réservés.\n\n\
                                     1. PROPRIÉTÉ INTELLECTUELLE\n\
                                     Ce logiciel, son architecture P2P, son moteur de partage coopératif et son interface\n\
                                     sont la propriété exclusive de Loic HUON.\n\n\
                                     2. CONDITIONS D'UTILISATION\n\
                                     Toute reproduction, revente, modification ou ingénierie inverse non autorisée est\n\
                                     strictement interdite.\n\n\
                                     3. CONFIDENTIALITÉ & SÉCURITÉ P2P\n\
                                     Les sessions de jeu et de partage d'écran s'effectuent directement de pair à pair\n\
                                     entre les participants.",
                                )
                                .color(WIN11_TEXT_MAIN)
                                .small(),
                            );
                        });
                });

            ui.add_space(10.0);
            ui.checkbox(
                &mut self.license_accepted,
                RichText::new("J'accepte les termes du contrat de licence")
                    .color(WIN11_TEXT_MAIN)
                    .strong(),
            );
        });
    }

    fn render_destination(&mut self, ui: &mut Ui) {
        ui.vertical(|ui| {
            ui.label(
                RichText::new("Emplacement d'installation")
                    .color(WIN11_TEXT_MAIN)
                    .strong()
                    .size(16.5),
            );
            ui.label(
                RichText::new("Sélectionnez le dossier dans lequel installer CoopLink :")
                    .color(WIN11_TEXT_SUB),
            );
            ui.add_space(14.0);

            ui.horizontal(|ui| {
                ui.label(RichText::new("Dossier :").color(WIN11_TEXT_MAIN).strong());
                ui.add(egui::TextEdit::singleline(&mut self.install_dir).desired_width(430.0));
            });

            ui.add_space(16.0);
            egui::Frame::none()
                .fill(WIN11_SUBTLE)
                .stroke(Stroke::new(1.0_f32, WIN11_BORDER))
                .inner_margin(12.0)
                .rounding(Rounding::same(8.0))
                .show(ui, |ui| {
                    ui.label("Espace disque nécessaire : ~15 Mo");
                    ui.label("Intégration Windows 11 : Ajout/Suppression de programmes inclus");
                });
        });
    }

    fn render_options(&mut self, ui: &mut Ui) {
        ui.vertical(|ui| {
            ui.label(
                RichText::new("Options d'installation")
                    .color(WIN11_TEXT_MAIN)
                    .strong()
                    .size(16.5),
            );
            ui.label(
                RichText::new("Choisissez les options de configuration automatique :")
                    .color(WIN11_TEXT_SUB),
            );
            ui.add_space(14.0);

            ui.checkbox(
                &mut self.create_desktop_shortcut,
                RichText::new("Créer un raccourci sur le Bureau").color(WIN11_TEXT_MAIN),
            );
            ui.add_space(6.0);

            ui.checkbox(
                &mut self.create_start_menu_shortcut,
                RichText::new("Ajouter un raccourci dans le Menu Démarrer Windows").color(WIN11_TEXT_MAIN),
            );
            ui.add_space(6.0);

            ui.checkbox(
                &mut self.configure_firewall,
                RichText::new("Configurer automatiquement le Pare-feu Windows Defender (recommandé)")
                    .color(WIN11_TEXT_MAIN)
                    .strong(),
            );

            ui.add_space(14.0);
            ui.label(
                RichText::new("Cliquez sur « Installer » pour démarrer l'installation.")
                    .color(WIN11_TEXT_MUTED)
                    .small(),
            );
        });
    }

    fn render_installing(&self, ui: &mut Ui) {
        ui.vertical_centered(|ui| {
            ui.add_space(30.0);
            ui.label(
                RichText::new("Installation de CoopLink en cours...")
                    .color(WIN11_TEXT_MAIN)
                    .strong()
                    .size(17.0),
            );
            ui.add_space(18.0);

            let progress_bar = egui::ProgressBar::new(self.progress)
                .show_percentage()
                .animate(true)
                .fill(WIN11_BLUE);

            ui.add_sized([ui.available_width() - 40.0, 22.0], progress_bar);

            ui.add_space(14.0);
            ui.label(RichText::new(&self.status_label).color(WIN11_TEXT_SUB));

            if let Some(ref err) = self.error_msg {
                ui.add_space(10.0);
                ui.label(RichText::new(format!("Erreur : {}", err)).color(Color32::RED));
            }
        });
    }

    fn render_finished(&mut self, ui: &mut Ui) {
        ui.vertical(|ui| {
            ui.add_space(10.0);
            ui.horizontal(|ui| {
                draw_dot(ui, WIN11_GREEN, 5.5);
                ui.label(
                    RichText::new("CoopLink a été installé avec succès")
                        .color(WIN11_GREEN)
                        .strong()
                        .size(18.0),
                );
            });
            ui.add_space(10.0);

            ui.label(
                RichText::new(
                    "CoopLink est maintenant prêt. Vous pouvez créer un salon P2P avec vos amis, jouer en réseau local \
                     ou partager votre écran et vos manettes en un clic.",
                )
                .color(WIN11_TEXT_SUB),
            );

            ui.add_space(18.0);
            ui.checkbox(
                &mut self.launch_on_finish,
                RichText::new("Lancer CoopLink maintenant")
                    .color(WIN11_TEXT_MAIN)
                    .strong(),
            );

            ui.add_space(12.0);
            ui.label(
                RichText::new("Cliquez sur « Terminer » pour quitter l'assistant.")
                    .color(WIN11_TEXT_MUTED)
                    .small(),
            );
        });
    }

    fn start_installation(&mut self) {
        if self.install_started.swap(true, Ordering::SeqCst) {
            return;
        }

        let target_dir = PathBuf::from(&self.install_dir);
        let create_desktop = self.create_desktop_shortcut;
        let create_start = self.create_start_menu_shortcut;
        let setup_firewall = self.configure_firewall;
        let complete_flag = self.install_complete.clone();

        thread::spawn(move || {
            let _ = fs::create_dir_all(&target_dir);
            thread::sleep(Duration::from_millis(250));

            let current_exe = env::current_exe().unwrap_or_default();
            let source_cooplink = current_exe
                .parent()
                .map(|p| p.join("cooplink.exe"))
                .unwrap_or_else(|| PathBuf::from("cooplink.exe"));

            let target_exe = target_dir.join("cooplink.exe");

            if source_cooplink.exists() {
                let _ = fs::copy(&source_cooplink, &target_exe);
            } else if current_exe.exists() {
                let _ = fs::copy(&current_exe, &target_exe);
            }
            thread::sleep(Duration::from_millis(250));

            if create_desktop {
                if let Ok(user_profile) = env::var("USERPROFILE") {
                    let desktop_lnk = PathBuf::from(user_profile)
                        .join("Desktop")
                        .join("CoopLink.lnk");
                    let _ = create_windows_shortcut(
                        &target_exe,
                        &desktop_lnk,
                        "CoopLink - Réseau P2P & Partage de Jeu Co-op",
                    );
                }
            }
            thread::sleep(Duration::from_millis(150));

            if create_start {
                if let Ok(app_data) = env::var("APPDATA") {
                    let start_menu = PathBuf::from(app_data)
                        .join("Microsoft")
                        .join("Windows")
                        .join("Start Menu")
                        .join("Programs")
                        .join("CoopLink.lnk");
                    let _ = create_windows_shortcut(
                        &target_exe,
                        &start_menu,
                        "CoopLink - Réseau P2P & Partage de Jeu Co-op",
                    );
                }
            }
            thread::sleep(Duration::from_millis(150));

            if setup_firewall {
                let _ = setup_silent_firewall(&target_exe);
            }
            thread::sleep(Duration::from_millis(150));

            let _ = register_in_windows_uninstall(&target_dir, &target_exe);
            thread::sleep(Duration::from_millis(150));

            complete_flag.store(true, Ordering::SeqCst);
        });

        self.progress = 1.0;
        self.status_label = "Finalisation de l'installation...".to_string();
    }
}

fn setup_installer_theme(ctx: &egui::Context) {
    let mut style = (*ctx.style()).clone();
    let mut visuals = egui::Visuals::light();
    visuals.panel_fill = WIN11_BG;
    visuals.window_fill = WIN11_CARD;
    visuals.faint_bg_color = WIN11_SUBTLE;
    visuals.extreme_bg_color = WIN11_SUBTLE;
    visuals.override_text_color = Some(WIN11_TEXT_MAIN);

    let rounding = Rounding::same(6.0);
    visuals.widgets.noninteractive.bg_fill = WIN11_CARD;
    visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0_f32, WIN11_BORDER);
    visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0_f32, WIN11_TEXT_MAIN);
    visuals.widgets.noninteractive.rounding = rounding;

    visuals.widgets.inactive.bg_fill = WIN11_SUBTLE;
    visuals.widgets.inactive.bg_stroke = Stroke::new(1.0_f32, Color32::from_rgb(203, 213, 225));
    visuals.widgets.inactive.fg_stroke = Stroke::new(1.0_f32, WIN11_TEXT_MAIN);
    visuals.widgets.inactive.rounding = rounding;

    visuals.widgets.hovered.bg_fill = Color32::from_rgb(238, 244, 252);
    visuals.widgets.hovered.bg_stroke = Stroke::new(1.2_f32, WIN11_BLUE);
    visuals.widgets.hovered.fg_stroke = Stroke::new(1.0_f32, WIN11_BLUE);
    visuals.widgets.hovered.rounding = rounding;

    style.text_styles.insert(
        TextStyle::Body,
        FontId::new(14.0, FontFamily::Proportional),
    );
    style.text_styles.insert(
        TextStyle::Button,
        FontId::new(14.0, FontFamily::Proportional),
    );
    style.visuals = visuals;
    ctx.set_style(style);
}

fn load_installer_logo(ctx: &egui::Context) -> Option<egui::TextureHandle> {
    let image_bytes = include_bytes!("../../../images/CoopLink.png");
    if let Ok(dynamic_img) = image::load_from_memory(image_bytes) {
        let mut rgba = dynamic_img.to_rgba8();
        for pixel in rgba.pixels_mut() {
            let [r, g, b, _] = pixel.0;
            if r >= 246 && g >= 246 && b >= 246 {
                pixel.0 = [255, 255, 255, 0];
            }
        }
        let size = [rgba.width() as usize, rgba.height() as usize];
        let color_image = egui::ColorImage::from_rgba_unmultiplied(size, &rgba.into_raw());
        Some(ctx.load_texture("installer_logo", color_image, egui::TextureOptions::LINEAR))
    } else {
        None
    }
}
