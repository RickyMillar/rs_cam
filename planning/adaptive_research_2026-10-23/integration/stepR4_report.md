# Removal step R4: the AgentSearch and ContourSpiral planners leave

Date: 2026-10-28. Branch `geodesic-integration`, base `951f1bb13` (R3).
The implementer did not commit. Design: `removal_design.md` §1.2, §1.5, §5.

Patch: `stepR4_tree_vs_951f1bb13.patch`, md5
`8d5d16ccb3ca91ddd9667b3c56e4eac2`. 22 file entries (20 tracked, +142/-2061,
and two new goldens). No planning file. `git apply --check -R` passes.

## 1. Product code

| Place | Change |
|---|---|
| `adaptive3d/mod.rs` | `ClearingStrategy3d` keeps `ContourParallel` and `GeodesicAdaptive`; the enum doc records R3/R4 and that the slice pipeline and the 2D agent planner stay (the F-038 forecast reads the agent planner); the effective-rule arms shrink |
| `adaptive3d/path.rs` | the dispatch arm is `GeodesicAdaptive` alone, into the same slice dispatch |
| `adaptive3d/clearing.rs` | the slice's 2D params set `PathStrategy2d::Agent` (the value the geodesic door sets; ContourSpiral took the spiral). No other line: the slice pipeline, `is_geodesic` gates and the forecast stay for R5 |

The 2D engine (`adaptive/`) and the 2D Adaptive op's `path_strategy`
(Agent, ContourSpiral) are not touched.

## 2. Goldens (byte parity)

`agent_search.txt` (380 lines) and `contour_spiral.txt` (997) are deleted.
Their dial coverage moves to two new cases, each blessed alone:

| New case | Dials | Moves | Caps |
|---|---|---|---|
| `geodesic_adaptive_boundary` | `boundary` (the old AgentSearch rectangle), stay-down 8 mm, `min_region_cut_length_mm` 3 | 901 | 149 |
| `contour_parallel_by_area` | `RegionOrdering::ByArea` | 393 (plain CP: 433, so the dial acts) | 0 |

`trochoid_cap_mult` (the other ContourSpiral dial) is read by no strategy
left. `contour_parallel`, `contour_parallel_flat_blend` and
`geodesic_adaptive` are unchanged (`git status`: two deletions, two
additions). The test doc's bless log records it.

## 3. Tests

Retargeted to Geodesic Adaptive (the claim is about the slice pipeline,
which Geodesic Adaptive runs):

| Test | Result | Note |
|---|---|---|
| `adaptive3d/tests.rs`: `planner_sim_dexel_parity_geodesic_adaptive`, `..._world_stock_declared` (were `..._agent_search`), the flat and cut-only probes (ignored), the Z-drop diagnostic and the spacing probe (renamed) | 37 ok, 7 ignored | the parity bars hold on Geodesic Adaptive, the world-stock arm included (`sim_higher` 0) |
| `traced_adaptive3d_emits_spans_hotspots_and_annotations` | ok after a re-scope | the `adaptive_pass` spans and hotspots came from the 2D agent walker planning AgentSearch regions; Geodesic Adaptive plans with its own field, so no 3D strategy emits them now (the 2D op does). The test asserts the `agent2d_region` slice spans instead, with Z-level, heightmap and annotation checks unchanged |
| the D65 capped-split test | ok | "the other strategy" is CP alone |
| `adaptive3d_boundary_clear_parity` | 4/4 after one per-strategy pin | §3.1 |
| `adaptive3d_by_area_cells_confine_jobs`, `adaptive3d_by_area_matches_global_stock`, `adaptive3d_global_gate_drapes_below_floor` | ok | Geodesic Adaptive replaces ContourSpiral / AgentSearch in the strategy lists |
| `adaptive3d_entry_coalescing_f038` | ok, 118 s (debug) | AgentSearch → Geodesic Adaptive |
| `adaptive3d_keep_down_link_f038b` | 4/4, 40 s | the same |
| `adaptive3d_planner_never_ahead_of_emitted_path` | 2/2, 87 s | the same (the session arm moved in R2) |
| `agent_search_coverage` | ok, 14 s | the same; the file name keeps its history |
| `wanaka_z_layer_render` (ignored evidence) | not run | Back Rough is Geodesic Adaptive |
| `geodesic_adaptive_holds_the_pass_load_c12` | 2 plain ok | the printed control is Contour Parallel (was AgentSearch); the heavy judge is not re-run (its asserted arm did not change) |
| `param_sweep` (ignored harness) | — | the strategy sweep is CP against Geodesic; `sweep_adaptive3d_engagement_measure` (the AgentSearch sub-pass measure) is deleted, as no strategy left reads the measure |

Deleted: `agent_search_axial_doc.rs`, an ignored instrument of the
AgentSearch Back Rough (`wanaka100/README.md` says so).

TIME: three gate binaries are slower on Geodesic Adaptive in debug:
F-038 coalescing 118 s, never-ahead 87 s, keep-down link 40 s (each was
seconds to about 20 s on AgentSearch). The lead may want them in the
"name one binary and ask" class.

### 3.1 One pin per strategy (`adaptive3d_boundary_clear_parity`, bar B2)

B2 pins the exact count of emitted cutting endpoints fully outside the
containment (the `waterline_cleanup` leak, D4 F-B): 135, "identical on
both strategies". On Geodesic Adaptive the count is 312. B1, B3 and B4 hold
(probe: the pin set to 312 for one run, then restored). The cause: on
Geodesic Adaptive the waterline cleanup runs on other contours (it skips
the pieces the arm leaves, and the arm's regions follow the outer-band
clip). The pin is now per strategy: `WATERLINE_LEAK_CUT_MOVES = 135` (CP,
unchanged) and `WATERLINE_LEAK_CUT_MOVES_GEODESIC = 312`, dated, with the
cause. A drop to 0 still means F-B landed. Geodesic Adaptive leaks 2.3 times
as many waterline endpoints outside the boundary as CP on this fixture: a
finding for the D4 package, not a defect of R4.

## 4. Gates

| Gate | Result |
|---|---|
| `cargo fmt --all -- --check`, clippy workspace all targets, full features | ok |
| `adaptive3d_emission_byte_parity` | 6/6 (CP ×3, Geodesic ×2, determinism) |
| `every_op_keeps_its_toolpath_g_opgolden` | 1/1, no bless |
| the retargeted sentries above, plan-sim gap, entry stock, pieces, the s7_4 pins, the cap census, the C12 judge (plain), D44 ×2, D48, D50, the volumetric wrapper, `folder_maps_name_real_files_f140`, `every_ignore_states_its_kind_f152` | all ok |
| core `--lib` filters `adaptive3d`, `adaptive::` | 149 ok, 7 ignored |

The 2D adaptive sentries were not run: no file of `adaptive/` changed.

## 5. Next

R5: the slice pipeline keeps only its geodesic branches (`is_geodesic`
gates collapse; the 3D planner-engagement plumbing goes), Geodesic output
byte-identical.
