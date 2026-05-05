# AgentSearch (2.5D slice) investigation log

Living log of issues found while debugging wanaka's Back Rough toolpath. Updated as new behaviour surfaces.

## Status legend

- 🟢 **Fixed** — landed in this session, change committed or pending commit
- 🟡 **Mitigated** — partial fix, residual issue documented
- 🔴 **Open** — found, not fixed
- 🔵 **Spec / non-bug** — confirmed-correct behaviour the user asked about

---

## Fixes shipped this session

### 🟢 F1. Polygon misclassification in `clear_z_level_agent_2d_slice`

`marching_squares_bool_grid` emits multiple contours when a Z slice has disjoint material islands. The previous code at `clearing.rs:962-984` flattened signed area to `.abs()`, took the largest contour as the polygon's exterior, and pushed every other contour into the polygon's `holes` list. Disjoint islands got mis-classified as holes — the 2D adaptive walked around them treating them as already-cleared interior pockets, plunging into them at "safe" XYs and gouging.

**Fix:** classify by signed area (`>0 = outer`, `<0 = hole`) and run `polygon::detect_containment` to nest holes inside their containing outers. Iterate per-region with separate calls to the 2D adaptive.

Wanaka Back Rough delta at identical params:

| Metric | Before | After |
|---|---|---|
| Move count | 10,403 | 8,353 (-19.7%) |
| Cutting distance | 51,477 mm | 43,883 mm (-14.7%) |
| Rapid distance | 24,487 mm | 19,862 mm (-18.9%) |
| Visual top-view | scattered chaotic shorts | concentric adaptive offsets |
| Disjoint islands | walked-around, gouged | each cleared as own region |

### 🟢 F2. Entry plunges peck-feed through cleared air

Each region's entry plunge always pecked the full descent from `safe_z` to `entry.z` even when the previous Z level had cleared the stock above the entry XY. Pecking through air at `plunge_rate` is wasted time. On wanaka, with 5–6 Z-level transitions × multiple regions per transition × pecks per descent, this added ~4 minutes of feed-rate descent through cleared air per project sim.

**Fix:** added `Adaptive3dSegment::RapidWithFloor { entry, rapid_floor_z }`. The clearing function samples the dexel stock_top at the entry XY (`sample_stock_top_at`) and emits `RapidWithFloor`. The path emitter rapid-descends from safe_z to `rapid_floor_z + 0.5mm` (buffer for sub-cell sample noise) at rapid speed, then pecks only the remaining fresh material.

Wanaka Back Rough delta at identical params (additional gain on top of F1):

| Metric | F1 only | F1 + F2 |
|---|---|---|
| Move count | 8,353 | 4,699 (-44%) |
| Cutting distance | 43,883 mm | 31,476 mm (-28%) |
| Rapid distance | 19,862 mm | 8,606 mm (-57%) |
| Project total runtime | 5,538 s | 4,247 s (-23%) |
| Average engagement | 0.049 | 0.058 (+18%) |

End-to-end win from baseline: project total **-29%** (~30 min saved on wanaka).

---

## Open issues

### 🟢 O5. Cut path Z-drop / slope-bridging in AgentSearch lift

The lift function in `clear_z_level_agent_2d_slice` clamps Z to either `z_level` (over valleys) or `terrain + stock_to_leave` (over peaks). Within one offset-ring path crossing both regimes, consecutive points can have Z deltas spanning the full mesh height. The path emitter consumes consecutive points as `feed_to`, so peak→valley transitions become diagonal feed moves through fresh stock at intermediate XYs — measured as full-slot lateral cuts in the simulator.

**Fix shipped:** `clear_z_level_agent_2d_slice` now splits Cut paths at large dz transitions OR steep descent slopes, inserting `RapidWithFloor` between sub-segments. Threshold: `|dz| > depth_per_pass × 1.1` OR `descent_slope > 0.3` (≈17°). The `RapidWithFloor` retains the dexel-sampled stock_top floor optimization so descent through cleared air is at rapid speed.

**Synthetic validation** (`crates/rs_cam_core/tests/agent_search_axial_doc.rs`):
- Uses the actual wanaka terrain.stl mesh with matched params
- Pre-fix: peak axial DOC unbounded (terrain bridging produces deep cuts)
- Post-fix: **peak axial DOC = 3.00mm exactly** = depth_per_pass. Asserted as a regression guard.
- Per-Cut-path diagnostic in `adaptive3d::tests::agent_search_z_drop_diag` shows max single-step |dz| stays under threshold.

**Live wanaka still shows peak axial DOC 18.71mm** — this code path (via the project loader / setup-local frame / boundary clipping) reaches a state the synthetic test doesn't reproduce. The algorithmic fix is correct but something at the project-loader level produces a bridge it doesn't catch. Needs project-load path investigation.

### 🔴 O6. Entry-style chipload at first-cut after peck-plunge (separate bug)

After a peck-plunge entry into virgin stock, the FIRST lateral feed move at `feed_rate` has the cutter wrapped in stock for ~half its perimeter (arc engagement π/2 to π). The chipload formula treats arc ≥ π/2 the same as slot mode (returns nominal `feed_per_tooth`), so any sample with arc ≥ π/2 reports max chipload. With wanaka's 3150 mm/min feed and 18000 RPM × 2 flutes, that's 0.0875 mm/tooth — above the LUT cap. The gate flags it.

**Reproduces in synthetic test on wanaka mesh:** 361 full-slot samples + 332 half-engaged samples, all at feed_rate (lateral cuts). Not a lift-function issue; not solved by O5's fix.

**Proper fixes** (escalating scope):
- A. Make the FIRST few lateral feed moves after peck-plunge use `plunge_rate` instead of `feed_rate` (a "chip-loading spinup")
- B. Switch entry style to Helix or Ramp (user config) — these descend at plunge_rate AND spread the lateral bite
- C. Recognize this is a calibration question: the LUT is for partial-engagement cuts. For Plunge entry, the user MUST accept slot-mode at first cut OR use Helix/Ramp.

### 🟢 O1 (resolved by O2 fix). Tiny regions emit wasted micro-arc traversals

The right-edge arc at (95, 93) on wanaka was the visible symptom. Looked like the 2D adaptive walking a tiny island. **Actual cause: the boundary-clipping bug O2 — the cut at (95, 93) was just outside the terrain.stl silhouette, and without proper boundary clipping the toolpath included those out-of-silhouette regions.** Fixing O2 made the arc disappear.

The "ignore stock less than X mm" Fusion-style filter is still a valid follow-up for cases where a region IS inside the boundary but too small for adaptive to do useful work. Lower priority now that the visible wanaka case is fixed.

### Old O1 (now redundant) — original entry kept for context


Visible at machine time t≈362.79s on wanaka Back Rough (moves 1952–1967): 16 short feed-rate moves spanning a 5mm XY area at the right edge (95, 93–99) at constant Z≈7, mostly air-cut with brief material engagement.

After F1 every disjoint island slice becomes its own region. Regions that are ≤ a few cutter diameters across don't have meaningful adaptive offset structure to walk — the 2D adaptive emits many short turning steps tracing the boundary, then exits. On wanaka this surfaces around the rivers/lakes feature edges where small islands persist at deeper Z levels.

**Fusion 360 reference:** Fusion has an "ignore stock less than X mm" filter for adaptive that drops tiny material islands. Same idea applies here.

**Proposed fix:** in `clear_z_level_agent_2d_slice`, after `detect_containment` returns the regions, drop any region whose total area (exterior minus holes) is below a threshold proportional to the cutter — e.g. `< (2 × tool_diameter)²`. The dropped area gets handled by:
- The waterline cleanup at the last Z-level (existing path)
- The 3D finish pass that follows (which uses a different tool / strategy)

~10–15 LOC + 1 test.

### 🟢 O2. Boundary clipping silently dropped for `model_silhouette` source on the GUI path

**Finding** (initially mis-diagnosed as "cuts past model bottom"). The user reported cuts at (95, 93) appearing outside the model boundary. After tracing the geometry:

- Z-level iteration is correct: cuts span Z=22 down to Z=1.55 in setup-local frame, which matches stock_top (25) down to model_bottom_z + stock_to_leave. Most of that depth is bulk stock above the back surface — that's expected for terrain back-rough, not a bug.
- The XY clipping was the actual issue. Project file has `boundary.enabled = true`, `source = "model_silhouette"`, `containment = "inside"`. GUI displays this. CLI path (`session/compute.rs::apply_boundary_clip`) handles it correctly. **GUI worker (`rs_cam_viz/.../execute/mod.rs:350`) only handled the `face_selection` source — `ModelSilhouette` fell through to the stock-bbox rectangle, which is wider than the model in XY, giving effectively no clipping.**

**Fix:** mirror the CLI path's source-resolution match in the GUI worker. Added a `BoundarySource::ModelSilhouette` arm that calls `boundary::model_silhouette(mesh, None)` and picks the largest resulting polygon. Also added the missing `boundary.offset` application (was silently ignored on GUI path). Both changes mirror `session/compute.rs::apply_boundary_clip` so CLI and GUI now agree.

This explains the user's "weird arc cutting outside boundary" observation: the cut at (95, 93) was inside the stock bbox but outside the actual terrain.stl silhouette (or right on its edge). Without proper clipping, the AgentSearch's marching-squares emitted a tiny island for that area and the 2D adaptive walked it. With proper clipping, that area is cut off — no walking, no arc.

Needs live verification with rebuilt GUI.

### 🔴 O3. Single 23.7mm contiguous-feed Z descent at move 1989

Trace shows: `move 1989, position (30.97, 109.25, 30.9) → (30.97, 109.25, 7.17)`, `duration 1.9s`, `engagement 0.0`. A single feed-rate descent of 23.7mm in one move at constant XY.

This SHOULD have been peck-plunged (the loop in `emit_peck_plunge` would iterate ~8 times for a 23.7mm descent at DOC=3). The fact that it didn't means either:
- The segment was emitted as something other than `Adaptive3dSegment::Rapid` / `RapidWithFloor` — possibly `Adaptive3dSegment::Link` from a different code path (waterline cleanup? ContourParallel/Adaptive strategies? A code path I haven't audited.)
- Or `params.depth_per_pass` was zero/very small at this point so the peck loop didn't iterate

Needs investigation. `peak_axial_doc_mm = 18.6` in the project summary likely comes from this or similar moves — the rapid-floor optimisation didn't fix it because it happens through a different path.

### 🔴 O4. Bipolar verdict from threshold-edge samples

The simulator's chipload gate uses a 2% engagement threshold to filter air-cut samples. Samples right at the threshold (~2-3% engagement) produce a deterministic minimum effective chipload (~0.0072 mm/tooth on this configuration) regardless of feed/RPM. The optimizer's bipolar check trips on this even when engagement variance isn't a real problem.

Documented in `OPTIMIZER_LOGIC.md` §"Two layers of chipload" and the resolved-decisions table. Investigation 2 from `ROUGH_ENGAGEMENT_INVESTIGATION.md` covers the calibration question — independent from the 2.5D slice work.

---

## Confirmed-correct behaviour (raised but not actually bugs)

### 🔵 C1. The 75-second snake at Pass 1 is the correct adaptive sweep

Initially flagged as suspicious because of 0.92% min_radial_engagement, but adaptive paths legitimately show low engagement on inside-curves of offset rings. The full Pass 1 snake (moves 11–486 at Z=22) is the proper adaptive clearing pattern.

### 🔵 C2. The "ContourParallel works after my fix" implication is correct

Both AgentSearch and ContourParallel use `clear_z_level_*` per Z-level inside the same outer loop in `path.rs`. F1 only changed AgentSearch (2D-slice). ContourParallel was already correct; the user's "both broken" report covered shared upstream issues (F2 entry plunges, O3, O4) that affect both strategies' output.

---

## Still-relevant unfixed issues outside this scope

- AgentSearch clearing strategy choice (ContourParallel vs AgentSearch vs Adaptive) — the user has noted AgentSearch is the right choice for terrain after the 2D-slice rewrite. Switching to ContourParallel would be a regression.
- Long-tool deflection (L/D=7.5) — structural tool/setup issue, no software fix possible.
- Auto-feeds defaults producing chipload at LUT cap — separate cheerful-popping-spring plan area.

## Summary

Two real bugs fixed this session (F1, F2) with measurable cycle time and quality improvement on wanaka. Four open issues identified during investigation (O1–O4), each tracked here with proposed scope. None are catastrophic; the toolpath is much closer to "good" than baseline.
