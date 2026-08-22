# Feature Catalog

Canonical product-surface reference for `rs_cam`.

For source attribution and upstream lineage, see [`CREDITS.md`](CREDITS.md).

## Product surface

| Component | Role |
|-----------|------|
| `rs_cam_core` | CAM library: geometry, import, tool modeling, toolpath generation, dressups, simulation, feeds/speeds, and G-code |
| `rs_cam_cli` | Batch CLI and TOML job runner |
| `rs_cam_viz` / `rs_cam_gui` | Desktop CAM application built with `egui` and `wgpu` |

## Operations

| Category | Operation | Core module | GUI | Direct CLI | Status |
|----------|-----------|-------------|-----|------------|--------|
| 2.5D | Face | `face.rs` | Yes | No | Shipped |
| 2.5D | Pocket | `pocket.rs` | Yes | Yes | Shipped |
| 2.5D | Profile | `profile.rs` | Yes | Yes | Shipped |
| 2.5D | Adaptive | `adaptive.rs` | Yes | Yes | Shipped |
| 2.5D | VCarve | `vcarve.rs` | Yes | Yes | Shipped |
| 2.5D | Rest Machining | `rest.rs` | Yes | Yes | Shipped |
| 2.5D | Inlay | `inlay.rs` | Yes | Yes | Shipped |
| 2.5D | Zigzag | `zigzag.rs` | Yes | No | Shipped |
| 2.5D | Trace | `trace.rs` | Yes | No | Shipped |
| 2.5D | Drill | `drill.rs` | Yes | No | Shipped — first-class `OperationFamily` with peck cycles, diameter-scaled `peck_depth` / `plunge_rate_base`, and drill-native metrics (`DrillToolpathSummary` + `drill_gates`) in place of engagement axes. Hole targets can be picked from imported DXF (POINT entities + circle/arc centres, with layer attribution) via viewport click or per-layer "select all"; default (no selection) drills every closed-polygon centroid |
| 2.5D | Chamfer | `chamfer.rs` | Yes | No | Shipped |
| 3D | 3D Finish | `dropcutter.rs` | Yes | Yes | Shipped |
| 3D | 3D Rough | `adaptive3d.rs` | Yes | Yes | Shipped. The planner's stamp now mirrors the emitter's stock-to-leave drape (2026-08-04); before that the planner over-stated removal on curved and steep terrain, whose operator-visible symptom was skipped passes and standing material, not a gouge. Planner/simulator parity is gated directionally as well as by count |
| 3D | Waterline | `waterline.rs` | Yes | Yes | Shipped |
| 3D | Pencil Finish | `pencil.rs` | Yes | Yes | Shipped |
| 3D | Scallop Finish | `scallop.rs` | Yes | Yes | Shipped |
| 3D | Unified Finish | `unified_finish.rs` + `finish_planner.rs` | Yes | Yes | Experimental (P2.f — regions by true-surface slope, waterline/scallop/raster per region, greedy link-costed routing; sweep-locked defaults 45/75; scallop chord-refinement + tip-radius cusp + serpentine, live-validated; creases fold into the claims pipeline and are live — `unified_finish.rs:22-31`, `ClaimsConfig::crease_reference`; what is NOT built is the region-level territory filter ("S2"): it was built on top of S1's per-cell rest measurement, measured dead on both settings it could take, and removed 2026-07-27 — `unified_finish.rs:33-54`). **The tier-by-tier speed/quality comparison this row used to assert ("−20% at the speed tier", "plain Scallop wins the fine tier") is SUPERSEDED — it was measured through four instrument defects since fixed. See `planning/review_2026-07-29/SUPERSEDED_CONCLUSIONS.md`; a dated verdict belongs in planning prose, not in a capability catalog.** |
| 3D | Steep/Shallow | `steep_shallow.rs` | Yes | Yes | Shipped |
| 3D | Ramp Finish | `ramp_finish.rs` | Yes | Yes | Shipped |
| 3D | Spiral Finish | `spiral_finish.rs` | Yes | No | Shipped |
| 3D | Radial Finish | `radial_finish.rs` | Yes | No | Shipped |
| 3D | Horizontal Finish | `horizontal_finish.rs` | Yes | No | Shipped |
| 3D | Project Curve | `project_curve.rs` | Yes | No | Shipped |

## Tooling and setup

### Tool families

- Flat end mill
- Ball nose
- Bull nose
- V-bit
- Tapered ball nose

### Tool metadata exposed in the GUI

- geometry: diameter, cutting length, corner radius, included angle, taper angle
- collision envelope: holder diameter, shank diameter, shank length, stickout
- cutting metadata: flute count, tool material, cut direction
- catalog metadata: vendor, product ID

### Machine and material models

- stock material library in `rs_cam_core::material`
- machine profiles in `rs_cam_core::machine`
- feeds/speeds calculator in `rs_cam_core::feeds`
- vendor LUT seeding from embedded observations in `crates/rs_cam_core/data/vendor_lut`

## Toolpath modifiers and control layers

- heights system: clearance, retract, feed, top, bottom
- entry dressups: plunge replacement via ramp or helix
- dogbone overcuts
- lead-in / lead-out arcs
- link moves / keep-tool-down linking
- arc fitting to `G2` / `G3`. **Arcs do not cross an intent boundary** (2026-08-04): the fitter's run key carries `Move::intent` and breaks on `Region` span edges, so a fitted arc's label is exact rather than inherited from its first source move. Fitted arcs are tagged `SpanKind::GeometryRefit` and stay **in** tool-load gate populations; only true dressup bridges (dogbones, links, lead-outs) are excluded
- feed optimization dressup with stock-aware engagement estimation on supported workflows
- air-cut filter dressup: removes cutting moves through cleared stock when using remaining-stock mode
- stock-aware generation: per-toolpath "Use remaining stock" toggle pre-simulates prior operations to build actual material state
- per-operation manual pre/post G-code blocks (editable in GUI, emitted in export)
- TSP rapid-order optimization
- stock-boundary clipping with center / inside / outside containment. **Not unconditional** (2026-08-05): when the boundary offset *collapses*, the path passes through unclipped rather than over-clipped, and the report-only `ToolpathStats::boundary_clip_dropped` finding names it; when the offset *fails* (rejected input or a contained library panic), the operation now refuses instead of silently emitting an uncontained path
- dual compute lanes: toolpath generation plus analysis (simulation / collision)
- lane-status chips and a single `Cancel All` overlay action

## Simulation, verification, and export

### Import

- STL mesh import
- SVG vector import
- DXF vector import
- STEP file import (AP203/AP214 via truck crate, face-aware tessellation)

### BREP / face selection

- BREP face picking and selection in the viewport (click to toggle faces on/off)
- Per-face pastel coloring with selection highlighting on enriched meshes
- Face-derived 2D boundaries for 2.5D operations (horizontal planar faces)
- Face-derived containment boundaries for 3D operations
- Face selection persistence in project files (deterministic face IDs from STEP topology)
- BREP topology metadata panel (face count, adjacency, surface type breakdown)

### Export

- G-code: GRBL, grblHAL, LinuxCNC, Mach3 (`PostFormat`; grblHAL differs from GRBL by accepting `M6`/`M7`, which GRBL's post filters out)
- SVG toolpath preview
- HTML setup sheet
- TOML project/job persistence with editable-state round-trip

### Verification

- tri-dexel stock simulation (Z/X/Y grids). That is a claim about the dexel **engine**, which addresses all 6 cardinal face orientations; it is not a claim that the side-face *workflow* is finished. Setups on `Front`/`Back`/`Left`/`Right` emit G-code, metrics, gates and rest stock correctly (each is simulated in its own setup-local frame, where local Z is always the tool axis), and since 2026-08-22 a 2D drawing on such a setup is consumed in that setup's work plane rather than collapsing to a line. Also since 2026-08-22: the live-scrub viewport shows lateral cuts (G-LATERALSCRUB fixed — a lateral group is replayed in its own frame and mapped out, because the global playback stock's side grids append open surfaces to a closed solid and can never show the cut), and G-DRILLLATERAL — analytic drill removal abstaining on a lateral axis — is closed as **unreachable** the same day: lateral drills are simulated setup-locally where the axis is always Z, and no shipped path passes a lateral direction to the kernel any more. The abstention arm stays as the kernel's honesty contract. One lateral gap remains open: fixtures/keep-out zones are refused rather than projected (G-LATERALKEEPOUT)
- agent-readable MCP toolpath narration (`narrate_toolpath`) for Z-level structure, cut-run vs marching-squares region counts, engagement histogram, suspicious arcs, peak axial DOC, and air-cut summaries — timed at 4 ms on a 12.6k-move pass with a 70k-sample cut trace (2026-08-06)
- bounded typed simulation triage (`SimulationTriage`, `ProjectSession::simulation_triage`) — one contract consumed by the GUI diagnostics panel, MCP `get_diagnostics`, the CLI `project` report and narration: safety events, then actions, then capped/deduped advisories with a true pre-cap count. Replaces reading the raw `issue_count`, of which three different quantities ship under one name
- measurability abstention (`sim_measurability::MeasurabilityReport`) — a metric that cannot be resolved at the selected cell reports `NotMeasurable` with a reason and its gate ABSTAINS, instead of a hard zero dressed as a percent clearing every bar. Collision detection is never disabled by an abstention; the GUI prints a `NOT MEASURED:` strip above the panel
- structural toolpath spans (`Operation`, `DepthPass`, `Region`, entry/lead/link/dressup artifacts) propagated into simulation cut samples for span-aware filtering and outline navigation
- playback, scrub, and checkpoints
- tool visualization during playback
- holder/shank collision checks
- deterministic renderless GUI regression harness with stable automation IDs
- literature-matrix feeds validation suite — 56 cited cells × 19 invariants exercising the feeds engine against vendor chipload and RPM envelopes (`crates/rs_cam_core/tests/literature_matrix/` plus the `_litmatrix_*` sentries). The **drill** per-peck and chip-welding bands in it are declared repo-authored, not vendor or handbook: a 2026-08-04 audit retrieved the cited Onsrud drill chart and the FPL Wood Handbook and neither contains peck or depth-to-diameter guidance for wood
- drill ops produce drill-native verification (`DrillToolpathSummary` incl. `per_peck_max_dtd`, `drill_gates` for chip welding / peck adequacy / plunge feed sanity) instead of engagement metrics, which don't apply to Z-only kinematics. The cycle model is rooted at the **R-plane**, matching the emitter, through one shared `drill::fed_descents`

### Provenance gates

- source-freshness reporter — flags warn/stale vendor citations in `crates/rs_cam_core/tests/literature_matrix/sources.toml`, plus offline `citation_url` shape validation. The clock runs: it was frozen until 2026-08-04 because its default "today" equalled the seed date of 30 of 32 rows, so no row could ever age
- procedural analytic reference fixture (ARP-1, `crates/rs_cam_core/tests/common/reference_plate.rs`) — closed-form height and normal everywhere, tessellated to a measured rule; worst-zone p99 tessellation error 4.54 um against a 50 um grid-alias floor at the finest cell this repo has ever simulated. Quality bins are qualified against it, not against `terrain.stl`
- `/refresh-lit-matrix` skill — guided re-verification or replacement of stale citations

## Known partial areas

These features exist in state, UI, or helper code, but are not yet end-to-end complete:

| Area | Current state |
|------|---------------|
| Project save/load | editable state round-trips and model files are re-imported on load, but computed toolpaths, simulation checkpoints, and collision outputs are not persisted |
| Controller-side compensation | `G41` / `G42` output is not yet implemented; the “In Control” option has been hidden from the Profile UI until it is wired |
| Feed-optimization dressup | Supported only for fresh-stock, flat-stock workflows with known stock bounds; remaining-stock workflows use the air-cut filter instead |
| Rapid collision rendering | Core collision detection exists, but rapid collisions are not yet rendered in the viewport |
| Simulation deviation colors | Wired end-to-end (`StockVizMode::Deviation` → `sim_render::deviation_colors`, `crates/rs_cam_viz/src/app/gpu_upload.rs:66-77`). The real limitation: this path colors per-VERTEX deviations, which are corner-bilinear averages over 2×2 dexel columns and are for DISPLAY only — quality work (fidelity histograms, on-size verdicts) must read the unaveraged per-column instrument, `SimulationResult::column_deviations` / `ColumnDeviation` (`crates/rs_cam_core/src/compute/simulate.rs:245-262`) |
| Vendor LUT integration | Fully wired: embedded Amana vendor observations are auto-loaded at startup via `LazyLock` and passed into the feeds calculator for all GUI operations |
| BREP face selection scope | Face-derived boundaries work only for approximately-horizontal planar faces; non-planar and tilted faces produce no polygon (falls back to stock bounds). Surface classifier is heuristic (axis-aligned planes only). |
| BREP hover highlighting | Rendering path supports hover colors, but hover face tracking is not yet wired (face under cursor is not detected on mouse move) |
| ~~Workholding rigidity UI~~ | Fully wired: GUI ComboBox (Low/Medium/High) on stock panel, passed through to feeds calculator |
| 2D offset failure reporting | `offset_polygon_reported` distinguishes collapse / rejected input / library failure, and four families (pocket, profile, trace, zigzag) opt in and publish `ToolpathStats::offset_library_failures`. The other consumers still call the plain name and honestly report `None` = not measured. The count is per offset **call**, not per ring |
| Offset panic classes in debug vs release | Three `debug_assert!` sites (two in transitive dependencies) are caught in debug and **not** in release, where the library proceeds on unvalidated input. Accepted and documented, not measured; `offset_library_failures` is not comparable across builds |
| UnifiedFinish band residual | The band mix's off-part run-off is fixed and measured at zero, but ~200 um of overcut remains on the grooved reference fixture, located to the rim/wall break line and attributed to the shallow raster band. Open with a named single-column follow-up |
| Drill gate tool divisor | All three drill gates divide by the tool's **envelope** radius with a flat profile hardcoded, and `Drill` carries no tool precondition. On a tapered ball this overstates diameter and the gates read `Within` on an overloaded cutter — a silent pass. Tracked in the radius programme |

## CLI surface

Verified direct CLI commands (T9 cull, 2026-06-07 — the ~13 per-op
subcommands were replaced by the registry-driven generic `run`):

- `version` — build info for all workspace crates
- `job` — TOML job file (multi-tool/multi-op batch; executes through the session pipeline)
- `run` — ANY of the 23 operations: `run <op> --input model --tool type:diameter --set k=v --output out.nc`; `run --list-ops` / `run <op> --list-params` print the registry
- `sweep` — parameter sweep over a job file with fingerprint diffs
- `project` — GUI project file (format_version=3) full-diagnostics executor
- `smoke` — F-037 smoke baseline suite
- `nc-time` — G-code cycle-time prediction

Every operation in the registry is reachable from the CLI; a new
operation appears in `run` with zero CLI code.
