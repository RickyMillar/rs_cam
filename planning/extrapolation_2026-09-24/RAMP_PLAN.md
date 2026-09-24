# RAMP_PLAN: the sourced ramp feed (Suggest writes `ramp_feed_rate`)

Scope: the approved design in RULINGS.md §"Open proposal (2026-09-25)". The ramp feed is `min(cut feed, axial_chip × rpm × Z / tan θ)`. The axial chip comes from the G6 drill claim. Every other cell writes `None` and the card says why. This plan removes the old clamp. It adds no shims.

---

## 0. Findings the editors must know first

**F1. The chip term almost never wins at the shipped entry angles.** The axial chip is the side chip / Z (drill.rs:7-9). So the vertical rate is `side × rpm`, and the uncapped cut feed is `side × rpm × Z × tier`. The chip term is therefore smaller than the cut feed only when `tan θ > 1 / (Z × tier)`. That is above 26.57° for 2 flutes and above 18.43° for 3 flutes, at depth tier 1. The shipped defaults are a 3° ramp, a helix of 4.55° (r 2, pitch 1), and the Adaptive3d 10° ramp or 10.03° helix (0.3 × D, pitch 2). None of them comes near that angle. In practice, the operator approves "on G6 cells the helix runs at the cut feed". The card will say "cut feed" on almost every cell. The sentry needs one steep-angle case to prove that the min works.

**F2. An Adaptive3d entry-style rewrite does not reach the operation.** `pick_adaptive3d_entry_style` (adaptive_entry.rs:132, called from invariants.rs:177) writes `entry_style` on the scratch clone. `apply_feeds_subset` copies back only feed, plunge, rpm, stepover and depth per pass (apply.rs:216-229). So `StrategyRewrote` reports a change that never ships. This is an existing defect and out of scope. It still sets a rule for this plan: **θ is read from the operation that ships (the real `operation` after the copy-back), not from the scratch.** If someone later fixes the copy-back, the resolver follows it with no change. Report the defect to the orchestrator.

**F3. `FeedsInput` is not `Clone`** (feeds/mod.rs:379, no derive; `SetupContext` derives only `Default`, mod.rs:331). Build the drill query from `LookupQuery`, not from a copy of the input.

**F4. The ruling uses tan θ, and the physics uses sin θ.** The machine feed is the path speed, so its vertical part is F × sin θ. The ruling's `/ tan θ` is lower than `/ sin θ` by a factor of cos θ (0.9986 at 3°, 0.997 at 4.55°). That is conservative. Keep tan θ as the ruling says, and state it once in the detail text.

---

## 1. Answers to the questions

### A. Can the explain door see θ? Where does θ enter?

No. `feeds_explain_for_operation` takes the operation, tool, material, machine, LUT and spindle strategy only (suggest.rs:998-1009). `FeedsInput` has no entry geometry (mod.rs:379-424). `FeedsExplain` carries `recommended: FeedsResult` and nothing about dressups (explain_payload.rs:60-90).

θ must not go into `FeedsInput`. The calculator cannot know the feed or the rpm that ship:
- pass 9 re-derives the feed (invariants.rs, `rescale_feed_to_final_geometry`), and pass 10 lowers it (`recheck_power_after_rescale`);
- `with_explored_speeds` replaces feed and rpm (apply.rs:660-672);
- apply rounds the rpm (apply.rs:147).

**Decision: split the value into two parts.**
1. **The sourced half, independent of θ, on `FeedsResult`.** The new field `ramp: RampBasis` is set in `calculate` step 8. Its arms are `Sourced { axial_chip_mm, drill: Box<DrillClaim>, size: Option<Box<Claim>> }` and `PlungeRate { reason }`. `explain_feeds` carries it to the card, and `CutterOpProfile.feeds` carries it to MCP. The vertical rate is not stored here, because it needs the rpm that ships (decision 3). This refines decision 1, based on the evidence above.
2. **The number, in the funnel.** `apply_feeds_subset` computes it after `enforce_invariants` (apply.rs:177), with one pure function `feeds::ramp::resolve_ramp_feed`. θ comes from `SuggestContext.dressups: Option<&'a DressupConfig>` for dressup ops, and from the operation for Adaptive3d (F2). A new `SuggestWarning::RampFeed { from_mm_min, record }` carries the result. Every surface already receives that vector:
   - the card, through `suggest_for_operation` (feeds_speeds.rs:189) → `why::draw_suggest_lines` (why.rs:1060);
   - MCP and the CLI, through `cutter_op_profile` → `profile.warnings` (generation.rs:38-78; cli project.rs:1052);
   - export, through `suggest_warnings_for_toolpath` (export.rs:361).

   The card calls the same funnel with the same context as the controller apply (events/mod.rs:1237). So GUI, MCP and CLI show one number.

**Refinement of decision 2: carry the `DressupConfig` reference, not a resolved `EntryGeometry`.** The resolver needs the operation for Adaptive3d, because the registry forces the dressup entry to `None` for Adaptive3d (registry.rs:1039) and its entry lives on `Adaptive3dConfig` (operation_configs.rs:701-712). `Option<&DressupConfig>` is `Copy`, so `SuggestContext` stays `Copy + Default`. The resolver goes in the new file `feeds/ramp.rs`. A method on `DressupConfig` would land in compute/config.rs, which rs-cam-e2 owns.

**MCP basis.** `mcp_get_suggest_rationale` (generation.rs:59-75) gets a new `"ramp"` key with two parts:
- `source`: `feeds.ramp.card_text()`, the claim independent of θ;
- `headline` and `detail`: from the `RampFeed` record in `profile.warnings`, which holds the number, the winning arm and θ.

The rationale tree gets the same record as one row (new `RationaleParam::RampFeed`).

### B. Production `SuggestContext { … }` literals

| Site | `..Default` | ToolpathConfig / dressups in scope | Action | Owner |
|---|---|---|---|---|
| cli/examples/apply_suggest_save.rs:72 | **no (full)** | yes, `tc` | add `dressups: Some(&tc.dressups)` (needed to compile) | cli |
| cli/src/smoke.rs:539 | yes | built later, `for_op(op_type)` at :560 | none: `suggest_params` defaults it (step 2) | cli |
| viz/app/mcp/commands.rs:818 | yes | built later, `for_op(op_type)` at :859 | none (same default) | viz |
| viz/controller/events/toolpath.rs:126 | yes | built later, `for_op(op_type)` at :178 | none (same default) | viz |
| viz/controller/events/model.rs:871 | yes | AlignmentPinDrill | none (drill, no write) | viz |
| viz/controller/events/mod.rs:1048 | yes | `tc` | none: `resolve_operation_invariants` does not write the ramp | viz |
| viz/controller/events/mod.rs:1237 | yes | `draft` | **add `dressups: Some(&draft.dressups)`** (disjoint field borrow with `&mut draft.operation`) | viz |
| core/feeds/suggest/apply.rs:164 | `..context` | — | none, the field passes through | feeds |
| viz/ui/properties/pills.rs:155 | yes | no (operation only) | none: no ramp pill exists; `FieldApplyPreviews` gets no ramp slot | viz |
| viz/ui/properties/feeds_speeds.rs:201 and :227 | yes | yes, `entry.dressups` | **add `dressups: Some(&entry.dressups)` to both** | viz |
| core/session/compute/export.rs:405 | yes | `tc` | add `dressups: Some(&tc.dressups)` | **e2** |
| core/session/compute.rs:1304 | yes | Adaptive3d only | none: θ comes from the operation | **e2** |
| core/session/multitool.rs:867 | yes | built later, `for_op(dressup_op)` at :482 | pass `Some(&DressupConfig::for_op(dressup_op))` and move the `dressup_op` match above the call (the finish tiers then read "entry off", not "entry unknown") | **e2** |
| core/session/mod.rs:1621 (`cutter_op_profile`) | yes | `tc` | **add `dressups: Some(&tc.dressups)`** (MCP basis, CLI apply) | **e2** |

Test literals: feeds/suggest/tests.rs:1493 and tests/wanaka_suggest_integration.rs:1189 are full literals and must be changed. All the others use `..`. g_visible.rs:507 must mirror the card context (`dressups`).

**Edits in rs-cam-e2 files:** session/mod.rs, session/compute/export.rs, session/multitool.rs, and session/compute/params.rs (manual stamp, step 5). The plan needs **no edit** in compute/*, adaptive3d/* or dressup/entry_descent.rs. The resolver only reads `OperationConfig`, `Adaptive3dConfig` and `DressupConfig`. The write uses `op.as_params_mut().set_ramp_feed_rate(..)`, because `OperationConfig` has a getter at catalog.rs:1130 but no setter wrapper. Do not add a wrapper; that would be a compute edit.

### C. Gates

No gate compares an EntryHelix or EntryRamp feed with the plunge rate or a plunge cap. **No gate needs a change.**
- `sim_triage::plunge_class_finding` (sim_triage.rs:922-944) grades only `MotionClass::Plunge`. `classify_move` (kinematic_utilization.rs:125-140) puts a descent into Plunge only inside the plunge cone about the vertical (`PLUNGE_CLASS_HALF_ANGLE_DEG`). A 3-10° helix or ramp is `MotionClass::Ramp`, which is not graded (doc at :99-110, sim_triage.rs:904-910).
- The modulator's plunge cap (feed_modulation.rs:917-925) and feedopt (feedopt.rs:238) cap only `MotionClass::Plunge`. EntryRamp and EntryHelix are modulated as they are today (`should_skip_modulation` skips only EntryPlunge, feed_modulation.rs:431-447).
- `entry_load_observation` (sim_triage.rs:752+) measures the bite, not the feed.
- `plunge_stress::check` (plunge_stress.rs:29-75) compares the configured `plunge_rate` with a ball-tip cap. A ball never gets a sourced ramp.
- `drill_gates::classify_plunge_feed` (drill_gates.rs:263) applies to drill ops only, and they get no write.
- `from_static_checks::feed_checks(feed, plunge)` (from_static_checks.rs:45) reads feed and plunge only.

**One change in behaviour (a risk line, not a trip).** The chipload gate uses only samples within 5 % of the commanded feed (chipload.rs:64-73). When the ramp feed equals the cut feed, entry samples join that steady-state population for the first time. They read the same commanded advance per tooth, so the verdict does not move.

### D. `FeedsProvenance`

It has a slot for each field: `feed_rate`, `plunge_rate`, `spindle_rpm`, `stepover`, `depth_per_pass`, `scallop_height` (provenance.rs:115-126). The operation field keeps no record of who wrote it, and `set_toolpath_param` stamps Manual by field name (session/compute/params.rs:189-290). **So yes, add a slot:**
- `FeedsProvenance.ramp_feed_rate: Option<ValueProvenance>` with `#[serde(default, skip_serializing_if = "Option::is_none")]`. The project file stays compatible, because `feeds_provenance_is_empty` compares with `default()` (project_file.rs:597).
- `FeedsField::RampFeedRate`.

Stamp rules (`FeedsProvenance::stamp_ramp(&RampFeed)`, called after feed and plunge are stamped):
- `Sourced`, arm `ChipTerm` → `vendor_lut(drill.side_row)`;
- `Sourced`, arm `CutFeed` → a copy of the `feed_rate` stamp;
- `PlungeRate` → a copy of the `plunge_rate` stamp (the entry runs at the plunge rate).

### E. Every consumer of `ramp_feed_mm_min`

- feeds/mod.rs:466 (field), :2529 (clamp), :2541 (copy), :2785 (literal). Replace all four.
- feeds/provenance.rs:395 (test literal).
- diagnostics/tests.rs:654 (test literal).
- tests/cut_efficiency_is_closed_form_g_specenergy.rs:165 and tests/efficiency_abstains_without_kc_g_specenergy.rs:109 (test literals, `feed * 0.6`).
- research/feeds_and_speeds_integration_plan.md:275 is an old doc. Leave it.

No production reader exists: none in viz, CLI, MCP, benches or examples. `rg ramp_feed\b` hits only `EntrySafety.ramp_feed` (dressup), which reads the op field, not the calculator.

### F. The G6 lookup for a pocket or adaptive tool

Build it in `calculate` (feeds/mod.rs, new step 8b), which holds `input.vendor_lut`:

1. If `input.operation == OperationFamily::Drill` → `PlungeRate { DrillCycle }`. If `input.vendor_lut` is `None` → `PlungeRate { NoLut }`.
2. `let mut q = vendor_normalize::to_lookup_query_unrouted(input);` (vendor_normalize.rs:180). Then set `q.operation_family = LutOperationFamily::Drill; q.pass_role = LutPassRole::Roughing;`. Use the unrouted builder on purpose: `to_lookup_query` runs `lut_query_for` for the op kind (vendor_normalize.rs:71-104), and that would turn a ProjectCurve query into (Contour, Finish). The drill family routes to itself. Put this in a named function `vendor_normalize::ramp_drill_query(input) -> LookupQuery`, with a doc comment that explains the unrouted choice.
3. `vendor_lookup::find_best_row_for_geometry(lut, &q, &input.tool_geometry)` (vendor_lookup.rs:413).
4. Only a row with `row.drill_basis.claim() == Some(..)` and `!row.size_basis.is_refused()` is `Sourced`. The chip is `row.printed_chipload().point_mm()`, already scaled by 1/Z and by hardness (drill.rs:254-261; G6 sentry uses `chip_load_max_mm`). Copy the size claim when present. Anything else → `PlungeRate { NoDrillClaim { tool_family, diameter_mm, flutes } }`. Its text names the G6 range: flat end mill, Amana Spektra, 3.175-6.0 mm, 2 or 3 flutes.

Rows hit (amana_flat_end.json, all `amana_spektra_spiral_plunge_v24`, home (Pocket, Roughing), rpm_nominal 18 000):

| Tool | Softwood (600) | Hardwood (1450) | Plywood (BalticBirch → plywood_hardwood 1000) | MDF (1100) |
|---|---|---|---|---|
| 6 mm 2F | `amana-flat-softwood-pocket-6000-2f-spektra` 0.127 → **0.0635** | `…hardwood-pocket-6000-2f…` 0.127 → **0.0635** | `…plywood-hardwood-pocket-6000-2f…` 0.127 → **0.0635** | `…mdf-pocket-6000-2f…` 0.1524 → **0.0762** |
| 3.175 mm 2F | `…softwood-pocket-3175-2f…` 0.1016 → **0.0508** | `…hardwood-pocket-3175-2f…` 0.1016 → **0.0508** | `…plywood-hardwood-pocket-3175-2f…` 0.1016 → **0.0508** | `…mdf-pocket-3175-2f…` 0.127 → **0.0635** |

A named species (for example Walnut) takes the G2 hardness scale of the hardwood row, capped. The chip then moves with it.

**Worked example.** 6 mm 2F flat, generic softwood, shipped rpm 18 000. To keep that rpm the machine needs a cutting ceiling of at least 4572; the generic router's 4000 ceiling lowers the rpm to 15 748 (R4 Q10).
- Axial chip = 0.127 / 2 = 0.0635 mm/tooth. Vertical rate = 0.0635 × 18 000 × 2 = **2286 mm/min** (= the printed Ramp Down of 90 in/min × 25.4).
- Cut feed = 0.127 × 18 000 × 2 × tier 1.0 = **4572 mm/min**.
- Helix r 2 mm, pitch 1 mm: tan θ = 1 / (2π·2) = 0.079577, θ = 4.55°. Chip term = 2286 / 0.079577 = **28 726 mm/min**. Ramp feed = min(4572, 28 726) = **4572**; the cut feed wins.
- Ramp 3°: tan θ = 0.052408. Chip term = **43 619 mm/min**. Ramp feed = **4572**; the cut feed wins.
- Plunge rate: 1000 mm/min (softwood base, h = 1). The helix entry runs 4.57× faster than today. The old clamp would have given 1500, but nothing wrote it.
- Steep check (for the sentry): the same tool at a 30° ramp on the generic router gives rpm 15 748, vertical 2000.0, chip term 3464.1 → **3464**, below the 4000 cut feed, so the chip term wins. The drill RPM cap would have given 0.0635 × 14 000 × 2 / tan 30° = 3079. So this case also proves decision 3 (the shipped rpm, not the drill cap).

### G. Tests

No test asserts the old clamp value. Four literals set the f64 field (list E). They must change to the new `ramp` field (use `RampBasis::PlungeRate { reason: RampFallback::NoLut }` in fixtures).

New sentry: **`crates/rs_cam_core/tests/a_ramp_feed_is_the_g6_chip_over_the_entry_slope_g6ramp.rs`**, cases in step 3. Viz card-line test: **`the_ramp_line_names_the_arm_and_the_angle_g_visible`**, in `crates/rs_cam_viz/tests/every_stage_that_moves_a_number_is_on_the_card_g_visible.rs`.

---

## 2. Steps

The steps are for editors without cargo; the orchestrator builds. Steps 1-4 must land in one build, because the type changes break every literal. Step 5 (rs-cam-e2 files) can land in the same build or just after. Until it lands, MCP and CLI show `EntryUnknown` for dressup ops.

### Step 1: the ramp module and the calculator (feeds-owned)

**New file `crates/rs_cam_core/src/feeds/ramp.rs`:**
- `pub const RAMP_RULE_TEXT: &str = "ramp feed = min(cut feed, axial chip x RPM x Z / tan θ); tan θ is conservative against sin θ";`
- `pub enum RampBasis { Sourced { axial_chip_mm: f64, drill: Box<DrillClaim>, size: Option<Box<Claim>> }, PlungeRate { reason: RampFallback } }`. Derive `Debug, Clone, PartialEq`. Add `card_text() -> (String, String)`. Example headline: "ramp chip (G6 drill claim): 0.0635 mm/tooth axial". Reuse `DrillClaim::card_text` and the size claim for the detail.
- `pub enum RampFallback { DrillCycle, NoLut, NoDrillClaim { tool_family: ToolFamily, diameter_mm: f64, flutes: u32 }, SizeRefused, EntryOff, EntryUnknown, DegenerateEntry, NoShippedRpm }`. Add `text() -> String`, one sentence each, ending in "the entry uses the plunge rate".
- `pub enum EntryGeometry { Ramp { angle_deg: f64 }, Helix { radius_mm: f64, pitch_mm: f64 } }`:
  - `tan_theta() -> Option<f64>`: the ramp is `tan(angle)` for 0 < angle < 90; the helix is `pitch / (2π r)` for r > 0 and pitch ≥ 0.01. These are the guards of entry_descent.rs:434 and :689.
  - `theta_deg()` and `label()`.
- `pub enum EntrySource { Dressup, Adaptive3d }`.
- `pub fn entry_geometry(op: &OperationConfig, dressups: Option<&DressupConfig>, tool_diameter_mm: f64) -> Result<(EntryGeometry, EntrySource), RampFallback>`:
  - Adaptive3d reads `cfg.entry_style`: Plunge → `EntryOff`; Ramp → `ramp_angle_deg`; Helix → `tool_diameter_mm × helix_radius_factor`, `helix_pitch`. This mirrors finish_3d.rs:55-69.
  - Every other op reads `dressups`: `None` → `EntryUnknown`; `DressupEntryStyle::None` → `EntryOff`; Ramp → `ramp_angle`; Helix → `helix_radius`, `helix_pitch`. This mirrors dressup_apply.rs:600-650.
  - Put the decision in the doc comment: "two θ systems; a helix slope is pitch / (2π r); a ramp is tan(angle)".
- `pub enum RampArm { CutFeed, ChipTerm }`.
- `pub struct SourcedRamp { value_mm_min, arm, entry, source, tan_theta, theta_deg, axial_chip_mm, rpm: f64, flutes: u32, vertical_mm_min, chip_term_mm_min, cut_feed_mm_min, drill: Box<DrillClaim>, size: Option<Box<Claim>> }`.
- `pub enum RampFeed { Sourced(Box<SourcedRamp>), PlungeRate { reason: RampFallback, plunge_mm_min: f64 } }`. Derive `Debug, Clone, PartialEq`. Add:
  - `value() -> Option<f64>`;
  - `card_text() -> (String, String)`. Face-line example: "Ramp feed 4000 mm/min: the cut feed (chip limit 38 162 mm/min at ramp 3.00°)". The detail names the chip, rpm, Z, the vertical rate, the source (dressup or Adaptive3d), the G6 claim headline and `RAMP_RULE_TEXT`. PlungeRate example: "Ramp feed: the plunge rate, 703 mm/min: <reason>".
- `pub fn resolve_ramp_feed(basis: &RampBasis, entry: Result<(EntryGeometry, EntrySource), RampFallback>, cut_feed_mm_min: f64, rpm: Option<u32>, flutes: u32, plunge_mm_min: f64) -> RampFeed`:
  - Order: basis PlungeRate → that reason; entry Err → that reason; no rpm, or rpm 0 → `NoShippedRpm`; `tan_theta()` None → `DegenerateEntry`.
  - Then `vertical = chip × rpm × Z`, `chip_term = vertical / tan θ`, `raw = cut.min(chip_term)`, `value = round_suggestion_value_down(raw, 1.0)` (T-9), `arm = if chip_term < cut { ChipTerm } else { CutFeed }`.
- Unit tests in the file: the worked-example numbers (28 726, 43 619, 3464); every fallback; the helix slope formula; round-down never above min(cut, chip).

**`crates/rs_cam_core/src/feeds/mod.rs`:**
- `pub mod ramp;` and re-export `RampBasis, RampFeed, RampFallback, EntryGeometry`.
- `FeedsResult` (:461-466): replace `pub ramp_feed_mm_min: f64` with `pub ramp: ramp::RampBasis` and a doc comment: "the sourced half of the ramp feed; the number needs θ and the shipped feed and rpm, so `suggest::apply` resolves it".
- Step 8 (:2528-2529, :2541): delete the clamp and `ramp_feed_rate`. Update the step 9 comment (:2533-2537): "the plunge ships the material base; the ramp is sourced by G6 or falls back to the plunge".
- New step 8b: `let ramp = ramp::ramp_basis(input);`. Implement `ramp_basis` in ramp.rs as in §1F.
- Literal (:2785): `ramp,`.

**`crates/rs_cam_core/src/feeds/vendor_normalize.rs`:** add `pub fn ramp_drill_query(input: &FeedsInput) -> LookupQuery` (§1F.2).

**Test literals:** feeds/provenance.rs:395, diagnostics/tests.rs:654, tests/cut_efficiency_is_closed_form_g_specenergy.rs:165, tests/efficiency_abstains_without_kc_g_specenergy.rs:109. Replace the field with `ramp: RampBasis::PlungeRate { reason: RampFallback::NoLut }`.

### Step 2: Suggest writes the ramp (feeds-owned)

**`crates/rs_cam_core/src/feeds/suggest.rs`:**
- `SuggestContext` (:111-177): add `pub dressups: Option<&'a crate::compute::config::DressupConfig>`. The doc says: "the toolpath's dressups, for the entry θ of a dressup op. `None` means not known; the ramp falls back to the plunge rate. Adaptive3d reads its own entry."
- `SuggestWarning`: add `RampFeed { from_mm_min: Option<f64>, record: crate::feeds::RampFeed }`. The doc says: "the entry feed Suggest wrote; one per apply that wrote the speeds, on an op with the field".
- `suggest_params` (:856-867): `let default_dressups = DressupConfig::for_op(input.op_type);` then pass `context: SuggestContext { dressups: input.context.dressups.or(Some(&default_dressups)), ..input.context }`. This covers the four add doors in the §1B table that do not edit their call site (smoke.rs, mcp/commands.rs, toolpath.rs, model.rs). Each one builds `for_op(op_type)` after the call.
- `AggressivenessSkip::Drill` text (:748): "Plunge: material base, no factor; the dial does not act."

**`crates/rs_cam_core/src/feeds/suggest/apply.rs` (`apply_feeds_subset`):** add a block after the copy-back (after :229 and before the provenance stamp at :255), inside `if write_speeds`:
```rust
let from = operation.ramp_feed_rate();
let entry = crate::feeds::ramp::entry_geometry(operation, context.dressups, tool.diameter);
let record = crate::feeds::ramp::resolve_ramp_feed(
    &result.ramp, entry, operation.feed_rate(), operation.spindle_rpm(),
    tool.flute_count, operation.plunge_rate());
if operation.as_params_mut().set_ramp_feed_rate(record.value()) {
    ramp_record = Some(record.clone());
    warnings.push(SuggestWarning::RampFeed { from_mm_min: from, record });
}
```
Notes on this block:
- It reads the real `operation` after the copy-back. Those are the shipped feed and rpm (decision 3) and the shipped entry (F2).
- Decision 5: a drill op's setter returns `false`, so there is no warning.
- After `provenance.apply_suggested_subset(..)`, add: `if let Some(r) = &ramp_record { provenance.stamp_ramp(r) }`.
- `FieldApplyPreview::write_to` (:398-414): add the arm `FeedsField::RampFeedRate => { operation.as_params_mut().set_ramp_feed_rate(Some(self.value)); }`.
- `FieldApplyPreviews::get` (:436-444): `FeedsField::RampFeedRate => None` (no pill; the record carries it).

**`crates/rs_cam_core/src/feeds/provenance.rs`:**
- Add the `ramp_feed_rate` slot and `FeedsField::RampFeedRate`.
- Add arms in `get`, `slot_mut`, `detect_manual_edits` (compare `o.ramp_feed_rate() != n.ramp_feed_rate()`), and `stamped_optimizer`.
- `FeedsResult::provenance()` sets `ramp_feed_rate: None`.
- New `stamp_ramp(&mut self, r: &RampFeed)` (§1D).

**`crates/rs_cam_core/src/feeds/rationale.rs`:**
- `RationaleParam::RampFeed` and `RationaleReason::RampFeed`.
- New arm in `entry_for_warning`: `from_value: from_mm_min`, `to_value: record.value()`, headline and detail from `record.card_text()`.
- `PLUNGE_AT_MATERIAL_BASE_TEXT` (:231) becomes "Plunge: material base, no factor." No test pins the literal (rg).

**`crates/rs_cam_core/src/feeds/CLAUDE.md`:**
- Invariant line: "G6 ramp: `FeedsResult::ramp` holds the axial chip; `suggest::apply` writes `ramp_feed_rate = floor(min(shipped feed, chip × shipped rpm × Z / tan θ))`; elsewhere `None` with the reason; no clamp rule."
- Add the sentry name.
- File list: `ramp.rs`.

### Step 3: core tests

- feeds/suggest/tests.rs:1493: add `dressups: None`.
- New sentry `tests/a_ramp_feed_is_the_g6_chip_over_the_entry_slope_g6ramp.rs`. The module doc must hold the §1F table and the worked example, derived from the JSON rows. Cases:
  - (a) Pocket, 6 mm 2F, softwood, dressup Helix r2/p1, machine ceiling 6000: shipped rpm 18 000, feed 4572, record `Sourced`, arm CutFeed, `value == op.ramp_feed_rate() == Some(4572)`, θ 4.55° ±0.01, chip term 28 726 ±1, chip 0.0635 ±1e-9, side row `amana-flat-softwood-pocket-6000-2f-spektra`.
  - (b) The same with Ramp 3°: chip term 43 619 ±1.
  - (c) Generic router, softwood, dressup Ramp 30°: rpm 15 748, arm ChipTerm, value 3464. Assert that the value is not 3079 (it did not use the drill cap).
  - (d) Adaptive3d, own Helix (0.3, pitch 2), `dressups: None`: source Adaptive3d, θ 10.03°.
  - (e) The chip table: 3.175 2F wood 0.0508, MDF 6 mm 0.0762, MDF 3.175 0.0635, plywood 6 mm 0.0635.
  - (f) Fallbacks write `None` and give a reason. Entry style None → `EntryOff`. A dressup op with `dressups: None` → `EntryUnknown`. Bull 6 mm, ball 6 mm, flat 6.35 mm, flat 6 mm 4F → `NoDrillClaim`, and the text contains "G6". Acrylic → `NoDrillClaim`.
  - (g) Drill and AlignmentPinDrill: no `RampFeed` warning, and the op is unchanged.
  - (h) Scope: `apply_cut_geometry_to_op` does not change `ramp_feed_rate`; `apply_speeds_to_op` writes it.
  - (i) One number: `suggest_for_operation(..).warnings`'s record value equals the `ramp_feed_rate()` that `apply(FeedsPreview::applicable, Both, ..)` writes with the same context.
  - (j) Round-down: value ≤ min(feed, chip term), and value > min − 1.
  - (k) Provenance: ChipTerm stamps VendorLut with the side row. CutFeed stamps a copy of the feed stamp. PlungeRate stamps a copy of the plunge stamp. A later hand edit is detected as Manual.
- Update the `pill_writes_clamped_value_g_pillclamp.rs:60` match: `FeedsField::RampFeedRate => op.ramp_feed_rate()`. Check the function's return type there, and adapt if it is `f64`.

### Step 4: viz, MCP, CLI (not e2)

- **viz/ui/properties/feeds_speeds.rs:201, :227:** `dressups: Some(&entry.dressups)`.
- **viz/controller/events/mod.rs:1237:** `dressups: Some(&draft.dressups)`.
- **viz/ui/feeds/why.rs `suggest_line` (:915):** new arm `SuggestWarning::RampFeed { record, from_mm_min } => Some((line, false))`. The line is `record.card_text().0`, plus ", now {from} mm/min" or ", now the plunge rate". The line must name the arm and θ. Update the module doc: the ramp record is a face line.
- **viz/ui/feeds/why.rs `explain_plunge` (:262):** the text stays the same, but the constant now says "Plunge:" (comes from step 2).
- **viz/ui/feeds/shared.rs:304 `calculator_cut`:** add `FeedsField::RampFeedRate => {}`. `WRITTEN_FIELDS` stays at 5.
- **viz/app/mcp/generation.rs:**
  - basis (:59-75): add `"ramp": { "source": feeds.ramp.card_text(), "headline"/"detail": ..., "value_mm_min": ... }`, with the last three from the `SuggestWarning::RampFeed` record in `profile.warnings`. If there is no record (a drill), use `null`.
  - `mcp_apply_feeds` reply (:814-823): add `"ramp_feed_rate": tc.operation.ramp_feed_rate()`.
- **cli/examples/apply_suggest_save.rs:72:** `dressups: Some(&tc.dressups)`. This is needed to compile.
- **cli/src/project.rs:1076-1077:** after `apply_suggested`, find the `RampFeed` record in `profile.warnings` and call `provenance.stamp_ramp(record)`. Read `warnings` before or beside the partial move of `suggested_operation` and `feeds`.
- **viz tests, g_visible.rs:**
  - `classify_suggest`: `RampFeed` → `OnTheFace`.
  - `suggest_numbers`: push the value (0 decimals) and θ (2 decimals).
  - `records()` (:507): add `dressups: Some(&entry.dressups)`, or the config's dressups, as the card does.
  - The `feed_ceiling_rpm` fixture (Ø6 flat pocket, generic hardwood; the default dressup is Ramp 3°, registry roughing): add `RampFeed` to `must_raise`.
  - New fn `the_ramp_line_names_the_arm_and_the_angle_g_visible`. It renders that fixture and asserts that one face run contains "Ramp feed 4000", "cut feed" and "3.00°". Chip term = 0.0635 × 15 748 × 2 / tan 3° ≈ 38 162.
- **viz/ui/feeds/CLAUDE.md:** "the ramp record is one face line, from the funnel's warning".

### Step 5: rs-cam-e2 files (one separate step)

- **session/mod.rs:1621 (`cutter_op_profile`):** `dressups: Some(&tc.dressups)`. Without this, MCP `basis.ramp` and CLI project apply read `EntryUnknown`.
- **session/compute/export.rs:405:** `dressups: Some(&tc.dressups)`. At :394 add `&prov.ramp_feed_rate` to `any_from_suggest`.
- **session/multitool.rs:467-477, :867:** compute `dressup_op` before `suggest_feeds_for`, and pass `Some(&DressupConfig::for_op(dressup_op))`. Add a `dressups` parameter to `suggest_feeds_for`.
- **session/compute/params.rs:~189-290:** if `set_toolpath_param("ramp_feed_rate", ..)` reaches the op through this match or the generic registry setter, add a Manual stamp on `FeedsField::RampFeedRate`, as `feed_rate` has at :197.
- **tests/wanaka_suggest_integration.rs:1189:** add `dressups: Some(&tc.dressups)`. This keeps parity with `cutter_op_profile`, which the test checks at :1211.
- No edit in session/compute.rs:1304 (Adaptive3d only), compute/*, adaptive3d/* or dressup/entry_descent.rs.

---

## 3. Risks

1. **F2, the lost entry rewrite.** The ramp describes the entry that ships. The `StrategyRewrote` warning still claims a Plunge→Helix change that does not land. Report it and do not fix it here.
2. **The chip term rarely binds (F1).** The review may read the card's "cut feed" everywhere as a bug. The detail line states the chip limit, so the reader sees the margin.
3. **The slope can be steeper than nominal.** The dressup ramp on a rest-stock pass uses `pencil::plan_entry_ramp`, which goes up to 12° (pencil/emission.rs:70). A clip-to-floor lift can also make a later segment steeper. At 12° on a 2F tool the chip term is still about 4.7× the vertical rate / tan, far above the cut feed. The worst case needs about 26°. Note it in the detail text: "θ is the configured slope".
4. **The chipload gate population now includes entry samples** (§1C). The verdict is not expected to move. Watch `the_chipload_verdict_is_one_row_g_chipverdict` and the wanaka run.
5. **The ramp is not clamped to the feed.** Nothing re-clamps `ramp_feed_rate ≤ feed` after a later hand or optimizer feed decrease (`resolve_operation_invariants`, events/mod.rs:1048). This is a follow-up: a pass-1 sibling `clamp_ramp_to_feed` would need a new warning variant.
6. **No rpm.** An op with no rpm (the project default, when `rpm_written` is false) gets `NoShippedRpm` → `None`. This is rare.
7. **Helix radius key.** The Adaptive3d helix radius uses `tool.diameter`, while finish_3d.rs:66 uses `ctx.tool_def.diameter()`. They are the same for a flat end mill, which is the only family G6 serves.
8. **The pill preview** (pills.rs:155) passes no dressups. Its dry run records `EntryUnknown`, and nothing shows it. Keep it that way. If a ramp pill is ever added, it needs the dressups first.

## 4. Tests to run (orchestrator)

- `cargo test -p rs_cam_core -q --lib feeds::ramp`
- `cargo test -p rs_cam_core -q --lib feeds::provenance`
- `cargo test -p rs_cam_core -q --lib feeds::suggest`
- `cargo test -p rs_cam_core -q --lib feeds::rationale`
- `cargo test -p rs_cam_core -q --lib diagnostics`
- `cargo test -p rs_cam_core -q --test a_ramp_feed_is_the_g6_chip_over_the_entry_slope_g6ramp`
- `cargo test -p rs_cam_core -q --test a_flat_end_plunge_is_the_side_chip_over_z_g6`
- `cargo test -p rs_cam_core -q --test pill_writes_clamped_value_g_pillclamp`
- `cargo test -p rs_cam_core -q --test cut_efficiency_is_closed_form_g_specenergy`
- `cargo test -p rs_cam_core -q --test efficiency_abstains_without_kc_g_specenergy`
- `cargo test -p rs_cam_core -q --test suggest_feed_matches_final_geometry`
- `cargo test -p rs_cam_core -q --test a_clamped_feed_ships_at_or_below_its_ceiling_g_feeddown`
- `cargo test -p rs_cam_core -q --test restore_snapshot_invalidates_like_the_setter_n14`
- `cargo test -p rs_cam_viz -q --test every_stage_that_moves_a_number_is_on_the_card_g_visible`
- `cargo test -p rs_cam_viz -q --test the_recommended_column_is_what_apply_writes_g_recomapplied`
- `cargo test -p rs_cam_viz -q --test the_recommendation_explains_each_row_g_whyrow`
- `cargo test -p rs_cam_viz -q --test the_chipload_verdict_is_one_row_g_chipverdict`
- `cargo test -p rs_cam_viz -q --test a_claimed_row_states_its_claim_on_the_card_g_claimcard`
- `cargo build -p rs_cam_cli --examples`
- Ask the operator first (takes minutes): `cargo test -p rs_cam_core -q --test wanaka_suggest_integration`

### Critical Files for Implementation
- /home/ricky/personal_repos/rs_cam/crates/rs_cam_core/src/feeds/mod.rs
- /home/ricky/personal_repos/rs_cam/crates/rs_cam_core/src/feeds/suggest/apply.rs
- /home/ricky/personal_repos/rs_cam/crates/rs_cam_core/src/feeds/suggest.rs
- /home/ricky/personal_repos/rs_cam/crates/rs_cam_core/src/feeds/provenance.rs
- /home/ricky/personal_repos/rs_cam/crates/rs_cam_viz/src/ui/feeds/why.rs
---

## 5. Orchestrator decisions (2026-09-25)

1. Accept the plan as written, including the split (the sourced half on
   `FeedsResult::ramp`, the number in the funnel) and `dressups` on
   `SuggestContext` in place of a resolved `EntryGeometry`.
2. F1 is a result, not a defect: on the G6 cells at the shipped entry
   angles the helix and the ramp run at the cut feed. The card states the
   chip limit so that the margin is visible.
3. F2 (the Adaptive3d entry-style rewrite does not reach the op) is a
   separate defect. Record it in PROGRESS as G-ENTRYREWRITE; do not fix it
   here.
4. Risk 5 (a later feed decrease leaves the ramp above the feed) is a
   follow-up, G-RAMPCLAMP. The entry code (rs-cam-e2) or a pass-1 clamp can
   close it.
5. rs-cam-e2 approved the step 5 edits in session/mod.rs,
   session/compute/export.rs, session/multitool.rs (and params.rs by the
   same grant). Do not touch adaptive3d/{clearing,path}.rs,
   dressup/entry_descent.rs, dexel_stock or
   tests/adaptive3d_entry_stock_aware.rs until rs-cam-e2 says its entry
   fix has merged.
6. `wanaka_suggest_integration` takes seconds once built; run it without
   asking.
