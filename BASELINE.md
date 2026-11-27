# TinyCut — Open-Source Client Baseline & Protocol Spec

> **Tagline:** Fast, local-first AI tricks for creators—clean bilingual subtitles, smart cuts, and instant visual/audio privacy protection, all offline by default with an optional open Cloud Boost protocol.

---

## 1. Product Vision & Positioning

**TinyCut** is a Rust-first, cross-platform creator utility targeting **macOS, Windows, Linux, iOS, iPadOS, and Android**. It solves painful, repetitive pre-edit and post-edit bottlenecks for video creators (YouTube, TikTok, Reels, Podcasts, street vlogs, and tech screen recordings), with a strong focus on **bilingual speech (Vietnamese + English code-switching)** and **street & screen privacy redaction**.

### Core Principles
1. **Local-First by Default:** Every core feature runs 100% on-device with zero internet required after downloading open weights.
2. **Instant Personalization (Day-1 Value):** User subtitle corrections immediately train a local **Personal Glossary** (SQLite) injected into future runs—users feel the app get smarter on their very next video.
3. **Visual Privacy Shield + Creator Whitelist:** Automatically blurs 1-line car plates, 2-line motorbike plates, bystander faces (while keeping the creator's own face unblurred), and on-screen PII leaks.
4. **Open, Auditable Privacy Boundaries:** The NDA lock (`Project::allow_cloud_or_telemetry`) and on-device PII scrubber (`tinycut_sync_client::scrub_pii`) live in this open-source repo so any user can inspect and verify them, or compile an air-gapped build with `--no-default-features`.
5. **Decoupled Cloud Protocol:** Optional Cloud Boost and strictly opt-in telemetry communicate over a versioned `/v1` HTTP/JSON protocol defined in `crates/core-domain/src/protocol.rs`. Hosted inference, storage, and training live outside this client repository.

---

## 2. Core Feature Pillars

### Pillar 1: Smart Subtitles & Auto-Cut (Audio / Text)
- **Local Bilingual Transcription & Cleanup:** Local ASR (`Whisper`) followed by a compact local post-processing LLM (`Qwen2.5`) that restores punctuation, capitalization, English loanwords/brand names, and clean subtitle line-breaks.
- **Interactive Subtitle & Diff Editor:** Fast keyboard-driven timeline/text editor to fix typos, split/merge lines, and strip filler words (`ờ, à, kiểu như`).
- **Instant Personal Glossary:** Corrected terms (e.g., `"cơ sơ"` → `"Cursor"`) are stored locally in SQLite and injected into ASR `initial_prompt`/hotwords and the LLM prompt on subsequent projects.
- **Smart Auto-Cut:** Detects silences and repeated bad takes via local VAD (`Silero-VAD`) + transcript similarity.
- **1-Click Creator Exports:** `SRT`, `ASS`, Final Cut Pro (`.fcpxml`), Premiere Pro / DaVinci Resolve (`.xml` / `.edl`), and CapCut-compatible exports.

### Pillar 2: Visual Privacy Shield (Video)
- **License Plate Redaction:** Detects both 1-line car plates and 2-line motorbike plates, including motion blur, night glare, and oblique street-vlog angles.
- **Face Blur + Creator Whitelist:** Blurs all bystander faces automatically while letting the user click **"Keep Unblurred"** on their own (or co-host's) face via local face Re-ID embeddings (`SFace` / `MobileFaceNet`).
- **On-Screen PII & QR Redaction (Screen-Recording Mode):** Detects emails, phone numbers, home addresses, bank/ID numbers, API keys (`sk-...`), chat pop-ups, and QR codes.
- **Smooth Tracking & Zero-Leak Padding:** Pure-Rust `ByteTrack` / Kalman Filter tracking across frames with **+10% spatial margin** and **±3 frames temporal padding** so no sensitive frame flashes unblurred.
- **Flexible Export:** Fast GPU burn-in (Gaussian blur, pixelate, solid mask via `wgpu`) or non-destructive transparent alpha-mask overlay / keyframed XML for Premiere, Final Cut, and DaVinci Resolve.

### Pillar 3: Spoken PII Auto-Beep (Audio + Text Synergy)
- Leverages word-level timestamps from the transcript to automatically flag spoken phone numbers, addresses, or sensitive names, allowing 1-click beep/mute redaction at exact timestamps.

---

## 3. Client Architecture (Rust-Focused, 6 Platforms)

90%+ of application logic lives in pure Rust crates inside a single Cargo workspace, paired with a thin cross-platform UI shell.

### Cargo Workspace Layout
```text
tinycut/
├── crates/
│   ├── core-domain/       # Shared structs (optional specta derives), InferenceBackend trait, & v1 wire protocol
│   ├── model-hub/         # Default MIT/Apache-2.0 lightweight model downloader & SHA-256 verifier
│   ├── media-engine/      # Streaming frame decode/encode, preview proxy cache, waveform peaks, smart proxy builder
│   ├── ai-audio/          # whisper-rs (GGML ASR) + Silero-VAD (ort ONNX) + extensible SubtitleFixer trait
│   ├── vision-shield/     # Streaming ONNX Runtime (ort) detectors + PP-OCR + ByteTrack/Kalman tracker
│   ├── gpu-compositor/    # wgpu shaders (Gaussian blur, pixelate, solid mask) with buffer reuse + CPU fallback
│   ├── export-engine/     # SRT, ASS, FCPXML, Premiere XML, DaVinci EDL, Alpha Mask
│   ├── local-store/       # SQLite (rusqlite) for projects, glossary, face whitelists, offline queue
│   └── sync-client/       # On-device PII scrubber, NDA guard, feature-gated Cloud Boost & telemetry client
└── apps/
    ├── ui-web/            # SolidJS + Vite + TypeScript frontend (type-safe bindings via tauri-specta)
    └── ui-shell/          # Tauri v2 native desktop host (macOS, Win, Linux); mobile shells use UniFFI + SwiftUI/Compose
```

### Unified Inference Abstraction (`core-domain`)
```rust
#[async_trait::async_trait]
pub trait InferenceBackend: Send + Sync {
    async fn transcribe(&self, media: &MediaRef, glossary: &[String]) -> DomainResult<Transcript>;
    async fn fix_subtitles(&self, raw: &Transcript, glossary: &[String]) -> DomainResult<Transcript>;
    async fn detect_privacy(&self, media: &MediaRef, cfg: &ShieldConfig) -> DomainResult<Vec<RedactionTrack>>;
}
```
- `LocalBackend`: Runs consolidated 2-runtime local inference (`whisper-rs` GGML + `ort` ONNX Runtime) plus deterministic Personal Glossary post-processing.
- `CloudBoostBackend` *(feature `cloud-boost`)*: Builds a lightweight smart proxy (48kbps Opus audio or 720p@10–15fps video — never raw 4K source files), uploads to a presigned ephemeral URL, streams SSE progress, and returns identical domain structs to render locally at native 4K 60fps via `wgpu`.

### Platform Hardware & Memory Matrix
- **Hardware Acceleration:**
  - macOS / iOS / iPadOS → **Metal** & **CoreML** (audio/video via AVFoundation)
  - Windows → **CUDA** / **DirectML** / **Vulkan** (DX12 for `wgpu`)
  - Linux → **CUDA** / **Vulkan**
  - Android → **Vulkan** / **NNAPI** / CPU NEON (video via MediaExtractor/MediaCodec)
- **Tiered Memory Profiles (Critical for iOS/Android):**
  - *Desktop:* `Whisper` + `Qwen GGUF` + ONNX Vision models.
  - *Mobile (strict <2.5GB RAM budget):* **Sequential model swapping** — load one model at a time and drop/free it before loading the next stage.

---

## 4. Open Client-Server Wire Contract (`v1`)

Defined in `crates/core-domain/src/protocol.rs`. The client talks to any server configured via `TINYCUT_API_BASE_URL` (or official hosted defaults when enabled):

| Endpoint | Request / Response DTO | Purpose |
| :--- | :--- | :--- |
| `GET /health` | `200 OK` | Liveness check |
| `GET /v1/models/manifest` | `ModelManifest` | Versioned local model catalog URLs + SHA-256 checksums |
| `POST /v1/jobs/cloud-boost` | `CreateCloudBoostJobRequest` → `CreateCloudBoostJobResponse` | Request ephemeral proxy upload slot + `job_id` |
| `GET /v1/jobs/{job_id}/stream` | SSE `JobProgressEvent` + final JSON | Stream progress and final `Transcript` / `Vec<RedactionTrack>` |
| `POST /v1/telemetry/text-diffs` | `TextDiffTelemetryBatch` | Strictly opt-in, locally PII-scrubbed subtitle diffs |
| `POST /v1/telemetry/vision-negatives` | `VisionHardNegativeRequest` → `VisionHardNegativeUploadResponse` | Strictly opt-in false-positive crops (`<= 256x256px`) |
| `DELETE /v1/telemetry/me` | `204 No Content` | One-click erasure of all data tied to `anonymous_client_id` |

---

## 5. Client Privacy Guarantees (Hard Rules)

1. **Default = 100% Offline & Private.** Zero telemetry without explicit opt-in.
2. **Per-Project "NDA / Private" Lock:** Enforced in Rust (`Project::allow_cloud_or_telemetry`) — blocks both Cloud Boost and telemetry for embargoed/client projects.
3. **Compile-Time Strip Option:** `tinycut-sync-client` gates Cloud Boost (`cloud-boost`) and telemetry batching (`telemetry`) behind Cargo features so air-gapped builds can disable them completely.
4. **On-Device PII Scrubbing:** Emails, URLs, API keys (`sk-...`), and long numeric sequences (phone/bank/ID numbers) are redacted locally before any opt-in text diff is queued.
5. **Commercial-Safe Default Weights:** Every default local model in `core-domain` uses MIT or Apache-2.0 licenses.

---

## 6. Client Roadmap

- **v0.1 — Desktop MVP (macOS, Windows, Linux):** Local bilingual subtitles + Personal Glossary + 1-click Privacy Shield (Plates + Faces + Creator Whitelist) + streaming media/GPU burn-in + Tauri v2 / SolidJS TypeScript (`tauri-specta`) Studio UI.
- **v0.2 — Cloud Boost Client & OTA Model Manifests:** Wire `CloudBoostBackend` HTTP/SSE transport, Smart Proxy Offloading, and signed manifest model updates against the `v1` protocol.
- **v0.3 — Mobile Expansion (iOS, iPadOS, Android):** Thin native SwiftUI & Jetpack Compose shells over the Rust core (`UniFFI`), native AVFoundation / MediaExtractor bindings, and strict single-model-in-RAM swapping.
- **v0.4 — Pro Integrations:** Screen-Recording PII detector (`PP-OCRv4` + NER/Regex), Spoken PII Auto-Beep, and non-destructive transparent alpha-mask / keyframed XML export for FCP, Premiere, and DaVinci Resolve.
