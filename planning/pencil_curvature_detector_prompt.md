# Build a literature-faithful curvature valley-line detector for pencil finishing

Implement **ridge/valley line extraction from surface curvature** as the pencil
detector — faithfully following the published method, with NO bolt-on heuristic
gates. The goal is dense coverage of every concave valley/seam in the relief,
**filterable by a single saliency/significance threshold**.

## Why (read this — it's the whole point)

Prior attempts FAILED and the user is (rightly) done with hacks:

- **Dihedral crease detector** (`PencilDetector::Dihedral`, the original in
  `crates/rs_cam_core/src/pencil.rs::compute_shared_edges`) is a crude
  approximation of bitangent two-point-contact pencil tracing. It fragments into
  incoherent short chains on the noisy 661k-tri rivermap mesh.
- **Drainage/flow-accumulation detector** (`PencilDetector::Drainage` in
  `crates/rs_cam_core/src/valley_network.rs`, UNCOMMITTED) uses real GIS
  algorithms (priority-flood/Barnes 2014, MFD/Freeman, flow-accumulation) but is
  the WRONG literature (hydrology, not CAM pencil) and only finds the sparse
  *major drainage trunks*, not every hill valley. To fight artifacts it grew a
  pile of non-literature heuristics — wide box-smoothing, distance-transform
  width gate, slope/incision gates, border trim. That hand-tuned stack is why it
  "looks off." **Do not extend it.** Leave it behind its flag; it is not the path.

The user's exact ask: *a real, faithful literature implementation* that traces
**every concave seam** (what "pencil" means) and lets them **filter for the ones
they want** (deep/sharp valleys vs all of them).

## The faithful method

Curvature-based crest-line extraction. Primary references:
- **Ohtake, Belyaev & Seidel 2004**, "Ridge-Valley Lines on Meshes via Implicit
  Surface Fitting," ACM TOG 23(3):609-612.
- **Yoshizawa, Belyaev & Seidel 2005**, "Fast and Robust Detection of Crest
  Lines on Meshes," ACM SPM, 227-232.
- **Rusinkiewicz 2004**, "Estimating Curvatures and Their Derivatives on
  Triangle Meshes," 3DPVT — the standard per-vertex curvature estimator.

Algorithm:
1. **Per-vertex principal curvatures** κ1 ≥ κ2 with principal directions t1, t2,
   via Rusinkiewicz 2004 (per-face second fundamental form, area/angle-weighted
   to vertices). FIRST search the repo for existing curvature code to reuse
   (`rg -i "curvature|principal|second fundamental|shape operator"`,
   `codebase_search`); scallop/enriched_mesh may already estimate it.
2. **Curvature derivatives** (the 2-3-1 tensor of Rusinkiewicz) to get the
   directional derivative of κ2 along t2 — the "extremality" e = ∂κ2/∂t2.
3. **Valley lines** = zero-crossings of e (extremality) on mesh edges, kept only
   where κ2 < 0 (concave) and the line is a MAXIMUM of |κ2| in t2 (valley, not
   ridge). March per triangle: find the edge zero-crossings of e, connect the
   two crossing points into a segment; chain segments across triangles into
   polylines.
4. **Saliency filter = THE significance dial.** Rank/threshold lines by Yoshizawa
   "ridgeness/valleyness" — the curvature integral ∫|κ2| ds along the line (or a
   simpler |κ2| threshold to start). High threshold → only the sharp deep
   valleys; low → all of them. This is the user's "filter for that." Expose it as
   the main param.
5. **Noise robustness the literature way:** smooth the curvature TENSOR field /
   use the implicit-fit or local-polynomial-fit smoothing from OBS/Yoshizawa —
   NOT a DEM box-blur. Smoothing curvature, not geometry, is the faithful denoise.

## Integration

- New `PencilDetector::Curvature` (or `RidgeValley`) variant alongside
  `Dihedral`/`Drainage` in `pencil.rs`. Output `Vec<Vec<P3>>` valley centreline
  polylines feeding the EXISTING downstream pipeline unchanged: fair
  (`fair_polyline_xy`) → drop-cutter lift (`lift_to_surface`) → offset → hookup
  link → emit. Keep the reference-tool rest gate (`polyline_passes_depth`) as an
  optional secondary filter.
- New module e.g. `crates/rs_cam_core/src/crest_lines.rs`. Wire params through
  `PencilParams`, `compute/operation_configs.rs` (PencilConfig + serde defaults),
  `compute/catalog.rs` (PENCIL_PARAMS), `compute/execute.rs`, and ALL PencilParams
  test literals (`rg "PencilParams \{"`; there are sites in pencil.rs tests,
  tests/param_sweep.rs, tests/capability_link_moves_safety.rs).
- Main param: `valley_saliency` (or curvature threshold). Keep it small and
  intuitive — the user wants ONE filter dial.

## Validation — DO NOT lean on unit tests (explicit user instruction)

The user demanded: *"validate the paths much more than just testing. the test
code is not a fallback here."* Validate by RENDERING ACTUAL PATHS and looking:

- Reuse/extend the render harnesses in `pencil.rs` (`render_pencil_real_mesh`,
  `#[ignore]`, env `RS_CAM_PENCIL_*`) and `valley_network.rs`
  (`render_drainage_debug`, which **hillshades the terrain (slope-shaded) and
  overlays the detected centrelines in green** — this is the key view; top-down
  Z-colouring CANNOT distinguish wall from valley on slopes, hillshade can).
  Build an equivalent overlay for the curvature detector and `xdg-open` it.
- Fixture: real mesh `/home/ricky/Downloads/wanaka200/rivmap_export/terrain.stl`
  (661k tris) and bundled `crates/rs_cam_core/tests/fixtures/terrain.stl`. It is
  rolling-hills relief with many valley grooves + one low basin/flat corner (a
  lake the user wants excludable — saliency filtering should naturally drop it
  since a flat basin has ~zero curvature).
- Then validate LIVE in the 3D GUI on wanaka200 via the `rs-cam` MCP:
  `load_project /home/ricky/Downloads/wanaka200/wanaka200.toml`, pencil op is
  **index 8** (tool = tapered ball 2mm tip). `set_toolpath_param` detector +
  saliency, `generate_toolpath 8`, `screenshot_toolpath`. The user rebuilds the
  GUI binary themselves; the running GUI uses the OLD binary until they restart.
- The bar: green lines must sit IN the valley grooves, densely covering the hill
  valleys, and the saliency dial must visibly thin the set from "all seams" to
  "only deep sharp valleys."

## Constraints (durable, from CLAUDE.md + session)

- ONE cargo job at a time: check `free -g` + `pgrep -af "[c]argo (check|test|build)"`
  (bracket a char to avoid self-match) before launching; concurrent heavy cargo
  thrashes swap and crashes the machine. A `cargo build --release` of the GUI by
  the user counts — yield to it.
- Zero-warning clippy: `cargo clippy --workspace --all-targets -- -D warnings`
  (16 deny lints incl. unwrap_used, expect_used, panic, indexing_slicing,
  print_stdout/stderr). Use `#[allow]` + `// SAFETY:` only for provably-safe
  bounded indexing. Test modules are exempt for unwrap/panic but NOT for
  expect_used/indexing_slicing without an explicit allow.
- Per-crate tests only (`cargo test -p rs_cam_core --lib`), never workspace-wide.
- `cargo fmt` cascades into `strategy_advisor.rs`/`strategy_advisor_smoke.rs`
  (pre-existing debt) — `git checkout --` revert those before any commit.
- Commit/push ONLY when the user asks. End commit messages with
  `Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>`. wanaka200.toml live
  params are LIVE-ONLY — don't save them.

## Uncommitted state you inherit

Working tree (branch `experiment/adaptive-spiral`) has the UNCOMMITTED drainage
work: `valley_network.rs` (new), `pencil.rs` (PencilDetector enum + Drainage
branch + `paths_from_sampled`/`resample_polyline`/`polyline_passes_depth` helpers),
and the param wiring in operation_configs/catalog/execute + test literals. All of
that compiles and is clippy-clean. Decide with the user whether to keep the
drainage variant behind its flag or strip it; either way, the curvature detector
is the new real one. The prior pencil improvements (fairing, hookup linking,
bisector, reference-tool rest gate) are COMMITTED (bf4e95d, 9077eea, 20733c8) and
stay.

Memory: see `project_multitool_finishing` — Phase 2 (pencil rebuild) of the
`planning/finishing_speedup_project.md` umbrella.
