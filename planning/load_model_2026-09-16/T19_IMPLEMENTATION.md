# T-19 — the export gate's unmodelled half reads `criteria()`

Implemented 2026-09-18, on top of S3 (`79578772`) and S4.

`gcode::enforce_load_policy` had two halves and they read the criterion
tier from two different places. The exceeded half derived from
`ToolpathLoadVerdict::criteria()`. The unmodelled half decided from
`report.any_unmodeled()` — which reads `criteria()` — and then built its
MESSAGE by matching `v.chipload`, `v.power` and `v.deflection` one at a
time. Five milling rows exist since S1 and S3, so two of them could
never appear in a refusal.

---

## 1. The red evidence

The fixture is `depth_only_verdict()` in the sentry: chipload, power and
deflection measure, the depth row is
`Unmodeled(NotImplemented("no usable tool diameter for the rigidity
cap"))`, and the gantry row is the known absence every milling toolpath
carries. On HEAD `00b2d9ca` the gate refused the export with this
message, printed by the sentry's first arm:

```text
G-code export refused: tool load not fully modeled for toolpath(s):
  toolpath 4: 
Pass `accept_unmodeled=true` to acknowledge unmodeled criteria.
```

The line after `toolpath 4:` is empty. The operator is told the job is
refused and is handed nothing to act on. That is defect class 3: an
absence rendered as a blank line.

```text
thread 'a_depth_only_refusal_names_the_depth_row' panicked at
crates/rs_cam_core/tests/an_unmodelled_refusal_names_every_row_g_t19.rs:107:5:
the refusal must name the row it refused on, got: …
test result: FAILED. 1 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out
```

Today the same fixture reads:

```text
G-code export refused: tool load not fully modeled for toolpath(s):
  toolpath 4: depth of cut: not implemented yet — no usable tool diameter for the rigidity cap
Pass `accept_unmodeled=true` to acknowledge unmodeled criteria.
```

The first line and the override hint are byte-for-byte the old ones.

---

## 2. The two pure helpers

`crates/rs_cam_core/src/gcode/mod.rs`, the same shape S4 gave the
exceeded half:

```rust
pub fn refusing_unmodeled<'a>(criteria: &'a [CriterionStatus<'a>])
    -> Vec<&'a CriterionStatus<'a>>;

pub fn enforce_unmodeled_policy(
    per_toolpath: &[(ToolpathId, Vec<CriterionStatus<'_>>)],
    accept_unmodeled: bool,
) -> Result<(), ExportError>;
```

`enforce_load_policy` now decides nothing of its own. It builds the
criterion tier once and hands the same list to both halves:

```rust
let note = enforce_exceedance_policy(&per_toolpath, policy.accept_exceeded)?;
enforce_unmodeled_policy(&per_toolpath, policy.accept_unmodeled)?;
Ok(note)
```

**What counts.** `refusing_unmodeled` returns the rows with
`state == Unmodeled` that `is_known_absence()` rejects, which is the S1
counter verbatim — so the gantry-push row still refuses nothing.

**The drill partition.** `any_unmodeled` also returns false when
`drill_gates.is_some()` and every milling gate reads
`NotApplicableForOp`: a drill cycle is measured, not unmeasured. The
helper is pure and never sees `drill_gates`, so it reads the same fact
off the rows: drill rows are in `criteria()` exactly when `drill_gates`
is `Some`. One private predicate, `is_drill_gate`, names the three drill
kinds. The direction is deliberate — a new MILLING gate takes the
default arm and refuses like the four beside it with no edit here, and a
drill gate nobody adds to the list reads as a milling gate, so the
partition does not fire and the export refuses. The unsafe direction
costs an edit; the safe one is free.

**The wording.** `UnmodeledReason` carries no `Display`, and the old
message printed `{:?}` — `SimulationRequired` in an operator's refusal.
A private `unmodeled_clause` words each reason: `vacuity_clause()` when
the row is vacuous, otherwise a match that prints the reason's own
detail string, because the gate that refused wrote that string for this
reader. The words are the GUI's own
(`viz/src/ui/sim_diagnostics.rs:1260`) minus its `Unmodeled: ` prefix.

**The stale headline** moved with it. A cached-but-invalid trace is a
different operator action from a missing one, and the headline still
says which. The old test matched the three typed verdicts; the new one
scans the criterion rows for `UnmodeledReason::StaleSimulation`. The two
agree: only `project_load_report`'s `rewrite_sim_required_to_stale_*`
writes that reason, and it writes it onto the chipload, power and
deflection rows, all three of which are in `criteria()`. Nothing pinned
the stale headline before; the sentry pins it now. (§7 adds the depth
row to that rewrite. It is a fourth row in the same list, so the
equivalence above is unchanged and the scan now reaches one row more.)

---

## 3. The decision did not change

Three independent proofs.

**a. The equivalence arm.** `ToolpathLoadVerdict::any_unmodeled` is the
decision source the old gate read and T-19 does not touch it. The sentry
arm `the_helper_refuses_exactly_where_any_unmodeled_says_it_must` asserts
`!refusing_unmodeled(&criteria).is_empty() == v.any_unmodeled()` over
five fixtures — depth only, fully modelled, chipload and depth, a drill
cycle with its gates, and a drill cycle without them — and first asserts
that the table holds both answers, so the arm cannot pass on a constant.

**b. The two lib tests named in the brief are unchanged and green.**
`enforce_blocks_unmodeled_by_default` and
`enforce_lets_exceeded_through_only_with_explicit_flag` read exactly as
they did on HEAD.

**c. One lib test's assertions moved, and its claim did not.**
`enforce_distinguishes_unmodeled_reasons` asserted
`m1.contains("SimulationRequired")` and `m2.contains("NoVendorData")` —
the `{:?}` text, which is the defect. Its claim is that two reasons
produce two actionable messages; the assertions now read the operator's
words (`"simulation has not been run"`, `"no vendor LUT row for this
tool/material combination"`) and one added line asserts `m1 != m2`. This
is the only existing test in the repository that this change edits.
Nothing outside `gcode/mod.rs` couples to the refusal wording:
`rg -n "not fully modeled|acknowledge unmodeled|cached simulation is
stale" --type rust crates/` matches the gate and the new sentry, and no
other crate.

---

## 4. The sentry

`crates/rs_cam_core/tests/an_unmodelled_refusal_names_every_row_g_t19.rs`,
ten arms. A new file, not an arm on
`a_weak_bound_cannot_refuse_an_export_g_s4weak.rs`: that file's claim is
that a bound's PROVENANCE decides whether an exceedance may refuse, and
this claim is that the unmodelled half names every row it refused on.
Different half, different rule.

| Brief | Arm |
|---|---|
| a | `a_depth_only_refusal_names_the_depth_row_and_its_reason` |
| a | `the_refusal_keeps_its_headline_and_its_override_hint` |
| — | `a_stale_trace_keeps_its_own_headline_and_names_its_row` |
| b | `a_refusal_names_every_counting_row_in_criterion_order` |
| c | `the_gantry_row_alone_refuses_nothing_and_is_never_named` |
| d | `a_drill_cycle_with_its_gates_still_exports` |
| d | `a_drill_cycle_without_its_gates_still_refuses` |
| e | `the_override_accepts_the_rows_the_gate_would_name` |
| — | `the_helper_refuses_exactly_where_any_unmodeled_says_it_must` |
| f | `a_fully_modelled_report_passes` |

The drill arms drive the shipped drill path — `emit_drill_samples`,
`build_drill_toolpath_summary`, `drill_gates::evaluate` — so the
partition is tested against the gates that produce it, not against a
description of them. The milling fixtures are hand-built verdicts,
because the case the defect hid needs four gates to measure while the
fifth does not, and no fixture in the suite reaches that combination.

### Red-first, twice

1. By construction. The first arm ran against HEAD and printed the
   message in §1.
2. Two defects injected after the build, each removed and the file
   re-run green.

Defect 1 — `refusing_unmodeled` drops the drill partition
(`if false && ran_drill_gates && …`):

```text
test result: FAILED. 8 passed; 2 failed; 0 ignored; 0 measured; 0 filtered out
    a_drill_cycle_with_its_gates_still_exports
    the_helper_refuses_exactly_where_any_unmodeled_says_it_must
```

Defect 2 — the exact T-19 defect, narrowed to the row it hid: the
message loop filters `CriterionKind::DepthOfCut` out of the rows it
names.

```text
test result: FAILED. 6 passed; 4 failed; 0 ignored; 0 measured; 0 filtered out
    a_depth_only_refusal_names_the_depth_row_and_its_reason
    a_refusal_names_every_counting_row_in_criterion_order
    the_gantry_row_alone_refuses_nothing_and_is_never_named
    the_refusal_keeps_its_headline_and_its_override_hint
```

---

## 5. Verification

Every command through `scripts/cargo_lane.sh`.

| Target | Result |
|---|---|
| `--test an_unmodelled_refusal_names_every_row_g_t19` | `test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s` |
| `-p rs_cam_core --lib -q` | `test result: ok. 2530 passed; 0 failed; 12 ignored; 0 measured; 0 filtered out; finished in 41.54s` |
| `--test export_disabled_cached_n1` | `test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s` |
| `--test adaptive_feed_modulation_pipeline_f036b` | `test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.74s` |
| `--test export_honors_coolant_p0d1` | `test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s` |
| `--test post_format_round_trip_p1` | `test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s` |
| `--test a_weak_bound_cannot_refuse_an_export_g_s4weak` | `test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s` |
| `--test an_absent_limit_is_visibly_absent_g_gantry` | `test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s` |
| `--test export_datum_setup_frame` | `test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s` |
| `--test the_depth_that_cut_is_a_measured_load_g_s3depth` | `test result: ok. 11 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s` |
| `--test frozen_snapshot_regeneration_s4` | `test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 4.74s` |
| `--test gate_population_vacuity_xvac` | `test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s` |
| `--test drill_evidence_wording_d3` | `test result: ok. 11 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s` |

The first ten targets are every file
`rg -l "enforce_load_policy|enforce_exceedance_policy|refusing_exceedances|accept_unmodeled|ExportError" crates/rs_cam_core/tests/`
names, plus the two the brief adds by name.

Clippy, the core gate command:

```text
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 31.65s
```

`fmt --all -- --check` reports **no diff in any file this step owns**. It
is not silent: it reports eight files, all of them another session's
in-flight work —

```text
Diff in crates/rs_cam_mcp/src/server.rs
Diff in crates/rs_cam_viz/src/ui/preflight.rs
Diff in crates/rs_cam_viz/src/ui/readiness_panel.rs
Diff in crates/rs_cam_viz/src/ui/sim_diagnostics.rs
Diff in crates/rs_cam_viz/src/ui/sim_timeline.rs
Diff in crates/rs_cam_viz/tests/readiness_shows_the_limits_g_readylimits.rs
Diff in crates/rs_cam_viz/tests/the_limit_rows_read_their_own_bound_g_ownbound.rs
Diff in crates/rs_cam_viz/tests/zz_dump_tmp.rs
```

Neither `crates/rs_cam_core/src/gcode/mod.rs` nor the new sentry appears.

---

## 6. Unanticipated

**1. The wording door is duplicated, and it should not be.**
`UnmodeledReason` now has three renderers that word the same ten
variants and share no code: `gcode::unmodeled_clause` (new),
`viz/ui/sim_diagnostics.rs:1260` and
`viz/ui/preflight.rs:519` (a short label), with a fourth wording in
`core/diagnostics/adapters/from_tool_load.rs:830`. The right fix is one
door on the type in `verdict.rs`, which every renderer formats. That
file is outside this step's file set, so this step words the refusal in
place and files the door as a follow-up. It is a wording-drift risk, not
a decision risk: the four sites render, and none of them decides.

**2. The depth row cannot report a stale trace.** Registered as T-21 and
closed in §7 below.
`project_load_report` rewrites `SimulationRequired` to
`StaleSimulation` on the chipload, power and deflection verdicts only.
S3 added the depth gate and no rewrite beside it, so a stale trace
leaves the depth row saying "simulation has not been run" while the
three rows next to it say "stale". The refusal still reaches the stale
HEADLINE through those three, so no export decision is wrong today, and
the operator reads one row that disagrees with the others. Outside this
file set; filed below.

**3. A drill gate added later needs an edit here.** `CriterionKind` has
no `is_drill_gate` predicate, so `gcode` carries its own. The fail-safe
direction is covered in §2, and the predicate belongs beside
`is_unmodeled_by_design` in `verdict.rs` when that file is next open.

Findings 1 and 2 are register candidates; this step did not write them,
because its register edit is the T-19 row and entry only.

---

## 7. T-21 — the stale rewrite covers the depth row

Implemented 2026-09-18, after T-19 landed at `465d2d2d`. This is a
section of the T-19 report and not a file of its own: T-21 is finding 2
of §6 above, it is four lines of code, and a separate document would be
mostly a cross-reference to this one.

### The defect

`project_load_report` discards a trace whose provenance no longer
matches the project, runs the gates without it, and rewrites each
`Unmodeled(SimulationRequired)` into `Unmodeled(StaleSimulation)`. The
rewrite named three verdicts. S3 added the depth gate beside them and no
rewrite arm, so one toolpath reported three rows stale and one row never
simulated, for the one trace and the one cause.

### Red first

The sentry ran against the shipped code before the fix. The three rows
beside the depth row passed the same loop, which isolates the defect to
the one arm:

```text
thread 'a_stale_trace_marks_the_depth_row_like_the_rows_beside_it' panicked at
crates/rs_cam_core/tests/a_stale_trace_marks_every_row_it_invalidated_g_t21.rs:131:9:
assertion `left == right` failed: the depth row must read as stale evidence, not as missing evidence
  left: Some(SimulationRequired)
 right: Some(StaleSimulation)
test result: FAILED. 1 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out
```

No defect had to be injected: the red run IS the shipped defect, caught
by the arm that now guards it.

### The change

`rewrite_sim_required_to_stale_depth` in `gcode/mod.rs`, the shape of
the three functions beside it, called from the same `if sim_is_stale`
loop. Four lines of body and one call.

**The drill gates take no rewrite and need none, and this was read
rather than assumed.** `drill_gates::DrillGateOutcome` has two arms,
`Within` and `Exceeds`; it carries no `Unmodeled` variant and no
`UnmodeledReason` at all. Those gates read the drill op's own geometry
and feeds — hole depth, peck depth, feed over diameter — and never a
simulation trace, so no trace can invalidate them. The comment at the
call site says so, so the next reader does not re-derive it.

### The sentry

`crates/rs_cam_core/tests/a_stale_trace_marks_every_row_it_invalidated_g_t21.rs`,
two arms. A new file rather than an arm on the T-19 sentry: T-19's claim
is that the refusal MESSAGE names every row, and this claim is that the
rewrite REACHES every row. Different function, different rule, and this
one needs a `ProjectSession` the T-19 file does not build.

- `a_stale_trace_marks_the_depth_row_like_the_rows_beside_it` — the
  claim. It asserts every milling row reads `StaleSimulation`, and then
  that the depth row's reason equals the deflection row's, which is the
  self-contradiction the defect produced.
- `with_no_trace_the_same_rows_ask_for_a_simulation` — non-vacuity. The
  same fixture with no trace reads `SimulationRequired` on the same four
  rows, so the rewrite is what moves the reason and the first arm is not
  reading a constant the gates set themselves.

The stale condition is reached without simulating: a trace with no
provenance block carries no evidence of what it was simulated against,
and `sim_trace_is_fresh` reads it as stale (L7, operator ruling
2026-09-16). The first arm asserts `!sim_trace_is_fresh(…)` as its
fixture guard, so the arm cannot pass through a fresh-trace path by
accident. The fixture needs no generated result either: a report is
built per enabled toolpath config.

**Not built: a fresh-trace arm.** Stamping a matching provenance by hand
needs `compute::simulate::hash_toolpath`, which is not public, and
running a real simulation in this sentry buys a slower fixture for a
path T-21 does not change. The freshness decision is
`sim_trace_is_fresh`'s own, it is untouched here, and the guard above
pins which side of it this fixture is on.

### Verification

Every command through `scripts/cargo_lane.sh`.

| Target | Result |
|---|---|
| `--test a_stale_trace_marks_every_row_it_invalidated_g_t21` | `test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s` |
| `-p rs_cam_core --lib -q` | `test result: ok. 2530 passed; 0 failed; 12 ignored; 0 measured; 0 filtered out; finished in 39.10s` |
| `--test export_disabled_cached_n1` | `test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s` |
| `--test adaptive_feed_modulation_pipeline_f036b` | `test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.76s` |
| `--test export_honors_coolant_p0d1` | `test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s` |
| `--test post_format_round_trip_p1` | `test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s` |
| `--test a_weak_bound_cannot_refuse_an_export_g_s4weak` | `test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s` |
| `--test an_absent_limit_is_visibly_absent_g_gantry` | `test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s` |
| `--test export_datum_setup_frame` | `test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s` |
| `--test the_depth_that_cut_is_a_measured_load_g_s3depth` | `test result: ok. 11 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s` |
| `--test frozen_snapshot_regeneration_s4` | `test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 5.05s` |
| `--test an_unmodelled_refusal_names_every_row_g_t19` | `test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s` |
| `--test gate_population_vacuity_xvac` | `test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s` |
| `--test drill_evidence_wording_d3` | `test result: ok. 11 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s` |

Clippy, the core gate command:

```text
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 29.61s
```

`fmt --all -- --check` reports no diff in either file this step owns. It
reports one file, `crates/rs_cam_mcp/src/server.rs`, which is another
session's in-flight work.

### Unanticipated

Nothing beyond the entry above. T-20 — the four renderers that word the
same ten `UnmodeledReason` variants — stands open and is unchanged by
this step.
