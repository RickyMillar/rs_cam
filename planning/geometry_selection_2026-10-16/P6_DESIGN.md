# Geometry selection P6: open paths and holes (design, 2026-10-20)

Status: DRAFT for the lead. Author: the P6 design agent (read-only).
Inputs: `PLAN.md` (P6, the status table, every ruling: D2/D13, P2.2
"open paths keep the drawn direction", P2.2 "notes", P1b, the operator
ruling "P6: yes, extend an open pocket path to the stock"),
`REQUIREMENTS.md` (S9, S10, S11, the op table rows Profile, Trace,
Chamfer, Drill), `DESIGN.md` (4.2, 4.3, 12, 13, the P6 row of 15, D2,
D13), `GAP_ANALYSIS.md` (with the 2026-10-19 open-vector addendum),
`P3_DESIGN.md`, `P4_DESIGN.md`, and the code on `master` at 20425fc3.
Another session edits files in the tree. Line numbers can move.

Conventions (as P3_DESIGN and P4_DESIGN): **NEW** marks a name that does
not exist today. Core paths start at `crates/rs_cam_core/src/`, viz paths
at `crates/rs_cam_viz/src/`. **[OP]**: changes what is machined and no
ruling covers it; the operator rules it. **[LEAD]**: structure, UI or
wire; the lead rules it. Each item is one editor session.

---

## 0. Summary

P6 makes three cuts honest: a line cut on an open path, an open path that
closes against the stock, and a hole of a stated size.

1. **On the line and to one side.** Profile gets side `On`. A NEW open
   offset kernel lets Trace cut `Left`/`Right` of an open path. Every
   open path keeps its drawn direction (P2.2) and gets no lead past its
   ends (P1b).
2. **Extend to stock.** A Pocket option closes each open path by the
   straight tangent extension of its two ends to the stock outline. The
   region on one side of the drawn direction is the pocket.
3. **Holes.** The Drill drills a circle only when the circle is the tool
   size. A NEW `Bore` op helix-mills a circle that is larger than its
   tool. The stated size is the cut size (S11). The Drill gets no helix
   cycle (section 4.1).

| # | Item | Output change | Visible result |
|---|---|---|---|
| P6.1 | `ProfileSide::On`, `ChamferSide` split, G40 for On | none (new value only) | Profile side combo shows On |
| P6.2 | The open offset kernel; Trace `Left`/`Right` on open paths; the side sign (O-1) | Trace Left/Right on closed loops (O-1) | Trace Left on a line cuts beside it |
| P6.3 | Profile On: the adapter, open passes, tabs, leads, exact reach band | new value only | Profile On cuts a line |
| P6.4 | Pocket "extend to stock" (`ItemUse::RegionsExtendOpen`) | new option only | a pocket bounded by a curve and the stock edge |
| P6.5 | The Drill hole fit: a circle that is not the tool size is not drilled (O-3) | circles not of the tool size (O-3) | "3 circles are larger than the tool" note |
| P6.6 | The `Bore` op (helix-mill a circle to size) | new op only | a 10 mm hole from a 6 mm tool |
| P6.7 | GUI: side On, open-path direction and side, extend preview, hole sizes | none | sketches in section 7 |
| P6.8 | MCP and CLI parity, docs, catalog, registry closure | none | none |

---

## 1. What exists (the base of every item)

| Piece | Where | Fact |
|---|---|---|
| `ItemUse` | `compute/catalog/capabilities.rs:211` | `None`, `Regions`, `Curves`, `Holes { may_be_empty }`; no wildcard match at the readers |
| `ItemKind::serves` | `selection/view.rs:44` | `Path` serves `Curves` only |
| `view` | `selection/view.rs:293` | pure; a `Curves` view keeps open paths top level (`curves_of`, `:632`); notes kept for `Path` only (`:500`) |
| Trace override | `compute/catalog.rs:872–881` | `Curves` for `None`, `Regions` for `Left`/`Right` |
| Trace offset | `ops/trace_path.rs:64–80` | `Left` = `-tool_radius`, `Right` = `+tool_radius` through `offset_polygon_reported` |
| Offset sign | `polygon.rs:570` doc; cavalier `traits.rs:1527` doc | positive = inward on a CCW ring = to the LEFT of travel |
| Closed-only offset | `polygon.rs:272–273` `exterior_to_pline` (`with_capacity(n, true)`); `polygon.rs:844` refuses < 3 points | no open offset today |
| cavalier open offset | `cavalier_contours-0.7.0/src/polyline/internal/pline_offset.rs:1068`, `:1242` | an open polyline offsets; end circles trim the raw offset; the result can be several open pieces |
| Arc joins | `polygon.rs:284` `from_pline` drops the bulge; `polygon.rs:1088–1093` note | a chord cuts a 90° join by 29 % of the offset distance |
| One flattening | `polygon.rs:1117` `FlattenPolicy`, `:1166` `untoleranced()` (`UNTOLERANCED_MM`, `:1147`) | bounded arc flattening |
| Profile | `ops/profile.rs:22` `ProfileSide { Outside, Inside }`; `:112` `profile_contour_reported`; `:145` `profile_ring` (climb reverses); `:156` closed emitter | closed rings only |
| Profile adapter | `compute/execute/clearing_2d.rs:118` (`regions()`, `:125`); `ProfilePass` `:435`; `reach_band` `:485` | reads `regions()` |
| Trace adapter | `clearing_2d.rs:78`, `TracePass` `:337`, `outlines` = `curves()` `:359`, P1 band note `:373` | open paths on the line since P1 |
| Chamfer | `compute/operation_configs.rs:463` `ChamferConfig.side: ProfileSide`; `ops/chamfer.rs:26`, `:42`, `:98–99` | shares the Profile enum |
| G41/G42 | `gcode/mod.rs:1278` `controller_compensation_for`, exhaustive on `ProfileSide` (`:1289–1292`) | Profile `InControl` only |
| Leads | `compute/catalog.rs:184–229`; `dressup/mod.rs:1038` `LeadScope`; `capabilities.rs:528` `TRANSFORM_TRACE` | Trace: leads on closed passes only (P1b) |
| Reach band | `compute/execute/layered.rs:123` `ReachBand { rings, half_width }`; `standing_material.rs:326` | every ring reads as closed |
| Stock items | `selection/stock.rs:13` `stock_items` (a rectangle); `selection/resolve.rs:37` `ResolveScope.stock_bbox` (emission frame) | `ResolvedSelection` (`resolve.rs:159`) keeps no stock |
| Drill | `compute/execute/drilling.rs:323` `generate_drill`; `:138` `selected_hole_xys`; `:197` `unique_holes`; `:234` `drill_hole_columns` (D8) | every hole at the tool size; `Hole.diameter` is not read |
| `Hole` | `selection/view.rs:144` | `xy`, `diameter: Option<f64>`, `layer` |
| Helix entry | `dressup/entry_descent.rs:762` `emit_helix`, 36 steps a turn (`:801`) | an ENTRY, intent `EntryHelix` |
| Constants | `compute/alignment_pins/mod.rs:84` `PIN_CAD_ROUNDING_MM` = 0.01; `compute/config.rs:718` `DRESSUP_HELIX_PITCH_MM` = 1.0; `selection/join.rs:24` `CLOSE_TOL_MM` = 0.01 | cited where used |

### Defects found on the way (code reading, not run)

- **D-A (live, Trace side).** `TraceCompensation::Left` documents "to
  the left of the travel direction" (`ops/trace_path.rs:25`). The code
  offsets by `-tool_radius` (`:71–75`): outward on a CCW ring, so to the
  RIGHT of travel. The lib tests pin the code meaning (`:277`, `:337`).
  P6.2 must define Left on an open path, so the two meanings meet. **[OP]
  O-1.**
- **D-B (outside P6, for the lead).** `controller_compensation_for` maps
  (Outside, climb) to G42 (`gcode/mod.rs:1289`). Climb makes the pass CW
  (`ops/profile.rs:145`); on a CW pass round a part the outside is left
  of travel: G41. The map looks swapped, as D-A. Prove it with a G-code
  word test first. Posts with no cutter compensation are not affected.
- **D-C (outside P6, for the lead).** Profile Outside and Trace offsets
  go through `from_pline` (`polygon.rs:855`, `:284`), which keeps only
  the end points of an arc join. By `polygon.rs:1088–1093` that cuts a
  90° outside corner by 29 % of the tool radius: a gouge into the part.
  A fix moves the Profile golden. The P6 open kernel does not repeat it
  (section 2.4).
- **D-D (live, S11).** A Drill drills a 10 mm circle with a 6 mm tool
  and says nothing (`drilling.rs:138` reads `xy` only). P6.5 closes it.

---

## 2. Open paths on the line and to one side (question 1)

### 2.1 Which ops take open paths

| Op | Setting | `ItemUse` today | `ItemUse` after P6 | Open path |
|---|---|---|---|---|
| Trace | compensation None | Curves | Curves | on the line (P1) |
| Trace | Left / Right | Regions | Curves | NEW: offset to one side of the drawn direction |
| Profile | side On | — | Curves | NEW: on the line |
| Profile | Outside / Inside | Regions | Regions | skipped with a note (D2/D13) |
| Pocket | open paths Skip (default) | Regions | Regions | skipped with a note |
| Pocket | open paths Extend | — | RegionsExtendOpen | NEW: closed against the stock (section 3) |
| Project Curve | — | Curves | Curves | on the line (unchanged) |
| Chamfer | — | Regions | Regions | skipped with a note (L-6) |
| Zigzag, Adaptive 2D, VCarve, Inlay, InlayPlug | — | Regions | Regions | skipped with a note (L-7) |

### 2.2 The direction rule

- An open path is cut from its first point to its last point, in the
  drawn direction (P2.2 fix b; `detect_containment` keeps it). No op
  reverses an open path.
- Profile `climb` does not act on an open path. On the line both flanks
  cut, so climb has no meaning there. On a closed ring with side On,
  `profile_ring` (`ops/profile.rs:145`) keeps today's rule. **[LEAD] L-1.**
- `Left` and `Right` are relative to the drawn direction: `Left` puts
  the tool centre on the left of travel (as Estlcam, GAP_ANALYSIS
  addendum, and the G41 meaning). The P2.2 ruling names this rule.
- The setup mirror does not reverse an open path (P2.2), so `Left` stays
  `Left` on a Bottom setup in the drawing view. A sentry holds it.

### 2.3 Lead-in, lead-out and entry at open ends

- **Leads.** No lead goes past the drawn end of an open path (P1b). Every
  op that can emit an open pass states `LeadScope::ClosedPassesOnly`
  through its transform: Trace already does (`capabilities.rs:528`).
  Profile states NEW `TRANSFORM_PROFILE = TRANSFORM_DEPTH_ORDER
  .with_leads_on_closed_passes_only()`. A closed pass keeps its leads
  (`open_pass_ends` flags open passes only), so Outside and Inside stay
  byte-identical (section 6). **[LEAD] L-2.**
- **Entry.** The plunge is at the first point. A ramp or a helix entry
  dressup must stay on the cut line. The P1b sentry pattern
  (`open_paths_are_never_chord_closed_p1.rs`, arm (e)) extends to Profile
  On and Trace Left/Right: every fed move stays inside the extent of the
  cut pass, grown by `CLOSE_TOL_MM`.
- **Extension past the ends.** Not by default. An opt-in `ends:
  AsDrawn | ToStock` (NEW) on Trace and Profile On reuses the section 3
  kernel, so a groove runs off the stock edge. **[LEAD] L-3** (recommend:
  yes, in P6.4, default `AsDrawn`).

### 2.4 The open offset kernel (P6.2)

NEW in `polygon.rs`, beside `offset_polygon_reported`:

```rust
/// The offset of an open path: positive = to the LEFT of the drawn
/// direction. Gives the open pieces in drawn order. Closed input refuses.
pub fn offset_open_path_reported(
    path: &Polygon2,
    distance: f64,
) -> (Vec<Polygon2>, Option<OffsetFailure>);
```

1. Refuse a closed polygon and a path below 2 points
   (`OffsetRejection` NEW arms). Dedupe with `dedupe_pline`
   (`polygon.rs`, `PLINE_POS_EQUAL_EPS`).
2. Build an OPEN `Polyline` (`with_capacity(n, false)`); NEW
   `Polygon2::path_to_pline`. `exterior_to_pline` does not change.
3. `pline.parallel_offset(distance)` inside the same `catch_unwind`
   chokepoint as `offset_one` (`polygon.rs:609`): the R1 panic text names
   the open-polyline branch of cavalier.
4. Flatten each piece with `arcs_to_approx_lines(FlattenPolicy::untoleranced()
   .max_deviation_mm())`, as `flatten_ring` does (`polygon.rs:1370`).
   Never `from_pline` (D-C).
5. Each piece is `Polygon2::open_path`, in the order cavalier stitches
   them (vertex order of the input).

What cavalier guarantees (`pline_offset.rs:1068–1090`, `:1242–1290`): a
raw offset segment closer than `|distance|` to the input is cut out, and
the circles at the two input ends trim the raw offset. So no piece comes
nearer than the tool radius to the drawn line, and no piece goes past the
end normal. A bend tighter than the tool radius splits the offset into
several pieces. Each piece is cut as its own open pass. NEW finding
`OpenOffsetSplit { path, pieces }` (through `ctx.record`): "the tool
cannot follow the inside of a bend tighter than its radius here; N pieces
are cut". The material in the bend stays (the safe direction).

### 2.5 Profile side On (P6.1, P6.3)

**The enum.** `ProfileSide` gains `On`. `ChamferConfig.side` moves to NEW
`ChamferSide { Outside, Inside }` with the same serde names, so no
Chamfer file changes. A chamfer "on the line" is a V groove, not a bevel,
and the type must not hold it. **[LEAD] L-4.** Blast radius (`rg -c
ProfileSide`): 27 files; Chamfer: `ops/chamfer.rs` (7 sites),
`operation_configs.rs:472`, `tests/chamfer_inside_edge_bevels_into_the_material_g_chamferinside.rs`,
viz `engrave.rs`, `registry.rs:380`, `state/toolpath/configs.rs`.

**G-code.** `controller_compensation_for` gains `(ProfileSide::On, _) =>`
no compensation (G40), by the exhaustive match. `On` with
`CompensationType::InControl` refuses at every door through
`domain::validate_operation` (W6b): "side On cuts on the line; it has no
side for G41/G42. Set compensation to In computer." **[LEAD] L-5.**

**Capabilities.** `OperationConfig::capabilities()` gains a Profile arm:
`On` → `item_use: Curves`; `Outside`/`Inside` → `Regions`. The row
transform becomes `TRANSFORM_PROFILE` (2.3).

**Adapter** (`clearing_2d.rs:118`). `generate_profile` branches:
`Regions` reads `prepared.regions()` as today; `Curves` reads
`prepared.curves()`. `ProfilePass::prepare` with `On`:

- closed ring: the contour is the ring itself (as the `InControl`
  branch of `profile_path_reported`, `ops/profile.rs:66`), with tabs;
- open path: the contour is the path, cut by NEW
  `profile_path_open_to_toolpath` through `emit_path_segment_with_intent`
  (`toolpath.rs:262`), first point to last point.

**Tabs.** Auto tabs and hand tabs apply to closed rings only. An open
path with `tab_count > 0` gets a `TabPlacementFinding` note "open path:
no tabs". `resolve_hand_tabs` snaps to closed rings only. **[LEAD] L-8.**

**Reach band.** NEW `ReachBand.open: Vec<Vec<P2>>` beside `rings`. NEW
arm in `levels_in_reach` (`standing_material.rs:326`): an open path
reads a capsule band (segments, no closing chord). Trace and Profile fill
it. This closes the P1 note at `clearing_2d.rs:373`.

### 2.6 Trace Left and Right on open paths (P6.2)

- The Trace override in `compute/catalog.rs:872–881` collapses to
  `Curves` for every compensation. The P1 refusal path for Trace
  Left/Right on an open path goes away.
- `trace_compensated_polygons_reported` (`ops/trace_path.rs:64`):
  closed input → `offset_polygon_reported` as today; open input →
  `offset_open_path_reported`, distance `+R` for `Left`, `-R` for `Right`.
- The offset pieces are open passes; `trace_polygons_at_z` already cuts
  them on the open branch (`:98`).
- O-1 decides the closed meaning (D-A).

---

## 3. Extend to stock for Pocket (question 2)

The operator ruled it in scope (PLAN "P6: yes"). The open choices are the
side rule and where the region ends (O-2).

### 3.1 The type

```rust
// compute/catalog/capabilities.rs
pub enum ItemUse {
    None,
    Regions,
    /// NEW (P6): closed areas, and each open path closed by its tangent
    /// extension to the stock outline; the area on `side` of the drawn
    /// direction is cut.
    RegionsExtendOpen { side: PathSide },
    Curves,
    Holes { may_be_empty: bool, fit: HoleFitRule },   // section 4.5
}

// selection/rule.rs (NEW)
pub enum PathSide { Left, Right }

// compute/operation_configs.rs, PocketConfig (NEW field)
#[serde(default, skip_serializing_if = "OpenPaths::is_skip")]
pub open_paths: OpenPaths,           // Skip (default) | ExtendToStock { side: PathSide }
```

`OperationConfig::capabilities()` gains a Pocket arm: `ExtendToStock`
→ `RegionsExtendOpen { side }`. `ItemKind::serves`: `Loop`, `Circle`,
`Face`, `Path` serve it; `Point` does not. Every exhaustive reader gains
the arm by compile error: `use_phrase`, `note_fix`
(`selection/refusal.rs:176`), `view` (`:431`), `selection_status.rs:735`,
`:775`, the frozen W2.4 table. Blast radius: 143 `ItemUse::` sites in 24
files (`rg -c`), most in tests and the registry.

### 3.2 Where the stock comes from

`view` is pure and has no stock. NEW field `ResolvedSelection.stock_xy:
Option<[f64; 4]>`, filled by `resolve` from `ResolveScope.stock_bbox`
(`resolve.rs:37`). The frame door does not move a stock item, so the
rectangle is in the setup frame with the items. `from_polygons` gives
`None`, and an extend then refuses `NoStock` (NEW). No `view` caller (23
sites) changes. **[LEAD] L-9.** Reason: the GUI item states and the
kernel run one `view` (P4.4), so both see the same extended region.

### 3.3 The algorithm (`selection/extend.rs`, NEW; pure)

Input: one open path `P = [p0 .. pn]` in the setup frame, the stock
rectangle `S`, the side.

1. **Ends inside.** Each end must be inside `S` or on its edge within
   `CLOSE_TOL_MM` (`selection/join.rs:24`). An end on the edge needs no
   ray.
2. **Tangent.** At the end: the direction of the last segment, read back
   over segments shorter than `CLOSE_TOL_MM`. At the start: the reverse
   direction of the first segment, read the same way.
3. **Ray.** From an end inside `S`, the ray leaves the convex rectangle
   exactly once. The hit `h0` (start) and `hn` (end) are on `∂S`.
4. **Chord.** `E = [h0, p0 .. pn, hn]` must be simple: no crossing of
   `P` with itself, of a ray with `P`, of the two rays, or of `P` with
   `∂S` between its ends. Use `Polygon2::has_self_intersection` on the
   closed result plus a ray-to-path segment test.
5. **Close.** For `Left`, walk `∂S` from `hn` counter-clockwise to `h0`,
   adding the rectangle corners on the way; the ring is CCW with the
   region on the left of `E`. For `Right`, walk clockwise, then reverse
   the ring to CCW (`ensure_winding`).
6. **Result.** A `Loop` shape that stands in for the path item in the
   nesting. `view` then nests it with the other selected rings
   (`nest_closed_rings_at`, `view.rs:601`). The path item role is `Cut`.

O-2 (B) grows `S` by the tool radius before step 3. `view` has no tool,
so ONE input carries the tool diameter to both readers: the prologue
reads `ctx.tool_def`, and the status memo key `StatusKey`
(`session/selection_status.rs:1241`) gains `tool_diameter`. The hole fit
(4.3) reads the same input. Both call one NEW
`ResolvedSelection::with_stock_grown(r)` before `view` (L-9).

The territory does not undo (B). Pocket is `TerritoryXy::ClipAfter`
(`compute/catalog/capabilities.rs:306`): it reads no XY containment while
it plans. The session clip after generation applies only with a boundary
on; the default boundary is off (`compute/config.rs:740`). A boundary on
the stock with `Center` containment keeps a tool centre on the stock
edge, so (B) holds; `Tool` containment clips it back to (A), as the user
asked.

### 3.4 Failure cases (each refuses by item; NEW `RefusalCause::CannotExtend { why }`)

| Case | `why` | Fix in the message |
|---|---|---|
| An end is outside the stock | `EndOutsideStock` | "trim the path to the stock, or move the stock" |
| The path crosses the stock edge between its ends | `CrossesStockEdge` | "split the path at the stock edge" |
| A ray or the path crosses the path | `SelfCrossing` | "the path crosses itself after extension; edit the drawing" |
| The two rays cross inside the stock | `RaysCross` | "the ends point at each other; close the path with Close with line" |
| The region has zero area (path on the stock edge) | `NoArea` | "the path lies on the stock edge" |
| A selected loop crosses the extended region edge | `CrossesItem { other }` | "the loop on layer X crosses the extension; it can be no island" |
| No stock in the selection scope | `NoStock` | internal: a hand-built context |

The last data row matters for safety: a loop that straddles `E` does not
nest, so the loop would be cut as its own pocket where the user drew an
island. It refuses.

Under the default rule a path that cannot extend refuses too (it is not a
kind skip: the op serves the kind). Two extended regions that overlap are
two regions; the overlap is cut twice (no gouge). **[LEAD] L-10.**

### 3.5 Not a drawing edit

The extension depends on the stock, which the ledger
(`drawing_edit.rs:47`, file units, no stock) does not know. A stock
change re-extends at once. So it is a view step, not a `DrawingEdit`.

---

## 4. Holes larger than the drill (question 3)

### 4.1 Which op owns it: a NEW `Bore` op

A Drill toolpath carries a `DrillOp`: the sim removes its holes
analytically and skips its moves (`simulate.rs:1588`, `:1706`), the
export writes canned cycles (`program_builder.rs:761`), and the drill
gates read the plunge (`tool_load/drill_gates.rs:151`). A helix there is
not simulated and has no canned form. A NEW `Bore` op is an ordinary
milling toolpath: swept by the sim, linked, exported as moves, one
`for_each_op!` row (`compute/catalog.rs:309`). **[LEAD] L-11.** "From the same selection"
(S10): the Drill note for an oversize circle has the action "Add a Bore
for these holes", which adds a Bore with a copy of the Drill selection.
Both ops then split the holes by the fit rule (4.2).

### 4.2 The fit rule (one function: NEW `selection::hole_fit`)

Let `d` = the circle diameter (`Hole.diameter`), `D` = the tool diameter
(`tool_def.radius() * 2.0`, as `drilling.rs:62`), `e` =
`PIN_CAD_ROUNDING_MM` (`compute/alignment_pins/mod.rs:84`: "two diameters
closer than this are one value: the rounding of a CAD export"; 1/4 in and
6 mm stay apart).

| Hole | Fit (NEW `HoleFit`) | Drill | Bore |
|---|---|---|---|
| a point (no diameter) | `NoSize` | drilled at `D` (S11 has no stated size) | not served (`LargerThanTool`) |
| `|d − D| ≤ e` | `ToolSize` | drilled | skipped, note "drill it" |
| `d > D + e` and `d ≤ 2D` | `Larger` | skipped, note "add a Bore" (O-3) | helix at `r = (d − D)/2` |
| `d > 2D` | `LeavesCore` | skipped, note | refused or skipped, note "use a Pocket on the circle or a larger tool" (L-12) |
| `d < D − e` | `Smaller` | skipped, note "the tool is larger than the hole" (O-3) | skipped, same note |

The `2D` bound is geometry: one helix of radius `r` sweeps the annulus
`r − D/2 .. r + D/2`; it reaches the centre when `r ≤ D/2`, so `d ≤ 2D`.
A through hole with `d > 2D` drops a loose core; a blind hole keeps a
post. Both are refused.

Under the default rule a misfit circle is skipped with a `SelectionNote`
(NEW `NoteCause::HoleFit { fit, tool_diameter }`) by layer: it is a
likely tool or drawing mistake, so it gets a note, as an open path does
(P2.2 "notes"). A circle the user picked that does not fit refuses with
NEW `RefusalCause::HoleDoesNotFit { fit, tool_diameter }`. `Empty` names
both.

### 4.3 Where the fit runs

The fit needs the tool, which `view` does not have. It runs after `view`
in the prologue, as the P3.7 face-at-the-top rule does
(`compute/execute/prologue.rs:256` `region_ladders`): NEW
`OpItems::skip_holes(indices)` marks each misfit role `Skipped`, NEW
`hole_fit_notes` gives the notes. `selection_status` runs the same
function, so item states and the cut agree; its memo key `StatusKey`
(`session/selection_status.rs:1241`) gains the tool diameter.

### 4.4 The `Bore` op (P6.6)

Row: `ItemUse::Holes { may_be_empty: false, fit: HoleFitRule::LargerThanTool }`;
`GeometryRequirement::Items`; `is_drill_kinematics: false`; transform
`TRANSFORM_XY_INDEPENDENT` (each hole is independent, as Drill); `rest_use:
StockColumn` (D8: the hole top is the remaining stock over the hole,
`drilling.rs:234`, with the hole radius); `floor_owner: OpFloor` with a
`floor: Floor` (P3, D9; `StockBottom` gives a through hole, L-13 of P3);
tool constraint: a flat or bull-nose end mill (a ball or V-bit refuses
through `tool_refusal`); feeds family: the ramp feed of a 2.5D clearing
op. **[LEAD] L-14** names the feeds row.

NEW `BoreConfig` (W5 `OpParams`): `floor`, `depth_per_pass` is not used
(the helix sets the step); `pitch` (mm per turn; default
`DRESSUP_HELIX_PITCH_MM`, `compute/config.rs:718`, a repo rule with no
source; range from that dressup param), `climb` (default true, as
`ProfileConfig`, `operation_configs.rs:680`), `finish_allowance` (radial
mm; default 0 = one pass), `feed_rate`, `ramp_feed_rate`, `spindle_rpm`,
`selection` (driver-set).

NEW kernel `ops/bore.rs::bore_hole(centre, r, top_z, floor_z, params)`:

1. Rapid over the centre at safe Z; rapid to the R-plane (as
   `DrillHole::retract_z`).
2. Feed out to radius `r − a` (`a` = `finish_allowance`) above the top.
3. Helix down at `pitch` per turn to `floor_z`, CCW for climb (the wall is
   on the right of travel), 36 line segments a turn (the
   `entry_descent.rs:801` step; the arc-fit dressup can fit arcs).
4. One full turn at `floor_z` (flat floor).
5. With `a > 0`: feed out to `r`, one full turn at `floor_z`, then a
   helix UP to the top at `pitch` (a climb finish of the wall).
6. Feed to the centre at the floor; retract.

Intent: the turns are `MoveIntent` cut intents (`FinishingCut` or the
roughing intent), never `EntryHelix`: the steady-state gate filters entry
spans (`tool_load/locality.rs`), and a bore is the cut itself. **[LEAD]
L-15.** `a ≥ r` refuses at generation (the rough radius would be ≤ 0).

### 4.5 The `Holes` view with diameter

`Hole.diameter` already exists (`view.rs:144`). P6 adds:

- NEW `HoleFitRule { None, ToolSize, LargerThanTool }` in the `Holes`
  payload. The fit step (4.3) matches it with no wildcard, so the prologue
  needs no op-type match (W2.4): Pin Drill `None` (its pins are
  `cfg.diameter`, extras at the tool size; `pin_holes_are_drilled_by_size_r1`
  stays green, Pin Drill does not change), Drill `ToolSize`, Bore
  `LargerThanTool`.
- `ItemKind::serves(Point, Holes { fit: LargerThanTool, .. })` is false,
  so the pick bar disables the point chip for Bore with the `use_phrase`
  reason ("mills circles only").
- NEW `SelectionReport.hole_fits: Vec<(MatchKey, HoleFit)>` beside
  `hole_items` (`selection_status.rs:264`), from 4.3.
- The F095 predicate (viz test) reads `ItemUse::Holes { .. }`, so Bore
  joins the hole pick with no viz list.

---

## 5. Capabilities, selection defaults and notes (question 4)

| Change | Where | Effect on the default rule |
|---|---|---|
| Trace: `Curves` for every compensation | `compute/catalog.rs:872` | Trace Left/Right now cut the open paths of the default rule. The P2.2 path note for Trace Left/Right goes away. |
| Profile: `Curves` for `On` | NEW arm in `capabilities()` | Profile On cuts open paths; no path note. Outside/Inside keep the note. |
| Pocket: `RegionsExtendOpen` for `ExtendToStock` | NEW arm | Pocket Extend cuts open paths; no path note. Skip keeps the note. |
| `note_fix` (`refusal.rs:176`) | text | `(Path, Regions)`: "Use Trace, Profile with side On, or Pocket with Extend to stock, or close it in the drawing." `(Path, Holes)` unchanged. `(Path, RegionsExtendOpen)` none. |
| `Holes { fit }` | registry rows | Pin Drill `None` (unchanged), Drill `ToolSize`, Bore `LargerThanTool`. |
| `KindFilter::Served` | `rule.rs:35` | unchanged: it reads `serves`, so each new arm is the new default. |
| NEW notes | `NoteCause::HoleFit` | Drill and Bore: misfit circles, by layer (4.2). |

A sentry holds `GeometryRequirement::Items` iff `item_use != None`
(DESIGN 4.2); the three new values are all `!= None`.

---

## 6. Byte identity (question 5)

G-OPGOLDEN (`tests/every_op_keeps_its_toolpath_g_opgolden.rs`) does not
move for an existing case:

- The fixture drawing holds closed loops and POINT drill targets only
  (`tests/common/op_fixtures.rs:85`, `:219–230`). Points fit as
  `NoSize` and drill as today. Do not add a circle to `outline_model`.
- `ProfileSide::On`, `OpenPaths::ExtendToStock`, `ends: ToStock` and
  `Bore` are new values; every existing case reads the default.
- `TRANSFORM_PROFILE`: a closed pass is never flagged by `open_pass_ends`,
  so Outside and Inside keep their leads bit for bit (prove it: the
  `profile/fresh` and `profile/inside` records do not move).
- No golden arm sets Trace Left/Right, so O-1 does not move the golden.
  It moves the lib tests at `ops/trace_path.rs:277–360`, `:436` by name.
- The closed offset path (`offset_polygon_reported`) does not change.
  The D-C fix is not in P6.

NEW golden cases (each with its own fixture model through `CaseSpec`, the
`each_face_model` pattern):

| Case | Fixture (NEW) |
|---|---|
| `profile/on_open`, `profile/on_closed` | `open_path_model`: an open L, an open arc, a 2-point line, one square |
| `trace/left_open`, `trace/right_open` | `open_path_model` |
| `pocket/extend_left`, `pocket/extend_right` | `open_path_model` (the L on a board) |
| `drill/fit_mixed` | `hole_model`: 2 points, a circle of the tool size, a larger circle, a smaller circle |
| `bore/fresh`, `bore/finish_pass`, `bore/rest` | `hole_model` |

The `fresh` arm of `Bore` on the shared fixture (W4 and the golden loop
over `OperationType::ALL`) gets `hole_model` through the per-op fixture
choice, not a refusal record. **[LEAD] L-16.**

---

## 7. The GUI (question 6)

**Profile side combo** (`ui/properties/operations/boundary_2d.rs:326`).

```
 Side          [ Outside ▾ ]           ComboBox: Outside | Inside | On the line
 Compensation  [ In computer ▾ ]       In control greyed under On, hover: "side On has no side for G41/G42"
 Climb         [x]                     under On: hint "open paths cut in drawn direction"
 Tabs          2 per loop              under On: "closed loops only"
```

**Trace compensation** (`engrave.rs:21`): the labels become "On the
line", "Left of the drawn direction", "Right of the drawn direction".
The diagram (`registry.rs` `dia_trace`) draws an arrow and the tool on
its side.

**Open path marks** (P4.4 marks the ends). P6 adds a direction arrow at
the path middle, and for Left/Right the tool-centre offset in the
preview colour, from the item states. The arrow is the start rule of
P2.2 on screen.

```
   start ●───────────▶──────────────╮
                                    │      ▶ = drawn direction
          · · · · · · · · · · · ·   │      · = tool centre (Left)
                                    ╰──● end
```

**Pocket "Open paths" row** (`boundary_2d.rs`, Pocket form):

```
 Open paths    ( ) Skip   (•) Extend to stock
 Side          [ Left of drawn direction ▾ ]   [ ⇄ Flip ]
               2 paths extended · 1 refused: "an end is outside the stock" (Show me)
```

The viewport draws each extended region outline in the cut colour, and
each ray as a dashed line from the path end to the stock edge (NEW
`SelectionReport.extended: Vec<Polygon2>`, the item-state colours of
P4.4).

```
 ┌───────────────── stock ─────────────────┐
 │ ░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░ │   ░ = pocket region (Left)
 │ ░░░░░░░░░░░╭──▶──────╮░░░░░░░░░░░░░░░░░ │
 ├ ─ ─ ─ ─ ─ ─╯         ╰─ ─ ─ ─ ─ ─ ─ ─ ─ ┤   ─ ─ = tangent rays
 │                                         │
 └─────────────────────────────────────────┘
```

**Hole sizes** (`ui/properties/operations/drill.rs:28`
`draw_hole_legend`, and the Selection section rows):

```
 HOLES   tool Ø 6.000
 Ø 6.000  × 4   drilled                         StatusChip Role::Ok
 Ø 10.000 × 3   larger than the tool  [ Add a Bore for these ]   Role::Caution
 Ø 4.000  × 1   smaller than the tool                            Role::Caution
 points   × 2   drilled at Ø 6.000
```

The Bore panel shows the same rows with "helix r 2.000" per size and the
`LeavesCore` row in `Role::Danger`. Drill markers (`app/gpu_upload.rs`)
draw a misfit circle in the skipped colour (`render/colors.rs`).

---

## 8. MCP and CLI parity (question 7)

| Surface | Change | Pinned by |
|---|---|---|
| `set_toolpath_param side on` | `ParamEnum` gives `on` with no wire code | `schema_enum_values_g_schemaenum` (moves by design: Profile gains `on`; Chamfer keeps two) |
| `set_toolpath_param open_paths` | NEW W5 param (an object: `{"extend_to_stock": {"side": "left"}}`) | NEW `mcp_and_gui_give_one_extended_pocket_p6_4` (viz lib) |
| `add_toolpath` kind `bore` | `kind_str` from the `for_each_op!` row | `operation_type_serde_repr_pinned`, `every_op_conforms_w4` |
| `get_operation_schema` | Bore params; `ends`, `open_paths` | `every_config_field_is_published_driver_set_or_skipped` |
| `selection_status` | `item_states` gain `hole_fit`; `extended` outlines | NEW `mcp_hole_fit_equals_the_gui_report_p6_5` (viz lib) |
| `set_toolpath_floor` on Bore | the P3.3 door, `FloorOwner::OpFloor` | `the_floor_wire_rows_name_the_floor_door_p3_8` gains the Bore row |
| "Add a Bore for these" | NEW `Command::AddBoreFromDrill { index }` (one undo step; copies the selection, picks no tool) | `command_registry_completeness`, `production_writes_go_through_apply_wp15a`; MCP `add_bore_from_drill` |
| CLI job file | op kind `bore`; keys `open_paths`, `ends` read as params | NEW `a_job_file_bores_a_hole_p6_8` (`rs_cam_cli`) |
| `mcp_wire_surface_pin` | moves by design | viz |

MCP and the GUI read one `SelectionReport` (P4.4), so `hole_fit` and
`extended` cannot differ by surface.

---

## 9. Items (question 8)

Gates per PLAN (named sentries, folder sentries, viz `--lib`, `rs_cam_mcp`,
clippy, fmt, through `scripts/cargo_lane.sh`; ask before 3 minutes). A
red proof puts the defect back, sees red, restores.

**P6.1 — `ProfileSide::On`, `ChamferSide`, G40** (no output change).
Files: `ops/profile.rs:22`; NEW `ChamferSide` in `ops/chamfer.rs`;
`operation_configs.rs:472`; `gcode/mod.rs:1289` (`On` → `None`);
`compute/domain.rs` (`On` + `InControl` refusal); viz `boundary_2d.rs:326`,
`registry.rs:370`, `:380`, `engrave.rs` (Chamfer combo). Profile `On`
refuses "not yet" in `generate_profile` until P6.3. Sentries: NEW lib
`side_on_asks_no_controller_compensation_p6_1`; NEW
`side_on_with_in_control_refuses_at_every_door_p6_1`;
`schema_enum_values_g_schemaenum` moves by name; `chamfer_bevel_sits_on_the_edge_g_chamferside`,
`chamfer_inside_edge_bevels_into_the_material_g_chamferinside` green;
G-OPGOLDEN unchanged. Red proof: map `On` to `Left`; the lib sentry
fails. Operator looks at: nothing.

**P6.2 — The open offset kernel; Trace Left/Right on open paths; O-1.**
Files: `polygon.rs` (`offset_open_path_reported`, `path_to_pline`, the
`OffsetRejection` arms); `ops/trace_path.rs:64–80` (open branch; O-1
sign); `compute/catalog.rs:872` (Trace → `Curves`); `compute/toolpath_stats.rs`
(`OpenOffsetSplit`); `ops/CLAUDE.md` (the P1 line "offset, closed loops
only" changes). Sentries: NEW lib `polygon::open_offset_*` (a line, an L,
a bend tighter than R gives 2 pieces, a closed input refuses, every
piece point is `≥ R − FlattenPolicy::UNTOLERANCED_MM` from the input);
NEW `trace_cuts_beside_an_open_path_p6_2` (session path: Left of an
eastward line cuts at +Y; Right at −Y; on a Bottom setup the side holds
in the drawing frame; no fed move past the end normals; P1b arm (e)
pattern); `open_paths_are_never_chord_closed_p1` arm (d) is rewritten
(Trace Left now cuts the open path); in
`every_op_states_its_capabilities_w2_4`, `trace_compensation_picks_its_item_use_p1`
moves by name (Left/Right → `Curves`); the frozen
`default_selection_equals_the_old_lists_p2_2` names no Trace setting and
does not move; G-OPGOLDEN unchanged plus the
`trace/left_open`, `trace/right_open` cases. Red proof: swap the open
sign; the side sentry fails. Operator looks at: a Trace Left on an open
curve in `layers.dxf`, then Right.

**P6.3 — Profile On** (new value only). Files: `compute/catalog.rs`
(Profile arm; `TRANSFORM_PROFILE`); `compute/catalog/capabilities.rs`;
`compute/execute/clearing_2d.rs:118–590` (the `Curves` branch, open
contours, tab rule); `ops/profile.rs` (`profile_path_open_to_toolpath`);
`dressup/tabs.rs` (closed only, the note); `compute/execute/layered.rs:123`
and `standing_material.rs:326` (open band). Sentries: NEW
`profile_on_cuts_on_the_line_p6_3` (an open L and a 2-point line cut
exactly, no chord, no lead past the ends; a closed square On cuts the
outline; tabs on the square only, with the note; climb does not reverse
the open path); NEW lib `an_open_band_has_no_chord_p6_3`
(`levels_in_reach` on a U path skips a level whose material lies only on
the chord); `every_op_states_its_capabilities_w2_4` moves by name (Profile
transform); G-OPGOLDEN unchanged plus `profile/on_*`. Red proof: send the
open path through the closed emitter; the chord check fails. Operator
looks at: Profile On on an open path and on a loop, with tabs on.

**P6.4 — Extend to stock** (new option only). Files: NEW
`selection/extend.rs`; `selection/resolve.rs` (`stock_xy`);
`selection/view.rs` (the `RegionsExtendOpen` arm); `selection/refusal.rs`
(`CannotExtend`, `use_phrase`, `note_fix`); `compute/catalog/capabilities.rs`
(`ItemUse`, `PathSide`); `operation_configs.rs:524` (`open_paths`);
`compute/catalog.rs` (Pocket arm); `compute/execute/prologue.rs` (O-2
growth); `session/selection_status.rs:735`, `:775`, `extended`; the
`ends: ToStock` option of L-3 (Trace, Profile On). Sentries: NEW lib
`selection::extend` (each row of 3.4, Left and Right of one L give the two
complements of the stock, an end on the edge needs no ray, a loop inside
the Left region becomes its island); NEW
`a_pocket_extends_an_open_path_to_the_stock_p6_4` (session path: the
cut stays on the chosen side; nothing is cut on the other side of the
path by more than `CLOSE_TOL_MM`; the item state of the path is `Cut`);
`item_states_match_the_cut_p4_4` gains the arm;
`selection_refusals_name_the_item_p2_4` gains the `CannotExtend` rows.
Red proof: walk `∂S` the wrong way; the side sentry fails. Operator looks
at: `layers.dxf`, a Pocket with Extend on an open path, Flip, an end
moved outside the stock.

**P6.5 — The Drill hole fit** (O-3). Files: NEW `selection/hole_fit.rs`
(`HoleFit`, `hole_fit`); `selection/view.rs` (`OpItems::skip_holes`,
`Holes { fit }`, `HoleFitRule`); `selection/refusal.rs` (`NoteCause::HoleFit`,
`HoleDoesNotFit`); `compute/execute/prologue.rs` (the fit step);
`compute/execute/drilling.rs:50`, `:323` (both read the fitted holes);
`session/selection_status.rs` (`hole_fits`, `StatusKey` tool diameter).
Sentries: NEW `a_drill_drills_only_the_tool_size_p6_5` (`hole_model`: 2
points and the tool-size circle drilled; the larger and smaller circles
skipped with one note each; a pick of the larger circle refuses; `e` =
`PIN_CAD_ROUNDING_MM` both sides, 6.35 against 6.0 apart);
`drill_picks_resolve_to_targets_g_drillpickstale`,
`a_drill_hole_is_cut_once_g_drilltwice`, `pin_holes_are_drilled_by_size_r1`
green; G-OPGOLDEN unchanged plus `drill/fit_mixed`. Red proof: read `xy`
only again; the larger circle is drilled and the sentry fails. Operator
looks at: a Drill on a DXF with 5 mm and 10 mm circles and a 5 mm tool.

**P6.6 — The `Bore` op.** Files: `compute/catalog.rs:309` (row),
`compute/catalog/registry.rs` (NEW `REG_BORE`), `operation_configs.rs`
(`BoreConfig`, `impl_operation_params!`), NEW `ops/bore.rs`, NEW
`compute/execute/bore.rs` adapter (or `drilling.rs`, L-11),
`feeds/` row (L-14), `FEATURE_CATALOG.md`. Sentries: NEW
`a_bore_cuts_the_stated_size_p6_6` (simulated stock: the hole wall at
`d/2 ± sim cell`, the floor flat at the floor Z, no core for `d ≤ 2D`;
`LeavesCore` and `Smaller` refuse by item; a Bore on remaining stock
starts at the column top); NEW lib `ops::bore` (turn count =
`ceil(depth / pitch)`, climb direction CCW, finish pass radius);
`every_op_conforms_w4` with the Bore row; `retract_intent_move_type_census_w6`
gains Bore; G-OPGOLDEN `bore/*` cases. Red proof: helix at `d/2` (no tool
offset); the wall check fails by `D/2`. Operator looks at: a 10 mm and a
12 mm hole with a 6 mm tool in the simulation; a through hole with floor
Stock bottom.

**P6.7 — GUI.** Files: section 7 sites; NEW `Command::AddBoreFromDrill`
GUI path. Sentries: NEW viz `side_on_greys_in_control_p6_7`,
`the_hole_rows_show_each_fit_p6_7`, `add_bore_copies_the_selection_p6_7`;
keep green `the_inspector_text_is_bounded_ic1`,
`panels_read_the_token_module_up1`,
`egui_draw_sites_write_through_commands_wp6`, the F095 scan. Operator
looks at: every sketch of section 7 (rebuild the release GUI from HEAD).

**P6.8 — Parity and docs.** Section 8 rows, each pinned by a named test;
`FEATURE_CATALOG.md`; `selection/CLAUDE.md`, `compute/CLAUDE.md`,
`ops/CLAUDE.md` (each stays at 40 lines or fewer); the MCP tool docs; the
`PLAN.md` status row.

Order: P6.1 → P6.2 → P6.3 (one file family, serial). P6.5 → P6.6. P6.4
after P6.2 (shares `ends: ToStock` with L-3). P6.7 and P6.8 last.

---

## 10. Decisions (question 9)

### For the operator (each changes what is machined)

| # | Question | Recommendation |
|---|---|---|
| O-1 | Trace `Left` today cuts to the RIGHT of travel on a closed loop (D-A). For open paths, P2.2 says the drawn direction decides, as Estlcam. (A) one meaning for both: `Left` = tool on the left of travel; every existing closed Trace Left/Right cuts on the other side; the commit states the break. (B) keep the closed code meaning and give open paths the same sign (Left = right of travel); fix the doc only. | (A). It matches the label, Estlcam and G41. The 2026-09-16 rule allows the break. No golden arm sets it. The lead tells the operator which job files hold a Trace Left/Right. |
| O-2 | Extend to stock: (i) which side is the pocket, and (ii) where the region ends. (i) Left of the drawn direction by default with a Flip, or the smaller piece, or no default (the user must choose). (ii) (A) at the stock edge (the tool edge touches it), or (B) one tool radius past it (the tool centre reaches the stock edge). | (i) Left of the drawn direction, with Flip and the preview: one rule with Trace and the P2.2 ruling; the smaller piece changes side when the drawing changes. (ii) (B): no sliver at the stock edge when the real stock is a little larger, as Fusion's extension past the stock. |
| O-3 | A Drill circle that is not the tool size (D-D). Today it is drilled at the tool size with no word. (A) Skip it with a note under the default rule, refuse a picked one (S11; the safe direction of D2). (B) Drill it at the tool size as today, with a caution. | (A). S11 says the stated size is the cut size. The note offers "Add a Bore". Existing projects with such circles cut less and say why. |

### For the lead (structure, UI, wire)

| # | Question | Recommendation |
|---|---|---|
| L-1 | Profile `climb` on side On | Not read for an open path (drawn direction, P2.2); a closed ring keeps `profile_ring` |
| L-2 | Profile transform | NEW `TRANSFORM_PROFILE` with closed-pass leads; byte-identical for Outside/Inside |
| L-3 | `ends: AsDrawn | ToStock` on Trace and Profile On | Yes, default `AsDrawn` (P1b stays the default); reuses the P6.4 kernel |
| L-4 | Chamfer side type | NEW `ChamferSide`; same file names, so no file break |
| L-5 | `On` + `InControl` | Refuse at every door (W6b) |
| L-6 | Chamfer on open edges (REQUIREMENTS "P (edge)") | Not in P6. A follow-up can take `ChamferSide` = material on the left/right of the drawn direction with the open kernel |
| L-7 | Extend for Zigzag and Adaptive 2D | Not in P6 (plan scope: Pocket). Each is one config field and one `capabilities()` arm later |
| L-8 | Tabs on an open path under On | None; a note when `tab_count > 0`; hand tabs snap to closed rings |
| L-9 | The stock for `view` | `ResolvedSelection.stock_xy` from `ResolveScope.stock_bbox`; no new `view` parameter |
| L-10 | Overlapping extended regions; a path that cannot extend under the default rule | Two regions (cut twice, no gouge); refuse (the op serves the kind) |
| L-11 | Who owns the helix | NEW `Bore` op; not a Drill cycle (simulation and canned-cycle export) |
| L-12 | A Bore circle with `d > 2D` | Refuse by item, skip under default with a note; fix "use a Pocket on the circle or a larger tool". A spiral-out mode is a follow-up |
| L-13 | Tool-size tolerance | `PIN_CAD_ROUNDING_MM`; move it to a shared NEW `DIAMETER_ROUNDING_MM` in `selection/key.rs` with the pin code reading it (one meaning) |
| L-14 | Bore feeds row | The ramp feed of the 2.5D clearing family; the feeds owner confirms the row |
| L-15 | Bore move intents | Cut intents, not `EntryHelix` |
| L-16 | Bore on the shared W4/golden fixture | A per-op fixture (`hole_model`), not a refusal record |
| L-17 | "Add a Bore for these" | NEW `Command::AddBoreFromDrill`: copies the Drill selection, picks no tool (P8 owns tool advice) |
| L-18 | A note for a skipped misfit circle extends the P2.2 "notes" ruling (notes for skipped `Path` items only) | Yes, on purpose: a circle of the wrong size is a likely tool or drawing mistake, as an open outline is; other skipped kinds stay note-free |
| L-19 | The hole-fit payload | `HoleFitRule` in `ItemUse::Holes` (not a bool, not an op-type match), so Pin Drill states `None` and keeps its holes |

---

## 11. Risks

- **cavalier open offset panics.** The R1 assertion text
  (`polygon.rs:612–620`) names the open-polyline branch. The open kernel
  sits inside the same `catch_unwind`; a failure is a finding, never a
  silent empty pass.
- **Ramp entry on a short open path** can run past its end. The P6.2 and
  P6.3 sentries check every fed move against the pass extent.
- **O-1 (A) moves real cuts**: tell the operator before the commit.
- **D-C is a live gouge** on every software-compensated outside Profile.
  Recommend a separate item with a sentry before P6.3 (same adapter).
- **The memo** gains the tool diameter; a tool edit must stale the Drill
  and Bore item states (P4.4 freshness pattern).
