# Handoff prompt — P2: selective finishing (rest regions → derived boundaries)

Paste everything below into a fresh session.

---

Continue the rs_cam finishing work. Last session closed the cleanup campaign (commit
`4b105da`, 2026-07-06): 13 bugs fixed, duplication eliminated into shared modules
(`point_runs`, `grid2`, `grid_field`, `marching_squares`, `finish_setup`,
`pencil_dihedral`), cancellation for 19/23 op families, full /verify green.
Ledger: `planning/finishing_stack_review_2026-07.md` (§P2 is this phase's spec;
§R1.5/R1.6 are its prerequisites). Known reds (pre-existing, don't chase):
adaptive3d peck/rapid ×2, planner_sim_dexel_parity, wanaka_suggest_baseline
(needs re-baseline to unified-load-model behavior — do as a small first commit).

## Goal

**Coarse-tool rough-finish everything fast → rest analysis finds where detail
remains → fine tool (scallop/pencil) runs ONLY inside those regions.**
All building blocks exist; this phase wires them end-to-end:

1. **Prereq (R1.5/R1.6, tracker §R1)**: harden Polygon2 for computed input —
   self-intersection/pinch guard before offset (today a catch_unwind swallows
   cavalier panics), boolean ops (union/difference via geo::BooleanOps) to
   merge/subtract region polygons, boundary epsilon for point-in-polygon.
2. **P2.1**: rest mask components → `marching_squares_bool_grid` → dilate by
   fine-tool radius + margin (use the EDT in `grid_field`, not O(cells×radius)
   loops) → `Vec<Polygon2>` stored beside `rest_grid` on the generated toolpath.
   `ClearingRegion` (bbox-only, consumed by nothing) gets replaced/augmented.
3. **P2.2**: new `BoundarySource::DerivedRestRegions { source_toolpath_id }`
   (compute/config.rs), resolved in `apply_boundary_clip` (session/compute.rs),
   with FromRemainingStock-style fail-hard staleness preconditions.
4. **P2.3**: pre-clip finish generation to the boundary (post-clip works today but
   generates the whole part then discards — at 0.5mm stepover that's minutes).
   With `point_runs` in place this is a point-in-polygon predicate on the same
   run-splitter for drop-cutter ops; waterline-family needs contour clipping.
5. **Task #3 (GUI)**: render the rest heatmap overlay in the wgpu viewport —
   `render_annotated.rest_grid` is plumbed to gpu_upload.rs already; build a
   colored mesh via `SimMeshGpuData::from_heightmap_mesh_colored`, draw via the
   alpha `sim_mesh_pipeline`, toggle + legend UI. The display-frame bug is
   already fixed (`AnnotatedToolpath::translated()` owns the invariant). The
   overlay and the derived boundaries must share ONE source of truth.
6. **Validate live on wanaka200** (LIVE-ONLY project — NEVER save from GUI/MCP):
   Ø6 aggressive coarse → rest analysis → region-clipped Ø1–Ø2 pencil+scallop;
   compare wall-clock + coverage vs the all-over scallop baseline via
   run_simulation + narrate_toolpath.

## Working rules (standing, from memory — do not relearn the hard way)

- Implementation via **Sonnet 5 subagents** with tight file-scoped prompts; the
  main session orchestrates only. Parallel editor agents get NO cargo; one
  verifier agent at a time runs builds/tests.
- **Cargo discipline**: heavy jobs (test/clippy/build) exclusive — check
  `free -g` + `pgrep -f "bin/[c]argo"` first, one at a time, never
  workspace-wide `cargo test`. `cargo check` may run concurrently when >20 GB
  free. Concurrent heavy cargo has crashed this PC three times.
- **Targeted testing** (`--lib <module>` filters) between waves; full gate +
  slow sentries only at phase end. Zero-warning clippy (16 deny lints).
  Never run bare rustfmt mid-wave (cascades); `cargo fmt` only pre-commit.
- Commit only when the user asks. Agents must never git
  stash/checkout/restore/commit.
- Update `planning/finishing_stack_review_2026-07.md` checkboxes + changelog
  as items land; keep memory current.
