# S4 — every criterion carries its bound and its provenance

Written 2026-09-18. Step S4 of `SURFACE_IMPL.md` §1. It is the backbone
of the limits surface: before it, a criterion row carried a reading and
no limit, so the GUI built two of the three caps it drew against.

One of those two was an L over D ratio of 4.0 set beside a gate that
judges MILLIMETRES against `deflection::EXCEEDS_BOUND_MM = 0.200`
(`rs_cam_viz/src/ui/sim_diagnostics.rs:1126`). That is defect class 2 of
this programme: a quantity divided by a fraction of a DIFFERENT
quantity.

S4 adds **no number**. Every bound it publishes is the value the gate
already judged against.

---

## 1. What shipped

### 1.1 Two fields on `CriterionStatus`

`crates/rs_cam_core/src/tool_load/verdict.rs`:

```rust
pub bound: Option<f64>,
pub bound_source: Option<BoundSource>,
```

`bound` is in the row's own `unit`. `bound_source` is `None` exactly
when `bound` is `None`.

### 1.2 One typed enum

```rust
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum BoundSource {
    MachinePowerCurve { rpm: f64, safety_factor: f64 },
    VendorChipBand {
        floor_mm_per_tooth: Option<f64>,
        ceiling_mm_per_tooth: f64,
        source: ChipBoundsSource,
    },
    DeflectionBudget,
    RigidityRuleOfThumb { factor: f64, diameter_mm: f64 },
    DrillEnvelope,
}
```

with `setting()`, `clause()` and `gates_export()`.

Two adjustments to the plan's shape, both to keep a fact:

| Plan | Shipped | Cause |
|---|---|---|
| `VendorChipBand(ChipBoundsSource)` | a struct variant with the floor, the ceiling and the source | Decision 1 requires the floor to ride in the source, and the ceiling is the bound the row reads against. `ChipBoundsSource` alone carries neither. |
| `floor_mm_per_tooth: f64` | `Option<f64>` | `ChipBounds::min_mm_per_tooth` is an `Option`: some LUT rows ship only an upper bound. A fabricated `0.0` floor is defect class 3. |

The serde tag is `kind`, not `source`: an internal tag named `source`
collides with the `VendorChipBand::source` field, and `kind` is the tag
every other enum in this file uses.

`setting()` returns one of `"machine"`, `"tool"`, `"material"`,
`"vendor row"`:

| Variant | Setting | Why |
|---|---|---|
| `MachinePowerCurve` | machine | the power curve and the safety factor are both `MachineProfile` fields |
| `VendorChipBand` | vendor row | the matched LUT row |
| `DeflectionBudget` | tool | the budget is a fixed constant, so no setting moves the BOUND; the tool moves the reading under it. The arm says so in a comment. |
| `RigidityRuleOfThumb` | machine | `RigidityProfile` is a `MachineProfile` field (`machine/mod.rs:55`), and the factor is what makes the bound a rule of thumb |
| `DrillEnvelope` | material | all three drill envelopes are material-derived |

`gates_export()` is false for `RigidityRuleOfThumb` only.

### 1.3 Two methods and one owned row

- `CriterionStatus::bound_clause()` — `"limit {bound:.4} {unit}, from
  {source.clause()}"`, empty when there is no bound. This is where the
  bound's own digits reach an operator; `BoundSource::clause()` states
  the provenance and cannot format a bound it does not carry (see §5).
- `CriterionStatus::refuses_export()` — `Exceeds` **and**
  `bound_source.is_none_or(gates_export)`. One predicate, so a renderer
  and the export gate cannot disagree.
- `CriterionRow` + `ToolpathLoadVerdict::criterion_rows()` — the owned,
  serializable form of the criterion tier.

---

## 2. Per gate: where the bound and the source live

Decision 3 says the bound and the source are STORED on the verdict, not
recomputed. Four of the five gates already stored every fact, so
`as_criterion_status` projects them; only the power gate needed new
storage, and that is stated below.

| Gate | `bound` | Source | Stored where |
|---|---|---|---|
| Chipload | `bounds.max_mm_per_tooth` of the metric that decided the verdict (`approach_to_max` on `Within`, `triggering` on `Exceeds`) | `VendorChipBand { floor, ceiling, source }` | already on `ChiploadMetric::bounds` (`ChipBounds`) — no new field |
| Power | `available_kw` | `MachinePowerCurve { rpm, safety_factor }` | **new** `bound_source` field on `PowerVerdict::Within` and `::Exceeds` |
| Deflection | `bounds.exceeds_mm`, which the gate sets from `EXCEEDS_BOUND_MM` | `DeflectionBudget` | already on `DeflectionBounds` — the variant is a unit, so no new field |
| Drill (all three) | `DrillGateOutcome::bound()`, the `threshold` both arms already carry | `DrillEnvelope` | already on the outcome — new `bound()` accessor only |
| Gantry push | `None` | `None` | no thrust rating exists (T-10) |

**The power gate is the only one that gained storage.** `available_kw`
is a PRODUCT — `power_at_rpm(rpm) × safety_factor` — and both factors
are gone by the time a consumer reads it. `power::evaluate` now tracks
the rpm beside `peak_available_at_peak` and `last_available_kw` and
carries it out on the verdict. The field is
`Option<BoundSource>` with `#[serde(default, skip_serializing_if =
"Option::is_none")]`, so an older serialized verdict still
deserializes, and a `None` fails safe at the export gate.

The sentry checks this the hard way: it reads `rpm` and
`safety_factor` back off the row and asserts `power_at_rpm(rpm) ×
safety_factor == available_kw`. No third number hides in the gate.

---

## 3. The export gate

`crates/rs_cam_core/src/gcode/mod.rs`, two pure public helpers:

```rust
pub fn refusing_exceedances<'a>(criteria: &'a [CriterionStatus<'a>])
    -> Vec<&'a CriterionStatus<'a>>;

pub fn enforce_exceedance_policy(
    per_toolpath: &[(ToolpathId, Vec<CriterionStatus<'_>>)],
    accept_exceeded: bool,
) -> Result<Option<String>, ExportError>;
```

`enforce_load_policy` builds the per-toolpath criteria list and
delegates. Its signature moves from `Result<(), ExportError>` to
`Result<Option<String>, ExportError>`; the `Ok` payload is the note the
caller logs. Every existing call site compiled unchanged except the two
internal ones, which now bind `let _note = …?;`.

**The plan asked for one helper and this is two.** The cause is that
`RigidityRuleOfThumb` has no producer until S3 lands the depth-of-cut
gate, so a report built from the shipped gates cannot reach the weak
arm at all. With only the filter exposed, the decision the plan
specifies — export, but report; refuse, and name both — would have had
no test until S3. The second helper is pure and takes hand-built rows,
so the rule is tested on the day it is written.

### How the message reads

Refusal (something gates, something else does not):

```
G-code export refused: tool load exceeded on toolpath(s):
  toolpath 4: power=spindle power
Exceeded without refusing the export, because the bound does not gate one:
  toolpath 4: deflection, peak 4.2000 mm, limit 3.0000 mm, from the machine
  rigidity factor 0.25 times the tool diameter 12.00 mm, which is 3.00 mm; a
  rule of thumb with no published source
Pass `accept_exceeded=true` to override (this is a known-dangerous override).
```

Nothing gates: the same second block returns as `Ok(Some(note))`, and
the export goes ahead. Nothing exceeded: `Ok(None)`.

`accept_exceeded: true` still returns the note. The override is about
danger, not about silence.

Every figure in both cases is formatted from the value it describes.
The peak comes from `display_peak`, the limit from `bound`, the
provenance from the source's own fields.

`exceeded_criteria()` is unchanged in meaning: it still reports what
exceeded, gating or not.

---

## 4. Compile-break sites

**Outside `rs_cam_core`: one.**
`crates/rs_cam_viz/src/ui/sim_diagnostics.rs:1878`, a `CriterionStatus`
literal in the test module. It is an `Unmodeled` chipload fixture, so
both fields are `None`. No layout changed, and `git status --short`
showed the file clean before the edit.

`rs_cam_cli` and `rs_cam_mcp` needed no edit. Every `PowerVerdict`
pattern in `cli/smoke.rs`, `viz/ui/preflight.rs` and
`viz/ui/sim_timeline.rs` already uses `..`.

**Inside `rs_cam_core`: eighteen**, all struct literals of
`PowerVerdict::Within` / `::Exceeds` in test modules and fixture
helpers, plus one pattern. Each took `bound_source: None` or `..`:

| File | Sites |
|---|---|
| `tool_load/verdict.rs` (tests) | 5 |
| `gcode/mod.rs` (tests) | 2 |
| `tool_load/optimize/tests.rs` | 2 |
| `tool_load/optimize/retarget/power.rs` | 2 |
| `tool_load/optimize/strategy/retarget.rs` | 2 |
| `tool_load/optimize/{delta,narrative,rank}.rs` | 1 each |
| `tool_load/optimize/strategy/{grid,headroom}.rs` | 1 each |
| `diagnostics/adapters/from_tool_load.rs` | 1 pattern, took `..` |

A fixture that sets `bound_source: None` is correct: it is a
hand-written verdict that no machine profile produced.

**No snapshot was re-blessed.** `mcp_wire_surface_pin` passes unchanged
(`2 passed; 0 failed`) because it pins MCP tool INPUT schemas, and this
step changes an output shape.

---

## 5. Where `BoundSource::clause()` could not carry the bound

The plan's sentry asked that "every `clause()` … contains the formatted
bound". Two variants cannot honour that from their own fields:

- `MachinePowerCurve` carries the rpm and the safety factor, not the
  product. It has no `MachineProfile`, so it cannot evaluate
  `power_at_rpm`.
- `DrillEnvelope` is a unit variant.

Putting the bound inside the variant would have duplicated a value the
row already carries, which is the duplication this step exists to
remove. So `clause()` states the PROVENANCE, and
`CriterionStatus::bound_clause()` formats the bound beside it and
appends the clause. The sentry asserts the digits in `bound_clause()`
and non-emptiness in `clause()`. `bound_clause()` is also the hover
string V2 needs, in one place, so GUI, CLI and MCP cannot word it
differently.

---

## 6. Where `criterion_rows()` joins the MCP report

`crates/rs_cam_viz/src/app/mcp/generation.rs:112`,
`mcp_get_tool_load_report`. It serialises `&report` into `load_report`,
which is the typed verdict array and never calls `criteria()`. The
addition is one more key beside `summary` and `load_report`:

```rust
"criteria": serde_json::to_value(
    report.per_toolpath
        .iter()
        .map(|v| (v.toolpath_id, v.criterion_rows()))
        .collect::<Vec<_>>(),
).unwrap_or(serde_json::Value::Null),
```

**Not wired in this step**, per decision 4; it is coordinated with the
other session's W5. Until it lands, an MCP client still sees three
typed verdicts and no bound, no provenance and no gantry row — S1 §7
finding 1, now unblocked.

`diagnostics/adapters/from_tool_load.rs` reads the typed verdicts too,
so `get_toolpath_diagnostics` does not carry the rows either. Same
decision, same place.

---

## 7. The sentries

Both under `crates/rs_cam_core/tests/`.

### `a_criterion_carries_its_own_bound_g_s4bound.rs` — 10 arms

The milling fixture runs `chipload::evaluate`, `power::evaluate` and
`deflection::evaluate` against a measured 12-sample trace on
`MachineProfile::shapeoko_makita()`, so every bound it reads is the one
a shipped gate judged against.

| Arm | Test |
|---|---|
| a | `every_modelled_milling_row_carries_a_bound_and_a_source` |
| a | `at_least_one_row_is_a_measured_within_with_a_positive_peak` |
| b | `the_deflection_bound_is_the_gates_own_millimetre_budget` |
| c | `the_power_bound_is_available_kw_and_names_the_rpm_and_the_safety_factor` |
| d | `the_chipload_bound_is_the_band_ceiling_and_the_source_carries_the_floor` |
| e | `every_bound_clause_is_formatted_from_the_value_it_describes` |
| e | `the_deflection_clause_formats_the_constant_it_names` |
| f | `the_gantry_row_carries_no_bound_and_no_source` |
| g | `a_drill_cycles_rows_carry_the_drill_envelope` |
| h | `criterion_rows_round_trip_and_equal_the_borrowed_criteria` |

Arm (d) uses a hand-built `ChiploadVerdict::Within` with a known band.
The measured fixture's chipload verdict depends on a vendor LUT match,
and S1's own sentry states only that "at least one" milling criterion
is modelled. A band arm keyed on a match that may not happen is a
vacuous arm.

Non-vacuity: arm (a) requires at least two modelled rows, arm (e)
requires at least two rows carrying a source, arm (h) requires at least
one owned row with a bound and one with a reading, and
`at_least_one_row_is_a_measured_within_with_a_positive_peak` is the
anchor that stops the file passing on an all-unmodelled report.

### `a_weak_bound_cannot_refuse_an_export_g_s4weak.rs` — 8 arms

| Arm | Test |
|---|---|
| a | `only_a_bound_with_a_gating_source_refuses` |
| a | `the_row_predicate_agrees_with_the_filter` |
| a | `only_the_rule_of_thumb_declines_to_gate` |
| b | `a_weak_exceedance_alone_exports_and_is_still_reported` |
| b | `a_hard_exceedance_refuses_and_the_message_names_both` |
| b | `the_override_never_hides_the_advisory` |
| b | `nothing_exceeded_says_nothing` |
| c | `the_shipped_gate_still_refuses_a_deflection_exceedance` |

Arm (c) is the anchor: it runs the shipped `enforce_load_policy` on a
real report, so the file cannot pass while the export gate has stopped
refusing.

The weak row uses `CriterionKind::Deflection` on purpose. The rule keys
on the SOURCE and never on the quantity, and the arm proves it by
putting a weak source on a kind that normally gates.

### Red-first, twice each

By construction: neither file compiled on HEAD.

```
error[E0432]: unresolved imports `rs_cam_core::tool_load::verdict::BoundSource`,
              `rs_cam_core::tool_load::verdict::CriterionRow`
error[E0432]: unresolved imports `rs_cam_core::gcode::enforce_exceedance_policy`,
              `rs_cam_core::gcode::refusing_exceedances`
error[E0560]: struct `CriterionStatus<'_>` has no field named `bound`
error[E0599]: no method named `refuses_export` found for struct `CriterionStatus<'a>`
error[E0599]: no method named `criterion_rows` found for struct `ToolpathLoadVerdict`
error: could not compile `rs_cam_core` (test "a_weak_bound_cannot_refuse_an_export_g_s4weak") due to 14 previous errors
error: could not compile `rs_cam_core` (test "a_criterion_carries_its_own_bound_g_s4bound") due to 19 previous errors
```

Defect injected once into each, after the build.

Defect 1 — `refuses_export` ignores the source
(`self.state == LoadState::Exceeds` alone):

```
failures:
    a_hard_exceedance_refuses_and_the_message_names_both
    a_weak_exceedance_alone_exports_and_is_still_reported
    only_a_bound_with_a_gating_source_refuses
    the_override_never_hides_the_advisory
    the_row_predicate_agrees_with_the_filter
test result: FAILED. 3 passed; 5 failed; 0 ignored; 0 measured; 0 filtered out
```

Defect 2 — the deflection row reads against `Some(4.0)`, which is the
GUI's `DEFLECTION_SAFE_LD_RATIO` verbatim, the exact defect this step
removes:

```
assertion `left == right` failed: the deflection row must read against the gate's own bound
  left: Some(4.0)
 right: Some(0.2)
failures:
    the_deflection_bound_is_the_gates_own_millimetre_budget
test result: FAILED. 9 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out
```

Both defects were reverted and both files went green.

---

## 8. Verification

Every command through `scripts/cargo_lane.sh`. Viz through `-j 2`.

| Command | Result |
|---|---|
| `fmt --all -- --check` | exit 0, no diff |
| `test -p rs_cam_core -q --test a_criterion_carries_its_own_bound_g_s4bound` | `ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out` |
| `test -p rs_cam_core -q --test a_weak_bound_cannot_refuse_an_export_g_s4weak` | `ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out` |
| `test -p rs_cam_core --lib -q` | `ok. 2521 passed; 0 failed; 12 ignored; 0 measured; 0 filtered out` |
| `test -p rs_cam_core -q --test an_absent_limit_is_visibly_absent_g_gantry` | `ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out` |
| `test -p rs_cam_core -q --test gate_population_vacuity_xvac` | `ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out` |
| `test -p rs_cam_core -q --test predicted_feed_gates_f035` | `ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out` |
| `test -p rs_cam_core -q --test drill_evidence_wording_d3` | `ok. 11 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out` |
| `test -p rs_cam_core -q --test gcode_phase0_capture` | `ok. 0 passed; 0 failed; 16 ignored; 0 measured; 0 filtered out` |
| `clippy -p rs_cam_core --all-targets --features heavy-tests,research,test-support -- -D warnings` | `Finished dev profile [unoptimized + debuginfo] target(s) in 24.78s` — no error, no warning |
| `check -p rs_cam_viz -j 2 --all-targets` | `Finished dev profile … in 9.12s` |
| `check -p rs_cam_cli --all-targets` | `Finished dev profile … in 9.76s` |
| `check -p rs_cam_mcp --all-targets` | `Finished dev profile … in 9.96s` |
| `test -p rs_cam_viz -j 2 -q --test mcp_wire_surface_pin` | `ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out` |
| `test -p rs_cam_cli -q` (4 binaries) | `ok. 35 passed`, `ok. 4 passed`, `ok. 9 passed`, `ok. 2 passed`; all `0 failed` |
| `test -p rs_cam_mcp -q` (2 binaries) | `ok. 31 passed; 0 failed`, `ok. 0 passed; 0 failed` |

The core full gate, the whole viz suite and `wanaka_suggest_integration`
were not run.

`rg -l "CriterionStatus|criteria\(\)|verdict_badge|pct_of_cap"
crates/rs_cam_viz/tests/` returns nothing, so `mcp_wire_surface_pin` is
the only viz target this step names.

`gcode_phase0_capture` is fully `#[ignore]`d on a plain run; per
`tests/CLAUDE.md` that marks an instrument, not a break.

**One interruption worth recording.** Midway through, `cargo check -p
rs_cam_core --all-targets` reported two red targets,
`optimize_toolpath_is_a_job_wp14b` and
`feed_optimization_refusals_wp18`, with `missing field epoch in
initializer of AdoptSimulationArgs`. That is another session's
in-flight W0c work in `session/`, not this step. It landed at
`92c25308` before the clippy gate ran, and the gate is clean.

---

## 9. What the plan did not anticipate

1. **`ExceededCriterion` holds `&'static str` fields**, so its derived
   `Deserialize` impl only exists for a `'static` input. `CriterionRow`
   contains one, so it inherits the bound; the struct states it as
   `#[serde(bound(deserialize = "'de: 'static"))]` to keep the error at
   the definition rather than at every call site. Serializing is
   unaffected, so the MCP server side in §6 is unaffected. A Rust
   client deserializing a row needs a `&'static str` of JSON; the
   sentry leaks one small string to exercise the real wire path. Worth
   knowing before W5 designs a round trip.
2. **Nothing gates on a rule of thumb yet, so the weak arm has no
   producer.** This is why §3 has two helpers rather than one. The day
   S3 lands `DepthOfCut` with `RigidityRuleOfThumb`, the end-to-end arm
   becomes reachable from a real report and should be added there.
3. **The refusal loop for UNMODELED criteria still enumerates the
   three typed verdicts by hand** (`gcode/mod.rs`, the
   `!policy.accept_unmodeled` branch). S1 §7 found it; S4 did not fix
   it, because its file set covers the EXCEEDED branch. The exceeded
   branch now derives from `criteria()`, so the two halves of one
   function disagree about where a criterion comes from. That is a real
   inconsistency and a good next small job.
4. **`crates/rs_cam_core/src/tool_load/CLAUDE.md` still sits at its
   40-line ceiling**, so the two new sentries are not named there. S1
   left the same note. The file is outside this step's set.

No `planning/TECH_DEBT_REGISTER.md` row was added: items 1 and 2 are
constraints rather than defects, and item 3 is already recorded in
`S1_IMPLEMENTATION.md` §7.

---

## 10. Files changed

- `crates/rs_cam_core/src/tool_load/verdict.rs`
- `crates/rs_cam_core/src/tool_load/power.rs`
- `crates/rs_cam_core/src/tool_load/drill_gates.rs`
- `crates/rs_cam_core/src/gcode/mod.rs`
- `crates/rs_cam_core/src/diagnostics/adapters/from_tool_load.rs`
- `crates/rs_cam_core/src/tool_load/optimize/delta.rs`
- `crates/rs_cam_core/src/tool_load/optimize/narrative.rs`
- `crates/rs_cam_core/src/tool_load/optimize/rank.rs`
- `crates/rs_cam_core/src/tool_load/optimize/retarget/power.rs`
- `crates/rs_cam_core/src/tool_load/optimize/strategy/grid.rs`
- `crates/rs_cam_core/src/tool_load/optimize/strategy/headroom.rs`
- `crates/rs_cam_core/src/tool_load/optimize/strategy/retarget.rs`
- `crates/rs_cam_core/src/tool_load/optimize/tests.rs`
- `crates/rs_cam_core/tests/a_criterion_carries_its_own_bound_g_s4bound.rs` (new)
- `crates/rs_cam_core/tests/a_weak_bound_cannot_refuse_an_export_g_s4weak.rs` (new)
- `crates/rs_cam_viz/src/ui/sim_diagnostics.rs`
- `planning/load_model_2026-09-16/S4_IMPLEMENTATION.md` (this file)

Not committed.
