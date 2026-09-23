# Feeds matrix: back every recommendation and every warning with a source

Opened 2026-09-23. Status: plan, not started. `PROMPT.md` beside this file
is the brief for the session that runs it.

## 1. The ask

Three observations from the operator, 2026-09-23:

1. Depth-of-cut warnings fire on 3D finishes and roughs that a machinist
   would run.
2. The recommended feed, speed, stepover and depth are good on some
   tool-and-operation pairs and wrong on others.
3. A recommendation must rest on a published figure or a named rule. Where
   the engine has neither, it must refuse to suggest, not invent a number.
   A de-rate must say which published rule it applies.

The programme therefore has three arms: a **matrix** (every tool type on
every operation, what fires and what is recommended), an **evidence**
pass (each firing warning and each recommendation against the published
row it should agree with), and a **declaration** (one place per operation
that says whether a recommendation is backed, and the calculator refuses
when it is not). No fix lands before the rulings in §7.

## 2. Where this sits against work in flight

Two peer efforts touch the same code. This plan is the third arm and
reads them; it does not reopen them.

| Effort | Question | State on 2026-09-23 |
|---|---|---|
| `planning/wanaka200_feeds_check_2026-09-19/` (H1–H7) | Why does the calculator read 3× to 7× LOW in wood on one scallop and one V-bit pass? | H1 verified (the raw Amana row is 0.012–0.024; the derate chain and the band-as-limit clamp make the rest). Two entry defects fixed. H2–H7 open. The three docs and `MACHINIST_REFERENCE_CHECK.md` are UNCOMMITTED in the tree; confirm they landed before citing a line number. |
| `planning/load_model_2026-09-16/DERATE_SPEC.md` + `WHERE_THIS_LANDS.md` | Which lever does a binding limit pull? | Specification, not implemented. Measured: the rigidity cap `factor × D` set the depth on 100 % of 54 recipes; power bound on 4 %. |
| This plan | Across ALL tool types and ALL operations: what fires, what ships, what backs it. | Not started. |

Shared ruling: H7 ("should a formula-only V-bit recommendation ship at
all") IS the declaration question of §5. One ruling closes both. Do not
answer it twice.

## 3. What the code holds today (read, not run)

- Calculator: `feeds::calculate` (`feeds/mod.rs:1216`), ten steps (RPM,
  LUT chipload or formula, DOC/WOC, flute and shank guards, feed, setup
  derates, power, safety factor). Suggest applies it
  (`feeds/suggest.rs:809 suggest_for_operation`), then
  `suggest/invariants.rs::clamp_dpp_to_rigidity` lowers the depth to the
  rigidity cap.
- The cap: `machine/mod.rs:132 depth_cap_mm` = `factor × diameter`, factor
  by family then role: Drill none, Adaptive `adaptive_doc_factor`,
  Roughing `doc_roughing_factor`, Finish/SemiFinish `doc_finishing_factor`.
  The default profile ships 0.2 / 0.08 / 1.5. `BoundSource::RigidityRuleOfThumb`,
  no published source, does not gate an export.
- Post-simulation gates (`tool_load/`): chipload, power, deflection,
  plunge stress, depth (`depth.rs`, S3 2026-09-18: the peak
  `axial_engagement_mm` against the same cap).
- Pre-simulation static checks (`diagnostics/adapters/from_static_checks.rs`):
  `geom.dpp_over_1_5x_diameter` fires above 1.5 × D with no source cited
  (`:369`). Also `feeds.dpp_vs_lut`, `feeds.feed_vs_lut.high/low`,
  `feeds.stepover_vs_lut`, `feeds.chipload_clamped_to_floor`,
  `feeds.no_vendor_rows_for_routed_operation`, `feeds.doc_exceeds_flute`
  (`diagnostics/ids.rs:99-124`).
- The operation declares its feeds route as two static registry fields,
  `OperationSpec::feeds_family` and `feeds_pass_role`
  (`compute/catalog.rs:69-79`, rows in `catalog/registry.rs`). That is the
  declaration site §5 extends. `FeedsError::WrongToolForOperation`
  (`feeds/mod.rs:874`) is the one refusal that exists: a flat or V tool
  on a scallop.
- The vendor LUT: 256 rows in `data/vendor_lut/`, 58 populated
  `tool_family × operation_family × material_family` cells. Row keys:
  `tool_family`, `tool_subfamily`, `operation_family`, `pass_role`,
  `material_family`, `chipload_min/max_mm_tooth`, `ap_*`, `ae_*`,
  `rpm_*`, `evidence_grade`, `source_url`, `accessed_on`, `row_kind`
  (exact 228, derived 15, fallback 13).

### 3.1 LUT coverage, the table that decides the matrix

Rows per cell, wood materials only (hardwood / softwood / mdf /
plywood_hardwood). Aluminium, acrylic and the plastics are 62 rows and out
of scope for the first pass.

| tool_family | adaptive | contour | face | parallel | pocket | scallop | trace |
|---|---|---|---|---|---|---|---|
| flat_end | 25 | 34 | . | . | 65 | . | . |
| ball_nose | . | . | . | 19 | . | 1 | . |
| tapered_ball_nose | . | . | . | 8 | . | 2 | . |
| chamfer_vbit | . | 1 | . | . | . | . | 24 |
| bull_nose | 2 | . | . | . | 1 | . | . |
| facing_bit | . | . | 7 | . | . | . | . |

Read across: a flat end mill has no published row for any finishing
family; a ball has none for pocket, adaptive or trace; a bull nose has
three rows in total; nothing publishes a scallop row for a flat or V
tool, which `WrongToolForOperation` already refuses. Every empty cell
that an operation can reach is a cell the calculator fills by formula
today. §5 makes that visible; §7 rules on it.

## 4. The matrix instrument (Phase 1)

A read-only harness, a core integration test behind `--ignored` plus a
CSV writer, not a product surface:

- Rows: the five `ToolType` tokens (`end_mill`, `ball_nose`, `bull_nose`,
  `v_bit`, `tapered_ball_nose`) at two diameters each (a small and a
  common size, from the tools the LUT rows carry).
- Columns: `OperationType::ALL` (24).
- Depth: the four wood `material_family` values above, on the default
  `Generic Wood Router` profile and one measured profile if one exists.
- Per cell: (a) `suggest_params` result or refusal; (b) rpm, feed, plunge,
  stepover, depth per pass; (c) the LUT row id, grade and `row_kind`, or
  "formula" with the formula's named source; (d) every `feeds.*` and
  `geom.*` id the static checks raise on the suggested recipe; (e) for a
  chosen subset (§6), the post-simulation `load.*` verdicts.
- Output: `planning/feeds_matrix_2026-09-23/matrix_<date>.csv` and a
  markdown summary with one line per cell class: refuse / vendor-backed /
  formula-only / fires-a-warning.

The harness runs the same doors the GUI and MCP use (`suggest_params`,
`static_checks`, `tool_load::evaluate_toolpath`). It adds no arithmetic.

## 5. The declaration (Phase 0, small)

One registry field, resolved per (operation, tool_family, material_family):

```
pub enum FeedsSupport {
    /// A vendor row exists for this cell; the LUT answers.
    VendorBacked,
    /// No row; the calculator's formula answers, and this names its source.
    FormulaOnly { source: &'static str },
    /// The engine has no basis. Suggest refuses with the reason.
    Refuse { reason: &'static str },
}
```

- The static half lives on `OperationSpec` beside `feeds_family` (which
  operations can be formula-backed at all, and by what). The dynamic half
  is the LUT lookup the calculator already does; the cell's arm is the
  join of the two.
- `suggest_for_operation` returns `Err(FeedsError::Unbacked { .. })` on
  the `Refuse` arm. The GUI Suggest button, the MCP `apply_feeds` door and
  the CLI all surface the refusal; none writes a recipe. This is the
  H7 ruling made mechanical.
- A sentry asserts every `OperationType` has a declared arm for every
  `ToolType` (a match, so a new operation cannot join without one), and
  that `Refuse` is never reachable from a cell that has a vendor row.
- NOT a trait per operation, NOT a per-parameter policy object, NOT a
  rewrite of `calculate`. One enum, one field, one refusal variant.

## 6. Evidence per firing cell (Phase 2)

For every cell that fires a warning or ships a formula-only figure, one
row in `EVIDENCE.md`: the published figure or rule it should agree with
(URL, table row, condition line), the engine's number, the ratio, and a
verdict: agrees / engine low / engine high / no source exists.

Sources already in the tree: `load_model_2026-09-16/MACHINIST_REFERENCE_CHECK.md`
(Onsrud per-material chipload tables with the 1×D / 2×D −25 % / 3×D −50 %
depth rule; ShopBot; others), `data/vendor_lut/source_manifest.json`,
`CREDITS.md`. New sources go into the manifest and `CREDITS.md` in the
same change.

Two worked examples to run first, because they are the operator's
complaint:

**(a) The waterline depth gate.** Corne case, 2026-09-18: `Waterline 3`,
Ø6 flat, `z_step` 1.0, post-sim depth gate `Exceeds`, peak 9.84 mm against
cap 0.48 mm (= `doc_finishing_factor` 0.08 × 6). A wall-following finish
engages the wall height by construction; the axial step is 1.0 mm and the
radial bite is the finish allowance. The cap table has no arm for a pass
whose load is radial. Hypothesis: the depth criterion is the wrong
quantity for the Parallel family on a vertical wall, and the published
rule to agree with is the vendor's finishing `ae`/`ap` row (Onsrud
77-100, Amana 3D profiling), not `factor × D`.

**(b) `geom.dpp_over_1_5x_diameter`.** Onsrud publishes 1×D at full
chipload, 2×D at −25 %, 3×D at −50 %. A 1.5 × D hint with no de-rate
attached and no source fires on a rough that Onsrud's own chart allows.
Hypothesis: replace the hint with the Onsrud depth rule as a de-rate
(the one published de-rate we have), and cite it.

Both are hypotheses for the session to test, not verdicts.

## 7. Rulings the programme needs (Phase 3)

R1. H7 / declaration: does a formula-only recommendation ship? Options:
(a) ship with a visible `FormulaOnly` provenance and a Caution; (b) refuse
unless the tool family has at least one vendor row in that operation
family; (c) refuse unless the exact cell has a row. Recommendation: (b)
for wood, with the formula's source named; (c) is the LUT's current 58
cells and would refuse most 3D finishing.

R2. The depth criterion on radial-load finishes (§6a).

R3. The 1.5 × D static hint versus the Onsrud depth rule (§6b).

R4. De-rate vocabulary: which published rules may scale a figure
(Onsrud depth rule; the DOC piecewise scale already in `feeds::geometry`;
the vendor `ap`/`ae` rows), and the rule that an unpublished scale flags
and does not scale.

R5. Coverage gaps the operator wants filled from the reference check
(which Onsrud series rows to add, which tool families stay formula-only).

## 8. Fixes (Phase 4) and the tests that envelop them

Only inside the rulings. Each fix carries: the matrix cell(s) it moves,
the evidence row it agrees with, and a sentry. The matrix itself becomes
the regression net: a sentry pins each cell's `(support arm, warning ids)`
and a fix that moves a cell re-blesses with the cause named. Keep the
existing 58 feeds sentries green; `wanaka_suggest_integration` takes
minutes, ask first.

## 9. Not in scope

Non-wood materials; the DERATE_SPEC lever work (its own spec); the H2–H6
derate-chain instrumentation (the peer's arm, read its result); the
kinematics model; any GUI redesign of the Feeds tab.
