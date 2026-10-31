pub mod moonlight_client;
pub mod pairing;
pub mod sunshine_host;

use moonlight_client::MoonlightClientManager;
use pairing::ZeroConfigPairingManager;
use std::net::Ipv4Addr;
use sunshine_host::SunshineHostManager;

#[allow(dead_code)]
pub struct StreamingEngine {
    pub sunshine_host: SunshineHostManager,
    pub moonlight_client: MoonlightClientManager,
    pub pairing: ZeroConfigPairingManager,
    pub selected_app_name: String,
    pub auto_pair_on_connect: bool,
}

impl StreamingEngine {
    pub fn new(default_host_vip: Ipv4Addr) -> Self {
        Self {
            sunshine_host: SunshineHostManager::new(default_host_vip),
            moonlight_client: MoonlightClientManager::new(default_host_vip),
            pairing: ZeroConfigPairingManager::new(),
            selected_app_name: "Desktop (Session Graphique Partagée)".to_string(),
            auto_pair_on_connect: true,
        }
    }
}
