use axum::{
    Json, Router,
    http::StatusCode,
    routing::{delete, get, post},
};
use tinycut_core_domain::{ModelManifest, TextDiffTelemetryBatch, default_model_manifest};

async fn health() -> &'static str {
    "ok"
}

async fn get_manifest() -> Json<ModelManifest> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    Json(default_model_manifest(now))
}

async fn ingest_text_diffs(Json(batch): Json<TextDiffTelemetryBatch>) -> StatusCode {
    tracing::info!(
        anon_id = %batch.anonymous_client_id,
        count = batch.corrections.len(),
        "received opt-in text diff batch"
    );
    StatusCode::ACCEPTED
}

async fn delete_my_telemetry() -> StatusCode {
    // Phase 2: delete all Postgres rows & R2 objects matching authenticated/header anonymous_client_id.
    StatusCode::NO_CONTENT
}

pub fn build_router() -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/v1/models/manifest", get(get_manifest))
        .route("/v1/telemetry/text-diffs", post(ingest_text_diffs))
        .route("/v1/telemetry/me", delete(delete_my_telemetry))
}

#[tokio::main]
async fn main() {
    let app = build_router();
    let listener = tokio::net::TcpListener::bind("0.0.0.0:8080")
        .await
        .expect("bind 0.0.0.0:8080");
    axum::serve(listener, app).await.expect("serve tinycut-server");
}
