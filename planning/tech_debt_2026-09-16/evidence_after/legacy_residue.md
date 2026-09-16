# Legacy / compat / migration residue in production sources (479 lines, 103 files)

Ruling: no legacy project-format support; judge every hit. Historical notes that say something WAS removed are fine; code that still reads or writes an old shape is not.

## Per file

| hits | file |
|---|---|
| 37 | `crates/rs_cam_core/src/compute/execute.rs` |
| 28 | `crates/rs_cam_core/src/compute/operation_configs.rs` |
| 25 | `crates/rs_cam_core/src/feeds/suggest.rs` |
| 19 | `crates/rs_cam_core/src/finish_setup.rs` |
| 19 | `crates/rs_cam_core/src/feeds/vendor_lookup.rs` |
| 17 | `crates/rs_cam_core/src/compute/catalog.rs` |
| 15 | `crates/rs_cam_core/src/feeds/vendor_lut.rs` |
| 12 | `crates/rs_cam_core/src/pencil.rs` |
| 12 | `crates/rs_cam_core/src/compute/config.rs` |
| 11 | `crates/rs_cam_core/src/feeds/rationale.rs` |
| 11 | `crates/rs_cam_viz/src/ui/properties/mod.rs` |
| 10 | `crates/rs_cam_core/src/unified_finish.rs` |
| 10 | `crates/rs_cam_core/src/dexel_stock/stamping.rs` |
| 10 | `crates/rs_cam_core/src/adaptive3d/path.rs` |
| 9 | `crates/rs_cam_core/src/dressup.rs` |
| 9 | `crates/rs_cam_core/src/diagnostics/adapters/from_generation.rs` |
| 9 | `crates/rs_cam_core/src/gcode/mod.rs` |
| 8 | `crates/rs_cam_core/src/scallop.rs` |
| 8 | `crates/rs_cam_core/src/adaptive/path.rs` |
| 7 | `crates/rs_cam_core/src/simulation_cut.rs` |
| 7 | `crates/rs_cam_core/src/tool_load/verdict.rs` |
| 6 | `crates/rs_cam_core/src/toolpath.rs` |
| 6 | `crates/rs_cam_core/src/surface_link.rs` |
| 6 | `crates/rs_cam_core/src/diagnostics/ids.rs` |
| 6 | `crates/rs_cam_core/src/diagnostics/adapters/from_static_checks.rs` |
| 6 | `crates/rs_cam_core/src/feeds/mod.rs` |
| 5 | `crates/rs_cam_core/src/material.rs` |
| 5 | `crates/rs_cam_core/src/machine_kinematics.rs` |
| 5 | `crates/rs_cam_core/src/narrate.rs` |
| 5 | `crates/rs_cam_core/src/session/mod.rs` |
| 5 | `crates/rs_cam_core/src/tool_load/optimize/mod.rs` |
| 5 | `crates/rs_cam_core/src/gcode/post.rs` |
| 4 | `crates/rs_cam_core/src/feed_modulation.rs` |
| 4 | `crates/rs_cam_core/src/diagnostics/tests.rs` |
| 4 | `crates/rs_cam_core/src/session/compute.rs` |
| 4 | `crates/rs_cam_core/src/adaptive3d/mod.rs` |
| 4 | `crates/rs_cam_core/src/adaptive/mod.rs` |
| 4 | `crates/rs_cam_core/src/compute/stats.rs` |
| 3 | `crates/rs_cam_core/src/dexel_mesh.rs` |
| 3 | `crates/rs_cam_core/src/finish_surface_cache.rs` |
| 3 | `crates/rs_cam_core/src/steep_shallow.rs` |
| 3 | `crates/rs_cam_core/src/diagnostics/adapters/from_project_diagnostics.rs` |
| 3 | `crates/rs_cam_core/src/adaptive3d/clearing.rs` |
| 3 | `crates/rs_cam_core/src/gcode/program_builder.rs` |
| 3 | `crates/rs_cam_viz/src/ui/components/pill.rs` |
| 3 | `crates/rs_cam_viz/src/app/mcp.rs` |
| 2 | `crates/rs_cam_core/src/ramp_finish.rs` |
| 2 | `crates/rs_cam_core/src/dexel_mesh_mc.rs` |
| 2 | `crates/rs_cam_core/src/machine.rs` |
| 2 | `crates/rs_cam_core/src/collision.rs` |
| 2 | `crates/rs_cam_core/src/reach.rs` |
| 2 | `crates/rs_cam_core/src/simulation.rs` |
| 2 | `crates/rs_cam_core/src/arcfit.rs` |
| 2 | `crates/rs_cam_core/src/session/mutation.rs` |
| 2 | `crates/rs_cam_core/src/session/project_file.rs` |
| 2 | `crates/rs_cam_core/src/tool_load/locality.rs` |
| 2 | `crates/rs_cam_core/src/tool_load/mod.rs` |
| 2 | `crates/rs_cam_core/src/tool_load/optimize/strategy/retarget.rs` |
| 2 | `crates/rs_cam_core/src/feeds/profile.rs` |
| 2 | `crates/rs_cam_core/src/compute/execute/project_curve_chaining.rs` |
| 2 | `crates/rs_cam_core/src/gcode/emitter.rs` |
| 2 | `crates/rs_cam_viz/src/ui/sim_op_list.rs` |
| 2 | `crates/rs_cam_viz/src/ui/tokens.rs` |
| 2 | `crates/rs_cam_viz/src/ui/properties/stock.rs` |
| 2 | `crates/rs_cam_viz/src/ui/properties/operations/boundary_2d.rs` |
| 2 | `crates/rs_cam_cli/src/project.rs` |
| 2 | `crates/rs_cam_cli/src/job.rs` |
| 1 | `crates/rs_cam_core/src/dxf_input.rs` |
| 1 | `crates/rs_cam_core/src/spiral_finish.rs` |
| 1 | `crates/rs_cam_core/src/dropcutter.rs` |
| 1 | `crates/rs_cam_core/src/semantic_trace.rs` |
| 1 | `crates/rs_cam_core/src/ids.rs` |
| 1 | `crates/rs_cam_core/src/toolpath_spans.rs` |
| 1 | `crates/rs_cam_core/src/monotone_cells.rs` |
| 1 | `crates/rs_cam_core/src/grid2.rs` |
| 1 | `crates/rs_cam_core/src/sim_triage.rs` |
| 1 | `crates/rs_cam_core/src/diagnostics/mod.rs` |
| 1 | `crates/rs_cam_core/src/diagnostics/adapters/from_stale_default.rs` |
| 1 | `crates/rs_cam_core/src/session/multitool.rs` |
| 1 | `crates/rs_cam_core/src/session/save.rs` |
| 1 | `crates/rs_cam_core/src/dexel_stock/band.rs` |
| 1 | `crates/rs_cam_core/src/dexel_stock/whole_path.rs` |
| 1 | `crates/rs_cam_core/src/adaptive3d/search.rs` |
| 1 | `crates/rs_cam_core/src/tool_load/chipload.rs` |
| 1 | `crates/rs_cam_core/src/tool_load/deflection.rs` |
| 1 | `crates/rs_cam_core/src/tool_load/power.rs` |
| 1 | `crates/rs_cam_core/src/tool_load/optimize/policy.rs` |
| 1 | `crates/rs_cam_core/src/tool_load/optimize/delta.rs` |
| 1 | `crates/rs_cam_core/src/compute/simulate.rs` |
| 1 | `crates/rs_cam_core/src/compute/tool_config.rs` |
| 1 | `crates/rs_cam_core/src/gcode/modal.rs` |
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
| 1 | `crates/rs_cam_viz/src/ui/properties/operations/mod.rs` |
| 1 | `crates/rs_cam_viz/src/controller/events/mod.rs` |

## Lines

- `crates/rs_cam_core/src/ramp_finish.rs:404` /// `LegacyEnvelopeQuarter` until PR-8a, under the approved Checkpoint B.
- `crates/rs_cam_core/src/ramp_finish.rs:427` /// legacy cell does not (gated behind Checkpoint C), and steep/shallow's
- `crates/rs_cam_core/src/dxf_input.rs:5` //! - **Polyline** — closed legacy polylines (with optional bulge arcs)
- `crates/rs_cam_core/src/dexel_mesh.rs:105` /// Backwards-compatible helper for callers that specifically want the top
- `crates/rs_cam_core/src/dexel_mesh.rs:426` // 50-vertex / 288-index layout of the legacy heightmap. Assert
- `crates/rs_cam_core/src/dexel_mesh.rs:567` /// than the legacy fixed-index layout.
- `crates/rs_cam_core/src/dexel_mesh_mc.rs:3` //! Replaces the cell-centre heightmap mesh produced by the legacy
- `crates/rs_cam_core/src/dexel_mesh_mc.rs:648` // cuts match the legacy behaviour.
- `crates/rs_cam_core/src/material.rs:1734` // Legacy alias: 2026-05-30 Phase 2C renamed
- `crates/rs_cam_core/src/material.rs:1737` // compat for project files saved before the rename. Write
- `crates/rs_cam_core/src/material.rs:2245` fn legacy_southern_yellow_pine_key_aliases_to_longleaf() {
- `crates/rs_cam_core/src/material.rs:2249` // `LongleafPine` (Janka 870 lbf, Wood Database). Legacy
- `crates/rs_cam_core/src/material.rs:2259` "legacy SYP key must map to LongleafPine"
- `crates/rs_cam_core/src/spiral_finish.rs:610` /// the point-runs migration, non-contacted spiral samples were simply
- `crates/rs_cam_core/src/machine.rs:86` /// serde compatibility with existing project files and
- `crates/rs_cam_core/src/machine.rs:327` /// production path reads it. The viz legacy project loader that also
- `crates/rs_cam_core/src/finish_surface_cache.rs:93` //! `cell_source`). `explicit(0.75)` and a `legacy_envelope_quarter` that
- `crates/rs_cam_core/src/finish_surface_cache.rs:503` let derived = FinishResolutionPolicy::legacy_envelope_quarter(&ball, 0.05);
- `crates/rs_cam_core/src/finish_surface_cache.rs:518` let floored = FinishResolutionPolicy::legacy_envelope_quarter(&fine, 0.5);
- `crates/rs_cam_core/src/unified_finish.rs:30` //! up). `claims: None` reproduces the pre-v3 op byte-for-byte: `decompose`
- `crates/rs_cam_core/src/unified_finish.rs:337` /// **Serde compatibility is load-bearing.** The two pre-A/M6 names are
- `crates/rs_cam_core/src/unified_finish.rs:559` /// reproduces the pre-v3 op exactly: no detector run, no crease claims, no
- `crates/rs_cam_core/src/unified_finish.rs:665` /// surface, same as the pre-v3 op. Creases still claim corridors.
- `crates/rs_cam_core/src/unified_finish.rs:775` /// the GUI span list, so this string is a compatibility surface.
- `crates/rs_cam_core/src/unified_finish.rs:926` /// on any plain ball — the migration moves tapered tools only.
- `crates/rs_cam_core/src/unified_finish.rs:1587` // byte-identical to the pre-v3 op (module doc).
- `crates/rs_cam_core/src/unified_finish.rs:2351` // material and the legacy surface-riding link is correct;
- `crates/rs_cam_core/src/unified_finish.rs:3543` /// table did, so the migration is provably value-preserving and the
- `crates/rs_cam_core/src/unified_finish.rs:4233` fn claims_off_matches_legacy_band_only_output() {
- `crates/rs_cam_core/src/collision.rs:173` /// Use 0.0 to check endpoints only (legacy behavior).
- `crates/rs_cam_core/src/collision.rs:293` /// `step_mm <= 0.01` checks the move endpoint only (legacy behavior).
- `crates/rs_cam_core/src/dressup.rs:59` /// When absent, the entry keeps the legacy straight legs — honest only
- `crates/rs_cam_core/src/dressup.rs:82` /// fresh-stock entry keeps the legacy two-leg ramp exactly.
- `crates/rs_cam_core/src/dressup.rs:842` /// `None` at the call site keeps the legacy straight legs. The adaptive3d
- `crates/rs_cam_core/src/dressup.rs:1038` // LADDER OR PLUNGE, and never the legacy legs below. Both of the
- `crates/rs_cam_core/src/dressup.rs:1076` // The rapid keeps the air part at rapid speed, as the legacy
- `crates/rs_cam_core/src/dressup.rs:1181` // Legacy blind legs — honest only with no mesh surface to probe AND
- `crates/rs_cam_core/src/dressup.rs:1200` /// polyline round-trips to exactly the legacy moves. The first point of
- `crates/rs_cam_core/src/dressup.rs:3959` // legacy unconditional link.
- `crates/rs_cam_core/src/dressup.rs:3986` /// Entry safety with no surface probe — the legacy blind-leg
- `crates/rs_cam_core/src/dropcutter.rs:312` // compatibility — see the struct-level doc caveat: those two do not
- `crates/rs_cam_core/src/reach.rs:20` //! Ball tools migrate too (approved ruling — the matrix is the plan's
- `crates/rs_cam_core/src/reach.rs:705` /// envelope radius), so the migration moves tapered tools only.
- `crates/rs_cam_core/src/simulation_cut.rs:67` /// legacy `radial_engagement: f64` scalar was removed in the §10.3 follow-up
- `crates/rs_cam_core/src/simulation_cut.rs:76` /// length to divide by — legacy traces, drill/analytical samples, and
- `crates/rs_cam_core/src/simulation_cut.rs:148` /// backward-compatibility.
- `crates/rs_cam_core/src/simulation_cut.rs:176` /// Legacy wire name for axial cutting engagement. Pure-vertical plunges
- `crates/rs_cam_core/src/simulation_cut.rs:195` /// (dexel simulator) populate every applicable axis. Legacy traces
- `crates/rs_cam_core/src/simulation_cut.rs:198` /// want to reject pre-v4 traces explicitly.
- `crates/rs_cam_core/src/simulation_cut.rs:236` /// **`None` = not carried**, never "Unknown": legacy traces
- `crates/rs_cam_core/src/semantic_trace.rs:48` /// compatibility surface — but the KEY half of it does not have to be free
- `crates/rs_cam_core/src/steep_shallow.rs:512` /// SteepShallow selects `FinishResolutionMode::LegacyEnvelopeQuarter` — the
- `crates/rs_cam_core/src/steep_shallow.rs:516` /// # Why this op STAYS on the legacy cell (PR-8c, written deferral)
- `crates/rs_cam_core/src/steep_shallow.rs:557` FinishResolutionPolicy::legacy_envelope_quarter(cutter, tolerance)
- `crates/rs_cam_core/src/toolpath.rs:49` /// migrated. All in-tree generators emit non-`Unknown` tags; the
- `crates/rs_cam_core/src/toolpath.rs:80` /// Fallback for legacy / unaware generators. Deprecation marker.
- `crates/rs_cam_core/src/toolpath.rs:1054` // Non-intent-aware callers (legacy generators) still benefit from
- `crates/rs_cam_core/src/toolpath.rs:1057` // the caller is migrated, because we don't know whether it's
- `crates/rs_cam_core/src/toolpath.rs:1060` // reclassification work even for legacy generators.
- `crates/rs_cam_core/src/toolpath.rs:1070` // Body moves keep the caller-supplied intent (Unknown for legacy).
- `crates/rs_cam_core/src/finish_setup.rs:47` /// [`Self::LegacyEnvelopeQuarter`]: one named variant among several, not a
- `crates/rs_cam_core/src/finish_setup.rs:51` /// RampFinish, SteepShallow) select `LegacyEnvelopeQuarter` and the
- `crates/rs_cam_core/src/finish_setup.rs:61` LegacyEnvelopeQuarter,
- `crates/rs_cam_core/src/finish_setup.rs:83` /// legacy cell drove a 2.39 mm gouge on the narrow-valley fixture and a
- `crates/rs_cam_core/src/finish_setup.rs:90` /// numbers is that number, so this mode resolves to exactly the legacy
- `crates/rs_cam_core/src/finish_setup.rs:92` /// `finish_resolution_policy_pr3::ball_resolves_the_geo_mean_to_the_legacy_cell`.
- `crates/rs_cam_core/src/finish_setup.rs:104` Self::LegacyEnvelopeQuarter => "legacy envelope/4",
- `crates/rs_cam_core/src/finish_setup.rs:121` Self::LegacyEnvelopeQuarter => CellSource::EnvelopeRadius,
- `crates/rs_cam_core/src/finish_setup.rs:146` /// `(envelope_radius / 4).max(tolerance)` — [`FinishResolutionMode::LegacyEnvelopeQuarter`].
- `crates/rs_cam_core/src/finish_setup.rs:148` pub fn legacy_envelope_quarter(cutter: &dyn MillingCutter, tolerance: f64) -> Self {
- `crates/rs_cam_core/src/finish_setup.rs:154` FinishResolutionMode::LegacyEnvelopeQuarter,
- `crates/rs_cam_core/src/finish_setup.rs:426` /// those keep calling this variant so migrating them onto the shared helper
- `crates/rs_cam_core/src/finish_setup.rs:445` /// Build a [`FinishSurface`] under [`FinishResolutionMode::LegacyEnvelopeQuarter`]:
- `crates/rs_cam_core/src/finish_setup.rs:448` /// Adapter kept for callers (and sentries) that want the legacy formula
- `crates/rs_cam_core/src/finish_setup.rs:463` // is now ANSWERED-BY-NAME rather than hidden: `LegacyEnvelopeQuarter`
- `crates/rs_cam_core/src/finish_setup.rs:471` FinishResolutionPolicy::legacy_envelope_quarter(cutter, tolerance),
- `crates/rs_cam_core/src/finish_setup.rs:519` /// [`FinishResolutionMode::LegacyEnvelopeQuarter`], so on a tapered ball
- `crates/rs_cam_core/src/finish_setup.rs:635` /// Migrated verbatim from the identical `scallop.rs` / `ramp_finish.rs`
- `crates/rs_cam_core/src/finish_setup.rs:638` /// The extraction commit also migrated `execute.rs`, despite this comment
- `crates/rs_cam_core/src/feed_modulation.rs:14` //!   behaviour can opt back into the legacy algorithm without
- `crates/rs_cam_core/src/feed_modulation.rs:299` /// Captures both the legacy "how many moves changed" scalar and the
- `crates/rs_cam_core/src/feed_modulation.rs:630` // pre-T-11 behaviour for legacy and analytical traces rather than
- `crates/rs_cam_core/src/feed_modulation.rs:639` /// F-036 — "target band-mid" per-move feed (legacy heuristic).
- `crates/rs_cam_core/src/machine_kinematics.rs:141` /// `MachineKinematics::junction_deviation_mm` on legacy project files.
- `crates/rs_cam_core/src/machine_kinematics.rs:718` /// collects feed moves from legacy generators that never tagged
- `crates/rs_cam_core/src/machine_kinematics.rs:1648` fn legacy_kinematics_json_defaults_new_fields() {
- `crates/rs_cam_core/src/machine_kinematics.rs:1652` let legacy =
- `crates/rs_cam_core/src/machine_kinematics.rs:1654` let kin: MachineKinematics = serde_json::from_str(legacy).expect("legacy deserialize");
- `crates/rs_cam_core/src/pencil.rs:172` /// of the same diameter. `None` = legacy nominal-diameter behaviour.
- `crates/rs_cam_core/src/pencil.rs:183` /// this is `Some`. `None` keeps the legacy behaviour: emit a surface
- `crates/rs_cam_core/src/pencil.rs:929` /// The legacy symmetric fan with no per-point truncation or spacing —
- `crates/rs_cam_core/src/pencil.rs:1059` /// treated as too short to enter along and the legacy descent is kept.
- `crates/rs_cam_core/src/pencil.rs:1128` /// Returns `None` — keeping the legacy single descent — when there is no
- `crates/rs_cam_core/src/pencil.rs:1365` /// Legacy stay-down surface link: feed along the mesh straight into the
- `crates/rs_cam_core/src/pencil.rs:1390` /// the link's own path: keep the legacy stay-down link, byte for byte.
- `crates/rs_cam_core/src/pencil.rs:1537` /// matters most. `params.link_kinematics = None` keeps the legacy behaviour
- `crates/rs_cam_core/src/pencil.rs:1547` /// legacy single fed descent and links keep riding the mesh surface. Every
- `crates/rs_cam_core/src/pencil.rs:1762` // straight line into `first` that a legacy link ends
- `crates/rs_cam_core/src/pencil.rs:1934` // Wave D1: this legacy entry point predates the tip-float channel
- `crates/rs_cam_core/src/pencil.rs:2208` // longer read — it is deprecated and reported, see
- `crates/rs_cam_core/src/ids.rs:21` /// payloads are byte-compatible with the pre-newtype format.
- `crates/rs_cam_core/src/toolpath_spans.rs:196` /// [`Self::DressupArtifact`]: that kind carried two incompatible
- `crates/rs_cam_core/src/simulation.rs:11` //! The module doc called this module "legacy" until L13 (tech debt
- `crates/rs_cam_core/src/simulation.rs:12` //! 2026-09-16). It is not legacy. The simulation ENGINE is
- `crates/rs_cam_core/src/monotone_cells.rs:109` /// compatibility). At 0° that fast path is honest — the rotation there IS
- `crates/rs_cam_core/src/grid2.rs:16` //! adopters — deliberately NOT migrated here, since each has its own cosmetic
- `crates/rs_cam_core/src/narrate.rs:211` /// cutting. That keeps the GUI's intent signal (a legacy generator's
- `crates/rs_cam_core/src/narrate.rs:288` //  - deprecated_dial:   surfaced as a load/diagnostic notice
- `crates/rs_cam_core/src/narrate.rs:291` deprecated_dial: _,
- `crates/rs_cam_core/src/narrate.rs:303` //    `deprecated_dial`. It is a statement about the rest-region
- `crates/rs_cam_core/src/narrate.rs:845` /// Legacy Z-level discovery: cluster cutting move Z-coordinates with
- `crates/rs_cam_core/src/sim_triage.rs:175` /// **LEGACY.** Contiguous air/low-engagement RUNS, coalesced per
- `crates/rs_cam_core/src/arcfit.rs:53` /// - When `spans_valid` is `false`, the legacy unconditional collapse runs and
- `crates/rs_cam_core/src/arcfit.rs:1174` // legacy unconditional collapse, and spans pass through untouched.
- `crates/rs_cam_core/src/scallop.rs:1833` /// Scallop selects `FinishResolutionMode::LegacyEnvelopeQuarter` — the same
- `crates/rs_cam_core/src/scallop.rs:1849` FinishResolutionPolicy::legacy_envelope_quarter(cutter, tolerance)
- `crates/rs_cam_core/src/scallop.rs:1941` /// `None` is the legacy relink, byte for byte: `reorder: false`,
- `crates/rs_cam_core/src/scallop.rs:2166` /// `link_stage: None` is the legacy intra-pass relink, byte for byte — which
- `crates/rs_cam_core/src/scallop.rs:2679` // legacy arm rejected 493 as `too_far` and ZERO on the surface or
- `crates/rs_cam_core/src/scallop.rs:2687` // test): the legacy literal below, byte for byte.
- `crates/rs_cam_core/src/scallop.rs:2695` let legacy = crate::surface_link::RelinkParams {
- `crates/rs_cam_core/src/scallop.rs:2731` None => (legacy, None),
- `crates/rs_cam_core/src/surface_link.rs:79` /// legacy behaviour.
- `crates/rs_cam_core/src/surface_link.rs:220` /// Fixed ends. The default, and the byte-identical legacy treatment.
- `crates/rs_cam_core/src/surface_link.rs:266` /// byte-identical to the legacy surface-riding link.
- `crates/rs_cam_core/src/surface_link.rs:485` /// [`LinkCeiling`]. `None` keeps the legacy surface-riding link — the
- `crates/rs_cam_core/src/surface_link.rs:571` /// [`FragmentKind`], which is the byte-identical legacy arm.
- `crates/rs_cam_core/src/surface_link.rs:957` // material — the legacy surface-riding link is correct
- `crates/rs_cam_core/src/diagnostics/mod.rs:19` //! Each emitted shape-incompatible findings, sometimes contradicting
- `crates/rs_cam_core/src/diagnostics/ids.rs:199` pub const CONFIG_DEPRECATED_DIAL: &str = "config.deprecated_dial";
- `crates/rs_cam_core/src/diagnostics/ids.rs:224` // ── Tool / operation compatibility ───────────────────────────────────
- `crates/rs_cam_core/src/diagnostics/ids.rs:225` pub const COMPAT_END_MILL_SCALLOP_PENCIL: &str = "compat.end_mill_on_scallop_pencil";
- `crates/rs_cam_core/src/diagnostics/ids.rs:226` pub const COMPAT_BALL_NOSE_FLAT_CLEARING: &str = "compat.ball_nose_on_flat_clearing";
- `crates/rs_cam_core/src/diagnostics/ids.rs:328` COMPAT_END_MILL_SCALLOP_PENCIL,
- `crates/rs_cam_core/src/diagnostics/ids.rs:329` COMPAT_BALL_NOSE_FLAT_CLEARING,
- `crates/rs_cam_core/src/diagnostics/tests.rs:140` "COMPAT_END_MILL_SCALLOP_PENCIL",
- `crates/rs_cam_core/src/diagnostics/tests.rs:141` ids::COMPAT_END_MILL_SCALLOP_PENCIL,
- `crates/rs_cam_core/src/diagnostics/tests.rs:144` "COMPAT_BALL_NOSE_FLAT_CLEARING",
- `crates/rs_cam_core/src/diagnostics/tests.rs:145` ids::COMPAT_BALL_NOSE_FLAT_CLEARING,
- `crates/rs_cam_core/src/diagnostics/adapters/from_project_diagnostics.rs:5` //! high, generated-empty) into the unified schema. The legacy
- `crates/rs_cam_core/src/diagnostics/adapters/from_project_diagnostics.rs:53` severity: severity_from_legacy(v.severity),
- `crates/rs_cam_core/src/diagnostics/adapters/from_project_diagnostics.rs:65` fn severity_from_legacy(s: VerdictSeverity) -> Severity {
- `crates/rs_cam_core/src/diagnostics/adapters/from_generation.rs:42` out.extend(deprecated_dial(toolpath_id, stats));
- `crates/rs_cam_core/src/diagnostics/adapters/from_generation.rs:67` /// notice nobody reads — the same rule [`deprecated_dial`] and
- `crates/rs_cam_core/src/diagnostics/adapters/from_generation.rs:113` /// [`deprecated_dial`] makes, and for the same reason. Nothing is wrong with
- `crates/rs_cam_core/src/diagnostics/adapters/from_generation.rs:192` /// and the same rule already governs `zero_removal` and `deprecated_dial`.
- `crates/rs_cam_core/src/diagnostics/adapters/from_generation.rs:277` /// deprecated-dial rule guards against: claims run only on a `UnifiedFinish`
- `crates/rs_cam_core/src/diagnostics/adapters/from_generation.rs:422` /// `record_deprecated_dial` follows.
- `crates/rs_cam_core/src/diagnostics/adapters/from_generation.rs:492` fn deprecated_dial(toolpath_id: ToolpathId, stats: &ToolpathStats) -> Vec<Diagnostic> {
- `crates/rs_cam_core/src/diagnostics/adapters/from_generation.rs:494` let Some(f) = stats.deprecated_dial.as_deref() else {
- `crates/rs_cam_core/src/diagnostics/adapters/from_generation.rs:498` id: DiagnosticId::from(ids::CONFIG_DEPRECATED_DIAL),
- `crates/rs_cam_core/src/diagnostics/adapters/from_stale_default.rs:54` // matching on `legacy_rule_id` can branch identically against
- `crates/rs_cam_core/src/diagnostics/adapters/from_static_checks.rs:6` //! stepover > diameter) live here, as do tool/op compatibility rules
- `crates/rs_cam_core/src/diagnostics/adapters/from_static_checks.rs:42` out.extend(tool_op_compat_checks(&scope, op, tool));
- `crates/rs_cam_core/src/diagnostics/adapters/from_static_checks.rs:129` // ── tool/operation compatibility ────────────────────────────────────
- `crates/rs_cam_core/src/diagnostics/adapters/from_static_checks.rs:131` fn tool_op_compat_checks(
- `crates/rs_cam_core/src/diagnostics/adapters/from_static_checks.rs:147` id: DiagnosticId::from(ids::COMPAT_BALL_NOSE_FLAT_CLEARING),
- `crates/rs_cam_core/src/diagnostics/adapters/from_static_checks.rs:174` id: DiagnosticId::from(ids::COMPAT_END_MILL_SCALLOP_PENCIL),
- `crates/rs_cam_core/src/session/multitool.rs:1478` // Absent entries default to unified — byte-identical legacy plans.
- `crates/rs_cam_core/src/session/mutation.rs:489` // Enforce the per-operation dressup invariant so incompatible
- `crates/rs_cam_core/src/session/mutation.rs:1143` /// `None` reverts a `Drill` op to its legacy all-polygon-centroids
- `crates/rs_cam_core/src/session/mod.rs:1380` /// Structured project-level verdict. Replaces the legacy single-line
- `crates/rs_cam_core/src/session/mod.rs:1382` /// (with the highest-severity verdict's headline) for backward compatibility
- `crates/rs_cam_core/src/session/mod.rs:2378` /// L1. The load-time dressup migration rewrites a value the
- `crates/rs_cam_core/src/session/mod.rs:2387` "name = \"Dressup Migration\"\n",
- `crates/rs_cam_core/src/session/mod.rs:2422` // The migration still writes what it always wrote.
- `crates/rs_cam_core/src/session/save.rs:63` /// Ported verbatim in behaviour from `rs_cam_viz`'s fallback loader
- `crates/rs_cam_core/src/session/compute.rs:3974` // whose generator hasn't been migrated still gets flagged.
- `crates/rs_cam_core/src/session/compute.rs:4265` /// `samples` still hold the legacy `Arc` and we want them to remain
- `crates/rs_cam_core/src/session/compute.rs:4791` /// matching the legacy in-diagnostics behavior.
- `crates/rs_cam_core/src/session/compute.rs:7693` deprecated_dial: None,
- `crates/rs_cam_core/src/session/project_file.rs:736` // One-shot migration: projects saved before operation-specific dressup
- `crates/rs_cam_core/src/session/project_file.rs:752` "Normalized incompatible dressups on load"
- `crates/rs_cam_core/src/dexel_stock/band.rs:262` /// instead of letting rows migrate between cores on every subsegment — is a
- `crates/rs_cam_core/src/dexel_stock/whole_path.rs:8` //! — the same rows migrate between cores on every subsegment.
- `crates/rs_cam_core/src/dexel_stock/stamping.rs:123` /// Both legacy predicates are **monotone in `d_sq`** (`sqrt` is monotone and
- `crates/rs_cam_core/src/dexel_stock/stamping.rs:127` /// squared bound and walk ULPs until the legacy predicate's own answer flips.
- `crates/rs_cam_core/src/dexel_stock/stamping.rs:134` /// Smallest `d_sq` for which the legacy test `√d_sq − ext_diag ≥ r` holds.
- `crates/rs_cam_core/src/dexel_stock/stamping.rs:137` /// Largest `d_sq` for which the legacy test `√d_sq + ext_diag ≤ r` holds,
- `crates/rs_cam_core/src/dexel_stock/stamping.rs:142` /// regime — no cell can satisfy the legacy test at all, because distances
- `crates/rs_cam_core/src/dexel_stock/stamping.rs:2226` /// the same technique `legacy_fast_path` uses for S7 — and this asserts
- `crates/rs_cam_core/src/dexel_stock/stamping.rs:2370` fn legacy_fast_path(center_d_sq: f64, r_sq: f64, cs: f64) -> Option<f32> {
- `crates/rs_cam_core/src/dexel_stock/stamping.rs:2454` let legacy = legacy_fast_path(d_sq, r_sq, cs);
- `crates/rs_cam_core/src/dexel_stock/stamping.rs:2456` if legacy != s7 {
- `crates/rs_cam_core/src/dexel_stock/stamping.rs:2458` "r={r} cs={cs} d={d:.17e}: legacy {legacy:?}, S7 {s7:?}"
- `crates/rs_cam_core/src/adaptive3d/mod.rs:44` /// Clear all areas at each Z level globally (default, backward compat).
- `crates/rs_cam_core/src/adaptive3d/mod.rs:131` /// Entry strategy (default: Plunge for backward compat).
- `crates/rs_cam_core/src/adaptive3d/mod.rs:139` /// Region ordering strategy (default: Global for backward compat).
- `crates/rs_cam_core/src/adaptive3d/mod.rs:681` // but the unit-test fixtures here exercise the legacy code path
- `crates/rs_cam_core/src/adaptive3d/clearing.rs:1954` crate::adaptive::CleanupStrategy::Legacy
- `crates/rs_cam_core/src/adaptive3d/clearing.rs:2006` // For non-Legacy cleanup strategies, run the same cleanup
- `crates/rs_cam_core/src/adaptive3d/clearing.rs:2015` crate::adaptive::CleanupStrategy::Legacy => segs_2d_raw,
- `crates/rs_cam_core/src/adaptive3d/path.rs:1102` /// otherwise (caller falls back to the legacy retract path).
- `crates/rs_cam_core/src/adaptive3d/path.rs:1712` // F-038b: legacy `segments_to_toolpath` tests need a mesh + index +
- `crates/rs_cam_core/src/adaptive3d/path.rs:1726` fn legacy_test_mesh() -> (crate::mesh::TriangleMesh, crate::mesh::SpatialIndex) {
- `crates/rs_cam_core/src/adaptive3d/path.rs:1749` fn legacy_test_cutter() -> crate::tool::FlatEndmill {
- `crates/rs_cam_core/src/adaptive3d/path.rs:1805` // F-038b: legacy `segments_to_toolpath` unit tests expect the
- `crates/rs_cam_core/src/adaptive3d/path.rs:1845` let cutter = legacy_test_cutter();
- `crates/rs_cam_core/src/adaptive3d/path.rs:1929` let cutter = legacy_test_cutter();
- `crates/rs_cam_core/src/adaptive3d/path.rs:1995` let cutter = legacy_test_cutter();
- `crates/rs_cam_core/src/adaptive3d/path.rs:2152` let (mesh, si) = legacy_test_mesh();
- `crates/rs_cam_core/src/adaptive3d/path.rs:2153` let cutter = legacy_test_cutter();
- `crates/rs_cam_core/src/adaptive3d/search.rs:30` /// `fraction = cells_with_material / cells_at_z` (matches the legacy
- `crates/rs_cam_core/src/tool_load/locality.rs:17` //! authoritative signal. When no span data is plumbed (legacy traces,
- `crates/rs_cam_core/src/tool_load/locality.rs:154` /// as steady-state preserves the pre-D7 trip behaviour for legacy
- `crates/rs_cam_core/src/tool_load/mod.rs:127` /// Every compatible LUT row has a chipload range that, even at
- `crates/rs_cam_core/src/tool_load/mod.rs:184` "every compatible LUT row falls outside the machine's feed or RPM range"
- `crates/rs_cam_core/src/tool_load/chipload.rs:1119` /// Adapts this module's legacy positional-arg test calls to the
- `crates/rs_cam_core/src/tool_load/verdict.rs:120` ///   single legacy "chipload" clamp band; F-036c-style retro reads
- `crates/rs_cam_core/src/tool_load/verdict.rs:525` // Typed verdict scaffolding (G16 Step 7a). Lives alongside the legacy
- `crates/rs_cam_core/src/tool_load/verdict.rs:526` // flat `Verdict` until the per-gate evaluators migrate. No consumers
- `crates/rs_cam_core/src/tool_load/verdict.rs:999` /// chipload evaluator migrates (Step 7d). Carries both bounds-approach
- `crates/rs_cam_core/src/tool_load/verdict.rs:1335` /// `(&'static str, ExceedsReason)` pair returned by the legacy
- `crates/rs_cam_core/src/tool_load/verdict.rs:2107` /// migration (G16 Step 7). The MCP `get_tool_load_report` tool serves
- `crates/rs_cam_core/src/tool_load/verdict.rs:2452` /// Callers without a name resolver (legacy test fixtures, headless
- `crates/rs_cam_core/src/tool_load/deflection.rs:382` /// Adapts this module's legacy positional-arg test calls to the
- `crates/rs_cam_core/src/tool_load/power.rs:601` /// Adapts this module's legacy positional-arg test calls to the
- `crates/rs_cam_core/src/tool_load/optimize/policy.rs:622` hypothesis: "1.5mm matches conservative wood-router defaults for legacy grid plumbing.",
- `crates/rs_cam_core/src/tool_load/optimize/mod.rs:429` //    the legacy `run_stage_1_grid` (G16 Step 6c).
- `crates/rs_cam_core/src/tool_load/optimize/mod.rs:566` /// the (at most one) candidate it produces. Replaces the legacy
- `crates/rs_cam_core/src/tool_load/optimize/mod.rs:626` /// sim. Replaces the legacy `run_stage_f_retarget` (G16 Step 6b).
- `crates/rs_cam_core/src/tool_load/optimize/mod.rs:628` /// **Behaviour change.** The legacy chipload retarget produced a
- `crates/rs_cam_core/src/tool_load/optimize/mod.rs:772` /// Replaces the legacy `run_stage_1_grid` (G16 Step 6c) — same anchor
- `crates/rs_cam_core/src/tool_load/optimize/delta.rs:271` /// a generic state+peak helper once chipload + deflection migrate.
- `crates/rs_cam_core/src/tool_load/optimize/strategy/retarget.rs:11` //! **Behaviour change vs. old Stage F.** The legacy
- `crates/rs_cam_core/src/tool_load/optimize/strategy/retarget.rs:625` /// flip vs. the legacy `solve_chipload_retarget`.
- `crates/rs_cam_core/src/feeds/vendor_lut.rs:207` /// `migrate_ap_rule` binary from the row's `ap_rule` prose.
- `crates/rs_cam_core/src/feeds/vendor_lut.rs:273` /// compat for partial schema changes) and the test fails loud with
- `crates/rs_cam_core/src/feeds/vendor_lut.rs:433` // to the same bar (with a documented legacy allowlist that may only
- `crates/rs_cam_core/src/feeds/vendor_lut.rs:665` /// Phase 1 LUT migration invariant
- `crates/rs_cam_core/src/feeds/vendor_lut.rs:672` /// quantify. The migration binary (`migrate_ap_rule` example) keeps
- `crates/rs_cam_core/src/feeds/vendor_lut.rs:704` add an entry to `examples/migrate_ap_rule.rs` and re-run \
- `crates/rs_cam_core/src/feeds/vendor_lut.rs:705` the migration, or extend `LABEL_ONLY_AP_RULES` if the rule \
- `crates/rs_cam_core/src/feeds/vendor_lut.rs:712` /// F3.2 — legacy rows that predate the loader-validation rules and
- `crates/rs_cam_core/src/feeds/vendor_lut.rs:719` const LEGACY_DEGENERATE_RANGE_ROWS: &[&str] = &[
- `crates/rs_cam_core/src/feeds/vendor_lut.rs:771` /// modulo the shrink-only legacy allowlist above. Strict equality
- `crates/rs_cam_core/src/feeds/vendor_lut.rs:780` if !LEGACY_DEGENERATE_RANGE_ROWS.contains(&obs.observation_id.as_str()) {
- `crates/rs_cam_core/src/feeds/vendor_lut.rs:786` for legacy in LEGACY_DEGENERATE_RANGE_ROWS {
- `crates/rs_cam_core/src/feeds/vendor_lut.rs:788` violating.contains(legacy),
- `crates/rs_cam_core/src/feeds/vendor_lut.rs:789` "{legacy} no longer violates any rule — remove it from \
- `crates/rs_cam_core/src/feeds/vendor_lut.rs:790` LEGACY_DEGENERATE_RANGE_ROWS (the list may only shrink)"
- `crates/rs_cam_core/src/feeds/mod.rs:195` /// and compatibility decisions.
- `crates/rs_cam_core/src/feeds/mod.rs:201` ///   compatibility) where the geometry numbers don't matter.
- `crates/rs_cam_core/src/feeds/mod.rs:241` /// `facing_bit_is_a_lut_only_family`). Any compatibility logic
- `crates/rs_cam_core/src/feeds/mod.rs:881` /// Validate that the input's tool geometry is physically compatible
- `crates/rs_cam_core/src/feeds/mod.rs:4018` fn test_no_lut_backward_compatible() {
- `crates/rs_cam_core/src/feeds/mod.rs:4495` /// compatibility story (LUT routing, constraints) to be revisited
- `crates/rs_cam_core/src/feeds/profile.rs:15` //! - **Owns:** tool/op compatibility ([`FeedsError`] feasibility — NOT
- `crates/rs_cam_core/src/feeds/profile.rs:118` /// Tool × operation compatibility — `Err` carries the same
- `crates/rs_cam_core/src/feeds/suggest.rs:50` /// 2026-06-03 directive. Conservative preserves the v2.1 / pre-v3 recipe
- `crates/rs_cam_core/src/feeds/suggest.rs:64` /// `machine.max_feed_mm_min`. Equivalent to the pre-v2
- `crates/rs_cam_core/src/feeds/suggest.rs:88` /// support this". `FeedsWithGates` preserves the pre-v3.3 / v3.0d
- `crates/rs_cam_core/src/feeds/suggest.rs:96` /// risky. Pre-v3.3 / v3.0d behaviour.
- `crates/rs_cam_core/src/feeds/suggest.rs:689` /// op). Callers that want the legacy "always produce something"
- `crates/rs_cam_core/src/feeds/suggest.rs:2660` /// Pass 8: v1.3 combined-Suggest plunge-entry / DPP compatibility warning.
- `crates/rs_cam_core/src/feeds/suggest.rs:2968` /// compatibility warning. Returns `None` for non-Adaptive ops, for
- `crates/rs_cam_core/src/feeds/suggest.rs:3110` /// S1: with no scallop target the legacy formula-based stepover is
- `crates/rs_cam_core/src/feeds/suggest.rs:3115` fn scallop_height_none_preserves_legacy_ae() {
- `crates/rs_cam_core/src/feeds/suggest.rs:3119` "legacy DropCutter stepover on a 1 mm ball should stay small \
- `crates/rs_cam_core/src/feeds/suggest.rs:3448` /// legacy entry points it now backs — the funnel was extended, not
- `crates/rs_cam_core/src/feeds/suggest.rs:3451` fn apply_scope_matches_the_legacy_entry_points_exactly() {
- `crates/rs_cam_core/src/feeds/suggest.rs:3469` let mut legacy_op = base.clone();
- `crates/rs_cam_core/src/feeds/suggest.rs:3470` let mut legacy_prov = crate::feeds::FeedsProvenance::default();
- `crates/rs_cam_core/src/feeds/suggest.rs:3471` let legacy = match scope {
- `crates/rs_cam_core/src/feeds/suggest.rs:3476` legacy(
- `crates/rs_cam_core/src/feeds/suggest.rs:3477` &mut legacy_op,
- `crates/rs_cam_core/src/feeds/suggest.rs:3478` &mut legacy_prov,
- `crates/rs_cam_core/src/feeds/suggest.rs:3493` legacy_op.feed_rate(),
- `crates/rs_cam_core/src/feeds/suggest.rs:3498` legacy_op.plunge_rate(),
- `crates/rs_cam_core/src/feeds/suggest.rs:3503` legacy_op.spindle_rpm(),
- `crates/rs_cam_core/src/feeds/suggest.rs:3508` legacy_op.as_params().stepover(),
- `crates/rs_cam_core/src/feeds/suggest.rs:3513` legacy_op.as_params().depth_per_pass(),
- `crates/rs_cam_core/src/feeds/suggest.rs:4038` /// - DPP=9 mm, D=6 mm (pre-v1.1 motivating failure, ratio 1.5)
- `crates/rs_cam_core/src/feeds/suggest.rs:4047` (9.0, "pre-v1.1 case: DPP=9 mm on 6 mm tool (ratio 1.5)"),
- `crates/rs_cam_core/src/feeds/vendor_lookup.rs:139` /// that don't record an angle (legacy data) are exempt from the gate and
- `crates/rs_cam_core/src/feeds/vendor_lookup.rs:154` /// worse than `find_best_row` on angle-less legacy data. This is the slot
- `crates/rs_cam_core/src/feeds/vendor_lookup.rs:311` /// All compatible rows for the given criteria, sorted by composite score
- `crates/rs_cam_core/src/feeds/vendor_lookup.rs:656` fn materials_compatible(query: MaterialFamily, obs: MaterialFamily) -> bool {
- `crates/rs_cam_core/src/feeds/vendor_lookup.rs:669` if !materials_compatible(query.material_family, obs.material_family) {
- `crates/rs_cam_core/src/feeds/vendor_lookup.rs:672` if !tool_family_compatible(query.tool_family, obs.tool_family) {
- `crates/rs_cam_core/src/feeds/vendor_lookup.rs:763` /// Check if a query tool family is compatible with an observation tool family.
- `crates/rs_cam_core/src/feeds/vendor_lookup.rs:764` fn tool_family_compatible(query: ToolFamily, obs: ToolFamily) -> bool {
- `crates/rs_cam_core/src/feeds/vendor_lookup.rs:768` // Fallback compatibility
- `crates/rs_cam_core/src/feeds/vendor_lookup.rs:786` _ => 0, // shouldn't reach here due to compatible filter
- `crates/rs_cam_core/src/feeds/vendor_lookup.rs:826` fn find_best_row_matches_legacy_lookup_best() {
- `crates/rs_cam_core/src/feeds/vendor_lookup.rs:840` let via_legacy = lookup_best(&lut, &query).expect("legacy row");
- `crates/rs_cam_core/src/feeds/vendor_lookup.rs:841` assert_eq!(via_canonical.observation_id, via_legacy.observation_id);
- `crates/rs_cam_core/src/feeds/vendor_lookup.rs:1309` // With no query angle the gate is inert — every angle-compatible
- `crates/rs_cam_core/src/feeds/vendor_lookup.rs:1310` // row is eligible and the best-scoring one wins (legacy behaviour,
- `crates/rs_cam_core/src/feeds/vendor_lookup.rs:1373` // Either the new Spektra engrave row OR the legacy
- `crates/rs_cam_core/src/feeds/vendor_lookup.rs:1379` let won_the_legacy_row = result.observation_id == "amana-vgroove-softwood-trace-30deg-1f";
- `crates/rs_cam_core/src/feeds/vendor_lookup.rs:1381` won_an_angle_only_row || won_the_legacy_row,
- `crates/rs_cam_core/src/feeds/vendor_lookup.rs:1382` "expected angle-only or legacy 30° row, got {} (row_diameter={})",
- `crates/rs_cam_core/src/feeds/rationale.rs:44` /// a legacy-estimate disclaimer on a measured number is a new defect, not a
- `crates/rs_cam_core/src/feeds/rationale.rs:51` pub const LEGACY_ESTIMATE_LABEL: &str = "Legacy pre-simulation estimate";
- `crates/rs_cam_core/src/feeds/rationale.rs:53` /// The sentence that says *why* [`LEGACY_ESTIMATE_LABEL`] applies, so a
- `crates/rs_cam_core/src/feeds/rationale.rs:56` pub const LEGACY_ESTIMATE_NOTE: &str = "Estimated against the arc-mean chip observation the post-sim gate retired on 2026-08-06, \
- `crates/rs_cam_core/src/feeds/rationale.rs:105` /// The chipload figures on this entry are a **legacy pre-simulation
- `crates/rs_cam_core/src/feeds/rationale.rs:106` /// estimate** — see [`LEGACY_ESTIMATE_LABEL`].
- `crates/rs_cam_core/src/feeds/rationale.rs:112` /// estimate, not a gate observation ([`LEGACY_ESTIMATE_LABEL`]).
- `crates/rs_cam_core/src/feeds/rationale.rs:326` "{LEGACY_ESTIMATE_LABEL}: chipload {predicted_observed_chipload_before:.4} → {predicted_observed_chipload_after:.4} mm/tooth (LUT target {lu
- `crates/rs_cam_core/src/feeds/rationale.rs:347` "{LEGACY_ESTIMATE_LABEL}: {predicted_observed_mm_per_tooth:.4} mm/tooth vs LUT target {lut_target_mm_per_tooth:.4} at feed {feed_at_terminat
- `crates/rs_cam_core/src/feeds/rationale.rs:693` fn chipload_recalibration_entries_are_labelled_a_legacy_estimate() {
- `crates/rs_cam_core/src/feeds/rationale.rs:715` detail.contains(LEGACY_ESTIMATE_LABEL),
- `crates/rs_cam_core/src/adaptive/mod.rs:38` /// - `Legacy`: pre-2026-05 behaviour. After the main spiral, the planner
- `crates/rs_cam_core/src/adaptive/mod.rs:69` Legacy,
- `crates/rs_cam_core/src/adaptive/mod.rs:262` CleanupStrategy::Legacy => segments,
- `crates/rs_cam_core/src/adaptive/mod.rs:314` cleanup_strategy: CleanupStrategy::Legacy,
- `crates/rs_cam_core/src/adaptive/path.rs:105` cleanup_strategy: crate::adaptive::CleanupStrategy::Legacy,
- `crates/rs_cam_core/src/adaptive/path.rs:245` // Legacy cleanup strategy — adaptive3d and other downstream
- `crates/rs_cam_core/src/adaptive/path.rs:248` // their current behaviour via the Legacy path. If no DT-max
- `crates/rs_cam_core/src/adaptive/path.rs:252` if matches!(params.cleanup_strategy, CleanupStrategy::Legacy) {
- `crates/rs_cam_core/src/adaptive/path.rs:307` // For non-Legacy strategies with a successful helical entry:
- `crates/rs_cam_core/src/adaptive/path.rs:319` // per-region calls), the spiral runs Legacy-style across
- `crates/rs_cam_core/src/adaptive/path.rs:325` && !matches!(params.cleanup_strategy, CleanupStrategy::Legacy)
- `crates/rs_cam_core/src/adaptive/path.rs:479` && !matches!(params.cleanup_strategy, CleanupStrategy::Legacy)
- `crates/rs_cam_core/src/compute/catalog.rs:1538` /// geometrically incompatible with this op: compute strips them all
- `crates/rs_cam_core/src/compute/catalog.rs:1580` /// generation has been migrated onto the shared `GenerateFn`
- `crates/rs_cam_core/src/compute/catalog.rs:1583` /// in `execute_operation_annotated`. Migration state is pinned by
- `crates/rs_cam_core/src/compute/catalog.rs:1584` /// `generate_adapter_migration_is_an_explicit_per_op_decision`.
- `crates/rs_cam_core/src/compute/catalog.rs:1643` "enum:Legacy|ResidueMop|ContourParallelNarrow|ContourParallelHybrid",
- `crates/rs_cam_core/src/compute/catalog.rs:1849` // M8 (2026-09-03) — iso-field ring source; absent = legacy cascade.
- `crates/rs_cam_core/src/compute/catalog.rs:1874` // v3 S1/S2 claims pipeline: serde-defaulted for project-file back-compat
- `crates/rs_cam_core/src/compute/catalog.rs:1887` // territory_clip` doc) — same serde-defaulted back-compat treatment.
- `crates/rs_cam_core/src/compute/catalog.rs:1890` // back-compat treatment again.
- `crates/rs_cam_core/src/compute/catalog.rs:1906` // §9/§11 link caps, both serde-defaulted for back-compat:
- `crates/rs_cam_core/src/compute/catalog.rs:2306` "Incompatible with 3D Finish: each raster segment's ramp entry would carve a diagonal trench across the stock.",
- `crates/rs_cam_core/src/compute/catalog.rs:2428` "Incompatible with Unified Finish: ramp/lead/link dressups would carve diagonal trenches across the mesh surface; the op emits its own surfa
- `crates/rs_cam_core/src/compute/catalog.rs:2546` "Incompatible with Project Curve: each ring would get a phantom diagonal cut.",
- `crates/rs_cam_core/src/compute/catalog.rs:2631` // does. `None` keeps the legacy `ae_factor × diameter` stepover.
- `crates/rs_cam_core/src/compute/catalog.rs:2835` /// Phase 5 (T11): GenerateFn migration is a per-family DECISION,
- `crates/rs_cam_core/src/compute/catalog.rs:2836` /// not drift. Each op is either migrated (registry adapter — the
- `crates/rs_cam_core/src/compute/catalog.rs:2841` fn generate_adapter_migration_is_an_explicit_per_op_decision() {
- `crates/rs_cam_core/src/compute/stats.rs:59` deprecated_dial: None,
- `crates/rs_cam_core/src/compute/stats.rs:181` deprecated_dial: _,
- `crates/rs_cam_core/src/compute/stats.rs:208` deprecated_dial,
- `crates/rs_cam_core/src/compute/stats.rs:236` deprecated_dial: deprecated_dial.map(Box::new),
- `crates/rs_cam_core/src/compute/simulate.rs:1104` // transform invalidated the spans (legacy invalidators;
- `crates/rs_cam_core/src/compute/tool_config.rs:937` /// wire format, MCP error messages, and the viz legacy tool-type
- `crates/rs_cam_core/src/compute/operation_configs.rs:396` /// in narrow strips + contour-parallel residue sweep). Legacy
- `crates/rs_cam_core/src/compute/operation_configs.rs:397` /// behaviour can be restored by setting this to `Legacy`.
- `crates/rs_cam_core/src/compute/operation_configs.rs:556` /// preserves the legacy formula-based stepover and the F-037 smoke
- `crates/rs_cam_core/src/compute/operation_configs.rs:867` /// [`crate::compute::config::DeprecatedDialFinding`] →
- `crates/rs_cam_core/src/compute/operation_configs.rs:868` /// `diagnostics::ids::CONFIG_DEPRECATED_DIAL`, so the operator is told
- `crates/rs_cam_core/src/compute/operation_configs.rs:873` /// rest reference (all three detectors). `None` = legacy nominal-diameter
- `crates/rs_cam_core/src/compute/operation_configs.rs:965` /// §M7–M8). Default OFF: an absent key loads the legacy cascade
- `crates/rs_cam_core/src/compute/operation_configs.rs:1064` /// carved). `false` reproduces the pre-v3 op exactly — no detector
- `crates/rs_cam_core/src/compute/operation_configs.rs:1362` /// absent key now loads `true`: a legacy project file gets the cell
- `crates/rs_cam_core/src/compute/operation_configs.rs:1364` /// per-operation opt-out, and the X5 back-compat test pins both directions.
- `crates/rs_cam_core/src/compute/operation_configs.rs:2500` /// The A/M6 deserialization-compatibility matrix, all three cells
- `crates/rs_cam_core/src/compute/operation_configs.rs:2511` /// the letter, and the wire names are the pre-A/M6 ones so no migration
- `crates/rs_cam_core/src/compute/operation_configs.rs:2516` fn unified_finish_claims_reference_serde_round_trip_and_backcompat() {
- `crates/rs_cam_core/src/compute/operation_configs.rs:2569` // Cell 3: legacy payload predating `claims_reference` and its S1/S2
- `crates/rs_cam_core/src/compute/operation_configs.rs:2572` let legacy = r#"{
- `crates/rs_cam_core/src/compute/operation_configs.rs:2585` let cfg: UnifiedFinishConfig = serde_json::from_str(legacy).unwrap();
- `crates/rs_cam_core/src/compute/operation_configs.rs:2588` // legacy project only sees the new default act if it also turned
- `crates/rs_cam_core/src/compute/operation_configs.rs:2591` // S4 (`territory_clip`) postdates this legacy payload too — must
- `crates/rs_cam_core/src/compute/operation_configs.rs:2592` // default off, same backcompat contract as its S1/S2 siblings.
- `crates/rs_cam_core/src/compute/operation_configs.rs:2597` // ruling"); the default is now ON. An absent key loads ON: a legacy
- `crates/rs_cam_core/src/compute/operation_configs.rs:2603` let pinned = legacy.replace(
- `crates/rs_cam_core/src/compute/operation_configs.rs:2681` /// Drill selection fields must round-trip, and legacy TOML/JSON that
- `crates/rs_cam_core/src/compute/operation_configs.rs:2682` /// predates them must deserialize to the legacy "all centroids" behaviour
- `crates/rs_cam_core/src/compute/operation_configs.rs:2685` fn drill_selection_serde_round_trip_and_backcompat() {
- `crates/rs_cam_core/src/compute/operation_configs.rs:2686` // Legacy payload with neither selection key present.
- `crates/rs_cam_core/src/compute/operation_configs.rs:2687` let legacy = r#"{
- `crates/rs_cam_core/src/compute/operation_configs.rs:2696` let cfg: DrillConfig = serde_json::from_str(legacy).unwrap();
- `crates/rs_cam_core/src/compute/operation_configs.rs:2697` assert_eq!(cfg.selected_holes, None, "legacy => all targets (None)");
- `crates/rs_cam_core/src/compute/config.rs:153` /// The old spelling survives ONLY as a legacy JSON key emitted beside
- `crates/rs_cam_core/src/compute/config.rs:280` /// a measurement, it is a compatibility notice: the field is still
- `crates/rs_cam_core/src/compute/config.rs:289` pub deprecated_dial: Option<Box<DeprecatedDialFinding>>,
- `crates/rs_cam_core/src/compute/config.rs:441` /// `None` = **nothing inert is set**. Like [`Self::deprecated_dial`] this
- `crates/rs_cam_core/src/compute/config.rs:1093` pub struct DeprecatedDialFinding {
- `crates/rs_cam_core/src/compute/config.rs:1112` /// value AND the retired one is what makes the migration checkable on a real
- `crates/rs_cam_core/src/compute/config.rs:1612` /// `post.safe_z` is a legacy 2D-friendly default (often 10mm above work Z=0),
- `crates/rs_cam_core/src/compute/config.rs:1761` /// keep working for callers that haven't migrated; combining with the
- `crates/rs_cam_core/src/compute/config.rs:2201` /// disabling any dressup that's geometrically incompatible with how the
- `crates/rs_cam_core/src/compute/config.rs:2202` /// operation emits toolpaths. Intended as a one-shot migration on
- `crates/rs_cam_core/src/compute/config.rs:2290` // Backwards-compat for callers that build `HeightContext::simple` /
- `crates/rs_cam_core/src/compute/config.rs:2400` fn backward_compat_old_json() {
- `crates/rs_cam_core/src/compute/execute.rs:106` /// See [`crate::compute::config::DeprecatedDialFinding`].
- `crates/rs_cam_core/src/compute/execute.rs:107` pub deprecated_dial: Option<crate::compute::config::DeprecatedDialFinding>,
- `crates/rs_cam_core/src/compute/execute.rs:275` fn record_deprecated_dial(
- `crates/rs_cam_core/src/compute/execute.rs:277` finding: crate::compute::config::DeprecatedDialFinding,
- `crates/rs_cam_core/src/compute/execute.rs:282` cell.borrow_mut().deprecated_dial = Some(finding);
- `crates/rs_cam_core/src/compute/execute.rs:287` /// Like [`record_tip_float`] and unlike [`record_deprecated_dial`], this
- `crates/rs_cam_core/src/compute/execute.rs:314` /// A no-op at the defaults, on the same rule [`record_deprecated_dial`]
- `crates/rs_cam_core/src/compute/execute.rs:537` /// Unlike [`record_deprecated_dial`] this is NOT suppressed at the
- `crates/rs_cam_core/src/compute/execute.rs:725` /// migrated family shares ONE signature; in particular `cancel` is
- `crates/rs_cam_core/src/compute/execute.rs:821` /// keeps the legacy distance-only hookup decision — production
- `crates/rs_cam_core/src/compute/execute.rs:837` /// per family as the Phase-5 cutover proves each one; unmigrated
- `crates/rs_cam_core/src/compute/execute.rs:1551` /// Migrated onto the shared `toolpath_at_levels_with_cancel` choke point
- `crates/rs_cam_core/src/compute/execute.rs:2153` /// stage is then byte-identical to the legacy surface-riding link, which is
- `crates/rs_cam_core/src/compute/execute.rs:2391` record_deprecated_dial(
- `crates/rs_cam_core/src/compute/execute.rs:2393` crate::compute::config::DeprecatedDialFinding {
- `crates/rs_cam_core/src/compute/execute.rs:2498` // legacy relink at the same `intra_pass_hookup_mm`, byte for byte.
- `crates/rs_cam_core/src/compute/execute.rs:3390` /// here and discards the spans for backwards compatibility.
- `crates/rs_cam_core/src/compute/execute.rs:3485` /// against. `None` is a byte-identical no-op (legacy distance-only
- `crates/rs_cam_core/src/compute/execute.rs:3511` // legacy distance-only hookup decision.
- `crates/rs_cam_core/src/compute/execute.rs:3527` // exhaustive match below remains the fallback for unmigrated
- `crates/rs_cam_core/src/compute/execute.rs:3529` // compile until it has an arm — migrated arms delegate to the SAME
- `crates/rs_cam_core/src/compute/execute.rs:3569` // Migrated to the registry GenerateFn (T11); arm kept for the
- `crates/rs_cam_core/src/compute/execute.rs:3572` // Migrated to the registry GenerateFn (T11); arm kept for the
- `crates/rs_cam_core/src/compute/execute.rs:3575` // Migrated to the registry GenerateFn (T11); arm kept for the
- `crates/rs_cam_core/src/compute/execute.rs:3578` // Migrated to the registry GenerateFn (T11); arm kept for the
- `crates/rs_cam_core/src/compute/execute.rs:3581` // Migrated to the registry GenerateFn (T11); arms kept for the
- `crates/rs_cam_core/src/compute/execute.rs:3585` // Migrated to the registry GenerateFn (T11); arm kept for the
- `crates/rs_cam_core/src/compute/execute.rs:3588` // Migrated to the registry GenerateFn (T11); arms kept for the
- `crates/rs_cam_core/src/compute/execute.rs:3592` // Migrated to the registry GenerateFn (T11); arm kept for the
- `crates/rs_cam_core/src/compute/execute.rs:3595` // Migrated to the registry GenerateFn (T11); arm kept for the
- `crates/rs_cam_core/src/compute/execute.rs:3598` // Migrated to the registry GenerateFn (T11); arm kept for the
- `crates/rs_cam_core/src/compute/execute.rs:3603` // Migrated to the registry GenerateFn (T11); arm kept for the
- `crates/rs_cam_core/src/compute/execute.rs:3606` // Migrated to the registry GenerateFn (T11); arm kept for the
- `crates/rs_cam_core/src/compute/execute.rs:3609` // Migrated to the registry GenerateFn (T11); arm kept for the
- `crates/rs_cam_core/src/compute/execute.rs:3612` // Migrated to the registry GenerateFn (T11); arms kept for the
- `crates/rs_cam_core/src/compute/execute.rs:3619` // Migrated to the registry GenerateFn (T11); arms kept for the
- `crates/rs_cam_core/src/compute/execute.rs:3624` // Migrated to the registry GenerateFn (T11); arm kept for the
- `crates/rs_cam_core/src/compute/execute/project_curve_chaining.rs:290` let legacy: ProjectCurveConfig = serde_json::from_str(
- `crates/rs_cam_core/src/compute/execute/project_curve_chaining.rs:295` legacy.chain_distance_mm, 0.0,
- `crates/rs_cam_core/src/gcode/mod.rs:108` /// Re-export of `emitter::emit_program` for backward-compatible call
- `crates/rs_cam_core/src/gcode/mod.rs:395` /// STALE. The arm used to return `true` for backward compatibility
- `crates/rs_cam_core/src/gcode/mod.rs:433` // missing entry as a config match for backward-compat.
- `crates/rs_cam_core/src/gcode/mod.rs:1165` fn checked_phased_export_is_byte_identical_to_legacy_emitter() {
- `crates/rs_cam_core/src/gcode/mod.rs:1205` let legacy = emit_gcode_phased(&phases, post::grbl());
- `crates/rs_cam_core/src/gcode/mod.rs:1214` assert_eq!(checked, legacy);
- `crates/rs_cam_core/src/gcode/mod.rs:1218` fn checked_multi_setup_export_is_byte_identical_to_legacy_emitter() {
- `crates/rs_cam_core/src/gcode/mod.rs:1266` let legacy = emit_gcode_multi_setup(&setups, post::grbl(), 15.0);
- `crates/rs_cam_core/src/gcode/mod.rs:1276` assert_eq!(checked, legacy);
- `crates/rs_cam_core/src/gcode/modal.rs:3` //! Captures the controller-state book-keeping the legacy emitter did
- `crates/rs_cam_core/src/gcode/post.rs:122` /// G2/G3 word. Some legacy controllers reject sub-mm arcs outright;
- `crates/rs_cam_core/src/gcode/post.rs:230` /// Backward-compat default for post TOMLs lacking a `tool_change`
- `crates/rs_cam_core/src/gcode/post.rs:577` // Backward compat: a post TOML without `tool_change` must parse
- `crates/rs_cam_core/src/gcode/post.rs:578` // and fall back to the legacy M5 + M6 pair.
- `crates/rs_cam_core/src/gcode/post.rs:580` name = "Legacy"
- `crates/rs_cam_core/src/gcode/emitter.rs:12` //! Byte-parity vs the legacy `PostProcessor` trait is enforced by the
- `crates/rs_cam_core/src/gcode/emitter.rs:333` /// skipped and the legacy threshold-only behaviour applies.
- `crates/rs_cam_core/src/gcode/program_builder.rs:3` //! Three entry points mirror the legacy emitter modes:
- `crates/rs_cam_core/src/gcode/program_builder.rs:11` //! `emit_program`, yield byte-identical output to the legacy direct
- `crates/rs_cam_core/src/gcode/program_builder.rs:284` // Tool change (no idx>0 guard in multi-setup; the legacy
- `crates/rs_cam_viz/src/host.rs:45` //! subject of later steps in that design's migration order.
- `crates/rs_cam_viz/src/error.rs:30` /// Escape hatch for incremental migration.
- `crates/rs_cam_viz/src/mcp_server.rs:933` description = "Add a toolpath the way the GUI's Add menu does, by dispatching the same `AppEvent::AddToolpath` the menu item emits. Use this
- `crates/rs_cam_viz/src/app.rs:697` // is deprecated). We draw everything via panels nested in this root
- `crates/rs_cam_viz/src/lib.rs:77` // lets a later step in the migration drain MCP work from `about_to_wait`
- `crates/rs_cam_viz/src/render/toolpath_render.rs:9` // Re-export palette from centralized colors module for backward compatibility.
- `crates/rs_cam_viz/src/state/viewport.rs:201` /// rename carries no wire compatibility.
- `crates/rs_cam_viz/src/state/job.rs:41` /// `KeepOutZone`. Only the legacy project reader built them, and C01 and
- `crates/rs_cam_viz/src/ui/sim_op_list.rs:376` "Legacy semantic trace — spans invalidated"
- `crates/rs_cam_viz/src/ui/sim_op_list.rs:439` "Spans were invalidated; expand the legacy semantic trace fallback"
- `crates/rs_cam_viz/src/ui/theme.rs:10` //! migrate them, and it shrinks as they do.
- `crates/rs_cam_viz/src/ui/menu_bar.rs:26` // straight to the legacy direct-export pre-flight + file dialog.
- `crates/rs_cam_viz/src/ui/tokens.rs:295` /// that was WRONG, measured during the UP5–UP7 migration: **the strip has six
- `crates/rs_cam_viz/src/ui/tokens.rs:757` // Two agents migrating this crate independently reached the same wall, and
- `crates/rs_cam_viz/src/ui/components/pill.rs:90` /// Without one the pill keeps its legacy caller-supplied colour. UP3
- `crates/rs_cam_viz/src/ui/components/pill.rs:91` /// migrates the call sites; UP2 must not touch a production panel.
- `crates/rs_cam_viz/src/ui/components/pill.rs:171` // Legacy path, until UP3 migrates the call sites.
- `crates/rs_cam_viz/src/ui/properties/mod.rs:3688` /// compatibility — G-BOUNDARYINHERIT) and `planner_origin` (the
- `crates/rs_cam_viz/src/ui/properties/mod.rs:5546` // Some dressups are geometrically incompatible with specific operations
- `crates/rs_cam_viz/src/ui/properties/mod.rs:5554` let op_incompatible_msg: Option<&str> = dressup_policy.strip_all_reason;
- `crates/rs_cam_viz/src/ui/properties/mod.rs:5561` // keep using op_incompatible_msg).
- `crates/rs_cam_viz/src/ui/properties/mod.rs:5562` let entry_disabled_msg: Option<&str> = op_incompatible_msg.or_else(|| {
- `crates/rs_cam_viz/src/ui/properties/mod.rs:5645` ui.add_enabled_ui(op_incompatible_msg.is_none(), |ui| {
- `crates/rs_cam_viz/src/ui/properties/mod.rs:5649` if let Some(msg) = op_incompatible_msg {
- `crates/rs_cam_viz/src/ui/properties/mod.rs:5653` if cfg.lead_in_out && op_incompatible_msg.is_none() {
- `crates/rs_cam_viz/src/ui/properties/mod.rs:5681` ui.add_enabled_ui(op_incompatible_msg.is_none(), |ui| {
- `crates/rs_cam_viz/src/ui/properties/mod.rs:5685` if let Some(msg) = op_incompatible_msg {
- `crates/rs_cam_viz/src/ui/properties/mod.rs:5689` if cfg.link_moves && op_incompatible_msg.is_none() {
- `crates/rs_cam_viz/src/ui/properties/stock.rs:227` // action; correcting a stale one on load is a migration and
- `crates/rs_cam_viz/src/ui/properties/stock.rs:515` // No flipped setup, or a legacy Vertical axis no `FaceUp`
- `crates/rs_cam_viz/src/ui/properties/operations/mod.rs:207` /// back to Auto, and a legacy project can carry a pin that puts the resolved
- `crates/rs_cam_viz/src/ui/properties/operations/boundary_2d.rs:327` CleanupStrategy::Legacy,
- `crates/rs_cam_viz/src/ui/properties/operations/boundary_2d.rs:328` "Legacy",
- `crates/rs_cam_viz/src/controller/events/mod.rs:312` // Empty selection reverts to the legacy default (None) so a
- `crates/rs_cam_viz/src/app/mcp.rs:4572` // authoritative. The legacy keys keep their exact values for wire
- `crates/rs_cam_viz/src/app/mcp.rs:4573` // compatibility; the disambiguating names sit beside them and say
- `crates/rs_cam_viz/src/app/mcp.rs:5062` /// keeps backward compatibility with the existing string vocabulary.
- `crates/rs_cam_cli/src/project.rs:967` {} issue runs (COALESCED, the legacy \"issue_count\") | {} hotspots",
- `crates/rs_cam_cli/src/project.rs:1017` /// This record's key names are a compatibility surface — the doc comment
- `crates/rs_cam_cli/src/job.rs:266` /// Entry style for 3D ops. Accepts both `entry` and legacy `entry_style`.
- `crates/rs_cam_cli/src/job.rs:861` // entry_3d (with legacy alias) falls back to the shared 2D
