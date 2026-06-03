---
name: verify
description: Run the full CI quality gate locally before committing
allowed-tools: Bash
---

# /verify — Pre-Merge Quality Gate

Run these steps in order. They match `.github/workflows/ci.yml` exactly.

## Steps

### 1. Format check
```bash
cargo fmt --check
```
Fix: `cargo fmt` then review changes.

### 2. Test suite (per-crate — avoid workspace-wide)
```bash
cargo test -p rs_cam_core -q
cargo test -p rs_cam_cli -q
cargo test -p rs_cam_viz -q
cargo test -p rs_cam_mcp -q
```
Run per-crate — workspace-wide `cargo test` from the repo root can loop / swap-thrash on this repo (see operational guardrails).
Fix: run failing test alone with `cargo test -p <crate> <name> -- --nocapture`.

### 3. Clippy (zero warnings)
```bash
cargo clippy --workspace --all-targets -- -D warnings
```
Fix: address each warning — the project enforces `-D warnings`.

### 4. Demo job smoke test
```bash
mkdir -p demos && cargo run -p rs_cam_cli -- job fixtures/demo_job.toml
```
Fix: check `fixtures/demo_job.toml` and `crates/rs_cam_cli/src/job.rs`.

### 5. Viz regression harness
```bash
cargo test -p rs_cam_viz controller::tests::
cargo test -p rs_cam_viz compute::worker::tests::
```
Fix: check `crates/rs_cam_viz/src/controller/tests.rs` and `compute/worker.rs`.

## All pass?
Ready to commit. All five steps must pass — this is what CI enforces on every PR.
