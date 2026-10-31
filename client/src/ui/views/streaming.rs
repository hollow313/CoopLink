use crate::streaming::moonlight_client::{StreamDisplayMode, VideoCodec};
use crate::streaming::pairing::PairingStatus;
use crate::streaming::sunshine_host::{HardwareEncoder, VirtualGamepadType};
use crate::streaming::StreamingEngine;
use crate::types::AppState;
use crate::ui::retro_theme::ModernTheme;
use egui::{Color32, RichText, Rounding, Stroke, Ui};
use std::net::Ipv4Addr;

pub struct StreamingView;

impl StreamingView {
    pub fn render(
        ui: &mut Ui,
        state: &AppState,
        engine: &mut StreamingEngine,
        on_start_sunshine_host: &mut bool,
        on_stop_sunshine_host: &mut bool,
        on_start_moonlight_client: &mut bool,
        on_stop_moonlight_client: &mut bool,
    ) {
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                // 1. En-tête Zero-Config Pairing & P2P Direct
                egui::Frame::none()
                    .fill(ModernTheme::CARD_BG)
                    .stroke(Stroke::new(1.0_f32, ModernTheme::BORDER_SUBTLE))
                    .inner_margin(16.0)
                    .rounding(Rounding::same(10.0))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            let badge_color = match engine.pairing.status {
                                PairingStatus::Paired { .. } => ModernTheme::GREEN_VIVID,
                                PairingStatus::ExchangingCertificates | PairingStatus::ValidatingPinOnHost => {
                                    ModernTheme::ORANGE_CORAL
                                }
                                PairingStatus::Idle => ModernTheme::BLUE_ELECTRIC,
                                PairingStatus::Failed(_) => ModernTheme::RED_ERROR,
                            };

                            ui.label(RichText::new("●").color(badge_color).size(18.0));
                            ui.vertical(|ui| {
                                ui.label(
                                    RichText::new("Streaming Coopératif Basse Latence & Partage de Périphériques (Sunshine + Moonlight)")
                                        .color(ModernTheme::TEXT_PRIMARY)
                                        .strong()
                                        .size(15.5),
                                );
                                let pair_desc = match &engine.pairing.status {
                                    PairingStatus::Idle => {
                                        "Jumelage Automatisé (Zero-Config) actif : aucun code PIN à 4 chiffres ni copie d'IP requis. Flux 100% P2P direct."
                                            .to_string()
                                    }
                                    PairingStatus::ExchangingCertificates => {
                                        "Échange automatique des certificats TLS sur le canal chiffré CoopLink...".to_string()
                                    }
                                    PairingStatus::ValidatingPinOnHost => {
                                        "Validation automatique du jeton sur l'API locale Sunshine...".to_string()
                                    }
                                    PairingStatus::Paired {
                                        host_virtual_ip,
                                        cert_fingerprint,
                                        ..
                                    } => format!(
                                        "Jumelé automatiquement avec l'hôte {} ({}) — Latence décodage < 5 ms",
                                        host_virtual_ip, cert_fingerprint
                                    ),
                                    PairingStatus::Failed(err) => format!("Erreur de jumelage : {}", err),
                                };
                                ui.label(RichText::new(pair_desc).color(ModernTheme::TEXT_SECONDARY).small());
                            });
                        });
                    });

                ui.add_space(12.0);

                // 2. Deux colonnes : Architecture Hôte (Sunshine) & Architecture Client (Moonlight)
                ui.columns(2, |columns| {
                    // --- COLONNE 1 : ARCHITECTURE HÔTE (SUNSHINE) ---
                    egui::Frame::none()
                        .fill(ModernTheme::CARD_BG)
                        .stroke(Stroke::new(
                            1.5_f32,
                            if engine.sunshine_host.is_running {
                                ModernTheme::GREEN_VIVID
                            } else {
                                ModernTheme::BORDER_SUBTLE
                            },
                        ))
                        .inner_margin(16.0)
                        .rounding(Rounding::same(10.0))
                        .show(&mut columns[0], |ui| {
                            ui.horizontal(|ui| {
                                ui.label(
                                    RichText::new("Hôte — Serveur Sunshine")
                                        .color(ModernTheme::BLUE_ELECTRIC)
                                        .strong()
                                        .size(16.0),
                                );
                                if engine.sunshine_host.is_running {
                                    ui.label(
                                        RichText::new("● ACTIF")
                                            .color(ModernTheme::GREEN_VIVID)
                                            .strong()
                                            .small(),
                                    );
                                }
                            });
                            ui.label(
                                RichText::new(
                                    "Capture matérielle directe et écoute strictement liée à l'interface virtuelle CoopLink.",
                                )
                                .color(ModernTheme::TEXT_SECONDARY)
                                .small(),
                            );
                            ui.add_space(10.0);

                            let vip = state.virtual_ip.unwrap_or(Ipv4Addr::new(10, 244, 0, 1));
                            egui::Grid::new("sunshine_host_grid")
                                .num_columns(2)
                                .spacing([12.0, 8.0])
                                .show(ui, |ui| {
                                    ui.label(RichText::new("Interface liée :").color(ModernTheme::TEXT_SECONDARY));
                                    ui.label(
                                        RichText::new(format!("{} (Exclusif CoopLink)", vip))
                                            .color(ModernTheme::GREEN_VIVID)
                                            .strong(),
                                    );
                                    ui.end_row();

                                    ui.label(RichText::new("GPU détecté :").color(ModernTheme::TEXT_SECONDARY));
                                    ui.label(
                                        RichText::new(&engine.sunshine_host.detected_gpu_name)
                                            .color(ModernTheme::TEXT_PRIMARY)
                                            .strong(),
                                    );
                                    ui.end_row();

                                    ui.label(RichText::new("Encodeur vidéo :").color(ModernTheme::TEXT_SECONDARY));
                                    egui::ComboBox::from_id_salt("host_encoder_combo")
                                        .selected_text(engine.sunshine_host.active_encoder.display_name())
                                        .show_ui(ui, |ui| {
                                            ui.selectable_value(
                                                &mut engine.sunshine_host.active_encoder,
                                                HardwareEncoder::Nvenc,
                                                HardwareEncoder::Nvenc.display_name(),
                                            );
                                            ui.selectable_value(
                                                &mut engine.sunshine_host.active_encoder,
                                                HardwareEncoder::Amf,
                                                HardwareEncoder::Amf.display_name(),
                                            );
                                            ui.selectable_value(
                                                &mut engine.sunshine_host.active_encoder,
                                                HardwareEncoder::QuickSync,
                                                HardwareEncoder::QuickSync.display_name(),
                                            );
                                            ui.selectable_value(
                                                &mut engine.sunshine_host.active_encoder,
                                                HardwareEncoder::Software,
                                                HardwareEncoder::Software.display_name(),
                                            );
                                        });
                                    ui.end_row();

                                    ui.label(RichText::new("Cadence & Flux :").color(ModernTheme::TEXT_SECONDARY));
                                    ui.horizontal(|ui| {
                                        ui.selectable_value(&mut engine.sunshine_host.target_fps, 60, "1080p @ 60 FPS");
                                        ui.selectable_value(&mut engine.sunshine_host.target_fps, 120, "1440p @ 120 FPS");
                                    });
                                    ui.end_row();

                                    ui.label(RichText::new("Émulation ViGEmBus :").color(ModernTheme::TEXT_SECONDARY));
                                    egui::ComboBox::from_id_salt("vigem_type_combo")
                                        .selected_text(engine.sunshine_host.default_gamepad_type.label())
                                        .show_ui(ui, |ui| {
                                            ui.selectable_value(
                                                &mut engine.sunshine_host.default_gamepad_type,
                                                VirtualGamepadType::Xbox360,
                                                VirtualGamepadType::Xbox360.label(),
                                            );
                                            ui.selectable_value(
                                                &mut engine.sunshine_host.default_gamepad_type,
                                                VirtualGamepadType::DualShock4,
                                                VirtualGamepadType::DualShock4.label(),
                                            );
                                        });
                                    ui.end_row();
                                });

                            ui.add_space(12.0);
                            if !engine.sunshine_host.is_running {
                                let btn = egui::Button::new(
                                    RichText::new("Activer l'hôte de streaming Sunshine")
                                        .color(Color32::WHITE)
                                        .strong(),
                                )
                                .fill(ModernTheme::BLUE_ELECTRIC)
                                .rounding(Rounding::same(8.0))
                                .min_size(egui::vec2(ui.available_width(), 36.0));

                                if ui.add(btn).clicked() {
                                    *on_start_sunshine_host = true;
                                }
                            } else {
                                let btn = egui::Button::new(
                                    RichText::new("Arrêter le partage Sunshine")
                                        .color(Color32::WHITE)
                                        .strong(),
                                )
                                .fill(ModernTheme::ORANGE_CORAL)
                                .rounding(Rounding::same(8.0))
                                .min_size(egui::vec2(ui.available_width(), 36.0));

                                if ui.add(btn).clicked() {
                                    *on_stop_sunshine_host = true;
                                }
                            }
                        });

                    // --- COLONNE 2 : ARCHITECTURE CLIENT (MOONLIGHT) ---
                    egui::Frame::none()
                        .fill(ModernTheme::CARD_BG)
                        .stroke(Stroke::new(
                            1.5_f32,
                            if engine.moonlight_client.is_streaming {
                                ModernTheme::INDIGO_ACCENT
                            } else {
                                ModernTheme::BORDER_SUBTLE
                            },
                        ))
                        .inner_margin(16.0)
                        .rounding(Rounding::same(10.0))
                        .show(&mut columns[1], |ui| {
                            ui.horizontal(|ui| {
                                ui.label(
                                    RichText::new("Client — Vue Moonlight")
                                        .color(ModernTheme::INDIGO_ACCENT)
                                        .strong()
                                        .size(16.0),
                                );
                                if engine.moonlight_client.is_streaming {
                                    ui.label(
                                        RichText::new(format!(
                                            "● EN DIRECT ({:.1} ms)",
                                            engine.moonlight_client.estimated_decode_latency_ms
                                        ))
                                        .color(ModernTheme::GREEN_VIVID)
                                        .strong()
                                        .small(),
                                    );
                                }
                            });
                            ui.label(
                                RichText::new(
                                    "Décodage matériel obligatoire (< 5 ms) et transfert ultra-faible latence des manettes.",
                                )
                                .color(ModernTheme::TEXT_SECONDARY)
                                .small(),
                            );
                            ui.add_space(10.0);

                            egui::Grid::new("moonlight_client_grid")
                                .num_columns(2)
                                .spacing([12.0, 8.0])
                                .show(ui, |ui| {
                                    ui.label(RichText::new("Hôte détecté :").color(ModernTheme::TEXT_SECONDARY));
                                    ui.label(
                                        RichText::new(format!("{} (Auto-découvert)", engine.moonlight_client.target_host_vip))
                                            .color(ModernTheme::BLUE_ELECTRIC)
                                            .strong(),
                                    );
                                    ui.end_row();

                                    ui.label(RichText::new("Décodage matériel :").color(ModernTheme::TEXT_SECONDARY));
                                    egui::ComboBox::from_id_salt("client_codec_combo")
                                        .selected_text(engine.moonlight_client.preferred_codec.label())
                                        .show_ui(ui, |ui| {
                                            ui.selectable_value(
                                                &mut engine.moonlight_client.preferred_codec,
                                                VideoCodec::H264,
                                                VideoCodec::H264.label(),
                                            );
                                            ui.selectable_value(
                                                &mut engine.moonlight_client.preferred_codec,
                                                VideoCodec::Hevc,
                                                VideoCodec::Hevc.label(),
                                            );
                                            ui.selectable_value(
                                                &mut engine.moonlight_client.preferred_codec,
                                                VideoCodec::Av1,
                                                VideoCodec::Av1.label(),
                                            );
                                        });
                                    ui.end_row();

                                    ui.label(RichText::new("Affichage :").color(ModernTheme::TEXT_SECONDARY));
                                    egui::ComboBox::from_id_salt("client_display_combo")
                                        .selected_text(engine.moonlight_client.display_mode.label())
                                        .show_ui(ui, |ui| {
                                            ui.selectable_value(
                                                &mut engine.moonlight_client.display_mode,
                                                StreamDisplayMode::BorderlessFullscreen,
                                                StreamDisplayMode::BorderlessFullscreen.label(),
                                            );
                                            ui.selectable_value(
                                                &mut engine.moonlight_client.display_mode,
                                                StreamDisplayMode::EmbeddedWindowed,
                                                StreamDisplayMode::EmbeddedWindowed.label(),
                                            );
                                        });
                                    ui.end_row();
                                });

                            ui.add_space(6.0);
                            ui.checkbox(
                                &mut engine.moonlight_client.capture_gamepads_xinput_sdl,
                                RichText::new("Capturer les manettes locales (XInput / DirectInput / SDL2)")
                                    .color(ModernTheme::TEXT_PRIMARY),
                            );
                            ui.checkbox(
                                &mut engine.moonlight_client.capture_mouse_keyboard,
                                RichText::new("Capturer aussi la souris et le clavier (Navigation / Assistance)")
                                    .color(ModernTheme::TEXT_PRIMARY),
                            );

                            ui.add_space(10.0);
                            if !engine.moonlight_client.is_streaming {
                                let btn = egui::Button::new(
                                    RichText::new("Rejoindre la session graphique (Zero-Config)")
                                        .color(Color32::WHITE)
                                        .strong(),
                                )
                                .fill(ModernTheme::INDIGO_ACCENT)
                                .rounding(Rounding::same(8.0))
                                .min_size(egui::vec2(ui.available_width(), 36.0));

                                if ui.add(btn).clicked() {
                                    *on_start_moonlight_client = true;
                                }
                            } else {
                                let btn = egui::Button::new(
                                    RichText::new("Quitter le flux Moonlight")
                                        .color(Color32::WHITE)
                                        .strong(),
                                )
                                .fill(ModernTheme::ORANGE_CORAL)
                                .rounding(Rounding::same(8.0))
                                .min_size(egui::vec2(ui.available_width(), 36.0));

                                if ui.add(btn).clicked() {
                                    *on_stop_moonlight_client = true;
                                }
                            }
                        });
                });

                ui.add_space(12.0);

                // 3. Tableau d'assignation automatique des manettes (ViGEmBus)
                egui::Frame::none()
                    .fill(ModernTheme::CARD_BG)
                    .stroke(Stroke::new(1.0_f32, ModernTheme::BORDER_SUBTLE))
                    .inner_margin(16.0)
                    .rounding(Rounding::same(10.0))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new("Assignation Automatique des Manettes & Périphériques (ViGEmBus)")
                                    .color(ModernTheme::TEXT_PRIMARY)
                                    .strong()
                                    .size(15.5),
                            );
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                if ui.button("+ Simuler l'ajout d'une manette distante").clicked() {
                                    let next_num = (engine.sunshine_host.controller_slots.len() as u8) + 1;
                                    let peer_ip = Ipv4Addr::new(10, 244, 0, next_num);
                                    engine.sunshine_host.assign_remote_controller_slot(
                                        &format!("Ami Distant #{}", next_num - 1),
                                        peer_ip,
                                    );
                                }
                            });
                        });
                        ui.label(
                            RichText::new(
                                "Séparation transparente des contrôleurs locaux (Joueur 1) et distants (Joueur 2, 3, 4) pour éviter tout conflit dans les jeux coopératifs.",
                            )
                            .color(ModernTheme::TEXT_SECONDARY)
                            .small(),
                        );
                        ui.add_space(8.0);

                        egui::Grid::new("vigembus_slots_grid")
                            .num_columns(5)
                            .spacing([24.0, 8.0])
                            .striped(true)
                            .show(ui, |ui| {
                                ui.label(RichText::new("JOUEUR").color(ModernTheme::BLUE_ELECTRIC).strong());
                                ui.label(RichText::new("UTILISATEUR").color(ModernTheme::BLUE_ELECTRIC).strong());
                                ui.label(RichText::new("IP VIRTUELLE").color(ModernTheme::BLUE_ELECTRIC).strong());
                                ui.label(RichText::new("PÉRIPHÉRIQUE ÉMULÉ").color(ModernTheme::BLUE_ELECTRIC).strong());
                                ui.label(RichText::new("SLOT XINPUT").color(ModernTheme::BLUE_ELECTRIC).strong());
                                ui.end_row();

                                for slot in &engine.sunshine_host.controller_slots {
                                    let badge_color = if slot.is_local_host {
                                        ModernTheme::BLUE_ELECTRIC
                                    } else {
                                        ModernTheme::GREEN_VIVID
                                    };
                                    ui.label(
                                        RichText::new(format!("Joueur {}", slot.player_number))
                                            .color(badge_color)
                                            .strong(),
                                    );
                                    ui.label(RichText::new(&slot.owner_name).color(ModernTheme::TEXT_PRIMARY));
                                    ui.label(RichText::new(format!("{}", slot.virtual_ip)).color(ModernTheme::TEXT_SECONDARY));
                                    ui.label(RichText::new(slot.emulated_type.label()).color(ModernTheme::TEXT_PRIMARY));
                                    ui.label(
                                        RichText::new(format!("XInput #{} (Prêt)", slot.xinput_slot_index))
                                            .color(ModernTheme::GREEN_VIVID)
                                            .strong(),
                                    );
                                    ui.end_row();
                                }
                            });
                    });
            });
    }
}
