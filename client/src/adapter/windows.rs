//! Adaptateur réseau virtuel Windows (Wintun).
//!
//! Wintun (WireGuard LLC) fournit un pilote TAP signé WHQL : c'est lui qui donne aux
//! jeux une **vraie** interface réseau (10.244.0.x), indispensable pour la découverte
//! LAN et les jeux coopératifs qui se connectent à une IP du réseau local.
//!
//! `wintun.dll` (amd64) est livré à côté de `cooplink.exe` par l'installateur. S'il est
//! absent, l'adaptateur reste inactif : le partage vidéo fonctionne, mais pas le LAN.
//!
//! Prérequis : processus élevé (administrateur), comme pour le pare-feu et la métrique.

use super::VirtualAdapter;
use crate::platform::windows::metric::set_adapter_metric;
use crate::platform::windows::process::run_hidden;
use anyhow::{anyhow, Result};
use std::ffi::CString;
use std::net::Ipv4Addr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::JoinHandle;
use tokio::sync::mpsc;
use tracing::{info, warn};

use windows_sys::Win32::Foundation::{FreeLibrary, HMODULE};
use windows_sys::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryW};
use windows_sys::Win32::System::Threading::WaitForSingleObject;

/// Nom de l'interface tel qu'il apparaît dans Windows (netsh, jeux, pare-feu).
const ADAPTER_NAME: &str = "CoopLink";
/// Libellé interne du pilote.
const TUNNEL_TYPE: &str = "CoopLink";
/// Taille de l'anneau partagé avec le pilote (4 Mio, valeur recommandée par Wintun).
const RING_CAPACITY: u32 = 0x40_0000;
/// Préfixe du sous-réseau virtuel.
const VIRTUAL_PREFIX: [u8; 3] = [10, 244, 0];
/// MTU : 1500 moins la surcharge d'encapsulation du tunnel.
const DEFAULT_MTU: u32 = 1420;

type Handle = isize;

type CreateAdapterFn = unsafe extern "system" fn(*const u16, *const u16, *const u8) -> Handle;
type OpenAdapterFn = unsafe extern "system" fn(*const u16) -> Handle;
type CloseAdapterFn = unsafe extern "system" fn(Handle);
type StartSessionFn = unsafe extern "system" fn(Handle, u32) -> Handle;
type EndSessionFn = unsafe extern "system" fn(Handle);
type ReceivePacketFn = unsafe extern "system" fn(Handle, *mut u32) -> *mut u8;
type ReleaseReceivePacketFn = unsafe extern "system" fn(Handle, *const u8);
type AllocateSendPacketFn = unsafe extern "system" fn(Handle, u32) -> *mut u8;
type SendPacketFn = unsafe extern "system" fn(Handle, *const u8);
type GetReadWaitEventFn = unsafe extern "system" fn(Handle) -> Handle;
type GetDriverVersionFn = unsafe extern "system" fn(*mut u32) -> u32;

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

unsafe fn symbol(module: HMODULE, name: &str) -> Result<*const ()> {
    let cname = CString::new(name).map_err(|_| anyhow!("nom de symbole invalide"))?;
    match GetProcAddress(module, cname.as_ptr() as *const u8) {
        Some(f) => Ok(f as *const ()),
        None => Err(anyhow!("symbole {} absent de wintun.dll", name)),
    }
}

/// Table de fonctions de `wintun.dll`, résolue dynamiquement : le binaire démarre même
/// si la DLL est absente (le LAN est alors simplement indisponible).
struct Wintun {
    module: HMODULE,
    create_adapter: CreateAdapterFn,
    open_adapter: OpenAdapterFn,
    close_adapter: CloseAdapterFn,
    start_session: StartSessionFn,
    end_session: EndSessionFn,
    receive_packet: ReceivePacketFn,
    release_receive_packet: ReleaseReceivePacketFn,
    allocate_send_packet: AllocateSendPacketFn,
    send_packet: SendPacketFn,
    get_read_wait_event: GetReadWaitEventFn,
}

impl Drop for Wintun {
    fn drop(&mut self) {
        if self.module != 0 {
            unsafe { FreeLibrary(self.module) };
        }
    }
}

impl Wintun {
    fn load() -> Result<Self> {
        let mut candidates: Vec<std::path::PathBuf> = vec![
            std::path::PathBuf::from("wintun.dll"),
            std::path::PathBuf::from("bin/wintun.dll"),
        ];
        if let Ok(cur) = std::env::current_exe() {
            if let Some(dir) = cur.parent() {
                candidates.insert(0, dir.join("wintun.dll"));
                candidates.insert(1, dir.join("bin").join("wintun.dll"));
                candidates.insert(2, dir.join("redist").join("wintun.dll"));
            }
        }

        let mut last_err = String::from("wintun.dll introuvable");
        for path in &candidates {
            if !path.exists() {
                continue;
            }
            let wpath = wide(&path.to_string_lossy());
            let module = unsafe { LoadLibraryW(wpath.as_ptr()) };
            if module == 0 {
                last_err = format!("chargement impossible : {}", path.display());
                continue;
            }

            let resolved = (|| -> Result<Self> {
                unsafe {
                    let create_adapter: CreateAdapterFn = std::mem::transmute(symbol(module, "WintunCreateAdapter")?);
                    let open_adapter: OpenAdapterFn = std::mem::transmute(symbol(module, "WintunOpenAdapter")?);
                    let close_adapter: CloseAdapterFn = std::mem::transmute(symbol(module, "WintunCloseAdapter")?);
                    let start_session: StartSessionFn = std::mem::transmute(symbol(module, "WintunStartSession")?);
                    let end_session: EndSessionFn = std::mem::transmute(symbol(module, "WintunEndSession")?);
                    let receive_packet: ReceivePacketFn = std::mem::transmute(symbol(module, "WintunReceivePacket")?);
                    let release_receive_packet: ReleaseReceivePacketFn =
                        std::mem::transmute(symbol(module, "WintunReleaseReceivePacket")?);
                    let allocate_send_packet: AllocateSendPacketFn =
                        std::mem::transmute(symbol(module, "WintunAllocateSendPacket")?);
                    let send_packet: SendPacketFn = std::mem::transmute(symbol(module, "WintunSendPacket")?);
                    let get_read_wait_event: GetReadWaitEventFn =
                        std::mem::transmute(symbol(module, "WintunGetReadWaitEvent")?);

                    Ok(Self {
                        module,
                        create_adapter,
                        open_adapter,
                        close_adapter,
                        start_session,
                        end_session,
                        receive_packet,
                        release_receive_packet,
                        allocate_send_packet,
                        send_packet,
                        get_read_wait_event,
                    })
                }
            })();

            match resolved {
                Ok(api) => {
                    info!("wintun.dll chargé : {}", path.display());
                    return Ok(api);
                }
                Err(e) => {
                    unsafe { FreeLibrary(module) };
                    last_err = format!("{} ({})", e, path.display());
                }
            }
        }

        Err(anyhow!(last_err))
    }
}

#[allow(dead_code)]
pub struct WindowsVirtualAdapter {
    adapter_name: String,
    assigned_ip: Option<Ipv4Addr>,
    metric: u32,
    connected: Arc<AtomicBool>,
    stop: Arc<AtomicBool>,
    api: Option<Arc<Wintun>>,
    adapter: Handle,
    session: Handle,
    tx_queue: mpsc::Sender<Vec<u8>>,
    rx_queue: Option<mpsc::Receiver<Vec<u8>>>,
    threads: Vec<JoinHandle<()>>,
}

impl WindowsVirtualAdapter {
    pub fn new() -> (Self, mpsc::Sender<Vec<u8>>, mpsc::Receiver<Vec<u8>>) {
        // Canal 1 : Tunnel -> Adaptateur (paquets à injecter sur le réseau virtuel)
        let (net_to_adapter_tx, net_to_adapter_rx) = mpsc::channel(1000);
        // Canal 2 : Adaptateur -> Tunnel (paquets émis par les jeux locaux)
        let (adapter_to_net_tx, adapter_to_net_rx) = mpsc::channel(1000);

        let adapter = Self {
            adapter_name: ADAPTER_NAME.to_string(),
            assigned_ip: None,
            metric: 5,
            connected: Arc::new(AtomicBool::new(false)),
            stop: Arc::new(AtomicBool::new(false)),
            api: None,
            adapter: 0,
            session: 0,
            tx_queue: adapter_to_net_tx,
            rx_queue: Some(net_to_adapter_rx),
            threads: Vec::new(),
        };

        (adapter, net_to_adapter_tx, adapter_to_net_rx)
    }

    /// Configure l'interface dans Windows (IP, masque, MTU, métrique, route).
    fn configure_interface(&self, ip: Ipv4Addr, subnet_mask: Ipv4Addr, metric: u32, mtu: u32) {
        let name = self.adapter_name.clone();
        let _ = run_hidden(
            "netsh",
            &[
                "interface",
                "ipv4",
                "set",
                "address",
                &format!("name=\"{}\"", name),
                "static",
                &ip.to_string(),
                &subnet_mask.to_string(),
                "none",
            ],
        );
        let _ = run_hidden(
            "netsh",
            &[
                "interface",
                "ipv4",
                "set",
                "subinterface",
                &format!("\"{}\"", name),
                &format!("mtu={}", mtu),
                "store=persistent",
            ],
        );
        let _ = set_adapter_metric(name.as_str(), metric);

        // Le /24 est posé par `netsh set address` : une route manuelle avec la passerelle
        // égale à sa propre IP ne ferait que parasiter la table de routage.
    }
}

impl VirtualAdapter for WindowsVirtualAdapter {
    fn initialize(
        &mut self,
        _name: &str,
        ip: Ipv4Addr,
        subnet_mask: Ipv4Addr,
        metric: u32,
        mtu: u32,
    ) -> Result<()> {
        if self.connected.load(Ordering::SeqCst) {
            return Ok(());
        }
        // Le nom de l'interface reste stable : c'est lui que les jeux, netsh et le
        // pare-feu voient, et il doit correspondre à l'adaptateur créé par Wintun.
        self.adapter_name = ADAPTER_NAME.to_string();
        self.assigned_ip = Some(ip);
        self.metric = metric;
        let mtu = if mtu == 0 { DEFAULT_MTU } else { mtu };

        info!(
            "Initialisation de l'adaptateur virtuel Wintun '{}' (IP {}, MTU {}, métrique {})...",
            self.adapter_name, ip, mtu, metric
        );

        let api = Arc::new(Wintun::load().map_err(|e| {
            warn!(
                "Wintun indisponible ({}) : réseau local virtuel désactivé. \
                 Placez wintun.dll à côté de cooplink.exe.",
                e
            );
            e
        })?);

        let adapter_name_w = wide(&self.adapter_name);
        let tunnel_type_w = wide(TUNNEL_TYPE);

        // Réutilise l'adaptateur existant (créé à la première session) au lieu d'en
        // empiler un nouveau à chaque lancement.
        let mut adapter = unsafe { (api.open_adapter)(adapter_name_w.as_ptr()) };
        if adapter == 0 {
            adapter = unsafe {
                (api.create_adapter)(
                    adapter_name_w.as_ptr(),
                    tunnel_type_w.as_ptr(),
                    std::ptr::null(),
                )
            };
        }
        if adapter == 0 {
            let err = std::io::Error::last_os_error();
            return Err(anyhow!(
                "Création de l'adaptateur Wintun impossible ({}). Lancez CoopLink en administrateur.",
                err
            ));
        }
        self.adapter = adapter;

        let session = unsafe { (api.start_session)(adapter, RING_CAPACITY) };
        if session == 0 {
            let err = std::io::Error::last_os_error();
            unsafe { (api.close_adapter)(adapter) };
            self.adapter = 0;
            return Err(anyhow!("Ouverture de session Wintun impossible ({})", err));
        }
        self.session = session;
        let read_event = unsafe { (api.get_read_wait_event)(session) };

        self.configure_interface(ip, subnet_mask, metric, mtu);

        // --- Écriture : paquets venus du tunnel -> interface Windows ---
        let mut rx_queue = self
            .rx_queue
            .take()
            .ok_or_else(|| anyhow!("File d'entrée de l'adaptateur déjà consommée"))?;
        let stop_writer = self.stop.clone();
        let api_writer = api.clone();
        self.threads.push(std::thread::spawn(move || {
            while !stop_writer.load(Ordering::Relaxed) {
                let Some(pkt) = rx_queue.blocking_recv() else {
                    break;
                };
                if pkt.is_empty() || pkt.len() > u16::MAX as usize {
                    continue;
                }
                for attempt in 0..20 {
                    let buf = unsafe { (api_writer.allocate_send_packet)(session, pkt.len() as u32) };
                    if !buf.is_null() {
                        unsafe {
                            std::ptr::copy_nonoverlapping(pkt.as_ptr(), buf, pkt.len());
                            (api_writer.send_packet)(session, buf);
                        }
                        break;
                    }
                    if attempt == 19 || stop_writer.load(Ordering::Relaxed) {
                        break;
                    }
                    std::thread::sleep(std::time::Duration::from_millis(2));
                }
            }
        }));

        // --- Lecture : paquets émis sur l'interface -> tunnel ---
        let tx_queue = self.tx_queue.clone();
        let stop_reader = self.stop.clone();
        let api_reader = api.clone();
        self.threads.push(std::thread::spawn(move || {
            while !stop_reader.load(Ordering::Relaxed) {
                if read_event != 0 {
                    unsafe { WaitForSingleObject(read_event, 200) };
                } else {
                    std::thread::sleep(std::time::Duration::from_millis(5));
                }
                loop {
                    let mut size: u32 = 0;
                    let buf = unsafe { (api_reader.receive_packet)(session, &mut size) };
                    if buf.is_null() {
                        break;
                    }
                    let pkt = unsafe { std::slice::from_raw_parts(buf, size as usize) }.to_vec();
                    unsafe { (api_reader.release_receive_packet)(session, buf) };

                    // Seul l'IPv4 du sous-réseau virtuel traverse le tunnel : rien de la
                    // pile Internet de la machine ne doit fuiter chez les pairs.
                    let is_ipv4 = pkt.first().map(|b| b >> 4 == 4).unwrap_or(false);
                    if !is_ipv4 || pkt.len() < 20 {
                        continue;
                    }
                    let dst = Ipv4Addr::new(pkt[16], pkt[17], pkt[18], pkt[19]);
                    let in_subnet = dst.octets()[0..3] == VIRTUAL_PREFIX;
                    let is_broadcast = dst == Ipv4Addr::BROADCAST || dst.octets()[3] == 255;
                    let is_multicast = (224..=239).contains(&dst.octets()[0]);
                    if !in_subnet && !is_broadcast && !is_multicast {
                        continue;
                    }
                    if tx_queue.blocking_send(pkt).is_err() {
                        break;
                    }
                }
            }
        }));

        self.stop.store(false, Ordering::SeqCst);
        self.connected.store(true, Ordering::SeqCst);
        self.api = Some(api);
        info!(
            "Adaptateur Wintun '{}' opérationnel : {}.{}.{}.0/24, MTU {}",
            self.adapter_name, VIRTUAL_PREFIX[0], VIRTUAL_PREFIX[1], VIRTUAL_PREFIX[2], mtu
        );
        Ok(())
    }

    fn read_packet(&mut self) -> Result<Option<Vec<u8>>> {
        Ok(None)
    }

    fn write_packet(&mut self, packet: &[u8]) -> Result<()> {
        let _ = self.tx_queue.try_send(packet.to_vec());
        Ok(())
    }

    fn set_metric(&mut self, metric: u32) -> Result<()> {
        self.metric = metric;
        let name = self.adapter_name.clone();
        set_adapter_metric(&name, metric)
    }

    fn is_connected(&self) -> bool {
        self.connected.load(Ordering::Relaxed)
    }

    fn shutdown(&mut self) -> Result<()> {
        self.connected.store(false, Ordering::SeqCst);
        self.stop.store(true, Ordering::SeqCst);

        if let Some(api) = self.api.as_ref() {
            if self.session != 0 {
                unsafe { (api.end_session)(self.session) };
                self.session = 0;
            }
            if self.adapter != 0 {
                unsafe { (api.close_adapter)(self.adapter) };
                self.adapter = 0;
            }
        }
        for handle in self.threads.drain(..) {
            let _ = handle.join();
        }
        info!("Adaptateur virtuel CoopLink fermé");
        Ok(())
    }
}
