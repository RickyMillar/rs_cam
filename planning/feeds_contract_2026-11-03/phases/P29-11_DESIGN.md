# P29-11 and P29-last: the other metrics, M4, F6, and the search guard

Design note for the lead. Written 2026-11-14 at HEAD `c62e5b225`. No product code changed.
Probes: one throwaway core test (`tests/zz_probe_m4_p29_11.rs`, deleted after the runs) and one
scan script that removes comments, string text and `#[cfg(test)]` modules before it counts
(`scan.py`). Both are in the session scratchpad folder `p29_11/`. "Measured" is a probe output;
"reasoned" is a reading of the code. The four project files were loaded read-only, with no
simulation; the operator file kept its sha256 `f6a2b388…4e41`.

## 0. The answer in short

1. **M4 is a fact of the sample coalescer, not of two models.** The dexel writes ONE depth into
   both fields (`dexel_stock/simulation.rs:1371-1372`). The fields differ only after the
   coalescer folds the Z subdivisions of one move: `axial_doc_mm` becomes a time mean, and
   `axial_engagement_mm` and the arc become maxima (`dexel_stock/sample_coalesce.rs:238, 260-268`).
   So `Engagement::from_sample` (power, chip lift, depth-gate width) pairs a MEAN depth with a
   MAXIMUM arc. The coalescer doc says that the power gate reads the peaks
   (`sample_coalesce.rs:36-41`). The code does not do this.
2. **Measured on 8 simulated fixtures:** the depths differ on 0 % of the 2D body samples, on 0
   to 52 % of the entry samples (0 on the Profile and the dome fixtures) and on 2 to 18 % of
   the 3D body samples. The engaged depth is the larger (reasoned: a maximum against a time
   mean of the same values). With one depth (the engaged edge), 4 of 168 group rows move, all power,
   all up (largest: 7.5 % to 9.6 % of the limit). No row changes state. On 3 fixtures with
   modulation, no move was bound by power.
3. **M4 recommendation: one depth, the maximum, for every gate and for the feed modulator
   (M4-A).** **It is a behaviour change: an exported feed falls where the modulator's power
   cap binds.** It is the only modulator cap that M4-A changes: the chip lift reads
   `ae / d_eng = (1 - cos psi) / 2`, which holds no depth (section 3.4). No fixture move binds
   on power. The operator file was not simulated. The lead
   brings the operator in (section 3.5).
4. **F6:** the one cell that differs (2D Adaptive, Link group) is not a depth question. The
   depth gate gives `NoNumber(NoBound)` for "no force-budget root below the flute". That is a
   proof of `Within`, and the force reading shows it (1.73 N against 35.32 N). The depth gate
   also has a second width door, the time-mean width share, that the force reading does not
   read (66 of 399 Link samples). Two changes give one record (section 4); they do not depend
   on the M4 outcome.
5. **P29-last:** today, 402 comment-free hits of an op-type, role or depth-rule token are in
   45 core files of the guarded set (section 5.3), and 33 in 10 files of viz, MCP and the CLI. Most are a vendor row key, the
   write funnel, the declaration or a label. Section 5.4 lists the scope sites in 12 groups:
   S1 to S9 go by a type rule or a move into `cuts()`, byte-equal; S10 becomes a fact of each
   cut; S11 and S12 are P29-11 commits. The rest stay on an allow-list that can only shrink,
   each entry with its exemption row or its class.
6. **At the end of P29, if every 11 commit lands:** 34 exemption rows (from 37): 8 physical,
   3 ruled, 23 that need a ruling, each with an owner outside P29 (section 7). The end state is
   conditional on M4-A, on 11-3 giving every stated row a value function, and on 11-8.

## 1. Scope table

`P29_FINDINGS.md` section 6, line 182: "P29-11 | Other metrics on the cut: efficiency through
the engagement, one rubbing rule, cusp height of a stated stepover, one reached-feed measure,
the per-op `Capabilities` metric flags. | Small | - | later". Line 183: "P29-last | Delete the
scattered scope reads; a search guard keeps them out. | No | the guard | -".

Verdicts: **11** = P29-11; **L** = P29-last; **Out** = out of P29, with the owner. Rule: an item
that needs a new physical model, a constant with no source or a new generator behaviour is out.

| # | Item | Where | What is wrong today (one example) | Verdict |
|---|---|---|---|---|
| 1 | `M4` two sample depths | `feeds/engagement.rs:242`, `tool_load/deflection.rs:177`, `limits/cut.rs:345`, `limits/row.rs:1284` | Shelf 3D rough, entry group: power 0.0262 kW at the mean depth, 0.0291 kW at the engaged depth (measured) | **11** (section 3) |
| 2 | F6, the depth row against the force reading | `tool_load/depth.rs:332-345, 476-485`, `tool_load/cut_force.rs:210-222` | 2D Adaptive Link: depth `NoNumber`, force 1.73 of 35.32 N (measured) | **11** (section 4) |
| 3 | The operating point differs by door (flute count) | `feeds/cut_load.rs:43-46, 168-217` | A product sample takes the tool's planning flute count (`compute/simulate.rs:291`), so no product number differs (reasoned) | **11**, byte-equal: one `OperatingPoint::of_sample` |
| 4 | `U-efficiency` | `feeds/efficiency.rs:207-215` | The lift reads `RadialThinning::of(family, role)` and the nominal diameter; the force headroom reads a flat 200 um (`efficiency.rs:396`), not `deflection::exceeds_limit` | **11**: the cut's `thins_chip`, the engaged diameter, the deflection limit of the cut. A Suggest card number moves on ball and tapered tools and on a leave under 200 um |
| 5 | Narrate deep-depth rule | `trace/narrate.rs:50, 1863, 1917` | A judgement of a cut with its own constant (`DEEP_DOC_MULTIPLIER = 1.5`) on the MEAN depth | **11**: narrate reads the depth group row; the constant goes (text moves) |
| 6 | `S-NotJudgedHere` | `limits/project.rs:125-135`, `feeds/envelope.rs:3365, 3774, 3829` | Operator file, 67 rows (measured). 32 are drill-group cells of a scope row (P28-20 physical, P28-10 needs a ruling) that show `NotJudgedHere`. 35 are the stated point against an edge that the envelope does not draw: CuttingFeedCap 5, CuttingLength 3, EngagementFloor 10, FluteGuard 3, MoveBudget 4, ToolDiameter 7, VendorAxial 3 | Why 67, not the 55 of `P29-10_DESIGN.md` 13.6: that count was the `of_envelope` stated rows only, at `3a351d9a2`, before 10h and 10i; 10h moved the deflection rows out and 10i added per-cut stated rows, the drill ones among them (reasoned from the commit messages). **11**: (a) each scope cell shows its own row (24 P28-20 rows become `NotApplicable(NoRadialEngagement)`, 8 P28-10 rows name P28-10); (b) the 35 stated rows become value checks of the stated cut, as 10h did for deflection. Predictions first |
| 7 | No sample gate for the vendor depth table, the rubbing floor and the plunge rate | `tool_load/groups.rs:357-379` (`gate_of` gives `&[]`) | Judged before the simulation only. The folder file names row `P28-27` (`limits/CLAUDE.md` line 30), but `P28-27` is "the tip plunge cap" (`exemptions.rs:299-307`): the gap has no row | **11**: three `MetricEvaluator`s from existing functions (new findings: operator question O2). If the lead says no, the gap gets its own row |
| 8 | `Caveat::BeforeModulation` gaps | `P29-10i_DESIGN.md` section 14 | The link and the peck ladder get no caveat; bandless and own-material moves are not capped | **Out**: the modulation design step. Only the missing test of the disabled-op rule is **11** |
| 9 | `PlanFacts::NONE` at `operation_feeds_hints` and the gate context | `feeds/suggest/apply.rs:1631`, `tool_load/context.rs:254` | Both read `BodyDepth::speeds_mm()`, which is `None` on every arm that a plan fact fills (`feeds/planned.rs:339-351`): no number differs (reasoned) | **L**: a door with no facts argument (`body_speeds_depth_of`), so `NONE` has no product reader |
| 10 | The new-op context has no stock slot | `feeds/CLAUDE.md`, `NewOpSlots::context` | The stock reaches Suggest through `StockContext` | **Out**: the clean-up pass (no limit reads the slot) |
| 11 | The marker link of `CutFacts::depth_rule` | `limits/CLAUDE.md` line 28 | A read with no marker is not found | **L** (section 5) |
| 12 | One rubbing rule | `calculator/close.rs:131-182` (Step 9b), `planned.rs:1176-1186` (`RubbingFloorPlanned`), `efficiency.rs:193, 267` (ploughing share), part B 4.4 | One floor value (`min(0.025, band min)`, `limits/mod.rs:76`) but two statistics before the simulation: Step 9b warns on the COMMANDED advance `feed / (rpm x Z)`; the planned row judges the PEAK chip of each declared cut. After the simulation no gate reads the floor (item 7); the chip gate's low side reads the band minimum. The ploughing share is an energy fraction, not a floor | **11**: Step 9b becomes a projection of the body `RubbingFloor` row (peak chip); 11-8 gives the sample gate the same floor and statistic. Suggest warnings move (fewer: the peak chip is at or above the advance) |
| 13 | Cusp height of a stated stepover | `from_static_checks.rs:489-528`; part B 4.6 | A fifth inline copy of the chord law (`ball_r - sqrt(ball_r^2 - (s/2)^2)` with `diameter / 2`), shown only as a hint above an unsourced 0.1 mm, only for ball tools under `UiProcessRole::Finish`. Part B: the chord law has four other copies with different radius reads; no surface shows the cusp of a stated stepover on Spiral, Horizontal, Steep/Shallow, Radial, Ramp, Waterline, Pencil | **L**: the hint calls `finish::scallop_math::scallop_height_flat` at the tool's cusp radius (byte-equal where the radius is equal; a tapered ball is not checked); its role scope is class Q. One chord law and a cusp display for every finisher: **Out**, the clean-up pass and the UI work (a display, not a limit) |
| 14 | One reached-feed measure | `machine/kinematics.rs:1397-1447` (`reached_feed_share`), `machine/kinematic_utilization.rs:311-335` (`utilization`); part B 4.5 | Two definitions of one fact: the share counts every fed move (entries, leads, links) and only MCP `get_suggest_rationale` shows it; the utilisation is a time-weighted achieved feed on the load verdict and the op-list pill. Neither is a limit of a cut | **Out**: the modulation design step (the reach is a modulator binding, `BindingConstraint::KinematicReach`); a display number moves when the two become one |
| 15 | The `Capabilities` metric flags | `capabilities.rs:59-111`; part B line 217 | Flags that decide a LIMIT, each already held by a row: `adaptive_woc` (`P28-08`), `move_count` (`P28-09`), `axial_envelope` (`P28-25`), `lut_route` (`M6`), `planner_uniform_engagement` (`M10`). Flags that decide a METRIC: `air_cut_high_threshold_pct` (40 / 45 / 60 / none, "judgement on a cluster of nine", part B line 134), `flat_floor_clearing` (leaves out 2D Adaptive, part B line 153), `doc_basis`, `air_cut_hint`, `relink_dial` (wording) | **L**: class D on the guard allow-list. The limit flags go with their rows (Out, P25 / P27). The air-cut bars have no source: **Out**, P27. `flat_floor_clearing` on 2D Adaptive: **Out**, the clean-up pass |
| 16 | `CriterionStatus`, `Placement`, `JudgedValue`, `criteria()` | `P29-10_DESIGN.md` question L5 | Old records beside `Check`; the export gate reads them | **L**: projections with no judgement of their own; a record with no reader goes |
| 17 | Diagnostic and load-policy messages format their own numbers | `diagnostics/adapters/from_tool_load.rs`, `gcode` advisories (`P29-10_DESIGN.md` L9) | A second writer of a limit face | **L**. FM1 holds ids and severities, not messages (`P29-10_DESIGN.md` 13.7) |
| 18 | `P28-16`, `P28-21`, `P28-29`, `P28-30` | `limits/exemptions.rs:200-335` | The modulator side. `P28-29` still says "P29-6", but commit 6e was held (`8413af726`) | **Out**: the modulation design step. `P28-29` text fixed in 11-0 |
| 19 | `U-PerimeterSweep`, `U-EntryFallback` | `exemptions.rs:444-472` | A generation fact | **Out**: stay open, the reason is on the row |
| 20 | Width rules (`P28-05`, `P28-06`, `P28-08`, `P28-13`, `P28-14`, `F-OffsetRing`) | `feeds/envelope.rs:2069`, `compute/catalog.rs` | No printed width rule | **Out**: P27 |
| 21 | `P28-09`, `P28-19`, `P28-25`, `P28-27`, `P28-28`, `P28-31`, `P28-33`, `M6`, `M10`, `F-ThinsChip` | `exemptions.rs` | Each needs a source or a model | **Out**: P25 / P27, as each row says |
| 22 | The power bar is not the body power row | `ui/feeds/compare.rs:1222-1237` | 30 of 38 ops differ (the bar is the worst declared cut at the applied cut) | **Closed**: both are functions of declared cuts (`planned::worst_power`); the doc says why |

## 2. The other metrics

Source: a read-only survey of core, viz, MCP and CLI (an Explore agent; I checked these
citations: `efficiency.rs:207`, `compare.rs:90-93, 1222-1237`, `from_static_checks.rs:41-44,
463, 492`, `cut_force.rs:313`, `session/compute/diagnostics.rs:216-220`, `efficiency.rs:396`).
"Cut facts" = `ctx.body_facts()` or the headline group, both from `OperationConfig::cuts`.

| Metric | Computed at | From | Op type or role decides? | End state |
|---|---|---|---|---|
| Chip, power, deflection cards (histograms) | `tool_load/distribution.rs:751-768` | headline-group samples; lift from `body_facts().thins_chip` | cut facts only | stays (`P29_FINDINGS.md` 4.4: built from per-sample checks) |
| Force card, `CutForceReading`, over-budget dots | `distribution.rs:497-592`, `cut_force.rs:68-88, 291-326` | samples, `deflection::sample_engagement`; budget from the BODY depth rule for every group (`cut_force.rs:313`) | cut facts | F6-a: the depth row is the record; the reading has no state of its own |
| Force plane | `cut_force.rs:233-261`, `force_field.rs` | tool, material, feed, budget role | `scope(ForceBudget, headline_group)` | stays (a law over a grid) |
| Engagement and depth distributions | `distribution.rs:670-716, 769-777` | sample arc, `depth_number` | cut facts | stay (no GUI card) |
| Air cut %, low engagement %, skin share | `simulation_cut.rs` (~669), `stock/analysis.rs:58-71` | samples, fixed classes 0.02 and 0.10 | none | stays (a sample statistic, not a cut limit) |
| Air-cut offender | `session/compute/diagnostics.rs:216-250` | summary against `capabilities().air_cut_high_threshold_pct` | **yes, `Capabilities`** (which ops are judged, and the bar) | L: class D. The bar's value: Out, P27 |
| Hotspots, span aggregates, heat map | `stock/analysis.rs:316-373`, `ui/simulation.rs:824-862`, `tool_load/display.rs:184` | samples | none | stay |
| Op-list issue flags (rapid, holder, hotspot) | `ui/sim_op_list.rs:1125-1143` | issues | none | stay (load flags are already `CheckRow`, `:1145-1180`) |
| Kinematics pill, `reached_feed_share` | `sim_op_list.rs:1054-1105`, `kinematics.rs:1422-1446` | kinematic utilisation | none | Out (item 14) |
| Suggest chip verdict, wear and time ratios | `feeds/efficiency.rs:164-273` | `OperationConfig` + `FeedsResult`; lift via `RadialThinning::of(family, role)` | **yes, direct** (`efficiency.rs:207`) | 11 (item 4) |
| Force headroom phrase | `efficiency.rs:375-398`; `compare.rs:1013-1021` | `predict_peak_deflection_um(op)`, flat `EXCEEDS_BOUND_MM` | **yes, direct**: refused on a Drill family (`predict.rs:307-316`) | 11 (item 4): the cut's deflection limit; the drill test reads the declared body kind |
| Power bar | `compare.rs:1267-1312`, `operating_point.rs:383-403` | declared cuts, `planned::worst_power` | cut list | closed (item 22) |
| MRR, three forms in two units | `compare.rs:698-704` (ap x ae x f), `calculator/close.rs:399-405` (section x feed), sample `mrr_mm3_s` (mm³/s) | op, `FeedsResult`, sample | none | Out: the clean-up pass (one MRR function of `CutLoad`; a display number moves on a V-bit and a ball, where section != ap x ae) |
| Scallop override and derived stepover | `compare.rs:90-93, 1554-1662` | scallop height, tip radius | **yes, direct**: DropCutter only | L: a `Capabilities` flag (`supports_scallop_override`), byte-equal |
| Rubbing-floor row, envelope `rubbing` flag | `close.rs:131`, `why.rs:855-875`, `envelope.rs:876-879` | `FeedsResult` | none | closed (item 12) |
| MCP `per_depth_pass` | `app/mcp/simulation.rs:1017-1110` | samples by span path; a field named `peak_axial_doc_mm` (its writer not traced) | none | 11 with M4-A if it reads `axial_doc_mm` |
| Narration: peak depth warning | `narrate.rs:50, 1840-1957` | samples, `DEEP_DOC_MULTIPLIER` | `Capabilities::doc_basis` (wording); suppressed on a drill (`:1850`) | 11 (item 5) |
| Narration: air-cut line, drill block, engagement buckets | `narrate.rs:2141-2196, 1969-2083, 725-784` | summary, samples | `Capabilities`, `is_drill_kinematics` (`:224-229`) | L: class D (wording and the drill block) |
| Triage: standing material, link cut, entry load, plunge class | `stock/sim_triage.rs:1006-1054, 1669-1763, 1391-1470, 1851-` | samples; own intent predicates, not `CutKind::of_sample` | none | Out: the clean-up pass (one classifier; a finding moves only where the two classifiers disagree, not measured) |
| Static checks: finish stepover > 50 %, ball cusp height | `from_static_checks.rs:463, 489-528` | op stepover, tool | **yes, role** (`UiProcessRole::Finish`) | L: class Q (item 13) |
| Static checks: ball on a flat floor, depth past the vendor table, depth > cutting length | `from_static_checks.rs:244, 541, 575-` | op, tool | `Capabilities::flat_floor_clearing` / none | stay; the vendor-table and length checks duplicate `VendorDepthTable` and `CuttingLength` rows: Out, the clean-up pass |
| Stale adaptive stepover | `compute/validate.rs:284-` | stepover against 0.15 D | `Capabilities::adaptive_woc` (row `P28-08`) | Out (P28-08) |
| Cycle time | `cycle_time.rs:368-400` | trace or distance / feed | none | stays |

**Reads of op type or role that decide a metric value or its scope, outside `cuts()`** (the
P29-last list for metrics): `feeds/efficiency.rs:207`, `feeds/predict.rs:307-316`,
`ui/feeds/compare.rs:90-93`, `from_static_checks.rs:463, 492`,
`session/compute/diagnostics.rs:216-220`, `trace/narrate.rs:224-229, 1850, 1884, 2142-2143`. That
is 6 files, 10 sites. Every simulation card reads only the cut facts.

## 3. M4, numerically

### 3.1 The two depths

| Field | Writer | Meaning | Readers |
|---|---|---|---|
| `axial_engagement_mm` | the stamp height (`dexel_stock/simulation.rs:1353-1372`; 0 on a pure plunge), then the MAXIMUM over a coalesced group (`sample_coalesce.rs:260`) | the largest height of stock the cutter stands in during the sample | depth gate (`depth.rs:181, 259`); deflection and force (`deflection.rs:189`); the modulator's deflection and force caps (`feed_modulation.rs:405`); triage (`sim_triage.rs:1024-1034`) |
| `axial_doc_mm` | the same stamp value (`simulation.rs:1371`), then the TIME MEAN over the group (`sample_coalesce.rs:238, 268`) | the mean height | power and the chip lift (`engagement.rs:242`); the depth gate's width (`depth.rs:477`); the modulator's power cap (`feed_modulation.rs:404`) and its per-move mean (`feed_modulation.rs:363`: the zero test, the planner floor, the fallback); narrate (`narrate.rs:1863, 1917`) |

The coalescer folds a move only when its Z subdivision gives more samples than the sample step
(`sample_coalesce.rs:17-22`). So the depths differ only on moves that descend or climb: ramps,
helices, sloped 3D passes.

### 3.2 Where the depths differ (measured)

Fixtures: the five of `common::kind_fixtures` (6 mm flat end mill, modulation off), and three
of mine on a dome (`z = 4 + 6 (1 - (r/30)^2)` mm, 80 x 80 mm, cell 0.25 mm): a 6 mm ball
geodesic 3D rough (stepover 2, depth 3), a 6 mm ball drop cutter (stepover 1) and a tapered
ball (1 mm tip, 7°, 6 mm shank) drop cutter (stepover 0.5). Run time 65.9 s.

| Fixture | Group | Removing | Depths differ | engaged − removed p50 / p95 / max (mm) | ratio p50 / max |
|---|---|---|---|---|---|
| Pocket (ramp), Pocket + dogbone | Body / Cleanup | 1204 / 120 | 0 | - | - |
| Pocket (ramp) | Entry | 80 | 40 (50.0 %) | 0.081 / 0.464 / 0.464 | 1.66 / 9.0 |
| Profile (tabs, leads) | Body, Entry, Link | 905, 52, 14 | 0 | - | - |
| 2D Adaptive (helix) | Body, Link | 3228, 751 | 0 | - | - |
| 2D Adaptive (helix) | Entry | 571 | 117 (20.5 %) | 0.217 / 0.306 / 0.318 | 1.50 / 2.0 |
| Shelf 3D rough (flat) | Body | 7089 | 161 (2.3 %) | 0.232 / 2.941 / 4.549 | 1.25 / 12.0 |
| Shelf 3D rough (flat) | Cleanup | 1327 | 169 (12.7 %) | 0.259 / 1.945 / 3.572 | 1.34 / 12.0 |
| Shelf 3D rough (flat) | Entry | 459 | 238 (51.9 %) | 0.073 / 0.485 / 1.423 | 1.13 / 10.0 |
| Dome 3D rough (ball) | Body | 27498 | 4024 (14.6 %) | 0.040 / 0.158 / 0.757 | 1.09 / 4.0 |
| Dome drop cutter (ball) | Body | 27439 | 4610 (16.8 %) | 0.101 / 1.648 / 2.786 | 1.49 / 4.0 |
| Dome drop cutter (taper) | Body | 62286 | 11288 (18.1 %) | 0.267 / 1.580 / 1.805 | 2.00 / 3.0 |

The engaged depth is not the smaller (reasoned: a maximum against a time mean of the same
values; the probe printed no minimum). On a flat tool the width at both depths is the same; on
the ball and the taper the width differs too (3760, 3468 and 10231 samples; ball rough `ae`
delta p50 0.081 mm, max 1.20 mm).

### 3.3 What each row reads with each depth (measured)

Method: the session door `limit_rows(0, Some(trace), Off).verdict()` on three traces: as
simulated ("base"), with `axial_doc_mm := axial_engagement_mm` ("A"), and with
`axial_engagement_mm := axial_doc_mm` ("B"). 21 group rows per fixture, 168 in all.

| Fixture | Row | Base | A (engaged) | B (removed) |
|---|---|---|---|---|
| 2D Adaptive | Entry power | 0.01066 kW, 1.3 % | 0.01213 kW, 1.5 % | as base |
| 2D Adaptive | Entry depth | 0.782 of 16.60 mm, 4.7 % | as base | 0.795 of 17.39 mm, 4.6 % |
| Shelf 3D rough | Entry power | 0.02623 kW, 3.3 % | 0.02907 kW, 3.6 % | as base |
| Shelf 3D rough | Entry deflection | 5.90 um, 2.9 % | as base | 5.77 um, 2.9 % |
| Shelf 3D rough | Entry depth | 2.558 of 19.60 mm, 13 % | as base | 2.521 of 22.69 mm, 11 % |
| Dome drop cutter (ball) | Body power | 0.0603 kW, 7.5 % | 0.0765 kW, 9.6 % | as base |
| all 162 other rows | - | - | as base | as base |

A moves 4 rows (power, up); B moves 3 (deflection and depth, down). No state moves; every row is
`Within`. The force reading reads the engaged edge today (`cut_force.rs:79-80`).

### 3.4 The modulator (measured)

Three fixtures simulated with modulation ON. Modulated moves by binding (moves; of them, moves
with a sample whose depths differ):

| Fixture | Bindings |
|---|---|
| Pocket | FeedCap 8; 0 · KinematicReach 26; 15 · MachineMaxFeed 26; 0 |
| Dome 3D rough (ball) | FeedCap 467; 77 · KinematicReach 1462; 417 · MachineMaxFeed 384; 9 · PlungeRate 2; 0 |
| Dome drop cutter (taper) | ChiploadMax 25438; 7110 · KinematicReach 482; 0 |

No move binds on `PowerMax`. The chip cap reads `CutLoad::thinning_lift`, which is
`RadialThinning::lift(ae, d_eng)` (`cut_load.rs:456-458`, `geometry.rs:74-79`). In
`Engagement::from_sample` both `ae = r (1 - cos psi)` and `d_eng = 2 r` scale with the radius
at the depth (`engagement.rs:243-246`), so `ae / d_eng = (1 - cos psi) / 2` holds no depth, on
every tool (reasoned; measured: 0 chip rows moved on the ball rough and the taper finish under
A). So the power cap is the only modulator cap that M4-A changes (the deflection and force caps
read the engaged edge already). The operator file was not simulated. `P29_FINDINGS.md` line
175 gives 2.68 kW required for an 18 mm Contour Parallel rough at 10 x 5.5 mm (a Suggest
figure, not a modulated run); so the power cap can bind on the operator's 18 mm roughs
(reasoned).

### 3.5 Options

| Option | What | Rows (fixtures) | FM1 | Exported feed or depth |
|---|---|---|---|---|
| **M4-A** (my choice) | `Engagement::from_sample` reads `axial_engagement_mm`. The power gate, the chip lift, the depth width and the modulator's power cap read the maximum. `axial_doc_mm` keeps one reader, the modulator's per-move mean (renamed `mean_axial_mm`). `SampleDepth` and `Caveat::SampleDepth` go (every site of `rg SampleDepth`: `limits/`, two sentries). `M4` closes | 4 of 168, power up | sim subset (modulation on): `power_peak_kw` can rise on the dome cells (Waterline, Adaptive3d, Scallop, DropCutter); no state moves: the largest power share in `matrix_2026-09-23_sim.csv` is 15.7 % of the available power (Waterline, end mill; measured from the CSV; the FM1 effect is predicted, not run) | **Yes: an exported feed falls where the power cap binds after a modulated simulation.** None on the fixtures; the operator file is not measured |
| M4-A1 | As A, gates only; the modulator keeps the mean | the same | the same | none, but the modulator and the gates read two definitions again, against lead ruling 2 of P20g (`feed_modulation.rs:392-397`) |
| M4-B | One depth, the mean | 3, deflection and depth down | depth and deflection columns can fall | **Yes**: the deflection and force caps loosen. Against "a peak is kept exactly or over-stated, never under-stated" (`sample_coalesce.rs:40-41`). Rejected |
| M4-C | No merge: the operator rules two depths (`Ruled`); the caveat stays | 0 | 0 | none |
| M4-D | A maximum in both fields in the coalescer | every power row of a folded move, and the per-move mean | many | **Yes, broad** (the zero test, the planner floor, the fallback: `sample_coalesce.rs:31-35`). Rejected |

My choice: **M4-A as one commit, held until a wanaka read**. The lead runs one modulated
simulation of the 6 mm working file with the commit and without it, and compares
`modulated_feeds` move by move. If no move binds on power, A is display and verdict only on that
file. M4-A1 is the fallback if the operator does not want an exported feed to move in P29.

## 4. F6: the depth row and the force reading

### 4.1 What differs (measured)

The depth and force cells of each group on the four milling kind fixtures. Every judged cell
agrees in state, except one:

| Cell | Depth gate | Force reading |
|---|---|---|
| 2D Adaptive, Link (751 samples) | `Reported`: "peak engaged depth 0.562 mm; no force-budget limit below the flute; 330 samples with a depth have no measured width (no number)", population 69 | 1.729 N, budget 35.32 N, population 3 |

Two causes, both in `tool_load/depth.rs`:

1. **"No root below the flute" gives `NoNumber`.** When every judged sample has no force-budget
   root below the flute, the gate returns `Reported` (`depth.rs:332-345`), and the row shows
   `NoNumber(NoBound)`. But a force at the flute length under the budget proves `Within`: no
   depth the tool can cut reaches the budget.
2. **A second width door.** `sample_width_mm` takes the arc width, else the width share
   `radial_woc_fraction x 2 r` (`depth.rs:476-485`), a time mean of a coalesced group
   (`sample_coalesce.rs:237`). The force reading reads the arc only (`deflection.rs:184-190`).
   Depth samples that take the share (measured): 2D Adaptive Body 9 of 3117, Link 66 of 399;
   Pocket Entry 6 of 74; Shelf Body 445 of 6888, Cleanup 78 of 1269, Entry 4 of 387, Link 2 of
   84; Profile 0.

The cells that agree in state still differ in population for the same reason (Pocket entry
66 against 60; Shelf entry 333 against 329; measured).

### 4.2 Options

| Option | What | What moves |
|---|---|---|
| **F6-a** (my choice) | (1) `NoLimitBelowFlute` on every judged sample gives `Within` with the flute length as the bound. (2) The depth gate takes its width from the force reading's door, `deflection::sample_engagement` (the arc at the engaged depth, the depth the gate already judges). The width-share fallback goes; a sample with a depth and no arc is "no measured width (no number)", counted, as P29-6 6c did for the full-slot fallback. This holds under M4-A, M4-A1 and M4-C: the gate's depth and width then come from one door whatever `from_sample` reads | Row states: 2D Adaptive Link `NoNumber` to `Within` (measured cell). The depth population becomes the force population (measured counts of samples with a force engagement: 2D Adaptive Body 1998, Entry 366, Link 3; Pocket Body 1112, Entry 60; Profile 791, 21, 6; Shelf Body 4425, Cleanup 912, Entry 329, Link 12). On a ball or taper the width of a folded sample moves (the radius at the engaged depth). A judged sample can change where a share sample held the worst ratio (not measured). No exported number: the depth row does not gate (`BoundSource::MachineForceBudget`, `depth.rs:39-46`) and the modulator reads its own caps |
| F6-b | One `ForceBudget` row whose value is the force (N) | Against ruling 3 of `P29-10_DESIGN.md` section 14 (a depth row compares depths). Rejected |
| F6-c | Keep both (section 14 ruling 1) | none; the `NoNumber` stays where a proof exists |

After F6-a the depth gate and the force reading read one engagement door. A sentry
claim holds it: in every fixture group, the two populations are equal, and the depth row is
`Within` exactly when the force peak is at or under the budget.

## 5. P29-last: the guard

### 5.1 What it forbids

A read of `OperationType`, `OperationFamily`, `PassRole`, `UiProcessRole`, `OperationConfig::<Variant>`,
`op_type()`, `feeds_style()`, `pass_role`, `.role()`, `depth_rule`, `body_depth_limit`,
`depth_cap_of` or `.capabilities()` in a limit or metric file. The files: `feeds/`,
`tool_load/`, `limits/`, `dressup/feed_modulation.rs`, `session/limit_rows.rs`,
`session/load_report.rs`, `session/compute/diagnostics.rs`, `trace/narrate.rs`,
`diagnostics/adapters/`, `machine/mod.rs`, `compute/simulate.rs`; viz `ui/feeds/`, `ui/sim_*.rs`,
`state/simulation.rs`, `app/mcp/`; the CLI. The viz scan of a `match` on a limit, group or state
stays as it is. `session/compute.rs` and `session/compute/generation.rs` are not guarded (they
hold generation); their sites are in 5.4 for completeness.

### 5.2 How it avoids false hits, and how its list only grows by a declaration

- It reads code through `tests/common/source_scan.rs` (comments out). String text and
  `#[cfg(test)]` modules go too; files `tests.rs`, `*_tests.rs` and `tests/` are not read.
- Non-vacuity: it asserts that the declaration file `compute/catalog/cuts.rs` holds a hit, and
  that a planted line in a fixture string is found.
- The classes of an allow entry (closed list): **V** the vendor row key (`M6`, P25/P27); **W**
  the write funnel and the op draft; **D** the declaration (`cuts()`, `ConfiguredEntry`, the
  registry `Capabilities`, the move model, precondition dispatch); **G** generation planning;
  **Q** a quality hint by role (not a limit); **T** a label or a report field; **X** a scope read
  that an `EXEMPTIONS` row holds (the entry names the row id).
- **Two kinds of entry.** A dispatch entry (V, W, D, G, Q, T) names a file and a function, with
  NO count: a new op adds one arm to `apply.rs` or `from_preconditions.rs`, and the compiler asks
  for that arm, so a count there would block every new op. An X entry names a file, a function,
  the row id and an EXACT count. The test fails when an X count is above or below its entry, and
  when the sum of X counts is above a pinned total; that pin only goes down. An X entry and its
  row close together.
- **Zero tolerance** (no entry of any class may name them): the gate files of `tool_load/`
  (all but the row-key module), `limits/`, `feeds/planned.rs`, `feeds/cut_load.rs`,
  `feeds/engagement.rs`, `feeds/calculator/`, `dressup/feed_modulation.rs`,
  `session/limit_rows.rs`, the viz `ui/sim_*.rs` and `state/simulation.rs`. These are the files
  where a limit is computed or shown; after L-1 and L-2 they hold no hit (S1, S2 move the drill
  tests and the row key out).
- The `EXEMPT(..)` marker stays for X entries; the guard replaces its weak half (a read with no
  marker is now a hit).

### 5.3 Hits today (measured, `scan.py`, the 5.1 pattern and core files)

Core: **402 hits, 45 files**. The larger files:

| File | Hits | Class |
|---|---|---|
| `feeds/suggest/apply.rs` | 93 | W (the funnel matches each op to write its fields) |
| `feeds/mod.rs` | 53 | V (`FAMILY_RULES`, `LutRoute`) and X `P28-31` (default cut table) |
| `feeds/envelope.rs` | 35 | 25 peck match (S5), 3 depth rule (S10), 1 X `P28-05/06`, the rest T and imports |
| `feeds/vendor_normalize.rs` | 30 | V |
| `diagnostics/adapters/from_preconditions.rs` | 26 | D (which precondition checks run) |
| `diagnostics/adapters/from_static_checks.rs` | 13 | Q, T |
| `tool_load/chipload.rs` | 11 | V (chip row key) |
| `feeds/input.rs` | 10 | V fields and 1 drill test (S1) |
| `feeds/support.rs` | 10 | V |
| `trace/narrate.rs` | 10 | D (`Capabilities` wording, drill block), T |
| `feeds/suggest/adaptive_entry.rs` | 9 | X `P28-28` |
| `machine/mod.rs` | 9 | the depth-rule table (S4) |
| `tool_load/context.rs` | 8 | V (row key, S2) and D (`cuts()` call) |
| `session/compute/diagnostics.rs` | 8 | D (air-cut bar) |
| `feeds/suggest.rs` | 7 | T and W (`default_operation`) |
| `feeds/predict.rs`, `feeds/ramp.rs` | 6, 6 | D (move model; Adaptive3d entry), S11, V (home cell), X (`ramp.rs:985`) |
| 28 other files | 1 to 5 each, 58 in all | listed in `p29_11/guard_counts_core.txt` |

Viz display, MCP and CLI: **33 hits, 10 files** (`rs_cam_cli/src/suggest.rs` 7, `run.rs` 6,
`smoke.rs` 5, `job.rs` 4; viz `compare.rs` 2, `app/mcp/*` 8, `ui/feeds/vendor.rs` 1; plus 2 in
the test file `explore_tests.rs`, not counted). The viz editor files (registry 69, `surface_3d`
57, …: survey counts) are editor dispatch and are not in the guarded set. The CLI hits are op
parsing and labels (class W, T).

### 5.4 The scope sites, what replaces each, and whether a number moves

| # | Site | Today | Replacement | Rule kind | Number moves? |
|---|---|---|---|---|---|
| S1 | `input.rs:278`, `plunge.rs:430`, `ramp.rs:417`, `calculator/spindle.rs:163`, `feed.rs:237`, `close.rs:248` | `operation == OperationFamily::Drill` | `FeedsInput::body_kind() == CutKind::Drill` from `cuts()`; the family and role fields become a private `RowKey` that only the vendor modules open | **type** (`RowKey` private fields) | no (reasoned: the two ops of the Drill family, `registry.rs:631, 1565`, declare a `Drill` body, `cuts.rs:661-668`) |
| S2 | `tool_load/context.rs:52, 234` `operation_kind` | op type in the gate context | into the opaque `RowKey` | **type** | no |
| S3 | `CutFacts` (`limits/cut.rs:427`, public fields) | any code can build facts | private fields, getters, one constructor that only `cuts.rs` calls (scan on the constructor) | **type** + scan | no |
| S4 | `machine/mod.rs:282-297` | the depth rule by (family, role, sides) | moves into `compute/catalog/cuts.rs` | move | no |
| S5 | `envelope.rs:4072-4108` | the peck by op variant | the drill cut of `cuts()` carries its peck | move | no |
| S6 | `geometry.rs:59-63` `RadialThinning::of(family, role)` | the thinning rule | moves into `cuts.rs`; readers take `CutFacts::thins_chip` (after 11-7 the efficiency card too) | move | no (`F-ThinsChip` stays open: the rule is still the label) |
| S7 | `compute/simulate.rs:317` | Drill or Pin Drill op type, third of three tests | the declared body kind | move | no |
| S8 | `ui/feeds/compare.rs:90-93` | DropCutter only | `Capabilities::supports_scallop_override` | D | no |
| S9 | `apply.rs:1631`, `context.rs:254` | `PlanFacts::NONE` | `body_speeds_depth_of(op, hint)` | type (no facts argument) | no |
| S10 | `envelope.rs:3251, 4971`, `force_budget.rs:522`, `session/compute.rs:2289`, `depth.rs:167, 453, 525`, `cut_force.rs:220` | the BODY depth rule for every cut and group | a fact of each declared cut and each group (`SampleContext` gives the group's facts) | fact read | no today (every cut holds the body's rule, `cuts.rs`) |
| S11 | `efficiency.rs:207`, `predict.rs:307-316` | role and Drill family | the cut facts (11-7) | 11 | yes, display (item 4) |
| S12 | `narrate.rs:1917` | own threshold | the depth row (11-6) | 11 | text |
| X | `envelope.rs:2070` (`P28-05/06`), `ramp.rs:985` (straight-plunge note by role), `adaptive_entry.rs`, `invariants.rs:152`, `geometry_class.rs` (`P28-28`), `feeds/mod.rs` default profile and `calculator/cut.rs:16` (`P28-31`) | scope by role | stay, allow-listed with their row | X | - |
| G, Q, D | `generation.rs:582, 628`; `from_static_checks.rs:463, 492`; `diagnostics.rs:216`, `narrate.rs:224, 1884, 2142` | planning, hints, registry wording | stay, allow-listed | G/Q/D | - |

The `ramp.rs:985` note has no exemption row today; 11-0 adds it to `P28-28` (the same family of
entry warnings) or the lead names a row.

## 6. Commits

Byte-equal commits first. Each behaviour commit stands alone with its predictions written
before the code. "FM1" = `feeds_matrix_instrument_fm1`; "golden" = the op golden.

| Id | What | A number moves? | Tests and sentries | Predictions |
|---|---|---|---|---|
| 11-0 | Text: `P28-29` names the modulation design step; the sample-gate gap gets the right row text in `limits/CLAUDE.md`; `ramp.rs:985` gets its row; a test of the disabled-op rule of `Caveat::BeforeModulation` | no | `the_exemption_list_only_shrinks_p29_3`, `one_record_on_every_surface_p29_10` | none |
| 11-1 | One `OperatingPoint::of_sample` (item 3); a debug assert that a product sample's flute count is the tool's | no | `one_cut_gives_one_load_on_every_door_p29_1` (claim C moves to a synthetic test) | FM1 byte-equal |
| L-1 | S4, S5, S6, S7, S8 (moves into `cuts()` and `Capabilities`) | no | `every_op_declares_its_cuts_p29_3`, `every_limit_judges_every_declared_cut_p29_7`, FM1, golden | byte-equal |
| L-2 | S1, S2, S3, S9 (type rules: `RowKey`, private `CutFacts`, the no-facts depth door) | no | the same, viz `--lib` | byte-equal |
| L-3 | The guard test `no_limit_reads_the_op_type_p29_last` with today's allow-list after L-1 and L-2 | no | itself; inject one read, see red | none |
| 11-2 | `S-NotJudgedHere` (a): a scope cell shows its own row | row states only: 24 operator rows `NoNumber` to `NotApplicable`; 8 name `P28-10` | `one_record_on_every_surface_p29_10`, the stated-point sentry | the row list per file (counts measured: section 1 item 6) |
| 11-3 | `S-NotJudgedHere` (b): value checks of the stated cut for the 7 limits | row states; a new face line is possible (operator question O3) | the same + a claim per limit | each of the 35 operator rows and the 13 of each rivmap file with its value, bound and state, before the code. The fixed-axis ops (operator: Project Curve 12 rows, Scallop 4) have no stated depth before the simulation: they get `NoNumber(OnlyALowerBound)` or `NoNumber(NoEarlierLeave)` (existing reasons), not a number. Closes `S-NotJudgedHere` only if no row is left without a value function or a named reason |
| 11-5 | F6-a | row states and populations (depth rows) | `the_depth_that_cut_is_a_measured_load_g_s3depth`, `no_removing_sample_is_outside_the_gates_p29_6`, the F6 claim | the 2D Adaptive Link row; population counts of section 4.1 |
| 11-6 | Narrate reads the depth row; `DEEP_DOC_MULTIPLIER` goes | text | narrate tests | the warning lines on the five fixtures |
| 11-7 | Efficiency through the cut (item 4); closes `U-efficiency` | Suggest card numbers (ball, taper, leave < 200 um; operator question O4) | `suggests_own_recipe_is_not_above_the_band_p11b2`, compare tests | the chip verdict and headroom of every FM1 row with a ball or taper tool (FM1 holds no efficiency column: reasoned) |
| 11-8 | Sample gates for `VendorDepthTable`, `RubbingFloor`, `PlungeRate` (question O2) | new findings; FM1 `diagnostic_ids_after_sim` can move | `every_limit_judges_every_declared_cut_p29_7`, P29-6 sentry | the new findings on the five fixtures and the FM1 sim cells, before the code |
| 11-9 | One rubbing rule (item 12): Step 9b reads the body `RubbingFloor` row (peak chip) | Suggest warnings: `ChiploadBelowRubbingFloor` fires on fewer cuts (operator question O5); FM1 cells that hold the warning id can move | `rubbing_floor_warns_and_never_lifts`, FM1 | the FM1 cells with the warning today and after, before the code |
| 11-4 | M4-A, after the wanaka read | **yes, an exported feed where power binds**; power rows | the M4 claim (one depth for every door), FM1 re-bless of the moved cells | the 4 fixture rows of 3.3; the FM1 dome cells; the 6 mm working file move list |
| L-4 | Old records as projections (item 16) and one writer of each message (item 17) | text of messages | `a_criterion_carries_its_own_bound_g_s4bound`, export tests, FM1 | message diff per fixture |
| L-5 | X counts and the X total lowered to the final state; the folder files say the guard is the rule | no | the guard | none |

## 7. Exemption pins

Pins now: `ROWS` 37, `MAX_NEEDS_RULING` 26, `ROWS_WITH_A_FILE` 37 (`the_exemption_list_only_shrinks_p29_3.rs:51-99`).

Each line holds only if its commit lands as designed.

| Commit | Row | Pins after |
|---|---|---|
| 11-3 | `S-NotJudgedHere` closes (if every stated row gets a value function or a named reason) | 36 / 25 / 36 |
| 11-7 | `U-efficiency` closes | 35 / 24 / 35 |
| 11-4 (M4-A) | `M4` closes (M4-C: `Ruled`, NR 23 but rows 35) | 34 / 23 / 34 |
| 11-8 refused | a new row for the sample-gate gap | +1 each |

**What P29 leaves open, and why** (34 rows if every line above lands):

| Rows | Count | Why it stays | Owner |
|---|---|---|---|
| `P28-04`, `P28-07`, `P28-17`, `P28-20`, `P28-24`, `P28-26`, `P28-30p`, `M9` | 8 | Physical: no radial engagement, not an offset ring, no vertical feed | stays |
| `P28-02`, `P28-18`, `S-NoBound` | 3 | Operator rulings (R2 2026-09-23; P11 L1/L2) | stays |
| `P28-16`, `P28-21`, `P28-29`, `P28-30` | 4 | The feed modulator's own caps and finding | the modulation design step |
| `P28-05`, `P28-06`, `P28-08`, `P28-13`, `P28-14`, `F-OffsetRing` | 6 | No printed width rule | P27 |
| `P28-09`, `P28-19`, `P28-25`, `P28-27`, `P28-28`, `P28-31`, `P28-33`, `M6`, `M10`, `F-ThinsChip` | 10 | A model or a source does not exist (move model, ball axial thinning, vendor axial bound, tip plunge cap, plunge deflection, default cut, V-bit tip, row by op kind, planner floor, finish thinning) | P25 / P27 |
| `U-PerimeterSweep`, `U-EntryFallback` | 2 | Generation facts (the grow refuses; a helix with no room ramps) | a generator change, outside P29 |
| `P28-10` | 1 | Drill power as a limit: no end-cut force source | P27 |

## 8. Questions

For the operator (each changes what they cut with or see by default):

- **O1. M4-A.** The power reading of a sloped or ramped sample reads the deepest part of the
  sample, not the mean. On a modulated run this can lower an exported feed where the spindle
  power is the binding cap. On my fixtures no move changes. Accept, or keep the gates and the
  modulator apart until the modulation design step (M4-A1)? My choice: accept, after the lead
  shows the 6 mm working file move list.
- **O2. New sample findings (11-8).** After a simulation the gates also judge the vendor depth
  table, the rubbing floor and the plunge rate (warnings only, like every gate). Accept? My
  choice: yes.
- **O3. Stated-cut rows (11-3).** The Limits drawer judges the stated cut against 7 more limits
  (cutting feed cap, cutting length, engagement floor, flute guard, move budget, tool diameter,
  vendor axial bound); a new caution line can show on the card. Accept? My choice: yes.
- **O4. The Suggest efficiency numbers (11-7).** On a ball or tapered tool the chip verdict and
  the force headroom read the engaged diameter, and the headroom reads the stock to leave when
  it is under 200 um. The card numbers change on those ops. Accept? My choice: yes.
- **O5. The rubbing warning (11-9).** Suggest warns about rubbing on the peak chip of the cut,
  as the Limits rows do, not on the feed per tooth. It warns on fewer cuts. Accept? My choice:
  yes.

For the lead (each with my choice):

- **L1. F6-a:** the depth row becomes the one force-budget record; "no root below the flute"
  is `Within`; the width-share fallback goes. Choice: yes.
- **L2. 11-3:** judge the 7 limits at the stated cut as value checks. Choice: yes, with the
  35 + 13 + 13 predicted rows first; the operator sees it (O3).
- **L7. Guard entries:** dispatch classes have no count, X entries an exact count, the
  limit files zero tolerance (5.2). Choice: as stated.
- **L3. Guard scope:** core limit and metric folders, the viz display files, MCP and the CLI;
  not the viz editor files. Choice: as stated.
- **L4. The triage classifier, MRR and the duplicate static checks** go to the clean-up pass,
  not P29. Choice: yes (none is a limit of a cut).
- **L5. The air-cut bar** (40 / 45 / none, `capabilities.rs`) has no source. Choice: P27.
- **L6. Order:** 11-4 (M4-A) last, so that no other commit waits for the operator. Choice: yes.

## 9. Not verified

- The operator file and the 6 mm working file were not simulated: the M4 effect on their
  modulated feeds and gate rows is unknown.
- FM1 was not run. The FM1 effects in section 6 are predictions from the CSV and the code.
- The F6 measure covers the four milling kind fixtures; no fixture sample exceeds the budget,
  so the `Exceeds` side of F6-a is not tested. Whether a width-share sample is ever the judged
  sample was not measured.
- The survey of section 2 was an agent's read; I checked 8 citations. The rows marked
  "reasoned" or "not traced" (the writer of the MCP `per_depth_pass` depth field, the triage rough islands, the sample flute
  count of every door) were not traced line by line.
- The guard counts come from a scan script with a heuristic test-module cut; the guard test
  must use `source_scan.rs` and its counts will differ a little.
- 11-3: not checked that each of the 7 limits has a value function at the stated cut for a bar
  or a fixed envelope; the Project Curve and Scallop rows get a `NoNumber` reason, not a value.
- The cusp hint's radius on a tapered ball (`diameter / 2`) was not checked against the tool's
  cusp radius.
- 11-9: the FM1 cells that hold the rubbing warning were not counted.
- The post-L-2 allow-list counts are not computed; L-3 writes them.

## 10. Lead rulings (2026-11-14)

The operator is away until about 2026-11-15 and gave the lead the calls of P29. The lead brings
the operator in only where a change alters what they cut with.

- **O1 (M4-A): NOT built without the operator.** It can lower an exported feed where the spindle
  power binds. No commit 11-4 in this run. The row `M4` stays open, with its caveat. The lead
  shows the operator section 3 and asks for the choice between M4-A (after a modulated run of the
  6 mm working file, with and without) and M4-A1.
- **O2 (11-8): yes.** A limit with a planned row and no sample gate is a limit that the samples
  skip. Warnings only, no refusal, no feed moves. Predictions first: the new findings on the five
  fixtures and each FM1 cell that moves. If a gate would refuse an export or move a feed: stop.
- **O3 (11-3): yes**, with the predicted rows first and the face line of all 18 operator ops
  before and after. A row with no value function gets a named reason, never a guess.
- **O4 (11-7): yes**, display only. Predictions first: each card number that moves on the four
  project files. If a Suggest PROPOSAL (a feed, an RPM, a depth) moves: stop.
- **O5 (11-9): yes**, on one condition: Step 9b only warns today
  (`rubbing_floor_warns_and_never_lifts`). If the change moves a proposed feed on any FM1 cell or
  on any op of the four files: stop.
- **L1 (F6-a): yes.** The depth gate and the force reading read one engagement door. The sentry
  claim of section 4.2 holds it. The "exceeds" side has no fixture: build one (a fixture sample
  over the budget) so that the claim tests both sides.
- **L2, L3, L5, L6, L7: as chosen.**
- **L4: yes, and commit L-4 goes with it.** The old records as projections and the one writer of
  each message go to the clean-up pass. P29 ends with the guard.
- **Order.** Round 1, byte-equal: 11-0, 11-1, L-1, L-2, L-3. Round 2, each alone with its
  predictions before the code: 11-2, 11-3, 11-5, 11-6, 11-7, 11-8, 11-9. Then L-5. A moved FM1
  cell or golden row that the predictions do not name stops the work.
- **End state.** Pins 35 / 24 / 35 if 11-3 and 11-7 land (`M4` open for the operator). The table
  "what P29 leaves open, and why" of section 7 goes into `phases/P27_BRIEF.md` with the last
  commit.

## 11. Round 2: predicted and measured

Round 2a, written 2026-11-14 at HEAD `c4467a288`. Two throwaway probes, deleted after the round
(copies in the session scratchpad folder `p29_11b/`): `zz_probe_rows_p29_11b` dumps every row of
`ProjectSession::limit_rows` (envelope, RPM, verdict with no trace, the profile) for every
toolpath of the four project files, with modulation off and on; `zz_probe_gates_p29_11b` dumps
the verdict rows, the group record `GroupVerdicts`, the force reading and the depth and force
cards of the five simulated kind fixtures. Each dump ran two times at HEAD and gave the same
bytes. The "before" dumps of HEAD are the baseline of every subsection below.

### 11.1 S10: the depth rule of each cut and group

Prediction: byte-equal. Every declared cut holds the body's facts (`Declare::cut` copies
`self.facts`), so the facts of a group equal the body facts on every op.

The change:

- `RigidityProfile::force_budget_of(CutFacts)` (`machine/mod.rs`) is the one door from the
  facts of a cut to its force budget. `Err` names the `NoDepthLimit`. It holds the one read of
  the depth rule that is left in a guarded file: a class D entry of the guard.
- The envelope reads the facts of the declared cuts that set a ceiling
  (`ceiling_force_budget`: the first such cut with a budget; with none, the reason of the
  first; with no such cut, the body's). `rigidity_limit` and `no_edge_reason` read it.
- `ToolpathLoadContext::group_facts(group)`: the facts of the first declared cut of the group,
  else the body facts (a group that the op declares no cut of, for example the links of most
  ops). The depth gate (`judge`, `vacuous_verdict`), the depth histogram (`depth_number`, the
  headline group) and the force reading (each group its own budget line; `budget_line` is the
  headline group's) read it.
- `force_budget::rigidity_depth_cap` takes the facts of the cut, not a `DepthLimit`.
- The guard: `PENDING_S10` and its pin are deleted; `depth.rs` and `cut_force.rs` have zero
  tolerance and no hit. `session/compute.rs:2289` (the modulator's body rule) is not guarded and
  not in this step; it still calls `depth_cap_of` on the body facts.

Measured: the four project dumps and the fixture dump are byte-equal to HEAD (`cmp`).

### 11.2 Commit 11-2: a scope cell shows its own row (prediction)

Written before the code, from the "before" dump. The rule: a planned or stated row of the
envelope whose scope cell is `NotJudged` (`limits::scope`) takes the class of its own
`EXEMPTIONS` row. A `Physical(geometry)` row gives `NotApplicable(geometry)`; a row that needs a
ruling (or is ruled) keeps `NoNumber(NotJudgedHere)`. The exemption field names the row in both
cases, as it does at HEAD.

Predicted, for each modulation pass (`off` and `runs`) and in the profile line of the dump:

| File | `NoNumber(NotJudgedHere)` to `NotApplicable(NoRadialEngagement)` (P28-20) | P28-10 rows, no change |
|---|---|---|
| operator | 24: toolpaths 3 and 4, each `planned` and `stated_cut`, each `body` and `drill cycle`, each chip max, chip min, rubbing floor | 8 (power, the same toolpaths and cuts) |
| wanaka21 | 24: the same 24 cells (toolpaths 3 and 4) | 6 |
| rivmap100, rivmap100_live | 0 | 0 |

The 8 P28-10 rows of the operator file already name `P28-10` at HEAD (the exemption field reads
the scope row since P29-10), so they do not change: `P28-10` needs a ruling, and invariant 3
keeps such a cell a `NoNumber`. A moved row changes its `state`, its `tone`, its `face`, its
`detail` and its place in `in_default_view`; the value, the bound and the exemption stay.

Not in this commit: the gate rows of the drill group (`GroupCell::NotJudged`, `rows_of_verdict`):
16 operator toolpaths each show chip max (P28-20), deflection (P28-17) and force budget (P28-04)
as `NoNumber(NotJudgedHere)`, all three `Physical(NoRadialEngagement)`; 48 rows. The same rule
would move them; the lead decides (section 11.5). The fixture dump does not change (it holds no
envelope rows).

Measured (11-2): as predicted. operator 24 rows and `nz_wanaka_21` 24 rows in each modulation
pass go from `NoNumber(NotJudgedHere)` to `NotApplicable(NoRadialEngagement)` (face "drill cycle ·
chip max no side cut", tone `not_applicable`, detail "not applicable; the cut has no radial
engagement"), exemption `P28-20` kept; the profile lines of toolpaths 3 and 4 change by the same
words only. The 8 (operator) and 6 (`nz_wanaka_21`) `P28-10` rows do not move. rivmap100 and
rivmap100_live: 0 rows. The fixture dump: no change. The stated-point sentry gains the claim (a
scope cell takes the class of its row, at both points, with a non-vacuity count of each kind).
Wire: the `state` (`not_applicable`, reason `no_radial_engagement`), `tone` and `face` of those
rows; no key is added or removed.

### 11.3 Commit 11-3: the stated point judged as value checks (prediction)

Written before the code, from the dump of the 11-2 tree (`pre113`, which adds the card face
line to the probe). Today `of_envelope` gives `NoNumber(NotJudgedHere)` to the stated point
against a limit on a fixed axis (a fixed axis does not place the point) and against a per-point
limit. The rule as it will be built (`SafeEnvelope::stated_value_check`, read by `of_envelope`;
the placement, the proposal and every drawn edge do not change):

- **Depth axis** (flute guard, cutting length, vendor axial bound, the depth engagement floor):
  the value is the body depth by the one depth door (`DeclaredCuts::body_depth`), as the stated
  cut rows read it: a lower bound `AtLeast(b)` (a Project Curve) reads `b`; an earlier leave
  reads the leave; `NoEarlierLeave` gives `NoNumber(NoEarlierLeave)`; a plane depth reads the
  stated point depth when the envelope has a depth axis, else no depth: `NoNumber(NotModelled)`
  (the reason of the deflection row of the same cut). The bound is the edge bound at that depth.
- **Width axis** (tool diameter, the width engagement floor, the move-budget width floor): the
  value is the body width: a slot (`DeclaredWidth::Slot`) cuts the engaged diameter of the tool at
  the body depth (`EngagedDiameter::of_tool`, the door of the diameter edge); a stated width reads
  it; a body with no width rule reads the stated point width. A curve with no bound at the depth
  gives `NoNumber(NotModelled)`.
- **Lower bound.** A value from a lower-bound depth: a ceiling that holds gives
  `NoNumber(OnlyALowerBound)`, a ceiling under the value gives `Exceeds`; a floor at or under the
  value gives `Within`, a floor above it `NoNumber(OnlyALowerBound)`.
- **Per point, the cutting feed cap:** the value is the stored op feed, the bound the cutting
  ceiling of the op (`MachineProfile::op_cutting_ceiling` at the input's `feed_max_rate`, the
  ceiling of Step 7); no stated feed gives `NoNumber(NoStatedFeed)`.
- A limit with a planned role (a limit of each cut: the force budget, the deflection, the vendor
  depth table, the power, the chip band, the rubbing floor) on a fixed axis or per point gives a
  new reason, `NoNumber(JudgedPerCut)`: its stated cut rows judge it. The stated rows never show
  these checks (`write_rows` keeps only the limits that are not limits of each cut), so no row on
  a surface shows the new reason.
- After this, `of_envelope` gives no `NotJudgedHere`. The reason stays only for a scope cell whose
  row needs a ruling (P28-10 and the gate rows of the drill group), and each names its row. So
  `S-NotJudgedHere` closes: its row and its two markers go; pins 36 / 25 / 36.

Predicted rows (the `off` and `runs` passes give the same rows; margin = share inside the bound):

| File | idx | Limit (axis) | Value | Bound | State |
|---|---|---|---|---|---|
| operator | 0, 11, 12 | feed cap | 6488 mm/min | 15000 (the X and Y rate; no `max_cutting_feed_mm_min`) | Within (0.567) |
| operator | 1 | feed cap | 8475 | 15000 | Within (0.435) |
| operator | 14 | feed cap | 8060 | 15000 | Within (0.463) |
| operator | 1, 2, 14, 15 | move budget (width floor) | 5.5, 1.5, 3.6, 1.382 | 0.2208, 1.1015, 0.1605, 0.3629 | Within |
| operator | 3, 4 (drills) | tool diameter; engagement floor (width) | 6 | 6; 0.02 | Within (at 100 %); Within |
| operator | 5 (Project Curve, no lower bound) | flute guard, cutting length, vendor axial, engagement floor (depth) | - | - | `NoNumber(NotModelled)` (4 rows) |
| operator | 5 | tool diameter; engagement floor (width) | 6 (6 mm flat, slot) | 6; 0.02 | Within (100 %); Within |
| operator | 9 (Project Curve, `AtLeast(2.418)`, V-bit) | flute guard, cutting length, vendor axial | 2.418 | 6.522, 8.152, 3.175 | `NoNumber(OnlyALowerBound)` (3 rows) |
| operator | 9 | engagement floor (depth) | 2.418 | 0.05 | Within |
| operator | 9 | tool diameter; engagement floor (width) | the cone at 2.418 mm (under 3.175) | 3.175; 0.02 | `NoNumber(OnlyALowerBound)`; Within |
| operator | 10, 13 (Trace V60) | tool diameter | the cone at the depth (3.002, 0.693) | 3.002, 0.693 | Within (at 100 %: a slot cuts the cone) |
| operator | 10, 13 | engagement floor (width) | 3.002, 0.693 | 0.02 | Within |
| operator | 16 (Scallop, earlier leave 0.2) | flute guard, cutting length, vendor axial, engagement floor (depth) | 0.2 | 16, 20, 6, 0.05 | Within (4 rows) |
| operator | 17 (Profile) | tool diameter; engagement floor (width) | 6 | 6; 0.02 | Within (100 %); Within |
| rivmap100, live | 0 | feed cap | 2200 | the cutting ceiling (no axis rates: the travel rate, 10000) | Within |
| rivmap100, live | 1 | feed cap; move budget (width) | 2400; 1.2 | 10000; 0.1018 (live 0.0251) | Within; Within |
| rivmap100, live | 2 (Scallop, leave 0.2) | 4 depth rows | 0.2 | 16, 20, 6, 0.05 | Within |
| rivmap100, live | 3 (Project Curve, `AtLeast(1.0)`) | flute guard, cutting length, vendor axial | 1.0 | 12, 15, 5.5 | `NoNumber(OnlyALowerBound)` |
| rivmap100, live | 3 | engagement floor (depth) | 1.0 | 0.05 | Within |
| rivmap100, live | 3 | tool diameter; engagement floor (width) | the slot width at 1.0 mm | 5.5; 0.02 | `NoNumber(OnlyALowerBound)` (or Within on a flat tool at D); Within |
| `nz_wanaka_21` | 0, 1, 12, 14 | feed cap | no stated feed | - | `NoNumber(NoStatedFeed)` |
| `nz_wanaka_21` | 2 | move budget (width) | - | no bound at the depth | `NoNumber(NotModelled)` |
| `nz_wanaka_21` | 1, 14, 15 | move budget (width) | 3.6, 3.6, 1.382 | 0.2457, 0.1605, 0.3629 | Within |
| `nz_wanaka_21` | 3, 4, 5, 9, 10, 13, 16, 17 | as operator idx 3, 4, 5, 9, 10, 13, 16, 17 | | | the same states |

Counts, operator (35 rows): `Within` 27 (feed cap 5, move budget 4, drills 4, op 5 width 2, op 9
depth floor and width floor 2, Trace 4, Scallop 4, Profile 2); `NoNumber(OnlyALowerBound)` 4 (op 9);
`NoNumber(NotModelled)` 4 (op 5). No row `Exceeds`. Each rivmap file (13 rows): `Within` 9,
`NoNumber(OnlyALowerBound)` 4 (idx 3; 3 if the idx 3 tool is flat, then `Within` 10).

**The face line of the 18 operator ops: no change** (no new row exceeds; the rank reads only rows
that exceed). The 18 lines before and after:

| idx | Face line (before = predicted after) |
|---|---|
| 0, 3, 4, 5, 9, 10, 12, 13 | none |
| 1 | caution: planner cap · depth 387 % over force cap |
| 2 | caution: ramp entry · depth 100 % over force cap · worst case |
| 6 | caution: ramp entry · chip min 76 % under floor · worst case |
| 7, 8 | caution: body · chip max 27 % over limit · before modulation |
| 11 | caution: ramp entry · chip min 66 % under floor · worst case |
| 14 | caution: ramp entry · depth 78 % over force cap · worst case |
| 15 | caution: ramp entry · depth 84 % over force cap · worst case |
| 16 | caution: body · chip min 16 % under floor |
| 17 | caution: plunge entry · plunge rate 17 % over limit · may not occur |

The other three files: no face line moves. The explainer data: the six envelope replies of
`P29-10_explainer/data/` (operator idx 0, 1, 2, 6, 14, 15) hold stated rows of the feed cap
(idx 0, 1, 14) and the move budget (idx 1, 2, 14, 15) that move; they need a regeneration.

Measured (11-3), the dump `a113` against `pre113`:

- Rows: operator 35, `nz_wanaka_21` 34, rivmap100 13, rivmap100_live 13 stated rows change, in
  each modulation pass; no other row changes (the profile lines change by the same rows). The
  states are as predicted on every row: operator 27 `Within`, 4 `OnlyALowerBound` (idx 9), 4
  `NotModelled` (idx 5); each rivmap file 9 `Within`, 4 `OnlyALowerBound` (idx 3: the tool is not
  flat); `nz_wanaka_21` as the operator file, plus 4 `NoStatedFeed` (idx 0, 1, 12, 14) and 1
  `NotModelled` (idx 2, the move-budget curve has no width at the depth). No row `Exceeds`.
- One value differs from the prediction: the Scallop finish (operator and `nz_wanaka_21` idx 16,
  rivmap idx 2) reads the earlier leave 0.5 mm, not 0.2 mm. The prediction read the stated point
  depth of the plane mark; the depth door gives the leave (the value that its stated cut rows
  read). The states are as predicted (`Within`: 3.1 %, 2.5 %, 8.3 % of the depth caps, 1000 % of
  the depth floor).
- The feed cap: operator 6488, 8475, 6488, 6488, 8060 mm/min against 15000 (43 %, 56 %, 43 %,
  43 %, 54 %); rivmap 2200 and 2400 against 10000. The width of a V-bit slot: idx 10 and 13 read
  the cone, 3.0022 and 0.6928 mm, at 100 % of the diameter cap; idx 9 reads 0.8527 mm at the lower
  bound 2.418 mm.
- The face line: 44 of 44 lines of the four files do not change (the 18 operator lines of the
  table above).
- `S-NotJudgedHere` closed: pins `ROWS` 36, `MAX_NEEDS_RULING` 25, `ROWS_WITH_A_FILE` 36. A new
  claim 4 of `every_declared_cut_is_judged_at_the_stated_point_p29_10` holds the value checks.
- The explainer: `data/` regenerated for the rows that 11-3 moves (`regen_113.py` in the
  scratchpad: each moved row of the six envelope replies and of the CLI records is replaced by its
  row of the probe dump of the GUI run modulation, after a check that the old row equals the
  "before" dump; a writer that formats as `serde_json` does, checked by a round trip of every
  file). 7 rows in the replies, 35 + 3 in the CLI records; the tool-load report holds no envelope
  row. `build_page.py` and `assert_claims.py`: 11 of 11 claims pass.
- Wire: the stated rows of these limits carry a state, a margin and, for the feed cap, the unit
  `mm/min`; their `exemption` is `null` (was `S-NotJudgedHere`). A new `NoNumberReason`,
  `judged_per_cut`, that no row shows.
- A sentry outside the list moved: `every_limit_is_a_limit_id_p29_2`
  (`the_checks_of_a_planned_point_hold_the_placement`) held the least `Within` margin of
  `of_envelope` equal to the placement margin. The feed cap check (0.75 on its pocket fixture) is
  a value check of the stated feed, not a margin that places the point; the claim now reads the
  checks whose edge is not per point.

### 11.4 Commit 11-5: F6-a (prediction; the code is not built in round 2a)

Written from the HEAD fixture dump (`base_b`) and the FM1 sim CSV, before any code. The code of
11-5 is not in this round (section 11.5).

- **The 2D Adaptive Link row:** `NoNumber(NoBound)`, value 0.5625 mm, population 69 of 6074, to
  `Within` with the flute length of the fixture tool as the bound (the `ToolConfig` builder
  default, an estimated 25 mm cutting length; the depth caps of the dump, 20.5 to 21.0 mm, sit
  under it), population 3 (the force population). Exemption `S-NoBound` goes from the row (a `Within` row has none).
- **Populations** (depth now, predicted = the force reading now): 2D Adaptive Body 2007 to 1998,
  Entry 366 to 366, Link 69 to 3; Pocket Body 1112 to 1112, Entry 66 to 60; Profile 791, 21, 6
  (no change); Shelf Body 4870 to 4425, Cleanup 990 to 912, Entry 333 to 329, Link 14 to 12;
  Pocket with dogbones: Body 1106 to 1106, Entry 66 to 60, Cleanup 24 to 24.
- **FM1 sim cells that move** (the 4 cells with a force budget and a depth `Reported` for "no
  root below the flute"): cell 8 (EndMill Profile softwood), 12 (BullNose Profile softwood), 14
  (BullNose Profile mdf), 16 (EndMill Adaptive softwood): `depth_cap_mm` empty to 18.0000 (the
  FM1 tool states a cutting length of `max(3 D, 12)` = 18 mm),
  `depth_cap_factor` empty to 35.3224, `depth_detail` "Reported" to empty; `depth_state` stays
  "Within"; `diagnostic_ids_after_sim` `load.depth.reported` to `load.depth.within` (and its
  severity in `diagnostic_severities_after_sim`). `depth_peak_mm` moves only if the deepest sample
  has no arc (not measured). A cell whose judged sample took the width share can move
  `depth_peak_mm` and `depth_cap_mm` (possible on cells 0 to 2, 4 to 6, 9, 10, 13, 17, 18, 20 to 22,
  28, 29; not measured). No feed, depth or proposal column moves (the depth row does not gate, and
  the modulator reads its own caps).

### 11.5 Open for the lead after round 2a

- **11-5 is not built.** Its prediction is section 11.4. The code needs: the width door
  `deflection::sample_engagement` in the depth gate (the width share goes), "no root below the
  flute" as `Within` at the flute length, a fixture with a sample over the force budget, the F6
  claim in a sentry, and a rewrite of the gate tests that read the width share with no arc
  (`tool_load/depth.rs` lib tests, `the_depth_that_cut_is_a_measured_load_g_s3depth`,
  `finish_depth_is_reported_not_capped_fm6`), then the FM1 re-bless of the cells of 11.4.
- **11-2, the gate rows of the drill group:** the same rule would make 48 operator gate rows
  `NotApplicable` (chip max P28-20, deflection P28-17, force budget P28-04); the P28-10 power row
  stays. Not done: the lead's count named the 24 envelope rows only.
- **S10, the machine door:** `RigidityProfile::force_budget_of(CutFacts)` holds the one read of
  the depth rule in a guarded file (a class D entry). `depth_cap_of(DepthLimit)` stays for
  `session/compute.rs` (the modulator, not guarded, not in S10).
- **11-3, `NotModelled` for a body with no depth before the simulation** (a Project Curve with no
  lower bound: operator and `nz_wanaka_21` idx 5): the existing reason of the deflection row of
  the same cut, not a new reason.

## 12. Round 2b: predicted and measured

Round 2b, written 2026-11-14 at HEAD `9c0bf936c`. The same two probes as round 2a, with two more
lines: the row probe writes `EFF`, the Suggest efficiency numbers (`feeds::efficiency::
cut_efficiency` on the stored op and the Suggest result of the profile) of each toolpath; the
gate probe writes `NARR`, the narration of each kind fixture. The HEAD dumps equal the round 2a
dumps `a113` and `base_b` byte for byte (`cmp`). Copies are in the session scratchpad folder
`p29_11c/`.

### 12.1 Commit 11-2b: a gate scope cell takes the class of its row (prediction)

Lead ruling: the rule of 11-2 holds for the gate rows (`rows_of_verdict`) through the same
function as the envelope rows. The function is `limits::scope_cell_state(row)`: a `Physical(g)`
row gives `NotApplicable(g)`, any other class gives `NoNumber(NotJudgedHere)`. The envelope cell
(`CutRow::of_cell`) and the gate cell (`GroupCell::NotJudged` in `gate_rows`) both call it.

The rows of a drill-group cell in `EXEMPTIONS`: `P28-20` (chip max), `P28-17` (deflection) and
`P28-04` (force budget) are `Physical(NoRadialEngagement)`; `P28-10` (power) needs a ruling.

Predicted, from the HEAD dumps (the verdict rows with no trace of the row probe, and the verdict
rows of the gate probe):

| Dump | Rows that move: `NoNumber(NotJudgedHere)` to `NotApplicable(NoRadialEngagement)` | `P28-10` rows, no change |
|---|---|---|
| operator | 48: the drill group of the 16 toolpaths that are not drill cycles, chip max, deflection, force budget | 16 |
| `nz_wanaka_21` | 24 (8 toolpaths) | 8 |
| rivmap100, rivmap100_live | 12 each (4 toolpaths) | 4 each |
| the five kind fixtures | 15 (one drill group each, three rows) | 5 |

A moved row changes its `state`, `tone` (`not_applicable`), `face` ("drill · chip max no side
cut") and `detail` (the clause "the cut has no radial engagement" replaces "this limit is not
judged on this cut"); the `exemption` (the row id) and the extra clause "drill cycle — no
continuous engagement" stay. No value, bound, envelope row, profile line or face line moves. No
FM1 cell moves (FM1 holds no row text).

Not moved, and a decision for the lead: the HEADLINE drill rows of a drill op (operator idx 3
and 4) come from `of_criterion` on the verdict `Unmodeled(NotApplicableForOp)`. They read
`NotApplicable(NoRadialEngagement)` with no exemption row, the power row too, although `P28-10`
needs a ruling. The headline verdict does not hold the row id, and `gcode::warning_unmodeled`
keys on `NotApplicableForOp`. 11-2b does not change them.

The explainer: the CLI records of operator idx 3 and 4 hold 12 envelope rows each from before
11-2 (`P28-20`, `NoNumber(NotJudgedHere)`); the tool-load report holds 48 gate rows that 11-2b
moves. Both are refreshed from the probe dumps.
