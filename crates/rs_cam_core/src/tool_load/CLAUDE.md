# `tool_load/` — the cutting-load guardrails

Independent gates over a simulated cut. Entry: `tool_load::evaluate_toolpath`.

## Files

- `mod.rs` — the monitor and the report entry point. `chipload.rs`, `power.rs`,
  `deflection.rs`, `depth.rs`, `plunge_stress.rs` — the milling gates; `cut_force.rs` — the
  report-only force reading (P17); `drill_gates.rs` — chip welding, peck, plunge feed.
  `load_fraction.rs` — the one load `L`.
- `locality.rs`, `boundary.rs`, `verdict.rs`, `display.rs`, `distribution.rs`,
  `metric_guide.rs` — steady-state predicate, boundary contract, verdicts,
  display, histograms (call the gate filters; never copy them), hover copy.

## Invariants

- Every gate filters `in_transit_span` samples through the one predicate `locality::is_steady_state_for_gate`. Never write a second copy.
- P11b2: the chipload gate quantity is the PEAK chip, `display::peak_chip_per_tooth` (advance / `RadialThinning::lift` at the measured arc, `Engagement::from_sample`; role `ctx.pass_role`, Finish lift 1; a drill is `NotApplicableForOp` first), on both sides, against the printed band at every depth (no de-rate). A thinning sample with no engagement counts in `missing_arc_count`. The histogram reads it; `commanded_above_band` reads `CommandedStage::peak_chip_mm` (median lift). The heat map (`advance_per_tooth_per_move`) still paints the advance: open gap.
- The cutting load `L` of a pass (chip area over a full pass, per strip) is
  `load_fraction::chip_load_fraction` only (INTEGRATION_PLAN D5, Q1); it reads
  `mrr_cross_section_mm2`, as power does. Never write a second form.
- P29-1: `sample_power_kw`, `sample_tip_deflection_mm`, `cut_force::sample_peak_force_n` and `display::peak_chip_per_tooth` call `feeds::cut_load::CutLoad`; never call a model function here. K7 (P9): the power gate reads the one engagement, `feeds::engagement::Engagement::from_sample`: the width is the geometric inverse `ae = r (1 - cos psi)` of the measured arc (lead decision D-W), the section is the cutter's own; the arc-equivalent slab `(psi / pi) x 2 r` is retired. P22-0c-6b: a tapered ball's width stops at the body's cutting cap (`EngagedDiameter::of_tool`), the cap Suggest reads. The simulation placement on the envelope (`compute::simulate::envelope_placement`) reads the same population (`is_steady_state_for_gate`) and `sample_power_kw`; P20i: it judges power by `power::gate_sample_power` (the gate's feed, width and rated power), never by the drawn Power edge.
- A gate with no population proved nothing; check the count. S6: a sample's
  material is `effective_material_for_sample`; split populations ONLY in `evaluate_toolpath`.
- B6: power and deflection read `Material::force_line()`. Power is `line ×
  grain_factor` (1.0 on every line); no line is `MaterialUnvalidated`. A
  force-line or grain-factor change needs the slow `--test` sims, not `--lib`.
- F197: a stale trace rewrites a verdict through `ToolpathLoadVerdict::mark_simulation_stale` only; it
  destructures every field. F070: build a gate context with `ProjectSession::load_context_parts(..).ctx()`.
- P11a (L5): the deflection gate reads the PEAK engaged-edge force of the sample (`deflection::sample_engagement`: `Engagement::from_tool_cut` at `axial_engagement_mm` and the arc's width `r (1 - cos psi)`), through `feeds::predict::tip_deflection_from_engagement`; the power gate's mean is the same sum's mean. P17: `cut_force::sample_peak_force_n` is that force; the force card, `ToolpathLoadVerdict::cut_force` (report-only, not a criterion) and the deflection gate read one engagement and one law. The budget line exists only where the op has a `DepthCapRule` (`cut_force::budget_line`, lead ruling 3).
- P17: the deflection `Exceeds` limit is `deflection::exceeds_limit`: the radial stock to leave of a rough pass (`ToolpathLoadContext::radial_stock_to_leave_mm`, `BoundSource::StockToLeave`), else the finish target `EXCEEDS_BOUND_MM`; `DeflectionBounds::basis` names the case. Suggest, the envelope and F082 keep the finish target.
- F082: the deflection-cap compliance is `deflection::tip_compliance_mm_per_n` (Suggest's ceiling and the modulator read it). F080: `UnmodeledReason::operator_text` is the reason's one clause.
- The drill gates model the R-plane-rooted emitted schedule and read cutting
  geometry, not fed distance; a tapered drill uses the envelope diameter.

## Sentries
Run one with `cargo test -p rs_cam_core -q --test <name>`: `gate_population_vacuity_xvac`, `chipload_boundary_g_chip_ulp`, `predicted_feed_gates_f035`, `drill_evidence_wording_d3`, `an_absent_limit_is_visibly_absent_g_gantry`, `a_criterion_carries_its_own_bound_g_s4bound`, `a_weak_bound_cannot_refuse_an_export_g_s4weak` (only a sourced bound may refuse an export), `the_depth_that_cut_is_a_measured_load_g_s3depth` (depth exceeds, never refuses; P13c: the cap is `feeds::force_budget::rigidity_depth_cap` at the peak sample's arc width and chip, `BoundSource::MachineForceBudget`, non-gating; P29-4: the Adaptive family too), `finish_depth_is_reported_not_capped_fm6`, `histogram_population_is_the_gate_population_g_cuthist`, `suggests_own_recipe_is_not_above_the_band_p11b2`, `the_gate_reads_the_printed_band_at_every_depth_p11b2`, `the_cut_force_is_the_deflection_force_p17`, `a_rough_pass_deflects_up_to_its_stock_to_leave_p17`, `one_steady_feed_for_the_cap_the_gates_and_the_card_p20e` (the steady-state filter reads `ctx.steady_feed`, not the op feed), `the_envelope_finding_reads_the_gate_feed_p20i`; `--lib tool_load::load_fraction` (the full-pass identity `w/D`).
