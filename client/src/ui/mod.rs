pub mod logo;
pub mod retro_theme;
pub mod views;

use crate::adapter::windows::WindowsVirtualAdapter;
use crate::config::AppConfig;
use crate::network::broadcast_relay::BroadcastRelay;
use crate::network::tunnel::NetworkTunnel;
use crate::network::upnp::{UpnpManager, UpnpState};
use crate::platform::windows::firewall::{check_firewall_status, setup_firewall_rules};
use crate::platform::windows::metric::set_adapter_metric;
use crate::streaming::StreamingEngine;
use crate::types::{AppState, ConnectionStatus, LogEntry, LogLevel, NodeRole};
use chrono::Local;
use eframe::egui;
use egui::{Color32, RichText, TextureHandle};
use logo::load_embedded_logo;
use parking_lot::Mutex;
use retro_theme::ModernTheme;
use std::net::{Ipv4Addr, SocketAddr};
use std::sync::Arc;
use tokio::runtime::Runtime;
use tokio::sync::mpsc;
use views::console::ConsoleView;
use views::firewall::FirewallView;
use views::lobby::LobbyView;
use views::network::NetworkView;
use views::settings::SettingsView;
use views::streaming::StreamingView;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActiveTab {
    Lobby,
    Streaming,
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
    pub streaming_engine: StreamingEngine,
    pub to_adapter_tx: mpsc::Sender<Vec<u8>>,
    pub from_net_rx: Option<mpsc::Receiver<Vec<u8>>>,
    pub to_bcast_tx: mpsc::Sender<(u16, Vec<u8>)>,
    pub from_bcast_rx: Option<mpsc::Receiver<(u16, Vec<u8>)>>,
    pub to_tunnel_bcast_tx: mpsc::Sender<(u16, Vec<u8>)>,
    pub from_tunnel_bcast_rx: Option<mpsc::Receiver<(u16, Vec<u8>)>>,
    pub logo_texture: Option<TextureHandle>,
    pub interfaces_rx: Option<crossbeam_channel::Receiver<Vec<crate::platform::windows::metric::InterfaceMetricInfo>>>,
}

impl CoopLinkApp {
    pub fn new(cc: &eframe::CreationContext<'_>, rt: Arc<Runtime>) -> Self {
        ModernTheme::apply(&cc.egui_ctx);
        let config = AppConfig::load();
        let mut state = AppState::default();

        let (_adapter, to_adapter_tx, from_net_rx) = WindowsVirtualAdapter::new();
        let (to_bcast_tx, from_bcast_rx) = mpsc::channel(1000);
        let (to_tunnel_bcast_tx, from_tunnel_bcast_rx) = mpsc::channel(1000);

        let tunnel = Arc::new(NetworkTunnel::new());
        let broadcast_relay = Arc::new(BroadcastRelay::new());
        let upnp_mgr = Arc::new(Mutex::new(UpnpManager::new()));
        let streaming_engine = StreamingEngine::new(config.host_virtual_ip);

        state.firewall_ok = check_firewall_status();
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
            streaming_engine,
            to_adapter_tx,
            from_net_rx: Some(from_net_rx),
            to_bcast_tx,
            from_bcast_rx: Some(from_bcast_rx),
            to_tunnel_bcast_tx,
            from_tunnel_bcast_rx: Some(from_tunnel_bcast_rx),
            logo_texture,
            interfaces_rx: None,
        };

        app.trigger_refresh_interfaces();
        app.add_log(
            LogLevel::Success,
            "CoopLink prêt (Architecture P2P directe & Streaming Sunshine/Moonlight intégrés).",
        );

        if app.config.auto_firewall_rule && !app.state.firewall_ok {
            let port = app.config.listen_port;
            let subnet = format!("{}.0/24", app.config.virtual_subnet_prefix);
            let _ = setup_firewall_rules(port, &subnet);
            app.state.firewall_ok = check_firewall_status();
            app.add_log(LogLevel::Info, "Règles pare-feu Windows appliquées automatiquement.");
        }

        if app.config.auto_metric_optimization {
            let target_metric = app.config.target_metric;
            let adapter_name = app.state.adapter_name.clone();
            let _ = set_adapter_metric(&adapter_name, target_metric);
            app.state.metric_applied = true;
            app.add_log(
                LogLevel::Info,
                &format!("Métrique prioritaire ({}) appliquée à l'interface virtuelle.", target_metric),
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
}

impl eframe::App for CoopLinkApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if let Some(ref rx) = self.interfaces_rx {
            if let Ok(ifaces) = rx.try_recv() {
                self.state.cached_interfaces = ifaces;
                self.state.is_refreshing_interfaces = false;
                self.interfaces_rx = None;
            }
        }

        self.state.total_bytes_tx = self.tunnel.bytes_tx.load(std::sync::atomic::Ordering::Relaxed);
        self.state.total_bytes_rx = self.tunnel.bytes_rx.load(std::sync::atomic::Ordering::Relaxed);
        self.state.broadcast_packets_relayed = self.broadcast_relay.relayed_count();

        self.state.virtual_ip = *self.tunnel.my_virtual_ip.read();
        self.state.peers = self.tunnel.peers.read().values().cloned().collect();

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
                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new("CoopLink")
                                    .color(ModernTheme::TEXT_PRIMARY)
                                    .strong()
                                    .size(20.0),
                            );
                            ui.label(
                                RichText::new("Réseau Virtuel P2P & Streaming Coopératif")
                                    .color(ModernTheme::BLUE_ELECTRIC)
                                    .size(13.5),
                            );
                        });
                    });

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let (dot_color, status_label) = match self.state.status {
                            ConnectionStatus::Hosting => (ModernTheme::GREEN_VIVID, "Salon Hôte Actif"),
                            ConnectionStatus::Connected => (ModernTheme::BLUE_ELECTRIC, "Connecté en P2P"),
                            ConnectionStatus::Connecting => (ModernTheme::ORANGE_CORAL, "Connexion..."),
                            ConnectionStatus::Disconnected => (ModernTheme::TEXT_MUTED, "Prêt"),
                            ConnectionStatus::Error => (ModernTheme::RED_ERROR, "Erreur réseau"),
                        };

                        ui.label(RichText::new(status_label).color(dot_color).strong());
                        ui.label(RichText::new("●").color(dot_color).size(14.0));
                    });
                });

                ui.add_space(10.0);

                // Barre de navigation moderne style Windows 11
                ui.horizontal(|ui| {
                    tab_button(ui, &mut self.active_tab, ActiveTab::Lobby, "Salons & Réseau P2P");
                    tab_button(
                        ui,
                        &mut self.active_tab,
                        ActiveTab::Streaming,
                        "Streaming & Manettes",
                    );
                    tab_button(
                        ui,
                        &mut self.active_tab,
                        ActiveTab::Network,
                        "Priorité & Cartes",
                    );
                    tab_button(ui, &mut self.active_tab, ActiveTab::Firewall, "Pare-feu");
                    tab_button(ui, &mut self.active_tab, ActiveTab::Settings, "Paramètres");
                    tab_button(ui, &mut self.active_tab, ActiveTab::Console, "Activité");
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
                    ui.label(
                        RichText::new(format!(
                            "Trafic P2P Direct — Envoyé : {:.2} Ko | Reçu : {:.2} Ko",
                            self.state.total_bytes_tx as f64 / 1024.0,
                            self.state.total_bytes_rx as f64 / 1024.0
                        ))
                        .color(ModernTheme::TEXT_SECONDARY)
                        .small(),
                    );

                    ui.add_space(16.0);
                    ui.label(
                        RichText::new(format!("Participants : {}", self.state.peers.len()))
                            .color(ModernTheme::BLUE_ELECTRIC)
                            .strong()
                            .small(),
                    );

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(
                            RichText::new("CoopLink v1.0.0 — Windows 11")
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
                let mut on_start_sunshine_host = false;
                let mut on_stop_sunshine_host = false;
                let mut on_start_moonlight_client = false;
                let mut on_stop_moonlight_client = false;

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
                            &self.state,
                            &mut self.streaming_engine,
                            &mut on_start_sunshine_host,
                            &mut on_stop_sunshine_host,
                            &mut on_start_moonlight_client,
                            &mut on_stop_moonlight_client,
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
                        ConsoleView::render(ui, &self.logs, &mut on_clear_console_clicked);
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
                if on_start_sunshine_host {
                    let vip = self.state.virtual_ip.unwrap_or(self.config.host_virtual_ip);
                    let _ = self.streaming_engine.sunshine_host.start_daemon(vip);
                    self.add_log(
                        LogLevel::Game,
                        &format!(
                            "Serveur Sunshine démarré et lié exclusivement à l'IP virtuelle {} (Encodeur: {}).",
                            vip,
                            self.streaming_engine.sunshine_host.active_encoder.display_name()
                        ),
                    );
                }
                if on_stop_sunshine_host {
                    self.streaming_engine.sunshine_host.stop_daemon();
                    self.add_log(LogLevel::Warning, "Partage d'écran Sunshine arrêté.");
                }
                if on_start_moonlight_client {
                    let host_vip = self.streaming_engine.moonlight_client.target_host_vip;
                    let my_vip = self.state.virtual_ip.unwrap_or(Ipv4Addr::new(10, 244, 0, 2));
                    let req = self.streaming_engine.pairing.build_pair_request(
                        1002,
                        &self.config.player_name,
                        my_vip,
                        "x360",
                    );
                    let slot = self
                        .streaming_engine
                        .sunshine_host
                        .assign_remote_controller_slot(&self.config.player_name, my_vip);
                    let ack = self
                        .streaming_engine
                        .pairing
                        .authorize_client_on_host(&req, host_vip, slot);
                    self.streaming_engine.pairing.complete_pairing_on_client(&ack);

                    let app_name = self.streaming_engine.selected_app_name.clone();
                    let _ = self
                        .streaming_engine
                        .moonlight_client
                        .start_stream(host_vip, &app_name);
                    self.add_log(
                        LogLevel::Success,
                        &format!(
                            "Jumelage Zero-Config réussi & Flux Moonlight démarré vers {} (Assigné: Joueur {}).",
                            host_vip, slot
                        ),
                    );
                }
                if on_stop_moonlight_client {
                    self.streaming_engine.moonlight_client.stop_stream();
                    self.add_log(LogLevel::Info, "Flux Moonlight fermé.");
                }
            });

        ctx.request_repaint_after(std::time::Duration::from_millis(100));
    }
}

impl CoopLinkApp {
    fn start_host_node(&mut self) {
        self.state.status = ConnectionStatus::Hosting;
        self.state.role = NodeRole::Host;
        let port = self.config.listen_port;
        let player_name = self.config.player_name.clone();
        let room_code = self.state.room_code.clone();
        let host_ip = self.config.host_virtual_ip;

        self.add_log(
            LogLevel::Game,
            &format!("Création du salon '{}' (Port UDP P2P {})...", room_code, port),
        );

        let tunnel = self.tunnel.clone();
        let to_adapter = self.to_adapter_tx.clone();
        let from_adapter = self.from_net_rx.take().unwrap_or_else(|| mpsc::channel(1000).1);
        let to_bcast = self.to_bcast_tx.clone();
        let from_bcast = self.from_tunnel_bcast_rx.take().unwrap_or_else(|| mpsc::channel(1000).1);

        let bcast_relay = self.broadcast_relay.clone();
        let to_tunnel_bcast = self.to_tunnel_bcast_tx.clone();
        let from_bcast_net = self.from_bcast_rx.take().unwrap_or_else(|| mpsc::channel(1000).1);

        self.rt.spawn(async move {
            let _ = bcast_relay.start(host_ip, to_tunnel_bcast, from_bcast_net).await;
            if let Err(e) = tunnel
                .start_host(
                    port,
                    player_name,
                    room_code,
                    host_ip,
                    to_adapter,
                    from_adapter,
                    to_bcast,
                    from_bcast,
                )
                .await
            {
                tracing::error!("Erreur de démarrage du nœud hôte: {}", e);
            }
        });

        // Démarre automatiquement le service Sunshine lié à l'IP virtuelle de l'hôte
        let _ = self.streaming_engine.sunshine_host.start_daemon(host_ip);

        self.apply_low_metric();
        self.add_log(
            LogLevel::Success,
            &format!(
                "Salon hébergé actif sur {} (Sunshine prêt pour le jumelage Zero-Config).",
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
        let room_code = self.state.room_code.clone();

        self.add_log(
            LogLevel::Info,
            &format!("Connexion P2P au salon '{}' ({})...", room_code, server_addr),
        );

        let tunnel = self.tunnel.clone();
        let to_adapter = self.to_adapter_tx.clone();
        let from_adapter = self.from_net_rx.take().unwrap_or_else(|| mpsc::channel(1000).1);
        let to_bcast = self.to_bcast_tx.clone();
        let from_bcast = self.from_tunnel_bcast_rx.take().unwrap_or_else(|| mpsc::channel(1000).1);

        let bcast_relay = self.broadcast_relay.clone();
        let to_tunnel_bcast = self.to_tunnel_bcast_tx.clone();
        let from_bcast_net = self.from_bcast_rx.take().unwrap_or_else(|| mpsc::channel(1000).1);

        self.rt.spawn(async move {
            let dummy_vip = Ipv4Addr::new(10, 244, 0, 99);
            let _ = bcast_relay.start(dummy_vip, to_tunnel_bcast, from_bcast_net).await;
            if let Err(e) = tunnel
                .start_client(
                    server_addr,
                    player_name,
                    room_code,
                    to_adapter,
                    from_adapter,
                    to_bcast,
                    from_bcast,
                )
                .await
            {
                tracing::error!("Erreur client tunnel: {}", e);
            }
        });

        self.apply_low_metric();
        self.state.status = ConnectionStatus::Connected;
        self.add_log(
            LogLevel::Success,
            "Connexion directe P2P établie ! Vous pouvez lancer le streaming ou votre jeu.",
        );
    }

    fn disconnect_node(&mut self) {
        self.tunnel.stop();
        self.broadcast_relay.stop();
        self.streaming_engine.sunshine_host.stop_daemon();
        self.streaming_engine.moonlight_client.stop_stream();
        self.upnp_mgr.lock().close_port();
        self.state.public_ip = None;
        self.state.status = ConnectionStatus::Disconnected;
        self.state.role = NodeRole::Idle;
        self.state.virtual_ip = None;
        self.state.peers.clear();
        self.add_log(LogLevel::Warning, "Session fermée.");
    }

    fn apply_low_metric(&mut self) {
        let metric = self.config.target_metric;
        let adapter_name = self.state.adapter_name.clone();
        let _ = set_adapter_metric(&adapter_name, metric);
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
            let list = crate::platform::windows::metric::get_network_interfaces();
            let _ = tx.send(list);
        });
    }

    fn repair_firewall(&mut self) {
        let port = self.config.listen_port;
        let subnet = format!("{}.0/24", self.config.virtual_subnet_prefix);
        match setup_firewall_rules(port, &subnet) {
            Ok(_) => {
                self.state.firewall_ok = true;
                self.add_log(LogLevel::Success, "Règles pare-feu Windows appliquées.");
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
        RichText::new(label).color(Color32::WHITE).strong()
    } else {
        RichText::new(label).color(ModernTheme::TEXT_SECONDARY)
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

    if ui.add(btn).clicked() {
        *current = tab;
    }
}
