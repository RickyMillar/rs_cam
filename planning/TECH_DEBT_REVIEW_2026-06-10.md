# Tech-debt review brief — observations from the defect-class cleanup

**Source:** noticed while implementing F2–F4 (`planning/DEFECT_CLASS_CLEANUP_2026-06-10.md`,
branch `defect-class/cleanup`, 2026-06-10). These are *observations with evidence*,
not yet investigations — each item names where it was seen so a review session can
start from concrete code.

Priority order = expected source of the next live-run incident.

## R1 — Optimizer candidate-evaluation isolation + generator robustness at extremes — **DONE 2026-06-10**

Landed as a three-layer fix (deep-dive session, same day):

- **Candidate isolation (decided: guard + catch_unwind, not clone).**
  `evaluate_candidate` wraps its inner apply→regen→sim→gate in
  `catch_unwind`; a panic surfaces as `SessionError::OperationFailed` and
  costs one candidate, not the optimize run. Session-clone was rejected —
  `ToolpathConfig` is deliberately not `Clone` (see `optimize/mod.rs` walk
  comment). Unwind safety verified: `generate_toolpath` / `run_simulation`
  publish into session state only in their Ok arms, and the
  `BaselineRestoreGuard` restores params on every exit. `refine_stage2`
  now drops failed candidates (matching the grid/retarget loops) instead
  of `?`-aborting the whole refinement; only cancellation propagates.
- **Chokepoint containment.** All cavalier_contours calls go through
  `polygon::offset_polygon`; it now catch_unwinds the offset and maps a
  panic to "collapsed offset" (empty — every caller's under-cut-safe
  path) with a `tracing::warn`. The captured WANAKA Back Rough slice
  (86-vert exterior, 13 holes, inward 5.53 mm —
  `test_data/cavalier_panic_polygon_r1.json`) still asserts inside
  cavalier 0.7.0 (latest) even with clean input, so containment is the
  only fix for that class.
- **Root fix for the second class.** The new generator-extremes fuzz
  found a *different* cavalier assert at pocket@0.05 mm floors ("repeat
  position vertexes" — our input-contract violation via chained
  `pocket_offsets`). Fixed at the root: `remove_repeat_pos(1e-5)` dedupe
  before every cavalier offset call.
- **Tests:** `offset_polygon_degenerate_inputs_r1.rs` (captured asset +
  synthetic repeat-vertex, 0.02 s), `generator_extremes_fuzz_r1.rs`
  (2D matrix at floors/ceilings in CI; adaptive3d-at-floors `#[ignore]`d
  — ~40 min, run manually when touching clearing/offsetting),
  panic-payload seam tests in `optimize/candidate.rs`.
- **Release panic strategy: DECIDED 2026-06-10 — `panic = "unwind"`.**
  Both cavalier asserts are `debug_assert!` (dev/test crash, release
  silently proceeds; the dedupe fixes the known silent case), but
  `profile.release` previously set `panic = "abort"`, which (a) made
  every catch_unwind isolation boundary dead code in the shipped build
  and (b) meant any panic in any compute-worker thread aborted the whole
  GUI with the user's unsaved project. Removed the abort override —
  robustness beats the marginal codegen/binary-size cost for a desktop
  CAM app. Caveat recorded in Cargo.toml: F-034/F-036c cycle-time
  anchors were calibrated under abort; re-pin if they drift.
- Still open (unchanged): `mem::replace` placeholder session (MCP
  observability) backlog item.

## R2 — Error semantics that lie — **INVESTIGATED 2026-06-10: premise wrong, no fix needed**

- Verdict: every `map_err(|_…| …Cancelled)` in the workspace (10 sites: 4 in
  `compute/execute.rs`, 2 each in `compute/simulate.rs` / `collision_check.rs`,
  2 in viz worker) wraps an error type that can ONLY mean cancellation —
  `interrupt::Cancelled` is a unit struct, `CollisionCheckError` has exactly
  one variant. Nothing is being swallowed; the mapping is honest.
- The real lesson: the geometry stack has **no error channel at all**. Its
  failure mode is panics (cavalier_contours asserts), which is R1's territory —
  the two items merged. If a generator ever grows a real error type, the
  `|_e|`-shaped closures here are where a swallow would silently appear;
  `clippy::map_err_ignore` only catches `|_|`, not `|_e|`.
- `OperationError::Other(String)` stringly-typing and `RefuseReason`
  conflation remain backlog-tracked in the DEFECT_CLASS doc.

## R3 — Toolpath id vs index confusion

- APIs mix `usize` index-into-`toolpath_configs` and `tc.id` freely:
  `generate_toolpath(index)` vs `SimulationCutSample.toolpath_id` /
  `tool_load_report().per_toolpath[].toolpath_id` (ids). Bit me during the F2
  WANAKA probe (Back Rough = index 1, id 4) — silent wrong-lookup class.
- Tools have a `ToolId` newtype; toolpaths don't. Review shape: introduce
  `ToolpathId` + audit every `toolpath_id == idx`-shaped comparison.

## R4 — Golden-number test pins vs spec assertions — **DECIDED 2026-06-10**

- Convention (canonical copy lives in the `wanaka_suggest_integration.rs`
  header, where authors will see it):
  1. Literature-backed values → pin exact band + cite source (litmatrix style).
  2. Behavioral outcomes (the default) → assert identity and direction, not
     magnitude: which warning fires, which cap binds, monotonic relations,
     relative bands against model outputs, named constants instead of
     literals.
  3. Determinism sentries (rare) → raw numeric pin allowed only with a
     comment naming what legitimately re-baselines it.
- Applied: the two `6000.0` cutting-ceiling pins now assert against
  `machine::DEFAULT_CUTTING_FEED_CAP_MM_MIN`; the 3.69 mm DPP pin is labeled
  a determinism sentry. The rest of the file already followed rule 2 after
  the F3/F4 repins (cap_hit identity, warning identity, relative bands).

## R5 — Sim-type test-fixture sprawl

- Adding one field to `ChiploadVerdict::Within` touched 18 construction sites
  (F3.3); `SimulationCutSample`/`SimulationCutTrace` ~25-field fixtures are
  hand-copied in ≥5 test modules, many stamped `schema_version: 1` while the
  real schema is v5 (F1).
- Review shape: test-builder helpers (`SimulationCutSample::test_default()`-style
  or a builder in a shared test-support module) + decide whether schema_version
  in fixtures should track the real version.

## R6 — Name-string identity

- `MachineProfile::to_key()` dispatches on `name.contains("VFD")`/"Makita";
  `LookupResult.source_vendor` is `format!("{:?}")`. Cheap fixes; silent
  breakage on rename.

## Already tracked elsewhere (don't double-count)

- Four deflection-model implementations / 0.7·D closed form +36% bias (A4 +
  DEFECT_CLASS backlog — LUT side unified by F3.4, model side not).
- Magic numbers outside `SearchPolicy`; Face op `feeds_family: Pocket` stranding
  the 9 facing-bit LUT rows; 48 single-point LUT rows behind the shrink-only
  allowlist (per-source disposition pending); A2 minor findings (z_step hint
  asymmetry, plunge_rate provenance, CLI raw defaults).
