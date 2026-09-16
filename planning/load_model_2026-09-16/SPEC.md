# Implementation spec — cut efficiency and the chipload corridor

**This is the frozen design.** `ADVICE.md` holds the physics, `UX_PLAN.md`
the product shape. This file names the code: every function that exists,
every function to be written, every sentry that will move.

Written after reading the call graph, not from the plan. Three findings
below changed the shape of the work.

---

## Findings from the code survey

### S-1. The deflection→chipload ceiling is already solved, in the wrong place

`feed_modulation.rs:431-460` inverts the affine force model onto a feed cap
in closed form:

```
budget_force_n = max_tip_deflection_mm / compliance_mm_per_n
edge_force_n   = ap · F_edge
fz_cap         = (budget_force_n / ap − F_edge) / (Ks · sin θ_peak)
cos ψ = 1 − 2·woc_fraction,   θ_peak = min(ψ, π/2)
```

It is **private**, inside `max_safe_feed_for_move`, operating on
per-move engagement. Phase B needs exactly this number pre-simulation.

**Do not duplicate it.** Extract it (§ N-2). This codebase's recurring
defect is two copies of one model drifting apart — it already happened
between `force.rs` and `power.rs`, which is the whole reason for this
package.

### S-2. R1's blast radius is three sites, not one

The linear power model appears in:

| Site | Form |
|---|---|
| `tool_load/power.rs:68` `predicted_power_kw` | the helper (`pub(crate)`) |
| `feeds/mod.rs:1728`, `:1950` | both call the helper — covered automatically |
| `feed_modulation.rs:462-` + `PowerLimitInputs` | **a second, independent implementation** |

A change to the helper alone leaves `feed_modulation` on the old model and
the optimizer disagreeing with the gate — the exact class of bug this
package exists to close.

### S-3. Phase C needs no core work at all

`ApplyScope::{Speeds, CutGeometry, Both}` (`suggest.rs:840`) and
`apply_speeds_to_op` (`suggest.rs:992`) are **intact**. `540715c1` deleted
only the GUI button. Phase C is a UI control plus a command-registry row.

---

## What already exists and is public

Verified `pub`, reachable from `rs_cam_viz`:

| Item | Path | Gives |
|---|---|---|
| `affine_coefficients(&Material) -> Option<(f64, f64)>` | `feeds::force:109` | `(Ks, F_edge)` |
| `immersion_angle(ae, r) -> f64` | `feeds::force:86` | ψ |
| `effective_rubbing_floor(Option<ChiploadBounds>) -> f64` | `feeds::mod:902` | the floor, band-aware |
| `RUBBING_FLOOR_MM_TOOTH = 0.025` | `feeds::mod:858` | |
| `predict_peak_deflection_um(op, &ToolConfig, mat, machine)` | `feeds::predict:129` | pre-sim δ + breakdown |
| `WITHIN_BOUND_MM = 0.050`, `EXCEEDS_BOUND_MM = 0.200` | `tool_load::deflection:61,64` | the bounds |
| `ToolDefinition::tip_deflection_mm(1.0, ap, E)` | `tool::mod:668` | compliance mm/N |
| `FeedsResult.chipload_bounds` | `feeds::mod` | the band |
| `ApplyScope`, `apply_speeds_to_op` | `feeds::suggest:840,992` | the narrow write |

`predict_peak_deflection_um` takes the **GUI's `ToolConfig`**, so the
inspector reaches the deflection model without constructing a
`ToolDefinition`.

Module visibility verified for the whole list: `rs_cam_core` exports
`feeds`, `tool_load` and `feed_modulation`; `feeds` exports `force` and
`predict`; `tool_load` exports `deflection`. Every anchor above was checked
against the file at the stated line.

`predicted_power_kw` is the one exception — it is `pub(crate)`, so the UI
cannot call it. That is the right shape and N-1 does not need it: core
computes the verdict, the UI renders it.

---

## New code

### N-1. `feeds::efficiency` — the bounded typed answer

New module `crates/rs_cam_core/src/feeds/efficiency.rs`.

Core computes the verdict; the UI renders it. This follows the rule in
`crates/rs_cam_viz/CLAUDE.md` — *"GUI state is not an alternate data model
… do not recompute a narrower stale answer in the UI"* — and the
`SimulationTriage` precedent of one bounded typed answer.

```rust
pub enum ChipVerdict { Thin, InBand, Heavy, NoBand }

pub struct CutEfficiency {
    pub advance_per_tooth_mm: f64,
    /// u = P / MRR, in J/mm³. ADVICE.md §2.
    pub specific_energy_j_per_mm3: f64,
    /// (u − Ks) / u — the share of cutting power spent ploughing, 0..1.
    pub ploughing_share: f64,
    pub verdict: ChipVerdict,
    pub band: Option<ChiploadBounds>,
    pub rubbing_floor_mm: f64,
    /// `None` when the deflection model refuses (drill, no Kc, no stickout).
    pub deflection_ceiling_mm: Option<f64>,
    /// u(fz) / u(band midpoint). `None` without a band.
    pub wear_ratio_vs_band_mid: Option<f64>,
    /// fz_mid / fz — time is inversely proportional to chipload at fixed
    /// RPM, DOC and WOC. `None` without a band.
    pub time_ratio_vs_band_mid: Option<f64>,
    /// 1 − predicted_δ / EXCEEDS_BOUND. `None` when deflection refuses.
    pub force_headroom: Option<f64>,
}

pub fn cut_efficiency(
    operation: &OperationConfig,
    tool: &ToolConfig,
    material: &Material,
    machine: &MachineProfile,
    result: &FeedsResult,
) -> Option<CutEfficiency>;
```

Closed forms, all from `ADVICE.md` §2:

```
u            = Ks + (F_edge · D · ψ) / (2 · ae · fz)
ploughing    = (u − Ks) / u
wear_ratio   = u(fz) / u(fz_mid)
time_ratio   = fz_mid / fz
```

`None` when `affine_coefficients` returns `None` — the material has no
primary-source Kc. **Return `None`, never a fabricated constant.** The
engine already refuses this way (`UnmodeledReason::MaterialUnvalidated`)
and the UI must render the refusal, not a zero.

### N-2. `feeds::force::chipload_cap_for_deflection` — extract S-1

```rust
pub fn chipload_cap_for_deflection(
    ks_n_per_mm2: f64,
    f_edge_n_per_mm: f64,
    compliance_mm_per_n: f64,
    max_tip_deflection_mm: f64,
    axial_doc_mm: f64,
    radial_woc_fraction: f64,
) -> Option<f64>;
```

`None` for the can't-satisfy case (edge force alone over budget) and for
zero lateral engagement — the two branches `feed_modulation.rs:442` already
distinguishes.

**`feed_modulation` is refactored to call it.** Pure refactor, no behaviour
change; `deflection_cap_inverts_affine_model_onto_the_bound`
(`feed_modulation.rs:838`) must stay green **unmodified**. If that test
needs editing, the refactor changed behaviour and is wrong.

---

## Phases

### Phase A — the chipload verdict row

| | |
|---|---|
| Touches | `ui/feeds/compare.rs` (`rail_power_row` → `rail_efficiency_row`), new `feeds/efficiency.rs` |
| Depends on | N-1 |
| Does **not** depend on | R1 |

The row replaces the power line. Three states from `ChipVerdict`, plus the
refusal:

```
Chip  0.038 mm/tooth  ⚠ thin — 1.6× tool wear, 1.8× the time   ⓘ
Chip  0.067 mm/tooth  ✓ in vendor range · 38 % force headroom  ⓘ
Chip  0.110 mm/tooth  ⚠ heavy — 8 % force headroom             ⓘ
Chip  0.067 mm/tooth  — efficiency not modelled for this material ⓘ
```

Hover carries `u` in J/mm³, the ploughing share, the band, the floor, the
ceiling and how each was derived.

Ratios are shown only when `band.is_some()`; headroom only when
`force_headroom.is_some()`. **A `None` renders as an abstention, never as a
zero or a blank** — the failure shape recorded four times in this
repository's history.

### Phase B — the corridor on the nomogram

| | |
|---|---|
| Touches | `ui/feeds/explore.rs` (`draw_chart_c`), `ui/feeds/shared.rs` (`wedge_polygon` reuse) |
| Depends on | N-1, N-2 |

Two shaded wedges: below `rubbing_floor_mm`, above `deflection_ceiling_mm`.
Both are iso-chipload rays, so both reuse `wedge_polygon`. Drawn in the same
idiom as the machine walls — fill, no stroke, no floating label.

**No new legend rows.** `Sources ⓘ` gains two clauses.

When `deflection_ceiling_mm` is `None`, the upper wedge is **not drawn** and
`Sources` says the ceiling is not modelled. Do not substitute a default.

### The corridor is one-sided for ordinary tools — measured during N-1

On the reference fixture (6 mm 2-flute flat, 4.20 DOC, 2.10 WOC, generic
softwood) the deflection ceiling comes out **9.36 mm/tooth**. The vendor
band maximum is 0.0850, and the top of the chart — 4 000 mm/min at 17 000
RPM on 2 flutes — is 0.1176 mm/tooth. The ceiling is around eighty times
the highest chipload the chart can draw.

It is not wrong. A stubby carbide cutter genuinely is not
deflection-limited, which is what `force.rs`'s module docs and ADVICE.md
§4 both already said: on this class of machine the binding constraints
are the feed cap, the rubbing floor and rigidity — not tooth force.

**So the upper wedge would be drawn off the top of the chart for ordinary
tools, exactly as the power contour would.** For long or thin tools it
does come on chart: compliance rises with the cube of stickout, so the
ceiling falls fast.

The rule for Phase B: **draw the upper wedge only when the ceiling falls
inside the plot's y-range, and abstain visibly when it does not.** Never
clamp it to the chart edge — a wedge pinned to the top reads as "you are
near the force limit", which would be false by two orders of magnitude,
and that is the absence-rendered-as-a-reading failure this programme
keeps meeting. `Sources` names the ceiling and says when it is off scale.

Expect the corridor to be bounded by the rubbing floor and the machine
walls in the common case, with the force ceiling appearing only for tools
that actually have a deflection problem. That is the honest picture, and
more useful than a symmetric corridor would be.

### Phase C — `Match vendor chipload`

| | |
|---|---|
| Touches | `ui/feeds/compare.rs`, `ui_command.rs`, `controller/events/mod.rs` |
| Core work | **none** (S-3) |

A second button beside `⚡ Apply all — changes the cut`, routing
`ApplyScope::Speeds` through `feeds::suggest::apply`. Holds DOC and WOC.

Contract notes:
- `apply_contract_a3.rs` asserts **no feeds surface pushes a per-field
  apply**. This is not per-field — it is the funnel with a narrower scope,
  which the funnel was built to support. The sentry needs no change;
  confirm by running it, not by reasoning.
- The registry row must declare an honest `Reach`. `ToggleFeedsProvenance`
  claimed `gui: Reach::Reached` while unreachable; do not repeat it.
- Attribution stays on the button face (A-3): the label must say it changes
  the speeds and not the cut.

### Phase D — the aggressiveness dial

Deferred. Re-evaluate only after A–C ship. Likely unnecessary once the
verdict row and the corridor exist.

### R1 — power on the affine model

**Independent of A–C. Needs explicit authorisation before starting.**

Touches all three sites in S-2, together. Sentries that will move:

- `power_ceiling_parity_f2.rs`
- `suggest_power_ceiling_after_pass9_g_suggest_powerstale.rs`
- `_litmatrix_rpm_only_lut_chipload.rs`
- `_litmatrix_milling_rpm_diameter_tier.rs`
- `_litmatrix_rubbing_floor_clamp.rs`
- `feed_modulation`'s power-cap tests

Before starting, settle two questions `ADVICE.md` §6 leaves open:

1. **The duty-cycle factor `z·ψ/2π`.** A derivation, not a measurement.
   Check it against per-move engagement in a real cut trace first — the
   simulation already computes engagement, so the check is cheap.
2. **`GRAIN_ANISOTROPY_FACTOR = 2.0`.** Power applies it; the `force.rs`
   fit does not. Read `planning/UNIFIED_LOAD_MODEL_2026-06-18.md` §6 and
   `KC_MILLING_CALIBRATION_2026-06-17.md` before dropping it.

---

## A constraint on `ui/feeds/`, learned during D-3

Any new `.rs` file added **directly under `crates/rs_cam_viz/src/ui/feeds/`**
needs three things, or existing sentries fail:

1. an entry in `OPERATION_SCOPE_FILES` in
   `tests/the_feeds_modal_holds_one_scope_dc5a.rs` (six entries since D-3);
2. an `include_str!` in `FEEDS_MODAL_SRCS` in `tests/apply_contract_a3.rs`;
3. nothing project-scope inside it — that directory is per-operation.

Item 2 is enforced automatically: the `the_apply_contract_scans_every_feeds_surface`
arm walks the directory and asserts the apply contract names every file it
finds. A file missing from `FEEDS_MODAL_SRCS` is a hole in the apply
contract, which is why the sentry checks rather than trusts.

Phases A–C edit existing files and should not trip this. It is recorded for
whatever comes after them.

## New sentries

| Sentry | Pins |
|---|---|
| `cut_efficiency_is_closed_form_g_specenergy` (core) | `u` matches the closed form at three chiploads; `u` is invariant under RPM and under DOC — the two cancellations that make it an efficiency figure. Non-vacuity: `u` must *change* with `fz` and with `ae`. |
| `efficiency_abstains_without_kc_g_specenergy` (core) | A material with no primary-source Kc yields `None`, and the row renders an abstention rather than a zero. |
| `the_chipload_verdict_is_one_row_g_chipverdict` (viz) | The Feeds tab paints exactly one verdict row; it changes state across thin / in-band / heavy fixtures; no power `ProgressBar` returns. |
| `the_corridor_bounds_the_band_g_corridor` (viz) | Rendered: the floor wedge sits below the band and the ceiling wedge above it. Non-vacuity: with the ceiling `None`, only one wedge is drawn. |
| `the_speeds_apply_holds_the_cut_g_speedsonly` (viz) | Driving `Match vendor chipload` changes feed/plunge/RPM and leaves DOC and WOC **byte-identical**. |

The one that matters most is `efficiency_abstains_without_kc`. Every other
arm can be satisfied by a surface that quietly substitutes a default.

---

## Explicitly not built

Each refused on a measurement, recorded so the decision is not re-litigated
from taste:

| Not building | Measurement |
|---|---|
| Power bar / gauge | Peak utilisation 23.6 % across the whole shipped matrix; typical 1 % (`feeds/mod.rs:1714`). |
| Power contour on the chart | Feed at 0.6 kW is ~10⁵ mm/min against a 4 000 mm/min cap — two orders of magnitude off the axis. |
| Surface-speed readout | 5.3 m/s at 17 000 RPM, 7.5 m/s at the 24 000 ceiling; an order of magnitude under the carbide-in-wood optimum, so it cannot become the constraint. |
| Tool life in hours or metres | No wear model, no bench data. Taylor exponents for carbide in wood vary by an order of magnitude. A fabricated constant is worse than the engine's current honest silence. |

---

## Order of work

`REVIEW.md` adds three preconditions. All three are removals or moves, none
changes behaviour, and together they cost less than Phase A. D-1 in
particular must come first: Phase A edits the consumers of the file it
cleans.

```
D-1  delete FeedsExplain::sibling_rows + rows_by_diameter/rows_by_hardness
       — zero readers since today's chart deletion, and populating it scores
         and sorts 256 LUT observations on every frame the Feeds tab draws
D-2  rename explain.rs → explain_payload.rs, explanation.rs → feed_explanation.rs
       — FeedsExplain vs FeedExplanation, one letter apart, both alive
D-3  move the Explore window out of ui/feeds/mod.rs into ui/feeds/window.rs;
       mod.rs becomes a module root whose doc is a map
─────────────────────────────────────
N-2  extract the deflection cap        ← pure refactor, existing test must not move
N-1  feeds::efficiency + 2 core sentries
A    verdict row + sentry              ← smallest shippable increment of value
B    corridor + sentry
C    Match vendor chipload + sentry    ← no core work
─────────────────────────────────────
R1   power on the affine model         ← separate, authorised, re-baselined
D    aggressiveness dial               ← re-evaluate after A–C
```

Stop after A if it does not read well. Everything after it assumes the
verdict row earned its place.

---

## R1 shipped — power on the affine model (2026-09-16)

`tool_load/power.rs` now builds power from the same `(Ks, F_edge)` pair
`feeds/force.rs` owns:

```text
P_kW = A · ( Ks · MRR  +  F_edge · ap · Vc · z·ψ/2π ) / 60e6
```

`A = GRAIN_ANISOTROPY_FACTOR = 2.0` rides **both** terms. ADVICE §6's
correction is honoured: the factor is power's safety allowance for
transient grain spikes, deflection keeps raw `Kc`, and R1 changed the
model's SHAPE, not its safety scoping.

### The three sites, closed together

| Site | Before | After |
|---|---|---|
| `tool_load/power.rs` `predicted_power_kw` | linear helper | `PowerTerms::of` + `kw_at_feed` + `feed_for_kw` |
| `feeds/mod.rs:1728, :1950` | called the helper | call `power_model_terms` (one assembly point) |
| `feed_modulation.rs` power-cap solver | **its own copy** of the formula | calls `PowerTerms::feed_for_kw` |

T-6's last open bullet is closed: there is no second implementation left.

### Measured before/after — ADVICE §1's reference fixture

6 mm 2-flute flat, 17 000 RPM, DOC 4.20, WOC 2.10, generic softwood
(`Kc = 17.55`, `Ks = 24.975`, `F_edge = 2.650`, ψ = 1.2661 rad). Taken
from live code; pinned in `tool_load::power::r1_two_term_model`.

| fz (mm/tooth) | MRR | pre-R1 kW | R1 kW | ratio | edge share |
|---|---|---|---|---|---|
| 0.0380 (running) | 11 395 | 0.006666 | 0.057399 | **8.61×** | 83.5 % |
| 0.0675 (vendor mid) | 20 242 | 0.011842 | 0.064763 | 5.47× | 74.0 % |
| 0.0850 (vendor max) | 25 490 | 0.014912 | 0.069132 | 4.64× | 69.3 % |

8.61× confirms §6's corrected estimate of "roughly 8.6×". §1's table read
4.3× / 2.7× / 2.3× because it computed the two-term column without `A`.
Edge shares match §1 to the printed digit (83 % / 74 % / 69 %).

The edge floor on this fixture is **0.0479 kW at any feed**. Halving the
feed cuts total power by 11 %, not 50 %.

### The duty-cycle factor was checked, not assumed

`z·ψ/2π` is the same expression `tool::flat_chip_geometry_for_radius`
already computes as `arc_engagement_radians / flute_pitch`
(`flute_pitch = 2π/z`), and `ψ` is the one immersion angle
`force::immersion_angle`, `feed_modulation`, `chipload_cap_for_deflection`
and `dexel_stock::stamping` all share. The existing expression carries a
helix-wrap term this model does not — logged as **T-9's sibling T-7** in
`planning/TECH_DEBT_REGISTER.md`, deliberately out of scope because adding
it is a second feed-moving recalibration.

### Sentries re-baselined — numbers, not assertions

| Sentry | Before | After |
|---|---|---|
| `power.rs` `light_cut_is_within_with_available_kw` | peak < 0.01 kW | peak < 0.1 kW (measured 0.04554) |
| `power.rs` `vbit_triangular_cross_section_halves_power_vs_flat` | 0.00072 kW, renamed `..._halves_the_shear_term` | 0.013320 kW, and the halving claim now pins the shear term, which is what the cross-section actually governs |
| `power.rs` `heavy_cut_within_with_power_breach_tolerance` | slot 14 mm @ 6000 (0.77 kW) | slot 7 mm @ 3000 (0.82 kW); the Exceeds-at-default arm added so "borderline" is asserted at both ends |
| `power_ceiling_parity_f2` `the_power_ceiling_does_not_bind_on_shipped_presets` | 0 fixtures, peak 23.6 % | renamed `the_power_ceiling_binds_on_three_shipped_fixtures`; peak **80.0 %**, three fixtures pinned by name |
| `power_ceiling_parity_f2` `a_power_limited_feed_lands_exactly_on_the_gate_ceiling` | synthetic 0.05 kW | synthetic **0.36 kW**; assertion (100 % ± 2 %) unchanged and still exact |
| `suggest_power_ceiling_after_pass9` local power model | pre-R1 linear copy | two-term copy |
| `suggest_power_ceiling_after_pass9` synthetic | 0.05 kW | **0.58 kW** |
| `suggest_power_ceiling_after_pass9` `shipped_presets_stay_clear...` | `worst <= 1.0` | `worst <= 1.002` — one whole-mm/min feed-rounding step, cause measured (T-9) |

Also fixed, not re-baselined: `shipped_utilisation` in that file multiplied
`required` by `safety_factor` while the ceiling already carried it, so every
figure it printed was 25 % low. Removing it **raises** every number there.

Named in the brief but **unmoved**: `_litmatrix_rpm_only_lut_chipload`,
`_litmatrix_milling_rpm_diameter_tier`, `_litmatrix_rubbing_floor_clamp`,
`constrained_max_modulation_f039`. No assertion in any sentry was loosened
except the rounding allowance above, which is stated and 15× the measured
overshoot.

### Step 6's clamp changed axis, not strength

Pre-R1, clamping `raw_feed` against the unfactored `power_at_rpm` and letting
Step 9 scale by `safety_factor` landed the commanded power exactly on the
gate ceiling — linearity did that for free. The edge term carries no feed, so
`P(sf·f) ≠ sf·P(f)` and the composition is now written out: Step 6 caps
`raw_feed` so `sf · raw_feed` draws no more than `power_at_rpm · sf`. With no
edge term the two expressions are algebraically identical.

New refusal: when the feed-free edge term alone exceeds the budget,
`PowerTerms::feed_for_kw` returns `None` and the calculator leaves the feed
alone rather than serving a fabricated one (a zero, or the floor). The
warning carries the conflict. This is the power-side twin of
`DeflectionCapRefusal::EdgeForceOverBudget`.

### One sentry is left RED, deliberately

`tests/literature_matrix` cell **`bull_12mm_pocket_oak`**: `moderate` →
**`major`**.

| | master | R1 |
|---|---|---|
| feed | 3000 mm/min | 1200 mm/min |
| fpt | 0.0625 mm/tooth | **0.0250** (at the rubbing-floor clamp) |
| power | 0.131 kW | 0.649 kW |
| verdict | moderate (`fpt` −0.8 % under band) | **major** (`anti.bull_misclassified_as_ball_chipload`, `fpt < 0.030`) |

The model is not wrong here. Free-run engagement on that cell is ap 8.400 ×
ae 4.200 with four flutes at 9 000 RPM, where the edge term alone is
0.431 kW against a 0.450 kW budget — 96 % of it. Step 6 therefore derates
the feed 17× (`power_limit: 0.058`), Step 9b clamps the chipload back up to
the 0.025 floor, and the shipped recipe is **both** rubbing-adjacent and
still over the ceiling at 0.487 kW.

That is `ADVICE.md` §3's "wrong-ish" derate direction, become live: only the
shear term responds to feed, so thinning the chip cannot buy the power back
and it costs specific energy on the way. **The fix is R4**, which traverses
the constant-chipload line by dropping RPM — cutting both terms — and which
this brief did not authorise. The anti-pattern was not weakened and the cell
was not re-pinned. Recorded with its measurements as **T-8** in
`planning/TECH_DEBT_REGISTER.md`.

R4 is now a prerequisite for R1's gate to go green, not an optional
follow-up.
