# `offset_polygon` consumer rollout matrix — R2 / H3 research question 4

Date: 2026-08-04
Wave: W4 (lane C — 2D geometry)
Revision surveyed: `0e7d38b` (branch `experiment/adaptive-spiral`)
Status: **RESEARCH ONLY.** No consumer is migrated by this wave. Every row's
disposition is a *proposal* for Checkpoint C.

---

## 0. The question this answers

Checkpoint D (2026-08-03, commit `e3427f8`) adopted `polygon::OffsetRingSet`
— an offset cascade that keeps cavalier's arcs as the cascade STATE and
flattens exactly once, under an explicit `polygon::FlattenPolicy`. Two
consumers migrated. The ruling deliberately stopped there:

> A consumer that offsets **once** should keep calling `offset_polygon`: it
> inherits a single arc-join chord, which is a different and much smaller
> defect, and its topology stays exactly where it is.
> — `crates/rs_cam_core/src/polygon.rs:948-951`

R2 asks which of the remaining callers can follow, and which cannot, **with
the reason stated per consumer** rather than as a blanket policy. The rule
that decides it was learned the hard way in wave 14 and is written into the
primitive's own docs:

> Losslessness is a property of a MEASURE, and "the shape" is not the only
> measure a polygon is carrying.
> — `polygon.rs:727`

A ring's vertices can be carrying **shape** (used for containment, clipping,
masking, area) or **sampling** (each vertex is a probe position, a
drop-cutter query, a coverage-mask lookup, or a cut point). A cleanup that is
lossless in the shape domain destroys the sampling domain. Dropped into the
shared wrapper, `remove_redundant` turned a four-island scallop pass into an
**empty toolpath** (`capability_link_moves_safety`).

---

## 1. The primitives, as they stand

| Symbol | `polygon.rs` | Visibility | What it is |
|---|---|---|---|
| `offset_polygon(&Polygon2, f64) -> Vec<Polygon2>` | :262 | pub | The single-shot primitive. Repairs self-intersection (R1.5), then per piece. |
| `offset_one` | :276 | private | The `catch_unwind` chokepoint. A cavalier panic becomes an empty `Vec` — see `CAVALIER_SHAPE_FAILURE.md`. |
| `offset_polygon_inner` | :376 | private | Picks `Polyline::parallel_offset` (no holes) vs `Shape::parallel_offset` (holes). |
| `cleaned_flat_ring` | :357 | private | cavalier's `remove_redundant` on a chord-flattened ring. **Cascade-only, by ruling.** |
| `FlattenPolicy` | :651 | pub | `max_deviation_mm` + optional `max_segment_mm`. The deviation half is fidelity; the segment half is *sampling*. |
| `RingGroup` | :761 | private | One arc-carrying boundary plus its arc-carrying holes. |
| `OffsetRingSet` | :958 | pub | The cascade. `from_polygon` / `from_polygons` / `offset` / `offset_per_group` / `to_polygons(FlattenPolicy)`. |

There is **no type named `ArcCascade` in `polygon.rs`.** `ArcCascade` is a
variant of `scallop::RingCleanup` (`scallop.rs:620`) that selects the
`OffsetRingSet` path. Naming them apart matters because the rollout question
is about the polygon primitive, not about scallop's dial.

`FlattenPolicy` constants: `CHORD_TOLERANCE_SHARE = 0.10`,
`MIN_DEVIATION_MM = 0.001`, `MAX_DEVIATION_MM = 0.050`,
`UNTOLERANCED_MM = 0.010`.

---

## 2. The matrix

Columns:

- **cascade?** — does this call site feed `offset_polygon`'s own output back
  into `offset_polygon`? Only a cascade can suffer the vertex doubling M5
  measured (100–500× on concave input); a one-shot pays one arc-join chord.
- **carries sampling?** — do the returned vertices become probe/cut positions
  whose *spacing* is load-bearing, with the proving line cited?
- **disposition** — `MIGRATE`, `MIGRATE-WITH-SAMPLING-BOUND`,
  `NO-OP (one-shot geometry)`, or `INCOMPATIBLE` + reason.

### 2.1 Already migrated (Checkpoint D, `e3427f8`)

| # | Site | Fn | Cascade? | Carries sampling? | Policy in use |
|---|---|---|---|---|---|
| 1 | `pocket.rs:142-166` | `pocket_contours_with_cancel` | **YES, unbounded depth** (`loop { rings = rings.offset(stepover) }`) | Vertices ARE cut points (`pocket.rs:154` → `:215` `P3::new(p.x, p.y, cut_depth)`), but Z is constant, so no *surface* sampling | `FlattenPolicy::untoleranced()`, **no** `with_max_segment` |
| 2 | `scallop.rs:1394,1502-1509` | `scallop_toolpath_research` | **YES, `max_rings`** | **Yes — the canonical case.** `ring_to_3d` (`:1533`) → `point_drop_cutter` (`:909`) + `heightmap_covered_at_world` (`:911`) per vertex | `from_chord_tolerance(tol).with_max_segment(flat-ground stepover)` |

`scallop.rs:1499`'s flattened `offset_polygon` arm survives as a **research
arm only** — unreachable in production because `ScallopStepoverPolicy::SHIPPED`
pins `cleanup: RingCleanup::ArcCascade` (`scallop.rs:681`).

### 2.2 One-shot, vertices emitted as cut points — `INCOMPATIBLE` with a blanket cleanup

These are the rows that make "roll `remove_redundant` out everywhere"
unsafe. None of them is a cascade, so none of them has the defect
`OffsetRingSet` fixes; all of them would be *harmed* by the cleanup that
comes with it.

| # | Site | Fn | Carries sampling? — proof | Disposition |
|---|---|---|---|---|
| 3 | `profile.rs:78` | `profile_contour` | `.map(\|p\| p.exterior)` (`:85`) returned verbatim → `contour_to_toolpath` emits every point at `cut_depth` | `NO-OP (one-shot geometry)`. Constant Z; a chord flatten is a fidelity question, not a sampling one. Candidate for `FlattenPolicy`-parameterised single offset **if** the corner-cut defect is judged worth a signature change. |
| 4 | `trace.rs:54` (Left) | `trace_polygon_at_z` | `trace_ring` (`:71`) → `:127` `ring.iter().map(\|p\| P3::new(p.x, p.y, cut_z))`, no resample | Same as 3. |
| 5 | `trace.rs:61` (Right) | `trace_polygon_at_z` | Same | Same as 3. |
| 6 | `adaptive3d/clearing.rs:1818` | `emit_region_at_level` perimeter sweep | **`let path_3d = path_2d.iter().map(\|&p\| lift(p))` (`:1838`), and `lift` is `:1674-1675` `surface_hm.z_or_bbox_floor_at_world(p.x, p.y)`.** Every offset vertex is a heightmap probe, then emitted as `Adaptive3dSegment::Cut(path_3d)` (`:1872`) | **`INCOMPATIBLE` with any decimation; `MIGRATE-WITH-SAMPLING-BOUND` if it is ever touched.** This is the same trap scallop hit, in a consumer that was NOT migrated. It inherits its sampling density from cavalier's arc-join debris. See §4, finding O-1. |
| 7 | `adaptive/path.rs:1238` | `contour_parallel_segments` | `walk_contour_clearing` (`:1273`) → `AdaptiveSegment::Cut(path)` (`:1302`) | `NO-OP`. **Repeated but not compounding**: `for k in 0..MAX_LOOPS` (`:1229`) always re-offsets the ORIGINAL `machinable` at `dist = stepover * OFFSET_OVERLAP * k` (`:1234`). Offset depth is always 1; no vertex doubling is possible. |

### 2.3 One-shot, pure geometry — `NO-OP`, safe under any flatten policy

Result is used for containment, clipping, masking, a largest-by-area pick, or
a length statistic. Vertex spacing carries nothing.

| # | Site | Fn | Proof it is geometry-only |
|---|---|---|---|
| 8 | `boundary.rs:39` | `effective_boundary` (Inside) | feeds `clip_toolpath_to_boundary*` → `contains_point` only |
| 9 | `boundary.rs:41` | `effective_boundary` (Outside) | same |
| 10 | `region_set.rs:94` | `RegionSet::processed` | `largest_by_area(&offset_polys)?.clone()` (`:95`), used as a containment set |
| 11 | `session/compute.rs:1728` | boundary-polygon resolution | `largest_by_area` → clip boundary |
| 12 | `zigzag.rs:65` | `zigzag_lines` | inset ring decomposed into `all_edges` (`:71-84`) for scan-line intersection; only the *intersections* become cut endpoints |
| 13 | `rest.rs:74` | `rest_machining_toolpath` | pure boundary test `point_in_any_polygon` (`rest.rs:126`); cut points come from its own `sample_step` walk (`:105,:120-124`) |
| 14 | `inlay.rs:93` | `female_pocket_toolpath_with_cancel` | `inset_poly` handed to `pocket_toolpath_with_cancel` (`:108`) as a *seed region*; pocket re-offsets it |
| 15 | `adaptive/path.rs:132` | `adaptive_segments_with_debug` | `MaterialGrid::build_machinable_mask` (`:149`) — rasterised to a bool mask |
| 16 | `adaptive/path.rs:858` | `apply_residue_mop_cleanup` | mask only (`:863`) |
| 17 | `adaptive/path.rs:944` | `apply_hybrid_cleanup` | mask (`:950`); ring passed as *seed* to row 7 |
| 18 | `adaptive/path.rs:1192` | `is_narrow_machinable` | area predicate: `result.iter().map(\|p\| p.area())` (`:1198`) |
| 19 | `adaptive3d/clearing.rs:1724` | region cut-length forecast | `forecast_mm += polyline_xy_length(&path_2d)` (`:1738`); statistic, discarded |
| 20 | `adaptive3d/clearing.rs:1954` | marching-squares smoothing inset | immediately RDP'd (`simplify_path`, `:1959`) |
| 21 | `viz .../execute/mod.rs:169` | boundary-poly prep | `max_by(area)` → clip boundary |
| 22 | `viz .../execute/mod.rs:680` | boundary clip, re-clip path | `max_by(area)` → `effective_boundary` → `clip_annotated_to_boundary_set` (`:757`) |

Rows 8–22 are also where the **silent no-op** failure mode lives; see §4,
finding O-2.

### 2.4 Non-production

`adaptive/mod.rs:308,658,699,756,791,1143,1175,1242,1282,1766,2046,2116,2254`
(all inside `#[cfg(test)]`, module starts `:292`); `benches/perf_suite.rs:295-302`
(cascade bench) and `:317,:322,:327` (single-shot benches); the test files
listed in `crates/rs_cam_core/tests/` that drive the primitive directly.

---

## 3. Summary by disposition

| Disposition | Count | Rows |
|---|---|---|
| Already migrated to `OffsetRingSet` | 2 | 1, 2 |
| `NO-OP` — one-shot geometry, safe under any policy | 15 | 8–22 |
| `NO-OP` — one-shot, constant-Z emission (flatten is fidelity, not sampling) | 4 | 3, 4, 5, 7 |
| **`MIGRATE-WITH-SAMPLING-BOUND` if touched at all** | 1 | 6 (`adaptive3d/clearing.rs:1818`) |
| `INCOMPATIBLE` with a blanket `remove_redundant` | 5 | 3, 4, 5, 6, and (already ruled) scallop |

**There is no remaining cascade to migrate.** After `e3427f8`, `pocket` and
`scallop` are the only two sites that fed offset output back into the offset
primitive; row 7 looks like a cascade and is not, and every other production
caller offsets exactly once. So the answer to research question 4 is:

> The rollout is **already complete in the sense that matters** — every
> compounding consumer is migrated. What is left is not a migration backlog;
> it is one under-specified sampling contract (row 6) and a shared failure
> contract (§4).

---

## 4. Findings this census produced

### O-1 — `adaptive3d/clearing.rs:1818` inherits its surface sampling from arc-join debris

**What.** The perimeter sweep offsets a region boundary once, then lifts
*every returned vertex* onto the surface heightmap (`clearing.rs:1675`) and
emits the lifted chain as a cutting segment. The XY spacing of those probes
is whatever cavalier's offset happened to leave behind — dense where the
boundary curved and the offset arc-joined, arbitrarily sparse across a long
straight run.

**Why it matters.** This is structurally the same defect
`FlattenPolicy::with_max_segment` was created for. Scallop's version of it
emitted **nothing at all** on a four-island mesh, because a 50 mm straight
run's two endpoints both landed off the part and the run-splitter discarded
the whole run (`polygon.rs:713-720`). Row 6 has the same exposure and no
declared sampling density.

**Not reproduced.** This wave did not build a disjoint-island adaptive3d
fixture to fire it. It is a structural finding from the code, and it is
recorded as such. Severity is bounded by the fact that adaptive3d regions
come from marching squares on a classification grid, which is already
dense — but "already dense by accident" is exactly what scallop's was.

**Disposition:** report to the W8/adaptive3d lane. Not a 2D-lane fix.

### O-2 — the empty-`Vec` convention is *not* uniformly under-cut

`polygon.rs:295-300` states the safety argument for mapping a cavalier panic
to an empty result:

> Every caller already handles empty as "polygon collapsed" — pocket rings
> end, the adaptive machinability probe reports not-machinable — all under-cut
> directions, never a gouge.

That is true of rows 1–7 and 12–20. It is **false** of rows 11, 21 and 22.
All three have the shape:

```rust
let offset_polys = offset_polygon(p, -req.boundary.offset);
if let Some(largest) = offset_polys.into_iter().max_by(area) {
    *p = largest;                 // on empty: `p` keeps its UN-OFFSET value
}
```

An empty result there does not collapse the boundary; it **silently skips the
requested offset**, leaving the un-offset polygon in place. For a *negative*
`boundary.offset` — the operator shrinking the machining boundary inwards,
which is the protective direction — skipping it leaves the toolpath clipped
to a larger region than the operator asked for. That is an over-cut, and it
is the one direction the containment's safety argument says cannot happen.

**Reachability: high.** `BoundaryConfig::offset` is a GUI dial, and the two
viz sites are on the live worker's boundary path.

**Disposition:** Checkpoint C decision D-3 (see `CAVALIER_SHAPE_FAILURE.md`
§6). The minimal repair is local and does not touch the primitive: make the
three sites distinguish "offset produced nothing" from "offset produced
something" and surface the former, instead of falling through to the
un-offset polygon.

### O-4 — an empty offset at the boundary layer removes the boundary clip entirely

Rows 8 and 9 (`boundary::effective_boundary`) are the highest-severity
consumers in this table, and the reason is not visible at the call site. Their
empty result is consumed by `clip_annotated_to_boundary_set`, whose documented
contract (`boundary.rs:79-83`) is:

> An EMPTY `boundaries` slice … is not an error and not a clip: **the toolpath
> passes through** with an identity mapping.

So a `ToolContainment::Inside` request whose offset comes back empty does not
produce a collapsed boundary — it produces **no containment at all**. Both
consumers implement it explicitly: `session/compute.rs:1793-1804` and, on the
live GUI worker, `viz .../execute/mod.rs:696-712`.

The captured cavalier panic asset is a boundary-shaped polygon (86-vertex
exterior, 13 holes, inward 5.53 mm), so this is not a hypothetical pairing.

Full write-up, severity argument and proposed contracts:
`CAVALIER_SHAPE_FAILURE.md` §5.3 and decision **D-3a**. Ranked first in
`ADVERSARIAL_2D_FINDINGS.md`.

### O-5 — the single-region boundary path drops offset splits; the multi-region path does not

`session/compute.rs:1800` and `viz .../execute/mod.rs:701` take
`boundaries.first()`. A boundary that *splits* into several polygons under the
containment offset keeps piece 1 and clips everything outside it away — an
under-cut, and a silent one. `session/compute.rs:1881` (the multi-region
variant) `.flat_map(…)`s and keeps them all. The two paths disagree about the
same question.

### O-3 — `polygon.rs:249`: an `#[allow]` inside a doc-comment block

`#[allow(clippy::indexing_slicing)]` sits between two lines of
`offset_polygon`'s doc comment (`polygon.rs:245-262`). It compiles, it splits
the rendered doc, and it is broader than anything in that function's body
needs (`offset_polygon` does no indexing at all). Cosmetic; listed so it is
not rediscovered.

---

## 5. What a future migration would have to declare

If Checkpoint C ever authorises widening `OffsetRingSet` beyond rows 1–2,
each new consumer must state, in code, at the call site:

1. **one** `FlattenPolicy`, with its `max_deviation_mm` derived from the
   operation's own chord tolerance (`from_chord_tolerance`) or explicitly
   `untoleranced()`;
2. whether it needs `with_max_segment`, and **in world units, from the
   consumer's own physics** — scallop's is the flat-ground stepover, not a
   constant;
3. its termination condition and ring cap, because `OffsetRingSet::offset`
   has neither: `pocket.rs:145` is an unbounded `loop` that exits on
   collapse, and the only thing between it and the 13-minute hang is that the
   cascade now shrinks instead of growing.

No global cleanup may be applied on the way — plan rule §H3 fix-shape 4 and
acceptance gate "no global cleanup is silently applied".

---

## 6. Method and limits

- Census by `rg` over `crates/`, then per-site read of the enclosing function
  and of the first consumer of the returned value. Every "carries sampling"
  verdict cites the line that turns a vertex into a probe or a cut point.
- **Not measured:** no consumer was re-run to confirm its sampling verdict
  empirically. The verdicts are read from source. A verdict marked
  `NO-OP` on a *statistic* (rows 18, 19) is safe by construction; a verdict
  marked `NO-OP` on a *mask* (rows 15–17, 20) assumes the raster cell is
  coarser than the vertex spacing, which was not measured.
- **Not covered:** `geo::Buffer` / i_overlay, the alternative backend M5
  benched (`offset_lab::geo_cascade`). It is not a production consumer and
  i_overlay 4.0.7 indexes out of bounds on the pocket cross fixture
  (`offset_lab.rs:1187-1190`), which is its own finding and is not R2's.
