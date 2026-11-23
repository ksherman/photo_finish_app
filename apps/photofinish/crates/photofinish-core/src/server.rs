use axum::extract::{Path as AxumPath, Query, State};
use axum::http::{header, StatusCode};
use axum::response::{Html, IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use std::net::{IpAddr, SocketAddr, UdpSocket};
use std::sync::{Arc, RwLock};
use tokio::sync::oneshot;
use tower_http::cors::CorsLayer;

use crate::event::{EventBundle, EventSummary};
use crate::{Error, Result};

#[derive(Clone, Default)]
pub struct SharedActiveEvent {
    inner: Arc<RwLock<Option<Arc<EventBundle>>>>,
}

impl SharedActiveEvent {
    pub fn new(initial: Option<Arc<EventBundle>>) -> Self {
        Self {
            inner: Arc::new(RwLock::new(initial)),
        }
    }

    pub fn set(&self, bundle: Option<Arc<EventBundle>>) {
        let mut guard = self.inner.write().unwrap();
        *guard = bundle;
    }

    pub fn get(&self) -> Option<Arc<EventBundle>> {
        self.inner.read().unwrap().clone()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LanServerStatus {
    pub running: bool,
    pub port: u16,
    pub lan_ip: Option<String>,
    pub lan_url: Option<String>,
}

pub struct EmbeddedServer {
    port: u16,
    shutdown_tx: Option<oneshot::Sender<()>>,
}

impl EmbeddedServer {
    pub async fn start(port: u16, active_event: SharedActiveEvent) -> Result<Self> {
        let app = build_router(active_event);
        let addr = SocketAddr::from(([0, 0, 0, 0], port));
        let listener = tokio::net::TcpListener::bind(addr)
            .await
            .map_err(|e| Error::Io(format!("Failed to bind {}: {}", addr, e)))?;
        let bound_port = listener
            .local_addr()
            .map_err(|e| Error::Io(e.to_string()))?
            .port();

        let (tx, rx) = oneshot::channel::<()>();
        tokio::spawn(async move {
            let _ = axum::serve(listener, app)
                .with_graceful_shutdown(async move {
                    let _ = rx.await;
                })
                .await;
        });

        Ok(Self {
            port: bound_port,
            shutdown_tx: Some(tx),
        })
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    pub fn status(&self) -> LanServerStatus {
        let lan_ip = detect_lan_ip();
        let lan_url = lan_ip.as_ref().map(|ip| format!("http://{}:{}", ip, self.port));
        LanServerStatus {
            running: self.shutdown_tx.is_some(),
            port: self.port,
            lan_ip,
            lan_url,
        }
    }

    pub fn stop(&mut self) {
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(());
        }
    }
}

impl Drop for EmbeddedServer {
    fn drop(&mut self) {
        self.stop();
    }
}

pub fn build_router(active_event: SharedActiveEvent) -> Router {
    Router::new()
        .route("/", get(kiosk_index))
        .route("/api/status", get(api_status))
        .route("/api/subjects", get(api_search_subjects))
        .route("/api/subjects/{id}/photos", get(api_subject_photos))
        .route("/photos/thumbnail/{id}", get(serve_thumbnail))
        .route("/photos/preview/{id}", get(serve_preview))
        .layer(CorsLayer::permissive())
        .with_state(active_event)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ApiStatusResponse {
    service: &'static str,
    version: &'static str,
    active_event: Option<EventSummary>,
}

async fn api_status(State(active): State<SharedActiveEvent>) -> impl IntoResponse {
    let summary = active.get().and_then(|b| b.summary().ok());
    Json(ApiStatusResponse {
        service: "photofinish",
        version: env!("CARGO_PKG_VERSION"),
        active_event: summary,
    })
}

#[derive(Deserialize)]
struct SearchParams {
    #[serde(default)]
    q: String,
    #[serde(default = "default_limit")]
    limit: i64,
}

fn default_limit() -> i64 {
    25
}

async fn api_search_subjects(
    State(active): State<SharedActiveEvent>,
    Query(params): Query<SearchParams>,
) -> Response {
    let Some(bundle) = active.get() else {
        return (StatusCode::SERVICE_UNAVAILABLE, "No active event open").into_response();
    };
    match bundle.search_subjects(&params.q, params.limit.clamp(1, 100)) {
        Ok(subjects) => Json(subjects).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

async fn api_subject_photos(
    State(active): State<SharedActiveEvent>,
    AxumPath(subject_id): AxumPath<String>,
) -> Response {
    let Some(bundle) = active.get() else {
        return (StatusCode::SERVICE_UNAVAILABLE, "No active event open").into_response();
    };
    match bundle.list_subject_photos(&subject_id) {
        Ok(photos) => Json(photos).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

async fn serve_thumbnail(
    State(active): State<SharedActiveEvent>,
    AxumPath(photo_id): AxumPath<String>,
) -> Response {
    serve_derivative(active, &photo_id, false).await
}

async fn serve_preview(
    State(active): State<SharedActiveEvent>,
    AxumPath(photo_id): AxumPath<String>,
) -> Response {
    serve_derivative(active, &photo_id, true).await
}

async fn serve_derivative(active: SharedActiveEvent, photo_id: &str, preview: bool) -> Response {
    let Some(bundle) = active.get() else {
        return StatusCode::SERVICE_UNAVAILABLE.into_response();
    };
    let path = match bundle.get_photo_derivative_path(photo_id, preview) {
        Ok(Some(p)) => p,
        Ok(None) => return StatusCode::NOT_FOUND.into_response(),
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };

    match tokio::fs::read(&path).await {
        Ok(bytes) => (
            StatusCode::OK,
            [
                (header::CONTENT_TYPE, "image/jpeg"),
                (header::CACHE_CONTROL, "public, max-age=31536000, immutable"),
            ],
            bytes,
        )
            .into_response(),
        Err(_) => StatusCode::NOT_FOUND.into_response(),
    }
}

/// Lightweight built-in kiosk landing page served to iPads on the venue LAN.
/// Will be replaced with the compiled Vue Kiosk SPA embedded via rust-embed.
async fn kiosk_index(State(active): State<SharedActiveEvent>) -> Html<String> {
    let title = active
        .get()
        .and_then(|b| b.summary().ok())
        .map(|s| format!("{} — PhotoFinish Kiosk", s.name))
        .unwrap_or_else(|| "PhotoFinish Kiosk".to_string());

    Html(format!(
        r#"<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8" />
  <meta name="viewport" content="width=device-width, initial-scale=1" />
  <title>{title}</title>
  <style>
    body {{ font-family: -apple-system, BlinkMacSystemFont, sans-serif; margin: 0; background: #0f172a; color: #f8fafc; }}
    .wrap {{ max-width: 780px; margin: 48px auto; padding: 24px; }}
    .card {{ background: #1e293b; border-radius: 16px; padding: 28px; box-shadow: 0 10px 30px rgba(0,0,0,0.3); }}
    input {{ width: 100%; padding: 14px 18px; border-radius: 999px; border: 2px solid #334155; background: #0f172a; color: #fff; font-size: 18px; box-sizing: border-box; }}
    ul {{ list-style: none; padding: 0; margin: 16px 0 0; }}
    li {{ padding: 12px 16px; border-bottom: 1px solid #334155; display: flex; justify-content: space-between; cursor: pointer; }}
    li:hover {{ background: #334155; border-radius: 8px; }}
    .grid {{ display: grid; grid-template-columns: repeat(auto-fill, minmax(140px, 1fr)); gap: 8px; margin-top: 20px; }}
    .grid img {{ width: 100%; aspect-ratio: 2/3; object-fit: cover; border-radius: 8px; background: #334155; }}
    .muted {{ color: #94a3b8; font-size: 14px; }}
  </style>
</head>
<body>
  <div class="wrap">
    <div class="card">
      <h1 style="margin:0 0 6px">{title}</h1>
      <p class="muted" id="meta">Connecting to local PhotoFinish server...</p>
      <input id="q" type="search" placeholder="Search by name or number..." autocomplete="off" />
      <ul id="results"></ul>
      <div id="gallery" class="grid"></div>
    </div>
  </div>
  <script>
    const q = document.getElementById('q');
    const results = document.getElementById('results');
    const gallery = document.getElementById('gallery');
    const meta = document.getElementById('meta');

    fetch('/api/status').then(r => r.json()).then(data => {{
      if (data.activeEvent) {{
        meta.textContent = `${{data.activeEvent.photoCounts.ready}} photos ready • Search by ${{data.activeEvent.subjectLabel.toLowerCase()}}`;
      }} else {{
        meta.textContent = 'Waiting for operator to open an event...';
      }}
    }});

    let timer;
    q.addEventListener('input', () => {{
      clearTimeout(timer);
      timer = setTimeout(async () => {{
        const val = q.value.trim();
        gallery.innerHTML = '';
        if (!val) {{ results.innerHTML = ''; return; }}
        const list = await fetch('/api/subjects?q=' + encodeURIComponent(val)).then(r => r.json());
        results.innerHTML = list.map(s =>
          `<li data-id="${{s.id}}"><span><strong>${{s.displayName}}</strong></span><span class="muted">${{s.photoCount}} photos</span></li>`
        ).join('') || '<li class="muted">No matches found</li>';
      }}, 200);
    }});

    results.addEventListener('click', async (e) => {{
      const li = e.target.closest('li[data-id]');
      if (!li) return;
      const photos = await fetch(`/api/subjects/${{li.dataset.id}}/photos`).then(r => r.json());
      gallery.innerHTML = photos.map(p =>
        `<a href="/photos/preview/${{p.id}}" target="_blank"><img src="/photos/thumbnail/${{p.id}}" loading="lazy" /></a>`
      ).join('') || '<p class="muted">No ready photos yet</p>';
    }});
  </script>
</body>
</html>"#
    ))
}

/// Best-effort local LAN IPv4 discovery without making any external network connection.
pub fn detect_lan_ip() -> Option<String> {
    let socket = UdpSocket::bind("0.0.0.0:0").ok()?;
    socket.connect("192.0.2.1:80").ok()?;
    match socket.local_addr().ok()?.ip() {
        IpAddr::V4(ip) if !ip.is_loopback() => Some(ip.to_string()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::CreateEventParams;
    use axum::body::Body;
    use axum::http::Request;
    use tempfile::tempdir;
    use tower::ServiceExt;

    #[tokio::test]
    async fn serves_status_and_subject_search() {
        let dir = tempdir().unwrap();
        let bundle = Arc::new(
            EventBundle::create(CreateEventParams {
                root_path: dir.path().join("sample-meet"),
                name: "Test Meet".into(),
                description: None,
                preset_id: Some("gymnastics".into()),
                subject_label: None,
                folder_levels: None,
                order_code: Some("TST".into()),
                station_code: Some("A".into()),
                tax_rate_basis_points: None,
                scaffold: None,
            })
            .unwrap(),
        );
        bundle
            .add_subject(Some("1022"), "Kevin", Some("S"), Some("1A"), None)
            .unwrap();

        let app = build_router(SharedActiveEvent::new(Some(bundle)));

        let resp = app
            .clone()
            .oneshot(Request::builder().uri("/api/status").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/api/subjects?q=kevin")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }
}
