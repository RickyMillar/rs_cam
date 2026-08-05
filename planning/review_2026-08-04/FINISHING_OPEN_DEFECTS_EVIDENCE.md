# Finishing / export / agent-read open defects — evidence package

**Wave:** W8 (R7 lane, plan §H2 items H2.3–H2.6).
**Date:** 2026-08-05.
**Deliverable named by:** `TECH_DEBT_2_RESEARCH_AND_FIX_PLAN.md` §H2 "Research deliverables".
**Checkpoints this feeds:** **F2** (D-16.1 band run-off) and **F3** (D-16.2 Shallow
`stock_to_leave`, plus the agent-read defaults).

> **What is shipped in this wave and what is not.**
> H2.3 (the screenshot exporter, D-LV.1 + X-1) is **fixed and sentried** — the
> plan puts it at merge-order 20, "low risk", no checkpoint. Everything else in
> this document is **evidence only**: no production geometry moved, no gate,
> threshold, severity or default changed. D-16.1 and D-16.2 each carry a
> reproduction and a decision list, and each waits on an operator ruling before
> a line of generator code is touched, per plan §H2 fix-shape 4.

> **Standing corrections this document depends on.** Two counts quoted across
> the programme are stale and are corrected here, with sources:
> - `ToolpathStats` carries **14** report-only findings, not twelve and not
>   thirteen. Checkpoint C landed **two** slots, not one:
>   `offset_library_failures` (`crates/rs_cam_core/src/compute/config.rs:420`)
>   **and** `boundary_clip_dropped` (`:438`). `CLAUDE.md`,
>   `UNTOUCHED_TERRITORY_RISK_MAP.md:81` and `FINDINGS_PIPELINE_CENSUS.md:145-158`
>   all still say twelve.
> - W1's B7 hand-off says "three findings **plus `retract_trips`**" sit on the
>   wrong side of the narration gap. The three findings are confirmed;
>   **`retract_trips` is refuted** — see §3.D.

---

## 0. Index and owner table

| Item | Status after this wave | Owner | Gate |
|---|---|---|---|
| **D-LV.1 / X-1** — exporter vs viewport | **FIXED + sentried** (§1) | W8, done | none (plan: low risk) |
| **D-16.1** — band run-off overcut | Reproduction + mechanism + candidate table (§2) | operator ruling, then a named lane | **F2** |
| **D-16.2** — Shallow ignores `stock_to_leave` | Red exhibit committed + full semantics map (§3.A–3.C) | operator ruling, then a named lane | **F3** |
| **H2.6** — slow agent reads | Diagnosis + design, not implemented (§3.D) | operator ruling | **F3** (read-path portion) |
| **B7** — parallel narration-context literals | Diagnosis + helper sketch (§3.D.4) | folds into H2.6's PR | **F3** |
| **F-7** — adaptive3d sampling contract | Contract stated + fixture specified (§4) | adaptive3d lane | none — research only |
| **R7-L1** — MidSteep/Shallow clip observability | 20-site census + instrument proposal (§5) | later wave | none — report-only |

---

## 1. D-LV.1 + X-1 — the screenshot exporter (H2.3) — **FIXED**

### 1.1 What the two defects actually were

`D-LV.1` was filed live (`planning/review_2026-07-29/ORCHESTRATION_LOG.md:4992-5017`):
`screenshot_toolpath` rendered op 8's new-style `UnifiedFinish` emission
near-empty while the same toolpath simulated to a clean carve, and the operator's
follow-up viewport check scoped it to **the exporter only** — the live GUI 3D
viewport draws the same toolpath correctly.

`X-1` was found independently by W9's scout
(`UNTOUCHED_TERRITORY_RISK_MAP.md:106`) and handed here as "a *second* site of
D-LV.1's shape". It is the precise, code-level statement of the same thing: two
independent interpreters of one value, one on the production path.

The two classifiers disagreed **three** ways. All three are confirmed by reading:

| # | Aspect | Viewport (`crates/rs_cam_viz/src/render/toolpath_render.rs:212-246`, parent `6396eb0`) | Exporter (`crates/rs_cam_core/src/stock_mesh.rs:181-203`, parent `6396eb0`) |
|---|---|---|---|
| 1 | Walk direction | **Forward** — outermost span first | **`path.iter().rev()`** — innermost first |
| 2 | `DressupArtifact` | Records `decision`, keeps scanning; a nested `Entry` still wins | **Early-returns** — the dressup wins |
| 3 | `DepthPass` | Collects `pass_index` → per-pass lightness shift | **Ignored entirely** — flat `CUT_COLOR` |

Consequence: a move nested inside two interesting spans was coloured by the
**outermost** kind on screen and the **innermost** kind in the PNG, and a
multi-pass 2.5D operation exported as one undifferentiated green — the exact
structure the six-view export exists to show.

`spans_valid` is correctly guarded on both sides (`stock_mesh.rs:159`,
`toolpath_render.rs:213`), so that is not part of the defect. W9's note on this
was right.

### 1.2 The fix: one classifier, two renderers

Per plan fix-shape 3 ("reuse the viewport's classification/intent decision or a
shared renderer adapter"), the decision moved into core:

- **New:** `SpanClass` and `AnnotatedToolpath::classify_span_path(&[SpanId]) -> SpanClass`
  in `crates/rs_cam_core/src/toolpath_spans.rs`. Its doc block states the six
  rules and names X-1.
- **Viewport** (`crates/rs_cam_viz/src/render/toolpath_render.rs`): the private
  `SpanColor` enum and its 30-line walk are deleted; `classify` is now a
  two-line delegation, and the match arms rename `SpanColor::*` → `SpanClass::*`
  (`Default {pass_index}` → `Cut {pass_index}`).
- **Exporter** (`crates/rs_cam_core/src/stock_mesh.rs`): `span_color` is now a
  five-arm map from `SpanClass` to the existing palette constants, **plus** the
  previously-absent depth-pass gradient via a new `pass_shifted()` helper that
  reproduces the viewport's `1.0 + ((p % 4) - 1.5) * 0.06` curve exactly.

**The viewport is the reference and the exporter is the side that moved.** That
is deliberate and is stated in both files: the operator validated the viewport,
so bending the shared classifier toward the exporter's old behaviour would have
been the wrong repair.

### 1.3 Red-first proof, and why it is pinned rather than checked out

Sentry: `crates/rs_cam_core/tests/exporter_span_classifier_x1.rs`.

Four tests assert the exporter's **emitted vertex colours** — the bytes that
reach the PNG, not the classifier in isolation — and each fails on the parent
revision:

| Test | Parent behaviour | Asserted |
|---|---|---|
| `nested_entry_inside_link_bridge_takes_the_outer_kind` | cyan (inner `Entry`) | grey (outer `LinkBridge`) |
| `nested_dressup_inside_entry_does_not_beat_the_entry` | brown (inner `Dressup`) | cyan (outer `Entry`) |
| `dressup_wrapping_a_lead_out_yields_to_the_lead_out` | brown | magenta |
| `depth_passes_are_visually_distinguishable_in_the_export` | one flat green | three distinct shades of the same hue |

**On the form of the red-first evidence.** The programme's rule is that a fix is
demonstrated red on the parent revision. Checking out `6396eb0` was **not
available to this wave**: the working tree is shared with two other live lanes
(C-impl in polygon/boundary/session-compute/pocket, D-impl-2 in
tool_load/locality), and stashing the fix to run a build would have handed them
the wrong source mid-compile. Instead the parent's walk is **transcribed
verbatim** into the sentry as `parent_revision_exporter_walk()` and
`the_parent_revisions_walk_disagrees_with_the_shipped_classifier` asserts, on
the same fixtures, that it gives a different answer from the shipped classifier
on all three of X-1's aspects. That is the same evidence, it is executable in
perpetuity, and unlike a one-off checkout it keeps failing if anyone
reintroduces the reversed walk. **This substitution is disclosed rather than
glossed.**

### 1.4 Proof that the viewport did not move

Three independent bars, because "we changed the shared file too" is exactly the
claim that needs more than a diff:

1. **The diff is a delegation.** `toolpath_render.rs` loses a type and a walk
   and gains a two-line call; every colour expression, the `span_filter` arms,
   `push_dashed_segment`, `z_color`, `brighten` and the palette are untouched.
2. **`viewport_reference_rules_are_unchanged`** pins the classifier's answers to
   the rules the viewport implemented *before* extraction — `Entry` beats a
   `DepthPass` and drops the pass index; a move outside the `Entry` keeps it;
   `GeometryRefit` is transparent; `Region` and `Operation` are transparent; an
   invalidated span table yields `Cut { pass_index: None }`. It is green on both
   sides. Had the shared classifier been bent toward the exporter, this fails.
3. **`GeometryRefit` semantics are preserved verbatim.** Both renderers carried
   an uncommitted hunk from the simulation/arcfit lane making arc-fit spans
   transparent (Checkpoint D Q3, ruled 2026-08-04). Those hunks sit *inside* the
   functions this change replaces, so absorbing them was unavoidable; their
   **meaning is preserved exactly** (rule 5 of the classifier), and
   `viewport_reference_rules_are_unchanged` pins it. **No other lane's hunk was
   reverted, amended or "fixed".**

### 1.5 What this does NOT close — stated, not implied

The sentry adds `exported_geometry_stays_inside_the_toolpath_envelope`, which
bounds the exporter's emitted mesh to the toolpath's own bounding box plus one
ribbon radius. That is the geometry-domain guard for D-LV.1's *near-empty*
symptom: a renderer that emits geometry outside the path makes
`render_mesh_composite`'s auto-fit (`crates/rs_cam_core/src/fingerprint.rs:646-655`)
zoom out until everything real collapses to a few pixels.

**But the near-empty symptom itself was not reproduced in this wave, and I will
not claim it was.** What was established:

- The exporter drops **no** cutting geometry by classification — every non-rapid
  move becomes a prism regardless of span kind
  (`crates/rs_cam_core/src/stock_mesh.rs:205-238`). So "classification not
  receiving the new emission's intent data", the original suspect recorded at
  `planning/review_2026-07-29/ORCHESTRATION_LOG.md:4996`, is **refuted** as the
  cause of emptiness. It was, however, the correct suspect for X-1.
- A promising alternative — that the exporter's `linearize_arc` blows a
  near-zero sweep up to a full `TAU` circle
  (`crates/rs_cam_core/src/arc_util.rs:55-57`), exploding the auto-fit bbox,
  which would explain "viewport fine, exporter broken" because the viewport
  draws arc chords and never linearises — is **also refuted for the current
  tree**: `dexel_stock/simulation.rs` and `viz.rs` call the *same*
  `linearize_arc`, and the op in question simulated to a clean carve with zero
  collisions. Additionally `arcfit`'s reflex-direction guard
  (`crates/rs_cam_core/src/arcfit.rs:461-506`) shipped 2026-07-28, before the
  D-LV.1 observation.
- The live reproduction is a 199,745-move op on a user-modified project file
  (`planning/airrun_2026-06-01/wanaka.toml`) that this wave is forbidden to
  touch, and no committed fixture of that size exists.

**Honest disposition:** X-1 is fixed and proven. D-LV.1's *classification* half
is fixed and proven. D-LV.1's *near-empty* half is **not reproduced**, has two
refuted hypotheses on the record, and now has a permanent geometry-domain guard
that would catch the envelope-explosion class. If it recurs, the next reporter
should capture the exporter's `auto_ribbon_radius` and the tube mesh's bbox
alongside the PNG — those two numbers discriminate every remaining hypothesis in
one shot.

---

## 2. D-16.1 — banded finish overcuts where a band runs off the stock (→ **Checkpoint F2**)

### 2.1 The filing, restated

On `grooved_block(2.5, 70°, 1.2)` the `UnifiedFinish` band-mix arm (B) leaves a
**−235 µm overcut** — ten times the 22.5 µm cusp the dials asked for — while
all-over scallop (arm D) on the same fixture, same tool, same cusp target
leaves **−34.1 µm**. The bulk of the surface is a wash (±25 µm on-size 96.93%
vs 97.65%; p90 actually *better* on the mix arm). **A tail, not a shift.**
Rendered before it was written down, which changed the description: the arms
differ ONLY inside the groove, and the red cells sit at the groove's
**longitudinal ends, where the band runs off the block footprint**
(`planning/review_2026-07-29/ORCHESTRATION_LOG.md:4737-4762`).

### 2.2 The mechanism — two stages, both live at shipped defaults

**Stage 1 — the band polygon grows OUTWARD past the footprint.**
`crates/rs_cam_core/src/finish_planner.rs:473` extracts band polygons via

```rust
region_polygons_from_mask(&mask, origin_x, origin_y, cell, params.overlap_mm.max(0.0))
```

and `region_polygons_from_mask`
(`crates/rs_cam_core/src/region_mask.rs:63-71`) runs a whole-grid Euclidean
distance transform and **dilates the mask by `dilate_mm` before marching
squares, with no re-clamp to coverage afterwards**. `overlap_mm` defaults to
**2.0** (`crates/rs_cam_core/src/compute/operation_configs.rs:1047`) — this is
`UnifiedFinishConfig::default()`, not a harness-only dial. So the MidSteep band
polygon extends ~2 mm past the last covered classification cell, i.e. ~1.875 mm
past the mesh footprint after the 1-cell erosion.

**Stage 2 — a quantized coverage guard admits the resulting off-footprint ring
points.** The dilated polygon *is* the scallop ring-cascade seed boundary
(`crates/rs_cam_core/src/scallop.rs:2180-2183`; the first ring *is* that
boundary verbatim, `:1224`). `ring_to_3d` (`:905-921`) keeps a vertex when
`finite && heightmap_covered_at_world(...)`, and `heightmap_covered_at_world`
(`:759-774`) **rounds the query XY to the nearest cell of the GENERATION
heightmap and reads that cell's flag.** The generation heightmap's cell is
`envelope_radius_mm() / 4` (`scallop.rs:1819-1824`,
`crates/rs_cam_core/src/finish_setup.rs:148-158`) — **0.75 mm on the H4 tool,
six times the 0.125 mm classification cell** (`unified_finish.rs:1240-1245`).
So the guard admits points up to **half a generation cell (0.375 mm) outside
the true footprint**, and at such a point `point_drop_cutter` returns a
**rim-riding CL** — the ball resting on the mesh's end edge — whose tip sits
`r − √(r² − d²)` **below** the surface. That is the overcut.

### 2.3 Two prior suspects, refuted

- **The `min_z` sentinel is NOT the mechanism.** `ring_to_3d`'s off-mesh
  sentinel (`scallop.rs:915`) never reaches emission: a `kept == false` vertex
  is excluded by `split_run_ranges` / `closest_kept_point_idx`
  (`scallop.rs:2303-2325`), so it only forces a retract. Same for the two
  in-refinement guards (`:1035-1038`, `:1085-1088`). **Well guarded — scratch it.**
- **The morphological close is NOT a growth site.** At
  `finish_planner.rs:422-429` the dilation half is immediately re-clamped:
  `and_masks_in_place(&mut steep, covered)` against the **eroded** mask. It
  cannot reach the footprint. Likewise `absorb_small_regions` (`:729-805`) only
  *relabels* cells that already carry a band, and the `MAX_REST_REGIONS`
  truncation and sub-cell loop drop (`region_mask.rs:82-87`, `:113-135`) are
  purely subtractive.

### 2.4 The MidSteep/Shallow asymmetry — sharper than "no guard"

The Shallow band's off-mesh guard (`unified_finish.rs:1886-1913`) is **exact**:
for every raster grid point it queries `index.query(x, y, 0)` and tests
`tri.contains_point_xy(x, y)`, demoting misses to `contacted = false` which
`raster_toolpath_from_grid` then filters. Its own comment names the failure it
prevents: *"the tool rides the edge and carves a trench around the part."*

**So MidSteep is not missing a guard — it has one, quantized to a grid 6×
coarser than Shallow's exact test.** That is the asymmetry to state, and it is
a more actionable one than "no guard at all". VerySteep is not exercised on
this fixture (70° walls vs `waterline_threshold_deg = 75`).

`clip_toolpath_to_boundary` is not in play: it is a session-level feature
driven by an operator-configured machining boundary, unconfigured in the
reproduction — and a boundary polygon is not the mesh footprint anyway.

### 2.5 Why arm D is clean — the discriminator

This is the sharpest fact and it is now precise. **Arm D's ring-cascade seed
boundary is the mesh-bbox rectangle** (`scallop.rs:2072-2114`, taken via the
`_ => vec![boundary]` arm at `:2182`), which lies exactly *on* the footprint,
and every subsequent ring is offset **inward**. **Arm D therefore never
evaluates a CL outside the footprint at all.** Arm B replaces that seed with a
band polygon dilated 2 mm **outward**, so its first several rings sit entirely
outside the footprint and one of them lands in the quantization-admitted strip.
*Same scallop engine, same guard, different seed.*

### 2.6 The six rendered facts, each accounted for

| Fact | Accounted for by |
|---|---|
| (a) overcut — too **deep** | rim-riding CLs are strictly below the surface |
| (b) ≈10× the 22.5 µm cusp | analytic bound with this tool: `d_max = 0.375 mm`, `r_tip = 0.5` → `0.5 − √(0.25 − 0.1406) = 169 µm`; superposed on arm D's own ≈34 µm chord-refinement residual gives ≈203 µm against the reported 235 |
| (c) confined to the groove | only the 70° walls classify MidSteep→scallop; rim and floor are Shallow→raster, whose guard is exact — hence pixel-identical outside the groove |
| (d) at the longitudinal **ends** | the walls are the only band reaching the footprint edge, and only in Y; also phase-dependent — `y = 12.0` is exactly a 0.75 mm cell centre given `origin_y = −15.0`, so the full 0.375 mm is admitted |
| (e) a tail, few cells | only the column rows within a tool radius of `y = ±12` under a MidSteep polygon — a few hundred of 96,641 |
| (f) absent from arm D | §2.5 |

**Uncertainty, stated.** ≈30 µm of the 235 is unaccounted by the 169 + 34
superposition. The most plausible remainder is straight-line interpolation from
an admitted off-footprint CL inward across a decimated ring chord (floor
`cell × 0.75 = 0.5625 mm`) or a ring-to-ring cutting connector
(`link_threshold = cusp_r × 3 = 1.5 mm`, `scallop.rs:2285`). **This is
inference and is not closed.** The quantization phase argument is hand
arithmetic. Arm D's ≈34 µm matching the chord-refinement accept threshold
(`tolerance × CHORD_REFINE_ACCEPT_FRACTION = 50 × 0.70 = 35 µm`,
`scallop.rs:898`) is a strong but circumstantial corroboration.

### 2.7 Fixture non-vacuity — the groove DOES reach the edge

`crates/rs_cam_core/tests/common/meshes.rs:239-320`. The generator is a pure
heightfield **extruded along Y**: `z_at` is a function of X only (`:248-263`),
and the vertex loop (`:296-300`) sweeps `y` from `−y_half` to `+y_half`
(`y_half = 12.0`, `x_extent = 20.0`, `:203-206`, none of which has a builder
setter). **There are no end walls.** The mesh is an open single sheet — no
sides, no bottom — and the groove profile runs continuously from `y = −12` to
`y = +12`, **opening onto the footprint edge at both ends**.

The two MidSteep wall strips (`|x| ∈ [2.063, 2.5]`, since
`floor = 2.5 − 1.2/tan 70° = 2.063`) are 0.437 mm × 24 mm = **10.5 mm² each**,
comfortably above `min_region_area_mm2 = (2 × 0.5)² × 4 = 4 mm²`, so they
survive absorption and each becomes a scallop region whose 2 mm-dilated polygon
reaches `y ≈ ±13.875`.

**Four conditions the run-off population needs, all met here — and this is the
statement the acceptance gate's "cannot pass on a convex fixture" clause
requires:**

1. a band whose strategy generates from **region polygons** (MidSteep→scallop)
   must **touch the footprint edge** — `45 < 70 < 75`; ✓
2. `overlap_mm > 0` — default 2.0; ✓
3. the band must survive `min_region_area_mm2`; ✓
4. the footprint edge must be **open** — no wall turning the surface steep and
   terminating it inside the block. ✓ **A dome or an interior terrain patch
   fails condition 4, which is exactly why a convex fixture cannot adjudicate
   this.**

Column-population sanity from the filing: footprint 40 × 24 = 960 mm² ÷
(0.1 mm)² = 96,000, matching the reported 96,641; and
`collect_column_deviations` returns `None` for any column whose XY misses the
model (`crates/rs_cam_core/src/compute/simulate.rs:1064`), so **the −235 µm
column is genuinely on-footprint** — not an artifact of measuring outside the
part.

### 2.8 Reproduction — REPRODUCED, and the predicted bound landed exactly

`crates/rs_cam_core/tests/band_run_off_reproduction_d16_1.rs`.
**5 passed / 0 failed, 13.58 s** — a Tier-0 probe, one mesh build and one
generation per arm, **no simulation**.

Renders written **before** this verdict was written, and read:
`planning/review_2026-08-04/artifacts/w8/d16_1_runoff_xy_all.pgm`,
`d16_1_runoff_end_strip_y_pos.pgm`, `d16_1_runoff_end_strip_y_neg.pgm`,
`d16_1_runoff_summary.txt`.

| Measure | Value |
|---|---|
| Fixture | `grooved_block(2.5, 70°, 1.2)`, footprint x ∈ [−20, 20], y ∈ [−12, 12] |
| Tool | Ø1 tip / 7° / Ø6 shank — envelope r 3.000, cusp r 0.500 |
| `overlap_mm` | 2.0 (**shipped default**) |
| Moves | 14,221 total; 14,141 cutting |
| Band split | shallow 10,856 · mid-steep 3,365 · **very-steep 0** |
| **Off-footprint cutting targets** | **131** (FinishingCut 123, EntryPlunge 8) |
| **Worst distance past the footprint edge** | **0.3750 mm** |
| Violating X range | −4.438 … 4.141 mm (**the groove**) |
| Violating Y range | −12.375 … 12.349 mm (**past both ends**) |
| Attribution — MidSteep/scallop | **131 off-footprint of 3,287 cutting** |
| Attribution — Shallow/raster | **0 off-footprint of 10,854 cutting** |

**The mechanism is confirmed, not merely consistent.** §2.2 predicted the
admitted strip would be **half a generation cell — 0.75 / 2 = 0.375 mm**. The
measured worst run-off is **0.3750 mm**. That is the quantization bound landing
on the nose, and it is the strongest single piece of evidence here.

**The asymmetry of §2.4 is measured, not inferred:** the Shallow band's exact
point-in-triangle guard admits **zero** of 10,854, while the MidSteep band's
cell-rounded guard admits **131** of 3,287, on the same part in the same
generation. This retires "MidSteep is missing a guard" in favour of "MidSteep's
guard is quantized to a grid 6× coarser than its neighbour's".

**The renders reproduce the original filing's picture.** The XY map shows the
violating population confined to the groove's X range and breaking out of the
footprint rectangle at the top and bottom edges only; the end-strip renders show
the marks sitting outside the `y = ±12` boundary line, with nothing crossing
outside the groove's X span. That is `groove_b_dev.png`'s "red cells at the
groove's longitudinal ENDS" reproduced from an independent harness.

**Non-vacuity is proven three ways, not asserted:**

1. `the_grooved_block_fixture_actually_reaches_its_footprint_edge` proves by
   geometry that the fixture is an open extruded sheet with no end walls, so the
   70° wall band genuinely opens onto the footprint. **This is the test that
   would fail on a convex/no-boundary fixture**, which is the plan's explicit
   gate condition.
2. `the_overlap_dial_is_the_cause` re-runs the identical arm at
   `overlap_mm = 0.0` and the population collapses — the dilation is the cause,
   not something incidental to scallop.
3. The population is attributed **by region span**, not by a Z heuristic or a
   label literal, so "131 off-footprint" is provably MidSteep's.

**One honest qualification on the depth domain.** `very_steep` is 0 here, as
predicted (70° < the 75° waterline threshold), so this fixture exercises the
MidSteep↔Shallow pair only. And a further **52 off-footprint `Linking` targets**
were found and deliberately **not** asserted on: a link crossing outside the
footprint is a different question from a *cut* doing so, and folding them in
would have inflated the population with something the fix may legitimately
leave alone. They are reported in the artifact for completeness.

**What is still not measured:** collision counts, residual standing material and
runtime for any candidate repair. Those need both arms through the simulator at
0.1 mm — the Tier-1 job, scoped in F2-Q4. **No candidate in §2.9 should be ruled
"ship it" on Tier-0 evidence alone.**

### 2.9 Candidate behaviours — the operator's table

| # | Behaviour | Code shape | Cost | Regression risk |
|---|---|---|---|---|
| **(i)** | Clamp each band polygon to the **coverage mask / mesh footprint** after extraction, before it becomes a ring boundary | AND the dilated mask back against `covered` inside `region_polygons_from_mask`, or clip the returned `Polygon2`s in `finish_planner::decompose` | ~free — one extra mask AND, or a polygon boolean | **Must clamp to COVERAGE, never to the BAND.** Clamping to the band would kill the point of `overlap_mm` — bands would stop overlapping *each other*, reintroducing the inter-band seam the dial exists to close. Moves geometry on every `UnifiedFinish` in the repo |
| **(ii)** | Give MidSteep the Shallow band's **exact** guard — replace `heightmap_covered_at_world` with the point-in-triangle test at the three `ring_to_3d` / `refine_chord` sites (`scallop.rs:911`, `:1035`, `:1085`) | one shared helper, three call sites | one spatial-index query per ring vertex — **measurable**; `ring_to_3d` is already the ring loop's hot path (cancel is polled per ring for exactly that reason, `:1188-1190`) | Slightly more material left at genuine footprint edges. Moves geometry on **every scallop op repo-wide**, including arm D and standalone Scallop — re-pins fingerprints broadly |
| **(ii′)** | Keep the heightmap test but stop rounding — bilinear-sample the coverage, or AND it with the exact test | ~one line in `heightmap_covered_at_world` | negligible | Same repo-wide geometry move, smaller magnitude |
| **(iii)** | Erode the band mask by the tool radius at the footprint edge before extraction | one EDT against the complement of coverage | ~free | **Leaves an unfinished tool-radius collar at every part edge — a worse defect (standing material) than the one being fixed. Recommend refusing outright** |
| **(iv)** | Report-only: a `ToolpathStats` finding "band polygon extends N mm² past the model footprint" | new slot + narration + ~5 emit sites | free | Ships a known −235 µm overcut. The programme's own rule — *a warning nobody sees is not a warning* — argues against this as the terminal answer |

**Recommendation: (ii′) first, then (i) narrowed to a coverage clamp.** (ii′) is
a one-line, physically-correct repair to a guard that is *already the right
idea* and merely mis-quantized, and it is the honest one because the Shallow
band next door already does exactly this. (i) then removes the pathological
input rather than only rejecting its output.

**Both move geometry repo-wide** — (ii′)/(ii) touch every scallop op including
standalone Scallop and arm D; (i) touches every `UnifiedFinish`. **That is why
the original filing called it "a wave, not a footnote", and that judgement
stands.** No collision/residual/runtime numbers are offered here because none
were measured: measuring them requires running both arms through simulation at
0.1 mm, which is the Tier-1 job §2.8 scopes and which no ruling should be made
without.

---

## 3. D-16.2, and the agent-read defects (→ **Checkpoint F3**)

### 3.A D-16.2 — where the dial dies

The Shallow band drops `stock_to_leave` at **exactly one call boundary**:
`crates/rs_cam_core/src/unified_finish.rs:1919` hands the raw drop-cutter grid
to `raster_toolpath_from_grid`
(`crates/rs_cam_core/src/toolpath.rs:607-615`), whose six parameters are
`(grid, feed_rate, plunge_rate, safe_z, min_z, boundary_regions)` — **no
stock-to-leave argument, and no Z arithmetic anywhere in the function.** Every
emitted target is `grid.get(row, col).position()` verbatim. Nothing downstream
compensates: `UnifiedFinish` is a `strip_all` op for dressups
(`crates/rs_cam_core/src/compute/catalog.rs:1870`), `arc_fitting` is
Z-preserving, and `optimize_entry_descents`
(`crates/rs_cam_core/src/dressup.rs:200-227`) only shortens descents.

The band dispatch is one `match`:

| Band | Arm | Honours `stock_to_leave`? |
|---|---|---|
| `VerySteep` (waterline) | `unified_finish.rs:1726` | **NO** — see the twin, §3.B |
| `MidSteep` (scallop) | `:1796` | **YES**, threaded at `:1814` |
| `Shallow` (raster) | `:1852` | **NO — this is D-16.2** |

**The convention question resolves in the fix's favour, contrary to the risk the
brief anticipated.** All four honouring paths apply the same thing — a pure
**+Z shift on the drop-cutter contact point**, `cl.z + stock_to_leave`. None uses
a surface-normal offset; none uses a tool-radius-adjusted offset:

| Consumer | Site |
|---|---|
| MidSteep scallop | `crates/rs_cam_core/src/scallop.rs:913` (off-mesh sentinel `:915`; chord-refinement probes `:1039`, `:1089`) |
| crease / claims pencil | `crates/rs_cam_core/src/pencil.rs:433`, in `lift_to_surface` |
| surface links (inter-region) | `crates/rs_cam_core/src/surface_link.rs:49`, from `unified_finish.rs:2466` |
| surface links (intra-region relink) | same `:49`, from `unified_finish.rs:1940` |

So a naive `+Z` fix on the raster band is **consistent with all three** and in
fact *removes* two existing mixed-convention seams: today a MidSteep exit at
`surface + stl` linking into a Shallow entry at `surface + 0` steps down by
`stl`, and an intra-region link rides `stl` above the raster fragments it
connects. A surface-normal fix on the raster band alone would be the one that
creates a seam. `crates/rs_cam_core/src/steep_shallow.rs:468` is a shipped,
working precedent for exactly this band type on a standalone op.

**Flagged and deliberately NOT coupled:** the shared vertical convention is
itself an approximation — a `+Z` lift leaves only `stock_to_leave·cos θ` measured
normal to the surface, i.e. 0.71× to 0.26× of the dialled value across the
mid-steep band's own 45°–75° domain. Every finish op in the repo shares it
(`radial_finish.rs:119`, `spiral_finish.rs:211`, `horizontal_finish.rs:270`,
`steep_shallow.rs:264,468`, `ramp_finish.rs:587-591,727`) and two pin it in unit
tests, so it is a deliberate repo-wide convention — but the doc comment at
`unified_finish.rs:142` says "stock to leave **on the surface**", which is
normal-direction phrasing for a vertical quantity. **Do not couple this to
D-16.2.**

### 3.B The twin the filing missed — VerySteep drops it too

`WaterlineParams` (`crates/rs_cam_core/src/waterline.rs:23-32`) has exactly four
fields — `sampling, feed_rate, plunge_rate, safe_z`. There is no
`stock_to_leave` field, so the VerySteep arm at `unified_finish.rs:1726-1740`
**cannot** pass one. `grep stock_to_leave crates/rs_cam_core/src/waterline.rs`
returns nothing.

**Two of UnifiedFinish's three bands ignore the dial.** D-16.2 as filed names
only Shallow. This matters for the ruling: **fixing Shallow alone introduces a
`stock_to_leave`-sized step at the shallow↔waterline seam that does not exist
today.** That is the one genuine scope decision in F3.

The honest caveat already exists in the code, at `unified_finish.rs:142-143`:

> `/// Stock to leave on the surface (mm) — scallop path only (raster and waterline don't take one today; parity with the standalone ops).`

It sits on an internal params struct no operator ever sees, and its "parity with
the standalone ops" justification is **false** against `SteepShallow`, whose
shallow raster does apply it. It is absent from all three operator surfaces:

| Surface | Site | State |
|---|---|---|
| Config field | `crates/rs_cam_core/src/compute/operation_configs.rs:918` | **no doc comment at all** |
| GUI dial | `crates/rs_cam_viz/src/ui/properties/operations/surface_3d.rs:917-925` | user-settable 0–10 mm, **no caveat label** |
| MCP `ParamDef` | `crates/rs_cam_core/src/compute/catalog.rs:1403` | `ParamDef::required`, **no caveat** |

### 3.C D-16.2 — blast radius, and the red exhibit

**Blast radius is narrower than the deferral assumed.**

- **Other callers of `raster_toolpath_from_grid`:** only `generate_drop_cutter`
  (`crates/rs_cam_core/src/compute/execute.rs:2301`, `:2314`). `DropCutterConfig`
  (`operation_configs.rs:486-521`) has **no `stock_to_leave` field**, so
  standalone DropCutter has an *absent capability*, not a silently-dropped dial.
  No operator can set it there and get nothing.
- **All sibling finish ops that expose the dial honour it** — each via its own
  emit loop, none via this helper.
- **Committed configurations with a non-zero `stock_to_leave` on a UnifiedFinish
  or shallow-band op: NONE.** The only committed UnifiedFinish in a project file
  is `planning/airrun_2026-06-01/wanaka.toml:1056`, at `0.0`. Every non-zero hit
  in the repo is `Adaptive3dParams.stock_to_leave` or
  `stock_to_leave_axial`/`_radial` — a different field on a different engine.
- **Pinned fingerprints that would move: none identified.** `fingerprint.rs` has
  zero hits for `unified`/`UnifiedFinish`/`Shallow`; the five FNV constants in
  `crates/rs_cam_core/tests/transform_provenance_fingerprints.rs` are reachable
  only from Adaptive3d and face-chain fixtures; `param_sweep.rs` has no
  `sweep_unified_finish_*`. Twenty-three test files reference UnifiedFinish and
  **none sets a non-zero `stock_to_leave`**.

**Therefore: at `stock_to_leave = 0.0` — every committed configuration — the fix
is byte-identical.** The stated reason for deferring D-16.2 ("it re-pins
fingerprints") does not appear to hold on the current tree. *Caveat, stated: this
is an `rg` census over source and `.toml`, not a green run.*

**Predicted output shifts** for a fix mirroring `steep_shallow.rs:468` (lift
contacted grid points, and move the `min_z` sentinel and filter threshold
together, matching scallop's `ctx.min_z + ctx.stock_to_leave`):

- Every Shallow cut Z rises by **exactly `+stock_to_leave`**, uniformly, zero XY change.
- `cutting_distance` barely moves: a uniform Z translation changes no chord
  length; only each entry plunge shortens by `stock_to_leave`. **The sentry must
  assert on Z, never on distance.**
- **Emitted point count must not change.** This is the same contract pencil
  already pins (`crates/rs_cam_core/src/pencil.rs:2576`). **Review hazard worth
  naming:** if a fix lifts *all* points including the off-mesh sentinel without
  moving the `Some(effective_min_z)` filter, off-mesh points stop being filtered
  and the tool rides the mesh rim — the exact trench the guard at
  `unified_finish.rs:1888-1913` exists to prevent.
- Collisions could move slightly, most likely **downward** (raising the descent
  target increases clearance over thin uncut ridges). **Any comparison must be
  run at a single resolution** — the standing rule.
- **The MidSteep control must be green on BOTH sides**, and must select its
  population by `report.region_table` `move_range` filtered on
  `RegionKind::Band(FinishBand::MidSteep)` — never by a Z heuristic or a label
  literal.

### 3.C.1 The red exhibit — committed and RUN

`crates/rs_cam_core/tests/shallow_band_stock_to_leave_exhibit_d16_2.rs`,
commit `473a097`. W2's exhibit idiom exactly — not `#[ignore]`d, header stating
the form, each test's doc comment naming the one line that inverts it.
**4 passed / 0 failed** (3.16 s).

**This is a reproduction, not a code trace.** The defect is observed.

| Test | Today | After the fix |
|---|---|---|
| `d16_2_fixture_cuts_a_shallow_band_and_only_a_shallow_band` | green (non-vacuity control) | green |
| `exhibit_shallow_band_ignores_stock_to_leave_entirely` | **green — the defect** | **RED; invert `max_shift < 1e-9` to `(max_shift - STOCK_TO_LEAVE_MM).abs() < 1e-9`** |
| `control_mid_steep_band_honours_stock_to_leave_exactly` | green | green |
| `exhibit_very_steep_band_ignores_stock_to_leave_separate_defect` | **green — the twin** | red only when the *waterline* defect is fixed |

The three non-exhibit tests are what stop it being vacuous: the fixture
provably cut a shallow band and only a shallow band (so a zero shift is not
"nothing was generated"); the emitted-point count is pinned (so a fix cannot
satisfy the exhibit by changing *which* points are cuts); and **the MidSteep
control is green on both sides — the scallop band does shift by exactly the
dialled amount**, which proves the instrument works and that a zero on the
shallow band is the band's fault, not the measurement's.

Fixture: `make_test_hemisphere(20.0, 16)` — chosen over `grooved_block` because
a hemisphere splits by radius into shallow (≈628 mm²), mid-steep (≈542 mm²) and
very-steep (≈86 mm²) bands, all comfortably above
`min_region_area_mm2 = (2·cusp_r)²·4 = 36 mm²` for the Ø3 ball, whereas a
groove's wall band would need to be ~20 mm deep to clear absorption at 80° and
`GroovedBlock::y_half` has no setter. It is also the exact configuration
`capability_link_moves_safety.rs:1544-1612` already runs UnifiedFinish under
with a passing `very_steep_nodes >= 1` assertion.

**Four consumers of the dial had to be neutralised, not two.** Besides scallop
(`:1814`) and the intra-region relink (`:1940`), `stock_to_leave` also feeds the
claims/crease pencil emission (`:1492`, `:1680`) and the router's surface-link
costing (`:2466`). The fixture pins `intra_region_hookup_mm: 0.0`,
`claims: None` and `link_kinematics: None`. Without all four the exhibit would
have been a statement about link geometry rather than about the raster band.

**Two incidental corrections found while building it:**

- `FinishPlannerParams`'s `waterline_threshold_deg` **doc comment says
  "Default 65"** (`crates/rs_cam_core/src/finish_planner.rs:113`) while
  `for_tool` and `UnifiedFinishConfig::default` both ship **75.0**
  (`finish_planner.rs:202`, `operation_configs.rs:1046`). A stale doc on a
  shipped threshold — small, worth its own filing.
- `UnifiedFinishParams` has **no `overlap_mm`**; that dial lives on
  `FinishPlannerParams`. Relevant to D-16.1's fix shape (§2.9).

One fixture subtlety that would have made the exhibit contingent rather than
categorical: `intra_region_hookup_mm` ships at **6.0**, and `relink_fragments`
*does* pass `stock_to_leave` through to `surface_link.rs:49`
(`unified_finish.rs:1940`). So the dial is not fully inert on the shallow band
at default settings — it is inert on **cuts** and live on **links**. The exhibit
therefore pins `intra_region_hookup_mm: 0.0`, so it is a statement about the
raster band and nothing else.

### 3.D H2.6 — the slow agent reads

#### 3.D.1 What the architecture actually is

Every MCP tool builds an `McpRequestKind`, pushes it down an `mpsc` channel with
a `oneshot` reply, and awaits (`crates/rs_cam_viz/src/mcp_server.rs:89-104`).
`RsCamApp::drain_mcp_requests` (`crates/rs_cam_viz/src/app/mcp.rs:42-69`, called
once per repaint from `crates/rs_cam_viz/src/app.rs:509`) `try_recv`s the channel
dry into a `Vec` and then handles **every drained request, in order, in one
frame, on the egui main thread**. There is no per-frame cap, no cost estimate and
no priority. One frame can legitimately contain a long narration plus everything
queued behind it.

**The <1 s guarantee is two mechanisms, and neither is a priority queue:**

1. **A separate door.** `cancel_generation` and `generation_status` never touch
   `request_tx` at all — they are answered synchronously on the caller's tokio
   thread from `GenerationControl` (`mcp_server.rs:1190-1199` →
   `crates/rs_cam_viz/src/compute/mod.rs:130-147`). The
   `McpRequestKind::CancelGeneration` variant was deliberately **deleted** so
   the trap cannot be re-entered
   (`crates/rs_cam_viz/src/mcp_bridge.rs:378-381`). **Only these two tools are
   on this door**, and nothing in the design below touches `GenerationControl`.
2. **A 750 ms race plus a published-snapshot fallback**, for exactly five
   no-argument reads: `cheap_read` (`mcp_server.rs:115-139`,
   `BUSY_READ_DEADLINE = 750 ms` at `:41`), used by `project_summary`,
   `list_toolpaths`, `inspect_model`, `inspect_stock`, `inspect_machine`.

**A hole this work must not step on, and currently inherits.** `cheap_read`
diverts to the snapshot **only when the toolpath compute lane is active**
(`mcp_server.rs:116`). A long narration stalls the frame loop with the lane
**idle** — and in that state all five cheap reads take the *unbounded* path and
block for the full narration.
`crates/rs_cam_viz/tests/mcp_escape_hatches.rs:217-248`
(`an_idle_lane_keeps_the_old_unbounded_read_behaviour`) **pins that behaviour
deliberately**, so the existing sentry file would stay green through exactly the
stall H2.6 is about. The named acceptance gate still holds, because
cancel/status are on the other door.

#### 3.D.2 Where the cost is

| Tool | Handler | Complexity |
|---|---|---|
| `get_toolpath_params` | `app/mcp.rs:970-1053` | **O(1)** — its slowness is *pure queueing* |
| `inspect_spans` | `app/mcp.rs:1881-1924` | O(spans), output bounded |
| `get_toolpath_diagnostics` | `app/mcp.rs:3002-3018` → `session/compute.rs:3648` | **O(project × samples) for a single-toolpath read** — it calls `project_load_report` for the whole project (`:3669`) |
| `get_tool_load_report` | `app/mcp.rs:2953-2994` | O(toolpaths × samples) |
| `get_cut_trace` | `app/mcp.rs:1261-1490` | **accidental quadratic** |
| `narrate_toolpath` | `app/mcp.rs:1066-1176` → `narrate.rs:283` | several linear passes + two structural quadratics |

**The accidental quadratic, both loop sites named.** `build_span_cut_summaries`,
`crates/rs_cam_viz/src/app/mcp.rs:4547-4660`: outer `for (span_index, span) in
spans.iter().enumerate()` at **:4578**, inner `for sample in
trace.samples.iter().filter(...)` at **:4591**. Every span re-scans the entire
project sample vector; with no filter arguments the `continue` guard at
`:4580-4587` is skipped, so a bare `get_cut_trace()` costs
`Σ_toolpaths (spans × total_samples)`. **The fix is mechanical** — one pass over
samples scattering into per-span accumulators, exactly as its sibling
`build_per_depth_pass_summary` already does at `:4709-4731`. This is a
standalone bug fix and should land on its own.

**Narration's own quadratics**, `crates/rs_cam_core/src/narrate.rs`:
`find_or_create_level_accumulator` (`:697-714`) linear-scans all levels so far,
per cutting move, giving `O(moves × levels)` — reached whenever the toolpath has
no valid `DepthPass` spans, which is the normal state for surface-following
finish ops; and `apply_semantic_level_metrics` (`:1183-1228`) falls through to
`fallback_semantic_region_count_at_z` (`:1230-1254`), itself `O(items²)`, once
per level. Plus, in the GUI handler only, `MeasurabilityReport::from_trace`
(`app/mcp.rs:1101-1106`) scans all samples **once per toolpath in the project**
before narration emits a character.

> **A number this programme should stop quoting.** The "~12 min" figure
> (`planning/review_2026-07-29/ORCHESTRATION_LOG.md:1402`, restated at `:1960`,
> and now in the tool description at `mcp_server.rs:407`) is a **single wall-clock
> observation taken during a >40-minute `generate_all` in which everything
> queued**. No profile was ever taken. Sizing the loops above against that run's
> own figures (op 8 = 148,429 moves, semantic trace only **202 items**, depth
> levels **0**) does not reach 720 s. The reading here is that a substantial
> share of it was **queue time behind the in-flight generation**, not narration
> compute, and that the tool description over-attributes. **The structural defect
> stands regardless** — an unbounded super-linear scan on the shared frame-loop
> thread — but the number should not be cited as a measured narration cost. One
> `narrate_toolpath` on a fully idle lane, timed, settles it for the cost of a
> single run.

#### 3.D.3 What `McpReadCache` already covers: **none of it**

`crates/rs_cam_viz/src/mcp_bridge.rs:24-99`. It caches five already-rendered
**JSON strings** — `list_toolpaths`, `project_summary`, `inspect_model`,
`inspect_stock`, `inspect_machine` — plus a `published_at`. It has **no key**
(one global slot per payload kind, so parameterised reads are unrepresentable in
it by construction) and **no invalidation** (it is wholesale republished at
≤2 Hz, `app/mcp.rs:79-98`, and staleness is *reported* via `snapshot_age_s`
rather than prevented). It is populated on the GUI thread at the **end** of
`drain_mcp_requests` (`:68`), so during a long in-band handler it correctly ages
rather than lying.

**Coverage of the slow reads is zero** — not "a cache that misses", a cache that
was never asked to hold these. Its hit path is additionally gated on the lane
being active, so even for its own five payloads it is dead during a
read-induced stall.

#### 3.D.4 The design — immutable snapshots + bounded narration (**proposal only**)

**(1) `McpStateSnapshot`.** Generalise `McpReadCache` from five rendered strings
to one immutable typed bundle, published on the same GUI-thread cadence and read
off the MCP thread with **zero frame-loop occupancy**. It carries per-toolpath
`ToolpathResult` / traces / status / `stale_since`, the cut trace, the
measurability report (computed once per publish rather than once per read), the
small configs, and a monotonic `revision`.

**Memory: the whole point is that it copies nothing.** `Move` is 64 B padded
(`crates/rs_cam_core/src/toolpath.rs:120-124`), so the observed 199,745-move
toolpath is ≈ **12.8 MB**, and a multi-million-sample `SimulationCutTrace`
(`crates/rs_cam_core/src/simulation_cut.rs:163-242`, ≈ 220–260 B/sample) is
**hundreds of MB**. Both are *already* behind `Arc` in GUI state
(`crates/rs_cam_viz/src/state/toolpath/entry.rs:163-175`,
`crates/rs_cam_viz/src/state/simulation.rs:436`), so a publish is
`O(#toolpaths)` pointer clones plus a few small config `Clone`s — low tens of
kilobytes. The only real cost is that the previous generation's buffers stay
alive until the next publish drops them: worst case one extra toolpath result
(~13 MB) and one extra cut trace across a swap. **That must be in the type's
doc, and a publish-cost sentry must guard it** — a stray deep clone of an
`AnnotatedToolpath` would copy 12.8 MB per publish at 2 Hz *on the frame loop*,
which is the one way this design could regress the very guarantee it serves.

**Staleness: reuse, do not invent.** Four vocabularies already exist and cover
every case — `served_from`/`snapshot_age_s`/`generation_in_flight`
(`mcp_server.rs:156-183`); `ComputeStatus` with its
`Pending/Computing/Done/AwaitingPriorStock/Disabled/Error` taxonomy;
`ToolpathRuntime.stale_since` (`crates/rs_cam_viz/src/state/runtime.rs:32`,
already surfaced as `"stale"` at `app/mcp.rs:1051`); and sim-evidence freshness
via `SimEvidenceMeta::resolve` / `sim_trace_is_fresh`
(`crates/rs_cam_core/src/gcode/mod.rs:445-448`). Two rules must survive: a read
whose snapshot predates a *completed* generate must say so rather than pretend
currency, and **never-published must still refuse** — an empty toolpath array
reading as "this project has no toolpaths" is the failure
`mcp_escape_hatches.rs:196-212` exists to prevent.

**(2) Bounded / incremental narration.** Recommendation: **both**, with
precompute doing the work and the bound as a hard floor. Precompute alone leaves
an unbounded scan on any path it did not cover; bounding alone turns the
workflow's documented *first* diagnostic into a paginated crawl.

- *Precompute at the producers.* Everything narration derives from the move list
  is a pure function of the toolpath and is **already walked once** at
  generation time by `stats_with_findings`
  (`crates/rs_cam_core/src/compute/stats.rs:121-190`). Fold the Z-level table
  and the arc observations into that same walk as a `NarrationFacts` struct
  carried on `ToolpathResult`; move the engagement histogram and peak-DOC sample
  into `SimulationCutTrace.toolpath_summaries` at simulate time; move
  `MeasurabilityReport` to publish time. Narration then becomes string
  formatting over precomputed facts — **O(output length)**. This also removes
  the second quadratic outright, because levels join against semantic items once,
  at generation, through a map.
- *The bound.* Measured in **work units visited — moves + samples — not
  wall-clock**, so the same request truncates identically on every run and a
  truncated narration is reproducible evidence rather than a machine-speed
  artefact. A wall-clock valve may sit above it as a last resort, but must
  report `bound: "wall_clock"` so nobody compares two responses cut at different
  points.
- *The wire shape.* Reuse the existing vocabulary verbatim —
  `truncated` / `total_matching` / `returned` already ship on `get_cut_trace`
  (`app/mcp.rs:1467-1479`) and `inspect_spans` — and add
  `complete`, `sections_complete`, `sections_not_computed`, `work_units_budget`,
  `work_units_consumed` and a `continuation_token` pinning the snapshot
  `revision`. **Honouring the standing rule:** a section not reached is listed
  under `sections_not_computed` and its numeric fields are **absent or null,
  never 0 and never silently omitted**; and a complete response asserts
  `"complete": true` positively, so "that's all there is" is never inferred from
  the absence of a flag. A continuation against a newer revision **refuses**,
  naming the mismatch, rather than splicing two toolpaths together.

**Risk table.**

| # | Element | Files | Size | Risk | Wire keys | Can it regress <1 s? |
|---|---|---|---|---|---|---|
| C1 | Invert `build_span_cut_summaries` to one scatter pass | `app/mcp.rs:4547-4660` | **S** | low | none | no — removes frame-loop work |
| C2 | `McpStateSnapshot` + publish site | `mcp_bridge.rs`, `app/mcp.rs:79-98`, `app.rs` | **M** | medium | additive (`revision`) | **yes if publish is slow** — bound it, keep the ≥500 ms limiter, add a publish-cost sentry |
| C3 | Move parameterised reads onto the snapshot | `mcp_server.rs`, `app/mcp.rs` handlers | **M/L** | medium | additive | **improves it** |
| C4 | `NarrationFacts` precomputed at generation | `compute/stats.rs`, `narrate.rs`, `state/toolpath/entry.rs`, worker, `session/compute.rs` | **L** | medium-high — touches the generation hot path; **a golden-text narration sentry is mandatory** | none (prose) | no |
| C5 | Bounded narration + continuation token | `narrate.rs`, `mcp_server.rs` | **M** | low-medium — the risk is a truncated response read as complete | additive only | no |
| C6 | Decouple `cheap_read` from lane-activity (§3.D.1's hole) | `mcp_server.rs:115-139` | **S** | **medium — the one that can break the pinned guarantee** | none | **yes, directly** |

**Is `mcp_escape_hatches.rs` still sufficient?** For the *named* gate — yes: its
cancel/status tests run against a stalled GUI with no GUI thread at all, and
nothing here touches `GenerationControl`. **For the spirit of H2.6 — no**, and
this must be said when the work lands: the file has no test in which the frame
loop is stalled by a **read** with the lane **idle**, and test `:217-248`
currently pins that block as *correct*. Any C6 change must update that sentry
deliberately, in writing. Four tests to add:
`reads_answer_within_the_gate_while_a_narration_holds_the_frame_loop`;
`a_truncated_narration_says_so_and_names_what_it_did_not_measure`;
`a_continuation_token_from_a_stale_revision_is_refused`;
`publishing_the_state_snapshot_costs_no_deep_copy`.

#### 3.D.5 B7 — the two parallel `ToolpathNarrationContext` literals

W1's hand-off, checked. The literals are
`crates/rs_cam_core/src/session/compute.rs:2976-3025` and
`crates/rs_cam_viz/src/app/mcp.rs:1107-1166`. (W1's cited line numbers, `2848`
and `1098`, have drifted by ~128 / ~9 lines.)

- **Both are exhaustive** (no `..Default::default()`) and populate the **same 24
  fields**. **There is no missing-field divergence today.** The hazard is real
  but latent: the struct derives `Default` (`narrate.rs:46`), so either literal
  could be "fixed" with `..Default::default()` and start silently defaulting;
  and a new `ToolpathStats` channel needs three coordinated edits with the
  compiler enforcing only that the *context* be complete, never that it be
  **fed**.
- **W1's "three findings": CONFIRMED** — `deprecated_dial`,
  `derived_stepovers`, `claims_reference` are absent from
  `ToolpathNarrationContext` altogether and are un-narratable by construction.
  (`claims_reference` does reach agents by another route, the `runtime` block of
  `get_toolpath_params`, `app/mcp.rs:1037-1050`.)
- **W1's "plus `retract_trips`": REFUTED on this tree.** It is populated in
  **both** literals (`session/compute.rs:3011`, `app/mcp.rs:1154`) and rendered
  by `append_retract_trips` (`narrate.rs:1146`). Either it was closed after W1
  looked, or it was mis-attributed.
- **Four fields differ in their value expression** — genuine behavioural
  divergence between the two narrations of the same toolpath, and a finding W1
  did not have:

| Field | Core (`session/compute.rs`) | Viz (`app/mcp.rs`) | Consequence |
|---|---|---|---|
| `is_drill_cycle` | op-type only, `:2992` | op-type **OR** a move scan for `MoveIntent::Drilling`, `:1125-1135` | viz can report a drill cycle where core does not (a v-carve with drilled entries), suppressing the air-cut anomaly |
| measurability cell size | `sim.column_grid_cell_mm`, `:2973` | `state.simulation.resolution`, `:1104` | **two different quantities**; the measurability floors are cell-size-dependent, so the paths can return different `NotMeasurable` verdicts on identical evidence |
| tool | errors if the tool id is missing, `:2959-2961` | falls back to `tools().first()`, `:1077-1085` | **viz can narrate with the wrong tool's geometry**, which sets the large-arc threshold (`narrate.rs:1437`) and every tool-scaled hint |
| trace sources | `result.*`, `:3029/:3031` | `rt.*` preferring, falling back to `result.*`, `:1093-1097` | viz may narrate against a newer trace than the result it describes |

**Proposed helper** (sketch, not implemented) — mirror `stats_with_findings`
exactly, no `..`, so a 15th finding is a compile error in **one** place and both
readers inherit the decision:

```rust
// crates/rs_cam_core/src/narrate.rs
impl<'a> ToolpathNarrationContext<'a> {
    /// The ONE join from `ToolpathStats` into narration. Exhaustive by
    /// construction: adding a field to `ToolpathStats` fails to compile here
    /// until someone decides, in writing, whether narration carries it. A
    /// channel narration deliberately does not carry is bound to `_` with a
    /// comment saying why — never elided with `..`.
    pub fn absorb_stats(&mut self, stats: &'a ToolpathStats) {
        let ToolpathStats {
            move_count: _, cutting_distance: _, rapid_distance: _,
            truncated_core_mm2, untouched_material_mm2, reached_uncut_estimate_mm2,
            dropped_band, clipped_band, ramp_reach_clamp, tip_float,
            retract_trips, zero_removal, offset_library_failures,
            boundary_clip_dropped,
            // NOT rendered by narration. Deliberate, and listed so the
            // omission is a decision rather than an oversight:
            //  - deprecated_dial:   surfaced as a load/diagnostic notice
            //  - derived_stepovers: audit trail, not a part measurement
            //  - claims_reference:  surfaced by get_toolpath_params.runtime
            deprecated_dial: _, derived_stepovers: _, claims_reference: _,
        } = stats;
        /* … field-by-field assignment … */
    }
}
```

No wire impact: `ToolpathNarrationContext` derives only `Debug, Clone, Default`
and is never serialised. Existing sentries that build it with
`..Default::default()` (`narration_denominator_and_hints_d7.rs`,
`air_cut_denominators_lh1.rs`) need no change under `absorb_stats`.

**Adjacent finding, filed not fixed.** `ProjectSession.results` is written only
by core's own `generate_toolpath` (`session/compute.rs:1605`) and by tests; the
GUI stores results in `gui.toolpath_rt` instead. So **in GUI mode**
`diagnose_toolpath_with_trace` reads `stats = None`
(`session/compute.rs:3683-3688`) and `project_load_report` sees `spans = None`
for every toolpath (`gcode/mod.rs:468-471`) — the GUI's
`get_toolpath_diagnostics` / `get_tool_load_report` run **without** generation
findings and **without** spans, while the CLI's run with both. That is a second,
larger instance of B7's family. Code-reading result, not measured.

---

## 4. F-7 — the adaptive3d perimeter-sweep sampling contract (research only)

### 4.1 What is at `clearing.rs:1818`

`crates/rs_cam_core/src/adaptive3d/clearing.rs`, inside
`clear_z_level_agent_2d_slice` (declared `:1459`):

```rust
const PERIMETER_INSET_MARGIN_MM: f64 = 0.25;
let inset_polygons = crate::polygon::offset_polygon(
    region_polygon, ctx.tool_radius + PERIMETER_INSET_MARGIN_MM);
```

The "surface sampling" inherited is **the XY positions at which the surface
heightmap is probed**, three lines later: `:1831` clones the exterior, `:1832-1837`
pops the closing duplicate, `:1838` maps every vertex through `lift`
(`:1674-1682` → `surface_hm.z_or_bbox_floor_at_world`), and `:1872` emits the
result verbatim as `Adaptive3dSegment::Cut(path_3d)`. Holes take the identical
path at `:1889-1932`.

**Every vertex `offset_polygon` returns is one heightmap probe and one toolpath
vertex.** The producer chain is `build_material_bool_grid` (`:335`) →
`marching_squares_bool_grid` (`:1514`) → area filter (`:1549`) →
`detect_containment` (`:1559`) → min-area retain (`:1592`) → `offset_polygon`,
which delegates through `offset_polygon_reported` → `offset_one`
(`crates/rs_cam_core/src/polygon.rs:436`) → cavalier `parallel_offset`, whose
output is converted by `Polygon2::from_pline` — **which discards each arc join's
bulge and keeps its two endpoints** (`polygon.rs:803-805`). That is the
"arc-join debris".

The same `region_polygon` is offset three times per region per Z level (`:1724`
forecast, `:1818` sampling, `:1954` smoothing). **Only `:1818` turns vertices
into probes.**

### 4.2 The contract, stated

| Requirement | Status |
|---|---|
| **Exterior CCW** | **Load-bearing, ASSUMED, NOT CHECKED.** `offset_polygon`'s own doc (`polygon.rs:378-380`) states the sign convention holds *for a CCW exterior*. Site `:1818` passes a **positive** distance meaning "inset". On a CW exterior the same call offsets **outward** — putting the sweep outside the effective boundary, the exact condition `:1810-1816` says causes the wanaka 18 mm-DOC failure |
| Closed ring / closing duplicate | Handled explicitly, `:1832-1837`; the comment at `:1824-1830` records that failing to drop it silently made the whole sweep a no-op |
| ≥3 exterior vertices | Checked, `:1823` (holes `:1884`) |
| ≥2 lifted points | Checked, `:1864` / `:1919` |
| **Bounded maximum segment length** | **THIS IS F-7. Not declared, not checked, not bounded.** `FlattenPolicy::with_max_segment` (`polygon.rs:886-919`) exists precisely for this and says so — *"a 50 mm straight run with two endpoints is two samples of a surface, not a straight cut"*. Site `:1818` uses the single-shot primitive, which has no policy parameter |
| Deduplicated vertices | Enforced on *input* (`dedupe_pline`, `polygon.rs:488`), never on output |
| No self-intersection | Repaired inside the primitive (`polygon.rs:419-423`), not required of the caller |
| **Non-empty result** | **ASSUMED, NOT CHECKED** — see §4.3 |
| Finite coordinates | **Not checked anywhere.** `polygon.rs:311-318` states explicitly that `NaN` still reaches cavalier |

**A concrete winding gap, with the line where it opens.** `detect_containment`
normalises winding via `ensure_winding()` (`polygon.rs:1330-1332`) — but
**early-returns at `polygon.rs:1325-1327` when `polygons.len() <= 1`, before that
loop**. And `clearing.rs:1547-1555` filters loops on
`polygon_signed_area(pts).abs()` — absolute value, so a CW loop survives.
**Therefore a Z level whose material forms exactly one marching-squares loop
reaches `:1818` with un-normalised winding.** *Unverified:* what winding
`marching_squares_bool_grid` actually emits — `contour_extract.rs` documents no
contract, and its one "CCW" comment (`:119`) is about cell-corner traversal
order, not loop orientation. The guard is genuinely absent either way.

### 4.3 What the producer guarantees, and what happens on violation

**Spacing varies by orders of magnitude by construction.** Arc joins appear only
where the boundary turns; a long straight run offsets to one segment with two
endpoints, while a corner yields a cluster. At a 90° join the flattened arc's
corner cut is **29% of the offset distance** (`polygon.rs:804-809`). So a
boundary of a 40 mm straight run plus a rounded nose produces a handful of
probes on the run and a cluster on the nose.

Degenerate rings, sub-tolerance duplicate runs on output, and bowtie splits are
all reachable (`polygon.rs:557-566`, `:496-497`, `:385-389`); notably
`offset_polygon_reported` loops over the repaired pieces (`:427-431`), so **one
piece can fail while its siblings succeed** (`polygon.rs:408-413`) — arriving at
`:1818` as a partially-swept region with no signal.

**On an empty result the perimeter sweep is SILENTLY SKIPPED.** The
`for inset in &inset_polygons` loop at `:1822` iterates zero times; control falls
through. **No fallback to the un-offset region, no counter, no log line**, and
`ZLevelPlanMetrics.perimeter_sweep_length_mm` (`:1624-1633`) simply stays at
`0.0`, indistinguishable from "this region had no perimeter". The comment at
`:1799-1802` says what that costs: *"leaving stock that gets cleared at the
deepest pass with a single sample's axial DOC equal to the full uncleared depth
(~18mm on a 25mm stock)"*. **Silently skipping the sweep re-arms exactly the
failure the sweep was added to prevent.** (The sibling forecast offset at `:1724`
fails the same way but is at least counted, via `dropped_short_regions` `:1783`.)

**Sparse sampling — direction matters.** The lifted path goes through
`drape_path_to_leave` (`crates/rs_cam_core/src/adaptive3d/path.rs:1553-1561`,
mirrored by the planner stamp at `clearing.rs:506-516`), which densifies to
`≤ cutter.radius()` and takes `z.max(cl.z + stock_to_leave)` — a **max**. So a
sparse chord that would dip *below* the surface is **raised** (no gouge), but a
chord interpolating *above* a valley is **not lowered** — air-cut and standing
material, with the planner's stamp mirroring the same transform so the
planner/sim parity check cannot see the loss either. `lift` probes via
**nearest-cell rounding, not interpolation** (`crates/rs_cam_core/src/slope.rs:312-332`),
so the heightmap cell size is the natural unit for any sampling bound.

### 4.4 The finding W4's write-up overstates, corrected

`OFFSET_CONSUMER_ROLLOUT.md` §4 (O-1) says this site "has the same exposure" as
scallop's `capability_link_moves_safety` empty-toolpath failure. **It does not.**
Scallop's failure needed a coverage-mask **run-splitter** that *discards* runs
whose endpoints land off the part. Adaptive3d has no such splitter: `lift`
(`clearing.rs:1676-1680`) falls back to `z_level` when the probe misses, so an
off-part vertex yields a point, never a discard. **The "emits nothing at all"
mode is not reachable here.**

### 4.5 Site `:1818` is NOT one of Checkpoint C's opted-in sites — key finding

The sites that call `offset_polygon_reported` are `boundary.rs:76,80,131`,
`pocket.rs:229`, `profile.rs:110`, `trace.rs:73`, `zigzag.rs:90`; the four
adapters calling `record_offset_library_failures`
(`crates/rs_cam_core/src/compute/execute.rs:299`) are `generate_zigzag`,
`generate_trace`, `generate_profile`, `generate_pocket`.

**`crates/rs_cam_core/src/adaptive3d/` contains zero calls to
`offset_polygon_reported`, and `generate_adaptive3d` never calls
`record_offset_library_failures`.** So on an adaptive3d toolpath
`ToolpathStats::offset_library_failures` stays `None` — *honest* under the
not-measured contract, but it means **Checkpoint C's new typed failure channel
is blind to the one site the same checkpoint flagged as the remaining
under-specified contract.** Closing it is a per-site opt-in.

### 4.6 Proposed specification and the red-first fixture

**Producer doc, on `polygon::offset_polygon`:**

> **Guarantees: shape only.** Returned rings are chord-flattened arc joins, so
> vertex spacing is a function of where the *input boundary turned*, not of any
> density the caller asked for: a straight run of any length yields two vertices.
> There is no maximum segment length, no dedupe of the result, and no
> orientation normalisation — the CCW-exterior sign convention is **assumed of
> the input, not enforced**. A caller that treats returned vertices as SAMPLE
> POSITIONS is inheriting an accident; use
> `OffsetRingSet::to_polygons(FlattenPolicy::…with_max_segment(d))` and state
> `d` in world units from your own physics.

**Consumer doc, at `clearing.rs:1817`:** every vertex becomes one heightmap probe
and one toolpath vertex; required are (1) CCW exterior, (2) max XY spacing
≤ `surface_hm.cell_size` — the heightmap is a nearest-cell lookup, so a longer
chord samples terrain it never looks at — and (3) a non-empty result, because
empty means *no perimeter sweep*, which is the un-mitigated deep-DOC condition
and not "nothing to clear".

| Requirement | Proposed mechanism | Why |
|---|---|---|
| CCW at `:1818` | **debug assertion** + an `ensure_winding()` on the single-region path (or lift `detect_containment`'s early-return to normalise before returning) | one call; the assertion documents it |
| Non-empty offset | **report-only finding** — opt adaptive3d into `offset_polygon_reported` + `record_offset_library_failures`, plus a distinct counter for "perimeter sweep skipped, region *n*, Z *z*" | Checkpoint C ruled the shape; the geometric-collapse case is `(empty, None)` and needs its own count, because **here a collapse is not benign** |
| Max segment length | **report-only measurement first** (max and p95 emitted chord vs `surface_hm.cell_size`), *then* a behavioural fix | the standing rule from the v3 campaign: never gate on an aggregate without rendering the surface |
| Non-finite coordinates | **typed refusal at `Polygon2`'s constructor**, not here | `polygon.rs:317-318` already names it as a separate unruled item; per-consumer duplication is the wrong layer |

**The fixture that would fire it** (specified, deliberately **not built** — this
item is research only). A mesh on a 120 × 60 mm footprint carrying **four
disjoint 20 × 20 mm mesas** with long straight walls and a **single 3 mm
filleted corner** each — straight-vs-fillet is the point, giving ~2 probes per
20 mm wall and a cluster on the fillet, a **40× spacing ratio on one ring**.
Between two mesas, an **8 mm-wide ridge cresting at Z = −3, running perpendicular
to a mesa wall so it lies under the middle of a long straight offset run**: the
interpolated chord passes *above* the crest, the drape's `max` never lowers it,
and the ridge is left standing where the sweep claims coverage. That is the
measurable red. Add a fifth island below `min_region_area_mm2`
(`clearing.rs:1585-1590`) so the fixture separates "dropped by the area filter"
(counted) from "swept but under-sampled" (uncounted). Ø6 flat end mill
(`tool_radius = 3.0`). Run a second cell at a Z level where **exactly one** mesa
has material, to exercise the single-loop winding gap. **Assert pointwise, not
on an aggregate**: max consecutive-vertex XY spacing vs `surface_hm.cell_size`,
and a separate probe of the ridge crest for standing material. Neighbours to
model it on: `crates/rs_cam_core/tests/adaptive3d_interior_cell_parity_f029.rs`,
`adaptive3d_subtool_channel_gouge.rs`.

**Not verified:** nothing here was executed. The ridge-under-chord mechanism is
**inference** from `lift`'s interpolation plus `drape_path_to_leave`'s max-only
correction. Marching-squares loop winding is unverified. Whether cavalier ever
returns empty on real adaptive3d region polygons is unverified. **F-7 remains a
code-path finding, exactly as W4 recorded it.**

---

## 5. R7-L1 — MidSteep/Shallow clip observability census (report-only proposal)

### 5.1 The census — 20 reduction sites, 2 reach `ToolpathStats`

`FinishBand` is at `crates/rs_cam_core/src/finish_planner.rs:97-104`, wrapped as
`RegionKind::Band` at `unified_finish.rs:631`.

| # | Reduction site | file:line | Measured? |
|---|---|---|---|
| R1 | Machining-boundary / stock-footprint clip on classification cells | `unified_finish.rs:1325-1343` | **INVISIBLE** |
| R2 | Coverage erosion (1-cell ring **plus the whole grid perimeter**) | `finish_planner.rs:376-393` | **INVISIBLE** |
| R3 | Hysteresis classification + `and_masks_in_place(very, steep)` | `finish_planner.rs:398-419` | partial; `raw_*_islands` `:421-422` **stranded** |
| R4 | Morphological close, re-clamped to coverage | `finish_planner.rs:426-433` | **INVISIBLE** (no gained/lost count) |
| R5 | Min-area absorption (sliver → majority neighbour band) | `finish_planner.rs:450-451`, `:723-810` | `absorbed_regions` `:455` — **stranded** |
| R6 | Crease-corridor relabelling | `finish_planner.rs:455-467` | `claimed_creases`/`claimed_cells` — **stranded** |
| R7 | Sub-cell loop drop in region decomposition | `region_mask.rs:82-88` | **INVISIBLE** |
| R8 | Marching-squares boundary-loop drop | `region_mask.rs:79` | **INVISIBLE** |
| R9 | Sliver cap `MAX_REST_REGIONS = 64`, **also applied to band polygons** | `region_mask.rs:113-135` (const `:27`) | `warn!` with `dropped_area_pct` — **log only** |
| R10 | Territory filter / rest-mask AND | `unified_finish.rs:1557-1624` | `territory_masked_*` on `ClaimsReport` — **stranded**; `debug!` only |
| R11 | `territory_clip` requested but silently refused | `unified_finish.rs:1563-1570` | `warn!` only, **no counter** |
| R12 | `min_rest_depth_mm` skip | `unified_finish.rs:1575` | folded into R10, **not separable** |
| R13 | Over-fragmentation guard (>256 regions) | `unified_finish.rs:1651-1660` | `warn!`; count **stranded** |
| R14 | VerySteep region with no covered cells → `continue` | `unified_finish.rs:1727-1734` | **INVISIBLE** — and the `continue` precedes `stats.region_count += 1` (`:2048`), so the region vanishes from its own band's count |
| **R15** | **VerySteep Z-ladder clip** | `unified_finish.rs:1735-1776`, `BandHeightClip` `:1766-1775` | **MEASURED — the only one.** → `dropped_bands` `:2010` / `clipped_bands` `:2034` |
| **R15b** | **MidSteep equivalent** | arm `:1796-1851` | **INVISIBLE.** `height_clip` (declared `:1723`) is never assigned; the arm never reads `top_z`/`bottom_z` at all |
| **R15c** | **Shallow equivalent** | arm `:1852-1927` | **INVISIBLE.** Uses `effective_min_z = mesh.bbox.min.z - 0.1`; never consults `top_z`/`bottom_z` |
| R16 | Shallow raster off-mesh point demotion | `unified_finish.rs:1890-1911` | **INVISIBLE** |
| R17 | MidSteep ring-cascade cap | `scallop.rs:1411`, measured `:1541-1563` → `unified_finish.rs:1843-1849` | **MEASURED but band-agnostic** — carries no band label, so a MidSteep truncation is indistinguishable from a plain-Scallop one |
| R18 | Empty-toolpath `continue` | `unified_finish.rs:2050-2052` | **INVISIBLE** unless `height_clip` was set — i.e. VerySteep only |
| R19 | Intra-region relink discards | `unified_finish.rs:1936-1969` | counted on `UnifiedFinishReport` — **stranded** |
| R20 | Diagnostic area floor `STANDING_MATERIAL_FLOOR_MM2 = 1.0` | `diagnostics/adapters/from_generation.rs:30`, `:552`, `:609` | **measured then suppressed, with no counter for the suppression** |

Emit path for the two live channels: `unified_finish.rs:1997` → `:2010`/`:2034`;
folds `clipped_band_finding()` `:998-1024` and `dropped_band_finding()` `:1035-1051`;
transport `compute/execute.rs:1948-1956` → `record_dropped_band` `:209-215` /
`record_clipped_band` `:220-226` → `compute/stats.rs:155,156,179,183`.
**Neither is ever set to `Some(0.0)`** — both are strictly two-valued, and the
measured-clean distinction is reconstructed downstream from the operation kind
(`narrate.rs:857`, `:1046`).

**Score: 2 of 20 reach `ToolpathStats`.** Six more are counted into
`DecomposeStats` / `ClaimsReport` / `UnifiedFinishReport` and die there — only
`crates/rs_cam_core/tests/finish_planner_wanaka_decompose.rs:162,166,440,441`
ever reads them. Twelve are counted nowhere.

### 5.2 What "VerySteep clip limitation" means — and the correction it forces

Origin, `planning/review_2026-07-29/ORCHESTRATION_LOG.md:2655-2669`:

> *"**Partial height clipping.** Wave D1 already MEASURED the clip on every band
> and threw it away unless the region emitted nothing… Measured on
> `two_groove_plateau` with `bottom_z = −4.0`: **185.98 mm² of the VerySteep band
> laddered 5 of its 9 levels and stopped, leaving 4.311 mm of an 8.31 mm groove
> wall unfinished**… *Not fixed, stated*: only the `VerySteep` arm measures a
> clip at all — MidSteep and Shallow never set one, so a partial clip there is
> still invisible."*

Restated in source at `crates/rs_cam_core/src/compute/config.rs:737-742`.

**Why VerySteep's is visible:** it is a waterline **ladder**, so it computes
`planned_levels` from its own surface span and `resolved_levels` after the
`top_z`/`bottom_z` clamps (`unified_finish.rs:1747-1749`) — the clip is a
subtraction of two integers. MidSteep hands `ScallopParams` and Shallow hands a
drop-cutter grid, and **neither takes `start_z`/`final_z`, and neither reads
`top_z` or `bottom_z` at all.**

**The correction the code forces, and it changes what F-adjacent work should
build.** The log's phrase *"Wave D1 already MEASURED the clip on every band"* is
**not true of the shipped code**, and the plan's framing needs one refinement to
be actionable: MidSteep and Shallow are **not clipped by the height dials in the
first place**, so a like-for-like `BandHeightClip` widened to those two arms
would report a structural zero. What is genuinely invisible on them is a
*different clip class* — R1, R2, R4, R7–R14, R16, R18, plus R17's
band-unlabelled truncation. **An instrument that only widens `BandHeightClip`
will report clean and will be wrong**, for precisely the reason the v3 campaign
already recorded: *a warning nobody sees is not a warning.*

### 5.3 Proposal — one report-only, per-band, area-domain slot

**Proposal only. Nothing was written; none of it is validated against a build.**

One new slot, `ToolpathStats` field **15** (after `boundary_clip_dropped`,
`compute/config.rs:438`), boxed for the reason `dropped_band`'s own doc gives at
`:250-259`:

```
/// Per-band planned-vs-delivered coverage. NOT a height clip — that is
/// `clipped_band`, which exists on VerySteep only because only VerySteep has
/// a Z ladder to shorten.
///
/// `None` = NOT MEASURED. A band that planned area and lost none reports
/// `Some` with `lost_mm2 == 0.0` — measured and clean. Never coerce absent
/// to zero.
pub band_coverage: Option<Box<BandCoverageFinding>>,
```

carrying per-band `planned_mm2` / `delivered_mm2` / `lost_mm2` and a
`Vec<BandLossCause>` attributing the loss — `BoundaryClip` (R1),
`CoverageErosion` (R2), `TerritoryMask` (R10+R12, carrying `min_rest_depth_mm`),
`SliverAbsorbed` (R5), `RegionCapTruncation` (R9, with kept/total),
`DegenerateLoop` (R7+R8), `NoCoveredCells` (R14), `EmptyToolpath` (R18),
`HeightLadder` (R15, cross-referencing `clipped_band` rather than duplicating
it), `RingCascadeExhausted` (R17, **now band-labelled**).

Emit sites that must populate it are exactly the census rows above; several
already compute the number and only format it into a `warn!` (R9 at
`region_mask.rs:120-124`) or strand it on an intermediate report (R5, R10, R13,
R19). The transport join is `compute/stats.rs:145/163/187`, whose exhaustive
destructure makes a new field a compile error — the good kind.

**Deliberately excluded: any gate, verdict or refusal.** The plan's row says
"instrument census; do not claim all bands covered", and both existing siblings
are documented report-only (`config.rs:733`).

**Surfacing.** (1) A diagnostics adapter beside `clipped_band`
(`diagnostics/adapters/from_generation.rs:546`) and `unmachined_band` (`:599`),
registered at `:47-48` — **do not copy their `STANDING_MATERIAL_FLOOR_MM2`
suppression without a counter for what it swallowed**; that suppression is
itself an unmeasured clip (R20). (2) An `append_band_coverage` beside
`append_clipped_band` (`narrate.rs:1045-1085`), called from `:358-363`, with the
same three-branch `Some` / `None if plans_bands` / `None` shape so "not
measured" and "measured clean" read differently. **§3.D.5's B7 gap applies and
must be closed in the same commit**, or the new field is silently un-narrated
from birth.

---

## 6. The operator decision lists

Nothing below is decided. Each row is a question this wave gathered evidence
for and deliberately did not answer, because answering it moves emitted
geometry or an agent-facing default.

### 6.1 Checkpoint F2 — D-16.1 band run-off

| # | Question | Evidence | Wave's reading (not a decision) |
|---|---|---|---|
| **F2-Q1** | **Repair the guard, the input, both, or neither?** Options (i) coverage clamp on band polygons, (ii)/(ii′) exact or unrounded coverage guard in `ring_to_3d`, (iii) tool-radius erosion, (iv) report-only. | §2.9 | **(ii′) then (i) narrowed.** (ii′) is a one-line fix to a guard that is already the right idea and merely mis-quantized, and the Shallow band next door already does the exact version. **Refuse (iii)** — it trades an overcut for a standing-material collar at every part edge, which is worse. |
| **F2-Q2** | **Is a repo-wide scallop geometry move acceptable?** (ii)/(ii′) touch **every** scallop op, including standalone Scallop and the all-over control arm; (i) touches every `UnifiedFinish`. | §2.9 | This is why the filing said "a wave, not a footnote". A re-pin package must be scoped **before** the change, not after. |
| **F2-Q3** | **Must `overlap_mm`'s meaning be restated?** It currently means "dilate the band mask outward by N mm", with no distinction between *overlapping the neighbouring band* (its purpose) and *leaving the part* (this defect). | §2.2, §2.9(i) | Any clamp must be to **coverage**, never to the band — clamping to the band destroys the dial's purpose. Worth stating in its doc either way. |
| **F2-Q4** | **Is Tier-0 evidence sufficient to rule, or is a Tier-1 COLUMNS run required first?** | §2.8 | The mechanism is reproduced without a simulator. **No collision / residual / runtime numbers exist** — measuring them needs both arms simulated at 0.1 mm. If the ruling is "fix it", that table should be produced as the fix's own before/after, not as a precondition. If the ruling is "report only", it must be produced first, because §2.9(iv) ships a known overcut. |
| **F2-Q5** | **Does the ~30 µm unexplained remainder need closing before a ruling?** 169 µm (rim-riding CL) + 34 µm (arm-D chord residual) ≈ 203 vs the reported 235. | §2.6 | No — the mechanism is established and the remainder is a magnitude question, not an existence question. But it should be **stated in the fix's commit**, not quietly dropped. |

### 6.2 Checkpoint F3 — D-16.2 Shallow `stock_to_leave`, and the agent reads

| # | Question | Evidence | Wave's reading (not a decision) |
|---|---|---|---|
| **F3-Q1** | **Fix Shallow alone, or Shallow + VerySteep together?** | §3.B, §3.C.1 | **Together.** Two of three bands ignore the dial. Fixing Shallow alone *creates* a `stock_to_leave`-sized step at the shallow↔waterline seam that does not exist today. This is the one genuine scope decision here. |
| **F3-Q2** | **Confirm the vertical convention.** Apply `+Z` on the drop-cutter contact point, matching all four honouring paths and `steep_shallow.rs:468` — or take the opportunity to move to a normal offset? | §3.A | **`+Z`.** A normal offset on the raster band alone would be the thing that creates a seam. The convention's own approximation error (`cos θ`) is repo-wide and **must not be coupled to this**. |
| **F3-Q3** | **Does the "it re-pins fingerprints" deferral still hold?** | §3.C | **Apparently not.** No committed project, fixture or test sets a non-zero `stock_to_leave` on a UnifiedFinish, so at `0.0` the fix is byte-identical. Stated as an `rg` census, not a green run — worth one confirming run before relying on it. |
| **F3-Q4** | **Fix the code, document the limitation, or both?** The honest caveat exists at `unified_finish.rs:142-143` and is invisible on all three operator surfaces. | §3.B | Propagating the caveat to the config field, the GUI dial and the MCP `ParamDef` is a **zero-geometry change that removes the "dial reports no error and does nothing" class immediately**, and is worth doing regardless of the code ruling. |
| **F3-Q5** | **Approve the read-path design?** Immutable `McpStateSnapshot` + precomputed `NarrationFacts` + a work-unit bound with a continuation token. | §3.D.4 | Approve C1 (the `get_cut_trace` quadratic) **immediately and separately** — it is a standalone bug fix with no wire change. C2–C5 as one package. |
| **F3-Q6** | **C6 — decouple `cheap_read` from lane-activity?** It is the one element that can break the pinned <1 s guarantee, and `mcp_escape_hatches.rs:217-248` currently pins the *current* blocking behaviour as correct. | §3.D.1, §3.D.4 | Needed for the spirit of H2.6, but it **requires deliberately rewriting an existing sentry**, in writing. Should be its own ruling, not folded into C2. |
| **F3-Q7** | **Stop quoting the "~12 min" narration figure?** | §3.D.2 | It is a single wall-clock reading taken during a >40-minute `generate_all` in which everything queued; no profile was ever taken, and the loop sizes do not reach 720 s. **The structural defect stands regardless.** One timed `narrate_toolpath` on an idle lane settles it. |
| **F3-Q8** | **Approve `ToolpathNarrationContext::absorb_stats`, and rule on the four value-expression divergences?** | §3.D.5 | The helper is uncontroversial. The **four divergences are not** — especially viz falling back to `tools().first()`, which can narrate a toolpath with the wrong tool's geometry. That one looks like a defect in its own right. |

### 6.3 Not requesting a checkpoint

- **§1 (D-LV.1 / X-1)** is shipped: the plan records it as low-risk,
  exporter-only, merge-order 20, no checkpoint.
- **§4 (F-7)** and **§5 (R7-L1)** are research deliverables. Neither proposes a
  change this programme should make; both name an owner and a re-open
  condition.

---

## 7. D-16.1's residual, located — the rim/wall break, not the groove end

Date: 2026-08-06 · Wave: io-fixes · Research only, **no finishing geometry
was changed**.

### 7.0 Why this section exists

F23-impl closed D-16.1's **mechanism** claim and explicitly left its
**quality** claim open. With zero cut targets outside the part footprint,
arm B still overcut by **−201.6 µm**, and W8's superposition (169 µm
rim-riding + 34 µm chord refinement + ~30 µm unaccounted) did not survive
measurement — removing the 169 µm term moved the total by 33 µm, not 169.
Its `NOT FIXED, STATED` entry named the re-open condition word for word:

> a column-index probe on the grooved fixture that locates the
> worst-overcut column and attributes it — the harness already carries
> `ColumnDeviation`'s row/col, so this is instrumentation, not a campaign.

That probe is now committed as
`d16_1_residual_locating_probe` in
`crates/rs_cam_core/tests/strategy_comparison_h4.rs` (`#[ignore]`d;
research only — it asserts non-vacuity and reports, it gates nothing).

```text
cargo test -p rs_cam_core --test strategy_comparison_h4 --release \
    -- --ignored --nocapture --test-threads=1 d16_1_residual
```

Fixture / population / resolution: `grooved_block(2.5, 70°, 1.2)`, the
project's own Ø1-tip / 7° / Ø6-shaft taper (envelope r 3.0, cusp r 0.5),
both arms simulated at **0.1 mm**, **96,641 columns common to both** (B
96,641, D 96,641 — full overlap). Arm B is the shipped `UnifiedFinish`
band mix; arm D is all-over `Scallop` at the same cusp target. Both arms'
`(row, col)` grids are asserted to address the same world XY before any
cross-arm number is read.

Exhibits, rendered and read **before** this section was written:
`planning/review_2026-08-04/artifacts/io_probe/d161_probe_b_dev.png`,
`d161_probe_d_dev.png`, `d161_probe_b_minus_d.png`,
`d161_probe_b_overcut_mask.png`.

### 7.1 Fact — where the residual is

| zone | columns | B worst µm | D worst µm | B p50 \|µm\| | D p50 \|µm\| |
|---|---:|---:|---:|---:|---:|
| rim (flat, \|x\| ≥ 2.5) | 84,832 | **−201.6** | −34.1 | **0.00** | **0.00** |
| wall (70°, 2.0632 < \|x\| < 2.5) | 1,928 | −54.6 | −15.9 | 36.77 | 27.13 |
| floor (flat, \|x\| ≤ 2.0632) | 9,881 | −0.0 | −0.0 | 5.63 | 7.26 |

The whole-arm worst overcut lives on the **rim** — the flat, 0°,
shallow-raster territory — whose **median absolute deviation is 0.00 µm
on both arms**. A band that is otherwise exact carries the worst column
in the run. That is the signature of a corner artifact, not of a band-wide
error.

Localised: **477 of 96,641 columns (0.4936%)** are overcut worse than
50 µm on arm B.

- **100.0% of them sit within 0.25 mm of a profile break.** The
  0.25–0.5, 0.5–1.0 and ≥ 1.0 mm bins are all **zero**.
- **99.8% (476/477) are on the rim side** of the rim/wall break; exactly
  one is on the wall.
- Overcut-set extent: `x ∈ [−2.500, 2.600]`, `y ∈ [−11.800, 11.800]`.
  x is pinned to the two break lines at ±2.5 (plus one 0.1 mm sim cell);
  y spans essentially the **entire** 24 mm groove.

### 7.2 Fact — the "one longitudinal end" reading is refuted

F23-impl located the residual from a render as sitting "at one
longitudinal end of the groove — the same place the run-off was". The
column index says otherwise: only **32 of 477 (6.7%)** sit within 1 mm of
a longitudinal end. The residual is a **line along the rim/wall break
running the full length of the groove**, with its brightest cell near
`y = −7.6`. The overcut mask render shows exactly that — a one-cell-wide
stripe at a single x, not an end patch.

This is a correction to a stated F23-impl uncertainty, which had flagged
that the location was "from the render, not from a column-index probe".
It was the right thing to flag.

### 7.3 Fact — the residual is B-specific, but the corner is not

**0 of B's 477 overcut columns (0.0%)** are also overcut past the 50 µm
gate on arm D. At those same cells D reads −17 to −26 µm.

But arm D is **not** clean at the break: its own worst column is −34.1 µm
and the D render shows the same thin orange line at the same `x = ±2.5`.
So the rim/wall break is hard for both strategies, and arm B is roughly
**6× worse** there than the all-over scallop control.

### 7.4 Fact — which band owns it

Every one of the twelve worst columns has `Shallow/raster` as the Region
owning its nearest cutting move, at a nearest-cut distance of
**0.000–0.100 mm** — the raster pass is cutting on the break line itself,
not near it. The worst column is `x = 2.500, y = −7.600`, `row 64,
col 245`, B −201.6 µm against D −17.3 µm, nearest cutting move index
4841.

**This is the finding that matters for the ruling.** F2 sharpened the
coverage guard in `ring_to_3d` — the **scallop ring** path, which serves
the **mid-steep** band. The residual is owned by the **Shallow raster**
band. The fix and the defect are in different bands. That is a sufficient
explanation for why removing the 169 µm rim-riding term moved the total
by 33 µm: F2 corrected a real defect, in a band that was not carrying the
worst column.

### 7.5 Interpretation, and the limit of what this probe can say

The probe **locates** and **attributes to a band**. It does not prove a
mechanism, and this section will not pretend otherwise.

The hypothesis with an address — stated as a hypothesis: the rim→wall
break at `|x| = 2.5` is a **convex** break descending into the groove.
A raster pass advancing across the flat rim at 0.3 mm stepover with a
0.5 mm-radius tip has its last on-rim contact point where the tool is
simultaneously tangent to the rim plane and to the 70° wall; a
drop-cutter contact evaluated on the rim triangle alone puts the tip
below the rim plane there. Every quantity in that sentence is the right
order of magnitude for 200 µm, and the 0.25 mm bin holding 100% of the
population is consistent with a cusp-radius-scaled corner effect. **None
of it is measured.** Confirming it needs a contact-point probe at a
single named column, not another aggregate.

### 7.6 Owner and re-open condition

**Owner:** the next finishing wave.

**Re-open condition (specific, and cheaper than what was just done):**
take the single column `row 64, col 245` (`x = 2.500, y = −7.600`) and
move 4841 on the grooved fixture, and dump the drop-cutter contact
evaluation for that move — which triangles were sampled, which one won,
and what the tool's lowest point was. One column, one move, no
simulation. If the winning triangle is the rim rather than the wall, the
mechanism above is confirmed and the fix is a band-local one in the
Shallow raster path; if it is not, this hypothesis is dead and the
`Shallow/raster` attribution in §7.4 still stands and still narrows the
search.

**What is now settled and should not be re-scouted:** the residual is not
the off-footprint run-off (F2 closed that, and it is 0 by measurement);
it is not a longitudinal-end effect (§7.2); it is not in the mid-steep
scallop band (§7.4); and it is not a universal geometric floor, because
the same corner costs arm D 34 µm and arm B 202 µm (§7.3).

**Not addressed here:** the `worst leftover` growth (222.9 → 339.7 µm)
that F23-impl also left open. The per-zone table above is consistent with
its "edge collar" attribution — the wall band carries B p50 36.77 µm
against D 27.13 µm — but that is one aggregate agreeing with a guess, not
a measurement, and it is left open with the same owner.
