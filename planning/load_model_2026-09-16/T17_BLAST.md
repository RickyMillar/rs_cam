# T-17 blast radius — the bending section in `tip_deflection_mm`

Read-only survey. No cargo command ran. Every number below traces to a file
and a line, to a git commit, or to the literature the code already names.

## Summary — what this costs to fix

1. The edit is **one expression** in `crates/rs_cam_core/src/tool/mod.rs:697-700`.
   No consumer changes, because every consumer reads a `δ` or a compliance and
   none reads the section diameter.
2. **Six** production sites read `tip_deflection_mm` or the sample wrapper.
   Two are guards, two are back-off solvers, one is a displayed colour, one is
   the axial-DOC envelope. All move together, so no two surfaces can disagree.
3. **Four sentries go red on magnitude**; three more are at risk and I cannot
   settle them by reading. **Eleven** deflection sentries survive, because they
   assert a ratio, an ordering or a round trip.
4. The "+36 %" in `DEFLECTION_BACKOFF_TARGET_UM` **does not support** the
   missing-flute hypothesis. It was written on 2026-06-07 against a predictor
   that commit `3b0dc487` deleted on 2026-06-17, and its fixture is 2-flute.
5. A second finding, outside the brief: `endmill_equivalent_diameter_fraction`
   (T-16, commit `6aca2ffd`, today) **changes no product number today**. Its
   only consumer is a breakdown field that nothing renders.

---

## 1. Every consumer

`ToolDefinition::tip_deflection_mm` is defined at
`crates/rs_cam_core/src/tool/mod.rs:648`.
`tool_load::deflection::sample_tip_deflection_mm` is defined at
`crates/rs_cam_core/src/tool_load/deflection.rs:80`. The wrapper calls
`feeds::predict::tip_deflection_from_engagement`
(`crates/rs_cam_core/src/feeds/predict.rs:476`), which calls
`tip_deflection_mm` at `predict.rs:492`. So both names reach one model.

### Direct production call sites of `tip_deflection_mm`

| Site | What the number decides |
|---|---|
|  `feeds/predict.rs:492` | The single shared entry point. Every other reader goes through it or calls the method directly. |
| `feeds/predict.rs:389` (via `tip_deflection_from_engagement`) | `predict_peak_deflection_um` returns the µm the Suggest DPP back-off loop iterates against. A back-off that reduces DPP by 20 % per step. |
| `feeds/efficiency.rs:339` | A compliance constant. `deflection_chipload_ceiling_mm` inverts it into the advance-per-tooth ceiling the feeds nomogram draws. |
| `session/compute.rs:1724` | A compliance constant. It fills `DeflectionLimitInputs::compliance_mm_per_n` for the F-039 per-move feed optimizer. |
| `feeds/cutter_constraints.rs` (`invert_deflection`) | A guard that iterates. A monotone binary search over `tip_deflection_from_engagement` returns `max_doc_deflection_mm`, the axial-DOC ceiling Suggest clamps DPP to. |
| `tool/mod.rs:778` | Not a consumer. `ToolDefinition` delegates `lookup_diameter_at` to the inner cutter. |

### Call sites of `sample_tip_deflection_mm`

| Site | What the number decides |
|---|---|
| `tool_load/deflection.rs:239` (inside `evaluate`) | A guard that refuses. The peak `δ` over all cutting samples selects `Within(Validated)` under 50 µm, `Within(Approximate)` to 200 µm, and `Exceeds { LongToolStiffnessUnsafe }` above it. `WITHIN_BOUND_MM` is `deflection.rs:60`; `EXCEEDS_BOUND_MM` is `deflection.rs:63`. |
| `crates/rs_cam_viz/src/app/simulation.rs:706` | A displayed figure. The peak per move feeds `deflection_render_color` (`simulation.rs:713`), which paints the cutter green, amber or red against the same two bounds. |

### The downstream constants the same number sets

- `feeds/force.rs:250` — `budget_force_n = max_tip_deflection_mm / compliance_mm_per_n`. This is the closed-form feed cap. `dressup/feed_modulation.rs:457` consumes it as `BindingConstraint::DeflectionMax`.
- `feeds/efficiency.rs:116` — `CutEfficiency::deflection_ceiling_mm`, drawn as the corridor ceiling wedge on the nomogram.

**Every one of these scales linearly with the compliance.** The model is linear
in force, so a uniform change of section stiffness moves all six by one factor.

---

## 2. The +36 % question

### What the comment says

`crates/rs_cam_core/src/feeds/suggest/invariants.rs:34-38`:

> 200 µm matches the post-sim `tool_load::deflection` critical
> threshold directly. The predictor over-shoots post-sim by ~36% on
> the Wanaka Back Rough motivating case (487 µm predicted vs 358 µm
> observed), so this is already conservative.

487 / 358 = 1.3603.

### What was compared against what

Commit `ba01f7a8`, dated **2026-06-07**, introduced that text
(`git log -S'over-shoots post-sim by'`). At that date
`predict_peak_deflection_um` used a **bespoke single-section formula**. The
module doc records it at `crates/rs_cam_core/src/feeds/predict.rs:29-32`:

> Before 2026-06-17 it used a bespoke single-section
> `δ = F·a²·(3L−a)/(6·E·I)` with `I = π·(0.7·D)⁴/64`,
> which applied the end-mill flute-relief factor to the *entire* stickout
> — including the stiff shank — and read ~3× hotter than the gate.

So the 487 µm came from a formula that:

- applied 0.7 to the **whole stickout**, shank included;
- used an aggregate `Kc·ap·ae` force, not the affine feed-aware force;
- had one uniform section, not the two-section stepped beam.

The 358 µm came from the post-sim gate, which integrated the stepped cantilever
through `lookup_diameter_at`. The two sides differed in three ways at once.

### What changed after the comment was written

- **2026-06-17**, commit `3b0dc487` "reconcile deflection predictor onto the
  gate's integrated cantilever". The predictor now delegates to
  `tip_deflection_from_engagement` → `tip_deflection_mm`
  (`feeds/predict.rs:389`). `bending_diameter_mm` and `i_eff_mm4` survive only
  as breakdown diagnostics (`feeds/predict.rs:35-37`, `predict.rs:349-356`).
- **2026-06-20**, commit `cb17369e` recalibrated the force to
  literature-absolute.

After 2026-06-17 the predictor and the post-sim gate call **the same function
with the same physics**. `deflection.rs`'s own sentry
`gate_routes_through_canonical_force_model` asserts
`(via_gate - via_canonical).abs() < 1e-12`
(`tool_load/deflection.rs`, test module, arm `gate_routes_through_canonical_force_model`).
Any residual difference between the two today comes from the **inputs**: the
predictor evaluates one nominal operating point, the gate takes the peak over
the simulated samples.

### The hypothesis

> The 36 % is the missing flute-count term measured on a 4-flute fixture.

**The code contradicts it, on three independent grounds.**

1. **The fixture is 2-flute, not 4-flute.** Every Wanaka Back Rough fixture in
   the tree sets `tool.flute_count = 2`:
   `feeds/suggest/tests.rs:1316`, `feeds/suggest/tests.rs:2308`,
   and the `carbide_flat` helper in `tool_load/deflection.rs` builds
   `ToolDefinition::new(..., 2, ToolMaterial::Carbide)`.
2. **The arithmetic does not land on 1.36.** With the published fractions the
   omitted stiffness factor is `1/f⁴`:

   | Flutes | Fraction | `1/f⁴` |
   |---|---|---|
   | 2 | 0.889 | 1.601 |
   | 3 | 0.841 | 1.999 |
   | 4 | 0.748 | 3.194 |
   | fallback | 0.700 | 4.165 |

   No sourced flute count gives 1.36. The 2-flute figure the fixture would use
   is 1.601. (The brief quoted 2.25 for three flutes; `0.841⁴ = 0.5002`, so the
   three-flute factor is 2.00.)
3. **The direction is wrong.** The old predictor applied 0.7 to the whole
   stickout, so it read **hotter** than the gate. The over-shoot came from the
   predictor's excess, not from the gate's deficit. Repairing the gate's
   section makes the gate hotter and would have **reduced** the same gap.

### The consequence for the threshold

The drift caveat at `invariants.rs:40-49` warns that tuning
`endmill_equivalent_diameter_fraction` closer to the integrator removes the
safe-side bias. That warning no longer describes the code. The constant reaches
no deflection magnitude. The bias it names was a property of a formula deleted
on 2026-06-17.

`DEFLECTION_BACKOFF_TARGET_UM = 200.0` (`invariants.rs:50`) equals
`EXCEEDS_BOUND_MM × 1000` (`tool_load/deflection.rs:63`). The back-off and the
gate therefore share one bound and one model. The T-17 fix moves both by the
same factor. It introduces **no new relative error between them**. It does
lower every `max_doc_deflection_mm` and every feed cap by that factor, which is
the intended tightening.

### What I cannot settle

I cannot confirm what today's predictor and today's gate read on the Wanaka
Back Rough case, because that needs a run. The comment's prose is stale
regardless of that number: its stated mechanism no longer exists in the code.
**Delete the +36 % claim rather than re-measure it.** If a replacement figure
is wanted, the honest statement is "the predictor and the gate share one model;
they differ only by the operating point each evaluates".

### Second finding — T-16 moved nothing

`endmill_equivalent_diameter_fraction` (`feeds/predict.rs:117`) has exactly one
production consumer: `bending_diameter_mm` (`predict.rs:438`). That function has
exactly one production consumer: `d_core` at `predict.rs:357`, which builds
`i_eff_mm4` at `predict.rs:358`. `i_eff_mm4` reaches a `tracing::debug!` field
and the `DeflectionBreakdown` struct (`predict.rs:701`).

`rg` over `crates/rs_cam_viz/src`, `crates/rs_cam_cli/src` and
`crates/rs_cam_mcp/src` finds **no reader** of `i_eff_mm4` or
`DeflectionBreakdown`.

So commit `6aca2ffd` (today) corrected a diagnostic, not a machining decision.
T-17 is what carries that research into the product.

---

## 3. Which sentries go red

### RED — they assert a magnitude and the fix moves it

| File / test | The assertion | Why it goes red |
|---|---|---|
| `crates/rs_cam_core/src/tool/mod.rs`, `tip_deflection_uniform_cylinder_matches_closed_form` (line 1191) | `rel_err < 0.01` against `I = π·6⁴/64` with `d = 6.0` | The test builds a `FlatEndmill` and compares against the FULL diameter. The corrected model reads 1.601× the closed form at 2 flutes. |
| `crates/rs_cam_core/src/tool/mod.rs`, `deflection_chain_matches_hand_calc_from_published_formulas` (line 1214) | `rel_err < 0.01` against a hand beam with `i = π·d⁴/64`, `d = 6.0` | Same cause. The hand calculation states the section as the nominal diameter. |
| `crates/rs_cam_core/src/tool/mod.rs`, `tip_deflection_two_segment_stepped_matches_hand_calc` (line 1272) | `expected = force × 41.917 / e` | The 41.917 mm³ hand derivation assumes a 6 mm cutter section. The cutter region carries the correction, so the constant must be re-derived. |
| `crates/rs_cam_core/src/feeds/predict.rs`, `wanaka_back_rough_predicts_within_post_sim_band` | `(30.0..=70.0).contains(&um)`, comment says ~48 µm | 48 × 1.601 ≈ 77 µm. That is above the 70 µm ceiling. |

Arms 1 to 3 are repairable by arithmetic alone. Each hand calculation needs the
same `0.889` factor on the cutter-region diameter. Arm 3's 41.917 mm³ constant
must be re-derived by hand; the shank part of that integral does not change.

Arm 4 needs a new band. Do not widen it to hide the shift; state the new
expected value in the comment as the old one is stated.

### AT RISK — a magnitude claim whose margin I cannot measure by reading

| File / test | The assertion | The risk |
|---|---|---|
| `crates/rs_cam_core/src/feeds/suggest/tests.rs`, `deflection_machinery_caps_dpp_for_long_reach_tool` (line 1293) | `(4.0..9.0).contains(&dpp_after)` on a Ø6 2F at 85 mm stickout | The safe DPP falls with the stiffness. The test asks whether it stays above 4.0 mm. |
| `crates/rs_cam_core/src/feeds/cutter_constraints.rs`, `full_envelope_picks_tighter_max` | `binding_constraint == VendorAp` and `safe_max_doc_mm() == 1.5` | The vendor cap is 1.5 mm. If the deflection bound falls below 1.5 mm the binding constraint flips to `Deflection` and both assertions fail. |
| `crates/rs_cam_viz/tests/the_corridor_bounds_the_band_g_corridor.rs`, arms 1 and 4 | `LONG_AND_THIN` (Ø1.5, 90 mm stickout) must still yield a ceiling, and it must stay on chart | `chipload_cap_for_deflection_with_reason` (`feeds/force.rs:250-256`) returns `EdgeForceOverBudget` when `edge_force_n >= budget_force_n`. The fix divides `budget_force_n` by 1.601. If that crosses the edge force, `measured()`'s `.expect(...)` panics and two arms die. |

The corridor file's own doc block (lines 12-23) states that arm 1's fixture sits
in a narrow window and warns that a shift needs the fixture re-derived rather
than the arm dropped. Treat a red here as expected work, not as a surprise.

I cannot settle these three by reading. Each needs the affine inverse evaluated
on its own fixture. That is arithmetic on values already in the tree
(`feeds::force::affine_coefficients`, the fixture's `ap` and `woc`), not a
machine measurement. **Do that arithmetic before the edit**, so the three are
either pre-adjusted or knowingly re-baselined.

The corridor file's arm 2 numbers survive comfortably. The `STUBBY` ceiling
sits about 80× above the chart top (file doc, lines 15-18). Dividing by 1.601
still leaves it about 50× above.

### GREEN — ratio, ordering or round trip, so a uniform stiffness change cannot move them

| File / test | What it asserts |
|---|---|
| `tool/mod.rs`, `tip_deflection_carbide_stiffer_than_hss_by_3x` | `hss / carbide == 3.0`. A section change cancels. |
| `tool/mod.rs`, `tip_deflection_zero_force_is_zero` | Exact zero at zero force. |
| `tool_load/deflection.rs`, `higher_feed_raises_deflection_but_edge_floor_holds` | `d_fast > d_slow` and `d_fast < 2·d_slow`. Both sides scale together. |
| `tool_load/deflection.rs`, `gate_routes_through_canonical_force_model` | Gate equals canonical within 1e-12. Both call the same function. |
| `tool_load/deflection.rs`, `gate_honors_optimizer_feed_down_into_within` | It derives its own safe feed from `tool.tip_deflection_mm(1.0, ap, e)` at line 603, so it tracks the model. |
| `tool_load/deflection.rs`, `long_hss_in_steel_kc_exceeds_at_model_level` | `delta_um > 200.0`. The fix raises `delta_um`. Safe direction. |
| `tool_load/deflection.rs`, `wanaka_endmill_back_rough_deflection_is_within` | `(8.0..=40.0)` around ~17 µm. 17 × 1.601 ≈ 27 µm, inside the band. The stated "~17 µm" in the comment goes stale and should be updated. |
| `tool_load/deflection.rs`, `small_engraver_low_feed_in_hardwood_passes` | `peak_um < 200.0` at ~50 µm. 50 × 1.601 ≈ 80 µm. |
| `feeds/predict.rs`, `shallow_dpp_predicts_well_below_threshold` | `(1.0..=50.0)` at ~5 µm. 5 × 1.601 ≈ 8 µm. |
| `feeds/predict.rs`, `doubling_stickout_octuples_deflection` | A pure ratio in stickout. |
| `feeds/cutter_constraints.rs`, `deflection_inversion_roundtrips_against_predict` | Forward then inverse on one model. |
| `feeds/cutter_constraints.rs`, `tapered_ball_quadratic_force_doubling_axial_doubles_deflection` | Tapered ball, which the fix does not touch, plus an ordering. |
| `feeds/cutter_constraints.rs`, `vbit_uses_deflection_bound_only` | V-bit, untouched, plus `> 0.0`. |
| `tool/mod.rs`, `tip_deflection_tapered_ball_lies_between_shank_and_tip_limits` | **Ordering, but see the trap below.** |
| `crates/rs_cam_core/tests/constrained_max_modulation_f039.rs`, `constrained_max_binds_on_deflection_for_long_tool` | It hard-codes `compliance_mm_per_n: 0.029` at line 197. It never calls the model. |
| `crates/rs_cam_core/tests/smoke_baseline_regression_f037.rs` | It compares `deflection_kind` strings in fixture CSV text. It runs no model. |
| `crates/rs_cam_viz/tests/the_chipload_verdict_is_one_row_g_chipverdict.rs`, `a_near_total_headroom_never_prints_as_a_flat_hundred_g_chipverdict` | It asserts the face is NOT `100 %`. A larger `δ` moves away from 100 %. |

**Trap inside the green list.** `tip_deflection_tapered_ball_lies_between_shank_and_tip_limits`
(`tool/mod.rs:1291`) asserts `shank < real < tip`, where `shank` and `tip` are
`FlatEndmill` bounds and `real` is a `TaperedBallEndmill`. The fix moves both
flat bounds by 1.601 and leaves `real` fixed. The ordering is therefore no
longer guaranteed by construction. Check `shank < real` before you trust it. I
cannot settle it by reading.

---

## 4. Where the fix belongs

### The other callers want the engagement diameter — confirmed

`rg -n "lookup_diameter_at"` returns these **production** call sites besides
`tip_deflection_mm`:

| Site | What it asks for |
|---|---|
| `session/compute.rs:1713` | `engagement_dia` for `PowerLimitInputs::engagement_diameter_mm`. The spindle power model needs the width that cuts. |
| `tool_load/optimize/context.rs:106` | `diameter_for_lut_lookup`. The doc at line 92-99 states it explicitly: "returns the actual engaged diameter". |
| `tool_load/chipload.rs:550` | The vendor-LUT row query in `matched_chip_envelope`. |
| `tool_load/chipload.rs:579` | `doc_ratio = lookup_axial_doc_mm / lookup_diameter_at_peak`, the DOC-derating scale. |
| `tool_load/mod.rs:336` | The same LUT query for the Suggest-facing envelope. |
| `tool_load/mod.rs:346` | The same `doc_ratio`. |
| `tool/mod.rs:778` | `ToolDefinition` delegates to the inner cutter. Not a consumer. |

The remaining hits are doc comments, test bodies and the parity sentry
`feeds::tests::engaged_diameter_at_doc_matches_lookup_diameter_at_across_shapes`,
which pins `lookup_diameter_at` against `ToolGeometryHint::engaged_diameter_at_doc`
(`feeds/mod.rs:124-132`).

**Every one of them wants the engagement diameter.** The method's own contract
at `tool/mod.rs:389-398` says so: "the cutter's actual contact width when
engaging material". Do **not** change `lookup_diameter_at`. Change only the
bending consumer.

Note: the brief said 14 other callers. The accurate count is **six** production
call sites plus one delegation. The rest are tests and prose.

### The minimal correct edit

One site: `crates/rs_cam_core/src/tool/mod.rs`, the region-2 block at lines
690-707. Replace it with the following.

```rust
        // Region 2: cutter, x ∈ [shank_top, load_pos].
        if load_pos > shank_top {
            const N_CUTTER: usize = 64;
            // `lookup_diameter_at` returns the ENGAGEMENT diameter — how wide
            // the cutter cuts. A bending model needs the EQUIVALENT diameter:
            // the solid shaft with the same compliance. The flutes remove
            // material from the section, so for a straight-walled end mill
            // the two differ by the flute-count fraction (Kivanc and Budak,
            // Sabanci MSc thesis 2004, Tables 3.1 and 3.2). `I` goes with the
            // 4th power, so the omission over-stated stiffness by 1.60× at
            // two flutes, 2.00× at three and 3.19× at four.
            //
            // Key the correction off the cutter SHAPE, not off the flute
            // count. A ball, bull, tapered ball and V-bit each carry their
            // own profile through `lookup_diameter_at`, and none of those
            // profiles is the flute-relieved cylinder this fraction
            // describes. `feeds::predict::bending_diameter_mm` makes the same
            // per-shape decision for its breakdown diagnostic; the two must
            // agree. The match is exhaustive so a 6th shape fails to compile
            // here rather than inheriting a wrong section silently.
            let bending_fraction = match self.cutter.geometry_hint().cutter_kind() {
                crate::feeds::CutterKind::Flat => {
                    crate::feeds::predict::endmill_equivalent_diameter_fraction(self.flute_count)
                }
                crate::feeds::CutterKind::Ball
                | crate::feeds::CutterKind::Bull
                | crate::feeds::CutterKind::TaperedBall
                | crate::feeds::CutterKind::VBit => 1.0,
            };
            let span = load_pos - shank_top;
            let dx = span / N_CUTTER as f64;
            for i in 0..N_CUTTER {
                let x = shank_top + (i as f64 + 0.5) * dx;
                let axial_from_tip = l - x;
                let d = (bending_fraction * self.cutter.lookup_diameter_at(axial_from_tip))
                    .max(D_FLOOR_MM);
                let i_mm4 = std::f64::consts::PI * d.powi(4) / 64.0;
                let arm = load_pos - x;
                let inv_ei = force_n / (e * i_mm4);
                delta_at_load += inv_ei * arm * arm * dx;
                slope_at_load += inv_ei * arm * dx;
            }
        }
```

Also update the method doc at `tool/mod.rs:639`, which currently says the
cutting region uses `lookup_diameter_at` without qualification.

### Trap (a) — the shape key

`MillingCutter` has no `cutter_kind()`. The route is
`self.cutter.geometry_hint().cutter_kind()`:

- `MillingCutter::geometry_hint` — `tool/mod.rs:530`, overridden per shape.
- `ToolGeometryHint::cutter_kind` — `feeds/mod.rs:182`.
- `ToolGeometryHint` derives `Copy` (`feeds/mod.rs:62`), so the by-value
  `self` receiver costs nothing.
- `CutterKind` has exactly five variants (`feeds/mod.rs:214-220`), so an
  exhaustive match is a compile-time sentry.

`ToolDefinition` already delegates `geometry_hint` at `tool/mod.rs:799-801` and
already matches on `ToolGeometryHint` in production at `tool/mod.rs:277` and
`tool/mod.rs:342`. No new capability is needed.

**Flagged concern, not a silent change.** A bull nose end mill is a fluted end
mill with a corner radius. Its bending section is flute-relieved in the same
way. `bending_diameter_mm` assigns it the full `D` with the rationale "the
corner radius leaves a near-solid shaft above" (`predict.rs:428`). That
rationale describes the tip, not the flute length above it, so I do not find it
convincing. The code above follows the brief and leaves Bull at 1.0 so that the
two models agree. Treat Bull as a separate, sourced decision.

### Trap (b) — the layering

**There is no import cycle and no new layering direction.** Rust resolves
module references inside one crate without ordering constraints, and
`tool` → `feeds` already exists in production code:

- `tool/mod.rs:277` uses `crate::feeds::ToolGeometryHint::TaperedBall`.
- `tool/mod.rs:293` uses `crate::feeds::ToolGeometryHint::Bull`.
- `tool/mod.rs:530` and `tool/mod.rs:799` return `crate::feeds::ToolGeometryHint`.

The reverse edge exists too:  `feeds/predict.rs:492` calls
`tool.tip_deflection_mm`. The two modules already reference each other. Adding
`crate::feeds::predict::endmill_equivalent_diameter_fraction` adds nothing new.

`endmill_equivalent_diameter_fraction` is `pub` (`predict.rs:117`) and takes a
plain `u32`, so it carries no `ToolConfig` or compute-layer type into `tool`.

**Keep the constant where it is.** Moving it to `tool/` would break the import
line in `crates/rs_cam_core/tests/the_bending_diameter_knows_the_flute_count_g_bendeq.rs:38-41`
for no gain. If a later refactor does move it, re-export it from
`feeds::predict` so that sentry keeps its path.

---

## 5. Tapered and V-bit

Both override `lookup_diameter_at` with a real profile.

**V-bit** — `crates/rs_cam_core/src/tool/vbit.rs:133`:

```rust
fn lookup_diameter_at(&self, axial_doc_mm: f64) -> f64 {
    (2.0 * self.engagement_radius(axial_doc_mm)).clamp(0.0, self.diameter())
}
```

**Tapered ball** — `crates/rs_cam_core/src/tool/tapered_ball.rs:177`: the same
body, over the ball and cone regions.

Both return the cone or ball width at that height. For these two shapes the
engagement diameter and the outer profile diameter are the same quantity,
because the cone surface **is** the outer envelope. So the integrator walks a
true taper, and that is a defensible bending section. `tip_deflection_mm`
integrates `x` from the collet down to the load point and reads
`axial_from_tip = l - x`, so the taper enters in the right place.

Two qualifications, both stated so the record is complete:

1. These cutters are also fluted, so the flutes relieve their section too. The
   literature this repository cites (Kivanc and Budak) covers end mills only. No
   sourced fraction exists for a cone or a ball. Leaving them at 1.0 is the
   honest choice, and the fallback direction is the conservative one for a
   flat only. For these shapes 1.0 **under-states** deflection by an unknown
   amount. Record that as a known gap, not as a modelled value.
2. Near the tip of a V-bit the profile diameter goes to zero. `D_FLOOR_MM =
   0.05` (`tool/mod.rs:668`) keeps `1/d⁴` finite. That floor is a numerical
   guard, not a physical section, and it already carries that comment. The fix
   does not change it.

**The same defect does not apply to these two overrides.** The defect is
specific to the flat end mill, whose trait default (`tool/mod.rs:422`) returns
the full cutting diameter and so models a solid cylinder. A ball nose inherits
that same default, so it models a solid cylinder too — which for a ball is
correct above the ball region and conservative within it.
