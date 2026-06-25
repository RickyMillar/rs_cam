# Project: Speed up freeform-relief finishing

**Umbrella tracker.** Goal: finish carved reliefs (wanaka-class, organic rivermap
meshes) **fast without losing detail**. Single source of truth across phases; links
out to the detailed docs. Owner: Ricky. Started 2026-06-24.

Sub-docs:
- A/B experiment + multi-tool roadmap: `planning/multitool_finishing_plan.md`
- Literature basis + citations: `research/multitool_finishing_optimization.md`
- Pencil rebuild design: this doc, §Phase 2.

---

## Phase status

| Phase | What | Status |
|-------|------|--------|
| **1. Single-tool path strategy** | Replace drop_cutter raster finish with continuous constant-scallop | ✅ **DONE** — 3.4× faster, whole wanaka200 job 8.26→4.24 hr, 0 collisions, full detail. Live in GUI (not saved to .toml per user). |
| **2. Pencil rebuild (tool-radius-aware)** | Fix valley/detail finishing so a small tool cleans only the genuine valleys — F360-quality coverage | 🔜 **IN PROGRESS** (this doc §Phase 2) |
| **3. Analysis step (reach-radius map)** | "ratio of areas within a given radius" → which tools reach what; drives tool-kit + region assignment. (= old Path C) | ⬜ NEXT, after Phase 2 |
| **4. Multi-tool optimizer** | Auto bulk+detail tool-set scored by accel-aware cycle time (= old Path D) | ⬜ gated on Phase 3 + a positive Phase-2 #3 re-test |

**Through-line:** Phase 1 proved the single-tool path-strategy win. Phase 2 makes the
*detail* tool usable on organic relief (today's pencil is broken for this — see below),
which is the missing primitive that made the multi-tool #3 variant fail in the A/B.
Phase 3 (reach-radius) is the user's original idea and the analysis that feeds Phase 4.

---

## Phase 2 — Pencil rebuild

### Problem (measured on wanaka200, 1mm tapered ball)
Pencil is unusable on the organic relief: `bitangency_angle=160°` → **366,672 moves /
1,323 m cutting / 335 m rapids**; `=120°` → **85 moves**. No usable middle. **But F360's
pencil cleans this surface fine** (valley bottoms get cut, mostly) → the surface is fine;
**our detector is wrong.**

### Root cause (code, `crates/rs_cam_core/src/pencil.rs`)
1. **Detection is raw mesh-dihedral (tool-radius-blind).** `pencil.rs:620-625` keeps an
   edge if `is_concave && dihedral_angle > (PI − threshold)`. On a dense triangulated
   organic mesh, almost every edge has *some* concave dihedral, so the angle threshold
   is a knife-edge between "all the triangulation noise" and "nothing." There is no
   notion of "concave *at the scale of the 1mm tool*." This is the dominant bug.
2. **`hookup_distance` is dead code.** Declared + documented (`pencil.rs:38`) but never
   read in `pencil_toolpath_structured_annotated`. Every fragment is emitted via
   `emit_path_segment_with_intent` (`pencil.rs:736`), which always retracts to safe_z at
   both ends (`toolpath.rs:211`). One full retract per micro-fragment ⇒ the 335 m rapids.
   The nearest-neighbour TSP (`order_paths_nearest`, `pencil.rs:496`) is real and correct
   but can only reorder retracts, not merge fragments.

### What F360 / the literature do (already in our research log, item 5)
Detect pencil curves in the **tool-radius offset / CL-surface domain**, not raw mesh
angle. A pencil curve = where the tool, offset by its radius, makes **bitangent (2-point)
contact** — found via self-intersection of the offset surface (**Park & Chung 2006**,
"Pencil curve detection from visibility data," CAD 38(3):223-231; **Ren, Zhu & Lee 2005**,
material-side tracing for pencil-cut on meshes, CAD 37(10):1015-1026). This is
**scale-aware by construction**: concavities broader than the tool radius produce no
pencil curve; micro-ripples and triangulation noise don't self-intersect at the 1mm
scale; genuine valleys produce clean connected curves.

### Plan (staged; iterate via core tests, GUI/MCP screenshot at each gate)

**Stage 1 — tool-radius-aware detection (the core).** Make a point qualify as a pencil
point by a *reach-gap (bridging) test* rather than raw edge angle:
- At a candidate point, drop the **ball cutter of the tool radius** (reuse
  `point_drop_cutter`) to get its CL rest height, and probe the **true surface height**
  (small/needle drop or vertical ray). If the tool tip sits **above** the true surface by
  more than a threshold, the tool *bridges* the valley → uncut material → pencil point.
  If the tool reaches the bottom, the regular finish already got it → skip.
- This is the in-process-difference *at a point*, computed analytically, and it is
  tool-radius-aware for free. The threshold can later be set to "what the bulk (6mm) tool
  left" → this *is* the rest-region detector for the multi-tool #3 variant.
- Seed candidate points cheaply (e.g. the existing concave-edge set as a candidate mask,
  or a coarse grid), then gate by the reach-gap test; chain the survivors.
- Keep the literature offset-surface self-intersection (Park & Chung) as the fidelity
  north-star if the reach-gap test under-covers.

**Stage 2 — linking.** Wire the dead `hookup_distance`: when consecutive ordered chains
have endpoints within `hookup_distance` (XY, with a clearance check), connect with a
feed/skim move at safe height instead of a full retract-to-safe-Z cycle. Re-test rapids.

**Stage 3 — validation + multi-tool re-test.**
- Visual bar: on wanaka200, valley bottoms cut, broad faces left to the bulk/scallop —
  compare against the F360 mental model ("valley bottoms get cut, mostly"). GUI/MCP
  screenshot.
- Re-run A/B variant **#3** (6mm bulk scallop + rebuilt-pencil valleys) vs Phase-1 #1
  (single 1mm scallop). If #3 wins on accel-aware `total_runtime_s` incl. one manual tool
  change → multi-tool finishing pays after all → green-light Phase 3/4.

### Stage 0 finding (2026-06-24)
In-repo fixtures landed (`pencil.rs` tests: gentle sine surface + V-groove,
`test_pencil_tool_radius_aware_gentle_vs_valley`, green). **Key insight: a clean
analytic surface does NOT reproduce the 366k-move explosion** — smooth troughs chain
coherently into a few chains. The blowup is **triangulation-noise-specific** to the
organic scanned mesh (per-triangle normal jitter → many tiny disconnected concave edges
that each become a fragment). ⇒ Stage 1 must reject *triangulation-scale* concavity and
keep only *tool-unreachable* valleys, and must be validated against the **real mesh**.
Plan: add an opt-in `#[ignore]` test loading the wanaka200 front mesh via env var
(`RS_CAM_PENCIL_FIXTURE`) reporting chain/move/rapid baseline — faithful repro, no GUI
restart. The synthetic test stays as a clean-surface regression guard.

### Stage 1 — reach-gap gate — LANDED + VALIDATED (2026-06-25)
Gate each angle-filtered concave edge by a tool-radius-aware *reach gap* =
`point_drop_cutter(midpoint.xy).z − midpoint.z` (footprint-aware drop minus the
on-surface seam midpoint); keep where `gap > 0.05mm`. Tool reaches flats/noise (gap≈0,
rejected); bridges genuine valleys (gap>0, kept). Code in `pencil.rs`
(`reach_gap_at_edge`, `reach_gap_threshold`, the Step-3 gate).

**False alarm, then recovery (important):** first pass *looked* dead — a 6mm ball over a
`make_v_groove` seam returned `cl.z=−5.0` (gap 0). Root cause was NOT `drop_cutter`: the
**`make_v_groove` test helper is wound so its walls face DOWN** (`wall normal z = −0.894`),
so `drop_cutter` rightly skips them and only the apex edge registers. On a **correctly-
wound** valley (`make_v_valley`, wall normal z = +0.894) the 6mm ball rests at the
geometric `cl.z=−4.646 → gap=0.354`. `drop_cutter` is fine; the gate is valid. Switched
the two toolpath tests off `make_v_groove`. 10/10 pencil unit tests green, clippy clean.

**Real-mesh result (terrain.stl, 661,212 tris, 1mm ball, bitangency 160, gap 0.05mm):**
`angle_chains 9277 → gated_chains 6994`; `gated_moves 64,256` (vs **366k pre-gate**, ~5.7×);
cutting 81m, **rapid 77m**. Verdict: the gate is **correct and principled but modest on
this surface** — the rivermap is tool-unreachable almost everywhere at 0.05mm, so it only
drops the ~25% tool-reachable chains. It is NOT sufficient alone.

**The two remaining levers (the real win is here):**
1. **Stage 2 — linking (`hookup_distance`, still dead).** 77m of rapids = one
   retract-to-safe-Z per fragment across ~7k chains. Wiring hookup (connect near chain
   endpoints with a low feed/skim instead of a full retract) is the biggest single
   reduction. ⚠ collision-safety: a link must be verified gouge-free (drop-cutter sample
   along it, else fall back to full retract) — do it test-verifiable, and confirm
   `rapid_collision_count==0` in sim before trusting it.
2. **Scale selection / reference tool.** ~7k chains = the fine texture as well as the
   rivers. For STANDALONE pencil, raise `gap`/`min_cut_length` to isolate the deep
   channels. For the **multi-tool #3**, the right reference is the **6mm BULK tool**: gate
   = "where the 6mm left material" → only the deep/narrow valleys the bulk couldn't reach,
   cut by the 1mm. That needs a small new param (`reference_tool_diameter`) decoupling
   detection-tool from cutting-tool. Worth measuring the 6mm-reference chain count next.

### Live demo + `min_valley_depth` dial (2026-06-25)
Demoed the gated pencil live on wanaka200's `terrain.stl` (2mm-tip ball, bitangency 160).
Result: **103,440 moves / 378m cut / 104m rapid** (vs ~366k pre-gate) — but the screenshot
showed it tracing the **entire textured surface as a solid fuzzy field, no clean channels**
(user: "scattered on all the tiny gaps… no long channels"). Root cause: the reach-gap
tolerance was **hardcoded at 0.05mm**, and this rough rivermap is concave deeper than 0.05mm
almost everywhere, so the gate keeps nearly all of it. `min_cut_length`/`bitangency` can't fix
it (texture fragments even the real channels).

**Fix landed: exposed the tolerance as a pencil param `min_valley_depth`** (mm). Plumbed
through `PencilParams` (gate now reads `params.min_valley_depth`), `PencilConfig`
(`#[serde(default = "crate::pencil::reach_gap_threshold")]` so old projects load),
`compute/catalog.rs` schema, and `execute.rs`. Default 0.05 (unchanged behaviour). 10/10
pencil tests green, all core test targets compile. **Requires a GUI rebuild + MCP restart to
use the new param live.** Next: sweep `min_valley_depth` 0.1→0.5mm on terrain.stl, screenshot
each, find where the texture drops out and only the river channels remain (= the visual bar
"valley bottoms, mostly"). If even at high tolerance there are no long channels, the front
mesh genuinely lacks deep channels and pencil is the wrong tool for it.

### ROOT-CAUSE BUG: concavity test was ~46% wrong (2026-06-25)
The "fuzzy fill, no channels" was NOT a surface problem (F360 finds the creases cleanly) —
it was a **detection bug**. `compute_shared_edges` decided concave-vs-convex with
`cross(n1,n2) · edge_vec > 0`, where `n1`/`n2` are in arbitrary face-insertion order and
`edge_vec` in arbitrary vertex-index order — **no geometric link between them**. Measured on
terrain.stl: of 426,032 edges with dihedral >20°, the sign test **disagreed with correct
geometry on 196,900 (46%)** — essentially a coin flip. So pencil traced a random mix of
ridges, valleys and noise → a fuzzy band instead of the valley/crease network.

Tell-tale: the **comment** already described the correct test ("the opposite vertex of face B
below face A's plane") but the **code** implemented the broken sign test. Fixed to the
geometric apex test: `is_concave = (apexB − edgePt)·nA > 0` (face B's far vertex above face
A's outward plane → the facets fold up toward each other → valley). Added a ridge-must-be-
convex regression test, and corrected `make_v_groove`'s winding (it was downward-wound, which
inverted both this test and drop_cutter). 11/11 pencil tests green, clippy clean.

**Perf (user flagged pencil slow vs F360):** parallelised the reach-gap gate (the ~213k
per-edge `drop_cutter` calls I'd added) across cores via rayon (`MillingCutter: Send+Sync`,
`default=["parallel"]`). Further levers, not yet done: (a) the full 1M-edge analysis re-runs
on every `min_valley_depth` change even though only the gate threshold moved — cache the edge
analysis so re-gating is near-instant; (b) the std `HashMap` (SipHash) in
`build_edge_adjacency`/`chain_concave_edges` is slow on 1M edges — `FxHashMap` would help.

**Needs a GUI rebuild + MCP restart** to see the fixed (clean-crease) + faster pencil live.

### Pencil speed: 40s → 10s (2026-06-25)
Profiled the generate on terrain.stl (661k tris). Phase timings showed the HashMap
detection is cheap (~2s); the 38s was all `drop_cutter` work. Three fixes (all keep
11/11 pencil tests green, clippy clean):
1. **Density-aware `SpatialIndex::build_auto`** (`mesh.rs`). The old `extent/50` rule
   gave ~4mm cells / ~400 triangles per cell on this mesh; every drop tested all ~400.
   Now targets ~8 triangles/cell (≈0.7mm here) with a 1M-cell memory cap. Benefits ALL
   drop-heavy ops (pencil, scallop, waterline, simulate, collision) in the GUI.
2. **Chain-level reach-gap gate** (`gate_chains_by_depth`, replacing the per-edge
   `filter_by_reach_gap`). Chain first, then sample ~8 vertices per chain and keep it if
   the MEDIAN gap clears the threshold. ~213k per-edge drops → a few per chain. Also
   drops shallow-texture chains wholesale (cleaner) and is parallelised across cores.
3. **Parallelised `lift_to_surface`** (rayon, per-point).
Result: `full_generate 40s → 10.3s` (~3.9×). Visual unchanged (same dendritic network).
Remaining lever (not done): when only `min_valley_depth` changes, the edge analysis +
chaining are identical — caching them would make mvd sweeps near-instant.

### Fast headless iteration loop (no GUI rebuild)
`render_pencil_real_mesh` (#[ignore], `pencil.rs` tests): loads an STL, runs pencil with
params from env (`RS_CAM_PENCIL_MVD/BIT/MINLEN/CELL`), writes a top-down PNG (agent-readable)
+ SVG (browser-crisp). No recompile for param changes, no GUI restart. ~10s/run now.
Latest renders copied to `~/Downloads/pencil_fix/`.

### Known pencil limitations (open, 2026-06-25)
- **Tool positioning ignores corner orientation (bisector/CL-surface).**
  `lift_to_surface` keeps the seam's X,Y and only solves Z by dropping the tool
  straight down (`P3::new(p.x, p.y, cl.z + stock_to_leave)`). Correct only for
  SYMMETRIC concavities (ball centre on a vertical bisector above the seam). For an
  ASYMMETRIC L-corner — e.g. a vertical lake wall meeting a flat floor — the ball
  centre should be OFFSET horizontally onto the floor side (45° bisector) to nestle
  the fillet; we don't compute that. So pencil traces the right *location* (the base,
  confirmed by depth colouring) but doesn't *orient* the tool into the corner. Proper
  fix = place the tool on the bitangent / CL-surface (Park & Chung 2006), a rework of
  the positioning step (not detection). Note: a 90° corner is NOT too steep for a ball
  (it fits, leaving an r-fillet); the vertical wall *face* itself is just not
  3-axis-finishable, so the base fillet cleanup is the only useful pass there.
- **Lines jagged** (follow 0.5mm triangle edges) → needs polyline smoothing.
- **Fragments don't connect** (chains break at junctions/band-gaps) → `hookup_distance`
  is still dead (Stage 2); wiring it joins fragments into continuous lines AND drops
  the ~76m of rapids.
- Suggested default: `bitangency_angle ~140` (sharper folds → narrower bands, cleaner).

### Validation-loop note (resolve before heavy iteration)
The running MCP GUI uses the *old* binary until restarted, so MCP screenshots need a GUI
restart after each rebuild. To iterate fast, validate detection via **core unit/integration
tests** + (if needed) a **CLI toolpath PNG export** that doesn't need the live GUI; take
GUI/MCP screenshots only at stage gates. (Tests must not depend on the Downloads mesh —
use a representative in-repo fixture, e.g. a noisy multi-valley surface, plus the existing
V-groove/hemisphere tests for regression.)

### Guardrails
- One cargo job at a time (check `free -g` + `pgrep -af "[c]argo"` before each build).
- Zero-warning clippy (16 deny lints); keep `#[allow(indexing_slicing)]` SAFETY pattern
  consistent with the file.
- Determinism: the file already fixed HashMap-order leaks (T14, `pencil.rs:199-206`,
  `:250-261`, `:322-324`) — any new candidate-set iteration must stay a pure function of
  the mesh (sort before iterating).
