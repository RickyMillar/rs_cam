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
  a `Stated` value. Suggest reads `CutSpec::todays_hints` through
  `operation_feeds_hints` (P4; Phase 5 deletes it). Sentries:
  `--test every_op_states_its_cut_k1`, `--doc cut_spec_refusals`.
- K4: every op states how it enters material. Each `REG_*` row states its
  `entry: EntryContract` (`Dressup`, `Planner`, `DescentIsCut`,
  `StraightPlunge { reason, feed }`; no `Default`; `EntrySet` is not empty),
  in `compute/catalog/entry_contract.rs`. It is the one entry source:
  `DressupConfig::for_op`, `normalize_for_op` and the dressup pipeline
  (`PipelineRun::effective_entry_style`) read it. The air lead above an
  entry is `MoveIntent::EntryApproach`. `JobPhases::run_stages` builds
  `Produced::entries` (`EntryRecord`, private fields). The dressup entry
  plunge feed is `PlungeFeedRule::FirstLinearFeedFraction` (F201 rows,
  Unsourced). Sentries: `--test every_op_states_its_entry_contract_k4`,
  `--test an_air_lead_is_an_approach_not_a_plunge_k4`,
  `--doc entry_contract_refusals`, `--lib entry_contract_table_is_pinned`.
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
