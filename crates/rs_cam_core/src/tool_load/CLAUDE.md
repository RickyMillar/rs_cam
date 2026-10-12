# `tool_load/` — the cutting-load guardrails

Independent gates over a simulated cut. Entry: `tool_load::evaluate_toolpath`.

## Files

- `mod.rs` — the monitor and the report entry point. `chipload.rs`, `power.rs`,
  `deflection.rs`, `depth.rs`, `plunge_stress.rs` — the milling gates;
  `drill_gates.rs` — chip welding, peck, plunge feed.
- `locality.rs`, `boundary.rs`, `verdict.rs`, `display.rs`, `distribution.rs`,
  `metric_guide.rs` — steady-state predicate, boundary contract, verdicts,
  display, histograms (call the gate filters; never copy them), hover copy.
- `optimize/` — the post-sim tuner (2026-10-12): `search.rs` (the levers: links,
  then depth x width cells with the calculator's speeds), `full_cut.rs` (volume,
  collisions, over-cut against the baseline), `candidate.rs`, `outcome.rs`,
  `narrative.rs`, `progress.rs`, `context.rs`.

## Invariants

- Every gate filters `in_transit_span` samples through the one predicate
  `locality::is_steady_state_for_gate`. Never write a second copy.
- The chipload gate quantity is advance per tooth, `effective_feed / (rpm *
  flutes)`. It is not the dexel chip thickness.
- A gate with no population proved nothing; check the count. S6: a sample's
  material is `effective_material_for_sample`; split populations ONLY in `evaluate_toolpath`.
- B6: power and deflection read `Material::force_line()`. Power is `line ×
  grain_factor` (1.0 on every line); no line is `MaterialUnvalidated`. A
  force-line or grain-factor change needs the slow `--test` sims, not `--lib`.
- The drill gates model the R-plane-rooted emitted schedule and read cutting
  geometry, not fed distance; a tapered drill uses the envelope diameter.
- The optimizer owns no formula. Suggest owns the pre-sim recipe; the tuner
  reads the simulation at the GUI operating point, takes a cell's speeds from
  `feeds::calculate`, its depth bound from `cutter_axial_constraints`, judges
  with `gcode::refusing_exceedances` (the depth rule-of-thumb row reports, never
  vetoes) and the full-cut check. One measure for baseline and candidates.

## Sentries
Run one with `cargo test -p rs_cam_core -q --test <name>`: `gate_population_vacuity_xvac`, `chipload_boundary_g_chip_ulp`, `predicted_feed_gates_f035`, `drill_evidence_wording_d3`, `an_absent_limit_is_visibly_absent_g_gantry`, `a_criterion_carries_its_own_bound_g_s4bound`, `a_weak_bound_cannot_refuse_an_export_g_s4weak` (only a sourced bound may refuse an export), `the_depth_that_cut_is_a_measured_load_g_s3depth` (depth exceeds, never refuses), `finish_depth_is_reported_not_capped_fm6`, `histogram_population_is_the_gate_population_g_cuthist`.
Optimizer: `optimizer_judges_by_sourced_bounds_g_opt1`, `a_feed_follows_the_depth_through_the_calculator_g_opt5`, `one_measure_for_baseline_and_candidate_g_opt6`, `a_gouge_or_a_thin_cut_is_never_ranked_g_opt7`, `optimize_smoke`, `optimize_toolpath_is_a_job_wp14b`, `optimize_reports_progress_wp29`; `optimize_z22_headline` (`#[ignore]`, 96 s) re-measures the Z22 channel.
