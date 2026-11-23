use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{EditDiff, ModelManifest, ShieldConfig};

/// Version tag for TinyCut's open client-server HTTP/JSON wire contract.
///
/// Any compatible backend (official TinyCut Cloud Boost or self-hosted) implements
/// these `/v1/*` payload shapes while keeping server storage, billing, GPU workers,
/// and training pipelines decoupled from the open-source client.
pub const PROTOCOL_VERSION: &str = "v1";

/// Batch of locally PII-scrubbed, opt-in subtitle corrections sent to
/// `POST /v1/telemetry/text-diffs`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TextDiffTelemetryBatch {
    pub anonymous_client_id: String,
    pub app_version: String,
    pub asr_model_version: String,
    pub llm_model_version: Option<String>,
    pub locale: String,
    pub content_domain: Option<String>,
    pub corrections: Vec<EditDiff>,
}

/// Request to upload a user-confirmed visual false-positive ("hard negative") crop
/// (`<= 256x256px`) to `POST /v1/telemetry/vision-negatives`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct VisionHardNegativeRequest {
    pub anonymous_client_id: String,
    pub app_version: String,
    pub model_version: String,
    pub rejected_label: String,
    pub crop_width: u32,
    pub crop_height: u32,
}

/// Presigned upload slot returned for a vision hard-negative crop.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct VisionHardNegativeUploadResponse {
    pub upload_id: Uuid,
    pub upload_url: String,
    pub expires_in_secs: u64,
}

/// Type of heavy inference job offloaded via Smart Proxy Offloading.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CloudBoostTaskKind {
    Transcribe,
    FixSubtitles,
    DetectPrivacy,
}

/// Request payload for `POST /v1/jobs/cloud-boost`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateCloudBoostJobRequest {
    pub task: CloudBoostTaskKind,
    pub locale: String,
    pub media_duration_ms: u64,
    pub proxy_content_type: String,
    pub proxy_size_bytes: u64,
    #[serde(default)]
    pub glossary_terms: Vec<String>,
    #[serde(default)]
    pub shield_config: Option<ShieldConfig>,
}

/// Response payload from `POST /v1/jobs/cloud-boost` containing an ephemeral
/// upload URL for the smart proxy and the SSE progress stream path.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CreateCloudBoostJobResponse {
    pub job_id: Uuid,
    pub upload_url: String,
    pub upload_expires_in_secs: u64,
    pub stream_path: String,
}

/// Server-Sent Event (SSE) progress frame streamed by `GET /v1/jobs/{job_id}/stream`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct JobProgressEvent {
    pub job_id: Uuid,
    pub stage: String,
    pub percent: u8,
    pub message: String,
}

/// Response wrapper for `GET /v1/models/manifest`.
pub type ModelManifestResponse = ModelManifest;
