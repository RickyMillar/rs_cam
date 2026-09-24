# B4 plan (G5 V-bit key): the planner's design and the orchestrator's decisions

## Orchestrator's decisions (2026-09-25; the operator took the recommendations)

- Q1: the 8 6.35 mm MDF / plywood V-bit cells refuse (a 25.4 mm row is 4x
  the tool); no single-flute row serves a two-flute tool without a ruling.
- Q2: the Onsrud 37-series softwood / hardwood rows stay parked.
- Q3: Onsrud rows (no printed RPM) keep the engine RPM; a V-bit RPM claim
  is a later package.
- Q4-Q7: separate clean-ups, recorded, not in B4.
- The Amana AMS-159 and Spektra-engraving rows are transcribed as printed:
  no cutting diameter (angle key), Operating RPM 18 000.

---

# B4 design: key each V-bit row at its printed key (cutting diameter or angle), one row for Suggest and the gate

The design has three parts:
1. Look up every V-bit at its nominal cutting diameter, at every lookup site.
2. Make the Amana V-bit rows angle-keyed by copying what the chart prints: no cutting diameter, and an operating RPM of 18,000.
3. For a V-bit, pick the chip row first in the recipe lookup, so Suggest and the gate read the same row.

The band de-rate moves to the nominal diameter. The surface-speed RPM, the formula chip and the deflection stay at the engaged width, as under A1.

**Matrix effect:** 26 V-bit cells move and 8 of them become refusals (410 → 418 refused). The largest single move is ×2.64 on the 6.35 mm and 12.7 mm softwood VCarve cells.

## What I checked (read-only)
- `fetch/G5/sources/amana_ams159_vgroove_v2.txt` prints "Operating RPM: 18,000". `pdftotext` on the Spektra v4 PDF prints "CNC Operating Spindle Speed: 18,000 RPM". All 16 wood rows from these two charts in the LUT carry `rpm_nominal: None`.
- `source_manifest.json` (AMS-159) says: "Chart states NO cutting diameter; diameters from per-SKU specs". So the `diameter_mm` of 12.7, 9.525 and 6.35 on those rows is not printed.
- I simulated the lookup scorer, `passes_must_match`, the angle gate, the tie-breaks and a copy of `SizeLaw` in Python. It reproduces today's matrix rows and scales (for example 0.4546^0.61 → 0.0471, and 0.25^0.61 → 0.0545).

## 1. The lookup key (one key for Suggest and the gate, following A1)

| Site | Today (V-bit) | B4 |
|---|---|---|
| `feeds/geometry.rs:399-401` `lut_key_diameter_mm` | engaged width | `nominal_d` |
| `feeds/geometry.rs:433-436` `lut_key_diameter_for_cutter` | `lookup_diameter_at(ap)` | `cutter.diameter()` |

These two functions feed all the lookup callers: `vendor_normalize.rs:201-208` (Suggest), `support.rs:517`, `tool_load/chipload.rs:554-555` (gate), `tool_load/mod.rs:454` (viewport and modulator), and `optimize/context.rs:113/121`. No code change is needed at those call sites.

Per site, where the engaged width stays or goes:
- **Depth ladder on the feed** (`geometry.rs:326`): already nominal for a V-bit. Unchanged.
- **Depth cap** (`geometry.rs:365`): already nominal. Unchanged.
- **Band de-rate:** moves to nominal (item 3). Unlike A1, B4 moves this one, because G5 §3.3 recommended it and the operator took the recommendations.
- **Surface-speed RPM and formula chip** (`feeds/mod.rs:1507-1521, 1563`): **keep the engaged width.** This follows A1 (P1_PLAN §2.1: "mod.rs:1455-1459 … stays for the SFM, the RPM and the formula"). Two tests depend on it:
  - Literature cell `vbit_60deg_vcarve_mdf` has an anti-pattern `rpm < 12000` of severity **major**, which blocks CI. At the nominal 6.35 mm the RPM would be 8522.
  - `feeds/tests.rs:727` `test_vbit_rpm_uses_engaged_diameter`.
- **Deflection** (`session/compute.rs:1846`): stays at the engaged width. A1 kept it there, and the file belongs to the other session.

Types (`feeds/extrapolation.rs:305-311, 328, 347, 359`): replace `SizeBasis::VBitExempt` with `SizeBasis::AngleKey { angle_deg: f64 }`. It is not refused, has no claim, uses scale 1.0, and `name()` returns `"AngleKey"` (this shows in the FM1 `claim_form` column). `SizeForm` doc l.127-128 needs updating.

`extrapolation/size.rs`:
- Delete step 0 (l.7-8 and l.362-368).
- In step 1 (l.370), before `NoDiameterAnchor`: if `family == ChamferVbit && anchor.diameter_mm.is_none()` and the anchor has `included_angle_deg = Some(a)`, return `AngleKey { angle_deg: a }`.
- A V-bit row with a printed diameter (Onsrud 37-50, 37-60, 37-80, and the Whiteside rows) then takes the normal G1 path: `Exact`, form A/B/C, or `Refused`. Form C already borrows the flat-end spread for V-bits (`spread_for`, l.279-282), which is what the card says.

`support.rs`:
- Delete the `VBitExempt` arm (l.548-560) and add `AngleKey` to the `VendorBacked` arm (l.561).
- The `Refused` arm (l.533-547) rebuilds the refusal text because the V-bit key could differ from the nominal diameter. The key now always equals the nominal diameter, so the arm can simply use `reason.clone()`.
- Keep `micro_extrapolation_refusal` as it is: FM9 calls it directly and pins its text.
- Fix the doc at l.161-174.

`vendor_lookup.rs`:
- **l.298-308:** V-bits get the nearer-diameter tie-break: replace `None` with `diameter_distance(criteria, obs)` on both sides, and delete the "until B4" comment.
- **l.363-377 (item 6):** the `VBit` arm becomes `find_best_vbit_row_where(lut, c, Some(angle), has_chipload).or_else(|| find_best_vbit_row(lut, c, Some(angle)))`. Lift the `has_chipload` closure from l.411-413 into a named function. An RPM-only row now answers a V-bit only when no row with a chipload matches.
  - This is a V-bit-only exception to the Checkpoint K rule, so the doc at l.321-362 needs a line about it.
  - `find_best_vbit_row` itself is unchanged, so `chip_envelope_lookup_skips_rpm_only_rows` (l.1705) stays green.
- Doc l.34-38 needs updating.

The new card line: add `pub fn vbit_key_text(tool: ToolFamily, row: &LookupResult) -> Option<String>` in `vendor_lookup.rs`. For `AngleKey` it returns "keyed at the printed angle 60° (the chart prints no cutting diameter)". Otherwise it returns "keyed at the printed cutting diameter 25.4 mm". Callers:
- `rs_cam_viz/src/ui/feeds/why.rs:553` `draw_row_basis_lines`, using `explain.query.tool_family`;
- FM1, as a new column `lut_key`.

**No new `LookupResult` field, so none of the 7 struct literals change.**

## 2. Does the V-bit leave `VBitExempt`? Yes

The only reason for the exemption was P1 risk 5: the gate keyed a V-bit at the engaged width. With B4 that reason is gone. What moves:
- A V-bit row with a printed diameter takes the full G1 claim, including the 0.5-2× window.
- A row keyed only by angle takes `AngleKey` and is unscaled at any size.
- On the matrix, the 6.35 mm tool in MDF and plywood matches `onsrud-{mdf,plywood-hardwood}-37-80-1-trace` (25.4 mm, ratio 0.25). It wins on the flute term, 1825 against 1775 for the single-flute 37-00 row. The size rule refuses it: "no published figure for a 6.35 mm V-bit; the nearest chart row is 25.4 mm, 4.0x the tool…". **These 8 cells go from shipping to refused.** See open question Q1.

## 3. The band de-rate at the nominal diameter (G5 §3.3)

Add a pair of helpers in `geometry.rs` next to `lut_key_*`:
- `depth_derate_diameter_mm(geom, ap, nominal, shank)`: the body of `feed_ladder_diameter_mm`, which should then delegate to it.
- `depth_derate_diameter_for_cutter(cutter, ap)`: the body of `depth_cap_diameter_mm`, which should then delegate to it.

Both give: tapered ball → engaged cone (R2 stands); V-bit, flat, ball and bull → nominal.

Change three call sites:
- `feeds/mod.rs:2591-2595` (`band_d`)
- `tool_load/chipload.rs:595`
- `tool_load/mod.rs:463`

Also fix the doc at `geometry.rs:306-308` and `348-350` ("the band de-rates a V-bit at its engaged width").

Effect: none at 53.13° and above, so no matrix cell moves. Below 53.13°, today's de-rate follows the angle alone (0.50 at 15°–18°); B4 makes it follow ap/D, and it is 1.0 up to 1 × D.

I am leaving one site: `suggest/axial_envelope.rs:382-385` de-rates through `SuggestContext.effective_diameter_mm`, which `apply.rs:167` fills with the chip-thinning diameter. That mismatch already exists for ball nose too (Q6).

## 4. RPM

**Recommendation: transcribe the printed RPM, and keep the engine rule for rows that print none. Do not pair a chip row from one vendor with an RPM-only row from another.**
- **Data:** set `rpm_nominal: 18000.0` on every AMS-159 and Spektra-engraving row (`amana_vgroove_engraving.json`, and the 3 rows in `amana_vgroove_aluminum_acrylic.json`). The header covers every material column. In the same edit, set `diameter_mm: null` on those rows and move the SKU diameter into `notes`. Update the two coverage notes in the manifest. Copy `fetch/G5/sources/amana_ams159_vgroove_v2.txt` into `data/vendor_lut/sources/`.
  - The diameter strip is load-bearing. Without it, at key 6.35 the AMS 45° single-flute row (SKU 6.35) ties the 60° two-flute row (SKU 12.7) at 1825 and wins on diameter distance.
- **Onsrud rows (no printed RPM):** keep the engine surface-speed rule at the engaged width. The code is unchanged.
- **Why not the pairing rule:**
  - After the transcription it moves no matrix cell. The Whiteside rows are filed as solid wood (material category 0), so they cannot pair with the Onsrud MDF or plywood rows, and in wood the AMS row now carries its own RPM.
  - Where it would act (the parked rows), it builds a feed no chart prints. Onsrud 37-00 single-flute 0.127 × Whiteside 22,000 × 2 flutes = 5,588 mm/min, about 2.4× Amana's printed 90 IPM for a 60° two-flute bit.
- **Engine rule at the printed key:** rejected. It fails the major (CI-blocking) literature anti-pattern named in item 1.

**Can the parked Onsrud 37-series softwood/hardwood rows load after B4? No. Keep them parked.** I simulated B4 with the diameter strip:
- `x-g2-onsrud-*-37-80-25400` scores 1905 against 1825 for AMS-159 60°. It wins on the +80 hardness term, because Onsrud rows carry a Janka value and Amana rows do not.
- At 6.35 mm the row is refused (0.25×), so 8 wood cells would refuse.
- At 12.7 mm it takes form C ×0.655 at the engine's 8000 rpm, which is ×0.49 of B4's feed.

So B4 removes the ×0.34 feed drop, but the rows would then refuse or halve the cells they win.

## 5. V-bit parallel finish

Confirmed: `VBIT_PARALLEL` stays (`support.rs:90-91, 296-298`). No V-bit row is filed under Parallel, and `FAMILY_RULES` covers only tapered and bull nose, so the 16 cells still go through `formula_backing` and still refuse. No text change, because FM5 pins the texts.

## 6. One row for Suggest and the gate
- The recipe lookup now tries chip rows first (`vendor_lookup.rs:363`) and both sides use the same key (§1). Suggest's chip row is therefore the gate's row by construction, which closes the 5 hardwood cells in P2 step 1.
- The A2 note is closed: 12.7 mm hardwood Trace reads `amana-vgroove-hardwood-trace-60deg-2f` on both sides. Today the gate reads the 45° single-flute row at ×0.30.
- `VendorRowPublishesNoChipload` stops firing on V-bit cells.

## 7. Matrix effect

Based on the simulation, before any feed ceiling. The FM1 re-run confirms.

| Cells | Today (row, rpm, chip, feed) | B4 | Feed |
|---|---|---|---|
| 6.35 SW Trace, Chamfer, Inlay (3) | Whiteside RPM-only, 22000, 0.0741 formula, 3261 | AMS 60° AngleKey, 18000, 0.0762 point, 2743 | ×0.84 |
| 6.35 HW Trace, Chamfer, Inlay (3) | same, 0.0475, 2090 | same | ×1.31 |
| 6.35 HW VCarve; 12.7 HW VCarve | Whiteside 1/4 in RPM, 22000, 0.0448, 1972 | AMS 60°, 18000, 0.0762, 2743 | ×1.39 |
| 6.35 and 12.7 SW VCarve | AMS ×0.618 at the width, 11027, 0.0471, 1038 | AMS, 18000, 0.0762, 2743 | ×2.64 |
| 12.7 SW/HW Trace, Chamfer, Inlay (6) | AMS, engine 8000, 0.0762, 1219 | AMS, 18000 | ×2.25 |
| 6.35 MDF/ply Trace, Chamfer, Inlay, VCarve (8) | 37-80 1 in ×0.43 / ×0.405, about 929-1021 | **refused (G1, 4.0×)** | ship → refuse |
| 12.7 MDF/ply VCarve (2) | ×0.405 at the width, 964 / 1021 | form C ×0.655, 0.0832 | ×1.62 |
| 12.7 MDF/ply Trace-family (6) | form C 0.655 | same | none |

Overall: 26 cells move and 8 become refusals (410 → 418). 9 cells change `chipload_source` from `FormulaFallback` to `VendorLut`. The 3 softwood cells lose the `Capped` hardness basis.

**Hardwood 0.37× question:** B4 does not change the Backed verdict (`support.rs:302-307` is untouched). The hardwood Trace-family cells stop using the formula and ship the printed AMS point instead. The question now covers only the formula-only cells: Face, Pocket, Profile, Rest, Zigzag and ProjectCurve. For those, formula ÷ AMS 0.0762 is 0.62 at 6.35 mm and 0.95 at 12.7 mm; against Onsrud it is 0.37 and 0.57.

## Steps (the orchestrator compiles between them)
1. **Data, key, basis and resolver** (one coherent change in the numbers):
   - the JSON and manifest edits from §4;
   - `geometry.rs` `lut_key_*`;
   - `extrapolation.rs` and `size.rs` (`AngleKey`);
   - `support.rs` arms;
   - `vendor_lookup.rs` tie-break, chip-first V-bit arm, and `vbit_key_text`;
   - doc comments at `mod.rs:127-131, 1484-1489`, `chipload.rs:105-110, 550-553`, `feed_explanation.rs:133-136`, `vendor_normalize.rs:198-200`, `tool_load/mod.rs:446-447`;
   - re-pin the tests listed below.
2. **De-rate at the nominal diameter:** the `depth_derate_*` pair, the three call sites, and the `geometry.rs` docs.
3. **Card and record:** `why.rs` key line (and the viz `g_claimcard` case); FM1 `lut_key` column and re-run with the CSV committed; the sentry list in `feeds/CLAUDE.md`; the landing record in `EXTRAPOLATION_G5.md` §5; the RULINGS "Landed" line.

## Sentry: new `a_vbit_row_is_read_at_its_printed_key_b4`
- Hint key equals cutter key equals nominal diameter for ap ∈ {0.1, 0.381, 5, 50}.
- AMS/Spektra rows have `diameter_mm` None and `rpm_nominal` 18000.
- 6.35 and 12.7 mm, softwood and hardwood, Trace and VCarve: recipe id equals envelope id equals the gate's `matched_chip_envelope` id (`amana-vgroove-{mat}-trace-60deg-2f`); basis `AngleKey{60}`; Suggest RPM 18000.
- 6.35 mm MDF Trace refuses with the text naming 25.4 mm and 4.0×. 12.7 mm MDF is a form C claim with scale 0.5^0.61 and spread `BorrowedFromFlatEnd`.
- MDF VCarve at a 3 mm hint keeps engine RPM ≥ 12000.
- (Step 2) 15° V-bit at ap = 0.5 D gives a de-rate of 1.0; at ap = 2 D, Suggest and the gate give the same doc ratio.
- The 16 parallel-finish cells refuse with `VBIT_PARALLEL`.

## Tests likely to move
- **Always run:** `adaptive_feed_modulation_pipeline_f036b`, `arc_fit_disposition_a5`, `a_rescaled_feed_stays_inside_the_power_ceiling_g_t15`. None of them references a V-bit, so they are expected green.
- **`literature_matrix`:** `vbit_60deg_vcarve_mdf` becomes a refusal, so every check reads NA. `vbit_60deg_pocket_oak_unadvised` is unchanged.
- **`wanaka_suggest_integration`** (ask the operator first): tp 5 and tp 6 do not move, because `lut_query_for` refuses ProjectCurve on a V-bit before any key is computed.
- **Re-pin:**
  - `a_size_claim_states_its_rule_range_and_residual_g1` l.507-532: rewrite.
  - `the_onsrud_vbit_rows_serve_mdf_and_plywood_g2` l.34 and 118-127: the 6.35 mm case now refuses.
  - `a_printed_value_is_held_as_a_point_a2` l.217-220 and 246-255: `gate_reads_the_point` becomes true.
  - `lookup_parity` l.142-155: the MDF 6.35 mm case.
  - `lut_resolver_census_a6`.
  - `lut_resolver_purposes_a7`: `trace/vbit60` drops out of the sweep.
  - `rubbing_floor_envelope_band_p1` l.545-560: "36 bandless V-bit Trace cells" probably goes to 0, so the non-empty assert fails and needs re-scoping.
  - `micro_extrapolation_refuses_fm9`: comments only.
- **Check:**
  - `predicted_feed_gates_f035` and `chipload.rs:2144`: the 90° row at 6.0 mm now takes form C ×1.035, is not flagged extrapolated, and stays a hard check.
  - `g_notheld`, `g_pillclamp` (VCarve).
  - `feed_explanation_snapshot_b3`: already red.
- `feeds_matrix_instrument_fm1`.

## Open questions (with a recommendation each)
- **Q1.** 6.35 mm MDF/plywood (8 cells): refuse honestly, or let the single-flute angle-keyed 37-00 row serve a two-flute tool (2165-2521 mm/min)? **Refuse.** The second option needs its own ruling. Do not add a general "skip size-refused rows" rule to the lookup.
- **Q2.** The parked softwood/hardwood 37-series rows: **keep them parked** (§4).
- **Q3.** RPM for Onsrud rows, which print none: keep the engine rule for now. Possible later package: a V-bit RPM claim built from the printed RPMs (14-22k, Amana 18k on every chart).
- **Q4.** The insert-v16 and Whiteside 120° rows that are not on their cited documents (G5 T5, D1/D2): leave them in B4, because they serve no 60° cell and f035 pins one. Clean them up separately.
- **Q5.** V-bit ProjectCurve routes to None: keep it for B4. Later, route it to (Trace, Finish).
- **Q6.** The axial envelope de-rate uses the chip-thinning diameter: fix it separately (ball nose is affected too).
- **Q7.** The Whiteside 1540/1550 RPM-only rows no longer answer 40-80° solid-wood V-bit queries, because a chip row always exists there. Keep them as data.

### Critical Files for Implementation
- /home/ricky/personal_repos/rs_cam/crates/rs_cam_core/src/feeds/vendor_lookup.rs
- /home/ricky/personal_repos/rs_cam/crates/rs_cam_core/src/feeds/geometry.rs
- /home/ricky/personal_repos/rs_cam/crates/rs_cam_core/src/feeds/extrapolation/size.rs (plus `feeds/extrapolation.rs`, `feeds/support.rs`)
- /home/ricky/personal_repos/rs_cam/crates/rs_cam_core/data/vendor_lut/observations/amana_vgroove_engraving.json (plus `amana_vgroove_aluminum_acrylic.json`, `source_manifest.json`)
- /home/ricky/personal_repos/rs_cam/crates/rs_cam_core/src/tool_load/chipload.rs (plus `tool_load/mod.rs:463`, `feeds/mod.rs:2591`)