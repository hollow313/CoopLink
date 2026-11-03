use crate::config::ServerConfig;
use crate::hub::{ServerHub, TelemetryPayload};
use axum::{
    extract::{Path, State},
    http::{header, HeaderMap, StatusCode},
    response::{Html, IntoResponse, Json},
    routing::{get, post},
    Router,
};
use serde::Deserialize;
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
    let udp_port = config.udp_port;
    let web_port = config.web_port;
    let state = WebState { hub, config };
    let app = Router::new()
        .route("/", get(render_dashboard))
        .route("/logo.png", get(serve_logo))
        .route("/api/stats", get(get_stats_api))
        .route("/api/telemetry", post(post_telemetry_api))
        .route("/api/myip", get(get_my_ip_api))
        .route("/api/rooms/register", post(post_register_room_api))
        .route("/api/rooms/{code}", get(get_room_by_code_api))
        .with_state(state);

    // 1. Écoute HTTP sur le port 30024 (TCP) pour le Reverse Proxy https://cooplink.home-lh.ovh -> 30024
    let app_primary = app.clone();
    let bind_primary = format!("0.0.0.0:{}", udp_port);
    tokio::spawn(async move {
        if let Ok(listener) = tokio::net::TcpListener::bind(&bind_primary).await {
            info!(
                "CoopLink Web & Reverse Proxy Endpoint actif sur http://{} (TCP)",
                bind_primary
            );
            let _ = axum::serve(listener, app_primary).await;
        }
    });

    // 2. Écoute HTTP secondaire sur le port 30026 (TCP)
    if web_port != udp_port {
        let bind_secondary = format!("0.0.0.0:{}", web_port);
        if let Ok(listener) = tokio::net::TcpListener::bind(&bind_secondary).await {
            info!(
                "CoopLink Metrics Web Platform accessible sur http://{} (TCP)",
                bind_secondary
            );
            let _ = axum::serve(listener, app).await;
        }
    } else {
        std::future::pending::<()>().await;
    }
}

async fn serve_logo() -> impl IntoResponse {
    let bytes = include_bytes!("CoopLink.png");
    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "image/png"), (header::CACHE_CONTROL, "public, max-age=86400")],
        bytes.as_slice(),
    )
}

#[derive(Debug, Deserialize)]
pub struct RoomRegisterPayload {
    pub room_code: String,
    pub host_name: String,
    pub port: u16,
    pub public_ip: Option<String>,
}

fn extract_real_ip(headers: &HeaderMap) -> String {
    if let Some(forwarded) = headers.get("x-forwarded-for") {
        if let Ok(s) = forwarded.to_str() {
            if let Some(first) = s.split(',').next() {
                let trimmed = first.trim();
                if !trimmed.is_empty() {
                    return trimmed.to_string();
                }
            }
        }
    }
    if let Some(real_ip) = headers.get("x-real-ip") {
        if let Ok(s) = real_ip.to_str() {
            let trimmed = s.trim();
            if !trimmed.is_empty() {
                return trimmed.to_string();
            }
        }
    }
    "127.0.0.1".to_string()
}

async fn get_my_ip_api(headers: HeaderMap) -> impl IntoResponse {
    let ip = extract_real_ip(&headers);
    Json(json!({ "ip": ip }))
}

async fn post_register_room_api(
    State(state): State<WebState>,
    headers: HeaderMap,
    Json(payload): Json<RoomRegisterPayload>,
) -> impl IntoResponse {
    let client_ip = payload.public_ip.unwrap_or_else(|| extract_real_ip(&headers));
    state.hub.register_web_room(&payload.room_code, &payload.host_name, &client_ip, payload.port);
    info!(
        "Salon enregistré via Web API : '{}' par '{}' -> {}:{}",
        payload.room_code, payload.host_name, client_ip, payload.port
    );
    Json(json!({
        "success": true,
        "room_code": payload.room_code,
        "host_public_ip": client_ip,
        "host_port": payload.port
    }))
}

async fn get_room_by_code_api(
    State(state): State<WebState>,
    Path(code): Path<String>,
) -> impl IntoResponse {
    if let Some(room) = state.hub.lookup_room(&code) {
        Json(json!({
            "found": true,
            "room_code": room.room_code,
            "host_name": room.host_name,
            "host_public_ip": room.host_public_ip,
            "host_port": room.host_port,
            "client_count": room.client_count
        }))
    } else {
        Json(json!({
            "found": false,
            "error": "Salon introuvable ou expiré"
        }))
    }
}

async fn post_telemetry_api(
    State(state): State<WebState>,
    Json(payload): Json<TelemetryPayload>,
) -> impl IntoResponse {
    let ack = state.hub.process_telemetry(payload);
    Json(ack)
}

async fn get_stats_api(State(state): State<WebState>) -> impl IntoResponse {
    let uptime_secs = state.hub.start_time.elapsed().as_secs();
    let active_instances = state.hub.get_active_instances_count();
    let active_tunnels = state.hub.get_active_tunnels_count();
    let total_tunnels_mounted = state.hub.total_tunnels_mounted_ever.load(Ordering::Relaxed);
    let active_screen_shares = state.hub.get_active_screen_shares_count();
    let total_ids_issued = state.hub.get_total_ids_issued();
    let instances = state.hub.get_active_instances();

    Json(json!({
        "server_name": state.config.server_name,
        "udp_port": state.config.udp_port,
        "web_port": state.config.web_port,
        "uptime_secs": uptime_secs,
        "active_instances": active_instances,
        "active_tunnels": active_tunnels,
        "total_tunnels_mounted": total_tunnels_mounted,
        "active_screen_shares": active_screen_shares,
        "total_ids_issued": total_ids_issued,
        "instances": instances,
    }))
}

async fn render_dashboard(State(state): State<WebState>) -> Html<String> {
    let active_instances = state.hub.get_active_instances_count();
    let active_tunnels = state.hub.get_active_tunnels_count();
    let total_tunnels_mounted = state.hub.total_tunnels_mounted_ever.load(Ordering::Relaxed);
    let active_screen_shares = state.hub.get_active_screen_shares_count();
    let total_ids_issued = state.hub.get_total_ids_issued();
    let uptime_min = state.hub.start_time.elapsed().as_secs() / 60;

    let html = format!(
        r#"<!DOCTYPE html>
<html lang="fr">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>CoopLink — Metrics & Unique Instance ID Hub</title>
    <link rel="icon" type="image/png" href="/logo.png">
    <style>
        :root {{
            --bg-app: #F4F7FB;
            --card-bg: #FFFFFF;
            --subtle-bg: #F8FAFC;
            --border-subtle: #E2E8F0;
            --blue-electric: #0066FF;
            --cyan-vibrant: #00BCF2;
            --green-vivid: #00BA88;
            --indigo-accent: #7B3FE4;
            --orange-coral: #FF7A28;
            --text-primary: #0F172A;
            --text-secondary: #475569;
            --text-muted: #64748B;
        }}
        * {{
            box-sizing: border-box;
            margin: 0;
            padding: 0;
            font-family: 'Segoe UI Variable', 'Segoe UI', -apple-system, BlinkMacSystemFont, Roboto, Helvetica, Arial, sans-serif;
        }}
        body {{
            background: var(--bg-app);
            color: var(--text-primary);
            min-height: 100vh;
        }}
        /* Barre supérieure moderne Windows 11 */
        .topbar {{
            background: var(--card-bg);
            border-bottom: 1px solid var(--border-subtle);
            padding: 14px 32px;
            display: flex;
            align-items: center;
            justify-content: space-between;
            box-shadow: 0 1px 3px rgba(15, 23, 42, 0.03);
        }}
        .brand {{
            display: flex;
            align-items: center;
            gap: 14px;
        }}
        .brand img {{
            width: 38px;
            height: 38px;
            object-fit: contain;
        }}
        .brand-text h1 {{
            font-size: 20px;
            font-weight: 800;
            color: var(--text-primary);
            letter-spacing: -0.3px;
        }}
        .brand-text span {{
            font-size: 13px;
            font-weight: 600;
            color: var(--blue-electric);
        }}
        .topbar-right {{
            display: flex;
            align-items: center;
            gap: 12px;
        }}
        .status-pill {{
            display: inline-flex;
            align-items: center;
            gap: 8px;
            background: rgba(0, 186, 136, 0.12);
            border: 1px solid rgba(0, 186, 136, 0.35);
            color: var(--green-vivid);
            padding: 6px 14px;
            border-radius: 999px;
            font-size: 13px;
            font-weight: 700;
        }}
        .dot-green {{
            width: 9px;
            height: 9px;
            border-radius: 50%;
            background: var(--green-vivid);
            box-shadow: 0 0 0 4px rgba(0, 186, 136, 0.22);
        }}
        .lang-select {{
            background: var(--subtle-bg);
            border: 1px solid var(--border-subtle);
            color: var(--text-primary);
            padding: 6px 12px;
            border-radius: 8px;
            font-size: 13px;
            font-weight: 600;
            cursor: pointer;
            outline: none;
        }}
        .container {{
            max-width: 1220px;
            margin: 28px auto;
            padding: 0 24px;
        }}
        .metrics-grid {{
            display: grid;
            grid-template-columns: repeat(auto-fit, minmax(220px, 1fr));
            gap: 18px;
            margin-bottom: 24px;
        }}
        .metric-card {{
            background: var(--card-bg);
            border: 1px solid var(--border-subtle);
            border-radius: 14px;
            padding: 20px 22px;
            box-shadow: 0 2px 8px rgba(15, 23, 42, 0.03);
            position: relative;
            overflow: hidden;
        }}
        .metric-card::before {{
            content: '';
            position: absolute;
            top: 0;
            left: 0;
            right: 0;
            height: 4px;
            background: var(--blue-electric);
        }}
        .metric-card.green::before {{ background: var(--green-vivid); }}
        .metric-card.cyan::before {{ background: var(--cyan-vibrant); }}
        .metric-card.indigo::before {{ background: var(--indigo-accent); }}
        .metric-card.orange::before {{ background: var(--orange-coral); }}
        .metric-label {{
            font-size: 12.5px;
            font-weight: 700;
            text-transform: uppercase;
            letter-spacing: 0.4px;
            color: var(--text-muted);
            margin-bottom: 8px;
        }}
        .metric-value {{
            font-size: 34px;
            font-weight: 800;
            color: var(--text-primary);
            line-height: 1.1;
        }}
        .metric-sub {{
            margin-top: 6px;
            font-size: 12.5px;
            color: var(--text-secondary);
        }}
        .card-section {{
            background: var(--card-bg);
            border: 1px solid var(--border-subtle);
            border-radius: 14px;
            padding: 22px 24px;
            box-shadow: 0 2px 8px rgba(15, 23, 42, 0.03);
        }}
        .section-header {{
            display: flex;
            align-items: center;
            justify-content: space-between;
            margin-bottom: 16px;
        }}
        .section-title {{
            display: flex;
            align-items: center;
            gap: 10px;
            font-size: 16.5px;
            font-weight: 700;
            color: var(--text-primary);
        }}
        .section-badge {{
            background: #EFF6FF;
            color: var(--blue-electric);
            border: 1px solid #BFDBFE;
            padding: 4px 10px;
            border-radius: 999px;
            font-size: 12px;
            font-weight: 700;
        }}
        table {{
            width: 100%;
            border-collapse: collapse;
        }}
        th {{
            text-align: left;
            padding: 12px 14px;
            font-size: 12px;
            font-weight: 700;
            text-transform: uppercase;
            color: var(--text-muted);
            border-bottom: 1px solid var(--border-subtle);
            background: var(--subtle-bg);
        }}
        td {{
            padding: 14px;
            font-size: 14px;
            color: var(--text-primary);
            border-bottom: 1px solid var(--border-subtle);
        }}
        tr:last-child td {{
            border-bottom: none;
        }}
        .id-pill {{
            display: inline-flex;
            align-items: center;
            gap: 6px;
            background: #EFF6FF;
            color: var(--blue-electric);
            border: 1px solid #BFDBFE;
            padding: 4px 10px;
            border-radius: 8px;
            font-family: ui-monospace, SFMono-Regular, Consolas, monospace;
            font-weight: 700;
            font-size: 13px;
        }}
        .state-pill {{
            display: inline-flex;
            align-items: center;
            gap: 6px;
            padding: 4px 10px;
            border-radius: 999px;
            font-size: 12.5px;
            font-weight: 700;
        }}
        .state-pill.ready {{
            background: rgba(0, 186, 136, 0.12);
            color: var(--green-vivid);
        }}
        .state-pill.tunnel {{
            background: rgba(0, 102, 255, 0.12);
            color: var(--blue-electric);
        }}
        .empty-row {{
            text-align: center;
            color: var(--text-muted);
            padding: 28px !important;
        }}
    </style>
</head>
<body>
    <header class="topbar">
        <div class="brand">
            <img src="/logo.png" alt="CoopLink">
            <div class="brand-text">
                <h1>CoopLink Hub</h1>
                <span id="lbl-subtitle">Télémétrie des Tunnels P2P & Attribution d'IDs Uniques</span>
            </div>
        </div>
        <div class="topbar-right">
            <div class="status-pill">
                <span class="dot-green"></span>
                <span id="lbl-udp-ready">Prêt — UDP 30024 Actif</span>
            </div>
            <select id="lang-picker" class="lang-select" onchange="applyLang(this.value)">
                <option value="fr">FR — Français</option>
                <option value="en">EN — English</option>
                <option value="es">ES — Español</option>
                <option value="de">DE — Deutsch</option>
                <option value="pt">PT — Português</option>
            </select>
        </div>
    </header>

    <main class="container">
        <div class="metrics-grid">
            <div class="metric-card green">
                <div class="metric-label" id="lbl-m1">Instances Actives (En ligne)</div>
                <div class="metric-value" id="val-instances">{active_instances}</div>
                <div class="metric-sub" id="sub-m1">Logiciels CoopLink connectés</div>
            </div>
            <div class="metric-card">
                <div class="metric-label" id="lbl-m2">Tunnels P2P Montés (Actifs)</div>
                <div class="metric-value" id="val-tunnels">{active_tunnels}</div>
                <div class="metric-sub" id="sub-m2">100% P2P direct entre joueurs</div>
            </div>
            <div class="metric-card cyan">
                <div class="metric-label" id="lbl-m3">Total Tunnels Montés (Cumulé)</div>
                <div class="metric-value" id="val-total-tunnels">{total_tunnels_mounted}</div>
                <div class="metric-sub" id="sub-m3">Sessions initialisées depuis le démarrage</div>
            </div>
            <div class="metric-card indigo">
                <div class="metric-label" id="lbl-m4">Écrans Partagés Co-op</div>
                <div class="metric-value" id="val-shares">{active_screen_shares}</div>
                <div class="metric-sub" id="sub-m4">Sessions vidéo & manettes actives</div>
            </div>
            <div class="metric-card orange">
                <div class="metric-label" id="lbl-m5">IDs Uniques Attribués</div>
                <div class="metric-value" id="val-ids">{total_ids_issued}</div>
                <div class="metric-sub" id="sub-m5">Séquence d'identifiants #CL-1001+</div>
            </div>
        </div>

        <section class="card-section">
            <div class="section-header">
                <div class="section-title">
                    <span class="dot-green"></span>
                    <span id="lbl-table-title">Instances CoopLink & Identifiants Uniques Attribués</span>
                </div>
                <span class="section-badge" id="val-uptime">Uptime : {uptime_min} min</span>
            </div>
            <table>
                <thead>
                    <tr>
                        <th id="th-id">ID Unique Instance</th>
                        <th id="th-name">Utilisateur</th>
                        <th id="th-state">Statut</th>
                        <th id="th-tunnel">Tunnel P2P / Salon</th>
                        <th id="th-peers">Participants</th>
                    </tr>
                </thead>
                <tbody id="instances-tbody">
                    <tr><td colspan="5" class="empty-row">En attente de connexion d'une instance CoopLink...</td></tr>
                </tbody>
            </table>
        </section>
    </main>

    <script>
        const DICT = {{
            fr: {{
                subtitle: "Télémétrie des Tunnels P2P & Attribution d'IDs Uniques",
                udpReady: "Prêt — UDP 30024 Actif",
                m1: "Instances Actives (En ligne)", sub1: "Logiciels CoopLink connectés",
                m2: "Tunnels P2P Montés (Actifs)", sub2: "100% P2P direct entre joueurs",
                m3: "Total Tunnels Montés (Cumulé)", sub3: "Sessions initialisées depuis le démarrage",
                m4: "Écrans Partagés Co-op", sub4: "Sessions vidéo & manettes actives",
                m5: "IDs Uniques Attribués", sub5: "Séquence d'identifiants #CL-1001+",
                tableTitle: "Instances CoopLink & Identifiants Uniques Attribués",
                thId: "ID Unique Instance", thName: "Utilisateur", thState: "Statut", thTunnel: "Tunnel P2P / Salon", thPeers: "Participants",
                empty: "Aucune instance active pour le moment. Lancez CoopLink sur votre PC.",
                ready: "Prêt", tunnelMounted: "Tunnel P2P Monté", none: "En veille"
            }},
            en: {{
                subtitle: "P2P Tunnel Telemetry & Unique Instance ID Assignment",
                udpReady: "Ready — UDP 30024 Active",
                m1: "Active Instances (Online)", sub1: "Connected CoopLink apps",
                m2: "Mounted P2P Tunnels (Active)", sub2: "100% Direct P2P between players",
                m3: "Total Tunnels Mounted", sub3: "Cumulative sessions since start",
                m4: "Co-op Shared Screens", sub4: "Active video & controller sessions",
                m5: "Unique IDs Issued", sub5: "Identifier sequence #CL-1001+",
                tableTitle: "CoopLink Instances & Assigned Unique IDs",
                thId: "Unique Instance ID", thName: "Player", thState: "Status", thTunnel: "P2P Tunnel / Room", thPeers: "Players",
                empty: "No active instance yet. Launch CoopLink on your PC.",
                ready: "Ready", tunnelMounted: "P2P Tunnel Mounted", none: "Idle"
            }},
            es: {{
                subtitle: "Telemetría de Túneles P2P y Asignación de IDs Únicos",
                udpReady: "Listo — UDP 30024 Activo",
                m1: "Instancias Activas", sub1: "Aplicaciones CoopLink conectadas",
                m2: "Túneles P2P Montados", sub2: "100% P2P directo entre jugadores",
                m3: "Total Túneles Montados", sub3: "Sesiones iniciadas acumuladas",
                m4: "Pantallas Compartidas Co-op", sub4: "Sesiones de vídeo y mandos activas",
                m5: "IDs Únicos Asignados", sub5: "Secuencia #CL-1001+",
                tableTitle: "Instancias de CoopLink e IDs Únicos Asignados",
                thId: "ID Único", thName: "Jugador", thState: "Estado", thTunnel: "Túnel P2P / Sala", thPeers: "Jugadores",
                empty: "Ninguna instancia activa. Inicia CoopLink en tu PC.",
                ready: "Listo", tunnelMounted: "Túnel P2P Montado", none: "En espera"
            }},
            de: {{
                subtitle: "P2P-Tunnel-Telemetrie & Eindeutige Instanz-IDs",
                udpReady: "Bereit — UDP 30024 Aktiv",
                m1: "Aktive Instanzen", sub1: "Verbundene CoopLink-Clients",
                m2: "Aktive P2P-Tunnel", sub2: "100% Direktes P2P zwischen Spielern",
                m3: "Gesamte P2P-Tunnel", sub3: "Kumulierte Sitzungen seit Start",
                m4: "Co-op Bildschirmfreigaben", sub4: "Aktive Video- & Controller-Sitzungen",
                m5: "Vergebene Eindeutige IDs", sub5: "Sequenz #CL-1001+",
                tableTitle: "CoopLink-Instanzen & Zugewiesene IDs",
                thId: "Eindeutige ID", thName: "Spieler", thState: "Status", thTunnel: "P2P-Tunnel / Raum", thPeers: "Spieler",
                empty: "Keine aktive Instanz. Starte CoopLink auf deinem PC.",
                ready: "Bereit", tunnelMounted: "P2P-Tunnel Aktiv", none: "Bereitschaft"
            }},
            pt: {{
                subtitle: "Telemetria de Túneis P2P e Atribuição de IDs Únicos",
                udpReady: "Pronto — UDP 30024 Ativo",
                m1: "Instâncias Ativas", sub1: "Aplicações CoopLink ligadas",
                m2: "Túneis P2P Montados", sub2: "100% P2P direto entre jogadores",
                m3: "Total de Túneis Montados", sub3: "Sessões acumuladas desde o início",
                m4: "Ecrãs Partilhados Co-op", sub4: "Sessões de vídeo e comando ativas",
                m5: "IDs Únicos Atribuídos", sub5: "Sequência #CL-1001+",
                tableTitle: "Instâncias CoopLink e IDs Únicos Atribuídos",
                thId: "ID Único", thName: "Jogador", thState: "Estado", thTunnel: "Túnel P2P / Sala", thPeers: "Jogadores",
                empty: "Nenhuma instância ativa. Inicie o CoopLink no seu PC.",
                ready: "Pronto", tunnelMounted: "Túnel P2P Montado", none: "Em espera"
            }}
        }};

        let currentLang = localStorage.getItem('cooplink_lang') || 'fr';
        document.getElementById('lang-picker').value = currentLang;

        function applyLang(lang) {{
            currentLang = lang;
            localStorage.setItem('cooplink_lang', lang);
            const t = DICT[lang] || DICT.fr;
            document.getElementById('lbl-subtitle').textContent = t.subtitle;
            document.getElementById('lbl-udp-ready').textContent = t.udpReady;
            document.getElementById('lbl-m1').textContent = t.m1;
            document.getElementById('sub-m1').textContent = t.sub1;
            document.getElementById('lbl-m2').textContent = t.m2;
            document.getElementById('sub-m2').textContent = t.sub2;
            document.getElementById('lbl-m3').textContent = t.m3;
            document.getElementById('sub-m3').textContent = t.sub3;
            document.getElementById('lbl-m4').textContent = t.m4;
            document.getElementById('sub-m4').textContent = t.sub4;
            document.getElementById('lbl-m5').textContent = t.m5;
            document.getElementById('sub-m5').textContent = t.sub5;
            document.getElementById('lbl-table-title').textContent = t.tableTitle;
            document.getElementById('th-id').textContent = t.thId;
            document.getElementById('th-name').textContent = t.thName;
            document.getElementById('th-state').textContent = t.thState;
            document.getElementById('th-tunnel').textContent = t.thTunnel;
            document.getElementById('th-peers').textContent = t.thPeers;
            refreshStats();
        }}

        async function refreshStats() {{
            try {{
                const res = await fetch('/api/stats');
                const data = await res.json();
                const t = DICT[currentLang] || DICT.fr;

                document.getElementById('val-instances').textContent = data.active_instances;
                document.getElementById('val-tunnels').textContent = data.active_tunnels;
                document.getElementById('val-total-tunnels').textContent = data.total_tunnels_mounted;
                document.getElementById('val-shares').textContent = data.active_screen_shares;
                document.getElementById('val-ids').textContent = data.total_ids_issued;
                document.getElementById('val-uptime').textContent = 'Uptime : ' + Math.floor(data.uptime_secs / 60) + ' min';

                const tbody = document.getElementById('instances-tbody');
                if (!data.instances || data.instances.length === 0) {{
                    tbody.innerHTML = `<tr><td colspan="5" class="empty-row">${{t.empty}}</td></tr>`;
                }} else {{
                    tbody.innerHTML = data.instances.map(inst => {{
                        const stateHtml = inst.tunnel_mounted
                            ? `<span class="state-pill tunnel"><span class="dot-green"></span>${{t.tunnelMounted}}</span>`
                            : `<span class="state-pill ready"><span class="dot-green"></span>${{t.ready}}</span>`;
                        const roomHtml = inst.tunnel_mounted ? `<strong>${{inst.room_code}}</strong>` : `<span style="color:#64748B">${{t.none}}</span>`;
                        const peersHtml = inst.tunnel_mounted ? `${{Math.max(1, inst.peer_count)}}` : `1`;
                        return `<tr>
                            <td><span class="id-pill">${{inst.assigned_id}}</span></td>
                            <td><strong>${{inst.player_name}}</strong></td>
                            <td>${{stateHtml}}</td>
                            <td>${{roomHtml}}</td>
                            <td>${{peersHtml}}</td>
                        </tr>`;
                    }}).join('');
                }}
            }} catch (e) {{}}
        }}

        applyLang(currentLang);
        setInterval(refreshStats, 2000);
    </script>
</body>
</html>"#,
        active_instances = active_instances,
        active_tunnels = active_tunnels,
        total_tunnels_mounted = total_tunnels_mounted,
        active_screen_shares = active_screen_shares,
        total_ids_issued = total_ids_issued,
        uptime_min = uptime_min,
    );

    Html(html)
}
