# TinyCut — Agent Rules

## Git Workflow (Auto Commit & Push)
- After completing any task and verifying that relevant tests/checks pass, automatically stage, commit with a concise Conventional Commits message, and push to the tracked remote branch (`origin`).
- Never commit secrets, `.env` files, local SQLite databases, or downloaded model weights.

## Verification Before Committing
- Run the relevant checks for modified areas before committing:
  - Core Rust workspace: `cargo test --workspace` and `cargo clippy --workspace --all-targets -- -D warnings`
  - WASM / feature-gate hygiene (when touching `core-domain` or `sync-client`): `cargo check -p tinycut-core-domain --no-default-features` and `cargo check -p tinycut-sync-client --no-default-features`
  - UI apps (when touching `apps/` or shared domain types): `cargo check --manifest-path apps/ui-shell/src-tauri/Cargo.toml` and `cargo check --manifest-path apps/ui-dioxus/Cargo.toml --target wasm32-unknown-unknown`

## Open-Core Repository Boundary
- This repository is **open-source** and contains only the local-first client app (`crates/`, `apps/`) and the versioned public `v1` wire protocol (`crates/core-domain/src/protocol.rs`).
- Do **not** add server-side data storage, billing/credit ledger logic, Cloud Boost GPU inference workers, or ML training pipelines to this repository.
- Keep `crates/core-domain` WASM-clean (`default-features = false`) and keep `crates/sync-client` auditable and feature-gated (`cloud-boost`, `telemetry`).
