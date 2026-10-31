use serde::{Deserialize, Serialize};
use std::net::Ipv4Addr;
use tracing::info;

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
    pub stream_port_https: u16,
    pub stream_port_http: u16,
    pub assigned_player_slot: u8,
    pub host_cert_fingerprint: String,
    pub message: String,
}

#[allow(dead_code)]
pub struct CoopSessionPairing {
    pub status: PairingStatus,
    pub active_token: String,
    pub client_cert_fingerprint: String,
}

#[allow(dead_code)]
impl CoopSessionPairing {
    pub fn new() -> Self {
        let token_num = rand::random::<u32>() % 900_000 + 100_000;
        let fp = format!(
            "SHA256:{:08X}:{:08X}:COOP-P2P",
            rand::random::<u32>(),
            rand::random::<u32>()
        );
        Self {
            status: PairingStatus::Ready,
            active_token: format!("{}", token_num),
            client_cert_fingerprint: fp,
        }
    }

    pub fn build_pair_request(
        &mut self,
        client_id: u64,
        player_name: &str,
        client_virtual_ip: Ipv4Addr,
        preferred_gamepad: &str,
    ) -> CoopPairRequest {
        self.status = PairingStatus::NegotiatingSsl;
        CoopPairRequest {
            client_id,
            player_name: player_name.to_string(),
            client_virtual_ip,
            session_token: self.active_token.clone(),
            client_cert_pem: format!(
                "-----BEGIN CERTIFICATE-----\nCOOPLINK-P2P-{}-{}\n-----END CERTIFICATE-----",
                client_id, self.client_cert_fingerprint
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
        info!(
            "Autorisation P2P CoopLink : Validation de '{}' ({}) -> Assigné au Joueur {}",
            req.player_name, req.client_virtual_ip, assigned_slot
        );

        let host_fp = format!("SHA256:HOST:{:08X}", rand::random::<u32>());
        self.status = PairingStatus::Paired {
            host_virtual_ip,
            assigned_player_slot: assigned_slot,
            session_fingerprint: host_fp.clone(),
        };

        CoopPairAck {
            success: true,
            host_virtual_ip,
            stream_port_https: 47984,
            stream_port_http: 47989,
            assigned_player_slot: assigned_slot,
            host_cert_fingerprint: host_fp,
            message: format!("Joueur {} connecté", assigned_slot),
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
