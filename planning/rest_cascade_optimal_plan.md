# Optimal rest-machining cascade — plan

**Captured 2026-07-04.** Owner goal, verbatim: *"effectively clear relief terrain
with a coarse ball-end, then take my fine ball ends and effectively clean up the
tight spaces. Not committed to one toolpath or another, just optimal time."*

The product shape: **coarse tool does the area, fine tool does the length.**
A finish pass is O(area/stepover); a pencil pass is O(valley length). Every mm²
the fine tool sweeps that the coarse tool could have finished is wasted time.
Optimal = each tool cuts only what it uniquely can.

```
rough → coarse ball finish (stepover sized to scallop target)
      → [rest analysis]
      → clearing pass(es) where rest is WIDE   (mid/fine tool, area strategy)
      → pencil pass where rest is NARROW       (fine tool, centreline strategy)
```

---

## Current state (measured 2026-07-03/04, wanaka200 live)

R1 (reference = real library tool) and R2 (reference = simulated machined stock)
are landed. Apples-to-apples on idx 8 pencil (tool 2, detector=rest_depth,
MVD 0.4), reference varied:

| Reference | Mode | rest_volume | pencil regions | clearing regions |
|---|---|---|---|---|
| pencil's own tool 2 (same-tool floor) | analytic | 712 mm³ | 23 | 0 |
| tool 8 = idx 7's actual finish tool (R1.5) | analytic | 7,569 mm³ | 943 | 0 |
| 6 mm ball (hypothetical bigger) | analytic | 20,725 mm³ | 467 | 0 |
| machined stock, 1.0 mm sim | measured | 33,987 mm³ | 93 | 1 |
| machined stock, 0.5 mm sim | measured | 46,859 mm³ | 91 | 2 |

Findings that drive this plan:

1. **Reference choice is a 30× knob** (712 → 20,725 analytic). Wrong reference =
   garbage pencil path. Inferring it from the prior op removes the foot-gun.
2. **The two methods find different *kinds* of rest.** Analytic finds many tiny
   tool-reach valleys (943 regions — pencil's home turf). Stock finds few big
   blobs (91 regions + ClearingRegions — area-clearing turf) *plus* phantom rest.
3. **Stock fidelity is suspect on steep walls.** Stock reads 6× the analytic
   prediction of the same physical tool, and refining the sim 1.0 → 0.5 mm made
   it read *more* (33,987 → 46,859), not converge. A dexel Z-grid on
   near-vertical faces turns XY cell size into multi-mm Z error. (Confound to
   re-check: idx 7's tool differed between those two sim runs.)
4. **ClearingRegions are detected and thrown away.** `pencil.rs` ~1419 logs
   "wide region routed to clearing (Phase D: adaptive3d)" — no such phase
   exists. The stock reference finds the big rest, then nothing cuts it.

### Plumbing facts (verified in code 2026-07-04)

- `ctx.initial_stock` (FromRemainingStock) is consumed by: **adaptive 2D**
  (`execute.rs` ~813), **adaptive3d** (~964), **pencil** (~1147).
  **Waterline does NOT take initial stock** — a waterline rest pass would re-cut
  the whole wall band unless we wire it.
- `BoundarySource` = Stock | ModelSilhouette | Geometry{imported chains} |
  FaceSelection. **No programmatic-polygon variant** — routing rest regions into
  a clearing op needs a new source variant (or equivalent).
- Rest-field heatmap renderer exists as an `#[ignore]` hillshade test in
  `rest_field.rs` (~1085), env-var driven, not GUI-wired.
- `RestReference::{Cutter, Stock}` enum + `rest_reference_mode` debug counter
  (0=nominal, 1=real tool, 2=stock) landed in R2.
- Sim→generate dependency UX plan already captured:
  `planning/remaining_stock_sim_dependency_ux.md` (badge + one-click sim).

---

## Phases

### P0 — Live cascade prototype (no code; MCP on wanaka) — **RUN 2026-07-04**

Answer *"does clearing the wide rest first collapse the pencil?"* empirically.
**Result: the premise was wrong, and P0 found the real lever. See below.**

#### Measured (all sim-free — generate move counts + free rest-field traces)

| Op | Tool | stepover | moves | cutting mm | notes |
|---|---|---|---|---|---|
| idx 7 finish (baseline) | Ø3 (t8) | 0.30 | **446,222** | ~145k (est) | the whole job's cost |
| idx 8 pencil (baseline) | Ø1 (t2), ref t8 | — | 8,506 | ~8k (est) | rounding error |
| Ø6 coarse finish (probe) | Ø6 (t12) | 0.42 | **227,529** | 103,670 | **scallop-MATCHED to Ø3@0.3 (h≈0.0075mm)** |
| Ø3 ribbon pencil (probe) | Ø3 (t8), ref t12 | — | 25,350 | 25,387 | middle-tool ribbon follow |

Rest-field, analytic, varying reference tool (free from the debug trace):

| Reference | coverage | pencil regions | **clearing regions** | rest mm³ | traced mm |
|---|---|---|---|---|---|
| Ø1 (t2, same-tool floor) | — | 23 | **0** | 712 | — |
| Ø3 (t8, the finish tool) | 6.3% | 943 | **0** | 7,569 | 1,566 |
| Ø6 (t12) | 12.8% | 194 | **0** | 20,595 | 3,168 |

#### Findings (decision-grade)

1. **The finish IS the job.** Ø3 full-surface finish = 446k moves (~36 min cut);
   pencil = 8.5k (~4 min). Optimizing "rest before pencil" targeted ~2% of cost.
2. **Rest after a ball finish is ALWAYS narrow ribbons, never wide areas.**
   `rest_clearing_regions = 0` for *every* analytic reference (Ø1, Ø3, Ø6).
   Geometrically necessary: a ball only leaves stock in concave features it can't
   fit. Broad flats/convex are fully finished by any ball. ⇒ **there is no wide
   rest to area-clear between finish and pencil.** Kills Variant-B-as-written
   (adaptive3d rest-clear there). Area clearing belongs at the **rough→finish**
   boundary (rest-roughing), not finish→pencil.
3. **The only "wide/clearing" rest is the stock-mode phantom** (33–46k mm³, 1–2
   clearing regions, grew at finer sim). Do NOT build on stock mode pre-P2.
4. **Bigger finish ball is the clean lever, no cascade needed.** Ø6 @ 0.42 =
   227k moves vs Ø3 @ 0.30 = 446k at *identical* 0.0075mm scallop → ~2× on the
   dominant op. Then ONE pencil. Est: Ø6 finish (~26 min) + Ø1 pencil ref-Ø6
   (17.5k moves, ~9 min) ≈ 35 min vs baseline ~40 min ≈ **10–15% faster**.
5. **The 3-tool nested cascade (Ø6 → Ø3 ribbon → Ø1) is ≈ a wash** vs the 2-tool
   (Ø6 → Ø1): the Ø3 ribbon pass (25k moves, slow finish feed) costs about what
   it saves the final pencil. Cascade complexity (region routing P3/P4) is **not
   justified by this part** on current estimates.

#### P0 verdict

The dramatic time win people imagine from multi-tool cascading **is not here** —
because a ball's rest is thin ribbons whose total path length barely shrinks, and
cleaning them needs slow finish-feed passes. The real, simple levers:
- **(a) Use the biggest ball that holds the scallop spec for the finish** (Ø6 not
  Ø3 → ~2× on the dominant op), then a single pencil. Free, no new machinery.
- **(b) Don't finish what needs no finishing** (slope/height-limited finishing:
  steep walls the rough left clean, near-flat regions) — the untested bigger lever.

**Open (needs the heavy A/B sim — this is really P5, not P0):** confirm the
~10–25% with machine-profile wall-clock, since (4)/(5) rest on move-count + feed
*estimates*, not timed sim. Recommend running that A/B before building any
region-routing (P3/P4). Steep/height-limited finishing (b) is the more promising
unexplored direction than the tool cascade.

#### P0 addendum — slope split probe (Ø3 finish, sim-free)

Split the Ø3 finish by surface slope (drop_cutter `slope_from`/`slope_to`):

| Band | moves | cutting mm | **rapid mm** |
|---|---|---|---|
| flats ≤40° | 367,171 | 194,682 | 96,337 |
| steep 40–90° | 165,523 | 178,916 | **148,099** |

(bands overlap at the 40° boundary — not cleanly additive; boundary samples
double-count.) First read was "steep band is ~50% air, so route steep→waterline."
**That was WRONG — the follow-up probes below disproved it.**

#### P0 addendum 2 — steep_shallow FAILS, and the real lever is LINKING

Two decisive probes (Ø3, threshold/scallop-matched, sim-free):

| Variant | moves | cutting mm | rapid mm | total motion |
|---|---|---|---|---|
| **idx 7 baseline** (real, region-restricted, tuned linking) | 336,543 | 103,712 | **133,585** | 237,297 |
| steep_shallow (thr 40°, z_step 0.3) | 544,169 | 442,645 | 361,175 | **803,820** |
| **bare Ø3 drop_cutter** (full area, DEFAULT linking) | 446,224 | 150,557 | **3,795** | 154,352 |

1. **steep_shallow is decisively WORSE here — 3.4× total motion.** On organic
   relief the "steep" regions are scattered texture, not coherent walls, so
   waterline Z-levels fragment into thousands of tiny loops with rapids between.
   **Retract the "elevate steep_shallow" recommendation.**
2. **The real waste is LINKING/rapids.** idx 7 rapids **133,585 mm**; a bare
   default-linked identical finish rapids **3,795 mm** — a **35×** gap from config
   alone. idx 7 is region-restricted (cuts 103k vs the bare 150k — it correctly
   avoids the lakes) but retracts to safe-Z between every resulting fragment.
   That's ~130,000 mm of rapid created by fragmentation + full retracts — the SAME
   defect we already fixed for the pencil (dead `hookup_distance` → surface-link,
   which cut pencil motion −72% on the low-accel machine). The finish's linking
   likely has the same problem.
3. On this low-accel router, retract/re-plunge is Z-accel-limited (slow) — so that
   130k mm of rapid plausibly dominates cycle time. **This is the new top lever.**

**Corrected P0 ranking of levers:**
1. **Finish linking / rapid reduction** (surface-link / defer retracts between
   fragments, like the pencil hookup fix) — biggest suspected win, ~130k mm rapid.
2. Bigger scallop-matched finish ball (Ø6 not Ø3) — clean ~2× on cutting passes.
3. ~~Tool cascade~~ — a wash; don't build P3/P4.
4. ~~steep_shallow / slope-splitting~~ — WORSE on organic relief; abandoned.

**Everything past here needs the machine-profile A/B sim**, because rapid-mm and
cut-mm convert to *time* very differently (rapid feed ≫ cut feed, but accel-limited
Z retracts are slow). That timed sim is the genuine next step — the move-count
probes have done their job: they killed two build directions and surfaced the
linking lever for free.

#### P0 addendum 3 — ROOT CAUSE of the finish's 133k rapid (code + live confirmed)

idx 7 has a **Machining Boundary = Model Silhouette (Inside)** (live GUI geometry
tab). My bare probe had no boundary. Toggling idx 7's boundary OFF and regenerating
reproduced the bare probe (rapid collapses to ~3.8k). **The boundary clip is the
fragmentation source.** Mechanism, traced in code:

- `generate_drop_cutter` (execute.rs:1433) itself rapids very little — with min_z
  filtering, `raster_toolpath_from_grid` (toolpath.rs:440) only splits a row where
  points fall to the min_z clamp, and on this continuous terrain almost nothing
  does (bare probe = 3,795 mm rapid).
- The boundary is applied as a **post-clip**: `clip_toolpath_to_boundary(tp,
  boundary, safe_z)` in `boundary.rs` (tests `clip_converts_outside_to_rapids`,
  `clip_reentry_plunges`). It converts **every span poking outside the silhouette
  into a full retract to safe_z + re-plunge**. On this crenellated terrain outline
  (jagged coast + corner cutoff) + the drop-cutter grid's 1-radius margin, that's
  ~133,585 mm of rapid.
- **No low-hop / surface-link anywhere on this path.** The smart gap-bridge linker
  ("bridge gaps ≤3 steps by cutting through, retract only between distant
  segments") exists ONLY in `raster_toolpath_from_grid_with_slope_filter`
  (toolpath.rs:542, doc lines 530-540) and in the pencil (`hookup_distance`,
  execute.rs:1117). The plain raster path and the boundary clip both hard-retract
  to safe_z. Same bug-family as the documented `3D_FINISH_BUGS.md` Bug 1 and the
  pencil's old dead hookup.

**Fix options (ranked):**
1. **Smart boundary-clip / raster linking** — when a clip or min_z gap is short and
   the surface between is safe, ride a low hop or cut through instead of full
   retract-to-safe_z (port the slope-filter variant's gap-bridge + the pencil's
   surface-link to `clip_toolpath_to_boundary` + `raster_toolpath_from_grid`).
   General fix; kills the 133k everywhere. Core-crate change + tests.
2. **Question the boundary** — drop_cutter already guards the part edge (the
   non-contact filter, execute.rs:1472-1486 "carves a trench around the part").
   So Model-Silhouette-Inside may be largely redundant for interior protection and
   only trimming the outer margin; a `stock` boundary, an offset, or none may clip
   far less. Cheap to A/B live.
3. **Lower the link/retract plane (Heights)** — if safe_z is high, each of the
   thousands of retracts is tall; a lower link plane cuts distance. Partial.

**Value:** ~130,000 mm of rapid eliminated (133,585 → 3,795). On this low-accel
router (retract/re-plunge is Z-accel-limited, slow), plausibly the single biggest
cycle-time lever — above tool size or strategy. Generalises to *any* 3D finish with
a boundary on a crenellated part. Convert to minutes with the machine-profile sim.

- LIVE-ONLY discipline honored: probes added, measured, removed; pencil reference
  restored to t8. Project never saved.

#### P0 addendum 4 — AGGRESSIVE coarse Ø6 ceiling (2026-07-04, sim-free)

The earlier Ø6 probe was scallop-MATCHED (0.42 stepover, h≈0.0075) — timid, and
NOT the user's "aggressive coarse" intent. Re-probed Ø6 (tool 12) at aggressive
stepovers on a fresh drop_cutter mirroring idx7's config (Model Silhouette/Inside
boundary, min_z −20), so directly comparable to the idx7 REAL baseline:

| Coarse pass | moves | cutting mm | rapid mm | scallop h | vs baseline moves |
|---|---|---|---|---|---|
| **idx7 real Ø3 @ 0.30** (current fine finish) | 336,543 | 103,712 | 133,585 | 0.0075 | — |
| Ø6 @ 0.42 (scallop-matched, addendum-0) | 227,529 | 103,670 | — | 0.0075 | 1.5× |
| **Ø6 @ 1.0** | 41,191 | 41,156 | 4,098 | 0.042 | **8.2×** |
| **Ø6 @ 1.5** | 18,484 | 27,377 | 2,707 | 0.094 | **18.2×** |
| **Ø6 @ 2.0** | 10,394 | 20,386 | 1,847 | 0.172 | **32.4×** |

Findings:
1. **The coarse pass is cheap — dramatically.** Aggressive Ø6 collapses total
   motion 5–30×. Cutting 2.5–5.1× less, rapid 33–72× less.
2. **The coarse stepover ALSO dissolves the boundary-fragmentation rapid.** rapid
   4,098 / 2,707 / 1,847 mm (vs baseline 133,585) even WITH the same Model
   Silhouette boundary — because boundary-clip retracts scale with pass count, and
   a 1.0–2.0 stepover has ~10–30× fewer passes. So the addendum-3 linking lever is
   largely a *fine-tool* problem; a coarse pass barely touches it.
3. **This is only the coarse pass.** It leaves 0.042–0.172 mm scallop everywhere
   PLUS all the rest in concavities the Ø6 can't fit (rivers, lake edges, tight
   valleys). The open question is whether **(coarse Ø6 + fine rest) < (single Ø3
   finish)** — step 2 (the sim-in-the-loop rest test) answers it. Step 1's job was
   to establish the coarse ceiling: it is large.
4. Scallop by stepover (Ø6 R3, h=s²/8R): 1.0→0.042, 1.5→0.094, 2.0→0.172 mm.
   Baseline Ø3@0.3 = 0.0075. The fine rest tool cleans the delta.

Probe = fresh drop_cutter idx9 (id21, tool 12) added/measured; still present as the
step-2 coarse-pass candidate (remove or repurpose at step-2 close). Project not saved.

#### P0 addendum 5 — STOCK-referenced rest after a real Ø6@1.0 coarse pass (2026-07-04, sim-in-loop)

First working **mode-2 (stock)** measurement of the cascade. Setup: stripped the live
project to [coarse Ø6@1.0 drop_cutter (idx0)] + [Ø1 pencil rest-reader,
detector=rest_depth, stock_source=from_remaining_stock, MVD 0.2 (idx1)]; simulated
the coarse (0.75 mm), then generated the reader against its checkpoint.

**Getting mode 2 to resolve via MCP is fiddly — recipe that worked:**
1. reader must be **enabled** AND in the simulated ordering (disabled reader ⇒ its
   FromRemainingStock predecessor lookup fails ⇒ silent fallback to nominal mode 0);
2. sim with **both** coarse+reader enabled/in-order so the coarse checkpoint is built
   with the reader present; 3. only THEN generate the reader. Also: `run_simulation`
   **ignores the `enabled` flag for which toolpaths it sims** (sims all *generated*
   ones in index order) and **caches on resolution** (re-running the same resolution
   returns the prior result byte-identical — change resolution to force recompute).
   `total_runtime_s` in the sim summary is a stale project aggregate — don't trust it.

| Reference | pencil regions | clearing regions | coverage | skeleton mm | reader path |
|---|---|---|---|---|---|
| nominal (mode 0, bogus fallback) | 362 | 0 | 7.4% | 39,951 | 20,019 mv |
| P0 analytic Ø6 tool (mode 1) | 194 | 0 | 12.8% | — | — |
| **real stock after Ø6@1.0 (mode 2)** | **32** | **1** | **40%** | 7,484 | 18,256 mv / 15,497 cut / 9,522 rap |

Findings:
1. **The plan's central prediction is CONFIRMED live:** analytic always says "0
   clearing regions, all narrow pencil ribbons"; the **real stock shows 1 wide
   clearing region** — a broad area the pencil can't clean, only an adaptive3d area
   pass can. So the cascade genuinely needs BOTH detail strategies here, not pencil
   alone. This is the first empirical case for building P3 (rest-region → area pass).
2. **BUT stock coverage (40%) is inflated vs analytic (12.8%)** — consistent with the
   known steep-wall dexel phantom, worse at this 0.75 mm than at 1.0 mm. The genuine
   concave rest is nearer the analytic 13% narrow ribbons; the 40%/clearing-blob is
   part real, part phantom. A slope-gated or coarser measure would separate them (P2).
3. **The screenshot (rest_reader_stock.png) shows distributed rest, not a few rivers.**
   This relief has fine concavity everywhere, so a Ø6 leaves rest broadly — the detail
   tool does real work (18k pencil moves + a clearing region), NOT the ~free pencil the
   analytic-only estimate implied. Heavy rapid (9,522 mm) in the reader = the same
   fragmentation/linking lever (addendum-3) — a surface-link fix would cut it.
4. **Cascade still wins hugely on motion:** Ø6 coarse 41k + stock pencil 18k + one
   adaptive3d clearing pass ≪ single Ø3 finish 447,515 mv. The coarse→detail split is
   validated; the open build item is routing the 1 clearing region to adaptive3d (P3)
   and de-phantoming the wall over-read (P2).

Live project heavily mutated for this (7 toolpaths removed, 2 probes added) — RELOAD
wanaka200.toml to restore; never saved.

#### P0 addendum 6 — adaptive3d as the area-rest clearer + the FROM-REMAINING-STOCK FAIL-HARD FIX (2026-07-04)

Tried adaptive3d (FromRemainingStock) as the *area* rest pass for the wide clearing
region addendum-5 found. Two hard lessons:

1. **A fine tool doing AREA rest is the wrong tool.** A Ø1 ball adaptive3d at 0.4
   stepover over ~40% of a 240×250 part is enormous — it's "machine the whole surface
   with one tiny tool" relocated to the rest pass, the exact thing the cascade avoids.
   The correct routing (reaffirmed): **coarse Ø6 does area → Ø3 adaptive3d does the
   *wide* rest → Ø1 pencil does the *narrow* valleys.** Fine tool = length, not area.
   Also: pencil vs adaptive3d with the *same* tool reach the SAME geometry (tool radius
   sets reach); adaptive just *fills* the concavity vs pencil's centreline. To reach a
   valley narrower than the tool you need a *smaller* tool, not a different strategy.

2. **FromRemainingStock silently fell back to fresh stock and caused a 2-hour runaway.**
   The Ø1 adaptive's remaining-stock checkpoint didn't resolve (the finicky mode-2
   dance from addendum-5), so it fell back to clearing the WHOLE relief with a 1 mm
   ball → unbounded compute → swap thrash → wedged the GUI for 2 h. Removing the
   toolpath did NOT cancel the background compute thread; only killing the process did.

   **FIX (this session, uncommitted): rest machining now FAILS HARD instead of falling
   back to fresh stock.** Both resolution sites guarded:
   - core `session/compute.rs` (~1208): `FromRemainingStock` + no `prior_stocks`
     snapshot ⇒ `return Err(SessionError::OperationFailed(...))` (was: `None` ⇒ fresh).
   - GUI worker `controller/events/compute.rs` (~326): no prior simulated stock ⇒ set
     `ComputeStatus::Error` + error notification + `return` (was: warning + submit with
     `prior_stock: None` ⇒ fresh). Message tells the user to run a sim first, or set
     Fresh if it's the first op.
   This makes the sim→generate dependency (the `remaining_stock_sim_dependency_ux.md`
   plan) enforced, not advisory — you now CAN'T accidentally rough the whole part.

**Process safety learned:** don't fire heavy generates against maxed swap on the live
GUI; a fresh-fallback whole-part clear with a fine tool is unbounded. Validate rest
recipes on a fixture via core/CLI (no GUI, no thrash), bring only the winner to the GUI.

### P1 — Rest-field heatmap ("render the rest at each stage")

See the rest, stop reading counters off debug traces. Diagnosis tooling for
every later phase, and the fidelity question in particular.

- **Core:** factor the hillshade harness into a reusable renderer; expose a
  rest-field artifact (grid + bbox + colormap PNG) from `detect_rest_valleys`
  behind a debug/analyze entry point, selectable reference (mode 0/1/2).
- **GUI:** viewport overlay — textured quad over the stock XY bbox, rest depth
  color-mapped, toggle + alpha + min/max legend, per-reference-mode selector.
  This is also the legibility answer: *show* what the pencil will chase before
  generating.
- **MCP:** `render_rest_field(index, reference?)` → PNG path (agent-readable).
- Per-stage rendering = pick which sim checkpoint feeds `RestReference::Stock`;
  surface *screenshots* per stage already work via sim scrub + screenshot.

### P2 — Reference & fidelity hardening

- **Inferred prior-op tool default (small, high value):** `reference_tool_id`
  unset → default to the previous enabled toolpath's tool (same pattern as
  `RestConfig.prev_tool_id`), UI shows "Auto (⟨prior op⟩'s tool)". Hierarchy
  becomes: stock → explicit tool → inferred prior tool → nominal Ø.
- **Steep-wall phantom rest investigation (heatmap-driven):** compare stock vs
  analytic rest per-pixel; candidate fixes, chosen with data:
  (a) slope-aware rest measure (surface-normal distance, not Z distance),
  (b) erode/exclude wall cells where |∇z| exceeds a slope gate,
  (c) hybrid clamp: `rest = min(stock_rest, analytic_prior_tool_rest)` — kills
      phantom wall rest but also hides real coverage gaps; only if (a)/(b) fail.
- **MVD floor tied to sim resolution:** warn (or auto-floor) when
  `min_valley_depth` < ~sim cell size — below that, quantization noise passes
  the threshold (observed: 1 mm sim + MVD 0.4 = phantom everywhere).

### P3 — "Phase D" for real: route ClearingRegions to a clearing pass

- Extract region **polygons** (marching squares on the wide-mask), not bboxes.
- New `BoundarySource::RestRegions` (or equivalent injected-polygon mechanism)
  so a clearing op can be confined to those polygons + offset.
- **Architecture — advisor pattern, not hidden magic:** a "propose rest-clear
  ops" action (strategy-advisor precedent) that materializes N adaptive3d ops
  (FromRemainingStock, boundary=region polys, min_z from region depth) the user
  can inspect/edit/delete. Alternatives considered: pencil emitting clearing
  moves inline (muddies op semantics), composite op (parallel flow — against
  guardrails).
- Wire waterline `initial_stock` only if P0-C shows steep-wall bands dominate
  and adaptive3d handles them poorly.

### P4 — Shape-aware routing

Classify each rest region and route to the right strategy:

| Shape signal | Class | Strategy |
|---|---|---|
| chamfer width < fine tool Ø × k | narrow valley | pencil centreline |
| wide + mean surface slope < ~40° | shallow blob | adaptive3d / raster rest |
| wide + slope ≥ ~40° | wall band | waterline / contour |

Slope from mesh normals (already sampled for drop); width from the chamfer
distance transform (already computed). Extends the strategy-advisor precedent.

### P5 — Metric + acceptance

- **Post-sim residual rest volume** per toolpath on the cut trace
  (`rest_remaining_mm3`) — the number that says "done, nothing left" (R3 item).
- **A/B:** single fine-tool full-surface finish vs coarse finish + cascade, on
  wanaka + a terrain fixture. Acceptance: cascade total machine time
  meaningfully lower (target ≥30%) at equal-or-better residual rest.
  This re-runs the invalidated 2026-06-24 "rest doesn't pay" A/B
  (FromRemainingStock was dead code then — result void).
- Sentry test pinning detector behavior on a fixture (regression net pattern).

---

## Sequencing & risk

**P0 → P1 → P2 → P3 → P4 → P5.** P0 informs everything and costs no code.
P1 before P2's fidelity work (need eyes). P2-inferred-tool can land any time
(tiny). P3/P4 are the build; P5 closes the loop.

Risks: steep-wall stock fidelity (gates how much we trust mode 2 — P2);
sim-in-the-loop cost + invalidation cascade (mitigate later with sim scoped to
predecessor ops; UX badge plan already captured); machine memory during sims
(one cargo/sim at a time, check free/pgrep).

---

## P0 addendum 4 — LIVE A/B/C confirms the linking fix is a CODE change (2026-07-05)

The live tuned state was lost on an MCP/GUI reboot (empty GUI, no saved cascade
TOML). Reloaded `wanaka_full_tuned_agent_experiment.toml` (2026-05-27) and ran a
3-way finish-linking A/B on **idx 7 "3D Finish 6"** (drop_cutter, Tapered Ball
2mm tip, stepover 0.4, `boundary = model_silhouette/inside`, `link_moves = false`,
`stock_source = from_remaining_stock`). Decoupled from the rest cascade by setting
stock_source → Fresh (the drop-cutter path is stock-independent; all runs
consistent). Sim-free generate move-counts:

| Variant | moves | cut mm | rapid mm | note |
|---|---|---|---|---|
| **A** boundary inside, link off (baseline) | 63,470 | 26,216 | **5,492** | the shipped config |
| **B** boundary OFF | 63,000 | 29,347 | **2,890** | ceiling; NOT shippable |
| **C** boundary inside + `link_moves = true` | 63,470 | 26,216 | **5,492** | byte-identical to A |

Decision-grade findings:

1. **Mechanism reproduced.** The Model-Silhouette-Inside boundary adds
   +2,602 mm of clip-retract rapid (A vs B) — same defect the plan diagnosed on
   the live Ø3 finish, just ~50× smaller here because this is a coarse
   tapered-ball op (63k moves) not the fine full-terrain Ø3 finish (446k moves,
   133k rapid). Magnitude scales with silhouette-crossing count ∝ pass count ∝
   coverage/stepover.
2. **`link_moves = true` does NOT reach the boundary clip.** C is byte-identical
   to A (and the set returned `"applied": false`). The existing staydown/linking
   flag operates on the raster gap-bridge, NOT on `clip_toolpath_to_boundary`.
   ⇒ **The fix is a genuine core-crate code change, not a config flip.** Confirms
   addendum-3's code trace live.
3. **"Just turn the boundary off" is wrong.** B's cut distance *rises* +3,131 mm —
   the finish wrongly cuts outside the silhouette (into lakes/frame). You need the
   boundary AND smart linking; they are not substitutes.
4. **Timed context (coarse resolution 1.5 sim, kinematic time is resolution-free):**
   global cutting 745.7 s, air-cut 588.7 s, rapid 48.7 s (`total_runtime_s` 5104 is
   the known-stale project aggregate — ignore). On THIS coarse finish rapid is a
   small slice; the payoff is specific to the fine full-terrain finish where rapid
   is ~24× and would dominate. The MCP `per_toolpath` block exposes distances/moves
   but **no per-toolpath wall-clock**, and `get_cut_trace` `summary` timing is
   global (not per-op) — a faithful per-finish wall-clock A/B needs the fine Ø3
   finish rebuilt (or a per-toolpath time field added to the sim report).

**Verdict:** Build plan fix option #1 — port the surface-link / low-hop gap-bridge
into `clip_toolpath_to_boundary` + `raster_toolpath_from_grid` so short clip gaps
ride a low hop instead of retract-to-safe_z. There is no config shortcut (finding
2) and the boundary can't simply be removed (finding 3). This is the confirmed
#1 lever.

**Live state left modified (NOT saved):** idx 7 = Fresh / boundary inside /
link_moves true. Reload the TOML to restore.
