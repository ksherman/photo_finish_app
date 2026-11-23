use axum::extract::{Path as AxumPath, Query, State};
use axum::http::{header, StatusCode, Uri};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use rust_embed::Embed;
use serde::{Deserialize, Serialize};
use std::net::{IpAddr, SocketAddr, UdpSocket};
#[cfg(debug_assertions)]
use std::path::PathBuf;
use std::sync::{Arc, RwLock};
use tokio::sync::oneshot;
use tower_http::cors::CorsLayer;

use crate::event::{EventBundle, EventSummary, PhotoRecord, SubjectRecord};
use crate::orders::{CreateOrderRequest, ProductRecord};
use crate::{Error, Result};

#[derive(Embed)]
#[folder = "kiosk-dist/"]
struct KioskAssets;

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
        .route("/api/status", get(api_status))
        .route("/api/kiosk/showcase", get(api_showcase))
        .route("/api/subjects", get(api_search_subjects))
        .route("/api/subjects/{id}", get(api_subject_detail))
        .route("/api/subjects/{id}/photos", get(api_subject_photos))
        .route("/api/products", get(api_products))
        .route("/api/orders", post(api_place_order))
        .route("/api/orders/{order_number}", get(api_get_order))
        .route("/photos/thumbnail/{id}", get(serve_thumbnail))
        .route("/photos/preview/{id}", get(serve_preview))
        .fallback(serve_kiosk_spa)
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

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ShowcaseResponse {
    photo_ids: Vec<String>,
}

async fn api_showcase(State(active): State<SharedActiveEvent>) -> Response {
    let Some(bundle) = active.get() else {
        return Json(ShowcaseResponse { photo_ids: vec![] }).into_response();
    };
    let photo_ids = bundle.list_showcase_photo_ids(24).unwrap_or_default();
    Json(ShowcaseResponse { photo_ids }).into_response()
}

#[derive(Deserialize)]
struct SearchParams {
    #[serde(default)]
    q: String,
    #[serde(default = "default_limit")]
    limit: i64,
}

fn default_limit() -> i64 {
    5000
}

async fn api_search_subjects(
    State(active): State<SharedActiveEvent>,
    Query(params): Query<SearchParams>,
) -> Response {
    let Some(bundle) = active.get() else {
        return (StatusCode::SERVICE_UNAVAILABLE, "No active event open").into_response();
    };
    match bundle.search_subjects(&params.q, params.limit.clamp(1, 10_000)) {
        Ok(subjects) => Json(subjects).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SubjectDetailResponse {
    subject: SubjectRecord,
    photos: Vec<PhotoRecord>,
    sections: Vec<String>,
}

async fn api_subject_detail(
    State(active): State<SharedActiveEvent>,
    AxumPath(subject_id): AxumPath<String>,
) -> Response {
    let Some(bundle) = active.get() else {
        return (StatusCode::SERVICE_UNAVAILABLE, "No active event open").into_response();
    };
    let subject = match bundle.get_subject(&subject_id) {
        Ok(Some(s)) => s,
        Ok(None) => return (StatusCode::NOT_FOUND, "Subject not found").into_response(),
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    };
    let photos = match bundle.list_subject_photos(&subject_id) {
        Ok(p) => p,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    };

    let mut sections: Vec<String> = Vec::new();
    for p in &photos {
        if !sections.contains(&p.section_label) {
            sections.push(p.section_label.clone());
        }
    }

    Json(SubjectDetailResponse {
        subject,
        photos,
        sections,
    })
    .into_response()
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

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ProductsResponse {
    products: Vec<ProductRecord>,
    tax_rate_basis_points: i64,
    subject_label: String,
    event_name: String,
}

async fn api_products(State(active): State<SharedActiveEvent>) -> Response {
    let Some(bundle) = active.get() else {
        return (StatusCode::SERVICE_UNAVAILABLE, "No active event open").into_response();
    };
    let summary = match bundle.summary() {
        Ok(s) => s,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    };
    let products = match bundle.list_products(false) {
        Ok(p) => p,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    };
    Json(ProductsResponse {
        products,
        tax_rate_basis_points: summary.tax_rate_basis_points,
        subject_label: summary.subject_label,
        event_name: summary.name,
    })
    .into_response()
}

#[derive(Serialize)]
struct ApiErrorBody {
    error: String,
}

async fn api_place_order(
    State(active): State<SharedActiveEvent>,
    Json(req): Json<CreateOrderRequest>,
) -> Response {
    let Some(bundle) = active.get() else {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(ApiErrorBody {
                error: "No active event open".into(),
            }),
        )
            .into_response();
    };
    match bundle.place_order(req) {
        Ok(order) => (StatusCode::CREATED, Json(order)).into_response(),
        Err(Error::InvalidInput(msg)) => (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(ApiErrorBody { error: msg }),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiErrorBody {
                error: e.to_string(),
            }),
        )
            .into_response(),
    }
}

async fn api_get_order(
    State(active): State<SharedActiveEvent>,
    AxumPath(order_number): AxumPath<String>,
) -> Response {
    let Some(bundle) = active.get() else {
        return (StatusCode::SERVICE_UNAVAILABLE, "No active event open").into_response();
    };
    match bundle.get_order_by_number(&order_number) {
        Ok(Some(order)) => Json(order).into_response(),
        Ok(None) => (StatusCode::NOT_FOUND, "Order not found").into_response(),
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

/// Serves the compiled Vue Kiosk SPA (embedded via `rust-embed` in release builds,
/// and reading fresh `kiosk-dist/` files from disk first in debug builds for instant UI iteration).
async fn serve_kiosk_spa(uri: Uri) -> Response {
    let raw_path = uri.path().trim_start_matches('/');
    if raw_path.starts_with("api/") || raw_path.starts_with("photos/") {
        return StatusCode::NOT_FOUND.into_response();
    }

    let candidate = if raw_path.is_empty() {
        "index.html"
    } else {
        raw_path
    };

    if let Some(resp) = read_kiosk_asset(candidate).await {
        return resp;
    }

    // SPA client-side route fallback (e.g. /subject/sub_xxx, /cart, /checkout, /order/XLR-A0001)
    if let Some(resp) = read_kiosk_asset("index.html").await {
        return resp;
    }

    (StatusCode::NOT_FOUND, "Kiosk UI bundle not found").into_response()
}

async fn read_kiosk_asset(rel_path: &str) -> Option<Response> {
    if rel_path.contains("..") {
        return None;
    }
    let mime = mime_guess::from_path(rel_path)
        .first_or_octet_stream()
        .to_string();
    let cache = if rel_path.starts_with("assets/") {
        "public, max-age=31536000, immutable"
    } else {
        "no-cache"
    };

    // In debug builds, check the live kiosk-dist directory first so `pnpm build:kiosk`
    // updates the LAN server immediately without recompiling Rust.
    #[cfg(debug_assertions)]
    {
        let disk_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("kiosk-dist")
            .join(rel_path);
        if let Ok(bytes) = tokio::fs::read(&disk_path).await {
            return Some(
                (
                    StatusCode::OK,
                    [
                        (header::CONTENT_TYPE, mime),
                        (header::CACHE_CONTROL, cache.to_string()),
                    ],
                    bytes,
                )
                    .into_response(),
            );
        }
    }

    KioskAssets::get(rel_path).map(|file| {
        (
            StatusCode::OK,
            [
                (header::CONTENT_TYPE, mime),
                (header::CACHE_CONTROL, cache.to_string()),
            ],
            file.data.into_owned(),
        )
            .into_response()
    })
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
    use crate::orders::{CreateOrderItemInput, OrderRecord, ShippingAddress};
    use axum::body::{to_bytes, Body};
    use axum::http::Request;
    use tempfile::tempdir;
    use tower::ServiceExt;

    #[tokio::test]
    async fn serves_numerically_sorted_subjects_and_places_order_with_square_qr() {
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
                tax_rate_basis_points: Some(850),
                scaffold: None,
            })
            .unwrap(),
        );
        bundle
            .add_subject(Some("1000"), "Zoe", Some("A"), Some("Session 1A"), None)
            .unwrap();
        let sub105 = bundle
            .add_subject(Some("105"), "Julia", Some("B"), Some("Session 1A"), None)
            .unwrap();
        bundle
            .add_subject(Some("1050"), "Mia", Some("C"), Some("Session 1A"), None)
            .unwrap();

        let app = build_router(SharedActiveEvent::new(Some(Arc::clone(&bundle))));

        // Numeric ordering + exact match ranking: "105" must come BEFORE "1050" and "1000"
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/api/subjects?q=10")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = to_bytes(resp.into_body(), 1024 * 1024).await.unwrap();
        let list: Vec<SubjectRecord> = serde_json::from_slice(&body).unwrap();
        let nums: Vec<&str> = list.iter().filter_map(|s| s.number.as_deref()).collect();
        assert_eq!(nums, vec!["105", "1000", "1050"]);

        // Place an order with Full Digital + USB Add-On + Print
        let products = bundle.list_products(false).unwrap();
        let full_dig = products.iter().find(|p| p.product_type == "full_digital").unwrap();
        let usb = products.iter().find(|p| p.product_type == "usb_addon").unwrap();
        let print8 = products
            .iter()
            .find(|p| p.product_type == "print" && p.size.as_deref() == Some("8x10"))
            .unwrap();

        let order_req = CreateOrderRequest {
            customer_name: "Jane Parent".into(),
            customer_email: Some("jane@example.com".into()),
            customer_phone: Some("555-1234".into()),
            shipping_address: Some(ShippingAddress {
                street: "123 Main St".into(),
                street2: None,
                city: "Chicago".into(),
                state: "IL".into(),
                zip: "60601".into(),
            }),
            notes: None,
            items: vec![
                CreateOrderItemInput {
                    product_id: full_dig.id.clone(),
                    subject_id: Some(sub105.id.clone()),
                    photo_id: None,
                    quantity: 1,
                },
                CreateOrderItemInput {
                    product_id: usb.id.clone(),
                    subject_id: Some(sub105.id.clone()),
                    photo_id: None,
                    quantity: 1,
                },
                CreateOrderItemInput {
                    product_id: print8.id.clone(),
                    subject_id: Some(sub105.id.clone()),
                    photo_id: None,
                    quantity: 1,
                },
            ],
        };

        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/orders")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&order_req).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::CREATED);
        let body = to_bytes(resp.into_body(), 1024 * 1024).await.unwrap();
        let order: OrderRecord = serde_json::from_slice(&body).unwrap();
        assert_eq!(order.order_number, "TST-A0001");
        assert_eq!(order.subtotal_cents, 8500 + 1000 + 2800);
        assert_eq!(order.tax_cents, 1046);
        assert_eq!(order.total_cents, 13346);
        assert!(order.square_pos_url.starts_with("square-commerce-v1://payment/create?data="));
    }
}
