# Suggest / Sim / Optimize acceptance framework

Date: 2026-05-23

## Purpose

This is the written target for answering: **is `rs_cam` working?**

The first deliverable is a source-backed acceptance dataset that lets us run a broad sweep and ask, per toolpath:

1. **Suggest** — did the first-shot settings land in a sane, externally defensible envelope?
2. **Sim** — did simulation and tool-load gates classify the cut correctly?
3. **Optimize** — did optimization improve cycle time honestly without hiding a load, collision, or finish-quality regression?

Seed data lives in:

- `planning/toolpath_acceptance/toolpath_catalog.csv` — all 23 `OperationType` variants and their acceptance gates.
- `planning/toolpath_acceptance/seed_goal_matrix.csv` — 299 initial goal rows from bundled vendor LUT observations, community chipload charts, ball-nose finish rules, and drill-native thresholds.

## Headline acceptance bars

These are intentionally stage-specific. A single chipload number is useful, but it is not enough for finishing quality, drill cycles, or collision safety.

| Stage | Metric | Initial target | Promotion target | Failure definition |
|---|---:|---:|---:|---|
| Suggest | first-shot landing rate | >= 90% of covered cells | >= 95% after LUT expansion | Suggested feed/RPM/DOC/stepover starts outside the applicable goal row without a machine-cap or geometry explanation |
| Sim | calibration agreement | >= 95% per gate on labeled sweep rows | >= 98% for grade-A/vendor rows | Sim reports `Within` for a row intentionally outside the bound, or `Exceeds` for an in-band row without a physical gate explaining it |
| Optimize | honest improvement rate | >= 95% of `Ranked` recommendations | >= 98% once quality gates are modeled | Recommended candidate is faster but worsens any applicable load, collision, drill, or surface-quality gate |
| Optimize | refusal correctness | 100% for not-applicable ops | 100% | Drill/Z-only ops, locked deflection setups, or unmodeled quality constraints are silently “optimized” |
| Export gate | unsafe G-code block | 100% | 100% | G-code export proceeds for `Exceeds` or unmodeled tool-load without explicit acceptance flag |

## Operation catalog summary

`rs_cam` currently exposes 23 operation kinds in `crates/rs_cam_core/src/compute/catalog.rs`: 22 user-facing machining operations plus `alignment_pin_drill`.

Acceptance families:

| Family | Operation kinds | Primary pass/fail gates |
|---|---|---|
| 2.5D clearing | `face`, `pocket`, `adaptive`, `rest`, `zigzag` | nominal chipload, engagement-corrected chipload, power, deflection, rapid/holder collision, air-cut ceiling |
| 2.5D contour | `profile` | same as clearing, with corner/full-engagement stress emphasized |
| Trace / V operations | `v_carve`, `inlay`, `trace`, `chamfer` | effective-diameter chipload, fit/geometry quality for inlays, collisions |
| Drill cycles | `drill`, `alignment_pin_drill` | drill-native chip-welding, peck adequacy, plunge-feed sanity, collision; milling chipload is n/a |
| 3D rough | `adaptive3d` | chipload, power, deflection, adaptive engagement, collisions, air-cut ceiling |
| 3D finish | `drop_cutter`, `waterline`, `pencil`, `scallop`, `steep_shallow`, `ramp_finish`, `spiral_finish`, `radial_finish`, `horizontal_finish`, `project_curve` | chipload/load gates plus **surface-quality gates**: stepover/scallop/cusp, slope/flatness applicability, collision |

The full row-level catalog, including optimizer surfaces, is in `planning/toolpath_acceptance/toolpath_catalog.csv`.

## Goal matrix structure

`seed_goal_matrix.csv` has one row per acceptance target. It deliberately mixes four kinds of evidence:

1. **Vendor LUT chipload rows** (`goal_type=vendor_lut_chipload`)
   - 67 rows copied from `crates/rs_cam_core/data/vendor_lut/observations/*.json`.
   - Highest-confidence target where operation/tool/material/diameter/flutes match.
   - Suggest should land inside the row; sim should classify in-band/out-of-band sweeps correctly; optimize must not move out of the row unless a higher-confidence row applies.

2. **Community nominal chipload rows** (`goal_type=community_nominal_chipload`)
   - Generic woodworking bands for 1/8, 1/4, 3/8, and 1/2 inch tools, expanded across roughing/clearing ops.
   - These are **nominal** feed-per-tooth targets: `feed / (rpm * flutes)`.
   - They are broad first-shot goals, not a substitute for vendor/tool-specific LUT rows.

3. **Effective-diameter trace/V rows** (`goal_type=community_effective_diameter_chipload`)
   - Lower-confidence rows for `v_carve`, `inlay`, `chamfer`, `trace`, and `project_curve` where the engaged cutter diameter varies with depth.
   - Optimizer should be conservative or refuse if effective diameter is ambiguous.

4. **Non-chipload rows**
   - `surface_finish_stepover_scallop`: final 3D finishing must respect stepover/cusp/scallop bands; load gates alone cannot bless a coarse finish.
   - `drill_native_gates`: Z-only drill operations use peck/chip-evacuation/plunge-feed gates, not milling chipload.

## Chipload semantics

We should record both chipload views in the sweep output:

| Name | Formula / source | Use |
|---|---|---|
| `nominal_fz_mm_tooth` | `feed_rate / (rpm * flutes)` | Compare Suggest and community/vendor chart bands |
| `corrected_chip_mm_tooth` | sim/tool-load engagement-corrected chip thickness | Compare tool-load report bounds and burn/breakage risk |

Do **not** compare corrected sim chip thickness directly to a community nominal chart without labeling the unit shift. The sweep should output both numbers and both verdicts.

## Per-family acceptance rules

### 1. Clearing / roughing

Applicable to `face`, `pocket`, `adaptive`, `rest`, `zigzag`, `profile`, `adaptive3d`.

Pass criteria per sweep row:

- Suggest nominal fz is inside either a matching vendor row or the community fallback row.
- Sim reports chipload/power/deflection `Within`, unless the test row is deliberately outside bounds.
- Rapid collision count is zero.
- Holder/shank collision count is zero.
- Air-cut percentage remains below the op-specific threshold in `OperationType::air_cut_high_threshold_pct()`.
- If machine feed/RPM caps prevent minimum chipload, Suggest/Optimize must surface the cap and suggest the physical knob: larger tool, larger stepover, lower RPM if available, shorter stickout, or different operation.

### 2. 3D finishing

Applicable to `drop_cutter`, `waterline`, `pencil`, `scallop`, `steep_shallow`, `ramp_finish`, `spiral_finish`, `radial_finish`, `horizontal_finish`, `project_curve`.

Pass criteria:

- Load gates are still required: chipload/power/deflection/collision must pass.
- Final-pass quality must also pass:
  - visible final finish target: 5–20% ball diameter stepover where stepover is the user-facing control;
  - semi-finish allowed: 20–35%, but only if the operation is marked semi-finish or another finish follows;
  - >35% is roughing-only and must not be accepted as a final finish optimization.
- If the operation exposes scallop height directly, compare to the target scallop band instead of only stepover.
- Optimize must not report a “safe improvement” by making stepover/scallop coarser unless the requested quality tier allows it.

### 3. Trace / V-bit / effective-diameter cuts

Applicable to `v_carve`, `inlay`, `trace`, `chamfer`, and some `project_curve` cases.

Pass criteria:

- Suggest computes or records effective engaged diameter before applying chipload goals.
- Where effective diameter is unknown, sim can be advisory but optimize should prefer refusal/low-confidence output over a confident recommendation.
- Inlay adds a non-load fit gate: plug/pocket geometry must remain within tolerance.

### 4. Drill cycles

Applicable to `drill` and `alignment_pin_drill`.

Pass criteria:

- Milling chipload and radial engagement are n/a.
- `tool_load_report().drill_gates` are the authority.
- Depth-to-diameter thresholds from the current rs_cam drill model are encoded in the dataset:
  - softwood / softwood plywood: fail around 8D, warn around 6D;
  - hardwood / MDF / hardwood plywood: fail around 5D, warn around 3.75D;
  - plastic: fail around 4D, warn around 3D.
- Optimizer should skip/refuse feed-RPM-DOC optimization for Z-only cycles.

## Sweep design

The first large sweep should produce one result row per `(operation_kind, geometry_fixture, tool_family, diameter, material, goal_row)` case.

Minimum fixtures:

| Fixture class | Existing repo fixture examples | Exercises |
|---|---|---|
| 2D pocket/profile | `fixtures/demo_pocket.svg`, `fixtures/demo_star.svg` | pocket, profile, adaptive, v-carve, trace, drill/chamfer/inlay shapes |
| 3D relief | `fixtures/terrain_small.stl` | adaptive3d and 3D finish family |
| STEP/flat faces | `fixtures/gui_step/*.step` | face-derived 2.5D, horizontal finish, BREP boundary fallback |
| Registration stock | stock alignment pins | `alignment_pin_drill` |

Output columns should include at least:

- identifying dimensions: `operation_kind`, `fixture`, `tool_family`, `diameter_mm`, `flute_count`, `material`, `goal_id`;
- Suggest: feed, rpm, DOC, stepover/scallop, `nominal_fz_mm_tooth`, source row chosen;
- Sim: corrected chipload summary, chipload verdict, power verdict, deflection verdict, drill gates, collision counts, air-cut percentage;
- Optimize: outcome variant, recommended deltas, cycle-time delta, gate regressions, quality-tier regressions;
- Verdict: pass/fail plus reason.

## Source policy

Evidence priority:

1. Grade-A vendor LUT row in `crates/rs_cam_core/data/vendor_lut/observations`.
2. Grade-B vendor/professional rows or formula references already in `CREDITS.md`.
3. Community chipload charts and forum rules of thumb.
4. Internal extrapolation — allowed only if marked as extrapolated and not treated as calibration ground truth.

Known limitations in this seed:

- Community chipload charts are generic woodworking starts; they are not operation-specific and do not model chip thinning.
- V-bit rows are deliberately low confidence because chipload depends on effective engaged diameter.
- Ball-nose finish quality rows cover surface cusp/stepover, not load.
- Drill rows are acceptance thresholds, not a full drill feeds/speeds database.

## Sources

- Bundled vendor LUT manifest: `crates/rs_cam_core/data/vendor_lut/source_manifest.json`.
- Cutter Shop chip-load chart: https://cutter-shop.com/chip-load-chart/
- IDC Woodcraft chipload calculator/chart: https://idcwoodcraft.com/pages/chipload-calculator
- Carbide3D community V-bit / chipload discussion: https://community.carbide3d.com/t/feeds-and-speeds-guide/17048
- GARR radial chip-thinning explanation: https://www.garrtool.com/knowledge-base/chip-thinning/
- Autodesk Adaptive Clearing reference: https://help.autodesk.com/cloudhelp/ENU/Fusion-CAM/files/GUID09E44604-DAD8-47D6-ADC6-C100869DE724.htm
- CutViewer ball-nose cusp/stepover calculator: https://cutviewer.com/tools/stepover-calculator/
- CNC Cookbook deep-hole drilling reference: https://www.cnccookbook.com/deep-hole-drilling/
