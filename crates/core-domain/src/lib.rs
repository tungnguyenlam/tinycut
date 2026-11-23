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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelType {
    /// Speech-to-text ASR (Whisper GGML).
    Asr,
    /// Neural voice activity detection (Silero-VAD ONNX).
    Vad,
    /// Local bilingual subtitle fixer LLM (Qwen2.5 GGUF).
    TextFixer,
    /// Tokenizer JSON paired with the local GGUF subtitle fixer LLM.
    TextFixerTokenizer,
    /// Face bounding-box detector (OpenCV Zoo YuNet ONNX).
    FaceDetector,
    /// Face identity embedder for Creator Whitelist Re-ID (OpenCV Zoo SFace ONNX).
    FaceEmbedder,
    /// License plate detector (Apache-2.0 RT-DETRv2 ONNX).
    PlateDetector,
    /// Screen PII text region detector (PP-OCRv4 mobile det ONNX).
    ScreenOcrDet,
    /// Screen PII text recognizer (PP-OCRv3 mobile rec ONNX).
    ScreenOcrRec,
    /// Legacy aggregate vision shield bundle tag (kept for manifest compatibility).
    VisionShield,
}

impl ModelType {
    /// All individual downloadable default lightweight model tasks in execution order.
    pub const ALL_DOWNLOADABLE: [ModelType; 9] = [
        Self::Asr,
        Self::Vad,
        Self::TextFixer,
        Self::TextFixerTokenizer,
        Self::FaceDetector,
        Self::FaceEmbedder,
        Self::PlateDetector,
        Self::ScreenOcrDet,
        Self::ScreenOcrRec,
    ];
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeviceTier {
    Mobile,
    Desktop,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DefaultModelSpec {
    pub task: ModelType,
    pub id: String,
    pub display_name: String,
    pub task_label: String,
    pub filename: String,
    pub runtime: String,
    pub license: String,
    pub download_url: String,
    pub sha256: String,
    pub size_bytes: u64,
    pub tier: DeviceTier,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelInstallStatus {
    pub spec: DefaultModelSpec,
    pub installed: bool,
    pub local_path: Option<String>,
    pub bytes_on_disk: u64,
}

/// Returns the verified default lightweight, commercial-safe (MIT / Apache-2.0) model
/// plugged into each TinyCut local AI task.
pub fn default_lightweight_models() -> Vec<DefaultModelSpec> {
    vec![
        DefaultModelSpec {
            task: ModelType::Asr,
            id: "whisper-base-q5_1".into(),
            display_name: "Whisper Base (Q5_1)".into(),
            task_label: "Speech-to-Text (ASR)".into(),
            filename: "ggml-base-q5_1.bin".into(),
            runtime: "whisper-rs (GGML)".into(),
            license: "MIT".into(),
            download_url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-base-q5_1.bin".into(),
            sha256: String::new(),
            size_bytes: 59_707_625,
            tier: DeviceTier::Mobile,
        },
        DefaultModelSpec {
            task: ModelType::Vad,
            id: "silero-vad-v5".into(),
            display_name: "Silero VAD v5".into(),
            task_label: "Voice Activity Detection".into(),
            filename: "silero_vad.onnx".into(),
            runtime: "ort (ONNX)".into(),
            license: "MIT".into(),
            download_url: "https://github.com/snakers4/silero-vad/raw/master/src/silero_vad/data/silero_vad.onnx".into(),
            sha256: String::new(),
            size_bytes: 2_204_103,
            tier: DeviceTier::Mobile,
        },
        DefaultModelSpec {
            task: ModelType::TextFixer,
            id: "qwen2.5-0.5b-instruct-q4_k_m".into(),
            display_name: "Qwen2.5 0.5B Instruct (Q4_K_M)".into(),
            task_label: "Subtitle Fixer LLM".into(),
            filename: "Qwen2.5-0.5B-Instruct-Q4_K_M.gguf".into(),
            runtime: "candle (GGUF)".into(),
            license: "Apache-2.0".into(),
            download_url: "https://huggingface.co/bartowski/Qwen2.5-0.5B-Instruct-GGUF/resolve/main/Qwen2.5-0.5B-Instruct-Q4_K_M.gguf".into(),
            sha256: String::new(),
            size_bytes: 397_808_192,
            tier: DeviceTier::Mobile,
        },
        DefaultModelSpec {
            task: ModelType::TextFixerTokenizer,
            id: "qwen2.5-0.5b-tokenizer".into(),
            display_name: "Qwen2.5 0.5B Tokenizer".into(),
            task_label: "Subtitle Fixer Tokenizer".into(),
            filename: "qwen2.5-0.5b-tokenizer.json".into(),
            runtime: "tokenizers (JSON)".into(),
            license: "Apache-2.0".into(),
            download_url: "https://huggingface.co/Qwen/Qwen2.5-0.5B-Instruct/resolve/main/tokenizer.json".into(),
            sha256: String::new(),
            size_bytes: 7_031_645,
            tier: DeviceTier::Mobile,
        },
        DefaultModelSpec {
            task: ModelType::FaceDetector,
            id: "yunet-2023mar".into(),
            display_name: "YuNet 2023mar".into(),
            task_label: "Face Detection".into(),
            filename: "face_detection_yunet_2023mar.onnx".into(),
            runtime: "ort (ONNX)".into(),
            license: "MIT".into(),
            download_url: "https://media.githubusercontent.com/media/opencv/opencv_zoo/main/models/face_detection_yunet/face_detection_yunet_2023mar.onnx".into(),
            sha256: String::new(),
            size_bytes: 232_589,
            tier: DeviceTier::Mobile,
        },
        DefaultModelSpec {
            task: ModelType::FaceEmbedder,
            id: "sface-2021dec".into(),
            display_name: "SFace 2021dec".into(),
            task_label: "Creator Face Re-ID".into(),
            filename: "face_recognition_sface_2021dec.onnx".into(),
            runtime: "ort (ONNX)".into(),
            license: "Apache-2.0".into(),
            download_url: "https://media.githubusercontent.com/media/opencv/opencv_zoo/main/models/face_recognition_sface/face_recognition_sface_2021dec.onnx".into(),
            sha256: String::new(),
            size_bytes: 38_696_353,
            tier: DeviceTier::Mobile,
        },
        DefaultModelSpec {
            task: ModelType::PlateDetector,
            id: "rtdetrv2-r18-plate".into(),
            display_name: "RT-DETRv2 R18 Plate".into(),
            task_label: "License Plate Detection".into(),
            filename: "plate_rtdetr.onnx".into(),
            runtime: "ort (ONNX)".into(),
            license: "Apache-2.0".into(),
            download_url: "https://huggingface.co/Topurrra/rtdetr-license-plate-detection-onnx/resolve/main/plate_rtdetr.onnx".into(),
            sha256: String::new(),
            size_bytes: 80_671_879,
            tier: DeviceTier::Desktop,
        },
        DefaultModelSpec {
            task: ModelType::ScreenOcrDet,
            id: "ppocrv4-mobile-det".into(),
            display_name: "PP-OCRv4 Mobile Det".into(),
            task_label: "Screen PII Text Detect".into(),
            filename: "ch_PP-OCRv4_det_infer.onnx".into(),
            runtime: "ort (ONNX)".into(),
            license: "Apache-2.0".into(),
            download_url: "https://huggingface.co/SWHL/RapidOCR/resolve/main/PP-OCRv4/ch_PP-OCRv4_det_infer.onnx".into(),
            sha256: String::new(),
            size_bytes: 4_745_517,
            tier: DeviceTier::Mobile,
        },
        DefaultModelSpec {
            task: ModelType::ScreenOcrRec,
            id: "ppocrv3-mobile-en-rec".into(),
            display_name: "PP-OCRv3 Mobile Rec".into(),
            task_label: "Screen PII Text Read".into(),
            filename: "en_PP-OCRv3_rec_infer.onnx".into(),
            runtime: "ort (ONNX)".into(),
            license: "Apache-2.0".into(),
            download_url: "https://huggingface.co/SWHL/RapidOCR/resolve/main/PP-OCRv3/en_PP-OCRv3_rec_infer.onnx".into(),
            sha256: String::new(),
            size_bytes: 8_967_018,
            tier: DeviceTier::Mobile,
        },
    ]
}

/// Look up the default lightweight model spec for a specific task.
pub fn default_model_spec(task: ModelType) -> Option<DefaultModelSpec> {
    default_lightweight_models()
        .into_iter()
        .find(|m| m.task == task)
}

/// Builds a `ModelManifest` populated with all default lightweight models.
pub fn default_model_manifest(generated_at_unix: u64) -> ModelManifest {
    let models = default_lightweight_models()
        .into_iter()
        .map(|spec| ModelArtifact {
            model_type: spec.task,
            tier: spec.tier,
            version: spec.id,
            download_url: spec.download_url,
            sha256: spec.sha256,
            size_bytes: spec.size_bytes,
        })
        .collect();
    ModelManifest {
        generated_at_unix,
        models,
    }
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
