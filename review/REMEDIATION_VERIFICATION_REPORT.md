# Remediation Verification Report

## Scope

- Remediation range reviewed: `8874408..fdbe84f`
- Pre-remediation baseline used for spot checks: `59782f1`
- Sources used: `review/FINDINGS.md`, `planning/REMEDIATION_TRACKER.md`, remediation commits, current `HEAD`
- Deliverable scope: report only; no code or tracker edits
- `E` and `F` findings were unlabeled in `review/FINDINGS.md`; this report assigns synthetic IDs `E1-E29` and `F1-F20` so every original finding has a stable row

## Checks Run

| Check | Result | Notes |
|---|---|---|
| `cargo fmt --check` | Failed | Formatting drift remains in remediation-touched files; this report treats that as evidence, not something to rewrite |
| `cargo test -q` | Passed | Workspace tests passed |
| `cargo clippy --workspace --all-targets -- -D warnings` | Passed | No warnings at current `HEAD` |
| `cargo test -p rs_cam_viz controller::tests::` | Passed | Focused controller regression suite passed |
| `cargo test -p rs_cam_viz compute::worker::tests::` | Passed | Focused compute-worker suite passed |
| `cargo run -p rs_cam_cli -- job fixtures/demo_job.toml` | Passed | Demo job smoke run completed |

Checks legend used below:

- `B`: baseline suite above
- `CI`: direct code / diff inspection
- `TT`: targeted existing tests in the touched module

## Status Summary

| Status | Count |
|---|---:|
| Fixed correctly | 61 |
| Partially fixed | 32 |
| Still open | 55 |
| Deferred/out of scope | 3 |
| Unable to verify | 1 |
| Fix incorrect | 0 |
| Finding invalid/overstated | 0 |

Highest-risk unresolved or under-fixed items:

- `C1`: core-library `unwrap()` count is still high (`rg` currently finds 167 in `crates/rs_cam_core/src`)
- `C11`: `ResetSimulation` still clears state without cancelling in-flight compute
- `C12`: toolpath compute queue is still unbounded
- `C16`: nonexistent `ModelId(0)` fallback still exists when adding toolpaths
- `D1`: dense toolpaths are still rendered as 1-pixel lines; the remediation only added config/documentation
- `I1` / `I2`: tool-change and coolant support exists in the emitter, but CLI/Viz export still passes `tool_number: None` and `CoolantMode::Off`

Questionable or stale remediation claims:

- `A2`: the machining-output fix landed, but semantic-trace reconstruction still converts degrees to radians
- `A10`: the GUI still produces one inlay toolpath entry; the fix only inserted a retract separator
- `I4`: tracker notes partial DXF work, but current code handles `Line`, `Arc`, and `Spline` entities with tests

Cross-cutting smells exposed during this pass:

- Formatting drift is present across remediation-touched files even though clippy/tests pass
- Angle-unit handling is still inconsistent in remediated 2D raster paths (`Pocket`, `Zigzag`, and rest semantic reconstruction)
- Several “implemented” G-code features are only present in `rs_cam_core::gcode`; end-to-end export wiring is incomplete
- Direct UI state mutation still exists in `properties` paths, so the controller-routing cleanup is incomplete
- An unused `kiddo` workspace dependency remains in the root `Cargo.toml` even though `rs_cam_core/Cargo.toml` no longer uses it

## A. Confirmed Bugs

| ID | Original claim | Attempted? | Current status | Assessment of original finding | Evidence | Checks run | Extra smells / improvements | Recommended follow-up |
|---|---|---|---|---|---|---|---|---|
| A1 | CLI Adaptive3d job execution used a mismatched tool radius | Yes | Fixed correctly | Valid | `crates/rs_cam_cli/src/job.rs` now caches `tool_radius = cutter.radius()` and uses it for `Adaptive3dParams`; regression coverage exists in `job.rs` radius tests | B, CI, TT | None beyond normal CLI duplication | None |
| A2 | Rest machining GUI double-converted scan angle | Yes | Partially fixed | Valid | Actual compute path uses `angle: cfg.angle` in `operations_2d.rs`; `rest.rs` still documents/tests degrees. But semantic reconstruction in `RestConfig::generate_with_tracing` still calls `self.angle.to_radians()` | B, CI, TT | Same degrees/radians mistake also exists in remediated `Pocket`/`Zigzag` semantic path generation | Remove remaining `.to_radians()` calls where core APIs already expect degrees, then add a regression test at the viz compute layer |
| A3 | Tabs were applied to all profile depth passes | Yes | Fixed correctly | Valid | `run_profile` only calls `apply_tabs` when `is_final`; targeted test `profile_multi_pass_tabs_only_on_final_depth` covers it | B, CI, TT | No tab-on-holes coverage yet | Add one profile-hole regression when the dressup gap is addressed |
| A4 | Face `OneWay` vs `Zigzag` selection was ignored | Yes | Fixed correctly | Valid | `FaceConfig::generate_with_tracing` un-reverses odd scan rows when `FaceDirection::OneWay`; targeted test `Face OneWay produces unidirectional cuts` exists | B, CI, TT | None | None |
| A5 | Inlay male region ignored polygon holes | Yes | Fixed correctly | Valid | `crates/rs_cam_core/src/inlay.rs` now extends `holes` with `polygon.holes`; tests include `test_male_region_respects_holes` | B, CI, TT | None | None |
| A6 | `VCarve.max_depth = 0.0` clamped everything to zero | Yes | Fixed correctly | Valid | `vcarve_toolpath` now treats `max_depth <= 0.0` as unlimited; regression test `test_vcarve_max_depth_zero_means_unlimited` exists | B, CI, TT | None | None |
| A7 | VBit `edge_drop` could produce NaN on negative sqrt input | Yes | Fixed correctly | Valid | `ccu_sq.max(0.0).sqrt()` is now used; regression tests check no NaN near zero discriminant | B, CI, TT | None | None |
| A8 | Tapered-ball `edge_drop` could produce NaN on negative sqrt input | Yes | Fixed correctly | Valid | Same guard added in `tapered_ball.rs`; regression tests cover near-zero discriminant behavior | B, CI, TT | None | None |
| A9 | Mesh bbox was not recomputed after winding fix | Yes | Fixed correctly | Valid | `TriangleMesh::from_stl_scaled` and `from_stl_bytes` both recompute `mesh.bbox` after `fix_winding()` | B, CI | No dedicated malformed-winding regression fixture | Add a winding-fix fixture test if this area changes again |
| A10 | GUI merged inlay female and male output into one toolpath | Yes | Partially fixed | Valid | `run_inlay` still returns one `Toolpath`; the remediation only inserts a safe-Z separator and the test asserts separator rapids, not separate GUI entities | B, CI, TT | GUI users still cannot independently enable, inspect, or export male vs female passes | Split GUI inlay generation into two user-visible toolpath entries or explicit sub-artifacts |

## B. Unwired / Dead Features

| ID | Original claim | Attempted? | Current status | Assessment of original finding | Evidence | Checks run | Extra smells / improvements | Recommended follow-up |
|---|---|---|---|---|---|---|---|---|
| B1 | Pre/post G-code text was stored but never emitted | Yes | Fixed correctly | Valid | `crates/rs_cam_viz/src/io/export.rs` passes `pre_gcode` / `post_gcode` into `emit_gcode_phased`; core tests `test_pre_post_gcode_emitted_*` cover emission order | B, CI, TT | None | None |
| B2 | Undo only recorded `StockChange`; other undo action kinds were dead | Yes | Unable to verify | Valid | `properties/mod.rs` now snapshots and pushes `ToolChange`, `PostChange`, `MachineChange`, and `ToolpathParamChange`; controller undo/redo arms exist | B, CI | History stack tests cover generic stack behavior, but there is no end-to-end test for the newly wired undo kinds | Add controller/UI tests that exercise undo/redo for tool, post, machine, and toolpath-parameter edits |
| B3 | `auto_regen` / `stale_since` existed but nothing triggered regeneration | Yes | Partially fixed | Valid | Toolpath edits now set `stale_since`; `AppController::process_auto_regen()` submits stale toolpaths and `app.rs` calls it each frame | B, CI | No direct test proves debounce, submission, or coverage of all mutation paths | Add a controller test that edits a toolpath, advances time, and asserts compute submission |
| B4 | Profile “In Control” compensation never emitted `G41/G42` | No | Still open | Valid | `G41`/`G42` only appear in UI labels in `operations.rs`; no emission exists in `gcode.rs` or export code | B, CI | UI still advertises controller compensation | Either wire real compensation output or hide the mode |
| B5 | Deviation coloring mode existed but no deviation data was computed | No | Still open | Valid | Simulation output still sets `deviations: None`; playback copies `simulation.deviations` into `display_deviations`, so the cache stays empty | B, CI | `ByOperation` remains placeholder too | Compute deviation fields during simulation or hide the mode until data exists |
| B6 | `ToggleSimToolpath` / `RecalculateFeeds` events had empty handlers | Yes | Fixed correctly | Valid | Those event variants are gone from current `rs_cam_viz` source; there are no remaining empty handler arms | B, CI | Removal is fine, but user-facing replacement behavior is not documented | Document the replacement UX in the next UI doc pass |
| B7 | 3D adaptive parameters were not exposed in the GUI | Yes | Fixed correctly | Valid | `ui/properties/operations.rs` now draws `entry_style`, `detect_flat_areas`, `region_ordering`, `fine_stepdown`, `min_cutting_radius`, and `stock_to_leave_radial` | B, CI | No targeted UI tests | Add one serialization round-trip test for the newly exposed fields if they change again |
| B8 | `finishing_passes` existed in `PocketConfig` but not in the UI | Yes | Fixed correctly | Valid | `draw_pocket_params` now exposes `Finishing Passes` | B, CI | No targeted UI test | None |
| B9 | Workholding rigidity was hardcoded to `Medium` in the GUI | Yes | Fixed correctly | Valid | `properties/mod.rs` now renders a `workholding_rigidity` combo box and uses the selected value in feed calculation | B, CI | Change still mutates state directly in the UI layer | Move the remaining stock-property edits through controller events |
| B10 | `StockVizMode::ByOperation` was a placeholder | No | Still open | Valid | `app.rs` still calls `operation_placeholder_colors`; `ui/sim_diagnostics.rs` still labels the mode as placeholder-solid | B, CI | User-facing mode exists without real per-operation attribution | Hide or badge the mode until per-cell ownership exists |
| B11 | Dead `run_face()` implementation remained beside the semantic path | Yes | Fixed correctly | Valid | `rg` finds no `run_face(` in current `compute/worker/execute` | B, CI | None | None |
| B12 | Geo conversion helpers were dead production code | No | Still open | Valid | `to_geo_polygon` / `from_geo_polygon` are still only referenced in tests | B, CI | This is harmless but adds maintenance noise | Remove or justify with a real production caller |
| B13 | Waterline `continuous` flag was stored but unused | No | Still open | Valid | `continuous` exists in UI/state, but `waterline.rs` itself has no `continuous` usage | B, CI | UI still exposes a dead flag | Either wire the flag or remove it from UI/state |
| B14 | Scallop `stock_to_leave_radial` existed but was unused | No | Still open | Valid | `stock_to_leave_radial` appears in state/UI, but there is no usage in `crates/rs_cam_core/src/scallop.rs` | B, CI | Users can edit a parameter with no machining effect | Wire it into scallop generation or remove the control |
| B15 | CLI setup definitions had dead `face_up` / `z_rotation` fields | Yes | Fixed correctly | Valid | Current `SetupDef` only has `name` and `output`; the dead fields are gone | B, CI | Other dead CLI fields remain for shank/holder metadata | Decide whether to wire or remove the remaining dead CLI tool metadata |
| B16 | `NoPaths` / `NoEntities` error variants existed but were never raised | No | Still open | Valid | `SvgError::NoPaths` and `DxfError::NoEntities` remain defined, but `load_svg_data` / `load_dxf` still return `Ok(polygons)` without those empty-input errors | B, CI | This weakens diagnostics for empty imports | Raise explicit empty-input errors and add tests |

## C. Error Handling & Robustness

| ID | Original claim | Attempted? | Current status | Assessment of original finding | Evidence | Checks run | Extra smells / improvements | Recommended follow-up |
|---|---|---|---|---|---|---|---|---|
| C1 | Core library had many `unwrap()` calls in non-test code | No | Still open | Valid | `rg -n "unwrap\\(" crates/rs_cam_core/src | wc -l` currently returns `167` | B, CI | Some are likely test-only modules, but the volume is still high and includes library paths like `waterline.rs` | Do a fresh production-path unwrap audit instead of relying on the old count |
| C2 | Worker thread panics could kill a lane permanently | Yes | Fixed correctly | Valid | Both toolpath and analysis lanes are wrapped in `std::panic::catch_unwind` and reset lane state on panic | B, CI | No targeted panic-recovery regression test exists | Add a deterministic panic-injected worker test |
| C3 | `.expect("lane mutex poisoned")` caused cascade crashes after panics | Yes | Fixed correctly | Valid | Worker lane mutex access now uses `unwrap_or_else(|e| e.into_inner())` throughout the queue/phase paths | B, CI | Poisoned-lock handling is fixed here, but debug/semantic recorder mutexes still use `expect("... poisoned")` | Keep lane recovery as-is and separately audit recorder poison paths |
| C4 | STL parsing lacked triangle-index bounds validation | Yes | Fixed correctly | Valid | `TriangleMesh::from_stl_scaled` and `from_stl_bytes` return `MeshError::IndexOutOfBounds` before building faces | B, CI | No malformed STL fixture test | Add a tiny malformed-index byte fixture when practical |
| C5 | Tool deletion could orphan toolpath references | Yes | Fixed correctly | Valid | `handle_remove_tool` blocks deletion when any toolpath still references the tool; controller test `remove_tool_blocked_when_toolpath_references_it` passes | B, CI, TT | UX is warning-only; no guided cascade/remove flow exists | Decide whether warning-only is sufficient or add a user choice dialog later |
| C6 | Zero / tiny divisors were unguarded in dexel/simulation hot paths | Yes | Fixed correctly | Valid | `DexelGrid::*_from_bounds` clamps tiny cell sizes; `dexel_stock.rs` now guards sample step and zero-length segments | B, CI | This fix is spread across several files and could regress quietly | Keep targeted clamp tests near these helpers |
| C7 | 2D ops returned `String` while 3D ops returned `ComputeError` | Yes | Partially fixed | Valid | Public compute boundary now returns `ComputeError`, but internal 2D helpers like `run_pocket` / `run_profile` still use `Result<Toolpath, String>` | B, CI | Error-shape inconsistency still exists below the semantic-op boundary | Finish migrating internal 2D helpers to `ComputeError` |
| C8 | NaN / Inf guarding in geometry code was minimal | No | Still open | Valid | Remediation touched a few sqrt sites, but there is still no systematic finite-value validation layer | B, CI | The new angle-unit inconsistency in raster code increases floating-point risk | Add boundary validation helpers and targeted NaN/Inf tests |
| C9 | `simulation.rs` had `.last().unwrap()` / `.first().unwrap()` on potentially empty vectors | No direct task | Fixed correctly | Valid | Current `simulation.rs` production code no longer has those unwraps; current matches are in tests only | B, CI | This was fixed indirectly and not tracked | None |
| C10 | Project saves were not atomic | Yes | Fixed correctly | Valid | `save_project` now writes `tmp_path` then renames; `test_save_project_atomic_write` exists | B, CI, TT | Rename semantics on cross-filesystem saves are still OS-dependent, but good for same-dir writes | None |
| C11 | `ResetSimulation` did not cancel in-flight compute | No | Still open | Valid | `handle_reset_simulation` still clears state without calling `cancel_lane` / `cancel_all` | B, CI | Stale simulation results can still race back into cleared state | Cancel analysis work inside reset before clearing state |
| C12 | Toolpath compute queue had no backpressure / cap | No | Still open | Valid | Toolpath lane uses an unbounded `VecDeque`; duplicate coalescing exists, but there is still no hard cap | B, CI | Heavy bursts can still build long queues | Add a queue bound or explicit drop/replace policy |
| C13 | Dressup errors were silently swallowed | No | Still open | Valid | Worker send path still ignores failures; feed optimization still warns and continues instead of surfacing strong errors | B, CI | Error handling remains inconsistent across dressups | Convert recoverable dressup failures into surfaced `ComputeError` / warnings with UI visibility |
| C14 | Scallop lacked tool-type pre-validation | Yes | Fixed correctly | Valid | Compute-layer test `scallop_rejects_non_ballnose_tool` exists and the semantic path rejects non-ball tools | B, CI, TT | None | None |
| C15 | Polygon hole re-pairing silently attached to the first polygon on failure | No | Still open | Valid | No remediation touched `polygon.rs`; the fallback behavior remains | B, CI | This can hide corrupted containment results | Surface a warning/error instead of silently attaching |
| C16 | Missing tool/model defaults silently fell back to nonexistent IDs | Yes | Partially fixed | Valid | Tool-add now validates missing tools, but `handle_add_toolpath` and another compute path still use `.unwrap_or(ModelId(0))` | B, CI | The tracker overstates this as fixed | Remove `ModelId(0)` fallbacks and block creation when no model exists |
| C17 | `waterline.rs` still had library-path unwraps | No | Still open | Valid | `chain_contours` still uses `chain.last().unwrap()` in library code | B, CI | These unwraps survive despite the earlier “unwrap cleanup” narrative | Replace with explicit empty-chain handling |
| C18 | `result_tx.send()` failures were silently dropped | No | Still open | Valid | Worker lane still uses `let _ = result_tx.send(...)` in both compute and analysis paths | B, CI | Result-channel failures can hide backend shutdown issues | Log or surface channel-send failures |
| C19 | Preset TOML parsing used brittle manual string slicing | No | Still open | Valid | `io/presets.rs` still uses `extract_field` / `extract_multiline_field` string slicing instead of parsing TOML | B, CI | Whitespace/comments can still break extraction | Parse with `toml` instead of string scanning |
| C20 | Profile tab placement assumed closed polygons | No | Still open | Valid | No remediation touched `profile.rs` open-contour tab logic | B, CI | The new “tabs only on final pass” fix did not address open contours | Add open-contour guards or reject tabs on open profiles |
| C21 | CLI overwrote output files without `--force` | No | Still open | Valid | No force/overwrite safeguard was added in CLI export paths | B, CI | Smoke test still writes into `demos/` without prompt | Add `--force` or refuse existing outputs |
| C22 | `indices.len() as u32` could overflow on huge meshes | No | Still open | Valid | `mesh_render.rs` still casts `indices.len() as u32` | B, CI | Low risk today, but still unchecked | Guard and fail gracefully on oversized meshes |
| C23 | Selection clearing was not fully cascading on parent deletion | No | Still open | Valid | Removal handlers mostly clear exact matches only; there is still no comprehensive parent-child cascade cleanup | B, CI | New tests improved visibility, but not all delete paths were fixed | Centralize cascade-selection cleanup in one helper |
| C24 | Core still had `todo!()` stubs | No direct task | Fixed correctly | Valid | Current `crates/rs_cam_core/src` search no longer shows `todo!()` stubs | B, CI | This was fixed outside the remediation tracker | None |

## D. Performance & Parallelism

| ID | Original claim | Attempted? | Current status | Assessment of original finding | Evidence | Checks run | Extra smells / improvements | Recommended follow-up |
|---|---|---|---|---|---|---|---|---|
| D1 | Toolpath lines were always 1-pixel wide | Yes | Partially fixed | Valid | `render/mod.rs` now has `LineWidthConfig`, but the same file explicitly says the value is “stored but not consumed by the GPU” and the pipeline is still `LineList` | B, CI | The tracker marked this as done even though the finding remains user-visible | Implement actual quad-based thick-line rendering or close the task as incomplete |
| D2 | Mesh GPU upload duplicated vertices instead of using indices | Yes | Fixed correctly | Valid | `MeshGpuData::from_mesh` now uploads shared vertices plus an index buffer | B, CI | Smooth-shaded normals are now default; flat-shade path is dead but documented | None |
| D3 | Simulation colors were recomputed / re-uploaded every frame | Yes | Fixed correctly | Valid | `SimMeshGpuData` now tracks `cached_color_fingerprint` and only writes the vertex buffer when colors change | B, CI | Deviation colors still never populate because upstream simulation returns `None` | Once deviations exist, re-check the fingerprint strategy with larger meshes |
| D4 | Only a small subset of hot paths used rayon | Yes | Partially fixed | Valid | Dropcutter batching is parallelized; adaptive/pocket follow-up was explicitly skipped in tracker and remains unimplemented | B, CI | Claimed speedup multipliers are not backed by reproducible benchmark data in repo | Keep the fix, but do not claim the full original perf finding is closed |
| D5 | Spatial-index dedup used `Vec<bool>` per query | Yes | Fixed correctly | Valid | `mesh.rs` query now uses a `u64` bitset with explicit comment about 8x memory reduction | B, CI | No benchmark artifact in repo, but implementation is correct | None |
| D6 | `kiddo` dependency looked unused | Yes | Fixed correctly | Valid | `crates/rs_cam_core/Cargo.toml` no longer depends on `kiddo` | B, CI | Root `Cargo.toml` still carries an unused workspace-level `kiddo = "4"` entry | Remove the stale workspace dependency too |
| D7 | Render pipeline had no frustum culling | No | Still open | Valid | No culling logic appears in current render path | B, CI | Larger simulation/mesh scenes still render everything | Add coarse culling before GPU submission |
| D8 | Arc data was lost through the offset pipeline | No | Still open | Valid | No remediation touched polygon offset arc preservation | B, CI | This still limits later arc-fit quality | Revisit after core correctness backlog is lower |
| D9 | Contour extraction chaining was O(n^2) in the worst case | No | Still open | Valid | No remediation touched `contour_extract.rs` | B, CI | Low severity but still a scaling risk | Document or optimize when contour workloads justify it |
| D10 | TSP 2-opt cost was high and undocumented | No | Still open | Valid | No remediation touched `tsp.rs` complexity or docs | B, CI | Documentation still does not set expectations | Document complexity and add escape hatches for large jobs |
| D11 | Undo stack overflow used `Vec::remove(0)` | No | Still open | Valid | `UndoHistory::push` still calls `self.undo_stack.remove(0)` | B, CI | Existing undo tests do not cover performance characteristics | Replace with `VecDeque` when touching history next |
| D12 | Tool wireframe generation was verbose and lacked LOD | No | Still open | Valid | No remediation touched the large tool wireframe section in `sim_render.rs` | B, CI | Not urgent, but still maintainability/perf debt | Revisit if render profiling points here |

## E. Testing Coverage & Quality (Synthetic IDs)

| ID | Original claim | Attempted? | Current status | Assessment of original finding | Evidence | Checks run | Extra smells / improvements | Recommended follow-up |
|---|---|---|---|---|---|---|---|---|
| E1 | CLI crate had effectively zero tests | Yes | Fixed correctly | Valid | `crates/rs_cam_cli/tests/integration.rs` now exists with parse and G-code tests | B, CI, TT | None | None |
| E2 | Interaction / picking had zero tests | No | Still open | Valid | No picking-specific tests exist under `interaction/` and the original `take(200)` sampling limit remains | B, CI | Picking is still fragile and under-covered | Add dedicated headless picking tests |
| E3 | Undo/redo system had zero tests | Yes | Partially fixed | Valid | `state/history.rs` now tests stack behavior, but there are still no end-to-end undo tests for non-stock edits | B, CI, TT | Same gap shown in `B2` | Add controller-driven undo scenarios |
| E4 | Controller CRUD events were largely untested | Yes | Fixed correctly | Valid | `controller::tests` now covers add/remove/rename lifecycles for tools, setups, and toolpaths | B, CI, TT | None | None |
| E5 | Dropcutter had only a few tests | Yes | Fixed correctly | Valid | `dropcutter.rs` now has broad tool-type, boundary, and cancellation coverage | B, CI, TT | No benchmark artifact, only correctness tests | None |
| E6 | `simulation_cut` analytics were barely tested | Yes | Fixed correctly | Valid | `simulation_cut.rs` now contains a substantial test block instead of only a couple of cases | B, CI, TT | None | None |
| E7 | Face milling was undertested | Yes | Fixed correctly | Valid | `face.rs` now has a materially larger test block and the viz compute layer also tests `OneWay` | B, CI, TT | No UI-facing face workflow test | None |
| E8 | `FlatEndmill` was severely undertested | Yes | Fixed correctly | Valid | `tool/flat.rs` now has broad `edge_drop`, facet, and boundary cases | B, CI, TT | None | None |
| E9 | GPU rendering had no headless / regression tests | No | Still open | Valid | No render regression harness was added; current focused viz tests stop at controller/worker layers | B, CI | This weakens confidence in `D1-D3` style rendering claims | Add snapshot or headless render smoke coverage |
| E10 | Property-based geometric tests were missing | Yes | Fixed correctly | Valid | `crates/rs_cam_core/tests/property_tests.rs` now exists with invariant-style checks | B, CI, TT | It is deterministic property-style testing, not `proptest` | Good enough for now |
| E11 | Parser fuzzing was missing | Yes | Deferred/out of scope | Valid | Tracker explicitly deferred `cargo-fuzz` integration | B, CI | Still missing coverage for parser hardening | Add fuzz targets in a separate dependency-adding change |
| E12 | End-to-end integration coverage was too thin | Yes | Fixed correctly | Valid | `end_to_end.rs` now covers SVG import, multi-operation flows, and multi-setup simulation | B, CI, TT | None | None |
| E13 | Multi-operation sequencing had no explicit tests | Yes | Fixed correctly | Valid | End-to-end tests now include multi-operation sequencing on shared stock | B, CI, TT | None | None |
| E14 | Cross-setup simulation lacked tests | Yes | Fixed correctly | Valid | End-to-end and worker tests now cover multi-setup simulation carry-forward | B, CI, TT | None | None |
| E15 | CLI integration lacked a demo-job CI smoke test | Yes | Fixed correctly | Valid | CLI integration tests exist and CI runs `cargo run -p rs_cam_cli -- job fixtures/demo_job.toml` | B, CI, TT | None | None |
| E16 | Export validation tests did not really validate generated G-code | Yes | Partially fixed | Valid | End-to-end tests now assert core G-code markers, but there is still no syntax parser / post-processor round-trip validation | B, CI, TT | Tests mostly check patterns, not structural correctness | Add a lightweight G-code parser or stricter syntax assertions |
| E17 | Cancellation behavior coverage was minimal | Yes | Partially fixed | Valid | Dropcutter and compute-worker tests now cover several cancellation paths, but not across all operations | B, CI, TT | Coverage improved materially but is not systematic | Add at least one cancellation test per long-running operation family |
| E18 | Degenerate geometry / malformed input tests were missing | Yes | Partially fixed | Valid | Some malformed/edge input cases now exist, but there is still no broad NaN/self-intersection test coverage | B, CI, TT | Still overlaps with `C8` | Add explicit degenerate-geometry fixtures |
| E19 | Profile lacked tests for multi-pass tabs, holes, dogbones | Yes | Partially fixed | Valid | Multi-pass final-tab behavior is now tested, but holes/dogbone-obtuse cases remain uncovered | B, CI, TT | Same residual gap as `A3` follow-up | Add hole/tab and obtuse-dogbone tests |
| E20 | Adaptive lacked narrow-slot / multi-island / validation tests | No direct task | Partially fixed | Valid | Generic invariant coverage improved, but the specific adaptive scenarios named in the finding are still not explicitly covered | B, CI | No targeted adaptive scenario expansion was found | Add scenario tests before touching adaptive again |
| E21 | VCarve lacked `max_depth=0`, cone-height, and thin-feature tests | Yes | Partially fixed | Valid | `max_depth=0` is now covered, but the other named scenarios are still missing | B, CI, TT | Good regression added, but not full closure | Add the remaining VCarve scenario tests |
| E22 | Inlay lacked hole/complementarity/sharp-corner tests | Yes | Partially fixed | Valid | Hole-related male-region tests exist now, but complementarity / sharp-corner coverage is still limited | B, CI, TT | A10 GUI split gap remains too | Add mating/complementarity assertions |
| E23 | Waterline lacked important terrain/island tests | No | Still open | Valid | No meaningful waterline test expansion was added | B, CI | Waterline also retains library-path unwraps | Add terrain/island/nesting fixtures |
| E24 | Pencil / Scallop lacked robust algorithmic tests | No direct task | Still open | Valid | There are semantic-trace tests, but not the algorithmic normal/curvature coverage the review called for | B, CI | UI exposes more scallop controls than test coverage justifies | Add algorithmic surface cases |
| E25 | Simulation lacked broad cutter/stamping coverage | No direct task | Partially fixed | Valid | Simulation-related tests expanded, but the specific Bull/VBit/Tapered stamping matrix is not fully covered | B, CI, TT | Still overlaps with tri-dexel complexity risk | Add per-tool simulation fixtures |
| E26 | Dressups lacked composition / lead-in-out / tabs-first-move coverage | No direct task | Partially fixed | Valid | Dressup helper extraction landed, but the specific scenario matrix is still incomplete | B, CI | Quality claim improved less than test count suggests | Add composition tests before more dressup work |
| E27 | Collision lacked tapered-holder / multi-segment / perf tests | No direct task | Partially fixed | Valid | Tapered and interpolated collision tests now exist, but there is still no performance coverage | B, CI, TT | This finding is less open than the original review implied | Add a coarse perf smoke benchmark if collision work resumes |
| E28 | State-management tests missed cascade/orphan/concurrency paths | No direct task | Partially fixed | Valid | Selection cascade tests were added, but concurrent undo+compute coverage is still absent | B, CI, TT | Same residual gap as `C23` | Add concurrency/state interaction tests |
| E29 | Coordinate transforms lacked 24 orientation-combo validation | Yes | Fixed correctly | Valid | `state/job.rs` now includes the 24-combo transform test matrix | B, CI, TT | None | None |

## F. Code Quality & Maintainability (Synthetic IDs)

| ID | Original claim | Attempted? | Current status | Assessment of original finding | Evidence | Checks run | Extra smells / improvements | Recommended follow-up |
|---|---|---|---|---|---|---|---|---|
| F1 | `compute/worker/execute.rs` was oversized and needed splitting | Yes | Fixed correctly | Valid | The old single file is replaced by `execute/mod.rs`, `operations_2d.rs`, and `operations_3d.rs` | B, CI | `execute/mod.rs` is still large, but the original monolith is gone | Future changes should keep logic in the split layout |
| F2 | `ui/properties/mod.rs` was oversized | Yes | Partially fixed | Valid | It dropped from ~2674 lines to ~1448 and offloaded work into `operations.rs`, but both files are still large | B, CI | The split reduced pressure but did not fully normalize the module size | Continue splitting by domain, not by arbitrary line count |
| F3 | `ui/sim_timeline.rs` was oversized | Yes | Still open | Valid | Current file is still ~1411 lines and remains a large mixed-responsibility unit | B, CI | Tracker claimed extraction, but the maintainability problem remains | Split transport, annotation, and rendering concerns into separate modules |
| F4 | `controller/events.rs` was monolithic | Yes | Partially fixed | Valid | `handle_internal_event` is more helper-driven now, but the file is still ~1521 lines and centralizes too much behavior | B, CI | Structural pressure remains high | Keep splitting by domain handlers and shared utilities |
| F5 | `adaptive.rs` was large but cohesive | No | Deferred/out of scope | Valid | File is still large (~2524 lines) and still cohesive enough that remediation did not target it | B, CI | No immediate regression tied to size alone | Revisit only if behavior work forces a split |
| F6 | `adaptive3d.rs` was very large but cohesive | No | Deferred/out of scope | Valid | File is still ~3702 lines; no remediation targeted it | B, CI | Same rationale as `F5` | Revisit only when behavior work demands it |
| F7 | Dressup tracing boilerplate was heavily duplicated | Yes | Fixed correctly | Valid | `apply_dressup_with_tracing()` now centralizes the tracing wrapper in `helpers.rs` | B, CI | This is one of the cleaner refactors in the series | None |
| F8 | Operation dispatch match arms were duplicated in multiple places | No | Still open | Valid | `semantic_op()` remains a long enum-to-trait match and related dispatch duplication still exists | B, CI | The refactor improved organization, not dispatch duplication | Consider macro/registry only if new ops continue to add friction |
| F9 | Feed/plunge/climb UI drawing was duplicated across operations | Yes | Fixed correctly | Valid | `draw_feed_params()` now centralizes the repeated pair in `operations.rs` | B, CI | None | None |
| F10 | Import handlers were duplicated across STL/SVG/DXF flows | Partial | Partially fixed | Valid | A generic `import_model` exists for reload paths, but dedicated import entrypoints still duplicate setup work | B, CI | Better than before, not fully normalized | Extract shared “push selected imported model + dirty + upload” logic |
| F11 | Depth-stepping iteration was duplicated across operations | No | Still open | Valid | There is still repeated per-level iteration logic across 2D ops | B, CI | No remediation addressed this hotspot directly | Extract only if another operation change needs it |
| F12 | `run_simulation_with_all/ids` duplicated group-building logic | Yes | Fixed correctly | Valid | `build_simulation_groups()` now centralizes shared setup/group construction | B, CI | None | None |
| F13 | Semantic-annotation boilerplate per operation was very repetitive | No | Still open | Valid | Operation semantic tracing still contains a lot of repeated scaffolding in `operations_2d.rs` / `operations_3d.rs` | B, CI | This also contributed to `A2` residual mismatch | Extract safer shared helpers before more semantic work |
| F14 | Parameter extraction / error wrapping was duplicated | No | Still open | Valid | No common builder/helper replaced the repeated extraction patterns | B, CI | Related to `C7` inconsistency | Consolidate when error-type migration happens |
| F15 | CLI parameter naming was inconsistent (`entry` vs `entry_style`) | Yes | Fixed correctly | Valid | `job.rs` now accepts `entry` with `#[serde(alias = "entry_style")]` for compat | B, CI | None | None |
| F16 | Event emission patterns were inconsistent across UI modules | No | Still open | Valid | Setup/property paths still mix direct mutation and event pushes inconsistently | B, CI | Same issue underlies `G1` | Standardize mutation flow around controller events |
| F17 | UI state mutation bypassed the controller in places | Yes | Partially fixed | Valid | Some stock/toolpath paths were rerouted, but direct mutation still exists in `properties/mod.rs` | B, CI | This remains the main architecture-drift smell in the GUI | Finish routing editable state through controller events |
| F18 | Error types were inconsistent across subsystems | Yes | Partially fixed | Valid | Boundary conversion to `ComputeError` landed, but internal helper signatures still diverge | B, CI | Same residual gap as `C7` | Finish the internal error-type migration |
| F19 | Epsilon values remained inconsistent across the codebase | No | Still open | Valid | No epsilon normalization pass was found | B, CI | Still a geometry-quality smell | Introduce shared constants when touching geometric tolerances next |
| F20 | Magic numbers were scattered | Yes | Partially fixed | Valid | Some pick/holder constants were extracted, but values like dogbone `170.0` and UI spacing literals remain | B, CI | Progress is real but incomplete | Keep extracting only where names improve readability |

## G. UI / UX Issues

| ID | Original claim | Attempted? | Current status | Assessment of original finding | Evidence | Checks run | Extra smells / improvements | Recommended follow-up |
|---|---|---|---|---|---|---|---|---|
| G1 | UI panels mutated state directly, bypassing controller/undo/event trail | Yes | Partially fixed | Valid | Some paths were rerouted, but `properties/mod.rs` still mutates state directly for several properties (for example workholding rigidity) | B, CI | This is still the biggest GUI architecture smell | Finish moving editable state through controller events |
| G2 | There was no model deletion UI | Yes | Fixed correctly | Valid | `AppEvent::RemoveModel` exists and is exposed in `project_tree.rs` / `setup_panel.rs` context menus | B, CI | No controller test specifically covers remove-model UX | Add one model lifecycle test if this area changes |
| G3 | There was no re-import / update workflow | Yes | Fixed correctly | Valid | `ReloadModel` event and `reload_model()` controller method now exist, wired from model context menus | B, CI | No dedicated regression test | Add one reload-model smoke test later |
| G4 | SVG/DXF import missed `pending_upload` and 2D camera fit | Yes | Partially fixed | Valid | `import_svg_path()` / `import_dxf_path()` now set `pending_upload`, but `app.rs` still calls `fit_camera_to_first_mesh()`, which does not fit polygon-only 2D models | B, CI | The remediation closed only half the finding | Add polygon-bounds camera fitting for 2D imports |
| G5 | Picking undersampled large toolpaths | No | Still open | Valid | `interaction/picking.rs` still computes `let step = (moves.len() / 200).max(1)` | B, CI | Still no picking tests | Raise sample density or adapt by geometry length |
| G6 | Scroll zoom direction inverted across platforms | Yes | Fixed correctly | Valid | `app.rs` now normalizes scroll input with `signum()` and clamped magnitude | B, CI | No dedicated cross-platform test harness | None |
| G7 | Escape in Simulation triggered even with text focus | Yes | Fixed correctly | Valid | `handle_simulation_shortcuts()` now exits early when `ctx.memory(|m| m.focused().is_some())` | B, CI | No UI regression test | None |
| G8 | Last-setup deletion looked clickable but silently did nothing | Yes | Fixed correctly | Valid | `project_tree.rs` uses `add_enabled(can_delete, ...)` for the delete action | B, CI | Feedback is now disabled-state rather than an explicit message | Good enough |
| G9 | Validation was fragmented between UI and generation-time checks | Yes | Partially fixed | Valid | `validate_toolpath()` is now shared between UI and submit path, but it still does not validate geometry/model presence or the rest-operation ordering rule | B, CI | Centralization landed without complete rule coverage | Expand `validate_toolpath()` before calling this fully fixed |
| G10 | Rest validation did not ensure the prior-tool operation exists earlier in the list | No | Still open | Valid | `validate_toolpath()` still only checks `prev_tool_id` presence and diameter ordering | B, CI | This remains a user-facing configuration trap | Validate operation ordering and setup-local references |
| G11 | Automation coverage was too low for deterministic UI testing | No | Still open | Valid | No UI automation harness expansion landed | B, CI | Same residual risk as `E9` | Add deterministic UI harness cases for core flows |
| G12 | UI abbreviations lacked tooltips | Yes | Fixed correctly | Valid | `status_bar.rs`, `viewport_overlay.rs`, and `setup_panel.rs` now use `.on_hover_text(...)` | B, CI | None | None |
| G13 | UI spacing magic numbers were scattered | No | Still open | Valid | `operations.rs` and `app.rs` still use literals like `8.0`, `4.0`, `240.0` directly | B, CI | Not urgent, but still noisy | Introduce small shared spacing constants when next touching layout |
| G14 | Simulation workspace lacked a staleness indicator outside preflight | No | Still open | Valid | No new staleness badge/indicator was found | B, CI | Users still have to infer staleness | Add a persistent workspace badge |
| G15 | Camera preset views snapped instantly | No | Still open | Valid | No camera-transition logic was added | B, CI | Low severity | Add interpolation only if UX work resumes |
| G16 | Operation defaults did not adapt to context | No | Still open | Valid | Default configs remain static in `state/toolpath/configs.rs` | B, CI | Low severity | Consider contextual defaults after core correctness backlog |
| G17 | Panel widths were not persisted across sessions | No | Still open | Valid | `app.rs` still uses `.default_width(...)`; no persistence logic was added for panel sizing | B, CI | Only unrelated small persisted UI state exists | Persist widths only if users keep hitting this |
| G18 | Workspace visibility changed without user hint | No | Still open | Valid | No explanatory hint/banner was added | B, CI | Low severity | Add a lightweight hint when workspace switching gets UI attention |
| G19 | Keyboard alternatives for pan/zoom/orbit were lacking | No direct task | Partially fixed | Valid | Shortcut coverage improved (simulation controls, view presets, delete/hide actions), but there are still no real keyboard pan/zoom/orbit controls | B, CI | The original finding is less absolute now, but still basically true for camera navigation | Add explicit camera-navigation shortcuts or adjust the wording in docs |
| G20 | Camera lacked orthographic mode | No | Still open | Valid | No orthographic projection support exists in current camera/render path | B, CI | Low severity | Revisit only during a broader camera overhaul |

## H. Documentation Drift

| ID | Original claim | Attempted? | Current status | Assessment of original finding | Evidence | Checks run | Extra smells / improvements | Recommended follow-up |
|---|---|---|---|---|---|---|---|---|
| H1 | Architecture doc still described heightmap simulation | Yes | Fixed correctly | Valid | `architecture/high_level_design.md` now documents tri-dexel simulation | B, CI | None | None |
| H2 | `README.md` still described heightmap simulation | Yes | Fixed correctly | Valid | `README.md` line 11 now references tri-dexel volumetric simulation | B, CI | None | None |
| H3 | New core modules were undocumented in architecture docs | Yes | Fixed correctly | Valid | Architecture docs now mention `dexel*`, `semantic_trace`, `debug_trace`, and `simulation_cut` | B, CI | None | None |
| H4 | `TRI_DEXEL_SIMULATION.md` was not indexed in architecture docs | Yes | Fixed correctly | Valid | `architecture/README.md` now indexes it | B, CI | None | None |
| H5 | Tri-dexel attribution was missing from credits | Yes | Fixed correctly | Valid | `CREDITS.md` now contains a tri-dexel attribution section | B, CI | Legacy heightmap-reference wording remains in credits as historical context, not active drift | None |
| H6 | `FEATURE_CATALOG` said vendor LUT was not wired | Yes | Fixed correctly | Valid | Current catalog describes vendor LUT seeding / GUI wiring as shipped | B, CI | None | None |
| H7 | Dressup application order was undocumented | Yes | Fixed correctly | Valid | `helpers.rs` now documents the fixed dressup order in a dedicated comment block | B, CI | None | None |
| H8 | Comment said “KD-tree” while code used a uniform grid | Yes | Fixed correctly | Valid | `mesh.rs` now documents the structure as a uniform XY grid | B, CI | None | None |
| H9 | Operation-specific docs had drift (for example VCarve/inlay semantics) | Partial | Partially fixed | Valid | VCarve `max_depth` docs now match behavior; no comparable cleanup was found for the broader inlay-doc drift noted in the review | B, CI | This bucket was broader than the tracker tasks that addressed it | Do a targeted doc sweep for operation-parameter semantics |

## I. Missing Features / Incomplete Implementations

| ID | Original claim | Attempted? | Current status | Assessment of original finding | Evidence | Checks run | Extra smells / improvements | Recommended follow-up |
|---|---|---|---|---|---|---|---|---|
| I1 | G-code output lacked M6 tool changes | Yes | Partially fixed | Valid | `rs_cam_core::gcode` can emit `M6 Tn`, but both CLI and Viz export still pass `tool_number: None` | B, CI, TT | Core support exists without end-to-end wiring | Thread actual tool numbers through export phases |
| I2 | G-code output lacked coolant support | Yes | Partially fixed | Valid | `CoolantMode` and emitter tests exist in core, but CLI/Viz export still hardcode `CoolantMode::Off` | B, CI, TT | Same “core only” problem as `I1` | Add coolant state to job/toolpath config and export wiring |
| I3 | Project Curve lacked tool compensation | No | Still open | Valid | No compensation logic was added for Project Curve | B, CI | This remains a feature gap, not a regression | Leave as backlog unless users need it soon |
| I4 | DXF `Line` / `Arc` / `Spline` entities were ignored | Yes | Fixed correctly | Valid | `dxf_input.rs` now handles `Line`, `Arc`, and `Spline`, with tests covering chained lines, arcs, and closed/open splines | B, CI, TT | Tracker note about partial DXF work is stale relative to current code | None |
| I5 | Collision check only processed the first STL-backed toolpath | No | Still open | Valid | `request_collision_check()` still uses `all_toolpaths().find_map(...)` and submits only the first matching toolpath | B, CI | This is a real end-to-end limitation despite collision-core test improvements | Extend collision requests to all relevant toolpaths |
| I6 | Boolean polygon ops were missing | No | Still open | Valid | No boolean-op feature was added | B, CI | Low priority | Keep as backlog |
| I7 | Drill hole ordering lacked TSP optimization | No | Still open | Valid | No drill-order optimization was added | B, CI | Low priority | Keep as backlog |
| I8 | UI lacked multi-select support | No | Still open | Valid | No multi-select behavior or selection model changes were added | B, CI | Low priority but still requested functionality | Keep as backlog |
| I9 | Dropcutter lacked mesh subdivision for coarse meshes | No | Still open | Valid | No subdivision/pre-refine stage was added | B, CI | Low priority | Keep as backlog |
| I10 | DXF `INSUNITS` and SVG unit conversions were missing | Yes | Fixed correctly | Valid | `dxf_input.rs` now scales from `$INSUNITS`; `svg_input.rs` now has `load_svg_data_mm()` and px-to-mm docs/tests | B, CI, TT | None | None |
| I11 | STL import lacked streaming/chunking for large files | No | Still open | Valid | No streaming import path was added | B, CI | Large-file behavior remains memory-bound | Keep as backlog unless large meshes become common |
| I12 | Face operation was not exposed in CLI | No | Still open | Valid | No `face` CLI command was added to current command surface | B, CI | Low priority but still inconsistent with GUI/core support | Add only if CLI users need 2.5D parity |
