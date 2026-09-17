# T-4 — implementation report

*Written 2026-09-18. The design is `T4_REFUSAL.md` §4; this file records what
shipped, what the design did not anticipate, and the evidence.*

---

## 1. The exits and their variants

`predict_peak_deflection_um` had nine refusal exits and every one returned
`predicted_um = 0.0`. It now returns
`Result<DeflectionPrediction, DeflectionUnmodeled>`. **Eleven exits refuse,
onto nine variants.** The V-bit exit is gone; three exits are new or newly
stated.

| # | Exit | Where | Variant | Change |
|---|---|---|---|---|
| 1 | Drill family | early guard | `NotApplicableForOp(op_type)` | typed |
| — | V-bit cutter | early guard | — | **DELETED**, see §2 |
| 2 | No `kc_n_per_mm2()` | early guard | `MaterialUnvalidated` | typed |
| 3 | `Material::Custom` | early guard | `MaterialCustom` | **NEW SITE** — was hidden in the delegate |
| 4 | Non-positive or NaN diameter | early guard | `NoDiameter` | typed |
| 5 | `depth_per_pass()` is `None` | early guard | `NoDepthPerPass` | typed |
| 6 | DPP not positive and finite | early guard | `NoDepthPerPass` | typed |
| 7 | Radial WOC not positive | early guard | `NoRadialEngagement` | typed |
| 8 | Zero chipload | early guard | `NoChipload` | typed |
| 9 | Stickout not positive and finite | early guard | `NoStickout` | **NEW SITE** — was hidden in the delegate |
| 10 | Engagement past twice the stickout | early guard | `DegenerateCantilever` | **NEW** — see §6 |
| 11 | Delegate returns a non-positive or non-finite figure | after the call | `DegenerateCantilever` | backstop |

Every variant carries a `clause()` (one operator-facing sentence) and an
`as_unmodeled_reason()` that lands on `tool_load::verdict::UnmodeledReason`.
`NotApplicableForOp` and `MaterialUnvalidated` map onto the post-simulation
variants of the same name; the six input-shaped variants map onto
`NotImplemented(clause)`. The module header now states the full set.

## 2. The V-bit guard

**Both claims in the brief confirmed with `rg` before the deletion:**

- `predict.rs` calls the integrator. The delegation runs through
  `tip_deflection_from_engagement` → `ToolDefinition::tip_deflection_mm`,
  which reads `lookup_diameter_at(axial_from_tip)` per integration step.
  `VBitEndmill` implements `lookup_diameter_at`.
- `cutter_constraints.rs` already computes a V-bit deflection bound.
  `invert_deflection` (line 258) carries no V-bit arm; the module's own
  table says the deflection bound applies to "All (binary search)", and
  `vbit_uses_deflection_bound_only` asserts a positive bound.

So the guard refused before reaching a model that works. It is deleted. A
V-bit now returns `Ok` carrying
`DeflectionCaveat::FluteReliefUnmodeled`.

`bending_diameter_mm`'s `CutterKind::VBit => 0.0` arm **stays**, with a
corrected comment. It feeds `i_eff_mm4`, which is a breakdown diagnostic and
explicitly not the magnitude. The zero now reads as "no representative
section", not "the caller refuses earlier". No constant was invented for
`f_V`; that gap is what the caveat exists to state.

## 3. The callers, found with `rg`

`rg -n "predict_peak_deflection_um|\.predicted_um" crates/` found **no
caller outside `rs_cam_core`**. `rs_cam_cli`, `rs_cam_viz` and `rs_cam_mcp`
were checked with `cargo check --all-targets` after the change and compile
untouched.

| Caller | How it reads the `Result` now |
|---|---|
| `feeds/efficiency.rs` `force_headroom` | `.ok()?`, then **`return None` on any caveat**. A floor cannot show a cut inside the budget, so a caveated figure abstains exactly as a refusal does. The comment says so. |
| `feeds/suggest/invariants.rs:404` (back-off, initial call) | `match`; on `Err` it pushes `DeflectionBackoffUnmodeled` and returns. See §5. |
| `feeds/suggest/invariants.rs:416` (back-off, in-loop call) | `match`; on `Err` it steps the DPP back to the last evaluated value, pushes the warning and breaks. |
| `feeds/profile.rs:206` | **Deleted**, see §4. |
| `feeds/suggest/tests.rs` ×3, `feeds/predict.rs` tests ×7 | `.expect(...)`, or an `expect_err` asserting the exact variant. |

`force_headroom` keeps its `Option<f64>` return. The design's wider option
(a `Result` field on `CutEfficiency`) was not taken — the orchestrator's
instruction fixed the narrower one, and it keeps `rs_cam_viz/compare.rs`
out of the change.

## 4. `CutterOpProfile::predictions` — deleted

`rg -n "\.predictions\b|predictions:|predictions\(" crates/` returned **one
line**: the field declaration at `profile.rs:145`. No reader in production,
none in any test. The two consumers of `CutterOpProfile`
(`rs_cam_cli/src/project.rs` and `rs_cam_viz/src/app/mcp/generation.rs`)
read `feasibility`, `suggested_operation`, `feeds` and `warnings` only.

Deleted: the field, the `Predictions` struct and the construction, with a
retirement comment in place giving the `rg` evidence. `predict_move_count`
keeps its live caller in `feeds::suggest::invariants`. The deletion is
confirmed by `cargo check --all-targets` on all four crates.

## 5. The back-off now acts

`backoff_dpp_for_deflection` used the channel Suggest already has for a step
it did not take: a `SuggestWarning` variant, rendered through the exhaustive
`entry_for_warning` match in `rationale.rs`. No parallel channel was added.

Two variants:

- `DeflectionBackoffUnmodeled { dpp_mm, reason }` — the predictor abstained,
  so no back-off ran. `reason.clause()` is the headline and
  `reason.as_unmodeled_reason()` is in the detail, so the pre-simulation and
  post-simulation halves print the same words. New rationale reason
  `RationaleReason::DeflectionUnmodeled`; `from_value` and `to_value` are
  both `None`, because nothing was written.
- `DeflectionBackoffFigureIsAFloor { dpp_mm, predicted_um, caveat }` — the
  figure is caveated. **The back-off still runs on it**, because backing a
  DPP off a floor is conservative, and the operator also gets the clause,
  because a floor does not show the cut inside the bound. New rationale
  reason `RationaleReason::DeflectionFigureIsAFloor`. The cutter shape does
  not change inside the loop, so one report covers every iteration.

`session/compute.rs:1082` uses `_ => false` and needed no change: neither
warning is deflection-binding, because nothing bound.

## 6. What the design did not anticipate

**A modelled zero WAS reachable, through one path.**
`ToolDefinition::tip_deflection_mm` returns `0.0` when
`load_pos = stickout − axial/2 <= 0`, and `tip_deflection_from_engagement`
returns `Some(0.0)` there rather than `None`. T4_REFUSAL §2e reasoned about
this case correctly but placed it under the delegate's `None`. Left alone,
that path would have carried the old sentinel straight into the `Ok` branch.
Handled two ways: an explicit guard before the call (exit 10) and a
post-call backstop that refuses any non-positive or non-finite figure (exit
11). Both name `DegenerateCantilever`. The sentry's 27-point sweep is what
holds this.

**The stickout fallback was dead once the guard moved.** `predict.rs` used
to fall back to `cutting_length + COLLET_EXPOSURE_MARGIN_MM` when
`tool.stickout` was non-positive, but `build_cutter` copies `tool.stickout`
straight through, so the fallback reached the breakdown and never the model
— the breakdown reported a stickout the model did not use (T4_REFUSAL §2).
With exit 9 refusing on `tool.stickout` directly, the fallback became
unreachable. It and `COLLET_EXPOSURE_MARGIN_MM` are deleted. Nothing
observable changes: both the old and the new code refuse that input, and the
breakdown exists only on the `Ok` path.

**Two stale prose sites found and corrected**, neither in the brief:

- `cutter_constraints.rs:42` "Why not reuse `predict_peak_deflection_um`"
  gave two reasons. Reason 1, "it refuses V-bits (returns 0)", no longer
  holds. Reason 2 (it needs a DPP the envelope is picking) survives.
- `cutter_constraints.rs:719`, a test comment saying "only the closed-form
  `predict_peak_deflection_um` refuses".

**`wanaka_suggest_integration.rs` needed two edits and NEITHER was run.**
This test needs the Wanaka project and runs for minutes. Both edits are
derived from source:

- `assert_no_dpp_capped`'s message lost its "/ V-bit" clause, as
  T4_REFUSAL §4 directs. Its two call sites are both drill toolpaths.
- The file's deliberate no-wildcard `match` over `SuggestWarning` gained two
  arms. Both **panic**, and the reasoning is structural, not a tolerance:
  `DeflectionBackoffUnmodeled` needs the back-off loop, the loop needs
  `depth_per_pass()`, and `DrillConfig`, `AlignmentPinDrillConfig` and
  `ProjectCurveConfig` carry no such field (confirmed in
  `operation_configs.rs`; the T-12 census in that same file records the same
  three as `CutGeometryFieldNotHeld` producers). Every other Wanaka toolpath
  cuts hard maple or white oak with a carbide cutter, and both materials
  carry a measured `Kc`. `DeflectionBackoffFigureIsAFloor` needs a V-bit on
  a ROUGHING operation, and `VCarve` is `PassRole::Finish`.
  **The panic arms are an argument from source, not a measurement.** If CI
  reds on them, the fixture carries a tool or a material the model does not
  cover, which is a real finding about the fixture.

## 7. The sentry

`crates/rs_cam_core/tests/a_refused_deflection_is_not_a_zero_g_t4.rs`, six
arms:

1. `a_rigid_cut_still_produces_a_figure` — **the non-vacuity anchor**. A
   6 mm four-flute carbide end mill at 45 mm stickout, hard maple, a
   roughing pocket: `Ok`, strictly positive, finite, `caveat: None`, and the
   breakdown reports the stickout the model used.
2. `no_modelled_operating_point_yields_a_zero` — **the second anchor**. A
   27-point grid over stickout × diameter × DPP; no `Ok` may carry the old
   sentinel. The arm asserts it evaluated 27 points, so a grid that ran
   nothing cannot pass.
3. `every_refusal_names_itself` — the drill case, the Acrylic case and the
   `Material::Custom`-with-`kc = 30.0` case. Each of the last two asserts
   its own premise first: Acrylic's `kc_n_per_mm2()` is `None`, and the
   custom material's is `Some(30.0)` — without that second assertion the arm
   would prove nothing about the hidden exit.
4. `a_v_bit_is_modelled_and_says_what_is_missing` — `Ok`, positive, finite,
   `Some(FluteReliefUnmodeled)`, non-empty clause.
5. `the_two_vocabularies_agree` — every variant maps to an
   `UnmodeledReason` of the right kind, every `clause()` is non-empty, and
   the clauses are pairwise unique. The fixture list is guarded two ways: an
   exhaustive `match` with no wildcard (a tenth variant fails to compile)
   and a length assertion (a tenth variant that was matched but not listed
   fails the run).
6. `the_backoff_states_its_abstention` — Suggest end to end on an Acrylic
   roughing pocket emits `DeflectionBackoffUnmodeled` carrying
   `MaterialUnvalidated`; the counterpart in hard maple never emits it.

**Inject-and-confirm, as the folder file requires.** Two injections, each
restored and re-verified green afterwards:

| Injection | Result |
|---|---|
| Restore `Ok { predicted_um: 0.0 }` on the `MaterialUnvalidated` exit | `test result: FAILED. 4 passed; 2 failed` — `every_refusal_names_itself` and `the_backoff_states_its_abstention` |
| Drop the V-bit caveat (`CutterKind::VBit => None`) | `test result: FAILED. 5 passed; 1 failed` — `a_v_bit_is_modelled_and_says_what_is_missing` |

The first injection's failure output is the defect in words: the back-off
saw `[RoughingDepthClampedToRigidity { requested: 4.0, capped: 1.2 }]` and
nothing else on an unvalidated plastic at 1.2 mm DPP — above the 0.5 mm
floor, so the loop ran and said nothing.

## 8. Verification

Every command through `scripts/cargo_lane.sh`.

```
cargo fmt --all -- --check
(no output, exit 0)

cargo test -p rs_cam_core -q --test a_refused_deflection_is_not_a_zero_g_t4
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s

cargo test -p rs_cam_core --lib -q
test result: ok. 2520 passed; 0 failed; 12 ignored; 0 measured; 0 filtered out; finished in 35.88s

cargo clippy -p rs_cam_core --all-targets --features heavy-tests,research,test-support -- -D warnings
Finished `dev` profile [unoptimized + debuginfo] target(s) in 22.20s
```

`rg -l "predict_peak_deflection|DeflectionBreakdown" crates/rs_cam_core/tests/`
returns the new sentry and nothing else, so no existing integration sentry
names the predictor.

`cargo check --all-targets` is clean on `rs_cam_cli`, `rs_cam_mcp` and
`rs_cam_viz` (the last with `-j 2`). No file in those crates changed, so
their focused test suites were not run.

**NOT run:** `wanaka_suggest_integration` (minutes, needs the Wanaka
project). It compiles. See §6 for the two edits it carries and why they are
an argument from source rather than a measurement.

## 9. Still open

`f_V`, a V-bit's equivalent-diameter fraction, has no source. Under the
no-bench-rig rule it stays unmodelled. The caveat makes the gap visible; it
does not close it.
