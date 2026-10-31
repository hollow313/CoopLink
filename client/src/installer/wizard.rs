use super::actions::*;
use eframe::egui;
use egui::{Color32, RichText, Rounding, ScrollArea, Stroke, Ui};
use std::env;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

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
        // En-tête de l'installateur style Dark Carbon 2026
        egui::TopBottomPanel::top("installer_header")
            .frame(
                egui::Frame::none()
                    .fill(Color32::from_rgb(17, 19, 23))
                    .inner_margin(14.0)
                    .stroke(Stroke::new(1.0_f32, Color32::from_rgb(38, 43, 55))),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    if let Some(ref logo) = self.logo_texture {
                        ui.image((logo.id(), egui::vec2(36.0, 36.0)));
                    } else {
                        ui.label(RichText::new("⚡").color(Color32::from_rgb(0, 210, 255)).size(26.0));
                    }
                    ui.vertical(|ui| {
                        ui.horizontal(|ui| {
                            ui.heading(
                                RichText::new("COOPLINK")
                                    .color(Color32::from_rgb(240, 244, 248))
                                    .strong(),
                            );
                            ui.label(
                                RichText::new("// VIRTUAL LAN GAMING ENGINE")
                                    .color(Color32::from_rgb(0, 210, 255))
                                    .monospace()
                                    .small(),
                            );
                        });
                        ui.label(
                            RichText::new("Assistant d'installation officiel Windows 11 — Expérience 100% Graphique")
                                .color(Color32::from_rgb(149, 161, 178))
                                .small(),
                        );
                    });
                });
            });

        // Pied de page avec boutons de navigation
        egui::TopBottomPanel::bottom("installer_footer")
            .frame(
                egui::Frame::none()
                    .fill(Color32::from_rgb(11, 12, 14))
                    .inner_margin(12.0)
                    .stroke(Stroke::new(1.0_f32, Color32::from_rgb(38, 43, 55))),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new("CoopLink v1.0.0 (c) 2026 Loic HUON")
                            .color(Color32::from_rgb(101, 113, 130))
                            .small(),
                    );

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        match self.step {
                            InstallStep::Welcome => {
                                if ui
                                    .add(
                                        egui::Button::new(
                                            RichText::new("Suivant >")
                                                .color(Color32::BLACK)
                                                .strong(),
                                        )
                                        .fill(Color32::from_rgb(0, 210, 255)),
                                    )
                                    .clicked()
                                {
                                    self.step = InstallStep::License;
                                }
                            }
                            InstallStep::License => {
                                let next_btn = egui::Button::new(
                                    RichText::new("Suivant >")
                                        .color(if self.license_accepted {
                                            Color32::BLACK
                                        } else {
                                            Color32::GRAY
                                        })
                                        .strong(),
                                )
                                .fill(if self.license_accepted {
                                    Color32::from_rgb(0, 210, 255)
                                } else {
                                    Color32::from_rgb(23, 26, 33)
                                });

                                if ui.add_enabled(self.license_accepted, next_btn).clicked() {
                                    self.step = InstallStep::Destination;
                                }

                                if ui.button("< Précédent").clicked() {
                                    self.step = InstallStep::Welcome;
                                }
                            }
                            InstallStep::Destination => {
                                if ui
                                    .add(
                                        egui::Button::new(
                                            RichText::new("Suivant >")
                                                .color(Color32::BLACK)
                                                .strong(),
                                        )
                                        .fill(Color32::from_rgb(0, 210, 255)),
                                    )
                                    .clicked()
                                {
                                    self.step = InstallStep::Options;
                                }

                                if ui.button("< Précédent").clicked() {
                                    self.step = InstallStep::License;
                                }
                            }
                            InstallStep::Options => {
                                let install_btn = egui::Button::new(
                                    RichText::new("🚀 Installer maintenant")
                                        .color(Color32::BLACK)
                                        .strong(),
                                )
                                .fill(Color32::from_rgb(0, 229, 153));

                                if ui.add(install_btn).clicked() {
                                    self.step = InstallStep::Installing;
                                    self.start_installation();
                                }

                                if ui.button("< Précédent").clicked() {
                                    self.step = InstallStep::Destination;
                                }
                            }
                            InstallStep::Installing => {
                                ui.label(
                                    RichText::new("Installation en arrière-plan...")
                                        .color(Color32::from_rgb(255, 109, 0)),
                                );
                            }
                            InstallStep::Finished => {
                                if ui
                                    .add(
                                        egui::Button::new(
                                            RichText::new("✔ Terminer")
                                                .color(Color32::BLACK)
                                                .strong(),
                                        )
                                        .fill(Color32::from_rgb(0, 229, 153)),
                                    )
                                    .clicked()
                                {
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

        // Contenu central de chaque étape (Dark Carbon 2026)
        egui::CentralPanel::default()
            .frame(
                egui::Frame::none()
                    .fill(Color32::from_rgb(11, 12, 14))
                    .inner_margin(20.0),
            )
            .show(ctx, |ui| match self.step {
                InstallStep::Welcome => self.render_welcome(ui),
                InstallStep::License => self.render_license(ui),
                InstallStep::Destination => self.render_destination(ui),
                InstallStep::Options => self.render_options(ui),
                InstallStep::Installing => self.render_installing(ui),
                InstallStep::Finished => self.render_finished(ui),
            });

        // Vérification de la fin de l'installation
        if self.step == InstallStep::Installing && self.install_complete.load(Ordering::Relaxed) {
            self.step = InstallStep::Finished;
        }

        ctx.request_repaint_after(Duration::from_millis(100));
    }
}

impl InstallerApp {
    fn render_welcome(&self, ui: &mut Ui) {
        ui.vertical(|ui| {
            ui.add_space(8.0);
            ui.heading(
                RichText::new("🎮 Bienvenue dans l'installation de CoopLink")
                    .color(Color32::WHITE)
                    .strong(),
            );
            ui.add_space(10.0);

            ui.label(
                RichText::new(
                    "CoopLink est un moteur haute performance de réseau local virtuel (LAN) conçu \
                     pour jouer en coopération avec vos amis sur vos jeux vidéo favoris, exactement \
                     comme si vous étiez connectés sur la même box internet.",
                )
                .color(Color32::from_rgb(240, 244, 248)),
            );

            ui.add_space(15.0);

            egui::Frame::none()
                .fill(Color32::from_rgb(23, 26, 33))
                .stroke(Stroke::new(1.0_f32, Color32::from_rgb(38, 43, 55)))
                .inner_margin(14.0)
                .rounding(Rounding::same(7.0))
                .show(ui, |ui| {
                    ui.label(RichText::new("✨ Fonctionnalités incluses :").color(Color32::from_rgb(0, 210, 255)).strong());
                    ui.add_space(6.0);
                    ui.label("✔ Ouverture de port automatique UPnP (aucune configuration de box nécessaire)");
                    ui.label("✔ Métrique réseau basse prioritaire (passe avant votre Wi-Fi pour les jeux)");
                    ui.label("✔ Découverte LAN universelle (Blizzard, Valve Source, Unreal Engine, Minecraft...)");
                    ui.label("✔ Multi-joueurs (2 clients ou plus connectés ensemble sur le même salon)");
                });

            ui.add_space(15.0);
            ui.label(
                RichText::new("Cliquez sur 'Suivant' pour continuer l'installation.")
                    .color(Color32::from_rgb(149, 161, 178)),
            );
        });
    }

    fn render_license(&mut self, ui: &mut Ui) {
        ui.vertical(|ui| {
            ui.heading(
                RichText::new("📜 Contrat de Licence Logicielle")
                    .color(Color32::from_rgb(240, 244, 248))
                    .strong(),
            );
            ui.label(
                RichText::new("Veuillez lire attentivement le contrat de licence propriétaire avant de continuer.")
                    .color(Color32::from_rgb(149, 161, 178)),
            );
            ui.add_space(10.0);

            ScrollArea::vertical()
                .max_height(230.0)
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    egui::Frame::none()
                        .fill(Color32::from_rgb(14, 16, 20))
                        .stroke(Stroke::new(1.0_f32, Color32::from_rgb(38, 43, 55)))
                        .inner_margin(12.0)
                        .rounding(Rounding::same(6.0))
                        .show(ui, |ui| {
                            ui.label(
                                RichText::new(
                                    "COOPLINK - PROPRIETARY COMMERCIAL SOFTWARE LICENSE\n\
                                     Copyright (c) 2026 Loic HUON. All Rights Reserved.\n\n\
                                     1. PROPRIETARY RIGHTS NOTICE\n\
                                     This software, binary distributions, designs, and associated assets\n\
                                     are the confidential and proprietary property of Loic HUON.\n\n\
                                     2. RESTRICTIONS\n\
                                     Except as expressly authorized, no person or entity may copy, redistribute,\n\
                                     sell, decompile, disassemble or reverse engineer CoopLink.\n\n\
                                     3. COMMERCIAL EXCLUSIVITY\n\
                                     This Software is strictly intended for proprietary exploitation.\n\
                                     No open source license is granted.",
                                )
                                .monospace()
                                .color(Color32::from_rgb(200, 210, 225))
                                .small(),
                            );
                        });
                });

            ui.add_space(12.0);
            ui.checkbox(
                &mut self.license_accepted,
                RichText::new("J'ai lu et j'accepte les termes du contrat de licence")
                    .color(Color32::from_rgb(0, 229, 153))
                    .strong(),
            );
        });
    }

    fn render_destination(&mut self, ui: &mut Ui) {
        ui.vertical(|ui| {
            ui.heading(
                RichText::new("📁 Dossier de destination")
                    .color(Color32::from_rgb(240, 244, 248))
                    .strong(),
            );
            ui.label(
                RichText::new("Choisissez l'emplacement sur votre disque où CoopLink sera installé :")
                    .color(Color32::from_rgb(149, 161, 178)),
            );
            ui.add_space(15.0);

            ui.horizontal(|ui| {
                ui.label(RichText::new("Dossier :").color(Color32::from_rgb(240, 244, 248)).strong());
                ui.add(egui::TextEdit::singleline(&mut self.install_dir).desired_width(400.0));
            });

            ui.add_space(15.0);
            egui::Frame::none()
                .fill(Color32::from_rgb(23, 26, 33))
                .stroke(Stroke::new(1.0_f32, Color32::from_rgb(38, 43, 55)))
                .inner_margin(12.0)
                .rounding(Rounding::same(6.0))
                .show(ui, |ui| {
                    ui.label("Espace disque requis : ~15 Mo");
                    ui.label("Espace disque disponible : Plus de 10 Go");
                });
        });
    }

    fn render_options(&mut self, ui: &mut Ui) {
        ui.vertical(|ui| {
            ui.heading(
                RichText::new("⚙ Options d'installation et de raccourcis")
                    .color(Color32::from_rgb(240, 244, 248))
                    .strong(),
            );
            ui.label(
                RichText::new("Sélectionnez les tâches à exécuter lors de l'installation :")
                    .color(Color32::from_rgb(149, 161, 178)),
            );
            ui.add_space(15.0);

            ui.checkbox(
                &mut self.create_desktop_shortcut,
                RichText::new("Créer une icône sur le Bureau Windows").color(Color32::from_rgb(240, 244, 248)),
            );
            ui.add_space(8.0);

            ui.checkbox(
                &mut self.create_start_menu_shortcut,
                RichText::new("Créer un raccourci dans le Menu Démarrer").color(Color32::from_rgb(240, 244, 248)),
            );
            ui.add_space(8.0);

            ui.checkbox(
                &mut self.configure_firewall,
                RichText::new("Autoriser automatiquement CoopLink dans le Pare-feu Windows Defender")
                    .color(Color32::from_rgb(0, 229, 153))
                    .strong(),
            );

            ui.add_space(15.0);
            ui.label(
                RichText::new("Tout est prêt ! Cliquez sur 'Installer maintenant' pour lancer la procédure.")
                    .color(Color32::from_rgb(149, 161, 178)),
            );
        });
    }

    fn render_installing(&self, ui: &mut Ui) {
        ui.vertical_centered(|ui| {
            ui.add_space(35.0);
            ui.heading(
                RichText::new("📦 Installation de CoopLink en cours...")
                    .color(Color32::from_rgb(240, 244, 248))
                    .strong(),
            );
            ui.add_space(20.0);

            let progress_bar = egui::ProgressBar::new(self.progress)
                .show_percentage()
                .animate(true)
                .fill(Color32::from_rgb(0, 229, 153));

            ui.add_sized([ui.available_width() - 40.0, 26.0], progress_bar);

            ui.add_space(15.0);
            ui.label(
                RichText::new(&self.status_label)
                    .color(Color32::from_rgb(0, 210, 255))
                    .monospace(),
            );

            if let Some(ref err) = self.error_msg {
                ui.add_space(10.0);
                ui.label(RichText::new(format!("Erreur: {}", err)).color(Color32::from_rgb(255, 68, 68)));
            }
        });
    }

    fn render_finished(&mut self, ui: &mut Ui) {
        ui.vertical(|ui| {
            ui.add_space(15.0);
            ui.heading(
                RichText::new("🎉 Installation de CoopLink terminée avec succès !")
                    .color(Color32::from_rgb(0, 229, 153))
                    .strong(),
            );
            ui.add_space(15.0);

            ui.label(
                RichText::new(
                    "Le moteur de réseau local virtuel CoopLink est maintenant installé et configuré \
                     sur votre système. Vos amis peuvent dès maintenant vous rejoindre pour jouer en LAN.",
                )
                .color(Color32::from_rgb(240, 244, 248)),
            );

            ui.add_space(20.0);
            ui.checkbox(
                &mut self.launch_on_finish,
                RichText::new("Lancer CoopLink immédiatement")
                    .color(Color32::from_rgb(0, 210, 255))
                    .strong(),
            );

            ui.add_space(15.0);
            ui.label(
                RichText::new("Cliquez sur 'Terminer' pour fermer l'assistant.")
                    .color(Color32::from_rgb(149, 161, 178)),
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
            // Étape 1 : Création du dossier cible
            let _ = fs::create_dir_all(&target_dir);
            thread::sleep(Duration::from_millis(300));

            // Étape 2 : Copie de l'exécutable cooplink.exe
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
            thread::sleep(Duration::from_millis(300));

            // Étape 3 : Création du raccourci Bureau si demandé (100% silencieux sans fenêtre CMD)
            if create_desktop {
                if let Ok(user_profile) = env::var("USERPROFILE") {
                    let desktop_lnk = PathBuf::from(user_profile)
                        .join("Desktop")
                        .join("CoopLink.lnk");
                    let _ = create_windows_shortcut(
                        &target_exe,
                        &desktop_lnk,
                        "CoopLink - Virtual LAN Gaming Engine",
                    );
                }
            }
            thread::sleep(Duration::from_millis(200));

            // Étape 4 : Création du raccourci Menu Démarrer (100% silencieux sans fenêtre CMD)
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
                        "CoopLink - Virtual LAN Gaming Engine",
                    );
                }
            }
            thread::sleep(Duration::from_millis(200));

            // Étape 5 : Configuration pare-feu silencieuse (sans aucune console CMD)
            if setup_firewall {
                let _ = setup_silent_firewall(&target_exe);
            }
            thread::sleep(Duration::from_millis(200));

            // Étape 6 : Enregistrement dans le Registre Windows
            let _ = register_in_windows_uninstall(&target_dir, &target_exe);
            thread::sleep(Duration::from_millis(200));

            complete_flag.store(true, Ordering::SeqCst);
        });

        self.progress = 1.0;
        self.status_label = "Installation terminée avec succès !".to_string();
    }
}

fn setup_installer_theme(ctx: &egui::Context) {
    let mut style = (*ctx.style()).clone();
    let mut visuals = egui::Visuals::dark();
    visuals.panel_fill = Color32::from_rgb(11, 12, 14); // #0B0C0E Dark Carbon
    visuals.window_fill = Color32::from_rgb(17, 19, 23); // #111317
    visuals.faint_bg_color = Color32::from_rgb(23, 26, 33); // #171A21
    visuals.extreme_bg_color = Color32::from_rgb(8, 9, 11);
    style.visuals = visuals;
    ctx.set_style(style);
}

fn load_installer_logo(ctx: &egui::Context) -> Option<egui::TextureHandle> {
    let image_bytes = include_bytes!("../../../images/logo.jpg");
    if let Ok(dynamic_img) = image::load_from_memory(image_bytes) {
        let rgba = dynamic_img.to_rgba8();
        let size = [rgba.width() as usize, rgba.height() as usize];
        let color_image = egui::ColorImage::from_rgba_unmultiplied(size, &rgba.into_raw());
        Some(ctx.load_texture("installer_logo", color_image, egui::TextureOptions::LINEAR))
    } else {
        None
    }
}
