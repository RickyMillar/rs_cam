# Dexel Z-Only Engagement Investigation

**Date:** 2026-05-19
**Trigger:** `planning/WANAKA_ASSESSMENT_2026-05-19.md` documented drill / pin_drill / project_curve TPs reporting near-zero engagement and 80–100 % air-cut despite visibly cutting stock. A separate user report adds: **drill holes do not show up in the simulation preview** at the project's default dexel resolution.
**Question:** Does the tri-dexel simulator structurally fail to register engagement on Z-dominant moves, as `CLAUDE.md` April-2026 caveats claim? And what is the right long-term fix for drilling fidelity overall?
**Scope:** code-grounded analysis with file:line citations. Ready for independent review.

**Revision 2026-05-19 (post-architecture-deep-dive):** Sections 6.E, 6.F, 6.I, 6.J revised after parallel agent investigation surfaced (a) concrete algorithmic gaps in F.a, (b) the right attachment point for `MoveIntent`, (c) the dual-representation invariant for `DrillOp`, (d) a clean scope split for J. Open questions §10.1, §10.6 closed; §10.5 promoted to hard acceptance gate. A new "Step 0" mcp.rs accumulator dedup added — independent of all other steps and shippable immediately. Line-number citations refreshed against current code; some had drifted ~50 lines.

---

## Status

Single source of truth for where this roadmap is. Implementing agents: update the checkbox + the "last touched" line when a step lands or moves to in-progress, and add a normal `planning/PROGRESS.md` entry for the chronological narrative.

| Step | Status | PR / commit | Notes |
|---|---|---|---|
| Step 0 — `mcp.rs` accumulator dedup | ☑ done | `e80614f` | `SpanCutAcc` + `DepthPassAcc` deleted; both routes now use canonical `SummaryAccumulator`. P3 transit-span peak gating now propagates to per-span and per-depth-pass summaries. |
| Step 1 — C + I (MoveIntent + retract reclassification) | ☑ done | `e51ec2d` | `MoveIntent` enum + `Move.intent` field; every in-tree generator emits non-`Unknown` tags; simulator reclassifies `MoveIntent::Retract` Linears as non-cutting; `metrics_not_applicable` now driven by `MoveIntent::Drilling` (op-kind fallback retained for legacy paths); CLAUDE.md "2D SVG engagement always zero" caveat removed. **Path B confirmed:** no kernel-swap bundled here. |
| Step 2 — D + H (Engagement vector + per-kinematics summary) | ☑ done | `d3fd473` | `Engagement` struct + `EngagementDirection` enum on `SimulationCutSample`; legacy `radial_engagement` scalar carried as a derived view (doc-deprecated; deletion follow-up tracked). `KinematicsSummary` block on `SimulationCutSummary` + `SimulationToolpathCutSummary`; `SummaryAccumulator.per_kinematics` switched from BTreeMap to `[KinematicsAccumulator; 5]` after the BTreeMap version regressed aggregation by 50% on 250K samples. MCP per-span + per-depth-pass JSON surfaces gain `per_kinematics` block. Air-cut / low-engagement / average-engagement docstrings updated to name the radial-WOC axis. CLAUDE.md "cylinder-volume engagement" caveat softened to point at `engagement.radial_woc_fraction` + per-kinematics block. |
| Step 3 — E (DrillOp first-class) | ☑ done | PR1: `5d32ae1` + `aff7738`; PR2: _pending commit_ | **Path B confirmed (2026-05-19).** PR1 lands data model, dual-representation invariant, analytical stock removal, and analytic mesh emission. PR2 adds `DrillSample` stream (`drill_metrics.rs`), `DrillToolpathSummary`, drill-specific gates `DrillGatesVerdict { chip_welding, peck_adequacy, plunge_feed }` on `ToolpathLoadVerdict.drill_gates`, narrate-output enrichment, and an in-repo `drill_metrics_pr2.rs` integration test substituting for WANAKA TP0/TP2 revalidation (the WANAKA TOML lives outside the repo). |
| Step 4 — F.a (sub-cell stamping) | ☐ pending | — | Watch the four algorithmic gaps in §6.F revision. |
| Step 5 — J (marching cubes) | ☐ pending | — | Replaces `dexel_stock_to_mesh` only; live preview path stays on heightmap. |

**Last touched:** 2026-05-19 — Step 3 PR2 landed: `DrillSample` per-peck stream + `DrillToolpathSummary` (peck adequacy, chip-welding risk, cycle time, chip-evacuation score) on `SimulationCutTrace.{drill_samples, drill_summaries}`; three drill-specific gates (`chip_welding`, `peck_adequacy`, `plunge_feed`) carried on `ToolpathLoadVerdict.drill_gates`; narrate output enriched with peck count + D/d + risk band; `metrics_not_applicable` docstring clarified to pair with `drill_summary_for(toolpath_id)`. Integration test `drill_metrics_pr2.rs` (build-session + sim + verdict) covers the WANAKA revalidation gate. CLAUDE.md drill-cycle row + thresholds table added. Step 3 closed; Step 4 (sub-cell stamping) is next.

**Per-step acceptance gates** (apply to every step before marking ☑):
- All tests pass; new tests cover the regression-locking surface called out in §9.
- Benchmark delta measured and within budget (§10.5 hard gate: regression > 20%/step requires justification; cumulative > 50% requires user sign-off before Step 4).
- `CLAUDE.md` updated if a caveat documented there is now closed (notably the "2D SVG engagement always zero" caveat, which should be removed at Step 1).
- Entry added to `planning/PROGRESS.md` following project convention.

---

---

## TL;DR

The user-facing symptoms are real but the structural cause documented in `CLAUDE.md` is wrong. The dexel simulator does *not* have a "Z-only blindness" bug — `stamp_segment_with_metrics` (`crates/rs_cam_core/src/dexel_stock/stamping.rs:212-265`) has a dedicated degenerate-segment branch that stamps pure-Z plunges and reports `radial_engagement = 1.0` whenever material is removed. What is *actually* wrong is a stack of four separate defects:

1. **Category error.** `radial_engagement` is the cylinder-side WOC fraction. It is the wrong metric for end-cutting tools (drills, drill cycles). Reporting `1.0` for a drill plunge is technically correct but semantically meaningless.
2. **Retract-feed inflation.** Pure-Z `Linear` retracts through already-cleared space are tagged `is_cutting = true` with `radial = 0`. They inflate `air_cut_time_s` on plunge-retract-loop ops (project_curve, v_carve, drill cycles).
3. **Cell-center binary stamping.** `stamp_point_on_grid` (`stamping.rs:67-90`) uses a binary inside-radius test at each cell center. Features narrower than ~5× the dexel cell size are stairstepped, under-represented, or lost. The "drill holes don't show" symptom is mostly this.
4. **Heightmap-flavored mesh extraction.** `z_grid_to_solid_mesh` (`dexel_mesh.rs:175`) only emits explicit walls at material/empty cell boundaries — i.e. through-holes. Blind features (drill holes, pockets) get tilted-quad approximations of their walls instead of true vertical geometry, which combined with (3) makes small blind features visually indistinguishable from surface noise.

The fixes split cleanly into two tiers:

- **"Things we should fix"** (3 options, ~1 week total) — reporting and viz changes that stop misleading the operator without changing the underlying model.
- **"Implementation gaps"** (5 options, ~3–4 weeks total) — structural changes to the data model, the simulator's classification, the engagement representation, the stock mesh extraction, and the treatment of drilling as a first-class operation kind.

The recommended sequence threads through both tiers in dependency order. The destination is a CAM simulator competitive with commercial systems on every fidelity axis that matters for 3-axis wood routing.

---

# Part I — What is actually wrong (verified findings)

## 1. The alleged "Z-only blindness" is not in the code

### 1.1 The degenerate-segment branch exists and runs

`stamp_segment_with_metrics` (`crates/rs_cam_core/src/dexel_stock/stamping.rs:189-265`) is the single function that produces the per-sample `(axial_doc_mm, radial_engagement, arc_engagement_radians, removed_volume)` tuple consumed by the metric accumulator. It checks the *planar* (UV-projected) segment length and, when zero, takes a dedicated branch:

```rust
// stamping.rs:205-265
let seg_len_sq = seg_du * seg_du + seg_dv * seg_dv;
if seg_len_sq < 1e-20 {
    let d = sd.min(ed);
    let descent = (sd - ed).abs();
    …
    for row in row_lo..=row_hi { for col in col_lo..=col_hi {
        if let Some(h) = lut.height_at_dist_sq(dist_sq) {
            …
            if from_high {
                let surface = (d + h) as f32;
                let above = ray_material_length_above(ray, surface) as f64;
                ray_subtract_above(ray, surface);
                removed_volume += above * cell_area;
            } else { … }
        }
    }}
    let radial = if removed_volume > 1e-9 { 1.0 } else { 0.0 };
    return (descent, radial, None, removed_volume);
}
```

Three consequences:

1. **A pure-Z plunge that bites fresh material reports `radial_engagement = 1.0`** (`stamping.rs:263`).
2. **`axial_doc_mm` is the segment's Z descent** (`stamping.rs:264`).
3. **`arc_engagement_radians = None`** for Z-only plunges, by design. There is no engagement arc to bin.

### 1.2 The caller dispatches plunges to this branch

`capture_cutting_segment` (`dexel_stock/simulation.rs:360-446`) is invoked for every `MoveType::Linear { feed_rate }`. It splits the segment into sub-segments of length ≤ `sample_step_mm` (`simulation.rs:380`), and for each sub-segment calls `estimate_and_stamp_cutting_subsegment` (`simulation.rs:393-402`), which calls `stamp_segment_with_metrics` after decomposing world XYZ into the cut-direction's `(u, v, depth)` axes (`simulation.rs:459-463`).

For `StockCutDirection::FromTop` the decomposition is identity on XY: a pure-vertical plunge has `seg_du = seg_dv = 0` → `seg_len_sq = 0 < 1e-20` → degenerate branch fires. The kinematic classifier `classify_cut_kinematics` (`simulation.rs:510-525`) tags the same condition as `CutKinematics::Plunge`, surfaced on `SimulationCutSample.cut_kinematics` (`simulation.rs:425`). `is_cutting` is hard-coded `true` for every Linear sub-segment regardless of material removal (`simulation.rs:424`).

### 1.3 The accumulator weights plunges the same as lateral cuts

`SummaryAccumulator::observe` in `simulation_cut.rs:531-563` adds `radial_engagement * segment_time_s` to a time-weighted sum unconditionally for `is_cutting = true` samples. A degenerate-Z sample with `radial = 1.0` contributes exactly the same way a half-immersion lateral cut at `radial = 0.5` would.

### 1.4 What WANAKA's "n/a" actually represents

The assessment reports drill engagement as `n/a` (e.g. TP0 row, `WANAKA_ASSESSMENT_2026-05-19.md:68`). That is **a judgment call by the author** — not the simulator returning n/a. The simulator returns a real number, typically averaging 0.3–0.7 across a peck cycle (whatever fraction of sub-segments crosses fresh material vs already-cleared bore). The author rounded to "n/a" because the value is meaningless for end-cutting kinematics, not because the simulator failed to produce one. The `CLAUDE.md` "Z-only blindness" claim is therefore **wrong about the mechanism**.

## 2. The tri-dexel abstraction (descriptive)

`TriDexelStock` (`dexel_stock/mod.rs:25-30`) is three orthogonal `DexelGrid`s — Z always present, X / Y lazily created (`mod.rs:80-92`). Each grid is a 2D array of rays; each ray is a stack of material segments along the grid's ray axis. For a `FromTop` cut the cutter footprint is projected onto the Z-grid's XY plane and each cell within the cutter radius gets a ray-trim along its Z column. `stamp_point_on_grid` (`stamping.rs:44-92`) walks cells inside the cutter's bounding box, consults the radial-profile LUT for the cutter's bottom-surface height `h` at each cell's `(du, dv)` offset, and calls `ray_subtract_above` (or `_below`) at `tip_depth + h`. The Z-column representation lets the simulator distinguish "ray top is at z=2" (uncut stock) from "ray top is at z=-3" (cut into a hole), giving an exact volumetric truth per cell — *at the cell-center sampling resolution*.

The side-engagement metric in the non-degenerate path (`stamping.rs:267-423`) measures *width of cut perpendicular to motion*: it bins cells whose pre-stamp ray top was meaningfully above the cutter surface (`pre_fresh > FRESH_MATERIAL_THRESHOLD_MM = 0.05`, `stamping.rs:299`), projects them onto the perpendicular-to-bearing axis, takes `perp_max − perp_min`, divides by `2R`. This is the "RWoC / D" engagement convention. It is well-defined whenever segments have a lateral component. The degenerate branch is the explicit fallback when they don't.

For 2D ops, the F-2 fix (`compute/simulate.rs:711-859`, commit 12dca81) reshaped `StockConfig::update_from_bbox` (`compute/stock_config.rs:199-213`) so a zero-Z-range polygon model produces stock at `[bbox.min.z − z, bbox.min.z]` rather than `[bbox.min.z, bbox.min.z + z]`. The regression test `two_d_pocket_simulation_reports_engagement` asserts `average_engagement > 0` for a polygon pocket. **The `CLAUDE.md` April-2026 caveat that "engagement is unusable for 2D SVG operations" is stale** — recommend removing it from `CLAUDE.md` once Option C lands.

## 3. The real defects

### 3.1 Engagement-as-radial-WOC is a category error for end-cutting tools

`radial_engagement` is defined as (perp-extent of fresh-material-above-cutter cells) / (2R) — the cylinder-side WOC fraction. For a drill plunging into fresh stock, every cell in the disk has material above → `radial = 1.0`, which is "100% radial engagement" — but a drill is end-cutting, not side-cutting. The relevant metric is axial chipload (mm/rev or mm/tooth-rev where flute-count=1 for drills, 2 for end-mills-as-drills). Reporting cylinder-WOC for a drill is like reporting tire pressure for a sailing boat — technically a number, semantically not the right question.

### 3.2 Retract-feed inflation

`drill_toolpath` and similar generators emit `Linear` feed moves for plunges, and `Rapid` moves for retracts (`drill.rs:55-66, 97-106, 131-141`). That's correct for drill cycles. **But** `project_curve`, `v_carve`, `trace`, and similar plunge-and-retract-loop ops emit `Linear` feeds for the retract too (the lift-off-from-cut motion runs at `plunge_rate`, not rapid, for tool safety). The simulator's `capture_cutting_segment` tags every Linear sub-segment as `is_cutting = true` (`simulation.rs:424`). For a retract that travels pure-Z upward through already-cleared space:

- The degenerate branch fires with `d = sd.min(ed) = sd` (the lower end, the starting point of the upward move).
- The cells in the disk have ray top already at `sd` (already cut on the way down).
- `above = ray_material_length_above(ray, sd + h) ≈ 0` → `removed_volume ≈ 0` → `radial = 0`.
- The sub-segment is reported with `is_cutting = true`, `radial = 0`, `segment_time_s > 0`.
- The accumulator adds this to `air_cut_time_s` (`simulation_cut.rs:485-488`, threshold `radial < 0.02`).

For a sparse plunge-cut-retract pattern (WANAKA TP3 Rivers: 35 Z-levels, many small disjoint segments), retract time dominates the `is_cutting`-tagged sample stream and the op reads as ~92 % air-cut. The cutting itself is fine; the metric is fooled by a classification accident.

### 3.3 Cell-center binary stamping under-represents small features

`stamp_point_on_grid` (`stamping.rs:67-90`) tests `dist_sq < r_sq` at each cell center. A cell is either fully inside the cutter (gets stamped at full depth) or fully outside (untouched). At 0.5 mm dexel resolution:

| Drill Ø | Cells in disk | Visual outcome |
|---|---|---|
| Ø6 mm | ~113 | Clean circular depression, visibly a hole |
| Ø3 mm | ~28 | Roughly circular, stairstep edges visible |
| Ø2 mm | ~12 | Lumpy patch, edges dominate, barely "circular" |
| Ø1 mm | ~3–4 | A few cells dropped a tiny amount; not recognisable as a hole |
| Ø0.5 mm | 0–1 | May miss the cell-center test entirely and stamp nothing |

For the WANAKA project (Ø6 pin-drill), holes should be visible. For finer drilling (v-bit spot-drills, Ø1 mm engrave-style drills), holes can be visually missing entirely. **This is the structural cause of the "sim does not show drill holes" report**, not a stamping bug — the stamp is doing exactly what its math says, and its math is wrong below a cell-size-dependent feature threshold.

### 3.4 Heightmap-flavored mesh extraction for blind features

`z_grid_to_solid_mesh` (`dexel_mesh.rs:160-216`) builds the rendered stock from per-cell `(ray_top, ray_bottom, effectively_empty)` triples. The comment block at `dexel_mesh.rs:160-173` describes six mesh components; the load-bearing distinction is:

- **Hole walls** are emitted *only* at boundaries between material and `effectively_empty` cells (a cell is "effectively empty" when `ray_top - ray_bottom < MIN_MATERIAL_THICKNESS = 0.05` mm — i.e. through-holes).
- **Blind features** (drill holes that don't go through, pockets) have material at the bottom of every ray. The mesher does not emit walls for them. Instead, the top-face quads tilt between adjacent cells with different `ray_top` values.

At 0.5 mm cell spacing between an uncut cell (top = 0) and a cut cell (top = −12), the tilted quad is near-vertical and visually reads as a wall. But combined with §3.3 — at Ø2 mm drills only a handful of cells are touched, none of them deeply, the "wall" tilts gradually and the hole renders as a shallow dimple. **The visual symptom compounds with the stamping fidelity defect.**

## 4. Op kinds affected (summary)

| Op kind | Code path | Reported engagement | Visual fidelity | Failure mode |
|---|---|---|---|---|
| `drill` / `pin_drill` (Ø ≥ 3 D_cell) | Degenerate branch, stamped correctly | Defined but wrong metric (category error) | Visible at coarse resolution | §3.1 |
| `drill` / `pin_drill` (Ø < 3 D_cell) | Degenerate branch, but cell-center under-samples | Same as above | **Invisible or barely visible** | §3.1 + §3.3 + §3.4 |
| 2D `pocket` / `profile` on SVG/DXF | Non-degenerate branch post-F-2 fix | Correct | Correct | Stale `CLAUDE.md` caveat |
| `project_curve` / `v_carve` / `trace` | Mixed: Linear lateral + Linear plunge/retract | **Inflated air-cut from retract feeds** | Generally OK | §3.2 |
| `adaptive3d`, `adaptive`, `pocket`, `waterline`, `scallop`, `drop_cutter` | Non-degenerate branch | Correct (within scalar limitations — see §6 H) | Correct | Healthy |
| `helix` / `ramp` entries | Non-degenerate branch (3D motion) | Correct | Correct | Healthy |

---

# Part II — The fix landscape

## 5. Tier split: "Things we should fix" vs "Actual implementation gaps"

**"Things we should fix"** = the underlying model is correct in principle but the reporting / viz / classification fails to communicate what it knows. Additive. Reversible. No architectural debt.

**"Implementation gaps"** = the model itself is incomplete or wrong. Fixing requires changing data structures, abstraction boundaries, or API surface. Expensive but compounding — every other feature gets better.

| Option | Tier | Defect addressed | One-line description |
|---|---|---|---|
| C | Polish | §3.1, §3.2 | Mark drill engagement `not_applicable` in reporting; reclassify pure-Z `Linear` retracts as non-cutting |
| D | Polish | §3.1 substrate | Per-kinematics summary block alongside scalar engagement (additive) |
| G | Polish | §3.3, §3.4 (drill subset) | Renderer-level overlay: draw drill cylinders as explicit geometry from toolpath semantic trace |
| F | **Gap** | §3.3 | Sub-cell analytical stamping — replace binary cell-center test with fractional area integral |
| J | **Gap** | §3.4 | Marching-cubes / dual-contour mesh extraction — proper walls on all features at sub-cell precision |
| E | **Gap** | §3.1 root cause | Drill ops as a first-class kind: separate data model, CSG removal, drill-native metric stream |
| H | **Gap** | §3.1 substrate | Multi-dimensional engagement stream — `radial_engagement` becomes one component of a vector (RWoC, axial DOC fraction, arc, mean/peak chip thickness) |
| I | **Gap** | §3.2 root cause | Intent-aware sample classification — toolpath generators tag each move with `MoveIntent` (Drilling, EntryPlunge, ClearingCut, FinishingCut, Retract, Link); simulator reads it instead of guessing from kinematics |

## 6. Per-option trade-off analysis

### Option C — `cut_kinematics`-aware reporting + retract-feed suppression (Polish, ½ day)

**What it fixes.** Drill ops stop reporting misleading 92 % air-cut. Plunge-and-retract-loop ops (project_curve, v_carve) report air-cut % based on real cutting time, not on retract-through-air time. Readers stop being misled.

**What it does NOT fix.** The underlying metric for drilling is still cylinder-WOC, still semantically wrong. The data model still treats drills as Linear feed segments. The renderer still under-represents small drills.

**State of existing infrastructure (Revision 2026-05-19):** C's reporting half is **already partially implemented**. `ToolpathNarrationContext.is_drill_cycle` (`narrate.rs:48`) already suppresses the air-cut anomaly text for drill ops (`narrate.rs:966-977`). `SimulationToolpathCutSummary.metrics_not_applicable: bool` (`simulation_cut.rs:193`) already exists and is set for drill / pin-drill toolpaths at `session/compute.rs:914-918` and `controller/events/simulation.rs:133-145`. The MCP surface for "engagement N/A" works today via this flag. What's still missing is the **retract-feed reclassification** (non-drill plunge-and-retract-loop ops) and the `MoveIntent`-driven generalization that makes the existing flag correct rather than op-kind-heuristic.

**Implementation.** Two changes (one mostly done, one new):
1. *(Largely done)* Audit the existing `is_drill_cycle` / `metrics_not_applicable` flag for completeness across narrate, MCP, GUI summary surfaces. Where coverage is missing, route through the existing field rather than introducing parallel `engagement_status: NotApplicable` plumbing.
2. *(New)* In `capture_cutting_segment` (`dexel_stock/simulation.rs:360-446`): for `Linear` segments with `dz > 0` and `xy_len_sq < 1e-18`, set `is_cutting = false`. **The `CutKinematics::Retract` variant proposed in the original plan is not architecturally load-bearing** — every consumer in `tool_load/` (power, chipload, deflection, optimize/context, mod) gates on `is_cutting` *first*, so setting `is_cutting = false` is sufficient. Add `Retract` only if narrate output benefits from labeling the kinematics class explicitly. If added, bump `SIMULATION_CUT_TRACE_SCHEMA_VERSION = 3` to 4 with `#[serde(other)]` fallback for old traces.

**Blast radius.** ~5 existing sim tests that exercise pure-Z retract feeds need expectation updates. The optimizer (`tool_load/optimize/mod.rs`) and power gate (`tool_load/power.rs`) do not gate on retract-tagged samples in any current code path. Verified by `grep -n "is_cutting\|Retract"` across `tool_load/`.

**Why it's a polish fix.** Doesn't change *what* the simulator knows, only *what it tells you*. The data is unchanged in `SimulationCutSample`.

**Why we can't skip it.** Without C, every downstream consumer (humans reading narrate output, agents like in WANAKA, optimizers) has to re-discover the caveat. Compounds.

**Why C alone isn't enough.** Heuristic. The "pure-Z move" test doesn't distinguish "drill peck" from "v-carve plunge entry" — both look the same kinematically, but only one is end-cutting. Option I makes C correct rather than heuristic; ideally land them together.

---

### Option D — Per-kinematics summary block (Polish, 1–2 days)

**What it adds.** Alongside the scalar `average_engagement`, emit `summary.linear.{average_engagement, runtime_s, ...}`, `summary.plunge.{...}`, `summary.helix.{...}`, `summary.retract.{...}`. Readers can ask the question that applies to their op.

**Implementation.** Additive in `SimulationCutSummary` / `SimulationToolpathCutSummary` (`simulation_cut.rs:170-241`). `SummaryAccumulator::observe` (`simulation_cut.rs:472`) gains per-kinematics sub-accumulators.

**Blast radius.** Zero — new fields go unread by anything that doesn't ask. JSON shape grows.

**Why D is worth doing as polish.** D is the *reporting* substrate that lets us answer "what *does* engagement look like for this op?" without committing to whether the scalar means what the user thinks it means. Lands the structure; H (below) fills the structure with multi-axis content.

**Trade-off vs H.** D segregates the existing scalar by kinematics. H replaces the scalar with a vector. D is faster to ship and is a strict subset of H — building D doesn't waste work toward H.

**Why D without H still isn't enough.** D still uses `radial_engagement` as the substrate for the plunge block, which is the wrong axis for drills. D buys clarity; H buys correctness.

---

### Option G — Drill viz overlay (Polish, ½ day)

**What it fixes.** "Drill holes don't show" symptom disappears: renderer walks the toolpath, identifies drill cycles via `ToolpathSemanticTrace` (existing infra — `build_move_semantic_lookup` in `dexel_stock/stamping.rs:509-562`), and emits explicit cylinder/cone geometry at each hole's XY/Z with the tool's profile.

**What it does NOT fix.** The dexel stock still under-represents the holes internally. The metrics still report cylinder-WOC. The overlay is cosmetic — if a downstream consumer (collision check, tool-life prediction) reads the dexel stock state, they still see the under-represented version.

**Why G as a standalone is questionable.** Renderer and simulator disagree on the stock state. Two sources of truth. If E (below) lands, G is subsumed: E's CSG mesh path produces correct geometry as a *consequence* of correct simulation.

**Recommendation.** Skip G as a standalone. Bundle into E. The single exception: if E is going to slip more than 2 weeks past now and the "drill holes don't show" feedback is high-visibility, ship G as a stopgap with a `// TODO: remove when E lands` comment.

---

### Option F — Sub-cell analytical stamping (Gap, 3–4 days; revised up from 2–3 after gap analysis)

**What it fixes (§3.3).** Small drills, v-carve tips, fine engrave features all render with correct circular/conical geometry at the cell level. Engagement metrics for small features become accurate rather than stairstep-quantized.

**The actual defect.** `stamp_point_on_grid` (`stamping.rs:48-96`) and `stamp_segment_on_grid` (`stamping.rs:104-168`) both use a binary test: cell is "inside the cutter" iff its center is inside `r`. For a Ø2 mm cutter at 0.5 mm cells, ~12 cells get full-depth stamps. The true cut footprint covers ~12.6 cells of equivalent area but distributed across ~28 cells with fractional coverage. The simulator loses 56 % of the engagement perimeter.

**Scope clarification (Revision 2026-05-19):** F.a does **not** fix small drills visually. At Ø2 mm on a 0.5 mm grid, F.a adds ~8–10 annular cells at fractional contribution; the hole is still represented by a 4-cell-radius feature and does not look circular. **Step E (DrillOp CSG) is the real fix for small drill visibility.** F.a's actual wins are concentrated on v-carve, fine engrave, narrow pocket fidelity, and engagement-metric accuracy near feature edges across all ops.

**What "doing it right" means.** For each cell within `r + cell_size·√2`, compute the fraction `f ∈ [0, 1]` of cell area inside the cutter footprint. The cell's ray-top update is scaled by `f`: if `f = 1` the ray is cut to `tip_depth + h(cell_center)` as today; if `f < 1` the ray is partially blended toward that target.

**Algorithmic gaps the original plan glossed over (Revision 2026-05-19):**

1. **Degenerate (pure-Z) branch must scale `removed_volume` by `f`.** The non-degenerate branch (`stamping.rs:271-428`) computes `removed_volume = pre_volume - post_volume` from actual pre/post ray lengths — it is self-correcting under fractional stamping. The degenerate branch (`stamping.rs:216-269`) accumulates `removed_volume += above * cell_area` *directly* at `stamping.rs:233,250-252`. Under fractional coverage this overestimates volume by `1/f` for annular cells unless explicitly fixed to `removed_volume += f * above * cell_area`. **This is a concrete bug if F.a is implemented as the original plan describes.**

2. **A new `ray_blend_above(ray, surface, f)` primitive is required in `dexel.rs`.** Today's `ray_subtract_above` unconditionally sets the ray top to `surface`. F.a needs a partial-blend version that moves the top toward `surface` by a fraction `f` of the gap. The original plan did not name this primitive.

3. **Annular cells outside `radius_sq` need `h` interpolated at the nearest in-radius point.** Cells with center distance in `(radius, radius + cell_size·√2)` are partially covered but `lut.height_at_dist_sq(dist_sq)` returns `None` at the cell center (the LUT's binary gate). F.a must query `h` at the nearest point inside the cutter radius for these cells, or extend the LUT's API. This requires loop-bounds changes in the stamping kernel — extend the bounding-box scan from `r + cell_size` to `r + cell_size·√2`.

4. **F.a should expose per-cell `coverage: f32` from day 1.** F.b's eventual sub-cell-resolved storage needs `coverage` as its initialization hint and as the bridge between single-top and sub-cell-tops APIs. Adding `coverage` as a sibling to `ray_top` from F.a onwards makes F.b a strict superset with no API break. The original plan did not call this out and would require F.b to re-derive coverage from neighbor inspection.

**Two implementation tiers.**
- **F.a (single-resolution, area-weighted with coverage)**: ray-top updated by `ray_blend_above(ray, target, f)`; per-cell `coverage: f32` returned alongside `ray_top`. Engagement numbers shift by ~1–3 % across the test corpus.
- **F.b (sub-cell resolved)**: each cell stores a quadtree (or 4×4 sub-grid) of ray-tops, initialized from F.a's `coverage`. True analytical accuracy. Engagement numbers shift by ~5–15 % for small-feature ops. All consumers of `ray_top` learn to ask "at which sub-cell?".

**Blast radius (corrected).**
- F.a: ~20–30 test assertions shift (not ~50 as the original plan estimated). Concentrated in `dexel_stock/mod.rs` inline unit tests with hard-coded `ray_top` values, `sim_radial_engagement_density.rs` engagement assertions, and `sim_chipload_invariant.rs` consistency checks. Tests that stamp into the center of a well-resolved disk (radius >> cell_size) are unaffected.
- F.a is **not invisible to the planning layer.** `adaptive3d::stock_top_z_at` (`adaptive3d/mod.rs:145`) reads `ray_top` for clearing-pass planning; F.a will shift edge-cell heights, causing slightly shallower bites at feature edges. Effect is small but real — add a sanity test, do not claim F.a is reporting/viz-only.
- Collision check (`collision.rs:310-393`) becomes marginally more conservative under F.a (edge cells retain slightly more stock) — correct direction, no regression risk.
- `feedopt::estimate_side_engagement` (`feedopt.rs:72`) becomes more accurate under F.a — pure win.
- Mesh extraction unaffected; just reads ray_top.
- F.b: every consumer of `ray_top` (~12 call sites) needs to choose between "average top" and "specific sub-cell." Worth doing once we know we'll keep sub-cell info around.

**Performance.**
- F.a: ~1.4× stamping time (one fractional integral per cell, computable from the LUT or as `clip(disk, square)` area).
- F.b: ~3–5× stamping time, ~16× memory per ray. Wood router CAM runs minutes-per-job today; this is acceptable. For interactive viz playback at 60 fps with active stamping, F.b is borderline — measure first.

**Why F is "world-class" territory.** Mastercam, Fusion 360 (HSM), PowerMill, and SolidCAM all use sub-resolution stamping for stock simulation. Cell-center binary is what hobbyist sims look like.

**Trade-off F.a vs F.b.** F.a captures ~80 % of the visual and metric improvement at 20 % of the cost. F.b is the right destination if we ever want adaptive3d's clearing engine to make sub-cell decisions (it currently doesn't). **Recommend F.a now, F.b as a follow-up once we have a use case that demands it.**

**What F doesn't fix.** Drilling is still a category error (Option E). Engagement is still a scalar (Option H). Mesh extraction still tilts blind-wall quads (Option J).

---

### Option J — Marching-cubes / dual-contour mesh extraction (Gap, 1 week)

**What it fixes (§3.4).** Drill holes have real vertical walls. V-carved letterforms have crisp edges. Pocket walls render true to depth. Small-feature visual verification becomes reliable.

**The actual defect.** `z_grid_to_solid_mesh` (`dexel_mesh.rs:175`) is heightmap-style: one top-Z per cell, optional through-hole detection. Adjacent cells with different `ray_top` get connected by tilted quads, not vertical walls. For features narrower than ~5 cells, walls visibly tilt; for features ≤2 cells, walls become indistinguishable from the noise floor.

**What "doing it right" means.** Treat the dexel volume as a signed distance field: positive inside material, negative in air. Sample at cell corners (the natural marching-cubes grid). Extract a watertight mesh by classifying each cubic cell against the iso-surface and emitting triangles per the marching-cubes lookup table (or use dual contouring for cleaner topology near sharp features).

**Architecturally clean sampling (Revision 2026-05-19):** Do **not** materialize a separate 3D voxel grid. Derive the SDF directly from ray data: for each cell-corner `(u, v, z)`, bilinearly interpolate `ray_top` from the four neighboring cells' values; `sdf = interp_ray_top − z`. Memory stays O(rows × cols), matching the existing data structure. Multi-segment rays (internal cavities, through-cuts) need a per-gap pass — rare in 3-axis-from-top because almost all rays become single-segment after the cut clears through, so handle it as a fallback when `max_segments > 1` in the grid. With F.a in place, `ray_top` is already coverage-weighted, feeding the bilinear interpolation directly.

**Why J pairs with F.** If F is in place, the SDF has sub-cell resolution at feature boundaries, and marching cubes produces sub-cell-accurate walls. Without F, J produces correctly-vertical walls *aligned to cell boundaries* — better than today, but still cell-stairstepped at small features.

**Scope (Revision 2026-05-19): J replaces the closed-solid mesh path only, not the live preview.**

The codebase already has two distinct mesh paths in `crates/rs_cam_viz/src/app/simulation.rs`:

- **Live playback** (`update_live_sim`, `simulation.rs:70`) calls `dexel_stock_to_entry_surface_mesh` — top-surface-only preview, no walls, no bottom, no cavity. Throttled to 20 Hz (`LIVE_MESH_UPLOAD_INTERVAL = 50ms`, `simulation.rs:219`). This is the hot path.
- **Pause / scrub-stop / checkpoint creation** calls the full `dexel_stock_to_mesh` (closed solid, all six components). Runs once on pause or per-toolpath-boundary on the compute thread.

**J replaces `dexel_stock_to_mesh` only.** The live preview stays on the fast heightmap top-surface path — walls aren't load-bearing for in-progress playback, only for static review and final-state visualization. This eliminates the LOD / incremental-extraction concerns from earlier analysis. Clean architectural split: fast heightmap for in-progress motion, accurate MC for static review.

**Side-grid path dropped from scope (Revision 2026-05-19):** `side_grid_to_mesh` (`dexel_mesh.rs:803`) is effectively unused in current 3-axis-from-top flows — per-setup group stocks always simulate with `FromTop` regardless of setup orientation (`compute/simulate.rs:333-335`), so `x_grid` and `y_grid` are `None` for the meshes that produce checkpoints and composite renders. Side-grid is "future 4-axis-ready" speculative work. Drop from Step 5 scope; pick up when 4-axis is genuinely scheduled.

**Blast radius.** Renderer-only. The mesh consumer at `crates/rs_cam_viz/src/render/sim_render.rs:158` (`from_heightmap_mesh`) is **topology-agnostic** — it consumes flat-array triangle soup (vertices, indices, colors); normals are computed CPU-side from winding before GPU upload (`sim_render.rs:407-459`). No consumer anywhere requires manifold or watertight geometry — no STL export of stock, no volume computation, no mesh-based collision (collision reads `DexelGrid.rays` directly). The function name `from_heightmap_mesh` is misleading; it places no heightmap constraint. Marching cubes can produce whatever topology it wants. Only constraint: consistent winding for CPU-side normal computation.

**Performance.** Marching cubes is well-optimized in the OSS ecosystem; reference implementations process millions of voxels per second. Wood-router grids (typically 100×100×50 cells = 500k voxels) are well under the budget. Since J runs on pause or checkpoint creation (not per-frame), absolute time matters more than throughput per second — sub-second per call is acceptable. Existing benchmarks at `benches/perf_suite.rs:474` (`bench_dexel_mesh_extraction`, three grid sizes) will catch regressions.

**Memory note.** Each `SimCheckpointMesh.mesh` stores a full `StockMesh`; MC's higher triangle count grows checkpoint memory ~5–10× per toolpath. For 5–10 toolpaths this is tens of MB — workable, but worth measuring on a representative project.

**Why this is the right long-term mesh path.** Heightmap meshing can't represent undercuts, 4-axis cuts from below, or overhangs. Marching cubes is the standard for volumetric stock representations and gives us all of these for free if we ever need them. Wood routing is 3-axis-from-top today, but the same engine could drive a future 4-axis indexing setup with no representation change.

**What J doesn't fix.** Engagement metrics (H), drilling category error (E), classification heuristic (I).

---

### Option E — Drill ops as a first-class kind (Gap, 4–6 days)

> **Decision needed before scheduling (Revision 2026-05-19):** E bundles two distinct things: (a) the *fidelity fix* for drill engagement metrics and small-drill visualization, and (b) the *operability expansion* that makes drilling a first-class CAM concept with parametric editability, hole-level reporting, and (future) canned cycles and tapping. The fidelity fix can be captured by a much cheaper **kernel-swap alternative**: tag drill moves via `MoveIntent::Drilling` (free with Step 1) and, in the simulator, replace per-segment stamping with analytical hole removal when the intent is `Drilling`. Same data model, same `Toolpath`, same mesh consumer; just a different removal kernel per intent. ~1 day vs E's 4–6 days, captures ~70% of E's user-visible value. Promote E to full `DrillOp` only when a *user-facing feature* demands the operability surface (e.g. "optimize hole travel order," "stepped/tapping cycles," "per-hole risk reporting"). If no such feature is on the roadmap, **execute kernel-swap as part of Step 1 and defer E indefinitely.**

The remainder of this section describes the full E plan for the case where the decision lands on promotion. Skip to §6.I / §8 Step 1 if defaulting to kernel-swap.

**What it fixes (§3.1 root cause).** Drilling stops being treated as Linear-feed milling. End-cutting tools get end-cutting metrics. Visualization gets analytically-correct hole geometry without depending on dexel resolution. Drill-specific failure modes (chip welding, peck pattern wrong for material, runout walking) become detectable.

**The actual gap.** Drilling and milling are different physics. The simulator doesn't know. `drill.rs:50` (`drill_toolpath`) produces a `Toolpath` with `Linear` feeds and `Rapid` retracts — exactly the same shape as any milling toolpath. The op-kind information is lost as soon as the toolpath leaves the generator.

**Existing groundwork (Revision 2026-05-19):** Much of "drill is special" is already plumbed. `OperationConfig::Drill(DrillConfig)` and `OperationConfig::AlignmentPinDrill(AlignmentPinDrillConfig)` are serde-serialized in project TOML and already enum-tagged (`catalog.rs:576,589`). The optimizer (`tool_load/optimize/mod.rs:147-152`) and chipload gate (`tool_load/chipload.rs:254-262`) **already skip drill ops with `Unmodeled(NotApplicableForOp)`**. `is_drill_cycle` flag and `metrics_not_applicable` field exist (see §6.C revision). E is promoting scattered special-cases to a proper data model, not building from zero.

**What "doing it right" means.** A new `DrillOp` carried as a variant of an `OpData` enum wrapping the existing `Arc<AnnotatedToolpath>` inside `ToolpathComputeResult` (`session/mod.rs:404`):

```rust
enum OpData {
    Toolpath(Arc<AnnotatedToolpath>),
    DrillOp(Arc<DrillOp>, Arc<AnnotatedToolpath>),  // see invariant below
}

struct DrillOp {
    holes: Vec<DrillHole>,
    hole_source: HoleSource,     // Snapshot | ModelDerived — see asymmetry note
    tool_profile: ToolProfile,   // for spot-drill / countersink / standard
    cycle: DrillCycle,           // existing enum at drill.rs:11
    feed_rate: f64,
    spindle_rpm: u32,
    flute_count: u32,            // 1 for true drill, 2 for EM-as-drill
    material: MaterialClass,     // for chip evacuation modeling
}

struct DrillHole {
    xy: P2,
    top_z: f64,
    bottom_z: f64,
}
```

**AlignmentPin vs Drill hole-source asymmetry (Revision 2026-05-19):** `AlignmentPinDrillConfig.holes: Vec<[f64; 2]>` (`operation_configs.rs:163`) stores XY positions *snapshot in the config* — they survive project save/load directly. Regular `Drill` ops take holes *from the model* at generate-time and discard them after toolpath emission. A unified `DrillOp` must handle both modes. Carry `HoleSource::Snapshot(Vec<[f64; 2]>)` vs `HoleSource::ModelDerived` on `DrillOp` so the generation path knows where to refresh holes from on regenerate, and so project IO can round-trip `Snapshot` directly while `ModelDerived` re-resolves on load.

**The dual-representation invariant (Revision 2026-05-19):** `OpData::DrillOp` carries **both** the `DrillOp` (for simulation, metrics, mesh) and an `AnnotatedToolpath` (for G-code export, wire-render screenshots, GUI overlay, MCP `move_count`). They are **produced atomically** from the same config in `compute/execute.rs` and **invalidated together** by any `set_toolpath_param` mutation. Single invariant: *config mutation clears both representations*. The current invalidation logic in `session/mod.rs` covers `annotated`; extend to cover both. Do **not** lazy-derive one from the other — too many drift windows in practice (G-code export reads `result.toolpath()` at `io/export.rs:51`; `screenshot_toolpath` and the GUI wire overlay read `annotated.toolpath.moves`; MCP `generate_toolpath` returns `move_count` from `Toolpath.moves.len()` — all are load-bearing on the linearized form).

Three parallel streams downstream:

1. **Stock removal.** Sim layer subtracts each hole's swept cylinder/cone from the stock analytically. For the dexel representation, this means walking each cell within the hole's XY footprint and setting `ray_top = bottom_z` (with `h(r)` profile for cone-tip drills); it bypasses the per-Linear-segment stamping loop entirely. The result is exact at cell-grid resolution, no stairstepping along the hole's depth axis.
2. **Mesh extraction.** `dexel_mesh.rs` gains an `append_drill_cylinders(mesh: &mut StockMesh, drill_ops: &[DrillOp])` helper that walks each `DrillHole` and appends analytic cylinder/cone triangles to the mesh produced by `dexel_stock_to_mesh`. The existing `append_mesh` helper (`dexel_mesh.rs:904`) provides the composition pattern. Renderer unchanged — consumes `StockMesh` triangle soup, topology-agnostic (see §6.J revision). **Seam risk:** between Step 3 and Step 5, the heightmap mesh's tilted-quad walls at hole edges will not perfectly align with the analytic cylinder walls. Test the Step-3-only intermediate state explicitly before Step 5 lands; the seam will be visible at low dexel resolution.
3. **Metric stream.** Drill-native sample emission: per peck, emit `DrillSample { hole_id, peck_index, descent_mm, axial_chipload_mm_per_rev, dwell_s, chip_evacuation_score }`. The summary block reports per-hole peck pattern adequacy, chip welding risk (`depth_to_diameter > threshold_for_material`), and drill-cycle cycle time. No `radial_engagement`, no `air_cut_percentage` — concepts that don't apply.

**Composition with other ops.** A drilled hole that a later milling op cuts across (e.g. pocket-then-drill, or drill-then-flycut) needs the milling-op stamping to see the drilled state of the stock. Path: the `DrillOp` removal applies to the dexel grid in-place before subsequent milling ops stamp it, so the stamping reads the post-drill ray state. No special compositional logic needed.

**Project IO and migration (Revision 2026-05-19):** `Toolpath` is **not** persisted to project files — only `OperationConfig` is. Project format break risk is zero. The "loader heuristic" from the original plan §10.6 is a trivial pattern match on the existing `OperationConfig::Drill | AlignmentPinDrill` variants — every saved project already self-identifies. No heuristic needed; no detector module needed.

**G-code export reality.** `gcode/program_builder.rs:21-79` is pure `MoveType` → `Statement` translation; no canned-cycle (G81/G83) awareness today. The plan claim "G-code wants the Linear/Rapid sequence anyway" is accurate for the current implementation. Adding canned-cycle emission is a new `Statement::CannedDrill { x, y, z, r, f, cycle }` variant and a corresponding G-code emit — doable but new work, scoped out of E. See §11.

**Effort.** 4–6 days end-to-end:
- 1 day: `DrillOp` + `OpData` data model + carry through session / project IO / MCP.
- 1 day: stock removal path with cone/cylinder analytical subtraction.
- 1 day: mesh extraction integration in `dexel_mesh.rs`.
- 1 day: `DrillSample` stream + `DrillToolpathSummary` + narrate output.
- 1–2 days: drill-specific gates (chip welding, peck adequacy) + tests.

**Blast radius.** Large but additive. Existing tests for drill ops keep passing if `generate_toolpath` continues to produce `AnnotatedToolpath` alongside `DrillOp` (the dual-representation invariant above). MCP surface mostly absorbs the change cleanly — `list_toolpaths`, `get_toolpath_params`, `set_toolpath_param`, `optimize_toolpath` are config-driven and continue to work. `generate_toolpath.move_count`, `screenshot_toolpath`, and `get_cut_trace` need to know which `OpData` variant they're looking at, but the dual-representation invariant means they can fall back to the `AnnotatedToolpath` path unchanged.

**Why this is the right long-term direction.** Every CAM system serious about drilling treats it as a distinct op kind end-to-end. We get correct metrics, correct visuals, accurate cycle time prediction, and the ability to flag drill-specific failure modes the milling pipeline can't see.

**What E enables next.** Countersinks, spot drills, stepped drills, tapping cycles, peck-cycle optimization — each becomes a parametric extension of `DrillOp` rather than a new toolpath generator.

**What E doesn't fix.** Non-drill plunge entries (helix-into-pocket, ramp-into-clearing) still go through the dexel. F is still needed for small-feature fidelity in milling ops. H is still needed for non-drill engagement representation.

---

### Option H — Multi-dimensional engagement stream (Gap, 3–4 days)

**What it fixes (§3.1 substrate).** A single scalar `radial_engagement` cannot represent the cutting interaction at a sample. Real cutting has at least: radial WOC fraction, axial DOC fraction, arc engagement (entry/exit angles), mean chip thickness, peak chip thickness, leading-edge velocity. Most CAM systems carry these as a structured engagement vector.

**The actual gap.** `SimulationCutSample` (`simulation_cut.rs:1-50ish`) carries `radial_engagement: f64`, `axial_doc_mm: f64`, `arc_engagement_radians: Option<f64>`, `chipload_mm_per_tooth: f64`, `effective_chip_thickness_mm: Option<f64>`. The pieces exist but aren't composed as a unit. Downstream code reads them as five disconnected scalars. The chipload-mean-vs-peak commentary in `stamping.rs:478-508` is evidence that the team has already noticed this and partly addressed it (mean vs peak chip thickness) but it isn't yet first-class.

**What "doing it right" means.** Introduce `Engagement` as a structured type carried per-sample:

```rust
struct Engagement {
    radial_woc_fraction: f64,       // 0..1, cylinder-side WOC
    axial_doc_fraction: f64,        // 0..1, of flute length
    arc_radians: Option<f64>,       // entry/exit arc
    mean_chip_thickness_mm: Option<f64>,
    peak_chip_thickness_mm: Option<f64>,
    leading_edge_speed_mm_min: f64, // for chip-load gate
    direction: EngagementDirection, // climb / conventional / mixed
}
```

Consumers (chipload gate, power gate, optimizer, hotspot detector) pick the axis they care about. The cylinder-WOC scalar becomes one component, not the whole story. The legacy `radial_engagement` field stays around as a derived view for one release deprecation window, then removed.

**Blast radius.** Substantial but additive. Every consumer of `radial_engagement` (~15 call sites) opts into the new field deliberately. `peak_engagement`, `low_engagement_time_s`, `air_cut_time_s` semantics need re-defining around which engagement axis triggers them.

**What H unlocks.** Tool deflection coupling (needs leading-edge speed + radial-WOC + chip thickness). Proper hotspot classification (heavy-radial vs heavy-axial vs heavy-arc — currently all collapsed). Adaptive3d's `target_engagement_fraction` becomes well-defined (currently ambiguous between radial-WOC and the algorithmic target — `adaptive_review_2026-04.md` notes the discrepancy).

**Trade-off vs D.** D segregates the *existing* scalar by kinematics. H replaces the scalar with a *vector*. D is faster to ship and gives most of the immediate clarity. H is structurally cleaner and unblocks several downstream features. **They compose: do D first as a polish fix, H second as the structural answer.**

**What H doesn't fix.** Doesn't fix drilling (E). Doesn't fix small-feature fidelity (F). Doesn't fix classification heuristic (I).

---

### Option I — Intent-aware sample classification (Gap, 2–3 days)

**What it fixes (§3.2 root cause).** The current classifier (`dexel_stock/simulation.rs:510-525`) infers `Plunge` from `xy_len < 1e-9 && dz > 1e-9`. That's a kinematic accident. A toolpath generator that *knows* "this segment is a drill peck" vs "this segment is the third pass of an adaptive entry helix" vs "this segment is a retract" should be able to *tell* the simulator. The semantic-trace infrastructure (`ToolpathSemanticTrace`, consumed in `stamping.rs:516-569`) is partway there — it tags operations and segments by structural role, and `build_move_semantic_lookup` maps moves to items — but the simulator never reads the intent tag for *classification*.

**Semantic-trace vs MoveIntent (Revision 2026-05-19):** `ToolpathSemanticTrace` is a range-to-item *structural grouping* (one item per region/level/pass spanning many moves — Operation, DepthLevel, Region, Pass, Hole, Cycle, etc.). `MoveIntent` is per-move *classification*. They are orthogonal and complementary; both should exist. `MoveIntent::Drilling` on each peck Linear is different in kind from `ToolpathSemanticKind::Hole` on the range of moves comprising one hole.

**Attachment point (Revision 2026-05-19):** Add `intent: MoveIntent` to the **`Move` struct** at `toolpath.rs:41-44`, not inside `MoveType::Linear { feed_rate }`. Hanging it inside the enum variant would force updating 60+ `MoveType::Linear { feed_rate }` pattern-match sites across ~15 files. On the `Move` struct, it co-locates with `target` and `move_type` and adds zero match-site breakage. `MoveType` is **not** serde-derived (only computed at generate-time, never persisted to project files), so adding `MoveIntent` introduces no project IO break risk.

**Highest-leverage emission point:** `Toolpath::emit_path_segment` (`toolpath.rs:106-127`) is the shared wrapper that emits plunge + feed Linears for ~8 generators (project_curve, ramp_finish, pencil, trace, pocket, profile, vcarve, ...). Adding an `intent: MoveIntent` parameter here propagates to 8 callers at once. Direct `feed_to` / `rapid_to` callers (~12 sites across drill.rs, adaptive/path.rs, scallop.rs, waterline.rs, adaptive3d/mod.rs) need individual updates with the intent tag.

**What "doing it right" means.** Each `Move` carries a `MoveIntent`:

```rust
enum MoveIntent {
    Drilling,         // drill cycle plunge
    EntryPlunge,      // milling op entering material
    ClearingCut,      // roughing material removal
    FinishingCut,     // finishing pass
    EntryHelix,       // helical entry
    EntryRamp,        // ramped entry
    Linking,          // tool-position-to-tool-position transition
    Retract,          // lift off material before rapid
    Unknown,          // fallback for legacy / unaware generators
}
```

Generated by every toolpath generator. Consumed by the simulator's classifier and the metric accumulator. The kinematic heuristic stays as the fallback for `Unknown`. **All in-tree generators must emit a non-`Unknown` tag in the same PR** — `Unknown` is a deprecation marker, not a long-term fallback. Tracked: deletion PR for `Unknown` once external/test generators are confirmed migrated.

**`CutKinematics::Retract` is decorative, not load-bearing (Revision 2026-05-19):** All five `tool_load/` consumers gate on `is_cutting` first:
- `power.rs:108`: `if !s.is_cutting { continue; }`
- `chipload.rs:191`: `!s.is_cutting` in early-return
- `deflection.rs:83`: `!sample.is_cutting` in early-return
- `optimize/context.rs:72`, `mod.rs:209`: `.filter(|s| s.is_cutting)`

Setting `is_cutting = false` on retracts is sufficient. The `Retract` variant on `CutKinematics` is only useful for narrate readability. Either skip it (saves schema-version bump on `SimulationCutTrace`) or add it purely for narrate, with `#[serde(other)]` fallback. Don't let the variant block Step 1.

**Blast radius.** Touches every toolpath generator — but additively, each emits a tag. The simulator's classification logic gains a single branch ("if intent is known, use it; else fall back to kinematic"). Tests gain coverage for "drill cycle plunge is classified as Drilling, not just Plunge."

**What I unlocks.** Intent tags let the optimizer reason about *why* a segment is slow (retract that could be a rapid? finishing cut correctly slow? entry-plunge that should be a helix?). They unblock retract optimization, entry-strategy auditing, and per-intent engagement reporting (the substrate D + H sit on top of). They make Option C *correct* rather than heuristic — instead of "if pure-Z then non-cutting retract," the rule becomes "if intent == Retract then non-cutting." They also let the kernel-swap alternative to E work cleanly: `intent == Drilling` triggers analytical hole removal instead of per-segment stamping.

**Trade-off vs C.** C is the kinematic-heuristic stopgap. I is the proper fix. C's effort is small enough to land alongside I in the same PR — C as the reporting/output side, I as the substrate. They compose naturally.

**What I doesn't fix.** Doesn't fix the engagement-as-scalar issue (H). Doesn't fix drilling category error (E) on its own — but combined with the kernel-swap removal kernel, it does. Doesn't fix small-feature fidelity (F).

---

## 7. Why "world-class" needs all the gap fixes

Wood-router CAM doesn't need 5-axis kinematics or NURBS surface contact modeling. The fidelity points that matter for this product:

1. **Every visible feature on the part shows correctly in the sim preview.** Drill holes, narrow slots, v-carved letterforms, sharp corners. *Needs F + J. E gives drills the easy path to it.*
2. **Every metric the operator looks at is meaningful for the op kind producing it.** Engagement only shown where it applies. Drill ops have drill metrics. Lateral ops have RWoC. *Needs E + H + I.*
3. **The simulator's understanding of an operation matches what the toolpath generator intended.** No kinematic-accident misclassification. *Needs I.*
4. **Optimizer decisions are grounded in the right cost function for each op.** Drill optimizer cares about chip evacuation. Adaptive optimizer cares about engagement variance. *Needs H (which then needs I to know which axis to optimize).*

The polish-only path (C + D + G) achieves none of the above durably. It stops misleading users today, which is valuable, but it leaves the architecture mismatched with the product's quality goal. The gap fixes are what take this from "competent hobbyist CAM" to "competitive with commercial systems on every axis that matters for 3-axis wood routing."

---

# Part III — Recommendation

## 8. Sequencing (5-step roadmap + Step 0)

Each step lands as one or more atomic PRs with full test coverage. Earlier steps unblock later ones; the order respects dependencies. Total: **~3–4 weeks of focused work.**

### Step 0 — `mcp.rs` accumulator dedup (½ day, independent of everything else)

**Added 2026-05-19.** The MCP handler contains two parallel re-implementations of `SummaryAccumulator::observe`:

- `mcp.rs:2884-2907`
- `mcp.rs:3040-3057`

Both duplicate the time-weighted engagement math (`engagement_time_weighted_sum += radial * time`) and the 0.02 / 0.10 air-cut / low-engagement thresholds. Any threshold change in canonical `SummaryAccumulator` (`simulation_cut.rs:531-563`) has to be made in three places, and drifting in practice would be silent. Refactor both to call the canonical accumulator.

**Independent of all other steps.** Lands tomorrow. Reduces the surface that needs updating in Step 2 (D + H) — H's `radial_engagement` rename only has to touch one accumulator implementation instead of three.

### Step 1 — C + I together (3 days)

**Land as one PR.** I is the substrate that makes C correct rather than heuristic.

- I introduces `MoveIntent` on the **`Move` struct** (not inside `MoveType::Linear`) and threads it through every toolpath generator. Highest-leverage attachment: `Toolpath::emit_path_segment` (`toolpath.rs:106-127`) gains an `intent` parameter; direct `feed_to`/`rapid_to` callers get individual updates. **All in-tree generators emit non-`Unknown` tags in the same PR**; `Unknown` is a deprecation marker, not a long-term fallback. Schedule a follow-up deletion PR for `Unknown` once external/test generators are confirmed migrated.
- C reuses the existing `is_drill_cycle` flag + `metrics_not_applicable` field surfaces (mostly already wired — see §6.C revision); reads `MoveIntent::Retract` to set `is_cutting = false`. **Do not** add `CutKinematics::Retract` variant unless narrate readability demands it — all `tool_load/` consumers gate on `is_cutting` first, so the variant is decorative.
- *(Decision point)* If the kernel-swap alternative to E is chosen (see §6.E callout), include the analytical drill-removal kernel in this same PR — `intent == Drilling` triggers per-cell ray-top set to `bottom_z` instead of segment stamping. Adds ~1 day to Step 1; subsumes most of E's fidelity wins.
- Tests: `drill_cycle_reports_not_applicable_engagement`, `project_curve_retracts_dont_inflate_air_cut`, per-generator tag-correctness tests, and (if kernel-swap is included) `drill_kernel_swap_visualizes_holes_correctly`.

**Closes:** §3.1 reporting half, §3.2 entirely. Stops misleading users. If kernel-swap included, also closes §3.1 root cause for drills and §3.3 small-drill visibility for drill ops specifically.

### Step 2 — D + H together (5 days)

**Land as one PR.** D is the reporting substrate, H is the data substrate. Building D first wastes work; build them together.

- H introduces the `Engagement` struct in `SimulationCutSample` with the legacy scalar carried as a derived view for one release.
- D builds the per-kinematics summary block consuming `Engagement` directly (each kinematics class reports the axes that matter — `Linear` reports radial-WOC + arc + chip thickness; `EntryHelix` reports radial-WOC + axial-DOC; etc.).
- Tests: per-kinematics engagement assertions, legacy scalar consistency.

**Closes:** §3.1 substrate. Unblocks the optimizer to query per-axis engagement.

### Step 3 — E (6 days) — *conditional, see §6.E decision callout*

**Skip if kernel-swap was chosen in Step 1.** If a user-facing feature demands `DrillOp`-level operability (parametric editing, hole-level reporting, future canned cycles or tapping), promote drilling to first-class via E. Otherwise the kernel-swap done in Step 1 already captures the fidelity wins.

By now I + H exist; the drill metric stream has somewhere clean to land. Threads:

- `OpData { Toolpath, DrillOp }` enum wrapping `Arc<AnnotatedToolpath>` inside `ToolpathComputeResult` (`session/mod.rs:404`).
- `DrillOp` data model with `HoleSource::Snapshot` vs `ModelDerived` to handle the AlignmentPin vs regular Drill hole-source asymmetry.
- **Dual-representation invariant:** `OpData::DrillOp` carries both `DrillOp` *and* `AnnotatedToolpath`. Produced atomically in `compute/execute.rs`; invalidated together by any `set_toolpath_param`. Extend `session/mod.rs` invalidation to cover both. Single invariant: config mutation clears both.
- Analytical stock removal (cell-by-cell ray-top set, no segment stamping).
- `DrillSample` stream + `DrillToolpathSummary`.
- Drill-specific gates (chip welding by depth-to-diameter, peck adequacy by material, plunge feed sanity).
- Mesh-extraction integration: `append_drill_cylinders(mesh, drill_ops)` helper in `dexel_mesh.rs` using the existing `append_mesh` composition pattern (`dexel_mesh.rs:904`). **Seam visibility test required** between Step 3 landing and Step 5 — heightmap tilted-quad walls will not align cleanly with analytic cylinder walls at hole edges.
- Project IO: trivial. `OperationConfig::Drill | AlignmentPinDrill` enum tags already exist in saved TOML; loader pattern-matches to construct `OpData::DrillOp`. No heuristic, no format break.

**Closes:** §3.1 root cause for drills (if not already closed by kernel-swap in Step 1). Subsumes Option G (drill viz overlay) — drill holes now render correctly because the mesh path is correct. Drill engagement metrics become drill-native. Unlocks future tapping, stepped drills, parametric peck-pattern editing.

### Step 4 — F.a (3 days)

**Sub-cell stamping for non-drill ops.** With drills out of the dexel path (Step 3), F.a's blast radius drops — drill regression tests don't move. F.a is now "small-feature fidelity for milling ops."

- Replace binary cell-center test in `stamp_point_on_grid` and `stamp_segment_on_grid` with area-weighted fractional coverage.
- Regenerate sim test snapshots. Expect ~1–3 % shifts in engagement across the corpus.
- Validation: WANAKA TP1 / TP6 / TP7 engagement deltas within ±2 percentage points.

**Closes:** §3.3 for milling. Drill holes already correct from Step 3; this fixes v-carve, fine engrave, narrow pocket fidelity.

### Step 5 — J (1 week)

**Marching-cubes mesh extraction.** By now F.a gives sub-cell-aware ray data (via per-cell `coverage`); J converts it to true 3D walls.

- Replace `z_grid_to_solid_mesh` (`dexel_mesh.rs:175`) with a marching-cubes / dual-contour implementation reading from the dexel ray data via bilinear-interpolated `ray_top` SDF (no separate voxel grid materialized — see §6.J revision).
- **Leave `dexel_stock_to_entry_surface_mesh` unchanged.** The 20 Hz live-preview path stays on the fast heightmap top-surface; MC only replaces the closed-solid path used on pause / scrub-stop / checkpoint creation (see §6.J revision for the rationale on this split). No LOD / incremental-extraction work needed.
- **Side-grid path (`side_grid_to_mesh`, `dexel_mesh.rs:803`) dropped from scope.** Effectively unused in 3-axis-from-top flows; defer to a real 4-axis epic. See §11.
- Renderer (`render/sim_render.rs:158`, `from_heightmap_mesh`) consumes the new mesh unchanged — it's topology-agnostic triangle soup with only a winding-consistency requirement.
- Pick algorithm (vanilla MC with disambiguation lookup vs dual contouring) and document manifold/winding guarantees before implementation starts. Vanilla MC is simpler; dual contouring is cleaner at sharp features.
- Measure checkpoint memory growth — MC produces 5–10× more triangles, so `SimCheckpointMesh` storage grows proportionately. Tens-of-MB scale, but verify on a representative project.

**Closes:** §3.4. All features render with correct walls on pause/static views. Future-proofs for undercuts (when needed). Live preview stays fast.

### Out-of-scope follow-ups (post-roadmap)

- **F.b** (sub-cell-resolved ray storage). Land when a feature demands it — likely when adaptive3d wants sub-cell clearing decisions.
- **Tool deflection coupling.** Reads from `Engagement.leading_edge_speed` × `radial_woc` to produce a deflection-vs-tolerance gate. Pure consumer; lands when there's a use case.
- **Material grain modeling.** Anisotropic cutting forces in wood — feeds into chipload gate. Independent of this roadmap.
- **5-axis stock representation.** Tri-dexel X/Y grids are already lazily-created; J's marching cubes covers them. Whole-system 5-axis support is a separate epic.

## 9. Validation strategy per step

Each step ships with:

1. **Regression-locking tests for the layer it touches.** Step 1 → MoveIntent emission per generator; Step 2 → Engagement vector consistency; Step 3 → DrillOp round-trip + analytical removal + drill-gate behavior; Step 4 → small-feature engagement vs analytical reference; Step 5 → mesh watertightness, wall verticality.
2. **WANAKA-style end-to-end revalidation.** After each step, re-run the WANAKA project through MCP. Specific deltas expected:
   - Step 1: TP3 air-cut drops from 92 % to ~30–50 % (retract reclassification). TP0 / TP2 engagement disappears from output, replaced by `not_applicable`.
   - Step 2: per-kinematics blocks appear. Optimizer can be re-tuned against per-axis engagement.
   - Step 3: TP0 / TP2 produce `DrillToolpathSummary` with peck pattern and chip-welding flags. Drill holes visible in screenshot at any resolution.
   - Step 4: TP7 (Ø1 mm tapered ball drop_cutter) engagement firms up; scallop visualization improves.
   - Step 5: Drill holes (and pocket walls, v-carve grooves) render with true vertical / conical walls at all resolutions.
3. **Counter-tests that prevent over-scope.** E.g. Step 1's retract reclassification must not affect adaptive3d's normal `is_cutting` time; Step 3's drill-CSG path must not affect milling ops that happen to use the same tool body.
4. **Benchmark snapshots.** Each step measures sim runtime on a fixed benchmark project. Regressions > 20 % require justification.

## 10. Open questions for independent review

1. ~~**Should `DrillOp` (Step 3) be a sibling of `Toolpath` or a richer toolpath variant?**~~ **Closed (2026-05-19):** `OpData { Toolpath(Arc<AnnotatedToolpath>), DrillOp(Arc<DrillOp>, Arc<AnnotatedToolpath>) }` enum wrapping `Arc<AnnotatedToolpath>` inside `ToolpathComputeResult` (`session/mod.rs:404`). The `DrillOp` variant carries *both* representations atomically — see §6.E dual-representation invariant.

2. **For F.a vs F.b: is the F.b sub-cell-resolved storage ever needed for this product?** F.b adds 16× memory per ray. Wood routing tolerances are typically ≥ 0.1 mm; F.a's coverage-weighted top-Z is probably indistinguishable from F.b at that scale. Recommend deferring F.b indefinitely unless adaptive3d's clearing engine demands it. *(Note 2026-05-19: F.a now exposes `coverage` per cell, so F.b is a strict superset when eventually needed — no API break.)*

3. **Should H deprecate the legacy `radial_engagement` field immediately, or carry it indefinitely?** Argument for immediate: clean API. Argument for indefinite: external G-code-and-metric consumers (the optimizer crate, narrate output, MCP) may have implicit expectations. Recommend one release deprecation window with a `#[deprecated]` attribute and a migration note in `CHANGELOG`. **Schedule the deletion PR as a tracked follow-up at the time of Step 2 landing — do not leave it as an open-ended "next release" promise.**

4. **Does CSG-based drill removal (Step 3) compose correctly with subsequent milling ops that pass over the drilled region?** Belief: yes, because drill removal is applied to the dexel state in-place before milling stamps it. Verify with a test: pocket-then-drill (existing pattern) and drill-then-pocket (less common but valid for through-holes that are tidied with a clean-up pass).

5. **Performance budget — promoted to hard acceptance gate (Revision 2026-05-19).** Wood-router sim today runs at ~5–10 minutes for a complex project. After Steps 1–5, expect ~7–14 minutes (Step 4 adds the most). **Each step ships with a measured sim-time delta on a fixed benchmark project; regression > 20% per step requires explicit justification, and cumulative regression > 50% requires user sign-off before Step 4 lands.** Existing benchmarks in `benches/perf_suite.rs` cover the relevant kernels (`bench_stamp_tool`, `bench_stamp_linear_segment`, `bench_simulate_toolpath`, `bench_dexel_mesh_extraction`).

6. ~~**Migration risk on existing saved projects.**~~ **Closed (2026-05-19):** `Toolpath` and `MoveType` are **not** persisted to project files — only `OperationConfig` is. Project format break risk is zero for Steps 1 and 3. The "is this a drill op?" detector for Step 3 is a trivial pattern match on the existing `OperationConfig::Drill | AlignmentPinDrill` enum tags (`catalog.rs:576,589`) — no heuristic, no separate detector module. `SimulationCutTrace` JSON files (cache and test fixtures) need a schema-version bump if `CutKinematics::Retract` is added (currently `SIMULATION_CUT_TRACE_SCHEMA_VERSION = 3` at `simulation_cut.rs:17`); use `#[serde(other)]` fallback for forward compat.

7. **Renderer migration order.** Step 3 introduces the drill-CSG mesh path; Step 5 replaces the heightmap mesh with marching cubes. Between Steps 3 and 5, the renderer composes two mesh sources (drill CSG + heightmap). The seam **will** be visible at low resolutions (heightmap tilted-quad walls at hole edges don't align with analytic cylinder walls). Add an explicit regression test for the Step-3-only intermediate state. Consider landing Step 5 before Step 3 if user-trust on the visual is higher-priority than drill-specific operability — the dependency from F.a → J is real but landing J first still produces cell-aligned vertical walls, which is a major improvement over today.

8. **Naming of `Engagement.radial_woc_fraction` vs legacy `radial_engagement`.** The new name is more precise. Migration cost is mostly grep-and-replace plus updating docs. Recommend renaming during Step 2. Note: 27 read sites need updating (not 15 as originally estimated), including two parallel re-implementations of `SummaryAccumulator` in `mcp.rs` — those are folded into Step 0 (independent dedup PR).

## 11. Out of scope

- Redesigning the tri-dexel base data structure (Z + lazy X + lazy Y grids). The structure is fine for this product's scope; the issues are downstream.
- The `peak_axial_doc_mm` lift-bridge artifact (CLAUDE.md April-2026 caveat). Tracked separately in `planning/AGENTSEARCH_INVESTIGATION_LOG.md` O5. Independent of this roadmap.
- The 2D-SVG stock-init "engagement-always-zero" caveat in `CLAUDE.md`. F-2 fix (`compute/simulate.rs:711-859`, commit 12dca81) closed it with regression test. Recommend a follow-up `CLAUDE.md` housekeeping pass after Step 1 lands to remove the stale caveat.
- Force/torque/thermal modeling beyond what an `Engagement` vector exposes (`Engagement.leading_edge_speed * Engagement.mean_chip_thickness_mm` is the substrate for a force model; the model itself is a separate workstream).
- Toolpath-quality feedback loops (sim → optimizer → re-generated toolpath). The data flow exists in places (agent_search clearing); making it a uniform mechanism is independent of this work.
- Material grain anisotropy for wood. A real "world-class wood router CAM" feature, but orthogonal to the simulator's fidelity defects this doc addresses.
- **Side-grid marching cubes (`side_grid_to_mesh` MC variant) — added 2026-05-19.** Speculative 4-axis future-proofing; not in active 3-axis-from-top flows. Pick up when 4-axis is genuinely on the roadmap.
- **G81/G82/G83 canned-cycle G-code emission — added 2026-05-19.** `gcode/program_builder.rs` is pure `MoveType` → `Statement` translation today. A `Statement::CannedDrill { x, y, z, r, f, cycle }` variant + emit path would unlock canned cycles. Doable but new work; orthogonal to the simulator fidelity defects this doc addresses. Most modern routers ingest expanded G-code fine.
- **F.b sub-cell-resolved ray storage.** Deferred indefinitely unless adaptive3d's clearing engine demands sub-cell decisions. F.a's `coverage` field keeps the API forward-compatible.
