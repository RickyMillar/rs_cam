# A2 plan (point mode, G4): the planner's design and the orchestrator's decisions

## Orchestrator's decisions (2026-09-24; the operator took the recommendations)

- Q1 (a): below the point is a burn ADVISORY on all 88 point cells (the
  evidence leans to "start"); never Exceeds(Low).
- Q2: above the point stays a hard trip (the "top" reading; a softer cap
  needs a second witness).
- Q3: the viewport draws a point cell grey.
- Q4: the normalization lives in code (build_result); the LUT files do not
  change.
- Q5: the 7 D1 repo-authored Spektra band rows stay out of A2 (a separate
  landing with an FM1 diff; x4.5 chipload).
- Q6: the advisor keeps refusing a cell with no chipload row.
- Q7: a move held at the point keeps the ChiploadMax binding tag.
- Q8: accept that the 3 grade-c idcwoodcraft ball rows lose the min-DOC
  floor (no FM1 cell resolves them); record it.
- Sequencing: step 1 starts after the step-ladder reader moves land
  (cutter_constraints.rs and axial_envelope.rs overlap). The session/
  edits (optimized_candidate, apply_adaptive_feed_modulation) are agreed
  with the feeds/dial session by function; SimulationOptions and
  AdoptSimulationArgs shapes do not change.

---

The design holds the printed value as a point everywhere and derives no band. It needs two edits in `session/`, and no recipe number moves anywhere in FM1. Nothing was built or run; the line refs are from reading HEAD e157d262.

## 1. The encoding

**Every one-value row in the LUT is already a single print.** 100 rows print a maximum only: Spektra 60, SpeTool tapered 27, Whiteside fusion360 13. 48 rows print `min == max`: amana_long_tail 26, idcwoodcraft 10, compression 6, vgroove 3, aluminium 3. No row prints a minimum only. So "maximum only" and "one value" are the same fact today, and one encoding is safe.

**The type** goes in `feeds/vendor_lookup.rs`, next to `LookupResult` (L39):
```rust
pub enum PrintedChipload { Band { min_mm: f64, max_mm: f64 }, Point { value_mm: f64 }, Unpublished }
impl LookupResult { pub fn printed_chipload(&self) -> PrintedChipload }
```
- It classifies the two stored limits:
  - both limits with `lo < hi` → `Band`;
  - `(None, Some(v))`, `(Some(v), None)` or `lo == hi` → `Point`;
  - anything else → `Unpublished`.
- It is a method, not a new field. `MatchedRow` is `pub type MatchedRow = LookupResult` (L27), so a new field would break about 14 struct literals across `src/` and `tests/`, and no one can compile between those edits.

**The one place it is built** is `build_result` (L479-548). At L517-520, when the row's raw `chipload_min == chipload_max`, `chip_load_min_mm` becomes `None`.
- After that, a point always reaches consumers as "maximum present, no minimum". The zero-width encoding no longer exists.
- Every existing pin already expects this shape: census a6:699, g1:306/343, micro_tapered g1:128, sub_1mm:148.
- `chip_load_mm` does not change, because `chipload_midpoint` (L787) gives v for both old encodings.

**How consumers read it:** anything that asks "band or point?" calls `printed_chipload()`. The `lo >= hi` check at `tool_load/chipload.rs` L628-632 goes away.

## 2. Point mode per consumer

| Consumer | Where | Point mode |
|---|---|---|
| Suggest bounds | `feeds/mod.rs` L1633-1645, L1690-1706, L2573-2603; `FeedsResult` L502 | `chipload_bounds` stays two-limit only (`None` for a point). New field `FeedsResult::chipload_point_mm: Option<f64>`: the point, depth-derated like the band. At most one of the two is `Some`. Literals to update: mod.rs:2721, provenance.rs:388, diagnostics/tests.rs:649. `suggest/apply.rs` L664 also clears the new field. |
| Rubbing floor | `feeds/mod.rs` L1149-1233 | New `rubbing_floor_at(band, point)`: `min(0.025, point)` with new `RubbingFloorSource::PrintedPoint`. This is the R4 Q9 rule 2 that Suggest could never reach, because `RequireBoth` dropped points. Keep `rubbing_floor(band)` as it is. Callers: mod.rs L2593, `suggest/adaptive_entry.rs` L465, `efficiency.rs` L253. For the point at a changed depth, add `chip_point_for_dpp` next to `suggest/axial_envelope.rs` L368; it reads `context.matched_lut_row`, so the 32 `SuggestContext` literals are untouched. Every point in the matrix is at least 0.047, so the floor stays 0.025 there. It drops only on SpeTool 0.5 and 0.794 mm tips (0.0178 and 0.0203), which is warn-only. |
| Burn gate | `tool_load/chipload.rs` L597-633 | A point gives `ChipBounds { min: Some(v′), max: v′, source: VendorLutPointPreset }`, where v′ is v depth-derated. The high side stays a hard trip at the printed value (today's behaviour). The low side is an advisory only, never `Exceeds(Low)` (`verdict.rs` L1292). The variant's `row_id` string is unchanged; its doc (L1273-1277) becomes "one printed value (A2)". The 8 AMS-159 cells behave exactly as today. The 75 Spektra and 5 SpeTool cells gain a burn advisory when the median chip is below v (see Q1). |
| Optimizer preflight | `tool_load/optimize/preflight.rs` L99-100 | Checks for bipolar engagement only when the row is `Band`. The 48 equal-limit rows no longer refuse on any spread either side of v. |
| Min-DOC chipload floor | `feeds/cutter_constraints.rs` L219-220 | Reads a `Band` minimum only. It already returns `None` for flat, bull and V-bit tools (L405-410). Only the 3 grade-c idcwoodcraft ball rows lose this floor, and no FM1 cell resolves them (Q8). |
| Modulator | `dressup/feed_modulation.rs` L117-146, L413-600, L653-700 | `ChiploadBand` gains a private `point: bool`, plus `ChiploadBand::point(v)` (stores min = max = v) and `is_point()`. Every outside construction uses `::new`, so this is additive. `new(v, v)` is unchanged (test L1257). **ConstrainedMax:** v × rpm × z is the chipload cap. Deflection, power, machine and reach caps are unchanged. The feed scale applies. The step-6 floor (L~575) is skipped for a point, because nothing printed a minimum. The deflection and power "pin" arms pin to v, since min = v. **BandMid:** target and ceiling are v, no floor. **Tolerance:** none, so the card shows none. The `(_, None)` bandless arm (L766) stays for rows with no chipload. `ModulationContext` and `modulate_annotated_against_trace` keep their types. |
| Envelope resolver | `tool_load/mod.rs` L267-373 | New `pub enum ChipTarget { Band(Range<f64>), Point(f64) }` with `modulation_band() -> Option<ChiploadBand>`. New `chip_target_for_toolpath` (today's body at L302-373, reading `printed_chipload`). New `modulation_bands_for_session(session, trace) -> HashMap<ToolpathId, ChiploadBand>`. `chipload_envelope_for_toolpath` and `chipload_envelopes_for_session` become band-only wrappers with unchanged signatures, so the viz consumers are untouched. |
| Viewport | via the `Range` wrapper | A point is absent from the map, so it draws grey. The 8 AMS-159 cells go from coloured against v..v to grey (Q3). |
| Advisor | `session/compute.rs::optimized_candidate` L1527-1535 | Uses `chip_target_for_toolpath(..)?.modulation_band()?`, so point cells are now optimised. With no row at all it still returns `None` (Q6). |
| Card and chart | `feeds/explain_payload.rs` L105-124; viz `ui/feeds/shared.rs` L383-391 | `published_chipload_band()` reads `printed_chipload()`. New `published_chipload_point()`, which `vendor_single_value` reads. The chart already draws one line and no band lines. The optional text in `why.rs` L414 is "one printed value, held as a point; no band". |

## 3. Functions to edit in `session/`

- `session/compute.rs::optimized_candidate` (L1496-1566): replace L1527-1535, and update the doc at L1482-1495 and the comment at L1536-1540.
- `session/compute/simulation.rs::ProjectSession::apply_adaptive_feed_modulation` (L481-): L489 (`use`), L498 (`modulation_bands_for_session`), L541-544 (`envelopes.get(&toolpath_id).copied()`).
- The test `session/compute/tests.rs::advisor_modulates_candidate_feeds_before_timing` (L1646-).
- Not touched: `SimulationOptions`, `AdoptSimulationArgs`, `modulate_annotated_against_trace`, `ModulationContext`, and the other files that session is editing.

## 4. Step split (each step leaves the tree building)

1. **Feeds and gate:**
   - The enum and method, and the `build_result` normalization.
   - The gate, preflight and `cutter_constraints` changes.
   - The `FeedsResult` field and its 3 literals.
   - `RubbingFloorSource::PrintedPoint` and `rubbing_floor_at`, with its 3 callers and `chip_point_for_dpp`.
   - `explain_payload` and viz `shared.rs`.
   - The `match` arm at `tests/rubbing_floor_envelope_band_p1.rs` L527, so `cargo test --no-run` still compiles.
2. **Modulator and envelope:** `ChiploadBand::point`, the point arms in `max_safe_feed_for_move` and `band_mid_feed_for_move` with unit tests in the same file, and `ChipTarget` plus the two new resolvers in `tool_load/mod.rs`. Nothing calls them yet, so behaviour is unchanged.
3. **The two `session/` sites and the advisor test.** At L1686-1710, the softwood 6 mm adaptive arm goes from `is_none()` to `expect(..)` and asserts `changed > 0`. The aluminium two-limit arm stays as the band witness.
4. **Sentries and docs:**
   - The new sentry and the test changes in section 5.
   - `feeds/CLAUDE.md`: a new invariant ("a single printed value is a point: `printed_chipload()`; no band is derived; the modulator caps at it with no floor; the gate is hard above it and advisory below it") and the sentry list.
   - The FM1 re-run (the orchestrator runs it) and the landing table in the planning package.

## 5. Tests and sentries

- **New `tests/a_printed_value_is_held_as_a_point_a2.rs`:**
  - (a) Every LUT row that prints one value resolves as `Point`, with `chip_load_min_mm == None`.
  - (b) For the AMS-159 60° hardwood Trace cell and the Spektra 6 mm hardwood Pocket cell: Suggest bounds are `None` and `chipload_point_mm` is Some(v); the gate has min = max = v with source `PointPreset` and an advisory low side; the target is `Point`; the envelope `Range` is `None`; `modulation_band().is_point()`.
  - (c) The modulator caps at v·rpm·z; feed scale 0.7 emits 0.7 × that with no floor; the edge-force arm pins to v.
  - (d) Floor is 0.01778 with source `PrintedPoint` on the SpeTool 0.5 mm tip; a point of 0.025 or more gives the constant.
  - (e) Suggest outputs (rpm, feed, plunge, stepover, depth per pass, chip load) are identical to a pre-A2 snapshot for the 88 point cells in FM1.
- **`drop_cutter_flat_roughing_row_g_dcflat.rs`:** set `EXPECTED_ROW = "amana-flat-plywood-hardwood-pocket-6000-2f-spektra"`. The old id is still in `amana_flat_end.json` but no longer wins. Replace the "needs BOTH bounds" assertion with `printed_chipload() == Point`. Raw values: min `None`, max `Some(0.127)`. Route identity is kept.
- **Advisor test:** as in step 3.
- **Census `lut_resolver_census_a6`** L697-699: add `s.printed_chipload()` is `Point`.
- **`g_chartlines`** (viz): arm 2's min == max case stays `None`. Add a check that `published_chipload_point()` is `Some` for maximum-only and equal-limit rows, and `None` for a two-limit row.
- **FM1** (`feeds_matrix_instrument_fm1.rs` L284-285, L511-512): add a `chipload_point_mm` column.
- **Re-run, because modulated feeds or advisor timings move:** `adaptive_feed_modulation_pipeline_f036b`, `arc_fit_disposition_a5`, `strategy_advisor_smoke`, `strategy_comparison_h4`, `every_consumer_reads_one_claimed_band_g1`, `chipload_advisory_disclosure_h4`, `rubbing_floor_*`. `wanaka_suggest_integration` takes minutes, so ask first.

## 6. Matrix effect

**The current CSV has 88 point cells:**
- 75 Spektra EndMill/BullNose cells that print a maximum only (0.1016, 0.127 and 0.1524).
- 5 SpeTool TaperedBallNose 3.175 mm MDF cells (0.1016), added by P1 and not among G4's 83.
- 8 AMS-159 V-bit cells with equal limits (0.0762, and 0.0471 when scaled to 6.35 mm).

**What does not move:**
- **Recipe numbers:** rpm, feed, plunge, stepover, depth per pass and chip load all stay. `chip_load_mm` is v under both old encodings, `chipload_bounds` feeds only warnings and display, and the min-DOC floor is `None` for V-bits.
- **Warnings:** every point in the grid is at least 0.025, so the floor stays 0.025. The A2 ruling does not require any recipe number to move.

**What moves:**
- **FM1 bound columns:** `chipload_bounds_min/max` go blank on the 8 AMS cells. The new `chipload_point_mm` column is filled on all 88.
- **Efficiency verdict:** it becomes `NoBand` on those 8 cells. Only the viz compare table shows this; there is no diagnostic id.
- **Simulation and advisor behaviour:**
  - Point toolpaths now modulate capped at v, where the 80 used to run bandless.
  - The 8 AMS cells lose their v floor.
  - The advisor now optimises point cells.
  - The sim CSV's 6 mm point cells read `Unmodeled`, so they show nothing; the flipped advisor test is the witness.

## 7. Decisions for you

- **Q1 – burn side of a point.** (a) Advisory below v: this is today's AMS behaviour applied to all 88. (b) Unmodelled: today's Spektra behaviour; the 8 AMS cells lose their advisory. I recommend (a). The evidence leans towards "start", and below a start value is the burn side. It is advisory only, so no verdict flips, but new advisories will appear on Spektra cells.
- **Q2 – high side stays hard at v.** This is the "top" reading. The ruling only says no band is derived, and a softer or higher cap needs a second witness (rule b). I recommend keeping it hard; it moves nothing.
- **Q3 – viewport for a point.** Grey (default), or a two-class colouring for above and below the point?
- **Q4 – where the normalization lives.** In code (`build_result`, as designed), or by also dropping the duplicated `chipload_min` from the 48 LUT rows, plus a loader sentry? Neither moves a number.
- **Q5 – the 7 D1 repo-authored Spektra band rows.** These are the `hardwood-contour-3175` cells, today 0.017-0.028. Turning them into points moves chip load about 4.5x, to 0.1016. I recommend keeping them out of A2 and landing them separately with an FM1 diff.
- **Q6 – advisor with no chipload row at all.** Keep refusing (recommended), or go bandless like the sim pass? The ruling covers points only.
- **Q7 – binding tag for a move held at the point.** Keep `ChiploadMax` (recommended), or add `ChiploadPoint`, which touches serialized traces and viz matches?
- **Q8 – min-DOC floor on the idcwoodcraft grade-c ball rows (A2: no minimum).** A point gives no floor, so these rows lose it. The ball cells that could resolve them are 3.175, 6.35 and 12.7 mm in hardwood. None resolves them in FM1, but a depth per pass could move outside the grid.

### Critical files for implementation
- /home/ricky/personal_repos/rs_cam/crates/rs_cam_core/src/feeds/vendor_lookup.rs
- /home/ricky/personal_repos/rs_cam/crates/rs_cam_core/src/dressup/feed_modulation.rs
- /home/ricky/personal_repos/rs_cam/crates/rs_cam_core/src/tool_load/mod.rs
- /home/ricky/personal_repos/rs_cam/crates/rs_cam_core/src/tool_load/chipload.rs
- /home/ricky/personal_repos/rs_cam/crates/rs_cam_core/src/feeds/mod.rs
- /home/ricky/personal_repos/rs_cam/crates/rs_cam_core/src/session/compute.rs
- /home/ricky/personal_repos/rs_cam/crates/rs_cam_core/src/session/compute/simulation.rs