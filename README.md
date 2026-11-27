# TinyCut

Fast, local-first AI tricks for creators — bilingual subtitles, smart cuts, and visual/audio privacy protection across **macOS, Windows, and Linux** (with shared Rust core designed for future native **iOS / iPadOS / Android** shells via UniFFI), powered by **Rust + Tauri v2 + SolidJS / TypeScript (`tauri-specta`)** with an optional **Cloud Boost** protocol.

- **[BASELINE.md](./BASELINE.md)** — Open-source client product & technical specification and public `/v1` API contract.
- **[WORKFLOW.md](./WORKFLOW.md)** — Local creator runtime, client-side Smart Proxy & privacy gate, and engineering workflows.

> **Open-Core Architecture:** This repository contains the 100% open-source, local-first TinyCut client application, on-device AI pipelines, and versioned `v1` client-server wire contract (`crates/core-domain/src/protocol.rs`). Hosted Cloud Boost infrastructure, telemetry storage, GPU inference workers, and model training pipelines live in a separate private repository.

## Workspace Layout

- `crates/core-domain` — Lightweight shared domain structs (`Transcript`, `RedactionTrack`, `EditDiff`, `ModelManifest`), `InferenceBackend` trait, optional `specta` TypeScript type derives, and versioned `protocol` (`v1`) wire DTOs.
- `crates/model-hub` — Local model downloader, SHA-256 verification, and cache manager (`~/.tinycut/models` or app data dir).
- `crates/media-engine` — Streaming RGBA frame decoding (`SampledFrameStream`), cached WebView preview proxy builder (H.264/AAC), waveform peak extraction, Smart Proxy builder (Opus / 720p@12fps), 3–5s audio slicer, and streaming video writer (`FrameVideoWriter`).
- `crates/ai-audio` — Consolidated 2-runtime local audio pipeline: ASR (`whisper-rs` GGML), Silero-VAD (`ort` ONNX), extensible `SubtitleFixer` trait + deterministic Vietnamese/English glossary fixer, and Personal Glossary prompt builder.
- `crates/vision-shield` — Streaming ONNX Runtime (`ort`) detectors (`YuNet`, `SFace`, `RT-DETRv2`, `PP-OCR`), pure-Rust QR (`rqrr`), Creator Face Whitelist similarity, and Zero-Leak padding (`+10%` box, `±3` frames).
- `crates/gpu-compositor` — Cross-platform `wgpu` blur / pixelate / mask shaders with persistent buffer reuse and deterministic CPU fallback.
- `crates/export-engine` — SRT, ASS, FCPXML, Premiere XML, DaVinci EDL, and Alpha Mask exporters.
- `crates/local-store` — Local SQLite persistence for projects, Personal Glossary, face whitelists, and offline opt-in queue.
- `crates/sync-client` — Auditable on-device PII scrubber, NDA privacy lock guard, and feature-gated (`cloud-boost`, `telemetry`) HTTP client for any compatible `/v1` server (point `TINYCUT_API_BASE_URL` at any endpoint, or compile with `--no-default-features` for an air-gapped build).
- `apps/ui-web/` — SolidJS + Vite + TypeScript Studio frontend with auto-generated `tauri-specta` bindings, 60fps `<canvas>` redaction overlay, real waveform timeline, and native Vietnamese IME editing.
- `apps/ui-shell/src-tauri/` — Tauri v2 native desktop host exposing all Rust crates via `tauri-specta`.

## Quick Start

### 1. Run Core Rust Workspace Tests & Lint
```bash
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo check -p tinycut-core-domain --no-default-features
cargo check -p tinycut-core-domain --features specta
cargo check -p tinycut-sync-client --no-default-features
```

### 2. Run the Tauri v2 + SolidJS (TypeScript) App
Install `pnpm` and the Tauri v2 CLI (`cargo-tauri`), then launch dev mode:
```bash
pnpm --dir apps/ui-web install
cargo install tauri-cli
cd apps/ui-shell/src-tauri
cargo tauri dev
```
