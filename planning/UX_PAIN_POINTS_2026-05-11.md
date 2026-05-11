# UX Pain Points — 2026-05-11

Findings from the 12-journey UX testing session
(`planning/UX_TESTING_SESSION_PLAN.md`).

Severity:

- 🔴 Critical — blocks correct result or hides safety information
- 🟡 Important — friction/confusion, recoverable
- 🟢 Polish — cosmetic or minor flow

Format:
> 🟡 **[Jx, surface]** Problem statement. _Where_: file.rs:line. _Fix idea_: shape.

Fixtures used: 7 skeleton TOMLs in `test_data/ux_*.toml` (paths fixed from
`fixtures/...` → `../fixtures/...` mid-session — original skeletons were broken
because the loader resolves relative to the project file, not CWD) + the mature
multi-setup wanaka project at `~/Downloads/wanaka100/wanaka_full.toml`.

---

## J1 — First-load impression

> 🔴 **[J1, project loader]** Skeleton TOMLs that referenced `fixtures/...`
> appeared to load with no warning, but every model silently failed
> (`load_error` populated only inside `inspect_model`). The MCP `load_project`
> response said "Loaded ... 1 setups, 0 toolpaths" — same string a healthy
> empty project produces. Without inspecting each model the user has no
> signal that geometry is missing. _Where_:
> `crates/rs_cam_viz/src/controller/io.rs:223` (warnings collected into
> `self.load_warnings`) + the GUI `project_load_warnings` modal at
> `controller/tests.rs:353`. The GUI modal works; the **MCP shell** doesn't
> expose the warning channel — its `load_project` tool only returns the
> setup/toolpath summary string. _Fix idea_: have the MCP `load_project`
> response include `warnings: [...]` and surface a count in the message
> (e.g. "Loaded ... -- 2 warnings: 1 model failed"). The GUI side already
> renders this; only the MCP wrapper drops it.

> 🟡 **[J1, project_summary]** Wanaka loads as `'Untitled' -- 2 setups, 7
> toolpaths` because the underlying TOML has an empty `[job].name`. The MCP
> echoes the empty name as "Untitled"; the GUI title bar likely does the
> same. There's no fall-back to filename. _Where_: rs_cam_core
> `session/project_file.rs` `name` field defaulted; needs filename fallback
> in `ProjectSession::load`. _Fix idea_: when `name` is empty, default to
> the file stem so the user can identify which project loaded.

> 🟡 **[J1, BREP loader]** All 4 fixture STEP files load successfully with
> `kind: "step"` (triangulated, e.g. `l_bracket.step` → 20 triangles, 36
> verts), but `inspect_brep_faces` returns
> `"Model 'X' has no BREP data (not a STEP model)"`. The model says STEP
> but face-selective ops have nothing to pick. Two ways this fails: (a) the
> kind label is wrong, or (b) the BREP topology is dropped during the
> import path used here. Either way the user thinks face picking will work
> and it won't. _Where_: `crates/rs_cam_core/src/io.rs:25-115` (model
> dispatch) + step-import path. _Fix idea_: when `kind=step` but BREP is
> absent, mark the model `kind: "step_mesh_only"` (or surface a warning),
> and disable the face-picker affordance with a tooltip explaining why.

> 🟡 **[J1, MCP tool naming]** `inspect_brep_faces(model_id=0)` returned
> "Model 0 not found" but `inspect_model` listed the same model with
> `id: 1`. The argument is a 1-based model ID, not a 0-based index, but
> the docstring says "Model ID (0-based)". Inconsistent with most other
> tools (`generate_toolpath`, `get_toolpath_params`) which take 0-based
> indices. _Where_: MCP tool definition for `inspect_brep_faces`. _Fix
> idea_: align — either make all `_index` and use 0-based, or rename to
> `_id` and document. Don't mix.

> 🟡 **[J1, stock auto-resize]** L-bracket (60×40×40 mm bbox) loads into a
> 100×100×60 mm stock with `auto_from_model=true`. The auto-resize only
> fires when a model is **attached** in the GUI, not on project load — so
> users see a wildly oversized stock with the auto checkbox lying ON. The
> stock origin shows -5/-5/0 but XY remains 100×100, leaving 35 mm of
> padding on every side. _Where_:
> `crates/rs_cam_viz/src/controller/events/model.rs:371` (update only on
> add/import path). _Fix idea_: on project load, if `auto_from_model` is
> set and a model bbox is available, run `update_stock_from_bbox` once.
> Alternative: clear the checkbox if the stock dims diverge from
> bbox+padding by more than a tolerance, so the lying state can't persist.

> 🟢 **[J1, stock z precision]** Wanaka stock z reads
> `10.97931981086731` mm — auto-derived from model bbox and serialised
> with full f64 precision. The stock panel `DragValue` will render this
> ugly number to the user. _Where_: `crates/rs_cam_viz/src/ui/properties/stock.rs:101`
> (DragValue without `.fixed_decimals`). _Fix idea_: render dimension
> fields with `.fixed_decimals(3)`; round the stored value when
> auto-derived if it's the result of bbox math.

> 🟢 **[J1, project name]** Wanaka also loads with `material: "Generic
> Softwood"` — appears legit, but the cross-fixture default of
> `GenericHardwood` in skeletons is jarring (no UI prompt to set
> material; if you skip the panel the LUT lookups silently fall back).
> _Where_: skeleton TOMLs ship hard-coded; no first-load nudge.
> _Fix idea_: on first toolpath generation, if material is `Generic*`
> and user hasn't touched it, surface a toast "material is generic —
> verify before cutting" once.

## J2 — Stock setup

> 🟡 **[J2, alignment_pins]** No "pin overlaps model footprint" warning.
> The 2D pocket fixture has a polygon at (5,5)→(75,55) and a default pin
> at (5,5) — pin sits at the polygon's corner inside the cut zone. The
> stock panel flags pins **outside stock bounds** but not pins inside
> the model. _Where_: `crates/rs_cam_viz/src/ui/properties/stock.rs:393-405`
> (only out-of-bounds check). _Fix idea_: add an XY check against the
> active model's polygon bbox or silhouette and emit a yellow "Pin N
> overlaps model" line with the offending pin index.

> 🟢 **[J2, stock auto checkbox]** The "Auto from model" checkbox is a
> bare boolean with no inline explanation of *when* it fires (on attach?
> on every dim edit? on load?). After the J1 finding above we know it's
> "on attach only", but a hover would close the gap.
> _Where_: `crates/rs_cam_viz/src/ui/properties/stock.rs:151-153`. _Fix
> idea_: add `.on_hover_text("Recomputes stock when a model is attached.
> Edits to dimensions are kept; toggle off to lock.")`.

> 🟢 **[J2, material picker]** Hardness Index and Kc rows are shown in a
> dim small font that visually reads "disabled" — they're correctly
> read-only but the dim style is weak feedback. New users may not realise
> these change with material selection. _Where_:
> `crates/rs_cam_viz/src/ui/properties/stock.rs:43-69`. _Fix idea_: keep
> dim, but add a top label "Material properties (auto from selection)"
> so the read-only-ness is explicit.

> 🟢 **[J2, two-sided setup]** "Two-sided setup" button only appears when
> `flipped` setup absent **and** pins list empty. Once a user adds a pin
> manually the convenience button disappears even though it would still
> simplify their flow. _Where_:
> `crates/rs_cam_viz/src/ui/properties/stock.rs:193-201`. _Fix idea_:
> show the button whenever no flipped setup exists; on click, only add
> pins if the list is empty (otherwise just create the flipped setup +
> set flip axis).

## J3 — Tool creation/editing

> 🟡 **[J3, LUT-sourced fields]** Tool panel shows ~12 numeric fields
> (diameter, cutting length, flutes, helix, corner radius, material,
> cut dir, holder/shank/shaft) all in identical input style. Nothing
> distinguishes "this came from a vendor LUT" vs "you typed this".
> The vendor lookup result is opaque to the user — yet the chipload
> verdict is calibrated against vendor LUT rows ("amana-flat-softwood-…"
> per the tool-load report). _Where_:
> `crates/rs_cam_viz/src/ui/properties/tool.rs:30-148`. _Fix idea_: add
> a small chip next to each field that's been seeded by the LUT match
> (e.g. green dot + tooltip "from vendor LUT row X"); user-edited
> values lose the chip.

> 🟡 **[J3, stale propagation]** No visible indication on the tool
> panel itself that toolpaths depending on this tool are now stale
> after editing. The toolpath panel shows the staleness on each row,
> but a user editing a tool may never look at toolpaths. _Where_:
> tool panel has no awareness of dependents.
> _Fix idea_: at the bottom of the tool panel show a small chip
> "3 toolpaths depend on this tool — they will need re-generation"
> after any edit.

> 🟢 **[J3, range bounds]** Diameter range 0.1..=100 mm, cutting length
> 0.1..=200 mm, taper half-angle 0.5..=89 deg — sensible. But corner
> radius range is `0..=diameter/2` which silently snaps when diameter
> shrinks below 2× the existing radius. Could surprise. _Where_:
> `crates/rs_cam_viz/src/ui/properties/tool.rs:78,113`. _Fix idea_: when
> diameter is reduced and clamps the radius, fire a small toast or
> dirty the field colour.

## J4 — Toolpath creation defaults

> 🔴 **[J4, drop_cutter default]** New `drop_cutter` ("3D Finish") on the
> 3D terrain fixture defaults `min_z = -50.0`. The terrain stock is 30 mm
> tall (origin_z=0, top=30), so default min_z reaches **20 mm below the
> stock floor**. If the user accepts defaults the cutter chases nothing
> for 20 mm. _Where_:
> `crates/rs_cam_core/src/operations/drop_cutter` defaults (operation
> config), surfaces in `properties/operations/surface_3d.rs`. _Fix idea_:
> default `min_z` to `stock.origin_z` (or `bbox.min.z`) when constructing
> a new drop_cutter, not -50.

> 🟡 **[J4, depths hard-coded]** All 2D op defaults bake fixed depths
> divorced from stock thickness:
> - Pocket: depth=3 (stock 12 mm = 25%)
> - Profile: depth=6 (stock 12 mm = 50%)
> - Drill: depth=10 (stock 12 mm = 83%)
> - Adaptive: depth=6
> A new user has to edit `depth` on every single 2D toolpath because the
> default rarely matches the stock. _Where_:
> `crates/rs_cam_core/src/operations/{pocket,profile,drill,adaptive}/config.rs`
> default impls. _Fix idea_: default `depth = stock.z` (full-through) for
> profile/drill, and `depth = min(stock.z, 5mm)` for pocket — both
> derivable at toolpath construction time via the session's stock config.

> 🟡 **[J4, feed_rate hard-coded]** Pocket/Profile default feed=1000;
> Adaptive 1500; Drill 300; 3D Rough 1500; Scallop/Drop_cutter/Waterline
> 1000. None are material-aware. The session has a chip_load LUT
> (`k0=0.024 p=0.61 q=1.26`) and a vendor LUT — defaults could call
> into the same calculator the optimizer uses. _Where_: each op
> config's default impl. _Fix idea_: switch defaults from constants to
> a `default_feeds_for(tool, material, machine)` helper (the optimizer
> already encapsulates this — share it).

> 🟡 **[J4, stepover units ambiguous]** Pocket/adaptive `stepover=2.0`
> with no inline label whether that's mm or % of diameter. For a 6mm
> tool 2.0 reads as 33%; for a 12mm tool the same number reads as 17%.
> _Where_: `crates/rs_cam_viz/src/ui/properties/operations/mod.rs`
> stepover field. _Fix idea_: add suffix " mm (= XX% of tool dia)" so
> both metrics are visible at once.

> 🟢 **[J4, adaptive3d param wall]** New adaptive3d toolpath shows 17
> default params (clearing_strategy, depth_per_pass, detect_flat_areas,
> entry_style, feed_rate, fine_stepdown, helix_pitch, helix_radius_factor,
> min_cutting_radius, plunge_rate, ramp_angle_deg, region_ordering,
> stepover, stock_to_leave_axial, stock_to_leave_radial, tolerance,
> z_blend). For a "just generate something sensible" first encounter
> this is overwhelming. _Where_:
> `crates/rs_cam_viz/src/ui/properties/operations/surface_3d.rs`. _Fix
> idea_: collapse advanced params (entry_style, helix_*, region_ordering,
> z_blend, fine_stepdown, detect_flat_areas, min_cutting_radius) into a
> "Generation strategy (advanced)" disclosure; show only depth_per_pass /
> stepover / feed_rate / tolerance / stock_to_leave by default.

> 🟢 **[J4, adaptive `slot_clearing` default]** 2D adaptive defaults
> `slot_clearing = true`. For a profile-flavoured pass that's wrong;
> for a pocket-clearing pass it's right. The default depends on the
> intent — surfacing it as a toggle on every adaptive op assumes the
> user knows what slot clearing is. _Where_: adaptive config default.
> _Fix idea_: hover tooltip "Pre-clears straight slots before the
> spiral pass — leave on for pockets, off for race-track passes."

### J4 second pass — more operation families

> 🟡 **[J4.2, face]** New `face` toolpath defaults `depth = 0.0`. A
> zero-depth facing op cuts nothing. The user must edit before
> generation produces a usable result. _Where_:
> `crates/rs_cam_core/src/operations/face/config.rs` default.
> _Fix idea_: default `depth = stock.padding` (skim the padding
> margin) or `min(0.5mm, stock.z * 0.05)` — both produce a meaningful
> first cut without risking digging into the model.

> 🟢 **[J4.2, face stock_offset=5]** `stock_offset = 5.0` adds 5mm
> to the facing perimeter beyond the stock. Sensible if the cutter
> needs run-off room, but unexplained — the user has no idea why a
> facing op wants to extend past the stock edge. _Where_: same
> config. _Fix idea_: hover "Cutter overruns the stock edge by this
> amount so each pass clears the corner; reduce to 0 for tight
> fixturing."

> 🟢 **[J4.2, horizontal_finish on terrain]** `angle_threshold = 5°`
> means only surfaces within 5° of horizontal get cut. On a terrain
> mesh that's mostly slopes, the toolpath legitimately produces almost
> nothing — but the user gets a successfully-generated empty toolpath
> with no warning that the model has no near-flat regions.
> _Where_: horizontal_finish post-generation should detect ~0
> applicable surface and emit a status. _Fix idea_: when a finishing
> op finds <1% applicable surface, mark the toolpath status as
> `Empty` and surface "no near-horizontal surfaces found at threshold
> 5°" in the toolpath panel.

> 🟢 **[J4.2, steep_shallow 11 params]** Same overwhelming-defaults
> pattern as adaptive3d (17 params), but with 11 instead. Same fix:
> collapse `overlap_distance / sampling / steep_first / stock_to_leave
> / threshold_angle / tolerance / wall_clearance / z_step` into an
> "Advanced" disclosure; show only `feed_rate / plunge_rate /
> stepover` by default.

> 🟡 **[J4.2, finish ops 0 stock_to_leave]** All four 3D finishing
> ops (scallop, drop_cutter, ramp_finish, horizontal_finish, pencil,
> steep_shallow, spiral_finish) default `stock_to_leave = 0.0`. If a
> user runs a roughing op with `stock_to_leave_radial/axial = 0.5`
> (the adaptive3d default) and then drops in a finishing pass with
> `stock_to_leave = 0`, the finisher needs to remove that 0.5mm in a
> single pass — typically fine on hardwood with a small ball nose,
> but if the user picks the same end mill as the rougher it could
> overload. _Where_: each finishing config default. _Fix idea_: leave
> at 0 (most common case) but add a tooltip "Should match the
> roughing op's stock_to_leave (typ. 0.0–0.5 mm)".

### J4 third pass — dressups, boundary, heights, generation status

> 🟡 **[J4.3, dressup entry_style=None]** `DressupConfig::default`
> sets `entry_style: DressupEntryStyle::None`. So every new toolpath
> uses **vertical plunge** unless the user manually picks Ramp/Helix
> in the dressup panel. For a pocket cut with a 6mm flat end mill
> into hardwood, that's the worst entry — chipload-low (rubbing) on
> the plunge, dramatically reduced tool life. The op-level defaults
> for pocket and adaptive should pre-pick Ramp/Helix on the dressup.
> _Where_: `crates/rs_cam_core/src/compute/config.rs:351` (default)
> and the per-op `OperationConfig::new_default` paths that don't
> override it. _Fix idea_: in `OperationConfig::new_default`, set
> `dressup.entry_style = Ramp` for pocket/profile/face/v_carve,
> `Helix` for adaptive/adaptive3d, leave `None` for drill/trace.

> 🟡 **[J4.3, dressup link_moves=false]** Default `link_moves: false`
> means every region has its own retract → rapid → plunge cycle, so
> a pocket with multiple disjoint cells produces N air-cut spikes.
> Linking is a near-universal win — `link_max_distance=10mm` keeps
> it safe. _Where_: `crates/rs_cam_core/src/compute/config.rs:359`.
> _Fix idea_: default `link_moves = true` with `link_max_distance =
> 10.0mm` and `link_feed_rate = 500.0` (current defaults already
> exist; just flip the bool).

> 🟡 **[J4.3, dressup feed_optimization=false]** Default off — so
> no per-segment feed scaling on entry corners, narrow regions, or
> high-curvature passes. Typically a 5–30% cycle-time win at no
> safety cost (the optimization caps at the user feed). _Where_:
> `crates/rs_cam_core/src/compute/config.rs:364`. _Fix idea_:
> default on; user can disable on a per-op basis for ops where
> precise feed control matters.

> 🟢 **[J4.3, dressup optimize_rapid_order=false]** Default off —
> rapids are emitted in generation order, which for adaptive3d's
> per-region passes can mean a lot of cross-stock rapids. _Where_:
> `crates/rs_cam_core/src/compute/config.rs:367`. _Fix idea_:
> default on; this is pure-win nearest-neighbour reorder of rapids
> only, never changes cutting order.

> 🟢 **[J4.3, boundary.enabled=false]** Default `BoundaryConfig {
> enabled: false, source: Stock, ... }`. For 3D ops on a small
> model in oversized stock (the L-bracket case from J1), the cutter
> sweeps over the entire stock rather than just the model
> silhouette — generates lots of air-cut. _Where_:
> `crates/rs_cam_core/src/compute/config.rs:277`. _Fix idea_: when
> the op type is 3D rough/finish AND the model has 3D bbox, default
> `boundary.enabled=true, source=ModelSilhouette` in
> `OperationConfig::new_default`.

> 🟢 **[J4.3, heights all Auto]** `HeightsConfig::default` =
> all five (`clearance/retract/feed/top/bottom`) set to
> `HeightMode::Auto`, with the `resolve` helper deriving sensible
> values from `safe_z + op_depth`. Good default. (Logged as
> success.) _Where_:
> `crates/rs_cam_core/src/compute/config.rs:158-168`.

### J5 third pass — generation failure display

> 🟡 **[J5.3, ERR chip swallows the message]** `ComputeStatus::Error(_)`
> renders as a red "ERR" chip with no tooltip — the inner error
> string is discarded by the wildcard match. So when a generator
> fails (wanaka's "Back Rough" → "No result produced") the user can
> see *that* it failed but not *why* without firing
> `get_generation_debug_trace`. _Where_:
> `crates/rs_cam_viz/src/ui/toolpath_panel.rs:316`. _Fix idea_: bind
> the error string and surface it as the chip's hover text:
> `ComputeStatus::Error(msg) => ("ERR", theme::ERROR)` plus a
> follow-up `.on_hover_text(msg)` call when status is Error.

## J5 — Param edit → generate → simulate cycle

> 🟢 **[J5, MCP staleness signal]** `set_toolpath_param` returns the
> string "Set toolpath N param 'X'. Regenerate to apply." but
> `list_toolpaths` afterwards exposes no `stale: true` flag. From the
> MCP, you can only know what's stale by remembering what you edited.
> _Where_: MCP `list_toolpaths` payload. The GUI uses
> `rt.stale_since` (from `toolpath_panel.rs:608-616`); the MCP layer
> drops the field. _Fix idea_: include `stale: bool` (and optionally
> `status: "fresh"|"stale"|"failed"|"pending"`) in `list_toolpaths`
> entries.

> 🟡 **[J5, generate_all silent failure]** Wanaka `generate_all`
> reported `"Generated 6 toolpaths (1 failed): toolpath 4: No result
> produced"`. The next `list_toolpaths` shows toolpath 0 ("Back Rough",
> id=4) still as `enabled: true` with no failure marker. _Where_:
> `list_toolpaths` doesn't return per-row status. _Fix idea_: surface
> `last_generation_error: Option<String>` on each list entry and a top-
> level `failed_count` so the user can tell post-hoc which ops failed.

> 🟢 **[J5, "No result produced"]** "Back Rough" failed with that
> exact opaque message. The user can't tell why — out-of-bounds? no
> entry point? config invalid? — without firing
> `get_generation_debug_trace`. _Where_: error formatting in the
> generator dispatch. _Fix idea_: include the underlying reason
> (`SessionError::*` variant or a debug-trace summary) so the user can
> act without a second tool call.

> 🟢 **[J5, GUI staleness banner — works]** The simulation workspace
> shows a yellow "⚠ Results may be stale / Parameters changed since
> the last simulation run" banner with a re-run button after any
> edit. _Where_: `crates/rs_cam_viz/src/ui/sim_op_list.rs:174-201`.
> Validated — the F2 reframe landed cleanly here. (Logged as 🟢
> success per the prompt's "save validated approaches too" guidance.)

## J6 — Simulation results — graphs and warnings

> 🔴 **[J6, issue count inflation]** Wanaka simulation reports
> `issue_count: 25,101` (and 11,524 on a re-run with one toolpath
> failed). The diagnostics panel grid shows "Issues 25101" with no
> severity breakdown — same pattern the prompt flagged from the
> previous F6 reframe. The hotspot count (694) is also a single
> integer with no risk vs noise split. _Where_:
> `crates/rs_cam_viz/src/ui/sim_diagnostics.rs:596-609`. _Fix idea_:
> partition issues by `SimulationIssueKind` (Hotspot vs AirCut vs
> LowEngagement vs RapidCollision vs HolderCollision) and render
> as `air_cut: 24800 / low_engagement: 287 / hotspot: 14 / collision: 0`
> so the user can ignore the air-cut bucket and focus on the rest.
> Better: bucket by danger ("must address" vs "informational") with
> the count and a one-click filter.

> 🟡 **[J6, verdict text-only]** `run_simulation` returns `verdict:
> "WARNING: high air cutting"` — a single string with no breakdown of
> *which* toolpath caused the air cutting. The GUI doesn't surface this
> verdict at all from what I can tell (the right-panel Findings grid
> shows raw counts). _Where_: verdict comes from
> `rs_cam_core::simulation::verdict`. _Fix idea_: render the verdict
> as a top-of-panel banner with the offending toolpath name(s)
> hyper-linked to a jump.

> 🟡 **[J6, "Within bounds: 0"]** With 7 toolpaths and the tool-load
> report having `summary: { within: 0, exceeds: 5, fully_unmodeled: 2 }`,
> the GUI Findings grid shows "Within bounds: 0 / Exceedances: 5 /
> Unmodeled: 16". The 16 is the count of `gate × toolpath` cells, not
> toolpaths. So "5 exceedances" and "16 unmodeled" use different
> denominators. Confusing. _Where_:
> `crates/rs_cam_viz/src/ui/sim_diagnostics.rs:685-708`
> (`verdict_counts_local` iterates all gates, summing across them).
> _Fix idea_: pick one denominator (toolpaths or gate-cells) and label
> consistently — e.g. "5 of 7 toolpaths exceed (chipload-low) /
> 2 of 7 fully unmodeled" instead of mixing scales.

> 🟡 **[J6, BURN label]** The `verdict_badge` for chipload `Exceeds
> Low` renders "BURN" — bold and red. Good. But "BURN" with no
> tooltip-readable explanation of why a *low* chipload causes burning
> is opaque to a user who doesn't know rubbing-burns-tools physics.
> _Where_:
> `crates/rs_cam_viz/src/ui/sim_diagnostics.rs:805-808`. _Fix idea_:
> ensure the verdict tooltip includes "Chipload below the vendor floor —
> the cutter rubs instead of slicing, generating heat and dulling the
> edge."

> 🟡 **[J6, hotspot count 694]** 694 hotspots on wanaka. Even partitioned
> they're a fire-hose. The hotspot card is well designed (Jump,
> Optimize, Clear buttons) but there's no triage UI — you can only step
> through one at a time. _Where_:
> `crates/rs_cam_viz/src/ui/sim_diagnostics.rs:347-419`. _Fix idea_:
> add a top-N hotspot list (sorted by `wasted_runtime_s`, top 10) with
> the same Jump/Optimize controls, so the user can knock down the worst
> few without scrolling through all 694.

> 🟢 **[J6, generator overlay default]** "Show generator steps" toggle
> only appears when `any_traces_recorded`, hiding it from users who
> haven't enabled the capture. Good gating. _Where_:
> `crates/rs_cam_viz/src/ui/sim_diagnostics.rs:137-152`. (Working
> well — logged as success.)

> 🟢 **[J6, stale results banner]** The "⚠ Results stale (params
> changed) — re-run sim" line appears in the project overview when
> simulation drifts from edits. _Where_:
> `crates/rs_cam_viz/src/ui/sim_diagnostics.rs:612-619`. (Working
> well.)

## J7 — Tool-load report

> 🟡 **[J7, all-low chipload]** Every wanaka toolpath that the gate
> can evaluate (5 of 7) reports `chipload exceeds low`. Examples:
> - TP12 observed 0.001131 mm/tooth vs vendor band 0.025–0.04
> - TP6 observed 0.001788 vs 0.0083–0.0154
> - TP10 observed 0.011004 vs 0.05–0.085
> All ~10–20× below the vendor floor. The user sees 5 BURN badges and
> the report says "Within: 0". This is a project-wide signal that the
> defaults (or the vendor LUT match) don't suit this user's workflow,
> not 5 separate problems. _Where_: report comes from
> `rs_cam_core::tool_load::verdict`. _Fix idea_: when ≥3 toolpaths show
> the same gate failing on the same side, surface a project-level
> "All chipload-low — feed too slow for tool/material? Run optimizer or
> raise feed_rate" banner above the per-toolpath rows.

> 🟡 **[J7, "fully_unmodeled: 2" without explanation]** The summary
> says 2 toolpaths are fully unmodeled, with reasons like
> `simulation_required` (TP4 — Back Rough that failed to generate)
> and `arc_engagement_not_captured` (TP7 — drill, makes sense). The
> GUI Findings grid shows "Unmodeled: 16" without saying *what is
> blocking* modelling. _Where_: badge tooltip path
> `verdict_tooltip` in `sim_diagnostics.rs`. _Fix idea_: when an
> Unmodeled badge is hovered, show the exact `UnmodeledReason` (already
> typed in the report) — not just "no model".

> 🟡 **[J7, deflection within = "validated" but peak 107 µm]** TP12
> deflection `kind: within, peak_mm: 0.107`, with bounds `validated_within
> 50 µm / exceeds 200 µm`. So 107 µm sits in the "approximate within"
> band — surface finish degradation expected — but the badge renders
> green (Within) with no inline cue that the peak is twice the
> validated threshold. _Where_:
> `crates/rs_cam_viz/src/ui/sim_diagnostics.rs:797-807`. _Fix idea_:
> when peak crosses `validated_within_mm` but stays under
> `exceeds_mm`, render the badge in a third "warn" color (yellow) so
> the user knows "this is fine for the tool but expect a worse
> surface".

> 🟢 **[J7, validated vs approximate]** The Confidence types are great
> (Validated / Approximate(detail) / etc.) and the "≈" suffix on
> approximate badges is clean. (Logged as success.)

## J8 — Optimizer journey

> 🟡 **[J8, modal blocks the viewport]** Optimize modal is anchored
> CENTER_CENTER with `default_width(640.0)` and is non-collapsible.
> While the optimizer runs (1–2 minutes per toolpath per the doc) the
> user can't see the toolpath in the viewport behind it. _Where_:
> `crates/rs_cam_viz/src/ui/optimize_modal.rs:36-44`. _Fix idea_:
> make the window collapsible, allow drag, and shrink default width
> when in `Loading` state to ~320 px — the spinner doesn't need 640.

> 🟡 **[J8, no batch optimize]** Wanaka has 5 chipload-exceeds
> toolpaths needing the same kind of fix (raise feed). The GUI requires
> opening the modal once per toolpath, waiting 1–2 min each. No "run
> the optimizer on all exceedances" button. _Where_: modal is per-
> toolpath only (`OpenOptimizeModal(ToolpathId)` event). _Fix idea_:
> add a project-level "Optimize all exceedances" action; open a list
> modal that runs sequentially with one progress bar.

> 🟢 **[J8, NoSafeImprovement narrative]** The structured narrative
> (`headline / envelope / suggestions / entry_advisories`) is well
> designed and the modal renders all four cleanly. _Where_:
> `optimize_modal.rs:110-150`. (Logged as success.)

## J9 — Error reasoning

> 🟡 **[J9, "exceeds chipload (low)" without machine context]** The
> tool-load report says TP10's chipload is 0.011 mm/tooth observed vs
> vendor band 0.05–0.085 — a ~5× shortfall. To act on this, the user
> needs to know: "is the limit feed_rate (4000 mm/min)? RPM ceiling
> (24000)? Tool flute count? Chipload formula
> `chipload = feed / (rpm × flutes)` ?" None of that surfaces. _Where_:
> badge tooltip in `sim_diagnostics.rs`. _Fix idea_: the chipload
> tooltip should compute and show "Feed 1500 mm/min ÷ (12000 rpm × 2
> flutes) = 0.063 mm/tooth target → bumping feed to 2400 mm/min
> closes the gap, but that exceeds machine max 4000 — try lowering
> spindle to 8000 rpm instead". The optimizer already does this
> calculation; surface a one-line teaser in the verdict tooltip so
> users learn the physics.

> 🟡 **[J9, air-cut warning at project level]** `verdict: "WARNING:
> high air cutting"` (56% on wanaka). Air-cut is mostly a runtime
> waste, not a damage signal — but the GUI verdict word is "WARNING"
> which most users associate with "something will break". _Where_:
> simulator verdict string. _Fix idea_: split verdict severity
> ("INFO: high air cutting (could be faster)" vs "WARNING:
> collision risk").

> 🟡 **[J9, drill peck "unmodeled" — not actionable]** Drill (TP7)
> reports all three gates as `unmodeled / arc_engagement_not_captured`.
> A drill cycle has *no* arc engagement — it's a vertical peck — so
> "arc engagement not captured" is technically true but useless to the
> user. _Where_: tool_load verdict reasons in
> `rs_cam_core/src/tool_load`. _Fix idea_: detect drill ops and emit
> `UnmodeledReason::DrillCycle { rationale: "drill thrust not modeled
> by chip-load gate; verify peck depth and spindle thrust separately" }`
> instead of the generic arc-engagement reason.

## J10 — STEP-specific journeys

> 🔴 **[J10, BREP topology dropped]** All 4 fixture STEP files load with
> `kind=step` and triangulated bbox/vertex/triangle counts present, but
> `inspect_brep_faces` reports "no BREP data (not a STEP model)". So
> face-selective ops are not exercisable from any of the STEP fixtures.
> Either the STEP files were saved without solid-body topology (likely:
> they're 12–36 vertex meshes — they look extruded-from-DXF, not
> proper BREP solids), **or** the loader silently strips BREP and
> keeps only the tessellation. The user can't tell which from the GUI.
> _Where_: import dispatch in `crates/rs_cam_core/src/io.rs:25-115`
> + `step_input.rs`. _Fix idea_: when STEP loads but BREP is absent,
> populate `kind: "step_mesh_only"` and disable the face picker UI
> with the explanation "STEP file has no solid-body topology — face-
> selective ops require AP203/AP214 with `MANIFOLD_SOLID_BREP`".
> Separately: regenerate the fixture STEP files with proper BREP so
> J10 can actually be tested.

> 🟡 **[J10, face picker discoverability]** Couldn't even reach the
> face picker (BREP missing). When a STEP DOES have BREP, the picker
> lives in operation panels — there's no top-level "select a face"
> affordance from the project tree. _Where_: BREP UI plumbing in
> ops panels. (Cannot evaluate further until fixtures are fixed.)

## J11 — Drill carve-outs

> 🟢 **[J11, F3 still works]** Wanaka simulation reports
> `rapid_collision_count: 0` and `collision_count: 0` across all 7
> toolpaths including "Holes" (drill, 216 moves, 744mm cutting). The
> F3 same-XY descent carve-out for peck cycles is holding. (Logged as
> success — explicit no-regression.) _Where_: drill carve-out logic
> per F3 in `rs_cam_core::sim`.

## J12 — Export

> 🟡 **[J12, 7-step wizard for "just give me g-code"]** Export wizard
> has 7 steps (Post / Output layout / Coordinate & units / Tool
> change / Setup pauses / Preview & validate / Save). For a user who
> just wants the g-code for one finished project, that's six clicks
> through screens they'll not change. The Save button only lives in
> step 7. _Where_:
> `crates/rs_cam_viz/src/ui/export_wizard.rs:25-92`. _Fix idea_: add a
> "Quick export" path on the wizard's first screen — Post + filename +
> Save, with the rest defaulting from session — and keep the full
> wizard for users who need to fiddle.

> 🟢 **[J12, gate on Exceeds]** `export_gcode` MCP refuses if any
> toolpath has tool-load Exceeds or Unmodeled verdicts unless the
> caller sets `accept_exceeded_tool_load` / `accept_unmodeled_tool_load`
> explicitly. Forcing the user to acknowledge the bypass is right.
> _Where_: tool definition. (Logged as success — opt-in danger,
> default-safe.)

> 🟢 **[J12, post says what it did]** The wizard has a "Preview &
> validate" step (5) that runs `gcode_validator::validate` and shows
> findings before export. Couldn't run end-to-end here (couldn't
> generate a clean wanaka), but the structure is right.

> 🟢 **[J12.2, Ctrl+Alt+E direct export]** The shortcuts window
> already lists `Ctrl+Alt+E` "Direct export (skip wizard)". So the
> 7-step wizard isn't a forced burden — there's a quick path. The
> issue is **discoverability**: `Ctrl+Alt+E` only appears in the
> shortcuts modal (Help menu somewhere). _Where_:
> `crates/rs_cam_viz/src/ui/shortcuts_window.rs:16`. _Fix idea_: add
> a "Quick export" button next to the wizard launcher in the menu
> bar so the keyboard shortcut isn't the only path.

### J12 second pass — actual export attempts

> 🔴 **[J12.2, export produces 0-byte file]** Reproducible on a
> single-toolpath 2D project: `add_toolpath pocket` →
> `generate_all` (success: "Generated 1 toolpaths") → `run_simulation`
> (success: real metrics including 230s runtime, 73% air-cut,
> 4 hotspots) → `export_gcode --accept_unmodeled --accept_exceeded`
> returns `"G-code exported to /tmp/.../pocket.nc"` and writes a
> **0-byte file**. Same on wanaka with 6/7 generated toolpaths. The
> MCP shell reports success; the user has nothing to load on their
> CNC. _Where_: `crates/rs_cam_core/src/session/compute.rs:1180`
> writes whatever `export_gcode_checked` returned; line 178-194 of
> `gcode/mod.rs` builds `phases` via
> `project.get_result(idx)?` — `filter_map` silently drops toolpaths
> when the result lookup returns `None`. If MCP-generated toolpath
> results don't surface through `get_result`, every phase is
> dropped, an empty `phases` vec emits an empty program, and the
> file is 0 bytes — but the IO write still succeeds. _Fix idea_:
> after building `phases`, refuse to write when empty:
> `if phases.is_empty() { return Err(ExportError::NothingToEmit) }`.
> Separately, audit the get_result path: `generate_all` says it
> generated N toolpaths but `get_result(idx)` may not see them —
> there are likely two diverging caches.

> 🔴 **[J12.2, export gate vs report disagree]** Same wanaka session:
> `get_tool_load_report` returns 5 chipload-exceeds + 2 fully-
> unmodeled with detailed verdicts (e.g. TP12 chipload observed
> 0.001131 mm/tooth vs vendor band 0.025–0.04). But
> `export_gcode` (no flags) refused with "tool load not fully
> modeled for toolpath(s): toolpath 12: chipload=SimulationRequired,
> ...". The export gate sees `SimulationRequired` for the same
> toolpath the inspection report has real chipload data for. Two
> separate evaluations of the tool-load report disagree about the
> same data. _Where_: both call `project_load_report` in
> `gcode/mod.rs:206`, but with different `sim_trace` arguments:
> the inspection path uses `self.simulation.cut_trace` (which
> exists), the export path also passes that same arg. Either the
> simulation cache lives in two places, or the cut_trace is
> consumed/cleared between the two calls. _Fix idea_: instrument
> both call sites to log which `sim_trace` value they receive and
> what `project_load_report` returns. The two should be identical;
> if they're not, the simulation result is being mutated between
> calls.

> 🟡 **[J12.2, GUI metric capture default]** The `Capture cutting
> metrics` checkbox defaults to whatever `metric_options.enabled`
> defaults to (likely false). Without it, the chipload gate has no
> per-sample data to evaluate — so the export gate falls back to
> `SimulationRequired`. The MCP `run_simulation` has no parameter
> to enable metric capture, so an MCP-driven session can never
> produce a passable export gate. _Where_:
> `crates/rs_cam_viz/src/ui/sim_op_list.rs:43-49` (the GUI
> checkbox) + MCP `run_simulation` schema. _Fix idea_: default
> `metric_options.enabled = true` (the cost is trivial vs the
> diagnostic value) AND expose a `capture_metrics: bool = true`
> parameter on the MCP `run_simulation` tool.

### Third-pass extras — multi-setup, MCP type coercion

> 🟡 **[Setup workflow, fresh-stock disclaimer]** The setup card
> displays a "Starts from uncut stock (prior setups not reflected)"
> warning for non-first setups. Honest framing — but it lives only
> on the setup card. The simulation viewport doesn't replay it; if
> the user simulates Setup 2 first they'll see "fresh stock" without
> the warning context unless they look back at the setup card.
> _Where_: `crates/rs_cam_viz/src/ui/setup_panel.rs:221-228`. _Fix
> idea_: when the active sim boundary is a non-first setup AND the
> user-visible playback head is inside that setup, show the same
> warning above the simulation viewport.

> 🟢 **[Setup workflow, move_toolpath ordering]** After moving "Holes"
> from Setup 1 to Setup 2, `list_setups` shows Setup 2's
> `toolpath_indices: [5, 6, 2]` — moved toolpath appended at the
> end, not interleaved by id. Cosmetic, but a user reading "cut
> order" off this list could misjudge sequence. _Where_:
> `move_toolpath_to_setup` ordering. _Fix idea_: insertion-sort by
> id (or surface a separate user-controlled order list).

> 🟢 **[Setup workflow, no face-vs-toolpath check]** Moved a Drill
> toolpath ("Holes") from setup 1 (face_up=Bottom) to setup 2
> (face_up=Top). The drill geometry was authored for the bottom
> face but no warning fires after the move. _Where_:
> `move_toolpath_to_setup` handler. _Fix idea_: when target setup's
> face_up differs from source, surface a yellow "Toolpath geometry
> may need re-verification on the new face" line in the panel.

> 🟡 **[MCP type coercion, set_*_param]** `set_toolpath_param depth=7`
> failed with `invalid type: string "7", expected f64` even though
> `set_toolpath_param stepover=3.7` succeeded with the same JSON
> shape. Boolean coercion: `set_dressup_field link_moves=true`
> failed (string "true" not bool); `set_dressup_field
> entry_style="ramp"` failed (the JSON-quoted form was rejected,
> only the unquoted enum variant `ramp` worked). The MCP value
> deserialization is silently inconsistent across calls. _Where_:
> the `set_*_param` MCP wrappers in `crates/rs_cam_viz/src/mcp/`
> (or wherever the JSON value is deserialized into the param type).
> _Fix idea_: have all `set_*` MCP tools accept `value` as `serde_json::Value`
> and explicitly coerce strings to numbers/bools where the target
> type allows. Document in each tool's description that booleans
> are JSON `true`/`false` (not `"true"`) and enums are bare strings
> (not double-quoted).

> 🟢 **[MCP, save round-trip stable]** Saved a project with edits
> (stepover, entry_style), reloaded, saved again — `diff` shows two
> consecutive saves are byte-identical. v3 round-trip is clean.
> The memory note about `auto_from_model` not surviving was about
> the **legacy** loader (`io/project.rs:763`); v3 path preserves
> the field correctly. (Logged as success.) _Where_:
> `crates/rs_cam_viz/src/io/project.rs:439-451` (current loader).

### Optimizer — actually-ran findings

> 🟡 **[J8.2, three near-identical candidates]** Optimizer on TP10
> (3D Rough 6) returned 3 ranked candidates that all close the
> chipload gate. Their cycle times: 145.5s / 150.4s / 150.7s — a
> 5s spread on a 250s baseline. They differ in `depth_per_pass`
> (3.9 / 6.9 / 3.0) but reach **identical** chipload verdicts (all
> within at observed 0.0600). Three rows competing for the user's
> Apply click when any one would be equivalent. _Where_:
> optimizer search in `crates/rs_cam_core/src/tool_load/optimize/`.
> _Fix idea_: dedupe candidates whose verdicts and (cycle_time
> within 5%) are equivalent — keep the one with the most
> conservative DOC.

> 🟡 **[J8.2, gate_deltas qualitative]** The candidate JSON has
> `gate_deltas: { chipload: "improved", deflection: "same",
> power: "same" }` — labels only, no magnitude. So the user
> can't tell "improved by 10×" from "improved by 1.05×".
> _Where_: `optimize` payload schema. _Fix idea_: add per-gate
> `_factor` fields like `chipload_improvement_factor: 9.71`
> (observed_after / observed_before) so the modal can sort by
> magnitude.

> 🟢 **[J8.2, F1 RPM-down compensation fires]** TP3 (Lakes back
> inside): baseline feed=800, observed chipload 0.0044 (BURN).
> Refined: feed=4000 (machine max) AND spindle=14884 (down from
> 18000). The optimizer co-moved RPM and feed to close the
> chipload gate without exceeding machine feed. F1 fix is
> validated end-to-end. (Logged as success.)

> 🟢 **[J8.2, big wins exist]** TP10 baseline 250s → best
> candidate 145s — **42% time reduction** AND chipload gate
> closes. The optimizer is unambiguously useful when it can run.
> (Logged as success.) The J8 batch-optimize finding stands —
> getting all 5 wanaka toolpaths through this loop currently
> means 5 modal sessions of 1–2 min each.

---

## Summary

Findings by surface (counts include all 12 journeys above):

| Surface | 🔴 | 🟡 | 🟢 |
|---|---:|---:|---:|
| Project loader / MCP wrapper | 1 | 3 | 1 |
| Stock panel | 0 | 1 | 3 |
| Tool panel | 0 | 2 | 1 |
| Operation defaults (params + dressup + boundary) | 1 | 9 | 7 |
| Optimizer (modal + ran end-to-end) | 0 | 4 | 3 |
| Save/load + multi-setup | 0 | 1 | 3 |
| MCP type coercion | 0 | 1 | 0 |
| Export end-to-end | 2 | 2 | 1 |
| Sim_diagnostics (right panel) | 1 | 4 | 2 |
| Tool-load report (verdict badges) | 0 | 3 | 1 |
| Optimizer modal (source review only) | 0 | 2 | 1 |
| Error reasoning / verdict text | 0 | 3 | 0 |
| STEP / BREP loader | 1 | 1 | 0 |
| Drill carve-out (F3) | 0 | 0 | 1 |
| Export wizard | 0 | 1 | 2 |
| Toolpath panel (generation status) | 0 | 1 | 0 |
| **Totals** | **6** | **38** | **26** |

### Next investment target

**REVISED after fourth pass:** the export end-to-end surface produces
2 🔴s (silent 0-byte file + gate-vs-report disagreement) and is now
**tied for top priority with operation defaults**.

| Tied target | Why |
|---|---|
| Operation defaults | 17 bullets, compounds into J7 BURN cluster, blocks first-time-user success |
| Export end-to-end | 2 🔴s — the user can build a perfect project, hit Export, get a 0-byte file with a "success" message and walk to the CNC empty-handed |

Of the two, **export should be fixed first** — operation defaults make
the experience worse, but a 0-byte g-code with a success message is the
kind of bug that erodes trust. Even one production user hitting it is
catastrophic.

The original recommendation:

**Operation defaults (`crates/rs_cam_core/src/operations/*/config.rs`).**

After two extra passes this surface owns 17 bullets (1🔴 + 9🟡 + 7🟢) —
by far the densest cluster — and the failure mode compounds: every new
toolpath inherits broken defaults, which then drives the per-toolpath
chipload-low verdicts (the J7 "all 5 toolpaths exceed chipload low"
project-wide signal). The biggest single lever is fixing the **dressup
defaults**: `entry_style=None` (vertical plunge), `link_moves=false`,
`feed_optimization=false`, `optimize_rapid_order=false` together turn
every fresh toolpath into a worst-case path that's chipload-low on
entry, full of air-cut between regions, and slow. Add to that the
op-config defects — `drop_cutter min_z=-50`, `face depth=0`, hard-coded
2D depths — and the editing burden on every new toolpath becomes the
defining first-time-user experience. Fixing this stack would:

1. Eliminate the two most dangerous defaults (drop_cutter chasing 20 mm
   below stock floor — 🔴; face producing zero-cut output — 🟡).
2. Cut the editing-on-every-toolpath churn (J4 🟡 cluster).
3. Replace vertical plunges with Ramp/Helix on pocket/adaptive ops by
   default — directly addresses the "BURN" badges the chipload-low
   gate fires on every entry.
4. Reduce the volume of chipload-low BURN verdicts the user sees in J7,
   which would in turn reduce the false-positive load on the optimizer
   (J8 batch-optimize problem).

Sim_diagnostics has 7 bullets (1🔴 + 4🟡 + 2🟢) but they're mostly
framing/labelling fixes — visible payoff, but they don't propagate
upstream the way operation defaults do.

The 🔴 BREP-topology finding is real but is partly a fixture problem; it
needs proper STEP fixtures regenerated before another UX session can
evaluate the face-picker UI.

---

# Implementation roadmap

Compiled from 5 parallel cam-navigator RCAs (2026-05-11). Each section
is implementation-ready: root cause + specific edits + tests + risk.

> **Important corrections from RCA — read first**
>
> Several findings in the journey sections above were partially wrong;
> the RCAs corrected them:
>
> - **STEP/BREP fixtures are NOT mesh-only.** They are valid AP203 BREP
>   solids with `MANIFOLD_SOLID_BREP` records. The fixture-bug
>   hypothesis is refuted; the loader is the sole cause. (J10)
> - **`feed_rate` defaults are already moot in GUI sessions.**
>   `calculate_and_apply_feeds` (`properties/mod.rs:964`) overwrites
>   static defaults every frame the Feeds tab is visible. The real
>   gap is MCP-only sessions that never touch that tab. (J4)
> - **`dressup entry_style` is partly already wired.** `for_op` ⇒
>   `for_role` already sets `Ramp` for SemiFinish/Finish; the gap is
>   only the Roughing role branch. (J4.3)
> - **Bug A (0-byte export) and Bug B (gate-vs-report disagreement)
>   share one root cause** — `mcp_export_gcode` calls into the wrong
>   layer. One fix closes both. (J12.2)
> - **`inspect_brep_faces` 0-vs-1 indexing isn't a bug.** Model `id`
>   is an opaque DB-assigned ID; the agent confused it with index.
>   Doc-only fix. (J1)
> - **`ToolLoadReport.summary()` already exists** with toolpath
>   denominators (`verdict.rs:139-253`). The GUI just doesn't call it;
>   instead `verdict_counts_local` re-counts at gate-cell granularity.
>   Trivial swap. (J6)

## Roadmap A — Export end-to-end (TIED #1 priority — 2🔴)

Both J12.2 🔴 bugs share root cause: `mcp_export_gcode`
(`crates/rs_cam_viz/src/app/mcp.rs:1852`) calls
`session.export_gcode_with_policy` (core), which reads from
`session.results` and `session.simulation`. But MCP-generated toolpath
results live in `gui.toolpath_rt[id].result` (viz worker output, see
`controller/events/compute.rs:340`), and viz simulation results live
in `state.simulation.results.cut_trace` (set at `controller/events/
compute.rs:465-476`). `session.run_simulation()`
(`session/compute.rs:943`) is never called by the GUI/MCP path. So:
- `phases` filter_map at `gcode/mod.rs:178-194` drops every toolpath
  → empty program → `fs::write(path, "")` succeeds → 0-byte file.
- `sim_trace` is `None` → chipload always `SimulationRequired` → gate
  blocks export despite `get_tool_load_report` showing real data.

`metric_options.enabled` defaults to `false` (`simulation_cut.rs:12`)
but `mcp_run_simulation` already forces it to `true`
(`app/mcp.rs:2377-2379`), so the cut_trace is always present in the viz
results — only the wrong cache is consulted at export time.

### Fix

- `crates/rs_cam_viz/src/app/mcp.rs:1841-1852` — replace the
  `session.export_gcode_with_policy(...)` call with
  `crate::io::export::export_gcode_from_session(&state.session,
  &state.gui, &state.simulation)` (the same path the GUI export
  wizard uses). This routes through `gcode_phase_for_session_toolpath`
  at `crates/rs_cam_viz/src/io/export.rs:46`, which reads from
  `gui.toolpath_rt[id].result`, and through `viz_sim_trace(sim)` at
  `io/export.rs:84-86`, which reads `state.simulation.results.cut_trace`.
- Thread the `accept_unmodeled` / `accept_exceeded` flags via the
  `gui.tool_load_overrides` field (set before the call, restore after);
  cleaner alternative is a new `export_gcode_from_session_with_policy`
  variant — preferred long-term.

### Tests

- Add an integration test that mirrors the reproduction sequence:
  load `test_data/ux_2d_pocket.toml` (paths fixed), `add_toolpath`
  pocket, `generate_all`, `run_simulation`, `export_gcode`, assert
  output file is non-empty.
- Existing `controller/tests.rs:398` (`export_gcode`) covers the GUI
  path and would catch a regression in that direction.

### Risk

Low. No other callers of the MCP export path. The override-then-restore
pattern is slightly fragile; the dedicated-policy variant is preferred.

---

## Roadmap B — Operation defaults (TIED #1 priority — 1🔴 + cluster)

After RCA, the cluster shrinks dramatically. The framework is mostly
right; specific branches and one MCP path need patches.

### B.1 — `drop_cutter min_z` 🔴

Root cause: `crates/rs_cam_core/src/compute/operation_configs.rs:436`
hardcodes `min_z: -50.0`. Constructor `OperationConfig::new_default(op_type)`
at `crates/rs_cam_core/src/compute/catalog.rs:864` takes only an
`OperationType` with no stock context. Both call sites
(`controller/events/toolpath.rs:51`, `app/mcp.rs:1990`) already have
`&self.state.session` in scope.

**Fix:** widen `new_default` to
`new_default(op_type, ctx: NewDefaultCtx<'_>)` where `NewDefaultCtx`
carries `stock_bottom_z`, `stock_top_z`, `stock_z`, `material`. In
`DropCutterConfig::default`-equivalent path, set
`min_z: ctx.stock_bottom_z.unwrap_or(-50.0)`.

**Tests:**
- `crates/rs_cam_core/tests/end_to_end.rs:59` asserts `cl.z >= -50.0
  - 1e-6` — must update to track new default.
- `crates/rs_cam_core/tests/fixtures/test_job.toml:849` serializes
  `min_z = -50.0` — round-trip test consumer; update fixture or leave
  the explicit override to test the override path.

### B.2 — `face depth = 0` 🟡

Root cause: `operation_configs.rs:97`. `0` is a valid sentinel ("single
pass at stock top") per the comment in `face.rs:30`, but produces a
do-nothing toolpath.

**Fix:** static-value change to `depth: 1.0` is acceptable
without widening `new_default`. With the widened ctx, prefer
`ctx.stock_padding.unwrap_or(1.0)` (the stock-top minus model-top
margin).

### B.3 — Pocket / Profile / Drill / Adaptive `depth` 🟡

Roots: `operation_configs.rs:229` (Pocket=3), `:263` (Profile=6),
`:147` (Drill=10), `:295` (Adaptive=6). All disconnected from stock.

**Fix:** with the widened ctx:
- `ProfileConfig::depth = ctx.stock_z` (full-through).
- `DrillConfig::depth = ctx.stock_z`.
- `PocketConfig::depth = (ctx.stock_z * 0.5).min(5.0)` (half-stock,
  capped at 5mm).
- `AdaptiveConfig::depth = ctx.stock_z * 0.5`.

### B.4 — `feed_rate` defaults (MCP-only gap) 🟡

Root cause: GUI sessions are auto-corrected by `calculate_and_apply_feeds`
at `crates/rs_cam_viz/src/ui/properties/mod.rs:964` — runs every frame
the Feeds tab is visible, writes back into the op config when
`FeedsAutoMode` flags are `true` (default). MCP sessions never trigger
this. `mcp_add_toolpath` (`app/mcp.rs:1990`) leaves the static
constants in place.

**Fix:** in `mcp_add_toolpath`, after constructing the config, call
`feeds::calculate(&tool, &material, &machine, &op_cfg)` and write the
result back into the op fields where the corresponding `feeds_auto.*`
flag is `true`. This mirrors what the GUI does on first display.

### B.5 — Dressup `entry_style` for Roughing role 🟡

Root cause: `DressupConfig::for_op` at `compute/config.rs:403` calls
`for_role`. `for_role(Roughing)` at `:379` sets `link_moves: true`,
`arc_fitting: true`, `optimize_rapid_order: true` but does NOT set
`entry_style` — so it stays `None` from the base default. Pocket /
Profile / Adaptive / Face / Adaptive3d are all in Roughing role, hence
all get vertical plunge.

**Fix:** in `for_role(Roughing)` at `config.rs:379`, add
`entry_style: DressupEntryStyle::Ramp`. Then in `normalize_for_op`
at `:428-455`, add an Adaptive/Adaptive3d override to change to
`Helix`, and a Drill/Trace override to force back to `None`.

### B.6 — Dressup pure-flips (3 fields) 🟡 + 🟢

`config.rs:359` `link_moves: false` → `true`. `normalize_for_op`
already strips link_moves for ProjectCurve and DropCutter, so the flip
is safe for everything else.

`config.rs:364` `feed_optimization: false` → `true`. Optimization can
only reduce feed; never exceeds user value.

`config.rs:367` `optimize_rapid_order: false` → `true`. Pure win,
nearest-neighbour rapid reorder, never changes cutting order.

**Tests:** existing fixtures (`test_job.toml:309, 314, 317, 401, 406,
493, 498, 602, 607, 610`) explicitly serialize these as `false`, so
round-trip continues to pass. `crates/rs_cam_viz/src/io/project.rs:1514-
1539` explicitly sets `feed_optimization = true` and asserts round-trip
— still passes.

### B.7 — Boundary `enabled = false` 🟢

`compute/config.rs:277`. For 3D ops on small models in oversized stock,
the cutter sweeps over the entire stock area.

**Fix:** in `OperationConfig::new_default`, when op type is 3D
(adaptive3d / drop_cutter / scallop / waterline / ramp_finish /
horizontal_finish / pencil / steep_shallow / spiral_finish) AND ctx
has a 3D model bbox, set `boundary.enabled = true,
boundary.source = ModelSilhouette`.

### B.8 — Stepover units hint 🟡

UI-only. `validate_operation` at `properties/operations/mod.rs:2031-2043`
already computes `(stepover / tool.diameter) * 100.0`. The `dv` helper
at `properties/mod.rs:2695` calls `tooltip_for("Stepover")` for static
tooltip text. Stepover dv calls live at `boundary_2d.rs:26, 69, 205,
249, 293, 343, 374` and `surface_3d.rs:17, 45, 391`.

**Fix:** add a post-`dv` dim label `"= XX% of tool dia"` after each
stepover DragValue, computing the percent live.

### B.9 — adaptive3d 17-param wall + steep_shallow 11-param wall 🟢

UI-only. `draw_adaptive3d_params` at
`crates/rs_cam_viz/src/ui/properties/operations/surface_3d.rs:37`.
`draw_steepshallow_params` in `finishing.rs`.

**Fix:** wrap advanced params in `egui::CollapsingHeader::new("Generation
strategy (advanced)")` default-collapsed:
- adaptive3d advanced: `entry_style`, `helix_*`, `ramp_angle_deg`,
  `region_ordering`, `z_blend`, `fine_stepdown`, `detect_flat_areas`,
  `min_cutting_radius`, `clearing_strategy`.
- steep_shallow advanced: `overlap_distance`, `wall_clearance`,
  `steep_first`, `sampling`, `stock_to_leave`, `tolerance`,
  `threshold_angle`.

---

## Roadmap C — Sim diagnostics + tool-load badges (1🔴 + 4🟡)

### C.1 — Issue count partition 🔴

Root cause: `crates/rs_cam_viz/src/ui/sim_diagnostics.rs:496-601`. Single
`issue_count = sim.issues(gui, max_feed).len()` rendered as one row.
`SimulationIssueKind` at `crates/rs_cam_viz/src/state/simulation.rs:63-69`
already has 6 variants, and `SimulationIssue` carries the `kind`
tag. CLAUDE.md confirms the air-cut bucket is noise.

**Fix:** at `sim_diagnostics.rs:596-601`, replace single Issues row
with two visual groups:
```rust
// Must address
for kind in [RapidCollision, HolderCollision, Hotspot] {
    let count = issues.iter().filter(|i| i.kind == kind).count();
    if count > 0 { /* render in theme::ERROR */ }
}
ui.separator();
// Informational
for kind in [LowEngagement, AirCut] {
    let count = issues.iter().filter(|i| i.kind == kind).count();
    /* render in theme::TEXT_MUTED */
}
```
Remove the existing separate Hotspot row at lines 603-608 (sourced
from `cut_trace.hotspots.len()`) — the new per-kind grid covers it.
Skip `Annotation` variant (low-volume, already missing label branch).

**Tests:** load wanaka, run sim, expect AirCut ~24,800, Hotspot ~14,
RapidCollision/HolderCollision 0.

### C.2 — Mixed denominator "Within bounds: 0" 🟡

Root cause: `verdict_counts_local` at `sim_diagnostics.rs:685-708`
iterates `report.per_toolpath × 3 gates` — gate-cell denominator. But
`ToolLoadReport.summary()` at `crates/rs_cam_core/src/tool_load/
verdict.rs:139-253` already returns toolpath-denominated counts.

**Fix:** in `draw_project_overview` at `sim_diagnostics.rs:498`,
replace
```rust
let (ok, _warn, bad, unmodeled) = verdict_counts_local(load_report);
```
with
```rust
let summary = load_report.summary();
let (ok, bad, unmodeled) = (summary.within, summary.exceeds, summary.fully_unmodeled);
```
Update labels to "TPs within bounds" / "TPs exceeding" /
"TPs fully unmodeled" so the denominator is explicit.
`verdict_counts_local` becomes dead code; remove it.

### C.3 — BURN tooltip uses wrong bound 🟡

Root cause: `draw_tool_load_badges` at `sim_diagnostics.rs:724-765`
threads `chipload_cap = range.end` (LUT max — the breakage ceiling) to
`verdict_badge` and `verdict_tooltip`. For burn-risk (chipload-low),
the relevant bound is `min_mm_per_tooth` (the floor), not the cap.
The min is on `ChiploadVerdict::Exceeds { triggering: { bounds: { min_mm_per_tooth } } }`
but isn't extracted.

**Fix:** in `draw_tool_load_badges`, when `burn_risk` is true, extract
`min_mm_per_tooth` from `verdict.chipload` and pass it as the cap
(or as a dedicated `floor` parameter — preferred). In `verdict_tooltip`
at `sim_diagnostics.rs:841-844`, extend the burn-risk branch to a
two-sentence explanation:

> "EXCEEDS: chipload below vendor min — rubbing/burning risk. At low
> chipload the tool edge rubs instead of cutting; friction generates
> heat that glazes and burns the wood. Increase feed rate or reduce
> RPM. (peak 0.0110 / floor 0.0500, validated)"

### C.4 — Hotspot triage list 🟡

Root cause: `draw_focused_hotspot_card` at `sim_diagnostics.rs:347-419`
is single-card via `sim.focused_hotspot_data()`. No top-N list. But
`SimulationCutHotspot.wasted_runtime_s` is already a field — sortable.

**Fix:** add a `draw_hotspot_triage_list` function that renders a
collapsible "Top hotspots" section after the Findings grid (after
line 609). Source: `sim.results.cut_trace.hotspots`. Sort by
`wasted_runtime_s` desc, take top 10. Render as selectable rows
identical to the in-scope list at lines 1178-1193 (click → focus +
jump). Also: sort the existing in-scope list at `sim_diagnostics.rs:1174`
by `wasted_runtime_s` before `.take(MAX_ROWS)`.

### C.5 — chipload tooltip with machine-context calc 🟡

Two-step fix:
- **Minimal (no plumbing):** add a sentence to the chipload tooltip
  branches at `sim_diagnostics.rs:820-886`: "Chipload = feed ÷ (RPM
  × flutes). Raise feed or lower RPM to increase chipload."
- **Full (recommended):** thread `Option<ChiploadContext>`
  (feed_mm_min, rpm, flutes) from the toolpath config into
  `draw_tool_load_badges` so the live values render in the tooltip.

### C.6 — Verdict text as banner 🟡

Root cause: `verdict` lives only in the MCP response, built at
`crates/rs_cam_viz/src/controller/events/compute.rs:931-937`. There
is no `simulation::verdict` module; the strings are inline. Three
possible: `"WARNING: rapid collisions detected"`, `"WARNING: high
air cutting"`, `"OK"`.

**Fix:** in `draw_project_overview` after line 558, add a coloured
heading line that mirrors the same rule: if `collision_count > 0`
render in `theme::ERROR`; else if total air-cut > 20% in
`theme::WARNING`; else `theme::SUCCESS`. Read existing
`collision_count` / air-cut values already computed at lines 499-505
— no duplicate computation.

---

## Roadmap D — STEP/BREP loader (1🔴)

Single root cause: `crates/rs_cam_core/src/session/project_file.rs:567-576`
calls `load_step` (which correctly returns an `EnrichedMesh` with face
groups), then deliberately downgrades to a flat `TriangleMesh` via
`(*enriched.mesh).clone()` and at line 745 sets `enriched_mesh: None`
on the resulting `LoadedModel`. The parallel loader
`crates/rs_cam_core/src/io.rs:90-108` (`load_model_file`, used by GUI
drag-drop and `add-model`) does it correctly:
`enriched_mesh: Some(Arc::new(enriched))`.

This is the same two-loader divergence pattern as the project memory
note. `inspect_brep_faces` guard at
`crates/rs_cam_viz/src/app/mcp.rs:1475` and
`crates/rs_cam_mcp/src/server.rs:1785` gates on
`model.enriched_mesh.is_none()`, so any model loaded through
`ProjectSession::load` silently lacks topology.

**Fixtures are NOT the problem** — `fixtures/gui_step/block_40x40x60.step`
is a valid AP203 BREP solid with `MANIFOLD_SOLID_BREP` (#16),
`CLOSED_SHELL` (#17), six `FACE_SURFACE` records (#18–#53),
`EDGE_CURVE` / `VERTEX_POINT` topology, `PLANE` surfaces. Generated
by truck. No regeneration needed.

### Fix

- `crates/rs_cam_core/src/session/project_file.rs:567-576` (the
  `ModelKind::Step` arm of `load_model_geometry`): introduce a
  `LoadedGeometry::Enriched(EnrichedMesh)` variant (or inline the
  enriched mesh into the LoadedModel construction at lines 737-748).
- `crates/rs_cam_core/src/session/project_file.rs:745` set
  `enriched_mesh: Some(Arc::new(enriched))` on the Step path.
- `LoadedGeometry` is internal to that file — non-public ABI change.

### Defensive UI fix (still worth landing even after the loader fix)

When `enriched_mesh` is `None` on a `kind = Some(ModelKind::Step)`
model, disable the face picker with a tooltip. The gating field
`has_enriched_mesh` is already threaded into the UI at
`crates/rs_cam_viz/src/ui/properties/operations/mod.rs:1643` and
checked at `:1678, :1723, :1899`. Add a visible warning row when
`kind == Step && !has_enriched_mesh`: "BREP not loaded (reload model)".

### Tests

- Integration test in `crates/rs_cam_core/tests/`: load a TOML
  referencing `fixtures/gui_step/*.step`, assert
  `model.enriched_mesh.is_some()` and
  `model.enriched_mesh.unwrap().face_count() > 0`.
- MCP smoke: `load_project` → `inspect_brep_faces(model_id=1)` must
  return face data, not "no BREP data".
- Confirm `inspect_model.triangle_count` is unchanged.

### Risk

Any existing project with persisted face_selection fields loaded
through this path was previously operating on an empty enriched mesh
(silent no-op). After fix, face lookups resolve correctly — no
regression expected.

---

## Roadmap E — MCP layer (1🔴 + 5🟡 + 1🟢)

### E.1 — `load_project` warnings channel 🔴

Root cause: `mcp_load_project` at `crates/rs_cam_viz/src/app/mcp.rs:1812`
returns only `"Loaded '{name}' -- {N} setups, {M} toolpaths"`. The
controller's `open_job_from_path` at `crates/rs_cam_viz/src/controller/
io.rs:166` collects warnings into `self.load_warnings` (lines 223,
261) and sets `self.show_load_warnings`. The GUI renders them in a
modal; the MCP wrapper never reads the field.

**Fix:** after the `Ok(())` arm, read `self.controller.load_warnings`
(already populated) and append to the response string:
```
"Loaded 'X' -- N setups, M toolpaths\nWarnings:\n  - <warning 1>\n  - <warning 2>"
```

### E.2 — ERR chip swallows error message 🟡

Root cause: `crates/rs_cam_viz/src/ui/toolpath_panel.rs:316` matches
`ComputeStatus::Error(_) => ("ERR", theme::ERROR)` with wildcard. The
`ui.label` at line 318 has no `.on_hover_text`.

**Fix:** capture the inner string and chain hover:
```rust
ComputeStatus::Error(msg) => ("ERR", theme::ERROR, Some(msg.as_str())),
// ...
let resp = ui.label(egui::RichText::new(status_text).small().strong().color(status_color));
if let Some(msg) = err_msg { resp.on_hover_text(msg); }
```

### E.3 — `list_toolpaths` missing stale flag 🟡

Root cause: `ToolpathSummary` struct at
`crates/rs_cam_core/src/session/mod.rs:413` only carries
`index/id/name/operation_label/enabled/tool_name`. Staleness lives in
`ToolpathRuntime.stale_since: Option<Instant>` at
`crates/rs_cam_viz/src/state/runtime.rs:32` — viz-only. `mcp_list_toolpaths`
at `app/mcp.rs:588` delegates to `session.list_toolpaths()` and never
augments with viz state.

**Fix:** in `mcp_list_toolpaths`, override the listing to zip core
summaries with `state.gui.toolpath_rt`, injecting `"stale": rt.stale_since.is_some()`
and `"status": rt.status.label()` per row.

### E.4 — `generate_all` "No result produced" opaque 🟡

Root cause: `crates/rs_cam_viz/src/controller/events/compute.rs:776`.
When a toolpath completes with `rt.result.is_none()`, the code matches
on `rt.status`: if `Error(e)` uses `e.clone()`, otherwise falls
through to literal `"No result produced"`. The fall-through fires
when status is `Pending` / `Computing` / `Done` (i.e. completion event
arrived before status updated to Error, or generator silently produced
zero moves).

**Fix:** richer match on `rt.status`:
- `Error(e)` → `e.clone()` (current behaviour)
- `Done` → `"Completed with no moves — check depth, stock, or model assignment"`
- other → format with toolpath name + status label

### E.5 — `inspect_brep_faces` 0-vs-1 indexing — DOC ONLY 🟢

Root cause: `model.id` is an opaque DB-assigned ID (typically starting
at 1, incremented per import), NOT a 0-based positional index.
`inspect_model` returns the same opaque ID. The "0 vs 1" confusion was
the agent calling `inspect_brep_faces(model_id=0)` when the model's ID
is `1` — caller mistake, not a code bug.

**Fix:** doc-only. Update `ModelIdParam` in `crates/rs_cam_mcp/src/server.rs`
with `/// Model ID as returned by inspect_model (not a 0-based index)`.
Update the tool description in `mcp_server.rs` to say "use the `id`
field from `inspect_model`".

### E.6 — MCP type coercion (3 sub-bugs) 🟡

Root cause splits into three:

**(a)** `set_toolpath_param` at
`crates/rs_cam_core/src/session/compute.rs:74` only applies `as_number`
string coercion in 4 explicit match arms (`feed_rate`, `plunge_rate`,
`stepover`, `depth_per_pass`). Op-specific fields (`depth`, `cut_depth`,
etc.) fall to the `_` wildcard at line 143 which does raw serde
round-trip — a JSON string `"7"` fails to deserialize into f64.
`stepover=3.7` worked because it arrived as JSON number, not string.

**(b)** `set_dressup_field` at
`crates/rs_cam_core/src/session/mutation.rs:203` reads `value`
verbatim and does `obj.insert + serde_json::from_value`. No `0/1 →
bool` coercion (which `set_toolpath_param`'s wildcard at line 166
*does* have for booleans).

**(c)** `value="ramp"` rejected because the JSON-RPC client wrapped
the string twice (became `"\"ramp\""`); `value=ramp` (bare JSON
string) succeeded. Schema gives no guidance.

**Fix:**
- Wildcard in `set_toolpath_param` (compute.rs ~line 166): extend
  coercion to parse numeric strings into JSON numbers when target
  field is numeric:
  ```rust
  if let serde_json::Value::String(s) = &value {
      if existing.map_or(true, |v| v.is_number()) {
          if let Ok(n) = s.parse::<f64>() { value = json!(n); }
      }
  }
  ```
- `set_dressup_field` (mutation.rs:203): add symmetric `0/1 → bool`
  coercion matching `set_toolpath_param`'s pattern.
- Schema doc on `SetDressupFieldParam.value`: explicitly state enum
  values should be passed as bare strings (e.g. `"ramp"` arriving as
  JSON string, not `"\"ramp\""`).

---

## Out-of-roadmap items

These bullets from the report are not in the roadmap because:

- **J1 stock auto-resize on load (🟡)** — fix is one call to
  `update_stock_from_bbox` in the project loader after model attach;
  trivial, hasn't been RCA'd separately because the call path is
  already documented at `controller/events/model.rs:371`.
- **J3 LUT-sourced field indicator (🟡)** — needs a new chip
  rendering pattern in the tool panel; no architectural blocker, but
  not high-leverage. Defer.
- **J7 all-low chipload project banner (🟡)** — pattern same as
  C.6 verdict banner. Fold into that work.
- **J7 deflection peak 107µm warn band (🟡)** — pattern same as
  C.3 BURN tooltip work. Fold in.
- **J8 batch optimize / 3-near-identical dedupe / gate_deltas
  magnitude (🟡)** — single coherent optimizer-modal workstream.
  Each is a separate edit but requires the same context. Recommend
  one PR covering all three.
- **Multi-setup face-vs-toolpath warning (🟢)** — small additive
  check in `move_toolpath_to_setup` handler. One-line `if`.
- **Stock pin-overlaps-model warning (🟡)** — needs a polygon
  bbox check; trivial. Defer.

---

## Suggested PR sequencing

1. **PR 1 — Export fix (Roadmap A).** Single call-site change in
   `mcp.rs`. Closes both 🔴s. ~1 day. Highest trust impact.
2. **PR 2 — STEP BREP loader fix (Roadmap D).** Two-line edit in
   `project_file.rs` + integration test. Unblocks J10. ~1 day.
3. **PR 3 — MCP layer cleanups (Roadmap E.1-E.5).** Five small fixes,
   one PR. ~1 day.
4. **PR 4 — Sim_diagnostics framing (Roadmap C).** Six related UI
   changes in one file. ~2 days.
5. **PR 5 — Operation defaults (Roadmap B).** Bigger PR — widens
   `new_default` signature, touches multiple op configs and tests.
   ~3-4 days.
6. **PR 6 — MCP type coercion (Roadmap E.6).** Independent of the
   above. ~1 day.
7. **PR 7 — Optimizer modal triple (J8 deferred bullets).** ~2 days.
8. **PR 8+ — Polish:** stock pin overlap, LUT chip, face-vs-toolpath
   warning, etc.

Total estimated work: ~12-15 dev-days for the high-impact stack
(PRs 1-6). Polish items add another week.

## Closed by PR

| PR  | Roadmap   | Bullets closed                                       | Severity delta  |
| --- | --------- | ---------------------------------------------------- | --------------- |
| 1   | A         | J12.2 0-byte export, J12.2 gate-vs-report disagree   | -2🔴            |
| 2   | D         | J10 STEP loader strips BREP (+ defensive UI warning) | -1🔴            |


