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
    #[allow(dead_code)]
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
                    .inner_margin(egui::Margin::symmetric(24.0, 16.0))
                    .stroke(Stroke::new(1.0_f32, WIN11_BORDER)),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    if let Some(ref logo) = self.logo_texture {
                        ui.image((logo.id(), egui::vec2(42.0, 42.0)));
                    }
                    ui.add_space(8.0);
                    ui.vertical(|ui| {
                        ui.label(
                            RichText::new("CoopLink")
                                .color(WIN11_TEXT_MAIN)
                                .strong()
                                .size(20.0),
                        );
                        let step_subtitle = match self.step {
                            InstallStep::Welcome => match lang {
                                InstallerLang::Fr => "Étape 1 / 5 — Présentation & Bienvenue",
                                InstallerLang::En => "Step 1 / 5 — Overview & Welcome",
                                InstallerLang::Es => "Paso 1 / 5 — Presentación y Bienvenida",
                                InstallerLang::De => "Schritt 1 / 5 — Übersicht & Willkommen",
                                InstallerLang::Pt => "Passo 1 / 5 — Apresentação e Boas-vindas",
                            },
                            InstallStep::License => match lang {
                                InstallerLang::Fr => "Étape 2 / 5 — Contrat de Licence & Décharge de Responsabilité",
                                InstallerLang::En => "Step 2 / 5 — End-User License Agreement & Liability Disclaimer",
                                InstallerLang::Es => "Paso 2 / 5 — Contrato de Licencia y Exención de Responsabilidad",
                                InstallerLang::De => "Schritt 2 / 5 — Lizenzvertrag & Haftungsausschluss",
                                InstallerLang::Pt => "Passo 2 / 5 — Contrato de Licença e Exclusão de Responsabilidade",
                            },
                            InstallStep::Destination => match lang {
                                InstallerLang::Fr => "Étape 3 / 5 — Dossier d'installation",
                                InstallerLang::En => "Step 3 / 5 — Destination Folder",
                                InstallerLang::Es => "Paso 3 / 5 — Carpeta de destino",
                                InstallerLang::De => "Schritt 3 / 5 — Zielordner",
                                InstallerLang::Pt => "Passo 3 / 5 — Pasta de destino",
                            },
                            InstallStep::OptionsAndDrivers => match lang {
                                InstallerLang::Fr => "Étape 4 / 5 — Vérification Matérielle & Options",
                                InstallerLang::En => "Step 4 / 5 — Hardware Check & Options",
                                InstallerLang::Es => "Paso 4 / 5 — Verificación de Hardware y Opciones",
                                InstallerLang::De => "Schritt 4 / 5 — Hardware-Prüfung & Optionen",
                                InstallerLang::Pt => "Passo 4 / 5 — Verificação de Hardware e Opções",
                            },
                            InstallStep::Installing => match lang {
                                InstallerLang::Fr => "Étape 5 / 5 — Installation des fichiers et des services...",
                                InstallerLang::En => "Step 5 / 5 — Installing software and services...",
                                InstallerLang::Es => "Paso 5 / 5 — Instalando software y servicios...",
                                InstallerLang::De => "Schritt 5 / 5 — Installation läuft...",
                                InstallerLang::Pt => "Passo 5 / 5 — A instalar software e serviços...",
                            },
                            InstallStep::Finished => match lang {
                                InstallerLang::Fr => "Installation terminée avec succès",
                                InstallerLang::En => "Installation completed successfully",
                                InstallerLang::Es => "Instalación completada con éxito",
                                InstallerLang::De => "Installation erfolgreich abgeschlossen",
                                InstallerLang::Pt => "Instalação concluída com sucesso",
                            },
                            InstallStep::Uninstalling => match lang {
                                InstallerLang::Fr => "Désinstallation complète en cours...",
                                InstallerLang::En => "Uninstalling CoopLink...",
                                _ => "Désinstallation en cours...",
                            },
                            InstallStep::UninstallFinished => match lang {
                                InstallerLang::Fr => "Désinstallation terminée",
                                InstallerLang::En => "Uninstallation completed",
                                _ => "Désinstallation terminée",
                            },
                        };
                        ui.label(
                            RichText::new(step_subtitle)
                                .color(WIN11_BLUE)
                                .size(13.0)
                                .strong(),
                        );
                    });

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        egui::ComboBox::from_id_salt("installer_lang_combo")
                            .selected_text(RichText::new(self.lang.code()).color(WIN11_BLUE).strong())
                            .width(70.0)
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
                    .inner_margin(egui::Margin::symmetric(24.0, 14.0))
                    .stroke(Stroke::new(1.0_f32, WIN11_BORDER)),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    draw_dot(ui, WIN11_GREEN, 4.0);
                    ui.label(
                        RichText::new("CoopLink v1.0.0 — Édition Officielle Windows 11")
                            .color(WIN11_TEXT_MUTED)
                            .size(12.0),
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
                                    RichText::new(next_lbl).color(Color32::WHITE).strong().size(13.5),
                                )
                                .fill(WIN11_BLUE)
                                .rounding(Rounding::same(8.0))
                                .min_size(egui::vec2(115.0, 36.0));

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
                                        .strong()
                                        .size(13.5),
                                )
                                .fill(if self.license_accepted {
                                    WIN11_BLUE
                                } else {
                                    Color32::from_rgb(226, 232, 240)
                                })
                                .rounding(Rounding::same(8.0))
                                .min_size(egui::vec2(115.0, 36.0));

                                if ui.add_enabled(self.license_accepted, next_btn).clicked() {
                                    self.step = InstallStep::Destination;
                                }

                                if ui.add_sized([105.0, 36.0], egui::Button::new(prev_lbl)).clicked() {
                                    self.step = InstallStep::Welcome;
                                }
                            }
                            InstallStep::Destination => {
                                let btn = egui::Button::new(
                                    RichText::new(next_lbl).color(Color32::WHITE).strong().size(13.5),
                                )
                                .fill(WIN11_BLUE)
                                .rounding(Rounding::same(8.0))
                                .min_size(egui::vec2(115.0, 36.0));

                                if ui.add(btn).on_hover_cursor(CursorIcon::PointingHand).clicked() {
                                    self.step = InstallStep::OptionsAndDrivers;
                                }

                                if ui.add_sized([105.0, 36.0], egui::Button::new(prev_lbl)).clicked() {
                                    self.step = InstallStep::License;
                                }
                            }
                            InstallStep::OptionsAndDrivers => {
                                let install_lbl = match lang {
                                    InstallerLang::Fr => "Installer maintenant",
                                    InstallerLang::En => "Install Now",
                                    InstallerLang::Es => "Instalar ahora",
                                    InstallerLang::De => "Jetzt installieren",
                                    InstallerLang::Pt => "Instalar agora",
                                };
                                let install_btn = egui::Button::new(
                                    RichText::new(install_lbl).color(Color32::WHITE).strong().size(13.5),
                                )
                                .fill(WIN11_BLUE)
                                .rounding(Rounding::same(8.0))
                                .min_size(egui::vec2(150.0, 36.0));

                                if ui.add(install_btn).on_hover_cursor(CursorIcon::PointingHand).clicked() {
                                    self.step = InstallStep::Installing;
                                    self.start_installation();
                                }

                                if ui.add_sized([105.0, 36.0], egui::Button::new(prev_lbl)).clicked() {
                                    self.step = InstallStep::Destination;
                                }
                            }
                            InstallStep::Installing => {
                                ui.label(
                                    RichText::new("Installation en cours...")
                                        .color(WIN11_BLUE)
                                        .strong(),
                                );
                            }
                            InstallStep::Finished => {
                                let finish_lbl = match lang {
                                    InstallerLang::Fr => "Terminer et lancer CoopLink",
                                    InstallerLang::En => "Finish & Launch CoopLink",
                                    InstallerLang::Es => "Finalizar y Abrir",
                                    InstallerLang::De => "Fertigstellen & Starten",
                                    InstallerLang::Pt => "Concluir e Abrir",
                                };
                                let finish_btn = egui::Button::new(
                                    RichText::new(finish_lbl).color(Color32::WHITE).strong().size(13.5),
                                )
                                .fill(WIN11_GREEN)
                                .rounding(Rounding::same(8.0))
                                .min_size(egui::vec2(190.0, 36.0));

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
                                    RichText::new("Fermer").color(Color32::WHITE).strong().size(13.5),
                                )
                                .fill(WIN11_BLUE)
                                .rounding(Rounding::same(8.0))
                                .min_size(egui::vec2(115.0, 36.0));
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
                    .inner_margin(20.0),
            )
            .show(ctx, |ui| {
                egui::Frame::none()
                    .fill(WIN11_CARD)
                    .stroke(Stroke::new(1.0_f32, WIN11_BORDER))
                    .inner_margin(22.0)
                    .rounding(Rounding::same(12.0))
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
                    InstallerLang::Fr => "Bienvenue dans l'assistant d'installation de CoopLink",
                    InstallerLang::En => "Welcome to the CoopLink Setup Wizard",
                    InstallerLang::Es => "Bienvenido al asistente de instalación de CoopLink",
                    InstallerLang::De => "Willkommen beim CoopLink-Installationsassistenten",
                    InstallerLang::Pt => "Bem-vindo ao assistente de instalação do CoopLink",
                })
                .color(WIN11_TEXT_MAIN)
                .strong()
                .size(17.5),
            );
            ui.add_space(8.0);

            ui.label(
                RichText::new(match lang {
                    InstallerLang::Fr => "CoopLink permet d'interconnecter plusieurs ordinateurs à distance avec une sécurité de niveau entreprise et une latence ultra-faible :",
                    InstallerLang::En => "CoopLink connects remote computers with enterprise-grade security and ultra-low latency:",
                    InstallerLang::Es => "CoopLink conecta equipos a distancia con seguridad avanzada y latencia ultra baja:",
                    InstallerLang::De => "CoopLink verbindet Computer aus der Ferne mit maximaler Sicherheit und minimaler Latenz:",
                    InstallerLang::Pt => "O CoopLink liga computadores à distância com máxima segurança e ultra baixa latência:",
                })
                .color(WIN11_TEXT_SUB)
                .size(13.5),
            );

            ui.add_space(16.0);

            egui::Frame::none()
                .fill(WIN11_SUBTLE)
                .stroke(Stroke::new(1.0_f32, WIN11_BORDER))
                .inner_margin(18.0)
                .rounding(Rounding::same(10.0))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        draw_dot(ui, WIN11_BLUE, 5.0);
                        ui.vertical(|ui| {
                            ui.label(
                                RichText::new(match lang {
                                    InstallerLang::Fr => "Réseau Local Virtuel Chiffré E2E (LAN P2P Direct)",
                                    InstallerLang::En => "Encrypted E2E Virtual Local Network (Direct P2P LAN)",
                                    InstallerLang::Es => "Red Local Virtual Cifrada E2E (LAN P2P Directo)",
                                    InstallerLang::De => "Ende-zu-Ende verschlüsseltes Virtuelles LAN (P2P)",
                                    InstallerLang::Pt => "Rede Local Virtual Encriptada E2E (LAN P2P Direto)",
                                })
                                .color(WIN11_BLUE)
                                .strong()
                                .size(14.5),
                            );
                            ui.add_space(2.0);
                            ui.label(
                                RichText::new(match lang {
                                    InstallerLang::Fr => "Chiffrement ChaCha20-Poly1305 avec contrôle d'intégrité strict. Jouez à vos jeux multijoueurs LAN comme si vos ordinateurs étaient reliés par un câble réseau physique, sans aucune redirection de port.",
                                    InstallerLang::En => "ChaCha20-Poly1305 authenticated encryption. Play LAN games as if your PCs were connected by a direct physical Ethernet cable, with zero manual router setup.",
                                    InstallerLang::Es => "Cifrado ChaCha20-Poly1305. Juega a cualquier juego LAN como si estuvieran en la misma red local, sin configurar puertos.",
                                    InstallerLang::De => "ChaCha20-Poly1305 Authentifizierung. Spiele alle LAN-Spiele ohne manuelle Portweiterleitung.",
                                    InstallerLang::Pt => "Encriptação ChaCha20-Poly1305. Jogue qualquer jogo LAN sem configurar portas no router.",
                                })
                                .color(WIN11_TEXT_SUB)
                                .size(13.0),
                            );
                        });
                    });

                    ui.add_space(12.0);

                    ui.horizontal(|ui| {
                        draw_dot(ui, WIN11_GREEN, 5.0);
                        ui.vertical(|ui| {
                            ui.label(
                                RichText::new(match lang {
                                    InstallerLang::Fr => "Partage de Jeu au Premier Plan & Manettes Co-op Virtuelles",
                                    InstallerLang::En => "Foreground Game Stream & Virtual Multi-Gamepads",
                                    InstallerLang::Es => "Juego Compartido y Mandos Co-op Virtuales",
                                    InstallerLang::De => "Vordergrund-Spielfreigabe & Virtuelle Controller",
                                    InstallerLang::Pt => "Partilha de Jogo e Comandos Virtuais Co-op",
                                })
                                .color(WIN11_GREEN)
                                .strong()
                                .size(14.5),
                            );
                            ui.add_space(2.0);
                            ui.label(
                                RichText::new(match lang {
                                    InstallerLang::Fr => "Partagez exclusivement la fenêtre du jeu actif sans aucun écho audio du chat vocal. Vos amis jouent avec leurs manettes (Joueurs 2, 3, 4) ou leur clavier avec décodage matériel haute performance.",
                                    InstallerLang::En => "Stream only the active game window with zero microphone or voice chat echo. Friends play with their controllers or keyboard with dedicated hardware decoding.",
                                    InstallerLang::Es => "Transmite únicamente la ventana de juego activa sin eco de voz. Tus amigos se unen con sus mandos o teclado.",
                                    InstallerLang::De => "Streame ausschließlich das aktive Spiel ohne Sprach-Echo. Freunde treten mit Controllern bei.",
                                    InstallerLang::Pt => "Transmita apenas a janela ativa sem eco de voz. Amigos jogam com os comandos.",
                                })
                                .color(WIN11_TEXT_SUB)
                                .size(13.0),
                            );
                        });
                    });
                });

            ui.add_space(20.0);
            ui.label(
                RichText::new(match lang {
                    InstallerLang::Fr => "Cliquez sur « Suivant » pour prendre connaissance du contrat de licence et de la décharge légale de responsabilité.",
                    InstallerLang::En => "Click 'Next' to review the software license agreement and legal disclaimer.",
                    InstallerLang::Es => "Haz clic en « Siguiente » para revisar el acuerdo legal y continuar.",
                    InstallerLang::De => "Klicke auf „Weiter“, um die Lizenzvereinbarung einzusehen.",
                    InstallerLang::Pt => "Clique em « Seguinte » para analisar o acordo legal.",
                })
                .color(WIN11_TEXT_MUTED)
                .size(13.0),
            );
            ui.add_space(6.0);
        });
    }

    fn render_license(&mut self, ui: &mut Ui) {
        let lang = self.lang;
        ui.vertical(|ui| {
            ui.label(
                RichText::new(match lang {
                    InstallerLang::Fr => "Contrat de Licence Logicielle & Limitation de Responsabilité",
                    InstallerLang::En => "Software License Agreement & Limitation of Liability",
                    InstallerLang::Es => "Contrato de Licencia y Exención de Responsabilidad",
                    InstallerLang::De => "Software-Lizenzvertrag & Haftungsausschluss",
                    InstallerLang::Pt => "Contrato de Licença e Exclusão de Responsabilidade",
                })
                .color(WIN11_TEXT_MAIN)
                .strong()
                .size(16.5),
            );
            ui.label(
                RichText::new(match lang {
                    InstallerLang::Fr => "Veuillez lire attentivement l'intégralité des clauses contractuelles ci-dessous avant de poursuivre :",
                    InstallerLang::En => "Please read the following contractual clauses carefully before proceeding:",
                    InstallerLang::Es => "Lee atentamente los términos del contrato antes de continuar:",
                    InstallerLang::De => "Bitte lies die folgenden Vertragsbedingungen sorgfältig durch:",
                    InstallerLang::Pt => "Leia atentamente as cláusulas contratuais antes de prosseguir:",
                })
                .color(WIN11_TEXT_SUB)
                .size(13.0),
            );
            ui.add_space(8.0);

            let eula_text = match lang {
                InstallerLang::Fr => {
                    "CONTRAT DE LICENCE UTILISATEUR FINAL (CLUF) ET DÉCHARGE DE RESPONSABILITÉ — COOPLINK\n\
                     Éditeur : Loic HUON. Tous droits réservés.\n\n\
                     AVERTISSEMENT LÉGAL IMPORTANT :\n\
                     Le logiciel « CoopLink » est un outil technique d'interconnexion réseau virtuel poste à poste (Peer-to-Peer) et de capture d'affichage local. L'Éditeur met à disposition cet outil logiciel neutre et n'exerce aucun contrôle, direct ou indirect, sur les usages, transferts, contenus ou activités réalisés par les utilisateurs au moyen du logiciel.\n\n\
                     ARTICLE 1 — EXCLUSION TOTALE DE RESPONSABILITÉ QUANT AUX USAGES DU LOGICIEL\n\
                     L'Utilisateur est expressément informé et reconnaît que l'Éditeur décline toute responsabilité quant à l'utilisation qui est faite de CoopLink. L'Utilisateur assume l'entière, unique et exclusive responsabilité juridique, civile, pénale et matérielle de ses actions, y compris mais sans s'y limiter :\n\
                     a) Toute transmission, partage ou diffusion de contenus, jeux, données ou fichiers protégés par des droits d'auteur, des brevets ou le droit de la propriété intellectuelle sans autorisation expresse de leurs ayants droit respectifs ;\n\
                     b) Toute tentative de contournement d'accès, d'intrusion réseau, d'usurpation d'identité ou d'attaque informatique menée au travers ou à destination des tunnels de communication ;\n\
                     c) Toute utilisation frauduleuse, illégale, immorale ou non conforme aux législations locales et internationales en vigueur ;\n\
                     d) Tout non-respect des conditions d'utilisation imposées par des tiers (plateformes de jeux en ligne, éditeurs de logiciels ou fournisseurs d'accès à Internet).\n\n\
                     ARTICLE 2 — FOURNITURE STRICTE « EN L'ÉTAT » (AS IS) SANS GARANTIE\n\
                     Le Logiciel est fourni « EN L'ÉTAT », selon sa disponibilité, sans aucune garantie expresse, implicite ou légale de conformité, de pérennité, d'absence d'erreurs, de sécurité absolue ou d'adéquation à un besoin spécifique. L'Utilisateur assume l'intégralité des risques inhérents à l'installation d'adaptateurs réseau virtuels, aux règles de pare-feu et au streaming matériel.\n\n\
                     ARTICLE 3 — LIMITATION DES DOMMAGES ET CLAUSE D'INDEMNISATION\n\
                     En aucun cas l'Éditeur ne pourra être tenu responsable de préjudices directs, indirects, accessoires ou consécutifs (perte d'exploitation, altération de fichiers, pannes matérielles, blocages de compte tiers). L'Utilisateur s'engage à garantir, indemniser et dégager l'Éditeur de toute responsabilité en cas de réclamation, poursuite ou condamnation émanant de tiers ou d'autorités judiciaires du fait de ses propres agissements."
                }
                _ => {
                    "END-USER LICENSE AGREEMENT (EULA) AND LEGAL DISCLAIMER — COOPLINK\n\
                     Publisher: Loic HUON. All Rights Reserved.\n\n\
                     IMPORTANT LEGAL NOTICE:\n\
                     'CoopLink' is a neutral networking utility providing peer-to-peer virtual tunneling and local display capture. The Publisher exercises no supervision, control, or monitoring over the activities, content, or data routed through the software by its users.\n\n\
                     SECTION 1 — COMPLETE DISCLAIMER REGARDING USER CONDUCT AND USAGE\n\
                     The User acknowledges and agrees that the Publisher disclaims any and all liability regarding how CoopLink is used. The User bears sole and exclusive legal, civil, and criminal responsibility for all actions performed using the software, including without limitation:\n\
                     a) Any transmission, sharing, or distribution of copyrighted files, games, or intellectual property without authorization from legitimate rights holders;\n\
                     b) Any unauthorized network access, spoofing, tampering, or malicious activity conducted across or against peer connections;\n\
                     c) Any illegal, fraudulent, or non-compliant use under applicable local or international laws;\n\
                     d) Any violation of third-party terms of service (such as game publisher EULAs or ISP contracts).\n\n\
                     SECTION 2 — 'AS IS' PROVISION WITHOUT WARRANTIES\n\
                     THE SOFTWARE IS DELIVERED 'AS IS' AND 'WITH ALL FAULTS', WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED. THE ENTIRE RISK AS TO INSTALLATION, VIRTUAL NETWORK ADAPTERS, FIREWALL MODIFICATIONS, AND STREAMING PERFORMANCE IS ASSUMED SOLELY BY THE USER.\n\n\
                     SECTION 3 — INDEMNIFICATION\n\
                     IN NO EVENT SHALL THE PUBLISHER BE LIABLE FOR ANY DAMAGES ARISING FROM SOFTWARE USE. THE USER AGREES TO INDEMNIFY AND HOLD HARMLESS THE PUBLISHER AGAINST ANY THIRD-PARTY CLAIMS OR PROCEEDINGS RESULTING FROM THE USER'S ACTIONS."
                }
            };

            egui::Frame::none()
                .fill(WIN11_SUBTLE)
                .stroke(Stroke::new(1.0_f32, WIN11_BORDER))
                .inner_margin(12.0)
                .rounding(Rounding::same(8.0))
                .show(ui, |ui| {
                    ScrollArea::vertical()
                        .max_height(200.0)
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            ui.label(
                                RichText::new(eula_text)
                                    .color(WIN11_TEXT_MAIN)
                                    .size(12.5),
                            );
                        });
                });

            ui.add_space(16.0);
            ui.checkbox(
                &mut self.license_accepted,
                RichText::new(match lang {
                    InstallerLang::Fr => "J'ai lu, compris et j'accepte sans réserve les termes du contrat et l'exonération totale de responsabilité de l'Éditeur.",
                    InstallerLang::En => "I have read, understood, and accept the terms of agreement and complete limitation of publisher liability.",
                    InstallerLang::Es => "He leído y acepto los términos del contrato y la exención total de responsabilidad.",
                    InstallerLang::De => "Ich habe die Bedingungen gelesen und akzeptiere den vollständigen Haftungsausschluss.",
                    InstallerLang::Pt => "Li e aceito os termos do contrato e a exclusão total de responsabilidade.",
                })
                .color(WIN11_TEXT_MAIN)
                .strong()
                .size(13.0),
            );
            ui.add_space(8.0);
        });
    }

    fn render_destination(&mut self, ui: &mut Ui) {
        let lang = self.lang;
        ui.vertical(|ui| {
            ui.label(
                RichText::new(match lang {
                    InstallerLang::Fr => "Emplacement d'installation du logiciel",
                    InstallerLang::En => "Installation Directory",
                    InstallerLang::Es => "Ubicación de instalación",
                    InstallerLang::De => "Installationsverzeichnis",
                    InstallerLang::Pt => "Local de instalação",
                })
                .color(WIN11_TEXT_MAIN)
                .strong()
                .size(16.5),
            );
            ui.add_space(4.0);
            ui.label(
                RichText::new(match lang {
                    InstallerLang::Fr => "Choisissez le répertoire de destination où CoopLink sera installé :",
                    InstallerLang::En => "Select the destination folder where CoopLink will be installed:",
                    InstallerLang::Es => "Elige la carpeta donde se instalará CoopLink:",
                    InstallerLang::De => "Wähle den Zielordner für CoopLink:",
                    InstallerLang::Pt => "Escolha a pasta de destino para o CoopLink:",
                })
                .color(WIN11_TEXT_SUB)
                .size(13.0),
            );
            ui.add_space(12.0);

            ui.horizontal(|ui| {
                ui.label(RichText::new("Dossier :").color(WIN11_TEXT_MAIN).strong().size(13.5));
                ui.add_space(4.0);
                ui.add(
                    egui::TextEdit::singleline(&mut self.install_dir)
                        .desired_width(320.0),
                );
                ui.add_space(6.0);
                if ui
                    .button(match lang {
                        InstallerLang::Fr => "Parcourir...",
                        InstallerLang::En => "Browse...",
                        _ => "Parcourir...",
                    })
                    .clicked()
                {
                    if let Some(folder) = browse_install_folder(&self.install_dir) {
                        self.install_dir = folder;
                    }
                }
                if ui.button("Program Files").clicked() {
                    self.install_dir = get_default_install_dir().to_string_lossy().to_string();
                }
            });

            ui.add_space(18.0);

            egui::Frame::none()
                .fill(WIN11_SUBTLE)
                .stroke(Stroke::new(1.0_f32, WIN11_BORDER))
                .inner_margin(16.0)
                .rounding(Rounding::same(8.0))
                .show(ui, |ui| {
                    ui.label(
                        RichText::new("Composants inclus dans l'installation standard :")
                            .color(WIN11_BLUE)
                            .strong()
                            .size(13.5),
                    );
                    ui.add_space(8.0);
                    ui.label(
                        RichText::new("• cooplink.exe & unins000.exe (Application principale & Utilitaire de désinstallation)")
                            .color(WIN11_TEXT_MAIN)
                            .size(12.5),
                    );
                    ui.add_space(5.0);
                    ui.label(
                        RichText::new("• bin\\ (Descripteurs des modules Capture 120 FPS, Stream Viewer & WASAPI Audio)")
                            .color(WIN11_TEXT_MAIN)
                            .size(12.5),
                    );
                    ui.add_space(5.0);
                    ui.label(
                        RichText::new("• drivers\\ (Manifeste matériel, Bus Manettes 4 Joueurs & Adaptateur LAN Virtuel)")
                            .color(WIN11_TEXT_MAIN)
                            .size(12.5),
                    );
                    ui.add_space(5.0);
                    ui.label(
                        RichText::new("• config\\ & resources\\ (Profils optimisés Métrique 5, Pare-feu, CLUF & Icônes HD)")
                            .color(WIN11_TEXT_MAIN)
                            .size(12.5),
                    );
                });
        });
    }

    fn render_options(&mut self, ui: &mut Ui) {
        let lang = self.lang;
        ui.vertical(|ui| {
            ui.label(
                RichText::new(match lang {
                    InstallerLang::Fr => "Options de configuration et pilotes recommandés",
                    InstallerLang::En => "Setup Options & Recommended Drivers",
                    InstallerLang::Es => "Opciones y controladores recomendados",
                    InstallerLang::De => "Optionen & Empfohlene Treiber",
                    InstallerLang::Pt => "Opções e controladores recomendados",
                })
                .color(WIN11_TEXT_MAIN)
                .strong()
                .size(16.5),
            );
            ui.add_space(4.0);
            ui.label(
                RichText::new(match lang {
                    InstallerLang::Fr => "Vérification automatique de l'accélération matérielle et configuration du système :",
                    InstallerLang::En => "Automatic hardware acceleration detection and system setup:",
                    _ => "Configuration automatique du système :",
                })
                .color(WIN11_TEXT_SUB)
                .size(13.0),
            );
            ui.add_space(10.0);

            // Carte de diagnostic matériel
            egui::Frame::none()
                .fill(WIN11_SUBTLE)
                .stroke(Stroke::new(1.0_f32, WIN11_BORDER))
                .inner_margin(12.0)
                .rounding(Rounding::same(8.0))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        draw_dot(ui, WIN11_GREEN, 4.5);
                        ui.label(
                            RichText::new(format!("GPU Détecté : {}", self.hw_check.gpu_name))
                                .color(WIN11_TEXT_MAIN)
                                .strong()
                                .size(13.0),
                        );
                    });
                    if let Some(ref vdd) = self.hw_check.virtual_display_adapter_name {
                        ui.add_space(4.0);
                        ui.label(
                            RichText::new(format!("Adaptateur d'affichage virtuel : {}", vdd))
                                .color(WIN11_TEXT_SUB)
                                .size(12.5),
                        );
                    }
                });

            ui.add_space(14.0);

            ui.checkbox(
                &mut self.create_desktop_shortcut,
                RichText::new(match lang {
                    InstallerLang::Fr => "Créer un raccourci sur le Bureau",
                    InstallerLang::En => "Create Desktop Shortcut",
                    InstallerLang::Es => "Crear acceso directo en el Escritorio",
                    InstallerLang::De => "Desktop-Verknüpfung erstellen",
                    InstallerLang::Pt => "Criar atalho no Ambiente de Trabalho",
                })
                .color(WIN11_TEXT_MAIN)
                .size(13.0),
            );
            ui.add_space(8.0);

            ui.checkbox(
                &mut self.create_start_menu_shortcut,
                RichText::new(match lang {
                    InstallerLang::Fr => "Ajouter CoopLink au Menu Démarrer",
                    InstallerLang::En => "Add CoopLink to Start Menu",
                    InstallerLang::Es => "Añadir al Menú Inicio",
                    InstallerLang::De => "Zum Startmenü hinzufügen",
                    InstallerLang::Pt => "Adicionar ao Menu Iniciar",
                })
                .color(WIN11_TEXT_MAIN)
                .size(13.0),
            );
            ui.add_space(8.0);

            ui.checkbox(
                &mut self.configure_firewall,
                RichText::new(match lang {
                    InstallerLang::Fr => "Autoriser automatiquement le port UDP P2P (42422) dans le Pare-feu Windows",
                    InstallerLang::En => "Automatically allow P2P UDP port (42422) in Windows Firewall",
                    InstallerLang::Es => "Configurar reglas en el Cortafuegos de Windows",
                    InstallerLang::De => "P2P-UDP-Port automatisch in der Windows-Firewall freigeben",
                    InstallerLang::Pt => "Autorizar porta UDP na Firewall do Windows",
                })
                .color(WIN11_TEXT_MAIN)
                .size(13.0),
            );
            ui.add_space(8.0);

            ui.checkbox(
                &mut self.launch_on_finish,
                RichText::new(match lang {
                    InstallerLang::Fr => "Lancer CoopLink à la fin de l'installation",
                    InstallerLang::En => "Launch CoopLink when setup finishes",
                    InstallerLang::Es => "Iniciar CoopLink al finalizar",
                    InstallerLang::De => "CoopLink nach der Installation starten",
                    InstallerLang::Pt => "Iniciar o CoopLink ao concluir",
                })
                .color(WIN11_TEXT_MAIN)
                .size(13.0),
            );
        });
    }

    fn render_installing(&self, ui: &mut Ui) {
        ui.vertical_centered(|ui| {
            ui.add_space(40.0);
            ui.spinner();
            ui.add_space(16.0);
            ui.label(
                RichText::new(&self.status_label)
                    .color(WIN11_TEXT_MAIN)
                    .strong()
                    .size(15.0),
            );
            ui.add_space(20.0);

            let pct_val = (self.progress * 100.0).round() as u32;
            let bar = egui::ProgressBar::new(self.progress)
                .text(RichText::new(format!("{} %", pct_val)).color(Color32::WHITE).strong())
                .animate(true);
            ui.add_sized([420.0, 24.0], bar);
            ui.add_space(40.0);
        });
    }

    fn render_finished(&self, ui: &mut Ui) {
        let lang = self.lang;
        ui.vertical_centered(|ui| {
            ui.add_space(24.0);
            draw_dot(ui, WIN11_GREEN, 14.0);
            ui.add_space(14.0);
            ui.label(
                RichText::new(match lang {
                    InstallerLang::Fr => "CoopLink a été installé avec succès sur votre ordinateur",
                    InstallerLang::En => "CoopLink has been successfully installed on your computer",
                    InstallerLang::Es => "CoopLink se ha instalado con éxito en tu equipo",
                    InstallerLang::De => "CoopLink wurde erfolgreich auf deinem PC installiert",
                    InstallerLang::Pt => "O CoopLink foi instalado com sucesso no seu computador",
                })
                .color(WIN11_TEXT_MAIN)
                .strong()
                .size(17.0),
            );
            ui.add_space(8.0);
            ui.label(
                RichText::new(match lang {
                    InstallerLang::Fr => "L'architecture logicielle, les profils de capture haute performance et les règles réseau sont configurés.",
                    InstallerLang::En => "Software files, high performance capture profiles and networking rules are ready.",
                    _ => "Tout est prêt pour démarrer vos sessions de jeu coopératif.",
                })
                .color(WIN11_TEXT_SUB)
                .size(13.5),
            );
            ui.add_space(16.0);
            ui.label(
                RichText::new(format!("Emplacement : {}", self.install_dir))
                    .color(WIN11_TEXT_MUTED)
                    .size(12.5),
            );
            ui.add_space(24.0);
        });
    }

    fn render_uninstall_finished(&self, ui: &mut Ui) {
        ui.vertical_centered(|ui| {
            ui.add_space(30.0);
            draw_dot(ui, WIN11_BLUE, 12.0);
            ui.add_space(12.0);
            ui.label(
                RichText::new("CoopLink a été entièrement désinstallé de votre ordinateur")
                    .color(WIN11_TEXT_MAIN)
                    .strong()
                    .size(16.0),
            );
            ui.add_space(6.0);
            ui.label(
                RichText::new("Tous les raccourcis du Bureau, les entrées du registre et les fichiers résiduels ont été supprimés.")
                    .color(WIN11_TEXT_SUB)
                    .size(13.0),
            );
            ui.add_space(30.0);
        });
    }

    fn start_installation(&mut self) {
        if self.install_started.swap(true, Ordering::SeqCst) {
            return;
        }

        let target_dir = PathBuf::from(&self.install_dir);
        let current_exe = env::current_exe().unwrap_or_default();
        let hw_check = self.hw_check.clone();

        let desktop_sc = self.create_desktop_shortcut;
        let start_sc = self.create_start_menu_shortcut;
        let setup_fw = self.configure_firewall;
        let bus_drv = self.install_gamepad_driver;
        let disp_drv = self.install_virtual_display_driver;

        let complete_flag = self.install_complete.clone();
        let prog_pct = self.progress_pct.clone();
        let status_txt = self.status_shared.clone();

        thread::spawn(move || {
            prog_pct.store(10, Ordering::SeqCst);
            *status_txt.lock() = "Création de l'arborescence logicielle...".to_string();
            thread::sleep(Duration::from_millis(250));

            let possible_source = current_exe
                .parent()
                .map(|p| p.join("cooplink.exe"))
                .unwrap_or_else(|| current_exe.clone());

            let effective_dir = populate_complete_software_tree(
                &target_dir,
                &possible_source,
                &current_exe,
                &hw_check,
            );
            let installed_exe = effective_dir.join("cooplink.exe");

            prog_pct.store(35, Ordering::SeqCst);
            *status_txt.lock() = "Initialisation des descripteurs de pilotes...".to_string();
            thread::sleep(Duration::from_millis(250));
            install_missing_drivers_silently(&effective_dir, bus_drv, disp_drv);

            prog_pct.store(60, Ordering::SeqCst);
            *status_txt.lock() = "Configuration des règles de pare-feu...".to_string();
            thread::sleep(Duration::from_millis(200));
            if setup_fw {
                let _ = setup_silent_firewall(&installed_exe);
            }

            prog_pct.store(80, Ordering::SeqCst);
            *status_txt.lock() = "Création des raccourcis et inscription système...".to_string();
            thread::sleep(Duration::from_millis(200));
            let _ = create_desktop_and_start_shortcuts(&installed_exe, desktop_sc, start_sc);
            let _ = register_in_windows_uninstall(&effective_dir, &installed_exe);

            prog_pct.store(100, Ordering::SeqCst);
            *status_txt.lock() = "Finalisation de l'installation (100%)...".to_string();
            thread::sleep(Duration::from_millis(300));
            complete_flag.store(true, Ordering::SeqCst);
        });
    }

    fn start_uninstallation(&mut self) {
        if self.install_started.swap(true, Ordering::SeqCst) {
            return;
        }

        let complete_flag = self.install_complete.clone();
        let prog_pct = self.progress_pct.clone();
        let status_txt = self.status_shared.clone();

        thread::spawn(move || {
            thread::sleep(Duration::from_millis(250));
            prog_pct.store(50, Ordering::SeqCst);
            *status_txt.lock() = "Suppression des raccourcis et nettoyage des registres...".to_string();
            let _ = perform_complete_uninstall();
            prog_pct.store(100, Ordering::SeqCst);
            *status_txt.lock() = "Désinstallation terminée avec succès.".to_string();
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

    let rounding = Rounding::same(8.0);
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

    // Typographie nette, aérée et professionnelle
    style.spacing.item_spacing = egui::vec2(10.0, 10.0);
    style.spacing.button_padding = egui::vec2(14.0, 7.0);

    style.text_styles.insert(
        TextStyle::Heading,
        FontId::new(17.5, FontFamily::Proportional),
    );
    style.text_styles.insert(
        TextStyle::Body,
        FontId::new(13.5, FontFamily::Proportional),
    );
    style.text_styles.insert(
        TextStyle::Button,
        FontId::new(13.5, FontFamily::Proportional),
    );
    style.text_styles.insert(
        TextStyle::Small,
        FontId::new(12.0, FontFamily::Proportional),
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
