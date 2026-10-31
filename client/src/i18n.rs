use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Language {
    Fr,
    En,
    Es,
    De,
    Pt,
}

impl Default for Language {
    fn default() -> Self {
        Self::from_os_locale()
    }
}

impl Language {
    pub fn from_os_locale() -> Self {
        let locale = crate::platform::windows::process::detect_windows_os_locale();
        if locale.starts_with("fr") {
            Language::Fr
        } else if locale.starts_with("es") {
            Language::Es
        } else if locale.starts_with("de") {
            Language::De
        } else if locale.starts_with("pt") {
            Language::Pt
        } else {
            Language::En
        }
    }

    pub fn all() -> &'static [Language] {
        &[
            Language::Fr,
            Language::En,
            Language::Es,
            Language::De,
            Language::Pt,
        ]
    }

    pub fn code(&self) -> &'static str {
        match self {
            Language::Fr => "FR",
            Language::En => "US/EN",
            Language::Es => "ES",
            Language::De => "DE",
            Language::Pt => "PT",
        }
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            Language::Fr => "Français (FR)",
            Language::En => "English (US)",
            Language::Es => "Español (ES)",
            Language::De => "Deutsch (DE)",
            Language::Pt => "Português (PT)",
        }
    }
}

pub struct I18n;

impl I18n {
    pub fn app_subtitle(lang: Language) -> &'static str {
        match lang {
            Language::Fr => "Réseau Virtuel P2P Chiffré & Partage de Jeu Co-op",
            Language::En => "Encrypted Virtual P2P LAN & Co-op Game Sharing",
            Language::Es => "Red Virtual P2P Cifrada y Juego Compartido Co-op",
            Language::De => "Verschlüsseltes P2P-LAN & Co-op Spiel-Freigabe",
            Language::Pt => "Rede Virtual P2P Encriptada e Partilha de Jogo Co-op",
        }
    }

    pub fn status_ready(lang: Language) -> &'static str {
        match lang {
            Language::Fr => "Prêt",
            Language::En => "Ready",
            Language::Es => "Listo",
            Language::De => "Bereit",
            Language::Pt => "Pronto",
        }
    }

    pub fn status_hosting(lang: Language) -> &'static str {
        match lang {
            Language::Fr => "Tunnel Hôte Actif",
            Language::En => "Host Tunnel Active",
            Language::Es => "Túnel Anfitrión Activo",
            Language::De => "Host-Tunnel Aktiv",
            Language::Pt => "Túnel Anfitrião Ativo",
        }
    }

    pub fn status_connected(lang: Language) -> &'static str {
        match lang {
            Language::Fr => "Tunnel P2P Connecté",
            Language::En => "P2P Tunnel Connected",
            Language::Es => "Túnel P2P Conectado",
            Language::De => "P2P-Tunnel Verbunden",
            Language::Pt => "Túnel P2P Conectado",
        }
    }

    pub fn status_connecting(lang: Language) -> &'static str {
        match lang {
            Language::Fr => "Connexion...",
            Language::En => "Connecting...",
            Language::Es => "Conectando...",
            Language::De => "Verbinden...",
            Language::Pt => "Conectando...",
        }
    }

    pub fn status_error(lang: Language) -> &'static str {
        match lang {
            Language::Fr => "Erreur réseau",
            Language::En => "Network Error",
            Language::Es => "Error de red",
            Language::De => "Netzwerkfehler",
            Language::Pt => "Erro de rede",
        }
    }

    pub fn tab_lobby(lang: Language) -> &'static str {
        match lang {
            Language::Fr => "Salons & Réseau P2P",
            Language::En => "Rooms & P2P Network",
            Language::Es => "Salas y Red P2P",
            Language::De => "Räume & P2P-Netzwerk",
            Language::Pt => "Salas e Rede P2P",
        }
    }

    pub fn tab_streaming(lang: Language) -> &'static str {
        match lang {
            Language::Fr => "Partage de Jeu & Manettes",
            Language::En => "Game Share & Controllers",
            Language::Es => "Juego y Mandos Co-op",
            Language::De => "Spiel & Controller",
            Language::Pt => "Partilha e Comandos",
        }
    }

    pub fn tab_speedtest(lang: Language) -> &'static str {
        match lang {
            Language::Fr => "Débit & Score Hôte",
            Language::En => "Speed & Host Score",
            Language::Es => "Velocidad y Nota",
            Language::De => "Speedtest & Score",
            Language::Pt => "Velocidade e Nota",
        }
    }

    pub fn tab_network(lang: Language) -> &'static str {
        match lang {
            Language::Fr => "Priorité & Cartes",
            Language::En => "Priority & Adapters",
            Language::Es => "Prioridad y Red",
            Language::De => "Priorität & Adapter",
            Language::Pt => "Prioridade e Placas",
        }
    }

    pub fn tab_firewall(lang: Language) -> &'static str {
        match lang {
            Language::Fr => "Pare-feu",
            Language::En => "Firewall",
            Language::Es => "Cortafuegos",
            Language::De => "Firewall",
            Language::Pt => "Firewall",
        }
    }

    pub fn tab_settings(lang: Language) -> &'static str {
        match lang {
            Language::Fr => "Paramètres",
            Language::En => "Settings",
            Language::Es => "Ajustes",
            Language::De => "Einstellungen",
            Language::Pt => "Definições",
        }
    }

    pub fn tab_console(lang: Language) -> &'static str {
        match lang {
            Language::Fr => "Activité",
            Language::En => "Activity",
            Language::Es => "Actividad",
            Language::De => "Aktivität",
            Language::Pt => "Atividade",
        }
    }

    pub fn footer_traffic(lang: Language, tx_kb: f64, rx_kb: f64) -> String {
        match lang {
            Language::Fr => format!("Tunnel P2P Chiffré E2E — Envoyé : {:.2} Ko | Reçu : {:.2} Ko", tx_kb, rx_kb),
            Language::En => format!("Encrypted E2E P2P Tunnel — Sent: {:.2} KB | Received: {:.2} KB", tx_kb, rx_kb),
            Language::Es => format!("Túnel P2P Cifrado E2E — Enviado: {:.2} KB | Recibido: {:.2} KB", tx_kb, rx_kb),
            Language::De => format!("Verschlüsselter P2P-Tunnel — Gesendet: {:.2} KB | Empfangen: {:.2} KB", tx_kb, rx_kb),
            Language::Pt => format!("Túnel P2P Encriptado E2E — Enviado: {:.2} KB | Recebido: {:.2} KB", tx_kb, rx_kb),
        }
    }

    pub fn footer_peers(lang: Language, count: usize) -> String {
        match lang {
            Language::Fr => format!("Participants : {}", count),
            Language::En => format!("Players: {}", count),
            Language::Es => format!("Jugadores: {}", count),
            Language::De => format!("Spieler: {}", count),
            Language::Pt => format!("Jogadores: {}", count),
        }
    }
}
