// CoopLink Relay — relais TCP pour les hôtes injoignables en direct (CGNAT, pas d'IPv6).
//
// Protocole (une ligne de texte, puis tuyau bidirectionnel brut) :
//   HELLO host <ROOM>\n   -> l'hôte attend un invité pour ce salon
//   HELLO guest <ROOM>\n  -> l'invité est apparié avec l'hôte en attente
// Si aucun hôte n'attend, le serveur répond `NO_HOST\n` et ferme.

use std::collections::HashMap;
use std::io;
use std::net::SocketAddr;
use std::time::{Duration, Instant};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Mutex;
use tracing::{debug, info, warn};

/// Un hôte en attente d'invité.
struct Pending {
    stream: TcpStream,
    since: Instant,
}

type Waiting = std::sync::Arc<Mutex<HashMap<String, Pending>>>;

pub async fn start(port: u16) -> anyhow::Result<()> {
    let listener = TcpListener::bind(("0.0.0.0", port)).await?;
    info!("Relais CoopLink en écoute sur 0.0.0.0:{} (TCP)", port);
    let waiting: Waiting = std::sync::Arc::new(Mutex::new(HashMap::new()));

    loop {
        let (stream, addr) = listener.accept().await?;
        let waiting = waiting.clone();
        tokio::spawn(async move {
            if let Err(e) = handle(stream, addr, waiting).await {
                debug!("Relais {} : {}", addr, e);
            }
        });
    }
}

/// Lit une ligne sans surconsommer le flux (le reste doit passer tel quel dans le tuyau).
async fn read_header_line(stream: &mut TcpStream) -> io::Result<String> {
    let mut line = Vec::with_capacity(64);
    let mut byte = [0u8; 1];
    loop {
        let n = stream.read(&mut byte).await?;
        if n == 0 {
            return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "header"));
        }
        if byte[0] == b'\n' {
            break;
        }
        if line.len() > 256 {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "header too long"));
        }
        line.push(byte[0]);
    }
    Ok(String::from_utf8_lossy(&line).trim().to_string())
}

async fn handle(mut stream: TcpStream, addr: SocketAddr, waiting: Waiting) -> anyhow::Result<()> {
    let _ = stream.set_nodelay(true);
    let header = read_header_line(&mut stream).await?;
    let mut parts = header.split_whitespace();
    let command = parts.next().unwrap_or("").to_ascii_uppercase();
    let role = parts.next().unwrap_or("").to_ascii_lowercase();
    let room = parts.next().unwrap_or("").to_ascii_uppercase();

    if command != "HELLO" || room.is_empty() || room.len() > 32 {
        let _ = stream.write_all(b"BAD_ROOM\n").await;
        return Ok(());
    }

    match role.as_str() {
        "host" => {
            info!("Relais : hôte en attente pour le salon {} ({})", room, addr);
            let mut map = waiting.lock().await;
            map.insert(
                room,
                Pending {
                    stream,
                    since: Instant::now(),
                },
            );
            // The stream now lives in the map until a guest takes it. If the host
            // disconnects meanwhile, the write side fails on the next pairing.
            Ok(())
        }
        "guest" => {
            let host = {
                let mut map = waiting.lock().await;
                // Drop entries whose host has been waiting for more than 30 minutes.
                map.retain(|_, p| p.since.elapsed() < Duration::from_secs(1800));
                map.remove(&room)
            };
            match host {
                Some(mut pending) => {
                    info!("Relais : appariement du salon {} ({} <-> {})", room, addr, "hôte");
                    let mut host_stream = pending.stream;
                    // Un hôte qui s'est déconnecté entre-temps échoue ici.
                    if host_stream.write_all(b"OK\n").await.is_err() {
                        let _ = stream.write_all(b"NO_HOST\n").await;
                        return Ok(());
                    }
                    if stream.write_all(b"OK\n").await.is_err() {
                        return Ok(());
                    }
                    match tokio::io::copy_bidirectional(&mut host_stream, &mut stream).await {
                        Ok((up, down)) => debug!(
                            "Relais salon {} terminé ({} octets hôte->invité, {} invité->hôte)",
                            room, up, down
                        ),
                        Err(e) => debug!("Relais salon {} interrompu : {}", room, e),
                    }
                    Ok(())
                }
                None => {
                    warn!("Relais : aucun hôte pour le salon {} (invité {})", room, addr);
                    let _ = stream.write_all(b"NO_HOST\n").await;
                    Ok(())
                }
            }
        }
        _ => {
            let _ = stream.write_all(b"BAD_ROLE\n").await;
            Ok(())
        }
    }
}
