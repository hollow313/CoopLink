use crate::network::protocol::constant_time_eq;
use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::net::Ipv4Addr;
use tracing::{info, warn};

type HmacSha256 = Hmac<Sha256>;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PairingStatus {
    Ready,
    NegotiatingSsl,
    ValidatingSession,
    Paired {
        host_virtual_ip: Ipv4Addr,
        assigned_player_slot: u8,
        session_fingerprint: String,
    },
    Failed(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoopPairRequest {
    pub client_id: u64,
    pub player_name: String,
    pub client_virtual_ip: Ipv4Addr,
    pub session_token: String,
    pub client_cert_pem: String,
    pub preferred_gamepad: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoopPairAck {
    pub success: bool,
    pub host_virtual_ip: Ipv4Addr,
    pub stream_port: u16,
    pub control_port: u16,
    pub assigned_player_slot: u8,
    pub host_cert_fingerprint: String,
    pub message: String,
}

#[allow(dead_code)]
pub struct CoopSessionPairing {
    pub status: PairingStatus,
    pub shared_secret: [u8; 32],
    pub active_token: String,
    pub client_cert_fingerprint: String,
}

#[allow(dead_code)]
impl CoopSessionPairing {
    pub fn new() -> Self {
        let secret: [u8; 32] = rand::random();
        let mut hasher = Sha256::new();
        hasher.update(b"CoopLink-Cert-Fingerprint-v2:");
        hasher.update(secret);
        let digest = hasher.finalize();
        let fp = format!(
            "SHA256:{:02X}{:02X}:{:02X}{:02X}:{:02X}{:02X}:{:02X}{:02X}",
            digest[0], digest[1], digest[2], digest[3], digest[4], digest[5], digest[6], digest[7]
        );

        let token = Self::compute_hmac_token(&secret, 0, Ipv4Addr::new(10, 244, 0, 1));
        Self {
            status: PairingStatus::Ready,
            shared_secret: secret,
            active_token: token,
            client_cert_fingerprint: fp,
        }
    }

    fn compute_hmac_token(secret: &[u8; 32], client_id: u64, client_vip: Ipv4Addr) -> String {
        let mut mac = HmacSha256::new_from_slice(secret).expect("HMAC key valid");
        mac.update(b"CoopLink-Stream-Pair-v2:");
        mac.update(&client_id.to_be_bytes());
        mac.update(&client_vip.octets());
        let bytes = mac.finalize().into_bytes();
        let mut hex = String::with_capacity(64);
        for b in bytes {
            use std::fmt::Write;
            let _ = write!(&mut hex, "{:02x}", b);
        }
        hex
    }

    pub fn build_pair_request(
        &mut self,
        client_id: u64,
        player_name: &str,
        client_virtual_ip: Ipv4Addr,
        preferred_gamepad: &str,
    ) -> CoopPairRequest {
        self.status = PairingStatus::NegotiatingSsl;
        let hmac_token = Self::compute_hmac_token(&self.shared_secret, client_id, client_virtual_ip);
        self.active_token = hmac_token.clone();

        CoopPairRequest {
            client_id,
            player_name: player_name.to_string(),
            client_virtual_ip,
            session_token: hmac_token,
            client_cert_pem: format!(
                "-----BEGIN CERTIFICATE-----\n{}\n-----END CERTIFICATE-----",
                self.client_cert_fingerprint
            ),
            preferred_gamepad: preferred_gamepad.to_string(),
        }
    }

    pub fn authorize_client_on_host(
        &mut self,
        req: &CoopPairRequest,
        host_virtual_ip: Ipv4Addr,
        assigned_slot: u8,
    ) -> CoopPairAck {
        self.status = PairingStatus::ValidatingSession;

        // Vérification cryptographique en temps constant du jeton HMAC-SHA256 de la session
        let expected_token =
            Self::compute_hmac_token(&self.shared_secret, req.client_id, req.client_virtual_ip);
        if !constant_time_eq(req.session_token.as_bytes(), expected_token.as_bytes()) {
            warn!(
                "Rejet d'appairage P2P CoopLink : jeton HMAC-SHA256 invalide pour '{}'",
                req.player_name
            );
            self.status = PairingStatus::Failed("Jeton HMAC-SHA256 invalide".to_string());
            return CoopPairAck {
                success: false,
                host_virtual_ip,
                stream_port: crate::streaming::settings::DEFAULT_STREAM_PORT,
                control_port: crate::streaming::settings::DEFAULT_CONTROL_PORT,
                assigned_player_slot: 0,
                host_cert_fingerprint: String::new(),
                message: "Authentification cryptographique HMAC échouée".to_string(),
            };
        }

        info!(
            "Autorisation P2P HMAC-SHA256 CoopLink : Validation de '{}' ({}) -> Assigné au Joueur {}",
            req.player_name, req.client_virtual_ip, assigned_slot
        );

        let mut hasher = Sha256::new();
        hasher.update(b"CoopLink-Host-Ack-v2:");
        hasher.update(self.shared_secret);
        hasher.update(host_virtual_ip.octets());
        hasher.update([assigned_slot]);
        let d = hasher.finalize();
        let host_fp = format!(
            "SHA256:HOST:{:02X}{:02X}:{:02X}{:02X}",
            d[0], d[1], d[2], d[3]
        );

        self.status = PairingStatus::Paired {
            host_virtual_ip,
            assigned_player_slot: assigned_slot,
            session_fingerprint: host_fp.clone(),
        };

        CoopPairAck {
            success: true,
            host_virtual_ip,
            stream_port: crate::streaming::settings::DEFAULT_STREAM_PORT,
            control_port: crate::streaming::settings::DEFAULT_CONTROL_PORT,
            assigned_player_slot: assigned_slot,
            host_cert_fingerprint: host_fp,
            message: format!("Joueur {} authentifié et connecté", assigned_slot),
        }
    }

    pub fn complete_pairing_on_client(&mut self, ack: &CoopPairAck) {
        if ack.success {
            self.status = PairingStatus::Paired {
                host_virtual_ip: ack.host_virtual_ip,
                assigned_player_slot: ack.assigned_player_slot,
                session_fingerprint: ack.host_cert_fingerprint.clone(),
            };
        } else {
            self.status = PairingStatus::Failed(ack.message.clone());
        }
    }
}
