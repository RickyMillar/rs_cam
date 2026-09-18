# Corne case: why the roughs weave and the waterline is empty

Date: 2026-09-18. Fixture: `~/corne_job.toml` with `Corne Case Hex.stl`
(a copy of the project as found is `corne_job_as_found.toml`). Method: live
MCP reads and controlled regenerations on the loaded project, plus three
read-only code traces. Nothing is committed. The GUI project was edited in
the course of the experiments; §8 lists every edit.

## 1. The part

A keyboard case tray. Top-down heightmap: `01_part_heightmap.png`.

| Feature | Z | Area |
|---|---|---|
| Wall top (3 mm wall) | 18.0 | 856 mm² |
| Floor top | 3.5 | 5 138 mm² |
| Hex through-holes (~8 mm across flats, 2 mm webs) and 5 screw holes (~2 mm) | 3.5 → 0.0 | ~6 000 mm² |
| USB notch underside | 15.0 | 100 mm² |

Model bbox X 5.3..148.1, Y −104.3..−3.1, Z 0..18. Stock Z 0..23, so 5 mm
of stock sits above the wall tops. Tools: 6 mm 2F flat, 3 mm 2F flat.

Footnote: the stock is 153 × 169 mm. `StockConfig::update_from_bbox` on the
mesh bbox gives 153 × 111 mm (`compute/stock_config.rs:149-163`). 58 mm of
stock in +Y holds no part. `auto_from_model` is `true`, so the origin of
the 169 is not the loader. Not chased.

## 2. What the operator expected, and what the program does

| Expectation | What the program does | Why |
|---|---|---|
| Rough the interior in one smooth block | Contour-parallel rings that follow every hex outline at Z 17, 11 and 5, far above the floor (`05_6mm_rough_silhouette_top.png`) | §3 |
| Pocket the hex details with the 3 mm at the bottom | The 3 mm rest pass never enters a hex. As found it cleared a band along the walls (`03_as_found_3mm_rough_top.png`) | §3, §6 |
| Waterline the wall sides | One level at Z −1.37, 84 loops, 18.5 mm bite, through the stock bottom (`04_as_found_waterline.png`, `06_sim_silhouette_boundary.png` bottom view) | §4 |
| See the part in the simulation | A flat red slab with a pocket. Nothing outside the walls is cut (`06_sim_silhouette_boundary.png`) | §3 |

## 3. Roughing: the silhouette boundary keeps the holes

### 3.1 Mechanism

`model_silhouette` rasterises every face, runs marching squares and nests
inner loops as holes through `detect_containment`
(`geometry/boundary.rs:456-491`). `contains_point` returns `false` inside a
hole (`polygon.rs:298-306`).

Before the Z ladder runs, adaptive3d clips its own material grid to the
boundary: every cell whose centre fails `contains_point` is ray-cleared to
the surface Z, or to the stock bottom when the heightmap has no surface
there (`adaptive3d/path.rs:530-563`). A hex cell has no surface, so it is
cleared to Z 0. The planner then sees an island of "no material" at every
level from 20 down.

Result: at Z 17, where the real stock is one solid slab, the material grid
is a honeycomb. Contour-parallel offsets that honeycomb. That is the
weaving.

The same clip removes everything outside the silhouette. The stock around
the walls stays at Z 23. The walls are never formed, and the outer band is
never faced.

### 3.2 Counterfactual, observed

Boundary source `stock` on the 6 mm rough (also `bottom_z` pinned to 4.0,
see §8): one contour-parallel fill of the slab at Z 17, 11, 5 and 4,
95 m of cutting, no hex outlines (`07_6mm_rough_stock_boundary_top.png`).
Boundary source `stock` on the 3 mm rest: the hexes and the screw holes
get cleared at Z 2.0 and 0.5 (`08_3mm_rest_stock_boundary_top.png`). The
simulation then shows the part (`10_sim_stock_boundary.png`).

The `stock` boundary also roughs the 58 mm of empty stock in +Y (§1
footnote). The recipe pays for that.

### 3.3 The ladder

`adaptive_3d_segments` builds the ladder as `stock_top − n·depth_per_pass`
down to `z_bottom = min surface + stock_to_leave` (`adaptive3d/path.rs:575-591`).
For the 6 mm: 17, 11, 5, 0.5. For the 3 mm: 20, 17, …, 2, 0.5. No level
snaps to the flat floor at 4.0. The floor is cut only because the LAST
level clamps each cell to `max(surface + leave, level)`
(`adaptive3d/clearing.rs:374`), so the "Z 0.5" pass emits floor moves at
Z 4.0 and hex moves at Z 0.5. The narration reads that as a pass at 0.5.
`detect_flat_areas` would add a bin-centred level at 4.05
(`adaptive3d/path.rs:35-78`), not 4.0.

### 3.4 Air cut

The as-found program simulated at 79 % air of cutting time. That number is
`degraded` for the 6 mm (blind fraction 0.34 at 0.4 mm cells) and
`not_measurable` for the 3 mm (blind 0.79). The 6 mm number is real in
direction: the honeycomb weave and the 5 mm of empty stock above the walls
make most of the moves.

## 4. Waterline: two defects, one dangerous result

### 4.1 Auto heights collapse the ladder to one level

The adapter passes `ctx.heights.top_z` and `ctx.heights.bottom_z` straight
into `waterline_z_levels` (`compute/execute/finish_raster.rs:431-440`).
Auto `top_z` is the stock top; Auto `bottom_z` is `top_z − op_depth`
(`compute/config.rs:305-316`). Waterline declares
`depth_semantics: None` (`compute/operation_configs.rs:2096-2100`), so
`op_depth` is 0 and `bottom_z == top_z == 23`. One level at Z 23, above
the mesh, zero contours. `generated_empty.rs:289-291` accepts an empty
result from any `FromRemainingStock` op as legitimate rest machining, so
no finding is raised. This is "the waterline generates nothing".

The viz test `heights_display_leaves_auto_rows_unpinned_g_heightstab`
pins "on a zero-depth op the resolver's Auto bottom sits AT the stock top"
(`ui/properties/operations/mod.rs:405-415`). The test guards the display;
it also fixes the empty ladder in place.

### 4.2 One drag on the Heights diagram pinned `top_z` to −1.37

The saved file has `heights.top_z = manual −1.3704614721537909`. The only
writer of an un-rounded `Manual` value is the drag handler of the Heights
diagram (`ui/properties/operations/height_diagram.rs:451-474`,
`Manual(current_z + dz)` with `dz` from screen pixels). On a waterline the
Top and Bottom lines coincide (§4.1). The hit test takes the first line
under the pointer with a strict `<` (`height_diagram.rs:380-388`); Top is
index 3, Bottom index 4, so a drag on the shared line moves Top. Bottom
(Auto) follows Top. One drag gives one level at −1.37.

### 4.3 What that level does

At Z −1.37 the 6 mm flat tip is below the whole part. The push-cutter only
blocks a fibre inside `[z, z + cutting_length]` at vertices and edges
(`surface/pushcutter.rs:256-258, 455-458`); flat facets above the tip block
nothing. The free set is every floor-triangle interior more than 3 mm
from a mesh edge: 84 small loops of ~12 mm. The silhouette clip turns the
loops outside the part into rapids. The ramp-entry dressup (3°) ramps into
each survivor. The simulation reads an 18.5 mm bite at (148.2, −60.6,
−1.37) and the bottom view shows circles punched through Z 0 along the
right wall. On the machine that is the outer wall and the bed.

No gate stopped it: the empty gate saw a non-empty result, the air-cut
filter found stock above every sample, and no reading checks a level
against the model bottom or the stock bottom.

### 4.4 Pinned 18 → 0, observed

With `top_z` 18 and `bottom_z` 0 the waterline emits 19 levels and traces
the inner wall faces (`09_waterline_pinned_18_0_top.png`). The outer faces
are still missing: the outer CL loop sits 3 mm outside the silhouette and
the `center` containment clips it (`geometry/boundary.rs:73, 405-411`).
Levels 1 and 0 reach into the half-hex slots along the walls; those are
through-holes by design, so the bottom view of `10_sim_stock_boundary.png`
still shows circles there. A level AT the stock bottom cuts the bed by
definition; that is a ruling (§7).

### 4.5 With a sane ladder the waterline still saws through the walls

Checkpoints: after the 6 mm and after the 3 mm the walls stand
(`11_sim_after_3mm_rest_front_left.png`); after the waterline the right
wall is a row of pins (`12_sim_after_waterline_front_left.png`,
`13_gui_simulation_stock_boundary.png`).

The G-code shows the mechanism. The OUTER CL loop runs at X 150.8 (the
silhouette clip makes it rapids). At world Y −18.1, −25.1, −32.1, −39.1,
−45.6, −52.6, −59.6, −66.6 (7 mm pitch, the wall's triangle strips share
the hex rows) the loop makes a 2 mm notch inward to X 146.3, INSIDE the
3 mm wall, at every level from 18 to 0. The notch is inside the
silhouette, so it survives as a cut. The ramp-entry dressup (3°) then
folds its descent along that 2 mm notch: a 1 mm drop at 3° is 19 mm of
XY, so the tool saws back and forth across the wall ten times per level,
per notch, per level. That is the row of pins and the 70 816 moves.

Root cause: `facet_push` skips vertical facets
(`surface/pushcutter.rs:320-323`). A vertical wall is seen only through
its edges and vertices, and a fibre through the middle of a long thin
wall triangle finds no contact. Terrain has no vertical facets, so this
never showed. Second contributor: levels that land exactly on a flat face
(Z 18.0 on the wall top, Z 15.0 on the USB notch floor) weave junk
across the face.

### 4.6 The export safety check

`export_gcode` on this program reported 18 680 errors: "Rapid (G0)
repositions in X/Y at Z5.000, below the clearance plane 10.000". The
program is emitted with Z0 at the stock top (retract 28 world = Z5), the
checker compares against the raw `post.safe_z` 10. Every datum-at-stock-top
export reads as unsafe. Frame defect in the checker, not in the program.

## 5. Boundary semantics, one table

| Surface | Silhouette keeps holes | Effect on this part |
|---|---|---|
| adaptive3d pre-clip (`path.rs:530-563`) | holes become no-material islands | weaving; hexes never cut |
| Post-generation clip (`boundary.rs:405-411`) | moves inside a hole become rapids | waterline cannot trace hex walls |
| `center` containment (`boundary.rs:73`) | outer CL loop is outside | outer wall faces never cut; outer stock never removed |

A rough wants the holes as material and the outside as material. A
waterline wants the hole walls and the outer faces. The silhouette as a
boundary serves neither on a part with through-holes.

## 6. State and plan gaps seen on the way

- The as-found row for the 3 mm rest read `status: Done, stale: false,
  depends_on: [{id: null, kind: stock, state: broken}]`. `list_toolpaths`
  serves `rt.status` and `rt.stale_since` (`app/mcp/project.rs:53-77`),
  not `freshness()`. A core `drop_result` on the consumer leaves the view
  `rt.result` and `rt.status` in place. The edit history that produced the
  band-shaped result is not recoverable from code; the wire state was
  permitted by this gap.
- `generation_plan::plan` reads `has_snapshot`, never `EdgeState`
  (`session/generation_plan.rs:132-137, 152-156`). A Broken consumer is
  planned and generated.
- F.4 (`controller/events/simulation.rs:196-208`): when the first enabled
  op is `FromRemainingStock`, the prefix simulation emits an empty group
  and the snapshot is untouched stock. The core refusal text "no simulated
  remaining-stock snapshot" (`session/compute.rs:2081-2094`) never sees
  the case. Design comment and refusal text disagree.
- `set_toolpath_heights(bottom_z 4.0)` on the 6 mm rough raised
  `geom.bottom_above_top_z`: "Bottom Z (4.0) is above Top Z (0.1)". The
  static check resolves Top Z as 0.1 on a stock whose top is 23. A false
  critical banner.
- The MCP door applies core commands but does not mirror
  `simulation_cleared` (`app/mcp.rs:361-363`, named in
  `state/freshness.rs:122-124`). After an MCP disable the view's
  `prior_stocks` outlive the core's.

## 7. Rulings needed

R1. Silhouette holes. Three fixes: (a) `model_silhouette` emits outer
loops only; (b) adaptive3d's pre-clip treats a silhouette hole as
material to the stock bottom and the post-clip treats it as inside;
(c) a new boundary source "Model outline, holes filled" and the
silhouette stays as is. Recommendation: (a). A keep-out is the mechanism
for "do not cut here"; a through-hole is material. `adaptive3d_boundary_clear_parity`
and the `model_silhouette` tests (`boundary.rs:1015-1160`) pin today's
behaviour and move with it.

R2. Outer faces. `center` containment against the silhouette can never
reach an outer face. Either the silhouette boundary offsets by the tool
radius for finishing families, or the default boundary for a rough and a
waterline is `stock`. Recommendation: default `stock` for adaptive3d and
waterline; silhouette stays an opt-in.

R3. Waterline Auto ladder. Auto top → model top or stock top; Auto bottom
→ model bottom or stock bottom. `HeightReference::ModelTop/ModelBottom`
exist (`compute/config.rs:194-200`). Recommendation: model top to model
bottom, and the empty gate refuses a one-level ladder on a non-rest op.

R4. A level at the stock bottom. The waterline at Z 0 cuts the bed. Either
the ladder floors at `stock_bottom + leave`, or the operator pins the
bottom. Recommendation: floor at the stock bottom plus `stock_to_leave`,
and a finding when the floor clamps.

R5. Heights diagram. A straight defect: coincident lines pick Top, and a
drag pins a height with no confirmation and no visible "pinned" mark on
the row. Recommendation: prefer Bottom on a tie, round the drag result to
0.1 mm, and show the pin on the row and the tab badge.

R6. Flat-floor level in the rough ladder. Snap a level to a flat shelf
(`detect_flat_areas` at exact Z, not bin centre), or document that the
last level clamps to the floor and stop the narration calling it "Z 0.5".

R7. The false `geom.bottom_above_top_z` banner (§6). Trace the static
check's `HeightContext`.

R8. F.4 versus the core refusal (§6). One text.

R9. Push cutter on vertical facets (§4.5). A defect: `facet_push` must
push the side of the cutter against a vertical facet interior, or the
waterline must refuse a mesh with vertical faces. Recommendation: push
them; add a box fixture to the waterline tests.

R10. Ramp entry on a finishing contour (§4.5). A 3° ramp folded along a
2 mm run is a saw. Recommendation: the waterline default is a plunge
outside material or an arc lead-in, and the fold refuses a run shorter
than one ramp length.

R11. Export safety check frame (§4.6). Compare in one frame.

## 8. Edits made to the live GUI project

The file on disk is untouched. The GUI session now differs from it:

| Toolpath | Edit |
|---|---|
| 0 "3D Rough 1" (6 mm) | enabled; `clearing_strategy` agent_search → contour_parallel; boundary silhouette → `stock`; `bottom_z` pinned 4.0 |
| 1 "3D Rough 1 (copy)" (3 mm) | boundary silhouette → `stock` |
| 2 "Waterline 3" | `top_z` −1.37 → 18.0, `bottom_z` Auto → 0.0 |

The `bottom_z` 4.0 pin on the 6 mm was for the picture. Without it the
last level clamps to 4.0 on the floor and to 0.5 in the hexes and outside
the walls, which is the better recipe: the 6 mm takes the outer band and
the 3 mm takes only the hex corners. With the pin the 3 mm took the whole
outer band (71 m of cutting).

A reload of `~/corne_job.toml` restores the as-found state, including the
−1.37 pin. The as-found copy is beside this file.

## 9. Recipe that works today

1. Both roughs: boundary source `stock`, containment `center`.
2. 6 mm rough: `contour_parallel`, heights Auto.
3. 3 mm rough: `from_remaining_stock`, heights Auto.
4. Waterline: `top_z` pinned to 18, `bottom_z` pinned to 0.5 (not 0).
   Boundary `stock` if the outer faces are wanted.
5. Re-fit the stock to the model (§1 footnote) before anything else; it
   halves the roughing time.

## 10. Landed 2026-09-18 (commits `ea3fa221..dba92e8c`, not pushed)

| Ruling | Commit | What moved |
|---|---|---|
| R9 | `ea3fa221` | The real cause of §4.5 was the edge SAMPLER in the push cutter (nine samples over the whole edge; a 6 mm contact window fell between samples at the hex-row pitch). Fixed by sampling the analytic contact window. The vertical-facet skip was a latent second hole, also closed. Pre-R9 every long or sloped edge under-blocked: the golden dome waterline CL was 310 mm against 385 mm now. |
| R1, R2 | `ab2e5e9e` | The machining boundary from a silhouette is the outer loop only (`silhouette_machining_outline`). A new 3D op gets silhouette + one tool diameter (a stored number). |
| R3, R4, R7 | `3bd47ef8` | Auto bottom on a zero-depth op is the model bottom; the waterline ladder nudges off flat faces and drops levels at or below the stock bottom with a finding. The "Top Z 0.1" banner was true: the 6 mm rough carried a second diagram-drag pin. |
| R10 | `7b65e58e` | Finish and SemiFinish roles create with no entry ramp; a fold laps a run three times at most. |
| R11 | `d25ca61d` | The export safety check compares against the emitted retract plane. |
| R5 | `070b6df9` | Tie-break prefers Bottom; a drag rounds to 0.1 mm on release; the row shows "(pinned)" with an Auto reset; the Heights badge reads INFO. |
| R12 | `5588502d` | A dimension edit on the Stock panel clears "Auto from model"; the tick refits. |

Gates after `dba92e8c`: core lib 2534/0, viz 942/0 over 86 binaries, cli
54/0, clippy and fmt clean. Goldens re-blessed with the causes named
(`67cd171e`, `dba92e8c`).

CLI proof on a corrected copy of the job (pins reset to Auto, silhouette
offset = tool diameter, waterline entry none, resolution 0.3): three
toolpaths generate, 0 collisions; the waterline ladders 18 → 1 (the level
at 0 dropped), 32 752 cutting moves, and ZERO cutting moves inside the
right wall below Z 17.5. The wall saw is gone. The GUI look waits for a
restart of the MCP GUI on the new release binary.

Open after landing: the R10 lap cap also reaches roughing (a Zigzag rough
entry that would lap more than three times now plunges into fresh stock);
a waterline on a solid emits loops inside material; `waterline_contours`
returns open arcs when the bbox extent divides the sampling evenly (the
grid is padded now, the chain builder is not fixed); UnifiedFinish's
VerySteep band lacks the stock floor and the flat-face nudge.

GUI look on the new binary (`14_gui_simulation_after_fixes.png`, project
`corne_job_corrected.toml`, `generate_all` at 0.3 mm, 6 steps, 3
simulations): the walls stand all round, a one-tool-diameter moat around
the outer faces, the far stock untouched, every hex and screw hole cut,
0 collisions, 59:39 machine time.
