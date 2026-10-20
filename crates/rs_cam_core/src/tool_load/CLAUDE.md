# `tool_load/` — the cutting-load guardrails

Independent gates over a simulated cut. Entry: `tool_load::evaluate_toolpath`.

## Files

- `mod.rs` — the monitor and the report entry point. `chipload.rs`, `power.rs`,
  `deflection.rs`, `depth.rs`, `plunge_stress.rs` — the milling gates;
  `drill_gates.rs` — chip welding, peck, plunge feed.
- `locality.rs`, `boundary.rs`, `verdict.rs`, `display.rs`, `distribution.rs`,
  `metric_guide.rs` — steady-state predicate, boundary contract, verdicts,
  display, histograms (call the gate filters; never copy them), hover copy.

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
- F197: a stale trace rewrites a verdict through `ToolpathLoadVerdict::mark_simulation_stale` only; it
  destructures every field. F070: build a gate context with `ProjectSession::load_context_parts(..).ctx()`.
- F082: the deflection-cap compliance is `deflection::tip_compliance_mm_per_n`;
  Suggest's ceiling and the feed-modulation optimizer both read it. F080:
  `UnmodeledReason::operator_text` is the reason's one clause.
- The drill gates model the R-plane-rooted emitted schedule and read cutting
  geometry, not fed distance; a tapered drill uses the envelope diameter.

## Sentries
Run one with `cargo test -p rs_cam_core -q --test <name>`: `gate_population_vacuity_xvac`, `chipload_boundary_g_chip_ulp`, `predicted_feed_gates_f035`, `drill_evidence_wording_d3`, `an_absent_limit_is_visibly_absent_g_gantry`, `a_criterion_carries_its_own_bound_g_s4bound`, `a_weak_bound_cannot_refuse_an_export_g_s4weak` (only a sourced bound may refuse an export), `the_depth_that_cut_is_a_measured_load_g_s3depth` (depth exceeds, never refuses), `finish_depth_is_reported_not_capped_fm6`, `histogram_population_is_the_gate_population_g_cuthist`.
