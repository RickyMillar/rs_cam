# D48: G-RESTFRAME, the published rest regions leave the source op's boundary

Tree: worktree `adaptive-head`, base `13caf4b29` (D47 committed). Not committed.
Patch: `tree_vs_13caf4b29.patch`, md5 `5ef1cc8bb134e991616dbe4053c6ee08`. It is separate from the D47 patch.

## 1. The space of a left piece

All line numbers are in the base tree.

- `ClearZLevelContext::research_left_pieces` (`adaptive3d/clearing.rs:470`) holds `(z_level, polygon)` pairs. `research_leave_piece` (`clearing.rs:3276`) pushes them from two sites:
  - `clearing.rs:3339` ("no field") pushes `polygon_for_adaptive`, the outline that the 2D planner plans;
  - `clearing.rs:4146` ("F-038 forecast") pushes `region_polygon`, the level region itself.
- A level region is made of planner-stock cells (`build_material_bool_grid`, `clearing.rs:487`). Two different rules set its edge:
  - At a model wall, the edge is where the drop-cutter surface (`SurfaceHeightmap`, tool-tip CL heights) rises above the level. This edge is in tool-centre space.
  - At the boundary, the research clip (`ResearchClip::restrict_regions`, `research_outer_band.rs:126`) keeps the cells whose centre is inside `geometry.boundary`. `geometry.boundary` is the RAW boundary polygon (`session/compute.rs:1382`, `.boundary(inputs.pre_boundary)`), which is the silhouette and not its inset. Also, `boundary_reach` (`research_outer_band.rs:96`) keeps stock out to one radius past it. So at the boundary the region reaches the boundary line itself, which is the material line.
- The 2D planner insets every outline by the tool radius again (`adaptive::inset_piece_outlines`, `planner_outlines`; the adaptive3d folder note: "A region is in tool-centre space; the 2D planner insets it again"). Thus op 1's tool centres stop one radius inside the boundary line (wanaka: x 23.01 to 216.99 on the outline x 20 to 220).
- D44 publishes the union of the pieces, grown by one planner cell (`research_outer_band.rs:74` `left_regions`, called at `adaptive3d/mod.rs:1410`), and does not change the space. At the boundary the published region reaches the boundary line plus one cell (wanaka: 19.56 to 220.44, that is the outline grown by 0.44 mm). A `Center`, offset 0 consumer puts its tool centre there, so its tool cuts up to one radius past op 1's boundary.

The D44 note says the published regions are in tool-centre space. This is true at a model wall and false at the boundary.

## 2. Fix: one space, tool-centre positions inside the source op's containment

- `left_regions` takes the run's own containment polygons (`Adaptive3dGeometry::centre_boundary`, `adaptive3d/mod.rs:274`). These are the boundary inset by the radius for `Inside`, outset for `Outside`, and as is for `Center`. `left_regions` intersects the grown union with them. An empty `centre_boundary` (no boundary) leaves the union unchanged.
- `Produced::rest_regions` (`trace/produced.rs`) now states the space of every producer: TOOL-CENTRE positions, for a `Center` consumer, in XY only.
- Pencil `RestDepth` uses the same space. The rest field's mask is made of cells of the fine tool's drop-cutter (centre) surface, grown by the tip radius plus a margin, "so a boundary-clipped fine-tool op can reach the region edge" (`surface/rest_field.rs:951`, `machining_region_polygons`). The rest field has no boundary of its own to respect.
- Limit: the guarantee holds for a consumer tool NO LARGER than the source op's tool. With a larger tool and `Center` containment, the consumer's disc can cross by the difference of the radii. A consumer with `Inside` containment insets the regions by its own radius, so it is always inside. A future step can carry the source boundary to the consumer, so that the consumer can apply its own radius. That step is not in this patch.
- Shipped output: the toolpath does not change. Only the published regions change, and `run_volumetric` drops them when no consumer names the op. So a job with no consumer is byte-identical (parity 5/5 plain and with research).

Sentry: `crates/rs_cam_core/tests/a_rest_consumer_stays_inside_the_source_boundary_d48.rs` (research and test-support; arm env; `--ignored`, as the D44 sentries are). In this fixture the D44 channel runs to the mesh edge. Op 1 has a model-silhouette boundary with `Inside` containment. The consumer is a CP rough on `DerivedRestRegions`, with `Center` containment, offset 0, and the same 6 mm tool. The test checks that every cutting-move end of the consumer is inside op 1's tool-centre polygons, so the tool disc stays inside the silhouette. The slack is op 1's own tolerance, and op 1's boundary is resolved as generation resolves it. The test first asserts that a region reaches the boundary, so that the test cannot pass on a fixture without the defect.

- Red on the base producer code (`sentry_red_on_13caf4b29.txt`): the region reaches 40.75 mm against the silhouette at 40.25 mm. 78 move ends cross, worst 3.447 mm (wanaka: about 3.5 mm).
- Green with the patch: the region reaches 37.25 mm (the silhouette inset by 3 mm), and no move end crosses (0 of 51 cutting moves).

## 3. Levels: not done, a proposal

The region model cannot carry a Z range today:
- `Produced::rest_regions` is `Arc<Vec<Polygon2>>`.
- The consumer resolves it through `RegionSet` (`session/compute/generation.rs:384-421`) into XY containment polygons (`boundary_centre`) and the XY post-generation clip.
- The consumer's level plan (`adaptive3d/path.rs`, `step_levels`) reads only its own heights and depth.

No field and no input carries a Z from the source to the consumer's level loop. So the consumer plans every level over the one XY union.

The producer already has the data: each left piece keeps its level (`research_left_pieces: Vec<(f64, Polygon2)>`), and `left_regions` drops the Z when it takes the union. The proposal takes no new number:
1. Publish the pieces by level. Add a new `Produced` field next to `rest_regions`, for example `rest_region_levels: Option<Arc<Vec<(f64, Polygon2)>>>`, with the same clip to `centre_boundary`. The producer's own level list gives each Z.
2. The consumer's adaptive3d level loop takes, at its level z, the union of the source pieces of the source levels at or below z, down to the next source level. The mapping comes from the two level lists, so it adds no constant. A level with an empty union plans nothing, and this removes the pecks at levels where no piece exists (fix33: 461 plunges for 110 mm of cut at Z −10.92).
3. A smaller first step needs no per-level mask: limit the consumer's levels to the Z span of the pieces (from the level above the highest piece level, down to the lowest piece level). This removes the levels outside the span only.

Item 2 is a new input to the adaptive3d planner, a per-level region mask. That is a change in the planner and the format, so the lead must decide on it before anyone starts it.

## Gates (all through `scripts/cargo_lane.sh`, in the foreground)

| Gate | Result |
|---|---|
| `cargo fmt --all -- --check` | ok |
| clippy, full features with `research` | ok |
| clippy, without `research` | ok |
| `adaptive3d_emission_byte_parity`, plain and research | 5/5 and 5/5 |
| `a_rough_publishes_the_regions_it_leaves_d44` (arm env) | 1/1 |
| `a_derived_rest_rough_cuts_only_the_leftovers_d44` (arm env) | 1/1 |
| `a_rest_consumer_stays_inside_the_source_boundary_d48` (arm env) | 1/1 (red before the fix, see above) |
| `a_skipped_op_does_not_stale_the_export_d47`, plain and research | 1/1 and 1/1 |
| `rs_cam_core --lib`, plain and research | 2698 and 2749 ok |
| `folder_maps_name_real_files_f140` | 1 red that existed before this change: `every_rs_file_is_on_a_map_f140` lists `adaptive/slab_field.rs`, `adaptive3d/keep_down_route.rs` and `adaptive3d/research_outer_band.rs`. None of them is on a folder map at the base, and this patch removes no name. Not fixed here. |

Viz and CLI are not run. The patch changes only `rs_cam_core`, and no viz or CLI code reads the changed function.
