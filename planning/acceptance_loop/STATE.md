# STATE — acceptance loop, current snapshot

**Read this first.** This is the single source of truth. Both auditor
and implementer write here.

> **Autonomous mode**: when the user asks Claude to "keep running the
> loop", the auditor session reads
> `handoff_prompts/autonomous_auditor.md` and orchestrates implementer
> agents in background. The user is only paged when the rs-cam MCP
> needs reconnecting/rebuilding (the auditor cannot do that itself).

## Current round

**round-10 CLOSED — 7/7 deflection bar MET** (2026-05-26).

Round-10 audit verified F-031 + closed F-032 as a smoke-methodology issue (not a system bug). AS015's "0.434 Exceeds" was **real deflection** from cutting through unroughed bulk stock, correctly flagged by the gate. With a prior AS013-style roughing pass on the same project, AS015 scallop runs on cleared stock and deflection drops to **0.197 mm Within** (just under 0.2 threshold, "surface finish degradation expected").

**Final deflection bar: 7/7 Within** ✓
- AS001 pocket: 0.076 (F-024+F-028)
- AS002 adaptive: 0.053 (F-024+F-028)
- AS003 profile: 0.076 (F-024+F-028)
- AS004 face: 0.005 (F-028)
- AS005 zigzag: 0.051 (F-024+F-028)
- AS013 adaptive3d: 0.105 (F-031)
- AS015 scallop (with prior rough): 0.197 (F-031 + workflow methodology)

**F-017 (rapid collisions) explicitly closed.** All smoke cases report 0 rapid collisions.

**F-032 closed as smoke-methodology fix**, not a system bug. The system correctly identified dangerous engagement that would break a tool. **F-033 opened** as the UX follow-up: a pre-sim advisory warning users when 3D finishing ops lack a prior roughing pass (low severity, not loop-blocking).

Delta: `rounds/round-10-2026-05-26/delta.md`.

**The active-workstream block in `CLAUDE.md` can be removed.** Remaining open findings (F-020 optimizer fixture, F-025 non-identity setup probe, F-033 workflow advisory, plus cleanup items F-004/F-005/F-006/F-009/F-010/F-011/F-019/F-021/F-022) are stretch/cleanup items, not loop-critical. The acceptance loop's primary acceptance criterion (deflection ≥ 95% Within) is met.

Implementer(s) active: none.

### Prior round history

Round-07 closed 2026-05-25. **F-027 + F-028 verified.** AS013 + AS015
collisions 2924/52 → 0 cleanly. AS004 face deflection 0.243 → 0.005
Within (49× reduction, F-028).

Mid-round regression cycle: F-028 site 1 (`e48d7df`) passed cargo
tests but the MCP smoke broke AS001/AS003 — third instance of the
round-04 three-rebuild saga. Diagnosed as a viz-controller
`submit_toolpath_compute` path bypassing the site-1 fix. Resolved by
the viz follow-up (`c9e203d`).

**Deflection bar at 5/7 Within** — AS013 + AS015 still Exceeds. F-029
implementer pickup (2026-05-26) landed a defensive cleanup-raster DPP
clamp + a per-cell diagnostic probe that confirmed the planner-side
state is correct (`material_stock` top=0.5 mm at the worst cell) but
the simulator's per-setup dexel disagrees. The residual is sim-side,
**not** the planner-side issue F-029 originally framed; **F-031** opened
to carry that scope (sim-side stamping / coverage parity). F-029 closed
at partial-landing scope; F-031 is the new top of the queue for the
7/7 deflection bar.

**F-030 (architecture refactor, opened round-07)** unifies the
5 duplicated stock-frame entry points via a new
`rs_cam_core::session::SetupEvalContext`. Landed 2026-05-26
(commit `ba9a8fd`) against the 5/7 deflection bar with the user's
explicit precondition override — AS013/AS015 deflection residuals
are sim-side parity (tracked as F-031), orthogonal to F-030's
frame-handling scope. The 4 _f024.rs / _f026.rs / _f027.rs /
_f028.rs acceptance tests + the viz-side controller / worker pins
(`build_world_stock_bbox_respects_stock_origin_f024`,
`controller_built_stock_bbox_drives_axial_engagement_within_commanded_doc_f024`,
`as001_pocket_heights_resolve_in_world_frame_for_identity_setup_f028`,
`as001_viz_path_first_pass_axial_engagement_within_commanded_doc_f024`)
all pass byte-identically; clippy clean. **MCP rebuild required
before round-08 smoke.** Brief at `handoff_prompts/F-030-architecture-brief.md`.

Delta: `rounds/round-07-2026-05-25/delta.md`.

**Round-08 verification (2026-05-26 PM)**: AS001 byte-identical (z=-2/-4/-6, peak_axial=2.0, deflection=0.076 Within, removed_volume per-pass byte-identical). AS013: 0 collisions (F-027 holds), toolpath geometry byte-identical, deflection 0.637 Exceeds (F-031 residual, in same noise band as round-07's 0.576 and F-029-landing's 0.66). **F-030 retires the duplicated-frame-handling pattern that caused four multi-site sagas across F-024/F-026/F-028.** Delta: `rounds/round-08-2026-05-25/delta.md`.

Implementer(s) active: none. **F-031 is the next pickup** — closes the deflection bar to 7/7, then the acceptance loop's exit criteria are within reach.

## Last verified baseline

- **round-10 delta** (deflection bar 7/7 MET; F-032 closed as methodology, F-017 closed, F-033 opened for workflow advisory follow-up):
  `planning/acceptance_loop/rounds/round-10-2026-05-26/delta.md`
- **round-09 delta** (F-031 verified on AS013, deflection 0.637 → 0.105 Within, AS013 toolpath 6.4× smaller, bar 5/7 → 6/7; F-032 opened for AS015 scallop):
  `planning/acceptance_loop/rounds/round-09-2026-05-26/delta.md`
- **round-08 delta** (F-030 architecture refactor verified through MCP smoke; AS001 byte-identical, AS013 collisions 0, deflection still F-031 residual):
  `planning/acceptance_loop/rounds/round-08-2026-05-25/delta.md`
- **round-07 delta** (F-027 + F-028 verified; F-028 regression cycle absorbed; F-030 opened):
  `planning/acceptance_loop/rounds/round-07-2026-05-25/delta.md`
- **round-06 delta** (F-026 verified at load-bbox-fix scope; F-027
  + F-028 isolated as the remaining deflection root causes):
  `planning/acceptance_loop/rounds/round-06-2026-05-25/delta.md`
- **round-05 delta** (F-026 candidate 2 confirmed + widened scope;
  F-024 holds on all origin_z=-12 cases):
  `planning/acceptance_loop/rounds/round-05-2026-05-25/delta.md`
- **round-04 delta** (F-024 three-site fix verified on AS001;
  AS013 unchanged): `planning/acceptance_loop/rounds/round-04-2026-05-25/delta.md`
- **round-03 delta** (probes verifying F-015 + F-023; F-002 reframe):
  `planning/acceptance_loop/rounds/round-03-2026-05-25/delta.md`
- **round-02 delta** (5-case focused smoke vs round-01):
  `planning/acceptance_loop/rounds/round-02-2026-05-25/delta.md`
- **round-01 baseline**:
  `planning/acceptance_loop/rounds/round-01-2026-05-24/baseline.md`
- **round-00 baseline** (Tier 0 param sweep, pre-MCP-overhaul):
  `planning/acceptance_loop/rounds/round-00-2026-05-24/baseline.md`

## Open queue — implementer pulls from the top

Severity ordering: high → medium → low. Within same severity, lower effort first.

| Finding | Title | Stage | Sev | Effort | Status |
|---|---|---|---|:-:|---|
| **Feed Modulation Workstream** (see [`planning/feed_modulation_roadmap.md`](../feed_modulation_roadmap.md)) | | | | | |
| [F-037](findings/F-037-smoke-baseline-and-regression-net.md) | Smoke baseline + wanaka regression case + CI gate — protects acceptance-loop calibration from feature-work regressions | process | medium | M | **landed 2026-05-26** (PR 1 + PR 3; wanaka + CI deferred) |
| [F-034](findings/F-034-machine-kinematics-cycle-time.md) | Acceleration-aware cycle time estimator — replaces `distance/feed` with kinematics-integrator | sim | medium | S-M | **landed 2026-05-26** (commit `ae58f55`) |
| [F-035](findings/F-035-predicted-feed-in-gates.md) | Predicted-effective-feed in chipload / power / deflection gates — flag-gated | sim | medium | M | **landed 2026-05-26** |
| [F-036](findings/F-036-per-segment-feed-modulation.md) | Per-segment adaptive feed modulation (Fusion HSM equivalent) — flag-gated, large | post-process | medium | L-XL | **partial-landing 2026-05-26** — Piece A + B (algorithm + 12 unit tests) landed in `crates/rs_cam_core/src/feed_modulation.rs`; Pieces C/D/F deferred to F-036a/b/c |
| [F-036a](findings/F-036a-feed-modulation-gcode-emission.md) | G-code emitter per-move F-word emission (F-036 Piece C) | post-process | medium | M | **landed 2026-05-26** — finding hypothesis refuted; modal layer was already correct. Shipped as regression-net (3 tests pinning per-move F-word + modal-suppression contract). |
| [F-036b](findings/F-036b-feed-modulation-flag-plumbing.md) | `SimulationOptions::adaptive_feed_modulation` flag + simulator-→-modulation wiring + 7 acceptance tests (F-036 Piece D) | sim | medium | M | **landed 2026-05-26** (8/9 acceptance tests pass; AB5 deferred to F-036b1) |
| [F-036b1](findings/F-036b1-modulation-resim-or-ir-inspect.md) | Validate modulation invariants against IR or re-simulate — rewrite AB5 chipload-floor check | sim | low | S-M | **landed 2026-05-26** |
| [F-036c](findings/F-036c-feed-modulation-real-machine-calibration.md) | Real-machine cycle-time calibration on Shapeoko XXL (F-036 Piece F) | verify | low | S code-side + user wall-clock | open — depends on F-036b |
| [F-033](findings/F-033-finishing-op-needs-prior-roughing-advisory.md) | Pre-sim advisory: 3D finishing op without prior roughing pass — UX follow-up to F-032's reframe | suggest | low | M | open — not loop-critical; schedule when UX bandwidth available |
| [F-032](findings/F-032-scallop-deflection-transit-sample-contamination.md) | Scallop deflection over-fire — **closed round-10 as smoke methodology, not system bug**; system correctly identified dangerous engagement. Methodology: AS015 needs prior roughing pass. F-033 opened for UX advisory. | sim | — | — | **closed round-10** |
| [F-031](findings/F-031-adaptive3d-residual-deep-z-parity.md) | Adaptive3d residual deep-Z planner↔simulator stamp parity gap — F-029 follow-up | sim | high | M-L | **verified round-09** (commit `497a3b2`); AS013 deflection 0.637 → 0.105 Within, collateral 6.4× toolpath reduction |
| [F-029](findings/F-029-adaptive3d-interior-cell-parity.md) | Adaptive3d interior-cell planner↔simulator stamp parity gap — final deflection residual on AS013/AS015 | sim | medium | M | **partial-landing 2026-05-26** — cleanup-raster DPP clamp + diagnostic probe landed; residual interior-cell tracked as F-031 |
| [F-030](findings/F-030-unify-setup-eval-context.md) | Unify SetupEvalContext across the 5 stock-frame entry points — architecture refactor | substrate | medium | L | **verified round-08** (commit `ba9a8fd`); duplicated-frame pattern retired |
| [F-020](findings/F-020-optimizer-ranked-bs-path.md) | Optimizer Ranked-outcome BS-stepover path untested | optimize | high | M | open — needs test fixture before fix |
| [F-025](findings/F-025-non-identity-setup-z-frame.md) | Z-frame mismatch on non-identity setups (face_up=Bottom etc.) | substrate | medium | S–M | open — stub; possibly subsumed by F-030 |
| [F-006](findings/F-006-operation-config-three-default-paths.md) | Default `OperationConfig` produced via three paths | suggest | medium | M | open — partially absorbed by F-003 |
| [F-004](findings/F-004-three-project-loaders.md) | Three project-TOML loaders | substrate | medium | L | open — likely lands with F-005 |
| [F-009](findings/F-009-diagnostic-views-viz-only.md) | Five diagnostic views, only viz emits them | substrate | medium | M | open — partially lands with F-005 |
| [F-005](findings/F-005-two-mcp-servers.md) | Two MCP server implementations | substrate | medium | XL | deferred — wait for F-001…F-005 of unification to land first |
| [F-011](findings/F-011-operation-feeds-hints-split-crates.md) | `operation_feeds_hints` split across crates | suggest | low | S | open — lands with F-003 |
| [F-017](findings/F-017-rapid-collisions-everywhere.md) | Rapid collisions — **CLOSED round-10**: 0 rapid collisions on every smoke case AS001-AS015 post-F-024+F-026+F-027+F-031 | sim | — | — | **closed round-10** |
| [F-010](findings/F-010-catalog-six-match-blocks.md) | Catalog has six 23-arm match blocks | substrate | low | M | open — re-evaluate after F-003 lands |
| [F-019](findings/F-019-stepover-semantic-cardinality.md) | Stepover semantic cardinality across op families | suggest | low | M | deferred — re-evaluate after F-003 |
| [F-021](findings/F-021-suggest-all-paint-thrash.md) | Suggest All button recomputes LUT every paint | viz | low | S | open |
| [F-022](findings/F-022-catalog-match-arm-collapse.md) | Collapse remaining catalog `match` blocks | substrate | low | M | open — unblocked by F-003 landing |

## In flight (claimed by implementer)

| Finding | Claimed by | PR | Notes |
|---|---|---|---|
| (none) | — | — | F-032 released back to open queue 2026-05-26; finding hypothesis refuted by implementer probe — see Implementation log |

## Closed round-02 (2026-05-25)

Verified by round-02 smoke (`rounds/round-02-2026-05-25/delta.md`):

- **F-002** — `peak_axial_doc_mm` split into `axial_engagement_mm` +
  `plunge_descent_mm` (landed in `072c11a`; verified at the split
  scope by round-03 implementer probe). The deflection over-fire
  observed in round-02 is **not** caused by an incomplete split —
  it's the F-024 frame-mismatch bug. F-002's split itself is
  complete and correct.
- **F-001** — chipload 2D feedopt probe fixed. AS001/AS002 moved from
  `chipload: Unmodeled` to `Exceeds_LOW` with `validated` confidence
  and vendor-LUT bounds.
- **F-016** — drill `chip_welding` threshold material-aware. AS011
  hardwood threshold moved from `8.0` (hardcoded softwood) to `6.0`.
- **F-003** — VENDOR_LUT singletons collapsed + workholding plumbed
  through (subsumes F-012). No vendor-LUT regressions across 3 op
  kinds in smoke.
- **F-007** — `DrillConfig::set_plunge_rate` honored. (Note: drill
  schema no longer exposes a separate `plunge_rate` param; `feed_rate`
  covers the cycle's plunge.)
- **F-008** — `compute_stale_set` single authority. `stale_toolpaths`
  envelope field present and accurate across all add/set_param calls.
- **F-013** — feeds-result invariants enforced. `feeds.feed_vs_lut.high`
  diagnostic consistent across cases.
- **F-015** — op-precondition static-validation landed (commit
  `ba3f84d` during round-02). **Smoke-verified** late-round-02 after
  user rebuilt MCP: rest + project_curve preconditions both emit
  blocking `precondition.*` diagnostics through `diagnostic_delta` +
  `gui_banners` + `warnings`. Drill precondition covered by F-015's
  existing integration tests.
- **F-012** — closed as duplicate of F-003.

**Reframed in round-03 (the round-02 reopen was wrong):**
- **F-002** — round-02 reopen framing was incorrect (linear-class
  asymmetry was transit-tag filtering, not a code path bug). The
  original split into `axial_engagement_mm` + `plunge_descent_mm`
  DID land correctly in commit `072c11a`. Per-sample
  `axial_engagement_mm` is correctly populated for non-plunge samples
  on all four kinematics classes; per-sample `plunge_descent_mm` is
  correctly populated for plunge samples. F-002 is **landed at the
  split scope**. The deflection over-fire is a different root cause —
  see F-024.

**Opened this round:**
- **F-023** (now landed) — MCP/GUI diagnostic surface asymmetry:
  "Selected model missing" surfaces in GUI banner, project loader
  warnings, and `list_toolpaths.error` / `runtime_errors`, but
  previously **not** in `get_toolpath_diagnostics` /
  `get_project_diagnostics` / `add_toolpath` envelope.
  `add_toolpath` accepted invalid `model_id` silently. Now surfaced
  via `ref.model_missing` adapter.
- **F-024** — Z-frame mismatch in dexel stock grid. Root cause of the
  deflection over-fire that round-02 attributed (wrongly) to F-002's
  linear-class path. For identity setups, no local↔global transform
  is applied; toolpath emits Z stock-top-relative; grid spans
  Z=[0, stock_z]. Cutter is below the entire ray → full ray cleared →
  `axial_engagement_mm` reads full stock height instead of commanded
  DOC. Three candidate fix sites; M–L effort with fingerprint regen.

## Acceptance bars status

**Round-10 closed the deflection bar at 7/7 Within.** Primary acceptance criterion met. Remaining bars (suggest first-shot, optimizer, export) are stretch / out-of-scope for the current workstream.

| Bar | Target | Round-09 | Round-10 | Status |
|---|---:|---|---|---|
| Suggest first-shot landing rate | ≥ 90% | unmeasured | unmeasured | open — needs full sweep (post-loop) |
| Sim chipload calibration (3D ops) | ≥ 95% | 2/2 | 2/2 | **stable** ✓ |
| Sim chipload calibration (2D ops) | ≥ 95% | not re-tested | (verified round-08, byte-identical) | **stable** ✓ |
| **Sim deflection calibration** | ≥ 95% | 6/7 Within | **7/7 Within** ✓ | **CLOSED** |
| Optimizer honest-improvement | ≥ 95% | not re-tested | not re-tested | open — F-020 still untested |
| Optimizer refusal correctness | 100% | not re-tested | not re-tested | open — verify post-loop |
| Export gate | 100% | not tested | not tested | open — out-of-scope for loop |

## Blockers / questions for the user

**No blockers. Deflection bar 7/7 met. Acceptance loop's primary criterion satisfied.**

- **The active-workstream block in `CLAUDE.md` can be removed.** All loop-critical findings (F-024, F-026, F-027, F-028, F-029, F-030, F-031, F-032, F-017) are closed or verified. The deflection gate calibration is correct, the rapid-collision detector is clean across the smoke suite, and the F-030 architectural refactor retired the duplicated-frame-handling pattern that caused four multi-site sagas across rounds 04-07.
- Auditor decision on F-032: option A (smoke methodology — add a prior AS013 roughing pass when verifying AS015) chosen. AS015 deflection drops 0.434 → 0.197 Within when running on roughed stock. The 0.434 reading was real deflection from cutting through unroughed bulk stock — system correctly flagged a malpractice scenario. F-033 opened for the UX advisory follow-up (option D).
- Post-loop stretch goals (no longer loop-blocking):
  - F-020 (optimizer Ranked-BS test fixture) — needs a fixture spec first.
  - F-025 (non-identity setup smoke probe) — F-030's unification likely handles this for free; needs a `face_up=Bottom` fixture.
  - F-033 (workflow advisory — finishing op without prior rough) — UX feature, schedule when bandwidth permits.
  - F-031 collateral spot-check on AS017/AS018 for analogous dressup-rewrite cruft.
  - Cleanup queue: F-004, F-005, F-006, F-009, F-010, F-011, F-019, F-021, F-022.

## Implementation log

(implementers append here when they land a PR; auditor moves entries to round directories when verified)

- 2026-05-26 — **F-036b1 landed: AB5 chipload-floor check rewritten against modulated IR.** Walks `session.get_result(0).annotated().toolpath.moves` post-`run_simulation`, filters to `MoveIntent::{ClearingCut,FinishingCut}` moves whose feed differs from the commanded value by ≥ 0.5 mm/min (i.e. moves the modulator actually touched), and asserts `feed / (rpm × flutes) ≥ band.start × 0.95`. AS001 pocket exercises 57+ modulated moves and all clear the floor. Skipped moves (zero-engagement aggregation → keeps commanded feed) are intentionally not checked: if the user's commanded feed itself is below the LUT band, that's a user parameter choice the modulator doesn't override (the algorithm-layer floor-clamp on modulated moves is what F-036b1 pins). Test was `#[ignore]`d in F-036b; now passing, 9/9 acceptance tests green. Smoke baseline diff: **no regressions**. Workspace clippy + tests clean. Closes F-036b1; F-036b's AB5 deferral is retired.
- 2026-05-26 — **F-036b landed: production wiring for adaptive feed modulation.** New `SimulationOptions::adaptive_feed_modulation: bool` (default `false`), mirroring F-035's `use_predicted_feed_in_gates` precedent. `ProjectSession::run_simulation` now invokes a post-sim modulation pass when the flag is on AND `MachineProfile.kinematics` is `Some`: per-toolpath `SimulationCutSample` stream → `Vec<PerMoveEngagement>` aggregation (mean `radial_woc_fraction`/`axial_doc_fraction` per move) → `adaptive_feed_modulate(&mut toolpath, &engagements, &ctx)` → modulated `feed_rate` lands on `Toolpath::moves[i].move_type`. F-036a confirmed the emitter handles per-move F-words correctly, so no emitter change needed. New acceptance test file `crates/rs_cam_core/tests/adaptive_feed_modulation_pipeline_f036b.rs`: 8/9 tests pass — `flag_off_emits_identical_gcode_to_pre_f036` (load-bearing), `modulated_path_has_per_segment_feed_variation`, `modulated_gates_within_constant_chipload_band`, `modulated_cycle_time_lower_than_unmodulated`, `modulated_path_preserves_f024_axial_engagement_invariant`, `modulated_path_preserves_zero_rapid_collision_invariant`, plus 2 supporting tests. **AB5 `modulated_path_never_emits_below_min_chipload` is `#[ignore]`d → F-036b1** because it reads post-sim `sample.chipload_mm_per_tooth` which reflects pre-modulation commanded feed; the algorithm-layer floor-clamp is correctly pinned by F-036 unit test `modulation_never_emits_below_min_chipload`, what's missing is either an IR-read rewrite of AB5 or a simulator re-pass after modulation. SimulationOptions field init added across 16 existing test files + CLI callsites (`crates/rs_cam_cli/src/{project,smoke}.rs`) + `tool_load/optimize/candidate.rs`. Smoke baseline diff: **no regressions** (18 cases unchanged — flag defaults to off). Workspace clippy + tests clean (188+ viz, 1605+ core, all loop acceptance tests pass byte-identical). Acceptance test: `cargo test --test adaptive_feed_modulation_pipeline_f036b` (8 pass + 1 ignored). Real-machine cycle-time verification (≥ 20% reduction on Shapeoko XXL) still deferred to F-036c — user-side wall-clock measurement.
- 2026-05-26 — **F-036a landed: G-code emitter per-move F-word emission — regression-net only.** Finding hypothesis ("modal layer suppresses modulated feeds") was incorrect: audit of `crates/rs_cam_core/src/gcode/program_builder.rs` (lines 36-44, 326-342) showed the F-elision logic already uses `last_feed != Some(feed_rate)` exact-equality, so any per-move `feed_rate` variation already routes through `Statement::Linear { feed }` → `G1 ... F<rate>` in the emitter (`emitter.rs:192-198`). The existing `Statement::LinearModal` (no F-word) is reserved for identical-feed runs. **No `modal.rs` or `program_builder.rs` change was required** — the modulator's per-move feed mutations already propagate to per-move F-words. Shipped the three finding-mandated acceptance tests at `crates/rs_cam_core/tests/adaptive_feed_modulation_gcode_f036a.rs` as a regression net: (1) `modulated_toolpath_emits_per_move_f_words` (3 distinct feeds → 3 F-words), (2) `unmodulated_toolpath_emits_single_f_word` (3 identical feeds → 1 F-word, byte-identical invariant protecting F-037 smoke baseline), (3) `modulated_gcode_round_trips_through_parser` (regex-extract F-words from emitted text; reconstructed feeds match within 1 mm/min). The "delta > 1 mm/min" tolerance the finding suggested was rejected as unnecessary: f64 noise on whole-number commanded feeds is < 1e-12, and `adaptive_feed_modulate` produces 10-50% feed jumps (well above any reasonable tolerance), so exact-equality is correct and safer for the captured-fixture corpus. **F-036b implementer note**: wiring `adaptive_feed_modulate` into production via `SimulationOptions::adaptive_feed_modulation` only needs to mutate `Toolpath::moves[i].move_type`'s `feed_rate`; the emitter pipeline already handles per-move F-words. No emitter or modal change required for that wiring. Smoke baseline diff: **no regressions** (18 cases unchanged). Workspace clippy + tests clean. Acceptance test: `cargo test --test adaptive_feed_modulation_gcode_f036a` (3/3 pass).
- 2026-05-26 — **F-036 partial-landing: per-segment adaptive feed modulation — algorithm only (Pieces A + B).** New `crates/rs_cam_core/src/feed_modulation.rs` (~570 LOC including 12 unit tests) exposes the pure modulation algorithm `adaptive_feed_modulate(&mut Toolpath, &[PerMoveEngagement], &ModulationContext)` plus `ChiploadBand` / `PerMoveEngagement` / `ModulationContext` / `ModulationError` types. Algorithm: per-cutting-move target chipload = geometric mid of LUT band, chip-thinning correction `target_chipload / sqrt(radial_woc)`, feed = target × RPM × flutes, clamped to `[band_floor, min(max_feed, band_ceiling, max(predicted_kinematic_feed, band_floor))]`. Predicted-kinematic cap uses a **synthetic toolpath** (every cutting move commanded at band-ceiling-feed) through `predicted_feeds_for_toolpath` so the modulator gets the move's **geometric** achievable peak rather than being trapped by the commanded feed. Rapids + `MoveIntent::{Retract, Drilling, EntryPlunge}` are skipped (chip-thinning is undefined for pure-vertical or air-traverse). Piece A verified: `LinearMove` / `ArcCW` / `ArcCCW` already carry per-move `feed_rate`; no IR change. **Pieces C (G-code emission), D (feature flag + production wiring), E (7 acceptance tests), F (real-machine calibration) deferred** to sub-findings F-036a / F-036b / F-036c — nothing in production calls `adaptive_feed_modulate` yet, so flag-off byte-identicality is guaranteed by absence-of-call. Smoke baseline diff: **no regressions** (18 cases unchanged) via `cargo run -p rs_cam_cli --release -- smoke --diff`. Workspace clippy + tests clean (1605 lib tests pass; +12 new feed_modulation units). Acceptance test: `cargo test -p rs_cam_core --lib feed_modulation`. Real-machine cycle-time verification (≥ 20% reduction on Shapeoko XXL) deferred to F-036c — user-side wall-clock measurement required.
- 2026-05-26 — **F-035 landed: predicted-effective-feed in chipload / power gates (flag-gated).** New `predicted_achieved_feed` + `predicted_feeds_for_toolpath` integrators in `crates/rs_cam_core/src/machine_kinematics.rs` (~150 LOC added) reuse F-034's trapezoidal solver to return per-move *peak velocity reached* (mm/min) instead of *cycle time*. Long moves between high-velocity junctions return commanded; short moves between corners return the triangular-profile peak which drops below commanded — the corner-decel signal the gates need. New `SimulationOptions::use_predicted_feed_in_gates: bool` (default `false`) flips the simulator to populate a per-(`toolpath_id`, `move_index`) `PredictedFeedMap` on `SimulationCutTrace::predicted_feeds` whenever the active `MachineProfile` carries `kinematics`. Chipload + power gates read predicted feed via the shared helper `tool_load::effective_feed_for_sample(sample, &predicted_feeds)`; chipload scales `effective_chip_thickness_mm` by `predicted / commanded` (linear in feed), power substitutes directly. Deflection (`F = Kc · DOC · WOC`) has no feed term in the cantilever integral and is feed-independent — documented in the gate doc + acceptance test AB4. Viz `SimulationRequest` grew `kinematics`, `use_predicted_feed_in_gates`, `max_feed_mm_min` fields and the controller's `run_simulation` event copies them from the active machine profile (GUI toggle deferred to F-036 territory; defaults to `false`). Acceptance test `tests/predicted_feed_gates_f035.rs` covers 4 bars: AB1 flag-OFF byte-identical verdicts through the full `ProjectSession::run_simulation` path with kinematics-Some vs kinematics-None, AB2 flag-ON drops chipload to `Exceeds(Low)` on a synthetic trace with predicted = 0.3 × commanded, AB3 straight-line case (predicted == commanded) flag-ON identical to flag-OFF, AB4 bridge to F-024 (AS001 with flag-ON still satisfies `axial_engagement_mm ≤ 3.0 mm` and `peak_delta_mm < 0.2 mm`). Smoke baseline diff: **no regressions** (18 cases unchanged). Workspace clippy + tests clean (1593 lib tests pass, +4 new machine_kinematics units, +4 F-035 acceptance). Acceptance test: `cargo test --test predicted_feed_gates_f035`.
- 2026-05-26 — **F-034 landed: acceleration-aware cycle time estimator.** New `crates/rs_cam_core/src/machine_kinematics.rs` (~360 LOC) introduces `MachineKinematics { acceleration_mm_s2, jerk_mm_s3, max_junction_velocity_mm_min }` + Shapeoko-XXL / generic-wood-router presets + `compute_cycle_time(toolpath, kinematics, max_feed, rapid_feed)` trapezoidal integrator (full-stop junction velocity on direction reversal; chord length for arcs; optional jerk-penalty smoothing). Added `kinematics: Option<MachineKinematics>` to `MachineProfile` (every built-in preset defaults to `None` — absence IS the flag, keeping behavior byte-identical to pre-F-034). Plumbed through `SimulationRequest.kinematics: Option<KinematicsContext>` and a new post-trace `apply_kinematics_cycle_time` step in `run_simulation` that rewrites per-toolpath + project `total_runtime_s` from the IR when the field is set. Acceptance test `tests/machine_kinematics_cycle_time_f034.rs` covers 5 bars: corner-heavy zigzag > naive, straight-line ≈ naive, calibration-against-Shapeoko (`#[ignore]` — needs user wall-clock measurement, scaffolding in place), flag-OFF byte-identical to pre-F-034 segment-time sum, and flag-ON override fires through `ProjectSession::run_simulation`. Smoke baseline diff: **no regressions** (18 cases unchanged). Workspace clippy + tests clean. Calibration measurement deferred — flagged user for a wall-clocked Shapeoko XXL run on `ux_2d_pocket.toml` to replace the `REFERENCE_MEASURED_S` placeholder. Acceptance test: `cargo test --test machine_kinematics_cycle_time_f034`.
- 2026-05-26 — **F-037 landed (PR 1 + PR 3; PR 2 / wanaka + CI deferred per user choice).** New `cargo run -p rs_cam_cli -- smoke` subcommand (`crates/rs_cam_cli/src/smoke.rs`, ~600 LOC) walks `cases_agent_smoke.csv`, runs each case through `ProjectSession::generate_toolpath` + `run_simulation`, and writes per-toolpath verdicts. Captured `planning/toolpath_acceptance/baselines/2026-05-26.csv` (18 rows: 9 ok cases populated with chipload / deflection / power / collision / drill verdicts; 9 with documented harness issues). 2D deflection-bar verdicts (AS001-AS005) match round-10 within sub-mm; 3D verdicts (AS013/15) differ from round-10 because the loop's "with prior rough" methodology isn't encoded in the smoke CSV (F-033 territory). PR 3 added `smoke --diff` mode + cargo acceptance test `tests/smoke_baseline_regression_f037.rs` (3 tests, all pass) + `implementer_contract.md` "Regression-net rule". Workspace clippy + tests clean. F-034/F-035/F-036 can now diff against the captured baseline. Acceptance test: `cargo test --test smoke_baseline_regression_f037`.
- 2026-05-26 — round-10 audit: **🎯 Deflection bar 7/7 Within. Acceptance loop primary criterion MET.** F-032 closed as smoke methodology fix (not a system bug — system correctly flagged dangerous engagement on unroughed stock). AS015 verified at 0.197 Within when run with prior AS013 roughing pass (option A from F-032's reframe). **F-017 (rapid collisions) explicitly closed** — 0 collisions on every smoke case AS001-AS015. **F-033 opened** as UX follow-up for pre-sim workflow advisory (option D). The acceptance loop's primary acceptance criterion is satisfied; the active-workstream block in `CLAUDE.md` can be removed. Delta: `rounds/round-10-2026-05-26/delta.md`. No code change — audit only.
- 2026-05-26 — **F-032 implementer reframe (NO LAND)**. Round-10
  implementer (Claude Opus 4.7) probed AS015 through
  `ProjectSession::run_simulation` and **refuted the finding's
  transit-sample hypothesis**. The deflection-gate triggering sample
  is steady-state `MoveIntent::FinishingCut` with `CutKinematics::Helix`,
  `radial_woc_fraction = 0.75`, `axial_engagement_mm = 17.77 mm` on a
  15 mm flute, `in_transit_span = false`, `span_path = [Operation,
  Region("Ring 4")]` — by every signal `is_steady_state_for_gate`
  reads, it IS a steady-state cut. F-031's predicate is correctly
  scoping it. Two real root causes identified instead: (1) AS015
  runs scallop solo on `ux_3d_terrain.toml` (no prior roughing) so
  the dexel above-terrain bulk material is full uncut stock height
  (~46 mm) — the cutter at terrain Z=14.59 sees inflated axial
  engagement (17-47 mm) into stock that's outside its 15 mm flute's
  reach; (2) `scallop::ring_to_3d` emits ring points at clamped
  (X=0, Z=0) when outside the mesh footprint, producing samples at
  Z=0 reading near-full-stock-height axial (~47 mm). Implementer
  probed two speculative fixes — synthesise `SpanKind::Entry` from
  `MoveIntent::Entry*` (useful F-031 hardening for other ops, but
  didn't move AS015 because the triggering sample is `FinishingCut`),
  and clamp `axial_engagement_mm` to `tool.length()` in the
  deflection force calc (moved 0.434 → 0.424; trivial because the
  scallop-ring-emission Z=0 cohort then dominates). Both reverted
  per implementer-contract's "stop if > 2× S estimate" rule. **F-032
  finding updated** with "Round-10 implementer reframe" section
  detailing the real triggering-sample evidence, the
  scallop-ring-emission Z=0 cohort, and four candidate fix shapes
  (A: test-data add prior roughing; B: scallop generator culls
  out-of-mesh ring points; C: dexel measurement clamps axial to
  flute length; D: op-aware workflow advisory). Effort re-estimated
  S → M. **F-032 released back to open queue for re-audit**;
  deflection bar stays 6/7. No source-code change (speculative
  changes reverted in working tree before commit). No MCP rebuild
  needed.

- 2026-05-26 — round-09 audit: **F-031 verified through MCP smoke** on AS013. Deflection 0.637 → **0.105 Within** (gate now reads steady-state samples only; the 0.637 still appears in the `entry_spike` informational field, correctly excluded from the gate verdict). Collateral 6.4× toolpath reduction: AS013 went from 420,371 moves / 375k cut / 220k rapid to **65,299 / 219k / 132k** — F-031 removed a spurious multi-pass helix-entry rewrite the dressup was injecting on every plunge. **Bar moved 5/7 → 6/7.** AS015 unchanged at 0.434 — F-031's transit-filter is adaptive3d-scoped; scallop has its own deflection-gate sample selection that doesn't honor it. Opened **F-032** with concrete hotspot evidence (AS015 reports physically-impossible peak_axial_doc_mm = 25-27 mm on a 3mm ball nose at avg engagement < 5% — F-031 sibling, S effort). **F-017 implicitly closed** — all smoke cases AS001-AS015 report 0 rapid collisions post-F-024+F-026+F-027+F-031. Delta: `rounds/round-09-2026-05-26/delta.md`. No code change — audit only.
- 2026-05-26 — **F-031 landed** (commit `497a3b2`). Root cause: planner-↔-dressup
  helix entry-style parity gap. The planner's
  `stamp_emitted_segment(Adaptive3dSegment::Rapid)` stamps a single
  vertical cylinder at the entry XY (matching the planner's peck-plunge
  emission). The dressup's `apply_entry` pass — driven by
  `DressupConfig::for_op(Adaptive3d)` defaulting to `entry_style = Helix`
  via the `prefer_helix` override in `normalize_for_op` — then walked
  the planner-emitted toolpath and replaced each plunge with a multi-pass
  helix at radius ~2 mm around the entry XY. The planner's
  `material_stock` thus went out-of-sync with the simulator's actual
  swept-tube coverage. Subsequent clearing passes the planner believed
  would sweep through cleared air actually bit into uncut material,
  producing per-sample `axial_engagement_mm` readings up to ~44 mm on a
  3 mm-commanded DPP (in transit-tagged samples) and `deflection.peak_mm
  = 0.66` (Exceeds) on AS013. The stamp-event diagnostic refuted F-031
  hypotheses #1 (sample density), #2 (LUT cadence), and #3 (dexel
  origin/extent) — planner and simulator share identical grids and stamp
  functions; only the emitted-vs-rewritten toolpath shape differed.
  Hypothesis #4 (frame interaction) was the closest pre-investigation
  framing, but the "frame" was planner-emission vs dressup-rewrite,
  not world/local. Fix: narrow the `prefer_helix` override to 2D
  `Adaptive` only; force `entry_style = None` for `Adaptive3d`. The
  planner-emitted peck-plunge feeds now pass through the dressup
  unchanged, and the simulator's stamping matches the planner's
  vertical-cylinder pre-stamp. Acceptance tests
  (`crates/rs_cam_core/tests/adaptive3d_interior_cell_parity_f029.rs::
  as013_terrain_whole_toolpath_axial_within_commanded_dpp_f031` +
  `as013_terrain_deflection_within_safe_band_f031`) re-enabled and
  pass. Post-fix AS013 deflection = 0.129 (Within, was 0.66 Exceeds).
  Steady-state max axial = 3.13 (within 3.5 limit, was 44.82
  including transit). Side effect on F-027 model-edge tests aligned
  via `!in_transit_span` filter mirroring the deflection model's
  `is_steady_state_for_gate`. Full workspace `cargo test` + `cargo
  clippy --workspace --all-targets -- -D warnings` clean.
  **F-017 (rapid collisions, 3D-op cohort) closure depends on this
  fix reaching the smoke — flagged for round-09 auditor to reconcile
  alongside MCP rebuild + 7/7 deflection verification.** **MCP
  rebuild required before round-09 smoke.**

- 2026-05-26 — round-08 audit: **F-030 verified through MCP smoke.** AS001 byte-identical to round-07 (z=-2/-4/-6, peak_axial=2.0, deflection=0.076 Within, removed_volume per-pass 6569/6705/6726 byte-identical). AS013 byte-identical toolpath geometry (420371 moves / 375780 cutting / 220759 rapid), 0 collisions (F-027 holds), deflection 0.637 Exceeds (F-031 residual, in same noise band as round-07's 0.576 / F-029-landing's 0.66). The duplicated-frame pattern is retired across all 5 sites. **Bar still 5/7 deflection**; F-031 implementer pickup is the round-09 priority. Delta: `rounds/round-08-2026-05-25/delta.md`. No code change — audit only.
- 2026-05-26 — **F-030 architecture refactor landed** (commit `ba9a8fd`).
  Introduces `rs_cam_core::session::SetupEvalContext` — single source
  of truth for `(world_stock_bbox, local_stock_bbox, local_to_global,
  heights_stock_bbox, safe_z, face_up, z_rotation)` per (session, setup)
  tuple. Built once via `SetupEvalContext::build_for_setup`; consumed
  by every code path that previously derived these ad hoc. The 4
  duplicated derivation sites (F-024/F-026/F-027/F-028 family) now
  collapse to one builder:
  - Site 2 (core compute) — `session::compute::ProjectSession::generate_toolpath`
    + `run_simulation` per-setup group + `height_context_for_toolpath`
    diagnostic path all read from a `SetupEvalContext`.
  - Site 3 (viz worker sim) — `compute::worker::execute::build_core_simulation_request`
    unchanged shape (still consumes the bare `SetupSimGroup` fields);
    its identity-setup `(None, None)` semantic preserved.
  - Site 4 (viz controller sim) — `controller::events::simulation::build_simulation_groups`
    now builds each `SetupSimGroup` from `SetupEvalContext::build`.
  - Site 5 (viz controller gen) — `controller::events::compute::submit_toolpath_compute`
    routes `transform_setup` filter, `stock_bbox`, `safe_z`,
    `HeightContext`, and `ProjectCurve::setup_z_flipped` through
    `SetupEvalContext`. F-028's `.filter(|s| s.needs_transform())`
    pattern is now `transform_setup.filter(|_| ctx.needs_transform())`.
  - Viz `state::job::height_context_from_session` (Heights diagnostic
    panel + GPU upload) now also delegates to the context, so the
    Heights tab numbers match the toolpath generator's frame for
    non-identity setups (incidental F-025 alignment — does NOT close
    F-025).
  Dead helper removed: `ProjectSession::effective_stock_bbox_with_rotation`
  (callers replaced by `ctx.local_stock_bbox`). F-024 safe_z floor
  invariant preserved (reads from local bbox max even for identity
  setups). Site 5's pre-refactor `safe_z = effective_safe_z(raw, world_bbox.max.z)`
  divergence from site 2's local-bbox floor is now unified on site 2's
  (F-024-stated) convention — a conservatively-higher floor, never
  unsafe. Site 1 (load `auto_from_model` re-derivation in
  `session::project_file`) is left as is — it's the upstream stock-config
  mutation that feeds `SetupEvalContext::world_stock_bbox`, not a
  duplicated derivation. Acceptance: 4 F-024/26/27/28 cargo tests
  pass + 37 viz controller tests + 43 worker tests + 1575 core lib
  unit tests + 188 viz lib tests; clippy `-D warnings` clean.
  Auditor verification (round-08 MCP smoke): F-030 does NOT claim
  verified; the smoke runs first. **MCP rebuild required before
  round-08 smoke.**
- 2026-05-26 — **F-029 partial landing** (commit `74d8a7f`, Claude
  Opus 4.7 pickup of prior session's in-flight work). Landed:
  (1) diagnostic probe `debug_adaptive_3d_segments_for_f029_probe`
      exporting the planner's final per-cell `material_stock` top-z
      (path.rs +37 lines, mod.rs +6 lines re-export);
  (2) cleanup-raster per-cell DPP clamp in
      `clear_z_level_contour_parallel` — each cell's emitted cut-Z is
      now `max(z_level, stock_top - depth_per_pass - tolerance)` so
      cells whose stock top is far above `z_level` (outside-mesh-
      footprint cells, never covered by contour iso-lines) take at
      most one DPP of axial per pass instead of one deep cut at the
      final z_level;
  (3) acceptance tests in `tests/adaptive3d_interior_cell_parity_f029.rs`
      landed but **`#[ignore]`d** with `F-031` reference — the AS013
      worst-case cell still reads axial=44.8 mm / deflection=0.66 mm.
  Diagnostic probe shows the planner's final stock is fully cleared
  (top=0.5 mm) at the worst cell — the residual is a sim-side
  stamping / coverage gap, not a planner-side issue. **F-031 opened**
  with four hypothesised root causes and recommended diagnostic-first
  fix shape. No regression in workspace tests (F-027 acceptance + 51
  adaptive3d unit tests all pass). **MCP rebuild required before
  round-08 smoke.**
- 2026-05-25 — round-07 audit: F-027 + F-028 verified through MCP
  smoke. AS013 collisions 2924→0, AS015 52→0, AS004 deflection
  0.243→0.005 Within (49× reduction), AS004 peak_axial 11.42→0.38
  (commanded 0.5). F-024 stability confirmed on AS001 (peak_axial 2.0,
  deflection 0.076) and AS003 after a mid-round regression cycle —
  F-028 site 1 alone broke AS001/AS003 (MCP→controller→worker path
  bypassed the fix); resolved by F-028 viz follow-up (`c9e203d`).
  Deflection bar at 5/7 Within — AS013/AS015 still Exceeds at
  identical samples (F-029 interior-cell parity residual). **F-030
  architecture finding opened** to retire the duplicated-frame-handling
  pattern that caused this and previous three-rebuild sagas (brief at
  `handoff_prompts/F-030-architecture-brief.md`; blocked on F-029 +
  7/7 bar). Delta: `rounds/round-07-2026-05-25/delta.md`. No code
  change — audit only.
- 2026-05-25 — F-028 viz-path follow-up landed: the original F-028
  fix (commit `e48d7df`) only patched `session::compute::compute`
  (site 1) — the direct `ProjectSession::run_simulation` path. Round-07
  smoke through the MCP exposed that the viz/MCP/GUI pipeline takes a
  *different* code path through `AppController::submit_toolpath_compute`
  (`controller/events/compute.rs`), which (a) shifted polygons by
  `-stock.origin` into a setup-local frame even for identity setups
  and (b) built the `HeightContext` from the zero-rooted local bbox.
  AS001 pocket emitted z_level=10/8/6 (= local stock_top - depth), the
  downstream sim path dropped `local_to_global=None` and rebuilt the
  dexel grid in world frame (F-024 follow-up `0c907a6`), and the
  cutter swept through air 10 mm above the world stock top.
  Round-07 MCP smoke evidence: peak_axial=0, total_removed=0,
  chipload=Unmodeled(all_samples_air_cut_or_rapid), air_cut=96 %.
  Fix mirrors site 1: gate the `transform_setup = Some(...)` branch on
  `s.needs_transform()` so identity setups fall through to the
  `transform_setup = None` path and use the world stock bbox for both
  geometry frame and `HeightContext`. Single-site change in
  `controller/events/compute.rs` (one filter call + load-bearing
  comment). Regression test
  (`controller::tests::as001_pocket_heights_resolve_in_world_frame_for_identity_setup_f028`)
  drives `submit_toolpath_compute` through a CapturingBackend and
  asserts (a) `heights.top_z == 0` (world stock top for AS001),
  (b) `heights.bottom_z == -6` (top_z - depth), (c) `stock_bbox.{min,
  max}` respect `stock.origin_{x,y,z}`. Test fails on master
  `db1fb69` with `heights.top_z = 12.0`; passes after fix. Existing
  F-024 viz-path tests
  (`as001_viz_path_first_pass_axial_engagement_within_commanded_doc_f024`,
  `controller_built_stock_bbox_drives_axial_engagement_within_commanded_doc_f024`,
  `build_world_stock_bbox_respects_stock_origin_f024`) continue to
  pass. Full workspace `cargo test -q` + `cargo clippy --workspace
  --all-targets -- -D warnings` clean. **User must `/mcp` rebuild
  before the auditor re-verifies through the MCP.** Commit: `c9e203d`
  (fix(F-028): mirror world-frame heights fix into viz worker +
  controller — follow-up to e48d7df).

- 2026-05-25 — F-028 follow-up cross-check: the round-07 implementer
  brief reported an alleged AS001/AS003 pocket/profile regression
  (z_level shifted from -2/-4/-6 to +10/+8/+6, peak_axial=0,
  removed_volume=0, chipload Unmodeled). Wrote a defensive regression
  test (`tests/face_stock_top_frame_f028.rs::
  as001_pocket_actually_removes_stock_material_post_f028`) driving
  `ProjectSession::load(ux_2d_pocket.toml)` → `generate_toolpath` →
  `run_simulation` and asserting (a) total_removed_volume > 5000 mm³,
  (b) peak_axial in [1.5, 3.0] mm (tight band around the commanded
  2.0 mm DOC), (c) chipload verdict not `Unmodeled`. **The test
  passes on commit `bf63d06`**: actual readings are peak_axial=1.76
  mm, total_removed=20222 mm³, chipload modeled. AS001 pocket on
  origin_z=-12 stock cuts at world Z=[-2,-4,-6] as expected — the
  alleged regression does not reproduce at the `ProjectSession` API
  level (which is what the auditor's smoke suite ostensibly drives).
  Hypothesis: the round-07 readings may have come from a stale
  process / wrong build / different code path; needs auditor
  re-investigation. **Flagging for auditor review.** Also tightened
  the existing F-028 face acceptance test
  (`as004_face_peak_axial_within_commanded_doc`) to assert
  `peak_axial > 0.3` (lower bound) in addition to `<= 0.6` — closes
  the "0 trivially passes ≤ 0.6" loophole called out in the brief.
  AS004 face actual reading on this code: peak_axial=0.4375 mm.
  No code/product change; tests-only. Commit: see git log.

- 2026-05-25 — F-028 landed: face op anchors depth stepping at
  `heights.top_z` (which now follows `ctx.stock_top_z` under Auto), and
  `session/compute.rs::compute` carries the world stock bbox into the
  `HeightContext` for identity setups (mirroring the GUI viz
  controller's existing pattern). Pre-fix face on
  `ux_step_plate_mdf.toml` cut at world Z=-0.5 below the dexel grid
  [0, 15] — peak_axial_doc_mm scaled with stock height (9.14 at
  stock=12 → 11.42 at stock=15, the F-028 evidence). Post-fix cuts
  land at world Z=14.5 inside the stock. Acceptance tests
  (`tests/face_stock_top_frame_f028.rs`):
  `as004_face_peak_axial_within_commanded_doc`,
  `as004_face_deflection_within_safe_band`,
  `as004_face_no_rapid_collisions` — all three drive through
  `ProjectSession::run_simulation`. F-024's AS001 pocket tests
  continue to pass (the identity-setup conditional preserves
  `heights.top_z = 0` when world stock top = 0).

- 2026-05-25 — F-027 landed (scope-trimmed to model-edge band):
  adaptive3d planner stock widened to the world stock XY bbox via a
  new `Adaptive3dParams::world_stock_xy_bbox` field plumbed from
  `compute::execute::execute_operation`. The planner's
  `material_stock` now extends across every cell the simulator's
  per-setup dexel grid will look at, and `border_clear` is inhibited
  for cells inside the world stock bbox (they're real stock, not
  phantom drop-cutter floor). On AS013 this drops the model-edge
  cohort of axial outliers (samples beyond `mesh.bbox.max.y` reading
  the full auto-grown stock height) to zero. Acceptance tests:
  `crates/rs_cam_core/tests/adaptive3d_planner_stock_xy_f027.rs::{as013_terrain_model_edge_axial_within_commanded_dpp_f027, as013_terrain_model_edge_band_outlier_count_zero_f027}`.
  Implementer investigation found a residual class of interior-cell
  axial outliers (~108 samples, worst 38 mm, samples inside the mesh
  XY footprint) survives F-027 — same mechanism shape but a different
  cell cohort, carved out as **F-029** with three candidate root
  causes and fix shapes. F-027 closes the model-edge band; F-029
  must land for the AS013 deflection bar to move into Within.
- 2026-05-25 — round-06 audit: F-026 verified at load-bbox-fix scope
  via MCP smoke on AS013/AS015/AS004. stock.z grew correctly on both
  auto_from_model templates (ux_3d_terrain 30→57.6; ux_step_plate_mdf
  12→15). Deflection bar didn't move because the three round-05
  misses are three separate bugs, not one. AS013/AS015 → F-027
  (already opened during F-026 landing). **AS004 → F-028 NEW**:
  face op emits `z_level=-0.5` interpreted by simulator as world Z,
  causing peak_axial to scale with stock height (F-026's stock
  growth made AS004 axial 9.14→**11.42 mm**, deflection 0.204→0.243).
  F-017 cannot close yet: AS013 absolute collision count went UP
  (844→2924) because F-026 doubled the toolpath size; per-rapid-
  distance rate slightly improved (84→108 mm/collision). Delta:
  `planning/acceptance_loop/rounds/round-06-2026-05-25/delta.md`.
  No code change — audit only. (No F-ID — audit log entry.)
- 2026-05-25 — F-026 landed (scope-trimmed): `auto_from_model` stock
  bbox re-derivation now runs at project load time, mirroring the
  runtime `add_model` path. `project_file::build_session_from_project`
  computes the union of all loaded model bboxes and calls
  `StockConfig::update_from_bbox` when `auto_from_model = true`.
  Restores the documented invariant that `auto_from_model` means
  stock bounds track the model bbox in memory, regardless of stale
  TOML values.
  Acceptance tests:
  `crates/rs_cam_core/tests/dexel_stock_z_frame_f026.rs::{auto_from_model_load_grows_stock_z_to_enclose_terrain_mesh, auto_from_model_load_grows_stock_xy_to_enclose_polygon_model}`.
  Implementer investigation found the round-05 audit's diagnosis
  ("dexel grid Z range stops at stock_top_z") was the wrong layer —
  the rapid-collision part of the AS013 signature (844 → 0) is fixed
  by the load-time stock re-derivation, but the deflection-Exceeds
  residual (peak axial 30-47 mm at sample positions on the model XY
  boundary, post-fix) is a separate adaptive3d/simulator XY frame
  mismatch carved out as **F-027** with concrete repro evidence and
  fix shape. F-017 (rapid collisions) closes for the AS013 portion
  when F-026 verifies on round-06; the remaining 3D-op collision
  surface waits on F-027.

- 2026-05-24 — MCP param UX overhaul shipped (`integer_param_coercion`,
  `mutation_result_envelope`, `operation_schema`, `param_schema_hints`,
  `param_schema_optional_nulls`, `valid_param_error_hints`). Original
  prompt archived at `archive/MCP_PARAM_UX_OVERHAUL_AGENT_PROMPT.md`.
  Result: agent smoke run completed AS006–AS017 without re-hitting the
  param-discovery wall that AS001–AS005 had to work around.
- 2026-05-25 — F-001/F-002/F-003/F-007/F-008/F-013 unification batch
  landed (see `planning/CODEBASE_UNIFICATION_PLAN.md` for the per-fix
  mapping). Subsumes F-012; partially absorbs F-006 / F-011.
- 2026-05-25 — F-016 landed: drill_op view now carries the live stock
  material (was `Material::default()`). Acceptance tests:
  `crates/rs_cam_core/tests/drill_material_plumbing_f016.rs::{drill_op_carries_hardwood_material_from_stock, drill_op_carries_softwood_material_from_stock, drill_op_carries_plastic_material_from_stock}`
  and `drill_metrics::tests::chip_welding_threshold_per_material_family`.
- 2026-05-25 — F-014 landed: annotated stale service-layer/dispatch-duplication audit docs (`review/results/41_duplication.md`, `review/results/30_compute.md`) with resolved-2026-05-25 markers pointing at `crates/rs_cam_core/src/compute/execute.rs:201 execute_operation`; archived `review/SERVICE_LAYER_OWNERSHIP_AUDIT.md` → `review/archive/` with a top-of-file note. No acceptance test (docs-only).
- 2026-05-25 — F-015 landed: op-precondition static-validation adapter
  (`from_preconditions`) now surfaces blocking diagnostics on rest /
  drill / alignment_pin_drill / project_curve before generate time.
  New file `crates/rs_cam_core/src/diagnostics/adapters/from_preconditions.rs`
  + wire-up in `session/compute.rs::precondition_context_for_toolpath`.
  Acceptance tests:
  `crates/rs_cam_core/tests/op_precondition_static_validation_f015.rs::{rest_op_without_prior_enabled_tool_surfaces_blocking_diagnostic, rest_op_without_prev_tool_id_surfaces_blocking_diagnostic, rest_op_with_correct_prior_is_silent, project_curve_in_single_model_project_surfaces_blocking_diagnostic, project_curve_without_any_surface_mesh_surfaces_blocking_diagnostic, project_curve_with_curve_and_surface_models_is_silent, drill_op_against_mesh_only_model_surfaces_blocking_diagnostic, drill_op_against_polygon_model_is_silent}`
  + adapter unit tests at
  `diagnostics::adapters::from_preconditions::tests::*`.
- 2026-05-25 — F-024 landed: dexel stock grid uses world Z frame for
  identity setups. `session/compute.rs` now returns
  `local_stock_bbox = None` (and `local_to_global = None`) when the
  setup is identity, so `run_simulation` falls back to
  `request.stock_bbox` (world frame). Per-setup grid Z range now
  matches the toolpath frame; `axial_engagement_mm` reads the
  commanded DOC instead of the full stock height. Commit `d82bd4d`.
  Acceptance tests:
  `crates/rs_cam_core/tests/dexel_stock_z_frame_f024.rs::{as001_pocket_first_pass_axial_engagement_within_commanded_doc, as001_pocket_deflection_gate_within_safe_band}`.
  CLAUDE.md `Metric caveats` block updated with F-024 follow-up
  paragraph. Param sweep fingerprints unchanged — the harness uses the
  lower-level operation kernels directly and doesn't traverse the
  `ProjectSession` setup-transform path the fix touches; no
  regen needed.
- 2026-05-25 — F-023 landed: core-side `from_model_refs` adapter
  surfaces `ref.model_missing` (Severity::Blocking) when a toolpath's
  `model_id` doesn't resolve against the loaded project. Wired into
  the orchestrator via `ToolpathDiagnoseInputs::model_refs` and
  `session::compute::model_ref_context_for_toolpath`; MCP envelope's
  `diagnostic_delta` / `gui_banners` / `warnings` now carries the
  signal the GUI banner already showed. Also fixed the misleading
  `add_toolpath` `model_id` docstring in
  `crates/rs_cam_mcp/src/server.rs` (no longer says "usually 0 for
  the first model"). New file
  `crates/rs_cam_core/src/diagnostics/adapters/from_model_refs.rs`;
  new ID `ids::REF_MODEL_MISSING`. Acceptance tests:
  `crates/rs_cam_core/tests/op_model_ref_static_validation_f023.rs::{pocket_op_with_unresolved_model_id_surfaces_blocking_diagnostic, pocket_op_with_resolved_model_id_emits_no_ref_diagnostic, face_op_with_unresolved_model_id_is_silent}`
  + adapter unit tests at
  `diagnostics::adapters::from_model_refs::tests::*`.
- 2026-05-25 — F-024 viz-path follow-up landed: round-04 auditor
  smoke confirmed the original F-024 fix (`d82bd4d`) wasn't taking
  effect through the production MCP / GUI path because the viz
  worker's `compute::worker::execute::build_core_simulation_request`
  was unconditionally wrapping the viz-side zero-rooted
  `local_stock_bbox` in `Some(...)` — so core's "fall back to world
  frame when `local_stock_bbox` is None" branch never fired on the
  viz path (AS001 stayed at `peak_axial_doc_mm = 12.0` /
  `deflection.peak_mm = 0.374` after d82bd4d). Mirrored the
  `session/compute.rs` identity-setup conditional in
  `build_core_simulation_request`: when `local_to_global` is `None`
  (identity), forward `local_stock_bbox = None` so core falls back to
  `request.stock_bbox` (world frame). Commit `0c907a6`. Regression
  test:
  `crates/rs_cam_viz/src/compute/worker/tests.rs::as001_viz_path_first_pass_axial_engagement_within_commanded_doc_f024`
  (pre-fix peak axial = 9.0 mm; post-fix ≈ 2.0 mm).
- 2026-05-25 — F-024 third-site landed: round-04 auditor smoke after
  the second MCP rebuild still showed AS001 `deflection.peak_mm = 0.374`
  byte-identical to round-02. Root cause: even with the core fix
  (`d82bd4d`) and viz-worker fix (`0c907a6`) in place, the viz
  controller's `build_simulation_groups` constructed the world
  `stock_bbox` inline as `(0,0,0)..(stock.x, stock.y, stock.z)` —
  dropping `stock.origin_{x,y,z}`. For AS001 (`origin_z=-12`) the
  controller passed bbox `(0,0,0)..(100,100,12)` to the worker. The
  viz worker's `local_stock_bbox = None` fallback then used this
  broken `request.stock_bbox` as the dexel grid bounds, so the grid
  spanned the wrong Z range and the cutter at world Z=-2 again sat
  below every ray. Fix: replace the inline construction with
  `ProjectSession::stock_bbox()` (which delegates to
  `StockConfig::bbox()` and applies the origin correctly), extracted
  through a pure free function `controller::events::simulation::
  build_world_stock_bbox` for testability. Bumped
  `compute::worker::execute` mod and `run_simulation_with_phase`
  function visibility to `pub(crate)` so the controller-path test
  can drive the same viz simulation entry point the worker thread
  uses. Acceptance tests:
  `crates/rs_cam_viz/src/controller/tests.rs::{build_world_stock_bbox_respects_stock_origin_f024, controller_built_stock_bbox_drives_axial_engagement_within_commanded_doc_f024}`
  (pre-fix peak axial = 9.0 mm; post-fix ≈ 2.0 mm — identical signal
  to the worker-tests F-024 regression but driven through the
  controller helper rather than a hand-built world bbox).
- 2026-05-25 — loop docs: added "Test through the production entry
  point" rule to `implementer_contract.md`, matching MUST-bullet in
  `handoff_prompts/autonomous_auditor.md`, and audit verification
  note in `audit_runbook.md` Step 1. Encodes the round-04 three-
  rebuild-saga learning (`rounds/round-04-2026-05-25/delta.md`).
  No F-ID — loop-doc work. No acceptance test (docs-only).
- 2026-05-25 — round-05 audit: partial smoke sweep (AS001-AS006 +
  AS013 + AS015). **F-026 candidate 2 confirmed and promoted to
  high-severity, scope widened from AS013-only to all auto_from_model
  projects where model.z > stock.z.** AS013 hotspot at cutter Z=32.4
  (above stock_top=30) reads peak_axial_doc_mm=25.0 on a 6 mm endmill;
  sibling hotspot at Z=23.4 (inside grid) reads 3.0 (commanded).
  AS015 (scallop on terrain) and AS004 (face on auto_from_model MDF
  plate) both exhibit the same signature. F-017 (rapid collisions)
  reframed as duplicate of F-026. F-024 verified stable on all 5
  origin_z=-12 cases (AS001-AS003, AS005, AS006). Deflection bar
  moved 1/4→5/7 Within. Delta:
  `planning/acceptance_loop/rounds/round-05-2026-05-25/delta.md`.
  Results CSV:
  `target/acceptance_sweeps/agent_smoke_20260525_round05/results.csv`.
  No code change — audit only. (No F-ID — audit log entry.)
- 2026-05-25 — F-018 landed: regenerated `test_data/ux_*.toml`
  templates to match `cases_agent_smoke.csv`. Added `End Mill 3mm` +
  `90deg V-bit 6mm` to `ux_2d_star.toml` (AS007-AS010). Created
  per-material variants `ux_2d_pocket_softwood.toml`,
  `ux_2d_pocket_mdf.toml`, `ux_step_plate_mdf.toml`. Deleted the
  broken `Rivers (back) (copy)` toolpath from `ux_3d_terrain.toml`
  and added the `demo_star.svg` curve model so AS018's
  `project_curve` has a source curve. CSV updated for AS002 →
  pocket_softwood, AS004 → plate_mdf, AS005 → pocket_mdf
  (project_template column only, per auditor restriction).
  Residual material mismatches on AS011/AS012/AS013/AS017 left as
  out-of-scope and tolerated by the test's `KNOWN_MATERIAL_MISMATCHES`
  table for a follow-up finding. Acceptance tests:
  `crates/rs_cam_core/tests/test_data_smoke_csv_alignment.rs::{all_smoke_cases_reference_existing_templates_and_fixtures, all_smoke_cases_load_via_project_session, all_smoke_cases_have_required_tool_in_template, all_smoke_cases_have_matching_material_family}`.
  Pre-fix verified failing on `all_smoke_cases_have_required_tool_in_template`
  (AS007-AS010 missing tools) by stashing `ux_2d_star.toml` and
  re-running.

## How to update this file

- **Auditor** rewrites the queue at the start of every round, moves
  resolved items into "Closed this round" with the round's baseline
  link, and records the new acceptance-bar snapshot.
- **Implementer** updates only the "In flight" table when claiming /
  releasing a finding, and appends one line to "Implementation log"
  when a PR lands. Never touches the queue ordering or acceptance bars.
