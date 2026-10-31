use crate::i18n::Language;
use crate::streaming::moonlight_client::{StreamDisplayMode, VideoCodec};
use crate::streaming::sunshine_host::{HardwareEncoder, VirtualGamepadType};
use crate::streaming::{CoopInputMode, StreamingEngine, StreamingRoleTab};
use crate::types::{AppState, NodeRole};
use crate::ui::retro_theme::ModernTheme;
use egui::{Color32, RichText, Rounding, Stroke, Ui};

pub struct StreamingView;

impl StreamingView {
    pub fn render(
        ui: &mut Ui,
        lang: Language,
        state: &AppState,
        engine: &mut StreamingEngine,
        on_start_host_stream: &mut bool,
        on_stop_host_stream: &mut bool,
        on_start_client_stream: &mut bool,
        on_stop_client_stream: &mut bool,
    ) {
        // Synchronise automatiquement l'onglet selon le rôle dans le salon si actif
        if state.role == NodeRole::Host {
            engine.role_tab = StreamingRoleTab::HostShareMyGame;
        } else if state.role == NodeRole::Client {
            engine.role_tab = StreamingRoleTab::GuestJoinFriendGame;
        }

        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                // =========================================================================
                // BANNIÈRE EXPLICATIVE SIMPLE + SÉLECTEUR DE RÔLE (HÔTE vs INVITÉ)
                // =========================================================================
                egui::Frame::none()
                    .fill(ModernTheme::CARD_BG)
                    .rounding(Rounding::same(12.0))
                    .inner_margin(18.0)
                    .stroke(Stroke::new(1.0_f32, ModernTheme::BORDER_SUBTLE))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.vertical(|ui| {
                                ui.label(
                                    RichText::new(match lang {
                                        Language::Fr => "Jouez ensemble sur le même écran, à distance",
                                        Language::En => "Play together on the same screen, remotely",
                                        Language::Es => "Jueguen juntos en la misma pantalla a distancia",
                                        Language::De => "Gemeinsam auf demselben Bildschirm aus der Ferne spielen",
                                        Language::Pt => "Joguem juntos no mesmo ecrã à distância",
                                    })
                                    .color(ModernTheme::TEXT_PRIMARY)
                                    .strong()
                                    .size(18.0),
                                );
                                ui.label(
                                    RichText::new(match lang {
                                        Language::Fr => "Un seul joueur a besoin de posséder et lancer le jeu (l'Hôte). Les amis rejoignent son écran en temps réel avec leur manette ou leur clavier/souris.",
                                        Language::En => "Only one player needs to own and launch the game (the Host). Friends join their screen in real time using their controller or keyboard/mouse.",
                                        Language::Es => "Solo un jugador necesita tener el juego (el Anfitrión). Tus amigos se unen a tu pantalla en tiempo real con su mando o teclado/ratón.",
                                        Language::De => "Nur ein Spieler muss das Spiel besitzen (der Host). Freunde treten dem Bildschirm in Echtzeit mit Controller oder Tastatur/Maus bei.",
                                        Language::Pt => "Apenas um jogador precisa de ter o jogo (o Anfitrião). Os amigos juntam-se ao ecrã em tempo real com comando ou teclado/rato.",
                                    })
                                    .color(ModernTheme::TEXT_SECONDARY)
                                    .size(13.5),
                                );
                            });

                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                if engine.sunshine_host.is_running || engine.moonlight_client.is_streaming {
                                    ModernTheme::status_pill(
                                        ui,
                                        ModernTheme::GREEN_VIVID,
                                        match lang {
                                            Language::Fr => "Partage en cours",
                                            Language::En => "Sharing Active",
                                            Language::Es => "Compartiendo",
                                            Language::De => "Sharing Aktiv",
                                            Language::Pt => "Partilha Ativa",
                                        },
                                    );
                                } else {
                                    ModernTheme::status_pill(
                                        ui,
                                        ModernTheme::GREEN_VIVID,
                                        match lang {
                                            Language::Fr => "Prêt à partager",
                                            Language::En => "Ready to share",
                                            Language::Es => "Listo para compartir",
                                            Language::De => "Bereit zum Teilen",
                                            Language::Pt => "Pronto a partilhar",
                                        },
                                    );
                                }
                            });
                        });

                        ui.add_space(14.0);

                        // Sélecteur visuel : Hôte vs Invité
                        ui.columns(2, |cols| {
                            let is_host_tab = engine.role_tab == StreamingRoleTab::HostShareMyGame;
                            let host_btn_text = match lang {
                                Language::Fr => "1. Je suis l'Hôte (Je partage mon jeu avec mes amis)",
                                Language::En => "1. I am the Host (I share my game with friends)",
                                Language::Es => "1. Soy el Anfitrión (Comparto mi juego con amigos)",
                                Language::De => "1. Ich bin der Host (Ich teile mein Spiel)",
                                Language::Pt => "1. Sou o Anfitrião (Partilho o meu jogo)",
                            };
                            let host_btn = egui::Button::new(
                                RichText::new(host_btn_text)
                                    .color(if is_host_tab { Color32::WHITE } else { ModernTheme::TEXT_PRIMARY })
                                    .strong()
                                    .size(14.0),
                            )
                            .fill(if is_host_tab { ModernTheme::BLUE_ELECTRIC } else { ModernTheme::SUBTLE_BG })
                            .stroke(Stroke::new(
                                1.5_f32,
                                if is_host_tab { ModernTheme::BLUE_ELECTRIC } else { ModernTheme::BORDER_SUBTLE },
                            ))
                            .rounding(Rounding::same(10.0))
                            .min_size(egui::vec2(cols[0].available_width(), 44.0));

                            if cols[0].add(host_btn).clicked() {
                                engine.role_tab = StreamingRoleTab::HostShareMyGame;
                            }

                            let is_guest_tab = engine.role_tab == StreamingRoleTab::GuestJoinFriendGame;
                            let guest_btn_text = match lang {
                                Language::Fr => "2. Je suis l'Invité (Je rejoins l'écran de mon ami)",
                                Language::En => "2. I am the Guest (I join my friend's screen)",
                                Language::Es => "2. Soy el Invitado (Me uno a la pantalla de mi amigo)",
                                Language::De => "2. Ich bin Gast (Ich trete dem Bildschirm bei)",
                                Language::Pt => "2. Sou Convidado (Junto-me ao ecrã do meu amigo)",
                            };
                            let guest_btn = egui::Button::new(
                                RichText::new(guest_btn_text)
                                    .color(if is_guest_tab { Color32::WHITE } else { ModernTheme::TEXT_PRIMARY })
                                    .strong()
                                    .size(14.0),
                            )
                            .fill(if is_guest_tab { ModernTheme::INDIGO_ACCENT } else { ModernTheme::SUBTLE_BG })
                            .stroke(Stroke::new(
                                1.5_f32,
                                if is_guest_tab { ModernTheme::INDIGO_ACCENT } else { ModernTheme::BORDER_SUBTLE },
                            ))
                            .rounding(Rounding::same(10.0))
                            .min_size(egui::vec2(cols[1].available_width(), 44.0));

                            if cols[1].add(guest_btn).clicked() {
                                engine.role_tab = StreamingRoleTab::GuestJoinFriendGame;
                            }
                        });
                    });

                ui.add_space(14.0);

                match engine.role_tab {
                    StreamingRoleTab::HostShareMyGame => {
                        Self::render_host_workflow(
                            ui,
                            lang,
                            state,
                            engine,
                            on_start_host_stream,
                            on_stop_host_stream,
                        );
                    }
                    StreamingRoleTab::GuestJoinFriendGame => {
                        Self::render_guest_workflow(
                            ui,
                            lang,
                            state,
                            engine,
                            on_start_client_stream,
                            on_stop_client_stream,
                        );
                    }
                }
            });
    }

    fn render_host_workflow(
        ui: &mut Ui,
        lang: Language,
        _state: &AppState,
        engine: &mut StreamingEngine,
        on_start_host_stream: &mut bool,
        on_stop_host_stream: &mut bool,
    ) {
        // =========================================================================
        // ÉTAPE 1 : SÉLECTION DU JEU OU DE L'ÉCRAN À PARTAGER
        // =========================================================================
        egui::Frame::none()
            .fill(ModernTheme::CARD_BG)
            .rounding(Rounding::same(12.0))
            .inner_margin(18.0)
            .stroke(Stroke::new(1.0_f32, ModernTheme::BORDER_SUBTLE))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ModernTheme::draw_dot(ui, ModernTheme::BLUE_ELECTRIC, 5.0);
                    ui.label(
                        RichText::new(match lang {
                            Language::Fr => "Étape 1 — Choisissez le jeu ou l'écran à partager",
                            Language::En => "Step 1 — Select the game or screen to share",
                            Language::Es => "Paso 1 — Elige el juego o la pantalla a compartir",
                            Language::De => "Schritt 1 — Spiel oder Bildschirm zum Teilen wählen",
                            Language::Pt => "Passo 1 — Escolha o jogo ou ecrã a partilhar",
                        })
                        .color(ModernTheme::BLUE_ELECTRIC)
                        .strong()
                        .size(16.0),
                    );
                });
                ui.add_space(6.0);
                ui.label(
                    RichText::new(match lang {
                        Language::Fr => "Sélectionnez un jeu déjà ouvert dans la liste, choisissez son fichier .exe sur votre PC, ou partagez votre écran complet :",
                        Language::En => "Pick an already running game from the list, browse for its .exe file on your PC, or share your full desktop screen:",
                        Language::Es => "Selecciona un juego abierto en la lista, busca su archivo .exe en tu PC o comparte la pantalla completa:",
                        Language::De => "Wähle ein laufendes Spiel aus der Liste, suche die .exe-Datei oder teile deinen gesamten Bildschirm:",
                        Language::Pt => "Selecione um jogo aberto na lista, procure o ficheiro .exe no PC ou partilhe o ecrã inteiro:",
                    })
                    .color(ModernTheme::TEXT_SECONDARY)
                    .size(13.0),
                );

                ui.add_space(10.0);

                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(match lang {
                            Language::Fr => "Jeu / Fenêtre cible :",
                            Language::En => "Target Game / Window:",
                            Language::Es => "Juego / Ventana:",
                            Language::De => "Zielspiel / Fenster:",
                            Language::Pt => "Jogo / Janela alvo:",
                        })
                        .strong(),
                    );

                    egui::ComboBox::from_id_salt("host_game_selector")
                        .selected_text(RichText::new(&engine.selected_app_name).strong().color(ModernTheme::BLUE_ELECTRIC))
                        .width(340.0)
                        .show_ui(ui, |ui| {
                            let windows = engine.detected_windows.clone();
                            for w in windows {
                                ui.selectable_value(&mut engine.selected_app_name, w.clone(), w);
                            }
                        });

                    if ui
                        .button(match lang {
                            Language::Fr => "Actualiser les jeux ouverts",
                            Language::En => "Refresh running games",
                            Language::Es => "Actualizar juegos abiertos",
                            Language::De => "Laufende Spiele aktualisieren",
                            Language::Pt => "Atualizar jogos abertos",
                        })
                        .clicked()
                    {
                        engine.refresh_running_games();
                    }

                    let browse_btn = egui::Button::new(
                        RichText::new(match lang {
                            Language::Fr => "Parcourir un jeu (.exe)...",
                            Language::En => "Browse game (.exe)...",
                            Language::Es => "Buscar juego (.exe)...",
                            Language::De => "Spiel durchsuchen (.exe)...",
                            Language::Pt => "Procurar jogo (.exe)...",
                        })
                        .color(ModernTheme::BLUE_ELECTRIC)
                        .strong(),
                    )
                    .fill(Color32::from_rgb(239, 246, 255))
                    .stroke(Stroke::new(1.0_f32, ModernTheme::BLUE_ELECTRIC))
                    .rounding(Rounding::same(8.0));

                    if ui.add(browse_btn).clicked() {
                        engine.browse_game_executable();
                    }
                });

                if !engine.custom_exe_path.is_empty() {
                    ui.add_space(8.0);
                    egui::Frame::none()
                        .fill(ModernTheme::SUBTLE_BG)
                        .rounding(Rounding::same(8.0))
                        .inner_margin(10.0)
                        .stroke(Stroke::new(1.0_f32, ModernTheme::BORDER_SUBTLE))
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ModernTheme::draw_dot(ui, ModernTheme::GREEN_VIVID, 4.0);
                                ui.label(
                                    RichText::new(format!("Exécutable sélectionné : {}", engine.custom_exe_path))
                                        .color(ModernTheme::TEXT_PRIMARY)
                                        .size(12.5),
                                );
                                if ui
                                    .button(match lang {
                                        Language::Fr => "Lancer le jeu maintenant",
                                        Language::En => "Launch game now",
                                        Language::Es => "Iniciar juego ahora",
                                        Language::De => "Spiel jetzt starten",
                                        Language::Pt => "Iniciar jogo agora",
                                    })
                                    .clicked()
                                {
                                    let _ = std::process::Command::new(&engine.custom_exe_path).spawn();
                                }
                            });
                        });
                }
            });

        ui.add_space(14.0);

        // =========================================================================
        // ÉTAPE 2 : COMMENT JOUEZ-VOUS ENSEMBLE ? (2 MANETTES vs 2 CLAVIERS/SOURIS)
        // =========================================================================
        egui::Frame::none()
            .fill(ModernTheme::CARD_BG)
            .rounding(Rounding::same(12.0))
            .inner_margin(18.0)
            .stroke(Stroke::new(1.0_f32, ModernTheme::BORDER_SUBTLE))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ModernTheme::draw_dot(ui, ModernTheme::GREEN_VIVID, 5.0);
                    ui.label(
                        RichText::new(match lang {
                            Language::Fr => "Étape 2 — Comment jouez-vous à 2 (ou jusqu'à 4) ?",
                            Language::En => "Step 2 — How are you playing together (2 to 4 players)?",
                            Language::Es => "Paso 2 — ¿Cómo van a jugar juntos (2 a 4 jugadores)?",
                            Language::De => "Schritt 2 — Wie spielt ihr zusammen (2 bis 4 Spieler)?",
                            Language::Pt => "Passo 2 — Como vão jogar juntos (2 a 4 jogadores)?",
                        })
                        .color(ModernTheme::GREEN_VIVID)
                        .strong()
                        .size(16.0),
                    );
                });
                ui.add_space(8.0);

                // Carte 1 : 2 à 4 Manettes
                Self::render_input_mode_card(
                    ui,
                    engine.coop_input_mode == CoopInputMode::MultiGamepad,
                    ModernTheme::GREEN_VIVID,
                    match lang {
                        Language::Fr => "Mode A — Avec des Manettes (2 à 4 Manettes indépendantes)",
                        Language::En => "Mode A — With Controllers (2 to 4 Independent Gamepads)",
                        Language::Es => "Modo A — Con Mandos (2 a 4 Mandos independientes)",
                        Language::De => "Modus A — Mit Controllern (2 bis 4 unabhängige Gamepads)",
                        Language::Pt => "Modo A — Com Comandos (2 a 4 Comandos independentes)",
                    },
                    match lang {
                        Language::Fr => "Vous êtes le Joueur 1. Dès que votre ami connecte sa manette, CoopLink crée une 2e manette virtuelle (Joueur 2) sur votre PC. Chacun contrôle son personnage sans aucun conflit.",
                        Language::En => "You are Player 1. As soon as your friend connects their controller, CoopLink creates a 2nd virtual controller (Player 2) on your PC. Everyone controls their own character with zero conflict.",
                        Language::Es => "Tú eres el Jugador 1. Tu amigo se asigna automáticamente como Mando Jugador 2 (y Jugador 3/4). Cada uno controla su propio personaje sin conflictos.",
                        Language::De => "Du bist Spieler 1. Dein Freund wird automatisch als virtueller Controller Spieler 2 zugewiesen. Jeder steuert seinen eigenen Charakter ohne Konflikte.",
                        Language::Pt => "Você é o Jogador 1. O seu amigo é atribuído automaticamente como Comando Jogador 2. Cada um controla a sua personagem sem conflitos.",
                    },
                    || {
                        engine.coop_input_mode = CoopInputMode::MultiGamepad;
                        engine.moonlight_client.capture_mouse_keyboard = false;
                    },
                );

                ui.add_space(8.0);

                // Carte 2 : 2 Claviers / Souris (Conversion intelligente en Manette J2)
                Self::render_input_mode_card(
                    ui,
                    engine.coop_input_mode == CoopInputMode::KeyboardToVirtualGamepad,
                    ModernTheme::BLUE_ELECTRIC,
                    match lang {
                        Language::Fr => "Mode B — Vous êtes tous les 2 au Clavier / Souris (Conversion Manette J2)",
                        Language::En => "Mode B — Both of you use Keyboard & Mouse (Smart Player 2 Emulation)",
                        Language::Es => "Modo B — Ambos usan Teclado y Ratón (Conversión a Mando Jugador 2)",
                        Language::De => "Modus B — Beide nutzen Tastatur & Maus (Emulation als Controller 2)",
                        Language::Pt => "Modo B — Ambos usam Teclado e Rato (Conversão para Comando Jogador 2)",
                    },
                    match lang {
                        Language::Fr => "Windows ne possède qu'un seul curseur de souris physique. Pour que votre ami ne bouge pas votre souris pendant que vous jouez : vous jouez au Clavier/Souris (Joueur 1) et le Clavier/Souris de votre ami (ZQSD, Espace, Souris) est converti en Manette Joueur 2 !",
                        Language::En => "Windows only has one physical mouse cursor. So your friend doesn't hijack your cursor: you play with Keyboard/Mouse (Player 1) while your friend's Keyboard/Mouse (WASD, Space, Mouse) is automatically converted into Player 2's Controller!",
                        Language::Es => "Windows solo tiene un cursor de ratón. Tú juegas con tu Teclado/Ratón (Jugador 1) y el Teclado/Ratón de tu amigo se convierte automáticamente en el Mando del Jugador 2.",
                        Language::De => "Windows hat nur einen Mauszeiger. Du spielst mit Tastatur/Maus (Spieler 1) und die Tastatur/Maus deines Freundes wird automatisch in den Controller für Spieler 2 umgewandelt.",
                        Language::Pt => "O Windows só tem um cursor de rato. Você joga com Teclado/Rato (Jogador 1) e o Teclado/Rato do seu amigo é convertido automaticamente no Comando do Jogador 2.",
                    },
                    || {
                        engine.coop_input_mode = CoopInputMode::KeyboardToVirtualGamepad;
                        engine.moonlight_client.capture_mouse_keyboard = false;
                    },
                );

                ui.add_space(8.0);

                // Carte 3 : Clavier / Souris Partagé
                Self::render_input_mode_card(
                    ui,
                    engine.coop_input_mode == CoopInputMode::SharedKeyboardMouse,
                    ModernTheme::ORANGE_CORAL,
                    match lang {
                        Language::Fr => "Mode C — Clavier & Souris Partagés (Même clavier ou Tour par tour)",
                        Language::En => "Mode C — Shared Keyboard & Mouse (Same keyboard or Turn-based games)",
                        Language::Es => "Modo C — Teclado y Ratón Compartidos (Mismo teclado o por turnos)",
                        Language::De => "Modus C — Geteilte Tastatur & Maus (Rundenbasierte Spiele)",
                        Language::Pt => "Modo C — Teclado e Rato Partilhados (Mesmo teclado ou por turnos)",
                    },
                    match lang {
                        Language::Fr => "Votre ami peut aussi déplacer le curseur et appuyer sur les touches de votre clavier (idéal pour les jeux au tour par tour, les jeux de stratégie, ou si Joueur 1 joue avec ZQSD et Joueur 2 avec les Flèches).",
                        Language::En => "Your friend can also move the mouse cursor and press keyboard keys directly (ideal for turn-based games, strategy games, or when Player 1 uses WASD and Player 2 uses Arrow keys).",
                        Language::Es => "Tu amigo también puede mover el ratón y pulsar teclas directamente (ideal para juegos por turnos o cuando J1 usa WASD y J2 las Flechas).",
                        Language::De => "Dein Freund kann auch den Mauszeiger bewegen und Tasten drücken (ideal für rundenbasierte Spiele oder wenn S1 WASD und S2 Pfeiltasten nutzt).",
                        Language::Pt => "O seu amigo também pode mover o rato e premir teclas diretamente (ideal para jogos por turnos ou quando J1 usa WASD e J2 usa as Setas).",
                    },
                    || {
                        engine.coop_input_mode = CoopInputMode::SharedKeyboardMouse;
                        engine.moonlight_client.capture_mouse_keyboard = true;
                    },
                );
            });

        ui.add_space(14.0);

        // =========================================================================
        // ÉTAPE 3 : DÉMARRER LE PARTAGE & VOIR LES JOUEURS BRANCHÉS
        // =========================================================================
        egui::Frame::none()
            .fill(ModernTheme::CARD_BG)
            .rounding(Rounding::same(12.0))
            .inner_margin(18.0)
            .stroke(Stroke::new(1.5_f32, ModernTheme::BLUE_ELECTRIC))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        ui.horizontal(|ui| {
                            ModernTheme::draw_dot(ui, ModernTheme::BLUE_ELECTRIC, 5.0);
                            ui.label(
                                RichText::new(match lang {
                                    Language::Fr => "Étape 3 — Lancer la session d'écran partagé",
                                    Language::En => "Step 3 — Start the shared screen session",
                                    Language::Es => "Paso 3 — Iniciar la sesión de pantalla compartida",
                                    Language::De => "Schritt 3 — Bildschirm-Sharing starten",
                                    Language::Pt => "Passo 3 — Iniciar a sessão de ecrã partilhado",
                                })
                                .color(ModernTheme::TEXT_PRIMARY)
                                .strong()
                                .size(16.0),
                            );
                        });
                        ui.label(
                            RichText::new(format!(
                                "Source active : {}  |  GPU détecté : {}",
                                engine.selected_app_name, engine.sunshine_host.detected_gpu_name
                            ))
                            .color(ModernTheme::TEXT_SECONDARY)
                            .size(13.0),
                        );
                    });

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if !engine.sunshine_host.is_running {
                            let start_btn = egui::Button::new(
                                RichText::new(match lang {
                                    Language::Fr => "Démarrer le Partage d'Écran",
                                    Language::En => "Start Screen Sharing",
                                    Language::Es => "Iniciar Pantalla Compartida",
                                    Language::De => "Bildschirmfreigabe starten",
                                    Language::Pt => "Iniciar Partilha de Ecrã",
                                })
                                .color(Color32::WHITE)
                                .strong()
                                .size(15.0),
                            )
                            .fill(ModernTheme::GREEN_VIVID)
                            .rounding(Rounding::same(10.0))
                            .min_size(egui::vec2(240.0, 44.0));

                            if ui.add(start_btn).clicked() {
                                *on_start_host_stream = true;
                            }
                        } else {
                            let stop_btn = egui::Button::new(
                                RichText::new(match lang {
                                    Language::Fr => "Arrêter le Partage",
                                    Language::En => "Stop Sharing",
                                    Language::Es => "Detener Pantalla Compartida",
                                    Language::De => "Freigabe beenden",
                                    Language::Pt => "Parar Partilha",
                                })
                                .color(Color32::WHITE)
                                .strong()
                                .size(15.0),
                            )
                            .fill(ModernTheme::RED_ERROR)
                            .rounding(Rounding::same(10.0))
                            .min_size(egui::vec2(200.0, 44.0));

                            if ui.add(stop_btn).clicked() {
                                *on_stop_host_stream = true;
                            }
                        }
                    });
                });

                ui.add_space(12.0);
                ui.separator();
                ui.add_space(8.0);

                // Résumé visuel des 4 emplacements de joueurs
                ui.label(
                    RichText::new(match lang {
                        Language::Fr => "Attribution automatique des joueurs sur votre PC :",
                        Language::En => "Automatic player slot assignment on your PC:",
                        Language::Es => "Asignación automática de jugadores en tu PC:",
                        Language::De => "Automatische Spielerzuweisung auf deinem PC:",
                        Language::Pt => "Atribuição automática de jogadores no seu PC:",
                    })
                    .color(ModernTheme::TEXT_SECONDARY)
                    .strong()
                    .size(13.0),
                );
                ui.add_space(6.0);

                ui.columns(4, |cols| {
                    for slot_num in 1..=4 {
                        let existing = engine
                            .sunshine_host
                            .controller_slots
                            .iter()
                            .find(|s| s.player_number == slot_num);

                        let (border_col, title, desc) = if let Some(s) = existing {
                            let col = if s.is_local_host {
                                ModernTheme::BLUE_ELECTRIC
                            } else {
                                ModernTheme::GREEN_VIVID
                            };
                            (
                                col,
                                format!("Joueur {} (Slot #{})", slot_num, s.xinput_slot_index),
                                s.owner_name.clone(),
                            )
                        } else {
                            (
                                ModernTheme::BORDER_SUBTLE,
                                format!("Joueur {} (Slot #{})", slot_num, slot_num - 1),
                                match lang {
                                    Language::Fr => "En attente d'un ami...".to_string(),
                                    Language::En => "Waiting for a friend...".to_string(),
                                    Language::Es => "Esperando a un amigo...".to_string(),
                                    Language::De => "Warte auf Freund...".to_string(),
                                    Language::Pt => "À espera de um amigo...".to_string(),
                                },
                            )
                        };

                        egui::Frame::none()
                            .fill(ModernTheme::SUBTLE_BG)
                            .rounding(Rounding::same(8.0))
                            .inner_margin(10.0)
                            .stroke(Stroke::new(1.0_f32, border_col))
                            .show(&mut cols[(slot_num - 1) as usize], |ui| {
                                ui.horizontal(|ui| {
                                    ModernTheme::draw_dot(ui, border_col, 4.0);
                                    ui.label(RichText::new(title).color(ModernTheme::TEXT_PRIMARY).strong().size(12.5));
                                });
                                ui.label(RichText::new(desc).color(ModernTheme::TEXT_SECONDARY).size(12.0));
                            });
                    }
                });

                ui.add_space(10.0);

                // Volet repliable pour ne pas perdre l'utilisateur avec les réglages techniques
                ui.collapsing(
                    RichText::new(match lang {
                        Language::Fr => "Paramètres vidéo & manettes avancés (Optionnel)",
                        Language::En => "Advanced video & controller settings (Optional)",
                        Language::Es => "Ajustes avanzados de vídeo y mandos (Opcional)",
                        Language::De => "Erweiterte Video- & Controller-Einstellungen (Optional)",
                        Language::Pt => "Definições avançadas de vídeo e comando (Opcional)",
                    })
                    .color(ModernTheme::TEXT_SECONDARY)
                    .size(13.0),
                    |ui| {
                        ui.add_space(6.0);
                        ui.horizontal(|ui| {
                            ui.label("Qualité vidéo :");
                            let is_1080 = engine.sunshine_host.target_height == 1080 && engine.sunshine_host.target_fps == 60;
                            if ui.selectable_label(is_1080, "1080p @ 60 FPS (Recommandé)").clicked() {
                                engine.sunshine_host.target_width = 1920;
                                engine.sunshine_host.target_height = 1080;
                                engine.sunshine_host.target_fps = 60;
                            }
                            let is_1440 = engine.sunshine_host.target_height == 1440 && engine.sunshine_host.target_fps == 120;
                            if ui.selectable_label(is_1440, "1440p @ 120 FPS (Ultra Fluide)").clicked() {
                                engine.sunshine_host.target_width = 2560;
                                engine.sunshine_host.target_height = 1440;
                                engine.sunshine_host.target_fps = 120;
                            }

                            ui.add_space(16.0);
                            ui.label("Encodeur GPU :");
                            for enc in [
                                HardwareEncoder::Nvenc,
                                HardwareEncoder::Amf,
                                HardwareEncoder::QuickSync,
                                HardwareEncoder::Software,
                            ] {
                                if ui
                                    .selectable_label(engine.sunshine_host.active_encoder == enc, enc.display_name())
                                    .clicked()
                                {
                                    engine.sunshine_host.active_encoder = enc;
                                }
                            }
                        });

                        ui.horizontal(|ui| {
                            ui.label("Type de manette émulée pour les invités :");
                            if ui
                                .selectable_label(
                                    engine.sunshine_host.default_gamepad_type == VirtualGamepadType::Xbox360,
                                    "Manette Xbox 360 (Compatible 100% des jeux PC)",
                                )
                                .clicked()
                            {
                                engine.sunshine_host.default_gamepad_type = VirtualGamepadType::Xbox360;
                            }
                            if ui
                                .selectable_label(
                                    engine.sunshine_host.default_gamepad_type == VirtualGamepadType::DualShock4,
                                    "Manette PlayStation DualShock 4",
                                )
                                .clicked()
                            {
                                engine.sunshine_host.default_gamepad_type = VirtualGamepadType::DualShock4;
                            }
                        });
                    },
                );
            });
    }

    fn render_guest_workflow(
        ui: &mut Ui,
        lang: Language,
        _state: &AppState,
        engine: &mut StreamingEngine,
        on_start_client_stream: &mut bool,
        on_stop_client_stream: &mut bool,
    ) {
        // =========================================================================
        // ÉTAPE 1 INVITÉ : CHOISIR SON CONTRÔLE (MANETTE OU CLAVIER/SOURIS)
        // =========================================================================
        egui::Frame::none()
            .fill(ModernTheme::CARD_BG)
            .rounding(Rounding::same(12.0))
            .inner_margin(18.0)
            .stroke(Stroke::new(1.0_f32, ModernTheme::BORDER_SUBTLE))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ModernTheme::draw_dot(ui, ModernTheme::INDIGO_ACCENT, 5.0);
                    ui.label(
                        RichText::new(match lang {
                            Language::Fr => "Étape 1 — Avec quoi jouez-vous sur votre PC ?",
                            Language::En => "Step 1 — What controller are you using on your PC?",
                            Language::Es => "Paso 1 — ¿Con qué dispositivo juegas en tu PC?",
                            Language::De => "Schritt 1 — Womit spielst du an deinem PC?",
                            Language::Pt => "Passo 1 — Com que dispositivo joga no seu PC?",
                        })
                        .color(ModernTheme::INDIGO_ACCENT)
                        .strong()
                        .size(16.0),
                    );
                });
                ui.add_space(8.0);

                Self::render_input_mode_card(
                    ui,
                    engine.coop_input_mode == CoopInputMode::MultiGamepad,
                    ModernTheme::GREEN_VIVID,
                    match lang {
                        Language::Fr => "Je joue avec une Manette (Xbox, PlayStation, Switch Pro...)",
                        Language::En => "I am playing with a Gamepad (Xbox, PlayStation, Switch Pro...)",
                        Language::Es => "Juego con un Mando (Xbox, PlayStation, Switch Pro...)",
                        Language::De => "Ich spiele mit einem Controller (Xbox, PlayStation...)",
                        Language::Pt => "Jogo com um Comando (Xbox, PlayStation, Switch Pro...)",
                    },
                    match lang {
                        Language::Fr => "Votre manette est détectée automatiquement et envoyée à l'hôte en tant que Manette Joueur 2 (ou Joueur 3/4).",
                        Language::En => "Your controller is automatically detected and sent to the host as Player 2's controller (or Player 3/4).",
                        Language::Es => "Tu mando se detecta automáticamente y se envía al anfitrión como Mando del Jugador 2.",
                        Language::De => "Dein Controller wird automatisch erkannt und als Spieler 2 an den Host gesendet.",
                        Language::Pt => "O seu comando é detetado automaticamente e enviado ao anfitrião como Comando do Jogador 2.",
                    },
                    || {
                        engine.coop_input_mode = CoopInputMode::MultiGamepad;
                        engine.moonlight_client.capture_mouse_keyboard = false;
                    },
                );

                ui.add_space(8.0);

                Self::render_input_mode_card(
                    ui,
                    engine.coop_input_mode == CoopInputMode::KeyboardToVirtualGamepad,
                    ModernTheme::BLUE_ELECTRIC,
                    match lang {
                        Language::Fr => "Je n'ai pas de manette : Convertir mon Clavier/Souris en Manette Joueur 2",
                        Language::En => "I don't have a gamepad: Convert my Keyboard/Mouse into Player 2 Gamepad",
                        Language::Es => "No tengo mando: Convertir mi Teclado/Ratón en Mando Jugador 2",
                        Language::De => "Kein Controller: Meine Tastatur/Maus in Spieler 2 Controller umwandeln",
                        Language::Pt => "Não tenho comando: Converter o meu Teclado/Rato em Comando Jogador 2",
                    },
                    match lang {
                        Language::Fr => "Touches : ZQSD/Flèches = Stick Gauche | Souris = Stick Droit | Espace/Clic G = Bouton A | Shift/Clic D = Bouton B | E = Bouton X | R = Bouton Y.",
                        Language::En => "Keys: WASD/Arrows = Left Stick | Mouse = Right Stick | Space/Left Click = Button A | Shift/Right Click = Button B | E = Button X | R = Button Y.",
                        Language::Es => "Teclas: WASD/Flechas = Stick Izq | Ratón = Stick Der | Espacio/Clic Izq = Botón A | Shift = Botón B | E = X | R = Y.",
                        Language::De => "Tasten: WASD/Pfeile = Linker Stick | Maus = Rechter Stick | Leertaste/Links-Klick = A | Shift = B | E = X | R = Y.",
                        Language::Pt => "Teclas: WASD/Setas = Analógico Esq | Rato = Analógico Dir | Espaço/Clique Esq = Botão A | Shift = Botão B.",
                    },
                    || {
                        engine.coop_input_mode = CoopInputMode::KeyboardToVirtualGamepad;
                        engine.moonlight_client.capture_mouse_keyboard = false;
                    },
                );

                ui.add_space(8.0);

                Self::render_input_mode_card(
                    ui,
                    engine.coop_input_mode == CoopInputMode::SharedKeyboardMouse,
                    ModernTheme::ORANGE_CORAL,
                    match lang {
                        Language::Fr => "Utiliser mon Clavier & ma Souris en direct sur l'écran de l'hôte",
                        Language::En => "Use my Keyboard & Mouse directly on the host's screen",
                        Language::Es => "Usar mi Teclado y Ratón directamente en la pantalla del anfitrión",
                        Language::De => "Meine Tastatur & Maus direkt auf dem Host-Bildschirm verwenden",
                        Language::Pt => "Usar o meu Teclado e Rato diretamente no ecrã do anfitrião",
                    },
                    match lang {
                        Language::Fr => "Partage le même curseur de souris et le même clavier que l'hôte (idéal pour les jeux au tour par tour ou sur un seul clavier).",
                        Language::En => "Shares the same mouse cursor and keyboard as the host (ideal for turn-based games or single-keyboard games).",
                        Language::Es => "Comparte el mismo cursor y teclado que el anfitrión (ideal para juegos por turnos).",
                        Language::De => "Teilt denselben Mauszeiger und dieselbe Tastatur wie der Host (ideal für rundenbasierte Spiele).",
                        Language::Pt => "Partilha o mesmo cursor de rato e teclado que o anfitrião (ideal para jogos por turnos).",
                    },
                    || {
                        engine.coop_input_mode = CoopInputMode::SharedKeyboardMouse;
                        engine.moonlight_client.capture_mouse_keyboard = true;
                    },
                );
            });

        ui.add_space(14.0);

        // =========================================================================
        // ÉTAPE 2 INVITÉ : REJOINDRE L'ÉCRAN EN 1 CLIC
        // =========================================================================
        egui::Frame::none()
            .fill(ModernTheme::CARD_BG)
            .rounding(Rounding::same(12.0))
            .inner_margin(18.0)
            .stroke(Stroke::new(1.5_f32, ModernTheme::INDIGO_ACCENT))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        ui.horizontal(|ui| {
                            ModernTheme::draw_dot(ui, ModernTheme::INDIGO_ACCENT, 5.0);
                            ui.label(
                                RichText::new(match lang {
                                    Language::Fr => "Étape 2 — Rejoindre l'écran de jeu de l'hôte",
                                    Language::En => "Step 2 — Join the host's game screen",
                                    Language::Es => "Paso 2 — Unirse a la pantalla de juego del anfitrión",
                                    Language::De => "Schritt 2 — Dem Spielbildschirm des Hosts beitreten",
                                    Language::Pt => "Passo 2 — Juntar-se ao ecrã de jogo do anfitrião",
                                })
                                .color(ModernTheme::TEXT_PRIMARY)
                                .strong()
                                .size(16.0),
                            );
                        });
                        ui.label(
                            RichText::new(format!(
                                "Connexion directe P2P vers l'Hôte ({}) — Aucun code PIN requis.",
                                engine.moonlight_client.target_host_vip
                            ))
                            .color(ModernTheme::TEXT_SECONDARY)
                            .size(13.0),
                        );
                    });

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if !engine.moonlight_client.is_streaming {
                            let join_btn = egui::Button::new(
                                RichText::new(match lang {
                                    Language::Fr => "Rejoindre l'Écran de l'Hôte",
                                    Language::En => "Join Host's Screen",
                                    Language::Es => "Unirse a la Pantalla del Anfitrión",
                                    Language::De => "Bildschirm des Hosts beitreten",
                                    Language::Pt => "Juntar-se ao Ecrã do Anfitrião",
                                })
                                .color(Color32::WHITE)
                                .strong()
                                .size(15.0),
                            )
                            .fill(ModernTheme::INDIGO_ACCENT)
                            .rounding(Rounding::same(10.0))
                            .min_size(egui::vec2(240.0, 44.0));

                            if ui.add(join_btn).clicked() {
                                *on_start_client_stream = true;
                            }
                        } else {
                            let leave_btn = egui::Button::new(
                                RichText::new(match lang {
                                    Language::Fr => "Quitter l'Écran Partagé",
                                    Language::En => "Leave Shared Screen",
                                    Language::Es => "Salir de la Pantalla",
                                    Language::De => "Bildschirm verlassen",
                                    Language::Pt => "Sair do Ecrã Partilhado",
                                })
                                .color(Color32::WHITE)
                                .strong()
                                .size(15.0),
                            )
                            .fill(ModernTheme::RED_ERROR)
                            .rounding(Rounding::same(10.0))
                            .min_size(egui::vec2(210.0, 44.0));

                            if ui.add(leave_btn).clicked() {
                                *on_stop_client_stream = true;
                            }
                        }
                    });
                });

                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    ui.label("Affichage :");
                    if ui
                        .selectable_label(
                            engine.moonlight_client.display_mode == StreamDisplayMode::BorderlessFullscreen,
                            "Plein écran sans bordure (Immersion)",
                        )
                        .clicked()
                    {
                        engine.moonlight_client.display_mode = StreamDisplayMode::BorderlessFullscreen;
                    }
                    if ui
                        .selectable_label(
                            engine.moonlight_client.display_mode == StreamDisplayMode::EmbeddedWindowed,
                            "Mode Fenêtré",
                        )
                        .clicked()
                    {
                        engine.moonlight_client.display_mode = StreamDisplayMode::EmbeddedWindowed;
                    }

                    ui.add_space(16.0);
                    ui.label("Décodage matériel :");
                    for codec in [VideoCodec::Hevc, VideoCodec::Av1, VideoCodec::H264] {
                        if ui
                            .selectable_label(engine.moonlight_client.preferred_codec == codec, codec.label())
                            .clicked()
                        {
                            engine.moonlight_client.preferred_codec = codec;
                        }
                    }
                });
            });
    }

    fn render_input_mode_card(
        ui: &mut Ui,
        selected: bool,
        accent: Color32,
        title: &str,
        description: &str,
        on_select: impl FnOnce(),
    ) {
        let fill = if selected {
            Color32::from_rgba_unmultiplied(accent.r(), accent.g(), accent.b(), 16)
        } else {
            ModernTheme::SUBTLE_BG
        };
        let stroke = if selected {
            Stroke::new(2.0_f32, accent)
        } else {
            Stroke::new(1.0_f32, ModernTheme::BORDER_SUBTLE)
        };

        let resp = egui::Frame::none()
            .fill(fill)
            .rounding(Rounding::same(10.0))
            .inner_margin(12.0)
            .stroke(stroke)
            .show(ui, |ui| {
                ui.set_min_width(ui.available_width());
                ui.horizontal(|ui| {
                    ModernTheme::draw_dot(
                        ui,
                        if selected { accent } else { ModernTheme::TEXT_MUTED },
                        if selected { 5.5 } else { 4.0 },
                    );
                    ui.vertical(|ui| {
                        ui.label(
                            RichText::new(title)
                                .color(if selected { accent } else { ModernTheme::TEXT_PRIMARY })
                                .strong()
                                .size(14.0),
                        );
                        ui.label(
                            RichText::new(description)
                                .color(ModernTheme::TEXT_SECONDARY)
                                .size(12.5),
                        );
                    });
                });
            })
            .response
            .interact(egui::Sense::click());

        if resp.clicked() {
            on_select();
        }
    }
}
