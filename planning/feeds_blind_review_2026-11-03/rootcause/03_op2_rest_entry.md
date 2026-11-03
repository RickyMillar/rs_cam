# Root cause: op 2 "Back rest 6 mm" — the 14.5 mm entry and the 13 mm links

Date: 2026-11-03. Read-only analysis. No source edit, no cargo run, no simulation run.
Code refs are at HEAD 40cf411a7 (paths under `crates/rs_cam_core/src/`).

Op 2 config (`planning/rivmap_block_job/nz_wanaka_21_operator.toml:719-790`): 3D Rough
(`adaptive3d`), `geodesic_adaptive`, `region_ordering = "global"`, remaining stock, 6 mm flat,
stepover 1.2, depth per pass 1.2, leave 0, `entry_style = "ramp"` 3 deg,
`detect_flat_areas = true`, `min_region_cut_length_mm = 15`, no rest region bound.
Op 1 (the 18 mm bulk) has levels Z 16.5 and Z 11.0 (manual `bottom_z` 11).

Evidence files in `scratch_op2_rest_entry/`:

- `probe_target.py`, `band.py`: sample `back_target_s1.stl` around (211.7, 41.6); compute the
  flat drop-cutter height of the 6 mm and the 18 mm tools; model op 1 as "each 18 mm disc whose
  centre can reach a level cuts its disc to that level".
- `assert_sliver.py`: asserts each geometric claim in section (a). Run:
  `python3 -I assert_sliver.py <back_target_s1.stl> 211.7 41.6 30`. Result: "OK: all claims hold".
- `band_out.txt`: the height maps.
- `op2_run_2026-11-02T1804_regions.txt`: the per-level region lines of the op 2 run in
  `~/.rs_cam_logs/mcp_proxy.log` (lines 30143-30323; "Z levels computed count=23 z_top=24.3
  z_bottom=0.5 depth_per_pass=1.2").

---

## (a) The 14.5 mm full-width entry

### Symptom

`project.entry_load` Critical: an entry removes up to 14.5 mm of wood at (211.7, 41.6, Z 11.0)
(job story, `planning/rivmap_block_job.md:611-620`, `:668-670`). The value stays with
`min_region_cut_length_mm = 0` and with an extra 18 mm level at Z 11 (`:752-753`).

### The geometry (proved by `assert_sliver.py`, idealised op 1)

- The entry is in the inner corner of a pocket. The pocket floor is Z 11.0. The model wall to
  Z 25.5 is at x ≈ 215.5 (right) and at y ≈ 32 (below).
- The 18 mm drop height at the entry is 25.5: the 18 mm centre cannot come within 9 mm of
  either wall. Op 1 leaves the corner fillet (R9 against a square corner): a sliver of 17.8 mm²,
  from Z 11 to Z 25.5. Its height is **14.5 mm = 25.5 − 11**, the reported peak.
- All 284 sliver cells lie within 3 mm of a wall. At each of them the 6 mm drop height is 25.5,
  which is not below the stock top there.

### Code path

1. The planner seed stock is the raw remaining stock of op 1 (`compute/execute/rough_3d.rs:187`,
   `initial_stock: inputs.seed_stock.cloned()`). The seed stock holds the sliver correctly.
2. The per-cell floor is the 6 mm drop-cutter height (`adaptive3d/path.rs:941`,
   `SurfaceHeightmap::from_mesh_with_cancel(.., cutter, ..)`).
3. The level plan: anchor 25.5, levels 24.3 … 0.5 plus the flat shelves
   (`adaptive3d/path.rs:1147-1210`). The log confirms 23 levels from Z 24.3.
4. The work map of each level: `build_material_bool_grid` (`adaptive3d/clearing.rs:433-470`)
   calls `cell_has_material` (`adaptive3d/clearing.rs:276-287`). A cell is work only when the
   stock AT THAT CELL stands above `max(drop height AT THAT CELL + leave, level)`
   (`adaptive3d/intended_floor.rs:35-37`). A sliver cell has drop height 25.5, so it is never
   work, at any level. The slice regions, the F-038 forecast (`clearing.rs:3082`) and the level
   gate (`clearing.rs:298-330`) read the same test. The waterline cleanup (`clearing.rs:1747`)
   is the only planned pass along the wall. It can skip a contour (slope filter `:1823`,
   left-piece filter `:1842`).
5. An entry at the pocket floor near the corner: `plan_entry` (`clearing.rs:814-855`) reads the
   rapid floor as the conservative stock top over ONE disc of R at the entry XY. The emitter
   (`adaptive3d/path.rs:2268-2300`) then calls `dressup::emit_ramp` with the fixed direction
   `(1.0, 0.0)` (`path.rs:2290`). That is +x here, which points into the right wall.
6. `emit_ramp` (`dressup/entry_descent.rs:520-700`): the legs are up to
   `ENTRY_CLEARANCE / tan(3°) / 2` = 19.1 mm long (`:645`). They are clipped to the MODEL floor
   only (`clip_polyline_to_floor`, `:672`). The stock-aware G-ISOCLIPENTRY ladder (`:557-628`)
   runs only when `rest_stock` is set. The adaptive3d door passes `rest_stock: None`
   (`adaptive3d/path.rs:2160`), also on a rest op.
7. The planner stamp mirror stamps an entry as a vertical line at the entry XY only
   (`clearing.rs:730-760`). It does not read or stamp the ramp legs.

### Root cause

The per-level work map compares each stock cell with the 6 mm drop height at the same cell
(`clearing.rs:276-287`). Stock within one tool radius of a model wall is therefore never
planned at any level. A rest op exists to cut exactly that stock (the 18 mm corner fillets),
so the fillet stands the full 14.5 mm until a pass or an entry near the floor meets it. The
entry emitter makes this worse: the ramp legs go in a fixed +x direction for up to 19 mm, are
clipped to the model only, and the planner neither reads nor stamps the stock they cross.

The exact move that takes the 14.5 mm is **needs sim**. The two candidates are:

- the ramp out-leg at the pocket floor, when the rapid-floor disc at the entry reads clear;
- the waterline cleanup at the Z 11.1 level, whose entry and cut run along the wall.

Both follow from the same blind band. The idealised op 1 model also needs sim: the real op 1
path can leave more than the R9 fillet.

### Answers to the questions in the brief

- Which step picks the entry point: the slice plan of the level (`clear_z_level_slice`,
  `clearing.rs:3697`) or the waterline cleanup (`clearing.rs:1889`, `Rapid(first)`).
  `plan_entry` sets its depth from the stock over the entry disc. Which of the two: needs sim.
- Does the remaining-stock heightmap see the leftover: the planner stock does (the seed is the
  simulated stock). The per-level work map does not (step 4).
- Is the entry in a region the 18 mm could not enter: yes. The 18 mm drop height at the entry is
  25.5. The 6 mm then meets a 14.5 mm tall column of uncut wood beside the wall.
- The rest region filter: not active on op 2 (no bound set;
  `compute/operation_configs.rs:1866-1871`). Not a cause.

### Fix options and blast radius

1. **Plan the wall band (preferred).** Add a per-level pass for stock that a disc centred on a
   reachable cell meets, but that no centre cell owns. C4 "Contour each level"
   (`adaptive3d/level_contour.rs`, G-RESTDOC) does this today for the band beyond the
   containment only (cells whose centre is outside `centre_boundary`). It does not cover a
   model wall inside the containment. Extend the band test to cells whose drop height is above
   their stock top. Blast radius: `cell_has_material(` 4 hits and `build_material_bool_grid(` 4
   hits, all in `adaptive3d/clearing.rs`. `level_floor_z(` 10 hits in `adaptive3d/`. The change
   moves every 3D Rough path that has walls: op golden re-bless.
2. **Make the adaptive3d ramp stock-aware on a rest op.** Pass the planner stock as
   `rest_stock` (`path.rs:2160`), or read the ramp legs in `plan_entry` and stamp them in the
   mirror. Also pick the leg direction along the first cut, not the fixed +x. Blast radius:
   `emit_ramp(` 6 hits (`adaptive3d/path.rs`, `dressup/entry_descent.rs`, `dressup/mod.rs`,
   `dressup/tests.rs`); `plan_entry(` 2 hits; `entry_floor_radius(` 5 hits in `adaptive3d/path.rs`.
   Byte parity of the emitter moves.
3. **Job workaround (no code).** Use a helix entry, or add a 6 mm 2.5D pocket or a profile at the
   pocket floors before op 2. Confidence that the workaround removes the peak: needs sim.

Sentries for this area: `adaptive3d_entry_stock_aware`, `entry_moves_stock_aware_g_rampterrain`,
`isoclip_entry_ramp_g_isoclipentry`, `a_rest_entry_does_not_peck_through_cleared_air_d55`,
`a_rest_op_holds_one_depth_at_the_band_c4`, `contour_each_level_is_off_by_default_c4`,
`adaptive3d_planner_never_ahead_of_emitted_path`, `adaptive3d_plan_matches_merged_path_g_plansimgap`,
`adaptive3d_emission_byte_parity`, `every_op_keeps_its_toolpath_g_opgolden` (already red, see (d)).
No sentry has a 3D Rough rest fixture with a pocket corner narrower than the source tool.

### Confidence

- The blind wall band (step 4): high (code read; geometry asserted).
- The sliver at the entry is 14.5 mm tall: medium-high (idealised op 1; matches the peak to 0.05 mm).
- Which move takes it (ramp leg or waterline): low; needs sim.

---

## (b) Links that cross 13 mm of uncut stock

### Symptom

`project.link_cuts_stock`: link samples remove up to 13.05 mm; 2.7 % of link samples are over
the commanded 1.2 mm (`rivmap_block_job.md:617-618`). Position not recorded.

### Code path (every fed `Linking` move of a 3D Rough)

1. Keep-down link of an entry (F-038b): `try_emit_stay_down_link` (`adaptive3d/path.rs:1691-1756`),
   proved by `StayDownProof::ceiling_z` = `keep_down::corridor_ceiling_z` over the swept disc
   (`clearing.rs:828-841`, `keep_down.rs:64-68`). The link Z is above that ceiling (`path.rs:1648-1654`).
2. Held Geodesic link: `clear_one_region` (`clearing.rs:3286-3316`), proved by
   `keep_down::keep_down_excess` (`keep_down.rs:83-95`) up to `z_level + DPP + clearance`.
3. Side link of a cut: `side_link_ok` (`clearing.rs:861-912`): centre line plus a disc of
   `R − cell`.
4. Routed keep-down travel: `routed_entry` (`clearing.rs:1228-1292`), off by default on this op.
5. The emitter feeds each `Link` along `emitted_link_path` (`path.rs:2324-2346`). The drape only
   raises a link.

Every proof reads the planner stock with the conservative disc read
(`dexel_stock/mod.rs:395`). The planner stock holds the wall slivers of (a). So a link in the
planned order that crosses a 14.5 mm sliver at depth must be refused by these proofs.
The TSP reorder is forbidden for 3D Rough (`compute/catalog/capabilities.rs:797-806`), so a
reorder does not break the proofs.

### Candidate root cause (needs sim)

The held-link proof reads a stale corridor. At `clearing.rs:3296` the proof starts at
`sink.cursor.tool_pos`. With the F-038 gate on (`defer_entries` when
`min_region_cut_length_mm > 0`, `clearing.rs:3811`; op 2 has 15), an entry that comes before the
link is still pending. The cursor has not moved to it. `sink.emit(Link)` then commits the
pending entry (`clearing.rs:2305-2307`, `:2365`), and the shipped link runs from the entry to the
end. The proof covered a different corridor (from the old cursor to the end). An all-air 2D cut
also becomes such a pending entry (`clearing.rs:3566-3583`). A second candidate: the planner
stamps the ramp entry as a vertical line only (a, step 7), so the planner and the simulation
disagree after each ramp. This makes the planner stock HIGHER than the real stock, which cannot
by itself make a link plough.

Neither candidate is proved without the link sample positions from a simulation.

### Fix options and blast radius

1. Prove the held link from the position the emitter starts it at: commit the pending entry
   before the proof, or start the proof at the pending entry XY. Blast radius:
   `keep_down_excess(` 5 hits (`adaptive3d/keep_down.rs`, `adaptive3d/clearing.rs`), one call
   site (`clearing.rs:3298`).
2. A debug run of op 2 with the link worst position (the `link_cut_observation` result,
   `stock/sim_triage.rs:1652`) decides between the candidates. Operator approval is needed
   (it needs the op 1 simulation).

Sentries: `adaptive3d_keep_down_link_f038b`, `a_link_through_stock_is_flagged_g_linkplough`,
`adaptive3d_entry_coalescing_f038`, `--lib adaptive3d::keep_down`,
`adaptive3d_planner_never_ahead_of_emitted_path`.

### Confidence

Low. The proofs are correct for the corridor that they read; the stale-cursor case is a code
read only. Needs sim.

---

## (c) The entry-load gate

### Where

- Rule: `entry_load_observation` (`stock/sim_triage.rs:1376-1436`) and `entry_load_finding`
  (`:1444-1514`). Bar: 2 x the pass's own median body bite (`ENTRY_LOAD_MEDIAN_MULTIPLE`,
  `:1253`). Critical at a peak of 1.0 mm or more (`ENTRY_LOAD_SEVERE_PEAK_MM`, `:1266`). Entry
  samples are `EntryPlunge | EntryRamp | EntryHelix` (`:1358-1368`); removal is
  `max(axial, plunge descent)` (`:1348-1350`).
- Wiring: `SimulationTriage::build_with_rest_context` (`:578-582`), only for toolpaths with
  `StockSource::FromRemainingStock` (`session/compute/diagnostics.rs:433-437`).
- The link rule (G-LINKPLOUGH): `link_cut_observation` / `link_cut_finding` (`:1652`, `:1723`).

### Why it only reports

- It is a post-simulation triage row: `TriageClass::Action`, `fix: None` (`:1504`). The comment
  at `:1258-1259` states the design: "it does NOT block export — nothing about this rule is a
  refusal".
- The export refusals are height order, `safety.cuts_into_model`, and G-CENTRECUT only
  (`session/compute/export.rs:84-115`). The generation gate (`session/run_state.rs:225-245`) reads
  `diagnose_toolpath_gate`, which holds no simulation finding.
- The tool-load gates skip entry spans (`tool_load/locality.rs:156`, `is_steady_state_for_gate`), so
  no other gate grades the 14.5 mm entry.

### Feedback to Suggest or generation

None. `rg entry_load` finds no consumer in `feeds/`, `compute/`, `session/` (except a comment in
`session/multitool.rs:1005`), `rs_cam_cli` or `rs_cam_viz`. Suggest is pre-simulation; it does
not write the entry style (`feeds/suggest/adaptive_entry.rs:6-9`). Its only entry check is the
plunge-entry stability warning for `entry_style = Plunge` (`adaptive_entry.rs:110`). The planner
has no loop that re-plans an entry from a simulation finding.

### Fix options and blast radius

1. Make a Critical `project.entry_load` / `project.link_cuts_stock` a refusal (or an explicit
   override, as F051 does for tool load). Blast radius: `entry_load_finding(` 6 hits, all in
   `stock/sim_triage.rs`; the export refusal list in `session/compute/export.rs`.
2. Feed the finding position back to the planner (re-plan the entry). This is a new loop, not a
   small change.

Sentries: `pencil_entry_ramp_g_entryload`, `isoclip_entry_ramp_g_isoclipentry`,
`a_link_through_stock_is_flagged_g_linkplough`, `--lib stock::sim_triage`.

### Confidence

High (code read).

---

## (d) Recent changes

- **c0f5d2567** (G-CENTRECUT part 2 and the 3D Rough rest region filter):
  - The rest region filter (`adaptive3d/rest_region_filter.rs`, applied in
    `build_material_bool_grid`) acts only when `rest_max_region_area_mm2` or
    `rest_max_region_width_mm` is set (`compute/operation_configs.rs:1866-1871`,
    `compute/execute/rough_3d.rs:178`). Op 2 sets neither. It does not touch op 2.
  - G-CENTRECUT: `effective_entry_style` (`adaptive3d/path.rs:2062`) and the entry limits act only
    on a tool with `non_centre_cutting` limits (the 18 mm). The 6 mm states none. It does not
    change the op 2 path. The commit text says it too: "3D Rough cut and link descents … are held
    by the gate only".
  - It does not cause the 14.5 mm entry: the entry was measured before it
    (`rivmap_block_job.md:611-620`, e84dd837e era).
  - Note: a rest filter with a width bound would make the blind band of (a) wider. A wide region
    is removed whole, and stock beside it then also stands to the floor (needs sim).
- **"R1 rest hang" (G-R1HANG; 1dc64150a, 91c6d6dff)**: planning notes only (no source change).
  It is a different op (the R1 tapered-ball 3D Rough, front setup) that planned for 4 h, and a
  3D Rough loop that does not poll cancel. It does not touch op 2. It shares one fact with (a):
  the rest op cannot see the wall-adjacent stock, so the operator widened
  `rest_max_region_width_mm` to 20 to reach it.
- The op golden `every_op_keeps_its_toolpath_g_opgolden` is red since before c0f5d2567 (commit
  text). Do not use it as the guard for a fix here until it is re-blessed.

---

## Summary

| Q | Root cause | Where | Confidence |
|---|---|---|---|
| a | The per-level work map never sees stock within R of a model wall (cell test uses the drop height at the same cell); the 18 mm corner fillet stands 14.5 mm until the floor level | `adaptive3d/clearing.rs:276-287`, `:433-470` | high (code + asserted geometry) |
| a | The adaptive3d ramp is blind to stock: fixed +x legs up to 19 mm, model clip only, `rest_stock: None`, not read or stamped by the planner | `adaptive3d/path.rs:2160`, `:2290`; `dressup/entry_descent.rs:645`, `:672`; `adaptive3d/clearing.rs:730-760`, `:814-855` | medium; the exact move needs sim |
| b | Candidate: held-link proof from a stale cursor while an entry is pending | `adaptive3d/clearing.rs:3296`, `:3811`, `:2305` | low; needs sim |
| c | The gate is a report by design; no refusal, no feedback | `stock/sim_triage.rs:1258`, `:1504`; `session/compute/export.rs:84-115` | high |
| d | c0f5d2567 and the R1 notes do not touch op 2 | — | high |
