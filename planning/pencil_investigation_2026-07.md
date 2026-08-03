# Pencil valley-targeting investigation — 2026-07-07

> **SUPERSEDED IN PART — H4, 2026-08-04.** Strategy verdicts in this file
> were measured through four instrument defects that are now fixed
> (classification grid 6x too coarse; finish-planner dials 6x/36x too large;
> rest-routing radius = shaft not tip; `claims_reference: self_probe`).
> Those verdicts are **void, not falsified** — the comparison could not have
> come out any other way. Which specific claims, and what replaced them:
> `planning/review_2026-07-29/SUPERSEDED_CONCLUSIONS.md`.


Investigator: Fable (personally, per user directive — Opus missed these flaws 3×).
Method: live GUI diagnostic loop on wanaka (LIVE-ONLY, never save) + code read + offline
hillshade harness. One variable per iteration. Handoff:
`planning/pencil_investigation_prompt.md`.

## Setup

- GUI `--mcp`, binary `6a7e164-dirty` (contains B+C = 98a472e content).
- Op: "Pencil DIAG" (index 10, id 18, setup 2): Ø2 tapered ball (tool id 6),
  detector `rest_depth`, reference tool id 12 (Ø6 ball nose), `min_valley_depth`
  0.15, `rest_cell_mm` 0.5, `route_width_factor` 2.0, `num_offset_passes` 1,
  `offset_stepover` 0.5, `min_cut_length` 2.0.
- MCP trap (filed): `add_toolpath` silently IGNORES its `params` blob, and
  `tool_index` is the tools-ARRAY index, not the tool id (bound Ø3 id 8 when given
  "6"). Params must be applied via `set_toolpath_param` after adding.

## Findings

### F-P1 — 86% of detected valley length is discarded; output is confetti (CONFIRMED)

Generation debug counters (rest_field span, run 1):

```
rest_skeleton_mm  = 5445.3
rest_traced_mm    =  745.8
rest_coverage     =  0.137
rest_pencil_regions = 33, rest_clearing_regions = 0
rest_volume_mm3   = 1234.8   (reference mode 1 = real tool)
```

- The detector finds 5.4 m of rest-region skeleton. Only 746 mm survives to
  cutting. 726 emitted pass-runs ≈ 242 chains × 3 passes ≈ **3.1 mm average
  chain length** — barely above the `min_cut_length` = 2 mm gate.
- Cutting-only screenshot (`pencil_diag_cutting.png`): scattered isolated dots
  across the mountainous half; no continuous valley trace anywhere. Matches the
  user's "picks non-ideal spots" exactly — the spots are the ≥2 mm survivors of a
  shredded spine.
- Mechanism (code): `rest_field.rs::trace_skeleton` breaks polylines at EVERY
  skeleton cell with 8-degree ≠ 2. Zhang-Suen output is riddled with degree-3
  staircase corners and spur junctions, so the main valley spine is chopped into
  node-to-node fragments mostly 1–4 cells (0.5–2 mm at cell 0.5) long; the
  `min_cut_length` filter in `rest_depth_arm` then deletes 86% of the length.
  Note `coverage()` exists in `RestFieldReport` precisely to flag this and reads
  0.14 — nothing consumes it.
- H1 (wide regions silently routed to clearing and never emitted) is NOT the
  active failure on this run: `clearing_regions = 0` at route_width_factor 2.0.
  Still a latent design gap (v1 emits nothing for clearing regions), but not
  what the user is seeing.

### F-P2 — the mask is the whole mountainside, not the valleys (CONFIRMED)

Probe: `min_cut_length` 2.0 → 0.1 (one variable), regenerate. Result: 163,179
moves, **550,376 mm "cutting"** (links + plunges between ~7,600 fragments
dominate). Cutting-only screenshot (`pencil_diag_mincut01.png`): a solid cyan
CARPET over the entire mountainous half — not a dendritic valley network.

- Mechanism: on rugged relief the Ø6 reference bridges fine texture EVERYWHERE,
  so `rest > 0.15` holds across the whole mountainside. The threshold mask is one
  giant blob; `min_valley_depth` semantically acts as "anywhere the coarse tool
  left ≥0.15 mm" (i.e. a REST-MACHINING region mask — good for P2 selective
  finishing, wrong for pencil), not "valley saliency".
- Zhang-Suen then produces the medial-axis HAIRBALL of that blob (5.4 m of
  skeleton in a ~90×100 mm area). Its geometry is unrelated to valley creases →
  H2 confirmed in a stronger form than "medial vs deepest within a valley blob":
  the skeleton isn't even valley-shaped.
- GUI heatmap (`pencil_diag_heatmap_gui.png`) confirms the FIELD is good: long
  continuous red spines (1–2 mm) along the true creases over a ~0.2 mm blue
  texture floor. Detection data is fine; extraction is what's wrong.
- min_cut_length restored to 2.0 after the probe.

### F-P3 — degree-based skeleton tracing shreds at staircase corners (CONFIRMED by prototype)

`trace_skeleton` breaks at raw 8-degree ≠ 2, but in an 8-connected skeleton every
mixed-direction staircase corner (… A=(0,0) B=(0,1) C=(1,1) D=(1,2): A–C and B–D
are diagonal-adjacent) has degree 3. Python prototype (`skel_proto.py`, exact
port of zhang_suen + trace_skeleton, sinuous 400×80 band, cell 0.5):

| junction rule | "nodes" | polylines | median len | max len | coverage @2mm |
|---|---|---|---|---|---|
| degree ≠ 2 (current) | 238/499 cells | 401 | 0.71 mm | 19.2 mm | 0.412 |
| ring-transitions ≠ 2 | 2 | 94 | 1.21 mm | **250.7 mm** | 0.795 |
| + spur prune + junction merge | 2 | 86 | — | 250.7 mm | 0.811 |

The correct junction test is the neighbour-ring 0→1 transition count — ALREADY
implemented in `ring_ab` for thinning; tracing just doesn't use it.

### Diagnosis summary (the user's complaints, mapped)

- "picks non-ideal spots": F-P2 (hairball skeleton of the whole-mountain mask)
  × F-P3 (random ≥2 mm shards of it survive).
- "always 3 equally spaced passes": H3 — `paths_from_sampled` emits
  1 + 2×`num_offset_passes` at fixed `offset_stepover`, width-blind; the chamfer
  DT knows the local half-width and is discarded after routing.
- Fusion comparison: Fusion's pencil traces ball bi-tangency creases = the RIDGE
  (local max) of the rest field, not the medial axis of a threshold mask.

### Fix spec (validated by prototypes)

Note: ordered/grayscale thinning was prototyped first and REJECTED (my
single-pass simple-point erosion deadlocks on 2-wide structures; 9651 residual
cells). The winning extractor is NMS + hysteresis (`ridge_nms_proto.py`):
**median dist-to-true-crease 0.45 mm vs 8.61 mm** for the current medial axis on
a synthetic texture-floor + asymmetric-crease field.

1. **Ridge extraction replaces medial axis for CENTERLINES ONLY**
   (`rest_field.rs::detect_rest_valleys`; the threshold mask → components →
   `region_polygons_from_mask` path is UNTOUCHED — P2 regions are correctly
   area-shaped):
   a. 3×3 box-smooth the rest field once (untrusted cells contribute 0).
   b. NMS candidates: trusted cells with `rest_sm ≥ 0.5×min_valley_depth` that
      are strict local maxima along ≥1 of the 4 direction pairs
      (N-S, E-W, NE-SW, NW-SE).
   c. Hysteresis: 8-connected candidate components; keep those whose PEAK
      `rest_sm ≥ min_valley_depth`. The dial becomes true valley saliency.
   d. `zhang_suen_thin` the kept candidates (near-thin already).
2. **Tracing fix** (`trace_skeleton`): junction test = ring 0→1 TRANSITION count
   ≠ 2 (via existing `ring_ab`), not raw degree ≠ 2 (staircase corners have
   degree 3 but transitions 2). Prototype: 238→2 junction cells, max polyline
   19→251 mm, coverage 0.41→0.80.
3. **Graph cleanup before `min_cut_length`**: iteratively prune leaf edges
   shorter than `min_cut_length`, then dissolve/merge nodes left with exactly
   two incident edges. (+0.02 coverage on the synthetic; real dendritic terrain
   benefits more.)
4. **Width-aware offset passes** (`pencil.rs::paths_from_sampled`, RestDepth arm
   only): per-chain half-width = median mask-DT along the ridge (mm);
   `n_offsets = min(num_offset_passes, max(0, round((half_width − pencil_r) /
   offset_stepover)))` — the user dial becomes a CAP; narrow V-creases get the
   centerline only. Dihedral/Curvature arms keep legacy behaviour (no width
   data).
5. Routing to clearing (median DT > `route_width_factor × pencil_radius`) stays,
   now evaluated along ridges. Clearing regions still not emitted by pencil —
   P2 selective finishing is the consumer.

### Suspects still open

- **H2 — medial axis ≠ deepest path**: Zhang-Suen thins the *binary* mask; rest
  depth never weights the skeleton, so on asymmetric valleys the centerline sits
  off the true crease. Verify with hillshade harness (pre-filter skeleton over
  terrain) once the user's sysml cargo job finishes (cargo exclusivity).
- **H3 — width-blind offsets**: `paths_from_sampled` always emits centerline +
  `num_offset_passes` × 2 at fixed `offset_stepover`, ignoring the chamfer DT
  (local half-width) the field already computed. This is the literal "3 equally
  spaced passes" complaint. Fix direction: derive pass count/spacing per-chain
  (or per-point) from the DT, or region-confine.

## Implementation (Sonnet agent, spec by Fable; diff reviewed by Fable line-by-line)

Uncommitted changes in `rest_field.rs` (+656/−51) and `pencil.rs` (+53):

- `box_smooth_rest` / `nms_candidates` (4 direction pairs, floor 0.5×mvd) /
  `hysteresis_ridge` (component peak ≥ mvd on the SMOOTHED field) → reuse
  `zhang_suen_thin` → fixed `trace_skeleton` (junction = `ring_ab` transition
  count ≠ 2, orthogonal-preferred walk; `skel_degree` deleted) →
  `cleanup_ridge_graph` (BTreeMap-deterministic leaf-prune < min_cut_length +
  degree-2 splice) → route by median mask-DT as before.
- `RestCenterline { points, half_width_mm }` replaces `Vec<P3>` centerlines.
- `pencil.rs::rest_depth_arm`: `offset_passes = min(num_offset_passes,
  round((half_width − r_pencil)/stepover))` — dial is now a cap; Dihedral/
  Curvature arms unchanged.
- Mask → components → `region_polygons_from_mask` / `rest_grid` heatmap paths
  untouched (P2 semantics preserved).
- New tests: `staircase_skeleton_traces_as_one_polyline` (with a documented
  intrinsic "chord" residual — assert whole-staircase-as-one + ≤2 polylines),
  `asymmetric_valley_centerline_hugs_crease` (steep 1.5 / gentle 0.25, ridge
  must hug y≈0 where the old medial axis drifted gentle-side),
  `graph_cleanup_merges_through_short_spur`.
- Review watch-items (validate empirically, don't pre-patch): (a) strict-
  both-sides NMS drops exact plateau ties — flat-floored valleys could lose
  their ridge line (they route to clearing anyway; real fields rarely tie);
  (b) trust-band-edge cells could produce spurious candidates on downhill
  gradients — box-smooth rolloff + hysteresis saliency should kill them;
  check the harness image for a boundary ring.
- `cargo check -p rs_cam_core`: PASS (first try).

### Red-green loop on the agent's tests (Fable personally)

First run: 33 pass, 2 red — and both reds exposed REAL design facts, not typos:

1. `asymmetric_valley_centerline_hugs_crease` (mean |y| = 5.30): a ball on a
   constant-slope PLANE floats a position-independent height, so ANY
   plane-wall V (fillet or not) reads a constant rest plateau — there is no
   ridge at the crease, for the old medial-axis code either (it also sat
   mid-wall). Verified numerically (1D drop sim, `fixture_check.py`): parabola
   and rounded-V profiles peak at the DOMAIN EDGE (rest grows with slope);
   only a narrow bridged TRENCH (the rivmap-channel case wanaka is made of)
   peaks at the crease: gaussian trench d=1.5 σ0.8/2.5 → peak 0.97 mm at
   y=0.00, prominence 0.22 mm. → Fixtures rewritten as `make_trench`
   (plane-wall V limitation documented on `nms_candidates`; clean CAD creases
   = Dihedral detector's home turf).
2. `tent_ridge_yields_none` (peaks 0.110, femtometre-identical): two defects.
   (a) strict-`>` NMS tie-breaks on 1e-15 float noise across constant
   plateaus → added prominence `δ = max(0.1×min_valley_depth, 0.005 mm)`.
   (b) zero-padded box-smooth created an artificial 1–2-cell rolloff band
   along every trust edge; the crest between that fake decline and a real
   one (tent apex ramp) read as a prominent ridge → smoothing now averages
   the TRUSTED subset only, and an NMS direction pair requires BOTH
   neighbours trusted (kills part-boundary ridge lines; a crease crossing
   the trust edge still qualifies via its parallel pair).

Second run after fixes: **36/36 pass** (incl. both former reds; monotone-mvd
and reference-diameter tests converted to trench fixtures so they assert
non-trivially under the prominence rule).

### F-P4 — hysteresis per-component gate carpets on connected networks (FOUND + FIXED on real terrain)

First real-terrain harness render (mvd 0.15, Ø6 ref / Ø2 pencil): coverage
0.137 → 0.73 and the network is crease-following — but it's a CARPET: 17 m of
skeleton, 5211 centerlines. Sidecar peaks show why: the whole dendritic
network is 8-connected into two giant components peaking at 3.6 mm, and
hysteresis keeps whole components — so at any mvd ≤ 3.6 everything survives
and the dial is dead again. Fix: per-BRANCH saliency gate after tracing —
keep a polyline only when its MEDIAN `rest_sm` ≥ `min_valley_depth` (same
median metric family as `pencil::polyline_passes_depth`), before the
skeleton-length/coverage accounting. Tests stay 36/36.

Dial sweep on the real terrain after the branch gate
(`rest_ridge_mvd{0.15,0.5,1.0}.png`):

| mvd | centerlines | coverage | visual |
|-----|-------------|----------|--------|
| 0.15 | 5196 | 0.73 | full fine-texture network (honest: all ≥0.15) |
| 0.5 | 2255 | 0.80 | coherent dendritic valley network, lines IN the dark creases |
| 1.0 | 508 | 0.80 | deepest trunks/gullies only |

Monotone thinning ✓, crease-following ✓, continuity ✓. (Old behaviour at any
mvd: 86% discarded confetti of a mask-medial hairball.) Note some green along
the NW shoreline at all mvd — consistent with the GENUINE deep rest band the
GUI heatmap shows at the mountain-front (Ø6 can't reach the base crease), not
a trust-edge artifact (bilateral-trusted NMS is in place).

### Live GUI validation (rebuilt binary, wanaka fresh load)

- "Pencil DIAG v2" (index 8, id 15): Ø2 tapered ball, rest_depth, ref id 12,
  mvd 0.15 → **9265 mm cutting, 5668 moves, 464 pass-runs ≈ ~20 mm avg per
  run** (old: 726 runs ≈ 3 mm confetti). User reaction to the live viewport:
  "woah, that looks way better".
- Screenshot-render note: with `include_rapids: false` the long straight
  lines still drawn are the rapid-over-at-safe-Z moves, which carry
  `MoveIntent::Linking` — the filter tests intent, not move type. Renderer
  quirk, not an emit bug (A/B with `include_rapids: true` shows the true
  rapids as a separate orange population). Follow-up filed.
- Sim: 14 rapid collisions / 31% air-cut, ALL attributable to the pencil
  running against VIRGIN stock — baseline sim with pencil disabled shows 0
  collisions / 6% air-cut. The setup-2 predecessors (3D Rough/Finish 6)
  cannot regenerate because of a pre-existing catch-22 (below), so the sim
  has no machined stock under the pencil. Not a pencil defect.

### NEW BUG FILED — FromRemainingStock regen catch-22 after fresh load

`controller/events/compute.rs` (~line 371): a `FromRemainingStock` toolpath
finds its prior-stock checkpoint by locating ITSELF in the last sim run's
`boundaries()` — but an errored/ungenerated op is never simulated, so it is
never in the boundaries, so it can never regenerate. After any fresh project
load, every FromRemainingStock op is permanently stuck in Error (wanaka's
Rivers/Lakes/3D Rough 6/3D Finish 6 were red all session, pre-dating the
pencil work). Blocks the R2 machined-stock pencil validation. Fix idea: the
checkpoint lookup should key on the op's position in the PLANNED sim order
(enabled toolpaths in sequence), not on membership in the last sim's
simulated set.

## Iteration log

- Run 1 (baseline, params above): moves 5013, cutting 4266.7 mm, rapid 1711.5 mm.
  Artifacts: `pencil_diag_cutting.png` (6-view, cutting only), debug-trace dump in
  session tool-results.
