# Legacy / compat / migration residue in production sources (417 lines, 119 files)

Ruling: no legacy project-format support; judge every hit. Historical notes that say something WAS removed are fine; code that still reads or writes an old shape is not.

## Per file

| hits | file |
|---|---|
| 19 | `crates/rs_cam_core/src/finish/finish_setup.rs` |
| 19 | `crates/rs_cam_core/src/compute/operation_configs.rs` |
| 18 | `crates/rs_cam_core/src/feeds/suggest/tests.rs` |
| 17 | `crates/rs_cam_core/src/feeds/vendor_lookup.rs` |
| 16 | `crates/rs_cam_core/src/feeds/vendor_lut.rs` |
| 11 | `crates/rs_cam_core/src/feeds/rationale.rs` |
| 10 | `crates/rs_cam_core/src/dexel_stock/stamping.rs` |
| 10 | `crates/rs_cam_core/src/stock/dexel_mesh_mc/stride_tests.rs` |
| 10 | `crates/rs_cam_viz/src/ui/properties/linking_dressup.rs` |
| 9 | `crates/rs_cam_core/src/adaptive3d/path.rs` |
| 8 | `crates/rs_cam_core/src/adaptive3d/tests.rs` |
| 8 | `crates/rs_cam_core/src/finish/unified_finish.rs` |
| 8 | `crates/rs_cam_core/src/adaptive/path.rs` |
| 8 | `crates/rs_cam_core/src/gcode/mod.rs` |
| 7 | `crates/rs_cam_core/src/diagnostics/ids.rs` |
| 7 | `crates/rs_cam_core/src/diagnostics/adapters/from_static_checks.rs` |
| 7 | `crates/rs_cam_core/src/machine/kinematics.rs` |
| 7 | `crates/rs_cam_core/src/finish/pencil/emission.rs` |
| 7 | `crates/rs_cam_core/src/stock/simulation_cut.rs` |
| 7 | `crates/rs_cam_core/src/compute/config.rs` |
| 6 | `crates/rs_cam_core/src/toolpath.rs` |
| 6 | `crates/rs_cam_core/src/diagnostics/tests.rs` |
| 6 | `crates/rs_cam_core/src/finish/surface_link.rs` |
| 6 | `crates/rs_cam_core/src/budget/grid.rs` |
| 6 | `crates/rs_cam_core/src/compute/catalog/registry.rs` |
| 5 | `crates/rs_cam_core/src/session/rest_stock.rs` |
| 5 | `crates/rs_cam_core/src/dressup/entry_descent.rs` |
| 5 | `crates/rs_cam_core/src/material/mod.rs` |
| 5 | `crates/rs_cam_core/src/gcode/post.rs` |
| 4 | `crates/rs_cam_core/src/session/project_file.rs` |
| 4 | `crates/rs_cam_core/src/tool_load/verdict.rs` |
| 4 | `crates/rs_cam_core/src/feeds/mod.rs` |
| 4 | `crates/rs_cam_core/src/finish/steep_shallow.rs` |
| 4 | `crates/rs_cam_core/src/dressup/feed_modulation.rs` |
| 4 | `crates/rs_cam_core/src/adaptive/mod.rs` |
| 3 | `crates/rs_cam_core/src/diagnostics/adapters/from_project_diagnostics.rs` |
| 3 | `crates/rs_cam_core/src/session/mod.rs` |
| 3 | `crates/rs_cam_core/src/maps/finish_surface_cache.rs` |
| 3 | `crates/rs_cam_core/src/adaptive3d/mod.rs` |
| 3 | `crates/rs_cam_core/src/adaptive3d/clearing.rs` |
| 3 | `crates/rs_cam_core/src/machine/mod.rs` |
| 3 | `crates/rs_cam_core/src/feeds/profile.rs` |
| 3 | `crates/rs_cam_core/src/feeds/suggest.rs` |
| 3 | `crates/rs_cam_core/src/stock/dexel_mesh.rs` |
| 3 | `crates/rs_cam_core/src/compute/catalog/tests.rs` |
| 3 | `crates/rs_cam_core/src/gcode/program_builder.rs` |
| 3 | `crates/rs_cam_viz/src/ui/components/pill.rs` |
| 2 | `crates/rs_cam_cli/src/project.rs` |
| 2 | `crates/rs_cam_core/src/export/ribbon.rs` |
| 2 | `crates/rs_cam_core/src/tool_load/locality.rs` |
| 2 | `crates/rs_cam_core/src/feeds/tests.rs` |
| 2 | `crates/rs_cam_core/src/feeds/suggest/adaptive_entry.rs` |
| 2 | `crates/rs_cam_core/src/finish/ramp_finish.rs` |
| 2 | `crates/rs_cam_core/src/finish/pencil.rs` |
| 2 | `crates/rs_cam_core/src/finish/scallop.rs` |
| 2 | `crates/rs_cam_core/src/finish/unified_finish/tests.rs` |
| 2 | `crates/rs_cam_core/src/dressup/mod.rs` |
| 2 | `crates/rs_cam_core/src/dressup/tests.rs` |
| 2 | `crates/rs_cam_core/src/dressup/arcfit.rs` |
| 2 | `crates/rs_cam_core/src/surface/reach.rs` |
| 2 | `crates/rs_cam_core/src/trace/narrate.rs` |
| 2 | `crates/rs_cam_core/src/stock/dexel_mesh_mc.rs` |
| 2 | `crates/rs_cam_core/src/stock/collision.rs` |
| 2 | `crates/rs_cam_core/src/compute/execute.rs` |
| 2 | `crates/rs_cam_core/src/compute/execute/project_curve_chaining.rs` |
| 2 | `crates/rs_cam_core/src/gcode/emitter.rs` |
| 2 | `crates/rs_cam_viz/src/ui/sim_op_list.rs` |
| 2 | `crates/rs_cam_viz/src/ui/tokens.rs` |
| 2 | `crates/rs_cam_viz/src/app/mcp/simulation.rs` |
| 1 | `crates/rs_cam_cli/src/sweep.rs` |
| 1 | `crates/rs_cam_cli/src/job.rs` |
| 1 | `crates/rs_cam_core/src/ids.rs` |
| 1 | `crates/rs_cam_core/src/diagnostics/mod.rs` |
| 1 | `crates/rs_cam_core/src/diagnostics/adapters/from_generation.rs` |
| 1 | `crates/rs_cam_core/src/diagnostics/adapters/from_stale_default.rs` |
| 1 | `crates/rs_cam_core/src/session/multitool.rs` |
| 1 | `crates/rs_cam_core/src/session/save.rs` |
| 1 | `crates/rs_cam_core/src/session/diagnostics_types.rs` |
| 1 | `crates/rs_cam_core/src/session/mutation/config.rs` |
| 1 | `crates/rs_cam_core/src/session/compute/simulation.rs` |
| 1 | `crates/rs_cam_core/src/io/dxf_input.rs` |
| 1 | `crates/rs_cam_core/src/geometry/monotone_cells.rs` |
| 1 | `crates/rs_cam_core/src/geometry/grid2.rs` |
| 1 | `crates/rs_cam_core/src/dexel_stock/band.rs` |
| 1 | `crates/rs_cam_core/src/dexel_stock/whole_path.rs` |
| 1 | `crates/rs_cam_core/src/adaptive3d/search.rs` |
| 1 | `crates/rs_cam_core/src/tool_load/chipload.rs` |
| 1 | `crates/rs_cam_core/src/tool_load/deflection.rs` |
| 1 | `crates/rs_cam_core/src/tool_load/power.rs` |
| 1 | `crates/rs_cam_core/src/tool_load/depth.rs` |
| 1 | `crates/rs_cam_core/src/finish/spiral_finish.rs` |
| 1 | `crates/rs_cam_core/src/finish/pencil/chain_paths.rs` |
| 1 | `crates/rs_cam_core/src/surface/dropcutter.rs` |
| 1 | `crates/rs_cam_core/src/trace/semantic_trace.rs` |
| 1 | `crates/rs_cam_core/src/trace/toolpath_spans.rs` |
| 1 | `crates/rs_cam_core/src/stock/sim_triage.rs` |
| 1 | `crates/rs_cam_core/src/compute/simulate.rs` |
| 1 | `crates/rs_cam_core/src/compute/tool_config.rs` |
| 1 | `crates/rs_cam_core/src/compute/toolpath_stats.rs` |
| 1 | `crates/rs_cam_core/src/compute/catalog/schema.rs` |
| 1 | `crates/rs_cam_core/src/compute/execute/shared.rs` |
| 1 | `crates/rs_cam_core/src/compute/execute/tests.rs` |
| 1 | `crates/rs_cam_core/src/gcode/modal.rs` |
| 1 | `crates/rs_cam_mcp/src/server.rs` |
| 1 | `crates/rs_cam_viz/src/host.rs` |
| 1 | `crates/rs_cam_viz/src/error.rs` |
| 1 | `crates/rs_cam_viz/src/mcp_server.rs` |
| 1 | `crates/rs_cam_viz/src/app.rs` |
| 1 | `crates/rs_cam_viz/src/lib.rs` |
| 1 | `crates/rs_cam_viz/src/render/toolpath_render.rs` |
| 1 | `crates/rs_cam_viz/src/state/viewport.rs` |
| 1 | `crates/rs_cam_viz/src/state/job.rs` |
| 1 | `crates/rs_cam_viz/src/ui/theme.rs` |
| 1 | `crates/rs_cam_viz/src/ui/menu_bar.rs` |
| 1 | `crates/rs_cam_viz/src/ui/overlays/registry.rs` |
| 1 | `crates/rs_cam_viz/src/ui/properties/mod.rs` |
| 1 | `crates/rs_cam_viz/src/ui/properties/operations/mod.rs` |
| 1 | `crates/rs_cam_viz/src/ui/properties/operations/boundary_2d.rs` |
| 1 | `crates/rs_cam_viz/src/app/mcp/diagnostics.rs` |

## Lines

- `crates/rs_cam_cli/src/sweep.rs:499` /// 2026-09-16, no legacy support.
- `crates/rs_cam_cli/src/project.rs:1445` {} issue runs (COALESCED, the legacy \"issue_count\") | {} hotspots",
- `crates/rs_cam_cli/src/project.rs:1529` /// This record's key names are a compatibility surface — the doc comment
- `crates/rs_cam_cli/src/job.rs:1568` /// Operator ruling 2026-09-16 — no legacy support. A job file on the
- `crates/rs_cam_core/src/toolpath.rs:49` /// migrated. All in-tree generators emit non-`Unknown` tags; the
- `crates/rs_cam_core/src/toolpath.rs:86` /// Fallback for legacy / unaware generators. Deprecation marker.
- `crates/rs_cam_core/src/toolpath.rs:1060` // Non-intent-aware callers (legacy generators) still benefit from
- `crates/rs_cam_core/src/toolpath.rs:1063` // the caller is migrated, because we don't know whether it's
- `crates/rs_cam_core/src/toolpath.rs:1066` // reclassification work even for legacy generators.
- `crates/rs_cam_core/src/toolpath.rs:1076` // Body moves keep the caller-supplied intent (Unknown for legacy).
- `crates/rs_cam_core/src/ids.rs:21` /// payloads are byte-compatible with the pre-newtype format.
- `crates/rs_cam_core/src/diagnostics/mod.rs:19` //! Each emitted shape-incompatible findings, sometimes contradicting
- `crates/rs_cam_core/src/diagnostics/ids.rs:348` // ── Tool / operation compatibility ───────────────────────────────────
- `crates/rs_cam_core/src/diagnostics/ids.rs:351` pub const COMPAT_TOOL_KIND_REFUSED: &str = "compat.tool_kind_refused";
- `crates/rs_cam_core/src/diagnostics/ids.rs:352` pub const COMPAT_BALL_NOSE_FLAT_CLEARING: &str = "compat.ball_nose_on_flat_clearing";
- `crates/rs_cam_core/src/diagnostics/ids.rs:356` pub const COMPAT_PROJECT_CURVE_POINT_PROJECTION: &str = "compat.project_curve_point_projection";
- `crates/rs_cam_core/src/diagnostics/ids.rs:492` COMPAT_TOOL_KIND_REFUSED,
- `crates/rs_cam_core/src/diagnostics/ids.rs:493` COMPAT_BALL_NOSE_FLAT_CLEARING,
- `crates/rs_cam_core/src/diagnostics/ids.rs:494` COMPAT_PROJECT_CURVE_POINT_PROJECTION,
- `crates/rs_cam_core/src/diagnostics/tests.rs:163` ("COMPAT_TOOL_KIND_REFUSED", ids::COMPAT_TOOL_KIND_REFUSED),
- `crates/rs_cam_core/src/diagnostics/tests.rs:165` "COMPAT_BALL_NOSE_FLAT_CLEARING",
- `crates/rs_cam_core/src/diagnostics/tests.rs:166` ids::COMPAT_BALL_NOSE_FLAT_CLEARING,
- `crates/rs_cam_core/src/diagnostics/tests.rs:169` "COMPAT_PROJECT_CURVE_POINT_PROJECTION",
- `crates/rs_cam_core/src/diagnostics/tests.rs:170` ids::COMPAT_PROJECT_CURVE_POINT_PROJECTION,
- `crates/rs_cam_core/src/diagnostics/tests.rs:717` .filter(|d| d.id.as_str() == ids::COMPAT_PROJECT_CURVE_POINT_PROJECTION)
- `crates/rs_cam_core/src/diagnostics/adapters/from_project_diagnostics.rs:5` //! high, generated-empty) into the unified schema. The legacy
- `crates/rs_cam_core/src/diagnostics/adapters/from_project_diagnostics.rs:56` severity: severity_from_legacy(v.severity),
- `crates/rs_cam_core/src/diagnostics/adapters/from_project_diagnostics.rs:68` fn severity_from_legacy(s: VerdictSeverity) -> Severity {
- `crates/rs_cam_core/src/diagnostics/adapters/from_generation.rs:413` /// deprecated-dial rule guards against: claims run only on a `UnifiedFinish`
- `crates/rs_cam_core/src/diagnostics/adapters/from_stale_default.rs:54` // matching on `legacy_rule_id` can branch identically against
- `crates/rs_cam_core/src/diagnostics/adapters/from_static_checks.rs:6` //! stepover > diameter) live here, as do tool/op compatibility rules
- `crates/rs_cam_core/src/diagnostics/adapters/from_static_checks.rs:43` out.extend(tool_op_compat_checks(&scope, op, tool));
- `crates/rs_cam_core/src/diagnostics/adapters/from_static_checks.rs:163` // ── tool/operation compatibility ────────────────────────────────────
- `crates/rs_cam_core/src/diagnostics/adapters/from_static_checks.rs:165` fn tool_op_compat_checks(
- `crates/rs_cam_core/src/diagnostics/adapters/from_static_checks.rs:178` id: DiagnosticId::from(ids::COMPAT_BALL_NOSE_FLAT_CLEARING),
- `crates/rs_cam_core/src/diagnostics/adapters/from_static_checks.rs:203` id: DiagnosticId::from(ids::COMPAT_TOOL_KIND_REFUSED),
- `crates/rs_cam_core/src/diagnostics/adapters/from_static_checks.rs:226` id: DiagnosticId::from(ids::COMPAT_PROJECT_CURVE_POINT_PROJECTION),
- `crates/rs_cam_core/src/session/rest_stock.rs:646` fn legacy_session_rule(min_radius: f64, bbox: &BoundingBox3) -> f64 {
- `crates/rs_cam_core/src/session/rest_stock.rs:659` fn legacy_groups_rule(min_radius: f64, bbox: &BoundingBox3) -> f64 {
- `crates/rs_cam_core/src/session/rest_stock.rs:669` fn the_one_auto_rule_gives_both_legacy_cells_bit_for_bit() {
- `crates/rs_cam_core/src/session/rest_stock.rs:687` legacy_session_rule(r, &bbox).to_bits(),
- `crates/rs_cam_core/src/session/rest_stock.rs:692` legacy_groups_rule(r, &bbox).to_bits(),
- `crates/rs_cam_core/src/session/multitool.rs:1663` // Absent entries default to unified — byte-identical legacy plans.
- `crates/rs_cam_core/src/session/mod.rs:2160` /// L1. The load-time dressup migration rewrites a value the
- `crates/rs_cam_core/src/session/mod.rs:2169` "name = \"Dressup Migration\"\n",
- `crates/rs_cam_core/src/session/mod.rs:2208` // The migration still writes what it always wrote.
- `crates/rs_cam_core/src/session/save.rs:62` /// Ported verbatim in behaviour from `rs_cam_viz`'s fallback loader
- `crates/rs_cam_core/src/session/diagnostics_types.rs:309` /// Structured project-level verdict. Replaces the legacy single-line
- `crates/rs_cam_core/src/session/project_file.rs:81` /// 2026-09-16: no legacy, breaking is acceptable. Ruling of 2026-10-15:
- `crates/rs_cam_core/src/session/project_file.rs:999` // One-shot migration: projects saved before operation-specific dressup
- `crates/rs_cam_core/src/session/project_file.rs:1015` "Normalized incompatible dressups on load"
- `crates/rs_cam_core/src/session/project_file.rs:1405` /// No legacy shim: the operator ruling of 2026-09-16 takes the break. A
- `crates/rs_cam_core/src/session/mutation/config.rs:31` // Enforce the per-operation dressup invariant so incompatible
- `crates/rs_cam_core/src/session/compute/simulation.rs:463` /// `samples` still hold the legacy `Arc` and we want them to remain
- `crates/rs_cam_core/src/maps/finish_surface_cache.rs:93` //! `cell_source`). `explicit(0.75)` and a `legacy_envelope_quarter` that
- `crates/rs_cam_core/src/maps/finish_surface_cache.rs:523` let derived = FinishResolutionPolicy::legacy_envelope_quarter(&ball, 0.05);
- `crates/rs_cam_core/src/maps/finish_surface_cache.rs:538` let floored = FinishResolutionPolicy::legacy_envelope_quarter(&fine, 0.5);
- `crates/rs_cam_core/src/io/dxf_input.rs:9` //! - **Polyline** — closed legacy polylines (with optional bulge arcs)
- `crates/rs_cam_core/src/geometry/monotone_cells.rs:109` /// compatibility). At 0° that fast path is honest — the rotation there IS
- `crates/rs_cam_core/src/geometry/grid2.rs:16` //! adopters — deliberately NOT migrated here, since each has its own cosmetic
- `crates/rs_cam_core/src/dexel_stock/band.rs:269` /// instead of letting rows migrate between cores on every subsegment — is a
- `crates/rs_cam_core/src/dexel_stock/whole_path.rs:12` //! — the same rows migrate between cores on every subsegment.
- `crates/rs_cam_core/src/dexel_stock/stamping.rs:125` /// Both legacy predicates are **monotone in `d_sq`** (`sqrt` is monotone and
- `crates/rs_cam_core/src/dexel_stock/stamping.rs:129` /// squared bound and walk ULPs until the legacy predicate's own answer flips.
- `crates/rs_cam_core/src/dexel_stock/stamping.rs:136` /// Smallest `d_sq` for which the legacy test `√d_sq − ext_diag ≥ r` holds.
- `crates/rs_cam_core/src/dexel_stock/stamping.rs:139` /// Largest `d_sq` for which the legacy test `√d_sq + ext_diag ≤ r` holds,
- `crates/rs_cam_core/src/dexel_stock/stamping.rs:144` /// regime — no cell can satisfy the legacy test at all, because distances
- `crates/rs_cam_core/src/dexel_stock/stamping.rs:2819` /// the same technique `legacy_fast_path` uses for S7 — and this asserts
- `crates/rs_cam_core/src/dexel_stock/stamping.rs:2963` fn legacy_fast_path(center_d_sq: f64, r_sq: f64, cs: f64) -> Option<f32> {
- `crates/rs_cam_core/src/dexel_stock/stamping.rs:3047` let legacy = legacy_fast_path(d_sq, r_sq, cs);
- `crates/rs_cam_core/src/dexel_stock/stamping.rs:3049` if legacy != s7 {
- `crates/rs_cam_core/src/dexel_stock/stamping.rs:3051` "r={r} cs={cs} d={d:.17e}: legacy {legacy:?}, S7 {s7:?}"
- `crates/rs_cam_core/src/adaptive3d/mod.rs:47` /// Clear all areas at each Z level globally (default, backward compat).
- `crates/rs_cam_core/src/adaptive3d/mod.rs:228` /// Region ordering strategy (default: Global for backward compat).
- `crates/rs_cam_core/src/adaptive3d/mod.rs:280` /// Entry strategy (default: Plunge for backward compat).
- `crates/rs_cam_core/src/adaptive3d/clearing.rs:2183` crate::adaptive::CleanupStrategy::Legacy
- `crates/rs_cam_core/src/adaptive3d/clearing.rs:2258` // For non-Legacy cleanup strategies, run the same cleanup
- `crates/rs_cam_core/src/adaptive3d/clearing.rs:2267` crate::adaptive::CleanupStrategy::Legacy => segs_2d_raw,
- `crates/rs_cam_core/src/adaptive3d/path.rs:1713` // F-038b: legacy `segments_to_toolpath` tests need a mesh + index +
- `crates/rs_cam_core/src/adaptive3d/path.rs:1727` fn legacy_test_mesh() -> (crate::mesh::TriangleMesh, crate::mesh::SpatialIndex) {
- `crates/rs_cam_core/src/adaptive3d/path.rs:1750` fn legacy_test_cutter() -> crate::tool::FlatEndmill {
- `crates/rs_cam_core/src/adaptive3d/path.rs:1800` min_region_cut_length_mm: 0.0, // F-038b: legacy `segments_to_toolpath` unit tests expect the
- `crates/rs_cam_core/src/adaptive3d/path.rs:1850` let cutter = legacy_test_cutter();
- `crates/rs_cam_core/src/adaptive3d/path.rs:1934` let cutter = legacy_test_cutter();
- `crates/rs_cam_core/src/adaptive3d/path.rs:2000` let cutter = legacy_test_cutter();
- `crates/rs_cam_core/src/adaptive3d/path.rs:2163` let (mesh, si) = legacy_test_mesh();
- `crates/rs_cam_core/src/adaptive3d/path.rs:2164` let cutter = legacy_test_cutter();
- `crates/rs_cam_core/src/adaptive3d/search.rs:34` /// `fraction = cells_with_material / cells_at_z` (matches the legacy
- `crates/rs_cam_core/src/adaptive3d/tests.rs:141` // but the unit-test fixtures here exercise the legacy code path
- `crates/rs_cam_core/src/adaptive3d/tests.rs:562` fn step_levels_are_the_legacy_loop_bitwise() {
- `crates/rs_cam_core/src/adaptive3d/tests.rs:566` let mut legacy = Vec::new();
- `crates/rs_cam_core/src/adaptive3d/tests.rs:569` legacy.push(z);
- `crates/rs_cam_core/src/adaptive3d/tests.rs:573` legacy.push(z_bottom);
- `crates/rs_cam_core/src/adaptive3d/tests.rs:577` assert_eq!(plan.len(), legacy.len());
- `crates/rs_cam_core/src/adaptive3d/tests.rs:578` for (l, z) in plan.iter().zip(&legacy) {
- `crates/rs_cam_core/src/adaptive3d/tests.rs:579` assert_eq!(l.to_bits(), z.to_bits(), "level {l} vs legacy {z}");
- `crates/rs_cam_core/src/export/ribbon.rs:93` /// The cuts-only view is intentionally semantic: legacy untagged cutting
- `crates/rs_cam_core/src/export/ribbon.rs:133` /// intents (and legacy `Unknown` cutting moves) are rendered. Falls through to
- `crates/rs_cam_core/src/machine/mod.rs:189` /// serde compatibility with existing project files and
- `crates/rs_cam_core/src/machine/mod.rs:220` /// there is no alias (ruling 2026-09-16, "no legacy, breaking OK").
- `crates/rs_cam_core/src/machine/mod.rs:675` /// production path reads it. The viz legacy project loader that also
- `crates/rs_cam_core/src/machine/kinematics.rs:146` /// `MachineKinematics::junction_deviation_mm` on legacy project files.
- `crates/rs_cam_core/src/machine/kinematics.rs:536` /// with `COMPATIBILITY_LEVEL` above 0 prints the Grbl welcome line instead
- `crates/rs_cam_core/src/machine/kinematics.rs:939` /// collects feed moves from legacy generators that never tagged
- `crates/rs_cam_core/src/machine/kinematics.rs:2098` fn legacy_kinematics_json_defaults_new_fields() {
- `crates/rs_cam_core/src/machine/kinematics.rs:2102` let legacy =
- `crates/rs_cam_core/src/machine/kinematics.rs:2104` let kin: MachineKinematics = serde_json::from_str(legacy).expect("legacy deserialize");
- `crates/rs_cam_core/src/machine/kinematics.rs:2207` /// too when built with `COMPATIBILITY_LEVEL` above 0 (report.c:314).
- `crates/rs_cam_core/src/tool_load/locality.rs:17` //! authoritative signal. When no span data is plumbed (legacy traces,
- `crates/rs_cam_core/src/tool_load/locality.rs:154` /// as steady-state preserves the pre-D7 trip behaviour for legacy
- `crates/rs_cam_core/src/tool_load/chipload.rs:1033` /// Adapts this module's legacy positional-arg test calls to the
- `crates/rs_cam_core/src/tool_load/verdict.rs:114` ///   single legacy "chipload" clamp band; F-036c-style retro reads
- `crates/rs_cam_core/src/tool_load/verdict.rs:2081` /// `(&'static str, ExceedsReason)` pair returned by the legacy
- `crates/rs_cam_core/src/tool_load/verdict.rs:2908` /// migration (G16 Step 7). The MCP `get_tool_load_report` tool serves
- `crates/rs_cam_core/src/tool_load/verdict.rs:3281` /// Callers without a name resolver (legacy test fixtures, headless
- `crates/rs_cam_core/src/tool_load/deflection.rs:394` /// Adapts this module's legacy positional-arg test calls to the
- `crates/rs_cam_core/src/tool_load/power.rs:622` /// Adapts this module's legacy positional-arg test calls to the
- `crates/rs_cam_core/src/tool_load/depth.rs:25` //! [`SimulationCutSample`] carries both. `axial_doc_mm` is the LEGACY
- `crates/rs_cam_core/src/feeds/vendor_lut.rs:242` /// `migrate_ap_rule` binary from the row's `ap_rule` prose.
- `crates/rs_cam_core/src/feeds/vendor_lut.rs:338` /// compat for partial schema changes) and the test fails loud with
- `crates/rs_cam_core/src/feeds/vendor_lut.rs:520` // to the same bar (with a documented legacy allowlist that may only
- `crates/rs_cam_core/src/feeds/vendor_lut.rs:779` /// Phase 1 LUT migration invariant
- `crates/rs_cam_core/src/feeds/vendor_lut.rs:786` /// quantify. The migration example
- `crates/rs_cam_core/src/feeds/vendor_lut.rs:787` /// `crates/rs_cam_cli/examples/migrate_ap_rule.rs` keeps
- `crates/rs_cam_core/src/feeds/vendor_lut.rs:823` `crates/rs_cam_cli/examples/migrate_ap_rule.rs` and re-run \
- `crates/rs_cam_core/src/feeds/vendor_lut.rs:824` the migration, or extend `LABEL_ONLY_AP_RULES` if the rule \
- `crates/rs_cam_core/src/feeds/vendor_lut.rs:831` /// F3.2 — legacy rows that predate the loader-validation rules and
- `crates/rs_cam_core/src/feeds/vendor_lut.rs:838` const LEGACY_DEGENERATE_RANGE_ROWS: &[&str] = &[
- `crates/rs_cam_core/src/feeds/vendor_lut.rs:888` /// modulo the shrink-only legacy allowlist above. Strict equality
- `crates/rs_cam_core/src/feeds/vendor_lut.rs:897` if !LEGACY_DEGENERATE_RANGE_ROWS.contains(&obs.observation_id.as_str()) {
- `crates/rs_cam_core/src/feeds/vendor_lut.rs:903` for legacy in LEGACY_DEGENERATE_RANGE_ROWS {
- `crates/rs_cam_core/src/feeds/vendor_lut.rs:905` violating.contains(legacy),
- `crates/rs_cam_core/src/feeds/vendor_lut.rs:906` "{legacy} no longer violates any rule — remove it from \
- `crates/rs_cam_core/src/feeds/vendor_lut.rs:907` LEGACY_DEGENERATE_RANGE_ROWS (the list may only shrink)"
- `crates/rs_cam_core/src/feeds/mod.rs:217` /// and compatibility decisions.
- `crates/rs_cam_core/src/feeds/mod.rs:223` ///   compatibility) where the geometry numbers don't matter.
- `crates/rs_cam_core/src/feeds/mod.rs:274` /// `facing_bit_is_a_lut_only_family`). Any compatibility logic
- `crates/rs_cam_core/src/feeds/mod.rs:1102` /// Validate that the input's tool geometry is physically compatible
- `crates/rs_cam_core/src/feeds/profile.rs:14` //! - **Owns:** tool/op compatibility ([`FeedsError`] feasibility — NOT
- `crates/rs_cam_core/src/feeds/profile.rs:44` // deflection predictor's signature into a `Result`, and migrating a
- `crates/rs_cam_core/src/feeds/profile.rs:107` /// Tool × operation compatibility — `Err` carries the same
- `crates/rs_cam_core/src/feeds/suggest.rs:67` /// support this". `FeedsWithGates` preserves the pre-v3.3 / v3.0d
- `crates/rs_cam_core/src/feeds/suggest.rs:75` /// risky. Pre-v3.3 / v3.0d behaviour.
- `crates/rs_cam_core/src/feeds/suggest.rs:861` /// op). Callers that want the legacy "always produce something"
- `crates/rs_cam_core/src/feeds/tests.rs:1334` fn test_no_lut_backward_compatible() {
- `crates/rs_cam_core/src/feeds/tests.rs:1754` /// compatibility story (LUT routing, constraints) to be revisited
- `crates/rs_cam_core/src/feeds/vendor_lookup.rs:273` /// that don't record an angle (legacy data) are exempt from the gate and
- `crates/rs_cam_core/src/feeds/vendor_lookup.rs:288` /// worse than `find_best_row` on angle-less legacy data. This is the slot
- `crates/rs_cam_core/src/feeds/vendor_lookup.rs:840` fn materials_compatible(query: MaterialFamily, obs: MaterialFamily) -> bool {
- `crates/rs_cam_core/src/feeds/vendor_lookup.rs:868` if !materials_compatible(query.material_family, obs.material_family) {
- `crates/rs_cam_core/src/feeds/vendor_lookup.rs:871` if !tool_family_compatible(query.tool_family, obs.tool_family) {
- `crates/rs_cam_core/src/feeds/vendor_lookup.rs:972` /// Check if a query tool family is compatible with an observation tool family.
- `crates/rs_cam_core/src/feeds/vendor_lookup.rs:973` fn tool_family_compatible(query: ToolFamily, obs: ToolFamily) -> bool {
- `crates/rs_cam_core/src/feeds/vendor_lookup.rs:977` // Fallback compatibility
- `crates/rs_cam_core/src/feeds/vendor_lookup.rs:995` _ => 0, // shouldn't reach here due to compatible filter
- `crates/rs_cam_core/src/feeds/vendor_lookup.rs:1036` fn find_best_row_matches_legacy_lookup_best() {
- `crates/rs_cam_core/src/feeds/vendor_lookup.rs:1050` let via_legacy = lookup_best(&lut, &query).expect("legacy row");
- `crates/rs_cam_core/src/feeds/vendor_lookup.rs:1051` assert_eq!(via_canonical.observation_id, via_legacy.observation_id);
- `crates/rs_cam_core/src/feeds/vendor_lookup.rs:1615` // With no query angle the gate is inert — every angle-compatible
- `crates/rs_cam_core/src/feeds/vendor_lookup.rs:1616` // row is eligible and the best-scoring one wins (legacy behaviour,
- `crates/rs_cam_core/src/feeds/vendor_lookup.rs:1685` let won_the_legacy_row = result.observation_id == "amana-vgroove-softwood-trace-30deg-1f";
- `crates/rs_cam_core/src/feeds/vendor_lookup.rs:1687` won_an_angle_only_row || won_the_legacy_row,
- `crates/rs_cam_core/src/feeds/vendor_lookup.rs:1688` "expected angle-only or legacy 30° row, got {} (row_diameter={})",
- `crates/rs_cam_core/src/feeds/rationale.rs:46` /// a legacy-estimate disclaimer on a measured number is a new defect, not a
- `crates/rs_cam_core/src/feeds/rationale.rs:53` pub const LEGACY_ESTIMATE_LABEL: &str = "Legacy pre-simulation estimate";
- `crates/rs_cam_core/src/feeds/rationale.rs:55` /// The sentence that says *why* [`LEGACY_ESTIMATE_LABEL`] applies, so a
- `crates/rs_cam_core/src/feeds/rationale.rs:58` const LEGACY_ESTIMATE_NOTE: &str = "Estimated against the arc-mean chip observation the post-sim gate retired on 2026-08-06, \
- `crates/rs_cam_core/src/feeds/rationale.rs:122` /// The chipload figures on this entry are a **legacy pre-simulation
- `crates/rs_cam_core/src/feeds/rationale.rs:123` /// estimate** — see [`LEGACY_ESTIMATE_LABEL`].
- `crates/rs_cam_core/src/feeds/rationale.rs:129` /// estimate, not a gate observation ([`LEGACY_ESTIMATE_LABEL`]).
- `crates/rs_cam_core/src/feeds/rationale.rs:549` "{LEGACY_ESTIMATE_LABEL}: chipload {predicted_observed_chipload_before:.4} → {predicted_observed_chipload_after:.4} mm/tooth (LUT target {lu
- `crates/rs_cam_core/src/feeds/rationale.rs:570` "{LEGACY_ESTIMATE_LABEL}: {predicted_observed_mm_per_tooth:.4} mm/tooth vs LUT target {lut_target_mm_per_tooth:.4} at feed {feed_at_terminat
- `crates/rs_cam_core/src/feeds/rationale.rs:1018` fn chipload_recalibration_entries_are_labelled_a_legacy_estimate() {
- `crates/rs_cam_core/src/feeds/rationale.rs:1040` detail.contains(LEGACY_ESTIMATE_LABEL),
- `crates/rs_cam_core/src/feeds/suggest/adaptive_entry.rs:97` /// Pass 8: v1.3 combined-Suggest plunge-entry / DPP compatibility warning.
- `crates/rs_cam_core/src/feeds/suggest/adaptive_entry.rs:570` /// compatibility warning. Returns `None` for non-Adaptive ops, for
- `crates/rs_cam_core/src/feeds/suggest/tests.rs:131` /// S1: with no scallop target the legacy formula-based stepover is
- `crates/rs_cam_core/src/feeds/suggest/tests.rs:136` fn scallop_height_none_preserves_legacy_ae() {
- `crates/rs_cam_core/src/feeds/suggest/tests.rs:140` "legacy DropCutter stepover on a 1 mm ball should stay small \
- `crates/rs_cam_core/src/feeds/suggest/tests.rs:506` /// legacy entry points it now backs — the funnel was extended, not
- `crates/rs_cam_core/src/feeds/suggest/tests.rs:509` fn apply_scope_matches_the_legacy_entry_points_exactly() {
- `crates/rs_cam_core/src/feeds/suggest/tests.rs:527` let mut legacy_op = base.clone();
- `crates/rs_cam_core/src/feeds/suggest/tests.rs:528` let mut legacy_prov = crate::feeds::FeedsProvenance::default();
- `crates/rs_cam_core/src/feeds/suggest/tests.rs:529` let legacy = match scope {
- `crates/rs_cam_core/src/feeds/suggest/tests.rs:534` legacy(
- `crates/rs_cam_core/src/feeds/suggest/tests.rs:535` &mut legacy_op,
- `crates/rs_cam_core/src/feeds/suggest/tests.rs:536` &mut legacy_prov,
- `crates/rs_cam_core/src/feeds/suggest/tests.rs:551` legacy_op.feed_rate(),
- `crates/rs_cam_core/src/feeds/suggest/tests.rs:556` legacy_op.plunge_rate(),
- `crates/rs_cam_core/src/feeds/suggest/tests.rs:561` legacy_op.spindle_rpm(),
- `crates/rs_cam_core/src/feeds/suggest/tests.rs:566` legacy_op.as_params().stepover(),
- `crates/rs_cam_core/src/feeds/suggest/tests.rs:571` legacy_op.as_params().depth_per_pass(),
- `crates/rs_cam_core/src/feeds/suggest/tests.rs:1114` /// - DPP=9 mm, D=6 mm (pre-v1.1 motivating failure, ratio 1.5)
- `crates/rs_cam_core/src/feeds/suggest/tests.rs:1123` (9.0, "pre-v1.1 case: DPP=9 mm on 6 mm tool (ratio 1.5)"),
- `crates/rs_cam_core/src/finish/ramp_finish.rs:416` /// `LegacyEnvelopeQuarter` until PR-8a, under the approved Checkpoint B.
- `crates/rs_cam_core/src/finish/ramp_finish.rs:439` /// legacy cell does not (gated behind Checkpoint C), and steep/shallow's
- `crates/rs_cam_core/src/finish/spiral_finish.rs:606` /// the point-runs migration, non-contacted spiral samples were simply
- `crates/rs_cam_core/src/finish/unified_finish.rs:30` //! up). `claims: None` reproduces the pre-v3 op byte-for-byte: `decompose`
- `crates/rs_cam_core/src/finish/unified_finish.rs:388` /// **Serde compatibility is load-bearing.** The two pre-A/M6 names are
- `crates/rs_cam_core/src/finish/unified_finish.rs:620` /// reproduces the pre-v3 op exactly: no detector run, no crease claims, no
- `crates/rs_cam_core/src/finish/unified_finish.rs:726` /// surface, same as the pre-v3 op. Creases still claim corridors.
- `crates/rs_cam_core/src/finish/unified_finish.rs:836` /// the GUI span list, so this string is a compatibility surface.
- `crates/rs_cam_core/src/finish/unified_finish.rs:987` /// on any plain ball — the migration moves tapered tools only.
- `crates/rs_cam_core/src/finish/unified_finish.rs:1877` // byte-identical to the pre-v3 op (module doc). Wave D1: the tip float
- `crates/rs_cam_core/src/finish/unified_finish.rs:2124` // material and the legacy surface-riding link is correct;
- `crates/rs_cam_core/src/finish/steep_shallow.rs:484` /// SteepShallow selects `FinishResolutionMode::LegacyEnvelopeQuarter` — the
- `crates/rs_cam_core/src/finish/steep_shallow.rs:488` /// # Why this op STAYS on the legacy cell (PR-8c, written deferral)
- `crates/rs_cam_core/src/finish/steep_shallow.rs:529` FinishResolutionPolicy::legacy_envelope_quarter(cutter, tolerance)
- `crates/rs_cam_core/src/finish/steep_shallow.rs:621` /// the slope classifier on its own offset surface (`legacy_envelope_quarter`,
- `crates/rs_cam_core/src/finish/finish_setup.rs:47` /// [`Self::LegacyEnvelopeQuarter`]: one named variant among several, not a
- `crates/rs_cam_core/src/finish/finish_setup.rs:51` /// RampFinish, SteepShallow) select `LegacyEnvelopeQuarter` and the
- `crates/rs_cam_core/src/finish/finish_setup.rs:61` LegacyEnvelopeQuarter,
- `crates/rs_cam_core/src/finish/finish_setup.rs:83` /// legacy cell drove a 2.39 mm gouge on the narrow-valley fixture and a
- `crates/rs_cam_core/src/finish/finish_setup.rs:90` /// numbers is that number, so this mode resolves to exactly the legacy
- `crates/rs_cam_core/src/finish/finish_setup.rs:92` /// `finish_resolution_policy_pr3::ball_resolves_the_geo_mean_to_the_legacy_cell`.
- `crates/rs_cam_core/src/finish/finish_setup.rs:104` Self::LegacyEnvelopeQuarter => "legacy envelope/4",
- `crates/rs_cam_core/src/finish/finish_setup.rs:121` Self::LegacyEnvelopeQuarter => CellSource::EnvelopeRadius,
- `crates/rs_cam_core/src/finish/finish_setup.rs:146` /// `(envelope_radius / 4).max(tolerance)` — [`FinishResolutionMode::LegacyEnvelopeQuarter`].
- `crates/rs_cam_core/src/finish/finish_setup.rs:148` pub fn legacy_envelope_quarter(cutter: &dyn MillingCutter, tolerance: f64) -> Self {
- `crates/rs_cam_core/src/finish/finish_setup.rs:154` FinishResolutionMode::LegacyEnvelopeQuarter,
- `crates/rs_cam_core/src/finish/finish_setup.rs:419` /// those keep calling this variant so migrating them onto the shared helper
- `crates/rs_cam_core/src/finish/finish_setup.rs:438` /// Build a [`FinishSurface`] under [`FinishResolutionMode::LegacyEnvelopeQuarter`]:
- `crates/rs_cam_core/src/finish/finish_setup.rs:441` /// Adapter kept for callers (and sentries) that want the legacy formula
- `crates/rs_cam_core/src/finish/finish_setup.rs:456` // is now ANSWERED-BY-NAME rather than hidden: `LegacyEnvelopeQuarter`
- `crates/rs_cam_core/src/finish/finish_setup.rs:464` FinishResolutionPolicy::legacy_envelope_quarter(cutter, tolerance),
- `crates/rs_cam_core/src/finish/finish_setup.rs:512` /// [`FinishResolutionMode::LegacyEnvelopeQuarter`], so on a tapered ball
- `crates/rs_cam_core/src/finish/finish_setup.rs:628` /// Migrated verbatim from the identical `scallop.rs` / `ramp_finish.rs`
- `crates/rs_cam_core/src/finish/finish_setup.rs:631` /// The extraction commit also migrated `execute.rs`, despite this comment
- `crates/rs_cam_core/src/finish/pencil.rs:172` /// of the same diameter. `None` = legacy nominal-diameter behaviour.
- `crates/rs_cam_core/src/finish/pencil.rs:183` /// this is `Some`. `None` keeps the legacy behaviour: emit a surface
- `crates/rs_cam_core/src/finish/scallop.rs:993` /// Scallop selects `FinishResolutionMode::LegacyEnvelopeQuarter` — the same
- `crates/rs_cam_core/src/finish/scallop.rs:1009` FinishResolutionPolicy::legacy_envelope_quarter(cutter, tolerance)
- `crates/rs_cam_core/src/finish/surface_link.rs:113` /// legacy behaviour.
- `crates/rs_cam_core/src/finish/surface_link.rs:303` /// hop, the mesh IS the material — the legacy surface-riding link is correct
- `crates/rs_cam_core/src/finish/surface_link.rs:413` /// Fixed ends. The default, and the byte-identical legacy treatment.
- `crates/rs_cam_core/src/finish/surface_link.rs:462` /// byte-identical to the legacy surface-riding link.
- `crates/rs_cam_core/src/finish/surface_link.rs:688` /// [`LinkCeiling`]. `None` keeps the legacy surface-riding link — the
- `crates/rs_cam_core/src/finish/surface_link.rs:774` /// [`FragmentKind`], which is the byte-identical legacy arm.
- `crates/rs_cam_core/src/finish/unified_finish/tests.rs:354` /// table did, so the migration is provably value-preserving and the
- `crates/rs_cam_core/src/finish/unified_finish/tests.rs:1128` fn claims_off_matches_legacy_band_only_output() {
- `crates/rs_cam_core/src/finish/pencil/chain_paths.rs:408` /// The legacy symmetric fan with no per-point truncation or spacing —
- `crates/rs_cam_core/src/finish/pencil/emission.rs:72` /// treated as too short to enter along and the legacy descent is kept.
- `crates/rs_cam_core/src/finish/pencil/emission.rs:141` /// Returns `None` — keeping the legacy single descent — when there is no
- `crates/rs_cam_core/src/finish/pencil/emission.rs:395` /// Legacy stay-down surface link: feed along the mesh straight into the
- `crates/rs_cam_core/src/finish/pencil/emission.rs:420` /// the link's own path: keep the legacy stay-down link, byte for byte.
- `crates/rs_cam_core/src/finish/pencil/emission.rs:587` /// matters most. `params.link_kinematics = None` keeps the legacy behaviour
- `crates/rs_cam_core/src/finish/pencil/emission.rs:597` /// legacy single fed descent and links keep riding the mesh surface. Every
- `crates/rs_cam_core/src/finish/pencil/emission.rs:830` // straight line into `first` that a legacy link ends
- `crates/rs_cam_core/src/dressup/mod.rs:108` /// When absent, the entry keeps the legacy straight legs — honest only
- `crates/rs_cam_core/src/dressup/mod.rs:131` /// fresh-stock entry keeps the legacy two-leg ramp exactly.
- `crates/rs_cam_core/src/dressup/feed_modulation.rs:15` //!   behaviour can opt back into the legacy algorithm without
- `crates/rs_cam_core/src/dressup/feed_modulation.rs:356` /// Captures both the legacy "how many moves changed" scalar and the
- `crates/rs_cam_core/src/dressup/feed_modulation.rs:721` // pre-T-11 behaviour for legacy and analytical traces rather than
- `crates/rs_cam_core/src/dressup/feed_modulation.rs:730` /// F-036 — "target band-mid" per-move feed (legacy heuristic).
- `crates/rs_cam_core/src/dressup/tests.rs:1007` // legacy unconditional link.
- `crates/rs_cam_core/src/dressup/tests.rs:1034` /// Entry safety with no surface probe — the legacy blind-leg
- `crates/rs_cam_core/src/dressup/entry_descent.rs:198` /// `None` at the call site keeps the legacy straight legs. The adaptive3d
- `crates/rs_cam_core/src/dressup/entry_descent.rs:524` // LADDER OR PLUNGE, and never the legacy legs below. Both of the
- `crates/rs_cam_core/src/dressup/entry_descent.rs:562` // The rapid keeps the air part at rapid speed, as the legacy
- `crates/rs_cam_core/src/dressup/entry_descent.rs:666` // Legacy blind legs — honest only with no mesh surface to probe AND
- `crates/rs_cam_core/src/dressup/entry_descent.rs:686` /// polyline round-trips to exactly the legacy moves. The first point of
- `crates/rs_cam_core/src/dressup/arcfit.rs:67` /// - When `spans_valid` is `false`, the legacy unconditional collapse runs and
- `crates/rs_cam_core/src/dressup/arcfit.rs:1356` // legacy unconditional collapse, and spans pass through untouched.
- `crates/rs_cam_core/src/adaptive/mod.rs:41` /// - `Legacy`: pre-2026-05 behaviour. After the main spiral, the planner
- `crates/rs_cam_core/src/adaptive/mod.rs:82` Legacy,
- `crates/rs_cam_core/src/adaptive/mod.rs:356` CleanupStrategy::Legacy => segments,
- `crates/rs_cam_core/src/adaptive/mod.rs:504` cleanup_strategy: CleanupStrategy::Legacy,
- `crates/rs_cam_core/src/adaptive/path.rs:659` cleanup_strategy: crate::adaptive::CleanupStrategy::Legacy,
- `crates/rs_cam_core/src/adaptive/path.rs:825` // Legacy cleanup strategy — adaptive3d and other downstream
- `crates/rs_cam_core/src/adaptive/path.rs:828` // their current behaviour via the Legacy path. If no DT-max
- `crates/rs_cam_core/src/adaptive/path.rs:832` if matches!(params.cleanup_strategy, CleanupStrategy::Legacy) {
- `crates/rs_cam_core/src/adaptive/path.rs:892` // For non-Legacy strategies with a successful helical entry:
- `crates/rs_cam_core/src/adaptive/path.rs:904` // per-region calls), the spiral runs Legacy-style across
- `crates/rs_cam_core/src/adaptive/path.rs:915` && !matches!(params.cleanup_strategy, CleanupStrategy::Legacy)
- `crates/rs_cam_core/src/adaptive/path.rs:1132` && !matches!(params.cleanup_strategy, CleanupStrategy::Legacy)
- `crates/rs_cam_core/src/budget/grid.rs:25` //! The divisors reproduce the legacy values exactly (16 M / 2 = 8 M and
- `crates/rs_cam_core/src/budget/grid.rs:42` /// the legacy 8 M of `session/rest_stock.rs:63`.
- `crates/rs_cam_core/src/budget/grid.rs:46` /// legacy 4 M of `dressup/mod.rs` (`OwnStockReplay::MAX_CELLS`).
- `crates/rs_cam_core/src/budget/grid.rs:95` /// than `cap_cells` columns. That is the legacy behaviour of all three
- `crates/rs_cam_core/src/budget/grid.rs:119` /// A NaN request stays NaN, as in the legacy `DexelGrid::clamp_cell_size`;
- `crates/rs_cam_core/src/budget/grid.rs:148` // `f64::max`, not `floor_cell`: the legacy `effective_cell_size` maps a
- `crates/rs_cam_core/src/surface/dropcutter.rs:312` // compatibility — see the struct-level doc caveat: those two do not
- `crates/rs_cam_core/src/surface/reach.rs:20` //! Ball tools migrate too (approved ruling — the matrix is the plan's
- `crates/rs_cam_core/src/surface/reach.rs:705` /// envelope radius), so the migration moves tapered tools only.
- `crates/rs_cam_core/src/trace/semantic_trace.rs:134` /// compatibility surface — but the KEY half of it does not have to be free
- `crates/rs_cam_core/src/trace/toolpath_spans.rs:193` /// [`Self::DressupArtifact`]: that kind carried two incompatible
- `crates/rs_cam_core/src/trace/narrate.rs:222` /// cutting. That keeps the GUI's intent signal (a legacy generator's
- `crates/rs_cam_core/src/trace/narrate.rs:879` /// Legacy Z-level discovery: cluster cutting move Z-coordinates with
- `crates/rs_cam_core/src/stock/dexel_mesh.rs:159` /// Backwards-compatible helper for callers that specifically want the top
- `crates/rs_cam_core/src/stock/dexel_mesh.rs:674` // 50-vertex / 288-index layout of the legacy heightmap. Assert
- `crates/rs_cam_core/src/stock/dexel_mesh.rs:815` /// than the legacy fixed-index layout.
- `crates/rs_cam_core/src/stock/dexel_mesh_mc.rs:3` //! Replaces the cell-centre heightmap mesh produced by the legacy
- `crates/rs_cam_core/src/stock/dexel_mesh_mc.rs:1158` // cuts match the legacy behaviour.
- `crates/rs_cam_core/src/stock/collision.rs:366` /// Use 0.0 to check endpoints only (legacy behavior).
- `crates/rs_cam_core/src/stock/collision.rs:476` /// `step_mm <= 0.01` checks the move endpoint only (legacy behavior).
- `crates/rs_cam_core/src/stock/simulation_cut.rs:62` /// legacy `radial_engagement: f64` scalar was removed in the §10.3 follow-up
- `crates/rs_cam_core/src/stock/simulation_cut.rs:71` /// length to divide by — legacy traces, drill/analytical samples, and
- `crates/rs_cam_core/src/stock/simulation_cut.rs:138` /// backward-compatibility.
- `crates/rs_cam_core/src/stock/simulation_cut.rs:166` /// Legacy wire name for axial cutting engagement. Pure-vertical plunges
- `crates/rs_cam_core/src/stock/simulation_cut.rs:185` /// (dexel simulator) populate every applicable axis. Legacy traces
- `crates/rs_cam_core/src/stock/simulation_cut.rs:188` /// want to reject pre-v4 traces explicitly.
- `crates/rs_cam_core/src/stock/simulation_cut.rs:226` /// **`None` = not carried**, never "Unknown": legacy traces
- `crates/rs_cam_core/src/stock/sim_triage.rs:175` /// **LEGACY.** Contiguous air/low-engagement RUNS, coalesced per
- `crates/rs_cam_core/src/stock/dexel_mesh_mc/stride_tests.rs:6` //!    mesh bit for bit: the legacy body below is a verbatim copy of the
- `crates/rs_cam_core/src/stock/dexel_mesh_mc/stride_tests.rs:15` // SAFETY: test module; the legacy oracle and the fixtures index in bounds,
- `crates/rs_cam_core/src/stock/dexel_mesh_mc/stride_tests.rs:30` // ── The legacy oracle: from `b3a4a7ac`, renamed, plus the F1 winding swap. ─
- `crates/rs_cam_core/src/stock/dexel_mesh_mc/stride_tests.rs:33` fn legacy_z_grid_marching_cubes(
- `crates/rs_cam_core/src/stock/dexel_mesh_mc/stride_tests.rs:311` legacy_emit_cavity_surfaces(
- `crates/rs_cam_core/src/stock/dexel_mesh_mc/stride_tests.rs:336` fn legacy_emit_cavity_surfaces(
- `crates/rs_cam_core/src/stock/dexel_mesh_mc/stride_tests.rs:495` fn the_refactored_full_build_matches_the_legacy_body() {
- `crates/rs_cam_core/src/stock/dexel_mesh_mc/stride_tests.rs:498` let legacy = legacy_z_grid_marching_cubes(&stock.z_grid, top, bot);
- `crates/rs_cam_core/src/stock/dexel_mesh_mc/stride_tests.rs:500` assert!(!legacy.indices.is_empty(), "{name}: vacuous fixture");
- `crates/rs_cam_core/src/stock/dexel_mesh_mc/stride_tests.rs:501` assert_bit_identical(name, &legacy, &now);
- `crates/rs_cam_core/src/compute/simulate.rs:1728` // transform invalidated the spans (legacy invalidators;
- `crates/rs_cam_core/src/compute/tool_config.rs:1300` /// wire format, MCP error messages, and the viz legacy tool-type
- `crates/rs_cam_core/src/compute/operation_configs.rs:658` /// in narrow strips + contour-parallel residue sweep). Legacy
- `crates/rs_cam_core/src/compute/operation_configs.rs:659` /// behaviour can be restored by setting this to `Legacy`.
- `crates/rs_cam_core/src/compute/operation_configs.rs:1053` /// preserves the legacy formula-based stepover and the F-037 smoke
- `crates/rs_cam_core/src/compute/operation_configs.rs:1468` /// rest reference (all three detectors). `None` = legacy nominal-diameter
- `crates/rs_cam_core/src/compute/operation_configs.rs:1631` /// §M7–M8). Default OFF: an absent key loads the legacy cascade
- `crates/rs_cam_core/src/compute/operation_configs.rs:1788` /// carved). `false` reproduces the pre-v3 op exactly — no detector
- `crates/rs_cam_core/src/compute/operation_configs.rs:2156` /// absent key now loads `true`: a legacy project file gets the cell
- `crates/rs_cam_core/src/compute/operation_configs.rs:2158` /// per-operation opt-out, and the X5 back-compat test pins both directions.
- `crates/rs_cam_core/src/compute/operation_configs.rs:3085` /// The A/M6 deserialization-compatibility matrix, all three cells
- `crates/rs_cam_core/src/compute/operation_configs.rs:3096` /// the letter, and the wire names are the pre-A/M6 ones so no migration
- `crates/rs_cam_core/src/compute/operation_configs.rs:3101` fn unified_finish_claims_reference_serde_round_trip_and_backcompat() {
- `crates/rs_cam_core/src/compute/operation_configs.rs:3154` // Cell 3: legacy payload predating `claims_reference` and its S1/S2
- `crates/rs_cam_core/src/compute/operation_configs.rs:3157` let legacy = r#"{
- `crates/rs_cam_core/src/compute/operation_configs.rs:3170` let cfg: UnifiedFinishConfig = serde_json::from_str(legacy).unwrap();
- `crates/rs_cam_core/src/compute/operation_configs.rs:3173` // legacy project only sees the new default act if it also turned
- `crates/rs_cam_core/src/compute/operation_configs.rs:3176` // S4 (`territory_clip`) postdates this legacy payload too — must
- `crates/rs_cam_core/src/compute/operation_configs.rs:3177` // default off, same backcompat contract as its S1/S2 siblings.
- `crates/rs_cam_core/src/compute/operation_configs.rs:3182` // ruling"); the default is now ON. An absent key loads ON: a legacy
- `crates/rs_cam_core/src/compute/operation_configs.rs:3188` let pinned = legacy.replace(
- `crates/rs_cam_core/src/compute/config.rs:159` /// `post.safe_z` is a legacy 2D-friendly default (often 10mm above work Z=0),
- `crates/rs_cam_core/src/compute/config.rs:308` /// keep working for callers that haven't migrated; combining with the
- `crates/rs_cam_core/src/compute/config.rs:1291` // `DressupPolicy::strip_all("Incompatible with Project Curve: each
- `crates/rs_cam_core/src/compute/config.rs:1307` /// disabling any dressup that's geometrically incompatible with how the
- `crates/rs_cam_core/src/compute/config.rs:1308` /// operation emits toolpaths. Intended as a one-shot migration on
- `crates/rs_cam_core/src/compute/config.rs:1524` // Backwards-compat for callers that build `HeightContext::simple` /
- `crates/rs_cam_core/src/compute/config.rs:1634` fn backward_compat_old_json() {
- `crates/rs_cam_core/src/compute/execute.rs:274` /// migrated family shares ONE signature; in particular `cancel` is
- `crates/rs_cam_core/src/compute/execute.rs:375` /// keeps the legacy distance-only hookup decision — production
- `crates/rs_cam_core/src/compute/toolpath_stats.rs:1236` /// value AND the retired one is what makes the migration checkable on a real
- `crates/rs_cam_core/src/compute/catalog/registry.rs:251` "Incompatible with V-carve: each run starts on the design outline at \
- `crates/rs_cam_core/src/compute/catalog/registry.rs:307` "Incompatible with inlay V walls: a lead or an entry at the design \
- `crates/rs_cam_core/src/compute/catalog/registry.rs:363` "Incompatible with inlay V walls: a lead or an entry at the design \
- `crates/rs_cam_core/src/compute/catalog/registry.rs:611` "Incompatible with 3D Finish: each raster segment's ramp entry would carve a diagonal trench across the stock.",
- `crates/rs_cam_core/src/compute/catalog/registry.rs:873` "Incompatible with Unified Finish: ramp/lead/link dressups would carve diagonal trenches across the mesh surface; the op emits its own surfa
- `crates/rs_cam_core/src/compute/catalog/registry.rs:1170` "Incompatible with Project Curve: each ring would get a phantom diagonal cut.",
- `crates/rs_cam_core/src/compute/catalog/tests.rs:30` // `generate_adapter_migration_is_an_explicit_per_op_decision`
- `crates/rs_cam_core/src/compute/catalog/tests.rs:966` let legacy: ProjectCurveConfig = serde_json::from_str(
- `crates/rs_cam_core/src/compute/catalog/tests.rs:970` assert_eq!(legacy.projection, ProjectCurveProjection::Cutter);
- `crates/rs_cam_core/src/compute/catalog/schema.rs:545` /// geometrically incompatible with this op: compute strips them all
- `crates/rs_cam_core/src/compute/execute/shared.rs:180` /// stage is then byte-identical to the legacy surface-riding link, which is
- `crates/rs_cam_core/src/compute/execute/project_curve_chaining.rs:300` let legacy: ProjectCurveConfig = serde_json::from_str(
- `crates/rs_cam_core/src/compute/execute/project_curve_chaining.rs:305` legacy.chain_distance_mm, 0.0,
- `crates/rs_cam_core/src/compute/execute/tests.rs:1240` .filter(|d| d.id.as_str() == crate::diagnostics::ids::COMPAT_TOOL_KIND_REFUSED)
- `crates/rs_cam_core/src/material/mod.rs:1704` // Legacy alias: 2026-05-30 Phase 2C renamed
- `crates/rs_cam_core/src/material/mod.rs:1707` // compat for project files saved before the rename. Write
- `crates/rs_cam_core/src/material/mod.rs:2252` fn legacy_southern_yellow_pine_key_aliases_to_longleaf() {
- `crates/rs_cam_core/src/material/mod.rs:2256` // `LongleafPine` (Janka 870 lbf, Wood Database). Legacy
- `crates/rs_cam_core/src/material/mod.rs:2266` "legacy SYP key must map to LongleafPine"
- `crates/rs_cam_core/src/gcode/mod.rs:108` /// Re-export of `emitter::emit_program` for backward-compatible call
- `crates/rs_cam_core/src/gcode/mod.rs:577` /// STALE. The arm used to return `true` for backward compatibility
- `crates/rs_cam_core/src/gcode/mod.rs:1742` fn checked_phased_export_is_byte_identical_to_legacy_emitter() {
- `crates/rs_cam_core/src/gcode/mod.rs:1786` let legacy = emit_gcode_phased(&phases, post::grbl());
- `crates/rs_cam_core/src/gcode/mod.rs:1795` assert_eq!(checked, legacy);
- `crates/rs_cam_core/src/gcode/mod.rs:1799` fn checked_multi_setup_export_is_byte_identical_to_legacy_emitter() {
- `crates/rs_cam_core/src/gcode/mod.rs:1851` let legacy = emit_gcode_multi_setup(&setups, post::grbl(), 15.0);
- `crates/rs_cam_core/src/gcode/mod.rs:1861` assert_eq!(checked, legacy);
- `crates/rs_cam_core/src/gcode/modal.rs:3` //! Captures the controller-state book-keeping the legacy emitter did
- `crates/rs_cam_core/src/gcode/post.rs:126` /// G2/G3 word. Some legacy controllers reject sub-mm arcs outright;
- `crates/rs_cam_core/src/gcode/post.rs:308` /// Backward-compat default for post TOMLs lacking a `tool_change`
- `crates/rs_cam_core/src/gcode/post.rs:749` // Backward compat: a post TOML without `tool_change` must parse
- `crates/rs_cam_core/src/gcode/post.rs:750` // and fall back to the legacy M5 + M6 pair.
- `crates/rs_cam_core/src/gcode/post.rs:752` name = "Legacy"
- `crates/rs_cam_core/src/gcode/emitter.rs:12` //! Byte-parity vs the legacy `PostProcessor` trait is enforced by the
- `crates/rs_cam_core/src/gcode/emitter.rs:413` /// skipped and the legacy threshold-only behaviour applies.
- `crates/rs_cam_core/src/gcode/program_builder.rs:3` //! Three entry points mirror the legacy emitter modes:
- `crates/rs_cam_core/src/gcode/program_builder.rs:11` //! `emit_program`, yield byte-identical output to the legacy direct
- `crates/rs_cam_core/src/gcode/program_builder.rs:325` // Tool change (no idx>0 guard in multi-setup; the legacy
- `crates/rs_cam_mcp/src/server.rs:2834` /// wire (operator ruling 2026-09-16, no legacy support).
- `crates/rs_cam_viz/src/host.rs:45` //! subject of later steps in that design's migration order.
- `crates/rs_cam_viz/src/error.rs:30` /// Escape hatch for incremental migration.
- `crates/rs_cam_viz/src/mcp_server.rs:1058` description = "Add a toolpath the way the GUI's Add menu does, by dispatching the same `AppEvent::AddToolpath` the menu item emits. Use this
- `crates/rs_cam_viz/src/app.rs:851` // is deprecated). We draw everything via panels nested in this root
- `crates/rs_cam_viz/src/lib.rs:100` // lets a later step in the migration drain MCP work from `about_to_wait`
- `crates/rs_cam_viz/src/render/toolpath_render.rs:9` // Re-export palette from centralized colors module for backward compatibility.
- `crates/rs_cam_viz/src/state/viewport.rs:212` /// rename carries no wire compatibility.
- `crates/rs_cam_viz/src/state/job.rs:41` /// `KeepOutZone`. Only the legacy project reader built them, and C01 and
- `crates/rs_cam_viz/src/ui/sim_op_list.rs:479` "Legacy semantic trace — spans invalidated"
- `crates/rs_cam_viz/src/ui/sim_op_list.rs:548` "Spans were invalidated; expand the legacy semantic trace fallback"
- `crates/rs_cam_viz/src/ui/theme.rs:10` //! migrate them, and it shrinks as they do.
- `crates/rs_cam_viz/src/ui/menu_bar.rs:26` // straight to the legacy direct-export pre-flight + file dialog.
- `crates/rs_cam_viz/src/ui/tokens.rs:306` /// that was WRONG, measured during the UP5–UP7 migration: **the strip has six
- `crates/rs_cam_viz/src/ui/tokens.rs:768` // Two agents migrating this crate independently reached the same wall, and
- `crates/rs_cam_viz/src/ui/components/pill.rs:90` /// Without one the pill keeps its legacy caller-supplied colour. UP3
- `crates/rs_cam_viz/src/ui/components/pill.rs:91` /// migrates the call sites; UP2 must not touch a production panel.
- `crates/rs_cam_viz/src/ui/components/pill.rs:171` // Legacy path, until UP3 migrates the call sites.
- `crates/rs_cam_viz/src/ui/overlays/registry.rs:625` // ignored (no legacy, operator ruling 2026-09-16).
- `crates/rs_cam_viz/src/ui/properties/mod.rs:735` /// compatibility — G-BOUNDARYINHERIT) and `planner_origin` (the
- `crates/rs_cam_viz/src/ui/properties/linking_dressup.rs:318` // Some dressups are geometrically incompatible with specific operations
- `crates/rs_cam_viz/src/ui/properties/linking_dressup.rs:326` let op_incompatible_msg: Option<&str> = dressup_policy.strip_all_reason;
- `crates/rs_cam_viz/src/ui/properties/linking_dressup.rs:333` // keep using op_incompatible_msg).
- `crates/rs_cam_viz/src/ui/properties/linking_dressup.rs:334` let entry_disabled_msg: Option<&str> = op_incompatible_msg.or_else(|| {
- `crates/rs_cam_viz/src/ui/properties/linking_dressup.rs:486` ui.add_enabled_ui(op_incompatible_msg.is_none(), |ui| {
- `crates/rs_cam_viz/src/ui/properties/linking_dressup.rs:496` if let Some(msg) = op_incompatible_msg {
- `crates/rs_cam_viz/src/ui/properties/linking_dressup.rs:500` if op_incompatible_msg.is_none()
- `crates/rs_cam_viz/src/ui/properties/linking_dressup.rs:523` ui.add_enabled_ui(op_incompatible_msg.is_none(), |ui| {
- `crates/rs_cam_viz/src/ui/properties/linking_dressup.rs:531` if let Some(msg) = op_incompatible_msg {
- `crates/rs_cam_viz/src/ui/properties/linking_dressup.rs:535` if op_incompatible_msg.is_none()
- `crates/rs_cam_viz/src/ui/properties/operations/mod.rs:261` /// back to Auto, and a legacy project can carry a pin that puts the resolved
- `crates/rs_cam_viz/src/ui/properties/operations/boundary_2d.rs:393` ui.selectable_value(&mut cfg.cleanup_strategy, CleanupStrategy::Legacy, "Legacy");
- `crates/rs_cam_viz/src/app/mcp/diagnostics.rs:697` /// keeps backward compatibility with the existing string vocabulary.
- `crates/rs_cam_viz/src/app/mcp/simulation.rs:645` // authoritative. The legacy keys keep their exact values for wire
- `crates/rs_cam_viz/src/app/mcp/simulation.rs:646` // compatibility; the disambiguating names sit beside them and say
