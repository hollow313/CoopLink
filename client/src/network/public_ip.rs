// CoopLink Public IP Detection Utility (Instant & Resilient)
// Copyright (c) 2026 Loic HUON. All Rights Reserved.

use std::io::{Read, Write};
use std::net::{Ipv4Addr, TcpStream, ToSocketAddrs};
use std::time::Duration;
use tracing::info;

/// Récupère l'adresse IP publique de la machine en quelques millisecondes sans dépendance externe
pub fn fetch_public_ip_sync() -> Option<Ipv4Addr> {
    #[cfg(windows)]
    {
        let ps_cmd = "(Invoke-WebRequest -Uri 'https://api.ipify.org' -UseBasicParsing -TimeoutSec 3).Content.Trim()";
        if let Ok(out) = crate::platform::windows::process::run_hidden_powershell(ps_cmd) {
            let trimmed = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if let Ok(ip) = trimmed.parse::<Ipv4Addr>() {
                info!("IP publique détectée via PowerShell : {}", ip);
                return Some(ip);
            }
        }
        let ps_fallback = "(Invoke-WebRequest -Uri 'https://ifconfig.me/ip' -UseBasicParsing -TimeoutSec 3).Content.Trim()";
        if let Ok(out) = crate::platform::windows::process::run_hidden_powershell(ps_fallback) {
            let trimmed = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if let Ok(ip) = trimmed.parse::<Ipv4Addr>() {
                info!("IP publique détectée via PowerShell fallback : {}", ip);
                return Some(ip);
            }
        }
    }

    let endpoints = [
        ("api.ipify.org", "api.ipify.org:80"),
        ("ifconfig.me", "ifconfig.me:80"),
        ("icanhazip.com", "icanhazip.com:80"),
    ];

    for (host, target) in endpoints {
        if let Ok(mut addrs) = target.to_socket_addrs() {
            if let Some(addr) = addrs.next() {
                if let Ok(mut stream) = TcpStream::connect_timeout(&addr, Duration::from_millis(1500)) {
                    let _ = stream.set_read_timeout(Some(Duration::from_millis(2000)));
                    let req = format!(
                        "GET / HTTP/1.1\r\nHost: {}\r\nUser-Agent: CoopLink/1.0\r\nConnection: close\r\n\r\n",
                        host
                    );
                    if stream.write_all(req.as_bytes()).is_ok() {
                        let mut response = String::new();
                        if stream.read_to_string(&mut response).is_ok() {
                            if let Some(body) = response.split("\r\n\r\n").nth(1) {
                                if let Ok(ip) = body.trim().parse::<Ipv4Addr>() {
                                    info!("IP publique détectée via TCP : {}", ip);
                                    return Some(ip);
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    None
}
