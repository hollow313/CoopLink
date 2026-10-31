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
        Language::Fr
    }
}

impl Language {
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
            Language::En => "EN",
            Language::Es => "ES",
            Language::De => "DE",
            Language::Pt => "PT",
        }
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            Language::Fr => "Français",
            Language::En => "English",
            Language::Es => "Español",
            Language::De => "Deutsch",
            Language::Pt => "Português",
        }
    }
}

pub struct I18n;

impl I18n {
    // --- Barre supérieure & Statuts ---
    pub fn app_subtitle(lang: Language) -> &'static str {
        match lang {
            Language::Fr => "Réseau Virtuel P2P & Partage de Jeu Coopératif",
            Language::En => "Virtual P2P LAN & Co-op Game Sharing",
            Language::Es => "Red Virtual P2P y Pantalla Compartida Co-op",
            Language::De => "Virtuelles P2P-LAN & Co-op Spiel-Sharing",
            Language::Pt => "Rede Virtual P2P e Partilha de Jogo Co-op",
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

    // --- Onglets ---
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
            Language::Fr => "Écran Partagé & Manettes",
            Language::En => "Shared Screen & Controllers",
            Language::Es => "Pantalla y Mandos Co-op",
            Language::De => "Bildschirm & Controller",
            Language::Pt => "Ecrã Partilhado e Comandos",
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

    // --- Barre inférieure ---
    pub fn footer_traffic(lang: Language, tx_kb: f64, rx_kb: f64) -> String {
        match lang {
            Language::Fr => format!("Trafic P2P Direct — Envoyé : {:.2} Ko | Reçu : {:.2} Ko", tx_kb, rx_kb),
            Language::En => format!("Direct P2P Traffic — Sent: {:.2} KB | Received: {:.2} KB", tx_kb, rx_kb),
            Language::Es => format!("Tráfico P2P Directo — Enviado: {:.2} KB | Recibido: {:.2} KB", tx_kb, rx_kb),
            Language::De => format!("Direkter P2P-Verkehr — Gesendet: {:.2} KB | Empfangen: {:.2} KB", tx_kb, rx_kb),
            Language::Pt => format!("Tráfego P2P Direto — Enviado: {:.2} KB | Recebido: {:.2} KB", tx_kb, rx_kb),
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
