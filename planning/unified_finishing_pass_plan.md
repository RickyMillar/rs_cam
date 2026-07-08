# Unified finishing pass — plan + tracking

Owner: Ricky + Fable. Started 2026-07-07 (same-day context: pencil ridge fix 29a6d61,
F.4 catch-22 fix + rest-UX 37ef782, sliver guard d0d6d75, MCP cancel 482be35).

## Goal (user's words, 2026-07-07)

"The goal is to cleanly remove shallow areas as fast as possible to a finish. I think
for steeps we will want to use contour still, but maybe look into similar methods for
linking, and build out some kind of steep/shallow/pencil method where we take the
shortest possible path, instead of doing them in phases."

Two threads, deliberately sequenced:

1. **The big lever: phase-merged, shortest-path finishing.** Today steep/shallow/crease
   finishing runs as separate ops — each traverses the part with its own retracts and
   its own boundary-clip fragmentation. All measured wins in this project were
   linking/ordering wins, not cutting wins.
2. **The quality increment: morphed spiral per shallow region.** One continuous path
   morphing boundary→center per island ("one clean path, no moves"). Commercial
   analogue: Fusion Morphed Spiral / PowerMill 3D-offset spiral. Stepover must be
   measured on the TOOL-CONTACT surface (or slope-corrected), not plan XY — planar
   spacing under-covers slopes by 1/cos(slope). This is a strategy PLUGIN for #1,
   not its own op, and it is built LAST.

## Evidence base (why linking is the suspected lever)

| Datum | Number | Source |
|---|---|---|
| Pencil R2 (machined stock) rapid vs cutting | 1027mm rapid vs 669mm cutting | 2026-07-07 live |
| Pencil hookup_distance surface-link fix | total motion −72% | 2026-06-25 (bf4e95d) |
| Contour-spiral trochoid cap vs adaptive agent | wall-clock −32%, rapid ÷3.6 | 2026-06-15 |
| 3D Finish boundary-clip fragmentation | 133,585mm rapid vs 3,795mm default-linked | 2026-07-04 probe |
| steep_shallow op A/B | 3.4× MORE total motion (WORSE) | 2026-07-04 probe |

The steep_shallow failure autopsy: it died of region fragmentation + per-fragment
retracts, NOT of the slope-classification idea. Lesson: never emit regions without
solving the linking. The planner must treat routing as first-class.

## Phases

### P0 — PROBE: linking share of finishing wall-clock (decision gate) `[ ]`

Measure the ADDRESSABLE overhead before building anything. On the current wanaka
finishing stack (coarse scallop + selective fine + pencil, and/or the rebuilt
Rivers→Finish chain), use the F-034 kinematics-aware integrator (machine profile is
real: $$-imported accels, junction deviation) to decompose wall-clock into:
cutting-in-material / air-cut feed (links) / retract+plunge / rapid travel.

- Available signals: `cut_trace.summary` (`cutting_runtime_s`, `rapid_runtime_s`,
  `air_cut_time_s`), per-toolpath `total_runtime_s` (integrator-backed when
  kinematics context is on), `MoveIntent` tags on moves (Retract / Linking /
  EntryPlunge / FinishingCut — tagged since the Step-1 2026-05-19 work).
- If per-intent TIME isn't already exposed, a small core addition (aggregate
  integrator time by MoveIntent class per toolpath) is in-scope for the probe.
- **Decision gate**: linking+retract+air share ≥ ~15–20% of finish wall-clock →
  proceed to P1/P2. Below that → deprioritize the planner; morphed spiral proceeds
  as a quality feature only (P3 direct).
- Log results here + probe log discipline (artifacts, params, numbers).

### P1 — Quantitative linker (benefits everything immediately) `[ ]`

Replace fixed-threshold link heuristics (`hookup_distance`, boundary-clip
retract-always, scallop connector hop cap) with a per-gap COST COMPARISON using
`compute_cycle_time`: time(surface-following feed across gap, gouge-checked) vs
time(retract + rapid + re-plunge). Pick the cheaper. Sites:

- pencil `emit_paths` (currently fixed hookup_distance)
- scallop continuous-mode ring/island connectors (currently hop ≤ 3R cap)
- `clip_toolpath_to_boundary*` re-entries (the 133k-rapid case)
- future planner region-to-region links

Acceptance: integrator wall-clock A/B per op on wanaka; no new sim collisions;
gouge-safety of surface links unchanged (drop-cutter sampled, fall back to retract).

**P1 detailed design (2026-07-07, post-probe code audit):**

- **W1 — costing helper** (`machine_kinematics.rs`): `retract_link_time` /
  `surface_link_time` — build the candidate move sequence as a tiny synthetic
  `Toolpath` (seed rapid at `from`, then the candidate moves) and integrate with
  `compute_cycle_time`. Rest-to-rest bracketing is conservative for both
  candidates equally.
- **W2 — plunge-descent optimization**: `emit_path_segment_with_intent`
  (`toolpath.rs:227`) plunges from `safe_z` at plunge feed — the 1461 s
  (Rivers) / 335 s (pencil). Add `descend_rapid_to: Option<f64>`: when `Some(z)`
  with `path[0].z < z < safe_z`, emit rapid-over(Linking) → rapid-down to z
  (Linking) → plunge(EntryPlunge) only the rest. SAFETY RULE: callers may pass
  `Some` ONLY when the descend height derives from drop-cutter sampling at that
  XY (tool-center safe height): raster grid (`first_pt.z + PLUNGE_CLEARANCE_MM`),
  pencil (valley pts are drop-cutter), scallop rings (verify ring_to_3d), and
  project_curve (surface = the z it would emit at depth 0, per z_flip branch).
  Waterline keeps `None` — its z-level points are not per-XY drop-cutter heights
  and walls can rise above them. `PLUNGE_CLEARANCE_MM = 2.0` (clears typical
  stock_to_leave + scallop crest).
- **W3 — boundary-clip re-entry** (`boundary.rs:110`): re-entry currently
  descends safe_z→target at the ORIGINAL CUTTING FEED, untagged — this is
  Finish 6's 656 s `unknown_s`. Fix: tag all clipper emissions (over=Linking,
  exit=Retract, re-entry descent=EntryPlunge), take `plunge_rate` +
  `descend_clearance: Option<f64>` from callers; finish-family ops
  (`feeds_pass_role == PassRole::Finish`) opt into rapid-down to
  `target.z + clearance` then plunge; clearing-family ops keep full-feed
  descent (mid-stock targets — rapid-down unsafe), tagged. Retract-hop culling
  for short excursions DEFERRED to P2 (needs stock context the clipper lacks).
- **W4 — cost-based link decisions** (after W1 lands): pencil — keep
  `hookup_distance` as the candidate CAP, decide surface-link vs retract by W1
  cost instead of emitting the link whenever it exists. Scallop — offer a
  drop-cutter-sampled surface link (share/promote pencil's `build_surface_link`)
  for non-helical connectors, cost-compared; ONLY when no region filter is
  active (selective scallop's excluded islands may hold uncleared stock on
  FromRemainingStock — mesh-sampled links are not gouge-safe there; planner/P2
  owns that case).
- **Gates**: clippy, core lib, f034+f036b, `param_sweep` (fingerprints WILL
  move — descent changes geometry; verify diffs are descent-shaped only),
  sentry battery, then live wanaka A/B via `runtime_by_intent` (expect entry_s
  and unknown_s to collapse on Rivers/pencil/Finish 6; 0 new rapid collisions).
- **P1 LIVE A/B FLAW (2026-07-07 ~21:30, caught by the 0-collision gate):**
  mesh-derived descend heights are WRONG for FromRemainingStock ops — Rivers
  regenerated with descents produced **151 rapid collisions** (baseline 0).
  River channels were never entered by the Ø6 rough, so remaining stock sits
  many mm above the mesh at exactly the entry XYs; "rapid down to mesh+2mm"
  descends through real material that the old plunge-at-feed legitimately CUT.
  Lesson: **a safe descend ceiling must come from the INPUT STOCK, never the
  mesh** — reworked as a post-generation pass (`optimize_entry_descents`)
  that queries `TriDexelStock::max_top_z_in_disc(x, y, tool_radius)` (prior
  dexel for rest ops, stock-top plane for fresh) and splits EntryPlunge feeds
  after the clip. Per-generator mesh descents + clip clearance descents
  REVERTED in favor of the single stock-aware pass.
- **P1 implementation notes (2026-07-07)**: W1+W2+W3+W4a landed together.
  Scallop's `test_scallop_continuous_no_rapids_between_rings` bound moved 4→6
  (each retract-entry is now 3 rapids: over, descend, then the plunge is a
  feed). W4a scope: pencil only — scallop cost-based surface links deferred
  (only matter on the region-gated selective path, which the gouge-safety rule
  excludes from mesh-sampled links anyway). KNOWN GAP: the GUI compute worker's
  `ComputeRequest` carries no machine profile, so GUI/MCP-triggered generates
  run the pencil with `link_kinematics: None` (legacy always-link behavior);
  core-session generates get the cost decision. Follow-up: thread the machine
  profile into `ComputeRequest`. W4b (scallop) revisit after the A/B.

### P2 — Finishing pass planner (phase merge) `[ ]`

One op (or orchestrated pass) that:

1. **Decomposes** the surface: steep regions (slope threshold — classifier exists in
   steep_shallow), shallow regions (complement), crease network (= the pencil ridge
   detector's centerlines, 29a6d61). All RegionSet-shaped; sliver guard d0d6d75
   applies.
2. **Assigns strategy** per region: steep → contour/waterline rings; shallow →
   raster or rings (morphed spiral later, P3); creases → pencil centerline trace.
3. **Routes globally**: order all region entry/exit points for shortest total time
   (greedy nearest-neighbour first; TSP polish only if the probe says it pays),
   with P1's quantitative link decision at every junction.

Constraints/reuse: RegionSet, per-island generation (P2 selective finishing),
existing waterline/scallop/pencil generators as region-scoped strategies; the
steep_shallow op's fragmentation failure is the anti-pattern this must not repeat.
Fresh design doc + Fable review before implementation (this is architecture).

### P3 — Morphed spiral strategy `[ ]`

Per shallow region: single continuous spiral morphing between the region boundary
and its innermost offset ring.

- Spacing measured on the tool-contact surface (slope-corrected), constant-scallop.
- Non-convex regions: split at medial-axis necks first, or per-region fallback to
  ring mode when a morph-degeneracy metric trips (boundary/center shape mismatch).
- Lives as a strategy in P2's planner (or a scallop mode if P2 is deferred), NOT a
  new top-level op.
- Spiral direction: outside-in ends at region center (needs retract) vs inside-out
  ends at boundary (can surface-link to the next region) — inside-out preferred for
  routing; verify chip/finish implications.

## Acceptance discipline (project-wide)

- Wall-clock = F-034 kinematics integrator, never move counts or path mm.
- A/B on wanaka: current phase-based stack vs unified pass, same tools + thresholds.
- Quality: sim deviation vs model, scallop-height spot checks, 0 new rapid
  collisions, sentries green.

## Tracking

- [x] P0 probe run + numbers logged below (2026-07-07)
- [x] P0 decision recorded: **PROCEED to P1+P2** (strict finishing overhead 25.2%,
      detail+finishing 39.3% — gate was 15–20%)
- [x] P1 quantitative linker + stock-aware entry descents + A/B (2026-07-07
      headless, GUI-modulation-equivalent): **project −16.6% (10692→8920 s),
      finishing/detail subset −18.2%; Rivers −54%, Lakes −35%, Finish 6 −9.1%;
      roughing controls unchanged; 0 rapid collisions; unknown_s eliminated.**
      Harness: `crates/rs_cam_core/tests/p1_headless_ab_wanaka.rs`
      (`--ignored --nocapture`). ComputeRequest threading landed (4ce5bac).
- [x] P1 LIVE CONFIRM (2026-07-08 morning, GUI): project 11 194 → **9 543 s
      (−14.7%)** incl. pencil; Rivers −54.5% (791 s), Lakes −40% (246 s),
      Finish 6 −6.7% (7 068 s; the 656 s unknown → 0, ~505 s of re-entry
      feed-through-air removed), collisions 4 = pre-existing baseline count
      (pencil adds 0), air-cut 14.3% → 5.0%. **Pencil finding**: only −6%
      (470 s, entry 392 s) — its valley entries descend next to V-walls, so
      the tool-radius stock ceiling is legitimately near wall-top; pencil
      plunges are mostly physics-bound, and its remaining lever is chain
      count/routing (P2), not descents. Remaining P1 tails: W4b scallop cost
      links; generate_all cancel race has a SECOND leg (auto-regen submits
      first after load, the MCP batch's own submit is the duplicate that
      cancels — 402bf10 fixed only sweep-after-MCP; cosmetic since the
      requeued job completes, fix candidate: generate_all skips
      already-Computing ids).
- [x] P2 design doc (`planning/unified_finish_planner_design.md`) — reviewed
      2026-07-08 (3 bands, corridor rule, risk register R1–R6, build order
      P2.b–P2.e; commit c3e29dc). P2.a prep refactors landed (172761b).
- [x] P2.b decomposition + conditioning (2026-07-08): `finish_planner` module
      — `decompose()` producing Shallow/MidSteep/VerySteep bands with R1
      conditioning (hysteresis seeded-flood, EDT morphological close,
      min-area absorption with deterministic majority vote) + crease corridor
      rule (`half_width < K×tool_radius` stays in-band, wider = own region,
      corridor carved from band masks) + `planned_regions_to_svg` debug view.
      11 unit tests (stripe-fragmentation A/B, dome 4-region nesting, seed
      requirement, absorption, both corridor branches, overlap dilation,
      determinism, degenerate inputs). **R1 acceptance PASSED**:
      `tests/finish_planner_wanaka_decompose.rs` (`--ignored`) → wanaka
      decomposes to **1 region** (3 raw steep islands = the diagonal
      river-channel walls, all < min-area → absorbed; correct — channels are
      crease/pencil territory), SVGs at `target/finish_planner_debug/`.
      **CLASSIFICATION-SURFACE FINDING (user-caught)**: first acceptance run
      classified on the Ø6 drop-cutter (ball-center offset) surface → wanaka
      read all-shallow (0.1% ≥45°) while the TRUE surface is 38.5% ≥45° (max
      89° vs 52°) — the ball bridges features at/below its radius. Fixed:
      `finish_setup::build_classification_surface_with_cancel` (tiny
      bare-surface probe, rest_field precedent) + decompose erodes coverage
      1 cell (stencil-safe slopes at the min_z clamp boundary). True-surface
      wanaka: 54 raw steep + 2 very-steep islands → **3 regions** (1 shallow
      + 2 mid-steep tracking the diagonal mountain range); border-clamp
      artifacts gone. steep_shallow op shares the offset-surface blind spot
      (quantified, left as-is — planner supersedes it). Design doc decision
      #3 amended. Diagnostic: `wanaka_slope_distribution_diagnostic` in the
      acceptance test file.
      **Shared-code fix that fell out**: `polygon::detect_containment` had no
      even-odd nesting — an island inside a hole (dome cap inside a mid-steep
      annulus, guaranteed for annular slope bands) was consumed as a hole of
      the outermost polygon, silently deleting its territory. Now counts
      containment depth: even → top-level, odd → hole of innermost even-depth
      container. Affects all mask→polygon users (rest_field, boundary,
      svg/dxf import, adaptive3d); depth ≤ 1 behaviour unchanged; +2 nesting
      tests in polygon.rs.
- [~] P2.c per-band generation + naive concat, A/B checkpoint #1 (must not
      regress the P1 stack). PROGRESS 2026-07-08: `unified_finish.rs`
      orchestrator (6/6 unit tests) + full op registration (X-macro,
      registry ball-tip, execute adapter + spans, feeds classification, GUI
      panel; catalog sentries bumped to 24) + `set_toolpath_operation`
      session API (in-place op swap preserving chain order for honest
      A/Bs). A/B harness `tests/p2c_headless_ab_wanaka.rs`: branch A
      measured 8919.5 s project / 6883.4 s finish (reproduces the P1
      headless 8920 s baseline to 0.5 s; now PINNED — B-only reruns skip
      A's ~35 min). First A/B attempt EXPOSED the scallop ring-cascade
      exponential (see ledger entry): fixed via drop-only ring decimation;
      cost curve linear post-fix. B dials: raster 0.3 (=A), scallop_height
      0.011 (A's effective mid-steep cusp), z_step 0.3. **CHECKPOINT #1
      PASSED (2026-07-08)**: B finish 7011.4 s (+1.9%) / project 9047.5 s
      (+1.4%) / collisions 0 ≤ 4 gate; cutting −12% (5839.5 → 5137.9 s,
      the banding win at better held cusp) vs entry+rapid +830 s (naive
      band-crossing plunges) — the +830 s is P2.d's quantified router
      target. Post-scallop-fix a full B chain is 51 s wall — A/Bs are now
      cheap (P2.e can afford real sweeps). Probe tests kept in the harness
      file (phase/cost-curve/offset-cascade) for future pathology hunts.
      Remaining P2.c tails: crease/pencil integration into the op, live
      GUI validation, offset_polygon root fix (tracked separately).
      **Coastline stencil FIXED 2026-07-08**:
      `SlopeMap::from_z_grid_max_gradient` (classification only; see design
      doc "Known classification gap" for measured effect) — coast ring now
      registers, decomposition matches true-surface area stats (42.8% vs
      45.8% ≥35°), and the B rerun flipped the checkpoint verdict to
      **B −4.1% project (−366.7 s) / finish −5.3% / collisions 0** before
      any routing. B finish entry+rapid still 2200 s → P2.d headroom.
- [x] P2.d router (greedy + link costing), A/B checkpoint #2. DONE
      2026-07-08: `unified_finish.rs` now generates PER REGION
      (single-polygon RegionSet; raster grid computed once and shared,
      scallop/waterline rebuild internal surfaces per call — P2.e datapoint
      if region counts grow) and routes greedily, seeded at the steepest
      band present. Junction cost = `min(retract_link_time,
      surface_link_time)` (F-034 integrator, never distance/feed); the
      WINNING candidate is emitted: surface link as Linking feeds with the
      follower's rapid+plunge preamble and the leader's trailing retracts
      stripped, else the native retract junction. `LinkKinematics` plumbed
      from `ctx.link_kinematics` (same P1 W4a path as pencil); without it
      the order falls back to steep-first band-major with native links.
      Route + per-junction decisions reported on `UnifiedFinishReport`
      (`route`, `links`). **CHECKPOINT #2 (2026-07-08, after the coastline
      stencil fix reset the baseline): B finish 6388.8 s (−7.2%) / project
      8424.9 s (−5.5%, −494.6 s vs pinned A) / collisions 0.** Router's own
      contribution −127.9 s (rapid 1397→1291 s, cutting −25 s from
      per-region waterline z-ranges); coastline classification fix
      contributed the other −366.7 s. 2-opt deliberately NOT built: greedy
      at O(4) regions leaves nowhere near 5% on the table (measure-first
      rule). Remaining finish-op entry_s (803 s) is INTERNAL strategy
      plunges (raster rows / rings / contours) — P3 morphed-spiral + arc-fit
      territory, not routing. 4 new unit tests (preamble/retract detection,
      router determinism, seeding+nearest-chaining on synthetic regions,
      hemisphere route report).
- [ ] P2.e decomposition-parameter sweep harness; lock defaults from data
- [ ] P3 morphed spiral strategy + degeneracy fallback
- [ ] Ledger + FEATURE_CATALOG + memory updates at each landing

## P0 results log

### 2026-07-07 — probe run (fresh wanaka chain + R2 pencil, live GUI)

**Instrumentation** (uncommitted at time of run): `CycleTimeBreakdown` — the F-034
integrator now buckets every move's time by `MoveIntent` class (`cutting` =
Clearing/Finishing/Drilling feeds, `entry` = EntryPlunge/Helix/Ramp/LeadIn,
`linking` = Linking/LeadOut, `retract`, `rapid` = MoveType::Rapid, `unknown` =
untagged feeds). Attached as `runtime_by_intent` per-toolpath + project-wide on the
cut trace, exposed via `get_cut_trace.toolpath_summaries`. Found + fixed en route:
the F-036b1 post-modulation re-walk rewrote `total_runtime_s` from modulated feeds
but left F-034's pre-modulation breakdown in place (Back Rough read 675 s total vs
459 s breakdown); both re-walk paths now rewrite the breakdown together
(`session/compute.rs` + `compute/simulate.rs`, `AddAssign` on the struct).

**Setup**: fresh load → generate_all → F.4 ladder to 3D Finish 6 → add rest_depth
pencil (Ø2 tapered ball, machined-stock reference) → sim. Machine: real $$ import
(accels 500/500/270, jd 0.020). Feed modulation ON (post-modulation numbers —
modulation inflates project integrator time 6813 s → 11 194 s, +64%). Chain
deterministic across two rebuilds. 4 rapid collisions in the full chain, pencil
adds 0 (pre-existing, not chased).

**Per-op integrator decomposition (seconds, % = overhead share of op total):**

| op | total | cutting | entry | linking | rapid | unknown | overhead % |
|---|---|---|---|---|---|---|---|
| Back Rough (id 4) | 675 | 570 | 41 | 13 | 51 | 1 | 15.6% |
| Rivers back (id 5) | 1737 | 81 | 1461 | 0 | 188 | 7 | **95.3%** |
| Lakes (id 6) | 412 | 86 | 257 | 0 | 61 | 9 | **79.2%** |
| 3D Rough 6 (id 10) | 296 | 201 | 12 | 17 | 64 | 1 | 31.9% |
| 3D Finish 6 (id 11) | 7573 | 6024 | 0 | 0 | 894 | 656 | **20.5%** |
| Pencil R2 (id 15) | 501 | 14 | 335 | 4 | 56 | 93 | **97.3%** |

Project: 11 194 s total; cutting 6 975 (62.3%); overhead 4 219 (37.7%) =
entry 2 105 + linking 34 + rapid 1 313 + unknown 767. (Drills excluded from
breakdowns — no engagement-side summaries by design.)

**Finishing-subset shares:**
- Strict finishing (Finish 6 + pencil): 8 074 s, overhead **25.2%**.
- Detail + finishing stack (+ Rivers + Lakes): 10 223 s, overhead **39.3%**.

**→ DECISION: gate (≥15–20%) PASSES decisively. Proceed to P1 + P2.**

**Structural findings:**
1. **Plunge-cycle ops are almost pure overhead.** Pencil: 97.3% overhead — 335 s
   plunging at plunge feed vs 14 s cutting. Rivers: 95.3% — 1 461 s of EntryPlunge.
   The cost is *plunge-from-clearance at plunge feed*, not rapids. P1's
   link-vs-retract costing attacks exactly this (surface-follow between nearby
   targets); a cheap independent win is rapid-down-to-near-surface before feeding.
2. **3D Finish 6 overhead is 894 s rapid + 656 s UNTAGGED feed moves** — the
   finish generator emits its links/cleanup with `MoveIntent::Unknown`, which is
   both the linking-share suspect (boundary-clip re-entries) and a tagging-honesty
   gap P1 must fix to be measurable.
3. **Naive vs integrator: Finish 6 is 10.1× naive** (749 s → 7 573 s). At 0.31 mm
   avg segment length, junction/accel physics + modulation dominate; commanded feed
   is nearly irrelevant. Implications: (a) smoother paths (morphed spiral, arc
   fitting, segment merge) have far more *time* leverage on finishing than mm
   deltas suggest — P3 is justified on time, not just quality; (b) P1's per-gap
   cost comparison MUST use the integrator, never distance/feed.
4. `retract_s` = 0 everywhere: generators emit retracts as `MoveType::Rapid`, so
   they land in `rapid_s`. Expected, not a bug.
