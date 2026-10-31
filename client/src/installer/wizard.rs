use super::actions::*;
use eframe::egui;
use egui::{Color32, CursorIcon, FontFamily, FontId, RichText, Rounding, ScrollArea, Stroke, TextStyle, Ui};
use std::env;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
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
const WIN11_ORANGE: Color32 = Color32::from_rgb(255, 122, 40);      // #FF7A28
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
    pub fn from_os() -> Self {
        let loc = detect_os_locale();
        if loc.starts_with("fr") {
            InstallerLang::Fr
        } else if loc.starts_with("es") {
            InstallerLang::Es
        } else if loc.starts_with("de") {
            InstallerLang::De
        } else if loc.starts_with("pt") {
            InstallerLang::Pt
        } else {
            InstallerLang::En
        }
    }

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
            InstallerLang::En => "US/EN",
            InstallerLang::Es => "ES",
            InstallerLang::De => "DE",
            InstallerLang::Pt => "PT",
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            InstallerLang::Fr => "Français (FR)",
            InstallerLang::En => "English (US)",
            InstallerLang::Es => "Español (ES)",
            InstallerLang::De => "Deutsch (DE)",
            InstallerLang::Pt => "Português (PT)",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstallStep {
    Welcome,
    License,
    Destination,
    OptionsAndDrivers,
    Installing,
    Finished,
    Uninstalling,
    UninstallFinished,
}

pub struct InstallerApp {
    lang: InstallerLang,
    step: InstallStep,
    install_dir: String,
    license_accepted: bool,
    create_desktop_shortcut: bool,
    create_start_menu_shortcut: bool,
    configure_firewall: bool,
    install_gamepad_driver: bool,
    install_virtual_display_driver: bool,
    hw_check: HardwareDriverCheck,
    launch_on_finish: bool,
    progress: f32,
    progress_pct: Arc<AtomicU32>,
    status_label: String,
    status_shared: Arc<parking_lot::Mutex<String>>,
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
        let hw_check = check_system_drivers();

        let is_uninstall_mode = env::args().any(|a| a.to_lowercase().contains("uninstall"))
            || env::current_exe()
                .ok()
                .and_then(|p| p.file_name().map(|f| f.to_string_lossy().to_lowercase()))
                .map(|name| name.contains("unins"))
                .unwrap_or(false);

        let mut app = Self {
            lang: InstallerLang::from_os(),
            step: if is_uninstall_mode {
                InstallStep::Uninstalling
            } else {
                InstallStep::Welcome
            },
            install_dir: default_dir.to_string_lossy().to_string(),
            license_accepted: false,
            create_desktop_shortcut: true,
            create_start_menu_shortcut: true,
            configure_firewall: true,
            install_gamepad_driver: true,
            install_virtual_display_driver: true,
            hw_check,
            launch_on_finish: true,
            progress: 0.0,
            progress_pct: Arc::new(AtomicU32::new(0)),
            status_label: "Préparation de l'installation...".to_string(),
            status_shared: Arc::new(parking_lot::Mutex::new(
                "Préparation de l'installation...".to_string(),
            )),
            install_started: Arc::new(AtomicBool::new(false)),
            install_complete: Arc::new(AtomicBool::new(false)),
            error_msg: None,
            logo_texture,
        };

        if is_uninstall_mode {
            app.start_uninstallation();
        }

        app
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
        self.progress = (self.progress_pct.load(Ordering::Relaxed) as f32 / 100.0).clamp(0.0, 1.0);
        self.status_label = self.status_shared.lock().clone();

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
                                InstallerLang::En => "CoopLink Setup Wizard",
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
                                InstallerLang::Fr => "Étape 2 sur 5 — Contrat de Licence & Limitation de Responsabilité",
                                InstallerLang::En => "Step 2 of 5 — End-User License Agreement & Liability Disclaimer",
                                InstallerLang::Es => "Paso 2 de 5 — Contrato de Licencia y Exención de Responsabilidad",
                                InstallerLang::De => "Schritt 2 von 5 — Lizenzvertrag & Haftungsausschluss",
                                InstallerLang::Pt => "Passo 2 de 5 — Contrato de Licença e Exclusão de Responsabilidade",
                            },
                            InstallStep::Destination => match lang {
                                InstallerLang::Fr => "Étape 3 sur 5 — Dossier de destination",
                                InstallerLang::En => "Step 3 of 5 — Destination Folder",
                                InstallerLang::Es => "Paso 3 de 5 — Carpeta de destino",
                                InstallerLang::De => "Schritt 3 von 5 — Zielordner",
                                InstallerLang::Pt => "Passo 3 de 5 — Pasta de destino",
                            },
                            InstallStep::OptionsAndDrivers => match lang {
                                InstallerLang::Fr => "Étape 4 sur 5 — Vérification GPU, Pilotes Virtuels & Pare-feu",
                                InstallerLang::En => "Step 4 of 5 — GPU Check, Virtual Drivers & Windows Firewall",
                                InstallerLang::Es => "Paso 4 de 5 — Verificación GPU, Controladores y Cortafuegos",
                                InstallerLang::De => "Schritt 4 von 5 — GPU-Prüfung, Virtuelle Treiber & Firewall",
                                InstallerLang::Pt => "Passo 4 de 5 — Verificação GPU, Controladores e Firewall",
                            },
                            InstallStep::Installing => match lang {
                                InstallerLang::Fr => "Étape 5 sur 5 — Installation du logiciel et des pilotes...",
                                InstallerLang::En => "Step 5 of 5 — Installing software and drivers...",
                                InstallerLang::Es => "Paso 5 de 5 — Instalando software y controladores...",
                                InstallerLang::De => "Schritt 5 von 5 — Software und Treiber werden installiert...",
                                InstallerLang::Pt => "Passo 5 de 5 — A instalar software e controladores...",
                            },
                            InstallStep::Finished => match lang {
                                InstallerLang::Fr => "Installation terminée avec succès",
                                InstallerLang::En => "Installation completed successfully",
                                InstallerLang::Es => "Instalación completada con éxito",
                                InstallerLang::De => "Installation erfolgreich abgeschlossen",
                                InstallerLang::Pt => "Instalação concluída com sucesso",
                            },
                            InstallStep::Uninstalling => match lang {
                                InstallerLang::Fr => "Désinstallation complète de CoopLink en cours...",
                                InstallerLang::En => "Uninstalling CoopLink...",
                                _ => "Désinstallation en cours...",
                            },
                            InstallStep::UninstallFinished => match lang {
                                InstallerLang::Fr => "Désinstallation terminée avec succès",
                                InstallerLang::En => "Uninstallation completed",
                                _ => "Désinstallation terminée",
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
                            .width(66.0)
                            .show_ui(ui, |ui| {
                                for &l in InstallerLang::all() {
                                    ui.selectable_value(&mut self.lang, l, l.label());
                                }
                            });
                    });
                });
            });

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

                                if ui.add(btn).on_hover_cursor(CursorIcon::PointingHand).clicked() {
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

                                if ui.add(btn).on_hover_cursor(CursorIcon::PointingHand).clicked() {
                                    self.step = InstallStep::OptionsAndDrivers;
                                }

                                if ui.add_sized([100.0, 34.0], egui::Button::new(prev_lbl)).clicked() {
                                    self.step = InstallStep::License;
                                }
                            }
                            InstallStep::OptionsAndDrivers => {
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

                                if ui.add(install_btn).on_hover_cursor(CursorIcon::PointingHand).clicked() {
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

                                if ui.add(finish_btn).on_hover_cursor(CursorIcon::PointingHand).clicked() {
                                    if self.launch_on_finish {
                                        let exe_path = PathBuf::from(&self.install_dir).join("cooplink.exe");
                                        if exe_path.exists() {
                                            let _ = launch_installed_cooplink(&exe_path);
                                        } else {
                                            let fallback_exe = get_user_local_install_dir().join("cooplink.exe");
                                            let _ = launch_installed_cooplink(&fallback_exe);
                                        }
                                    }
                                    std::process::exit(0);
                                }
                            }
                            InstallStep::Uninstalling => {
                                ui.label(
                                    RichText::new("Désinstallation...")
                                        .color(WIN11_ORANGE)
                                        .strong(),
                                );
                            }
                            InstallStep::UninstallFinished => {
                                let close_btn = egui::Button::new(
                                    RichText::new("Fermer").color(Color32::WHITE).strong(),
                                )
                                .fill(WIN11_BLUE)
                                .rounding(Rounding::same(6.0))
                                .min_size(egui::vec2(110.0, 34.0));
                                if ui.add(close_btn).on_hover_cursor(CursorIcon::PointingHand).clicked() {
                                    std::process::exit(0);
                                }
                            }
                        }
                    });
                });
            });

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
                        InstallStep::OptionsAndDrivers => self.render_options(ui),
                        InstallStep::Installing | InstallStep::Uninstalling => self.render_installing(ui),
                        InstallStep::Finished => self.render_finished(ui),
                        InstallStep::UninstallFinished => self.render_uninstall_finished(ui),
                    });
            });

        if self.step == InstallStep::Installing && self.install_complete.load(Ordering::Relaxed) {
            self.step = InstallStep::Finished;
        }
        if self.step == InstallStep::Uninstalling && self.install_complete.load(Ordering::Relaxed) {
            self.step = InstallStep::UninstallFinished;
        }

        ctx.request_repaint_after(Duration::from_millis(80));
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
                    InstallerLang::Fr => "CoopLink réunit deux technologies complémentaires dans une seule application sécurisée et sans configuration :",
                    InstallerLang::En => "CoopLink combines two complementary gaming technologies in a single secure, zero-configuration application:",
                    InstallerLang::Es => "CoopLink reúne dos tecnologías en una sola aplicación segura y sin configuración:",
                    InstallerLang::De => "CoopLink vereint zwei Technologien in einer einzigen, sicheren Anwendung ohne Konfiguration:",
                    InstallerLang::Pt => "O CoopLink reúne duas tecnologias numa única aplicação segura e sem configuração:",
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
                                    InstallerLang::Fr => "1. Réseau Local Virtuel Chiffré (LAN P2P Direct)",
                                    InstallerLang::En => "1. Encrypted Virtual Local Network (Direct P2P LAN)",
                                    InstallerLang::Es => "1. Red Local Virtual Cifrada (LAN P2P Directo)",
                                    InstallerLang::De => "1. Verschlüsseltes Virtuelles LAN (Direktes P2P)",
                                    InstallerLang::Pt => "1. Rede Local Virtual Encriptada (LAN P2P Direto)",
                                })
                                .color(WIN11_BLUE)
                                .strong(),
                            );
                            ui.label(
                                RichText::new(match lang {
                                    InstallerLang::Fr => "Reliez vos PC en connexion directe chiffrée (100% P2P) pour jouer à tous vos jeux multijoueurs LAN sans ouvrir manuellement les ports de votre box.",
                                    InstallerLang::En => "Connect your PCs over an encrypted direct P2P tunnel to play any LAN multiplayer game without manual router port forwarding.",
                                    InstallerLang::Es => "Conecta tus PCs directamente en P2P cifrado para jugar a cualquier juego LAN sin abrir puertos manualmente.",
                                    InstallerLang::De => "Verbinde eure PCs direkt per verschlüsseltem P2P für alle LAN-Multiplayer-Spiele.",
                                    InstallerLang::Pt => "Ligue os vossos PCs diretamente em P2P encriptado para jogar qualquer jogo LAN.",
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
                                    InstallerLang::Fr => "2. Partage de Jeu au Premier Plan & Manettes Co-op (Anti-Écho Audio)",
                                    InstallerLang::En => "2. Foreground Game Share & Co-op Controllers (Audio Anti-Echo)",
                                    InstallerLang::Es => "2. Juego Compartido y Mandos Co-op (Anti-Eco de Audio)",
                                    InstallerLang::De => "2. Vordergrund-Spielfreigabe & Co-op Controller (Audio-Echo-Schutz)",
                                    InstallerLang::Pt => "2. Partilha de Jogo e Comandos Co-op (Anti-Eco de Áudio)",
                                })
                                .color(WIN11_GREEN)
                                .strong(),
                            );
                            ui.label(
                                RichText::new(match lang {
                                    InstallerLang::Fr => "Partagez uniquement votre jeu actif au premier plan sans écho vocal : vos amis rejoignent votre partie avec leurs manettes (Joueur 2, 3, 4) ou leur clavier/souris.",
                                    InstallerLang::En => "Stream only your active foreground game with zero voice echo: friends join with their gamepads (Player 2, 3, 4) or keyboard/mouse.",
                                    InstallerLang::Es => "Transmite solo tu juego activo en primer plano sin eco de voz: tus amigos se unen con sus mandos o teclado/ratón.",
                                    InstallerLang::De => "Übertrage nur dein aktives Spiel im Vordergrund ohne Sprach-Echo mit bis zu 4 Spielern.",
                                    InstallerLang::Pt => "Transmita apenas o seu jogo em primeiro plano sem eco de voz com até 4 jogadores.",
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
                    InstallerLang::Fr => "Cliquez sur « Suivant » pour lire le contrat de licence et vérifier vos pilotes.",
                    InstallerLang::En => "Click 'Next' to review the license agreement and verify system drivers.",
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
        let lang = self.lang;
        ui.vertical(|ui| {
            ui.label(
                RichText::new(match lang {
                    InstallerLang::Fr => "Contrat de Licence Utilisateur Final (CLUF) & Clause de Non-Responsabilité",
                    InstallerLang::En => "End-User License Agreement (EULA) & Limitation of Liability",
                    InstallerLang::Es => "Contrato de Licencia de Usuario Final y Exención de Responsabilidad",
                    InstallerLang::De => "Endbenutzer-Lizenzvertrag (EULA) & Haftungsausschluss",
                    InstallerLang::Pt => "Contrato de Licença de Utilizador Final e Exclusão de Responsabilidade",
                })
                .color(WIN11_TEXT_MAIN)
                .strong()
                .size(15.5),
            );
            ui.label(
                RichText::new(match lang {
                    InstallerLang::Fr => "Veuillez lire attentivement les clauses suivantes avant d'installer CoopLink :",
                    InstallerLang::En => "Please read the following legal terms carefully before installing CoopLink:",
                    InstallerLang::Es => "Por favor, lee atentamente los siguientes términos legales antes de instalar:",
                    InstallerLang::De => "Bitte lies die folgenden rechtlichen Bedingungen sorgfältig durch:",
                    InstallerLang::Pt => "Por favor, leia atentamente os seguintes termos legais antes de instalar:",
                })
                .color(WIN11_TEXT_SUB)
                .small(),
            );
            ui.add_space(8.0);

            let eula_text = match lang {
                InstallerLang::Fr => {
                    "CONTRAT DE LICENCE LOGICIELLE ET D'EXONÉRATION DE RESPONSABILITÉ — COOPLINK\n\
                     Copyright © 2026 Loic HUON. Tous droits réservés.\n\n\
                     IMPORTANT — VEUILLEZ LIRE ATTENTIVEMENT : En installant, copiant ou utilisant le logiciel « CoopLink » (ci-après « le Logiciel »), vous acceptez sans réserve l'intégralité des dispositions du présent contrat.\n\n\
                     ARTICLE 1 — PROPRIÉTÉ INTELLECTUELLE ET LICENCE D'UTILISATION\n\
                     Le Logiciel, son code source, ses algorithmes de tunnelisation P2P, ses pilotes virtuels et son interface graphique sont la propriété exclusive de Loic HUON (ci-après « l'Éditeur »). L'Éditeur vous concède une licence personnelle, non exclusive, non transférable et révocable d'utilisation du Logiciel. Toute ingénierie inverse, décompilation, modification ou revente commerciale non autorisée est strictement interdite.\n\n\
                     ARTICLE 2 — FOURNITURE « EN L'ÉTAT » ET ABSENCE TOTALE DE GARANTIE\n\
                     Le Logiciel est fourni « TEL QUEL » (« AS IS »), sans aucune garantie expresse ou implicite de quelque nature que ce soit, incluant notamment toute garantie de bon fonctionnement ininterrompu, d'absence d'erreurs, de compatibilité matérielle ou logicielle, ou d'adéquation à un usage particulier. L'ensemble des risques liés à l'installation, aux performances et à l'utilisation du Logiciel repose exclusivement sur l'Utilisateur.\n\n\
                     ARTICLE 3 — CLAUSE EXONÉRATOIRE ET LIMITATIVE DE RESPONSABILITÉ\n\
                     DANS TOUTE LA MESURE PERMISE PAR LA LOI APPLICABLE, L'ÉDITEUR (LOIC HUON) DÉCLINE TOUTE RESPONSABILITÉ ET NE POURRA EN AUCUN CAS ÊTRE TENU RESPONSABLE DE QUELQUE DOMMAGE DIRECT, INDIRECT, ACCESSOIRE, SPÉCIAL, MATÉRIEL, LOGICIEL OU FINANCIER RÉSULTANT DE L'INSTALLATION, DE LA CONFIGURATION OU DE L'UTILISATION DU LOGICIEL, Y COMPRIS, SANS S'Y LIMITER :\n\
                     a) Toute perte, corruption ou altération de données, de fichiers système ou de paramètres réseau (pare-feu, routage, UPnP, pilotes d'affichage ou de périphériques virtuels) ;\n\
                     b) Tout dysfonctionnement matériel (carte graphique, processeur, périphériques, écrans) ou instabilité du système d'exploitation ;\n\
                     c) Toute suspension, restriction ou bannissement de compte prononcé par un éditeur de jeu tiers ou une plateforme en ligne en raison de l'utilisation d'un réseau virtuel ou d'un périphérique émulé ;\n\
                     d) Toute intrusion, attaque réseau, fuite d'adresse IP ou acte malveillant commis par un tiers ou un autre participant connecté à un salon P2P ;\n\
                     e) Toute utilisation illicite, contrefaisante ou non conforme aux lois en vigueur effectuée par l'Utilisateur via le tunnel pair-à-pair.\n\n\
                     ARTICLE 4 — RESPONSABILITÉ EXCLUSIVE DE L'UTILISATEUR\n\
                     L'Utilisateur reconnaît être seul et unique responsable des personnes qu'il invite dans ses salons P2P, des applications et jeux qu'il partage, ainsi que du respect des droits de propriété intellectuelle des éditeurs de jeux tiers."
                }
                _ => {
                    "END-USER LICENSE AGREEMENT (EULA) & LIMITATION OF LIABILITY — COOPLINK\n\
                     Copyright © 2026 Loic HUON. All Rights Reserved.\n\n\
                     IMPORTANT — PLEASE READ CAREFULLY: By installing, copying, or using the 'CoopLink' software (the 'Software'), you unconditionally agree to all terms of this Agreement.\n\n\
                     SECTION 1 — INTELLECTUAL PROPERTY & LICENSE GRANT\n\
                     The Software, its source code, P2P tunneling architecture, virtual drivers, and user interface are the exclusive property of Loic HUON (the 'Publisher'). Reverse engineering, decompilation, modification, or unauthorized commercial redistribution is strictly prohibited.\n\n\
                     SECTION 2 — 'AS IS' PROVISION & NO WARRANTY\n\
                     THE SOFTWARE IS PROVIDED 'AS IS' AND 'WITH ALL FAULTS', WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO WARRANTIES OF MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE, OR NON-INFRINGEMENT. THE ENTIRE RISK AS TO THE QUALITY, NETWORK SECURITY, AND PERFORMANCE OF THE SOFTWARE IS BORNE SOLELY BY THE USER.\n\n\
                     SECTION 3 — COMPLETE DISCLAIMER & LIMITATION OF LIABILITY\n\
                     TO THE MAXIMUM EXTENT PERMITTED BY APPLICABLE LAW, IN NO EVENT SHALL THE PUBLISHER (LOIC HUON) BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES WHATSOEVER ARISING OUT OF OR IN ANY WAY RELATED TO THE INSTALLATION OR USE OF THE SOFTWARE, INCLUDING WITHOUT LIMITATION:\n\
                     a) Any loss or corruption of data, system files, or network configurations (firewall, UPnP, routing, virtual display or controller drivers);\n\
                     b) Any hardware malfunction, GPU/CPU damage, or operating system instability;\n\
                     c) Any account suspension, restriction, or ban issued by third-party game publishers or online services;\n\
                     d) Any network intrusion, IP exposure, or malicious actions performed by third-party peers invited into a P2P room;\n\
                     e) Any unlawful or unauthorized use of the Software by the User.\n\n\
                     SECTION 4 — USER RESPONSIBILITY\n\
                     The User acknowledges full and sole responsibility for the peers they connect with, the applications they share, and compliance with all applicable laws and third-party terms of service."
                }
            };

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
                            ui.label(RichText::new(eula_text).color(WIN11_TEXT_MAIN).small());
                        });
                });

            ui.add_space(10.0);
            ui.checkbox(
                &mut self.license_accepted,
                RichText::new(match lang {
                    InstallerLang::Fr => "J'ai lu et j'accepte sans réserve les termes du contrat de licence et la clause d'exonération de responsabilité",
                    InstallerLang::En => "I have read and unconditionally accept the License Agreement and the Limitation of Liability",
                    InstallerLang::Es => "He leído y acepto sin reservas el contrato de licencia y la exención de responsabilidad",
                    InstallerLang::De => "Ich habe den Lizenzvertrag und den Haftungsausschluss gelesen und akzeptiere sie",
                    InstallerLang::Pt => "Li e aceito sem reservas o contrato de licença e a cláusula de exclusão de responsabilidade",
                })
                .color(WIN11_TEXT_MAIN)
                .strong(),
            );
        });
    }

    fn render_destination(&mut self, ui: &mut Ui) {
        let lang = self.lang;
        ui.vertical(|ui| {
            ui.label(
                RichText::new(match lang {
                    InstallerLang::Fr => "Emplacement d'installation",
                    InstallerLang::En => "Installation Folder",
                    InstallerLang::Es => "Carpeta de instalación",
                    InstallerLang::De => "Installationsort",
                    InstallerLang::Pt => "Local de instalação",
                })
                .color(WIN11_TEXT_MAIN)
                .strong()
                .size(16.5),
            );
            ui.label(
                RichText::new(match lang {
                    InstallerLang::Fr => "Sélectionnez le dossier dans lequel installer CoopLink :",
                    InstallerLang::En => "Select the destination folder where CoopLink will be installed:",
                    InstallerLang::Es => "Selecciona la carpeta donde instalar CoopLink:",
                    InstallerLang::De => "Wähle den Ordner für die Installation von CoopLink:",
                    InstallerLang::Pt => "Selecione a pasta onde instalar o CoopLink:",
                })
                .color(WIN11_TEXT_SUB),
            );
            ui.add_space(14.0);

            ui.horizontal(|ui| {
                ui.label(RichText::new("Dossier :").color(WIN11_TEXT_MAIN).strong());
                ui.add(
                    egui::TextEdit::singleline(&mut self.install_dir)
                        .desired_width((ui.available_width() - 8.0).max(200.0)),
                );
            });
            ui.add_space(6.0);
            ui.horizontal_wrapped(|ui| {
                if ui
                    .button("Dossier Program Files (Standard)")
                    .on_hover_cursor(CursorIcon::PointingHand)
                    .clicked()
                {
                    self.install_dir = get_default_install_dir().to_string_lossy().to_string();
                }
                if ui
                    .button("Dossier Utilisateur (AppData Local)")
                    .on_hover_cursor(CursorIcon::PointingHand)
                    .clicked()
                {
                    self.install_dir = get_user_local_install_dir().to_string_lossy().to_string();
                }
                if ui
                    .button("Parcourir...")
                    .on_hover_cursor(CursorIcon::PointingHand)
                    .clicked()
                {
                    if let Some(chosen) = browse_install_folder(&self.install_dir) {
                        self.install_dir = chosen;
                    }
                }
            });

            ui.add_space(10.0);
            egui::Frame::none()
                .fill(WIN11_SUBTLE)
                .stroke(Stroke::new(1.0_f32, WIN11_BORDER))
                .inner_margin(12.0)
                .rounding(Rounding::same(8.0))
                .show(ui, |ui| {
                    ui.label(RichText::new("Architecture logicielle complète installée :").strong().color(WIN11_BLUE));
                    ui.label("• cooplink.exe & unins000.exe (Application principale & Désinstalleur)");
                    ui.label("• bin\\ (cooplink-capture-service.exe, cooplink-stream-viewer.exe, moteurs DLL)");
                    ui.label("• drivers\\ (Pilotes Bus Manettes 4 Joueurs, Affichage Virtuel & LAN P2P)");
                    ui.label("• config\\, resources\\, locales\\, LICENSE.txt & README.txt");
                });
        });
    }

    fn render_options(&mut self, ui: &mut Ui) {
        let lang = self.lang;
        ui.vertical(|ui| {
            ui.label(
                RichText::new(match lang {
                    InstallerLang::Fr => "Diagnostic Matériel GPU & Installation Automatique des Pilotes",
                    InstallerLang::En => "GPU Hardware Check & Automatic Driver Installation",
                    InstallerLang::Es => "Diagnóstico GPU e Instalación Automática de Controladores",
                    InstallerLang::De => "GPU-Prüfung & Automatische Treiberinstallation",
                    InstallerLang::Pt => "Diagnóstico GPU e Instalação Automática de Controladores",
                })
                .color(WIN11_TEXT_MAIN)
                .strong()
                .size(16.0),
            );
            ui.add_space(8.0);

            // Encadré diagnostic matériel
            egui::Frame::none()
                .fill(WIN11_SUBTLE)
                .stroke(Stroke::new(1.0_f32, WIN11_BORDER))
                .inner_margin(12.0)
                .rounding(Rounding::same(8.0))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        draw_dot(ui, WIN11_GREEN, 4.0);
                        ui.label(RichText::new(format!("Carte Graphique (GPU) détectée : {}", self.hw_check.gpu_name)).strong());
                    });
                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        let col = if self.hw_check.gamepad_bus_present { WIN11_GREEN } else { WIN11_ORANGE };
                        draw_dot(ui, col, 4.0);
                        let txt = if self.hw_check.gamepad_bus_present {
                            "Pilote Bus Manettes Virtuelles (Multi-Joueurs) : Déjà présent sur ce PC"
                        } else {
                            "Pilote Bus Manettes Virtuelles : Manquant — Sera installé automatiquement"
                        };
                        ui.label(RichText::new(txt).color(WIN11_TEXT_SUB));
                    });
                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        let col = if self.hw_check.virtual_display_present { WIN11_GREEN } else { WIN11_ORANGE };
                        draw_dot(ui, col, 4.0);
                        let txt = if self.hw_check.virtual_display_present {
                            "Adaptateur d'Affichage & Capture DirectX 12 : Compatible et prêt"
                        } else {
                            "Adaptateur d'Affichage Virtuel : Sera configuré automatiquement"
                        };
                        ui.label(RichText::new(txt).color(WIN11_TEXT_SUB));
                    });
                });

            ui.add_space(10.0);

            ui.checkbox(
                &mut self.install_gamepad_driver,
                RichText::new(match lang {
                    InstallerLang::Fr => "Installer / Mettre à jour le pilote Bus Manettes Virtuelles CoopLink (Recommandé pour jouer à 2-4)",
                    InstallerLang::En => "Install / Update CoopLink Virtual Gamepad Bus Driver (Recommended for 2-4 players)",
                    InstallerLang::Es => "Instalar controlador de Mandos Virtuales CoopLink (Recomendado)",
                    InstallerLang::De => "CoopLink Virtuellen Controller-Treiber installieren (Empfohlen)",
                    InstallerLang::Pt => "Instalar controlador de Comandos Virtuais CoopLink (Recomendado)",
                })
                .color(WIN11_TEXT_MAIN)
                .strong(),
            );

            ui.checkbox(
                &mut self.install_virtual_display_driver,
                RichText::new(match lang {
                    InstallerLang::Fr => "Configurer l'Adaptateur d'Affichage Virtuel & Capture Premier Plan",
                    InstallerLang::En => "Configure Virtual Display Adapter & Foreground Game Capture",
                    InstallerLang::Es => "Configurar Adaptador de Pantalla Virtual y Captura de Juego",
                    InstallerLang::De => "Virtuellen Anzeigeadapter & Spielaufnahme konfigurieren",
                    InstallerLang::Pt => "Configurar Adaptador de Ecrã Virtual e Captura de Jogo",
                })
                .color(WIN11_TEXT_MAIN),
            );

            ui.checkbox(
                &mut self.configure_firewall,
                RichText::new(match lang {
                    InstallerLang::Fr => "Configurer automatiquement le Pare-feu Windows Defender",
                    InstallerLang::En => "Automatically configure Windows Defender Firewall rules",
                    InstallerLang::Es => "Configurar automáticamente el Cortafuegos de Windows",
                    InstallerLang::De => "Windows Defender Firewall automatisch konfigurieren",
                    InstallerLang::Pt => "Configurar automaticamente a Firewall do Windows",
                })
                .color(WIN11_TEXT_MAIN),
            );

            ui.checkbox(
                &mut self.create_desktop_shortcut,
                RichText::new(match lang {
                    InstallerLang::Fr => "Créer un raccourci sur le Bureau et dans le Menu Démarrer",
                    InstallerLang::En => "Create Desktop and Start Menu shortcuts",
                    InstallerLang::Es => "Crear accesos directos en el Escritorio y Menú Inicio",
                    InstallerLang::De => "Desktop- und Startmenü-Verknüpfungen erstellen",
                    InstallerLang::Pt => "Criar atalhos no Ambiente de Trabalho e Menu Iniciar",
                })
                .color(WIN11_TEXT_MAIN),
            );
        });
    }

    fn render_installing(&self, ui: &mut Ui) {
        let is_unins = self.step == InstallStep::Uninstalling;
        ui.vertical_centered(|ui| {
            ui.add_space(30.0);
            ui.label(
                RichText::new(if is_unins {
                    "Désinstallation de CoopLink et nettoyage des raccourcis en cours..."
                } else {
                    "Installation de CoopLink et des pilotes virtuels en cours..."
                })
                .color(WIN11_TEXT_MAIN)
                .strong()
                .size(17.0),
            );
            ui.add_space(18.0);

            let progress_bar = egui::ProgressBar::new(self.progress)
                .show_percentage()
                .animate(true)
                .fill(if is_unins { WIN11_ORANGE } else { WIN11_BLUE });

            ui.add_sized([ui.available_width() - 40.0, 22.0], progress_bar);

            ui.add_space(14.0);
            ui.label(RichText::new(&self.status_label).color(WIN11_TEXT_SUB));

            if let Some(ref err) = self.error_msg {
                ui.add_space(10.0);
                ui.label(RichText::new(format!("Erreur : {}", err)).color(Color32::RED));
            }
        });
    }

    fn render_uninstall_finished(&self, ui: &mut Ui) {
        ui.vertical(|ui| {
            ui.add_space(10.0);
            ui.horizontal(|ui| {
                draw_dot(ui, WIN11_GREEN, 5.5);
                ui.label(
                    RichText::new("CoopLink a été entièrement désinstallé")
                        .color(WIN11_GREEN)
                        .strong()
                        .size(18.0),
                );
            });
            ui.add_space(10.0);
            ui.label(
                RichText::new(
                    "L'icône du Bureau, le raccourci du Menu Démarrer, les entrées du registre et tous les fichiers d'installation ont été supprimés.",
                )
                .color(WIN11_TEXT_SUB),
            );
        });
    }

    fn render_finished(&mut self, ui: &mut Ui) {
        ui.vertical(|ui| {
            ui.add_space(10.0);
            ui.horizontal(|ui| {
                draw_dot(ui, WIN11_GREEN, 5.5);
                ui.label(
                    RichText::new("CoopLink et ses pilotes ont été installés avec succès")
                        .color(WIN11_GREEN)
                        .strong()
                        .size(18.0),
                );
            });
            ui.add_space(10.0);

            ui.label(
                RichText::new(
                    "Votre ordinateur est maintenant prêt. Tous les pilotes requis (Réseau P2P, Bus Manettes Virtuelles et Capture de Jeu au Premier Plan) sont opérationnels.",
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
                RichText::new("Cliquez sur « Terminer » pour fermer l'assistant.")
                    .color(WIN11_TEXT_MUTED)
                    .small(),
            );
        });
    }

    fn start_installation(&mut self) {
        if self.install_started.swap(true, Ordering::SeqCst) {
            return;
        }

        self.progress_pct.store(5, Ordering::SeqCst);
        *self.status_shared.lock() = "Étape 1/5 — Création de l'arborescence logicielle CoopLink...".to_string();

        let target_dir = PathBuf::from(&self.install_dir);
        let create_desktop = self.create_desktop_shortcut;
        let create_start = self.create_start_menu_shortcut;
        let setup_firewall = self.configure_firewall;
        let need_gamepad_drv = self.install_gamepad_driver && !self.hw_check.gamepad_bus_present;
        let need_vdd_drv = self.install_virtual_display_driver;
        let hw_info = self.hw_check.clone();
        let complete_flag = self.install_complete.clone();
        let prog_pct = self.progress_pct.clone();
        let status_txt = self.status_shared.clone();

        thread::spawn(move || {
            thread::sleep(Duration::from_millis(220));
            prog_pct.store(20, Ordering::SeqCst);
            *status_txt.lock() = "Étape 2/5 — Déploiement des binaires (cooplink.exe, services, moteurs DLL)...".to_string();

            let current_exe = env::current_exe().unwrap_or_default();
            let source_cooplink = current_exe
                .parent()
                .map(|p| p.join("cooplink.exe"))
                .unwrap_or_else(|| PathBuf::from("cooplink.exe"));

            let effective_dir = populate_complete_software_tree(
                &target_dir,
                &source_cooplink,
                &current_exe,
                &hw_info,
            );
            let target_exe = effective_dir.join("cooplink.exe");

            thread::sleep(Duration::from_millis(250));
            prog_pct.store(48, Ordering::SeqCst);
            *status_txt.lock() = "Étape 3/5 — Vérification et installation des pilotes virtuels (Bus Manettes & Affichage)...".to_string();

            install_missing_drivers_silently(&effective_dir, need_gamepad_drv, need_vdd_drv);

            thread::sleep(Duration::from_millis(220));
            prog_pct.store(72, Ordering::SeqCst);
            *status_txt.lock() = "Étape 4/5 — Création des raccourcis Bureau et Menu Démarrer...".to_string();

            let _ = create_desktop_and_start_shortcuts(&target_exe, create_desktop, create_start);

            thread::sleep(Duration::from_millis(220));
            prog_pct.store(90, Ordering::SeqCst);
            *status_txt.lock() = "Étape 5/5 — Configuration silencieuse du Pare-feu Windows & Registre...".to_string();

            if setup_firewall {
                let _ = setup_silent_firewall(&target_exe);
            }

            let _ = register_in_windows_uninstall(&effective_dir, &target_exe);

            prog_pct.store(100, Ordering::SeqCst);
            *status_txt.lock() = "Installation terminée avec succès (100%) !".to_string();
            thread::sleep(Duration::from_millis(260));

            complete_flag.store(true, Ordering::SeqCst);
        });
    }

    fn start_uninstallation(&mut self) {
        if self.install_started.swap(true, Ordering::SeqCst) {
            return;
        }
        self.progress_pct.store(15, Ordering::SeqCst);
        *self.status_shared.lock() = "Suppression des raccourcis Bureau, Menu Démarrer et fichiers...".to_string();

        let complete_flag = self.install_complete.clone();
        let prog_pct = self.progress_pct.clone();
        let status_txt = self.status_shared.clone();

        thread::spawn(move || {
            thread::sleep(Duration::from_millis(250));
            prog_pct.store(55, Ordering::SeqCst);
            *status_txt.lock() = "Suppression de l'icône du Bureau et nettoyage du registre...".to_string();
            let _ = perform_complete_uninstall();
            prog_pct.store(100, Ordering::SeqCst);
            *status_txt.lock() = "Désinstallation terminée (100%) !".to_string();
            thread::sleep(Duration::from_millis(250));
            complete_flag.store(true, Ordering::SeqCst);
        });
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
    style.interaction.selectable_labels = false;
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
