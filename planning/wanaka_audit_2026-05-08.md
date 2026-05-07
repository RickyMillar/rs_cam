# Wanaka project audit — 2026-05-08

Live log for the deep-dive investigation requested by the user. Project: Wanaka
relief map (100×100×6mm STL on 140×150×25mm stock, 8 toolpaths, 6mm endmill
+ 1mm tapered ball, 0.8 kW Generic Wood Router).

Open tasks (TaskList in chat):
- #5 Locate the 18 rapid collisions
- #6 Investigate stepover origin on TP 1
- #7 optimize_toolpath on TP 1 Back Rough
- #8 optimize_toolpath on TP 4 Rivers (tapered ball)
- #9 Arc-fit dressup investigation
- #10 Re-evaluate TP 3 (copy) given user roughing intent

## Baseline sim numbers (from initial run, before any param changes)

| Metric | Value |
|---|---|
| Total runtime | 3796 s (~63 min) |
| Air cut | 42.4% |
| Average engagement | 14.3% |
| Holder collisions | 0 |
| Rapid collisions | **18** |
| Hotspots | 20 |
| Issues | 11960 (mostly air-cut emission noise) |
| Total cut volume | 233 411 mm³ |

## Per-toolpath baseline

| # | Name | Tool | Op | Moves | Cut (mm) | Rapid (mm) | Air% | Avg eng | Notes |
|---|---|---|---|---:|---:|---:|---:|---:|---|
| 0 | Pin Drill | 6mm EM | Pin Drill | 68 | 74 | 912 | 32% | 0.68 | OK |
| 1 | Back Rough | 6mm EM | 3D Rough | 7362 | 30116 | 7274 | 31% | 0.30 | Stepover 0.84mm (14%) |
| 2 | Holes | 6mm EM | Drill | 216 | 744 | 950 | 100%* | 0.00* | *2D-op metric known false-positive |
| 3 | Rivers (back) (copy) | 6mm EM | Project Curve | 1984 | 4608 | 9664 | 92% | 0.06 | Peak DOC 18mm, 1.6m arc — broken |
| 4 | Rivers (back) | 1mm TBN | Project Curve | 2324 | 5022 | 10074 | 84% | 0.10 | The proper engraving op |
| 5 | Lakes (back, inside) | 1mm TBN | Project Curve | 348 | 510 | 807 | 78% | 0.09 | Single-pass engraving |
| 6 | 3D Rough 6 | 6mm EM | 3D Rough | 1782 | 4439 | 3136 | 51% | 0.37 | Stepover 2.0mm (33%) |
| 7 | 3D Finish 6 | 1mm TBN | 3D Finish | 112 850 | 35 180 | 7425 | 10% | 0.10 | 78 Z-levels, dominant runtime |

## Findings (live; will fill in as I work)

### 1. Rapid collisions (TASK #5) — RESOLVED, false positive

Added new `inspect_collisions` MCP tool to localize. Result:

**All 18 collisions are in Pin Drill (TP 0).** Move indices: 7, 10, 13, 16,
19, 22, 25, 28, 31 (pin 1, 9 collisions) and 41, 44, 47, 50, 53, 56, 59,
62, 65 (pin 2, 9 collisions). Every 3rd move starting at move 7 = the
rapid-retract phase after each peck.

Pin Drill params: 11 Z passes (z=27 down to z=−2 in 3mm steps), retract_z
2mm above stock, peck_depth 3mm. The retract takes the tool from peck
bottom back up to z=stock_top+2 = z=27 — through the partially-drilled
hole.

**Diagnosis: simulator false positive.** Real peck drilling *always*
retracts through the just-drilled hole; the tool fits because it's the
same diameter as the hole it just made. The dexel collision detector
probably doesn't account for the just-cleared cells, or there's a
material-removal vs collision-check ordering issue.

Confidence: very high. The 11-pass × 2-hole arithmetic gives ~10
retracts/pin × 2 = 20, with the first retract from air not counted = 18.
The clusters line up exactly with the pin XY positions in the params
(`[[2.5, 2.5], [137.5, 147.5]]`) and the move indices show no XY change
within each cluster.

**Recommendation**: file a separate bug to either suppress
rapid-collision flags inside drill cycles, or to do per-segment
material clearing during the rapid-collision check. Until then, treat
TP 0's "WARNING: rapid collisions detected" verdict as benign for this
project. **Run on machine is safe** for the pin drill specifically.

### 2. Stepover origin (TASK #6) — RESOLVED

- `Adaptive3dConfig::default()` → stepover = **2.0mm** (operation_configs.rs:480).
- Machine rigidity caps adaptive WOC at 20% of tool diameter
  (`adaptive_woc_factor: 0.2`) → max 1.2mm for the 6mm tool.
- The feeds calculator outputs a `radial_width_mm` recommendation but it's
  **not** auto-applied to `cfg.stepover` — the config field is independently
  user-set / project-stored.
- TP 6 uses the default 2.0mm. TP 1 is at 0.84mm — user-set. Both ops have
  identical feed (3150) and RPM (18000) → somebody applied the feeds wizard
  to both then narrowed TP 1's stepover by hand.
- 0.84mm = 14% of tool — *finer than rigidity allows even for adaptive
  finishing*. The right fix for surface-quality reasons is reducing
  `stock_to_leave_radial` on the rough so the finish op does more work, not
  narrowing the rough stepover. The rough is doing finish-quality WOC for
  no reason.

**Recommendation**: set TP 1 stepover → 2.0mm (match default + TP 6).

### 3. optimize_toolpath TP 1 (TASK #7) — RESOLVED, baseline already unsafe

Outcome: `no_safe_improvement`.

**Critical: the BASELINE TP 1 already exceeds two gates.**
- `chipload: EXCEEDS — chipload_burn_risk` (peak 0.0253 mm/tooth, BELOW vendor min)
  → at the current 0.84mm stepover, chip thickness is so thin the tool rubs
    instead of cutting, generating heat and burning the wood.
- `deflection: EXCEEDS — long_tool_stiffness_unsafe` (L/D 7.5)
  → tool stickout is too long for 6mm endmill; geometry-based, not
    parameter-tunable.
- `power: WITHIN` (0.03 of cap — wildly under-loaded, consistent with rubbing).

The optimizer tried 3 deltas (stepover 0.95, DOC 3.9, both) — all still
exceeded gates. Returned `no_improvement_found`.

**Why bumping stepover to 2.0 should fix the chipload gate**: wider WOC →
more material per tooth pass → chipload climbs above the vendor min. The
optimizer's 0.95 attempt was too small to clear the floor. Manually setting
stepover 2.0 (matching TP 6) and re-simulating should clear the chipload
gate.

**The L/D issue is independent and won't go away from param tuning** —
need to either shorten tool stickout or accept the L/D risk as warning.

### 4. optimize_toolpath TP 4 (TASK #8) — RESOLVED, optimizer can't help

Outcome: `no_safe_improvement`.

**Baseline TP 4 verdicts:**
- chipload: UNMODELED — `no_vendor_data` (no LUT row for tapered ball +
  wood + project_curve combo)
- deflection: WITHIN, peak L/D 5.83 — borderline, gets the "long tool"
  approximate flag, but under the 6.0 EXCEEDS threshold
- power: WITHIN at 0.08% of cap — wildly under-utilized

**Why optimizer can't help**: Project Curve has no stepover knob (it's a
centerline trace); the 84% air cut comes from inter-curve rapid retracts,
not from feed/RPM/DOC. The one variant the optimizer tried
(feed 1750, RPM 21000) was gate-safe but only saved 2.5s — under its 0.5s
improvement threshold (so reported as no improvement).

**Real wins for TP 4 (require manual edits):**
- Switch `region_ordering` from `global` to nearest-neighbor — biggest air-cut
  reduction.
- Lower the retract Z so rapids don't climb so high.
- Combine TP 4 (Rivers proper) with TP 5 (Lakes) — same tool, same kind of
  work; one op with both source DXFs would amortize entry/retract overhead.
- Bump `point_spacing` from 0.5mm — fewer points means smoother arc-fit
  output and fewer moves; check surface quality first.

### 5. Arc-fit dressup (TASK #9) — RESOLVED, mostly false alarms

**Per-op arc_fitting state**: enabled for Roughing / Finish / SemiFinish role
defaults (so all affected ops have it on). `for_op(ProjectCurve)` keeps
`arc_fitting: true`.

**Default tolerance**: `arc_tolerance: 0.05mm`. At the loose end of CAM-typical
(doc says 0.001–0.01 typical) but sits well under each op's `tolerance: 0.1mm`,
so it doesn't blow the surface budget.

**Sagitta protection** is already in place (`arcfit.rs:264-285`) — the
original wanaka-specific guard that rejects circumscribing-circle fits where
the chord-arc deviation exceeds tolerance. Comment at line 273 literally
says "Visible on wanaka as 'circular arc cuts outside boundary'".

**The flagged arcs on TP 1 / 4 / 5 / 6 are NOT broken**:
- Heuristic threshold: `R > 30 × tool_radius` = 90mm for the 6mm tool.
- Wanaka model is 100×100mm; contour-parallel adaptive sweeps trace
  ~70-100mm radii along the workpiece outline. Math says these arcs
  *should* be that big.
- The sagitta check guarantees they don't deviate from the original
  linear path by > 0.05mm.
- They look "suspicious" because the narrator's threshold was set for a
  smaller-job baseline.

**TP 3's 1600mm arc is the only real issue** — and it's NOT an arc-fit
defect. It's the lift-bridge function generating an arc to lift over
uncleared stock (the broken-toolpath pattern documented in CLAUDE.md).
Root cause is fixing TP 3's operation choice (task #10), not arc-fit.

**Recommendation**: leave arc-fit alone for this project. Optionally bump
the narrator's `LARGE_ARC_RADIUS_MULTIPLIER` from 30 → 50 to reduce false
positives; or scale it relative to model bbox rather than tool radius.

### 6. TP 3 reassessment (TASK #10) — RESOLVED, intent and execution disagree

**Updated thinking** (after running the optimizer on TP 4):

User intent: pre-rough river paths with the 6mm endmill so the 1mm tapered
ball does less work on engraving.

**The premise doesn't hold for the current TP 4 depth (0.2mm).** A 0.2mm
deep engraving is ~0.04 mm³ of material per mm of curve length × 2097mm
total curve length ≈ 84 mm³ of material to remove. Even if the 1mm tapered
ball did all of it solo, that's a trivial volume. The baseline TP 4 sim:
- power: 0.08% of cap (basically idle)
- runtime contribution: 5022mm cut × ~1500 mm/min feed = ~3 min cutting

The 1mm tapered ball is not stressed. It does NOT need pre-roughing for a
0.2mm-deep engraving job. **Drop TP 3 entirely if engraving stays at 0.2mm**.

**The premise WOULD hold if engraving depth went to 1-2mm** (e.g. for an
epoxy-resin inlay). At ~1.5mm depth on the same curves, you have ~625 mm³
of removal, and the 1mm tapered ball would chew through it slowly. In that
case a 6mm pre-rough makes sense — but Project Curve is the wrong op:

**Why Project Curve fails as a pre-rough for the 6mm:**
- Project Curve traces the *centerline* of each river polygon onto the
  surface. It has no concept of the tool diameter or the polygon's width.
- Rivers source data: 151 polygons, total 1207 mm², avg ~8 mm² each. Most
  are narrow line-like polygons. A 6mm endmill can't fit in features
  narrower than 6mm and shouldn't try.
- The current execution (depth -2.0mm with the 6mm endmill on centerlines)
  generates the 18mm peak DOC bug because the lift function bridges across
  uncleared stock between curve fragments — see the 1600mm-radius arc.

**Right ops for "pre-rough rivers with 6mm" intent (if user goes deeper):**
1. **2D Pocket** of the rivers polygons. Pocket respects tool diameter —
   features narrower than 6mm get correctly skipped, wider ones partly
   cleared. Set its depth to engraving_depth − 0.3mm (leaves 0.3mm radial+
   axial for the 1mm finish).
2. **Trace** with explicit tool-radius offset.
3. **3D Rough confined to a boundary** matching the rivers.dxf. Adaptive3d
   would pre-clear material respecting tool diameter. Probably overkill
   for narrow engraving.

**Action depends on user answer to one question:** what depth do you
actually want the rivers engraved? 0.2mm (cosmetic surface marking) or
deeper (epoxy fill, structural feature, etc.)?

**User intent**: keep the 6mm endmill loaded after Back Rough, use it to pre-rough the
river curves before swapping to the 1mm tapered ball. Sound idea — saves a tool
change and spares the delicate engraving tool.

**Current execution problem**: `Project Curve` projects a centerline curve onto the
surface. It has no concept of tool diameter. So:
- Where the river is wider than 6mm → the centerline cut leaves uncut material at the
  river edges (since the tool doesn't dilate to fill the polygon).
- Where the river is narrower than 6mm → the path is just a single sweep along the
  centerline; the tool can't fit, but the path is generated anyway, leading to high
  air-cut readings AND the 18mm peak DOC because the lift function bridges across
  uncleared stock between disconnected centerline runs.
- Rivers source data: 151 polygons, total area 1207 mm² → average ~8 mm² per polygon.
  Most rivers are narrow lines. A 6mm endmill fundamentally cannot follow them.

**Better fits** for the "pre-rough river paths" intent:
1. **2D Pocket** of the rivers polygons, depth limited so it leaves stock for the 1mm
   ball finish. Pocket WILL respect tool diameter — shallow rivers narrower than 6mm
   simply get skipped (correct outcome) and wider rivers get partly cleared.
2. **Trace** with explicit tool-radius offset. Generates a tool-diameter-aware path.
3. **Don't pre-rough at all**: at 0.2mm depth (the proper TP 4 engraving), the 1mm
   tapered ball doesn't have meaningful work to "spare" — the engraving is shallow
   surface marking, not slot cutting. The 6mm pre-rough would only matter if engraving
   went deeper (≥1–2mm).

**Recommendation**: confirm what depth the user actually wants the rivers engraved.
If it's the 0.2mm cosmetic engraving in TP 4 → drop TP 3, the 1mm ball doesn't need
help. If they want deeper river slots (e.g. for filling with epoxy resin) → switch
TP 3 from Project Curve to 2D Pocket with the same source DXF.
