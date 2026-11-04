# rs_cam_core contracts

The feeds contract (`planning/feeds_contract_2026-11-03/PLAN.md`) names the
rules that the compiler holds. One line per contract: the rule, the type or
setting, the file and the sentry. Each folder `CLAUDE.md` that a contract
touches names it in its invariants.

- Lint: no wildcard arm over two or more variants; `Cargo.toml`
  `wildcard_enum_match_arm` (deny). An exception is
  `#[expect(clippy::wildcard_enum_match_arm, reason = "..")]` on the `let`
  statement or the smallest `fn`. Sentry: the full clippy gate.
- K1: every op states its cut. `OperationConfig::cut_spec` builds a
  `CutSpec` (radial, axial, total depth, first pass) from the op's own
  config; the match names every variant. Each `REG_*` row states its
  `CutRule` (`Capabilities::cut_rule`); no type in
  `compute/catalog/cut_spec.rs` has `Default`, and only `compute/` can make
  a `Stated` value. Since P5 the calculator reads the operator's cut
  (`CutSpec::calc_inputs`: the stated depth, the stated width, the tool
  diameter for a slot, the scallop target) through `operation_feeds_hints`;
  `PresentBasis` is gone. Sentries: `--test every_op_states_its_cut_k1`,
  `--doc cut_spec_refusals`.
- K2: every shipped number names its source. `feeds/sourced.rs`:
  `Sourced<T>` (no public constructor; the owner of a rule builds it),
  `Basis` (`Operator`, `Printed`, `Claim`, `Rule`, `Machine`, `Derived`),
  the unit newtypes, the one cap API `lower_to_cap` (`min` only).
  `FeedsResult::bases` holds one basis per shipped field; `FeedsProvenance`
  is its projection (`ValueProvenance::of_basis`, one exhaustive `match`).
  `apply` takes only `Recommendation<Complete>`; every apply door calls
  `Recommendation::accept_unsourced` (lead decision D4) and files
  `SuggestWarning::UnsourcedRowsAccepted`. Q3: a score tie takes the lower
  printed chip; `LookupResult::tied_with` names the other row. Sentries:
  `--test suggest_never_raises_a_stated_depth_k2`,
  `--test a_tie_takes_the_lower_chip_and_names_both_q3`,
  `--doc sourced_refusals`.
- K2b: Suggest returns a space of safe choices. `feeds/envelope.rs`:
  `SafeEnvelope::for_cut` (no other constructor) gives the axes from the
  `CutSpec`, one `Limit { kind, law, basis, edge }` per limit, the
  `SpeedsMap` grid (`ENVELOPE_GRID_N`, a lookup `speeds_at`), the proposal
  (`min(stated, binding cap)`) and the operator's `Placement`. A geometry
  write needs an `EnvelopePick`, made only by `SafeEnvelope::pick`;
  `ApplyFeedsArgs { pick }` with `None` writes the speeds for the cut the op
  keeps. A point outside is K6 `project.outside_envelope` (a warning).
  Sentries: `--test apply_writes_speeds_unless_a_pick_k2b`,
  `--test the_envelope_keeps_each_limit_k2b`,
  `--test a_pick_outside_the_envelope_warns_k2b`, `--doc envelope_refusals`.
- K3: every tool fact states how it is known (rulings Q7, Q9). `compute/tool_fact.rs`:
  `ToolFact<T> { Stated, Catalogue, Estimated, Unknown }` (no `Default`, no `Deref`)
  and `CentreCutting { Yes, No(geometry) }`. The `ToolConfig` fact fields
  (`cutting_length`, `taper_half_angle`, `shank_diameter`, `stickout`,
  `flute_count`, `centre_cutting`, `v_tip_diameter`) are private; a reader takes
  `<fact>_planning_..()` (an `Unknown` fact plans at its `LEGACY_DEFAULT_*`, an F201
  Unsourced row) or `<fact>_fact()`. A file states each fact as a table; a bare
  value refuses to load and one message names every bare key (project file, tool
  library, CLI job file); an absent key is `Unknown`. Ruling D12 (P7 A2): centre
  cutting, and a V-bit's tip, are required inputs: an op whose tool does not hold
  them `known()` (stated or catalogue) stops with `NotSetKind::ToolFact`
  (`compute::not_set`), as an unset depth does. Sentries:
  `--test a_tool_fact_states_how_it_is_known_k3`, `--doc tool_fact` (`compile_fail`:
  a read of a fact field, a `Default`, a `Deref`).
- D8 (P10): no geometry default without a tool. The tool-derived fields
  (`stepover`, `depth_per_pass`, `peck_depth`, `offset_stepover`, `z_step`,
  Ramp Finish `max_stepdown`, Unified Finish `raster_stepover`) are
  `Option` (S11 unset; generation stops with `NotSetKind::Geometry`, "set a
  tool"). Each `REG_*` row states `geometry: &[GeometryDefault]` (a required
  field) with one `ToolRule` per field (`compute/geometry_default.rs`):
  `FractionOfD(KRow)`, k an F201 row (a stepover: the calculator's own width on an
  unstated axis, `feeds::default_radial_width`, stamped with the family
  `ae_factor` row; an axial field: `AXIAL_K_*`, today's default over 6 mm, Unsourced), or
  `ScallopChordElse` (a ball tip with the op's set `scallop_height` h:
  `2 sqrt(2 R h - h^2)`). `write_rule_values` is the one writer: the add door
  (`default_operation_for_tool`) and the tool change (`seed_geometry`; a rule
  stamp moves, a `Manual` stamp, a pick and no stamp stay). The stamp is
  `ValueProvenance::of_basis(Basis::Rule(row))`; the lineage shows it.
  Sentries: `--test every_geometry_default_is_a_fraction_of_the_tool_d8`,
  `--test a_tool_change_moves_a_derived_geometry_value_d8`.
- K7: one engagement definition before and after the simulation (P9, ruling
  Q5). `feeds/engagement.rs`: `Engagement { ap, ae, psi, d_eng, section, helix }`
  (private fields, accessors) is built only by `from_cut` (a planned cut: psi from
  `immersion_angle`) and `from_sample` (a simulation sample: the geometric inverse
  `ae = r (1 - cos psi)`, lead decision D-W); the section is the cutter's own
  `mrr_cross_section_mm2`. `PowerModelInputs` has private fields in the same file,
  so only `Engagement::power_inputs` builds it: the calculator Step 6, the
  operating-point door, the post-sim gate `sample_power_kw` and the modulator power
  cap read one power for one cut; the deflection predictor reads
  `Engagement::lateral_force`. Pre-sim power is the maximum over `PowerCases`
  (`Nominal`, `PlannerCap` = 3D Rough `trochoid_cap_mult x ae` at most D, `Slot` =
  `FirstPass::Slot` or `Radial::Slot`), with its `PowerCase` on `FeedsResult` and
  `PowerFigure`; the envelope power edge is drawn at that case. Q1 (P8): at the
  cutting-feed cap Step 7 compares arm L and arm S on the power curve
  (`cap_arm_demand_kw`, `pick_cap_arm`, `CapArms`) and ships the arm inside it
  (sentry `--test the_power_curve_picks_the_arm_at_the_feed_cap_q1`). The simulation is
  placed, not fed back: `compute::simulate::envelope_placement` (the tenth shared
  decision, the gate population) places each sample against the envelope ceilings
  (`SafeEnvelope::placement_of_measured`: no floors; the `Diameter` ceiling is the
  engaged diameter `Engagement::d_eng`; a cone tool is not judged by the slot rule);
  a point outside is `project.sim_outside_envelope` (a warning,
  `Gate::SimOutsideEnvelope`; it supersedes the pre-sim `project.outside_envelope`),
  and the K8 lineage records the measured peak, the case "measured" and the binding
  limit (`ProjectSession::lineage_measured`, `Lineage::measured`). Sentries:
  `--test one_engagement_before_and_after_the_simulation_k7`,
  `--test pre_sim_power_ships_the_worst_case_k7`,
  `--test the_simulation_is_placed_not_fed_back_k7`, `--doc engagement`
  (`compile_fail`: an `Engagement` literal).
- K8: every generated toolpath carries its lineage (ruling Q7). `trace/lineage.rs`:
  `Lineage { cut, pick, speeds, tool_facts, entry, findings, rules }` (no `Default`)
  is a required field of `ToolpathComputeResult` (`lineage: Arc<Lineage>`; lead
  decision D3: not on `Produced`). `JobPhases::finish` builds it with
  `Lineage::of` from the stored `FeedsProvenance` stamps (a "picked" stamp is the
  `PickRecord`), the tool, the entry contract, `Produced::entries` and the
  generation findings with their K6 consequence; it runs no calculator (Q5). A
  tool fact records its `FactKind` (K3; the diameter is `Stated`), and each
  Unknown fact adds one `tool.fact_unknown` warning (the facts the lineage lists,
  the shank diameter, and the taper of a tapered tool); `rules` holds the K5
  `RuleResult` rows of the feasibility stage (P7); `measured` (K7, P9) holds what a fresh
  simulation measured on the envelope, attached by `ProjectSession::lineage_measured`.
  `Lineage::unrecorded` is the test door of a hand-built result. Surfaces: the
  inspector Lineage `Details`, MCP `get_toolpath_lineage`, CLI `summary.json`
  `lineage`. Sentries: `--test every_generated_toolpath_has_a_lineage_k8`,
  `--doc lineage` (`compile_fail`: a result without `lineage`).
- K4: every op states how it enters material. Each `REG_*` row states its
  `entry: EntryContract` (`Dressup`, `Planner`, `DescentIsCut`,
  `StraightPlunge { reason, feed }`; no `Default`; `EntrySet` is not empty),
  in `compute/catalog/entry_contract.rs`. It is the one entry source:
  `DressupConfig::for_op`, `normalize_for_op` and the dressup pipeline
  (`PipelineRun::effective_entry_style`) read it. The air lead above an
  entry is `MoveIntent::EntryApproach`. `JobPhases::run_stages` builds
  `Produced::entries` (`EntryRecord`, private fields). The dressup entry
  plunge feed is `PlungeFeedRule::FirstLinearFeedFraction` (F201 rows,
  Unsourced). Q8 (P8): Project Curve is `StraightPlunge { feed:
  PlungeFeedRule::AxialChip }`: Suggest writes `min(G10 rule, G6 axial chip x rpm
  x Z)` where a G6 claim covers the tool (sentry
  `--test project_curve_plunges_at_the_axial_chip_q8`). Sentries: `--test every_op_states_its_entry_contract_k4`,
  `--test an_air_lead_is_an_approach_not_a_plunge_k4`,
  `--doc entry_contract_refusals`, `--lib entry_contract_table_is_pinned`.
- K5: every physical rule has the G-CENTRECUT shape (P7). `compute/feasibility.rs`:
  one `physical_rules!` row per rule (`CentreCut`, `ReachFlute`, `ReachHolder`,
  `VTipFlat`; ruling D12 deleted `PlungeKnown`) names the fact, the plan, generation, simulation and
  export `Stage` and the `diag_ids!` id; a missing stage, a `None` with no reason
  and a `Refuse` do not compile (`Stage` has no refusing value, ruling Q6).
  `JobPhases::guard_feasibility` (after `guard_model_gouge`, no mesh) records a
  `FeasibilityReport` (`ToolpathStats::feasibility`); each row warns on every
  surface (`Gate::Feasibility`, the G-code header, the diagnostics, the lineage
  `rules`). The text names the fact, its value and its `FactKind` (Q7). Commit A:
  Commit C: CentreCut plans its limit in the 3D Rough cut and link paths
  (`clamp_descents_to_limit`) and in the terrain ramp (`replan_descent_at_limit`), gives a
  lineage row where the G-CENTRECUT records fire (the surfaces print the G-CENTRECUT row),
  and a `StraightPlunge` op keeps its plunges and warns on each one deeper than the
  protrusion. Commit B: the ReachFlute simulation
  stage (`compute::simulate::flute_reach`, the ninth shared decision) gives
  `tool.flute_overreach` or the clean `tool.flute_reach_within`; with a fresh trace
  either supersedes the stock-box row (the one reducer, the export gate). Sentries:
  `--test every_physical_rule_warns_through_a_row_k5`, one `_k5_<rule>` per rule,
  `--doc feasibility`.
- K6: every finding states its consequence, and no finding refuses a job
  (ruling Q6). Each `diag_ids!` row (`diagnostics/ids.rs`) ends
  `=> warn(Danger::X)`, `=> INFO` or `=> NOT_READY` (a missing input, Q10);
  a row with none does not compile. `diagnostics::Consequence` has no
  refusing arm. Generation and every export door write the program and
  carry `ExportWarning`s (`gcode/mod.rs`); the G-code header prints one
  comment per warning. Only `EmitError` (`emit.*`) stops a file. Sentries:
  `--test a_job_with_each_finding_exports_and_names_it_k6`, viz
  `--test a_job_with_each_finding_exports_and_names_it_on_every_surface_k6`,
  `--lib every_id_states_its_consequence_k6`, `--doc diag_ids`.
