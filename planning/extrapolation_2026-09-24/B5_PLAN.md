# B5 plan (G6 drill): the planner's design and the orchestrator's decisions

## Orchestrator's decisions (2026-09-24; the operator took the recommendations)

1. The engine drill RPM cap (14 000 at D <= 6 mm) stays over Amana's
   printed 18 000 on a claimed row; the chip is held per tooth; the card
   says the feed is about 0.78x the printed Ramp Down.
2. The other Spektra sizes (1/4 in and up) are a later transcription job.
3. Every drill cell with no G6 claim refuses, plastics and aluminium too.
4. The bull feed-only claim refuses (no RPM; one 3-flute tool).
5. The plunge envelope ceiling above 6 mm is held per material.
6. 1- and 4-flute flat end mills refuse.
7. The literature drill cells whose bands were fitted to formula x 2.5 are
   re-banded from the Amana chart (an external source), not from the LUT.

---

# B5 (G6 drill) design

Recommendation: 16 of the 80 drill cells ship: the flat end mill at 3.175 and 6.0 mm, in four materials, on both drill operations. The other 64 refuse with a reason that names G6. The claim takes the tool's own printed Spektra side row and divides it by the flute count. It holds the chip per tooth. The engine's drill RPM cap stays, so the feed comes out at about 0.78x Amana's printed Ramp Down feed. The plunge feed ceiling moves up to the largest printed figure per mm of diameter. Nothing in `session/`, `adaptive3d/` or `compute/` needs editing.

## 1. How the flat-end plunge claim is expressed

**What `verified_rows.json` holds.** The 8 Amana rows are *derived*: the Ramp Down figure in in/min, converted to a per-tooth chip at the chart's 18,000 RPM. The side rows they come from are already in the LUT (`amana_flat_end.json`, `amana-flat-{softwood,hardwood,plywood-softwood,plywood-hardwood,mdf}-pocket-{3175,6000,6350}-{2f,3f}-spektra`). Since A2 those side rows are points.

**Do not load the 8 rows as `drill`-family LUT rows.** Loaded, they would read "vendor-backed" and hide the 1/Z derivation. The G1 size law would also stretch them past 3.175-6.0 mm, and the bull-to-flat tool fallback (`vendor_lookup.rs:896-905`) would put them on the bull cells. The 8 rows become the expected values of the new sentry instead. The drill family keeps zero rows, so `KNOWN_EMPTY = [Drill]` at `vendor_lut.rs:974` stays true; only its doc comment at :949-962 changes.

**New file `feeds/extrapolation/drill.rs`.** It is a G6 sibling of G3, not a G3 rule: `family.rs:33` says a family rule "moves no number", and this one moves one (x1/Z).

```rust
pub struct DrillRule { tool_family: ToolFamily /*FlatEnd*/, source_ids: &'static [&'static str] /*["amana_spektra_spiral_plunge_v24"]*/,
  tool_subfamily: &'static str /*"spektra_spiral_plunge"*/, home: (LutOperationFamily, LutPassRole) /*(Pocket, Roughing)*/,
  range_mm: (f64, f64) /*(3.175, 6.0)*/, flutes: &'static [u32] /*[2, 3]*/, witness: &'static str }
pub const DRILL_RULES: &[DrillRule];
pub const DRILL_RULE_TEXT: &str; // "Amana prints Ramp Down = feed rate / flutes at one RPM: axial chip per tooth = side chip / Z"
pub fn drill_rule(query: &LookupQuery, obs: &VendorObservation) -> Option<&'static DrillRule>;
pub struct DrillClaim { gap: Gap /*Drill*/, rule: &'static str, scale: f64 /*1/query.flute_count*/, flutes: u32,
  side_row: String, source_id: String, material_label: String, range_mm: RangeInclusive<f64>,
  printed_rpm: Option<f64> /*18000*/, witness: &'static str, confidence: ClaimConfidence /*OneWitness*/ }
impl DrillClaim { pub fn card_text(&self) -> (String, String) }
pub enum DrillBasis { Printed, Transferred(Box<DrillClaim>) }   // name(), claim(), scale() (1.0 or 1/Z)
pub fn drill_basis(query: &LookupQuery, obs: &VendorObservation) -> DrillBasis;
```

`drill_rule` returns a rule only when all of these hold:

- The query family is `Drill`.
- The query, the row and the rule are all `FlatEnd`. Without this, the bull-to-flat fallback would carry the claim onto the 16 bull cells.
- The source and subfamily match the rule, and the row is filed under the rule's home (Pocket, Roughing). The Spektra adaptive copies are excluded, so no row is counted twice.
- Both the query diameter and the row diameter are inside 3.175-6.0 mm, with `EXACT_DIAMETER_TOLERANCE` at the ends. This keeps out the 6.35 mm row, which no verified Ramp Down row covers.
- `query.flute_count` is 2 or 3.

A query outside these limits finds no row and falls to `formula_backing`, which refuses with the G6 text. A 4 mm tool is inside the range, so the G1 form A claim (interpolated) applies to the side row and the drill scale multiplies on top of it.

**Wiring:**

| Site | Change |
|---|---|
| `extrapolation.rs:54-85` | Add `Gap::Drill` ("G6", "drill"); `pub mod drill` and re-exports at :30-47 |
| `vendor_lookup.rs:271` and `:700` | `transferred = transfer_rule(..).is_some() \|\| drill_rule(..).is_some()`. This feeds `passes_must_match` (:779), the +45 role term (:826) and `beats` (:649) unchanged |
| `build_result` (`vendor_lookup.rs:556-640`) | `let drill_basis = drill_basis(query, obs);` and `total_scale = diameter_scale * hardness_scale * drill_basis.scale()`. The A2 point stays a point |
| `LookupResult` (`vendor_lookup.rs:~156`) | New field `pub drill_basis: DrillBasis`. Struct literals to update: `provenance.rs:376`, `cutter_constraints.rs:571`, `tool_load/optimize/bounds.rs:503`, `strategy/headroom.rs:614,670`, `stage1_grid_tests.rs`, `suggest/tests.rs`, the `vendor_lookup` tests |
| `support.rs:320-343` | New arm `FeedsSupport::DrillTransferred { drill: Box<DrillClaim>, size: Option<Box<Claim>> }`, with `card_text` at :349 |
| `support_for_lookup` (`support.rs:523-540`) | Same shape as G3: if `row.drill_basis.claim()` is Some, `VendorBacked` becomes `DrillTransferred{size: None}` and `Extrapolated{claim}` becomes `DrillTransferred{size: Some}` |
| viz `ui/feeds/why.rs:554-563` | Add the drill-claim line |

**RPM and which quantity is held.** Per tooth is held, which is how every LUT row works. A2 treats the point as a hard ceiling in the gate. Holding the printed feed per minute at the capped RPM would put the chip at 1.29x the point, which fails that gate. So the RPM code does not change:

- The vendor RPM of 18,000 is applied first.
- Step 2c (`mod.rs:1916-1924`) then clamps it to `drill_rpm_envelope_for_diameter` (`mod.rs:1292`), which gives 14,000 at D ≤ 6 mm.
- The spindle scale is booked at x0.78.

The claim's card says, as fixed text: "held per tooth; the chart's 18,000 RPM is replaced by the engine drill RPM cap (8,000-14,000 at D ≤ 6 mm, a repo rule), and the feed follows (about 0.78x the printed Ramp Down)."

**Other card lines.** The Spektra column is "Wood/Plywood"; the card says softwood and hardwood read it as derived grade b (R5/A4), from `material_label`. The card also says the column is named "Ramp Down" and that B5 reads it as a straight plunge. The claim is one witness and per family.

## 2. Deleting the 2.5 multiplier, and what serves a drill cell with no claim

- **The constant.** Delete `DRILL_CHIPLOAD_MULTIPLIER` and its comment block (`mod.rs:1557-1596`); `formula_chipload = milling_chipload` everywhere. That formula is now only the preview number shown for a refused cell (explain modal, raw `calculate`).
- **The formula source text.** Keep the symbol `DRILL_FORMULA_SOURCE` (`support.rs:48`), because `compute/catalog/registry.rs:8,924,1279` and fm0:155 import it. Change only its text, to something like "repo-derived milling formula k0*D^p*(1/H)^q with no drill multiplier (deleted, ruling B5); a preview only: Suggest refuses every drill cell that no G6 claim serves".
- **The refusal texts.** `formula_backing` keeps its five drill arms (`support.rs:224-230`). Only the texts at :94-102 change: drop "2.5 is unsourced" and name the group:
  - `DRILL_FLAT`: "No published figure for this flat end mill plunge: the Amana Spektra Ramp Down claim covers 3.175-6.0 mm, 2 or 3 flutes, in wood, plywood and MDF (G6 drill, ruling B5)."
  - `DRILL_BALL`, `DRILL_TAPER`, `DRILL_VBIT`: "... (G6 drill, ruling B5)."
  - `DRILL_BULL`: adds "the one printed bull plunge (PreciseBits) is a feed with no RPM for one 3-flute tool."
- **Non-wood materials.** Today a drill cell in plastic or aluminium keeps the formula. In `support_for_lookup`, before `formula_source_for_input` (:542), add: if `input.operation == OperationFamily::Drill` and there is no row, run `formula_backing(tool, Drill, …)` for every material and return `Refuse`. Every drill cell with no claim then refuses. `(_, Drill, _)` also needs a G6 text so that FacingBit is covered.
- **Follow-up for the compute owner:** set `feeds_formula_source: None` on the two drill registry rows once `compute/*` is free.

## 3. The plunge envelope, the RPM tiers and the peck bands

**The plunge envelope (`material/mod.rs:1290-1307`).** The ceiling moves to the largest printed plunge per mm of diameter. The floor stays repo-authored and is named as such.

| Material arm | Before (floor, ceiling) | After (floor, ceiling) |
|---|---|---|
| SolidWood / SolidWoodByJanka | 50, 400 | 50, **580** (72.5 in/min ÷ 3.175 mm) |
| Plywood | 40, 350 | 40, **580** (Wood/Plywood column) |
| SheetGood | 40, 350 | 40, **720** (MDF 90 in/min ÷ 3.175 mm) |

- Plywood and SheetGood are split into two arms. Name the constants, and update the doc at :1266-1289.
- One source still serves the Suggest clamp (step 9c, `mod.rs:2708`), the gate (`drill_gates.rs:146`) and `narrate`. There is no gate plumbing.
- At 14,000 RPM the shipped cells sit at 296-560 mm/min per mm, so no clamp fires.
- Caveat for the card and for CREDITS: the printed figure per mm falls with diameter (580 at 3.175 mm, 381 at 6.0 mm). At 6 mm the ceiling is therefore about 1.5x the printed figure, and above 6 mm it is not sourced.

**`drill_rpm_envelope_for_diameter`: kept as a named cap.** The ruling did not ask to move it, and the `_litmatrix_drill_rpm_*` tests pin it. Its doc (`mod.rs:1276-1291`) should say: "repo rule; on a G6-claimed plunge it caps Amana's printed 18,000; shown on the card."

**Peck bands:** values held. The claim card adds one fixed line: "peck depth: repo rule (per-peck max 6/5/4 × D by Janka, 1.5 × D sheet; Suggest writes half); no vendor prints a per-peck depth (G6 found none)." Add the same wording at `narrate.rs:1973` and in the doc of `apply_drill_defaults` (`apply.rs:768-802`).

## 4. Bull nose: refuse

The recommendation is to refuse all 16 bull cells. The PreciseBits row prints a feed and no RPM ("use your maximum RPM"). The engine works chip-first (feed = rpm × chip × Z, `mod.rs:2087`), and `VendorObservation` has no plunge-feed field. Shipping this claim needs three things the engine does not have:

- a held-per-minute mode, which is new;
- an RPM, which would be the machine maximum or the 14,000 cap; that is invented;
- a way past two gaps in the witness: it is a 3-flute tool (the matrix bull is 2-flute), and the hardwood row is grade c (`grade_wrong`).

So 16 cells ship, not 20. This is recorded as an open question below.

## 5. Ball, tapered ball and V-bit

48 cells refuse through the existing arms with the new G6 texts. The drill rule never matches them because it requires `FlatEnd`.

## 6. Matrix effect

This is an estimate from `matrix_2026-09-23.csv` (today all 80 drill cells refuse); it needs the FM1 rerun to confirm. It assumes 2 flutes and MatchChart, with the RPM going 18,000 to 14,000. Drill and AlignmentPinDrill give identical numbers.

| D (mm) | Material | Anchor side row | Axial chip (mm/tooth) | Feed = plunge (mm/min) | Per mm D | Printed Ramp Down (mm/min) | Engine ÷ printed |
|---|---|---|---|---|---|---|---|
| 3.175 | softwood, hardwood, plywood_hardwood | 0.1016 | 0.0508 | 1422 | 448 | 1841.5 | 0.77 |
| 3.175 | mdf | 0.127 | 0.0635 | 1777-1778 | 560 | 2286 | 0.78 |
| 6.0 | softwood, hardwood, plywood_hardwood | 0.127 | 0.0635 | 1777-1778 | 296 | 2286 | 0.78 |
| 6.0 | mdf | 0.1524 | 0.0762 | 2133 | 356 | 2730.5 | 0.78 |

- The feed rounds down; float rounding decides between 1777 and 1778 in those rows.
- The chip is 1.00x the verified derived chip, except 6.0 mm MDF at 0.0762 against 0.0758, where Amana rounded its printed in/min.
- Shipping: 16 EndMill cells. Refused: 16 each of BallNose, BullNose, TaperedBallNose and VBit.

## 7. Real wood drills

Out of scope. Onsrud, Leitz and CMT (19 rows, 0.13-0.50 mm per lip) need a `ToolFamily` drill arm first, and those rows must never serve an end-mill plunge. Record this in CREDITS "Drill-subsystem provenance" and in the RULINGS "Open" list.

## Implementation split (the orchestrator compiles between steps)

**Step 1: delete the multiplier, change the texts, move the envelope.** All drill cells still refuse at the end of this step.
- Edits: `mod.rs:1557-1596` (the multiplier) and the step 9c comment at :2676-2707; `support.rs:42-50` and :94-102; `material/mod.rs:1266-1307`; the `FeedsWarning::DrillFeedClampedToEnvelope` doc at `mod.rs:960`; CREDITS.
- Tests that move:
  - `feeds/tests.rs:546 test_drill_envelope_ceiling_drops_rpm_to_hold_chipload`: its "~0.107" premise was formula × 2.5, so rebaseline or delete it.
  - `feeds/tests.rs:450`: comment and the 0.05 floor.
  - `a_feed_lift_caps_at_the_cutting_ceiling_g_t18.rs:148-200`: the non-vacuity check (the clamp may no longer fire); refixture it.
  - `wanaka_suggest_integration.rs:281-284`: the refusal text contains "2.5"; change it to "G6".

**Step 2: the G6 claim.**
- Edits: `drill.rs`, `Gap::Drill`, the `LookupResult` field and its literals, the `vendor_lookup` wiring, the `FeedsSupport` arm and card, the refuse-every-unclaimed-drill check, viz `why.rs`, `feeds/CLAUDE.md` (files list, invariant, sentry), and the doc comment at `vendor_lut.rs:949`.
- New sentry `a_flat_end_plunge_is_the_side_chip_over_z_g6`:
  - For the 8 verified (D, Z, column) cells, the chip equals `chipload_max_mm_tooth` within 1%.
  - At 14,000 RPM the feed is under the printed Ramp Down.
  - Flat queries at 3.0 and 6.35 mm, and a 4-flute query, refuse with "G6".
  - Bull, ball, tapered and V-bit drills refuse with "G6".
  - A bull query never matches a Spektra row.
  - The MDF 3.175 mm cell reads Within on `classify_plunge_feed`.
- Tests that move:
  - `wanaka_suggest_integration`: tp 7 Holes and tp 14 Pin Drill now ship (6 mm 2-flute flat, GenericHardwood: 14,000 RPM, 1777-1778 mm/min, chip 0.0635). `refused` becomes empty. The catch-all at about line 830 must accept `AggressivenessNotApplied{Drill}` (`aggressiveness.rs:167`). Restore the drill no-lift assertions at :795-805. This test takes minutes: ask first.
  - `suggest/tests.rs:210 drill_apply_round_trips_suggested_feed`: plastic now refuses, so move it to GenericHardwood.
  - `feeds_matrix_instrument_fm1` (new arm name and `drill_basis` columns).
  - Every exhaustive `match` on `FeedsSupport` in fm0, fm5, fm9, g1, g2 and g3.
  - Extend `every_consumer_reads_one_claimed_band_g1` and `a_printed_value_is_held_as_a_point_a2` (the scaled point is still a point).
  - `lut_resolver_census_a6` and `lookup_parity`, since drill queries now match.

**Step 3: literature matrix, narrate, records.**
- `literature_matrix/cells.toml`:
  - `flat_6mm_drill_oak` (:1672) and `flat_6mm_drill_oak_using_endmill_unadvised` (:4662) now ship. Their `feed_per_tooth` bands 0.10-0.22 were fitted to formula × 2.5; change them to the Amana point, about 0.0635 with the white-oak hardness scale.
  - `drill_final_feed_in_plunge_envelope` has ceiling 400 on every drill cell; change it to 580.
  - The RPM 8,000-14,000 invariants still hold.
  - The 3.0, 2.0 and 12 mm cells stay refused (NA).
- `narrate.rs:1973` peck line.
- Check viz `every_stage_that_moves_a_number_is_on_the_card_g_visible.rs:615`.
- FM1 rerun, commit the CSV, and update EXTRAPOLATION_G6 / RULINGS to "landed".

Of your five named tests: `adaptive_feed_modulation_pipeline_f036b`, `arc_fit_disposition_a5` and `a_rescaled_feed_stays_inside_the_power_ceiling_g_t15` have no drill path. Run them after steps 1 and 2 as regressions only. `literature_matrix` and `wanaka_suggest_integration` move as described above.

**Traps:**
- The MDF 3.175 mm cell sits at 560/mm with the RPM cap. If the printed 18,000 RPM is ever allowed (see question 1), it lands at exactly 720.0/mm, the new ceiling, and the gate's `exceeds_high(.., 0.0)` float compare will flicker at that edge.
- If Step 2c were ever skipped for claimed rows, MaxSpeed (step 2b) would lift the drill RPM.

## Open questions, each with a recommendation

1. **Should Amana's printed 18,000 RPM win over the 14,000 cap on a claimed row, so the feed equals the printed Ramp Down?** Recommend no for now. The cap is conservative (same chip, lower RPM) and the ruling did not move it; reopen with a second witness.
2. **Extend the range to the rest of the Spektra sizes?** The chart prints the same Ramp Down at 1/4 in (6.35 mm) and prints the rule for every size. Recommend transcribing and verifying those cells, then widening `range_mm`; no new law is needed.
3. **Refuse drills in plastic and aluminium too?** Recommend yes. Without the 2.5 the formula is an unjudged milling number on a plunge.
4. **The bull feed-only claim:** refuse, as in section 4. Reopen with a PreciseBits RPM statement, or a 2-flute bull plunge chart.
5. **Envelope ceiling above 6 mm:** the per-mm scaling is not printed. Recommend holding the per-material ceiling for now. The alternative is to resolve the drill row in the gate's drill check through `ToolpathLoadContext` and use the claim's own feed as the ceiling.
6. **A 1- or 4-flute flat end mill:** refuse for now (Amana prints 2 and 3 flutes only).

### Critical Files for Implementation
- /home/ricky/personal_repos/rs_cam/crates/rs_cam_core/src/feeds/extrapolation.rs (and new /home/ricky/personal_repos/rs_cam/crates/rs_cam_core/src/feeds/extrapolation/drill.rs)
- /home/ricky/personal_repos/rs_cam/crates/rs_cam_core/src/feeds/vendor_lookup.rs
- /home/ricky/personal_repos/rs_cam/crates/rs_cam_core/src/feeds/support.rs
- /home/ricky/personal_repos/rs_cam/crates/rs_cam_core/src/feeds/mod.rs
- /home/ricky/personal_repos/rs_cam/crates/rs_cam_core/src/material/mod.rs