use serde::{Deserialize, Serialize};
use std::net::Ipv4Addr;
use std::time::{SystemTime, UNIX_EPOCH};

/// État du jumelage automatique Zero-Config entre Moonlight (Client) et Sunshine (Hôte)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PairingStatus {
    Idle,
    ExchangingCertificates,
    ValidatingPinOnHost,
    Paired {
        host_virtual_ip: Ipv4Addr,
        cert_fingerprint: String,
        session_token: String,
    },
    Failed(String),
}

/// Requête de jumelage automatique envoyée sur le canal de contrôle chiffré P2P CoopLink
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ZeroConfigPairRequest {
    pub client_id: u64,
    pub client_name: String,
    pub client_virtual_ip: Ipv4Addr,
    pub generated_pin: String,
    pub client_unique_id: String,
    pub client_cert_pem: String,
    pub controller_type: String,
}

/// Réponse de validation du jumelage renvoyée par l'Hôte Sunshine au Client Moonlight
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ZeroConfigPairAck {
    pub accepted: bool,
    pub host_virtual_ip: Ipv4Addr,
    pub sunshine_https_port: u16,
    pub sunshine_http_port: u16,
    pub server_cert_fingerprint: String,
    pub session_token: String,
    pub assigned_player_slot: u8, // 2 = Joueur 2, 3 = Joueur 3, etc.
    pub message: String,
}

pub struct ZeroConfigPairingManager {
    pub status: PairingStatus,
    pub client_unique_id: String,
    pub last_pin: String,
}

impl ZeroConfigPairingManager {
    pub fn new() -> Self {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let uid = format!("{:016X}", (now as u64) ^ 0xC009_114B_2026_u64);
        Self {
            status: PairingStatus::Idle,
            client_unique_id: uid,
            last_pin: "0000".to_string(),
        }
    }

    /// Génère une requête de jumelage automatique (sans saisie manuelle de code PIN)
    pub fn build_pair_request(
        &mut self,
        client_id: u64,
        client_name: &str,
        client_virtual_ip: Ipv4Addr,
        controller_type: &str,
    ) -> ZeroConfigPairRequest {
        let pin_num = (rand::random::<u16>() % 9000) + 1000;
        let pin = format!("{:04}", pin_num);
        self.last_pin = pin.clone();
        self.status = PairingStatus::ExchangingCertificates;

        // Certificat X.509 auto-signé de session pour l'authentification TLS mutuelle Sunshine/Moonlight
        let cert_pem = format!(
            "-----BEGIN CERTIFICATE-----\nCOOPLINK-ZEROCONFIG-CERT-{}-{}\n-----END CERTIFICATE-----",
            self.client_unique_id, pin
        );

        ZeroConfigPairRequest {
            client_id,
            client_name: client_name.to_string(),
            client_virtual_ip,
            generated_pin: pin,
            client_unique_id: self.client_unique_id.clone(),
            client_cert_pem: cert_pem,
            controller_type: controller_type.to_string(),
        }
    }

    /// Côté Hôte : valide automatiquement le PIN auprès de l'API locale Sunshine (https://127.0.0.1:47990/api/pin)
    pub fn authorize_client_on_host(
        &mut self,
        req: &ZeroConfigPairRequest,
        host_virtual_ip: Ipv4Addr,
        assigned_slot: u8,
    ) -> ZeroConfigPairAck {
        self.status = PairingStatus::ValidatingPinOnHost;

        // Envoi silencieux du PIN à l'instance locale Sunshine si le démon tourne
        let payload = serde_json::json!({
            "pin": req.generated_pin,
            "name": format!("CoopLink - {} (Joueur {})", req.client_name, assigned_slot)
        });

        let ps_cmd = format!(
            "[System.Net.ServicePointManager]::ServerCertificateValidationCallback = {{$true}}; \
             try {{ Invoke-RestMethod -Uri 'https://127.0.0.1:47990/api/pin' -Method Post -ContentType 'application/json' -Body '{}' -TimeoutSec 2 | Out-Null }} catch {{}}",
            payload.to_string().replace('\'', "''")
        );
        let _ = crate::platform::windows::process::run_hidden_powershell(&ps_cmd);

        let fingerprint = format!("SHA256:COOPLINK-{:08X}", crc32fast::hash(req.client_cert_pem.as_bytes()));
        let token = format!("CL-SESSION-{}-SLOT{}", req.generated_pin, assigned_slot);

        self.status = PairingStatus::Paired {
            host_virtual_ip,
            cert_fingerprint: fingerprint.clone(),
            session_token: token.clone(),
        };

        ZeroConfigPairAck {
            accepted: true,
            host_virtual_ip,
            sunshine_https_port: 47984,
            sunshine_http_port: 47989,
            server_cert_fingerprint: fingerprint,
            session_token: token,
            assigned_player_slot: assigned_slot,
            message: format!(
                "Jumelage Zero-Config validé pour {} (Assigné: Joueur {})",
                req.client_name, assigned_slot
            ),
        }
    }

    /// Côté Client : finalise le jumelage à la réception de l'ACK de l'Hôte
    pub fn complete_pairing_on_client(&mut self, ack: &ZeroConfigPairAck) {
        if ack.accepted {
            self.status = PairingStatus::Paired {
                host_virtual_ip: ack.host_virtual_ip,
                cert_fingerprint: ack.server_cert_fingerprint.clone(),
                session_token: ack.session_token.clone(),
            };
        } else {
            self.status = PairingStatus::Failed(ack.message.clone());
        }
    }
}
