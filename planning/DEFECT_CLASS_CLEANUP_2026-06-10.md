# Defect-class cleanup — tool_load / feeds / optimize pipelines

**Started:** 2026-06-10
**Branch:** `defect-class/cleanup`
**Origin:** Live optimization run on WANAKA (2026-06-09) surfaced four issues; root-cause
investigation found they are instances of five systemic defect classes, not isolated bugs.
Full investigation reports are summarized per-class below.

**Method:** fix the four confirmed instances now (Tracks F1–F4), while audit agents sweep
each defect class across all op families / data files to find further instances (Tracks A1–A4).
Every fix lands with a regression test; every audit finding gets triaged into this doc.
Validation bar: industry norms already in the repo's literature net (FPL Wood Handbook,
Onsrud drill bulletins, Vectric defaults, Amana published chiploads — see
`crates/rs_cam_core/tests/literature_matrix/`).

---

## The five defect classes

| # | Class | Confirmed instance | Generalizing audit |
|---|-------|--------------------|--------------------|
| C1 | Bad data wearing a "validated" badge — LUT rows pass the loader with degenerate/incomplete content | `whiteside_fusion360.json` tapered-ball rows with `chipload_min == chipload_max == 0.1016` (a Fusion360 nominal preset encoded as a range) win lookups and hard-block the optimizer with `Confidence::Validated` | A1 |
| C2 | Write-path clobbers — per-op setter aliases + ordered field application silently overwrite earlier writes | `DrillConfig`/`AlignmentPinDrillConfig` alias `set_feed_rate` and `set_plunge_rate` onto one field; `apply_feeds_subset` writes plunge last, clobbering the drill-tuned feed with the milling plunge baseline | A2 |
| C3 | Untagged samples polluting gates — entry/link/transit moves without span tags count as steady state | adaptive3d entry/re-entry sample with ~3× commanded axial DOC at slot arc drives a 622 µm deflection verdict (honest reading ≈ 98 µm) → spurious `DeflectionSetupLocked` | A3 |
| C4 | Signals plumbed but dead — fields computed and serialized but never consulted at the decision point | `Confidence`/`ChipBoundsSource` never read by `candidate_is_safe`; `pass_role` ignored by deflection gate despite rough/finish limits existing in `cutter_constraints`; `AxisPatch.clamped` not serialized onto candidates | A4 |
| C5 | Duplicated evaluation that drifts — same judgement implemented ≥2× with different inputs | 3 LUT row resolvers (gate / optimizer context / retargeter); 2 bending sections (solid-D gate vs 0.7·D-core Suggest, 4.16× apart); narrate re-derives drill envelope check instead of consuming `DrillGatesVerdict` | A4 |

Cross-cutting (tracked but not a class of its own):
- One `MachineProfile::max_feed_mm_min` field conflates GRBL travel rate ($110) with cutting-feed ceiling → optimizer can propose cutting at travel speed (Track F4).
- `mem::replace` placeholder session during optimize runs makes MCP inspectors report default state (observability hole; deferred — see Backlog).
- `RefuseReason` conflation: cancellation surfaces as `NoSafeImprovement`, `SteadyStateSamplesNotPresent` covers 3 unrelated situations (Backlog).

---

## Fix tracks

### F1 — Drill feed clobber + floor-grazing defaults  — STATUS: in progress

Confirmed behaviour: both WANAKA drill TPs run at exactly 50.0 feed/Ø — the SolidWood
rubbing floor. Literature band for hardwood drills is 0.08–0.18 mm/tooth ≈ 213–427 feed/Ø
(`literature_matrix/cells.toml:415-440`, sources: onsrud_drill / vectric_drill_default /
fpl_wood_handbook).

Sub-fixes:
1. Stop the clobber: `feeds::calculate` sets `plunge_rate_mm_min = feed_rate_mm_min` for
   `OperationFamily::Drill` so the aliased setters are write-order-safe
   (`crates/rs_cam_core/src/feeds/mod.rs`, `suggest.rs:727-728`).
2. Envelope sanity clamp on suggested drill feed: pass the chipload-derived feed through
   `material.drill_plunge_feed_envelope_per_mm() × diameter`; emit `FeedsWarning` when it
   binds (same convention as `RUBBING_FLOOR_MM_TOOTH`).
3. Kill hardcoded `feed_rate: 300.0` defaults (`operation_configs.rs:152,199`): route the
   pin-drill auto-create (`rs_cam_viz/controller/events/model.rs:529-552`) through the
   suggest funnel like GUI/MCP `add_toolpath` already does.
4. Chip-welding gate credits pecking: classify Peck/ChipBreak cycles on per-peck D/d
   (already computed for `peck_pattern_adequate`), not total-hole D/d
   (`drill_metrics.rs:261-270`). Add edge test at exactly 0.75×; document band as `[0.75t, t)`.
5. Reporting hygiene: split `DrillGateOutcome::Within.threshold` into `envelope_lo`/`envelope_hi`
   (`drill_gates.rs:24-38`); narrate consumes `DrillGatesVerdict` instead of re-deriving
   (`narrate.rs:1043-1065`); unify unit string to "mm/min per mm Ø"; add source citations
   to `material.rs:1105-1122`.
6. Lit-matrix invariant on **final op feed** implied chipload (`feed_rate / (rpm × flutes)`)
   at Major severity, so a clobber regression fails `flat_3mm/6mm_drill_*` cells
   (today the clobber is invisible to the matrix — plunge-fraction row reads 1.0 by construction).

Validation: drill suggest feeds land in the literature band for the cells.toml drill cells;
WANAKA pin drill + holes re-suggest off the floor.

### F2 — adaptive3d phantom samples inflating deflection — STATUS: pending

Confirmed behaviour: TP1 Back Rough peak sample carries ~9 mm axial engagement at slot arc
(commanded DOC 3.0) → 622 µm verdict → preflight `DeflectionSetupLocked`. Honest reading
≈ 98 µm (Within). Same family as F-024.

Sub-fixes:
1. Clamp/filter: in the deflection gate, exclude or clamp per-sample `axial_engagement_mm`
   to commanded depth-per-pass + tolerance, OR extend span tagging so adaptive3d
   entry/re-entry moves carry `Entry` ancestry and hit the existing advisory reroute
   (`deflection.rs:218-227`, `locality.rs:180-225`). Prefer tagging at emission —
   it fixes every gate at once, not just deflection.
2. Preflight computes instead of asserts: evaluate the closed-form model at the
   minimum-force corner of the search space before refusing (`preflight.rs:37-60`);
   dispatch per-gate retargeters on ANY Exceeds gate (today only chipload-Exceeds
   dispatches; `DeflectionDocRetargeter` is dead code — `optimize/mod.rs:297-308`).
3. Reconcile prescription: refusal targets 50 µm while citing the 200 µm limit
   (`refusal.rs:27,35`); fix stale L/D>6 prose (`tool_load/mod.rs:166-168`); promote
   peak_um / bound_um / target_stickout_mm to structured fields.

Out of scope (deliberately): recalibrating the deflection model itself. The 50/200 µm
bounds + un-multiplied shear-parallel Kc + solid-D section are co-tuned into a working
stiffness index. Touching one constant alone breaks discrimination. Needs its own
calibration effort with real measurements — see Backlog.

### F3 — degenerate Whiteside LUT rows hard-blocking the optimizer — STATUS: pending

Sub-fixes:
1. Demote the four `min==max` rows in `whiteside_fusion360.json`: drop their
   `chipload_min_mm_tooth` (gate already handles min-less rows honestly: "missing lower
   bound means burn cannot be modeled, not that we invent one" — `chipload.rs:376-391`).
   Keep them as RPM/nominal references.
2. Loader validation in `feeds/vendor_lut.rs`: reject `min == max` (or width < threshold
   fraction of midpoint) as a *range*; test that all embedded observation files pass.
3. Provenance-aware gating: extend `ChipBoundsSource` (point-preset / scaled / missing-ae);
   when the LOW side would trip on weak-provenance bounds, return `Within` + structured
   burn advisory instead of `Exceeds(Low)`. High side stays hard (breakage). Existing
   `MarginalSafe` tier is the natural landing zone for such candidates.
4. Unify LUT resolution: one shared `matched_chip_envelope(...)` helper consumed by the
   gate (`chipload.rs:334-360`), optimizer context (`optimize/context.rs:109-142`),
   preflight bipolar check (`preflight.rs:62-80`), and retargeter (`optimize/mod.rs:474-485`).
   Kills the "advice computed against a different envelope than the verdict" class
   (the 38198 mm/min narrative).

Validation: TP7-style drop_cutter finish candidate (structurally safe, 4.5× faster) is no
longer refused on burn-side chipload from preset-point bounds; Amana tapered rows
(real ranges 0.010–0.032 + ae calibration) win tapered-ball lookups.

### F4 — travel rate vs cutting-feed ceiling conflation — STATUS: pending

Sub-fixes:
1. Split `MachineProfile`: `max_feed_mm_min` (travel, $110-class — keep name for serde
   compat) + new `max_cutting_feed_mm_min: Option<f64>` (optimizer/suggest/modulation
   ceiling; None → derive conservative default, e.g. min(travel, preset cap)).
2. Plumb through `optimize/bounds.rs:227`, `strategy/headroom.rs:76`, retargeter clamps,
   and suggest.
3. `OptimizeOutcome` carries a `machine_snapshot` (name + caps used) so narratives are
   reconcilable with what the run actually consumed.
4. Surface `machine_library::resolve` silent library-override with a warning
   (`session/project_file.rs:992`).

Validation: ricky-XXL profile (travel 10000) can't be told to cut hardwood at 10000;
machine_kinematics.md guidance updated.

---

## Audit tracks (parallel agents, read-only)

| Track | Scope | Status | Findings |
|-------|-------|--------|----------|
| A1 | All `data/vendor_lut/observations/*.json`: degenerate ranges, missing ae/hardness, provenance gaps, cross-check against published vendor values | running | — |
| A2 | All 22 op configs: setter alias maps, `apply_feeds_subset` order dependencies, suggest→config round-trip integrity | running | — |
| A3 | Per kinematics class: which emitted move kinds reach steady-state filters untagged; sweep param_sweep fixtures for peak-sample axial > 1.5× commanded DPP | running | — |
| A4 | Dead-signal sweep (set-but-never-read fields in tool_load/feeds/optimize) + duplicated-evaluation inventory | running | — |

Findings get appended below as they come in, triaged into: fix-now (joins F-tracks),
backlog, or working-as-intended.

### A1 findings

(pending)

### A2 findings

(pending)

### A3 findings

(pending)

### A4 findings

(pending)

---

## Backlog (acknowledged, deliberately deferred)

- Deflection model recalibration: force model is feed-independent (`F = Kc·ap·ae`,
  drag-cut physics, ~16–60× energy-balance force at WANAKA params), offset by co-tuned
  constants. Needs physical measurements or a literature-anchored recalibration as one
  coordinated change. Until then: document the metric as a tuned stiffness index.
- `RefuseReason` enum split: cancellation ≠ `NoSafeImprovement`; `SteadyStateSamplesNotPresent`
  covers three unrelated situations (drill-kinematics skip, missing cycle time, genuine
  no-steady-state). Engraving-aware steady-state mode for shallow ops (project_curve at
  0.1 mm DOC) so they get cycle-time-only optimization.
- Placeholder-session swap (`controller/events/mod.rs:427,984`): MCP inspectors read an
  empty default session during optimize runs. Fix: read-only snapshot or "optimize in
  flight" response.
- Baseline/candidate verdict asymmetry: baseline scored from production trace, candidates
  from optimizer-internal sims with modulation off + coarser Stage-1 resolution → phantom
  Improved/Worsened near thresholds.
- Magic numbers outside `SearchPolicy`: rank penalties (`rank.rs:77,96,113`), suggestion
  margins, `STEADY_STATE_FEED_FRACTION`, duplicated deflection headroom 0.75, hardcoded
  `project_default_rpm: 18_000`.
- Headroom strategy on discrete spindles: RPM snap breaks constant-chipload premise while
  rationale claims uniform scaling (`headroom.rs:157-177`).

## Decision log

- 2026-06-10: branch from master (ia-cleanup viz deletions stashed:
  `git stash list` → "ia-cleanup dead-code-sweep viz deletions").
- 2026-06-10: fix order F1 → F2 → F3 → F4 chosen by real-world payoff: drills currently
  cut at the rubbing floor (burn risk on every default drill op); F2 converts a locked
  toolpath to optimizable; F3 unblocks a 4.5× cycle-time win; F4 makes the unblocked
  win safe to apply.
