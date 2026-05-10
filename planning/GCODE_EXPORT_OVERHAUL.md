# G-Code Export Overhaul — Roadmap

**Status:** Phase 0, 0.5, 1, 2, 3, 4a, 4b, and 5 complete. Phase 6 (power-user features) is the deferred backlog.
**Owner:** TBD
**Last updated:** 2026-05-10
**Worktree:** `/home/ricky/personal_repos/rs_cam-gcode-overhaul/` on branch `gcode-overhaul` (branched from `master` @ fe27805). All implementation work for this overhaul lives there; the main checkout stays on `master` for unrelated work and the other agent's optimizer changes.

## Why

The current emitter (`crates/rs_cam_core/src/gcode.rs`) is hand-rolled, with three `PostProcessor` impls (Grbl / LinuxCnc / Mach3) that each bake preamble, decimals, units, arc handling, and M6 sequencing into Rust code. Adding a dialect requires recompilation; adding a knob (units toggle, WCS, custom preamble) requires touching every impl. There is no external reference anchoring our output, so subtle modal or transition bugs can ship undetected — and bad g-code can break machines.

The fix is twofold: **anchor correctness against Fusion's published post library** via golden-file tests, then **separate dialect data from emission code** so new posts and knobs are configuration, not Rust.

## Goals

1. **Machine-safety confidence.** Every shipped dialect has byte-level parity (or normalized-diff parity) with a Fusion reference post on a fixed fixture corpus.
2. **Zero-Rust dialect addition.** New controllers ship as TOML, not code.
3. **Surface the right knobs in a wizard** — units, WCS, output split, tool-change preview, dry-run, validation summary — backed by real config, not workarounds.
4. **Catch transition-class bugs** (unsafe rapids, M6 with spindle on, missing safe-Z) with an invariant validator that runs on every emit.

## Non-goals

- Reinventing the post-processor *concept*. Fusion already nailed it; we are copying their decisions, not improving on them.
- Supporting controllers nobody asked for. Add posts on demand, not speculatively.
- Replacing the simulator. Validation is a cheap textual safety net, not a substitute for re-simulating output.

## Guiding principles

- **Data over code.** A dialect is a `PostDefinition` struct + a TOML file. The emitter is one code path.
- **One IR, one emitter.** All export modes (single / per-setup / per-toolpath) build the same `Program` IR; the emitter is mode-agnostic.
- **Reference parity is non-negotiable.** No dialect merges without a passing golden-file test against Fusion's `.cps`.
- **Prefer pure Rust deps.** `serde`, `toml`, `thiserror` are fine. Avoid template engines until justified — start with `str::replace` for variable substitution and upgrade only if conditionals creep in.
- **No `unwrap`/`expect`/`panic` in emitter or validator.** This is the lint policy, but it matters extra here: an `unwrap` in a post means a crash mid-export.
- **Newtypes at boundaries.** `Rpm(u32)`, `Feedrate(f64)`, `SafeZ(f64)` — formatting bugs from unit mixing have killed real machines elsewhere.

## Architecture target

```
                             ┌──────────────────────┐
   Toolpath + Job state ───▶ │  ProgramBuilder      │
                             │  (mode-aware:        │
                             │   single / per-setup │
                             │   / per-toolpath)    │
                             └──────────┬───────────┘
                                        │ Program (Vec<Statement>)
                                        ▼
                             ┌──────────────────────┐
   PostDefinition (TOML) ──▶ │  Emitter             │
                             │  (one impl, walks    │
                             │   Program, applies   │
                             │   PostDefinition)    │
                             └──────────┬───────────┘
                                        │ String (g-code)
                                        ▼
                             ┌──────────────────────┐
                             │  Validator           │
                             │  (modal + transition │
                             │   invariants)        │
                             └──────────┬───────────┘
                                        │ ExportResult { gcode, findings }
                                        ▼
                                 disk / wizard preview
```

### Key types (sketch)

```rust
// Mode-agnostic IR. One enum, exhaustive.
pub enum Statement {
    Comment(String),
    Rapid { x: Option<f64>, y: Option<f64>, z: Option<f64> },
    Feed  { x: Option<f64>, y: Option<f64>, z: Option<f64>, f: Feedrate },
    Arc   { plane: Plane, dir: ArcDir, end: Point3, center: ArcCenter, f: Feedrate },
    SpindleOn { rpm: Rpm, dir: SpindleDir },
    SpindleOff,
    Coolant(CoolantMode),
    ToolChange { tool: ToolNumber, comment: Option<String> },
    Dwell(Duration),
    Pause,                       // M0
    SetWcs(WcsCode),             // G54..G59
    SetUnits(Units),
    EndProgram,
    Raw(String),                 // user pre/post snippets — passed through verbatim
}

pub struct Program {
    pub statements: Vec<Statement>,
    pub metadata: ProgramMetadata, // job name, est time, etc.
}

// Data, not code. Loaded from posts/<name>.toml at startup.
#[derive(Deserialize)]
pub struct PostDefinition {
    pub name: String,
    pub units: Units,
    pub decimals: DecimalConfig,         // xyz, feed, ijk
    pub modal: ModalConfig,              // emit redundant G1, group axis words, ...
    pub arc: ArcConfig,                  // ijk vs r, max_radius_for_r, full_circle_split
    pub preamble: String,                // template w/ {spindle_speed} {safe_z} {units}
    pub postamble: String,
    pub tool_change: ToolChangeTemplate, // multi-line template w/ ordering rules
    pub commands: CommandMap,            // M6, M7/M8/M9, M30 vs M2, ...
    pub limits: PostLimits,              // max_rpm, max_feed for clamping
}

pub struct Emitter<'a> {
    post: &'a PostDefinition,
    state: ModalState, // tracks current G0/G1, F, units, WCS for elision
}

pub struct Validator;
pub struct ValidationFinding { pub kind: FindingKind, pub line: usize, pub message: String }
pub enum Severity { Info, Warning, Error }
```

### Where things live

```
crates/rs_cam_core/src/gcode/
    mod.rs              // public API: build_program, emit, validate
    ir.rs               // Statement, Program, ProgramMetadata
    program_builder.rs  // Toolpath -> Program, mode-aware
    post.rs             // PostDefinition, loader, validation of TOML
    emitter.rs          // single Emitter
    modal.rs            // ModalState (axis-word elision, motion-mode tracking)
    validator.rs        // invariant rules
    posts/              // shipped TOML dialects
        grbl.toml
        grblhal.toml
        linuxcnc.toml
        mach3.toml
    fixtures/           // job descriptions used to generate references
        pocket_2d.json
        profile_ramp.json
        ...
tests/
    posts/              // golden-file expected outputs
        grbl/pocket_2d.expected.nc
        grbl/profile_ramp.expected.nc
        ...
        regressions/    // bug-driven fixtures live here forever
```

## Phases

### Phase 0 — Reference corpus & gap report
**No production code changes. Pure investigation.**

**License decision (2026-05-10):** Fusion's `.cps` posts are `Copyright Autodesk, all rights reserved`, governed by the Autodesk License and Services Agreement — NOT open-source. We cannot redistribute them in this repo. The MIT license many sources cite applies only to Autodesk's `cam-posteditor` VS Code extension, not to the post files. Decision: treat `.cps` files as **local-only scratch reference**. The generated `.expected.nc` outputs ARE checked in (those are factual machine instructions, not Autodesk's creative work — the same as any tool's stdout in a golden-file test). Once the reference parity work is done in Phase 4, the local `reference/` dir can be deleted entirely; only the `.expected.nc` goldens remain.

- Pull Fusion's published posts (`grbl.cps`, `grblhal.cps`, `linuxcnc.cps`, `mach3mill.cps`) from `cam.autodesk.com/posts/posts/` to a **gitignored** `reference/posts/` dir at the worktree root.
- Read each post end-to-end. Document modal/transition decisions in `planning/post_reference_notes.md` (one section per dialect).
- Pick 6 fixture jobs covering: simple pocket, multi-pass profile w/ ramp, two-toolpaths-same-tool, two-toolpaths-different-tools, multi-setup, 3D adaptive (arc-heavy). Reuse existing `crates/rs_cam_core/tests/` fixtures where possible.
- Run current `emit_gcode_phased` on each fixture × dialect, save outputs to `planning/gcode_current_outputs/` for later diff.
- ~~For each fixture × dialect: post-process via Fusion~~ **Re-targeted (2026-05-10):** dropped Fusion-byte-parity as the success criterion. See [Phase 0.5](#phase-05--controller-emulator-validation-replaces-fusion-byte-parity) below. The remaining Phase 0 work is the spec-only gap analysis from reading the `.cps` source, which is sufficient to motivate Phase 1.
- Write `planning/gcode_gap_report.md` capturing the spec-only gaps surfaced from `.cps` reading + the inputs Phase 0.5 will need.

**Exit:** gap report exists; spec-only safety/encoding issues enumerated; reference notes exist; fixture corpus defined and committed.

---

### Phase 0.5 — Controller-emulator validation *(replaces Fusion byte-parity)*

**Why this exists:** Autodesk's `post` CLI is not available for Linux (it ships only inside Fusion 360, which has no native Linux build). Chasing byte-parity with Fusion would have required Wine + a Fusion install per contributor, with the binary under Autodesk EULA — fragile and non-hermetic. More importantly, byte-parity was a *proxy* for the real goal: machine safety. We can achieve that goal more directly by validating output against the **actual controller's parser**.

**Approach:** for each shipped dialect, install the canonical open-source emulator/parser for that controller in `reference/validators/` (gitignored), and pipe each fixture's emitted g-code through it. The controller either accepts the program (exit 0) or rejects it (non-zero + diagnostic). This is a strict upgrade over byte-parity: it tests what actually runs on the machine, not whether we coincidentally match one vendor's preferred style.

**Validators:**
- **Grbl:** [`grbl/grbl-sim`](https://github.com/grbl/grbl-sim) — compiles the real Grbl firmware as a Linux executable. Ships `gvalidate` for parser-level validation.
- **grblHAL:** [`grblHAL/Simulator`](https://github.com/grblHAL/Simulator) — ships `grblHAL_validator`, a synchronous CLI tool explicitly designed for CI batch validation.
- **LinuxCNC:** build `rs274ngc` (LinuxCNC's interpreter) from source, or use the `linuxcnc-uspace` package's bundled parser. Open-source, Linux-native.
- **Mach3:** no open-source emulator exists. Mach3 g-code is ≥90% LinuxCNC-compatible; use the LinuxCNC parser as proxy. Document any known divergences in the post's TOML notes (Phase 3).

**Deliverables:**
- `reference/validators/` (gitignored): each validator's source + built binary.
- `crates/rs_cam_core/tests/gcode_emulator_validation.rs`: integration test that, for each captured fixture × dialect, pipes through the matching validator and asserts exit 0 (or expected non-zero with documented reason).
- `planning/gcode_gap_report.md` updated: the "skeleton" rows fill in with real exit codes / diagnostics from the validators on our 18 current outputs. This is the *real* gap report.

**Exit:** every captured fixture × dialect either passes its validator or has a documented (and tracked) reason for failing. Failures become the work list for Phase 1+.

**Trade-off:** we lose the "byte-identical with Fusion" bragging right. We trade it for "passes the actual controller's parser in CI on every commit," which is what the safety claim was supposed to mean in the first place. Future stretch: integrate [CAMotics](https://camotics.org/) for full motion-path simulation + collision detection (Phase 6 if motivated).

---

### Phase 1 — Validator (safety net first) — **DONE**
**Land safety before refactoring.**

Implemented `crates/rs_cam_core/src/gcode_validator.rs` with **5 priority rules** focused on the machine-safety gaps surfaced in Phase 0+0.5:

1. **`UnsupportedM6`** — Grbl 1.1 doesn't implement M6 (gvalidate-confirmed real bug)
2. **`MissingG91_1`** — LinuxCNC arc-IJK absolute mode latent crash
3. **`WrongProgramEndCode`** — LinuxCNC must use M30, not M2
4. **`MissingProgramBrackets`** — LinuxCNC `%` tape begin/end
5. **`MissingWcs`** — explicit G54-G59 before first cutting move

Baseline test (`tests/gcode_validator_baseline.rs`) snapshots all 37 findings the current emitter produces across the 18 captured fixtures. Each finding kind has a clear resolution path tied to a downstream phase (mostly Phase 3 — data-driven `PostDefinition` lets each post declare which rules apply and which formats to emit).

**Deferred from the original plan:**

- Modal-state rules (`M6` preceded by spindle stop + safe-Z; `G0` preceded by Z lift; first cut after `M3` dwells) — need a proper modal-state machine, slots in cleanly with the Phase 2 IR refactor.
- Encoding rules (feed decimals, R-format arc radius, `M30`/`M2` modal spindle-off check, feed/rapid against `PostLimits`) — wait for the data-driven post (Phase 3) since they reference per-post configuration.
- Wiring validator into `emit_gcode_*` to return `(String, Vec<Finding>)` — deferred to Phase 2 when the IR refactor naturally surfaces an `ExportResult` boundary.

**Exit met:** validator runs (`cargo test -p rs_cam_core gcode_validator`); baseline locked at 37 findings across 18 captures (`cargo test -p rs_cam_core --test gcode_validator_baseline`); every existing test suite still green; clippy clean.

---

### Phase 2 — Extract `Program` IR — **DONE**
**Refactor without changing output.**

Module layout under `crates/rs_cam_core/src/gcode/`:

- `mod.rs` — public API surface (`emit_gcode`, `emit_program`, `PostProcessor`, `PostFormat`, etc.). The three legacy emit paths (`emit_gcode`, `emit_gcode_phased`, `emit_gcode_multi_setup`) are now thin wrappers that build a `Program` then render it via `emit_program`.
- `ir.rs` — `Statement` (Preamble, Postamble, ProgramPause, Comment, Raw, Rapid, Linear, LinearModal, ArcCw, ArcCcw, SafeZRetract), `Program`, `ProgramMetadata`. Each variant maps 1:1 to a byte slice the legacy emitter produced.
- `program_builder.rs` — `build_single`, `build_phased`, `build_multi_setup` produce `Program` from `Toolpath` inputs. Mirrors the legacy iteration order, modal-state transitions, and formatting decisions exactly.
- `modal.rs` — `ModalState` book-keeping (last_feed, current_rpm, current_tool, current_coolant) used by the builder.

Net diff: ~370 lines of imperative emission collapsed into the two-pass builder + emitter (~100 lines + ~390 lines of well-organized IR/builder code that Phase 3 will reuse against `PostDefinition`).

**Byte-identical verification:**
- 37 in-source unit tests in `gcode/mod.rs` pass unchanged.
- `gcode_validator_baseline` still snapshots 37 findings across the 18 captured fixtures.
- `gcode_phase0_capture --ignored` re-run produces ZERO diff in `planning/gcode_current_outputs/`.
- `cargo test --workspace` green; clippy clean.
- New `program_builder_is_deterministic` test guards against nondeterministic IR construction across all three builder entry points.

**Deferred to Phase 3:** newtype wrappers (`Rpm`, `Feedrate`, `SafeZ`) — adding them mid-refactor would have churned every test. Phase 3 introduces them naturally when `PostDefinition` lands.

**Exit met:** all three emit paths funnel through `ProgramBuilder`; existing tests byte-identical; clippy clean; new determinism test added.

---

### Phase 3 — Data-driven `PostDefinition` — **DONE**
**Replaced the trait with TOML.**

New module layout under `crates/rs_cam_core/src/gcode/`:

- `post.rs` — `PostDefinition` (`name`, `Decimals`, `CommentStyle`, `PostLimits`, preamble/postamble/program-pause templates), `serde::Deserialize` loader, plus `Rpm`/`Feedrate`/`SafeZ` newtypes (boundary-only — Statement IR keeps primitive types). Three shipped posts (`grbl()`, `linuxcnc()`, `mach3()`) embedded via `include_str!` and lazily parsed into `OnceLock` statics.
- `emitter.rs` — single `emit_program(&Program, &PostDefinition) -> String`. Move-line shape is hard-coded and parameterized by `decimals.{xyz,feed,ijk}`; preamble/postamble/program-pause come from TOML templates with `{spindle_rpm}` / `{message_comment}` substitution.

Three TOML posts live under `crates/rs_cam_core/posts/`:

- `grbl.toml` — 3 dp xyz/ijk, 0 dp feed, no G54.
- `linuxcnc.toml` — 4 dp xyz/ijk, 1 dp feed, G54 in preamble, `G53 G0 Z0` + M2 postamble.
- `mach3.toml` — 4 dp xyz/ijk, 1 dp feed, no G49, G4 P2 spindle dwell, `G28 G91 Z0` + M30.

Trait removal: `PostProcessor` trait, `GrblPost` / `LinuxCncPost` / `Mach3Post` impls, and `get_post_processor` helper deleted. `PostFormat::definition() -> &'static PostDefinition` and `get_post_definition(name) -> Option<&'static PostDefinition>` are the new public surface. CLI / viz / tests routed through `PostDefinition`.

**Byte-identical verification:**
- Side-by-side parity test (legacy trait vs new emitter, all 3 dialects × 6 fixtures + coolant/comp/raw edge case) green before deletion; removed once trait was gone (would be a tautology).
- `gcode_phase0_capture --ignored` re-run: ZERO diff in `planning/gcode_current_outputs/` (18 files unchanged).
- `gcode_validator_baseline`: still snapshots 37 findings unchanged.
- `cargo test --workspace`: green; clippy clean.

Net diff: ~325 lines of trait + impls deleted from `gcode/mod.rs`; ~470 lines added across `post.rs`, `emitter.rs`, and the three TOML files.

**Exit met:** `PostProcessor` trait deleted; all emission flows through `Emitter` + TOML; existing tests byte-identical; clippy clean; CLI/GUI surface unchanged. Newtype wrappers (`Rpm`, `Feedrate`, `SafeZ`) live in `post.rs` ready for Phase 4 limit enforcement.

---

### Phase 4a — Emulator-validation CI gate — **DONE**

Three validators surveyed; two wired into the test harness:

- **`gvalidate` (grbl-sim):** working, primary Grbl 1.1 parser; auxiliary syntax-check for LinuxCNC + Mach3 captures. Built once at `reference/validators/grbl/grbl/sim/gvalidate.exe`.
- **`rs274ngc` (LinuxCNC):** built from source under `reference/validators/linuxcnc/` (Ubuntu 24.04 has no apt package). Authoritative LinuxCNC + Mach3 (proxy) gate. Needs `--test-threads=1` (process-wide init state) and a generated tool table at `/tmp/rscam_rs274_tools.tbl`.
- **`grblHAL_validator` (grblHAL/Simulator):** built but unusable — `protocol_main_loop()` doesn't exit on EOF. Documented upstream bug; deferred to Phase 4b once upstream lands a fix. gvalidate covers grblHAL needs (grblHAL is a strict superset of Grbl 1.1).

Test harness (`crates/rs_cam_core/tests/gcode_emulator_validation.rs`) grew from 18 to 30 tests:
- 6 Grbl × gvalidate
- 6 LinuxCNC × rs274ngc + 6 LinuxCNC × gvalidate (auxiliary)
- 6 Mach3 × rs274ngc + 6 Mach3 × gvalidate (auxiliary)

CI gate via `CI_REQUIRE_VALIDATORS` env var: unset/0 → skip-on-missing; `1`/`true` → require all; csv (e.g. `gvalidate,rs274ngc`) → stage enforcement. GitHub Action job `gcode-emulator-gate` builds gvalidate, runs the test under `CI_REQUIRE_VALIDATORS=gvalidate` (rs274ngc CI build deferred to 4b — needs the LinuxCNC source build in the CI image).

**Per-fixture matrix:** see `planning/gcode_gap_report.md`. All 30 tests green, with one documented Grbl×F5 reject (M6 emitter bug — fix lands in 4b).

**Validator install steps:** see `planning/post_reference_notes.md` "Validator install".

---

### Phase 4b — Broaden corpus, grblHAL post, new PostDefinition fields — **DONE**

PostDefinition extended with three new boundary fields surfaced as data for the wizard (Phase 5):

- `wcs: Option<WcsCode>` (G54..G59) — drives `{wcs_word}` / `{wcs_line}` template substitution
- `units: Units` (mm | inch) — drives `{units_word}` (G21/G20)
- `arc_linearize: ArcLinearize { enabled, threshold_mm }` — consumed by `program_builder` when wired (deferred to Phase 5+; field already documents the contract)

`PostLimits.max_rpm` and `max_feed` are now enforced by the emitter via a new `Statement::SpindleSet` chokepoint and per-feed-word clamping in `Linear`/`ArcCw`/`ArcCcw`. Each clamp emits a comment line documenting requested vs clamped value. Shipped TOMLs leave `[limits]` unset, so the change is a no-op for default flows.

`posts/grblhal.toml` shipped — `PostFormat::GrblHal` variant wired through `definition()`, `get_post_definition`, validator invariants, and viz round-trip. Same decimals/comment style as Grbl; adds explicit G54 + WCS metadata.

Fixture corpus broadened from 6 to 16 (added F7-F16): full-circle, X-only feed, ramp-into-arc, sub-mm arcs, depth-step boundary, tool-change-at-Z-zero, climb-vs-conventional, multi-line pause message, embedded-newline pre/post snippets, G41/G40 round-trip.

**Bugs surfaced by the broadened corpus — all four FIXED** in the same Phase 4b cycle (commits `c7682e0` + `03b38a4` + `fecd7cb`):

1. ✅ **F10 sub-mm arcs**: every shipped post enables `arc_linearize`; the emitter substitutes a `G1` chord when arc radius < 0.05mm. Every parser accepts the linearised output.
2. ✅ **F14 multi-line pause messages**: `render_comment` and `render_program_pause` collapse `\n`/`\r`/`\t` into ` / ` / single-space so the comment stays on one parser-safe line.
3. ✅ **F15 M7 in user pre_gcode on Grbl**: new `PostDefinition.unsupported_mcodes` denylist (Grbl: `[7]`); emitter drops denied lines with a warning comment.
4. ✅ **F16 cutter compensation on Grbl/grblHAL**: new `PostDefinition.supports_cutter_comp` field; emitter drops G40/G41/G42 lines with a warning when the post doesn't support comp. LinuxCNC/Mach3 still emit comp natively (they support it); rs274ngc rejection is a documented validator-limitation, not an emitter bug.

**Verification:**

- `cargo test --test gcode_validator_baseline` — green at 98 findings across 64 captures (was 37/24).
- `RS274NGC_BIN=… cargo test --test gcode_emulator_validation -- --ignored --test-threads=1` — green at 96 tests (was 36).
- `cargo clippy --workspace --all-targets -- -D warnings` — clean.

**Total commit count for the overhaul** (Phase 0 through 4b inclusive): 27 commits on `gcode-overhaul`.

---

### Phase 5 — Wizard UX — **DONE**

Six-step Export Wizard modal in `crates/rs_cam_viz/src/ui/export_wizard.rs`, backed by `WizardState` on `ProjectSession` (resumable settings — `last_step_visited` puts the user back where they left off):

- **Step 1 — Post.** `PostFormat::ALL` dropdown, read-only metadata grid (units, default WCS, decimals, cutter-comp, arc-linearise), `PostLimits` warning when project RPM exceeds the post's `max_rpm`.
- **Step 2 — Output layout.** Radio for `OutputLayout::SingleFile` / `PerSetup` / `PerToolpath`, filename-template field with live `{job}/{setup}/{toolpath}/{ext}` preview, single-setup-PerSetup warning.
- **Step 3 — Coordinate & units.** WCS picker (G54..G59 + "Use post default"), units override (G21/G20 + "Use post default") with mismatch warning, safe-Z override behind a checkbox.
- **Step 4 — Tool change & spindle.** Read-only summary of tools used, with toolpath count + pre/post-snippet flags per tool and a "N tool changes required" callout. Spindle warmup dwell input. Coolant counts per `CoolantMode`.
- **Step 5 — Preview & validate.** Re-emits via `export_gcode_from_session` every frame; preview pane (first ~200 lines, monospace), findings list grouped by severity with icon/colour coding, "✓ No findings" banner when clean, override checkbox + red banner when any `Severity::Error` is present.
- **Step 6 — Save.** Summary table (post, layout, template, line count, moves, cutting distance, longest cut, est. cycle time, tool changes, validator findings). Save button dispatches `handle_wizard_save` which picks file or directory based on layout, applies template substitutions, respects the validator gate, writes file(s), and closes the wizard.

**Menu / shortcuts:** "Export G-code…" (Ctrl+Shift+E) opens the wizard; "Direct export (skip wizard)" submenu groups the legacy entries (All toolpaths via Ctrl+Alt+E, Combined for multi-setup, per-setup direct exports).

**`io::export` additions:** `export_single_toolpath_from_session` for the PerToolpath layout (mirrors `export_setup_gcode_from_session`).

**Tests:** `crates/rs_cam_viz/tests/wizard_e2e.rs` — four tests covering all three save layouts plus a state-mutation round-trip. The egui surface itself isn't driven (headless egui rendering is out of scope); the MCP layer doesn't currently expose wizard-driving tools, so the e2e covers the same data path the UI dispatches through.

**Exit met:** menu items reorganised; wizard data path tested end-to-end; legacy direct-export still reachable via menu submenu and Ctrl+Alt+E.

---

### Phase 6 — Power-user features (deferred)

Order by user demand, not speculation:

- **Editable preamble/postamble templates.** Per-project override of the post's templates, with variable substitution.
- **Per-tool pre/post g-code.** Move from `ToolpathConfig.pre_gcode/post_gcode` to `Tool.pre_gcode/post_gcode` (with per-toolpath override). Tool-change routines travel with the tool.
- **Dry-run mode.** Substitute Z with safe-Z in `Emitter` post-pass. Toggle in wizard.
- **Re-simulation gate.** Feed the emitted `Program` back through the simulator before saving — final modal/transition sanity check. The simulator already exists; this is wiring.
- **Custom user posts.** Pick up `~/.config/rs_cam/posts/*.toml` alongside shipped ones.
- **Post linter / authoring guide** if community contributions become a thing.

## Open questions

1. ~~**License audit on Fusion `.cps` files.**~~ **Resolved 2026-05-10:** Autodesk-copyrighted, not OSS. Local-only reference, generated outputs only in repo. See Phase 0 license decision.
2. **Normalization rules for diff.** Comments? Timestamps? Empty lines? Need a versioned `NormalizationProfile` so "matches reference" is reproducible.
3. **Template engine or string replace?** Start with `str::replace`. Upgrade to `minijinja` if tool-change blocks need conditionals (probe-or-not, length-comp-or-not).
4. **Where does WCS live?** Per-setup (each fixture origin = one WCS) or per-toolpath? Fusion does per-setup. Recommend matching that.
5. **Fixture corpus scope.** 6 fixtures is the floor. Likely grows to 15–20 as edge cases emerge. Budget for that.

## Appendix A — Reference posts inventory

| Dialect | Fusion `.cps` | Notes |
|---|---|---|
| Grbl 1.1+ | `grbl.cps` | Baseline reference. Most hobby CNCs. |
| grblHAL | `grblhal.cps` | Adds M6 macros, $TC, real-time overrides. |
| LinuxCNC | `linuxcnc.cps` | Full G-code spec. Trinity baseline. |
| Mach3 (mill) | `mach3mill.cps` | Legacy hobby/prosumer. |

Add as needed (Centroid, Masso, Buildbotics, Mach4, Smoothieware) on user request — each is one TOML + fixture set.

## Appendix B — Phase summary

| Phase | Scope | Breaks output? | Effort |
|---|---|---|---|
| 0 | Reference notes, fixture corpus, spec-only gap report | No | 1 day (done) |
| 0.5 | Controller emulator install + baseline validation | No | 1 day (done) |
| 1 | Validator (5 priority rules) | No | <1 day (done) |
| 2 | `Program` IR refactor | No (byte-identical) | 2–3 days (done) |
| 3 | Data-driven post (TOML) | No (byte-identical) | 3–4 days (done) |
| 4a | CI emulator gate (gvalidate + rs274ngc) | No | <1 day (done) |
| 4b | Broaden corpus + grblHAL post + new PostDefinition fields | No (additive) | 1 day (done) |
| 5 | Wizard UX | No (additive) | 1 day (done) |
| 6 | Power features (incl. CAMotics motion-sim option) | Per-feature | Open-ended |

**All non-deferred phases shipped on `gcode-overhaul`.** Phase 6 items remain on the backlog (per-tool pre/post g-code, dry-run mode, re-simulation gate, custom user posts, post linter, MCP wizard tools).
