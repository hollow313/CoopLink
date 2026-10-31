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
const WIN11_BG: Color32 = Color32::from_rgb(244, 247, 251);       // #F4F7FB
const WIN11_CARD: Color32 = Color32::from_rgb(255, 255, 255);     // #FFFFFF
const WIN11_SUBTLE: Color32 = Color32::from_rgb(248, 250, 252);   // #F8FAFC
const WIN11_BORDER: Color32 = Color32::from_rgb(226, 232, 240);   // #E2E8F0
const WIN11_BLUE: Color32 = Color32::from_rgb(0, 102, 255);       // #0066FF
const WIN11_GREEN: Color32 = Color32::from_rgb(0, 186, 136);      // #00BA88
const WIN11_TEXT_MAIN: Color32 = Color32::from_rgb(15, 23, 42);   // #0F172A
const WIN11_TEXT_SUB: Color32 = Color32::from_rgb(71, 85, 105);   // #475569
const WIN11_TEXT_MUTED: Color32 = Color32::from_rgb(100, 116, 139); // #64748B

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

impl eframe::App for InstallerApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
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
                            RichText::new("Installation de CoopLink")
                                .color(WIN11_TEXT_MAIN)
                                .strong()
                                .size(18.0),
                        );
                        let step_subtitle = match self.step {
                            InstallStep::Welcome => "Étape 1 sur 5 — Bienvenue dans l'assistant d'installation Windows 11",
                            InstallStep::License => "Étape 2 sur 5 — Contrat de licence logicielle",
                            InstallStep::Destination => "Étape 3 sur 5 — Dossier de destination",
                            InstallStep::Options => "Étape 4 sur 5 — Raccourcis et pare-feu Windows",
                            InstallStep::Installing => "Étape 5 sur 5 — Installation des composants en cours...",
                            InstallStep::Finished => "Installation terminée avec succès",
                        };
                        ui.label(
                            RichText::new(step_subtitle)
                                .color(WIN11_TEXT_SUB)
                                .size(13.0),
                        );
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
                    ui.label(
                        RichText::new("CoopLink v1.0.0")
                            .color(WIN11_TEXT_MUTED)
                            .small(),
                    );

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        match self.step {
                            InstallStep::Welcome => {
                                let btn = egui::Button::new(
                                    RichText::new("Suivant")
                                        .color(Color32::WHITE)
                                        .strong(),
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
                                    RichText::new("Suivant")
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

                                if ui
                                    .add_sized([100.0, 34.0], egui::Button::new("Précédent"))
                                    .clicked()
                                {
                                    self.step = InstallStep::Welcome;
                                }
                            }
                            InstallStep::Destination => {
                                let btn = egui::Button::new(
                                    RichText::new("Suivant")
                                        .color(Color32::WHITE)
                                        .strong(),
                                )
                                .fill(WIN11_BLUE)
                                .rounding(Rounding::same(6.0))
                                .min_size(egui::vec2(105.0, 34.0));

                                if ui.add(btn).clicked() {
                                    self.step = InstallStep::Options;
                                }

                                if ui
                                    .add_sized([100.0, 34.0], egui::Button::new("Précédent"))
                                    .clicked()
                                {
                                    self.step = InstallStep::License;
                                }
                            }
                            InstallStep::Options => {
                                let install_btn = egui::Button::new(
                                    RichText::new("Installer")
                                        .color(Color32::WHITE)
                                        .strong(),
                                )
                                .fill(WIN11_BLUE)
                                .rounding(Rounding::same(6.0))
                                .min_size(egui::vec2(120.0, 34.0));

                                if ui.add(install_btn).clicked() {
                                    self.step = InstallStep::Installing;
                                    self.start_installation();
                                }

                                if ui
                                    .add_sized([100.0, 34.0], egui::Button::new("Précédent"))
                                    .clicked()
                                {
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
                                let finish_btn = egui::Button::new(
                                    RichText::new("Terminer")
                                        .color(Color32::WHITE)
                                        .strong(),
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
        ui.vertical(|ui| {
            ui.label(
                RichText::new("Bienvenue dans l'assistant d'installation de CoopLink")
                    .color(WIN11_TEXT_MAIN)
                    .strong()
                    .size(17.0),
            );
            ui.add_space(8.0);

            ui.label(
                RichText::new(
                    "Cet assistant va installer CoopLink sur votre ordinateur. CoopLink permet de jouer en réseau \
                     local virtuel (P2P direct) et de partager votre session graphique et vos manettes à très basse \
                     latence (technologies Sunshine & Moonlight intégrées).",
                )
                .color(WIN11_TEXT_SUB),
            );

            ui.add_space(14.0);

            egui::Frame::none()
                .fill(WIN11_SUBTLE)
                .stroke(Stroke::new(1.0_f32, WIN11_BORDER))
                .inner_margin(14.0)
                .rounding(Rounding::same(8.0))
                .show(ui, |ui| {
                    ui.label(
                        RichText::new("Fonctionnalités installées :")
                            .color(WIN11_BLUE)
                            .strong(),
                    );
                    ui.add_space(4.0);
                    ui.label("• Maillage réseau local virtuel P2P direct (sans transit par serveur)");
                    ui.label("• Module de streaming coopératif Sunshine (Hôte) & Moonlight (Client)");
                    ui.label("• Jumelage automatique Zero-Config (sans code PIN manuel) & manettes ViGEmBus");
                    ui.label("• Ouverture de port UPnP et priorité réseau automatique (métrique basse)");
                });

            ui.add_space(12.0);
            ui.label(
                RichText::new("Cliquez sur « Suivant » pour continuer.")
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
                                     Ce logiciel, son architecture P2P, son interface et sa documentation sont la propriété\n\
                                     exclusive de Loic HUON.\n\n\
                                     2. CONDITIONS D'UTILISATION\n\
                                     Toute reproduction, revente, modification ou ingénierie inverse non autorisée est\n\
                                     strictement interdite.\n\n\
                                     3. COMPOSANTS TIERS\n\
                                     L'orchestration des modules de streaming s'appuie sur les protocoles compatibles\n\
                                     Sunshine et Moonlight.",
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
            ui.label(
                RichText::new("✔ CoopLink a été installé avec succès")
                    .color(WIN11_GREEN)
                    .strong()
                    .size(18.0),
            );
            ui.add_space(10.0);

            ui.label(
                RichText::new(
                    "L'application CoopLink ainsi que les modules de réseau P2P et de streaming coopératif \
                     sont maintenant prêts à être utilisés sur votre ordinateur.",
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
                        "CoopLink - Réseau P2P & Streaming Coopératif",
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
                        "CoopLink - Réseau P2P & Streaming Coopératif",
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
