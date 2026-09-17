# Design and feature-debt audit — synthesis

Source: `BRIEF.md` and the twelve group files in this directory.
135 findings, twelve groups, one read-only day (2026-09-17).

This file ranks across groups. It adds no finding. Every id cited exists in a
group file. Where two groups report one thing, the row cites both ids.

Read order for an orchestrator: `## Defects`, then `## Ranked programme`,
then `## Waves`. The themes explain why the rows repeat. The verification
list is the first command of wave 1.

## Defects

Twenty rows, twenty-two finding ids. Each row describes wrong behaviour
today: a guard that reports the wrong thing, a silent coercion, a dial two
surfaces advertise that nothing reads, or a metric a consumer loses. They
outrank every design item below. The order is the harm to an operator.

- CMP-14 a holder-collision check that FAILED reports zero collisions — `sim_triage` filters on `> 0`, so a toolpath nobody checked reads as a clean one on the question that wrecks a machine.
- EDG-06 MCP export discards the machine-safety findings the GUI shows — an agent that exports G-code with `RapidBelowClearance` or `ZBelowProgramFloor` at `Severity::Error` sees the same success text as a clean export.
- CUT-08 `apply_tabs` rewrites tagged cut moves into `MoveIntent::Unknown` — a tabbed finishing profile leaves the spacing instrument entirely, and two fed vertical descents ship untagged past every intent-keyed reader.
- CMP-15 a project file with an unknown `face_up` loads silently as `Top` — one typo turns a bottom setup into a top one and moves the cut direction, the local stock bbox and the whole emission frame.
- SES-07 a tool edit or a model refresh drops only the directly bound results — a `FromRemainingStock` or `DerivedRestRegions` toolpath downstream of the edited tool keeps a stale result under new inputs, against the folder's own stated invariant.
- UI-08 a Post-tab edit never marks the project dirty — the operator changes Safe Z or the post format, closes the window, and loses the change with no prompt.
- FDS-01 `SuggestContext::stock` is `None` at all ten mutating call sites — the `AlignmentPinDrill` peck clamp against spoilboard penetration cannot fire on any GUI, MCP, CLI or multitool path.
- CUT-05 + CLI-01 two stay-down dials differ only by a unit suffix, and the reachable one is not the one that is read — the CLI accepts `max_stay_down_dist`, coalesces it into the other key with `.or()`, and the core field of that name is `None` at its one production construction site.
- CUT-03 + CMP-17 `retract_strategy` is offered by a GUI dropdown and by the MCP description, saved to the project file, and read by nothing — the operator believes the retract is minimal and gets the retract they did not ask for (G-RETRACTDIAL).
- TLD-05 plunge stress is called a milling gate and cannot refuse an export — a flagged plunge rate reaches `export_gcode`, because the check never joins `enforce_load_policy` the way chipload, power and deflection do.
- FIN-14 only the `VerySteep` band can report a height clip — a clipped `MidSteep` or `Shallow` region reads as clean, not as not measured.
- STK-08 `peak_chip_thickness_mm` publishes `None` for a measured zero — one struct then carries two contracts, against its own doc that says `Some(0.0)` means measured and zero.
- EDG-01 three of eleven `WoodSpecies` arms return folklore Kc in the same `Some(..)` shape as the FPL-cited species — no gate and no operator can tell a citation from a guess.
- CMP-08 `set_toolpath_param` accepts four names the published schema omits — `get_operation_schema` is wrong in both directions for Waterline, RampFinish and Pencil, and the refusal message lists the wrong valid set.
- CLI-05 `sweep` reports the literal string `"default"` as the baseline for 20 of 42 job fields — the artifact an agent reads to learn what the sweep varied FROM states a value that was never read.
- TLD-03 the export refusal an operator sees for an unmodeled gate is a raw `Debug` tag — `chipload=SteadyStateSamplesNotPresent`, where the diagnostics adapter has already written the sentence.
- STK-04 `leading_edge_speed_mm_min` is an unconditional copy of `feed_rate_mm_min` — the field's own doc names a chipload consumer that does not exist, so the name promises an edge speed the emitter never computes.
- FIN-04 `route_width_factor` is still deserialized, still recorded as a deprecated dial, and steers nothing — and the recorder suppresses itself at the default, so an operator at defaults is told nothing.
- CMP-27 alignment-pin keying is validated on the GUI load path only — the CLI's headless load and MCP `load_project` hear nothing when a pin pair is invariant under the wrong symmetry, and both orientations then look right.
- STK-06 two open-coded `FaceUp` maps report a lateral setup as `FromTop` where the canonical six-arm accessor reports the lateral variant — the same table that has already produced two recorded defects.

Considered and not listed. CLI-08 (three tool-type vocabularies) rejects the
wrong token loudly, so it is authoring friction, not wrong behaviour; it is
ranked below. CMP-09 (`scallop_height` has a GUI route and no `ParamDef`),
CMP-25 (no stale-default row in the CLI) and STK-05 (`EngagementDirection`
only ever says `Mixed`) are surface-parity and half-built-capability gaps;
they are ranked, not called defects.

## Cross-cutting themes

Nine patterns appear in three or more groups. Each theme names its findings
once and states the one fix that closes the theme, not the instance.

### T1 One concept, two structs, and a hand-written copy between them

FIN-02, FIN-03, FIN-13, SES-01, SES-02, UI-07, SHL-01, CMP-16, CMP-19,
CMP-21, TLD-04, STK-03 + STK-07, FLD-01, CLI-03.
Groups: finish, session, viz-ui, viz-shell, compute, tool_load, stock,
fields, cli-mcp.

The crate keeps a Config beside a Params, a core type beside a viz mirror,
and a session field beside a GUI copy. A field added to one side compiles
and drops out at the hop. The evidence is already in the tree: CMP-12 shows
two re-export lists that diverged by two names, CMP-19 shows three fields
that drifted across one translation, and SHL-01 names a shipped regression
(W9/P-1 reset grblHAL to GRBL on reload). **What one fix looks like:** adopt
one rule for every boundary — the owning type crosses it, and a translation
exists only where the two sides genuinely differ. Where a translation must
stay, write it as an exhaustive destructure with no `..` and build the
result with no `..Default::default()`, the way `stats.rs:162-238` already
does. A new field is then a compile error at the hop instead of a silent
`None`. One sentry per boundary that asserts the two field sets agree
finishes the theme; the boundaries that cannot be typed away (session to
project file, SES-02) get that sentry and nothing else.

### T2 Enum dispatch replicated once per surface

UI-05, SHL-02, FLD-03, CLI-08, CMP-03, CMP-04, CMP-07, CMP-01, STK-02,
CUT (the add-a-2.5D-operation list), TLD (the add-a-gate list), UI-06.
Groups: viz-ui, viz-shell, fields, cli-mcp, compute, stock, cutting,
tool_load.

One new operation costs 20 production files across three crates
(core-compute's count), and five of the twenty are silent on omission.
One new MCP command costs five match blocks in one viz file plus three more
files. One new finishing strategy costs 27 non-test files.
**What one fix looks like:** the crate already owns the answer and applies
it in one place. `for_each_op!` generates eight surfaces from one row list,
and `OpRegistryEntry` carries five per-op decisions as data. The theme
closes when every per-op and per-command decision is a column in that row
rather than a match somewhere else — CMP-07 names the eight that stayed
outside, three of which fail OPEN on a new variant. The same table shape
then serves viz (UI-05) and the MCP surface (SHL-02). The test is
mechanical: for every `OperationType::ALL`, assert the row names a value for
every column, and delete each wildcard arm as its column lands.

### T3 Dials nothing reads, and dials no surface sets

CUT-03 + CMP-17, CUT-05 + CLI-01, CUT-15, FIN-04, STK-04, STK-05, FDS-02,
FDS-05, CMP-09, FLD-04, FLD-05, TLD-10.
Groups: cutting, compute, finish, stock, feeds, fields, cli-mcp, tool_load.

Three kinds sit under one name. A dial a surface offers and no code reads
(`retract_strategy`). A dial code reads and no surface sets
(`max_stay_down_dist`, `classification_sampler`, `DepthDistribution::Constant`).
A value computed on every call that nothing consumes (`CutterOpProfile::predictions`).
**What one fix looks like:** one sweep with one rule — every config field
either appears in a published `ParamDef`/field table and has a reader, or it
is deleted in the same commit. CMP-10 is the mechanism: a test that
serializes `OperationConfig::new_default(op)` for every op and compares the
key set against `param_names_for_type(op)`, with a named allow-list carrying
a reason per exemption. Extend the same test to `DressupConfig` (CMP-17,
which has no table at all) and the theme cannot grow back. The operator
ruling of 2026-09-16 means the answer to an inert dial is deletion, not a
deprecation shim.

### T4 Measured zero against not measured

STK-08, CMP-14, EDG-01, FIN-14, CLI-05, EDG-06, CMP-05, STK-05.
Groups: stock, compute, edges, finish, cli-mcp.

`diagnostics/CLAUDE.md` states the contract: `None` means not measured,
`Some(0.0)` means measured clean. Six findings break it in both directions.
A failed collision check becomes zero. A real zero chip peak becomes `None`.
A folklore constant returns the same `Some(..)` shape as a cited one. An absent `ParamRange`
means "not measured, not unbounded", and 238 of 243 params are absent.
A validator's findings are computed and dropped, so the export reads clean.
**What one fix looks like:** make the type carry the third state everywhere
the contract already claims it. `Result` at the producer, `Option` at the
carrier, and no `unwrap_or(0)` between them. Commit `70a3db27` fixed exactly
this shape in the CLI on the audit day — `collision_count: Option<usize>`
plus a separate `collision_checks_failed` — and that commit is the reference
for CMP-14, STK-08 and FIN-14 alike. One sentry per producer: feed it an
input it cannot measure, and assert the output is not a zero.

### T5 Stringly-typed doors

FIN-09, UI-04, UI-06, UI-07, STK-10, CMP-15, CMP-08, CLI-09, CLI-01,
CLI-08, FLD-03.
Groups: finish, viz-ui, stock, compute, cli-mcp, fields.

A kind, a field, an operation or a tab travels as a `String` where an enum
exists two files away. Three of them fail open: `PencilDetector::parse` sends
an unknown token to `Dihedral`, `FaceUp::from_key` sends it to `Top`, and
`tooltip_for` keys sixty arms on a visible label, so a label rename drops a
tooltip and an automation hook in silence.
**What one fix looks like:** parse once, at the boundary the string actually
arrives at, and carry the enum inside. Every enum on that boundary derives
its serde token and its `schemars` schema from one list, so the published
MCP schema enumerates the legal values instead of describing them in prose
(CLI-09). A parse that cannot name the token returns `Option`, and the
caller decides whether to refuse (the MCP rule) or to warn and default (the
loader rule, `tool_config.rs:82-86` Q4). The two rules already exist; the
doors that fail open simply do not follow either.

### T6 Wire vocabulary drift across the four surfaces

CLI-08, CLI-09, CMP-03, CMP-17, UI-06, UI-07, FLD-03, CMP-08.
Groups: cli-mcp, compute, viz-ui, fields.

The same five tool shapes have three spellings (`end_mill` / `flat`,
`bull_nose` / `bullnose`). The valid inspector-tab list is written twice, in
`ToolpathTab::parse` and again in the MCP refusal message. The dressup field
list is a prose string that omits six of 23 fields. `kind_str()` is a
hand-written copy of the derived serde name.
**What one fix looks like:** one token table per vocabulary, generated, and
every surface reads it — the parser, the published schema, the refusal
message and the help text. CMP-03 shows the generator already exists
(`for_each_op!`); the token is simply not a column yet. The sentry is the
one that already works for `ToolType`: assert the canonical token, the
parser and the published list agree for every variant.

### T7 God functions that already carry their own numbered seams

FIN-07, SHL-04, SHL-03, TLD-09, FLD-02, SES-04, SES-06, CMP-18, CMP-20 +
CUT-06 + CUT-11, CLI-07, CUT-09, UI-01, UI-12, STK-01.
Groups: finish, viz-shell, tool_load, fields, session, compute, cutting,
cli-mcp, viz-ui, stock.

Every one of these functions names its own steps in comments and then does
not cut on them. `detect_rest_valleys` numbers seven phases in 534 lines.
`unified_finish_toolpath_with_cancel_and_ceiling` numbers six steps in 1 150.
`optimize_toolpath_inner` numbers ten in 296. `drain_compute_results` runs
636 lines and three of its six arms already delegate to a named method.
**What one fix looks like:** no single refactor closes this theme, so the
close is a convention plus a gate. The convention: a numbered comment is a
function boundary, and the accumulators the steps share become one named
context struct instead of a destructured list restated at the snapshot site
(CMP-18 restates sixteen by hand). The gate: a source scan with a named
allow-list, in the style of `panels_read_the_token_module_up1`, that fails
on a new production function over 200 lines. Without the gate the count
grows back, which is what `section.rs:41` records happening to UI-03.

### T8 Test-only, dead and research `pub` surface in the product build

FIN-01, FIN-05, FIN-06, FIN-11, FIN-15, CUT-02, FLD-04, FLD-05, EDG-05,
TLD-10, TLD-08, TLD-06, SES (two checked-clear rows), FDS (four labelled
test doors).
Groups: finish, cutting, fields, edges, tool_load, session, feeds.

Three kinds again. A `pub` item only tests call (eight dressup wrappers,
five finish wrappers, four cache-stats accessors). A `pub` item nothing
calls (`GrblImport`, `MoveKinematics`, `WindingReport`, `RoutedLink`).
And 7 431 lines of measured research — `conformal_spiral`, `direction_field`,
`spiral_finish_compact` — compiled into every product build, 22 % of
`finish/`.
**What one fix looks like:** one convention with three tiers, applied
crate-wide. A product item is plain `pub`. A test seam is
`#[cfg(any(test, feature = "test-support"))]` and says so in its doc — the
convention `feeds/` already applies consistently and the other folders apply
by accident. A research arm sits behind a `research` cargo feature, with its
harnesses in `required-features`. The theme then has a sentry: a test that
no `src/` file outside the research set names a research module, in the
style of the existing arch guards.

### T9 A capability that reaches three surfaces of four

SES-05, CLI-03, CLI-06, CMP-23, CMP-25, CMP-27, EDG-06, CMP-09, FLD-04.
Groups: session, cli-mcp, compute, edges, fields.

Fixtures, keep-out zones and plan reorder exist as `Command` rows the GUI
reaches and neither MCP nor the CLI can. The multitool ladder planner is
live in core, GUI and MCP and absent from the CLI. `nc-time` hardcodes one
machine profile while a shared machine library exists. The S5 prefix memo
reaches one crate, and the CLI's own fixpoint ladder pays the 84 s the memo
was built to remove. Pin-keying validation, stale-default rules and export
findings each reach one or two surfaces.
**What one fix looks like:** the crate already has the instrument —
`command.rs`'s `Surfaces { gui, mcp, cli }` rows with a `Reach::Skip` reason
per surface, guarded by `command_registry_completeness`. The theme closes
when every capability, not only every `Command`, carries that declaration:
each diagnostic, each validator, each memo door states its reach and its
reason. A `Skip` with a reason is a decision; a `Skip` with no row is this
theme. Then the gaps that matter (workholding from an agent, export findings
on the wire) are visible as rows rather than found by audit.

## Ranked programme

Twenty-five rows, ranked by benefit per effort, defects first. Rows group
findings one commit would naturally land together. Power-session rows appear
here only because they are defects; the rest of that owner's work is in the
second table.

| # | id(s) | title | effort | risk | breaks | owner | sentry |
|---|---|---|---|---|---|---|---|
| 1 | CMP-14, CMP-24 | A failed collision check is not a clean one; build one index per model | S | low | yes — `ProjectEvidence::holder_collisions` and `SimTriageInputs` change type | | core twin of `a_failed_collision_check_is_not_a_clean_one_g_colfail` |
| 2 | EDG-06 | MCP export returns the machine-safety findings it already computes | S | low | no — additive text | | new: an MCP export that trips `Severity::Error` names it |
| 3 | CUT-08 | `apply_tabs` carries the move intent, and tags the two re-entries | S | low | no | | `retract_intent_move_type_census_w6` + a tabbed profile case |
| 4 | CMP-15 | `FaceUp::from_key` and `ZRotation::from_key` return `Option`; the loader warns | S | low | yes — two `pub fn` signatures | | `face_up_names_follow_drafting_convention_g_frontname` + a `face_up = "topp"` load |
| 5 | UI-08 | A Post-tab edit marks the project dirty | S | low | no | | new: apply a post edit through the panel, assert `gui.dirty` |
| 6 | CUT-03, CMP-17 | Delete `retract_strategy`; publish the dressup field list from a table | S (delete) / M (table) | low | yes — a project key, the MCP wire snapshot | | `adaptive3d_post_tsp_z_monotonicity`, `mcp_wire_surface.json` re-bless, new `dressup_field_names_are_published` |
| 7 | CUT-05, CLI-01 | One stay-down dial: delete `max_stay_down_dist`, drop the TOML alias | S | low | yes — a job-file key | | `adaptive3d_keep_down_link_f038b` + a job alias test |
| 8 | SES-07 | Tool and model invalidation walk the same chain every other path walks | M | medium | no — `Effects::stale` only grows | | fourth arm in `mutation_paths_invalidate_alike_p0` |
| 9 | FDS-01 | Every write path populates `SuggestContext::stock` | S | low | no | power session | new: add an `AlignmentPinDrill` through the MCP path, assert the peck clamp fired |
| 10 | STK-08 | The chip-peak accumulator reports `Some(0.0)` for a measured zero | S | low | no | | `measurability_abstention_r8` + a measured-zero case |
| 11 | CMP-08, CMP-09, CMP-10 | Publish the four accepted names; one test that `param_defs` covers the struct | S | low | yes — `get_operation_schema` gains rows | | `set_param_refuses_absent_field_n5` + new `param_defs_cover_every_config_field` |
| 12 | STK-04, STK-05 | `leading_edge_speed_mm_min` and `EngagementDirection`: earn the name or delete | S | low | yes — two wire keys, one golden field | | `engagement_vector_step2` |
| 13 | CLI-05 | `sweep` reports the field's real baseline or refuses | S | low | no | | new: sweep an uncovered field, assert `base_value` is not `"default"` |
| 14 | TLD-03 | One `UnmodeledReason::operator_message`; the export refusal is a sentence | S | low | no — refusal wording only | power session | `enforce_load_policy` tests in `gcode/mod.rs` |
| 15 | FIN-04, FIN-09 | Pencil config: delete `route_width_factor`, type the detector | S | low | yes — two project-file keys | | `finish/pencil/tests.rs` + a refused-token load test |
| 16 | CMP-27 | Pin keying runs on the core load path and publishes a `Diagnostic` | S | low | no — an added diagnostic row | | `alignment_pin_keying_g_pinauto` + a loaded-session case |
| 17 | STK-06 | Name the `FromTop` group rule in one function; delete the two open-coded maps | S | medium — the S5 key reads this value | no | | `cut_direction_matches_transform_g_lateralsign` |
| 18 | FIN-14 | The clip finding carries `bands_measured`; absence stops reading as clean | M | medium | yes — a diagnostic shape | | `unified_finish/tests.rs` + a clamped `MidSteep` case |
| 19 | EDG-01 | Folklore Kc carries its own provenance tag | M | medium | no — the values do not move | | `wood_species_library_provenance` + a per-arm provenance case |
| 20 | TLD-05 | Plunge stress: a fourth `MetricEvaluator`, or a documented side path | S (doc) / M (gate) | medium | yes for the gate — a field on `ToolpathLoadVerdict` | power session | new, beside `gate_population_vacuity_xvac` |
| 21 | CMP-01, CMP-26, CMP-16, CMP-22 | Four compute deletes and guards: dead fallback, dead ProjectCurve block, derived rest defaults, destructured S5 key | S | low | yes — `OpRegistryEntry::generate` field type | | `catalog/tests.rs`, `normalize_for_op_applies_registry_policy`, `sim_prefix_memo_s5`, new `rest_analysis_config_defaults_match_the_detector` |
| 22 | CLI-08, CLI-09 | One tool-type vocabulary; `z_rotation` is an enum on the wire | S | low | yes — job TOML tokens, a lenient MCP parse | | job `ToolType::ALL` round trip + the `set_setup_rotation` schema test |
| 23 | UI-06, UI-13 | The pending tab is a `ToolpathTab`; delete the `feeds_modal` alias | S | low | yes — `GuiState` field type only | | `controller/tests.rs:519` extended over `ToolpathTab::ALL` |
| 24 | EDG-03, EDG-04, EDG-05 | DXF/SVG size guard, one point-to-segment helper, three dead `pub` types | S | low | yes — two error enums gain a variant | | `step_import` as the model, `spacing_prize_split_f1`, `kinematics_per_axis_rate_p1` |
| 25 | SHL-01, SHL-05 | One door rebuilds the post mirror; one keyed-rebuild helper for six uploads | S | low | no | | `every_post_format_survives_the_session_round_trip`, `render_pipelines_headless_g_pipesmoke` |

### Power session — owned by the concurrent feeds/tool_load session

These are tagged `owner: power session` in their group files and are excluded
from the top 25 because they are not defects. No row proposes a change to a
constant, a band, a threshold or a gate number.

| id(s) | title | effort | risk | breaks | sentry |
|---|---|---|---|---|---|
| TLD-01 | One `locality` walk feeds all three gates; delete three hand-rolled filter loops | M | medium | no | `gate_population_vacuity_xvac`, `predicted_feed_gates_f035` |
| TLD-02 | The band-admit breach predicate moves onto `ChiploadVerdict` | S | low | no | `one_ulp_above_the_ceiling_does_not_demote_the_tier` |
| TLD-04 | Delete `DeflectionSetupDetail`; `OutcomeNarrative` holds the prescription | S | low | yes — an `OutcomeNarrative` field type | `orchestration_skip_tests.rs:357` |
| TLD-06 | One `verdict_fixtures` module for fifteen copied test builders | S | low | no — test-only | the five files' own unit tests |
| TLD-09 | Cut `optimize_toolpath_inner` on its ten numbered seams | M | low | no | `optimize/tests.rs`, `stage1_grid_tests.rs` |
| TLD-10 | `GatePopulation::filtered_out` behind a test-support cfg | S | low | yes — the sentry's import path | `gate_population_vacuity_xvac` |
| TLD-07 | `AxisPolicy` / `RankingPolicy` field counts — recorded, no action asked | L | low | none | n/a |
| TLD-08 | `wanaka_e2e_chipload_gate` is `#[ignore]`d on O3/O4 — needs an owner and a date | S | low | none | the test itself |
| FDS-02 | `CutterOpProfile::predictions` / `.constraints`: wire them in or retire them | S | low | yes — a `pub` struct shape | `feeds/profile.rs` module tests |
| FDS-03 | One named `bounds()` conversion across the three chipload-band types | S | low | no — additive | `lookup_parity`, the two modules' own tests |
| FDS-04 | Drop the `LookupCriteria` alias; one name for `LookupQuery` | S | low | yes — a type alias | `lookup_parity` |
| FDS-05 | `GeometryClass::{ShallowTerrain, SteepTerrain}` unreachable — documented placeholder, no action | S | low | none | `geometry_class.rs` module tests |

### Below the line

Findings the waves schedule after the top 25. Every id the `## Waves`
section names appears in one of these three tables; the remaining findings
stay in their group files until a later pass.

| id(s) | title | effort | why not top 25 |
|---|---|---|---|
| CLI-07 | Extract the 382-line `Commands::Job` arm into `job.rs` | S | pure code motion; no defect behind it |
| CUT-02 | Delete the eight plain dressup wrappers only tests call | M | ~40 test call sites move with it |
| CUT-12 | Rename `adaptive3d/search.rs` to `material.rs`; fix both file maps | S | a navigation fix, not behaviour |
| CUT-01 | One corner-blend walk; `blend_corners` linearises the other | S | internal duplication, no surface effect |
| CUT-15 | `DepthDistribution::Constant` and `finish_allowance`: expose or delete | S | needs the operator's answer on the documented 17 % staircase |
| FLD-03 | `ToolpathSemanticKind::ALL` and `ordinal()` | S | additive; the viz colour ramp follows in the tail |
| FLD-04, FLD-05 | Narrow the four cache-stats accessors and four test-door APIs | S | visibility only |
| FIN-11, FIN-15 | Five `pub` wrappers and two `pub` items with no product reader | S | visibility only |
| SES-01, SES-03 | Derive the four `Serialize` impls; move the diagnostic types out of `mod.rs` | S | ~440 lines of pure motion |
| SHL-04 | Extract three `ComputeMessage` adopt methods | M | finishes a pattern three arms already follow |
| CLI-04, CLI-06 | One bounded-response vocabulary; `nc-time --machine` | S | surface parity, no wrong reading |
| UI-01, UI-11 | The 25-argument panel into its existing snapshot; one drill-cycle helper | M / S | the snapshot change breaks the sentry-facing signature |
| FIN-01, FIN-02, FIN-07 | Research feature gate, config-to-params methods, cut the 1 150-line orchestrator | M / M / L | FIN-07 shares a dozen accumulators; do it after FIN-02 |
| FIN-05, FIN-06, FIN-12, FIN-13 | Split the `#[ignore]` harness, move the fixture out of `planning/`, return a `PencilReport`, merge the band findings | S–M | test hygiene and one diagnostic shape |
| STK-13, STK-02 | Delete the X and Y dexel grids and two of three constructors | M | compiler-checked deletion, but it must land after row 17 |
| STK-14, STK-11, STK-10 | Move the ribbon and colour ramps to `export/`; `Arc<[SpanId]>`; typed collision segment | S / M / M | `stock_mesh.rs` breaks four viz imports |
| CMP-12, CMP-21, CMP-28 | One config re-export list; move the finding family; split `stock_config.rs` | M each | each re-paths two crates; they cannot share a wave |
| CMP-19 | Delete the three viz mirror types; build the core `SimulationRequest` directly | M | medium risk; `build_playback_data` moves with it |
| CMP-02, CMP-18, CMP-20 + CUT-06 + CUT-11 | The 20-argument dispatch, the 609-line sim loop, the 15-argument dressup driver and its stage table | M / L / M | CMP-20 and CUT-06/CUT-11 are one change filed by two groups |
| CMP-03, CMP-04, CMP-06, CMP-07, CMP-11 | Generate `kind_str`, name every `cutting_levels` arm, macro the 24 accessor blocks, move three `matches!` onto the registry, fix the two-driver docs | S–M | they are the T2 theme; land them as one programme |
| SHL-02, SHL-03, SHL-06 | The MCP command table, the 6 736-line test file, the five lane prologues | L / M / M | SHL-02 is advised against as a blind rewrite |
| UI-02, UI-03, UI-04, UI-05, UI-07, UI-09, UI-10, UI-12 | The component-kit sweep, the tooltip key, the per-op draw table, the post draft, the temp-memory drafts, the raw `DragValue` sites, the 23 long draw functions | M–L | one visual programme; needs the kit sentries first |
| FLD-01, FLD-02, FLD-06 | Embed `GridSpec`; cut `detect_rest_valleys` on its seven phases; type the mesh error | M / M / S | FLD-01 re-paths viz overlays and two MCP handlers |
| CUT-04, CUT-09, CUT-13, CUT-14 | Split `Adaptive3dParams`, cut `clear_z_level_agent_2d_slice`, `Option<Params>` dressups, one Z ladder | L / L / M / M | each moves emitted motion or needs a byte-compare rig |
| STK-01, STK-03 + STK-07, STK-09, STK-12 | One stamping-cell driver, four duplicated scalars, one band dispatcher, one triage list | L / S–M / L / L | the hot loop and the operator-visible list; see `## Operator rulings — 2026-09-17 evening

- **FIN-01** approved: feature-gate the research arms. Runs in wave 1 as a
  seventh agent (finish folder, core `Cargo.toml`, six test targets).
- **FIN-08** approved with a condition: the unified planner must keep a
  plain two-band steep/shallow mode as a selectable preset, so an operator
  can still ask for just steep and shallow. Paired A/B before the swap.
  Scheduled after wave 3, not in wave 1.
- **STK-13** not ruled; the operator asked whether the deletion limits future
  code. Answer on record: yes for one future — a single global stock that
  shows every face's cuts needs the side grids. Recommendation: keep the
  engine capability, land STK-02 (one constructor) and note in
  `dexel_stock/CLAUDE.md` that the side grids are reachable from tests only.
- Feeds and tool_load rows stay with the power session.

## Not now` |
| SES-04, SES-06 | Cut `execute_job` and `plan_multitool_finishing` on their named phases | M each | every generated toolpath rides `execute_job` |
| CMP-23, CMP-25 | A memo door for the CLI ladder; a stale-default row in the CLI JSON | M / S | CMP-23 needs the `SimulationOptions` decision first |
| EDG-02, EDG-07 | Wood species library as data; one junction-walk integrator | M each | EDG-07 is the load-bearing cycle-time model |
| CUT-07, CMP-13, FIN-03, FIN-10, CLI-02, CLI-03 | Name the three dressup bypasses in the folder file; name the operation in the two geometry refusals; split `UnifiedFinishConfig` three ways; one entry point per finish family; the 42-field job `OperationDef`; a CLI door to the multitool planner | S–L | CUT-07's doc half and CMP-13 are S and are scheduled in wave 1 and wave 5; the other four are scope decisions, not cleanups |

## Waves

Five waves. One folder group per agent per wave. `crates/rs_cam_core/src/compute/`
is the hub: almost every row touches it, so the wave contract is file-level as
well as folder-level — **no file appears twice in one wave**, and each wave note
names the files an agent owns outside its own folder. Run the
`## Verification list` before wave 1 starts.

### Wave 1 — defects that are one folder deep, plus the S-effort deletes

Six agents, no shared file. No viz-shell agent runs in this wave, which is
what lets two of the six reach into viz.

| agent | rows | owns outside its folder |
|---|---|---|
| core-cutting | CUT-08, CUT-02, CUT-12, CUT-07 (doc) | `compute/execute/dressup_apply.rs` (the import line CUT-02 renames) |
| core-edges | EDG-01, EDG-03, EDG-04, EDG-05 | — |
| core-fields | FLD-03, FLD-04, FLD-05 | — (the viz colour ramp waits for wave 5) |
| core-stock | STK-08, STK-04, STK-05 | `rs_cam_viz/src/app/mcp/simulation.rs:1144`, `tests/perf_golden_sim_metrics.rs` |
| viz-ui | UI-08, UI-13 | `rs_cam_viz/src/app.rs:894` |
| cli-mcp | CLI-05, CLI-07 | — |

Folder `CLAUDE.md` invariants touched:
- `dressup/` — "driven from `compute/execute/dressup_apply.rs`, never called
  ad hoc" is false today; CUT-07 names the three exceptions or folds them in.
  The same file already records "the retract-strategy dial is dead
  (G-RETRACTDIAL)" that wave 3 acts on.
- `adaptive3d/` — the file map row for `search.rs` is wrong; CUT-12 corrects
  it. The folder's own warning that this engine emits untagged vertical
  descents is the class CUT-08 fixes on the 2.5D side.
- `material/`, `io/` — `io/CLAUDE.md` names the two on-disk catalogues; EDG-01
  and EDG-02 are the third catalogue that is code, not data.
- `maps/` — "a stale cache key is a live bug class"; `compute_reach_map` is
  named canonical and FLD-05 stops `reach_map_for_mesh` from looking like a
  second entry point.
- `trace/` — `SpanKind::ALL` is the contract FLD-03 extends to its sibling.
- `stock/`, `dexel_stock/`, `diagnostics/` — "`None` means not measured.
  `Some(0.0)` means measured clean" is the line STK-08 restores.
- `ui/properties/` — the panel doors that call `mark_edited()` on success;
  UI-08 is the one that does not.
- `crates/rs_cam_cli/CLAUDE.md` — CLI parity and replay/sweeps.

### Wave 2 — the compute and session defect cluster

Four agents. The core-compute agent owns every `session/` and `stock/` line
its rows need, so no core-session and no core-stock agent runs. `compute/` is
shared by two agents at file level only, and the split is named below.

| agent | rows | owns outside its folder |
|---|---|---|
| core-compute | CMP-14 + CMP-24, CMP-15, CMP-08 + CMP-10 + CMP-09 (`scallop_height` half), CMP-27, CMP-01, CMP-26, CMP-16, CMP-22 | `session/compute/diagnostics.rs`, `session/compute/params.rs`, `session/project_file.rs`, `session/mod.rs` (the `ProjectEvidence` type), `stock/sim_triage.rs` |
| viz-shell | EDG-06, SHL-01, SHL-04, SHL-05 | — (all of it is viz-shell) |
| core-finish | FIN-04, FIN-09, FIN-11, FIN-15, CMP-09 (`classification_sampler` half) | `compute/operation_configs.rs`, `compute/execute/finish_3d.rs`, `compute/execute/findings.rs` — the compute agent touches none of these three |
| core-tool_load (power) | TLD-03, TLD-05, TLD-01, TLD-02, TLD-04, TLD-06, TLD-09, TLD-10 | `gcode/mod.rs`, `diagnostics/adapters/from_tool_load.rs` |

Folder `CLAUDE.md` invariants touched:
- `compute/` — `config.rs` is described as "the configuration model"; CMP-16
  and CMP-26 stay inside that description, CMP-21 (wave 5) fixes it.
- `session/` — "a parameter, tool, model, stock or setup edit invalidates the
  affected cached result chain"; the compute agent must not weaken it while
  editing `session/compute/*`.
- `tool_load/` — "the four milling gates" against a module doc that says
  three criteria; TLD-05 settles which sentence is true. "Never write a second
  copy" is already broken three ways and TLD-01 closes it.
- `crates/rs_cam_viz/CLAUDE.md` and viz `compute/` — "a result arrives with
  the revision it was computed for"; SHL-04 must preserve the per-arm revision
  check while extracting the three adopt methods.
- `diagnostics/` — TLD-03's sentence is the adapter's, not a new one.

### Wave 3 — the cross-surface rows

Four agents. No compute agent and no viz-shell agent run, so the cutting
agent owns the whole CUT-03 + CMP-17 row — core, GUI and MCP description —
in one pathspec commit. Two files of that row go elsewhere, because another
agent already edits them: `rs_cam_mcp/src/server.rs:928` to cli-mcp and
`session/mod.rs:2418` to core-session. `app/mcp/` is shared by two agents at
file level only: cli-mcp takes `commands.rs`, viz-ui takes `view.rs`.

| agent | rows | owns outside its folder |
|---|---|---|
| core-session | SES-07, SES-01, SES-03, STK-06, and the CUT-03 line at `session/mod.rs:2418` | — (STK-06's two call sites are `session/compute/simulation.rs` and `session/compute.rs`; it reads `compute/transform.rs` and does not edit it. SES-03 restructures `session/mod.rs`, so the `retract_strategy = "full"` write is this agent's to delete) |
| core-cutting | CUT-03 + CMP-17, CUT-05 (core half), CUT-15, CUT-01 | `compute/config.rs`, `compute/mod.rs`, `compute/catalog.rs`, `compute/execute/finish_3d.rs`, `ops/depth.rs`, `ops/face.rs`, `rs_cam_viz/src/ui/properties/linking_dressup.rs`, `mcp_server.rs:1157`, `state/toolpath.rs`, `state/toolpath/support.rs`, `rs_cam_viz/src/compute/worker/execute/mod.rs:516`, `tests/snapshots/mcp_wire_surface.json` |
| cli-mcp | CLI-08, CLI-09, CLI-01 (job half), CLI-04, CLI-06, and the CUT-03 line in `rs_cam_mcp` | `rs_cam_viz/src/app/mcp/commands.rs` (the hand parse CLI-09 deletes), `rs_cam_mcp/src/server.rs:928` (the doc-comment example naming `retract_strategy`) |
| viz-ui | UI-06, UI-01, UI-11 | `rs_cam_viz/src/state/runtime.rs`, `app/mcp/view.rs` |

Folder `CLAUDE.md` invariants touched:
- `session/` — the invalidation invariant is the row SES-07 repairs; SES-03
  moves types out of `mod.rs` and must keep the re-export path the folder file
  names.
- `ops/` — the depth ladder and its "the written depth is one the machine
  cuts" sentry; CUT-15 decides the staircase.
- `dressup/` — G-RETRACTDIAL closes here.
- `ui/properties/` and `ui/components/` — UI-01 moves arguments into the
  existing snapshot; do not draw a raw widget where the kit owns the renderer.
- `crates/rs_cam_mcp/CLAUDE.md` — schema compatibility; CLI-09 changes a
  published schema from an open string to an enumeration.

### Wave 4 — the power-session sweep, the finish measurability row, the dexel deletion

Three agents. No compute, no session, no viz-shell, no viz-ui and no cli
agent runs, because the feeds agent reaches all five of those folders.

| agent | rows | owns outside its folder |
|---|---|---|
| core-feeds (power) | FDS-01, FDS-02, FDS-03, FDS-04, FDS-05 | the ten `SuggestContext` sites: `rs_cam_viz/src/app/mcp/commands.rs`, `controller/events/{toolpath,model,mod}.rs`, `ui/properties/{pills,feeds_speeds}.rs`, `rs_cam_cli/src/smoke.rs`, `session/compute.rs`, `session/multitool.rs` |
| core-stock | STK-13 + STK-02, STK-14, STK-11, STK-10 | `compute/transform.rs` (the four lateral variants), `export/` (the STK-14 target) and `export/fingerprint.rs:796`, `rs_cam_viz/src/app/gpu_upload.rs:88`, `rs_cam_viz/src/ui/overlays/panel.rs:444`, `rs_cam_viz/src/compute/worker/execute/mod.rs:173`, `rs_cam_cli/src/{main,sweep}.rs` (the two `FromTop` literals) |
| core-finish | FIN-14, FIN-01, FIN-02, FIN-12, FIN-13, FIN-05, FIN-06 | `compute/config.rs`, `compute/execute/findings.rs` |

STK-13 must land after wave 3's STK-06. Wave 3 lands STK-06 in its named-rule
form, not its accessor-call form; the accessor-call form would push the four
lateral variants into `SimGroupEntry.direction` that STK-13 then deletes.

Folder `CLAUDE.md` invariants touched:
- `feeds/` — no constant, band or threshold moves; FDS-01 widens a `None` to
  `Some` and changes no number.
- `dexel_stock/` — "the metric route and the playback route agree on the
  stamped volume"; STK-13 removes the side grids, not the Z route.
- `crates/rs_cam_core/CLAUDE.md` — "keep the crate GUI-free"; STK-14 is that
  rule applied to `stock_mesh.rs`.
- `finish/` — the research arms are named research in the folder file and not
  in the build; FIN-01 closes that. `CLAUDE.md:37` names a file as a sentry
  where only one of its four tests is a gate (FIN-05).

### Wave 5 — the tail: wide re-paths and the remaining god functions

**Sequential, one agent at a time.** Every row here re-paths files outside its
own folder, so no two can run together. Run in this order:

1. core-compute — CMP-12, then CMP-21 + CMP-28, then CMP-03 + CMP-04 + CMP-06 + CMP-07 + CMP-11 + CMP-13, then CMP-02, then CMP-18, then CMP-20 + CUT-06 + CUT-11 (one change, two groups filed it), then CMP-23 + CMP-25.
2. viz-shell — CMP-19, SHL-02, SHL-03, SHL-06.
3. viz-ui — UI-02, UI-03, UI-04, UI-05, UI-07, UI-09, UI-10, UI-12.
4. core-fields — FLD-01, FLD-02, FLD-06.
5. core-cutting — CUT-04, CUT-09, CUT-13, CUT-14.
6. core-stock — STK-01, STK-03 + STK-07, STK-09, STK-12.
7. core-session — SES-04, SES-06.
8. core-edges — EDG-02, EDG-07.

Folder `CLAUDE.md` invariants touched: every folder file in the two core
crates and the viz crate. The three that constrain the work hardest are
`adaptive3d/` (all three `ClearingStrategy3d` variants and `clear_z_level`
are live, so CUT-09 extracts and deletes nothing), `dexel_stock/` (the two
routes must stay bit-comparable, so STK-01 and STK-09 run under
`assert_grids_bit_identical`), and `ui/components/` (one renderer per
element, which is what UI-02, UI-03 and UI-10 restore).

## Not now

No finding proposes deleting anything on the brief's ruled-live list. The
auditors checked each one and said so: `AgentSearch`, all three
`ClearingStrategy3d` variants and `clear_z_level` (core-cutting), `crate::flow_accum`
and the pencil NMS detector (core-fields), `SimGroupEntry.direction`
(core-compute and core-stock, which also records that `Engagement::direction`
in STK-05 is a different field), `polygon.rs` and `feeds::calculate`
(untouched), and the other account's viz tokens (viz-ui states them out of
scope). The rows below are deferred for risk, for a missing measurement, or
for a decision that is not an engineer's to make.

- FIN-08 — making `SteepShallow` a preset of the unified planner changes a
  shipped operation's emitted moves. It needs a paired A/B first, and the
  folder invariant already demands one.
- CUT-10 — passing the operation's plunge rate to the entry and link steps
  changes fed descent rates on shipped files. The auditor could measure the
  two values side by side and not the gap between them; take a before/after
  reading first.
- CUT-14 — one Z ladder moves every Z level on one of the two families. Pin
  the 2.5D side against `z_ladder` before merging, not after.
- CUT-09 — a 1 032-line extraction inside the live AgentSearch arm. Build the
  byte-compare rig for a generated adaptive3d toolpath before the first cut.
- CUT-04 — thirty fields moved by hand; a mis-threaded field changes a
  toolpath in silence. Same rig as CUT-09, same reason.
- CUT-13 — `Option<Params>` dressups is a project-file and wire break. Land it
  after CMP-17's field table exists, or the break lands twice.
- CMP-05 — stating a range for 238 of 243 params needs the person who knows
  each dial, and MCP starts refusing values it used to accept. It is a sweep
  with an owner, not a refactor.
- STK-09 — the reduction order fixes the `f64` volume sum. Do not merge the
  two batch dispatchers until the determinism sentries are green on both
  routes with the merge in place.
- STK-12 — making the GUI list a projection of `SimulationTriage` changes what
  an operator sees in the timeline. That is an operator ruling, not a refactor.
- STK-06 against STK-13 — the two proposals conflict. STK-06's accessor-call
  form pushes the four lateral `StockCutDirection` variants into
  `SimGroupEntry.direction`; STK-13 deletes those variants. Land STK-06's
  stated alternative — name the `FromTop` group rule in one function — and the
  two agree.
- SES-05 — fixtures, keep-out zones and plan reorder have no MCP or CLI
  surface. Adding one is a product decision with new wire types, not debt.
- SES-02 — the finding proposes no code change. Land the round-trip sentry it
  asks for and leave the 458-line save/load pair alone.
- SHL-02 — the auditor advises against a blind macro rewrite of a 2 687-line
  load-bearing file. Name it as debt; do not fix it on this programme.
- UI-05 — a viz per-operation table that mirrors a core table still written by
  hand buys little. Do it after CMP-07 moves the three `matches!` predicates
  onto the registry row.
- TLD-07 — the auditor asks for no action: the two policy structs are
  declarative provenance tables, not argument bags.
- TLD-08 — `wanaka_e2e_chipload_gate` is blocked on O3 and O4. It needs an
  owner and a date, or one line saying why it is safe to leave dark.
- FDS-05 — `GeometryClass::{ShallowTerrain, SteepTerrain}` is a self-declared,
  honestly labelled placeholder. No action.
- FLD-01 — embedding `GridSpec` re-paths readers in the viz overlays and two
  MCP handlers, so it cannot share a wave with either viz agent. It is in the
  wave 5 tail for that reason, not for risk.

## Coverage gaps

What the auditors state they did not read, and what the file shapes show.

- **core-compute.** Part 1 left roughly 10 300 lines unread: `config.rs`
  (2 532), `simulate.rs` (2 304), `tool_config.rs` (1 297), `stock_config.rs`
  (809), `validate.rs` (783), `sim_prefix.rs`, `transform.rs` (618),
  `stats.rs` (335), `collision_check.rs` (181) and `execute/dressup_apply.rs`
  (800). Part 2 read exactly those and produced CMP-14 to CMP-28. The residue
  is the group's two large test modules — `execute/tests.rs` (1 786 lines) and
  `catalog/tests.rs` (526) — so the brief's test-only-`pub` and
  `#[ignore]`-harness angles are unexamined for the largest folder. Part 2
  wrote no residue section of its own, so its own blind spots are unstated.
- **core-feeds and core-tool_load.** Structure-only by the brief's ruling. No
  constant, band, threshold or gate number was read for meaning, and no
  finding proposes one. Five findings for `feeds/`, ten for `tool_load/`.
  Both files say so in their headers.
- **viz-shell.** Six findings for seven folders plus six crate-root files —
  the thinnest coverage per line in the audit. `render/` and `interaction/`
  produced no finding at all. `app/mcp/*.rs` carries 223 `json!` sites by
  cli-mcp's own count and only `simulation.rs` and `diagnostics.rs` were
  sampled. The file has no "not verified" section, so the gap is inferred from
  its shape rather than stated.
- **core-cutting, could not verify.** `apply_dressups`'s doc says it is called
  from three crates; the auditor found two production callers, both in core.
  Re-count before CMP-20 + CUT-11 starts. The runtime size of CUT-10's two
  plunge rates is unmeasured, because the audit ran no cargo.
- **core-edges.** `export/`, `diagnostics/` and `util/` produced no finding
  beyond checked-and-clear rows. The group's numbering runs EDG-01 to EDG-07
  with EDG-05 written last, which is cosmetic.
- **core-fields.** `geometry/` produced one finding (FLD-06). `polygon.rs`
  (3 004 lines) is ruled out of scope by the brief and was not read.
- **core-session.** SES-02 declines to propose a change for the 458-line
  save/load pair and asks for a sentry instead, so that boundary is named and
  not audited field by field.
- **core-stock.** States that `feeds/` and `tool_load/` numbers were not
  examined, and that STK-03 and STK-07 need the power session to review the
  rename because the feeds gates read `Engagement`.
- **Workspace.** No group read `benches/`. The `#[ignore]`-harness angle is
  answered by two findings only (FIN-05, TLD-08). No group audited the
  `tests/` directories of `rs_cam_viz` or `rs_cam_cli`.

## Verification list

The ten anchors an orchestrator should re-check before wave 1 starts. Each
command is read-only. A command whose output disagrees with the claim means
the row moved since 2026-09-17; re-read the group file before scheduling it.

- CMP-14 — `crates/rs_cam_core/src/session/compute/diagnostics.rs:367` — `rg -n 'unwrap_or\(0\)' crates/rs_cam_core/src/session/compute/diagnostics.rs`. Run on 2026-09-17 this returns **two** lines, not one. The second, `:719`, reads a toolpath that is absent from `evidence.holder_collisions` as zero, adds it to `total_collision_count` and tests it with `> 0`. It is the same defect one consumer further on; the row must fix both.
- CMP-15 — `crates/rs_cam_core/src/compute/transform.rs:65-74,198-206` — `rg -n '_ => FaceUp::Top|_ => ZRotation::Deg0' crates/rs_cam_core/src/compute/transform.rs`
- CUT-08 — `crates/rs_cam_core/src/dressup/mod.rs:870` — `rg -n 'feed_to\(' crates/rs_cam_core/src/dressup/mod.rs`
- CUT-03 + CMP-17 — `crates/rs_cam_core/src/compute/config.rs:2083` — `rg -n 'retract_strategy|RetractStrategy' crates --type rust`. Run on 2026-09-17 this returns 19 hits in 9 files, wider than CUT-03's list: it adds `rs_cam_mcp/src/server.rs`, `compute/mod.rs`, `state/toolpath.rs` and `state/toolpath/support.rs`. Three of the four are re-export lists (`compute/mod.rs:35`, `state/toolpath.rs:25`, `state/toolpath/support.rs:6` — the same hand-maintained lists CMP-12 reports) and the fourth is a doc-comment example naming `retract_strategy` as a settable dressup field (`rs_cam_mcp/src/server.rs:928`). None reads the field to decide a retract, so the deletion still holds, but four more files move with it and `rs_cam_mcp` puts the wave-3 row in a fourth crate.
- CUT-05 + CLI-01 — `crates/rs_cam_core/src/adaptive3d/mod.rs:138` — `rg -n 'max_stay_down_dist\b' crates --type rust`
- SES-07 — `crates/rs_cam_core/src/session/mutation/config.rs:314,365` — `rg -n 'drop_tool_results|drop_results_for_model|invalidate_output_dependents' crates/rs_cam_core/src`
- EDG-06 — `crates/rs_cam_viz/src/io/export.rs:24` — `rg -n 'log_machine_safety|validate_machine_safety' crates/rs_cam_viz/src`
- UI-08 — `crates/rs_cam_viz/src/ui/properties/mod.rs:232` — `rg -n 'mark_edited' crates/rs_cam_viz/src/ui/properties`
- FDS-01 — `crates/rs_cam_core/src/feeds/suggest.rs:144` — `rg -n 'SuggestContext\s*\{' crates --type rust`
- STK-06 against STK-13 — `crates/rs_cam_core/src/session/compute/simulation.rs:82` — `rg -n 'StockCutDirection::(FromLeft|FromRight|FromFront|FromBack)' crates --type rust`

One more is worth running before wave 5, because the tail depends on it:
CMP-20 + CUT-11 — `crates/rs_cam_core/src/compute/execute/dressup_apply.rs:338` —
`rg -n 'apply_dressups' crates --type rust`. The doc claims three calling
crates; the auditor found two callers, both in core.

## Wave 1 outcome — 2026-09-17 evening

Seven agents, 18 commits on master (`1a0a1ea5` .. `3c691ac2`), one row per
commit, a teeth check on every new sentry. Consolidated gate: see
`WAVE1_GATE.md`.

Landed: CUT-08, CUT-02, CUT-12, CUT-07 (doc); EDG-01, EDG-03, EDG-04;
FLD-03; STK-08, STK-04 + STK-05 (deleted; schema 5 → 6); UI-08, UI-13;
CLI-05 (+ a second defect found at runtime: a multi-operation sweep patched
the last operation and reported the first, `88c8e5e9`); CLI-07; FIN-01
(eight test targets gated, not six; CI runs them under the feature).

Skipped with cause:
- FLD-04, FLD-05: all eight doors are bound by integration-test crates, so
  `cfg(test)` cannot hide them. Mechanism: a `test-support` feature on
  `rs_cam_core` with `required-features` on seven test targets and a viz
  dev-dependency. One row, after wave 1.
- EDG-05: the claim is wrong. `GrblImport` and `MoveKinematics` are return
  types of live `pub fn` with external readers; only `WindingReport` plus
  `check_winding` could narrow to `pub(crate)`.
- UI-06: two files outside the viz-ui row; stays in wave 5.
- CUT-12's `search.rs` → `material.rs` rename: file-map row only this wave.

Deviations from the finding text, recorded in the commit bodies: CUT-08's
vertical re-entries are at `:878` and `:910`, not `:899`; CUT-02 found nine
wrappers, not eight, and `optimize_entry_descents_with_provenance` keeps its
suffix; FLD-03's `ordinal` takes `&self` (`Copy` would raise
`clone_on_copy` in two foreign files).

Follow-ups opened: a third point-to-segment copy in
`tests/scallop_isofield_gouge_m4.rs:484` (heavy-tests); the `entry_style`
serde alias on `OperationDef` (no-legacy delete candidate); UI-08 may mark a
loaded project dirty on its first Post-tab frame if a post config does not
round-trip (G-DIRTYONLOAD); `material/` still has no `CLAUDE.md`.

## Wave 2 outcome — 2026-09-17/18 night

Four agents, 21 commits (`3a6d5968` .. `130045ac`) plus the LH-1 sentry
repoint `5e10dc80`. Gate: `WAVE2_GATE.md`. No row skipped.

Landed: CMP-14 + CMP-24 (three-state holder check, one index per model),
CMP-15, CMP-08 + CMP-10 + CMP-09, CMP-27, CMP-01, CMP-26, CMP-16, CMP-22;
EDG-06 (MCP export names its safety findings), SHL-01 (+ a second fix: the
spindle-strategy path wrote the post mirror on a refused command), SHL-04,
SHL-05; FIN-04 + FIN-09, FIN-11, FIN-15; FLD-04 + FLD-05 as a `test-support`
feature (twelve doors, eight targets, the feature on every gate line).

Lesson: two agents edited `catalog/registry.rs` at once and each swept the
other's hunks; master was red for about an hour. From wave 3 the brief names
one owner per file and forbids `cargo fmt` without a file list.

Follow-ups opened: `Drill`/`AlignmentPinDrill` accept `plunge_rate` and
carry no field (N5 census pins it); the GUI's MCP project JSON does not emit
`collision_checks_failed`; a mesh-less toolpath skips the fixture half of the
holder check; `HolderCollisionCheck` sits in `compute::collision_check` and
`stock/sim_triage.rs` imports it (a new upward import; `stock::collision` is
the better home); the collision lane has no revision check and never had one
(`compute/CLAUDE.md`'s line is over-broad for it); EDG-06 split-setup
findings do not name the file; `DeprecatedDialFinding` and its adapter have
no producer; `ramp_finish_toolpath` is a sixth FIN-11-shape wrapper; the
four caches' `reset_stats`/`cache_len`/`clear` are the same test-only shape.

## Wave 3 outcome — 2026-09-18 00:30

Four agents, 26 commits (`4de1be71` .. `1cafe62b`). Gate: `WAVE3_GATE.md`.
No row skipped.

Landed: SES-07 (seed-set fixpoint), SES-01 (four derives, byte-identical),
SES-03 (`session/diagnostics_types.rs`), STK-06 (named `FromTop` rule;
lateral setups unchanged); CUT-03 + CMP-17 (`RetractStrategy` gone from
nine files, `DressupConfig::FIELD_DEFS` publishes 22 fields), CUT-05,
CUT-15 (DELETE: `DepthDistribution` and the finish-allowance arms; the
staircase stays documented because the stair sentry pins the Even
arithmetic and the Z ladder is an operator measurement), CUT-01; CLI-08
(one tool-type vocabulary, job TOML tokens changed), CLI-09 (`z_rotation`
enum on the wire), CLI-01 (+ the `entry_style` alias), CLI-04 (four
hand-rolled caps, not one; `get_generation_debug_trace` had no truncation
key at all), CLI-06; UI-06, UI-01 (25 params → 5, 1 255 lines → ~410 plus
four tab functions), UI-11.

Two live defects found by the agents' sentries and fixed by the
orchestrator: `set_dressup_field` refused the two F-040 lead feeds on a
fresh config (`9d01139b`); the Linking tab wrote `feed_optimization =
false` while drawing and staled the toolpath (`1cafe62b`, G-LINKFEEDOPT).
Two pre-existing vacuous sentries repaired: LH-1 (`5e10dc80`) and WP15a
(`5311fb91`), both reading pre-split paths.

Incident: the core-session agent ran a forbidden `git stash` that swept
25 in-flight peer files; it restored them by content and every peer
verified its edits. The stash survives as dangling commit `1b1f3c11`.

Follow-ups opened: `inspect_spans` detail mode still emits
`total_matching`/`truncated` (rename needs `mcp_server.rs:678` to move);
`DressupFieldDef::description` has no reader; `HolderCollisionCheck`
should move to `stock::collision`; the GUI's MCP project JSON lacks
`collision_checks_failed`; `ramp_finish_toolpath` is a sixth FIN-11-shape
wrapper; `feeds/suggest/apply.rs:92` names the deleted
`DepthDistribution::Even` in a comment (power session's file).

## Waves 4 + 5 outcome — 2026-09-18 morning

Nine sequential slots, 60 commits (`8e3a6a83` .. `32f7ca82`), one agent at
a time. Gate: `WAVE5_GATE.md`.

| slot | rows landed | skipped with cause |
|---|---|---|
| 1 finish | FIN-14, FIN-02, FIN-12, FIN-13, FIN-05 + FIN-06, `ramp_finish_toolpath` | — |
| 2 stock | STK-02, `HolderCollisionCheck` → `stock::collision`, STK-14, STK-10, STK-09, STK-01 (18 grid hashes bit-identical) | STK-11 and STK-03 + STK-07 need lines in the power session's `tool_load/` (patch for STK-11 in the orchestrator's scratchpad); STK-12 needs an operator ruling on the timeline (two GUI issue kinds have no core source); STK-13 not ruled |
| 3 compute | CMP-12, CMP-21 + CMP-28, CMP-03 + -04 + -06 + -07 + -11 + -13, CMP-02, CMP-18, CMP-20 + CUT-06 + CUT-11, CMP-23 + CMP-25 | — (three compile-forced lines in `tool_load/` were edited) |
| 4 viz-shell | CMP-19, SHL-02, SHL-06, SHL-03, `inspect_spans` keys, `collision_checks_failed` in the GUI JSON | — |
| 5 viz-ui | UI-02, UI-03, UI-10, UI-04, UI-05, UI-07, UI-09, UI-12 | — (four landed narrower, each with a non-vacuity arm) |
| 6 fields | FLD-01, FLD-02, FLD-06, 12 more test doors gated | — |
| 7 cutting | CUT-04, CUT-13 (five of seven bools), CUT-14 (fallback form), CUT-09 | `feed_optimization` fold needs one `tool_load/` line |
| 8 session | SES-04, SES-06, mesh-less fixture check, two folder files back to 40 lines | — |
| 9 edges | EDG-02 (132 rows to `data/wood_species.toml`, hash-pinned), EDG-07, `material/CLAUDE.md` | — |

Defects found by the slots' own sentries and fixed on the way: the S5
memo never hit on a session ladder (the semantic trace was re-allocated
per round, `b2b9bb8d`); the MCP collision JSON published a segment name
with escaped quotes (STK-10); `ReachMap` fails its own JSON round trip
(`NaN` floor cells, recorded, not fixed); `StepImportError::TessellationFailed`
carried a hard-coded shell index (deleted).

Add-an-operation cost: 4 decision files → 3 in core; the viz table
`operations/registry.rs` holds draw, diagram and validate per operation
with a completeness test.

Open for the operator: STK-13 (side grids), STK-12 (timeline projection),
FIN-08 (paired A/B, two-band preset), the `tool_load/` lines above, a
live look at the GUI after the kit replacements (UI-02/03/10 change
spacing and column widths), `io/CLAUDE.md` lists two `step`-feature
sentries the default build does not run, the UI-04 help ratchet at 127
unwritten lines, `adaptive3d_emission_byte_parity.rs` lacks a programme
suffix, and the corridor test's `LONG_AND_THIN` fixture (power session).
