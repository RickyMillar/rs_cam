# Provenance census — every number that reaches a cutting load

Read-only census, 2026-09-17. It covers every constant and coefficient on the
path from a user parameter to a newton or a kilowatt.

## How to read this

Each number carries one class:

- **MEASURED** — somebody measured it on a machine or a workpiece, and the
  measurement is retrievable.
- **LITERATURE** — a published source with a locator we can check.
- **FITTED** — a model fitted to data, with the fit stated.
- **DERIVED** — computed from other constants by stated arithmetic.
- **FOLKLORE** — plausible, no source. A round figure or a "typical" value.
- **FABRICATED** — invented to fill a gap, or a sentinel that reads as a
  measurement.

The classes appear worst first. **DERIVED sits below FOLKLORE here** because
most DERIVED entries in this engine inherit a FOLKLORE parent; a derivation is
only as good as its input.

The census cites by symbol first. Paths and line numbers are correct at
2026-09-17; the tree moved that day.

## Headline

| Class | Entries |
|---|---|
| MEASURED | **0** |
| LITERATURE | 12 |
| FITTED | 4 |
| DERIVED | 9 |
| FOLKLORE | 53 |
| FABRICATED | 3 |
| **Total** | **81** |

An entry is one table row. Some rows bundle a family that shares one
justification — the seven `RigidityProfile` fields, the seven
`base_cutting_speed_m_min` values, the three foam `Kc` integers. The count of
distinct literals is higher; the count of distinct *decisions* is this.

**Not one number on the load path was measured on a machine or on a
workpiece.** The engine models force and power for a 3-axis wood router, and
every input to that model is a transcription, a fit to somebody else's chart,
or a judgement. The tree contains no wattmeter reading, no dynamometer trace
and no dial-indicator measurement. `crates/rs_cam_core/src/material/mod.rs`
and `CREDITS.md` both say "pending bench validation" in six places. The bench
never happened.

---

# 1. The full table

## FABRICATED

| Symbol | Value | Where | Evidence for the class | What promotes it |
|---|---|---|---|---|
| `Material::kc_n_per_mm2`, foam arm | 1.0 / 2.0 / 3.0 N/mm² | `material/mod.rs:979-981` | No comment, no source, no TODO. The integers 1, 2, 3 are a placeholder sequence. The accessor's own doc says it returns `None` "for materials whose Kc has no primary measurement"; the foam arm returns `Some`, so every gate reads foam as a validated material. | One cut in each foam grade with a spindle-power log. Half a day. |
| `COLLET_EXPOSURE_MARGIN_MM` | 5.0 mm | `feeds/predict.rs:79` | The stated justification is "matching the convention in `tool_load::deflection` **test fixtures**". The evidence is the repository's own test data. The value enters the stickout, and tip deflection scales with stickout cubed. | Measure the collet-face-to-flute-start distance on the real ER collets. One hour with a caliper. |
| `MIN_TAPERED_TIP_DIAMETER_MM` | 0.5 mm | `tool_load/plunge_stress.rs:24` | The whole justification is "Matches the `(tip_radius * 2).max(0.5)` floor in `feeds/mod.rs`". The two sites cite each other and neither cites anything else. | Measure the tip flat on the tapered ball cutters in the tool library. |

## FOLKLORE

### Material

| Symbol | Value | Where | Evidence for the class | What promotes it |
|---|---|---|---|---|
| `Material::MILLING_KC_FACTOR` | 2.7 | `material/mod.rs:814` | See §2. The load-bearing step — "a mid hardwood cuts like particleboard" — has no source. The citation `planning/KC_MILLING_CALIBRATION_2026-06-17.md` is **dead** (§3). | Cut one hardwood sample at three chiploads and log spindle power. The factor then becomes FITTED. One afternoon. |
| `WoodSpecies::RadiataPine` Kc | 6.0 N/mm² | `material/mod.rs:857` | Self-declared: "NZ/AU species, not in FPL Ch.5 — folklore retained. TODO: source from CSIRO or FRI publications." | Fetch the CSIRO or Scion shear-parallel row. One hour of reading. |
| `WoodSpecies::Jarrah` Kc | 19.0 N/mm² | `material/mod.rs:875` | Self-declared folklore, TODO points at CSIRO. | Same. |
| `WoodSpecies::Ipe` Kc | 28.0 N/mm² | `material/mod.rs:878` | Self-declared folklore, TODO points at EMBRAPA / IPT. | Same. |
| `Material::kc_n_per_mm2`, plywood arm | 8.0 / 13.0 / 11.0 N/mm² | `material/mod.rs:894-896` | Self-declared: "per-grade plywood Kc has no fetched primary measurement; current values track shear-parallel shear strength of the dominant veneer rather than peripheral milling specific cutting force." Plywood also does **not** carry `MILLING_KC_FACTOR`, so plywood force sits 2.7× below solid wood of the same veneer. | Either an FPL-cited veneer derivation or one bench cut per grade. |
| `PlywoodGrade::effective_janka_lbf` | 600 / 1200 / 1000 lbf | `material/mod.rs:84-86` | Three round numbers, no comment on the values, no source. Drives `feed_scale_factor` and the LUT hardness query. | Take the dominant veneer species' published Janka. One hour. |
| `SheetGoodKind::effective_janka_lbf` | 1100 / 1300 / 750 lbf | `material/mod.rs:116-118` | The comment calls them "the substrate density proxy" and gives no source. Janka is a solid-wood test; these are invented stand-ins that then drive `CHIPLOAD_HARDNESS_EXPONENT`, which transfers vendor bands across materials. | Published Janka or Brinell for MDF, HDF and particleboard, or drop the proxy and key the LUT on density. |
| `janka_to_kc_n_per_mm2` | `janka / 100` | `material/mod.rs:544-552` | Self-declared "Folklore-grade". Honest and exemplary. See §2 for a factual error in its own justification. | A regression of measured Kc against Janka over five or more species. |
| `JANKA_CALIBRATED_BAND_LOW_LBF` / `_HIGH_LBF` | 200 / 4000 lbf | `material/mod.rs:520-521` | Round bounds chosen to bracket the existing species table, not a validity range of any regression. The doc calls it "the calibrated Janka band"; nothing was calibrated. | The band follows whatever data promotes `janka_to_kc_n_per_mm2`. |
| `feed_scale_factor` exponent | 0.4 | `material/mod.rs:719` | `reference/shapeoko_feeds_and_speeds/PARAMETER_DERIVATION.md` §"Hardness Index Mapping" states `H = (Janka/600)^0.4` and gives no derivation, no fit and no source. | Superseded in practice: `CHIPLOAD_HARDNESS_EXPONENT` already carries the FPL bracket. Reconcile the two paths instead of measuring this one. |
| `base_cutting_speed_m_min` | 200 / 180 / 170 / 250 / 100 / 300 / 120 m/min | `material/mod.rs:1010-1030` | Seven round numbers. Two arms self-declare "conservative placeholder". **First-order for power**: this sets RPM when no vendor row matches, and the edge power term is linear in RPM. | Vendor SFM tables per material class. One afternoon of chart reading. |
| `plunge_rate_base` baselines | 1000 / 900 / 1500 / 250 / 2000 / 200 / 800 mm/min at Ø6 | `material/mod.rs:1044-1056` | Round numbers, no source. The 6 mm reference is stated as "preserved exactly to keep existing recommendations stable" — the value's justification is that it is the value. | Vendor plunge tables, or a plunge test at three diameters. |
| `feed_scale_factor`, plastic / foam / fiberglass arms | 0.5 / 0.15 / 0.25 / 0.40 / 1.3 | `material/mod.rs:734-746` | The fiberglass arm self-declares "Placeholder pending bench validation". The others carry no comment. | Bench cut per family. |

### The force model

| Symbol | Value | Where | Evidence for the class | What promotes it |
|---|---|---|---|---|
| Linear `Kc` scaling of `Ks` and `F_edge` | `scale = Kc / 35.1` | `feeds/force.rs:119` | This is a modelling **choice**, not a constant, and it carries no source. It asserts that the affine slope and the edge intercept scale by the same ratio across every material from foam to 7075 aluminium. The woodresearch.sk study measured one wood; nothing shows the two coefficients track `Kc` together, or track it linearly. | Fit `Ks` and `F_edge` separately on two woods at opposite ends of the Janka range. One day. |
| `GRAIN_ANISOTROPY_FACTOR` | 2.0 | `tool_load/power.rs:95` | Cited to Pałubicki 2021, DOI 10.3390/ma14092208. See §2 — the comment's confidence claim does not survive the repository's own record. It multiplies **every** power number by two. | Read the DOI and record the measured spread verbatim, or run the same cut along and across grain and log power. |
| `RUBBING_FLOOR_MM_TOOTH` | 0.025 mm/tooth | `feeds/mod.rs:930` | Three sources named with no locator: "Onsrud min-chip-thickness rule, GWizard 'minimum chipload', FPL Wood Handbook chip-formation regime". No URL, no page, no retrieval date — unlike every other primary source in this repository. The number is round. The W6 audit of 2026-08-04 checked two comparable Onsrud claims and found neither survived. | Retrieve the Onsrud rule with a page locator, or run a burn test on three species. |

### Tools and deflection

| Symbol | Value | Where | Evidence for the class | What promotes it |
|---|---|---|---|---|
| `ENDMILL_CORE_FRACTION` | 0.7 | `feeds/predict.rs:71` | Cited to "Machinery's Handbook stiffness-correction notes" and the "FSWizard effective root diameter" with no edition, page or URL. **First-order**: the second moment of area goes as the fourth power of the diameter, so 0.7 against 0.8 changes predicted deflection by 1.7×. The comment itself records that the predictor carries a "+36 % safe-side bias" against the post-simulation integrator, which is evidence the value is wrong and not evidence of its size. | Pull a known transverse load on a real end mill and read the tip with a dial indicator. One afternoon; this is the highest-value measurement in the census. |
| `PREDICTOR_TAPERED_BALL_SHANK_WEIGHT` / `_TIP_WEIGHT` | 0.6 / 0.4 | `feeds/predict.rs:110-111` | A 60/40 split justified by "the bending stiffness integral … is dominated by the larger cone-shoulder section". The direction is argued; the split is not. | Integrate the real taper profile, which the tool model already carries. This is arithmetic, not a measurement. |
| `PLUNGE_CAP_PER_MM_TIP_DIAMETER` | 150 mm/min per mm | `tool_load/plunge_stress.rs:20` | The comment says "Calibrated from FSWizard / GWizard published plunge ranges (100–300 mm/min for sub-2 mm tools)". Picking a number inside a quoted range is not a calibration. No locator for either source. Its own citation `planning/P2_PLUNGE_STRESS_GATE_RCA.md` is **dead** (§3). | Retrieve the two ranges with locators, or plunge-test three ball cutters to failure. |
| `FLUTE_GUARD_FACTOR` | 0.8 | `feeds/mod.rs:1679` | Bare `const` inside a function body. No doc comment at all. Caps the axial DOC, hence the force. | Vendor flute-length guidance. |
| `LD_SEVERE_THRESHOLD` / `_MODERATE_THRESHOLD` | 6.0 / 4.0 | `feeds/mod.rs:1798-1799` | Bare consts, no doc beyond "long tools deflect more". | The deflection model already predicts this continuously. The step function is redundant, not unmeasured. |
| `LD_SEVERE_FACTOR` / `_MODERATE_FACTOR` | 0.75 / 0.88 | `feeds/mod.rs:1800-1801` | Bare consts. 0.88 carries two significant figures it cannot support. | Same as above. |
| `WORKHOLDING_LOW_FACTOR` / `_HIGH_FACTOR` | 0.85 / 1.03 | `feeds/mod.rs:1816-1817` | Bare consts, no doc. A 3 % feed **increase** for rigid workholding is precision theatre. | Noise. See §4. |

### Machines

| Symbol | Value | Where | Evidence for the class | What promotes it |
|---|---|---|---|---|
| `MachineProfile::safety_factor` | 0.75 / 0.80 / 0.80 | `machine/mod.rs:181, 210, 232` | No doc comment anywhere on the field or the values. It multiplies the available power directly (`power.rs:439`), so it sets the line every power verdict is measured against. The only test asserts it lies in `[0.5, 1.0]`. | A spindle duty-rating datasheet, or a thermal soak test. |
| `PowerModel::ConstantPower` for the generic router | 0.8 kW | `machine/mod.rs:167` (generic) | Round, no source. | Nameplate. |
| `PowerModel::ConstantPower` for the Makita RT0701C | 0.71 kW | `machine/mod.rs:222` | No source. It is not a nameplate figure for that router. The two-decimal form implies a derivation nobody wrote down. | The Makita datasheet, and a statement of whether the figure is input or output power. |
| `RigidityProfile::default` and the generic-router override, all seven fields | `doc_roughing 0.25/0.20`, `doc_finishing 0.10/0.08`, `woc_roughing 0.80/0.70`, `woc_roughing_max 6.35/5.0`, `woc_finishing 0.635/0.50`, `adaptive_doc 2.0/1.5`, `adaptive_woc 0.25/0.20` | `machine/mod.rs:65-77, 172-180` | No doc comment on the struct's values or on either preset. **The struct has seven fields, not four.** `woc_roughing_max_mm = 6.35` and `woc_finishing_mm = 0.635` are 1/4 inch and 0.025 inch — the profile is an imperial rule of thumb written in millimetres. | These set the DOC and WOC the load model is evaluated at. A rigidity measurement (push the gantry, read the deflection) promotes all seven at once. |
| `SpindleConfig` variable ranges | 8000–24000, 6000–24000 rev/min | `machine/mod.rs:164-165, 194-195` | Round, no source. | Nameplate. |
| `max_feed_mm_min` | 4000 / 5000 / 5000 mm/min | `machine/mod.rs:169, 206, 230` | Round, no source. | Controller `$110`–`$112` readout. Minutes. |
| `DEFAULT_CUTTING_FEED_CAP_MM_MIN` | 6000 mm/min | `machine/mod.rs:129` | The comment names "Onsrud industrial charts" with no locator and then rounds. | A locator for the Onsrud ceiling. |
| `MachineKinematics::generic_wood_router` acceleration | 200 mm/s² | `machine/mod.rs:146` (accessor doc) | Called "a conservative wood-router default" with no source. | Controller `$120`–`$122` readout. Minutes. |
| `PLUNGE_CLASS_HALF_ANGLE_DEG` / `LATERAL_MAX_DESCENT_ANGLE_DEG` | 15.0 / 2.0 deg | `machine/kinematic_utilization.rs:84, 92` | Round classifier thresholds, no source. They decide which samples count as plunging. | These are definitions, not measurements. Promotion is not available; they need a stated convention instead. |

### Chipload, the LUT and the solvers

| Symbol | Value | Where | Evidence for the class | What promotes it |
|---|---|---|---|---|
| `ChipLoadFormula::q` | 1.26 | `machine/mod.rs:48` | `reference/shapeoko_feeds_and_speeds/PARAMETER_DERIVATION.md:50` states, verbatim, "q = 1.26 (hardness exponent, **extrapolated**)". `k0` and `p` on the same line were fitted; `q` was not. | The LUT path already carries the FITTED `CHIPLOAD_HARDNESS_EXPONENT`. Composing `q = 1.26` with the 0.4 feed-scale exponent gives an effective −0.504, which the B-lit verdict reconciled with 0.5. Reconciling the two paths retires this constant. |
| `DRILL_CHIPLOAD_MULTIPLIER` | 2.5 | `feeds/mod.rs:1313` | Self-declared "REPO-AUTHORED and UNSOURCED", with its former justification retracted line by line. The **best comment in the census** — it states what was claimed, what is true, and why the value is held anyway. | The comment names the blocker: the one primary wood-drill chart (Onsrud 72-000) is footnoted for a gang drill, not a router. A drill test on a router promotes it. |
| `MAX_SPINDLE_SPEEDUP` | 1.5 | `feeds/mod.rs:357` | "conservative", "matches the rough magnitude of the chart's own spread". No measurement of that spread is recorded. | Measure the published `rpm_max / rpm_nominal` ratio across the 252 bundled rows. This is arithmetic on data already in the tree. |
| `SPINDLE_CEILING_HEADROOM` | 0.95 | `feeds/mod.rs:362` | Round, no source. | Noise. See §4. |
| `MIN_AP_MM` / `MIN_AE_MM` | 0.05 / 0.02 mm | `feeds/mod.rs:1694-1695` | Bare consts, no doc. | Noise. |
| `SLOTTING_THRESHOLD` / `SLOTTING_DOC_CAP` | 0.85 / 0.25 | `feeds/mod.rs:1702-1703` | Bare consts, no doc. The 0.25 cap is a real DOC limiter, so it moves the force. | Vendor slotting guidance, which several bundled charts publish. |
| `POWER_LADDER_AP_FLOOR_MM` / `_AE_FLOOR_MM` | 0.5 / 0.5 mm | `feeds/mod.rs:1157, 1161` | Solver floors, honestly documented as guards that raise a `PowerLimited` warning when they bind. | They bound a search, not a load. Not worth measuring. |
| `DEFLECTION_BACKOFF_FACTOR` / `_MAX_ITERATIONS` / `_DPP_FLOOR_MM` | 0.8 / 5 / 0.5 mm | `feeds/suggest/invariants.rs:65, 68, 58` | Solver knobs, justified by one motivating case ("Wanaka Back Rough settles inside 3 iterations"). | Not physical. They are tuned against a fixture and should be described that way. |
| `STEPOVER_BACKOFF_FACTOR` / `_TARGET_MOVES` / `_DIAMETER_FRACTION` | 1.5 / 500 000 / 0.5 | `feeds/suggest/invariants.rs:97, 82, 90` | Same: one motivating case, round targets. | Not physical. |
| `DEFAULT_SCALLOP_TARGET_UM` | 25.0 | `feeds/cutter_constraints.rs:74` | "Matches the operator's 'standard finish' expectation." That is a preference, correctly labelled, not a measurement. | Nothing to measure. It is a default. |
| `STEADY_STATE_FEED_FRACTION` | 0.95 | `tool_load/chipload.rs:206` | "loose enough … tight enough", with typical ramp and plunge fractions quoted and not sourced. It decides which samples every gate sees, and the folder instructions warn that "a gate with no population proved nothing". | Measure the feed distribution across the shipped fixtures. The data is already in the traces. |
| `BIPOLAR_SIDE_FRACTION` | 0.05 | `tool_load/chipload.rs:228` | Same form of justification. | Same. |
| `PLUNGE_ENTRY_UNSTABLE_DPP_OVER_D` | 0.5 | `feeds/suggest/adaptive_entry.rs:26` | Round threshold, no source. | Ramp-entry test. |
| `VBIT_ANGLE_TOLERANCE_DEG` / `VBIT_ANGLE_MATCH_BONUS` | 20.0 / 200.0 | `feeds/vendor_lookup.rs:141, 146` | Scoring weights, no source. They decide which vendor row a V-bit gets, and therefore its chipload band. | Not physical; they need a stated matching policy, not a measurement. |
| `SCALE_CLAMP_LO` / `_HI` | 0.1 / 10.0 | `feeds/vendor_lookup.rs:319-320` | "0.1× / 10× covers a 100× diameter span which is well past any real wood-tool extrapolation." Honest guard. | Noise. |
| `MIN_CHIPLOAD_RANGE_FRACTION` | 0.05 | `feeds/vendor_lut.rs:402` | Round ingest guard, no source. Its reasoning — that a narrower range is fabricated precision — is sound. | Noise. |
| `PREDICTOR_NON_ADAPTIVE_WOC_FRACTION` | 0.35 | `feeds/predict.rs:85` | Round fallback. | Only sizes a move-count guard. Noise. |
| `PREDICTOR_ADAPTIVE_WOC_FRACTION` | 0.3 | `feeds/predict.rs:92` | Round fallback. | Noise. |
| `PREDICTOR_ADAPTIVE3D_DPP_FRACTION` | 0.5 | `feeds/predict.rs:104` | "Conservative middle of the rigidity envelope." | Noise. |
| `PREDICTOR_RASTER_SAMPLE_DENSITY` | 0.1 | `feeds/predict.rs:99` | "Upper-bound-optimistic by design." Honest. | Noise. |
| `PREDICTOR_FALLBACK_RPM` and `FALLBACK_RPM` | 18 000 rev/min | `feeds/predict.rs:120`, `feeds/mod.rs:1234` | Two copies of the same round fallback in two modules. It enters the cutting velocity, so it enters the edge power term. | A default is a policy, not a measurement. It should be one constant, not two. |
| `depth_tier_multiplier`, the ratio > 3 rung | 0.45 | `feeds/geometry.rs:267` | The vendor derate table that the sibling `doc_derating_scale` quotes verbatim stops at 3×D. The 0.45 rung extrapolates past every source. See §2 for the larger problem with this function. | The vendor tables do not go further. The honest move is a refusal, not a number. |

## DERIVED

| Symbol | Value | Where | The arithmetic | Note |
|---|---|---|---|---|
| `LIT_ANCHOR_KC_N_PER_MM2` | 35.1 N/mm² | `feeds/force.rs:77` | `MILLING_KC_FACTOR 2.7 × GenericHardwood FPL shear 13.0 = 35.1` exactly. | **The derivation is prose only.** The literal lives in a different module from both parents, and the string `35.1` is also hard-coded in three test files. No test ties it to `2.7 × 13.0`. A change to either parent silently rescales every wood force and power number. See §2. |
| `Material::kc_n_per_mm2`, aluminium arm | ≈ 1422.6 N/mm² | `material/mod.rs:973-977` | `KC11 800 × h^(-mc)` at `h = 0.1 mm`, `mc = 0.25`. | The Kienzle pair is LITERATURE (VDI 3323 group 22, cross-checked against Sandvik). The representative chip thickness `h = 0.1 mm` is a FOLKLORE choice, and the result is fully sensitive to it. The comment states `≈ 1422.8`; the arithmetic gives 1422.6. Harmless, but it shows nobody re-ran it. |
| `SheetGoodKind::Hdf` Kc | 36.8 N/mm² | `material/mod.rs:915` | `MDF 31.4 × (880/750) ≈ 36.8`. | Honestly labelled, with a TODO for a real HDF measurement. The density figures carry no source. |
| `DEFAULT_ROUGH_DEFLECTION_LIMIT_UM` | 200.0 µm | `feeds/cutter_constraints.rs:65` | `EXCEEDS_BOUND_MM × 1000`. | Correctly derived from the gate bound so the pre-simulation and post-simulation answers agree. The **bound itself** is FOLKLORE — see below. |
| `DEFAULT_FINISH_DEFLECTION_LIMIT_UM` | 50.0 µm | `feeds/cutter_constraints.rs:69` | `WITHIN_BOUND_MM × 1000`. | Same. |
| `DEFLECTION_BACKOFF_TARGET_UM` | 200.0 µm | `feeds/suggest/invariants.rs:50` | Same bound again. | Same. |
| `CHIPLOAD_EXTRAPOLATION_LN_THRESHOLD` | `ln(1.4)` | `feeds/vendor_lookup.rs:327` | `ln` of a chosen ±40 %. | The ±40 % is chosen to sit between two named LUT row spacings. Stated clearly. |
| `MAX_CHIPLOAD_DIAMETER_FRACTION` | 0.12 | `feeds/vendor_lut.rs:410` | The hottest legitimate vendor row sits at 0.104×D; 0.12 clears it. | A guard derived from a LITERATURE maximum. Well reasoned, and it names the row. |
| `MachineProfile::max_shank_mm` | 6.35 / 7.0 / 6.35 mm | `machine/mod.rs:171, 208, 231` | 6.35 mm is 1/4 inch; 7.0 mm is the ER11 nominal maximum. | Derived from collet sizes, which are facts. Undocumented in the code. |

## FITTED

| Symbol | Value | Where | The fit | Note |
|---|---|---|---|---|
| `CHIPLOAD_DIAMETER_EXPONENT` | 0.61 | `feeds/vendor_lookup.rs:367` | Regression over all 252 bundled LUT observations, grouped by source × subfamily × material × flutes × role. Per-family exponents span 0.23–1.25; row-weighted centre 0.52; cross-family median 0.40. Adopted 0.61 to match `ChipLoadFormula::p` so one implementation moves. Magnitudes measured over 11 712 query/row pairs. | **The best-evidenced number in the engine.** Its doc even forbids over-citation: "a fitted summary of seven vendors who disagree, not a physical constant, and must never be cited as one." |
| `CHIPLOAD_HARDNESS_EXPONENT` | 0.5 | `feeds/vendor_lookup.rs:403` | Bracketed `[0.48, 0.67]` by FPL GTR-190 Table 5-11a; the conservative edge taken. The doc then records that the vendors' own charts imply ≈ 0.08, and states plainly that 0.5 is deliberately more conservative than the in-domain evidence. | Second best. The honest-uncertainty paragraph is the model the rest of the census should follow. |
| `ChipLoadFormula::k0` | 0.024 | `machine/mod.rs:46` | Power law fitted to the midpoints of the Shapeoko community chip-load table, soft-wood column, three diameters (`reference/shapeoko_feeds_and_speeds/PARAMETER_DERIVATION.md` §"Step 1"). | The fit is stated but weak: three points, no r², and the input is a community guideline table, not a measurement. The in-code comment "derived from empirical data" overclaims — see §2. |
| `ChipLoadFormula::p` | 0.61 | `machine/mod.rs:46` | Same fit, same three points. | Numerically equal to `CHIPLOAD_DIAMETER_EXPONENT`, which was later chosen to match it. The agreement is deliberate, not independent corroboration. |

## LITERATURE

| Symbol | Value | Where | The citation | Traceable? |
|---|---|---|---|---|
| `doc_derating_scale` table | 1.0 / 0.75 / 0.5 at 1×D / 2×D / 3×D | `feeds/geometry.rs:143-153` | Verbatim from the `ap_rule` field of `amana_compression.json` and `onsrud_plastic.json`: "1×D use recommended chip load; 2×D reduce 25%; 3×D reduce 50%." | **Yes.** The quote is in the bundled data, the data ships, and `feeds/mod.rs:1788` records that three vendors print the same table. The strongest entry in the census. |
| `LIT_KS_N_PER_MM2` | 49.95 N/mm² | `feeds/force.rs:62` | woodresearch.sk 2019, vol. 64 no. 5, art. 12 — `Fc1z = 49.95·h + 5.30`, R² ≈ 0.99. | **Partly.** The fit is reported consistently in three places and the numbers are internally consistent (`49.95/5.30 = 9.42`, crossover 0.106 mm). But the locator has no title, no authors, no DOI, no URL and no retrieval date, unlike every other primary source in `CREDITS.md`. The species is never named. See §2. |
| `LIT_FEDGE_N_PER_MM` | 5.30 N/mm | `feeds/force.rs:66` | Same fit. | Same. |
| `WoodSpecies` Kc, FPL rows | 6.5 / 10.4 / 13.0 / 16.0 / 9.5 / 13.0 / 13.8 N/mm² | `material/mod.rs:854-872` | FPL-GTR-190 (2010) Ch. 5 Table 5-3a, shear parallel to grain at 12 % MC, with a verbatim quote per species in the comment. | **Yes** for the seven cited species. The chain ends at `source_manifest.json::fpl_ch5_2010`. Note the base rows are shear strength, not milling Kc — that gap is what `MILLING_KC_FACTOR` papers over. |
| `WoodSpecies::janka_lbf` | 600 / 710 / 870 / 1450 / 1450 / 1010 / 1260 / 1360 / 1910 / 3510 lbf | `material/mod.rs:29-49` | Wood Database, with verbatim quotes in `planning/data_ingest_2026-05-29/hardness.md` (**alive**). | **Yes** for Radiata and Longleaf, which carry correction notes. The other eight carry no per-value comment. `GenericHardwood` and `HardMaple` are both 1450 — Generic appears to be a copy of Hard Maple, undocumented. |
| `SheetGoodKind::Mdf` Kc | 31.4 N/mm² | `material/mod.rs:910` | PMC6315737, round-shape Ks for MDF: average 31.44, SD 2.68, range 25.81–35.58. | **Yes.** Value, spread and range all quoted. |
| `SheetGoodKind::Particleboard` Kc | 35.0 N/mm² | `material/mod.rs:920` | Pałubicki 2021, DOI 10.3390/ma14092208 — average of slow (32.0) and fast (37.6) peripheral up-milling principal force. | **Yes**, and the averaging step is stated. |
| `PlasticFamily::Hdpe` Kc | 40.0 N/mm² | `material/mod.rs:930` | Yang 2022, DOI 10.3390/polym14010189 — cutting-yield-stress midpoint of 33.85–46.89. | **Yes**, though "midpoint" gives 40.37 and the code rounds to 40.0. |
| Aluminium Kienzle pair | `kc1.1 = 800`, `mc = 0.25` | `material/mod.rs:973-974` | VDI 3323 group 22 via the Machining Doctor chart (Wayback 2024-08-13 snapshot), cross-checked against Sandvik's 350–700 N/mm² aluminium bound. | **Yes**, with a snapshot date and a cross-check. Exemplary. |
| `CARBIDE_YOUNGS_MODULUS_N_PER_MM2` | 600 000 N/mm² | `compute/tool_config.rs:105` | "Cited range 550–650 GPa across grades; 600 is the canonical handbook value." | **Partly.** The range is right and uncontroversial, but no handbook is named. The tip deflection is exactly inversely proportional to this. |
| `HSS_YOUNGS_MODULUS_N_PER_MM2` | 200 000 N/mm² | `compute/tool_config.rs:108` | "Handbook value is 200–210 GPa for M2/M42 grades; 200 chosen as a clean reference." | Same. Honest about the rounding. |
| `SpindleConfig::Discrete` for the Makita RT0701C | 10000 / 12000 / 17000 / 22000 / 27000 / 30000 rev/min | `machine/mod.rs:220` | None in code. The list matches the RT0701C dial table. | **Untraceable as written.** The values are almost certainly the manufacturer's, but the code cites nothing, so a reader cannot tell them from invention. |

## MEASURED

Empty.

---

# 2. Constants whose comments claim more than their provenance supports

Ordered by how much the claim is worth.

## 2.1 The worst: "Calibration — literature-absolute" in `feeds/force.rs`

`feeds/force.rs:38` heads its calibration section **"literature-absolute"** and
states that "both the *shape* … and the *magnitude* come straight from the
woodresearch.sk quasi-orthogonal fit". "Literature-absolute" is the strongest
confidence word anywhere in the load model. It sits on the weakest chain in
the census.

The magnitude is not absolute. It is the published fit attached to an anchor,
and the anchor is:

1. `LIT_ANCHOR_KC_N_PER_MM2 = 35.1` (`force.rs:77`), which the comment itself
   derives as `MILLING_KC_FACTOR 2.7 × FPL shear 13.0`.
2. `MILLING_KC_FACTOR = 2.7` (`material/mod.rs:814`), whose stated reason is
   that "2.7 lands GenericHardwood at ~35 N/mm² (particleboard parity)".

So the absolute magnitude of every wood force and every wood power number in
the engine rests on the premise **that a mid hardwood cuts like
particleboard**. No source in this repository states that. The premise is the
load-bearing step, and it is unsourced.

Three further defects in the same chain:

- `force.rs:70` asserts the anchor is "a mid-hardwood close to the study's
  species class". The study's species is never named, in the code, in
  `CREDITS.md`, or in `planning/UNIFIED_LOAD_MODEL_2026-06-18.md`. The claim
  cannot be checked from anything in the tree.
- The citation for `MILLING_KC_FACTOR`,
  `planning/KC_MILLING_CALIBRATION_2026-06-17.md`, is **dead** (§3). It is
  cited twice — at `material/mod.rs:813` and `material/mod.rs:848`.
- `CREDITS.md:509-513` still says the opposite of the code. It records that
  the per-species values "stay in the FPL shear-parallel regime (6–28 N/mm²)"
  and that lifting them by a 3–5× size-effect factor "requires a coordinated
  anisotropy retune … and bench validation, both Phase 6+ scope". The code
  applies the lift. The retune did not happen (`GRAIN_ANISOTROPY_FACTOR` moved
  in the *earlier* Phase 2B change, for sheet goods). The bench validation did
  not happen. The project's own credits file says this factor should not be
  where it is.

Finally, the derivation `35.1 = 2.7 × 13.0` exists **only in prose**. The
literal sits in `feeds/force.rs`, its parents sit in `material/mod.rs`, the
string `35.1` is hard-coded again in three test files, and no test asserts the
relation. `material/mod.rs` does pin `GenericHardwood`'s Kc to
`MILLING_KC_FACTOR × 13.0`; nothing pins the anchor to the same product.
Changing `MILLING_KC_FACTOR` alone rescales every wood force and power number
and no sentry notices.

## 2.2 `GRAIN_ANISOTROPY_FACTOR = 2.0` — a knob that says it is not a knob

`tool_load/power.rs:86-95` states the value is "the **measured** directional
spread of specific cutting force for wood-class materials per Pałubicki 2021
(DOI 10.3390/ma14092208)", and that the rename from `ANISOTROPY_MULTIPLIER`
"reflects that this is a documented physical factor, **not a knob to tune
around under-modeled Kc**".

The repository's own record contradicts the second sentence. The same comment,
four lines earlier, says Phase 2B "paired the rename + value change with
literature-anchored sheet-good Kc so the product `Kc × factor` reflects
physics rather than the old absorption split." A value that moved 2.5 → 2.0 in
the same change that raised sheet-good Kc, in order to hold the product `Kc ×
factor` steady, was tuned. That is the definition the comment denies.

On the citation: the repository quotes the Pałubicki paper twice, and both
quotes are about magnitude, not spread — "slow (32.0) and fast (37.6)
peripheral up-milling principal cutting force", a ratio of 1.17. Nothing in
the tree records a measured directional spread of 2.0. The DOI is live and
checkable, so this is fixable by reading; but as it stands the number is
asserted *near* a real citation, not traceable *to* it.

The factor multiplies **every** power number by two, and it is applied to
power but not to deflection. That split is separately documented and
defensible. The value is not.

## 2.3 `RUBBING_FLOOR_MM_TOOTH = 0.025` — three named sources, no locator

`feeds/mod.rs:916-920` calls 0.025 "the canonical wood-router floor (Onsrud
min-chip-thickness rule, GWizard 'minimum chipload', FPL Wood Handbook
chip-formation regime)". Three sources, no URL, no page, no retrieval date —
in a repository where every other primary source carries all three.

This matters because the W6 audit of 2026-08-04 checked two structurally
identical claims and neither survived. That audit found the Onsrud drill chart
"contains chip load per tooth by cutting diameter and nothing else" and that
"the FPL Wood Handbook has no drilling chapter". The same audit method has not
been applied to this constant. Until it is, "canonical" is an assertion.

## 2.4 `ENDMILL_CORE_FRACTION = 0.7` — a handbook with no edition

`feeds/predict.rs:64-70` calls 0.7 "the canonical handbook value for 2- to
3-flute end mills (Machinery's Handbook stiffness-correction notes; matches
the FSWizard 'effective root diameter' recommendation)". Neither source has an
edition, a page or a URL.

The comment then does something unusual and useful: it records that the
predictor carries a "+36 % safe-side bias" against the post-simulation
integrator. That bias is evidence the value is wrong. The engine has since
built a back-off loop that *depends* on the bias
(`feeds/suggest/invariants.rs:40-50` warns that fixing `ENDMILL_CORE_FRACTION`
would require re-evaluating `DEFLECTION_BACKOFF_TARGET_UM`). A known error is
now load-bearing.

## 2.5 `ChipLoadFormula` — "derived from empirical data"

`machine/mod.rs:33` says the parameters are "derived from empirical data". The
reference document they came from calls the input "Shapeoko community
guidelines" and fits three points per column. `q = 1.26` is marked
"extrapolated" in that same document and was not fitted at all. "Empirical
data" in a CAM engine implies a measurement. This was a community table.

## 2.6 `doc_derating_scale` and `depth_tier_multiplier` — one vendor rule, two implementations

Both functions claim the same vendor derate table. They disagree.

- `feeds/geometry.rs:143` is **piecewise linear**: at 1.5×D it returns 0.875.
- `feeds/geometry.rs:261` is a **step function**: at 1.5×D it returns 0.75.

Both are live. `doc_derating_scale` derates the chipload band; `depth_tier_
multiplier` derates the feed. The first carries a verbatim vendor quote and a
long note on why a single canonical implementation prevents drift. The second
says only "From reference calcs.rs" and adds a 0.45 rung past 3×D that no
vendor publishes. The comment on the first implies the drift problem is
solved. It is solved for one of the two functions.

## 2.7 `janka_to_kc_n_per_mm2` — the stated exception list is wrong

`material/mod.rs:528-531` says the per-species table "fits within ±25 % of
`janka/100` **except for the `GenericHardwood` and `Walnut` hand-tuned
anchors**". Running the arithmetic on the ten shipped species:

| Species | `janka/100` | Table Kc | Deviation |
|---|---|---|---|
| Walnut | 10.1 | 9.5 | **5.9 %** |
| WhiteOak | 13.6 | 13.8 | 1.5 % |
| Jarrah | 19.1 | 19.0 | 0.5 % |
| Birch | 12.6 | 13.0 | 3.2 % |
| GenericSoftwood | 6.0 | 6.5 | 8.3 % |
| HardMaple | 14.5 | 16.0 | 10.3 % |
| GenericHardwood | 14.5 | 13.0 | **10.3 %** |
| RadiataPine | 7.1 | 6.0 | 15.5 % |
| LongleafPine | 8.7 | 10.4 | 19.5 % |
| Ipe | 35.1 | 28.0 | 20.2 % |

All ten fit within ±25 %. The two named exceptions are among the closest fits;
the worst three are not named. The claim is not merely imprecise, it is
inverted. It is a small defect, but it shows the sentence was written and
never checked, in a comment whose whole purpose is to rate its own confidence.

## 2.8 `PLUNGE_CAP_PER_MM_TIP_DIAMETER` — "calibrated"

`tool_load/plunge_stress.rs:17-20` says 150 is "Calibrated from FSWizard /
GWizard published plunge ranges … (100–300 mm/min for sub-2 mm tools)".
Choosing a value inside a quoted range is a choice, not a calibration. Neither
source carries a locator, and the module's own planning citation is dead.

## 2.9 `JANKA_CALIBRATED_BAND_LOW_LBF` / `_HIGH_LBF` — nothing was calibrated

`material/mod.rs:512-521` calls 200–4000 lbf "the calibrated Janka band" and
says that "outside this band the `Kc` regression has no citation-backed
validity". There is no regression, and the band's own justification says only
that the bounds sit outside the species table. The word "calibrated" implies a
procedure that did not occur.

## 2.10 `Material::kc_n_per_mm2` — the type-level honesty claim is not kept

The accessor's doc says `None` is returned "for materials whose Kc has no
primary measurement … This keeps the type system honest about which materials
carry a validated cutting-force model." Two arms break it:

- **Foam** returns `Some(1.0 / 2.0 / 3.0)` with no source and no comment.
- **Plywood** returns `Some(8.0 / 13.0 / 11.0)` and the comment on the arm
  itself says the values have "no fetched primary measurement".

The refuse-first policy is real and well kept for plastics and fiberglass. It
is claimed at the type level and broken for two material families, which is
worse than not claiming it, because a caller cannot tell which is which.

---

# 3. Dead citations

51 % of the `planning/…` paths cited from the load-model code no longer resolve
in the working tree. Of the 53 distinct paths cited from `feeds/`,
`tool_load/`, `material/`, `machine/` and `tool/`, **27 are dead and 26 are
alive**.

One qualification, which cuts both ways. `planning/CLAUDE.md` and
`planning/DELETED_INDEX.md` record that the purge of 2026-09-17 deleted 1010
files and that every one of them except four large blobs stays retrievable at
the annotated tag `planning-pre-purge-2026-09-17`. The citations were
deliberately not rewritten; the tag is the retrieval path. So a dead path here
is *recoverable*, not destroyed.

It is still an absent citation for the purpose of this census, for two
reasons. A reader following a doc comment sees a missing file, not a tag. And
this census could not verify any retrieved content, because the task forbids
running git — so nothing below states what a dead document actually said.

## Dead, and load-bearing

| Dead path | Cited by | What it is asked to support |
|---|---|---|
| `planning/KC_MILLING_CALIBRATION_2026-06-17.md` | `material/mod.rs:813` and `material/mod.rs:848` | **`MILLING_KC_FACTOR = 2.7`** — the single largest multiplier on every solid-wood force and power number. See §2.1. |
| `planning/cutter_axial_constraints_2026-06-06.md` | `feeds/cutter_constraints.rs` module doc §7 | The "do NOT use" ruling that explains why the axial envelope does not reuse the Suggest predictor. The reasoning is restated in the comment, so the loss is recoverable. |
| `planning/P2_PLUNGE_STRESS_GATE_RCA.md` | `tool_load/plunge_stress.rs:11` | The whole plunge-stress gate, including `PLUNGE_CAP_PER_MM_TIP_DIAMETER = 150`. |
| `planning/review_2026-08-04/FEEDS_CENSUS.md` | `feeds/mod.rs` (the "census F-3, T1.4" shadow-point note) | The census finding that names the `effective_d` shadow point. The note is self-contained; the identifier `F-3` is not. |
| `planning/tool_kinematics_chipload_audit_2026-05-31.md` | `feeds/` | A chipload audit. |
| `planning/review_2026-08-08/LUT_BOUNDARY_EVIDENCE.md` | `feeds/vendor_lut.rs` | The LUT boundary rules, including the degenerate-range row list. |
| `planning/review_2026-08-08/OPTIMIZER_ASSUMPTIONS.md` | `tool_load/optimize/` | The optimiser's stated assumptions. |
| `planning/review_2026-07-29/TOOL_SCALE_SEMANTICS.md` | `tool/` | Tool-scale semantics. |
| `planning/STRATEGY_ADVISOR_2026-06-17.md` | `machine/mod.rs:144` | The `effective_kinematics` accessor and the 200 mm/s² default. |
| `planning/PRE_OPTIMIZE_DEFAULTS_AUDIT.md` | `tool_load/optimize/` | The pre-optimise default audit. |

The remaining 17 dead paths (`architectural_refactor_2026-06-06_v2.md`,
`DEFECT_CLASS_CLEANUP_2026-06-10.md`, `DEXEL_Z_ONLY_INVESTIGATION.md`,
`feed_modulation_roadmap.md`, `OPTIMIZE_EXPLAINABILITY_AND_PEAK_FINDING.md`,
`OPTIMIZER_REFACTOR_G16.md`, `OPTIMIZER_UX_PLAN.md`,
`STEP5_PREP_RETARGETERS.md`, `STRUCTURAL_ENTRY_SPANS_AND_LOCALITY.md`,
`tool_diagnostics_generic_plan.md`, `unified_v3_design.md`,
`review_2026-08-04/SIMULATION_ISSUE_CHANNEL_CENSUS.md`,
`review_2026-08-04/TECH_DEBT_2_CLOSEOUT.md`,
`review_2026-08-08/APPLY_CONTRACT_CENSUS.md`,
`review_2026-08-08/ARC_FIT_RATIO_EVIDENCE.md`,
`review_2026-08-08/HEATMAP_VOCAB_CENSUS.md`,
`review_2026-08-08/XVAC_CENSUS.md`,
`review_2026-08-08/ORCHESTRATION_LOG.md`, plus the two
`review_2026-08-08/artifacts/a8i/narrow_band_census` paths) support process
and structure, not constants.

## Alive, and doing real work

`planning/UNIFIED_LOAD_MODEL_2026-06-18.md` (the affine force model),
`planning/data_ingest_2026-05-29/kc.md` and `hardness.md`,
`planning/data_ingest_2026-05-30/aluminum_kc.md` and `hardness_extra.md`,
`planning/review_2026-08-04/CHIPLOAD_LITERATURE_VERDICT.md`,
`LAW_MAGNITUDE_TABLES.md` and `DRILL_GATE_EVIDENCE_AUDIT.md`,
`planning/feeds_literature_matrix_2026-06-03.md`,
`planning/finishing_stack_review_2026-07.md`.

Note the pattern: **the citations that survived are the ones behind the
FITTED and LITERATURE entries.** The citation that died is the one behind the
largest FOLKLORE multiplier. That is not a coincidence — the data-ingest and
literature-verdict packages were kept precisely because `CREDITS.md` names
them, and `MILLING_KC_FACTOR`'s document was never named there.

## One more citation shape to watch

`material/mod.rs:585-613` (`drill_per_peck_max_dtd`) (`janka_to_drill_chip_welding_dtd`'s sibling, the
per-peck D/d accessor) is the counter-example and the standard the rest should
meet. It carries a heading — "**Provenance: REPO-AUTHORED. No primary source
states a per-peck depth-to-diameter limit for wood**" — then lists three
specific corrections to the citation it *used* to carry, each with what was
claimed and what checking found. It cites a live audit document. Nothing in
that comment can mislead a reader about its own strength.

---

# 4. What the load model would look like if every FOLKLORE entry were measured

Two questions matter: which numbers move a kilowatt or a newton, and which are
decoration. The engine's own reference cut (6 mm 2-flute flat, 17 000 rev/min,
DOC 4.20, WOC 2.10, generic softwood, from `ADVICE.md` §1) is the yardstick.

## 4.1 The ones that matter — first order

A ±20 % change in any of these moves the reported load by ≥ 20 %.

| Rank | Symbol | Current class | Leverage | Cost to measure |
|---|---|---|---|---|
| 1 | `GRAIN_ANISOTROPY_FACTOR` | FOLKLORE (cited) | **×2 on every power number.** Nothing else in the engine changes power by a whole factor on its own. | Read one DOI, or run one cut along grain and one across, with a clamp meter on the spindle. **One afternoon.** |
| 2 | `MILLING_KC_FACTOR` with `LIT_ANCHOR_KC_N_PER_MM2` | FOLKLORE + DERIVED | **×2.7 on every solid-wood force and power number.** The pair also sets solid wood's position relative to sheet goods, which do not carry the factor. | Three cuts in one hardwood at three chiploads, with a spindle-power log. The affine fit then comes out directly and both constants are replaced by one measurement. **One day.** |
| 3 | `ENDMILL_CORE_FRACTION` | FOLKLORE | Deflection goes as the inverse fourth power of the section, so 0.7 against 0.8 is 1.7× and against a solid section is 4.2×. It also decides where the back-off loop parks. | Clamp a tool at known stickout, hang a known weight, read the tip with a dial indicator. Repeat for three diameters. **One afternoon.** This is the highest value per hour in the census. |
| 4 | `base_cutting_speed_m_min` | FOLKLORE | Sets RPM when no vendor row matches, and the edge term — 83 % of cutting power at the reference chipload — is linear in RPM. Nobody treats this as a load constant; it is one. | Vendor SFM tables. **Two hours**, no bench needed. |
| 5 | `MachineProfile::safety_factor` and `power_kw` | FOLKLORE | They set the line every power verdict is compared against. A 0.75 against 0.80 factor is a 6.7 % shift in the threshold; 0.71 kW against a real 0.9 kW is 27 %. | Nameplate plus one no-load and one loaded clamp-meter reading. **One hour.** |
| 6 | `RUBBING_FLOOR_MM_TOOTH` | FOLKLORE | Sets the low end of chipload, hence the shear term, hence the burn verdict on every small tool. The feeds folder instructions already flag that "without a matched vendor row, a sub-2 mm diameter conclusion is provisional" — this constant is why. | A burn test: one species, one small tool, a chipload ladder. **Half a day.** |
| 7 | Foam Kc, plywood Kc, sheet-good effective Janka | FABRICATED / FOLKLORE | Wrong by a factor for a whole material family rather than by a percentage for all of them. Plywood currently reads 2.7× softer than solid wood of the same veneer purely because it skips `MILLING_KC_FACTOR`. | One cut per family. **One day for all three.** |
| 8 | `RigidityProfile`, all seven fields | FOLKLORE | They set the DOC and WOC the whole model is evaluated at. Wrong geometry makes a right force model report a wrong load. | Push the gantry with a luggage scale and read the deflection with a dial indicator. **One afternoon**, and it promotes all seven at once. |

**If exactly those eight were measured, the load model would change character.**
Today every absolute number in it traces back through `35.1` to one
unnamed-species study and one unsourced species-equivalence premise. After a
single day of bench work the chain would be: one measured `Ks` and `F_edge`
pair for one real wood on the operator's real machine, with the existing
literature used only for the *relative* ordering between materials, which is
what the literature is actually good for. The engine already has the right
shape — the affine two-term model, the immersion arc, the separation of
sustained force from transient allowance. What it lacks is a single point of
contact with reality.

The honest label would also change. `force.rs` currently says
"literature-absolute" and, two paragraphs later, "approximate / verify on a
test cut". Only the second sentence is true today. After the bench, only the
first would need to be rewritten.

## 4.2 The ones that are noise

These are FOLKLORE, and measuring them would not repay the afternoon.

- `WORKHOLDING_HIGH_FACTOR = 1.03`. A 3 % feed increase. Below the resolution
  of every other input to the same product.
- `SPINDLE_CEILING_HEADROOM = 0.95` and `MAX_SPINDLE_SPEEDUP = 1.5`. Policy
  choices about how close to a limit to run. There is nothing to measure; they
  need a stated rationale, not data.
- `MIN_AP_MM`, `MIN_AE_MM`, `POWER_LADDER_AP_FLOOR_MM`,
  `POWER_LADDER_AE_FLOOR_MM`, `SCALE_CLAMP_LO` / `_HI`, `BINSEARCH_*`,
  `MIN_CHIPLOAD_RANGE_FRACTION`, `D_FLOOR_MM`. Numerical guards. They exist to
  keep a solver finite, and each is documented as such.
- All five `PREDICTOR_*` fractions. They size a move-count estimate that
  triggers a runtime guard. They never reach a newton.
- `BIPOLAR_SIDE_FRACTION`, `VBIT_ANGLE_TOLERANCE_DEG`,
  `VBIT_ANGLE_MATCH_BONUS`, `PLUNGE_CLASS_HALF_ANGLE_DEG`,
  `LATERAL_MAX_DESCENT_ANGLE_DEG`. Classifier and scoring thresholds. They
  decide which bucket a sample or a row lands in. They need a stated
  convention, not a measurement.
- The back-off solver knobs (`DEFLECTION_BACKOFF_FACTOR`,
  `STEPOVER_BACKOFF_FACTOR`, the iteration caps, `STEPOVER_BACKOFF_TARGET_MOVES`).
  Tuned against one fixture, and correctly described that way. They change how
  fast a search converges, not what it converges to.
- `LD_*` and `WORKHOLDING_*` as a group. The deflection model already predicts
  the effect these step functions approximate, continuously and from the same
  inputs. Measuring them would buy a better approximation of something the
  engine can compute exactly.

## 4.3 The middle: cheap to promote, moderate payoff

- `DEFLECTION` bounds `WITHIN_BOUND_MM = 0.050` and `EXCEEDS_BOUND_MM = 0.200`.
  These are round and uncited, and three DERIVED constants hang off them. They
  are a **specification** — how much tip wander the operator will accept — not
  a measurement. They should be labelled as a policy and given an owner, and
  then they stop being FOLKLORE by reclassification rather than by bench work.
- `MAX_SPINDLE_SPEEDUP = 1.5`. The claim that it "matches the rough magnitude
  of the chart's own spread" is checkable with a one-line query over the 252
  bundled LUT rows. No bench needed.
- `ChipLoadFormula::q = 1.26`. Already superseded in effect by
  `CHIPLOAD_HARDNESS_EXPONENT = 0.5`. Reconciling the two paths retires a
  FOLKLORE constant without measuring anything.
- `depth_tier_multiplier` against `doc_derating_scale`. One of the two is
  wrong at every ratio between the vendor table's rungs. Resolving that is a
  code question, not a data question.
