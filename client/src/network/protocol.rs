use anyhow::{bail, Result};
use chacha20poly1305::{
    aead::{Aead, KeyInit, Payload},
    ChaCha20Poly1305, Key, Nonce,
};
use hmac::{Hmac, Mac};
use sha2::Sha256;
use std::net::Ipv4Addr;

type HmacSha256 = Hmac<Sha256>;

pub const PROTOCOL_MAGIC: &[u8; 4] = b"COOP";
pub const PROTOCOL_VERSION: u8 = 1;
pub const PROTOCOL_VERSION_AEAD: u8 = 2;
pub const MAX_PAYLOAD_SIZE: usize = 65_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PacketType {
    HandshakeRequest = 0x01,
    HandshakeResponse = 0x02,
    Ping = 0x03,
    Pong = 0x04,
    TunnelData = 0x05,
    BroadcastGaming = 0x06,
    PeerListUpdate = 0x07,
    Disconnect = 0x08,
}

impl PacketType {
    pub fn from_u8(val: u8) -> Option<Self> {
        match val {
            0x01 => Some(PacketType::HandshakeRequest),
            0x02 => Some(PacketType::HandshakeResponse),
            0x03 => Some(PacketType::Ping),
            0x04 => Some(PacketType::Pong),
            0x05 => Some(PacketType::TunnelData),
            0x06 => Some(PacketType::BroadcastGaming),
            0x07 => Some(PacketType::PeerListUpdate),
            0x08 => Some(PacketType::Disconnect),
            _ => None,
        }
    }
}

/// Dérive une clé de session 256 bits (ChaCha20-Poly1305) à partir du code de salon et du mot de passe
pub fn derive_session_key(room_code: &str, room_password: &str) -> [u8; 32] {
    let mut mac = <HmacSha256 as Mac>::new_from_slice(b"CoopLink-P2P-AEAD-SessionKey-v2")
        .expect("HMAC accepte toute taille de clé");
    mac.update(room_code.trim().as_bytes());
    mac.update(b"::");
    mac.update(room_password.trim().as_bytes());
    let result = mac.finalize().into_bytes();
    let mut key = [0u8; 32];
    key.copy_from_slice(&result);
    key
}

/// Calcule un jeton d'authentification HMAC-SHA256 pour le Handshake
pub fn compute_handshake_auth_tag(room_code: &str, room_password: &str, player_name: &str) -> [u8; 32] {
    let key = derive_session_key(room_code, room_password);
    let mut mac = <HmacSha256 as Mac>::new_from_slice(&key).expect("HMAC key valid");
    mac.update(b"CoopLink-Handshake-Auth-v2:");
    mac.update(room_code.trim().as_bytes());
    mac.update(b":");
    mac.update(player_name.as_bytes());
    let res = mac.finalize().into_bytes();
    let mut out = [0u8; 32];
    out.copy_from_slice(&res);
    out
}

/// Comparaison en temps constant pour prévenir les attaques par canal auxiliaire (timing attacks)
pub fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (&x, &y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

/// Lecteur binaire sécurisé avec vérification stricte des bornes (zéro panic possible)
struct ByteCursor<'a> {
    buf: &'a [u8],
    pos: usize,
}

impl<'a> ByteCursor<'a> {
    fn new(buf: &'a [u8]) -> Self {
        Self { buf, pos: 0 }
    }

    fn read_bytes(&mut self, len: usize) -> Result<&'a [u8]> {
        if len > MAX_PAYLOAD_SIZE {
            bail!("Longueur de champ excessive: {}", len);
        }
        let end = self
            .pos
            .checked_add(len)
            .ok_or_else(|| anyhow::anyhow!("Dépassement arithmétique d'offset"))?;
        let slice = self
            .buf
            .get(self.pos..end)
            .ok_or_else(|| anyhow::anyhow!("Paquet tronqué: besoin de {} octets à l'offset {}, taille totale {}", len, self.pos, self.buf.len()))?;
        self.pos = end;
        Ok(slice)
    }

    fn read_u8(&mut self) -> Result<u8> {
        let b = self.read_bytes(1)?;
        Ok(b[0])
    }

    fn read_u16(&mut self) -> Result<u16> {
        let b = self.read_bytes(2)?;
        Ok(u16::from_be_bytes([b[0], b[1]]))
    }

    fn read_u32(&mut self) -> Result<u32> {
        let b = self.read_bytes(4)?;
        Ok(u32::from_be_bytes([b[0], b[1], b[2], b[3]]))
    }

    fn read_u64(&mut self) -> Result<u64> {
        let b = self.read_bytes(8)?;
        Ok(u64::from_be_bytes([
            b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7],
        ]))
    }

    fn read_ipv4(&mut self) -> Result<Ipv4Addr> {
        let b = self.read_bytes(4)?;
        Ok(Ipv4Addr::new(b[0], b[1], b[2], b[3]))
    }

    fn read_string_u8(&mut self) -> Result<String> {
        let len = self.read_u8()? as usize;
        let bytes = self.read_bytes(len)?;
        Ok(String::from_utf8_lossy(bytes).to_string())
    }

    fn read_string_u32(&mut self) -> Result<String> {
        let len = self.read_u32()? as usize;
        let bytes = self.read_bytes(len)?;
        Ok(String::from_utf8_lossy(bytes).to_string())
    }

    fn read_vec_u32(&mut self) -> Result<Vec<u8>> {
        let len = self.read_u32()? as usize;
        let bytes = self.read_bytes(len)?;
        Ok(bytes.to_vec())
    }
}

/// Message de paquet CoopLink
#[derive(Debug, Clone)]
pub enum CoopPacket {
    HandshakeRequest {
        player_name: String,
        room_code: String,
        client_version: u8,
    },
    HandshakeResponse {
        success: bool,
        assigned_ip: Ipv4Addr,
        assigned_id: u64,
        host_name: String,
        error_msg: String,
    },
    Ping {
        seq: u32,
        timestamp_ms: u64,
    },
    Pong {
        seq: u32,
        timestamp_ms: u64,
    },
    TunnelData {
        src_ip: Ipv4Addr,
        dst_ip: Ipv4Addr,
        payload: Vec<u8>,
    },
    BroadcastGaming {
        src_ip: Ipv4Addr,
        port: u16,
        payload: Vec<u8>,
    },
    PeerListUpdate {
        raw_json: String,
    },
    Disconnect {
        peer_id: u64,
        reason: String,
    },
}

impl CoopPacket {
    /// Sérialise le corps interne du paquet (sans en-tête magic ni MAC)
    fn encode_body(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(512);
        match self {
            CoopPacket::HandshakeRequest {
                player_name,
                room_code,
                client_version,
            } => {
                buf.push(PacketType::HandshakeRequest as u8);
                buf.push(*client_version);
                let name_bytes = player_name.as_bytes();
                let nlen = name_bytes.len().min(255);
                buf.push(nlen as u8);
                buf.extend_from_slice(&name_bytes[..nlen]);

                let code_bytes = room_code.as_bytes();
                let clen = code_bytes.len().min(255);
                buf.push(clen as u8);
                buf.extend_from_slice(&code_bytes[..clen]);
            }
            CoopPacket::HandshakeResponse {
                success,
                assigned_ip,
                assigned_id,
                host_name,
                error_msg,
            } => {
                buf.push(PacketType::HandshakeResponse as u8);
                buf.push(if *success { 1 } else { 0 });
                buf.extend_from_slice(&assigned_ip.octets());
                buf.extend_from_slice(&assigned_id.to_be_bytes());

                let host_bytes = host_name.as_bytes();
                let hlen = host_bytes.len().min(255);
                buf.push(hlen as u8);
                buf.extend_from_slice(&host_bytes[..hlen]);

                let err_bytes = error_msg.as_bytes();
                let elen = err_bytes.len().min(255);
                buf.push(elen as u8);
                buf.extend_from_slice(&err_bytes[..elen]);
            }
            CoopPacket::Ping { seq, timestamp_ms } => {
                buf.push(PacketType::Ping as u8);
                buf.extend_from_slice(&seq.to_be_bytes());
                buf.extend_from_slice(&timestamp_ms.to_be_bytes());
            }
            CoopPacket::Pong { seq, timestamp_ms } => {
                buf.push(PacketType::Pong as u8);
                buf.extend_from_slice(&seq.to_be_bytes());
                buf.extend_from_slice(&timestamp_ms.to_be_bytes());
            }
            CoopPacket::TunnelData {
                src_ip,
                dst_ip,
                payload,
            } => {
                let capped_len = payload.len().min(MAX_PAYLOAD_SIZE);
                buf.push(PacketType::TunnelData as u8);
                buf.extend_from_slice(&src_ip.octets());
                buf.extend_from_slice(&dst_ip.octets());
                buf.extend_from_slice(&(capped_len as u32).to_be_bytes());
                buf.extend_from_slice(&payload[..capped_len]);
            }
            CoopPacket::BroadcastGaming {
                src_ip,
                port,
                payload,
            } => {
                let capped_len = payload.len().min(MAX_PAYLOAD_SIZE);
                buf.push(PacketType::BroadcastGaming as u8);
                buf.extend_from_slice(&src_ip.octets());
                buf.extend_from_slice(&port.to_be_bytes());
                buf.extend_from_slice(&(capped_len as u32).to_be_bytes());
                buf.extend_from_slice(&payload[..capped_len]);
            }
            CoopPacket::PeerListUpdate { raw_json } => {
                let json_bytes = raw_json.as_bytes();
                let capped_len = json_bytes.len().min(MAX_PAYLOAD_SIZE);
                buf.push(PacketType::PeerListUpdate as u8);
                buf.extend_from_slice(&(capped_len as u32).to_be_bytes());
                buf.extend_from_slice(&json_bytes[..capped_len]);
            }
            CoopPacket::Disconnect { peer_id, reason } => {
                buf.push(PacketType::Disconnect as u8);
                buf.extend_from_slice(&peer_id.to_be_bytes());
                let reason_bytes = reason.as_bytes();
                let rlen = reason_bytes.len().min(255);
                buf.push(rlen as u8);
                buf.extend_from_slice(&reason_bytes[..rlen]);
            }
        }
        buf
    }

    /// Décode le corps interne avec vérification stricte des bornes sur chaque champ
    fn decode_body(body: &[u8]) -> Result<Self> {
        let mut cur = ByteCursor::new(body);
        let ptype_byte = cur.read_u8()?;
        let ptype = PacketType::from_u8(ptype_byte)
            .ok_or_else(|| anyhow::anyhow!("Type de paquet inconnu: 0x{:02x}", ptype_byte))?;

        match ptype {
            PacketType::HandshakeRequest => {
                let client_version = cur.read_u8()?;
                let player_name = cur.read_string_u8()?;
                let room_code = cur.read_string_u8()?;
                Ok(CoopPacket::HandshakeRequest {
                    player_name,
                    room_code,
                    client_version,
                })
            }
            PacketType::HandshakeResponse => {
                let success = cur.read_u8()? == 1;
                let assigned_ip = cur.read_ipv4()?;
                let assigned_id = cur.read_u64()?;
                let host_name = cur.read_string_u8()?;
                let error_msg = cur.read_string_u8()?;
                Ok(CoopPacket::HandshakeResponse {
                    success,
                    assigned_ip,
                    assigned_id,
                    host_name,
                    error_msg,
                })
            }
            PacketType::Ping => {
                let seq = cur.read_u32()?;
                let timestamp_ms = cur.read_u64()?;
                Ok(CoopPacket::Ping { seq, timestamp_ms })
            }
            PacketType::Pong => {
                let seq = cur.read_u32()?;
                let timestamp_ms = cur.read_u64()?;
                Ok(CoopPacket::Pong { seq, timestamp_ms })
            }
            PacketType::TunnelData => {
                let src_ip = cur.read_ipv4()?;
                let dst_ip = cur.read_ipv4()?;
                let payload = cur.read_vec_u32()?;
                Ok(CoopPacket::TunnelData {
                    src_ip,
                    dst_ip,
                    payload,
                })
            }
            PacketType::BroadcastGaming => {
                let src_ip = cur.read_ipv4()?;
                let port = cur.read_u16()?;
                let payload = cur.read_vec_u32()?;
                Ok(CoopPacket::BroadcastGaming {
                    src_ip,
                    port,
                    payload,
                })
            }
            PacketType::PeerListUpdate => {
                let raw_json = cur.read_string_u32()?;
                Ok(CoopPacket::PeerListUpdate { raw_json })
            }
            PacketType::Disconnect => {
                let peer_id = cur.read_u64()?;
                let reason = cur.read_string_u8()?;
                Ok(CoopPacket::Disconnect { peer_id, reason })
            }
        }
    }

    /// Chiffre et authentifie le paquet sur le fil avec ChaCha20-Poly1305 (AEAD 256-bit)
    pub fn encode_encrypted(&self, session_key: &[u8; 32]) -> Vec<u8> {
        let plaintext = self.encode_body();
        let cipher = ChaCha20Poly1305::new(Key::from_slice(session_key));
        let nonce_bytes: [u8; 12] = rand::random();
        let nonce = Nonce::from_slice(&nonce_bytes);

        let mut header = [0u8; 5];
        header[0..4].copy_from_slice(PROTOCOL_MAGIC);
        header[4] = PROTOCOL_VERSION_AEAD;

        let ciphertext = cipher
            .encrypt(
                nonce,
                Payload {
                    msg: &plaintext,
                    aad: &header,
                },
            )
            .expect("Échec chiffrement AEAD ChaCha20-Poly1305");

        let mut out = Vec::with_capacity(5 + 12 + ciphertext.len());
        out.extend_from_slice(&header);
        out.extend_from_slice(&nonce_bytes);
        out.extend_from_slice(&ciphertext);
        out
    }

    /// Vérifie le tag Poly1305, déchiffre avec ChaCha20 et décode avec vérification stricte des bornes
    pub fn decode_encrypted(buf: &[u8], session_key: &[u8; 32]) -> Result<Self> {
        // Minimum: 4 (MAGIC) + 1 (VERSION) + 12 (NONCE) + 1 (PTYPE) + 16 (POLY1305 TAG) = 34 octets
        if buf.len() < 34 {
            bail!("Paquet AEAD trop court ({} octets)", buf.len());
        }
        if buf.get(0..4) != Some(PROTOCOL_MAGIC.as_slice()) {
            bail!("Magic bytes invalides");
        }
        let version = buf[4];
        if version != PROTOCOL_VERSION_AEAD {
            bail!("Version de protocole non chiffrée ou incompatible: {}", version);
        }

        let header = &buf[0..5];
        let nonce = Nonce::from_slice(&buf[5..17]);
        let ciphertext = &buf[17..];

        let cipher = ChaCha20Poly1305::new(Key::from_slice(session_key));
        let plaintext = cipher
            .decrypt(
                nonce,
                Payload {
                    msg: ciphertext,
                    aad: header,
                },
            )
            .map_err(|_| anyhow::anyhow!("Échec d'authentification AEAD ChaCha20-Poly1305 (clé de salon/mot de passe invalide ou paquet altéré)"))?;

        Self::decode_body(&plaintext)
    }

    /// Encodage standard (utilisé avec la clé par défaut ou pour les tests de rétrocompatibilité)
    #[allow(dead_code)]
    pub fn encode(&self) -> Vec<u8> {
        let default_key = derive_session_key("DEFAULT", "");
        self.encode_encrypted(&default_key)
    }

    /// Décodage sécurisé avec vérification stricte des bornes (supporte AEAD v2 et trames v1)
    #[allow(dead_code)]
    pub fn decode(buf: &[u8]) -> Result<Self> {
        if buf.len() < 9 {
            bail!("Paquet trop court");
        }
        if buf.get(0..4) != Some(PROTOCOL_MAGIC.as_slice()) {
            bail!("Magic bytes invalides");
        }
        let version = buf[4];
        if version == PROTOCOL_VERSION_AEAD {
            let default_key = derive_session_key("DEFAULT", "");
            return Self::decode_encrypted(buf, &default_key);
        }

        // Support rétrocompatible v1 (avec vérification stricte des bornes pour empêcher tout panic)
        let payload_len = buf
            .len()
            .checked_sub(4)
            .ok_or_else(|| anyhow::anyhow!("Paquet trop court pour CRC"))?;
        let crc_slice = buf
            .get(payload_len..buf.len())
            .ok_or_else(|| anyhow::anyhow!("CRC hors bornes"))?;
        let expected_crc = u32::from_be_bytes([
            crc_slice[0],
            crc_slice[1],
            crc_slice[2],
            crc_slice[3],
        ]);
        let calculated_crc = crc32fast::hash(&buf[..payload_len]);
        if expected_crc != calculated_crc {
            bail!("Échec de vérification d'intégrité CRC32");
        }

        let body = buf
            .get(5..payload_len)
            .ok_or_else(|| anyhow::anyhow!("Corps de paquet vide"))?;
        Self::decode_body(body)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_malformed_12_byte_and_42_byte_packets_never_panic() {
        // Test 1: Paquet malveillant de 12 octets avec name_len = 255 et CRC32 valide (Audit Critique #1)
        let mut pkt12 = vec![b'C', b'O', b'O', b'P', 1, 0x01, 1, 255];
        let crc12 = crc32fast::hash(&pkt12);
        pkt12.extend_from_slice(&crc12.to_be_bytes());
        assert_eq!(pkt12.len(), 12);
        assert!(CoopPacket::decode(&pkt12).is_err());

        // Test 2: Paquet TunnelData malveillant de 42 octets annonçant payload_len = 60000
        let mut pkt42 = vec![b'C', b'O', b'O', b'P', 1, 0x05];
        pkt42.extend_from_slice(&[10, 244, 0, 2]); // src_ip
        pkt42.extend_from_slice(&[10, 244, 0, 1]); // dst_ip
        pkt42.extend_from_slice(&(60000u32).to_be_bytes()); // len = 60000
        pkt42.extend_from_slice(&[0u8; 20]);
        let crc42 = crc32fast::hash(&pkt42);
        pkt42.extend_from_slice(&crc42.to_be_bytes());
        assert_eq!(pkt42.len(), 42);
        assert!(CoopPacket::decode(&pkt42).is_err());
    }

    #[test]
    fn test_aead_encryption_roundtrip_and_tamper_rejection() {
        let key = derive_session_key("COOP-7701", "MonMotDePasseSecret");
        let wrong_key = derive_session_key("COOP-7701", "MauvaisMotDePasse");

        let pkt = CoopPacket::HandshakeRequest {
            player_name: "Alice".to_string(),
            room_code: "COOP-7701".to_string(),
            client_version: PROTOCOL_VERSION,
        };

        let wire = pkt.encode_encrypted(&key);
        // Vérifie que ni "Alice" ni "COOP-7701" n'apparaissent en clair sur le fil
        let wire_str = String::from_utf8_lossy(&wire);
        assert!(!wire_str.contains("Alice"));
        assert!(!wire_str.contains("COOP-7701"));

        // Déchiffrement avec la bonne clé réussit
        let decoded = CoopPacket::decode_encrypted(&wire, &key).expect("Doit déchiffrer");
        match decoded {
            CoopPacket::HandshakeRequest {
                player_name,
                room_code,
                ..
            } => {
                assert_eq!(player_name, "Alice");
                assert_eq!(room_code, "COOP-7701");
            }
            _ => panic!("Mauvais type décodé"),
        }

        // Déchiffrement avec une mauvaise clé échoue
        assert!(CoopPacket::decode_encrypted(&wire, &wrong_key).is_err());

        // Altération d'un octet sur le fil est rejetée par Poly1305
        let mut tampered = wire.clone();
        let last = tampered.len() - 1;
        tampered[last] ^= 0x01;
        assert!(CoopPacket::decode_encrypted(&tampered, &key).is_err());
    }
}
