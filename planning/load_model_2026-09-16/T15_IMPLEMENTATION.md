# T-15 — the power ceiling is re-checked after the pass 9 rescale

Written 2026-09-18. Branch `master`, parent `7e7189d6`. Not committed; the
orchestrator reviews and commits.

---

## 1. What shipped

`enforce_invariants` gained **pass 10**, `recheck_power_after_rescale`, in
`crates/rs_cam_core/src/feeds/suggest/adaptive_entry.rs`, beside pass 9.

- It runs LAST, and only when pass 9 moved the feed. The gate is the warning
  pass 9 files, `SuggestWarning::FeedRescaledToFinalGeometry`, which also
  carries the pre-rescale feed pass 10 needs. An operation no pass touched
  keeps its feed byte-identical.
- It rebuilds `tool_load::power::PowerTerms` — the ONE power model — at the
  final `ap`, `ae`, RPM and feed.
- It compares against `machine.power_at_rpm(rpm) * machine.safety_factor`,
  what Step 6 names `gate_available_power`. Pass 9's feed is a COMMANDED feed,
  so the factor is applied once.
- The clamp is feed-only and downward. `PowerTerms::feed_for_kw` solves the
  fitting feed in closed form.
- When `feed_for_kw` returns `None` the edge term alone is over the ceiling and
  no feed fits. The feed goes back to the value pass 9 started from and the
  warning carries `fits_at_any_feed: false`.
- It re-runs `clamp_plunge_to_feed` after it moves the feed, as pass 9 does.

New warning `SuggestWarning::PowerRecheckedAfterRescale`, new
`RationaleReason::PowerCeilingAfterRescale`, and the rationale entry on
`RationaleParam::Feed` with a plain-words headline and detail. Every number in
both strings is formatted from a value.

Pass 9's doc paragraph "What it deliberately does not re-check" was rewritten.
The power ceiling now has its own heading pointing at pass 10, and the 23.6 %
justification is withdrawn as stale — measured before R1 rebuilt the power
model about 8.6× higher — not re-measured. The deflection-budget paragraph is
unchanged; it still holds.

---

## 2. The red run, and the arithmetic that predicted it

The sentry is
`crates/rs_cam_core/tests/a_rescaled_feed_stays_inside_the_power_ceiling_g_t15.rs`.

### The fixture

The three fixtures the register tried never crossed a tier boundary, and the
reason is in the older instrument, not in the engine: every one of them asks
for a full-width slot, `SlottingDetected` caps the depth at 0.25 × D before
Step 6 runs, and `depth_tier_multiplier` reads 1.00 on both sides of every
clamp. Pass 9 then short-circuits on an unchanged factor.

The crossing case needs a depth above a boundary that a clamp brings just
below it:

| Choice | Value | Why |
|---|---|---|
| Operation | `Adaptive` (2D) rough | Adaptive feeds family, so the rigidity clamp reads `adaptive_doc_factor`; outside the axial-envelope routing, so pass 0 leaves the depth alone |
| `adaptive_doc_factor` | 2.0 | the shipped `RigidityProfile::default()` value; the cap lands on 24.000 mm, exactly 2.0 × D |
| Tool | Ø12, 2 flutes, cutting length 40 mm, stickout 50 mm | the flute guard (0.8 × 40 = 32 mm) and the cutting-length clamp both clear 24.05 mm, and the tool is stiff enough that the deflection back-off does not fire |
| Operator depth | 24.05 mm | 2.00417 × D, one step above the `ap/D > 2.0` boundary |
| Stepover | 10.0 mm | 0.833 × D, under the 0.85 × D slotting threshold |
| Material | `GenericHardwood` | publishes a `Kc` |
| Spindle | synthetic `ConstantPower { 1.6 kW }` | ceiling 1.2 kW, which Step 6 clamps this recipe exactly onto |

Every one of those is a fixture choice — a depth, a diameter, a rating. No
model constant was invented.

### The prediction

Power is affine in the feed, `P = shear·feed + edge`. The shear term carries
the cross-section `ap · ae`, the edge term carries `ap` alone, and neither
carries the depth tier. So

```text
P_ship / P_calc = depth_ratio × (1 + (tier_ratio − 1) × shear_share)
```

Measured at the calculator's point: `depth_ratio = 24.000 / 24.050 =
0.997921`, `tier_ratio = 0.75 / 0.50 = 1.5`, `shear_share = 0.2180`.

```text
0.997921 × (1 + 0.5 × 0.2180) = 1.1067
```

### The measurement

With pass 10 disabled, the sentry reports:

```
G-T15 headline | calc 653.403 mm/min @ ap 24.0500 -> shipped 980.105 mm/min @ ap 24.0000;
                 shear share 0.2180, depth ratio 0.997921; predicted breach 1.1067x,
                 shipped load 110.67% of the ceiling
G-T15 un-clamped | Step 6 did not clamp (85.16% of the ceiling); pass 9 1976.000 ->
                   2965.052 mm/min; shipped load 104.43%
```

**110.67 % measured against 1.1067 predicted.** The prediction is the
mechanism, not a fit.

Three of the five arms were red; the two that do not depend on pass 10 — the
non-vacuity arm and the byte-identical arm — stayed green, which is correct.

The register's "about 1.46×" is the pure-shear bound (1.5 × 0.976). The edge
term carries no feed, so on this fixture 21.8 % of the load does not move with
the rescale and the real figure is 1.1067×. The arms quote the measured split
rather than the bound.

### How the red run was taken

The tree is shared, so `git stash` and every tree-wide command are forbidden.
The call to pass 10 in `enforce_invariants` was disabled with a one-line `&&
false` for a single test run and restored from a scratchpad copy immediately
afterwards. The restored file is byte-identical to the one verified below
(`grep -c "&& false"` returns 0). The same numbers had already been measured on
untouched HEAD by an exploratory sweep before any source edit.

---

## 3. How `effective_d` was reproduced

`feeds::calculate` computes it once at Step 5:

```rust
let effective_d = effective_diameter(
    input.tool_geometry, d, input.shank_diameter.unwrap_or(d), ap);
```

and passes that binding to `power_model_terms`, which builds ψ from it
(`force::immersion_angle(ae, effective_d / 2.0)`) and the cutting velocity
`Vc = π·D·n` from it.

Suggest does not hold the calculator's `FeedsInput`, so pass 10 rebuilds the
same three inputs from what it does hold:

| Input | Calculator | Pass 10 |
|---|---|---|
| geometry hint | `input.tool_geometry` | `build_cutter(tool).to_geometry_hint()` — what pass 9 already uses |
| nominal D | `input.tool_diameter` | `tool.diameter` |
| shank | `input.shank_diameter.unwrap_or(d)` | `tool.shank_diameter`, falling back to `tool.diameter` when it is not finite and positive |
| `ap` | the Step-4 depth | the operation's FINAL depth |

`feeds::effective_diameter` is already `pub` — it was made public in 2026-08
for pass 9's own sentry, with a doc saying a second implementation of the
chip-thinning diameter is exactly what Checkpoint C3 retired. Pass 10 calls it
rather than carrying a copy.

The shank binding is exact for the production door:
`suggest_for_operation` builds `FeedsInput { shank_diameter: Some(tool.shank_diameter), … }`
from the same `ToolConfig`. It reaches `effective_diameter` on the tapered-ball
arm only; flat, ball, bull and V-bit ignore it.

**One deliberate difference, stated because it matters.** The calculator sizes
`effective_d` at the Step-4 depth and keeps that binding even after the power
ladder shrinks `ap`. Pass 10 sizes it at the FINAL depth, which is what the
brief asked for and what the quantity means: the circle that actually touches
material at the depth the operation runs. On a flat end mill the two are the
same number, so the sentry's fixtures do not distinguish them; on a ball or a
V-bit they differ, and the final-depth value is the correct one.

---

## 4. Closed form, not bisection

`PowerTerms` exposes the split directly:

```rust
pub(crate) fn feed_for_kw(self, budget_kw: f64) -> Option<f64> {
    // headroom = budget − edge; feed = headroom / shear_slope
}
```

Required power is `shear·feed + edge`, affine and increasing in the feed, so
the feed at which required equals the ceiling is one division. The solved feed
is exact rather than converged, and `None` already means the one case no feed
answers — the edge floor is at or over the budget. Step 6 rung 4 inverts the
same function the same way, so the recommendation and the re-check cannot
disagree. No bisection was needed and none was written.

---

## 5. Decisions taken inside the brief

**The no-fit branch takes the lower of the two feeds, not the pre-rescale one
unconditionally.** Decision 5 says to put the feed back to the value pass 9
started from, and to never ship a feed above a value some ceiling check has
validated. Those two can conflict: when pass 9 LOWERED the feed, restoring the
pre-rescale value would RAISE the shipped feed on a cut no feed fits. Pass 10
writes `rescaled.min(pre_rescale)`, which satisfies both readings and is the
conservative one. On the sentry's refusal fixture pass 9 raised the feed, so
the two forms agree there.

**Pass 10 does not re-lift a clamped feed to the Step 9b rubbing floor.** The
power ceiling is a physical limit and the floor is an advisory band. Step 6
rung 4 resolves the same conflict the same way, and both warnings stand so the
operator sees that neither guarantee was met. This is stated in pass 10's doc.

**It abstains, silently, when the material publishes no `Kc`.** There is no
power model without one, and Step 6 applied no ceiling either, so there is no
ceiling here to re-check. The abstention goes to `tracing::debug!` rather than
a new warning variant: a warning would report a step not taken against a check
that never existed. Fabricating a `Kc` was never on the table.

---

## 6. Something the design did not anticipate

**The enum's forcing arm in `wanaka_suggest_integration.rs` fired, and that
file is outside the brief's allowed set.** Its match over `SuggestWarning` has
no `_` arm, deliberately, so a new variant is a compile error there. Without an
arm the crate does not build under `--all-targets`, which blocks every session
sharing this tree, so the arm was added. It is the only edit outside the
allowed set, and it is one match arm.

The arm is a **panic**, argued from the fixture:

- Pass 10 runs on Wanaka tp 4 and tp 10, the two `Adaptive3d` roughs whose DPP
  the axial envelope clamps 9.0 → 4.2 mm on a Ø6 tool. That is the crossing
  pass 9 already fires on there, and the existing block pins its direction.
- The crossing is 0.75 → 1.00, so `tier_ratio` is 1.333 and the bracket
  `1 + (tier_ratio − 1) × shear_share` is at most 1.333. The depth ratio is
  4.2 / 9.0 = 0.467. Even at a shear share of 1.0 the product is 0.62.
- So the load FALLS on both Wanaka toolpaths: a 53 % depth drop against a 33 %
  feed rise. Pass 10 has nothing to clamp, and the variant must not fire.

**This was argued but NOT run.** `wanaka_suggest_integration` is a long
simulation and the brief forbids it. T-4 took the same path on 2026-09-18 —
argue the arm from the fixture, then run it before the commit — and its two
arms came back 3/3 green with neither firing. **The orchestrator should run
`wanaka_suggest_integration` before committing.** If the new arm fires, the
axial envelope now clamps close to the boundary it crosses, or the Wanaka
machine profile lost power; both are real findings about the fixture.

Two smaller surprises, both handled:

- Pass 9 re-derives from the calculator's UNROUNDED feed while the operation
  carries the floored copy T-9 writes, so the lift measures ×1.5009 against the
  operation's 653.000 and ×1.5000 against the calculator's 653.403. The
  non-vacuity arm pins the tier ratio against the calculator's value, where it
  is exact, and prints both.
- The maximum breach this mechanism can produce is bounded. `P_ship / P_calc <
  tier_ratio = 1.5`, and the depth ratio and the edge term both pull it down.
  On the un-clamped fixture the lift is ×1.2262, so a recipe Step 6 never
  clamped has to sit above 1 / 1.2262 = 81.6 % of the ceiling before the
  rescale can push it over. The fixture is pinned into an 80-90 % band, which
  straddles the 89.4 % p90 the register re-measured after R1. That band IS the
  reachable window, and the arm says so rather than claiming a wider one.

---

## 7. Tests that moved

**None.** No pinned shipped value changed. Every apply-door target passed
before and after, unmodified.

---

## 8. Verification

Every command through `scripts/cargo_lane.sh`.

| Command | Result |
|---|---|
| `fmt --all -- --check` | clean (exit 0, no diff) |
| `clippy -p rs_cam_core --all-targets --features heavy-tests,research,test-support -- -D warnings` | `Finished \`dev\` profile [unoptimized + debuginfo] target(s) in 4.56s` |

```
=== a_rescaled_feed_stays_inside_the_power_ceiling_g_t15 ===
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
=== core lib ===
test result: ok. 2521 passed; 0 failed; 12 ignored; 0 measured; 0 filtered out; finished in 36.84s
=== a_clamped_feed_ships_at_or_below_its_ceiling_g_feeddown ===
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
=== a_feed_lift_caps_at_the_cutting_ceiling_g_t18 ===
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
=== arc_fit_disposition_a5 ===
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 21.47s
=== a_refused_deflection_is_not_a_zero_g_t4 ===
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
=== suggest_feed_matches_final_geometry ===
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.03s
=== suggest_power_ceiling_after_pass9_g_suggest_powerstale ===
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
=== chipload_thinning_magnitude_survey ===
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s
=== literature_matrix ===
test result: ok. 21 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.04s
=== literature_parity ===
test result: ok. 24 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
=== _litmatrix_scallop_refuses_flat ===
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
=== mcp_mutation_rows_reach_core ===
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
=== pill_writes_clamped_value_g_pillclamp ===
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
```

`wanaka_suggest_integration` was NOT run — the brief forbids it. Section 6 says
why the orchestrator should.

The apply-door list was re-derived with

```
rg -l "suggest::apply\b|apply_feeds|suggest_for_operation|FeedsPreview::build|suggested_operation|enforce_invariants" crates/rs_cam_core/tests/
```

It adds `crates/rs_cam_core/tests/literature_matrix/shim.rs`, which is the
shared helper behind `literature_matrix` and `literature_parity`; both ran.
