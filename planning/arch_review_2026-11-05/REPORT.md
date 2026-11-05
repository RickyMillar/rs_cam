# Architecture review: feeds contract and geometry selection (2026-11-05)

Scope: the committed state at `666e6df06`. Ranges: feeds contract `f56000c54..666e6df06` (56 commits); geometry selection from `97041cf82`. The reviewer read the code with `git show` and `git grep`. The working tree (P11c in progress) was not read. No cargo command ran. `assert_claims.py` checks every claim (25/25 PASS). "Known" means `STATUS.md` or `PROMPT_NEXT.md` already records the item.

The reviewer was an independent agent; the lead saved this text (the agent could not write the file).

## Findings

| ID | Sev | Category | Claim | Evidence (`@666e6df06`) | Consolidation | Size | New/known |
|---|---|---|---|---|---|---|---|
| H1 | High | Parallel flow | The selection pipeline (stock grow, `view`, part-free note, `fit_holes`, edge clear, open sides) is built twice by hand, in generation and in the status report; comments and tests hold the parity | `compute/execute/prologue.rs:505-545`; `session/selection_status.rs:844,989-990,1067`; `session/pins.rs:304-308`; `compute/execute/drilling.rs:75,199` | One function in `selection/` that all four call | M | New |
| H2 | High | Growth hotspot | `calculate_with` is one 1 478-line function; Step 6 is about 350 lines; `feeds/mod.rs` grew 3 539 -> 4 183 | `feeds/mod.rs:1990-3467`, `:2708`, `:3055` | One function per step over a `CalcState` | L | New |
| H3 | High | K2-K3 seam | `Basis::tool_fact()` has empty `from`; the planning readers drop `FactKind`; an Unknown fact's `LEGACY_DEFAULT_*` row is invisible to `unsourced_rows` and to a P13 lineage tree | `feeds/sourced.rs:144-149`; `compute/tool_config.rs:792-800`; `feeds/suggest.rs:984,1002`; `feeds/mod.rs:1844` | `Basis::ToolFact{fact,kind}` with the legacy row as a child | M | New |
| H3b | Med | Unsourced number | P5 ships `MIN_AP_MM` and `MIN_AE_MM` as `Basis::derived(.., Vec::new())`; they are bare `f64` | `feeds/mod.rs:1937,1940,2562,2571`; `feeds/envelope.rs:1331,1337` | Two F201 rows | S | New |
| M1 | Med | Duplication | 200 um typed 4 times (the P5 row retypes 200.0) plus 3 law strings | `tool_load/deflection.rs:61`; `suggest/invariants.rs:53,57-59`; `cutter_constraints.rs:76` | One row | S | New |
| M2 | Med | Duplication | Three chip-thinning laws: Garr, Garr clamped [1,4], `1/sqrt(woc)` | `feeds/geometry.rs:17,29,55`; `dressup/feedopt.rs:79`; `dressup/feed_modulation.rs:856` | `RadialThinning::lift` | S | Known |
| M3 | Med | Duplication / dead | Two diameter rules feed `Engagement`; `feed_ladder_diameter_mm` has no reader and a false doc | `feeds/mod.rs:4163`; `feed_modulation.rs:258`; `engagement.rs:108,129`; `geometry.rs:426,432` | One `d_eng` door | S | Known |
| M4 | Med | Duplication | Stepover typed in 5 places (0.25/0.20, 0.15/0.20, 0.35/0.3) | `machine/mod.rs:175,694`; `compute/validate.rs:277,284`; `feeds/predict.rs:261,268` | Read `default_radial_width` | S | Partly known |
| M5 | Med | Unsourced | Unsourced 144 -> 159, cited 3 -> 3, `::unsourced(` sites 99 -> 117; doc says 149 | F201 test; `feeds/CLAUDE.md` | Fix doc; P13 source hunt | S | New framing |
| M6 | Med | Contract drift | PLAN K2 wants a recorded acceptance; `accept_unsourced` is unconditional; `ApplyFeedsArgs` has no field | `PLAN.md:158`; `suggest/apply.rs:732,1112`; `suggest.rs:937`; `session/command.rs:1186-1193` | Drop the type state or add the field | S | New |
| M7 | Med | Two axes / false names | `Severity::Blocking` and `Consequence` joined by an id special case; `blocks_export` and `model_gouge_refusal` only warn | `session/run_state.rs:147-157`; `model_gouge.rs:256`; `session/compute/export.rs:394`; `diagnostics/mod.rs:40` | Refusal from `RowKind` only; renames | M | New |
| M8 | Med | Dead code | 14 of 26 `SuggestWarning` variants have no producer; `session/compute.rs:1558-1559` reads two | `feeds/suggest.rs` enum | Delete them | S | Known (list new) |
| M9 | Med | Surface parity | 3 surfaces build the shipped point by hand; MCP has no shipped power | `ui/feeds/compare.rs:860`; `app/mcp/generation.rs:33`; `cli/src/suggest.rs:94,142` | `CutterOpProfile::shipped_power()` | S | New |
| M10 | Med | Duplication | The add-menu copy of `serves` tests "any model has items" and "any model has a mesh" separately | `ui/toolpath_panel.rs:1317-1328`; `session/op_draft.rs:268-276` | Call `serves` | XS | Known (difference new) |
| M11 | Med | Dead fields | `woc_roughing_factor`, `woc_roughing_max_mm`, `woc_finishing_mm`: only the MCP echo reads them | `machine/mod.rs:61-68`; `app/mcp/project.rs:589-593` | Delete in P11c | XS | Partly new |
| M12 | Med | Instruction drift | `compute/CLAUDE.md` 24 030 B (6 371-character line); `session` 3 077 -> 21 234 B; `feeds` 8 938 -> 16 120 B; `CONTRACTS.md` 164 lines (PLAN: 40 or fewer); viz `app` 41 and `ui` 43 lines | the files; `PLAN.md:354` | Byte cap; one line per contract | M | New |
| L1 | Low | Stale docs | `enforce_invariants` named 20+ times with no function; deleted sentry and "pass 6b" named in docs | `feeds/mod.rs:4158,1938` | Sweep | XS | New |
| L2 | Low | Overlap | `DressupPolicy` remains beside `EntryContract`; its doc names entry ramps | `catalog/schema.rs:617-618`; `PLAN.md:254` | Rename or fold | S | New |
| L3 | Low | Unsourced | `helix_deg: 30.0` typed in 5 tool constructors, no F201 row | `tool/flat.rs:27`, `ball.rs:26`, `tapered_ball.rs:93`, `bullnose.rs:42`, `vbit.rs:61` | F201 row | XS | Known |
| L4 | Low | Unsourced | `DRILL_DWELL_S = 0.5` (P0) has no F201 row | `compute/operation_configs.rs:158` | F201 row | XS | New |
| L5 | Low | Test door | `Lineage::unrecorded` is `pub`, doc-hidden, test-only | `trace/lineage.rs:435-438` | `test-support` gate | XS | New |
| L6 | Low | Cohesion | `session/compute/export.rs` 947 -> 1 315 lines; holds diagnostics and lineage | `:634,:1195` | Split | S | New |
| L7 | Low | Test debt | 46 G-OPGOLDEN rows accepted red; 10 viz red targets; F201 pin raised 4 times | `opgolden_known_46_rows.txt`; `STATUS.md:96` | P12 lists; clean-up | M | Known (trend new) |
| L8 | Low | Duplication (pre-existing) | MCP `is_horizontal` uses `> 0.95`; core uses `< 0.95`, so the two disagree at exactly 0.95 | `app/mcp/project.rs:687`; `enriched_mesh.rs:312`; `selection/view.rs:630` | Read the constant | XS | New |

Counts: High 3, Medium 13, Low 8.

## High and Medium findings

**H1.** `selection_status` rebuilds each prologue step and says so in its comments ("as the prologue grows it", "THE hole fit step of the prologue"). The programme added four steps (P6.4, P6.5, R-A, open sides). Each step needed two edits, and `pins.rs` and `drilling.rs` hold more partial copies. Fix: one function in `selection/` that takes the resolved selection, the op, the tool and the mesh, and returns the items and the notes.

**H2.** Steps 1 to 9c share one body, and the P11b `DepthClamps` flag forks two branches. The step boundaries already exist (`flute_guard_of`, `slot_rule`). Split the function by step behind FM1 before P13 adds step records.

**H3.** `Basis::tool_fact()`'s doc says "P6 gives it a state"; P6 did not. `UnsourcedRowsAccepted` misses the legacy rows. The D13 item 7 tree stops at "tool library". Fix this in P13 before the lineage store.

**H3b, M1, M4, M5.** `MIN_AP_MM` and `MIN_AE_MM` ship as a derivation with no source. The P5 deflection row retypes 200.0 beside the constant. The predictor's stepover copies the table that P11c replaces. The programme made existing guesses visible and sourced none.

**M2, M3.** Both are on the STATUS list. This review adds the sites and the false doc of `feed_ladder_diameter_mm`.

**M6.** With 159 Unsourced rows nearly every apply is `Partial` and passes anyway. Make PLAN K2 and decision D4 agree.

**M7.** Derive the refusal from the id row alone. Rename `Blocking`, `blocks_export` and `model_gouge_refusal`.

**M8.** The 14 variants: `AxialDocBelowBurnFloor`, `AxialDocClampedByEnvelope`, `AxialEnvelopeSafeBandEmpty`, `ChiploadStillLowAfterRecalibration`, `DeflectionBackoffFigureIsAFloor`, `DeflectionBackoffUnmodeled`, `DepthClampedToCuttingLength`, `DppCappedByDeflection`, `FeedRaisedForChipload`, `FinishEnvelopeAdvisory`, `ProjectCurveDepthInfeasible`, `RoughingDepthClampedToRigidity`, `StepoverClampedToToolDiameter`, `StepoverRaisedForRuntime`.

**M9.** One core method gives every surface the same shipped power.

**M10.** With one DXF and one STL, the add menu offers an op that needs items and a mesh on one model. `serves` refuses each model.

**M11.** MCP prints three values that move nothing. The machine TOML uses `deny_unknown_fields`, so the files change too.

**M12.** The maps became change logs. Add a byte cap. Check whether the known-red `folder_maps_name_real_files_f140` is the sentry that should hold this rule.

## Held well

- One write path: `Command::ApplyFeeds` (`controller/events/mod.rs:1079`; MCP goes through the same funnel; `cli/src/project.rs:1362`). No `pub fn *_mut` in `session/`.
- One `export_warnings` (`session/compute/export.rs:165`); the GUI (`io/export.rs:357`) and the CLI read it.
- `flute_guard_of` and `slot_rule` are shared by the calculator and the envelope (`feeds/mod.rs:1838,1861`).
- `RadialThinning` is one rule.
- `Engagement` and `PowerModelInputs` have private fields.
- `FeedsHints`, `EntryStylePolicy` and `PlungeKnown` are gone.
- `HORIZONTAL_FACE_MIN_NORMAL_Z` is shared by three readers.
- MCP selection is 486 lines of JSON plus parity tests.

## Recommended order

1. P11c: M4, M11, M1.
2. P13: H3, then H3b, L3, L4; H2 if step records come; M5.
3. P12: carry M8, M9, M6, L7.
4. Clean-up phase: H1, H2, M2, M3, M7, M10, M12, L1, L2, L5, L6, L8.
