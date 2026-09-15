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
