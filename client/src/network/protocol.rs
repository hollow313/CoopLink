use anyhow::{bail, Result};
use std::net::Ipv4Addr;

pub const PROTOCOL_MAGIC: &[u8; 4] = b"COOP";
pub const PROTOCOL_VERSION: u8 = 1;

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
    pub fn encode(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(512);
        buf.extend_from_slice(PROTOCOL_MAGIC);
        buf.push(PROTOCOL_VERSION);

        match self {
            CoopPacket::HandshakeRequest {
                player_name,
                room_code,
                client_version,
            } => {
                buf.push(PacketType::HandshakeRequest as u8);
                buf.push(*client_version);
                let name_bytes = player_name.as_bytes();
                buf.push(name_bytes.len().min(255) as u8);
                buf.extend_from_slice(&name_bytes[..name_bytes.len().min(255)]);

                let code_bytes = room_code.as_bytes();
                buf.push(code_bytes.len().min(255) as u8);
                buf.extend_from_slice(&code_bytes[..code_bytes.len().min(255)]);
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
                buf.push(host_bytes.len().min(255) as u8);
                buf.extend_from_slice(&host_bytes[..host_bytes.len().min(255)]);

                let err_bytes = error_msg.as_bytes();
                buf.push(err_bytes.len().min(255) as u8);
                buf.extend_from_slice(&err_bytes[..err_bytes.len().min(255)]);
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
                buf.push(PacketType::TunnelData as u8);
                buf.extend_from_slice(&src_ip.octets());
                buf.extend_from_slice(&dst_ip.octets());
                buf.extend_from_slice(&(payload.len() as u32).to_be_bytes());
                buf.extend_from_slice(payload);
            }
            CoopPacket::BroadcastGaming {
                src_ip,
                port,
                payload,
            } => {
                buf.push(PacketType::BroadcastGaming as u8);
                buf.extend_from_slice(&src_ip.octets());
                buf.extend_from_slice(&port.to_be_bytes());
                buf.extend_from_slice(&(payload.len() as u32).to_be_bytes());
                buf.extend_from_slice(payload);
            }
            CoopPacket::PeerListUpdate { raw_json } => {
                buf.push(PacketType::PeerListUpdate as u8);
                let json_bytes = raw_json.as_bytes();
                buf.extend_from_slice(&(json_bytes.len() as u32).to_be_bytes());
                buf.extend_from_slice(json_bytes);
            }
            CoopPacket::Disconnect { peer_id, reason } => {
                buf.push(PacketType::Disconnect as u8);
                buf.extend_from_slice(&peer_id.to_be_bytes());
                let reason_bytes = reason.as_bytes();
                buf.push(reason_bytes.len().min(255) as u8);
                buf.extend_from_slice(&reason_bytes[..reason_bytes.len().min(255)]);
            }
        }

        // Ajout du checksum CRC32 en fin de paquet
        let crc = crc32fast::hash(&buf);
        buf.extend_from_slice(&crc.to_be_bytes());
        buf
    }

    pub fn decode(buf: &[u8]) -> Result<Self> {
        if buf.len() < 9 {
            bail!("Paquet trop court");
        }
        if &buf[0..4] != PROTOCOL_MAGIC {
            bail!("Magic bytes invalides");
        }
        let _version = buf[4];

        // Vérification CRC32
        let payload_len = buf.len() - 4;
        let expected_crc = u32::from_be_bytes([
            buf[payload_len],
            buf[payload_len + 1],
            buf[payload_len + 2],
            buf[payload_len + 3],
        ]);
        let calculated_crc = crc32fast::hash(&buf[..payload_len]);
        if expected_crc != calculated_crc {
            bail!("Échec de vérification d'intégrité CRC32");
        }

        let ptype_byte = buf[5];
        let ptype = PacketType::from_u8(ptype_byte).ok_or_else(|| anyhow::anyhow!("Type inconnu"))?;

        let mut offset = 6;
        match ptype {
            PacketType::HandshakeRequest => {
                let client_version = buf[offset];
                offset += 1;
                let name_len = buf[offset] as usize;
                offset += 1;
                let name = String::from_utf8_lossy(&buf[offset..offset + name_len]).to_string();
                offset += name_len;
                let code_len = buf[offset] as usize;
                offset += 1;
                let code = String::from_utf8_lossy(&buf[offset..offset + code_len]).to_string();

                Ok(CoopPacket::HandshakeRequest {
                    player_name: name,
                    room_code: code,
                    client_version,
                })
            }
            PacketType::HandshakeResponse => {
                let success = buf[offset] == 1;
                offset += 1;
                let ip = Ipv4Addr::new(buf[offset], buf[offset + 1], buf[offset + 2], buf[offset + 3]);
                offset += 4;
                let id = u64::from_be_bytes(buf[offset..offset + 8].try_into()?);
                offset += 8;

                let host_len = buf[offset] as usize;
                offset += 1;
                let host_name = String::from_utf8_lossy(&buf[offset..offset + host_len]).to_string();
                offset += host_len;

                let err_len = buf[offset] as usize;
                offset += 1;
                let error_msg = String::from_utf8_lossy(&buf[offset..offset + err_len]).to_string();

                Ok(CoopPacket::HandshakeResponse {
                    success,
                    assigned_ip: ip,
                    assigned_id: id,
                    host_name,
                    error_msg,
                })
            }
            PacketType::Ping => {
                let seq = u32::from_be_bytes(buf[offset..offset + 4].try_into()?);
                offset += 4;
                let ts = u64::from_be_bytes(buf[offset..offset + 8].try_into()?);
                Ok(CoopPacket::Ping {
                    seq,
                    timestamp_ms: ts,
                })
            }
            PacketType::Pong => {
                let seq = u32::from_be_bytes(buf[offset..offset + 4].try_into()?);
                offset += 4;
                let ts = u64::from_be_bytes(buf[offset..offset + 8].try_into()?);
                Ok(CoopPacket::Pong {
                    seq,
                    timestamp_ms: ts,
                })
            }
            PacketType::TunnelData => {
                let src_ip = Ipv4Addr::new(buf[offset], buf[offset + 1], buf[offset + 2], buf[offset + 3]);
                offset += 4;
                let dst_ip = Ipv4Addr::new(buf[offset], buf[offset + 1], buf[offset + 2], buf[offset + 3]);
                offset += 4;
                let len = u32::from_be_bytes(buf[offset..offset + 4].try_into()?) as usize;
                offset += 4;
                let payload = buf[offset..offset + len].to_vec();

                Ok(CoopPacket::TunnelData {
                    src_ip,
                    dst_ip,
                    payload,
                })
            }
            PacketType::BroadcastGaming => {
                let src_ip = Ipv4Addr::new(buf[offset], buf[offset + 1], buf[offset + 2], buf[offset + 3]);
                offset += 4;
                let port = u16::from_be_bytes(buf[offset..offset + 2].try_into()?);
                offset += 2;
                let len = u32::from_be_bytes(buf[offset..offset + 4].try_into()?) as usize;
                offset += 4;
                let payload = buf[offset..offset + len].to_vec();

                Ok(CoopPacket::BroadcastGaming {
                    src_ip,
                    port,
                    payload,
                })
            }
            PacketType::PeerListUpdate => {
                let len = u32::from_be_bytes(buf[offset..offset + 4].try_into()?) as usize;
                offset += 4;
                let raw_json = String::from_utf8_lossy(&buf[offset..offset + len]).to_string();
                Ok(CoopPacket::PeerListUpdate { raw_json })
            }
            PacketType::Disconnect => {
                let peer_id = u64::from_be_bytes(buf[offset..offset + 8].try_into()?);
                offset += 8;
                let rlen = buf[offset] as usize;
                offset += 1;
                let reason = String::from_utf8_lossy(&buf[offset..offset + rlen]).to_string();
                Ok(CoopPacket::Disconnect { peer_id, reason })
            }
        }
    }
}
