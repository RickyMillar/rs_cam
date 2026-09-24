# G10_PLAN: entry parameters, Phases 3-5 (the claims and the card)

Date: 2026-09-25. Status: plan. Nothing in this plan is built yet.

Input: RULINGS.md §"Operator rulings, 2026-09-25: G10" (Q1-Q12 accepted), EXTRAPOLATION_G10.md, INVENTORY_G10.md, g10_inventory_cells.csv, RAMP_PLAN.md, and the ramp code that landed in `043cece1`.

Scope:
- Part A is the feeds, tool_load and viz work. This session can start it now. It covers Q2, the Q3 text, Q4, Q5, the card for Q6-Q10, the feeds half of Q11, and the Q12 card line.
- Part B is the entry-code work. It waits for rs-cam-e2's By Area WP1 merge. It covers Q6, Q8, Q9, G-RAMPCLAMP and the compute half of Q11. Send the Part B file list (§6.1) to rs-cam-e2 before any Part B edit.

Q1 is done (`043cece1`). Paths below start at `/home/ricky/personal_repos/rs_cam/crates/`.

---

## 1. Findings the editors must know first

**F1. The plunge claim has one form for all four families.** Every accepted plunge ruling is "plunge = fraction × shipped side feed F":
- flat end mill: 1/Z (Q4, the Amana rule "Ramp Down = feed / flutes");
- ball: 0.50;
- tapered ball: 0.50;
- 60° V-bit: 0.33 (Q5).

The claim reads the tool family, the tool key, Z, the material and F. It does not scale a LUT row's chip, so it is not a `LookupResult` basis like `DrillBasis`. It is a rule table with evidence and a range, like `DRILL_RULES` and `FAMILY_RULES`. **Decision:** put the rule table and the claim in `feeds/extrapolation/plunge.rs`, and add a `Gap::Plunge` ("G10", "plunge"). Put the resolver in the new file `feeds/plunge.rs`, beside `ramp.rs`.

**F2. The Q4 value is F/Z, not the ramp's G6 vertical rate.** The ruling says "side feed / Z". The G6 ramp's vertical rate is `axial chip × RPM × Z`, which equals F / (Z × depth tier). The two values are equal at tier 1 (every Pocket cell). They differ on deep Adaptive cells (tier 0.45-0.75). This plan uses F/Z, as the ruling says. The G6 ramp arm stays as it is, so the pin `3464` in `a_ramp_feed_is_the_g6_chip_over_the_entry_slope_g6ramp` does not move. The ramp detail line shows both numbers.

**F3. G-RAMPCLAMP must compare with the operation's cut feed, not with the emitter's `feed_rate` argument.** In `dressup/entry_descent.rs:433` and `:674`, `feed_rate` is the dressup caller feed. That feed is `min(plunge move feed, 0.5 × first fed move)` (`compute/execute/dressup_apply.rs:524-532`), which is about 0.5 × plunge. So `ramp_feed.map_or(feed_rate, |r| r.min(feed_rate))` inside the emitter would pull every sourced ramp back to half the plunge. That would undo Q1. The clamp belongs where the operation's feed is in scope (§6.2 step B4).

**F4. Q11 is split across the two parts.**
- `pick_adaptive3d_entry_style`, `helix_feasible_in_bbox` (its only user) and `SuggestWarning::StrategyRewrote` (only that pass builds it) are feeds files: Part A.
- `PreferHelix` is in `compute/config.rs:1193`, `compute/catalog/schema.rs:478,509` and `compute/catalog/registry.rs:825`: Part B.

If Part B deletes `PreferHelix` and adds nothing, the default for 2D Adaptive becomes the Roughing Ramp (3°) instead of Helix. Decision D3 covers this.

**F5. The Q6 default (0.3 × D) cannot live in `DressupConfig::default()`,** because `for_op` has no tool. `DressupConfigWire.helix_radius` is a required `f64` (`compute/config.rs:806`). If the key gets a new name, every saved v3 project fails to load, unless `SUPPORTED_FORMAT_VERSION` goes from 3 to 4 and refuses them. Decision D2 covers this.

**F6. Removing the Suggest rewrite changes new Adaptive3d toolpaths, not the FM1 numbers.** The rewrite runs on the scratch copy and never reaches an applied operation (RAMP_PLAN F2, G-ENTRYREWRITE). But the creation doors take `s.operation` whole:
- `viz/controller/events/toolpath.rs:111`
- `viz/app/mcp/commands.rs:804`
- `cli/smoke.rs:526`
- `cli/examples/apply_suggest_save.rs:99`

So today a NEW Adaptive3d toolpath gets Ramp or Helix. After Q11 it gets the default, Plunge (the peck ladder), and `PlungeEntryUnstableAtDpp` warns. Risk R1 covers this.

**F7. Nothing today stops a plunge above 150 mm/min per mm of tip, except the cap.** The literal `150.0` is in two places: `feeds/mod.rs:2567` and `tool_load/plunge_stress.rs:20`. Part A makes `plunge_stress::safe_plunge_cap_mm_min` the one producer.

---

## 2. Decisions

### 2.1 Design decisions (this plan takes them)

1. **Where the plunge number comes from.** `calculate` Step 8 runs after Step 7, where F is final for the calculator. It sets two things:
   - `FeedsResult::plunge: PlungeBasis`, the part that does not depend on F;
   - `plunge_rate_mm_min`, the rule applied to the calculator's F.

   Keep the `f64`. The literature shim, `wanaka_defaults_validation`, `plunge_guard_*` and feedopt read it.

   The apply funnel runs the rule again at the shipped F, after `enforce_invariants` (pass 9 and pass 10 can move F) and after `with_explored_speeds`. This is the same rule as the ramp: the number describes what ships. The funnel sets the plunge before the ramp, because the Q2 ramp reads the shipped plunge.
2. **The tip cap stays a named rule** (Q5). Ball and tapered-ball plunge = `min(fraction × F, 150 × tip)`. The card names which term binds.
3. **The bull nose keeps the material base as a named rule** (EXTRAPOLATION §3.1). The card cites the Amana corner-radius rule as the nearest printed figure. No bull witness pairs a plunge with a feed.
4. **Outside a claim's range, the plunge falls back to the named rule** (material base, then the tip cap, then the feed). The card line starts "no source; repo rule …" and says why the claim refuses. Examples: a flat outside 3.175-6.0 mm or with 1 or 4 flutes, a V-bit that is not 60°, a material that is not wood, plywood or MDF.
5. **Provenance.** Add `ProvenanceSource::PublishedRule` (reference = the rule id) and `ProvenanceSource::RepoRule` (reference = the rule name). Today the plunge stamp names the chip load's LUT row, which is wrong (INVENTORY_G10 §3.3).
6. **The entry rules (Q6-Q10, Q12) go on the ramp record.** The card has no entry row. The funnel's `SuggestWarning::RampFeed` already names θ, r, pitch and the source. It gets an `EntryNotes` list. Each note is one face line on the card and one item in the MCP basis.

### 2.2 Decisions for the operator (before code lands)

- **D1. The range of the tapered-ball claim.** The printed tips are 0.5 mm (Sienci), 0.762 mm (IDC, tip radius 0.015 in) and 1.5875 mm (Sienci). The shanks are 3.175-6.35 mm.
  - (a) Strict: the key is the tip, and the range is 0.5-1.5875 mm. The FM1 tapered cells (tips 3.175 and 6.0) and Wanaka's tip of 2.0 mm stay on the named rule.
  - (b) Per family, with no size range: 152 FM1 cells move ×1.00-1.32. At every one of them the tip cap binds, not 0.5 F.

  **Recommend (a).** It matches the rule "refuse outside the range". Either way, the FM1 tapered plunge is set by the tip cap, not by the claim.
- **D2. The dressup helix radius field (Q6).** Make it `helix_radius: Option<f64>` with `#[serde(default, skip_serializing_if = "Option::is_none")]`.
  - `None` means the named rule 0.3 × D, applied where the tool is known.
  - `Some(r)` is an operator value.
  - A v3 file with `helix_radius = 2.0` loads as `Some(2.0)`. No fixture changes, and no format bump.

  The cost: existing toolpaths keep 2.0 mm as an operator value (the Q6 cap still applies). The other choice is a factor field with format v4, which refuses every saved project. **Recommend the Option.**
- **D3. The default for 2D Adaptive after `PreferHelix` goes.** Replace `EntryStylePolicy::PreferHelix` with `DefaultHelix`. Only `for_op` reads it, when it builds a new toolpath. `normalize_for_op` never overwrites an operator Ramp again. The E2 default stays Helix, and an operator Ramp stays Ramp. **Recommend this.** The other choice: delete the variant, and Adaptive takes Ramp 3°.
- **D4. The Adaptive3d default for the CLI (Q9).** The one rule is the GUI config default: ramp 10°, helix 0.3 × D, pitch 2.0. The CLI's 3°, 0.4 × D and 1.0 go. **Recommend this.**

---

## 3. Part A steps (feeds, tool_load, viz)

An editor without cargo can follow these steps in order. Steps A1-A5 must land in one build, because the type changes break every `FeedsResult` literal.

### A1. The evidence table: `rs_cam_core/src/feeds/extrapolation/plunge.rs` (new)

- In `feeds/extrapolation.rs`:
  - add `pub mod plunge;` and the re-exports;
  - add `Gap::Plunge`, with `group()` = "G10" and `label()` = "plunge";
  - add one line to the module doc.

  No other exhaustive match on `Gap` exists (checked with rg).
- Add `pub const PLUNGE_RULE_TEXT: &str = "plunge = fraction x shipped side feed; the fraction is the printed plunge / feed of the family"`.
- Add these types:
  - `pub enum PlungeKey { Diameter { lo_mm, hi_mm }, Tip { lo_mm, hi_mm }, Angle { deg, tol_deg } }`;
  - `pub enum PlungeFraction { OneOverZ, Fixed(f64) }`;
  - `pub struct PlungeRule { id, tool_family: ToolFamily, key: PlungeKey, fraction, flutes: &'static [u32], materials: &'static [MaterialFamily], witness: &'static str, statements: &'static [&'static str], spread: &'static str, confidence: ClaimConfidence }`.
- The materials of every rule are `Softwood, Hardwood, PlywoodSoftwood, PlywoodHardwood, Mdf`. These are the Sienci columns "Softwood, Soft Plywood, MDF" and "Hardwood, Hard Plywood", and the Amana columns.
- `pub const PLUNGE_RULES: &[PlungeRule]` has four rules. The statement ids are from `fetch/G10/statements.json`. All of them are `confirmed`.

| id | family, key, Z | fraction | witnesses (statement ids) | spread named on the card | confidence |
|---|---|---|---|---|---|
| `g10_plunge_flat` | FlatEnd, Diameter = `DRILL_RULES[0].range_mm` (3.175-6.0), Z = `DRILL_RULES[0].flutes` (2, 3). Read the G6 constants; do not copy them. | 1/Z | Amana Spektra v24 rule and cells (`g10-wood-amana-spektra-rule`, `-2f-quarter`, `-3f-quarter`, `-3f-3-4`; the 8 G6-verified Ramp Down cells in `fetch/G6/verified_rows.json` set the range). Second witnesses: Sienci, 60 rows `g10-hobby-036` … `g10-hobby-130` (0.500-0.505, 1.587-6.35 mm, grade a); IDC up-cut `g10-wood-idc-up-eighth`, `-up-quarter` (0.50); Carbide 3D S3 `g10-hobby-134` … `-138` (median 0.49) | IDC down-cut 0.30 / 0.43 (`g10-wood-idc-down-eighth`, `-down-quarter`); Nomad 0.24-0.44 (`g10-hobby-139` … `-143`) | TwoWitnesses |
| `g10_plunge_ball` | BallNose, Diameter 3.175-25.4, Z 2 | 0.50 | Sienci ball end mills 1/8 and 1/4 in (`g10-hobby-034, -035, -039, -043, -054, -058, -069, -073, -087, -088, -092, -096, -107, -111, -122, -126`, 0.500-0.503) and round groove 12.7-25.4 mm (`-048..-050, -063..-065, -078..-080, -101..-103, -116..-118, -131..-133`, 0.500); Amana ball nose v7 rule F/Z at Z 2 (`g10-wood-amana-ballnose-rule`, grade b) | IDC 0.25 (`g10-wood-idc-ball-eighth`), 0.43 (`-ball-quarter`) | TwoWitnesses |
| `g10_plunge_tapered` | TaperedBallNose, Tip 0.5-1.5875 (D1 a), Z 2 | 0.50 | Sienci tips 0.5 and 1.5875 (`g10-hobby-030..-033`, `-083..-086`); IDC tip Ø 0.762, 10° (`g10-wood-idc-taperball`, 0.417) | Sienci fine 0.334 (`g10-hobby-031`) | OneWitness (one per size) |
| `g10_plunge_vbit60` | ChamferVbit, Angle 60 ± 0.5°, any D (Sienci prints no D: the B4 `AngleKey` precedent), Z 2 | 1/3 | Sienci `g10-hobby-028, -029, -081, -082` (0.330-0.335); IDC 60° `g10-wood-idc-v60` (0.333) | 90° disagrees (IDC `-v90` 0.556); 30° 1F (`-v30` 0.571); Amana insert V-groove 0.5-1.0 (another tool) | TwoWitnesses |

- Add `pub struct PlungeClaim { gap: Gap::Plunge, rule: &'static PlungeRule, fraction: f64 }`. The `fraction` field holds the fraction at the tool's Z. Add `card_text() -> (String, String)`.
  - Headline example: "plunge claim (G10 plunge): 0.50 x feed, ball nose 3.175-25.4 mm, 2 flutes".
  - The detail names `PLUNGE_RULE_TEXT`, the witnesses, the statement ids, the spread and the range.
  - For the flat rule, the detail adds: "Amana Ramp Down is the vertical (Z) rate (ruling Q3); ruling Q4 reads it as feed / Z".
- Add `pub enum PlungeRefusal { NoRuleForFamily(ToolFamily), OutsideRange { rule, key_value }, FlutesNotPrinted { rule, flutes }, AngleNotPrinted { angle_deg }, MaterialNotPrinted(MaterialFamily) }`. Add `text()`. Each sentence names the printed range.
- Add `pub fn plunge_rule(family, diameter_mm, tip_d_mm, angle_deg: Option<f64>, flutes, material) -> Result<PlungeClaim, PlungeRefusal>`.
- Unit tests in the file:
  - each rule accepts its own FM1 key;
  - the flat rule refuses 6.35 mm, 1 flute and 4 flutes;
  - the V-bit rule refuses 90°;
  - every rule refuses acrylic;
  - the flat rule gives 0.5 at Z 2 and 0.333 at Z 3;
  - a test pins the flat rule's range to `DRILL_RULES`.

### A2. The resolver: `rs_cam_core/src/feeds/plunge.rs` (new)

- Add `pub const PLUNGE_BASE_RULE_TEXT`: "no source; repo rule: material base 1000/h (wood), 900/h (plywood, sheet) at 6 mm, x clamp(D/6, 0.25, 3), h = (Janka/600)^0.4 (a port of the Shapeoko reference calculator)".
- Add `pub const TIP_CAP_RULE_TEXT`: "no stored source; repo rule 150 mm/min per mm of tip (a code comment cites FSWizard / GWizard 100-300 mm/min; not stored)".
- Add `pub struct TipCap { tip_d_mm, cap_mm_min }`. Build it from `tool_load::plunge_stress::safe_plunge_cap_mm_min`, which is the one producer (F7).
- Add `pub enum PlungeBasis`:
  - `Claimed { claim: Box<PlungeClaim>, tip_cap: Option<TipCap> }`;
  - `MaterialBase { reason: PlungeRefusal, base_mm_min: f64, tip_cap: Option<TipCap> }`;
  - `DrillCycle`.

  Add `card_text() -> (String, String)`. Headline examples:
  - "Plunge = feed / 2 (G10 plunge claim: flat end mill, Amana Spektra rule, 3.175-6.0 mm, 2-3 flutes)";
  - "Plunge = 0.33 x feed (G10: 60° V-bit; Sienci, IDC)";
  - "Plunge: no source; repo rule material base (bull nose; nearest printed: Amana corner-radius Ramp Down = feed / flutes, no feed-paired witness)".

  Every detail that is not a drill cycle ends with the Q12 sentence: "The engine does not know the cut direction: a down-cut or compression tool must ramp, not plunge (Vortex, AXYZ, grade c; ruling Q12, future work)."
- Add `pub enum PlungeBinding { Rule, TipCap, Feed }`.
- Add `pub fn plunge_basis(input: &FeedsInput<'_>, material_base_mm_min: f64) -> PlungeBasis`:
  1. The drill family gives `DrillCycle`.
  2. Otherwise it builds the key from `input.tool_geometry` and `input.tool_diameter`: the tip for `TaperedBall { tip_radius }` (the same `max(0.5)` floor as the cap) and `included_angle` for `VBit`. It takes the material family from `vendor_normalize::to_lookup_query_unrouted(input)`. Then it calls `plunge_rule`.
- Add `pub fn resolve_plunge(basis: &PlungeBasis, feed_mm_min: f64, flutes: u32) -> Option<(f64, PlungeBinding)>`:
  - value = `round_suggestion_value_down(min(rule, tip cap, feed), 1.0)`;
  - `DrillCycle` gives `None`, so the caller does not write it.
- Unit tests:
  - the fraction at the given F;
  - the tip cap binds on a 1 mm tapered tip;
  - the feed binds when the material base is above F;
  - the value rounds down.

### A3. The calculator: `rs_cam_core/src/feeds/mod.rs`

- Add `pub mod plunge;`, and re-export `PlungeBasis` and `PlungeBinding`.
- `FeedsResult` (~:461): add `pub plunge: plunge::PlungeBasis`, with the doc "the part of the plunge that does not use F; the apply funnel runs it again at the shipped feed".
- Step 8 (:2527-2571):
  - keep `let plunge = material.plunge_rate_base(d);` as the material base;
  - set `let plunge_basis = plunge::plunge_basis(input, plunge);`;
  - set `plunge_rate = resolve_plunge(&plunge_basis, feed, input.flute_count).map_or(plunge, |(v, _)| v)`;
  - delete the inline "Fix 2" cap block (:2552-2571), because the cap is now inside the basis;
  - rewrite the Step 8 and Step 9 comments with the new rule.
- Step 9c (:2742): do not change `plunge_rate = feed` for a drill.
- In the `FeedsResult` literal (:2789), add `plunge: plunge_basis,`.
- Other literals that must add the field:
  - `feeds/provenance.rs` (~:430 test fixture);
  - `diagnostics/tests.rs` (~:654);
  - `tests/cut_efficiency_is_closed_form_g_specenergy.rs:165`;
  - `tests/efficiency_abstains_without_kc_g_specenergy.rs:109`.

  In all of them, use `plunge: PlungeBasis::DrillCycle` or a `MaterialBase` fixture.

### A4. The funnel: `rs_cam_core/src/feeds/suggest/apply.rs` and `suggest.rs`

- `apply_feeds_subset`: after `enforce_invariants` and before the copy-back (:237), and only when the op is not a drill, run the rule again. Write the plunge only when it moved by 1 mm/min or more:
  ```rust
  if let Some((to, binding)) = crate::feeds::plunge::resolve_plunge(&result.plunge, scratch.feed_rate(), tool.flute_count) {
      let from = scratch.plunge_rate();
      if (to - from).abs() >= 1.0 {
          scratch.set_plunge_rate(to);
          warnings.push(SuggestWarning::PlungeReDerived { from_mm_min: from, to_mm_min: to, binding });
      }
  }
  ```
- The ramp block (:244-261) already reads `operation.plunge_rate()` after the copy-back. The order is right: plunge first, then ramp.
- `suggest.rs`:
  - add `SuggestWarning::PlungeReDerived { from_mm_min, to_mm_min, binding }`, with the doc "the funnel ran the plunge rule again at the feed that ships";
  - change `RampFeed` to `RampFeed { from_mm_min, record, notes: crate::feeds::ramp::EntryNotes }` (A5);
  - delete `StrategyRewrote` (A6);
  - change the `AggressivenessSkip::Drill` text (~:748) to "Plunge: the plunge rule; the dial does not act."
- `feeds/rationale.rs`:
  - add an arm for `PlungeReDerived`, with keyword "plunge", so the Plunge row hover carries it;
  - delete `PLUNGE_AT_MATERIAL_BASE_TEXT` (:231) and its readers;
  - delete the `StrategyRewrote` arms (:548, :676) and their tests (:1077, :1097).
- `feeds/provenance.rs`:
  - add `ProvenanceSource::PublishedRule` and `ProvenanceSource::RepoRule`;
  - in `FeedsResult::provenance()` (:367), stamp the plunge by basis: `Claimed` with binding Rule gives `PublishedRule(rule.id)`; a binding TipCap gives `RepoRule("ball_tip_cap")`; `MaterialBase` gives `RepoRule("material_plunge_base")`; `DrillCycle` gives the chip stamp, as today;
  - update `provenance_is_per_field_independent` (:469), because the plunge is no longer `Formula`;
  - extend `stamp_ramp` for the new ramp arm (A5).

### A5. Q2 and the entry notes: `rs_cam_core/src/feeds/ramp.rs`

**The ramp arms (Q2).**
- Rename `RampBasis::PlungeRate { reason }` to `RampBasis::NoChip { reason }`.
- Add `RampArm::PlungeTerm`.
- Add `RampFeed::PlungeSlope(Box<PlungeSlopeRamp>)`. Its fields: `value_mm_min, arm (CutFeed | PlungeTerm), entry, source, tan_theta, theta_deg, plunge_mm_min, plunge_term_mm_min, cut_feed_mm_min, no_chip: RampFallback`.
- Rename the old `PlungeRate { reason, plunge_mm_min }` arm to `NoEntryFeed { reason, plunge_mm_min }`. It writes `None`.

**`resolve_ramp_feed`, in this order:**
1. An entry `Err` (EntryOff, EntryUnknown) gives `NoEntryFeed`.
2. A cut feed ≤ 0 gives `NoCutFeed`.
3. `tan_theta()` `None` gives `DegenerateEntry`.
4. A `Sourced` basis with an RPM gives the G6 arm. Do not change it.
5. Otherwise, a plunge ≤ 0 gives `NoEntryFeed { NoPlunge }`. Add `RampFallback::NoPlunge`.
6. Otherwise `PlungeSlope`: value = `round_down(min(F, plunge / tan θ))`. `no_chip` is the basis reason, or `NoShippedRpm`.

`DrillCycle` stays `NoEntryFeed`.

**The Q2 card text:**
- The face line: "Ramp feed {v} mm/min: the cut feed (plunge limit {t} mm/min = plunge {p} / tan {θ}°)". When the plunge term binds: "…: the plunge limit at {entry} (cut feed {F})".
- The detail: "no source; vertical rate = plunge (repo rule); no G6 chip: {no_chip.text()}".
- Update the endings of `RampFallback::text()`. The fallback reasons now say "so the ramp holds its vertical rate at the plunge". The `NoEntryFeed` reasons keep "the entry uses the plunge rate".

**`entry_geometry`:** return `Entry { geometry, source, clearance_mm }`. The clearance comes from `DressupConfig::entry_clearance_mm` or `Adaptive3dConfig::entry_clearance_mm`.

**The entry notes (Q6-Q10, Q12):**
- Add `pub struct EntryNote { headline: String, detail: String, caution: bool }` and `pub struct EntryNotes(Vec<EntryNote>)`.
- Add `pub fn entry_notes(op: &OperationConfig, dressups: Option<&DressupConfig>, tool: &ToolConfig, pass_role: PassRole) -> EntryNotes`. The notes:
  - **Ramp (Q9):** "Ramp angle {a}°: repo rule (default {d}°), no wood or router source". Or "…: operator value". The detail gives the metal context: "Harvey 3-10° soft, grade c; SGS compression router max 5°". The default is `DressupConfig::default().ramp_angle`, or the `ramp_angle_deg` of `OperationConfig::new_default(OperationType::Adaptive3d)`.
  - **Helix (Q8):** "Helix r {r} mm ({r/D:.2} x D), pitch {p} mm = {θ:.2}°". The detail says r and the pitch are each a repo rule or an operator value. It adds "no wood source (metal context: IMCO 0.5-5°, grade c)".
  - **Helix geometry (Q6, Q7).** Let `c = build_cutter(tool)`. The flat bottom is `c.width_at_height(0.0)`. The height at r is `c.height_at_radius(r)`.
    - Flat and bull tools (flat bottom > 0): if r ≤ the flat bottom, the note is "no core: r ≤ {cap} mm (the flat bottom; geometry; IMCO, Sandvik, Fusion print the same limit)". If r is larger, the note is a caution: "the helix leaves a core Ø {2(r−cap)} mm". Until Part B lands, add "the engine does not cap r yet (G10 Q6)".
    - Ball, tapered and V-bit tools: "the helix leaves a centre pip {h:.2} mm high (derived from the tool profile)". If `height_at_radius` gives `None`, the note is a caution: "a core Ø {2(r−R)} mm".
  - **Clearance (Q10):** "Entry starts {c} mm above the material: no source; operator rule 0.5 mm". If the value is not 0.5, the note says "operator value".
  - **Straight plunge on a Roughing op (Q12):** a caution. "Straight plunge entry: a down-cut or compression tool must ramp (Vortex, AXYZ, grade c); the engine has no cut-direction attribute (ruling Q12, future work)."
- In `apply.rs`, build the notes in the ramp block, with the op after the copy-back, and push them in the `RampFeed` warning.

**Unit tests:** split `every_fallback_writes_none` into two tests:
- `every_entry_fallback_writes_none`: EntryOff, EntryUnknown, DegenerateEntry, NoCutFeed.
- `a_no_chip_ramp_holds_the_plunge_slope`: NoLut and NoShippedRpm give `Some(min(F, p / tan θ))`.

Add `the_pip_and_the_core_come_from_the_tool_profile`. It uses a 6 mm ball at r 1.8 (pip 0.286), a 3.175 mm flat at r 2.0 (core 0.825) and a 60° V-bit at r 1.8 (pip 3.118).

### A6. Q11, the feeds half: `feeds/suggest/adaptive_entry.rs`, `invariants.rs`, `geometry_class.rs`

- Delete `pick_adaptive3d_entry_style` (adaptive_entry.rs:29-149), its import and the call at invariants.rs:172-179. Rewrite the NOTE comment at :117-121.
- Keep `check_plunge_entry_stability` and `adaptive_plunge_entry_label`.
- Keep `SuggestScope`. `pick_adaptive3d_clearing_strategy` still reads it.
- Delete `geometry_class::helix_feasible_in_bbox` (:116-160), its tests (:246-295) and the doc reference (:44).
- Update the module doc of `adaptive_entry.rs`.
- `feeds/suggest/tests.rs`: delete the rewrite tests at :2311 and :2422. Add `suggest_never_writes_entry_style`: every `Adaptive3dEntryStyle`, before and after, on the Wanaka-shaped input.

### A7. The Q3 text: `feeds/extrapolation/drill.rs`

- Change the `DrillClaim::card_text` detail (:218). Old text: "ruling B5 reads it as a straight plunge". New text: "rulings B5 and Q3 read it as the vertical (Z) rate: a straight plunge, and the vertical limit of a ramp".
- The test needle "straight plunge" (:391) still matches. Add the needle "vertical (Z) rate".

### A8. The viz card and MCP

- `rs_cam_viz/src/ui/feeds/why.rs`:
  - `explain_plunge` (:266): replace the fixed sentence and `PLUNGE_AT_MATERIAL_BASE_TEXT` with `explain.recommended.plunge.card_text()` (headline and detail).
  - `draw_row_basis_lines` (:565): add the plunge headline as one visible `detail_line` (tone `WARNING_MILD` for a claim, `TEXT_DIM` for the named rule). The hover is the detail. It reads `explain.recommended.plunge`, not `matched_row`.
  - `suggest_line` (:920): the `RampFeed` arm paints the record line (all three arms), then one line per note. A caution note gets "⚠". `PlungeReDerived` returns `None`; it is a row hover through the rationale. Delete the `StrategyRewrote` arm (:1068).
  - Update the module doc (the ramp record and the entry notes).
- `rs_cam_viz/src/ui/components/provenance.rs:102`: add badge kinds for `PublishedRule` and `RepoRule`, with a glyph and a token colour.
- `rs_cam_viz/src/app/mcp/generation.rs:59-99`: move the basis into a pure fn `suggest_basis_json(profile) -> serde_json::Value`, so a test can call it. Add these keys:
  - `"plunge": { "source": {headline, detail} from feeds.plunge.card_text(), "re_derived": {from, to, binding} | null }`;
  - `"entry": [ {headline, detail, caution} … ]`, from the `RampFeed` notes;
  - `"ramp"` also handles the `PlungeSlope` arm.
- `rs_cam_viz/src/ui/feeds/CLAUDE.md`: add "the plunge basis is one face line; the entry notes are face lines under the ramp record".

### A9. The matrix, the docs and the notes

- `rs_cam_core/tests/feeds_matrix_instrument_fm1.rs`: add the columns `plunge_basis` (Claimed / MaterialBase / DrillCycle), `plunge_rule` (rule id or refusal), `plunge_binding`, `ramp_feed_mm_min`, `ramp_arm` and `entry_notes`. Run it again (`-- --ignored`) and commit the CSV. Check first that no peer session is writing to `planning/feeds_matrix_2026-09-23/`.
- `feeds/CLAUDE.md`: in Files, add `plunge.rs` and `extrapolation/plunge.rs`. Add the invariant "G10: plunge = the family fraction x shipped feed inside `PLUNGE_RULES`, else the named material base; ball and tapered tip cap 150/mm; non-G6 ramp = min(F, plunge / tan θ)". Add the new sentries.
- `CREDITS.md`: add the sources that are not listed yet: Sienci metric chart, IDC Woodcraft chart, Amana ball nose v7, Amana corner radius, Carbide 3D S3.
- `PROGRESS.md`: G10 Part A landed; G-ENTRYREWRITE is closed.
- `planning/extrapolation_2026-09-24/`: record the FM1 moves in a short RESULTS section of this plan.

### A10. The new sentries

1. **`rs_cam_core/tests/a_plunge_is_a_named_fraction_of_the_side_feed_g10.rs`**
   - (a) Pocket, 6 mm 2F flat, hardwood, generic router: `plunge == floor(feed / 2)`, and the basis is `Claimed(g10_plunge_flat)`.
   - (b) The same at 3F: `floor(feed / 3)`.
   - (c) 6 mm ball: `min(floor(0.5F), 900)`, and the binding is named.
   - (d) 60° V-bit 12.7 mm: `floor(F/3)`, and no `PlungeClampedToFeed`.
   - (e) Refusals fall back to `MaterialBase`, and the text contains "no source; repo rule": a 6.35 mm flat, a 6 mm 4F flat, a 6 mm bull, a 90° V-bit, a 6 mm flat in acrylic, and a tapered tip of 3.175 (D1 a).
   - (f) A 1 mm tapered tip in wood: the tip cap binds, and the plunge is ≤ 150.
   - (g) Drill and AlignmentPinDrill: plunge == feed, as before.
   - (h) After pass 9 lowers the depth across a tier, the funnel's plunge == `floor(fraction × final F)`, and one `PlungeReDerived` is filed.
   - (i) An explored feed of 1200 on a ball: plunge = 600.
   - (j) Provenance: `PublishedRule(g10_plunge_flat)`; `RepoRule` on the bull.
   - (k) A test pins the flat claim's range to `DRILL_RULES`.
2. **`rs_cam_core/tests/a_ramp_off_the_g6_claim_holds_its_vertical_rate_at_the_plunge_g10.rs`**
   - A bull 6 mm Pocket (dressup Ramp 3°): `ramp_feed_rate == floor(min(F, plunge / tan 3°))`, with arm `CutFeed`.
   - A ball 3.175 mm Pocket with a dressup Helix r 0.95 / pitch 1 (9.49°): arm `PlungeTerm`.
   - EntryOff and EntryUnknown write `None`.
   - The notes: the angle is 3.00°; the clearance line contains "operator rule 0.5 mm"; a 6 mm ball at r 1.8 gives pip 0.29; a 3.175 flat at r 2.0 gives core 0.83 and a caution; a Roughing op with a straight plunge gives the Q12 caution.
3. **`rs_cam_core/tests/suggest_leaves_the_entry_style_to_the_operator_g10.rs`**
   - Adaptive3d Plunge at dpp > 0.5 D, with and without a model bbox: the style stays Plunge on the shipped op and in `s.operation`, and `PlungeEntryUnstableAtDpp` fires.
   - An operator Helix or Ramp is kept.
4. **`rs_cam_viz/tests/the_card_names_every_entry_rule_g10.rs`**
   - `the_plunge_line_names_its_claim_or_rule_g10`.
   - `the_entry_lines_name_angle_pitch_pip_and_clearance_g10`.
   - `the_mcp_basis_carries_plunge_and_entry_g10`, which calls `suggest_basis_json`.

---

## 4. Expected FM1 moves

These are estimates from the committed CSV. They use the shipped feed and plunge columns. The count is the 526 ok cells that are not drill cells. Confirm each against the new columns after the run.

| Family (ok cells) | Plunge after Part A | Ratio new / CSV | Notes |
|---|---|---|---|
| EndMill (120) | F/2, all in range (3.175, 6.0, 2F, wood / plywood / MDF) | 3.175: ×1.29-5.56 (Pocket ×3.46-5.36); 6.0: ×0.67-2.93 (Pocket ×2.00-2.93) | Some 6 mm finishing ops with a low F **fall** (×0.67). The plunge on the 3.175 mm tools reaches about 576-630 mm/min per mm. That is near the drill envelope ceilings (580 / 580 / 720), but no gate compares a milling plunge with them. |
| BallNose (98) | min(0.5F, 150 D) | ×0.69-1.28 | The tip cap binds on 54 of 98. |
| TaperedBallNose (152) | D1 (a): no move. D1 (b): ×1.00-1.32 | - | The tip cap binds on all 152 under (b). |
| VBit (48) | F/3 | 12.7: ×0.33-0.61; 6.35: ×0.43-1.23 | The 18 `PlungeClampedToFeed` warnings go. |
| BullNose (108) | no move (named rule) | 1.00 | |
| Drill (16) | no move | 1.00 | |

The other moves:
- **Q2 ramp.** 150 cells change from `ramp_feed_rate` = None to Some(F): the E1 and E2 cells that are not flat (Bull 48, Ball 34, Tapered 48, VBit 20). At 3° and at 4.55° (r 2, pitch 1), plunge / tan θ ≥ F on every one of them. The lowest margin is tapered 3.175 E2: 360 / 0.0796 = 4524 against F ≤ 4000. The E3 cells stay `None`: the op ships Plunge, which is EntryOff.
- **Q11.** On the 30 ok E3 cells, `suggest_warnings` loses `StrategyRewrote` and gains `PlungeEntryUnstableAtDpp`. No number moves.
- **New columns.** Every ok cell gets `plunge_basis` and `entry_notes`. `PlungeReDerived` appears where pass 9 or pass 10 moved the feed.

After Part B, with the E2 default helix at 0.3 × D, pitch 1:
- On the 3.175 mm tools, r = 0.95 and θ = 9.49°. The Q2 plunge term binds on about 11 E2 cells (bull 4, ball 3, tapered 4), with a ramp below F. The flat G6 cells stay at F (chip term 10 940 or more).
- On the 6 mm tools, r = 1.8 and θ = 5.05°. Every cell ships F.
- No flat or bull helix leaves a core.

---

## 5. Risks

- **R1. New Adaptive3d toolpaths plunge-enter (F6).** The warning doc at `adaptive_entry.rs:215-224` says an Adaptive3d plunge at dpp > 0.5 D once made "no cutting moves" (Wanaka 3D Rough 6, 2026-06-03). Before merge, generate the Wanaka Back Rough and 3D Rough 6 with Plunge on the peck ladder. Confirm that they cut. If they do not, the defect is the planner's, not Suggest's; report it and do not add a rewrite back.
- **R2. The flat plunge rises by 2-5x.** Two measures read the op plunge and loosen as it rises: the feed-modulation plunge guard (`dressup/feed_modulation.rs:917-925`) and `sim_triage::plunge_class_finding` (a ratio to the op plunge). This is behaviour, not a test failure. Watch `plunge_guard_p3`, `plunge_guard_ab_p3`, `feedopt_caps_plunges_g_feedoptplunge` and the Wanaka run.
- **R3. The Q4 range edge is a step.** A 6.0 mm flat plunges at F/2, about 2000 mm/min. A 6.35 mm flat stays on the base, about 1058. The card states the refusal. The Spektra transcription (`PROMPT_SPEKTRA.md`) can widen `DRILL_RULES`, and the flat plunge rule follows it with no edit.
- **R4. The pins checked.**
  - `apply_contract_a3::pocket_fixture_recipe_fingerprint_is_unmoved`: the tool is 6.35 mm (1058.33 = 1000 × 6.35/6), which is outside Q4. The plunge stays 1058. No re-pin.
  - `explore_apply_takes_the_clamps_but_keeps_the_dragged_point`: the same tool gives min(1058, 120) = 120. No re-pin.
  - `feeds/suggest/tests.rs:553-584` (explore, plunge 900): check the tool. If it is a family with a claim, the pin becomes fraction × explored feed. Re-pin with the cause, in the a3 manner.
  - `wanaka_defaults_validation`: ≤ 200 holds through the cap; > 300 holds (no LUT, so the base, about 700).
  - `feeds/tests.rs:236` and `:278`: the cap holds; the value stays > 400.
- **R5. The literature matrix.** The `plunge_feed` bands are 0.30-0.50 (Minor, report only). The flat and ball cells land on 0.50. Check `band_check` with `EDGE_FRACTION`. The V-bit cells move into the band (0.33).
- **R6. The card gets longer.** A roughing card gains three or four face lines: the plunge basis, the angle or helix line, the pip or core line, and the clearance line. The notes are short. `the_feeds_window_fits_the_screen_g_feedsfit` and `every_side_panel_fits_its_width_g_panelfit` must still pass.
- **R7. `export.rs` `from_suggest`** (`session/compute/export.rs:372-376`) does not list the new provenance sources. The feed stamp still marks the toolpath as suggested, so no export changes. Add the two variants in Part B (it is an e2 file).
- **R8. The Part A card tells the truth before Part B.** Until B lands, the core note says "the engine does not cap r yet". Part B changes that note in the same commit as the cap.

---

## 6. Part B: the entry code (after rs-cam-e2's By Area WP1 merges)

### 6.1 The exact file list (send it to rs-cam-e2 first)

Paths start at `crates/`.

**Production files (edit):**
1. `rs_cam_core/src/compute/config.rs`
   - `DressupConfig.helix_radius: Option<f64>` (D2), and the same in the wire struct (:805), both `From` impls (:839, :879), `FIELD_DEFS` (:936) and `DressupEntryStyle::to_core` (:623-635), which now needs `tool_diameter`.
   - Named constants: `DRESSUP_RAMP_ANGLE_DEG = 3.0`, `DRESSUP_HELIX_PITCH_MM = 1.0`, `HELIX_RADIUS_OVER_D = 0.3`, each with the doc "repo rule, no source (G10)". The defaults at :1036-1038 read them.
   - `for_op` gives Helix at construction for `DefaultHelix` (D3). Delete the `PreferHelix` arm of `normalize_for_op` (:1193-1198). Update the tests at :1516-1560.
2. `rs_cam_core/src/compute/catalog/schema.rs`: `EntryStylePolicy::PreferHelix` becomes `DefaultHelix`, and `PREFER_HELIX` becomes `DEFAULT_HELIX` (:470-510).
3. `rs_cam_core/src/compute/catalog/registry.rs`: the row at :825 uses `DressupPolicy::DEFAULT_HELIX`.
4. `rs_cam_core/src/compute/catalog.rs`: add `pub fn entry_feed_rate(&self) -> Option<f64> { self.ramp_feed_rate().map(|r| r.min(self.feed_rate())) }` beside :1145 (G-RAMPCLAMP).
5. `rs_cam_core/src/compute/operation_configs.rs`: add the named constants `ADAPTIVE3D_RAMP_ANGLE_DEG = 10.0` and `ADAPTIVE3D_HELIX_PITCH_MM = 2.0`. The helix factor reads `HELIX_RADIUS_OVER_D`. The Adaptive3d defaults (~:796-840) and the serde default fns read them.
6. `rs_cam_core/src/compute/execute/dressup_apply.rs:625-650`: the helix radius is `cfg.helix_radius.unwrap_or(HELIX_RADIUS_OVER_D * tool_diameter)`, capped at the flat bottom `cutter.width_at_height(0.0)` when that is > 0 (Q6). Record the capped value in the trace `SemanticKey::Radius`.
7. `rs_cam_core/src/compute/execute/finish_3d.rs`: at :66, apply the same cap to `ctx.tool_def.diameter() * cfg.helix_radius_factor`; at :159, use `ramp_feed_rate: op.entry_feed_rate()`.
8. `rs_cam_core/src/session/compute.rs:2269`: `ramp_feed_rate: tc.operation.entry_feed_rate()`.
9. `rs_cam_core/src/session/compute/export.rs:372-376`: add `PublishedRule | RepoRule` to `from_suggest` (R7).
10. `rs_cam_cli/src/job.rs`: at :1113-1121, the 2.5D entry reads `DRESSUP_RAMP_ANGLE_DEG`, `helix_radius: None` (the rule) and `DRESSUP_HELIX_PITCH_MM`. At :1344-1356, Adaptive3d uses `HELIX_RADIUS_OVER_D`, `ADAPTIVE3D_HELIX_PITCH_MM` and `ADAPTIVE3D_RAMP_ANGLE_DEG` (D4). Delete the "Pre-T9 0.4 x D" comment.
11. `rs_cam_viz/src/ui/properties/linking_dressup.rs:342-351`: the helix radius control has a "0.3 x D (rule)" state and an operator mm value.
12. `rs_cam_viz/src/app/gpu_upload.rs:1137`: the render radius is the resolved radius (tool D × rule, or the operator value, then capped).
13. `rs_cam_viz/src/ui/properties/feeds_speeds.rs:870`: the entry diagram reads the resolved radius.
14. `rs_cam_viz/src/mcp_server.rs:1162`: the description says "helix_radius (mm; null = 0.3 x tool diameter, repo rule)".
15. `rs_cam_core/src/feeds/ramp.rs` (forced by D2):
    - `entry_geometry` resolves `None` to 0.3 × D and applies the same cap;
    - the core note says "capped {r} → {cap} mm (no-core rule, geometry)" in place of "the engine does not cap r yet";
    - the ramp record reads `op.entry_feed_rate()` when it reports the value the entry runs at.

**Files that need no edit, and why:**
- `rs_cam_core/src/dressup/entry_descent.rs`: the clamp and the cap sit upstream (F3). Do not use its `feed_rate` argument for G-RAMPCLAMP.
- `dressup/mod.rs`, `adaptive3d/path.rs`, `adaptive3d/mod.rs`: they receive the clamped feed and the capped radius.
- `session/mutation/config.rs`, `session/project_file.rs`, `session/mutation/toolpath.rs`: they call `normalize_for_op`, and its signature does not change.
- `render/toolpath_render.rs`, `render/upload_cache.rs:451`, `ui/overlays/legend_rail.rs:738`: these are the render struct's own `helix_radius` fields, filled by `gpu_upload.rs`.
- Every fixture TOML that carries `helix_radius = 2.0` loads as `Some(2.0)` under D2.

**Tests (edit):**
- `rs_cam_core/tests/finishing_defaults_have_no_ramp_entry_r10.rs:135`: an operator Ramp now stays Ramp. A new Adaptive toolpath is Helix.
- `rs_cam_core/tests/adaptive3d_interior_cell_parity_f029.rs:20`: the comment.
- `rs_cam_core/tests/entry_moves_stock_aware_g_rampterrain.rs:100`, `rs_cam_core/tests/adaptive3d_entry_stock_aware.rs:595`, `rs_cam_core/tests/a_ramp_feed_is_the_g6_chip_over_the_entry_slope_g6ramp.rs` (`helix_dressup`): write `Some(r)`.
- `rs_cam_core/src/compute/execute/tests.rs:1476,1521`: check them.
- `rs_cam_core/src/session/mod.rs:2150`: the string test loads `Some(2.0)`; check the assertion.
- `rs_cam_viz/src/compute/worker/tests.rs:726`: the JSON value is now `null`.
- `rs_cam_viz/tests/dressup_field_names_are_published.rs`: the published names do not change; run it.

### 6.2 Part B steps

- **B1 (D3).** Change `PreferHelix` to `DefaultHelix` in schema.rs, registry.rs and config.rs. Only `for_op` reads it. Update the tests in config.rs and r10.
- **B2 (Q6 and D2).** Change the `helix_radius` Option in config.rs (the field, the wire, the `From` impls and `FIELD_DEFS`). Resolve and cap the radius in dressup_apply.rs and finish_3d.rs. Mirror the change in ramp.rs. Change viz 11-14.
- **B3 (Q8, Q9).** Add the named constants in config.rs and operation_configs.rs. Make the CLI mapping in job.rs read them. The card notes already read the defaults through `DressupConfig::default()` and `new_default(Adaptive3d)`, so they follow with no edit.
- **B4 (G-RAMPCLAMP).** Add `OperationConfig::entry_feed_rate()` in catalog.rs. Use it in session/compute.rs:2269 and finish_3d.rs:159. Add R7 in export.rs.
- **B5 (sentry).** Add **`rs_cam_core/tests/a_helix_leaves_no_core_and_a_ramp_never_outruns_the_feed_g10.rs`**:
  - a dressup Helix of r 2.0 on a 3.175 mm flat is emitted at r 1.5875;
  - on a 3.175 mm bull (rc 0.476) it is emitted at r 1.111;
  - a 6 mm ball is not capped, and the card shows the pip;
  - with `ramp_feed_rate = 4000` and the feed lowered to 1500, every `EntryHelix` and `EntryRamp` move is ≤ 1500, on the dressup door and on Adaptive3d;
  - a v3 project with `helix_radius = 2.0` loads `Some(2.0)`;
  - a new Adaptive toolpath is Helix, and an operator Ramp survives a dressup write and a project load;
  - the CLI Adaptive3d `entry_3d = "ramp"` writes 10°.

---

## 7. Test list

**Part A (the orchestrator runs these):**
- `cargo test -p rs_cam_core -q --lib feeds::plunge`
- `cargo test -p rs_cam_core -q --lib feeds::extrapolation`
- `cargo test -p rs_cam_core -q --lib feeds::ramp`
- `cargo test -p rs_cam_core -q --lib feeds::provenance`
- `cargo test -p rs_cam_core -q --lib feeds::rationale`
- `cargo test -p rs_cam_core -q --lib feeds::suggest`
- `cargo test -p rs_cam_core -q --lib feeds::geometry_class`
- `cargo test -p rs_cam_core -q --lib feeds::tests`
- `cargo test -p rs_cam_core -q --lib tool_load::plunge_stress`
- `cargo test -p rs_cam_core -q --lib diagnostics`
- New sentries:
  - `cargo test -p rs_cam_core -q --test a_plunge_is_a_named_fraction_of_the_side_feed_g10`
  - `cargo test -p rs_cam_core -q --test a_ramp_off_the_g6_claim_holds_its_vertical_rate_at_the_plunge_g10`
  - `cargo test -p rs_cam_core -q --test suggest_leaves_the_entry_style_to_the_operator_g10`
- Changed sentries:
  - `cargo test -p rs_cam_core -q --test a_ramp_feed_is_the_g6_chip_over_the_entry_slope_g6ramp`
  - `cargo test -p rs_cam_core -q --test a_flat_end_plunge_is_the_side_chip_over_z_g6`
  - `cargo test -p rs_cam_core -q --test the_dial_holds_the_load_and_never_cuts_the_feed_fm7`
  - `cargo test -p rs_cam_core -q --test cut_efficiency_is_closed_form_g_specenergy`
  - `cargo test -p rs_cam_core -q --test efficiency_abstains_without_kc_g_specenergy`
- Pins to watch:
  - `cargo test -p rs_cam_core -q --test pill_writes_clamped_value_g_pillclamp`
  - `cargo test -p rs_cam_core -q --test suggest_feed_matches_final_geometry`
  - `cargo test -p rs_cam_core -q --test a_clamped_feed_ships_at_or_below_its_ceiling_g_feeddown`
  - `cargo test -p rs_cam_core -q --test a_rescaled_feed_stays_inside_the_power_ceiling_g_t15`
  - `cargo test -p rs_cam_core -q --test suggest_power_ceiling_after_pass9_g_suggest_powerstale`
  - `cargo test -p rs_cam_core -q --test wanaka_defaults_validation`
- Behaviour to watch (R2):
  - `cargo test -p rs_cam_core -q --test plunge_guard_p3`
  - `cargo test -p rs_cam_core -q --test plunge_guard_ab_p3`
  - `cargo test -p rs_cam_core -q --test plunge_guard_bandless_p3`
  - `cargo test -p rs_cam_core -q --test feedopt_caps_plunges_g_feedoptplunge`
- `cargo test -p rs_cam_core -q --test literature_matrix` (report; R5)
- `cargo test -p rs_cam_core -q --test wanaka_suggest_integration` (the Q11 inversion at :489-518)
- `cargo test -p rs_cam_core -q --test feeds_matrix_instrument_fm1 -- --ignored` (writes the CSV)
- Viz, new: `cargo test -p rs_cam_viz -q --test the_card_names_every_entry_rule_g10`
- Viz, changed or watched:
  - `cargo test -p rs_cam_viz -q --test every_stage_that_moves_a_number_is_on_the_card_g_visible`
  - `cargo test -p rs_cam_viz -q --test apply_contract_a3`
  - `cargo test -p rs_cam_viz -q --test the_recommended_column_is_what_apply_writes_g_recomapplied`
  - `cargo test -p rs_cam_viz -q --test the_recommendation_explains_each_row_g_whyrow`
  - `cargo test -p rs_cam_viz -q --test a_claimed_row_states_its_claim_on_the_card_g_claimcard`
  - `cargo test -p rs_cam_viz -q --test the_speeds_apply_holds_the_cut_g_speedsonly`
  - `cargo test -p rs_cam_viz -q --test the_feeds_window_fits_the_screen_g_feedsfit`
  - `cargo test -p rs_cam_viz -q --test every_side_panel_fits_its_width_g_panelfit`
  - `cargo test -p rs_cam_viz -q --test mcp_wire_surface_pin`
- `cargo build -p rs_cam_cli --examples`

**Part B:**
- `cargo test -p rs_cam_core -q --test a_helix_leaves_no_core_and_a_ramp_never_outruns_the_feed_g10`
- `cargo test -p rs_cam_core -q --lib compute::config`
- `cargo test -p rs_cam_core -q --test finishing_defaults_have_no_ramp_entry_r10`
- `cargo test -p rs_cam_core -q --test adaptive3d_entry_stock_aware`
- `cargo test -p rs_cam_core -q --test entry_moves_stock_aware_g_rampterrain`
- `cargo test -p rs_cam_core -q --test ramp_contained_in_region_g_rampcontain`
- `cargo test -p rs_cam_core -q --test drill_entry_dressup_g_wanaka_drill_ramp`
- `cargo test -p rs_cam_core -q --test toolpath_fields_round_trip_c11`
- `cargo test -p rs_cam_core -q --test a_ramp_feed_is_the_g6_chip_over_the_entry_slope_g6ramp`
- `cargo test -p rs_cam_viz -q --test dressup_field_names_are_published`
- `cargo test -p rs_cam_viz -q --test mcp_wire_surface_pin`
- `cargo test -p rs_cam_cli -q`
- The feeds matrix instrument again (the E2 moves in §4).

### Critical Files for Implementation
- /home/ricky/personal_repos/rs_cam/crates/rs_cam_core/src/feeds/ramp.rs
- /home/ricky/personal_repos/rs_cam/crates/rs_cam_core/src/feeds/mod.rs
- /home/ricky/personal_repos/rs_cam/crates/rs_cam_core/src/feeds/suggest/apply.rs
- /home/ricky/personal_repos/rs_cam/crates/rs_cam_core/src/feeds/extrapolation/drill.rs
- /home/ricky/personal_repos/rs_cam/crates/rs_cam_viz/src/ui/feeds/why.rs
---

## 8. Operator decisions (2026-09-25)

- D1: strict. The tapered-ball plunge claim covers tips 0.5-1.5875 mm only.
- D2: `helix_radius: Option<f64>`; None = 0.3 x D (named rule); saved
  projects load their 2.0 mm as an operator value; no format bump.
- D3: `DefaultHelix`: a new 2D Adaptive starts with Helix; the engine never
  changes the entry style afterwards.
- D4: one Adaptive3d entry rule from the GUI values (10°, 0.3 x D, pitch
  2 mm) for the GUI and the CLI.

Also ruled (2026-09-25): a ProjectCurve on a V-bit routes to the printed
V-bit Trace rows (the wanaka Rivers and Lakes toolpaths). That landed with
the hardwood V-bit refusal package, not here.
