use crate::config::ServerConfig;
use crate::hub::ServerHub;
use axum::{
    extract::{Path, State},
    response::{Html, IntoResponse, Json},
    routing::{get, post},
    Router,
};
use serde_json::json;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use tracing::info;

#[derive(Clone)]
pub struct WebState {
    pub hub: Arc<ServerHub>,
    pub config: ServerConfig,
}

pub async fn start_web_server(hub: Arc<ServerHub>, config: ServerConfig) {
    let state = WebState { hub, config };
    let app = Router::new()
        .route("/", get(render_dashboard))
        .route("/api/stats", get(get_stats_api))
        .route("/api/rooms", get(get_rooms_api))
        .route("/api/rooms/{code}/kick", post(kick_room_api))
        .with_state(state.clone());

    let bind_addr = format!("0.0.0.0:{}", state.config.web_port);
    info!("CoopLink Web Admin Dashboard accessible sur http://{}", bind_addr);

    if let Ok(listener) = tokio::net::TcpListener::bind(&bind_addr).await {
        let _ = axum::serve(listener, app).await;
    }
}

async fn get_stats_api(State(state): State<WebState>) -> impl IntoResponse {
    let uptime_secs = state.hub.start_time.elapsed().as_secs();
    let tx = state.hub.total_bytes_tx.load(Ordering::Relaxed);
    let rx = state.hub.total_bytes_rx.load(Ordering::Relaxed);
    let relayed = state.hub.total_packets_relayed.load(Ordering::Relaxed);
    let rooms = state.hub.get_active_rooms();
    let total_clients = state.hub.get_total_clients();

    Json(json!({
        "server_name": state.config.server_name,
        "udp_port": state.config.udp_port,
        "web_port": state.config.web_port,
        "uptime_secs": uptime_secs,
        "total_rooms": rooms.len(),
        "total_clients": total_clients,
        "bytes_tx": tx,
        "bytes_rx": rx,
        "packets_relayed": relayed,
    }))
}

async fn get_rooms_api(State(state): State<WebState>) -> impl IntoResponse {
    let rooms = state.hub.get_active_rooms();
    Json(json!(rooms))
}

async fn kick_room_api(
    Path(code): Path<String>,
    State(state): State<WebState>,
) -> impl IntoResponse {
    let mut rooms = state.hub.rooms.write();
    if let Some(r) = rooms.remove(&code) {
        for c in r.clients {
            state.hub.clients_by_id.write().remove(&c.client_id);
            state.hub.endpoint_to_id.write().remove(&c.endpoint);
        }
        Json(json!({ "success": true, "message": format!("Salle {} expulsée", code) }))
    } else {
        Json(json!({ "success": false, "message": "Salle introuvable" }))
    }
}

async fn render_dashboard(State(state): State<WebState>) -> Html<String> {
    let rooms = state.hub.get_active_rooms();
    let total_clients = state.hub.get_total_clients();
    let tx_mb = (state.hub.total_bytes_tx.load(Ordering::Relaxed) as f64) / (1024.0 * 1024.0);
    let rx_mb = (state.hub.total_bytes_rx.load(Ordering::Relaxed) as f64) / (1024.0 * 1024.0);
    let _relayed = state.hub.total_packets_relayed.load(Ordering::Relaxed);
    let uptime_min = state.hub.start_time.elapsed().as_secs() / 60;

    let mut rooms_rows = String::new();
    let mut clients_rows = String::new();

    for r in &rooms {
        rooms_rows.push_str(&format!(
            "<tr>\
                <td><span class='badge cyan'>{}</span></td>\
                <td><strong>{}</strong></td>\
                <td><span class='badge mint'>{} joueurs</span></td>\
                <td>#{}</td>\
                <td><button onclick='kickRoom(\"{}\")' class='btn-kick'>Expulser</button></td>\
            </tr>",
            r.room_code, r.host_name, r.client_count, r.host_id, r.room_code
        ));

        for c in &r.clients {
            let role_badge = if c.is_host {
                "<span class='badge mint'>HÔTE</span>"
            } else {
                "<span class='badge cyan'>CLIENT</span>"
            };
            clients_rows.push_str(&format!(
                "<tr>\
                    <td><strong style='color:#00D2FF;'>#{}</strong></td>\
                    <td>{}</td>\
                    <td><span class='badge blue'>{}</span></td>\
                    <td>{}</td>\
                    <td><code>{}</code></td>\
                    <td><code>{}</code></td>\
                    <td><span style='color:#00E599;'>{} ms</span></td>\
                </tr>",
                c.client_id, c.player_name, c.room_code, role_badge, c.virtual_ip, c.endpoint, c.ping_ms
            ));
        }
    }

    if rooms_rows.is_empty() {
        rooms_rows = "<tr><td colspan='5' style='text-align:center; color:#8E8E93; padding:20px;'>Aucun salon actif pour le moment.</td></tr>".to_string();
    }
    if clients_rows.is_empty() {
        clients_rows = "<tr><td colspan='7' style='text-align:center; color:#8E8E93; padding:20px;'>Aucun client connecté pour le moment.</td></tr>".to_string();
    }

    let html = format!(r#"<!DOCTYPE html>
<html lang="fr">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>CoopLink Server // Administration Centrale</title>
    <style>
        :root {{
            --blue-electric: #0057FF;
            --cyan-vibrant: #00D2FF;
            --blue-deep: #0038A8;
            --green-mint: #00BFA5;
            --green-vivid: #00E599;
            --orange-coral: #FF6D00;
            --gray-neutral: #8E8E93;
            --bg-dark: #090D16;
            --panel-bg: #0F1626;
            --card-bg: #141D34;
        }}
        * {{ box-sizing: border-box; margin: 0; padding: 0; font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, Helvetica, Arial, sans-serif; }}
        body {{ background: var(--bg-dark); color: #F5F8FF; padding: 24px; }}
        .header {{ display: flex; align-items: center; justify-content: space-between; border-bottom: 2px solid var(--blue-deep); padding-bottom: 16px; margin-bottom: 24px; }}
        .header-title {{ display: flex; align-items: center; gap: 12px; }}
        .header-title h1 {{ font-size: 24px; color: #FFFFFF; font-weight: 800; letter-spacing: 0.5px; }}
        .header-title span {{ color: var(--cyan-vibrant); font-family: monospace; font-size: 14px; }}
        .status-pill {{ background: rgba(0, 229, 153, 0.15); border: 1px solid var(--green-vivid); color: var(--green-vivid); padding: 6px 14px; border-radius: 20px; font-weight: 700; font-size: 13px; }}

        .stats-grid {{ display: grid; grid-template-columns: repeat(auto-fit, minmax(200px, 1fr)); gap: 16px; margin-bottom: 24px; }}
        .stat-card {{ background: var(--panel-bg); border: 1px solid #1E2B4A; border-radius: 8px; padding: 18px; border-top: 3px solid var(--blue-electric); }}
        .stat-card.cyan {{ border-top-color: var(--cyan-vibrant); }}
        .stat-card.green {{ border-top-color: var(--green-vivid); }}
        .stat-card.orange {{ border-top-color: var(--orange-coral); }}
        .stat-label {{ color: var(--gray-neutral); font-size: 12px; font-weight: 600; text-transform: uppercase; margin-bottom: 6px; }}
        .stat-value {{ font-size: 26px; font-weight: 800; color: #FFFFFF; }}

        .section {{ background: var(--panel-bg); border: 1px solid #1E2B4A; border-radius: 8px; padding: 20px; margin-bottom: 24px; }}
        .section-header {{ display: flex; align-items: center; justify-content: space-between; margin-bottom: 16px; }}
        .section-header h2 {{ font-size: 18px; color: var(--cyan-vibrant); font-weight: 700; }}

        table {{ width: 100%; border-collapse: collapse; }}
        th {{ text-align: left; padding: 10px 14px; color: var(--gray-neutral); font-size: 12px; font-weight: 600; text-transform: uppercase; border-bottom: 1px solid #1E2B4A; }}
        td {{ padding: 12px 14px; border-bottom: 1px solid #152038; font-size: 14px; }}
        tr:hover {{ background: rgba(0, 87, 255, 0.05); }}

        .badge {{ padding: 4px 8px; border-radius: 4px; font-size: 12px; font-weight: 700; font-family: monospace; }}
        .badge.cyan {{ background: rgba(0, 210, 255, 0.15); color: var(--cyan-vibrant); border: 1px solid var(--cyan-vibrant); }}
        .badge.mint {{ background: rgba(0, 191, 165, 0.15); color: var(--green-mint); border: 1px solid var(--green-mint); }}
        .badge.blue {{ background: rgba(0, 87, 255, 0.15); color: var(--blue-electric); border: 1px solid var(--blue-electric); }}

        .btn-kick {{ background: #D32F2F; color: #FFF; border: none; padding: 5px 10px; border-radius: 4px; cursor: pointer; font-size: 12px; font-weight: 600; }}
        .btn-kick:hover {{ background: #FF5252; }}
    </style>
    <script>
        async function kickRoom(code) {{
            if (confirm("Voulez-vous vraiment expulser la salle " + code + " ?")) {{
                await fetch('/api/rooms/' + code + '/kick', {{ method: 'POST' }});
                location.reload();
            }}
        }}
        // Actualisation automatique toutes les 5 secondes
        setTimeout(() => location.reload(), 5000);
    </script>
</head>
<body>
    <div class="header">
        <div class="header-title">
            <h1>COOPLINK</h1>
            <span>// CENTRAL GAMING HUB & RELAY</span>
        </div>
        <div class="status-pill">● PORT UDP 30024 ACTIF</div>
    </div>

    <div class="stats-grid">
        <div class="stat-card cyan">
            <div class="stat-label">Salons de Jeu Actifs</div>
            <div class="stat-value">{total_rooms}</div>
        </div>
        <div class="stat-card green">
            <div class="stat-label">Clients Connectés (IDs)</div>
            <div class="stat-value">{total_clients}</div>
        </div>
        <div class="stat-card">
            <div class="stat-label">Trafic Réseau Relayé</div>
            <div class="stat-value">{tx_mb:.2} Mo <span style="font-size:14px; color:#8E8E93;">/ {rx_mb:.2} Mo</span></div>
        </div>
        <div class="stat-card orange">
            <div class="stat-label">Temps de fonctionnement</div>
            <div class="stat-value">{uptime_min} min</div>
        </div>
    </div>

    <div class="section">
        <div class="section-header">
            <h2>🕹️ Salons Actifs sur le Serveur</h2>
        </div>
        <table>
            <thead>
                <tr>
                    <th>Code Salon</th>
                    <th>Hôte de Salle</th>
                    <th>Joueurs Connectés</th>
                    <th>ID Hôte</th>
                    <th>Actions</th>
                </tr>
            </thead>
            <tbody>
                {rooms_rows}
            </tbody>
        </table>
    </div>

    <div class="section">
        <div class="section-header">
            <h2>👥 Tous les Clients Connectés (IDs Uniques)</h2>
        </div>
        <table>
            <thead>
                <tr>
                    <th>Client ID</th>
                    <th>Pseudo</th>
                    <th>Salle</th>
                    <th>Rôle</th>
                    <th>IP Virtuelle</th>
                    <th>Endpoint Réel</th>
                    <th>Latence</th>
                </tr>
            </thead>
            <tbody>
                {clients_rows}
            </tbody>
        </table>
    </div>
</body>
</html>"#,
        total_rooms = rooms.len(),
        total_clients = total_clients,
        tx_mb = tx_mb,
        rx_mb = rx_mb,
        uptime_min = uptime_min,
        rooms_rows = rooms_rows,
        clients_rows = clients_rows,
    );

    Html(html)
}
