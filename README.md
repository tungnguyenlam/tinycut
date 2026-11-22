# TinyCut

Fast, local-first AI tricks for creators — bilingual subtitles, smart cuts, and visual/audio privacy protection across **macOS, Windows, Linux, iOS, iPadOS, and Android**, powered by **Rust + Dioxus (WASM) + Tauri v2** with optional **Cloud Boost**.

- **[BASELINE.md](./BASELINE.md)** — Full product & technical specification.
- **[WORKFLOW.md](./WORKFLOW.md)** — Runtime, Cloud Boost, opt-in data flywheel, PyTorch ML, and milestone workflows.

## Workspace Layout

- `crates/core-domain` — WASM-clean shared structs (`Transcript`, `RedactionTrack`, `EditDiff`, `ModelManifest`) and `InferenceBackend` trait.
- `crates/media-engine` — Audio extraction, Smart Proxy Offloading (Opus / 720p@12fps), and 3–5s audio slicer.
- `crates/ai-audio` — Local ASR (`whisper-rs`), local LLM subtitle fixer, and Personal Glossary prompt builder.
- `crates/vision-shield` — ONNX Runtime (`ort`) detectors, Creator Face Whitelist similarity, and Zero-Leak padding (`+10%` box, `±3` frames).
- `crates/gpu-compositor` — Cross-platform `wgpu` blur / pixelate / mask shaders.
- `crates/export-engine` — SRT, FCPXML, Premiere XML, DaVinci EDL, and Alpha Mask exporters.
- `crates/local-store` — Local SQLite persistence for projects, Personal Glossary, face whitelists, and offline telemetry queue.
- `crates/sync-client` — On-device PII scrubber, NDA privacy lock guard, and `CloudBoostBackend`.
- `server/` — Rust `Axum` Control Plane reusing `core-domain`.
- `apps/ui-dioxus/` — 100% Rust Dioxus frontend compiled to `wasm32-unknown-unknown`.
- `apps/ui-shell/src-tauri/` — Tauri v2 native shell for all 6 platforms.
- `ml/` — PyTorch LoRA & synthetic license-plate training pipelines.

## Quick Start

### 1. Run Core Rust Workspace Tests & Lint
```bash
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo check -p tinycut-core-domain --no-default-features
```

### 2. Run the Tauri v2 + Dioxus (WASM) App
Install the WASM target, Dioxus CLI (`dx`), and Tauri v2 CLI (`cargo-tauri`):
```bash
rustup target add wasm32-unknown-unknown
cargo install dioxus-cli tauri-cli
cd apps/ui-shell/src-tauri
cargo tauri dev
```
