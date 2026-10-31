pub mod logo;
pub mod retro_theme;
pub mod views;

use crate::adapter::NativeAdapter;
use crate::config::AppConfig;
use crate::i18n::{I18n, Language};
use crate::network::broadcast_relay::BroadcastRelay;
use crate::network::telemetry::CentralTelemetryClient;
use crate::network::tunnel::NetworkTunnel;
use crate::network::upnp::{UpnpManager, UpnpState};
use crate::platform::{
    check_os_firewall_status, get_os_network_interfaces, set_os_adapter_metric,
    setup_os_firewall_rules, InterfaceMetricInfo,
};
use crate::streaming::StreamingEngine;
use crate::types::{AppState, ConnectionStatus, LogEntry, LogLevel, NodeRole};
use chrono::Local;
use eframe::egui;
use egui::{Color32, CursorIcon, RichText, TextureHandle};
use logo::load_embedded_logo;
use parking_lot::Mutex;
use retro_theme::ModernTheme;
use std::net::{Ipv4Addr, SocketAddr};
use std::sync::atomic::Ordering;
use std::sync::Arc;
use tokio::runtime::Runtime;
use tokio::sync::mpsc;
use views::console::ConsoleView;
use views::firewall::FirewallView;
use views::lobby::LobbyView;
use views::network::NetworkView;
use views::settings::SettingsView;
use views::speedtest::{SpeedtestManager, SpeedtestView};
use views::streaming::StreamingView;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActiveTab {
    Lobby,
    Streaming,
    Speedtest,
    Network,
    Firewall,
    Settings,
    Console,
}

pub struct CoopLinkApp {
    pub config: AppConfig,
    pub state: AppState,
    pub active_tab: ActiveTab,
    pub logs: Vec<LogEntry>,
    pub saved_notice: bool,
    pub rt: Arc<Runtime>,
    pub tunnel: Arc<NetworkTunnel>,
    pub broadcast_relay: Arc<BroadcastRelay>,
    pub upnp_mgr: Arc<Mutex<UpnpManager>>,
    pub telemetry: Arc<CentralTelemetryClient>,
    pub streaming_engine: StreamingEngine,
    pub speedtest_mgr: SpeedtestManager,
    pub adapter: NativeAdapter,
    pub to_adapter_tx: mpsc::Sender<Vec<u8>>,
    pub from_net_rx: Option<mpsc::Receiver<Vec<u8>>>,
    pub to_bcast_tx: mpsc::Sender<(u16, Vec<u8>)>,
    pub from_bcast_rx: Option<mpsc::Receiver<(u16, Vec<u8>)>>,
    pub to_tunnel_bcast_tx: mpsc::Sender<(u16, Vec<u8>)>,
    pub from_tunnel_bcast_rx: Option<mpsc::Receiver<(u16, Vec<u8>)>>,
    pub logo_texture: Option<TextureHandle>,
    pub interfaces_rx: Option<crossbeam_channel::Receiver<Vec<InterfaceMetricInfo>>>,
    pub show_close_prompt_modal: bool,
    pub remember_close_choice: bool,
}

impl CoopLinkApp {
    pub fn new(cc: &eframe::CreationContext<'_>, rt: Arc<Runtime>) -> Self {
        ModernTheme::apply(&cc.egui_ctx);
        let config = AppConfig::load();
        let mut state = AppState::default();

        let (adapter, to_adapter_tx, from_net_rx) = NativeAdapter::new();
        let (to_bcast_tx, from_bcast_rx) = mpsc::channel(1000);
        let (to_tunnel_bcast_tx, from_tunnel_bcast_rx) = mpsc::channel(1000);

        let tunnel = Arc::new(NetworkTunnel::new());
        let broadcast_relay = Arc::new(BroadcastRelay::new());
        let upnp_mgr = Arc::new(Mutex::new(UpnpManager::new()));
        let streaming_engine = StreamingEngine::new(config.host_virtual_ip);
        let speedtest_mgr = SpeedtestManager::new();

        let telemetry = CentralTelemetryClient::new(
            config.instance_uuid,
            config.player_name.clone(),
            config.central_server_address.clone(),
        );
        telemetry.update_state(
            &config.player_name,
            false,
            false,
            &state.room_code,
            0,
            false,
            &config.central_server_address,
            &config.remote_server_address,
        );
        telemetry.start_background_loop(&rt);

        state.firewall_ok = check_os_firewall_status();
        let logo_texture = load_embedded_logo(&cc.egui_ctx);

        let mut app = Self {
            config,
            state,
            active_tab: ActiveTab::Lobby,
            logs: Vec::new(),
            saved_notice: false,
            rt,
            tunnel,
            broadcast_relay,
            upnp_mgr,
            telemetry,
            streaming_engine,
            speedtest_mgr,
            adapter,
            to_adapter_tx,
            from_net_rx: Some(from_net_rx),
            to_bcast_tx,
            from_bcast_rx: Some(from_bcast_rx),
            to_tunnel_bcast_tx,
            from_tunnel_bcast_rx: Some(from_tunnel_bcast_rx),
            logo_texture,
            interfaces_rx: None,
            show_close_prompt_modal: false,
            remember_close_choice: true,
        };

        app.trigger_refresh_interfaces();
        app.add_log(
            LogLevel::Success,
            "CoopLink prêt (Tunnel P2P Chiffré E2E & Partage de Jeu au Premier Plan actifs).",
        );

        // Application automatique des règles de pare-feu dès le premier lancement (Windows / macOS / Linux)
        if app.config.auto_firewall_rule {
            let port = app.config.listen_port;
            let subnet = format!("{}.0/24", app.config.virtual_subnet_prefix);
            let _ = setup_os_firewall_rules(port, &subnet);
            app.state.firewall_ok = true;
            app.add_log(
                LogLevel::Info,
                "Règles de pare-feu système (UDP P2P & Sous-réseau virtuel) appliquées dès le lancement.",
            );
        }

        // Application automatique de la métrique prioritaire dès le premier lancement
        if app.config.auto_metric_optimization {
            let target_metric = app.config.target_metric;
            let adapter_name = app.state.adapter_name.clone();
            let _ = set_os_adapter_metric(&adapter_name, target_metric);
            app.state.metric_applied = true;
            app.add_log(
                LogLevel::Info,
                &format!(
                    "Métrique prioritaire ({}) appliquée à l'interface virtuelle.",
                    target_metric
                ),
            );
        }

        app
    }

    pub fn add_log(&mut self, level: LogLevel, message: &str) {
        let timestamp = Local::now().format("%H:%M:%S").to_string();
        self.logs.push(LogEntry {
            timestamp,
            level,
            message: message.to_string(),
        });
        if self.logs.len() > 300 {
            self.logs.remove(0);
        }
    }

    fn sync_telemetry(&self) {
        let tunnel_mounted = matches!(
            self.state.status,
            ConnectionStatus::Hosting | ConnectionStatus::Connected
        );
        let is_host = self.state.role == NodeRole::Host;
        let screen_sharing =
            self.streaming_engine.capture_host.is_running || self.streaming_engine.stream_viewer.is_streaming;
        self.telemetry.update_state(
            &self.config.player_name,
            tunnel_mounted,
            is_host,
            &self.state.room_code,
            self.state.peers.len(),
            screen_sharing,
            &self.config.central_server_address,
            &self.config.remote_server_address,
        );
    }
}

impl eframe::App for CoopLinkApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Interception de la fermeture par la croix (X) -> Demande la 1re fois, puis applique le choix mémorisé
        if ctx.input(|i| i.viewport().close_requested()) {
            if !self.config.close_behavior_asked {
                ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
                self.show_close_prompt_modal = true;
            } else if self.config.close_to_background {
                ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
                ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true));
                self.add_log(
                    LogLevel::Info,
                    "CoopLink réduit en arrière-plan (votre tunnel P2P et le partage de jeu continuent).",
                );
            } else {
                self.disconnect_node();
            }
        }

        if self.show_close_prompt_modal {
            let lang = self.config.language;
            egui::Window::new(match lang {
                Language::Fr => "Fermeture de CoopLink — Que souhaitez-vous faire ?",
                Language::En => "Closing CoopLink — What would you like to do?",
                Language::Es => "Cerrando CoopLink — ¿Qué deseas hacer?",
                Language::De => "CoopLink schließen — Was möchtest du tun?",
                Language::Pt => "Fechar CoopLink — O que pretende fazer?",
            })
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
            .fixed_size(egui::vec2(480.0, 210.0))
            .show(ctx, |ui| {
                ui.add_space(6.0);
                ui.label(
                    RichText::new(match lang {
                        Language::Fr => "Souhaitez-vous réduire CoopLink en arrière-plan (pour ne pas couper votre salon P2P ni le partage de jeu) ou fermer complètement l'application ?",
                        Language::En => "Would you like to minimize CoopLink to the background (to keep your P2P room and game share running) or quit the application completely?",
                        Language::Es => "¿Deseas minimizar CoopLink en segundo plano (para mantener tu sala P2P activa) o cerrar completamente la aplicación?",
                        Language::De => "Möchtest du CoopLink im Hintergrund minimieren (damit dein P2P-Raum aktiv bleibt) oder die Anwendung komplett beenden?",
                        Language::Pt => "Deseja minimizar o CoopLink em segundo plano (para manter a sala P2P ativa) ou fechar completamente a aplicação?",
                    })
                    .color(ModernTheme::TEXT_SECONDARY)
                    .size(13.5),
                );
                ui.add_space(12.0);
                ui.checkbox(
                    &mut self.remember_close_choice,
                    RichText::new(match lang {
                        Language::Fr => "Mémoriser mon choix et ne plus me demander (modifiable dans Paramètres)",
                        Language::En => "Remember my choice and don't ask again (can be changed in Settings)",
                        _ => "Mémoriser mon choix (modifiable dans Paramètres)",
                    })
                    .color(ModernTheme::TEXT_PRIMARY)
                    .strong(),
                );
                ui.add_space(14.0);
                ui.horizontal_wrapped(|ui| {
                    let min_btn = egui::Button::new(
                        RichText::new(match lang {
                            Language::Fr => "Réduire en arrière-plan",
                            Language::En => "Minimize to Background",
                            _ => "Réduire en arrière-plan",
                        })
                        .color(Color32::WHITE)
                        .strong(),
                    )
                    .fill(ModernTheme::BLUE_ELECTRIC)
                    .rounding(egui::Rounding::same(8.0))
                    .min_size(egui::vec2(190.0, 36.0));

                    if ui.add(min_btn).on_hover_cursor(CursorIcon::PointingHand).clicked() {
                        self.config.close_to_background = true;
                        if self.remember_close_choice {
                            self.config.close_behavior_asked = true;
                        }
                        let _ = self.config.save();
                        self.show_close_prompt_modal = false;
                        ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true));
                    }

                    let quit_btn = egui::Button::new(
                        RichText::new(match lang {
                            Language::Fr => "Fermer l'application",
                            Language::En => "Quit Application",
                            _ => "Fermer l'application",
                        })
                        .color(Color32::WHITE)
                        .strong(),
                    )
                    .fill(ModernTheme::RED_ERROR)
                    .rounding(egui::Rounding::same(8.0))
                    .min_size(egui::vec2(170.0, 36.0));

                    if ui.add(quit_btn).on_hover_cursor(CursorIcon::PointingHand).clicked() {
                        self.config.close_to_background = false;
                        if self.remember_close_choice {
                            self.config.close_behavior_asked = true;
                        }
                        let _ = self.config.save();
                        self.disconnect_node();
                        std::process::exit(0);
                    }

                    if ui
                        .add_sized([80.0, 36.0], egui::Button::new("Annuler"))
                        .on_hover_cursor(CursorIcon::PointingHand)
                        .clicked()
                    {
                        self.show_close_prompt_modal = false;
                    }
                });
            });
        }

        if let Some(ref rx) = self.interfaces_rx {
            if let Ok(ifaces) = rx.try_recv() {
                self.state.cached_interfaces = ifaces;
                self.state.is_refreshing_interfaces = false;
                self.interfaces_rx = None;
            }
        }

        self.state.total_bytes_tx = self.tunnel.bytes_tx.load(Ordering::Relaxed);
        self.state.total_bytes_rx = self.tunnel.bytes_rx.load(Ordering::Relaxed);
        self.state.broadcast_packets_relayed = self.broadcast_relay.relayed_count();

        self.state.virtual_ip = *self.tunnel.my_virtual_ip.read();
        self.state.peers = self.tunnel.peers.read().values().cloned().collect();
        self.state.unique_instance_id = self.telemetry.assigned_id.read().clone();
        self.state.central_server_connected = self.telemetry.server_connected.load(Ordering::Relaxed);
        self.sync_telemetry();

        match self.upnp_mgr.lock().state().clone() {
            UpnpState::Active {
                external_ip,
                mapped_port,
                ..
            } => {
                self.state.public_ip = Some(external_ip);
                self.state.upnp_status_text = format!("Actif (UDP {})", mapped_port);
            }
            UpnpState::Searching => {
                self.state.upnp_status_text = "Recherche UPnP...".to_string();
            }
            UpnpState::Failed { reason } => {
                self.state.upnp_status_text = format!("Échec ({})", reason);
            }
            UpnpState::Disabled => {
                self.state.upnp_status_text = "Inactif".to_string();
            }
        }

        let lang = self.config.language;

        // En-tête Moderne Windows 11 Fluent (Blanc épuré assorti à l'icône CoopLink.png)
        egui::TopBottomPanel::top("top_panel")
            .frame(
                egui::Frame::none()
                    .fill(ModernTheme::PANEL_BG)
                    .inner_margin(egui::Margin::symmetric(18.0, 12.0))
                    .stroke(egui::Stroke::new(1.0_f32, ModernTheme::BORDER_SUBTLE)),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    if let Some(ref logo) = self.logo_texture {
                        ui.image((logo.id(), egui::vec2(34.0, 34.0)));
                    }

                    ui.vertical(|ui| {
                        ui.horizontal_wrapped(|ui| {
                            ui.label(
                                RichText::new("CoopLink")
                                    .color(ModernTheme::TEXT_PRIMARY)
                                    .strong()
                                    .size(20.0),
                            );
                            ui.label(
                                RichText::new(I18n::app_subtitle(lang))
                                    .color(ModernTheme::BLUE_ELECTRIC)
                                    .size(13.5),
                            );
                        });
                    });

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let prev_lang = self.config.language;
                        egui::ComboBox::from_id_salt("header_lang_selector")
                            .selected_text(
                                RichText::new(self.config.language.code())
                                    .color(ModernTheme::BLUE_ELECTRIC)
                                    .strong(),
                            )
                            .width(66.0)
                            .show_ui(ui, |ui| {
                                for &l in Language::all() {
                                    ui.selectable_value(
                                        &mut self.config.language,
                                        l,
                                        l.display_name(),
                                    );
                                }
                            });
                        if self.config.language != prev_lang {
                            let _ = self.config.save();
                        }

                        ui.add_space(6.0);

                        // Pastille verte "Prêt" dessinée en vectoriel pur
                        let (pill_color, status_label) = match self.state.status {
                            ConnectionStatus::Hosting => (ModernTheme::GREEN_VIVID, I18n::status_hosting(lang)),
                            ConnectionStatus::Connected => (ModernTheme::GREEN_VIVID, I18n::status_connected(lang)),
                            ConnectionStatus::Connecting => (ModernTheme::ORANGE_CORAL, I18n::status_connecting(lang)),
                            ConnectionStatus::Disconnected => (ModernTheme::GREEN_VIVID, I18n::status_ready(lang)),
                            ConnectionStatus::Error => (ModernTheme::RED_ERROR, I18n::status_error(lang)),
                        };
                        ModernTheme::status_pill(ui, pill_color, status_label);

                        ui.add_space(6.0);

                        // Badge ID Unique d'instance attribué
                        egui::Frame::none()
                            .fill(ModernTheme::SUBTLE_BG)
                            .rounding(egui::Rounding::same(20.0))
                            .inner_margin(egui::Margin::symmetric(10.0, 5.0))
                            .stroke(egui::Stroke::new(1.0_f32, ModernTheme::BORDER_SUBTLE))
                            .show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    ui.spacing_mut().item_spacing.x = 5.0;
                                    ModernTheme::draw_dot(
                                        ui,
                                        if self.state.central_server_connected {
                                            ModernTheme::GREEN_VIVID
                                        } else {
                                            ModernTheme::BLUE_ELECTRIC
                                        },
                                        3.8,
                                    );
                                    ui.label(
                                        RichText::new(format!("ID : {}", self.state.unique_instance_id))
                                            .color(ModernTheme::TEXT_SECONDARY)
                                            .strong()
                                            .size(12.5),
                                    );
                                });
                            });
                    });
                });

                ui.add_space(10.0);

                // Barre de navigation moderne 100% responsive (horizontal_wrapped)
                ui.horizontal_wrapped(|ui| {
                    tab_button(ui, &mut self.active_tab, ActiveTab::Lobby, I18n::tab_lobby(lang));
                    tab_button(
                        ui,
                        &mut self.active_tab,
                        ActiveTab::Streaming,
                        I18n::tab_streaming(lang),
                    );
                    tab_button(
                        ui,
                        &mut self.active_tab,
                        ActiveTab::Speedtest,
                        I18n::tab_speedtest(lang),
                    );
                    tab_button(
                        ui,
                        &mut self.active_tab,
                        ActiveTab::Network,
                        I18n::tab_network(lang),
                    );
                    tab_button(ui, &mut self.active_tab, ActiveTab::Firewall, I18n::tab_firewall(lang));
                    tab_button(ui, &mut self.active_tab, ActiveTab::Settings, I18n::tab_settings(lang));
                    tab_button(ui, &mut self.active_tab, ActiveTab::Console, I18n::tab_console(lang));
                });
            });

        // Barre d'état inférieure moderne
        egui::TopBottomPanel::bottom("bottom_panel")
            .frame(
                egui::Frame::none()
                    .fill(ModernTheme::PANEL_BG)
                    .inner_margin(egui::Margin::symmetric(18.0, 8.0))
                    .stroke(egui::Stroke::new(1.0_f32, ModernTheme::BORDER_SUBTLE)),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ModernTheme::draw_dot(ui, ModernTheme::GREEN_VIVID, 3.8);
                    ui.label(
                        RichText::new(I18n::footer_traffic(
                            lang,
                            self.state.total_bytes_tx as f64 / 1024.0,
                            self.state.total_bytes_rx as f64 / 1024.0,
                        ))
                        .color(ModernTheme::TEXT_SECONDARY)
                        .small(),
                    );

                    ui.add_space(16.0);
                    ui.label(
                        RichText::new(I18n::footer_peers(lang, self.state.peers.len()))
                            .color(ModernTheme::BLUE_ELECTRIC)
                            .strong()
                            .small(),
                    );

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(
                            RichText::new("CoopLink v1.0.0 — Windows 11 / macOS / Linux")
                                .color(ModernTheme::TEXT_MUTED)
                                .small(),
                        );
                    });
                });
            });

        // Zone centrale sur fond doux Windows 11 (#F4F7FB)
        egui::CentralPanel::default()
            .frame(
                egui::Frame::none()
                    .fill(ModernTheme::BG_APP)
                    .inner_margin(18.0),
            )
            .show(ctx, |ui| {
                let mut on_host_clicked = false;
                let mut on_join_clicked = false;
                let mut on_disconnect_clicked = false;
                let mut on_apply_metric_clicked = false;
                let mut on_refresh_interfaces_clicked = false;
                let mut on_repair_firewall_clicked = false;
                let mut on_clear_console_clicked = false;
                let mut on_start_host_stream = false;
                let mut on_stop_host_stream = false;
                let mut on_start_client_stream = false;
                let mut on_stop_client_stream = false;
                let mut on_apply_auto_optimize = false;

                match self.active_tab {
                    ActiveTab::Lobby => {
                        LobbyView::render(
                            ui,
                            &mut self.state,
                            &mut self.config,
                            &self.tunnel,
                            &mut on_host_clicked,
                            &mut on_join_clicked,
                            &mut on_disconnect_clicked,
                        );
                    }
                    ActiveTab::Streaming => {
                        StreamingView::render(
                            ui,
                            lang,
                            &self.state,
                            &mut self.streaming_engine,
                            &mut on_start_host_stream,
                            &mut on_stop_host_stream,
                            &mut on_start_client_stream,
                            &mut on_stop_client_stream,
                        );
                    }
                    ActiveTab::Speedtest => {
                        SpeedtestView::render(
                            ui,
                            lang,
                            &mut self.speedtest_mgr,
                            &self.state,
                            &self.streaming_engine.capture_host.detected_gpu_name,
                            self.streaming_engine.capture_host.detected_virtual_display_name.as_deref(),
                            self.streaming_engine.capture_host.active_encoder,
                            self.streaming_engine.capture_host.auto_optimized_mode,
                            &self.config.player_name,
                            &mut on_apply_auto_optimize,
                        );
                    }
                    ActiveTab::Network => {
                        NetworkView::render(
                            ui,
                            &mut self.state,
                            &mut self.config,
                            &mut on_apply_metric_clicked,
                            &mut on_refresh_interfaces_clicked,
                        );
                    }
                    ActiveTab::Firewall => {
                        FirewallView::render(
                            ui,
                            &mut self.state,
                            &mut self.config,
                            &mut on_repair_firewall_clicked,
                        );
                    }
                    ActiveTab::Settings => {
                        SettingsView::render(ui, &mut self.config, &mut self.saved_notice);
                    }
                    ActiveTab::Console => {
                        ConsoleView::render(ui, lang, &self.logs, &mut on_clear_console_clicked);
                    }
                }

                if on_host_clicked {
                    self.start_host_node();
                }
                if on_join_clicked {
                    self.start_client_node();
                }
                if on_disconnect_clicked {
                    self.disconnect_node();
                }
                if on_apply_metric_clicked {
                    self.apply_low_metric();
                }
                if on_refresh_interfaces_clicked {
                    self.trigger_refresh_interfaces();
                }
                if on_repair_firewall_clicked {
                    self.repair_firewall();
                }
                if on_clear_console_clicked {
                    self.logs.clear();
                }
                if on_apply_auto_optimize {
                    let measured_upload = self
                        .speedtest_mgr
                        .last_result
                        .as_ref()
                        .map(|r| r.upload_mbps)
                        .unwrap_or(100.0);
                    self.streaming_engine
                        .capture_host
                        .apply_auto_optimized_profile(measured_upload);
                    self.config.target_metric = 5;
                    self.apply_low_metric();
                    self.repair_firewall();
                    let _ = self.config.save();
                    self.speedtest_mgr.recalculate_with_optimizations(
                        self.streaming_engine.capture_host.active_encoder.is_hardware(),
                        self.state.metric_applied,
                        self.state.firewall_ok,
                    );
                    self.add_log(
                        LogLevel::Success,
                        "Configuration Auto Optimisée appliquée (Encodeur GPU matériel, Métrique 5, Pare-feu et Audio exclusif du jeu).",
                    );
                }
                if on_start_host_stream {
                    let vip = self.state.virtual_ip.unwrap_or(self.config.host_virtual_ip);
                    let app_target = self.streaming_engine.selected_app_name.clone();
                    let _ = self.streaming_engine.capture_host.start_daemon(vip, &app_target);
                    self.add_log(
                        LogLevel::Game,
                        &format!(
                            "Partage du jeu au premier plan démarré sur {} (Jeu : '{}', Encodeur : {}, Isolation Audio : {}).",
                            vip,
                            app_target,
                            self.streaming_engine.capture_host.active_encoder.display_name(),
                            self.streaming_engine.capture_host.audio_capture_mode.as_conf_str()
                        ),
                    );
                }
                if on_stop_host_stream {
                    self.streaming_engine.capture_host.stop_daemon();
                    self.add_log(LogLevel::Warning, "Partage de jeu arrêté.");
                }
                if on_start_client_stream {
                    let host_vip = self.streaming_engine.stream_viewer.target_host_vip;
                    let my_vip = self.state.virtual_ip.unwrap_or(Ipv4Addr::new(10, 244, 0, 2));
                    let req = self.streaming_engine.pairing.build_pair_request(
                        1002,
                        &self.config.player_name,
                        my_vip,
                        "x360",
                    );
                    let slot = self
                        .streaming_engine
                        .capture_host
                        .assign_remote_controller_slot(&self.config.player_name, my_vip);
                    let ack = self
                        .streaming_engine
                        .pairing
                        .authorize_client_on_host(&req, host_vip, slot);
                    self.streaming_engine.pairing.complete_pairing_on_client(&ack);

                    let app_name = self.streaming_engine.selected_app_name.clone();
                    let _ = self
                        .streaming_engine
                        .stream_viewer
                        .start_stream(host_vip, &app_name);
                    self.add_log(
                        LogLevel::Success,
                        &format!(
                            "Session de jeu partagé ouverte vers {} (Assigné : Joueur {}).",
                            host_vip, slot
                        ),
                    );
                }
                if on_stop_client_stream {
                    self.streaming_engine.stream_viewer.stop_stream();
                    self.add_log(LogLevel::Info, "Session de jeu partagé fermée.");
                }
            });

        ctx.request_repaint_after(std::time::Duration::from_millis(250));
    }
}

impl CoopLinkApp {
    fn start_host_node(&mut self) {
        self.state.status = ConnectionStatus::Hosting;
        self.state.role = NodeRole::Host;
        let port = self.config.listen_port;
        let player_name = self.config.player_name.clone();
        let room_code = self.state.room_code.trim().to_string();
        let auth_token = if self.config.room_password.trim().is_empty() {
            room_code.clone()
        } else {
            format!("{}#{}", room_code, self.config.room_password.trim())
        };
        let host_ip = self.config.host_virtual_ip;

        self.add_log(
            LogLevel::Game,
            &format!("Création du salon '{}' (Port UDP P2P {})...", room_code, port),
        );

        let (adapter, to_adapter_tx, from_adapter) = NativeAdapter::new();
        let (to_bcast_tx, from_bcast_net) = mpsc::channel(1000);
        let (to_tunnel_bcast_tx, from_bcast) = mpsc::channel(1000);
        self.adapter = adapter;
        self.to_adapter_tx = to_adapter_tx.clone();
        self.to_bcast_tx = to_bcast_tx.clone();
        self.to_tunnel_bcast_tx = to_tunnel_bcast_tx.clone();

        let tunnel = self.tunnel.clone();
        let bcast_relay = self.broadcast_relay.clone();

        self.rt.spawn(async move {
            let _ = bcast_relay.start(host_ip, to_tunnel_bcast_tx, from_bcast_net).await;
            if let Err(e) = tunnel
                .start_host(
                    port,
                    player_name,
                    auth_token,
                    host_ip,
                    to_adapter_tx,
                    from_adapter,
                    to_bcast_tx,
                    from_bcast,
                )
                .await
            {
                tracing::error!("Erreur de démarrage du nœud hôte: {}", e);
            }
        });

        self.apply_low_metric();
        self.sync_telemetry();
        self.add_log(
            LogLevel::Success,
            &format!(
                "Salon hébergé actif sur {} (0% charge CPU en veille — prêt pour les jeux LAN et le partage de jeu Co-op).",
                host_ip
            ),
        );

        if self.config.enable_upnp {
            self.state.upnp_status_text = "Recherche UPnP...".to_string();
            let upnp = self.upnp_mgr.clone();
            let listen_port = port;

            self.rt.spawn_blocking(move || {
                let mut mgr = upnp.lock();
                let _ = mgr.open_port(listen_port);
            });
        }
    }

    fn start_client_node(&mut self) {
        let server_str = self.config.remote_server_address.clone();
        let server_addr: SocketAddr = match server_str.parse() {
            Ok(addr) => addr,
            Err(_) => match format!("{}:{}", server_str, self.config.listen_port).parse() {
                Ok(addr) => addr,
                Err(e) => {
                    self.add_log(LogLevel::Error, &format!("Adresse invalide: {}", e));
                    return;
                }
            },
        };

        self.state.status = ConnectionStatus::Connecting;
        self.state.role = NodeRole::Client;
        let player_name = self.config.player_name.clone();
        let room_code = self.state.room_code.trim().to_string();
        let auth_token = if self.config.room_password.trim().is_empty() {
            room_code.clone()
        } else {
            format!("{}#{}", room_code, self.config.room_password.trim())
        };

        self.add_log(
            LogLevel::Info,
            &format!("Connexion P2P au salon '{}' ({})...", room_code, server_addr),
        );

        let (adapter, to_adapter_tx, from_adapter) = NativeAdapter::new();
        let (to_bcast_tx, from_bcast_net) = mpsc::channel(1000);
        let (to_tunnel_bcast_tx, from_bcast) = mpsc::channel(1000);
        self.adapter = adapter;
        self.to_adapter_tx = to_adapter_tx.clone();
        self.to_bcast_tx = to_bcast_tx.clone();
        self.to_tunnel_bcast_tx = to_tunnel_bcast_tx.clone();

        let tunnel = self.tunnel.clone();
        let bcast_relay = self.broadcast_relay.clone();

        self.rt.spawn(async move {
            let dummy_vip = Ipv4Addr::new(10, 244, 0, 99);
            let _ = bcast_relay.start(dummy_vip, to_tunnel_bcast_tx, from_bcast_net).await;
            if let Err(e) = tunnel
                .start_client(
                    server_addr,
                    player_name,
                    auth_token,
                    to_adapter_tx,
                    from_adapter,
                    to_bcast_tx,
                    from_bcast,
                )
                .await
            {
                tracing::error!("Erreur client tunnel: {}", e);
            }
        });

        self.apply_low_metric();
        self.state.status = ConnectionStatus::Connected;
        self.sync_telemetry();
        self.add_log(
            LogLevel::Success,
            "Connexion directe P2P établie ! Vous pouvez lancer votre jeu LAN ou rejoindre le jeu partagé.",
        );
    }

    fn disconnect_node(&mut self) {
        self.tunnel.stop();
        self.broadcast_relay.stop();
        self.streaming_engine.capture_host.stop_daemon();
        self.streaming_engine.stream_viewer.stop_stream();
        self.upnp_mgr.lock().close_port();
        self.state.public_ip = None;
        self.state.status = ConnectionStatus::Disconnected;
        self.state.role = NodeRole::Idle;
        self.state.virtual_ip = None;
        self.state.peers.clear();
        self.sync_telemetry();
        self.add_log(LogLevel::Warning, "Session fermée.");
    }

    fn apply_low_metric(&mut self) {
        let metric = self.config.target_metric;
        let adapter_name = self.state.adapter_name.clone();
        let _ = set_os_adapter_metric(&adapter_name, metric);
        self.state.metric_applied = true;
    }

    pub fn trigger_refresh_interfaces(&mut self) {
        if self.state.is_refreshing_interfaces {
            return;
        }
        self.state.is_refreshing_interfaces = true;
        let (tx, rx) = crossbeam_channel::bounded(1);
        self.interfaces_rx = Some(rx);
        std::thread::spawn(move || {
            let list = get_os_network_interfaces();
            let _ = tx.send(list);
        });
    }

    fn repair_firewall(&mut self) {
        let port = self.config.listen_port;
        let subnet = format!("{}.0/24", self.config.virtual_subnet_prefix);
        match setup_os_firewall_rules(port, &subnet) {
            Ok(_) => {
                self.state.firewall_ok = true;
                self.add_log(LogLevel::Success, "Règles pare-feu système appliquées.");
            }
            Err(e) => {
                self.add_log(LogLevel::Error, &format!("Erreur pare-feu: {}", e));
            }
        }
    }
}

fn tab_button(ui: &mut egui::Ui, current: &mut ActiveTab, tab: ActiveTab, label: &str) {
    let is_selected = *current == tab;
    let text = if is_selected {
        RichText::new(label).color(Color32::WHITE).strong().size(13.5)
    } else {
        RichText::new(label).color(ModernTheme::TEXT_SECONDARY).size(13.5)
    };

    let btn = egui::Button::new(text)
        .fill(if is_selected {
            ModernTheme::BLUE_ELECTRIC
        } else {
            ModernTheme::SUBTLE_BG
        })
        .stroke(if is_selected {
            egui::Stroke::new(1.0_f32, ModernTheme::BLUE_ELECTRIC)
        } else {
            egui::Stroke::new(1.0_f32, ModernTheme::BORDER_SUBTLE)
        })
        .rounding(egui::Rounding::same(8.0));

    if ui.add(btn).on_hover_cursor(CursorIcon::PointingHand).clicked() {
        *current = tab;
    }
}
