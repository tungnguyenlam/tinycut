# TinyCut

Fast, local-first AI tricks for creators — bilingual subtitles, smart cuts, and visual/audio privacy protection across **macOS, Windows, Linux, iOS, iPadOS, and Android**, powered by **Rust + Dioxus (WASM) + Tauri v2** with an optional **Cloud Boost** protocol.

- **[BASELINE.md](./BASELINE.md)** — Open-source client product & technical specification and public `/v1` API contract.
- **[WORKFLOW.md](./WORKFLOW.md)** — Local creator runtime, client-side Smart Proxy & privacy gate, and engineering workflows.

> **Open-Core Architecture:** This repository contains the 100% open-source, local-first TinyCut client application, on-device AI pipelines, and versioned `v1` client-server wire contract (`crates/core-domain/src/protocol.rs`). Hosted Cloud Boost infrastructure, telemetry storage, GPU inference workers, and model training pipelines live in a separate private repository.

## Workspace Layout

- `crates/core-domain` — WASM-clean shared domain structs (`Transcript`, `RedactionTrack`, `EditDiff`, `ModelManifest`), `InferenceBackend` trait, and versioned `protocol` (`v1`) wire DTOs.
- `crates/model-hub` — Local model downloader, SHA-256 verification, and cache manager (`~/.tinycut/models`).
- `crates/media-engine` — Audio extraction, Smart Proxy builder (Opus / 720p@12fps), and 3–5s audio slicer.
- `crates/ai-audio` — Local ASR (`whisper-rs`), Silero-VAD (`ort`), local LLM subtitle fixer (`candle`), and Personal Glossary prompt builder.
- `crates/vision-shield` — ONNX Runtime (`ort`) detectors, Creator Face Whitelist similarity, and Zero-Leak padding (`+10%` box, `±3` frames).
- `crates/gpu-compositor` — Cross-platform `wgpu` blur / pixelate / mask shaders with deterministic CPU fallback.
- `crates/export-engine` — SRT, ASS, FCPXML, Premiere XML, DaVinci EDL, and Alpha Mask exporters.
- `crates/local-store` — Local SQLite persistence for projects, Personal Glossary, face whitelists, and offline opt-in queue.
- `crates/sync-client` — Auditable on-device PII scrubber, NDA privacy lock guard, and feature-gated (`cloud-boost`, `telemetry`) HTTP client for any compatible `/v1` server (point `TINYCUT_API_BASE_URL` at any endpoint, or compile with `--no-default-features` for an air-gapped build).
- `apps/ui-dioxus/` — 100% Rust Dioxus frontend compiled to `wasm32-unknown-unknown`.
- `apps/ui-shell/src-tauri/` — Tauri v2 native shell for all 6 platforms.

## Quick Start

### 1. Run Core Rust Workspace Tests & Lint
```bash
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo check -p tinycut-core-domain --no-default-features
cargo check -p tinycut-sync-client --no-default-features
```

### 2. Run the Tauri v2 + Dioxus (WASM) App
Install the WASM target, Dioxus CLI (`dx`), and Tauri v2 CLI (`cargo-tauri`):
```bash
rustup target add wasm32-unknown-unknown
cargo install dioxus-cli tauri-cli
cd apps/ui-shell/src-tauri
cargo tauri dev
```
