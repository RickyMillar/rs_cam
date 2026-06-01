# Prompt — Feeds & Speeds modal finishing enhancements

**Predecessor:** SpindleStrategy work landed in commits `0bf8651`
(core), `267e19a` (CLI + MCP). Plan doc:
`planning/feeds_modal_enhancements_2026-06-02.md`.

**You are starting a fresh session.** Read the plan doc first, then
work through S1 → S2 → S3 → S4 in order. Each step is one commit
with the existing gate run between.

## Read these in order

1. `planning/feeds_modal_enhancements_2026-06-02.md` — the actual
   plan. Has scope, file pointers, test expectations, ordering
   rationale, exit gate.
2. `CLAUDE.md` at repo root — operational guardrails (the lint
   policy, the smoke runner, the test commands, the "don't" list).
3. `MEMORY.md` at `/home/ricky/.claude-personal/projects/-home-
   ricky-personal-repos-rs-cam/memory/MEMORY.md` — durable session
   memory. Pay attention to `feedback_no_concurrent_release_builds`,
   `feedback_no_background_field_locking`, and
   `feedback_worktree_extraction` in particular.

## Operational guardrails (read FIRST, no exceptions)

These come from prior-session feedback memory. Follow exactly:

- `cargo test --workspace` **loops indefinitely** — use
  `cargo test -p rs_cam_core --lib` and
  `cargo test -p rs_cam_core --tests` instead.
- **NEVER run `cargo test` while a release build of `rs_cam_viz`
  is in flight** — the combo thrashes swap and crashes the
  machine. Always `pgrep -af "cargo --release|rs_cam_gui"` first;
  if anything prints, wait or coordinate with the user.
- `cargo fmt` cascades to sibling modules — don't run workspace-
  wide; rely on `cargo clippy` for style enforcement.
- **Zero-warning clippy** (16 deny lints in `Cargo.toml`). Use
  `#[allow(clippy::the_lint)]` + `// SAFETY:` comment for provably
  safe sites only.
- Test modules carry
  `#[allow(clippy::unwrap_used, expect_used, panic,
  indexing_slicing)]` — use them freely in tests.
- **Never** stage `crates/rs_cam_core/src/feeds/explain.rs` or
  `crates/rs_cam_viz/src/ui/feeds_modal.rs` — both are untracked
  WIP that must stay unstaged. You CAN edit them (this round
  is largely about feeds_modal.rs); just never `git add` them.
  **This means the S1-S4 changes to feeds_modal.rs land as
  working-tree edits, NOT in the commits.** The core-side
  changes (operation_configs.rs, suggest.rs) DO get committed.
- **Never** touch `.mcp.json` or
  `MachineKinematics::shapeoko_xxl_ricky_tuned()`.
- **Never** use `--no-verify`, `--no-gpg-sign`, `--amend` for
  published commits, or `git reset --hard`.
- **Sentries that MUST stay green** through the whole work:
  F-024, F-026, F-027, F-028, F-031, F-036b, F-037 smoke baseline
  diff. Each step's exit gate re-runs these.

## What you're building

Four sequenced UX improvements on the Feeds & Speeds modal,
targeting the finishing-operation pain points the operator hit on
2026-06-02. Full scope at
`planning/feeds_modal_enhancements_2026-06-02.md`.

Short form:
- **S1**: `scallop_height: Option<f64>` on `DropCutterConfig` +
  scallop-derived stepover in suggest path + modal control.
- **S2**: Engaged-diameter-at-DOC annotation in modal for
  tapered/V-bit tools.
- **S3**: Chipload-min visual warning for finishing ops.
- **S4**: One-line attestation that chipload is at engaged
  diameter, not tip.

## Sequencing

Each commit gets its own gate. Don't skip — the smoke baseline is
the safety net.

```bash
# Per-step gate (paste at the end of each commit's work)
cargo test -p rs_cam_core --lib   # smoke + unit tests
cargo test -p rs_cam_core --tests # integration including F-037
cargo clippy --workspace --all-targets -- -D warnings
cargo run -p rs_cam_cli -- smoke --output /tmp/smoke_s${N}.csv
cargo run -p rs_cam_cli -- smoke --diff \
  --baseline planning/toolpath_acceptance/baselines/2026-06-04.csv \
  --output /tmp/smoke_s${N}.csv
```

Replace `${N}` with the step number (1 → 4).

## Key infra to lean on (already exists)

- `geometry::scallop_stepover(ball_r, scallop_height)` — the
  formula already exists in `crates/rs_cam_core/src/geometry.rs`
  (or similar — search for `scallop_stepover`). Returns
  `Option<f64>` (None when geometrically invalid).
- `ScallopConfig` (in `compute/operation_configs.rs`) already has
  `scallop_height: f64` — use it as the schema template for the
  DropCutter addition.
- `operation_feeds_hints` in `crates/rs_cam_core/src/feeds/suggest.rs`
  already pipes `cfg.scallop_height` for `OperationConfig::Scallop`.
  Mirror for `DropCutter` (the new field, `Option<f64>` — pass
  Some when set).
- `vendor_normalize::lookup_diameter_for_input(input)` in
  `crates/rs_cam_core/src/feeds/vendor_normalize.rs` already
  computes engaged diameter at DOC for tapered/V-bit. Use the
  same call from the modal for the S2/S4 annotation.
- The modal's derate-row pattern (`derate_row` helper around
  `feeds_modal.rs:870-955`) is the visual template for new rows.
- `ChiploadVerdict::Exceeds { side: ChipSide::Low, .. }` already
  fires from the gate — S3 just renders it differently in the
  finish-op context.

## What to expect re: smoke baseline

Default behaviour is preserved across all four steps:

- S1: `scallop_height = None` by default → existing formula path
  runs → ae unchanged → no smoke shift.
- S2: pure UI, no calc change → no smoke shift.
- S3: pure UI, no calc change → no smoke shift.
- S4: pure UI, no calc change → no smoke shift.

If the smoke diff DOES flag anything, stop and report — that's an
unexpected side-effect.

## Commit message templates

```
S1: feat(feeds): scallop-driven stepover on DropCutter

Adds `scallop_height: Option<f64>` to DropCutterConfig. When set,
the suggest path overrides the formula-default radial width with
`geometry::scallop_stepover(ball_r, h)`. For tapered ball tools the
tip radius drives cusp geometry (the cone half-angle does not enter
the cusp formula). The modal exposes a "Scallop height (μm)" input
that writes into the field; the stepover row renders as derived
when scallop-mode is active with the formula shown in the tooltip.

Default behaviour unchanged when scallop_height is None — F-037
smoke baseline preserved.

Three focused tests cover (1) some-overrides-formula, (2) none-
preserves-formula, (3) tapered-ball-uses-tip-radius-only.

S2: feat(ui): engaged-diameter-at-DOC annotation for tapered/V tools

S3: feat(ui): finishing chipload-min warning surfaces gate verdict

S4: feat(ui): attest chipload computed at engaged diameter
```

## Stop and consult the user if

- Smoke baseline shifts unexpectedly on any step.
- The scallop_stepover formula doesn't exist in geometry.rs (you'd
  need to add it — that's a separate piece of work).
- DropCutterConfig requires schema migration on existing project
  TOMLs (it shouldn't — `Option<f64>` with `#[serde(default)]`
  handles legacy files; verify against `wanaka.toml`).
- The modal layout breaks egui's existing constraints (too tall,
  scroll problems, etc.).

## After all four steps

Update the existing `planning/feeds_modal_enhancements_2026-06-02.md`
with a "Status (2026-06-XX): CLOSED" footer summarising the
commits. No new planning doc needed.

The follow-up workspace breakout work is tracked separately at
`planning/feeds_workspace_breakout_investigation_2026-06-02.md` —
do NOT start it from this prompt.
