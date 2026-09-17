# S1 — gantry push as a visibly absent row

Written 2026-09-18. Step S1 of `SURFACE_IMPL.md` §1. Register item **T-10**.

The gantry push force is the one load this crate computes and never judges.
`feeds::force::lateral_cutting_force` returns a number. No `MachineProfile`
field states the thrust the gantry can deliver, so no bound exists. Before S1
the criterion tier listed three rows and the fourth load was not on the
surface at all.

S1 adds the row. It adds no number, no constant and no physics.

---

## 1. What the row is

| Property | Value |
|---|---|
| Kind | `CriterionKind::GantryPush`, after `Deflection`, before the drill kinds |
| Label | `"gantry push"` |
| Unit | `"N"` |
| State | `LoadState::Unmodeled`, always |
| Reason, milling | `NotImplemented("no machine-side thrust rating is published; register T-10")` |
| Reason, drill | `NotApplicableForOp("drill cycle — no continuous engagement")` |
| `display_peak` | `None` |
| `population` | `None` |
| `sample_range` | `None` |
| `exceeded` | `None` |

The clause cites the register item. It names no `planning/…` path
(`REVIEW_DESIGN` §6.3: 117 of 231 cited paths are dead). The sentry asserts
both halves.

One builder holds it: `verdict::gantry_push_criterion`. Two `pub const`
strings hold the wording, so the GUI, the CLI and the MCP print one clause:

- `GANTRY_PUSH_UNMODELED_CLAUSE`
- `GANTRY_PUSH_NOT_APPLICABLE_CLAUSE`

`UnmodeledReason` carries a `String` because it deserializes over the MCP
wire, so the two reasons are `LazyLock` statics, borrowed for `'static`.

---

## 2. Where the row joins, and why there

The row goes in `ToolpathLoadVerdict::milling_criteria`, not in `criteria`.
`all_not_applicable` reads `milling_criteria`; a row added to `criteria`
alone would have left that partition reading three rows while the tier read
four.

The drill arm is decided by the three gates beside it, not by
`drill_gates.is_some()`:

```rust
let plunge_only = rows.iter().all(|s| is_not_applicable(s.unmodeled_reason));
```

The optimizer path leaves `drill_gates` as `None` on a drill toolpath (the
note at `tool_load::evaluate_toolpath` says so). Keying on `drill_gates`
would have printed "no thrust rating" on a hole. The sentry arm
`a_drill_without_drill_gates_still_buckets_as_not_applicable` pins this.

---

## 3. Every match site the compiler found

The plan predicted "about a dozen need a decision". The compiler found
**three**, in two files.

| Site | Decision |
|---|---|
| `tool_load/verdict.rs` `CriterionKind::label` | `"gantry push"` |
| `tool_load/verdict.rs` `CriterionKind::unit` | `"N"` |
| `rs_cam_viz/src/ui/sim_op_list.rs:1212` `criterion_short_label` | `"gantry"` |

Sites that reference `CriterionKind` and did NOT break, each checked by hand:

- `tool_load/drill_gates.rs:353` — the `ExceededCriterion` match has a `_`
  fallback arm. It is reached only on `Exceeds`, which this row never is.
- `rs_cam_viz/src/ui/sim_diagnostics.rs:1241` — matches
  `(status.kind, burn_risk)` with a `_` arm, reached only on `Exceeds`.
- `rs_cam_core/src/ops/drill.rs:33` — a comment, not code.
- `tests/gate_population_vacuity_xvac.rs`, `tests/drill_evidence_wording_d3.rs`
  — both name drill kinds only; neither enumerates the kind list. Neither
  needed an edit, and both pass unchanged.

`crates/rs_cam_viz/src/ui/sim_diagnostics.rs` therefore takes **no edit** in
S1.

---

## 4. The counters audited

Rule: a row that no operator action can fill must not make a toolpath count
as unmodelled for the purpose of "run the simulation or supply tool data".

One predicate carries it: **`CriterionStatus::is_known_absence`**.

| Counter | File | Result |
|---|---|---|
| `ToolpathLoadVerdict::any_unmodeled` | `verdict.rs` | **Edited.** Skips a known absence. Every site below reads it. |
| `enforce_load_policy` | `core/src/gcode/mod.rs:689` | Unchanged; reads `any_unmodeled`. Fixed by the above. |
| `ToolLoadReportSummary::fully_unmodeled` | `verdict.rs` | Unchanged. It needs `modeled_count() == 0`, and an extra `Unmodeled` row does not move that count. |
| `ToolLoadReportSummary::not_applicable` | `verdict.rs` | Unchanged. The drill arm keeps `all_not_applicable` true. |
| `ToolLoadReportSummary::within` / `exceeds` | `verdict.rs` | Unchanged. |
| Readiness "unmodeled" `CountPill` | `viz/ui/readiness_panel.rs:212` | **No edit needed.** It reads `summary.fully_unmodeled`, which does not move. |
| Readiness triage | `viz/ui/readiness.rs:346` | Reads `report.any_unmodeled()`. Fixed centrally. |
| `tool_load_summary_detail` | `viz/ui/preflight.rs:375` | Reads `v.any_unmodeled()`. Fixed centrally. |
| `draw_tool_load_overrides` | `viz/ui/preflight.rs:451,476,487` | Reads `any_unmodeled()`. Fixed centrally; the "Accept unmodeled criteria" checkbox does not appear because of this row. |
| `format_verdict_line` | `viz/ui/preflight.rs:535` | **No edit needed, but see §7.** It matches the three typed verdicts, not `criteria()`, so it never prints the row. |
| `toolpath_status_flags` | `viz/ui/sim_op_list.rs:1082` | **Edited.** Skips a known absence, beside the existing `is_drill_not_applicable` skip. Without it every milling row painted a `? gantry` triage flag. |
| `sim_timeline.rs:172`, `sim_diagnostics.rs:465` | viz | Read `summary.fully_unmodeled`. No edit needed. |
| `sim_timeline.rs:1914` | viz | Reads `verdict.any_unmodeled()`. Fixed centrally. |
| MCP `get_tool_load_report` | `viz/src/app/mcp/generation.rs:112` | See §7. It serialises the typed verdicts, not `criteria()`. |

`all_not_applicable` keeps its meaning: it is still "every milling row is
`NotApplicableForOp`", and the gantry row obeys it.

No counter was found in a viz file outside the permitted set that needed an
edit.

---

## 5. The finding the plan did not anticipate

**The brief's rule 4 said "a row whose reason is `NotImplemented(_)` is a
known absence". Implemented literally, that silently weakens two working
gates.**

`NotImplemented` has two live producers today that are not the gantry row:

| Producer | Clause | The operator CAN fix it by |
|---|---|---|
| `tool_load/power.rs:281` | `"machine profile not provided to evaluator"` | supplying a machine profile |
| `tool_load/deflection.rs:187` | `"tool reports zero stickout"` | setting the tool stickout |

Both name a missing input. A predicate keyed on the bare variant would stop
`enforce_load_policy` refusing on either, under `accept_unmodeled: false`.

**Measured evidence.** With the over-broad predicate in place, two core lib
tests went red:

```
failures:
    gcode::tests::enforce_blocks_unmodeled_by_default
    gcode::tests::enforce_lets_exceeded_through_only_with_explicit_flag
test result: FAILED. 2519 passed; 2 failed; 12 ignored
```

Both use `power_not_implemented()` as their unmodelled fixture, and the
second says so in as many words: *"Exceeds bypassed but Unmodeled still
blocks"*. These are not stale pins. They are the export gate working.

**The repair.** The distinction is the KIND, not the reason. A new predicate
`CriterionKind::is_unmodeled_by_design` returns true only for `GantryPush`:
the crate has no gate, no bound and no producer for it, and no project input
creates one. `CriterionStatus::is_known_absence` is then:

```rust
self.kind.is_unmodeled_by_design()
    && matches!(self.unmodeled_reason, Some(UnmodeledReason::NotImplemented(_)))
```

The reason check stays, so a drill's `NotApplicableForOp` gantry row keeps
the arm the three milling gates take beside it.

With the repair, both lib tests pass with **no fixture change**. Nothing was
tuned. The sentry arm
`a_fillable_not_implemented_row_is_not_a_known_absence` pins the
distinction, using the power gate's own clause verbatim.

The day a thrust rating exists, `GantryPush` leaves
`is_unmodeled_by_design` and every counter starts counting it, with no other
change.

---

## 6. The badge hard-codes; the row paints nothing yet

`draw_tool_load_badges` (`sim_diagnostics.rs:1009`) does **not** iterate
`criteria()`. It hard-codes three `verdict_badge` calls with three cap
arguments bound at `sim_diagnostics.rs:947`. The gantry row therefore paints
no badge today.

Per decision 7 this is left alone; step **V2** deletes the cap arguments and
makes the row read its own bound. Until V2 lands, the row is visible only
through `criteria()` — which today reaches `exceeded_criteria` (where it
never appears) and `toolpath_status_flags` (where it is now skipped).

**This means S1 ships no operator-visible change.** It lays the core row and
the prompt rule that V2 and V3 render. That is the intended sequence, and it
is worth stating plainly rather than implying the surface changed.

---

## 7. The MCP wire did not move, and the row is not on it

`mcp_wire_surface_pin` passes unchanged: `2 passed; 0 failed`. No snapshot
was re-blessed anywhere in this step.

The reason is that decision 6's premise does not hold. `criteria()` is a
DERIVED view, not a serde struct. `mcp_get_tool_load_report`
(`viz/src/app/mcp/generation.rs:147`) serialises `&report`, which is
`ToolLoadReport { per_toolpath: Vec<ToolpathLoadVerdict> }` — the typed
`chipload` / `power` / `deflection` / `drill_gates` fields. It never calls
`criteria()`.

Two consequences, both reported rather than fixed, because S1's file set does
not cover them and each needs a decision:

1. **The MCP does not see the gantry row.** An agent reading
   `get_tool_load_report` still sees three criteria. Putting the row on the
   wire means either a new serialised field on `ToolpathLoadVerdict` or
   serialising `criteria()` beside `load_report`. That is a wire change and
   belongs with S4, which moves the same struct.
2. **`diagnostics_from_load_verdict`** (`core/src/diagnostics/adapters/from_tool_load.rs`)
   also reads the typed verdicts directly, so `get_toolpath_diagnostics` does
   not carry the row either.

A third, smaller one: with the defect injected (§5), `enforce_load_policy`
printed its refusal as

```
G-code export refused: tool load not fully modeled for toolpath(s):
  toolpath 0: 
Pass `accept_unmodeled=true` to acknowledge unmodeled criteria.
```

— an empty criterion list, because that loop also enumerates the three typed
verdicts by hand. The refusal is now unreachable for this row, so nothing is
broken today, but the loop will go silent again for any future criterion that
is not one of the three. It should derive from `criteria()`, as
`exceeded_criteria` already does.

---

## 8. Vacuity: the row is absent, not vacuous

`is_vacuous()` is `self.population.is_some_and(GatePopulation::is_vacuous)`.
The row sets `population: None`, so the predicate is **false** and
`vacuity_clause()` is empty. This is correct and deliberate.

X-VAC is a POPULATION bar: a gate that was handed units and measured an
EMPTY set. This row was handed nothing and measured nothing, which is a
different finding with a different remedy. `None` reads as "not stated",
never as zero. The sentry arm `the_gantry_row_is_absent_not_vacuous` pins it.

---

## 9. The sentry

`crates/rs_cam_core/tests/an_absent_limit_is_visibly_absent_g_gantry.rs`,
nine arms.

**Red-first, twice.**

1. By construction: the file did not compile on HEAD —
   `error[E0599]: no variant ... named GantryPush found for enum CriterionKind`,
   plus the same for `is_known_absence`.
2. Defect injected once, after the build, per `tests/CLAUDE.md`. Reverting
   `!s.is_known_absence()` in `any_unmodeled` gave:

```
failures:
    the_gantry_row_does_not_make_a_toolpath_unmodeled
    the_gantry_row_never_refuses_an_export
test result: FAILED. 6 passed; 2 failed; 0 ignored
```

The guard was restored and the file went green.

The milling fixture runs the three real gates (`chipload::evaluate`,
`power::evaluate`, `deflection::evaluate`) against a measured 12-sample
trace, so the report is the shipped shape. The drill fixture runs
`drill_gates::evaluate` on a real `DrillOp`.

| Arm | Test |
|---|---|
| a | `a_milling_toolpath_carries_exactly_one_gantry_push_row` |
| a | `the_gantry_row_is_absent_not_vacuous` |
| b | `a_drill_cycle_reads_not_applicable_not_not_implemented` |
| b | `a_drill_without_drill_gates_still_buckets_as_not_applicable` |
| c | `the_gantry_row_never_refuses_an_export` |
| d | `the_gantry_row_does_not_make_a_toolpath_unmodeled` |
| d | `a_fillable_not_implemented_row_is_not_a_known_absence` |
| d | `a_drill_gantry_row_is_not_a_known_absence` |
| e | `the_three_milling_criteria_are_still_there_and_at_least_one_is_modelled` |

Arm (d) note: `ToolLoadReportSummary` has no "partly unmodelled" count, so
the test pins the four counts it does have —
`within == 1`, `exceeds == 0`, `fully_unmodeled == 0`, `not_applicable == 0`.

---

## 10. Verification

Every command through `scripts/cargo_lane.sh`. Viz through `-j 2`.

| Command | `test result:` |
|---|---|
| `test -p rs_cam_core -q --test an_absent_limit_is_visibly_absent_g_gantry` | `ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out` |
| `test -p rs_cam_core --lib -q` | `ok. 2521 passed; 0 failed; 12 ignored; 0 measured; 0 filtered out` |
| `test -p rs_cam_core -q --test predicted_feed_gates_f035` | `ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out` |
| `test -p rs_cam_core -q --test gcode_phase0_capture` | `ok. 0 passed; 0 failed; 16 ignored; 0 measured; 0 filtered out` |
| `test -p rs_cam_core -q --test gate_population_vacuity_xvac` | `ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out` |
| `test -p rs_cam_core -q --test drill_evidence_wording_d3` | `ok. 11 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out` |
| `test -p rs_cam_viz -j 2 -q --test mcp_wire_surface_pin` | `ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out` |
| `test -p rs_cam_viz -j 2 -q --test ribbon_and_mcp_diagnostic_ids_n4` | `ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out` |
| `test -p rs_cam_viz -j 2 -q --test the_chipload_verdict_is_one_row_g_chipverdict` | `ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out` |
| `test -p rs_cam_cli -q` (4 binaries) | `ok. 35 passed`, `ok. 4 passed`, `ok. 9 passed`, `ok. 2 passed`; all `0 failed` |
| `test -p rs_cam_mcp -q` | `ok. 31 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out` |

Non-test gates, all clean:

- `clippy -p rs_cam_core --all-targets --features heavy-tests,research,test-support -- -D warnings` — no error, no warning.
- `check -p rs_cam_viz -j 2 --all-targets` — clean.
- `check -p rs_cam_cli --all-targets` — clean.
- `check -p rs_cam_mcp --all-targets` — clean.

**Format.** `fmt --all -- --check` reports a diff in
`crates/rs_cam_core/tests/a_published_power_is_at_the_depth_that_cuts_g_s2.rs`.
That file is the S2 agent's, not this step's, and it was not touched.
`rustfmt --edition 2024 --check` over this step's three source files is
clean.

`gcode_phase0_capture` is fully `#[ignore]`d on a plain run; per
`tests/CLAUDE.md` that marks an instrument, not a break.

The core full gate, the whole viz suite and `wanaka_suggest_integration` were
not run.

---

## 11. Left for the orchestrator

- `crates/rs_cam_core/src/tool_load/CLAUDE.md` names the folder's sentries
  and sits at exactly **40 lines**, its stated ceiling (commit `4027bed1`).
  Adding the new sentry needs a line removed. That file is outside this
  step's set, so it was not edited.
- The two wire gaps in §7, and the hand-written refusal loop in
  `enforce_load_policy`. All three belong with S4, which moves the same
  struct.

---

## 12. Files changed

- `crates/rs_cam_core/src/tool_load/verdict.rs`
- `crates/rs_cam_core/tests/an_absent_limit_is_visibly_absent_g_gantry.rs` (new)
- `crates/rs_cam_viz/src/ui/sim_op_list.rs`
- `planning/TECH_DEBT_REGISTER.md` (T-10 entry and its table row)
- `planning/load_model_2026-09-16/S1_IMPLEMENTATION.md` (this file)

Not committed.
