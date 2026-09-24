# P2 plan (planner's design) and the orchestrator's decisions

## Orchestrator's decisions (2026-09-24; the operator took the recommendations)

1. Park 22 of the 76 verified V-bit rows: 11 laminated-plywood rows (they tie
   the hard-plywood rows and win on id) and 11 angle-less 37-80 rows at 1 1/4
   and 2 in (they pass the angle gate for any angle). Split 37-00 / 37-20 by
   their printed angles, diameter None. Load 60 V-bit rows + 15 Amana v7 ball
   rows (pocket only, A3). 441 -> 516.
2. V-bit MDF / plywood judgement: STRICT (R1 threshold, stored-text figures
   only). The same-angle Onsrud 37-80 figure puts the formula at 0.41-0.48x at
   6.35 mm, under 0.5x: the 24 cells stay Clueless with a new text that names
   the figure. The hardwood V-bit Pocket formula sits at 0.37x of the same
   Onsrud rows: record it in RULINGS.md as an open question; do not change the
   hardwood verdict in P2.
3. Janka: one table (solid wood reads material::WoodSpecies generics; no
   hardness scale in the composite categories), 145 banded cells.
4. The soft/hard cap at the largest printed ratio per tool family (Ball 1.50,
   Flat 1.43, V-bit 1.42, Tapered 1.00, Facing 1.30, Bull borrows Flat 1.43),
   as a typed clamp with a card line (hardness.rs), not an Extrapolation impl.
5. The 15 v7 rows are pocket rows only, with no adaptive copies, until G3 lands.
7. AMENDED at step 1 (measured): the softwood and hardwood 37-series rows
   are parked too (40 V-bit rows load, not 60; 441 -> 496). Loaded, they
   displaced the Whiteside 1/4 in RPM anchor of 5 softwood V-bit cells, and
   the engine RPM (10 026) replaced a printed 22 000: feed x0.34, not the
   -26 % the plan predicted from the chipload alone. G2 is the MDF / plywood
   gap; the solid-wood V-bit size key is ruling B4's question.
6. why.rs is free (the feeds/dial session moved to adaptive3d).

---

## The planner's design: load, fix, cap, refuse (ruling B2)

The main result: the Janka fix moves about 150 matrix cells, not 26. The stack is:

- **Step 1:** load 60 V-bit rows and 15 ball rows (441 → 516). 16 MDF/plywood V-bit trace cells start shipping; 5 softwood V-bit cells change row.
- **Step 2:** re-run the R1 judgement. 24 cells can ship the formula, if you accept the "Backed" reading below.
- **Step 3:** one Janka table. 145 banded cells go to a hardness scale of x1.00.
- **Step 4:** the softwood cap. Only 6 matrix cells move.
- **Ball nose in plywood:** nothing moves. The 24 cells stay refused.

Everything was read-only. All counts come from a Python copy of `passes_must_match`, `score_observation`, the angle gate and `beats`. It reproduces the recipe row of all 366 matrix cells that have one (366/366). Two inputs are my own reconstruction, not read from code: the operation-to-(family, role) map, and the V-bit lookup keys (5.773 mm for VCarve, the nominal size otherwise). The cell counts rest on them.

## 1. What to load

**Verified rows as they stand (91).** If loaded unchanged, two problems appear:

- **Laminated plywood wins ties.** The laminated-plywood rows mapped to `plywood_hardwood` tie with the hard-plywood rows at 1889. On id order, `onsrud-lamply-…` beats `onsrud-plywood_hardwood-…`, so all 8 plywood trace cells would name the laminated-plywood table.
- **Angle-less rows get past the angle check.** Three sets of rows have no angle: 37-00/37-20, 37-80 at 1¼ in and 37-80 at 2 in. The angle check in `find_best_vbit_row_where` (`vendor_lookup.rs:193-202`) lets any angle-less row through. So a 150° V-bit query would match, and the unit test `vbit_rejects_row_outside_angle_tolerance` (`vendor_lookup.rs:1664`) fails. Off the matrix, 15°, 30° or 150° V-bits in MDF or plywood would ship from a 50.8 mm lettering row at (d/50.8)^0.61.

**Recommendation: load 60 V-bit rows plus 15 ball rows.**

| Item | Recommendation |
|---|---|
| **Laminated plywood → `plywood_hardwood`?** | **No, park all 11 rows.** The laminate face is not a hardwood face, and the printed bands equal the hard-plywood table at every shared size, so nothing is lost. If you want them loaded anyway, use ids like `onsrud-plywood-hardwood-laminated-…`; the tie then goes to the hard-plywood rows. |
| Laminated chipboard → `particleboard` | Load (10 rows). It never beats an MDF row on an MDF query: no +100 family bonus, and a hardness score of 42 against 80. |
| **37-00/37-20 shank column** | Keep `diameter_mm = None`: the 1/4 in column is the shank, so scaling the band from it would be wrong. Split each table's row into `…-37-00` (60°) and `…-37-20` (30°), with angles from PCT-19 p.20 as the flutes already are. Give them separate subfamilies so the rule-6 conflict check (`vendor_lut.rs:593`) keys them apart. 1 flute, band 0.1016-0.1524, unscaled. |
| **Sizes with no catalogue part** | 37-60 at 3/8 in: load as printed (90° from the series), with a note. Laminated chipboard 9/16 and 7/8 in: load as printed, noting a probable layout error; their bands equal the 1/2 and 3/4 in cells, and they serve only particleboard. **37-80 at 1¼ in and 2 in: park (11 rows) until B4.** They have no angle, and the angle check exists to stop exactly that transfer. |
| **15 Amana v7 ball rows** | **Not in the LUT.** It holds only the 1/8 and 1/4 in v7 rows (12, pocket and adaptive, no `hardness_value`). Load them as pocket rows only (per A3's one row per printed cell), with ids like `amana-ball-{mat}-pocket-{d}-2f-v7`. **They move 0 matrix cells.** At 6.0 mm the 6.35 row still wins (1808 against 1771 for 9.525), and the 3.175-6.35 form A bracket is unchanged. Off the matrix they turn 9.5-19 mm ball pocket queries into exact rows or form A. Do not backfill `hardness_value` on the old 12 rows: that adds 80 to their score, and step 3 already makes the two sets agree numerically. |

**Row count:** 5 tables × 10 + chipboard 10 = 60 V-bit rows. 441 + 60 + 15 = **516.**

## 2. R1 re-run for `VBIT_MDF_PLY` (step 2)

**None of the 24 cells gets a row.** All the 37-series rows are trace/finish, and the lookup requires an exact operation-family match (`vendor_lookup.rs:698`). V-bit ProjectCurve is refused by routing (`vendor_normalize::lut_query_for`). So `formula_backing` alone decides them.

Formula chipload, MDF (Janka 1100): 0.0546 mm at 6.35 and 0.0833 mm at 12.7. Plywood (1200): 0.0523 and 0.0798. The table gives formula ÷ printed band middle:

| Figure | MDF 6.35 / 12.7 | Plywood 6.35 / 12.7 |
|---|---|---|
| Onsrud 37-80 1 in, 60° (same angle), as printed | 0.43 / 0.66 | 0.41 / 0.63 |
| Onsrud 37-50 1/4 in, 90° | 0.48 / – | 0.46 / – |
| Onsrud 37-60 1/2 in, 90° | – / 0.66 | – / 0.63 |
| Amana insert v16, 6 mm 90° (in the LUT as a/exact; no stored text) | 1.15 / 1.75 | 1.16 / 1.77 |
| Whiteside 12 mm, 120° | 0.84 / 1.28 | – |
| Onsrud 37-80 1 in carried by the engine's own ^0.61 | 1.00 / 1.00 | 0.96 / 0.96 |

**Recommendation: Backed** for Pocket/Roughing, Contour/Roughing and Trace/Finish in MDF and plywood. The v1 rule was "one figure backs both diameters, even when another is outside" (softwood: "AV backs both diameters. IDC is above 2x at d6"). The v1 text "No V-bit chart prints an MDF or plywood row" was already false: `amana-vbit-mdf/plywood-hardwood-trace-6000-2f` were in the LUT.

Edits in `support.rs`:
- Delete `VBIT_MDF_PLY` (l.91-92) and its arm (l.282-284).
- Remove the `if sw_hw` guard at l.270 (`VBIT_PARALLEL`) and l.273 (`VBIT_CONTOUR_FINISH`). Both texts hold in MDF too: RampFinish in MDF is 0.0127 mm, 0.10-0.27 of every figure.
- Change the guard at l.276-281 to `sw_hw || mdf_ply`, and fix the comment at l.266.

Result: 24 cells refused → FormulaOnly. The other 24 (the 6 parallel/contour-finish operations) stay refused, now with the parallel / contour-finish texts.

The strict alternative, if you rule same-angle figures with stored text only: at 6.35 mm the only such figure is 0.43, so the class stays Clueless. The new text would be: "the formula lies below half of the Onsrud 37-series V-bit band at 1/4 in (0.41-0.48x)". The same Onsrud rows put the **hardwood** formula at 0.37 at 6.35 mm, which the current Backed verdict does not see; I flag it rather than fix it.

## 3. Janka: one table (step 3)

**Design.** The solid-wood row default reads the query table. Composite boards get no hardness scale at all.

- Replace `family_default_janka` (`vendor_lookup.rs:476-489`): Softwood → `WoodSpecies::GenericSoftwood.janka_lbf()` (600), Hardwood → `GenericHardwood.janka_lbf()` (1450). The one table is therefore `material/mod.rs:113-133`. Everything else returns `None`.
- `hardness_ratio_raw` (l.494-525): return `1.0` first when `material_category(query.material_family)` is 5 or 6, for per-row values and defaults alike.
- Leave `SheetGoodKind` and `PlywoodGrade::effective_janka_lbf` (`material/mod.rs:167-205`) unchanged. `feed_scale_factor` (l.817-818, the formula path) and `literature_parity` still use them. Only their doc comments need to drop "the LUT hardness query".
- Particleboard's 500 lbf is a minimum; record it in the manifest only.

**Cells that move (recipe; 145 banded, plus 5 RPM-only rows with no band):**

| Material | Cells | Scale | Rows |
|---|---|---|---|
| MDF | 26 | x0.798 → 1.00 | 12 ball v7, 6 bull and 8 flat on `onsrud-mdf-60-100mw` |
| Plywood | 68 | x0.913 / x0.957 → 1.00 | 28 tapered (Onsrud 77-100), 22 flat, 18 bull |
| Hardwood | 22 (+5 RPM-only) | x0.943 → 1.00 | – |
| Softwood | 29 | x0.913 → 1.00 | – |

The hardness flag clears on the 26 MDF cells (the "approx ×0.80" token goes). A re-pin is needed in `every_consumer_reads_one_claimed_band_g1` (form A): 0.029819170734 → 0.0254 × 1.286034051728 = **0.032665264914**. Composites only, keeping 500/1290 for solid wood, would move 94 cells, but leaves two tables.

## 4. Softwood cap (step 4)

**Recommendation: a typed clamp with its own card line, not an `Extrapolation` impl.**

- The trait returns `SizeBasis`, and `Claim` is built around diameters (`anchor_diameter_mm`, `range_mm`). A cap is not a claim with a range.
- `FeedsSupport::Extrapolated` holds one claim, and a cell can carry a size claim and a cap at once.
- The clamp still runs inside `build_result`, so every consumer reads one number.

New `feeds/extrapolation/hardness.rs`, re-exported from `extrapolation.rs`:

```rust
pub enum HardnessBasis { Unscaled, CompositeBoard,
    Law { ratio_raw: f64, scale: f64, row_janka: f64, from: JankaFrom },
    Capped { ratio_raw: f64, law_scale: f64, cap: SoftHardCap } }
pub enum JankaFrom { Row, FamilyGeneric }
pub struct SoftHardCap { pub family: ToolFamily, pub ratio: f64, pub borrowed_from_flat: bool }
pub const SOFT_OVER_HARD_PRINTED_MAX: &[(ToolFamily, f64)] = /* Ball 1.50, Flat 1.43, V-bit 1.42, Tapered 1.00, Facing 1.30; Bull borrows 1.43 */;
pub fn hardness_basis(query: &LookupQuery, obs: &VendorObservation) -> HardnessBasis
```

- Add `Gap::Hardness` (group "G2"), used only for the card headline.
- The cap applies inside solid wood when the law's scale is above 1 and above the cap of the **row's** tool family. It covers any softer query, so Longleaf and Walnut are covered too.
- The raw ratio is left unchanged, so the extrapolation flag still reads raw ratios.
- `build_result` l.545/555 reads `hardness_basis`. Add `LookupResult.hardness_basis` and update the 7 struct literals: `provenance.rs:376`, `cutter_constraints.rs:568`, `suggest/tests.rs:53`, `optimize/bounds.rs:503`, `stage1_grid_tests.rs:208`, `headroom.rs:614`, `headroom.rs:668`.
- Card: a line in `why::draw_row_basis_lines` (`rs_cam_viz/src/ui/feeds/why.rs:542`), e.g. "hardness transfer capped at x1.50 (the largest softwood/hardwood ratio ball-nose charts print); the Janka law gives x1.56". Also mention it in the `claim_hover` text (l.572).
- New FM1 columns: `hardness_ratio_raw`, `hardness_scale`, `hardness_basis`.

**Moves:** 6 cells. Softwood ball Scallop, UnifiedFinish and SpiralFinish at 3.175 and 6.0 mm, on `amana-ball-hardwood-scallop-6000-2f`, go x1.555 → **1.50**; a median cap would give 1.29. The 3 Whiteside V-bit softwood cells left that row in step 1, so no V-bit cell is capped.

## 5. Ball nose in plywood

Nothing moves them:
- No ball-nose plywood row is loaded; the 37-series rows are V-bit, and the v7 rows are softwood, hardwood and MDF.
- The category filter keeps hardwood rows out of plywood (`vendor_lookup.rs:708`).
- `BALL_PLY` (`support.rs:251-257`) is untouched.
- Steps 3 and 4 act only on a matched row.

## Steps (each leaves the tree building)

**1. Data.**
- New `data/vendor_lut/observations/onsrud_vbit_37.json` (60 rows). Append 15 rows to `amana_ball_nose.json`.
- Strip `verification`, `verbatim` and `extrapolation_group`, and fix the stored-text line references in the notes.
- `EMBEDDED_FILES` (`vendor_lut.rs:316-446`). Count 441 → 516 at `vendor_lut.rs:664` and `tests/vendor_lut_sub_1mm.rs:170`.
- Add `"printed Cut column: 'Varies'"` to `LABEL_ONLY_AP_RULES` (`vendor_lut.rs:725`).
- Manifest: new entries for laminated chipboard (sha 21f3c09a…) and `onsrud_pct19_catalog`. Parking notes for laminated plywood and the angle-less 37-80 rows. Coverage notes on the 5 Onsrud sheets and v7. Copy three texts from `fetch/G2/sources/` into `data/vendor_lut/sources/`. The existing Onsrud texts already hold the 37-series lines; they differ only in the 2 header lines.
- **Recipe row changes:** 16 MDF/plywood cells (Chamfer, Inlay, Trace, VCarve × 2 sizes) go refused → VendorBacked. They are exempt from size claims until B4, flagged extrapolated and carry the "approx ×" token: ×0.43 and band 0.0436-0.0654 at 6.35; ×0.66 at 12.7; ×0.41 for VCarve. 5 softwood V-bit cells change row: 6.35 Trace/Chamfer/Inlay go 0.0741 (formula) → 0.0545 (−26%); VCarve at 6.35 and 12.7 goes 0.0430 → 0.0515 (+20%).
- **Gate only:** 5 hardwood cells (6.35 Trace, Chamfer, Inlay, VCarve; 12.7 VCarve) have their envelope row change to Onsrud. Suggest stays on the Whiteside RPM-only row. A census re-run is needed; nothing there pins a count.

**2. Judgement.** `support.rs` as in section 2. FM5 pins: V-bit Pocket in MDF → FormulaOnly; V-bit Waterline in MDF → `Unbacked` with the contour-finish text; ball Pocket in plywood → `BALL_PLY`; no reason contains "No published V-bit chipload exists for MDF". FM0 checks against the table itself, so it needs no edit.

**3. Janka fix.** `vendor_lookup.rs` only, plus doc comments at l.87-102 and 461-525, the `material/mod.rs` doc, and the form A re-pin above. New sentry `one_janka_table_for_row_and_query_g2`:
- the MDF v7 6.35 pocket row reads 0.1524-0.2032, scale 1.0, not flagged;
- HDF on an MDF row gives 1.0, and Baltic birch on `onsrud-plywood-hardwood-60-100mw` gives 1.0;
- a hardwood row with no Janka on a GenericHardwood query gives 1.0;
- Ipe still derates.

**4. Cap and card.** `hardness.rs`, `Gap::Hardness`, the new field and the 7 literals, `why.rs`, FM1 columns, `feeds/CLAUDE.md` (the invariant "hardness-agnostic within a family" and the sentry list), `EXTRAPOLATION_G2.md` §5. New sentries:
- `a_hardness_transfer_caps_at_the_printed_soft_hard_ratio_g2`: pins the constants; scallop softwood gives 1.50 with raw 2.4167 unchanged and the flag still set; recipe and gate bands agree.
- `the_onsrud_vbit_rows_serve_mdf_and_plywood_g2`: the MDF and plywood row ids (not laminated plywood); 150° gives `None`; a 1-flute 60° query gets the 37-00 row, unscaled.
- `lookup_parity`: add a V-bit MDF case and a capped case.

Re-run FM1 after every step. Ask before `wanaka_suggest_integration`: step 3 can move its hardwood pins by x1.06. The tp 11 x1.0 pin (l.773) holds.

## Decisions for you

1. **Park 22 of the 76 verified V-bit rows** (11 laminated plywood, 11 angle-less 37-80), and split 37-00/37-20. That departs from "load the 76"; the evidence is the unit test and the tie-break. If they load anyway, re-premise l.1664 and accept that V-bits of any angle ship in MDF and plywood.
2. **V-bit MDF/plywood judgement:** Backed (+24 ship, as v1 decided it) or strict Clueless (24 stay refused, new text). Also: re-judge hardwood V-bit Pocket against Onsrud's 0.37?
3. **Janka scope:** all 145 cells (one table, as recommended) or composites only (94).
4. **Cap values:** the largest printed ratio (recommended, 6 cells, −3.5%) or the family median (−17%). Bull nose borrows the flat 1.43. The cap also covers softer hardwoods, which blocks all upward transfer on tapered rows.
5. **The 15 v7 rows as pocket-only**, with no adaptive copies (A3), until G3 is built.
6. Step 4 edits `rs_cam_viz/src/ui/feeds/why.rs`: check the feeds/dial session has left it (decision 9 of P1).

### Critical files for implementation
- /home/ricky/personal_repos/rs_cam/crates/rs_cam_core/src/feeds/vendor_lookup.rs
- /home/ricky/personal_repos/rs_cam/crates/rs_cam_core/src/feeds/support.rs
- /home/ricky/personal_repos/rs_cam/crates/rs_cam_core/src/feeds/extrapolation.rs (plus a new `extrapolation/hardness.rs`)
- /home/ricky/personal_repos/rs_cam/crates/rs_cam_core/src/feeds/vendor_lut.rs (plus `data/vendor_lut/observations/`, `source_manifest.json`, `sources/`)
- /home/ricky/personal_repos/rs_cam/crates/rs_cam_viz/src/ui/feeds/why.rs