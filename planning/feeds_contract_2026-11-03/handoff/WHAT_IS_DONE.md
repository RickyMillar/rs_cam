# What the feeds contract did (P0 to P14)

Hand-off commit: HEAD `f0150aeb5`. Each claim cites a file:line, a commit or a sentry.
Sentries run as `cargo test -p rs_cam_core -q --test <name>` (viz: `-p rs_cam_viz`).
The phase table at `planning/feeds_contract_2026-11-03/STATUS.md:349-366` is the short form.
Each phase card in `planning/feeds_contract_2026-11-03/phases/` holds the full result.

## 1. The contracts

The one-line form of each contract is `crates/rs_cam_core/CONTRACTS.md:5-15`.

| Contract | Rule | Sentries (`--test` unless named) | Line |
|---|---|---|---|
| Lint | No wildcard arm over two or more variants | the full clippy gate | `crates/rs_cam_core/CONTRACTS.md:5` |
| K1 | Every op states its cut (`CutSpec`) | `every_op_states_its_cut_k1`, `--doc cut_spec_refusals` | `crates/rs_cam_core/CONTRACTS.md:6` |
| K2 | Every shipped number names its source (`Sourced<T>`, `Basis`) | `suggest_never_raises_a_stated_depth_k2`, `a_tie_takes_the_lower_chip_and_names_both_q3` | `crates/rs_cam_core/CONTRACTS.md:7` |
| K2b | Suggest returns a space of safe choices (`SafeEnvelope`) | `apply_writes_speeds_unless_a_pick_k2b`, `the_envelope_keeps_each_limit_k2b`, `a_pick_outside_the_envelope_warns_k2b` | `crates/rs_cam_core/CONTRACTS.md:8` |
| K3 | Every tool fact states how it is known (`ToolFact<T>`) | `a_tool_fact_states_how_it_is_known_k3` | `crates/rs_cam_core/CONTRACTS.md:9` |
| D8 | No geometry default without a tool | `every_geometry_default_is_a_fraction_of_the_tool_d8` | `crates/rs_cam_core/CONTRACTS.md:10` |
| K4 | Every op states how it enters material (`EntryContract`) | `every_op_states_its_entry_contract_k4`, `project_curve_plunges_at_the_axial_chip_q8` | `crates/rs_cam_core/CONTRACTS.md:11` |
| K5 | Every physical rule has the G-CENTRECUT shape | `every_physical_rule_warns_through_a_row_k5` | `crates/rs_cam_core/CONTRACTS.md:12` |
| K6 | Every finding states its consequence; no finding refuses a job | `a_job_with_each_finding_exports_and_names_it_k6` (core and viz) | `crates/rs_cam_core/CONTRACTS.md:13` |
| K7 | One engagement before and after the simulation | `one_engagement_before_and_after_the_simulation_k7`, `the_power_curve_picks_the_arm_at_the_feed_cap_q1` | `crates/rs_cam_core/CONTRACTS.md:14` |
| K8 | Every toolpath carries its lineage | `every_generated_toolpath_has_a_lineage_k8`, `every_calculator_step_leaves_one_record_p13b` | `crates/rs_cam_core/CONTRACTS.md:15` |

## 2. The phases

Each row: what changed (operator terms), the commits, the contract, what was not done.

| Phase | What changed | Commits | Contract | Not done, reds |
|---|---|---|---|---|
| P0 | Local fixes from the nz-wanaka-21 blind review: pin tool, drill dwell deleted (D4), Amana tapered rows (D5) | 890850a67 | none | FM1 re-blessed, 13 cells (`planning/feeds_contract_2026-11-03/STATUS.md:351`) |
| P1 | The workspace denies `clippy::wildcard_enum_match_arm` | b01777df5 | Lint | 563 hits: 256 made exhaustive, 307 `#[expect]` (`planning/feeds_contract_2026-11-03/STATUS.md:352`) |
| P2 | Each diagnostic id states its consequence; no finding refuses a job (Q6) | 4d69c2e3a, c6826ad05 | K6 | none named |
| P3 | Each op states how it enters material | c0db8dcb6 | K4 | no G-code byte moved |
| P4 | Each op states its cut | e8ebe65ac | K1 | FM1 unchanged |
| P5 | Suggest returns an envelope; it writes geometry only on a pick | bbaf105ef, a5ec62f44 | K2, K2b | 16 `SuggestWarning` variants left unproduced (P14 deleted 14: `planning/feeds_contract_2026-11-03/phases/P14.md:61`) |
| P5b | The envelope plot on the Passes tab; each toolpath result has a lineage | baca127f5 | K2b view, K8 | MCP cannot drag the envelope point (`planning/feeds_contract_2026-11-03/STATUS.md:370-373`) |
| P5c, P5d | Two operator reviews of the plot: only binding limits draw, every limit is named | bf95f5fae, d0e7f2e5c, 13c35395e, 545ac9935 | K2b view | label follow-ups at `planning/feeds_contract_2026-11-03/STATUS.md:399-405` |
| P6 | Each tool fact states how it is known; the operator library migrated (D6) | db86f0b66, 7db25cb25, c7936ddd8, 90bfd3105 | K3 | no number moved; F201 149 -> 154 |
| P7 | One feasibility stage, the K5 rule table; centre cutting and the V tip are required inputs (D12) | 8ddfe1079, 72ef44c49, deb03a1a9, e254e219d, b9bf55844 | K5 | wanaka op 1 time 15 s -> 27-48 s, 12 caught panics (reviewer item R1) |
| D9 | The machine aggressiveness dial is removed | dee399486 | none | no number moved (`planning/feeds_contract_2026-11-03/phases/D9.md:32-40`) |
| P9 | One engagement before and after the simulation; the simulation is placed on the envelope, not fed back | fb260023b | K7 | open list at `planning/feeds_contract_2026-11-03/phases/P9.md:248-260` |
| P8 | At the feed cap the power curve picks the rpm arm (Q1); Project Curve plunges at the axial chip (Q8); one collision warning on every surface (Q6) | b92b78090, 42a7aa9eb | K7, K4, K6 | Q6 frame cost (reviewer item R5) |
| P10 | Geometry defaults come from the tool (D8) | 84bfeab68 | D8 | radial and axial `k` tables Unsourced (R6) |
| P11 step 0 | Validation study of the force model (read-only); verdict FAIL | de71b78aa | none | `planning/feeds_contract_2026-11-03/research_p11_step0/REPORT.md` |
| P11a | The engaged-edge force with the helix (peak for deflection, mean for power); envelope defects D-A, D-B | 537aa146a | K7 | 430 FM1 `force_n` cells moved; list at `planning/feeds_contract_2026-11-03/phases/P11.md:267-304` |
| P11b | Garr chip thinning in Suggest; chip first; the vendor 1/2/3 x D table is a limit; a stated depth ships | 55f50147b | K2b | L9 page FAIL (W 2/7, D 23/455; `planning/feeds_contract_2026-11-03/STATUS.md:221-222`) |
| P11b2 | The post-sim gate and the modulator judge the peak chip; no depth de-rate on any band | f5170da03 | K7 | Waterline keeps `commanded_above_band` on 2 FM1 sim cells (R9) |
| Derate audit | Read-only trace of every multiplier from vendor row to shipped number | 379cdefef, 3510e1533 | none | `planning/feeds_contract_2026-11-03/research_derate/REPORT.md` |
| P11c | The slot cap moves into `RigidityProfile`; `MachineClass` Hobby/Industrial; three `woc_*` fields deleted | 36fd50c54 | K2 | item 2 (computed Pocket width) did not land; P13c did it |
| P13 step 0 | Research: inputs and assumptions per output (24/24 asserts) | abdc325da, be7b7253f | none | `planning/feeds_contract_2026-11-03/research_p13_step0/FINDINGS.md` |
| P13a | `calculate_with` split by step; `Basis::ToolFact` | 4efcb4eaa | K2, K3 | open items at `planning/feeds_contract_2026-11-03/phases/P13.md:77-89` |
| P13c | Hobby force budget from six printed slot rows sets the roughing depth; the proposal is the removal-rate maximum, bounded by the corner rule 0.854 D | b57dab8de, 255d8db90, ebe64d70f | K2b | FM1 200 pre-sim + 16 sim cells; open list at `planning/feeds_contract_2026-11-03/phases/P13.md:327-343` |
| P13b | One step record per calculator step, stored per toolpath in the project file | 5ab85f4b2, 7048456f8 | K8 | project file grows 1.25x / 1.82x (`planning/feeds_contract_2026-11-03/phases/P13.md:187-198`) |
| P13 items 4-5 | Flat finishes read their own roughing row; the four Freud industrial rows are test baselines only | 7a7c3a449, d82985059, fb882530a | K2 | FM1 43 cells; 20 ball and 8 V-bit cells stay on the formula chip (`planning/feeds_contract_2026-11-03/phases/P13.md:484-487`) |
| P14 | Tidy from the architecture review; one envelope memo; warn names | a02099f31, f0150aeb5 | K6, K7 | bull-nose `d_eng` defect logged (R30) |

Planning commits before P0: f56000c54 (the plan), 218ba2858 (first rulings), 98344a12f (perf base).

## 3. The rulings

| Ruling | Decision (short) | Where |
|---|---|---|
| Q1 | At the feed cap, ship the rpm arm that stays inside the power curve | `planning/feeds_contract_2026-11-03/PLAN.md:10` |
| Q2 | Folded into Q4 | `planning/feeds_contract_2026-11-03/PLAN.md:11` |
| Q3 | A tie between vendor rows takes the lower chip; the card names both | `planning/feeds_contract_2026-11-03/PLAN.md:12` |
| Q4 | Suggest is a space of safe choices | `planning/feeds_contract_2026-11-03/PLAN.md:13` |
| Q5 | No feedback loop from the simulation | `planning/feeds_contract_2026-11-03/PLAN.md:14` |
| Q6 | The program never refuses a job; every finding warns | `planning/feeds_contract_2026-11-03/PLAN.md:15` |
| Q7 | The lineage shows which facts a toolpath used | `planning/feeds_contract_2026-11-03/PLAN.md:16` |
| Q8 | Project Curve: straight plunge now | `planning/feeds_contract_2026-11-03/PLAN.md:17` |
| Q9 | A bare number in an old project file fails to load | `planning/feeds_contract_2026-11-03/PLAN.md:19` |
| Q10 | A missing input stops generation or emission with a reason | `planning/feeds_contract_2026-11-03/PLAN.md:18` |
| D1-D12 | Pin tool, dwell, Amana rows, library, missing input, envelope sketch, tool-derived defaults, dial removal, chip thinning as a model, required tool facts | `planning/feeds_contract_2026-11-03/STATUS.md:283-294` |
| D13 | Derate audit rulings 1-7 (depth margin, computed stepover, slot cap, fitted law, industrial rows, hobby default, lineage store) | `planning/feeds_contract_2026-11-03/STATUS.md:297-323` |
| P11c lead | Mechanism for D13 items 1, 2, 3, 6 | `planning/feeds_contract_2026-11-03/STATUS.md:331-345` |
| O1, O4, O3 | Hobby depth from a force budget; proposal = most removal per minute; Pocket width = the computed width under 0.854 D | `planning/feeds_contract_2026-11-03/STATUS.md:78-86` |
| O2, O5 | Measured frame stiffness not now; heat and tool life out of scope | `planning/feeds_contract_2026-11-03/STATUS.md:87` |
| P11 L1-L9 | Chip thinning decisions | `planning/feeds_contract_2026-11-03/phases/P11.md:74-119` |
| P14 lead | M2 accepted (BandMid 2400); M3 merge waits (bull-nose defect); open false names; breaking renames | `planning/feeds_contract_2026-11-03/STATUS.md:93-110` |

## 4. Reviewer items

The reviewer judges each item. "Close" says what ends it. "R" numbers are for reference only.

### 4.1 Engine defects and open model terms

| # | Item | Cause | Site | Close |
|---|---|---|---|---|
| R30 | Bull nose below its corner radius: two diameter rules disagree (6 mm, r 1, ap 0.2 mm: 1.2 against 5.2 mm) | `bull_nose_effective_diameter` uses the corner circle only and drops the flat centre | `crates/rs_cam_core/src/feeds/geometry.rs:140-148`; the other rule `crates/rs_cam_core/src/tool/mod.rs:496` | One `d_eng` door and an FM1 re-bless (`planning/feeds_contract_2026-11-03/STATUS.md:99-103`) |
| R31 | `BandMid` modulation still exists; its doc says "one release cycle" | P14 ported the law instead of deleting the strategy | `crates/rs_cam_core/src/dressup/feed_modulation.rs:83` | Delete `BandMid` in a tidy (`planning/feeds_contract_2026-11-03/STATUS.md:96-98`) |
| R32 | Names that say "refuse" but only warn: `Gate::HeightRefuses`, `Gate::LoadRowRefuses`, `HeightOrderViolation::refuses` | P14 item 10 left them | `crates/rs_cam_core/src/diagnostics/consequence.rs:58`, `crates/rs_cam_core/src/diagnostics/consequence.rs:66`, `crates/rs_cam_core/src/compute/config.rs:485` | Rename, or show each refuses |
| R33 | `RationaleParam::StockToLeave` has no producer | dead variant | `crates/rs_cam_core/src/feeds/rationale.rs:45` | Delete it or add a producer |
| R34 | The depth solver returns the inside end of the bracket (3.4056 against the root 3.4073 mm) | `shallowest_crossing` bisects to the last inside point | `crates/rs_cam_core/src/feeds/cutter_constraints.rs:350` | Judge if 0.002 mm matters; else document |
| R35 | FM1 writes the depth at the calculator chip, but `force_n` and `cap_mm` at the floored feed | the instrument reads two feeds | `crates/rs_cam_core/tests/feeds_matrix_instrument_fm1.rs:825` | One feed per row, re-bless |
| R36 | The hobby budget is set by the Sienci MDF reading, so hardwood picks deeper than the printed Sienci hardwood slot (2.5 mm) | the budget is the largest of six points | `crates/rs_cam_core/src/feeds/force_budget.rs:164`, `crates/rs_cam_core/src/feeds/force_budget.rs:261` | Judge in the VFD ladder (brief part C) |
| R37 | 3.175 mm hardwood Pocket picks 2.2 x D (6.985 mm) | the vendor depth table binds at chip share 0.70; the table warns, it does not cap | `crates/rs_cam_core/src/feeds/geometry.rs:352`; numbers `planning/feeds_contract_2026-11-03/phases/P13.md:548-559` | Judge chip clearing and breakage there |
| R38 | Harvey 809500 prints Finish = 0.76 x Slot-Rough at 0.1 D; thinned reading 0.833, P11b reading 0.5 | finish passes do not thin (L2) | `planning/extrapolation_2026-09-24/fetch/G10/sources/metal_harvey_sf_809500.txt:13-32`; rule `crates/rs_cam_core/src/feeds/geometry.rs:62-66` | Judge if a light finish pass should thin |
| R39 | Two size laws: typed `CHIPLOAD_DIAMETER_EXPONENT` 0.61 against the fitted in-series slope (0.560 / 0.522) | G1 form C is typed | `crates/rs_cam_core/src/feeds/vendor_lookup.rs:541` | One size law; moves 88 `Extrapolated` cells (`planning/feeds_contract_2026-11-03/phases/P13.md:481-483`) |
| R40 | 20 ball-nose and 8 V-bit cells still use the formula chip | no B3 family rule; no V-bit diameter series | `crates/rs_cam_core/src/feeds/calculator/chip.rs:36` | B3 rule with a simulation witness |
| R41 | Waterline thins (it is `SemiFinish`); Suggest assumes 0.1 D width, the sim measures 90 degrees or more | L2 reads `PassRole::Finish` only | `crates/rs_cam_core/src/feeds/geometry.rs:60-66` | Decide the Waterline width assumption |
| R42 | No cap on the thinning lift; a very light width lands on the feed cap with rpm down to 0.515 x | L3: one formula, no cap | `crates/rs_cam_core/src/feeds/geometry.rs:30-37` | Judge against shop practice |
| R43 | Engaged-edge sum uses one radius `d_eng / 2` for the whole edge: an upper bound on ball, bull, taper and V-bit | P11a simplification | `crates/rs_cam_core/src/feeds/engagement.rs:323` | Per-element radius |
| R44 | `K_r = 0`: the resultant is tangential only | no printed wood radial ratio | `crates/rs_cam_core/src/feeds/engagement.rs:220-222` | A printed `K_r` for wood |
| R45 | The 30 degree helix default (Unsourced) reaches every force reader | no helix in most tool files | `crates/rs_cam_core/src/tool/mod.rs:219` | A printed helix per tool |
| R46 | Scans can miss a crossing inside one step: force budget 24 steps, deflection 16 steps | fixed step counts | `crates/rs_cam_core/src/feeds/force_budget.rs:296`, `crates/rs_cam_core/src/feeds/cutter_constraints.rs:101` | Bound the miss, or refine |
| R47 | `efficiency.rs` specific energy reads psi at the nominal D | not moved to `d_eng` | `crates/rs_cam_core/src/feeds/efficiency.rs:292` | Read `d_eng` |
| R48 | The power gate reads `axial_doc_mm`, the deflection gate `axial_engagement_mm` | two depth fields on one sample | `crates/rs_cam_core/src/tool_load/power.rs:219` | One depth field |
| R49 | Ball axial thinning is reported, not applied | P11b scope | `crates/rs_cam_viz/src/ui/feeds/why.rs:402` | Rule on it |
| R50 | No tool-strength limit (shank bending stress) | no printed carbide strength in the repo | none: absent | A printed strength value |
| R51 | The heat map paints the advance, not the peak chip | gap after P11b2 | `crates/rs_cam_core/src/tool_load/display.rs:154` | Paint the peak chip |
| R52 | `doc_derating_scale` has no production reader; it is kept as the printed table | P11b2 | `crates/rs_cam_core/src/feeds/geometry.rs:215` | Keep as the table, or fold into the inverse |
| R53 | The slot threshold 0.85 D (Unsourced) sits 0.4 % under the corner bound 0.8536 D; the pick lands on it | typed threshold | `crates/rs_cam_core/src/feeds/mod.rs:1919-1923` | A printed slot threshold |
| R54 | A project save writes the derived hobby budget 35.32 N as a stated value | `RigidityProfile` serializes `force_budget_n` | `crates/rs_cam_core/src/machine/mod.rs:85` | Store the class default as derived |
| R55 | The budget rows read `planning/extrapolation_2026-09-24/fetch/` files at test time | the sources live in planning | `crates/rs_cam_core/src/feeds/force_budget.rs:149` | Move the stored text under `crates/rs_cam_core/data/` |
| R56 | Power is read between width samples in the pick | the pick samples width | `crates/rs_cam_core/src/feeds/envelope.rs:1131` | Exact solve at the pick |
| R57 | Envelope cost: 81-97 ms per 6 mm Pocket envelope after the memo; the FM1 walk takes 4.9 s | unique force solves | `crates/rs_cam_core/src/feeds/envelope.rs:2424`; timing `planning/feeds_contract_2026-11-03/phases/P14.md:297-298` | A cheaper solve (moves FM1 depths in the last bits) |
| R58 | Stepover numbers typed with no source: 0.12 D (default table), adaptive width factor (0.25 hobby default), 0.15 D stale check | ports and rules of thumb | `crates/rs_cam_core/src/feeds/mod.rs:2041`, `crates/rs_cam_core/src/machine/mod.rs:335`, `crates/rs_cam_core/src/compute/validate.rs:267` | A printed stepover source |
| R59 | Radial and axial add-door `k` tables are Unsourced | P10 scaled the 6 mm values | `crates/rs_cam_core/src/compute/geometry_default.rs:685-699` | A printed source per `k` |
| R60 | The modulator compliance reads the toolpath peak depth; the force reads the move | two depths in `delta = C F` | `crates/rs_cam_core/src/tool_load/deflection.rs:80` | Compliance per move |
| R61 | FM1 sim: the Suggest recipe is outside its own envelope on 18 of 35 cells | P9 measurement | `crates/rs_cam_core/src/compute/simulate.rs:967` | Explain each cell |
| R62 | The 3D Rough planner cuts near-slot widths (4-5x the stepover) on the opening cut of each level | planner, not feeds | `planning/feeds_contract_2026-11-03/phases/P9.md:255-257` | Planner fix |
| R63 | rivmap100 "Face 5" states 0.812 mm per pass and cuts 2.00 mm in one level | Face top at the model top, not the stock top | `planning/fixtures/rivmap100/rivmap100.toml:154` | Face ladder fix |
| R64 | No noise floor on `project.sim_outside_envelope` (3 of 264564 samples warn) | a threshold would be a new Unsourced constant | `crates/rs_cam_core/src/compute/simulate.rs:967` | Rule on a floor |

### 4.2 Surface and record items

| # | Item | Site | Close |
|---|---|---|---|
| R1 | `cavalier_contours` panics 12 times (caught) in wanaka "Front 3D rough"; op 1 time 15 s -> 27-48 s | `planning/feeds_contract_2026-11-03/STATUS.md:195-196` | Find the input that panics |
| R2 | The words "long-tool share of the load target" stay; nothing reads that target | `crates/rs_cam_viz/src/ui/feeds/why.rs:368` | Delete the words or the target |
| R3 | The "Measured" row shows only after an inspector memo; the plot draws no sim points; placement reads the toolpath material, not the per-sample material | `planning/feeds_contract_2026-11-03/phases/P9.md:250-252` | GUI follow-up |
| R4 | The warning name `RpmLoweredForFeedCeiling` stays when arm S ships; a material with no force line counts an arm as inside | `crates/rs_cam_core/src/feeds/suggest.rs:457` | Rename; rule on the no-line arm |
| R4b | P8: the two plunge terms name two rows | `planning/feeds_contract_2026-11-03/STATUS.md:204-205` | One row per plunge, or name why two |
| R5 | Q6: the Readiness panel builds export warnings each frame; the status bar, the live overlay, the overlay registry and `issue_triage` still read the GUI move list; the card's "simulation ran" test reads GUI state | `crates/rs_cam_viz/src/ui/status_bar.rs:130`; `planning/feeds_contract_2026-11-03/STATUS.md:206-209` | Read the core rows; measure frame cost; test the session |
| R16 | P11a: the post-sim deflection gate runs the edge sum per sample (FM1 sim 40.2 s -> 40.8 s) | `crates/rs_cam_core/src/feeds/engagement.rs:323`; `planning/feeds_contract_2026-11-03/phases/P11.md:269-274` | Memoise `edge_sums` per toolpath on exact bits |
| R17 | P11a: `helix_lag_rad` equals the old expression by inspection only; SAME46 does not cover the sim-prefix hash | `planning/feeds_contract_2026-11-03/phases/P11.md:279-281` | A test on the hash |
| R18 | P11a: `Edge::Unmodelled` names a modelled absence; a zero-chip column also reads as no root | `crates/rs_cam_core/src/feeds/envelope.rs:2466`; `planning/feeds_contract_2026-11-03/phases/P11.md:283-285` | Rename the variant |
| R19 | P11a: a column already over the target at its shallowest depth draws an edge at 0 mm | `planning/feeds_contract_2026-11-03/phases/P11.md:286-287` | Judge the draw rule |
| R20 | P11a: `DepthClamps::DrawnAsLimits` omits the `DocExceedsFlute` and `SlottingDetected` warnings at grid points | `planning/feeds_contract_2026-11-03/phases/P11.md:288-289` | Confirm no depth moved; keep or add |
| R21 | P11a: the `feeds::predict` wanaka back-rough fixture now reads 18.8 um (was about 48) | `planning/feeds_contract_2026-11-03/STATUS.md:217` | Judge the new value |
| R22 | P11b: the depth-table ceiling divides by the nominal diameter (a tapered ball reads the tip); a one-value row allows only 1 x D at the suggested chip | `crates/rs_cam_core/src/feeds/geometry.rs:352`; `planning/feeds_contract_2026-11-03/phases/P11.md:489`; `planning/feeds_contract_2026-11-03/STATUS.md:225` | Read `d_eng`; rule on one-value rows |
| R23 | P11b: the envelope power edge at an operator chip calls `power_at_operating_point` per grid point and bisection step (cost not measured) | `crates/rs_cam_core/src/feeds/operating_point.rs:219`; `planning/feeds_contract_2026-11-03/phases/P11.md:492-493` | Measure the cost |
| R24 | P11b: the k2b fixtures state the full Suggest chip; the flute-guard cell states half, so the depth table does not bind first | `planning/feeds_contract_2026-11-03/phases/P11.md:494-495` | Confirm the fixtures test what they name |
| R25 | P11b2: the commanded lift has two sources (the stated cut, else the median measured lift) | `planning/feeds_contract_2026-11-03/phases/P11.md:639` | One source |
| R26 | P11b2: `PerMoveEngagement::thinning_lift` is the minimum over all cutting samples of a move (a wider set than the gate's) | `planning/feeds_contract_2026-11-03/phases/P11.md:640-643` | Judge; same population as the gate |
| R27 | P11b2: the modulator band keys the row at the peak depth; its deflection and power caps read the modelled width while its chip reads the measured arc | `planning/feeds_contract_2026-11-03/phases/P11.md:644`; `planning/feeds_contract_2026-11-03/STATUS.md:235-236` | One width per move |
| R28 | P13a: the `feeds/mod.rs` module doc lists the pre-R4 pipeline (a safety factor step) | `planning/feeds_contract_2026-11-03/phases/P13.md:84` | Rewrite the doc |
| R29 | P13b: a loaded op with step records and no result draws one more row (over the IC1 cap); `later_write` reads the stamp only | `planning/feeds_contract_2026-11-03/phases/P13.md:189-190`; `planning/feeds_contract_2026-11-03/phases/P13.md:194-195` | GUI row budget; records for every writer |
| R29b | P13c: inside a stated box the pick may trade width for depth on the force plateau (G-CHIPVERDICT box: 0.85 D x 3.41 mm) | `planning/feeds_contract_2026-11-03/phases/P13.md:327-330` | Judge the trade against shop practice |
| R29c | P14: `session/compute/diagnostics.rs` and `toolpath_diagnostics.rs` sit side by side | `planning/feeds_contract_2026-11-03/phases/P14.md:235-236` | Sharper names |
| R6 | P10: the calculator's `ap` factors put the add-door default outside the envelope (Rigidity 118 -> 134, VendorAxial 10 -> 33 FM1 cells) | `planning/feeds_contract_2026-11-03/STATUS.md:218-220` | Superseded in part by P13c; confirm |
| R7 | P13a: the GUI envelope hover sees the legacy row only through the plunge tree; shank and stick-out are in no basis | `crates/rs_cam_viz/src/ui/feeds/envelope.rs:723` | Add both facts to a basis |
| R8 | P13a: check H2 kept the order of every warning push and of the two `feed_law` calls | `crates/rs_cam_core/src/feeds/calculator/feed.rs:14` | Review by diff of 4efcb4eaa |
| R9 | P13b: a project saved before P13b loads with no step records until an apply; step names are code names on the hover | `planning/feeds_contract_2026-11-03/phases/P13.md:187-198` | GUI text; migration note |
| R10 | P13c: `ComputedWidthAssumptions` fires on every Pocket-family roughing op, a typed width included | `planning/feeds_contract_2026-11-03/phases/P13.md:336` | Fire only on a computed width |
| R11 | P13 items 4-5: the ramp follows the feed row (`FeedRowOffTheDrillChart`) on 3 cells | `planning/feeds_contract_2026-11-03/phases/P13.md:446-456` | Accepted by the lead; reviewer confirms |
| R12 | P14: three `retired_lift_*` tests still assert | `crates/rs_cam_core/src/feeds/suggest/tests.rs:1264` | Delete or rename |
| R13 | P14: `diagnostics/CLAUDE.md` is 47 lines (rule: 40) | `crates/rs_cam_core/src/diagnostics/CLAUDE.md:47` | Split |
| R14 | P14: no MCP capability token for `basis.shipped_power` | `planning/feeds_contract_2026-11-03/phases/P14.md:135-136` | Add a token |
| R15 | P11c: the slot cap and the add-door depth were both 0.25 D; the 0.35 D Pocket width cell; `doc_finishing_factor` had no reader | `planning/feeds_contract_2026-11-03/phases/P11.md:859-866` | P13c replaced the add-door depth and width and retired `doc_finishing_factor` (b57dab8de, `crates/rs_cam_core/src/machine/mod.rs:67`); confirm closed |

### 4.3 Accepted reds

| Red | Cause | Where | Close |
|---|---|---|---|
| G-OPGOLDEN, 46 rows | rows that differ at clean HEAD since 98344a12f | `planning/feeds_contract_2026-11-03/opgolden_known_46_rows.txt:1-46`; test `crates/rs_cam_core/tests/every_op_keeps_its_toolpath_g_opgolden.rs`; rule `planning/feeds_contract_2026-11-03/STATUS.md:419-421` | A gate passes on the SAME 46 rows. Only a lead ruling closes a row; never a field bless (`planning/feeds_contract_2026-11-03/STATUS.md:461-464`) |
| W4 check6 | `contour_each_level` has `skip_serializing_if` | `crates/rs_cam_core/tests/every_op_conforms_w4.rs`; `planning/feeds_contract_2026-11-03/STATUS.md:434-436` | Fix the serde attribute |
| `_litmatrix_rubbing_floor_clamp` | Ipe 6 mm winner is the Freud row, the test expects Amana | `planning/feeds_contract_2026-11-03/STATUS.md:441-443` | Rule on the expected row |
| `a_clearing_cut_holds_the_pass_load_g_adaptpassload` | red since c6826ad05 | `planning/feeds_contract_2026-11-03/STATUS.md:438-440` | Investigate |
| `a_rough_cuts_no_deeper_than_its_leave_g_roughgouge` (5/1) | message says "warn", test wants "refuse" | `planning/feeds_contract_2026-11-03/phases/P14.md:272-273` | Update the test text |
| `the_envelope_meets_its_time_budget` | load-flaky (326-479 ms under a loaded run, 135 ms alone) | `crates/rs_cam_core/src/feeds/envelope.rs:2756` | Run alone before trusting a red |
| Core integration list (19 targets on S11 "feeds not set" and 10 others) | own fixtures, pre-existing | `planning/feeds_contract_2026-11-03/STATUS.md:444-460` | Fix each fixture |
| viz, 10 targets + 1 flake | pre-existing | `planning/feeds_contract_2026-11-03/STATUS.md:183-188` | Per target |
| viz `folder_maps_name_real_files_f140` | names `ui/components/glyph_tests.rs`; checks no size | `crates/rs_cam_viz/tests/folder_maps_name_real_files_f140.rs` | Fix the map |
| `the_inspector_text_is_bounded_ic1` | "Tool: 18 visible text rows, cap 17" | `planning/feeds_contract_2026-11-03/phases/P13.md:362` | GUI row count |
| fmt diffs in 3 files | files no phase touched | `planning/feeds_contract_2026-11-03/STATUS.md:161-163` | Format them in their own commit |
