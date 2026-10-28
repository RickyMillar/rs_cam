# W3 report: G-WLAIRLABEL (option B, the label only)

Patch: `W3.patch` = `git diff efd34d1b0 --` 6 paths, md5 `2812f57691e1d12007792cdfc791f5c8`. `git apply --check`
passes on a clean `efd34d1b0` checkout. Nothing is committed. Pre-registration: `W3_PREREG.md` (option B approved, the
bar reads the true share).

## What changed

- `stock/simulation_cut.rs`: `SimulationCutSample::removes_material()` = `is_cutting && removed_volume_est_mm3 > 0.0`,
  the one definition (it was inline in `sim_measurability`).
- `stock/sim_measurability.rs`: the removing filter calls the method (same population).
- `stock/simulation_cut/analysis.rs`: `SimulationCutTrace::air_cut_removing_share(Option<ToolpathId>)`: the time of
  the cutting samples in `RadialEngagementClass::Air` that remove material, over the time of all of them, on the
  samples' clock. No struct field is added: a field would touch about 15 struct literals and the G-AIRDENOM rebase in
  `session/compute/simulation.rs`. The share is read from the samples, which every reader of the summary also holds.
- `trace/narrate.rs`: the air line reports the true air (`air × (1 − share)`) and names the skin in the same line; the
  ⚠ marker reads the true share of the total runtime. With share 0 the line is the old line, character for character.
  The WOC in the text comes from `AIR_CUT_RADIAL_WOC_FRACTION`.
- `rs_cam_viz/src/app/mcp/simulation.rs` (the MCP summary, not a viz UI file): each `toolpath_summaries` row gains
  `air_cut_removing_share`.
- Not changed: the partition, `is_air`, the gates, `air_cut_time_s`, the air thresholds.

The narration line (the D69 split):
`ℹ 14.3% of CUTTING time is air-cut (+70.5% skin cut under 2% WOC) (14.3% of TOTAL runtime, the measure the GUI banner
and the per-operation thresholds use); average engagement …`

## For I4 (the GUI change to queue)

- Source: `SimulationCutTrace::air_cut_removing_share(Some(toolpath_id))` in core, or the MCP row field
  `air_cut_removing_share` (0 to 1).
- True air % = the air % × (1 − share); skin % = the air % × share (same denominator as the shown air %).
- One-line GUI form: `Air 14 % (+74 % skin cut under 2 % WOC)` (the skin share printed after the true air, in the same
  chip or line). Show the skin part only when the share is above 0.

## Sentries

- `stock::simulation_cut::tests::air_that_removes_material_is_a_skin_g_wlairlabel` (lib): the D69 split gives share
  279.3 / 335.9; `air × (1 − share)` = 56.6 s (the air that removes nothing); cleared ground gives 0; a rapid with a
  volume does not remove material.
- `trace::narrate::tests::the_air_line_reads_the_true_air_share_g_wlairlabel` (lib): the D69 split (Waterline, 45 %
  bar) gives ℹ 14.3 % with the skin clause, in one line; the same op whose air removes nothing gives ⚠ 84.8 % and the
  old line.
- Red check: with `removes_material` forced false, both tests fail.

## Re-bless

None. No golden holds the air line or the MCP row.

## Gates (worktree, efd34d1b0 + W3; other agents' uncommitted adaptive3d work was in the tree)

- fmt clean; `clippy -p rs_cam_core -p rs_cam_viz --all-targets --features heavy-tests,research,test-support -D
  warnings` clean.
- `cargo test -p rs_cam_core -q --lib`: 2727 passed.
- `air_cut_denominators_lh1`, `gate_population_vacuity_xvac`, `narrate_regions_closed_c8`: green.
- Not run: the viz tests (no viz test reads the MCP air fields: rg over `crates/rs_cam_viz`).
