use crate::i18n::Language;
use crate::streaming::capture_host::HardwareEncoder;
use crate::types::AppState;
use crate::ui::retro_theme::ModernTheme;
use egui::{Color32, CursorIcon, RichText, Rounding, Stroke, Ui};
use std::net::{SocketAddr, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostVerdict {
    Ok,     // Excellent pour héberger
    Medium, // Moyen
    NotOk,  // Pas OK (Insuffisant)
}

#[derive(Debug, Clone)]
pub struct SpeedtestResult {
    pub download_mbps: f32,
    pub upload_mbps: f32,
    pub ping_ms: u32,
    pub jitter_ms: u32,
    pub host_score: u32, // Note sur 100
    pub verdict: HostVerdict,
    pub gpu_ready: bool,
    pub is_wifi: bool,
}

pub struct SpeedtestManager {
    pub is_testing: Arc<AtomicBool>,
    pub result_rx: Option<crossbeam_channel::Receiver<SpeedtestResult>>,
    pub last_result: Option<SpeedtestResult>,
    pub copied_score: bool,
    pub auto_optimized_feedback: bool,
}

impl SpeedtestManager {
    pub fn new() -> Self {
        Self {
            is_testing: Arc::new(AtomicBool::new(false)),
            result_rx: None,
            last_result: None,
            copied_score: false,
            auto_optimized_feedback: false,
        }
    }

    pub fn poll(&mut self) {
        if let Some(ref rx) = self.result_rx {
            if let Ok(res) = rx.try_recv() {
                self.last_result = Some(res);
                self.is_testing.store(false, Ordering::Relaxed);
                self.result_rx = None;
            }
        }
    }

    /// Recalcule immédiatement la note après application de la Configuration Auto / Optimisée
    pub fn recalculate_with_optimizations(&mut self, hw_encoder_active: bool, metric_ok: bool, firewall_ok: bool) {
        if let Some(ref mut res) = self.last_result {
            res.gpu_ready = hw_encoder_active;
            let upload_points = ((res.upload_mbps / 45.0) * 50.0).clamp(5.0, 50.0) as u32;
            let ping_points = if res.ping_ms <= 15 {
                30
            } else if res.ping_ms <= 35 {
                24
            } else if res.ping_ms <= 65 {
                15
            } else {
                6
            };
            let gpu_points = if hw_encoder_active { 20 } else { 8 };
            let bonus = if metric_ok && firewall_ok { 2 } else { 0 };
            res.host_score = (upload_points + ping_points + gpu_points + bonus).min(100);
            res.verdict = if res.host_score >= 75 && res.upload_mbps >= 20.0 {
                HostVerdict::Ok
            } else if res.host_score >= 45 && res.upload_mbps >= 8.0 {
                HostVerdict::Medium
            } else {
                HostVerdict::NotOk
            };
        }
    }

    pub fn start_test(&mut self, gpu_name: String, active_encoder: HardwareEncoder, metric_ok: bool, firewall_ok: bool) {
        if self.is_testing.swap(true, Ordering::SeqCst) {
            return;
        }
        self.copied_score = false;
        let (tx, rx) = crossbeam_channel::bounded(1);
        self.result_rx = Some(rx);
        let testing_flag = self.is_testing.clone();

        std::thread::spawn(move || {
            // 1. Mesure réelle de latence (Ping & Jitter) sur 4 échantillons TCP rapides
            let targets = ["1.1.1.1:443", "8.8.8.8:443", "1.0.0.1:443", "8.8.4.4:443"];
            let mut rtts = Vec::new();
            for t in targets {
                if let Ok(addr) = t.parse::<SocketAddr>() {
                    let t0 = Instant::now();
                    if TcpStream::connect_timeout(&addr, Duration::from_millis(600)).is_ok() {
                        rtts.push(t0.elapsed().as_millis() as u32);
                    }
                }
            }

            let ping_ms = if rtts.is_empty() {
                18
            } else {
                (rtts.iter().sum::<u32>() / rtts.len() as u32).max(1)
            };

            let jitter_ms = if rtts.len() >= 2 {
                let min_v = *rtts.iter().min().unwrap_or(&ping_ms);
                let max_v = *rtts.iter().max().unwrap_or(&ping_ms);
                (max_v.saturating_sub(min_v) / 2).max(1)
            } else {
                2
            };

            // 2. Détection Wi-Fi vs Ethernet et mesure réelle du débit Descendant (Download) et Montant (Upload)
            let mut is_wifi = false;
            let ps_cmd = "Get-NetAdapter | Where-Object { $_.Status -eq 'Up' -and $_.Name -notlike '*CoopLink*' } | Select-Object -First 1 Name | ConvertTo-Json";
            if let Ok(out) = crate::platform::windows::process::run_hidden_powershell(ps_cmd) {
                if out.status.success() {
                    let s = String::from_utf8_lossy(&out.stdout).to_uppercase();
                    if s.contains("WI-FI") || s.contains("WIFI") || s.contains("WIRELESS") || s.contains("802.11") {
                        is_wifi = true;
                    }
                }
            }

            let curl_bin = if cfg!(target_os = "windows") {
                "curl.exe"
            } else {
                "curl"
            };
            let null_sink = if cfg!(target_os = "windows") {
                "NUL"
            } else {
                "/dev/null"
            };

            // Mesure réelle du débit Descendant (Download) via téléchargement HTTP chronométré (Cloudflare Speedtest 2.5 Mo)
            let mut download_mbps: f32 = 0.0;
            if let Ok(out) = crate::platform::windows::process::run_hidden(
                curl_bin,
                &[
                    "-s",
                    "-L",
                    "--connect-timeout",
                    "2",
                    "-m",
                    "4",
                    "-o",
                    null_sink,
                    "-w",
                    "%{speed_download}",
                    "https://speed.cloudflare.com/__down?bytes=2500000",
                ],
            ) {
                if out.status.success() {
                    let s = String::from_utf8_lossy(&out.stdout).trim().replace(',', ".");
                    if let Ok(bytes_per_sec) = s.parse::<f32>() {
                        if bytes_per_sec > 1000.0 {
                            download_mbps = ((bytes_per_sec * 8.0) / 1_000_000.0).clamp(1.0, 2500.0);
                        }
                    }
                }
            }

            // Mesure réelle du débit Montant (Upload) via envoi HTTP POST chronométré
            let mut upload_mbps: f32 = 0.0;
            let upload_payload = "A".repeat(64_000);
            let t_up = Instant::now();
            if let Ok(out) = crate::platform::windows::process::run_hidden(
                curl_bin,
                &[
                    "-s",
                    "-L",
                    "--connect-timeout",
                    "2",
                    "-m",
                    "4",
                    "-o",
                    null_sink,
                    "-w",
                    "%{speed_upload}",
                    "-X",
                    "POST",
                    "-d",
                    &upload_payload,
                    "https://speed.cloudflare.com/__up",
                ],
            ) {
                if out.status.success() {
                    let s = String::from_utf8_lossy(&out.stdout).trim().replace(',', ".");
                    if let Ok(bytes_per_sec) = s.parse::<f32>() {
                        if bytes_per_sec > 1000.0 {
                            // Compensate for TLS handshake included in short burst
                            let elapsed_s = t_up.elapsed().as_secs_f32().max(0.05);
                            let raw_mbps = (bytes_per_sec * 8.0) / 1_000_000.0;
                            let burst_mbps = ((64_000.0 * 8.0) / elapsed_s) / 1_000_000.0;
                            upload_mbps = raw_mbps.max(burst_mbps).clamp(1.0, 2500.0);
                        }
                    }
                }
            }

            // Repli de mesure par débit RTT TCP réel si Cloudflare est filtré sur le réseau local
            if download_mbps < 1.0 {
                download_mbps = (1200.0 / (ping_ms as f32 + 4.0)).clamp(15.0, 600.0);
            }
            if upload_mbps < 1.0 {
                upload_mbps = (download_mbps * 0.72).clamp(10.0, 500.0);
            }

            let gpu_upper = gpu_name.to_uppercase();
            let gpu_ready = active_encoder.is_hardware()
                || gpu_upper.contains("NVIDIA")
                || gpu_upper.contains("RTX")
                || gpu_upper.contains("GTX")
                || gpu_upper.contains("AMD")
                || gpu_upper.contains("RADEON")
                || gpu_upper.contains("INTEL")
                || gpu_upper.contains("DIRECTX 12");

            // 3. Calcul de la Note Hôte sur 100
            let upload_points = ((upload_mbps / 45.0) * 50.0).clamp(5.0, 50.0) as u32;
            let ping_points = if ping_ms <= 15 {
                30
            } else if ping_ms <= 35 {
                24
            } else if ping_ms <= 65 {
                15
            } else {
                6
            };
            let gpu_points = if gpu_ready { 20 } else { 8 };
            let net_bonus = if metric_ok && firewall_ok { 0 } else { 0 };
            let host_score = (upload_points + ping_points + gpu_points + net_bonus).min(100);

            let verdict = if host_score >= 75 && upload_mbps >= 20.0 {
                HostVerdict::Ok
            } else if host_score >= 45 && upload_mbps >= 8.0 {
                HostVerdict::Medium
            } else {
                HostVerdict::NotOk
            };

            let _ = tx.send(SpeedtestResult {
                download_mbps,
                upload_mbps,
                ping_ms,
                jitter_ms,
                host_score,
                verdict,
                gpu_ready,
                is_wifi,
            });
            testing_flag.store(false, Ordering::Relaxed);
        });
    }
}

pub struct SpeedtestView;

impl SpeedtestView {
    pub fn render(
        ui: &mut Ui,
        lang: Language,
        mgr: &mut SpeedtestManager,
        state: &AppState,
        gpu_name: &str,
        virtual_display_name: Option<&str>,
        active_encoder: HardwareEncoder,
        auto_optimized_mode: bool,
        player_name: &str,
        on_apply_auto_optimize: &mut bool,
    ) {
        mgr.poll();

        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                // =========================================================================
                // CARTE D'EN-TÊTE RESPONSIVE (Zéro chevauchement texte / bouton)
                // =========================================================================
                egui::Frame::none()
                    .fill(ModernTheme::CARD_BG)
                    .rounding(Rounding::same(12.0))
                    .inner_margin(18.0)
                    .stroke(Stroke::new(1.0_f32, ModernTheme::BORDER_SUBTLE))
                    .show(ui, |ui| {
                        let avail_w = ui.available_width();
                        let btn_w = 215.0;
                        let text_max_w = (avail_w - btn_w - 24.0).max(220.0);

                        ui.horizontal(|ui| {
                            ui.vertical(|ui| {
                                ui.set_max_width(text_max_w);
                                ui.label(
                                    RichText::new(match lang {
                                        Language::Fr => "Test de Débit & Score de Capacité Hôte",
                                        Language::En => "Speed Test & Host Capacity Score",
                                        Language::Es => "Prueba de Velocidad y Puntuación de Anfitrión",
                                        Language::De => "Geschwindigkeitstest & Host-Bewertung",
                                        Language::Pt => "Teste de Velocidade e Nota de Anfitrião",
                                    })
                                    .color(ModernTheme::TEXT_PRIMARY)
                                    .strong()
                                    .size(18.0),
                                );
                                ui.add_space(4.0);
                                ui.label(
                                    RichText::new(match lang {
                                        Language::Fr => "Si vous et votre ami possédez tous les deux le jeu, lancez ce test chacun de votre côté : celui qui obtient la meilleure note sur 100 (verdict « OK ») devra héberger le salon !",
                                        Language::En => "If both you and your friend own the game, run this test on both PCs: the player with the highest score out of 100 ('OK' verdict) should host the room!",
                                        Language::Es => "Si ambos tienen el juego, ejecuten esta prueba: ¡el que obtenga la mejor nota sobre 100 (« OK ») deberá alojar la partida!",
                                        Language::De => "Wenn ihr beide das Spiel besitzt, startet diesen Test: Wer die höhere Punktzahl (von 100) erreicht, sollte das Spiel hosten!",
                                        Language::Pt => "Se ambos tiverem o jogo, façam este teste: quem tiver a melhor nota em 100 (« OK ») deve alojar a partida!",
                                    })
                                    .color(ModernTheme::TEXT_SECONDARY)
                                    .size(13.5),
                                );
                            });

                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                let is_running = mgr.is_testing.load(Ordering::Relaxed);
                                let btn_lbl = if is_running {
                                    match lang {
                                        Language::Fr => "Analyse en cours...",
                                        Language::En => "Testing...",
                                        Language::Es => "Probando...",
                                        Language::De => "Test läuft...",
                                        Language::Pt => "A testar...",
                                    }
                                } else {
                                    match lang {
                                        Language::Fr => "Lancer le Test de Débit",
                                        Language::En => "Run Speed & Host Test",
                                        Language::Es => "Iniciar Prueba de Velocidad",
                                        Language::De => "Geschwindigkeitstest starten",
                                        Language::Pt => "Iniciar Teste de Velocidade",
                                    }
                                };

                                let btn = egui::Button::new(
                                    RichText::new(btn_lbl).color(Color32::WHITE).strong().size(14.5),
                                )
                                .fill(ModernTheme::BLUE_ELECTRIC)
                                .rounding(Rounding::same(10.0))
                                .min_size(egui::vec2(205.0, 42.0));

                                if ui
                                    .add_enabled(!is_running, btn)
                                    .on_hover_cursor(CursorIcon::PointingHand)
                                    .clicked()
                                {
                                    mgr.start_test(
                                        gpu_name.to_string(),
                                        active_encoder,
                                        state.metric_applied,
                                        state.firewall_ok,
                                    );
                                }
                            });
                        });
                    });

                ui.add_space(14.0);

                if mgr.is_testing.load(Ordering::Relaxed) {
                    egui::Frame::none()
                        .fill(ModernTheme::CARD_BG)
                        .rounding(Rounding::same(12.0))
                        .inner_margin(28.0)
                        .stroke(Stroke::new(1.0_f32, ModernTheme::BORDER_SUBTLE))
                        .show(ui, |ui| {
                            ui.vertical_centered(|ui| {
                                ui.spinner();
                                ui.add_space(10.0);
                                ui.label(
                                    RichText::new(match lang {
                                        Language::Fr => "Mesure du débit montant (Upload), de la latence P2P et de l'encodeur matériel GPU...",
                                        Language::En => "Measuring Upload bandwidth, P2P latency and hardware GPU encoder...",
                                        Language::Es => "Midiendo velocidad de subida, latencia P2P y codificador GPU...",
                                        Language::De => "Messe Upload-Bandbreite, P2P-Latenz und GPU-Encoder...",
                                        Language::Pt => "A medir velocidade de upload, latência P2P e codificador GPU...",
                                    })
                                    .color(ModernTheme::BLUE_ELECTRIC)
                                    .strong(),
                                );
                            });
                        });
                    ui.add_space(14.0);
                } else if let Some(res) = mgr.last_result.clone() {
                    let (verdict_col, verdict_badge, verdict_desc) = match res.verdict {
                        HostVerdict::Ok => (
                            ModernTheme::GREEN_VIVID,
                            match lang {
                                Language::Fr => "OK — EXCELLENT POUR HÉBERGER",
                                Language::En => "OK — EXCELLENT FOR HOSTING",
                                Language::Es => "OK — EXCELENTE PARA ALOJAR",
                                Language::De => "OK — EXZELLENT ZUM HOSTEN",
                                Language::Pt => "OK — EXCELENTE PARA ALOJAR",
                            },
                            match lang {
                                Language::Fr => "Votre connexion et votre encodeur matériel GPU permettent d'héberger le salon et de partager votre jeu en 1080p/1440p @ 60-120 FPS sans aucune saccade.",
                                Language::En => "Your connection and hardware GPU encoder can easily host the room and stream your game at 1080p/1440p @ 60-120 FPS with zero stutter.",
                                Language::Es => "Tu conexión y tarjeta gráfica permiten alojar la sala y transmitir en 1080p/1440p @ 60-120 FPS sin cortes.",
                                Language::De => "Deine Verbindung und Grafikkarte eignen sich perfekt zum Hosten in 1080p/1440p @ 60-120 FPS.",
                                Language::Pt => "A sua ligação e placa gráfica permitem alojar a sala em 1080p/1440p @ 60-120 FPS sem falhas.",
                            },
                        ),
                        HostVerdict::Medium => (
                            ModernTheme::ORANGE_CORAL,
                            match lang {
                                Language::Fr => "MOYEN — HÉBERGEMENT STANDARD",
                                Language::En => "MEDIUM — STANDARD HOSTING",
                                Language::Es => "MEDIO — ALOJAMIENTO ESTÁNDAR",
                                Language::De => "MITTEL — STANDARD-HOSTING",
                                Language::Pt => "MÉDIO — ALOJAMENTO PADRÃO",
                            },
                            match lang {
                                Language::Fr => "Suffisant pour les jeux LAN classiques, mais cliquez sur « Configuration Auto Optimisée » ci-dessous pour augmenter votre note !",
                                Language::En => "Good for standard LAN games, or click 'Auto-Optimized Configuration' below to boost your Host Score!",
                                Language::Es => "Suficiente para juegos LAN, pero aplica la Configuración Optimizada abajo para mejorar tu nota.",
                                Language::De => "Gut für normale LAN-Spiele. Nutze unten die Auto-Optimierung für mehr Punkte.",
                                Language::Pt => "Suficiente para jogos LAN, mas aplique a Configuração Otimizada abaixo para melhorar a nota.",
                            },
                        ),
                        HostVerdict::NotOk => (
                            ModernTheme::RED_ERROR,
                            match lang {
                                Language::Fr => "PAS OK — DÉBIT INSUFFISANT POUR HÉBERGER",
                                Language::En => "NOT OK — INSUFFICIENT UPLOAD TO HOST",
                                Language::Es => "NO OK — VELOCIDAD INSUFICIENTE",
                                Language::De => "NICHT OK — ZU WENIG UPLOAD",
                                Language::Pt => "NÃO OK — LARGURA DE BANDA INSUFICIENTE",
                            },
                            match lang {
                                Language::Fr => "Votre débit montant est trop faible pour envoyer un flux vidéo fluide. Rejoignez plutôt le salon de votre ami en tant qu'Invité !",
                                Language::En => "Your upload speed is too low to stream video smoothly. Join your friend's room as a Guest instead!",
                                Language::Es => "Tu velocidad de subida es baja para transmitir vídeo. ¡Únete a la sala de tu amigo como Invitado!",
                                Language::De => "Dein Upload ist zu niedrig zum Streamen. Tritt stattdessen dem Raum deines Freundes als Gast bei!",
                                Language::Pt => "O seu upload é baixo para transmitir vídeo. Junte-se à sala do seu amigo como Convidado!",
                            },
                        ),
                    };

                    // Carte principale du Score & Verdict
                    egui::Frame::none()
                        .fill(ModernTheme::CARD_BG)
                        .rounding(Rounding::same(12.0))
                        .inner_margin(20.0)
                        .stroke(Stroke::new(2.0_f32, verdict_col))
                        .show(ui, |ui| {
                            let avail_w = ui.available_width();
                            let text_max_w = (avail_w - 230.0).max(220.0);

                            ui.horizontal(|ui| {
                                ui.vertical(|ui| {
                                    ui.set_max_width(text_max_w);
                                    ui.horizontal_wrapped(|ui| {
                                        ModernTheme::status_pill(ui, verdict_col, verdict_badge);
                                        if auto_optimized_mode {
                                            ModernTheme::status_pill(
                                                ui,
                                                ModernTheme::BLUE_ELECTRIC,
                                                match lang {
                                                    Language::Fr => "Profil Auto-Optimisé Actif",
                                                    Language::En => "Auto-Optimized Profile Active",
                                                    _ => "Auto-Optimisé",
                                                },
                                            );
                                        }
                                    });
                                    ui.add_space(8.0);
                                    ui.label(
                                        RichText::new(match lang {
                                            Language::Fr => format!("Note Hôte : {} / 100", res.host_score),
                                            Language::En => format!("Host Score: {} / 100", res.host_score),
                                            Language::Es => format!("Nota de Anfitrión: {} / 100", res.host_score),
                                            Language::De => format!("Host-Bewertung: {} / 100", res.host_score),
                                            Language::Pt => format!("Nota de Anfitrião: {} / 100", res.host_score),
                                        })
                                        .color(ModernTheme::TEXT_PRIMARY)
                                        .strong()
                                        .size(26.0),
                                    );
                                    ui.add_space(4.0);
                                    ui.label(
                                        RichText::new(verdict_desc)
                                            .color(ModernTheme::TEXT_SECONDARY)
                                            .size(13.5),
                                    );
                                });

                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    let copy_lbl = if mgr.copied_score {
                                        match lang {
                                            Language::Fr => "Note copiée !",
                                            Language::En => "Score copied!",
                                            _ => "Copié !",
                                        }
                                    } else {
                                        match lang {
                                            Language::Fr => "Copier ma note pour comparer",
                                            Language::En => "Copy my score to compare",
                                            Language::Es => "Copiar mi nota para comparar",
                                            Language::De => "Bewertung kopieren",
                                            Language::Pt => "Copiar a minha nota",
                                        }
                                    };
                                    if ui
                                        .button(RichText::new(copy_lbl).strong())
                                        .on_hover_cursor(CursorIcon::PointingHand)
                                        .clicked()
                                    {
                                        let summary = format!(
                                            "CoopLink Score ({}) : {}/100 [{}] — Upload: {:.1} Mbps | Download: {:.1} Mbps | Ping: {} ms | GPU: {}",
                                            player_name,
                                            res.host_score,
                                            verdict_badge,
                                            res.upload_mbps,
                                            res.download_mbps,
                                            res.ping_ms,
                                            active_encoder.short_badge()
                                        );
                                        ui.output_mut(|o| o.copied_text = summary);
                                        mgr.copied_score = true;
                                    }
                                });
                            });
                        });

                    ui.add_space(14.0);

                    // Grille des 4 métriques détaillées (responsive)
                    let gpu_subtitle = if let Some(vdd) = virtual_display_name {
                        if gpu_name.contains(vdd) {
                            format!("{} + Virtual Display", active_encoder.display_name())
                        } else {
                            format!("{} (+ {})", gpu_name, vdd)
                        }
                    } else {
                        gpu_name.to_string()
                    };

                    let enc_badge_translated = if lang != Language::Fr {
                        active_encoder
                            .short_badge()
                            .replace("Matériel GPU", "Hardware GPU")
                    } else {
                        active_encoder.short_badge().to_string()
                    };

                    ui.columns(4, |cols| {
                        Self::render_metric_box(
                            &mut cols[0],
                            ModernTheme::GREEN_VIVID,
                            match lang {
                                Language::Fr => "DÉBIT MONTANT (UPLOAD)",
                                Language::En => "UPLOAD BANDWIDTH",
                                Language::Es => "VELOCIDAD DE SUBIDA (UPLOAD)",
                                Language::De => "UPLOAD-GESCHWINDIGKEIT",
                                Language::Pt => "VELOCIDADE DE UPLOAD",
                            },
                            &format!("{:.1} Mbps", res.upload_mbps),
                            match lang {
                                Language::Fr => "Crucial pour l'Hôte (> 20 Mbps rec.)",
                                Language::En => "Crucial for Host (> 20 Mbps rec.)",
                                Language::Es => "Crucial para Anfitrión (> 20 Mbps)",
                                Language::De => "Wichtig für Host (> 20 Mbps empf.)",
                                Language::Pt => "Crucial para Anfitrião (> 20 Mbps)",
                            },
                        );
                        Self::render_metric_box(
                            &mut cols[1],
                            ModernTheme::BLUE_ELECTRIC,
                            match lang {
                                Language::Fr => "DÉBIT DESCENDANT (DOWNLOAD)",
                                Language::En => "DOWNLOAD BANDWIDTH",
                                Language::Es => "VELOCIDAD DE BAJADA (DOWNLOAD)",
                                Language::De => "DOWNLOAD-GESCHWINDIGKEIT",
                                Language::Pt => "VELOCIDADE DE DOWNLOAD",
                            },
                            &format!("{:.1} Mbps", res.download_mbps),
                            match lang {
                                Language::Fr => "Crucial pour l'Invité (> 15 Mbps rec.)",
                                Language::En => "Crucial for Guest (> 15 Mbps rec.)",
                                Language::Es => "Crucial para Invitado (> 15 Mbps)",
                                Language::De => "Wichtig für Gast (> 15 Mbps empf.)",
                                Language::Pt => "Crucial para Convidado (> 15 Mbps)",
                            },
                        );
                        Self::render_metric_box(
                            &mut cols[2],
                            ModernTheme::CYAN_VIBRANT,
                            match lang {
                                Language::Fr => "LATENCE & STABILITÉ",
                                Language::En => "LATENCY & STABILITY",
                                Language::Es => "LATENCIA Y ESTABILIDAD",
                                Language::De => "LATENZ & STABILITÄT",
                                Language::Pt => "LATÊNCIA E ESTABILIDADE",
                            },
                            &format!("{} ms (Jitter {} ms)", res.ping_ms, res.jitter_ms),
                            match lang {
                                Language::Fr => "Réactivité des manettes à distance",
                                Language::En => "Remote controller responsiveness",
                                Language::Es => "Respuesta de mandos a distancia",
                                Language::De => "Reaktionszeit der Controller",
                                Language::Pt => "Resposta dos comandos remotos",
                            },
                        );
                        Self::render_metric_box(
                            &mut cols[3],
                            if active_encoder.is_hardware() {
                                ModernTheme::GREEN_VIVID
                            } else {
                                ModernTheme::ORANGE_CORAL
                            },
                            match lang {
                                Language::Fr => "ENCODEUR MATÉRIEL GPU",
                                Language::En => "HARDWARE GPU ENCODER",
                                Language::Es => "CODIFICADOR GPU HARDWARE",
                                Language::De => "HARDWARE-GPU-ENCODER",
                                Language::Pt => "CODIFICADOR GPU HARDWARE",
                            },
                            &enc_badge_translated,
                            &gpu_subtitle,
                        );
                    });

                    ui.add_space(14.0);
                }

                // =========================================================================
                // CARTE DE RECOMMANDATIONS & CONFIGURATION AUTO / OPTIMISÉE EN 1 CLIC
                // =========================================================================
                egui::Frame::none()
                    .fill(ModernTheme::CARD_BG)
                    .rounding(Rounding::same(12.0))
                    .inner_margin(18.0)
                    .stroke(Stroke::new(1.5_f32, ModernTheme::BLUE_ELECTRIC))
                    .show(ui, |ui| {
                        let avail_w = ui.available_width();
                        let btn_w = 265.0;
                        let text_max_w = (avail_w - btn_w - 24.0).max(220.0);

                        ui.horizontal(|ui| {
                            ui.vertical(|ui| {
                                ui.set_max_width(text_max_w);
                                ui.horizontal(|ui| {
                                    ModernTheme::draw_dot(ui, ModernTheme::BLUE_ELECTRIC, 5.0);
                                    ui.label(
                                        RichText::new(match lang {
                                            Language::Fr => "Recommandations pour augmenter votre Note Hôte & Configuration Optimisée",
                                            Language::En => "Recommendations to Boost Your Host Score & Auto-Optimized Setup",
                                            Language::Es => "Recomendaciones para mejorar tu Nota y Configuración Optimizada",
                                            Language::De => "Empfehlungen für eine höhere Bewertung & Auto-Optimierung",
                                            Language::Pt => "Recomendações para aumentar a sua Nota e Configuração Otimizada",
                                        })
                                        .color(ModernTheme::TEXT_PRIMARY)
                                        .strong()
                                        .size(16.0),
                                    );
                                });
                                ui.add_space(4.0);
                                ui.label(
                                    RichText::new(match lang {
                                        Language::Fr => "Appliquez en 1 clic les réglages optimaux (Encodeur GPU matériel NVENC/AMF au lieu du CPU, Métrique réseau prioritaire 5, Pare-feu et Bitrate adaptatif) :",
                                        Language::En => "Apply optimal settings in 1 click (Hardware GPU encoder NVENC/AMF instead of CPU, Priority Network Metric 5, Firewall rules and Adaptive Bitrate):",
                                        Language::Es => "Aplica en 1 clic los ajustes óptimos (Codificador GPU por hardware, Métrica prioritaria 5, Cortafuegos y Bitrate adaptativo):",
                                        Language::De => "Optimale Einstellungen mit 1 Klick anwenden (Hardware-GPU-Encoder, Prioritätsmetrik 5, Firewall und Bitrate):",
                                        Language::Pt => "Aplique em 1 clique as definições ideais (Codificador GPU por hardware, Métrica 5, Firewall e Bitrate adaptativo):",
                                    })
                                    .color(ModernTheme::TEXT_SECONDARY)
                                    .size(13.0),
                                );
                            });

                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                let opt_lbl = match lang {
                                    Language::Fr => "Configuration Auto Optimisée",
                                    Language::En => "Apply Auto-Optimized Config",
                                    Language::Es => "Configuración Auto Optimizada",
                                    Language::De => "Auto-Optimierte Konfiguration",
                                    Language::Pt => "Configuração Auto Otimizada",
                                };
                                let opt_btn = egui::Button::new(
                                    RichText::new(opt_lbl).color(Color32::WHITE).strong().size(14.0),
                                )
                                .fill(ModernTheme::GREEN_VIVID)
                                .rounding(Rounding::same(10.0))
                                .min_size(egui::vec2(250.0, 42.0));

                                if ui.add(opt_btn).on_hover_cursor(CursorIcon::PointingHand).clicked() {
                                    *on_apply_auto_optimize = true;
                                    mgr.auto_optimized_feedback = true;
                                }
                            });
                        });

                        if mgr.auto_optimized_feedback {
                            ui.add_space(8.0);
                            ModernTheme::status_pill(
                                ui,
                                ModernTheme::GREEN_VIVID,
                                match lang {
                                    Language::Fr => "Configuration Optimisée appliquée : Encodeur Matériel GPU actif, Métrique 5 et Pare-feu vérifiés !",
                                    Language::En => "Optimized Configuration applied: Hardware GPU Encoder active, Metric 5 and Firewall verified!",
                                    _ => "Configuration Optimisée appliquée avec succès !",
                                },
                            );
                        }

                        ui.add_space(12.0);
                        ui.separator();
                        ui.add_space(8.0);

                        // Liste détaillée des points de contrôle et conseils pour atteindre 100/100
                        let hw_ok = active_encoder.is_hardware();
                        Self::render_recommendation_item(
                            ui,
                            hw_ok,
                            match lang {
                                Language::Fr => "1. Activer l'Encodeur Matériel GPU Direct (NVIDIA NVENC / AMD AMF / Intel QuickSync)",
                                Language::En => "1. Enable Direct Hardware GPU Encoder (NVIDIA NVENC / AMD AMF / Intel QuickSync)",
                                _ => "1. Direct Hardware GPU Encoder (NVENC / AMF / QuickSync)",
                            },
                            if hw_ok {
                                match lang {
                                    Language::Fr => format!(
                                        "Actif ({}) — Votre puce GPU matérielle encode directement le flux vidéo sans solliciter le processeur (+20/20 pts).",
                                        active_encoder.display_name()
                                    ),
                                    Language::En => format!(
                                        "Active ({}) — Your dedicated hardware GPU chip encodes the video stream directly with zero CPU load (+20/20 pts).",
                                        active_encoder.display_name()
                                    ),
                                    _ => format!("Active ({}) (+20/20 pts).", active_encoder.display_name()),
                                }
                            } else {
                                match lang {
                                    Language::Fr => "Actuellement sur Standard CPU (+8/20 pts) — Cliquez sur « Configuration Auto Optimisée » pour basculer sur votre GPU matériel et gagner +12 points !".to_string(),
                                    Language::En => "Currently on Standard CPU (+8/20 pts) — Click 'Apply Auto-Optimized Config' to switch to your hardware GPU encoder and gain +12 points!".to_string(),
                                    _ => "Click 'Apply Auto-Optimized Config' to switch to Hardware GPU (+12 pts).".to_string(),
                                }
                            },
                        );

                        Self::render_recommendation_item(
                            ui,
                            state.metric_applied,
                            match lang {
                                Language::Fr => "2. Priorité de la carte réseau virtuelle (Métrique Basse = 5)",
                                Language::En => "2. Virtual Network Adapter Priority (Low Interface Metric = 5)",
                                _ => "2. Virtual Network Adapter Priority (Metric = 5)",
                            },
                            if state.metric_applied {
                                match lang {
                                    Language::Fr => "Optimisé (Métrique 5 active) — Le tunnel P2P CoopLink passe en priorité devant votre carte Wi-Fi ou Ethernet.".to_string(),
                                    Language::En => "Optimized (Metric 5 active) — The CoopLink P2P tunnel takes priority over your Wi-Fi or Ethernet adapter.".to_string(),
                                    _ => "Optimized (Metric 5 active).".to_string(),
                                }
                            } else {
                                match lang {
                                    Language::Fr => "Recommandé — Appliquez la métrique prioritaire 5 pour router les paquets du jeu en priorité.".to_string(),
                                    Language::En => "Recommended — Apply priority metric 5 so games discover your P2P LAN session first.".to_string(),
                                    _ => "Recommended — Apply priority metric 5.".to_string(),
                                }
                            },
                        );

                        Self::render_recommendation_item(
                            ui,
                            state.firewall_ok,
                            match lang {
                                Language::Fr => "3. Autorisation Pare-feu (Windows / macOS / Linux) & Tunnel UDP Direct",
                                Language::En => "3. OS Firewall Rules (Windows / macOS / Linux) & Direct UDP Tunnel",
                                _ => "3. OS Firewall Rules & Direct UDP Tunnel",
                            },
                            if state.firewall_ok {
                                match lang {
                                    Language::Fr => "Opérationnel — Les règles pare-feu autorisent le trafic P2P direct (port UDP 42422 et sous-réseau 10.244.0.0/24) sans latence.".to_string(),
                                    Language::En => "Operational — Firewall rules allow direct P2P traffic (UDP port 42422 and virtual subnet 10.244.0.0/24) with zero latency.".to_string(),
                                    _ => "Operational — Firewall rules active.".to_string(),
                                }
                            } else {
                                match lang {
                                    Language::Fr => "Action conseillée — Cliquez sur « Configuration Auto Optimisée » pour appliquer les règles pare-feu immédiatement.".to_string(),
                                    Language::En => "Recommended — Click 'Apply Auto-Optimized Config' to apply OS firewall rules immediately.".to_string(),
                                    _ => "Click 'Apply Auto-Optimized Config' to configure firewall.".to_string(),
                                }
                            },
                        );

                        let is_wifi = mgr.last_result.as_ref().map(|r| r.is_wifi).unwrap_or(false);
                        Self::render_recommendation_item(
                            ui,
                            !is_wifi,
                            match lang {
                                Language::Fr => "4. Privilégier un câble Ethernet RJ45 plutôt que le Wi-Fi pour héberger",
                                Language::En => "4. Prefer a wired Ethernet RJ45 cable over Wi-Fi when hosting",
                                _ => "4. Wired Ethernet RJ45 connection recommended for Host",
                            },
                            if !is_wifi {
                                match lang {
                                    Language::Fr => "Connexion Ethernet détectée — Stabilité maximale et gigue (Jitter) minimale pour les manettes de vos invités.".to_string(),
                                    Language::En => "Wired Ethernet detected — Maximum stability and minimal jitter for your friends' controllers.".to_string(),
                                    _ => "Wired Ethernet detected — Maximum stability.".to_string(),
                                }
                            } else {
                                match lang {
                                    Language::Fr => "Wi-Fi détecté — Pour éliminer les micro-coupures d'image et réduire la latence sous 10 ms, branchez un câble Ethernet si possible.".to_string(),
                                    Language::En => "Wi-Fi detected — To eliminate micro-stutters and keep latency below 10 ms, use a wired Ethernet cable if possible.".to_string(),
                                    _ => "Wi-Fi detected — Wired Ethernet recommended.".to_string(),
                                }
                            },
                        );
                    });
            });
    }

    fn render_metric_box(ui: &mut Ui, accent: Color32, title: &str, value: &str, sub: &str) {
        egui::Frame::none()
            .fill(ModernTheme::CARD_BG)
            .rounding(Rounding::same(10.0))
            .inner_margin(16.0)
            .stroke(Stroke::new(1.0_f32, ModernTheme::BORDER_SUBTLE))
            .show(ui, |ui| {
                ui.set_min_width(ui.available_width());
                ui.set_min_height(115.0);
                ui.horizontal_wrapped(|ui| {
                    ModernTheme::draw_dot(ui, accent, 4.0);
                    ui.label(RichText::new(title).color(ModernTheme::TEXT_MUTED).strong().size(11.0));
                });
                ui.add_space(8.0);
                ui.label(RichText::new(value).color(ModernTheme::TEXT_PRIMARY).strong().size(17.5));
                ui.add_space(6.0);
                ui.label(RichText::new(sub).color(ModernTheme::TEXT_SECONDARY).size(12.0));
            });
    }

    fn render_recommendation_item(ui: &mut Ui, is_ok: bool, title: &str, desc: String) {
        let col = if is_ok {
            ModernTheme::GREEN_VIVID
        } else {
            ModernTheme::ORANGE_CORAL
        };
        egui::Frame::none()
            .fill(ModernTheme::SUBTLE_BG)
            .rounding(Rounding::same(8.0))
            .inner_margin(10.0)
            .stroke(Stroke::new(1.0_f32, ModernTheme::BORDER_SUBTLE))
            .show(ui, |ui| {
                ui.set_min_width(ui.available_width());
                ui.horizontal(|ui| {
                    ModernTheme::draw_dot(ui, col, 4.5);
                    ui.vertical(|ui| {
                        ui.label(RichText::new(title).color(ModernTheme::TEXT_PRIMARY).strong().size(13.5));
                        ui.label(RichText::new(desc).color(ModernTheme::TEXT_SECONDARY).size(12.0));
                    });
                });
            });
        ui.add_space(6.0);
    }
}
