# TinyCut — Product & Technical Baseline

> **Tagline:** Fast, local-first AI tricks for creators—clean bilingual subtitles, smart cuts, and instant visual/audio privacy protection, all offline by default with optional Cloud Boost.

---

## 1. Product Vision & Positioning

**TinyCut** is a Rust-first, cross-platform creator utility targeting **macOS, Windows, Linux, iOS, iPadOS, and Android**. It solves painful, repetitive pre-edit and post-edit bottlenecks for video creators (YouTube, TikTok, Reels, Podcasts, street vlogs, and tech screen recordings), with a strong initial moat in **bilingual speech (Vietnamese + English code-switching)** and **Southeast Asian / global street privacy redaction**.

### Core Differentiators
1. **Local-First by Default ($0 Free-User GPU Cost):** Every core feature runs 100% on-device with zero internet required and zero server burn on free users.
2. **Instant Personalization (Day-1 Value):** User subtitle corrections immediately train a local **Personal Glossary** (SQLite) injected into future runs—users feel the app get smarter on their very next video without waiting for cloud retraining.
3. **Visual Privacy Shield + Creator Whitelist:** Automatically blurs 1-line car plates, 2-line motorbike plates, bystander faces (while keeping the creator's own face unblurred), and on-screen PII leaks.
4. **Hybrid Paid Cloud Boost (Smart Proxy Offloading):** Paid users offload heavy inference to server GPUs using tiny compressed proxies (never uploading raw 4K video), receiving lightweight JSON tracks back to render locally at native 4K 60fps.
5. **Privacy-Safe Opt-In Data Flywheel:** Strictly opt-in, anonymized text diffs and vision false-positive ("hard negative") crops continuously improve TinyCut's PyTorch models without ever collecting private faces, plates, or NDA content.

---

## 2. Core Feature Pillars

### Pillar 1: Smart Subtitles & Auto-Cut (Audio / Text)
- **Local Bilingual Transcription & Cleanup:** Local ASR (`Whisper`) followed by a compact local post-processing LLM that restores punctuation, capitalization, English loanwords/brand names, and clean subtitle line-breaks.
- **Interactive Subtitle & Diff Editor:** Fast keyboard-driven timeline/text editor to fix typos, split/merge lines, and strip filler words (`ờ, à, kiểu như`).
- **Instant Personal Glossary:** Corrected terms (e.g., `"cơ sơ"` → `"Cursor"`) are stored locally in SQLite and injected into ASR `initial_prompt`/hotwords and the LLM prompt on subsequent projects.
- **Smart Auto-Cut:** Detects silences and repeated bad takes via local VAD (`Silero-VAD`) + transcript similarity.
- **1-Click Creator Exports:** `SRT`, `ASS`, Final Cut Pro (`.fcpxml`), Premiere Pro / DaVinci Resolve (`.xml` / `.edl`), CapCut-compatible exports, plus auto-generated YouTube chapters, titles, and descriptions.

### Pillar 2: Visual Privacy Shield (Video)
- **License Plate Redaction:** Detects both 1-line car plates (white/blue/yellow) and 2-line motorbike plates, including motion blur, night glare, and oblique street-vlog angles.
- **Face Blur + Creator Whitelist:** Blurs all bystander faces automatically while letting the user click **"Keep Unblurred"** on their own (or co-host's) face via local face Re-ID embeddings (`MobileFaceNet`).
- **On-Screen PII & QR Redaction (Screen-Recording Mode):** Detects emails, phone numbers, home addresses, bank/ID numbers (e.g., CCCD), API keys (`sk-...`), chat pop-ups, and QR codes.
- **Smooth Tracking & Zero-Leak Padding:** Pure-Rust `ByteTrack` / Kalman Filter tracking across frames with **+10% spatial margin** and **±3 frames temporal padding** so no sensitive frame flashes unblurred.
- **Flexible Export:** Fast GPU burn-in (Gaussian blur, pixelate, solid mask via `wgpu`) or **non-destructive transparent alpha-mask overlay / keyframed XML** for Premiere, Final Cut, and DaVinci Resolve.

### Pillar 3: Spoken PII Auto-Beep (Audio + Text Synergy)
- Leverages word-level timestamps from the transcript to automatically flag spoken phone numbers, addresses, or sensitive names, allowing 1-click beep/mute redaction at exact timestamps.

---

## 3. Hybrid Execution & Monetization Model

### Free (Local Default) vs. Paid (Cloud Boost)

| Capability | Free (Local Default — On-Device) | Paid (**Cloud Boost** — Server GPU) |
| :--- | :--- | :--- |
| **Speech (ASR)** | `Whisper-small` (Mobile) / `v3-turbo Q4` (Desktop) | Full FP16 fine-tuned `Whisper-large-v3-turbo` + Speaker Diarization |
| **Subtitle Fixer LLM** | `Qwen 1.5B–3B (Q4_K_M)` | Fine-tuned `Qwen 14B/32B` (slang, tech jargon, auto-chapters, hooks) |
| **Privacy Shield (Vision)** | `D-FINE-n` / `RT-DETR-tiny` + `YuNet` (`INT8`) | High-Res `D-FINE-Large` + Full `PP-OCRv4 Server` + Context NER |
| **Batch & Background** | 1 video at a time on-device | Queue multiple videos, background cloud processing + push notify |
| **Offline Behavior** | 100% offline | Seamless automatic fallback to `LocalBackend` when offline |

### Smart Proxy Offloading Strategy
Raw 4K video **never leaves the user's device**:
- **Audio/Subtitle Jobs:** Device extracts audio-only 48kbps Opus (~3.5MB per 10-min video) → Cloud GPU returns ~50KB JSON transcript.
- **Vision Privacy Jobs:** Device hardware-encodes a 720p @ 10–15fps proxy (~15–25MB) → Cloud GPU returns normalized bounding-box tracks JSON `(frame, track_id, x, y, w, h, label)` → Device interpolates to 60fps and renders native 4K 60fps locally via `wgpu`.

### Target Unit Economics & Pricing
- **Server Cost (NVIDIA L4 / RTX 4090 @ ~$0.50–$0.80/hr):**
  - 10-min Audio ASR + 14B LLM Fixer: ~5–7s GPU time (~$0.0015).
  - 10-min Vision Privacy Shield (720p@10fps): ~15–20s GPU time (~$0.004).
  - **Total per 10-min video:** < $0.01 (< 250 VND).
- **Pricing Tiers:**
  - **Free:** Unlimited local processing on all 6 platforms.
  - **TinyCut Pro (~$7/mo or 99k–149k VND/mo):** 300–500 Cloud Boost minutes/month + batch queue + flagship models (~90%+ gross margin).
  - **Top-Up Packs:** Non-expiring minute packs (e.g., 50k VND for 200 minutes) via Apple IAP, Google Play Billing, Stripe, and local VietQR / PayOS.

---

## 4. Client Architecture (Rust-Focused, 6 Platforms)

90%+ of application logic lives in pure Rust crates inside a single Cargo workspace, paired with a thin cross-platform UI shell.

### Cargo Workspace Layout
```text
tinycut/
├── crates/
│   ├── core-domain/       # Shared structs (Transcript, EditDiff, RedactionTrack, Project, Glossary, Manifest)
│   ├── media-engine/      # Video demux/mux, hardware decode/encode, audio extraction, proxy builder, 3-6s slicer
│   ├── ai-audio/          # whisper-rs (ASR) + llama-cpp-2 / mistral.rs (Text Fixer) + Silero-VAD (ort)
│   ├── vision-shield/     # ONNX Runtime (ort) detectors + PP-OCRv4 + rxing (QR) + ByteTrack/Kalman tracker
│   ├── gpu-compositor/    # wgpu shaders (Gaussian blur, pixelate, solid mask)
│   ├── export-engine/     # SRT, ASS, FCPXML, Premiere XML, DaVinci EDL, Alpha Mask video, Markdown
│   ├── local-store/       # SQLite (sqlx/rusqlite) for projects, glossary, face whitelists, offline sync queue
│   └── sync-client/       # Opt-in anonymized diff batching, local PII scrubber, R2 presigned upload, OTA models
├── apps/
│   ├── ui-dioxus/         # 100% Rust Dioxus frontend compiled to WASM (imports core-domain directly)
│   └── ui-shell/          # Tauri v2 native host (macOS, Win, Linux, iOS, iPadOS, Android + Swift/Kotlin plugins)
├── server/                # Rust Axum Control Plane (reuses core-domain!)
└── ml/                    # PyTorch training, synthetic data generators, ONNX/GGUF export & eval scripts
```

### Unified Inference Abstraction (`core-domain`)
```rust
#[async_trait::async_trait]
pub trait InferenceBackend: Send + Sync {
    async fn transcribe(&self, media: &MediaRef, glossary: &[String]) -> Result<Transcript>;
    async fn fix_subtitles(&self, raw: &Transcript, glossary: &[String]) -> Result<Transcript>;
    async fn detect_privacy(&self, media: &MediaRef, cfg: &ShieldConfig) -> Result<Vec<RedactionTrack>>;
}
```
- `LocalBackend`: Runs `whisper-rs`, `llama-cpp-2`/`mistral.rs`, and `ort` on local hardware.
- `CloudBoostBackend`: Builds smart proxy, uploads to ephemeral R2 via presigned URL, streams SSE progress, returns identical domain structs.

### Platform Hardware & Memory Matrix
- **Hardware Acceleration:**
  - macOS / iOS / iPadOS → **Metal** & **CoreML** (audio/video via AVFoundation to avoid GPL App Store issues)
  - Windows → **CUDA** / **DirectML** / **Vulkan** (DX12 for `wgpu`)
  - Linux → **CUDA** / **Vulkan**
  - Android → **Vulkan** / **NNAPI** / CPU NEON (video via MediaExtractor/MediaCodec)
- **Tiered Memory Profiles (Critical for iOS/Android):**
  - *Desktop:* `Whisper-large-v3-turbo Q5_K` (~600MB) + `Qwen 3B Q4_K_M` (~2GB) + ONNX Vision models.
  - *Mobile (strict <2.5GB RAM budget):* `Whisper-small` (~250MB) + `Qwen 1.5B Q4_K_M` (~800MB). **Sequential model swapping:** load one model at a time and drop/free it before loading the next stage.

---

## 5. Server-Side Architecture

Split into a lightweight **Rust Control Plane** and **Scale-to-Zero Python/PyTorch GPU Workers**.

### Tech Stack
- **Control Plane:** Rust (`Axum`, `Tokio`, `tower`, `sqlx`) on a low-cost VPS / Fly.io / Railway.
- **Database:** **PostgreSQL** (users, credit ledger, text diffs, vision negatives metadata, model releases).
- **Queue / PubSub:** **Redis** (job queue + SSE progress streaming).
- **Object Storage & CDN:** **Cloudflare R2** ($0 egress fees):
  - `tinycut-models` (public CDN for GGUF/ONNX weights)
  - `tinycut-cloud-temp` (ephemeral Cloud Boost proxies; deleted immediately on completion + 15-min lifecycle failsafe)
  - `tinycut-flywheel` (strictly opt-in anonymized false-positive crops & Phase-2 audio snippets)
- **GPU Workers:** Python + PyTorch / CUDA (`faster-whisper`, `vLLM` / `TensorRT-LLM`, `onnxruntime-gpu`) deployed on **Modal** or **RunPod Serverless** (scale-to-zero) initially, migrating to dedicated RTX 4090 / L40S nodes at >500 paid users.

### Core API Endpoints (`Axum`)
- `POST /v1/jobs/cloud-boost` — Auth + atomic credit check → returns 5-min presigned R2 upload URL + `job_id`.
- `GET /v1/jobs/{job_id}/stream` — Server-Sent Events (SSE) progress stream + final JSON result.
- `GET /v1/models/manifest` — Latest versioned `asr`, `text_fixer`, and `vision_shield` R2 URLs + SHA-256 checksums.
- `POST /v1/telemetry/text-diffs` — Opt-in scrubbed `(raw_asr, llm_draft, user_final)` subtitle corrections.
- `POST /v1/telemetry/vision-negatives` — Opt-in user-confirmed **false-positive** crops (max 256×256px) via presigned R2 URL, gated by server-side face/OCR check.
- `DELETE /v1/telemetry/me` — One-click full erasure of all telemetry rows & R2 objects tied to `anonymous_client_id` (GDPR / App Store compliance).

### Core PostgreSQL Tables
- `users`, `subscriptions`, `credit_ledger` (atomic minute deductions & automatic failure refunds).
- `text_corrections` (`id`, `anon_id`, `asr_version`, `llm_version`, `domain`, `raw_asr`, `llm_draft`, `user_final`, `edit_distance_ratio`, `quality_status`, `created_at`).
- `vision_hard_negatives` (`id`, `anon_id`, `model_version`, `r2_object_key`, `rejected_label`, `safety_check_passed`, `created_at`).
- `audio_samples` (Phase 2: `id`, `correction_id`, `r2_object_key`, `duration_ms`, `snr_score`, `verified_verbatim`, `created_at`).
- `model_releases` (`id`, `model_type`, `tier`, `version`, `r2_download_url`, `sha256_hash`, `eval_score`, `is_active`, `created_at`).

---

## 6. Privacy & Data Flywheel Policy (Hard Rules)

1. **Default = 100% Offline & Private.** Zero telemetry without explicit opt-in.
2. **Per-Project "NDA / Private" Lock:** One toggle guarantees zero telemetry and disables cloud calls for embargoed/client projects.
3. **Hard Firewall Between Cloud Boost & Training:** Paid Cloud Boost proxies are **never** stored or used for model training; they are purged immediately after inference.
4. **Strict Feature Telemetry Matrix:**

| Feature | Allowed (Opt-In Only) | NEVER Collected |
| :--- | :--- | :--- |
| **Subtitles / Text** | Anonymized diffs `(raw_asr -> user_final)` after local regex PII scrubbing (emails, phones, URLs, bank/ID numbers). | Full transcripts, un-scrubbed PII, NDA-locked projects. |
| **Visual Privacy Shield** | **False Positives ONLY ("Hard Negatives"):** User-confirmed non-PII crops (e.g., street signs mistaken for plates, ≤256×256px) + anonymous aggregate counts. Server auto-discards if face/OCR detects any PII. | Real faces, real license plates, screen recordings, user-drawn "missed PII" boxes. |
| **Speech Audio (Phase 2)** | Clipped 3–5s Opus audio window around a corrected word (requires separate incentivized Level-2 opt-in). | Full video or full audio tracks. |

---

## 7. PyTorch ML Baselines & Licensing

> **License Rule:** Avoid AGPL weights/code (e.g., Ultralytics GPL/AGPL) in commercial client/server binaries. Stick to **Apache-2.0 / MIT** baselines.

| Pipeline | Local Baseline | Cloud Baseline | Training Strategy | Edge Export |
| :--- | :--- | :--- | :--- | :--- |
| **A. Subtitle Fixer LLM** *(Start First)* | `Qwen2.5 / Qwen3 (1.5B–3B)` | `Qwen2.5 / Qwen3 (14B–32B)` | LoRA (`peft`/`unsloth`, `bf16`); **70% clean general VN/EN + 30% user diffs** (replay mixing) | Merge LoRA → `GGUF (Q4_K_M)` |
| **B. Visual Privacy Shield** | `D-FINE-n` / `RT-DETR-tiny` / `YOLOX-nano` (plates), `YuNet`/`SCRFD` + `MobileFaceNet` (faces), `PP-OCRv4 mobile` + Regex/NER (screen PII) | `D-FINE-Large`, `PP-OCRv4 Server`, Context NER | **Synthetic VN/global plate & screen PII generator** + opt-in Hard Negative mining | `torch.onnx.export` → `FP16` (GPU) / `INT8` (mobile CPU) |
| **C. Speech ASR** *(Phase 2)* | `Whisper-small` (Mobile) / `v3-turbo Q5_K` (Desktop) | FP16 `Whisper-large-v3-turbo` + Diarization | LoRA on attn layers; 70/30 replay mix; **strict verbatim filter** (drop edits with >40% rewrite CER) | Merge LoRA → `whisper.cpp` GGML/GGUF |

### Evaluation Gate (Golden Test Set)
No model promotes to `model_releases` unless it beats the current production baseline on a fixed, hand-verified Golden Test Set:
- **Vision Privacy:** Plate & Face **Recall ≥ 99%** (missing PII is far worse than an extra blur box) while reducing false positives.
- **LLM Fixer:** Brand/tech term exact match, punctuation F1, and near-zero hallucination rate.
- **ASR:** Lower WER and higher code-switched English term accuracy across Northern/Central/Southern Vietnamese accents.

---

## 8. Phased Build Roadmap

- **v0.1 — Desktop MVP (macOS & Windows):** Cargo workspace (`core-domain`, `media-engine`, `ai-audio`, `vision-shield`, `gpu-compositor`, `export-engine`, `local-store`) + UI shell. Ship local bilingual subtitles + Personal Glossary + 1-click Privacy Shield (Plates + Faces + Creator Whitelist).
- **v0.2 — Control Plane, Cloud Boost & First Flywheel:** Launch Rust `Axum` + Postgres + Redis + Cloudflare R2 + Modal/RunPod Serverless GPU worker. Enable Cloud Boost Smart Proxy Offloading, opt-in Text-Diff & Vision Hard-Negative telemetry, and OTA model manifest updates.
- **v0.3 — Mobile Expansion (iOS, iPadOS, Android):** Responsive mobile UI, native AVFoundation / MediaExtractor bindings, strict single-model-in-RAM swapping, and Apple IAP / Google Play Billing.
- **v0.4 — Pro Integrations & Audio Flywheel:** Screen-Recording PII detector, Spoken PII Auto-Beep, non-destructive alpha-mask export for FCP/Premiere/DaVinci, and incentivized Level-2 clipped audio fine-tuning.

---

## 9. Locked Decisions & Remaining Open Choices

1. **UI Framework (Locked):** **Tauri v2 + Dioxus (WASM)** — `apps/ui-dioxus` renders in Tauri's WebView (giving native Vietnamese Telex/VNI IME, `<video>`, and `<canvas>` while sharing `crates/core-domain` Rust structs directly), and `apps/ui-shell/src-tauri` handles 6-OS packaging, auto-updates, and Swift/Kotlin mobile plugins. `core-domain` stays WASM-clean (`default-features = false` on WASM).
2. **Initial Video Demux/Mux Strategy (Open):** LGPL FFmpeg on desktop + native AVFoundation (Apple) / MediaExtractor (Android) on mobile.
3. **First Serverless GPU Provider (Open):** **Modal** vs. **RunPod Serverless** for v0.2 Cloud Boost workers.
