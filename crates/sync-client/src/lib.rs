#[cfg(feature = "cloud-boost")]
use async_trait::async_trait;
#[cfg(feature = "cloud-boost")]
use tinycut_core_domain::{
    DomainError, DomainResult, InferenceBackend, MediaRef, RedactionTrack, ShieldConfig,
    Transcript,
};
use tinycut_core_domain::Project;
#[cfg(feature = "telemetry")]
use tinycut_core_domain::{EditDiff, TextDiffTelemetryBatch};

/// Environment variable used to override the Cloud Boost / telemetry API endpoint
/// (e.g. `http://127.0.0.1:8080` when co-developing with a local control-plane server).
pub const API_BASE_URL_ENV: &str = "TINYCUT_API_BASE_URL";

/// On-device PII scrubber run BEFORE any opt-in text diff leaves the device.
/// Redacts emails, URLs, API key prefixes (`sk-...`), and long numeric sequences (phones/bank/IDs).
pub fn scrub_pii(input: &str) -> String {
    input
        .split_whitespace()
        .map(scrub_token)
        .collect::<Vec<_>>()
        .join(" ")
}

fn scrub_token(token: &str) -> String {
    let trimmed = token.trim_matches(|c: char| matches!(c, ',' | '.' | ';' | ':' | ')' | '('));
    if trimmed.contains('@') && trimmed.contains('.') {
        return "[REDACTED_EMAIL]".into();
    }
    if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
        return "[REDACTED_URL]".into();
    }
    if trimmed.starts_with("sk-") && trimmed.len() >= 12 {
        return "[REDACTED_KEY]".into();
    }
    let digit_count = trimmed.chars().filter(|c| c.is_ascii_digit()).count();
    if digit_count >= 9 {
        return "[REDACTED_NUMBER]".into();
    }
    token.to_string()
}

/// Checks both user opt-in consent and the project's NDA/Private lock.
/// Always compiled in so NDA enforcement is auditable across all builds.
pub fn can_send_telemetry(user_opted_in: bool, project: &Project) -> bool {
    user_opted_in && project.allow_cloud_or_telemetry().is_ok()
}

/// Endpoint configuration for connecting the open-source client to a compatible
/// `/v1/*` TinyCut API server (hosted or self-hosted).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApiEndpointConfig {
    pub base_url: String,
    pub auth_token: Option<String>,
}

impl ApiEndpointConfig {
    pub fn new(base_url: impl Into<String>, auth_token: Option<String>) -> Self {
        Self {
            base_url: base_url.into().trim_end_matches('/').to_string(),
            auth_token,
        }
    }

    /// Reads `TINYCUT_API_BASE_URL` from the environment if set and non-empty.
    pub fn from_env(auth_token: Option<String>) -> Option<Self> {
        let raw = std::env::var(API_BASE_URL_ENV).ok()?;
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return None;
        }
        Some(Self::new(trimmed, auth_token))
    }

    pub fn endpoint(&self, path: &str) -> String {
        let suffix = path.trim_start_matches('/');
        format!("{}/{}", self.base_url, suffix)
    }
}

/// Scrubs and packages opt-in subtitle corrections into a wire-ready `TextDiffTelemetryBatch`.
/// Returns `None` if the user has not opted in, the project is NDA-locked, or `diffs` is empty.
#[cfg(feature = "telemetry")]
pub fn build_scrubbed_text_diff_batch(
    user_opted_in: bool,
    project: &Project,
    anonymous_client_id: impl Into<String>,
    app_version: impl Into<String>,
    asr_model_version: impl Into<String>,
    llm_model_version: Option<String>,
    diffs: &[EditDiff],
) -> Option<TextDiffTelemetryBatch> {
    if !can_send_telemetry(user_opted_in, project) || diffs.is_empty() {
        return None;
    }

    let corrections = diffs
        .iter()
        .map(|d| EditDiff {
            raw_asr: scrub_pii(&d.raw_asr),
            llm_draft: d.llm_draft.as_deref().map(scrub_pii),
            user_final: scrub_pii(&d.user_final),
            edit_type: d.edit_type,
            audio_clip_ref: None,
        })
        .collect();

    Some(TextDiffTelemetryBatch {
        anonymous_client_id: anonymous_client_id.into(),
        app_version: app_version.into(),
        asr_model_version: asr_model_version.into(),
        llm_model_version,
        locale: project.locale.clone(),
        content_domain: project.content_domain.clone(),
        corrections,
    })
}

/// Optional Cloud Boost HTTP client implementing `InferenceBackend` against any server
/// speaking TinyCut's open `v1` protocol (`core_domain::protocol`).
#[cfg(feature = "cloud-boost")]
#[derive(Debug, Clone)]
pub struct CloudBoostBackend {
    pub endpoint: ApiEndpointConfig,
}

#[cfg(feature = "cloud-boost")]
impl CloudBoostBackend {
    pub fn new(api_base_url: impl Into<String>, auth_token: Option<String>) -> Self {
        Self {
            endpoint: ApiEndpointConfig::new(api_base_url, auth_token),
        }
    }

    pub fn from_env(auth_token: Option<String>) -> Option<Self> {
        ApiEndpointConfig::from_env(auth_token).map(|endpoint| Self { endpoint })
    }
}

#[cfg(feature = "cloud-boost")]
#[async_trait]
impl InferenceBackend for CloudBoostBackend {
    async fn transcribe(&self, _media: &MediaRef, _glossary: &[String]) -> DomainResult<Transcript> {
        Err(DomainError::Inference(format!(
            "CloudBoostBackend::transcribe targeting {} is wired in Phase 2 (v0.2)",
            self.endpoint.endpoint("/v1/jobs/cloud-boost")
        )))
    }

    async fn fix_subtitles(
        &self,
        _raw: &Transcript,
        _glossary: &[String],
    ) -> DomainResult<Transcript> {
        Err(DomainError::Inference(format!(
            "CloudBoostBackend::fix_subtitles targeting {} is wired in Phase 2 (v0.2)",
            self.endpoint.endpoint("/v1/jobs/cloud-boost")
        )))
    }

    async fn detect_privacy(
        &self,
        _media: &MediaRef,
        _cfg: &ShieldConfig,
    ) -> DomainResult<Vec<RedactionTrack>> {
        Err(DomainError::Inference(format!(
            "CloudBoostBackend::detect_privacy targeting {} is wired in Phase 2 (v0.2)",
            self.endpoint.endpoint("/v1/jobs/cloud-boost")
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tinycut_core_domain::{EditType, InferenceMode, MediaRef};
    use uuid::Uuid;

    fn sample_project(is_nda_private: bool) -> Project {
        Project {
            id: Uuid::new_v4(),
            title: "Demo".into(),
            media: MediaRef {
                id: Uuid::new_v4(),
                path: "/tmp/demo.mp4".into(),
                duration_ms: 10_000,
                width: 1920,
                height: 1080,
                fps: 30.0,
            },
            is_nda_private,
            inference_mode: InferenceMode::Local,
            locale: "vi-VN".into(),
            content_domain: Some("tech".into()),
        }
    }

    #[test]
    fn scrubs_emails_phones_urls_and_keys() {
        let raw = "Liên hệ test@example.com hoặc 0912345678 xem https://secret.vn key sk-1234567890abcdef";
        let scrubbed = scrub_pii(raw);
        assert_eq!(
            scrubbed,
            "Liên hệ [REDACTED_EMAIL] hoặc [REDACTED_NUMBER] xem [REDACTED_URL] key [REDACTED_KEY]"
        );
    }

    #[test]
    fn endpoint_normalizes_trailing_slash() {
        let cfg = ApiEndpointConfig::new("http://127.0.0.1:8080/", None);
        assert_eq!(
            cfg.endpoint("/v1/models/manifest"),
            "http://127.0.0.1:8080/v1/models/manifest"
        );
    }

    #[cfg(feature = "telemetry")]
    #[test]
    fn nda_lock_blocks_telemetry_batch_creation() {
        let diff = EditDiff {
            raw_asr: "cơ sơ test@example.com".into(),
            llm_draft: None,
            user_final: "Cursor test@example.com".into(),
            edit_type: EditType::WordFix,
            audio_clip_ref: Some("should-not-leak".into()),
        };

        let nda_project = sample_project(true);
        assert!(
            build_scrubbed_text_diff_batch(
                true,
                &nda_project,
                "anon-1",
                "0.1.0",
                "whisper-base",
                None,
                std::slice::from_ref(&diff),
            )
            .is_none()
        );

        let open_project = sample_project(false);
        let batch = build_scrubbed_text_diff_batch(
            true,
            &open_project,
            "anon-1",
            "0.1.0",
            "whisper-base",
            None,
            &[diff],
        )
        .expect("opt-in non-NDA batch should build");
        assert_eq!(batch.corrections[0].raw_asr, "cơ sơ [REDACTED_EMAIL]");
        assert_eq!(batch.corrections[0].user_final, "Cursor [REDACTED_EMAIL]");
        assert!(batch.corrections[0].audio_clip_ref.is_none());
    }
}
