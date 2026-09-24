# P1 plan (planner's design) and the orchestrator's decisions

## Orchestrator's decisions on the planner's open points (2026-09-24)

1. Load the 54 micro tapered rows only (Amana ZrN v8 27 + SpeTool tapered 27).
   The 42 SpeTool spiral flat rows wait (RULINGS.md, note under B1).
2. Form C on a tapered ball at 1.5 mm or more: allowed inside 0.5-2x, but the
   spread is PER TOOL FAMILY: flat (0.29, 1.25), tapered (0.00, 1.06). Ball,
   bull and V-bit have no series: form C uses the flat spread and the card says
   "spread borrowed from flat end mills". Pin the constants in a sentry.
3. D1 rows: delete the four tapered D1 rows, but KEEP ball_nose copies for the
   cells that name the straight ball 46471 (1.0 mm 2F and 0.794 mm 3F), with a
   note, so a 1 mm straight ball does not refuse for no data reason.
4. The MDF tapered anchor change (about 11 cells): accept; record it in the
   FM1 diff. No "prefer a banded row" tie-break in P1 (that is A2 territory).
5. The acceptance tie (Amana v8 vs SpeTool at 1825) resolves by id. Pin the id
   AND the band in the acceptance sentry.
6. A Refused cell's gate verdict must not report under a `*.within` id; assert
   it in the step 3 sentry.
7. "One step" = the adjacent printed ratio at that end of the series (as §2.3).
8. Design departure from PLAN §4, to record in EXTRAPOLATION_G1 §5: the claim
   runs inside `build_result` on every off-size row match, not only when no row
   matches.
9. Step 4 (compare.rs, why.rs) waits for a check that the feeds/dial session
   has committed and left those files.

---

## The planner's design: the tapered tip key (A1), the G1 micro rows, the `Extrapolation` trait with the G1 size claim, and the card line

## 0. Four findings to settle before editing

1. **The trait does not fire for the acceptance case.** After A1 and the row load, tp 11 queries (TaperedBallNose, 1.0 mm, 2 flutes, Hardwood 1450, Parallel/Finish). The Amana v8 hardwood 1.0 mm 2-flute row (derived/b) exists. Its score is 1000+220+70+30+80+200+80+45+100 = 1825, and the raw ratio is 1.0, so the arm is `VendorBacked` with the band 0.01905–0.0508. The SpeTool 1.0 mm 2-flute hardwood row also scores 1825. The tie goes to the smaller id (`beats`, `vendor_lookup.rs:564-572`), so the ids must keep `amana-…` sorting before `spetool-…`. The trait serves the other off-size cells.
2. **A1 has four lookup call sites, not one.** Suggest keys at `vendor_normalize.rs:199-206`. The gate, the viewport and the optimizer key at `tool.lookup_diameter_at(doc)`: `tool_load/chipload.rs:550`, `tool_load/mod.rs:348`, `tool_load/optimize/context.rs:103-108`. If only Suggest changes, `lookup_parity` and the census (`lut_resolver_census_a6`, the assert at ~531) break, and Suggest and the gate read different rows.
3. **I recommend loading only 54 of the 96 verified rows in P1. That departs from B1, so the orchestrator must rule on it.** The 42 SpeTool spiral rows are flat end mills from 1/16 to 1/2 in, not micro rows. They are exact/b and the Spektra wood rows are derived/b. A SpeTool 6.35 mm hardwood pocket row scores 1858, above the printed Spektra 6.0 mm row at 1825. So every 3.175 mm and 6 mm solid-wood pocket-family cell changes vendor. Up-cut, down-cut and compression tie at one size, so the id spelling picks the row (compression). This breaks `a5_band_divergence_is_the_reroute_not_the_entry_point`, which pins `amana-flat-hardwood-pocket-6000-2f-spektra`. It also moves the wanaka 6 mm roughing pins. Land those rows later, with the re-pins and an FM1 diff.
4. **Some claim outcomes must refuse without shipping a number.** The trait therefore returns three outcomes, not an `Option`. FM0 and wanaka need the refusal text. A refused result sets its band to `None`, so no consumer can use it.

## 1. How the lookup picks a row today

- **Scoring:** `score_observation` (`vendor_lookup.rs:695-755`) gives:
  - base 1000;
  - tool family: +220, or a fallback score (tapered→ball +110);
  - row kind: exact 120, derived 70, fallback 30;
  - grade: a 60, b 30, c 10;
  - flutes: equal +80, off by one +30, otherwise −20;
  - **diameter term** `(1 − |ln(q/row)|/ln 2)·200` clamped to 0–200 (l.712-718);
  - hardness up to +80, subfamily +50;
  - pass role +45 if equal, −25 if not;
  - material family +100 if equal.
- **Filter:** the must-match filter (l.652-693) checks operation family, material category and tool family, and only a 0.1–10× diameter sanity window.
- **Selection:** `lookup_best_where` (l.579-609) and `find_best_vbit_row_where` (l.165-205) keep the best row, then call `build_result` (l.510-557).
- **Scaling in `build_result`:**
  - `diameter_ratio_raw` (l.522, clamped 0.1–10);
  - `is_extrapolated` reads the raw ratios, with ln 1.4 as the threshold (l.524, rider sentry);
  - `diameter_scale = raw^0.61` (l.525, `CHIPLOAD_DIAMETER_EXPONENT`, l.367);
  - `total_scale = diameter × hardness` multiplies min, mid and max uniformly (l.530-532).
- **Size rule:** `support_for_lookup` (`support.rs:379-393`) calls `micro_extrapolation_refusal(tool, nominal, query.diameter_mm, row_d)` (l.138-166). It refuses when the key is under 1.5 mm and the row is outside 0.5–2×. The rule acts only in Suggest's support arm. **The gate never applies it**: it judges the micro tool against the 0.61-scaled band.
- **Consumers:**
  - Suggest: `recipe_row_lookup` (`support.rs:318-333`) → `find_best_row_for_geometry`.
  - Explain: `explain_payload.rs:144`, a second lookup through the same entry point.
  - Gate, viewport, optimizer, advisor and modulator: `matched_chip_envelope` (`chipload.rs:113-151`) → `find_best_chip_envelope_row`. The modulator reaches it through `chipload_envelope_for_toolpath` (`tool_load/mod.rs:302`), called at `session/compute/simulation.rs:544` and `session/compute.rs:1527`.
  - **All of them pass through `build_result`.** That is the one place to put the claim so that every consumer reads one number.

## 2. Design

### 2.1 A1: the lookup key (`feeds/geometry.rs`, next to `depth_cap_diameter_mm` at l.354)
```rust
pub fn lut_key_diameter_mm(geom: ToolGeometryHint, ap: f64, nominal: f64, shank: f64) -> f64
pub fn lut_key_diameter_for_cutter(cutter: &dyn MillingCutter, ap: f64) -> f64
```
- Tapered ball: the nominal tip.
- V-bit: the engaged diameter, unchanged (G5/B4 later).
- Flat, ball, bull: nominal, unchanged.

Callers:
- `vendor_normalize::lookup_diameter_for_input` (l.199-206).
- `chipload.rs:550`. Keep l.579-580: the depth-derate denominator stays at the cone.
- `tool_load/mod.rs:348`. Keep l.358.
- `optimize/context.rs:106`.
- `LutBandStage.queried_diameter_mm` (`chipload.rs:848`) becomes the key. Fix the doc at `feed_explanation.rs:133-134`.

Do not touch:
- `feed_ladder_diameter_mm`, `depth_cap_diameter_mm`, `mod.rs:2541` (the band derate) and `session/compute.rs:1832` (deflection). The depth half of R2 stands, and FM3 and FM6 pin it.
- `mod.rs:1455-1459`. The engaged diameter stays for the SFM, the RPM and the formula. Update the comment at l.1437.

### 2.2 Types: new `feeds/extrapolation.rs`, plus `feeds/extrapolation/size.rs` for G1
```rust
pub enum Gap { Size }                       // G1; G2.. later. fn group() -> "G1"
pub enum ClaimConfidence { OneWitness, TwoWitnesses }   // not `Confidence` (clashes: tool_load::verdict, diagnostics)
pub enum SizeForm {
    Interpolated { lo_mm: f64, hi_mm: f64 },               // form A
    SeriesSlope { slope: f64, r2: f64, sizes: usize },     // form B
    GenericFallback { exponent: f64 },                     // form C (0.61)
}
pub enum ClaimResidual { Bracket { lo_value: f64, hi_value: f64 }, FitRms { fraction: f64 }, VendorSpread { lo: f64, hi: f64 } }
pub struct Claim { gap: Gap, form: SizeForm, rule: &'static str, scale: f64,
    anchor_diameter_mm: f64, query_diameter_mm: f64, source_rows: Vec<String>,
    range_mm: RangeInclusive<f64>, residual: ClaimResidual, confidence: ClaimConfidence }
impl Claim { pub fn card_text(&self) -> (String /*headline*/, String /*detail*/) }
pub enum SizeBasis { Exact, NoDiameterAnchor, NoChipload, Claim(Box<Claim>), Refused { gap: Gap, reason: String } }
pub trait Extrapolation {
    fn gap(&self) -> Gap;
    fn basis(&self, lut: &VendorLut, query: &LookupQuery, anchor: &VendorObservation) -> SizeBasis;
}
pub struct SizeLaw;  // the G1 impl
```

### 2.3 The G1 rule (`SizeLaw::basis`), in order
The key `d` is the query diameter; `d_row` is the anchor's diameter.

1. No `diameter_mm` gives `NoDiameterAnchor`. No chipload (an RPM-only anchor) gives `NoChipload`, so today's RPM carry keeps shipping.
2. If `|d/d_row − 1| ≤ 1e-3`, the result is `Exact`.
3. A tapered ball with `d < 0.5` (`TAPERED_MIN_TIP_MM`) gives `Refused`.
4. **The series** is the set of rows that match the anchor on (source_id, tool_family, tool_subfamily, material_family, flute_count, operation_family, pass_role), with a diameter, a chipload, grade a or b, and `row_kind` other than fallback, deduplicated by diameter. A grade-c anchor has an empty series.
5. **A:** if two adjacent sizes of the series bracket `d`, interpolate the band mids log-log. `scale = m(d)/mid(anchor)`, range lo..=hi.
6. **Tapered ball under 1.5 mm:** if A fails on the anchor's series, try A on any other grade a/b series with the same (Tapered, material, flutes, op family, role), because the ruling reads "inside *a* chart's printed tips". If that also fails, `Refused`: the ruling says a tapered ball ships only inside a chart's printed tips.
7. **B** (not tapered, series of 3 or more sizes): `d` may sit up to one printed step beyond the span, that is `≥ d_min²/d_2` or `≤ d_max²/d_{n−1}`. Fit OLS on the log mids and state r² and the rms. `scale = (d/d_row)^slope`.
8. **C:** if `r = d/d_row` lies in [0.5, 2.0], inclusive with a 1e-9 tolerance, `scale = r^0.61`. The spread is `r^(0.29−0.61)` to `r^(1.25−0.61)`, which gives 0.80–1.56 at 2×. New constants `GENERIC_SLOPE_SPREAD = (0.29, 1.25)` come from EXTRAPOLATION_G1 §3.1.
   - A V-bit gets C with no window (the micro rule only under 1.5 mm) until B4.
   - A tapered ball at 1.5 mm or more also gets C (see open question 3).
9. Otherwise `Refused`:
   - under 1.5 mm, with the existing `micro_extrapolation_refusal` text;
   - otherwise with a new text that starts "no published figure for a …" and names G1, so `is_size_refusal` in FM0 and wanaka keeps working.

Where the claim plugs in, with no parallel flow:
- `build_result` takes `lut` (pass it from l.203 and l.607) and calls `SizeLaw.basis`.
- For `Claim`, `diameter_scale = claim.scale` replaces l.525. The one scalar multiplies min, mid and max, so `vendor_sidebyside`'s derate identity `bounds.min/(raw_min·total_scale)` still holds, and so do `combined_scale` in the UI and `LutBandStage.diameter_scale`.
- For `Refused`, `chip_load_mm = 0`, `min = max = None`.
- `is_extrapolated` stays on the raw ratios, as the rider requires.
- New `LookupResult` fields: `size_basis`, `material_label`, `evidence_grade`, `row_kind`. The last three are the A4 channel, and FM1 wants them too.
- Because both resolvers pass through `build_result`, Suggest's `chipload_bounds`, the gate, the modulator, the advisor and the viewport all see the claimed band.

### 2.4 The support arm (`support.rs`)
- `FeedsSupport` (l.287-295): drop `Eq` (a `Claim` holds f64; `assert_eq!` still compiles) and add `Extrapolated { claim: Box<Claim> }`.
- `support_for_lookup` (l.379-393) maps the basis:
  - `Claim` → `Extrapolated`;
  - `Refused` → `Refuse`;
  - everything else → `VendorBacked`.
  - `micro_extrapolation_refusal` stays public and becomes the text for form C refusals under 1.5 mm.
- `calculate` (`mod.rs:1597-1700`): a `Refused` row takes the formula-fallback tuple with no vendor RPM and **without** `VendorRowPublishesNoChipload`.
- `matched_chip_envelope` (`chipload.rs:150`) returns `None` on `Refused`. That fixes today's gap where the gate judges what Suggest refuses. The census subset invariant still holds.
- Add `FeedsSupport::card_text()` for the MCP and FM1.

### 2.5 Surfacing
Do not add a `FeedsWarning` variant. It would ripple into `from_feeds.rs`, `ids::ALL`, the `g_visible` sentry and `why.rs`.
- **compare.rs l.273-282:** replace `approx ×{combined_scale}` with `claim.card_text()` as a visible wrapped line, with the detail on hover. When `row_kind == Derived`, add `material_label` as the A4 note.
- **why.rs l.505-509:** one string. After A1, "Every advance/tooth figure … computed at the engaged diameter, not at the tool tip" is false for a tapered ball. The new text: the row is read at the tip, and the depth ladder and deflection use the engaged diameter.
- **shared.rs, feeds_speeds.rs:** no change.
- **MCP:** in `rs_cam_viz/src/app/mcp/generation.rs:43-53` (`mcp_get_suggest_rationale`), add `"basis": profile.feeds.map(|f| f.support.card_text())`. No other MCP read of the basis exists, and the `add_toolpath` refusal already carries its text (`commands.rs:816`).

## 3. Steps (each leaves the tree building)

**Step 1: data only.**
- `vendor_lut.rs`:
  - add `Vendor::Spetool` (`#[serde(rename="spetool")]`, Display "SpeTool") at l.11-57;
  - add EMBEDDED_FILES entries `amana_zrn_tapered_v8.json` (27 rows) and `spetool_tapered.json` (27 rows);
  - change the count at l.640-655 from 389 to **437** (389 − 6 + 54).
- Delete the six D1 rows in `amana_ball_nose.json` l.355-518. The v8 rows reprint the same cells. Filed as tapered, the D1 rows would stay exact/a and beat v8's derived/b (1905 > 1825).
- Convert the verified rows:
  - drop the `x-g1-` prefix, giving ids such as `amana-tapered-hardwood-parallel-1000-2f-zrn-v8` and `spetool-tapered-hardwood-parallel-1000-2f`;
  - strip `verification`, `verbatim`, `extrapolation_group` and `tip_diameter_mm`. The schema documents `tip_diameter_mm` as a V-bit flat tip, and after A1 `diameter_mm` already is the tip.
  - For softwood and hardwood rows, write `material_label` as the card sentence: "Wood, MDF, Sign-Foam (one printed row); hardwood is not printed apart" (A4).
- Manifest:
  - `amana_zrn_3d_profiling_v8` (l.221-228): add `stored_text`, `pdf_sha256` 5cdfb9c0…, `stored_on`, and notes on the tip frame, the A4 derived b rows and the 1/16 in cell at grade b;
  - `amana_zrn_3d_profiling` (l.26-35): supersede the "HARDWOOD AT SUB-1MM IS A DOCUMENTED GAP" note (A4) and record the D1 removal;
  - new entries `spetool_2d3d_tapered_router_bit_chart` and `amana_46xxx_tool_identity`;
  - copy `fetch/G1/sources/{amana_zrn_3d_v8,spetool_2d3d_tapered,amana_46xxx_identity}.txt` into `data/vendor_lut/sources/`.
- Test edits:
  - `vendor_lookup.rs:1017-1056`: the row id becomes the v8 softwood row;
  - `vendor_lookup.rs:~1164`: 0.5 mm now matches SpeTool 0.5 exactly, so rewrite it;
  - `tests/vendor_lut_sub_1mm.rs`: the id, the 0.5 mm test, and the count, which is **252 today and already red**.

**Step 2: A1 and the tip floor. The acceptance case passes here.**
- The §2.1 helper, applied at the four sites.
- `support.rs`: tapered key < 0.5 refuses; docs at l.126-136.
- `why.rs` string.
- Tests:
  - `lookup_parity`: add a tapered case built through the key helper (l.159-168);
  - FM9: the "(engaged " text (l.150-156);
  - wanaka: `refused_ids` becomes `[7, 14]`; re-add a tp 11 arm (the Amana v8 row, band 0.01905–0.0508, `VendorBacked`) at l.262-292 and l.730-739;
  - `rubbing_floor_envelope_band_p1` (l.290-420): re-premise it on the Amana 4-flute 1.5 mm row (max 0.0165);
  - check the tapered cases in `tapered_width_model_parity_c3` (l.185-199) and `feed_explanation_snapshot_b3`.
- Re-run FM1: the 112 tapered cells move. The 3.175 mm tip scales ×1.0; the 6.0 mm tip scales ×0.966.

**Step 3: the trait, the claim and the arm.**
- New `extrapolation.rs` and `extrapolation/size.rs`.
- `LookupResult` gets its four new fields. Update the 8 literals: `provenance.rs:349`, `cutter_constraints.rs:544`, `suggest/tests.rs:29`, `optimize/bounds.rs:479`, `stage1_grid_tests.rs:184`, `headroom.rs:590` and `:640`, `tests/feed_explanation_snapshot_b3.rs:364`.
- `build_result`, the `FeedsSupport` arm, `calculate`'s refused branch, `matched_chip_envelope`.
- Tests:
  - FM0 (l.206-229) accepts `Extrapolated`;
  - FM5 (l.139-145);
  - FM1 `support_columns` (l.306-310) gets the new arm plus columns `diameter_ratio_raw`, `claim_form`, `claim_scale`, `claim_range`, `claim_residual`;
  - FM9 `a_micro_tool_with_a_near_row_ships`: 1.0 mm flat on the Spektra 0.794/1.5 series becomes `Extrapolated` form A.
- New sentries:
  - `a_tapered_row_is_read_at_the_tip_a1`: the Suggest key equals the gate key, and the derate denominator is still the cone;
  - `the_micro_tapered_finish_ships_the_printed_tip_row_g1`: the unit-level acceptance, including SpeTool 0.0254 lying inside the Amana band and the A4 label;
  - `a_size_claim_states_its_rule_range_and_residual_g1`, with pinned numbers for:
    - form A: 1.0 mm flat, softwood;
    - form B: 6.0 mm flat on Onsrud 60-100mw, slope about 0.40, one step down to about 4.2 mm;
    - form C: 3.175 mm ball on the grade-c 6.0 mm row, ratio 0.529;
    - refusals outside the window, for a tip under 0.5 mm, and for a 1.0 mm tapered scallop;
    - the inclusive edges: V-bit at 0.5 and at 2.0;
  - `every_consumer_reads_one_claimed_band_g1`: Suggest's pre-derate band equals the envelope resolver's band for a claimed cell;
  - constant pins: 0.61, (0.29, 1.25), 0.5, and [0.5, 2].

**Step 4: surfacing and the record.**
- compare.rs line; MCP `basis`.
- `feeds/CLAUDE.md`: file map and sentries.
- Re-run FM1 and commit the CSV. Write `EXTRAPOLATION_G1.md` §5, listing the cells that moved.

**Later, after a ruling:** the 42 SpeTool spiral rows, with the a5 and wanaka re-pins.

## 4. Risks and open questions (counts from the committed matrix CSV)

1. **Cells that change arm.** About 110 shipping cells sit off their row's size, and all are inside 0.5–2×: ball 24, bull 12, flat 16, tapered 56, V-bit 2. They become `Extrapolated`, except the V-bit on an RPM-only row, which stays `NoChipload`. **No matrix cell newly refuses.** Outside the matrix, B1's window now refuses tools more than 2× from any row beyond one series step, at every size. The micro rule never did that. Wanaka tool 4 (25.4 mm) is used by no toolpath.
2. **The MDF tapered parallel cells change anchor** when the rows load. About 11 cells:
   - at 3.175 mm, SpeTool 2-flute (point 0.1016) beats Onsrud 1/8 in 3-flute on the flute term;
   - at 6.0 mm, Amana v8 MDF ties Onsrud 1/4 in (both a/exact with hardness 1100) and wins on the id.
   Accept this, or add a "prefer a banded row" tie-break (A2 point-mode territory).
3. **A tapered ball at 1.5 mm or more, off-size.** The 56 cells at a 6.0 mm tip sit on Onsrud 6.35 mm, a one-size series.
   - My default is C with the spread stated (×0.966, spread ×0.96–1.02).
   - EXTRAPOLATION_G1 §3.1 says form C is not supported by the tapered set.
   - If C is ruled out, the Pocket, Adaptive and Scallop tapered cells refuse (about 36 cells). The Parallel cells could only take cross-chart A from Amana v8 (mid about 0.198 against 0.147).
4. **Deleting the D1 rows drops the straight ball 46471.** The 1.0 mm 2-flute and 0.794 mm 3-flute Parallel cells in softwood and MDF also list a straight ball, 46471. A 1 mm straight ball on Parallel then refuses under the micro rule. Keep two ball_nose copies that name 46471 only?
5. **V-bit is exempt from the generic window in P1.** The gate keys a V-bit at the engaged width at peak depth, so windowing there would turn judged V-bit verdicts into `Unmodeled`. B4 settles this.
6. **The acceptance tie between Amana and SpeTool is decided by id.** Pin it in the sentry. "SpeTool as a second vendor" is a data sentry only; `TwoWitnesses` stays unused in P1.
7. **"One step"** in form B is defined here as the adjacent printed ratio at that end of the series. PLAN only says "one modest step".
8. **Runtime.** `wanaka_suggest_integration` takes minutes, so ask the operator before running it. Run the rule-6 conflict detector test after the load. The Amana v8 subfamily must differ from the existing derived-c `amana-tapered-*` rows.

### Critical files for implementation
- /home/ricky/personal_repos/rs_cam/crates/rs_cam_core/src/feeds/vendor_lookup.rs
- /home/ricky/personal_repos/rs_cam/crates/rs_cam_core/src/feeds/support.rs
- /home/ricky/personal_repos/rs_cam/crates/rs_cam_core/src/feeds/vendor_normalize.rs (plus `feeds/geometry.rs` for the key helper)
- /home/ricky/personal_repos/rs_cam/crates/rs_cam_core/src/tool_load/chipload.rs
- /home/ricky/personal_repos/rs_cam/crates/rs_cam_core/src/feeds/vendor_lut.rs (plus `data/vendor_lut/observations/`, `source_manifest.json`, `sources/`)