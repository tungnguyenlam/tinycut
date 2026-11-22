use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum DomainError {
    #[error("project is locked in NDA/private mode; cloud and telemetry operations are blocked")]
    NdaPrivacyBlocked,
    #[error("inference failed: {0}")]
    Inference(String),
    #[error("media error: {0}")]
    Media(String),
    #[error("storage error: {0}")]
    Storage(String),
    #[error("export error: {0}")]
    Export(String),
}

pub type DomainResult<T> = Result<T, DomainError>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InferenceMode {
    Local,
    CloudBoost,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MediaRef {
    pub id: Uuid,
    pub path: String,
    pub duration_ms: u64,
    pub width: u32,
    pub height: u32,
    pub fps: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    pub id: Uuid,
    pub title: String,
    pub media: MediaRef,
    pub is_nda_private: bool,
    pub inference_mode: InferenceMode,
    pub locale: String,
    pub content_domain: Option<String>,
}

impl Project {
    pub fn allow_cloud_or_telemetry(&self) -> DomainResult<()> {
        if self.is_nda_private {
            Err(DomainError::NdaPrivacyBlocked)
        } else {
            Ok(())
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WordTiming {
    pub word: String,
    pub start_ms: u64,
    pub end_ms: u64,
    pub confidence: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TranscriptSegment {
    pub id: Uuid,
    pub start_ms: u64,
    pub end_ms: u64,
    pub raw_asr: String,
    pub llm_draft: Option<String>,
    pub text: String,
    pub speaker: Option<String>,
    pub words: Vec<WordTiming>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Transcript {
    pub locale: String,
    pub asr_model_version: String,
    pub llm_model_version: Option<String>,
    pub segments: Vec<TranscriptSegment>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GlossaryEntry {
    pub spoken_variant: String,
    pub preferred_term: String,
    pub hit_count: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EditType {
    WordFix,
    Punctuation,
    LineSplit,
    Rewrite,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EditDiff {
    pub raw_asr: String,
    pub llm_draft: Option<String>,
    pub user_final: String,
    pub edit_type: EditType,
    pub audio_clip_ref: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextDiffTelemetryBatch {
    pub anonymous_client_id: String,
    pub app_version: String,
    pub asr_model_version: String,
    pub llm_model_version: Option<String>,
    pub locale: String,
    pub content_domain: Option<String>,
    pub corrections: Vec<EditDiff>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RedactionKind {
    LicensePlate,
    Face,
    ScreenPii,
    QrCode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BlurStyle {
    Gaussian,
    Pixelate,
    SolidMask,
}

/// Normalized bounding box in `[0.0, 1.0]` image coordinates.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct NormalizedBox {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl NormalizedBox {
    /// Expands the box around its center by `margin_ratio` (e.g. `0.10` for +10% zero-leak padding)
    /// and clamps to `[0.0, 1.0]`.
    pub fn expand(&self, margin_ratio: f32) -> Self {
        let dw = self.w * margin_ratio;
        let dh = self.h * margin_ratio;
        let x1 = (self.x - dw * 0.5).clamp(0.0, 1.0);
        let y1 = (self.y - dh * 0.5).clamp(0.0, 1.0);
        let x2 = (self.x + self.w + dw * 0.5).clamp(0.0, 1.0);
        let y2 = (self.y + self.h + dh * 0.5).clamp(0.0, 1.0);
        Self {
            x: x1,
            y: y1,
            w: (x2 - x1).max(0.0),
            h: (y2 - y1).max(0.0),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct RedactionKeyframe {
    pub frame_index: u64,
    pub bbox: NormalizedBox,
    pub confidence: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RedactionTrack {
    pub track_id: u64,
    pub kind: RedactionKind,
    pub enabled: bool,
    pub whitelisted: bool,
    pub keyframes: Vec<RedactionKeyframe>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShieldConfig {
    pub detect_plates: bool,
    pub detect_faces: bool,
    pub detect_screen_pii: bool,
    pub detect_qr: bool,
    pub blur_style: BlurStyle,
    pub spatial_padding_ratio: f32,
    pub temporal_padding_frames: u32,
}

impl Default for ShieldConfig {
    fn default() -> Self {
        Self {
            detect_plates: true,
            detect_faces: true,
            detect_screen_pii: false,
            detect_qr: true,
            blur_style: BlurStyle::Gaussian,
            spatial_padding_ratio: 0.10,
            temporal_padding_frames: 3,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelType {
    Asr,
    TextFixer,
    VisionShield,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeviceTier {
    Mobile,
    Desktop,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelArtifact {
    pub model_type: ModelType,
    pub tier: DeviceTier,
    pub version: String,
    pub download_url: String,
    pub sha256: String,
    pub size_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelManifest {
    pub generated_at_unix: u64,
    pub models: Vec<ModelArtifact>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobProgressEvent {
    pub job_id: Uuid,
    pub stage: String,
    pub percent: u8,
    pub message: String,
}

#[cfg(feature = "native")]
#[async_trait::async_trait]
pub trait InferenceBackend: Send + Sync {
    async fn transcribe(&self, media: &MediaRef, glossary: &[String]) -> DomainResult<Transcript>;
    async fn fix_subtitles(
        &self,
        raw: &Transcript,
        glossary: &[String],
    ) -> DomainResult<Transcript>;
    async fn detect_privacy(
        &self,
        media: &MediaRef,
        cfg: &ShieldConfig,
    ) -> DomainResult<Vec<RedactionTrack>>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalized_box_expands_and_clamps() {
        let b = NormalizedBox {
            x: 0.02,
            y: 0.50,
            w: 0.20,
            h: 0.20,
        };
        let expanded = b.expand(0.10);
        assert!(expanded.x >= 0.0);
        assert!(expanded.w > b.w);
        assert!(expanded.h > b.h);
    }

    #[test]
    fn nda_project_blocks_cloud_and_telemetry() {
        let project = Project {
            id: Uuid::new_v4(),
            title: "Embargoed Review".into(),
            media: MediaRef {
                id: Uuid::new_v4(),
                path: "/tmp/video.mp4".into(),
                duration_ms: 10_000,
                width: 3840,
                height: 2160,
                fps: 60.0,
            },
            is_nda_private: true,
            inference_mode: InferenceMode::Local,
            locale: "vi-VN".into(),
            content_domain: Some("tech".into()),
        };
        assert!(matches!(
            project.allow_cloud_or_telemetry(),
            Err(DomainError::NdaPrivacyBlocked)
        ));
    }
}
