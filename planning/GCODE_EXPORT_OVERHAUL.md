# G-Code Export Overhaul — Roadmap

**Status:** in progress (Phase 0)
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

### Phase 1 — Validator (safety net first)
**Land safety before refactoring.**

- Add `gcode/validator.rs` with rules:
  - `M6` must be preceded within 5 lines by spindle stop + Z move to safe-Z
  - Every `G0` with X or Y must be preceded by Z lift to safe-Z (unless the previous Z is already ≥ safe_z)
  - `G2`/`G3` with R-format → `|R|` ≥ 0.5 × chord length
  - `M30`/`M2` requires modal spindle off + coolant off
  - First cut after `M3` must be preceded by dwell (G4) of ≥ post-defined `spindle_warmup_ms`
  - Feed and rapid values within `PostLimits`
- Wire validator into `emit_gcode_*` — return `(String, Vec<Finding>)`.
- Run on every existing test fixture; investigate any findings (they may be real bugs).
- Add a regression test that snapshots findings count per fixture (zero is the goal).

**Exit:** validator runs on every emit; existing test suite green; any findings are either fixed or explicitly waived in fixture metadata.

**Why first:** any subsequent refactor risks regressions. The validator catches the worst class of regression (machine-breakers) before they ship, regardless of whether golden files cover the case.

---

### Phase 2 — Extract `Program` IR
**Refactor without changing output.**

- Define `Statement`, `Program`, `ProgramBuilder` in `gcode/ir.rs` and `gcode/program_builder.rs`.
- Refactor `emit_gcode_phased` into two passes:
  1. `Toolpath → Program` (in `program_builder.rs`)
  2. `Program → String` (still using current trait-based `PostProcessor`)
- All existing tests must remain **byte-identical** (the existing tests at `gcode.rs:1013+` are the safety net here).
- New test: `program_builder_is_deterministic` — same input → same Program.

**Exit:** `emit_gcode`, `emit_gcode_phased`, `emit_gcode_multi_setup` all funnel through `ProgramBuilder`; existing tests byte-identical; clippy clean.

**Risk:** "byte-identical" is brittle. If we discover the current emitter has a bug worth fixing en route, fix it and update the expected output in the same commit, with `gap_report.md` annotated.

---

### Phase 3 — Data-driven `PostDefinition`
**Replace the trait with TOML.**

- Define `PostDefinition` struct + `serde` impl.
- Author `posts/grbl.toml`, `posts/linuxcnc.toml`, `posts/mach3.toml` mirroring current behavior.
- Build new `Emitter` that consumes `Program` + `PostDefinition` → string.
- Side-by-side test: for each shipped post, the new emitter output equals the old `PostProcessor` output, byte-identical.
- Once green, delete the trait and the three impls. The `PostFormat` enum becomes a thin newtype around the post name (or a discoverable list of TOMLs in `posts/`).

**Exit:** `PostProcessor` trait deleted; all emission flows through `Emitter` + TOML; existing tests pass; clippy clean; CLI/GUI surface unchanged.

**Architecture clean-up nice-to-have:** include the TOML files in the binary via `include_str!` or `rust-embed` so end users get the shipped posts without filesystem lookups. Custom user posts live alongside in a config dir.

---

### Phase 4 — Broaden corpus & lock in emulator-validation CI gate
**Earn the safety claim through controller-parser passes, not Fusion bytes.**

- Grow the fixture corpus from 6 to 15-20 (add edge cases as they emerge: full-circle, single-axis-only-feed, ramp-into-arc, extremely small arcs, depth-step boundaries, etc.). Same `Toolpath`-based fixture style as Phase 0.
- Add `grblhal.toml` post + run its fixtures through `grblHAL_validator`. Source a community grblHAL `.cps` for design reference (Autodesk doesn't publish one).
- Add `WcsCode`, units toggle, arc-linearize as `PostDefinition` fields; surface to fixture jobs to verify each toggle changes output as expected and still passes the validator.
- CI gate: `cargo test --test gcode_emulator_validation` (from Phase 0.5) runs on every PR; any non-zero validator exit blocks merge.
- Optional secondary anchor: pick 1-2 hand-traced reference outputs from the `.cps` source for spot-check golden tests on cosmetic style (header layout, comment case). Strictly cosmetic — emulator validation is the safety gate.

**Exit:** every shipped dialect has both (a) a green emulator-validation suite covering 15+ fixtures and (b) at least one hand-traced cosmetic-style golden per dialect. New dialect = new TOML + new fixture rows + green emulator pass + (optional) hand-traced cosmetic golden.

---

### Phase 5 — Wizard UX
**Now the UI can be built against a stable, knob-rich backend.**

Replace the current "Export G-code (all)" / "Export Combined" / per-setup menu items with a single **Export Wizard** modal:

- **Step 1 — Post.** Dropdown of available posts (from `posts/` dir). Show post metadata (controller name, version, notes). Validate against `PostLimits` (max RPM, max feed) — flash warnings if project values exceed.
- **Step 2 — Output layout.** Radio: Single file / One file per setup (with M0 between) / One file per toolpath. File-naming template field.
- **Step 3 — Coordinate & units.** WCS picker (G54..G59). Units (auto from post / mm / inch override). Safe Z (per-project default + per-export override).
- **Step 4 — Tool change & spindle.** Per-tool pre/post snippets (read-only summary; edit in tool inspector). Spindle warmup dwell. Coolant default.
- **Step 5 — Preview & validate.** Render the first ~200 lines of output. Run `Validator`; show findings inline. Block "Save" if `Severity::Error` findings present (with override checkbox + scary warning).
- **Step 6 — Save.** File picker (or directory for split modes). Show summary: line count, est. time, tool changes, longest cut, validator findings count.

**UX detail:** the wizard should be **resumable** — settings persist per-project, so re-export uses last choices. Stored on `ProjectSession`.

**Exit:** menu items replaced; wizard tested end-to-end via `mcp` automation harness; current export flows still accessible via keyboard shortcut for power users.

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
| 0.5 | Controller emulator install + baseline validation | No | 1 day |
| 1 | Validator | No | 2 days |
| 2 | `Program` IR refactor | No (byte-identical) | 2–3 days |
| 3 | Data-driven post (TOML) | No (byte-identical) | 3–4 days |
| 4 | Broaden corpus + grblHAL + CI emulator gate | Maybe (intentional fixes) | 3–5 days |
| 5 | Wizard UX | No (additive) | 3–4 days |
| 6 | Power features (incl. CAMotics motion-sim option) | Per-feature | Open-ended |

**Total to land Phase 5: ~3 weeks of focused work.** Phase 0–1 alone (1 week) gets you the safety net even if the rest slips.
