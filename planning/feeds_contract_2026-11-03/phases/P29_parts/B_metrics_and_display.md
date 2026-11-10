# P29 part B: metrics beyond limits, and the display surfaces

Read-only audit, 2026-11-11. No product code, no cargo run. Source: the code at HEAD
`484542abe`. Line numbers are the lines read on that commit. Paths with no crate prefix are
under `crates/rs_cam_core/src/`; `viz/` is `crates/rs_cam_viz/src/`; `cli/` is
`crates/rs_cam_cli/src/`.

Scope: items 4 and 5 of `P29_INVESTIGATION_PLAN.md`. The limits (power, force, deflection,
chip maximum) are in `P28_limit_audit.md` and in part A. Where a row here touches a P28 row,
it names the row and does not repeat it.

Reason classes, as P28: PHYSICAL (no meaning there), SCOPE (left out or deferred),
UNSOURCED RULE (a rule of thumb stands in), UNKNOWN (no reason found).

Not verified in this audit: the bodies of the 2D generators' own plunges (Pocket, Profile,
Zigzag, Face), the Unified Finish planner entry, and the full body of
`dressup/feed_modulation.rs`. Rows that depend on them say so.

---

## 4. Metrics beyond limits

### 4.1 Entry rules, per emitter

How each op enters is a declared field: `EntryContract` on the registry row
(`compute/catalog/entry_contract.rs:29-65`). Four variants: `Dressup`, `Planner`,
`DescentIsCut`, `StraightPlunge`. The contract states which EMITTER makes the entry. It does
not state the rule values. Each emitter holds its own values.

The contract of each op (`compute/catalog/registry.rs`, the line of the `entry:` field):

| Contract | Ops | Line |
|---|---|---|
| `Dressup`, any style, default from the role (Roughing: Ramp; Finish and SemiFinish: None) | Face, Pocket, Profile, Zigzag, Chamfer, Waterline, Scallop, Steep/Shallow, Ramp Finish, Spiral, Radial, Horizontal | 97, 151, 207, 521, 788, 970, 1091, 1222, 1279, 1340, 1394, 1448 |
| `Dressup`, default Helix, None = stock-aware plunge | 2D Adaptive | 266 |
| `Dressup` plus its own crease ramp | Pencil | 1029 |
| `Planner` (plunge, peck ladder, ramp, helix) | 3D Adaptive | 909 |
| `Planner` (plunge only) | Unified Finish | 1163 |
| `StraightPlunge` at the op plunge rate | VCarve, Inlay pocket, Inlay plug, Trace, Parallel finish, Project Curve | 336, 400, 464, 578, 846, 1511 |
| `DescentIsCut` (no entry record) | Drill, Bore, Pin Drill | 639, 722, 1579 |

The rule values, one row per place where the emitter or the op decides the value:

| Metric | Function | What it branches on | Which ops get a different rule or no value | Class | Evidence |
|---|---|---|---|---|---|
| Ramp angle, default | `DRESSUP_RAMP_ANGLE_DEG` = 3.0; `ADAPTIVE3D_RAMP_ANGLE_DEG` = 10.0 | The emitter (dressup door or the 3D planner) | Every `Dressup` op: 3 deg. 3D Adaptive: 10 deg. Both comments say "Repo rule, no source". | UNSOURCED RULE | `compute/config.rs:981`; `compute/operation_configs.rs:2001` |
| Ramp angle of a fallback ramp (the helix does not fit) | `apply_entry`; `segments_to_toolpath` | Fixed constant, not the op value | Both doors use the dressup 3 deg, also on a 3D Adaptive op whose own ramp angle is 10 deg | UNKNOWN | `dressup/mod.rs:517-519`; `adaptive3d/path.rs:2559-2561` |
| Ramp angle of the crease ramp and the rest ramp ladder | `finish::pencil::plan_entry_ramp`, `ENTRY_RAMP_MAX_ANGLE_DEG` = 12.0 | The emitter | Pencil's own ramp, and any op whose plunge the session turns into the rest ladder (G-ISOCLIPENTRY). A third angle. The operator has no dial for it. | UNSOURCED RULE | `finish/pencil/emission.rs:70`, `:113`, `:224`; `entry_contract.rs:93-96` |
| Ramp slope of the helical waterline descent | `ops::waterline::lap_count` | Fixed constant | Waterline descends along the ring at the dressup 3 deg constant. It does not read the op's `ramp_angle`. | UNKNOWN | `ops/waterline.rs:842-848` |
| Ramp angle of a tool that does not cut at its centre | `CoreZone::clamp_ramp_angle_deg`; `effective_entry_style` | The tool body core (a cut property) | The emitters clamp. The 3D planner also turns the Plunge style into a ramp at the core limit angle. A `StraightPlunge` op keeps its plunge and only warns. | PHYSICAL for the clamp; SCOPE for the `StraightPlunge` exemption (ruling Q8 is named in the code) | `tool/body.rs:586-595`; `adaptive3d/path.rs:2323-2333`; `entry_contract.rs:52-56` |
| Helix radius | `DressupConfig::helix_radius_for`; `Adaptive3dConfig::helix_radius_for` | The config type | Same rule (0.3 x D, capped at the flat bottom, raised to the core floor) through one resolver `HelixRadius::resolve`. The dressup stores an optional radius in mm. The 3D op stores a factor of D. Two fields for one quantity. | none (one rule) | `compute/config.rs:994`, `:1618-1628`; `compute/operation_configs.rs:2039-2050` |
| Helix radius, fitted to the region | `contained_helix_radius` (dressup); `adaptive::Bore::at` (3D) | `helix_floor_lap` capability; strategy and style | The dressup fits only for an op with `helix_containment` (2D Adaptive). The 3D bore fits only for Geodesic Adaptive with a helix style. A 3D Contour Parallel helix and every other dressup helix get no fit. | SCOPE (P22b ruling Q1 names the 3D case) | `dressup/mod.rs:506-525`; `adaptive3d/path.rs:1788-1812`; `compute/catalog/capabilities.rs:786` |
| Helix pitch, default | `DRESSUP_HELIX_PITCH_MM` = 1.0; `ADAPTIVE3D_HELIX_PITCH_MM` = 2.0 | The emitter | Every `Dressup` op: 1 mm per turn. 3D Adaptive: 2 mm per turn. "Repo rule, no source". | UNSOURCED RULE | `compute/config.rs:987`; `compute/operation_configs.rs:2008` |
| Helix pitch of a Bore | `ops::bore` `pitch` | Op type | Bore's pitch is its axial cut (`Axial::HelixPitch`). The Bore files show no read of the core rule (`rg` for `core_of`, `max_helix_pitch` in `compute/execute/bore.rs` and `ops/bore.rs`: no hit). | UNKNOWN | `ops/bore.rs:59`, `:123`; P28 row 32 |
| Flat lap at the helix floor | `helix_floor_lap` | Capability flag | Only 2D Adaptive ends its helix on a flat lap. | SCOPE | `compute/catalog.rs:197`, `:327`; `capabilities.rs:786` |
| Entry clearance above the material | `entry_clearance_mm` | Two fields with one default | `DressupConfig::entry_clearance_mm` and `Adaptive3dConfig::entry_clearance_mm`; one default `default_entry_clearance_mm`. | none | `compute/config.rs:1087`, `:1236`; `compute/operation_configs.rs:1841` |
| Feed of the plunge and of the air lead | `PlungeFeedRule` | The emitter | Dressup door: 0.5 x the first linear feed of the toolpath, else 500 mm/min (`FirstLinearFeedFraction`, two Unsourced F201 rows). It does not read the op's `plunge_rate`. 3D planner: `params.plunge_rate`. `StraightPlunge` ops: the op plunge rate. | UNSOURCED RULE | `entry_contract.rs:236-266`; `compute/execute/dressup_apply.rs:746-748`; `adaptive3d/path.rs:2578-2602` |
| Feed of a ramp or helix through material | `EntrySafety::ramp_feed` = `OperationConfig::entry_feed_rate` | Whether the op states `ramp_feed_rate` | Stated: `min(ramp_feed_rate, feed_rate)`. Not stated: the caller's plunge feed above, so the two doors differ again. | SCOPE | `compute/catalog.rs:2048-2052`; `dressup/entry_descent.rs:543`, `:972` |
| Suggest's ramp feed and entry notes | `feeds::ramp::entry_geometry` | `if let OperationConfig::Adaptive3d` then the dressup style | 3D Adaptive reads its own style. Every other op reads `dressups.entry_style`. An op with a planner or own entry that is not 3D Adaptive gets `EntryOff`: no ramp feed, no note. That covers the Pencil crease ramp, the rest ramp ladder, the helical waterline descent, the Bore helix, the Unified Finish plunges. | SCOPE | `feeds/ramp.rs:332-375`, `:937-954` |
| Straight-plunge caution note | `entry_notes` | `PassRole::Roughing` | Only a Roughing role gets the Q12 note. Trace, VCarve, Inlay, Project Curve are filed Finish and plunge straight into a full-width groove; they get no note. | UNKNOWN (follows the filing of P28 row 3) | `feeds/ramp.rs:947-952`; `registry.rs:578, 336, 400, 1511` |
| Plunge rate (Suggest) | `plunge_basis`, `PLUNGE_RULES` | Tool family; Drill family | One rule per tool family (flat 1/Z, ball 0.5, tapered, V 60). Keyed on the tool, not the op. A drill cycle returns `DrillCycle`. | PHYSICAL | `feeds/extrapolation/plunge.rs:254-300`; `feeds/plunge.rs:425-432` |
| Vertical-rate limit | `VerticalRateLimit::bound` | none | One door for every plunge, the drill feed and the vertical part of a ramp (P20b). This one is already a pure function of tool, row and machine. | none | `feeds/CLAUDE.md` invariant G6/G10; `feeds/vertical.rs` |
| Plunge depth limit | `CoreZone::plunge_refusal`, `max_plunge_depth_mm` = blade protrusion | The tool body core | Exists only for a tool that does not cut at its centre. A centre-cutting tool has no plunge depth limit anywhere. | PHYSICAL for the core rule; UNKNOWN for "no limit otherwise" | `tool/body.rs:665`; `stock/contact.rs:896`; `compute/feasibility.rs:432` |
| Peck stroke of a plunge entry | `emit_peck_plunge` | The emitter | Only the 3D planner pecks its plunge entry (stroke = depth per pass). No other op's plunge pecks. | SCOPE | `adaptive3d/path.rs:177-196`; `entry_contract.rs:126-127` |
| Plunge-entry stability warning | `check_plunge_entry_stability` | Adaptive family and Roughing | P28 row 28. Threshold 0.5 x D, calibrated on one wanaka reading. | UNSOURCED RULE | `feeds/suggest/adaptive_entry.rs:30`, `:110-130` |
| Ramp fold lap cap | `OperationType::ramp_fold_lap_cap` | Process role; the door | Finish and SemiFinish roles get a cap. A rough gets none. The 3D door always passes `None`. | SCOPE (operator ruling 2026-09-18 is named) | `compute/catalog.rs:759-761`; `dressup/mod.rs:182-186` |
| Entry starts from the op's own replayed stock | `EntrySafety::own_stock` | The door | The dressup door replays the op's stock. The 3D door passes `None` and uses the planner floor. | SCOPE | `dressup/mod.rs:187-194` |
| Entry record (kind, from, to, lowest feed) | `entry_records` | Contract | `DescentIsCut` gives no record, so the lineage of a Drill, Bore or Pin Drill lists no entry. | PHYSICAL | `entry_contract.rs:360-366` |
| Centre-cutting judgement at generation | `straight_plunge_refusals` | `EntryContract::StraightPlunge` | Only the six `StraightPlunge` ops are scanned for steep descents. A steep descent in the body of any other op is judged only after the simulation. | SCOPE | `compute/feasibility.rs:386-414` |

The 2D against 3D differences (P22b), in one list:

1. Default ramp angle 3 deg against 10 deg.
2. Default helix pitch 1 mm against 2 mm.
3. Helix radius stored as mm against a factor of D (same resolver).
4. Plunge and air-lead feed: half of the first linear feed against the op plunge rate.
5. Plunge style: one stroke against a peck ladder at the depth per pass.
6. Helix fit to the region: containment shrink plus floor lap (2D Adaptive) against the bore
   of Geodesic only; Contour Parallel 3D has neither.
7. Fold lap cap from the role against none.
8. Start height from the op's replayed stock against the planner floor.
9. Suggest reads the 3D style through one `if let`; all other own entries are invisible to it.
10. Both doors share the emitters `emit_helix` and `emit_ramp`, the resolver
    `HelixRadius::resolve` and the core rules of `tool/body.rs` (`dressup/CLAUDE.md`).

### 4.2 Tool-body checks: before the simulation against in the simulation

| Metric | Function | What it branches on | Which ops get no value or a different rule | Class | Evidence |
|---|---|---|---|---|---|
| Reach past the flute; reach to the holder (pre-sim) | `FeasibilityInputs::reach` | none on the op; the reference top is the stock BOX top (a Drill reads its hole base) | Every op is judged. The depth is "below the stock box top", not below the live stock, so a rest op deep in a roughed pocket warns the same as a fresh cut. | SCOPE (the comment says "by the stock box") | `compute/feasibility.rs:354-378`, `:519-548` |
| Centre cut (pre-sim) | `centre_cut` | `EntryContract::StraightPlunge`; the recorded entry refusal | See 4.1 last row. | SCOPE | `compute/feasibility.rs:386-440` |
| Helix inside the core (pre-sim) | `helix_core` | The emitters that record `HelixCoreFallback` | Only the dressup helix, the 2D planner hole and the 3D bore record one. | PHYSICAL | `compute/feasibility.rs:445-500` |
| V tip flat against flute length | `v_tip_flat` | `ToolType::VBit` | V-bit only. | PHYSICAL | `compute/feasibility.rs:550-575` |
| Body zones on live stock (shank, holder, shoe, core, flute top) in the simulation | `WalkChecks::mounted`, `zone_contact` | The carve arm: drill or not | Every milled op runs the walk. A drill entry takes the analytic removal arm: it gets `drill_flute_overreach` only. No `WalkChecks` is built in that arm, so the shank, holder, shoe and core zones are not tested on a Drill or a Pin Drill by this walk. | UNKNOWN (the comment explains only the flute case) | `compute/simulate.rs:2187-2222` (drill arm), `:2253-2255` (walk); `stock/contact.rs:922-934` |
| Shank in the calculator | Step 4c | none | Every op with a depth. Project Curve has no pre-sim depth (P28 row 11). | as P28 row 11 | `feeds/CLAUDE.md` P22-0c-6b |
| Model gouge guard | `ModelGougeRule` | Registry row | `Exempt`: VCarve, Inlay pocket, Inlay plug, Trace, Pin Drill, Project Curve and all eleven 3D finish ops. The stated reason: "the cut is the intent ... or the op follows the surface itself". | PHYSICAL for engraving; SCOPE for the finishers (no guard checks that they do follow the surface) | `compute/catalog/capabilities.rs:172-212`; `registry.rs:371, 435, 499, 614, 881, 1002, 1066, 1123, 1200, 1257, 1311, 1372, 1426, 1487, 1548, 1618` |

Result record of the pre-sim checks: `RuleResult {rule, fact (how the tool fact is known),
measured, limit: Option, text}` (`compute/feasibility.rs:234-240`). It already holds value,
limit and basis. The simulated twins use other types (`ToolpathBodyContact`,
`ToolpathFluteOverreach`, `ToolpathCoreDescent`, `compute/simulate.rs:848, 904, 1189`).

### 4.3 Vendor depth table, axial bound, axial envelope

P28 rows 24 and 25 hold the exemptions. Added here: how the value is computed per op.

| Metric | Function | What it branches on | Which ops get a different rule | Class | Evidence |
|---|---|---|---|---|---|
| Axial envelope class | `Capabilities::axial_envelope` | Registry row | `NotApplied`: Face, Pocket, Profile, 2D Adaptive, both Inlay ops, Zigzag, Trace, Drill, Bore, Chamfer, Pencil, Ramp Finish, Pin Drill. `Roughing3d`: 3D Adaptive. `VBitGroove`: VCarve only (the Inlay ops cut the same V groove and are `NotApplied`). `CurveRough`: Project Curve. `Finish3d`: the other nine finishers and Waterline. | UNKNOWN for the split (P28 row 25: the stated reason is stale) | `capabilities.rs:670-685`; registry lines 114, 168, 224, 288, 356, 420, 484, 538, 598, 658, 739, 805, 866, 933, 987, 1051, 1108, 1182, 1240, 1296, 1357, 1411, 1472, 1533, 1596 |
| Width the axial envelope assumes | `axial_envelope_for_operation` | The class above | Four typed widths with no source: 0.4 x D (3D rough with no stepover), 0.2 x D (V-bit fallback, Project Curve), half the V width (VCarve), 0.15 x D (3D finish with no stepover). | UNSOURCED RULE | `feeds/suggest/axial_envelope.rs:62`, `:95-97`, `:110`, `:123` |
| Chip the axial envelope reads | same | none | It reads the op's STORED feed and RPM (`operation.feed_rate()`), not Suggest's chip. An unset feed gives chip 0. | UNKNOWN | `feeds/suggest/axial_envelope.rs:49-55` |
| Deflection target of the axial envelope | same | The class | Rough classes 200 um; `Finish3d` 50 um. The post-sim gate uses 200 um or the stock to leave (P28 rows 15, 16). Three values. | as P28 rows 15, 16 | `feeds/cutter_constraints.rs:77, 81`; `tool_load/deflection.rs:69` |
| Axial scallop bound | `max_doc_scallop` | Tool geometry hint | A pure ball returns `None` (the hint holds no radius), so the ball, the most common finish tool, gets no axial scallop bound from this function. Flat and V: none (physical). | UNKNOWN for the ball | `feeds/cutter_constraints.rs:411-433` |
| Depth the calculator reads | `CutSpec::calc_inputs` | `Axial` kind | P28 row 32 (Bore pitch, Chamfer width, peck). | as P28 | `compute/catalog/cut_spec.rs:335-352` |
| Depth basis words in the narration | `DocBasis` | Registry row | Five wordings by op. A label only. | none | `capabilities.rs:640-655` |

### 4.4 Minimum chip (rubbing)

| Metric | Function | What it branches on | Which ops or spans differ | Class | Evidence |
|---|---|---|---|---|---|
| Rubbing floor before the simulation | `step9b_rubbing_floor`, `rubbing_floor_at` | The band or point of the row; no op branch found | Value `min(0.025, band minimum)`, read against the COMMANDED advance per tooth (feed / (rpm x Z)), not the peak chip. | UNSOURCED RULE for 0.025 (`RubbingFloorSource::RepoConstant`) | `feeds/mod.rs:1865`, `:1923-1941`; `feeds/calculator/close.rs:131-182` |
| Burn side after the simulation | chipload gate, `ChipBounds::burn_reference_mm` | Row kind (band or point); Drill; span filter | Value: the band minimum (or the printed point), read against the MEDIAN peak chip of the steady-state samples. The 0.025 constant is not in it. A drill is `NotApplicableForOp`. Entry and cleanup spans are out (P28 rows 21, 22). A point row gives an advisory only. | The two sides use two limits and two statistics: UNKNOWN | `tool_load/verdict.rs:1734-1738`; `tool_load/chipload.rs:389-395`, `:841-871` |
| Rubbing in the modulator | `effective_rubbing_floor`, the floor arm | Lead moves, plunge moves skipped | P28 rows 29, 30. Not read in full here. | as P28 | `dressup/feed_modulation.rs:118`, `:937` |
| Minimum depth for the chip floor | `CutterAxialConstraints::min_doc_chipload_floor_mm` | The axial envelope class | Only ops with an axial envelope (4.3). | SCOPE | `feeds/cutter_constraints.rs:142` |

### 4.5 Efficiency, air cut, reached feed share

| Metric | Function | What it branches on | Which ops or spans differ | Class | Evidence |
|---|---|---|---|---|---|
| Specific energy `u`, ploughing share | `cut_efficiency` | none on the op | One planned point per op: `result.radial_width_mm` and the nominal `tool.diameter`. It does not go through `Engagement` or the `d_eng` door (K7, P15), so a ball at a shallow depth or a V-bit reads the nominal diameter. Width 0 returns `None` (drill: physical). | UNKNOWN (a second engagement path beside K7) | `feeds/efficiency.rs:164-190` |
| Chip verdict Thin / InBand / Heavy | same, `RadialThinning::of(family, role)` | Family and role | Finish role: lift 1 (P28 row 18, sourced). The width of the lift comes from `PowerCases::of_operation` (planner width per op). | SOURCED RULE | `feeds/efficiency.rs:205-214` |
| Deflection chip ceiling; force headroom | `deflection_chipload_ceiling_mm`, `force_headroom` | Drill; V-bit; missing inputs | Both use the finish target `EXCEEDS_BOUND_MM` (200 um) for every role, a rough pass included. Drill and V-bit: `None`. | P28 row 16 (two deflection limits) reaches this metric too | `feeds/efficiency.rs:62`, `:117-139`, `:252-254` |
| Efficiency after the simulation | none | - | No function computes `u` on the simulated samples. The metric exists only at the planned point. | SCOPE | `rg cut_efficiency`: callers are `viz/ui/feeds/compare.rs:260-261` and `viz/ui/feeds/explore.rs:131` only |
| Air-cut share and its bar | `air_cut_high_threshold_pct` | Registry row | Three typed bars: 40 % (2D ops, 3D rough), 45 % (surface finishes), 60 % (Project Curve). `None` for Drill and Pin Drill ("the dexel stock cannot see Z-only moves"). The bars are "judgement on a cluster of nine", fitted to fixture readings. | UNSOURCED RULE for the bars; PHYSICAL for the drills | `capabilities.rs:562-575`; `registry.rs:644, 1584`; readers `session/compute/diagnostics.rs:220`, `trace/narrate.rs:2143` |
| Air-cut hint text | `AirCutHint` | Registry row | Four wordings by op. A label only. | none | `capabilities.rs:657-668` |
| Air / low engagement in the summaries | `SummaryAccumulator::observe` | `in_transit_span` | Transit samples are left out of the engagement sums. | PHYSICAL (P28 row 23) | `stock/simulation_cut/accumulate.rs:44`, `:118` |
| Reached feed share | `kinematics::reached_feed_share` | none on the op | Counts every fed move of the toolpath: entries, leads and links too, not only cutting moves. `None` with no kinematics or no result. Shown only by MCP `get_suggest_rationale` (`basis.rpm.reached_feed_share`). No card, no CLI field found. | SCOPE (one surface) | `machine/kinematics.rs:1397-1447`; `session/compute/toolpath_diagnostics.rs:437-452`; `viz/app/mcp/generation.rs:275-292` |
| Kinematic utilisation | `ToolpathKinematicUtilization` | none read | A second "how much of the commanded feed is reached" figure (`utilization`, time-weighted achieved feed) on the load verdict. It overlaps the row above with another definition. | UNKNOWN (two measures of one thing) | `machine/kinematic_utilization.rs:311-335`; `tool_load/verdict.rs:346-364` |
| Move count for the runtime floor | `MoveCountModel` | Registry row | P28 row 9. `FeatureDriven` or `NotModelled` for 14 ops: no gate. | as P28 | `capabilities.rs:620-637` |

### 4.6 Finish quality

| Metric | Function | What it branches on | Which ops get no value | Class | Evidence |
|---|---|---|---|---|---|
| Scallop height as an INPUT | `Radial::Scallop` | `RadialRule` of the row | Scallop and Unified Finish (always), Parallel finish (when set). | none | `compute/catalog/cut_spec.rs:281-285`; `operation_configs.rs:1726, 2404, 2560` |
| Scallop height as an OUTPUT of a stated stepover | none | - | No function reports the cusp height that a stated stepover gives. Spiral, Horizontal, Steep/Shallow, Radial, Ramp Finish, Waterline (Z step), Pencil and a Parallel finish with a stepover show no cusp value on any surface. `scallop_height_flat` exists and has no product reader outside the generators' tests (`rg`: only `finish/scallop_math.rs`). | SCOPE | `finish/scallop_math.rs:15-23` |
| The chord law, stepover from scallop | four copies | The call site | (1) `finish::scallop_math::stepover_from_scallop_flat` (generators, the add-door rule). (2) `feeds::geometry::scallop_stepover` (the envelope axes, `max_doc_scallop`). (3) An inline inverse in the apply funnel. (4) The curved form in `metrology/floor.rs`. The radius each one reads differs: the add-door rule reads `tool.diameter / 2` for any ball tip; the envelope and the funnel read the tip radius of a tapered ball and the corner radius of a bull nose. | UNKNOWN (not checked whether `tool.diameter` is the tip of a tapered ball) | `compute/geometry_default.rs:462-472`; `feeds/geometry.rs:271-280`; `feeds/envelope.rs:2378-2392`; `feeds/suggest/apply.rs:1151-1166`; `metrology/floor.rs:169-171` |
| Tip float | `ToolpathStats::tip_float` | The generator | Only Pencil and Unified Finish (its crease stage) fill it. `None` = not measured on every other op, a Scallop or Parallel finish in a tight valley included. "Report-only: no gate consumes it." | SCOPE | `compute/toolpath_stats.rs:141-150`; `finish/pencil.rs:582, 652`; `finish/unified_finish.rs:1573-1638` |
| Derived stepovers, dropped band, clipped band, monotone cells, relink totals | `ToolpathStats` fields | The generator | Unified Finish only (the rest-analysis post-pass also derives a stepover). | PHYSICAL (the op plans bands) | `compute/toolpath_stats.rs:140, 173, 184, 367, 507` |
| Ramp reach clamp | `ToolpathStats::ramp_reach_clamp` | The generator | Ramp Finish only. | PHYSICAL | `compute/toolpath_stats.rs:200` |
| Waterline ladder, rest levels, pencil link | `ToolpathStats` fields | The generator | Waterline; rest ops; Pencil. | PHYSICAL | `compute/toolpath_stats.rs:322, 373, 533` |
| Rough leaves islands | `rough_leaves_islands` | 3D Rough | Stated as 3D Rough coverage. 2D roughs are not compared. | SCOPE | `stock/CLAUDE.md` invariants |
| Flat-floor scallop hint for a ball | `flat_floor_clearing` | Registry flag | Face, Pocket, Zigzag, Bore. Not 2D Adaptive, which also clears a flat floor. | UNKNOWN | `capabilities.rs:106-108`; `registry.rs:120, 174, 544, 747` |

`ToolpathStats` is the generation-side shape of this disease: one struct with about 30
`Option<..Finding>` fields, each filled by one or two generators, each with its own type and
its own "None = not measured" doc (`compute/toolpath_stats.rs:12-533`).

### 4.7 Suggest's width and depth proposals, per family

P28 rows 1, 5, 6, 9, 11 and 31 hold the exemptions. The mechanics per family:

| Metric | Function | What it branches on | Result per family | Class | Evidence |
|---|---|---|---|---|---|
| Which axes are free | `axes_plan` | `CutSpec` radial and axial kind (the cut, not the op type) | Plane, axial bar, radial bar or fixed. This part is already a function of the cut. | none | `feeds/envelope.rs:2375-2454` |
| Computed width (most removal per minute) | `axes_plan`, `has_computed_width` | `family == Pocket && role == Roughing && Radial::Width` | Face, Pocket, Zigzag, Bore only. The same test is written twice. | SCOPE (P28 row 5) | `feeds/envelope.rs:2461-2464`; `feeds/suggest.rs:750-759` |
| Depth cap rule | `RigidityProfile::depth_cap_rule`, `rigidity_cap_mm` | Family and role | Force budget (Pocket, Contour roughing), factor x D (Adaptive), none (finish, drill). | P28 rows 1 to 4 | `feeds/suggest/invariants.rs:213-232` |
| No depth proposal | `ProposedDepth::None` | No force line; `Axial::SurfaceOffset` | Project Curve; any op on a material with no force line. | P28 row 11; B6 | `feeds/envelope.rs:84-121` |
| Depth write of a pick | `apply` | `match operation` (every variant named) | Drill and Pin Drill write a peck through `peck_for_hole`; all others write the depth field. | PHYSICAL | `feeds/suggest/apply.rs:1191-1240` |
| Width write of a pick | `apply` | `Radial` kind | `Scallop`: the pick width is turned back into a scallop height. Others: the stepover, rounded down. | none | `feeds/suggest/apply.rs:1148-1182` |
| Stock-aware defaults | `apply_stock_defaults` | `match operation` | Parallel finish, Face, Profile, Drill get one; Pocket and 2D Adaptive lost theirs by ruling S11; the rest have none. | SOURCED (ruling S11 named) | `feeds/suggest/apply.rs:1547-1594` |
| Default cut when unset | `operation_default_profile` | Family and role | P28 row 31. | UNSOURCED RULE | `feeds/mod.rs:2306-2412` |
| Strategy recommendation | `pick_adaptive3d_clearing_strategy` | `OperationConfig::Adaptive3d`, Roughing | 3D Adaptive only; warn-only. | SCOPE | `feeds/suggest/adaptive_entry.rs:55-99` |
| Vendor row routing | `LutRoute` | Registry row | `AdaptiveAsPocket` (3D Adaptive), `FlatAsRoughPocket` (Trace, Parallel, Ramp, Radial, Horizontal), `ByCutter` (Project Curve), else as stated. 2D Adaptive is `AsStated`; 3D Adaptive is not. | P25 owns this | `capabilities.rs:531-556`; `registry.rs:301, 946` |
| 2D Adaptive pass ceiling multiplier | literal `trochoid_cap_mult: 1.2` | Typed at the 2D door | 2D Adaptive is called with 1.2; the 3D config default is 1.6 (P28 row 13). | UNSOURCED RULE | `compute/execute/clearing_2d.rs:1026`; `compute/operation_configs.rs:1988-1990` |

### 4.8 The basis label: which values carry one

Five separate "how is this known" vocabularies exist:

| Vocabulary | Type | Carried by | Evidence |
|---|---|---|---|
| K2 basis | `feeds::sourced::Basis` (Operator, Printed, Claim, Rule, Machine, ToolFact, Derived) | `PointBases` of a `FeedsResult`: rpm, chip, feed, plunge, ramp, flute count, ap, ae, power. Each envelope `Limit`. The `SafeEnvelope::proposal`. | `feeds/sourced.rs:215-242`, `:444-463`; `feeds/envelope.rs:379-388`, `:895` |
| Stored-field stamp | `ValueProvenance { source: ProvenanceSource, reference }` | Nine stored op fields: feed, plunge, rpm, stepover, depth per pass, scallop height, ramp feed, peck, Z step; plus the step records. | `feeds/provenance.rs:33-63`, `:201-237` |
| Gate bound source | `tool_load::verdict::BoundSource` (7 variants, with `gates_export` and `setting`) | The bound of each criterion row. | `tool_load/verdict.rs:1048-1100` |
| Tool fact kind | `FactKind` (Stated, Catalogue, Estimated, Unknown) | Each tool fact; each pre-sim `RuleResult`. | `compute/feasibility.rs:236`; `feeds/sourced.rs:190-195` |
| Formula constant | `FormulaConstant` Cited / Unsourced (F201) | Each named constant; reaches a value only through `Basis::Rule`. | `feeds/CLAUDE.md` (F201, pin 142) |

Others: `Confidence` (Validated / Approximate(text)) on each gate verdict
(`tool_load/verdict.rs:286-293`); `ClaimConfidence` on a G-claim; `MeasurementProvenance` on
some generation findings (`compute/toolpath_stats.rs:129-131`); the diagnostics `Confidence`
(verified / approximate / static / heuristic, `diagnostics/mod.rs:195`).

Values on a display path with NO basis label:

- The gate readings themselves (`peak_kw`, `peak_mm`, `observed_mm_per_tooth`, `peak_n`):
  only their bound has a source.
- `CutForceReading`: `rule` is a free `String`, not a `BoundSource` (`tool_load/cut_force.rs:159-172`).
- `CutEfficiency`: no field states a basis; `rubbing_floor_mm` drops the
  `RubbingFloorSource` that `rubbing_floor_at` returns (`feeds/efficiency.rs:265`).
- `ReachedFeedShare`, `ToolpathKinematicUtilization`, the air-cut bars, `EntryLoadObservation`
  (`threshold_mm` with no source, `stock/sim_triage.rs:1334-1342`), `PlungeStressWarning`
  (`safe_cap_mm_min` with no source, `tool_load/plunge_stress.rs:55-59`), `EntrySpike`
  (`bound` with no source, `tool_load/verdict.rs:1665-1671`).
- The entry rule values: stated only as note TEXT ("repo rule", "operator value") in
  `EntryNote.headline` (`feeds/ramp.rs:955-975`); the default is detected by comparing the
  value with the constant.
- `SpeedsPoint` serialises the four sourced values as bare numbers (`value_only`,
  `feeds/envelope.rs:631-636`), so MCP `get_safe_envelope` grid points have no basis.
- Every `ToolpathStats` finding except the few with `MeasurementProvenance`.

### 4.9 Other places with the same disease

| Place | What it branches on | Effect | Evidence |
|---|---|---|---|
| `is_drill_kinematics()` | Registry kinematics | 13 product call sites return early or abstain: 7 in `tool_load/`, `feeds/predict.rs:313`, `compute/generated_empty.rs:285`, `trace/narrate.rs:229`, `session/compute/diagnostics.rs:1419`, `session/pins.rs:351, 613`. One declared flag, read in 13 private places. | `compute/catalog.rs:749-751` |
| `Capabilities` | Registry row | About 25 per-op fields. Those that decide a metric or check: `air_cut_high_threshold_pct`, `move_count`, `doc_basis`, `air_cut_hint`, `axial_envelope`, `adaptive_woc` (the 0.15 x D static floor), `flat_floor_clearing`, `planner_uniform_engagement` (the modulator may read the stepover target as an engagement floor), `floor_inside_stock`, `lut_route`, `model_gouge`. Each has its own enum and its own reader. This is the existing "declared table"; it is per op, not per cut. | `compute/catalog/capabilities.rs:45-170` |
| Span filter readers | `SpanKind` through one predicate | 14 call sites of `is_steady_state_for_gate`, `is_phantom_transit`, `is_configured_entry`: the four gates, `cut_force`, the histograms, the envelope placement, the modulator's sample table, two triage rules (which pass `None` for the span lookup, the conservative fallback), `tool_load/mod.rs:302`. The predicate is one function; the decision "which metric drops which span" is still inside each reader. | `tool_load/locality.rs:154-216`; `stock/sim_triage.rs:944, 1713`; `compute/simulate.rs:1116`; `session/compute.rs:2271` |
| Drill has a second sample stream | The carve arm | A drill produces `DrillSample`s and a `DrillToolpathSummary`, never a `SimulationCutSample`. Every sample-based metric (histograms, chart, envelope placement, cut force) is empty for a drill by construction, not by a `match`. | `compute/simulate.rs:2213-2221` |
| `ProjectVerdict` | Variant list | Unknown, Collisions, RapidCollisions, HighAirCut, Ok. No variant for a load criterion that exceeds. Not checked whether another field carries that state to the headline. | `session/diagnostics_types.rs:503-521` |
| Static stepover floor | `adaptive_woc` | P28 row 8. | `compute/validate.rs:283` |

---

## 5. Display surfaces

### 5.1 The surfaces

Columns V L M B Ba: does the surface show the value, the limit, the margin, which limit
binds, and the basis. "calc" = the surface computes it from core values.

| Surface | File | Type(s) it reads | Where the type is built | V | L | M | B | Ba | Own filter by span or op | One record per cut would replace | It would NOT cover |
|---|---|---|---|---|---|---|---|---|---|---|---|
| Sim limit rows ("Other limits", Readiness) | `viz/ui/sim_diagnostics.rs:1125-1209`, `:1292-1420` | `CriterionStatus` from `ToolpathLoadVerdict::criteria()` | `tool_load::evaluate_toolpath`; memo `SimulationState::cached_load_report` | yes | yes (hover) | calc: `pct_of_bound` = peak / bound (`:1233-1240`) | no (one row per criterion; no "which binds") | yes: `BoundSource::setting()` | yes: drops rows with `NotApplicableForOp` (`:1146-1168`) | The whole row. `CriterionStatus` is already close to the record. | Population counts; the vacuous state; the two-sided chip band |
| Four cut-metric cards (chipload, power, deflection, cut force) | `viz/ui/sim_diagnostics.rs:1616-1672`, `:1957-2010`, `:2135-2215`; set built in `viz/state/simulation.rs:347-403` | `CutMetricSet { cards: Vec<CutMetricCard { metric, outcome: DistributionOutcome, series }> }` | `tool_load::metric_distribution` and `distribution::gate_population` (core); the series joined in viz | yes (peak, histogram) | yes (floor, ceiling in the histogram) | no number; shares of time below, in, above | no | yes (`MetricDistribution::bound_source`) | no own filter; a metric with no card for the op is absent (`metric_distribution` returns `None`); `CUT_METRIC_ORDER` fixes four metrics (`viz/state/simulation.rs:200-205`) | The headline: peak, limit, state, basis | The histogram (time-weighted bins) and the over-time series: these are distributions of many samples. A record per sample feeds them; one record per cut does not replace them. |
| Force readings under the cards | `viz/ui/sim_diagnostics.rs:2014-2056` | `CutMetricSet::samples: Vec<CutPoint3>` | viz computes min and max of `ap_mm` and `ae_mm / D` | yes | no | no | no | no | no | The reading, if the record carries ap and ae | - |
| Width x depth chart (simulation) | `viz/ui/sim_cut_chart.rs`; `viz/state/simulation/force_field.rs` | `SafeEnvelope` (limits), `CutPoint3` samples, `budget_n`, `ForceField` | Envelope: `ProjectSession::safe_envelope`. Samples: `cut_force::cut_points`. Field: built IN VIZ by calling core `force_budget::peak_force_n` on a 48 x 32 grid at the planned chip (`force_field.rs:93-113`). Over-budget dots: viz compares sample force with the budget. | yes (heat map, dots) | yes (limit lines) | no | no (a hover names a cell) | limit names only | The field and the budget exist only under `DepthCapRule::ForceBudget`, so an Adaptive-family op draws no contours (`viz/state/simulation.rs:437-452`; P28 row 34) | The planned dot and each sample dot become the same record, placed by the same limits. `SafeEnvelope::placement_of` and `placement_of_measured` already do this for one point each (`feeds/envelope.rs:1459`, `:1483`). | The limit CURVES (ap as a function of ae), the heat map, the iso-force field: these are 2D fields over the plane, not records. |
| Safe-envelope plot (Passes tab) | `viz/ui/feeds/envelope.rs` | `SafeEnvelope`: `boundary: Vec<Limit>`, `proposal`, `operator: Option<Placement>`, `speeds: SpeedsMap` | `SafeEnvelope::for_cut`, through `FeedsMemo::envelope` | yes (marks) | yes (every limit drawn and named) | no number on the face; `Placement` holds it | yes: `Placement::Outside { binding }` for the operator and the candidate (`:412`, `:789-793`, `:976-980`) | yes: hover = law, mm, basis | no | The mark, the placement, the binding limit, the proposal | The limit curves, the safe-region fill, the speeds grid under a drag |
| Nomogram (feed against RPM) | `viz/ui/feeds/explore.rs` | `FeedsExplain` (`recommended: FeedsResult`, `machine: MachineEnvelope`, the published band), `CutEfficiency`, `SpeedsSample` for the candidate | `explain_feeds`; `cut_efficiency` is CALLED IN VIZ per draw (`:131-137`); the `Corridor` type is built in viz (`:160-262`) | yes | yes (walls, band lines, floor, ceiling) | no | no | hover only | no | The marks and the two corridor bounds as records | The iso-chip rays and walls are lines in (rpm, feed), a different plane from (ae, ap). The walls read `MachineEnvelope` (spindle range, feed ceiling), not the P21 `RpmChoice::limits`; the named RPM limits are not drawn here. |
| Suggest card and its rows | `viz/ui/feeds/compare.rs`, `why.rs`, `viz/ui/properties/feeds_speeds.rs` | `FeedsPreview`, `AppliedRecipe`, `CutEfficiency`, viz `PowerReading { figure: Result<PowerFigure, PowerUnmodeled>, source: Option<BoundSource>, .. }`, `SuggestRationale`, `SuggestWarning`, `RpmChoice`, `EntryNotes`, `VendorBand` | Core doors: `power_at_shipped_point` (`compare.rs:875`), `cut_efficiency` (`compare.rs:260`), `SuggestRationale::from_warnings`. The card evaluates each twice (the applied cut and the calculator cut) to word a note. | yes | yes (power: `available_kw`; chip: band) | calc: power as a percent of available | partly: `RpmChoice::binding` (RPM); `proposal_binding` (depth) | yes: each row names its source (ruling R4) | no | Power, chip, deflection headroom, rubbing rows | The step chain ("every stage that moves a number is one row"): that is a history of one value, not a limit check. The vendor band member list. |
| Suggest rationale | core `feeds/rationale.rs:104-145` | `RationaleEntry { param, reason, from_value, to_value, headline, detail }` | `SuggestRationale::from_warnings(&[SuggestWarning])` | from and to | no | no | the `reason` enum names it in part | text | no | Entries whose reason is "a limit lowered this" | Entries that are advice (strategy, plunge stability) |
| Toolpath lineage | `viz/ui/properties/lineage.rs`; MCP reply `viz/state/freshness.rs:328-351` | `trace::lineage::Lineage { cut, pick, speeds, tool_facts, entry, findings, rules: Vec<RuleResult>, measured: Option<MeasuredLineage>, steps }` | `ProjectSession::lineage_measured` | yes | only in `rules` and `measured.judged` | no | `measured.binding` | yes (the purpose of the surface) | no | `rules` (K5) and `measured` (worst sample: value, limit, binding) | The step records, the tool facts, the entry counts |
| Warnings tab; Readiness check cards; chips | `viz/ui/sim_diagnostics.rs:573-1010`; `viz/ui/readiness*.rs`, `preflight.rs` | `SimulationTriage` of `Finding { class, diagnostic: Diagnostic, dedup_key, occurrences, worst: WorstEvidence }` | `stock::sim_triage`; the `Diagnostic` adapters in `diagnostics/adapters/` | in the message; sometimes in `evidence` | `evidence.threshold` (optional) | no | no | `Diagnostic::confidence`, `source` | no own filter found | A finding that is "value over limit" could be made FROM a record | Findings with no number: collisions, generated-empty, stale state, missing inputs |
| MCP `get_tool_load_report` | `viz/app/mcp/generation.rs:356-398` | `ToolLoadReport` serialised whole; `ToolLoadReportSummary`; `per_depth_pass` | The report: core memo. `per_depth_pass`: built in viz by walking the samples and `SpanKind::DepthPass` spans into core `SummaryAccumulator`s (`viz/app/mcp/simulation.rs:1017-1090`) | yes | yes | no | no | yes (`bound_source`, `confidence`) | `per_depth_pass` groups by DepthPass spans in viz | Each verdict arm | `per_depth_pass` is a grouped summary (a record per pass would be the natural form). Drill gates use another outcome type. |
| MCP `get_suggest_rationale` | `viz/app/mcp/generation.rs:57-236`, `:251-300` | `CutterOpProfile`; hand-built JSON `basis` (`suggest_basis_json`), `SuggestRationale`, `reached_feed_share` | `ProjectSession::cutter_op_profile`; the JSON object is assembled in viz | yes | partly (`shipped_power`, `rpm.limits`) | no | `rpm` binding | yes (headline and detail pairs) | no | `shipped_power`, the RPM limits | The prose pairs; the vendor band |
| MCP `get_safe_envelope` | `viz/app/mcp/generation.rs:1062-1114` | `SafeEnvelope` serialised; the grid capped in viz | `ProjectSession::safe_envelope` | yes (grid) | yes (`boundary`) | `operator` placement margin | `proposal_binding`, `operator.binding` | per limit yes; per grid point no (`value_only`) | no | `proposal`, `stated`, `operator` | The grid and the edges (fields) |
| MCP `get_toolpath_diagnostics`, `get_project_diagnostics` | `viz/app/mcp/diagnostics.rs:681-709` | `Vec<Diagnostic>` | `ProjectSession::diagnose_toolpath_with_evidence`, `diagnose_project_with_evidence` | message; `DiagnosticEvidence` | `threshold` optional | no | no | `confidence`, `source` | no | as the Warnings tab | as the Warnings tab |
| MCP `get_cut_trace` | `viz/app/mcp/simulation.rs:80`, `:510` | `SimulationCutTrace`: semantic and span summaries, hotspots, issues, drill summaries, capped moves | core trace; filters by `span_kind`, `span_id`, `pass_index` on request | peaks (`peak_chipload_mm_per_tooth`, `peak_axial_doc_mm`, `peak_plunge_descent_mm`) | no | no | no | no | by request only; the kind synonyms come from core `DebugSpanKind::structural` (`viz/app/mcp/diagnostics.rs:725-745`) | The peaks, if each span summary were a set of records | Time and distance sums; the hotspot list. Note: `peak_chipload_mm_per_tooth` here is the advance, while the gate reads the peak chip (`tool_load/CLAUDE.md`: "open gap"). |
| MCP `inspect_spans` | `viz/app/mcp/diagnostics.rs:558`, `:837` | The span tree of the result | core spans | structure only | no | no | no | no | by request | Nothing (no load value) | - |
| MCP `get_toolpath_lineage` | `viz/state/freshness.rs:328-351` | `Lineage::to_json` | core | as the lineage row | | | | | no | as the lineage row | as the lineage row |
| CLI `suggest` | `cli/src/suggest.rs:86-150` | `CutterOpProfile`, `FeedsResult` fields picked by hand into JSON, `shipped_power_json`, `SafeEnvelope`, `SuggestRationale` | core; the `calculator` object is assembled in the CLI | yes | partly | no | `rpm_choice` | yes (card text pairs) | no | as MCP `get_suggest_rationale` | as MCP |
| CLI `project` summary and stderr report | `cli/src/project.rs:561-722`, `:1184-1381`, `:1590-1670` | `ProjectDiagnostics`, `ProjectVerdict`, `SimulationTriage`, `gcode::warnings_report` | core | air-cut shares, verdict headline, finding list | no | no | no | finding ids | no | The findings made from records | Runtime totals |
| CLI `smoke` | `cli/src/smoke.rs:27-33`, `:1027` | `ToolLoadReport`, `DrillGatesVerdict`, `ChipSide` | core | not read in detail | | | | | | | |

### 5.2 The verdict, finding and status types on the display path

Side by side. "pop" = the population (how many samples reached the comparison).

| Type | Where | Value | Limit | Margin | Binding | Basis | State | Other |
|---|---|---|---|---|---|---|---|---|
| `ChiploadVerdict` | `tool_load/verdict.rs:1786` | `ChiploadMetric.observed_mm_per_tooth` (two metrics when Within) | `ChipBounds` (min optional, max, source) | none | `side` on Exceeds | `ChipBounds.source`; `Confidence` | Within / Exceeds / Unmodeled | `entry_spikes`, `burn_advisory`, statistic (median or peak) |
| `PowerVerdict` | `:1912` | `peak_kw` | `available_kw` | none | none | `bound_source: Option<BoundSource>`; `Confidence` | Within / Exceeds / Unmodeled | `entry_spike`, `SampleEvidence` |
| `DeflectionVerdict` | `:2115` | `peak_mm` | `DeflectionBounds { validated_within_mm, exceeds_mm, basis }` | none | none | `DeflectionBasis`; `Confidence` | same three | `entry_spike` |
| `DepthVerdict` | `:2242` | `peak_mm` | `RigidityDepthCap` or none | none | none | through the cap; `Confidence` | Within / Exceeds / Reported / Unmodeled | a fourth state: Reported (no bound) |
| `DrillGateOutcome` x 3 | `tool_load/drill_gates.rs:24` | `observed` | `threshold`, `envelope_lo`, `envelope_hi` | none | none | none on the type (`BoundSource::DrillEnvelope` on the row) | Within / Exceeds + `DrillGateSeverity` | no Unmodeled arm |
| `CutForceReading` | `tool_load/cut_force.rs:159` | `peak_n` | `budget_n: Option` | none | none | `rule: String` | none (report only) | pop, `peak_sample_index` |
| `CriterionStatus` / `CriterionRow` | `tool_load/verdict.rs:1223`, `:648` | `display_peak` | `bound` | none | none | `bound_source`, `confidence` | `LoadState`; `unmodeled_reason`; `exceeded` | pop; unit; `sample_range`. The common projection of the six types above. |
| `MetricDistribution` | `tool_load/distribution.rs:331` | histogram | floor and ceiling in the histogram | shares of time | none | `bound_source` | `Option<LoadState>` | pop |
| `Limit` | `feeds/envelope.rs:379` | none (it is the limit) | `edge` (ceiling, floor, curve, per point, unmodelled) | - | - | `basis: Basis`, `law` | - | `kind: LimitKind` (14 kinds) |
| `Placement` | `feeds/envelope.rs:573` | none | none | `margin` (share of the limit) | `binding: LimitKind` when Outside | none | Inside / Outside | |
| `JudgedValue` | `feeds/envelope.rs:556` | `measured` | `limit` | none | none (the caller holds it) | none | none | `axis`, `unit`, `feed_mm_min` |
| `MeasuredPower` | `feeds/envelope.rs:534` | `required_kw` | `available_kw` | `margin()` | - | none | none | `feed_mm_min` |
| `SpeedsPoint` | `feeds/envelope.rs:592` | rpm, feed, plunge, chip, `power_kw` | `available_kw` | none | none | `Sourced` inside, not serialised | flags `fits_power`, `rubbing`, `feed_capped` | `power_case`, `power_width_mm` |
| `PowerFigure` / `PowerUnmodeled` | `feeds/operating_point.rs:136`, `:74` | `required_kw` | `available_kw` | none | none | none | Ok / 7 named refusals | ap, ae, rpm, feed |
| `RpmChoice` / `RpmLimitValue` | `feeds/mod.rs:1185-1215` | `rpm` | `limits: Vec<{limit, rpm}>` | none | `binding: RpmLimit` | the limit name | none | `chart_rpm` |
| `DeflectionPrediction` | `feeds/predict.rs:649` | `predicted_um` | none | none | none | `caveat` | Ok / `DeflectionUnmodeled` | `breakdown` |
| `CutEfficiency` | `feeds/efficiency.rs:84` | advance, `u`, ploughing share | `band`, `rubbing_floor_mm`, `deflection_ceiling_mm` | `force_headroom` (signed), two ratios | none | none | `ChipVerdict` Thin / InBand / Heavy / NoBand | |
| `CutterAxialConstraints` | `feeds/cutter_constraints.rs:120` | none | four depth bounds | none | `binding_constraint` | none | none | |
| `RuleResult` (K5) | `compute/feasibility.rs:234` | `measured` | `limit: Option` | none | - | `fact: FactKind` | present = warns | `text`, `id` |
| `EntrySpike` | `tool_load/verdict.rs:1665` | `observed` | `bound` | none | none | none | advisory | `locality`, `side` |
| `EntryLoadObservation` | `stock/sim_triage.rs:1334` | `peak_entry_bite_mm` | `threshold_mm` | none | none | none | by count | sample counts |
| `PlungeStressWarning` | `tool_load/plunge_stress.rs:55` | `plunge_rate_mm_min` | `safe_cap_mm_min` | none | none | none | present = warns | |
| `ReachedFeedShare` | `machine/kinematics.rs:1397` | two times; `share()` | none | none | none | none | none | |
| `Diagnostic` | `diagnostics/mod.rs:245` | in `message`; `DiagnosticEvidence::{SampleRange, LutCitation, GeometryCompare}.observed` | `threshold` / `min` / `max` (optional) | none | none | `confidence`, `source` | `severity`, `state` | `id`, `face`, `anchor`, `fix`, `supersedes` |
| `Finding` (triage) | `stock/sim_triage.rs:185` | `worst: WorstEvidence` | none | none | none | through the `Diagnostic` | `class` | `occurrences` |
| `RationaleEntry` | `feeds/rationale.rs:104` | `to_value` | none | none | `reason` | text | none | `from_value` |
| `SuggestWarning`, `FeedsWarning` | `feeds/suggest.rs`, `feeds/mod.rs:1396` | per-variant fields | per-variant | none | per-variant | none | present = warns | large enums; each variant has its own fields |
| `ProjectVerdict` | `session/diagnostics_types.rs:503` | a count or a percent | none | none | none | none | 5 kinds | |
| `ToolpathStats` findings | `compute/toolpath_stats.rs` | per-type | rarely | none | none | rarely | `None` = not measured | about 30 types |

Overlap that the table shows:

1. Value plus limit, with no margin field, appears in 13 types. The margin is a stored field
   only in `Placement`, `MeasuredPower::margin()` and `CutEfficiency::force_headroom`. Every
   other surface divides by itself or shows none.
2. "Which limit binds" exists in four unrelated forms: `Placement::Outside::binding`
   (`LimitKind`), `RpmChoice::binding` (`RpmLimit`), `CutterAxialConstraints::binding_constraint`,
   `tool_load::verdict::BindingConstraint` (the modulator, `verdict.rs:32`). The post-sim
   gates have none: each gate is independent by design ("no scalar load %").
3. Three limit name lists for the same physical limits: `LimitKind` (14, pre-sim),
   `CriterionKind` (8, post-sim), `BoundSource` (7). Power is `LimitKind::Power` /
   `CriterionKind::Power` / `BoundSource::MachinePowerCurve`. The GUI adds its own short names
   (`limit_name`, `viz/ui/feeds/envelope.rs:162-175`; `CutMetricSpec::title`,
   `viz/ui/sim_diagnostics.rs:1627-1672`).
4. The envelope side (`Limit` + `JudgedValue` + `Placement`) and the gate side
   (`CriterionStatus`) are each about three quarters of the target record. The envelope side
   has margin, binding and basis and no state or population. The gate side has state,
   population and bound source and no margin or binding.
5. "Not modelled" has five forms: `UnmodeledReason` (gates), `PowerUnmodeled`,
   `DeflectionUnmodeled`, `Edge::Unmodelled { reason }`, `NoDepthProposal`; plus `None` on
   `FeedsResult::power_kw` and on each `ToolpathStats` field.

### 5.3 Surfaces that compute a value themselves

Each calls a core law; none holds a formula of its own that I found. They still decide WHAT
to evaluate, so two surfaces can show different numbers for one cut.

| Surface | What it computes | Evidence |
|---|---|---|
| Sim limit rows | percent of bound = peak / bound | `viz/ui/sim_diagnostics.rs:1233-1240` |
| Sim chart | the iso-force field: core `peak_force_n` on a grid at the planned chip `steady_feed / (rpm x flutes)` assembled in viz | `viz/state/simulation.rs:437-452`; `viz/state/simulation/force_field.rs:93-113` |
| Sim chart | which samples are over the budget (sample force against `budget_n`) | `viz/ui/sim_cut_chart.rs` header, "Over-budget dots" |
| Sim cards | min and max of sample depth and width | `viz/ui/sim_diagnostics.rs:2026-2056` |
| Sim cards | micrometre scaling of deflection (`scale: 1000.0`) | `viz/ui/sim_diagnostics.rs:1640-1647` |
| Nomogram | calls `cut_efficiency` at draw time; builds the `Corridor` | `viz/ui/feeds/explore.rs:131-137`, `:160-262` |
| Suggest card | calls `cut_efficiency` and `power_at_shipped_point` twice each (applied cut, calculator cut) and words the difference | `viz/ui/feeds/compare.rs:258-296` |
| MCP `get_tool_load_report` | `per_depth_pass`: groups samples by DepthPass span | `viz/app/mcp/simulation.rs:1017-1090` |
| MCP `get_suggest_rationale` | assembles the `basis` JSON object field by field; adds `reached_feed_share` | `viz/app/mcp/generation.rs:57-236`, `:275-292` |
| MCP `get_safe_envelope` | caps the grid | `viz/app/mcp/generation.rs:1081-1104` |
| CLI `suggest` | assembles the `calculator` JSON object field by field | `cli/src/suggest.rs:99-128` |

---

## 6. The five facts that most constrain a design

1. **Two record shapes already exist, and each is most of the target.** `CriterionStatus`
   (`tool_load/verdict.rs:1223`) is the post-sim record: kind, value, bound, bound source,
   state, population. `Limit` + `JudgedValue` + `Placement` (`feeds/envelope.rs:379, 556, 573`)
   is the pre-sim record: limit, basis, value, margin, binding. `SafeEnvelope::placement_of`
   and `placement_of_measured` already place a planned point and a simulated sample by the
   same limits. A new record should join these two; a third shape beside them adds to the
   count.
2. **The op decides the EMITTER, and the emitter decides the entry rule.** `EntryContract`
   makes every op state how it enters, and a new op does not compile without it. The rule
   values (ramp 3 / 10 / 12 deg, pitch 1 / 2 mm, plunge feed by two formulas) live in each
   emitter. An entry is not a declared cut: `CutSpec` has no entry member, and
   `feeds::ramp::entry_geometry` sees the 3D style through one `if let`. To judge an entry by
   the limits, the entry must become a cut with its own width (a slot), depth and slope.
3. **Many metrics are distributions or 2D fields, not one number per cut.** The histograms,
   the over-time series, the heat map, the limit curves `ap(ae)`, the iso-force field and the
   speeds grid cannot be one `{value, limit, margin}` record. They can be built FROM records
   (one per sample) or from a limit evaluated over a grid. The record must be cheap enough to
   make per sample (the wanaka traces hold tens of thousands of samples), or the per-sample
   path keeps its own lighter type.
4. **A drill is outside the sample model.** It has analytic removal, its own samples
   (`DrillSample`), its own gate outcome type with no Unmodeled arm, no `SimulationCutSample`,
   no body-zone walk (4.2) and no air-cut figure. 13 call sites branch on
   `is_drill_kinematics()`. "Does not apply from the cut's geometry" must cover it, or the
   drill keeps a parallel path.
5. **The basis has five vocabularies and the measured values have none.** `Basis` (pre-sim
   values and limits), `ValueProvenance` (stored fields), `BoundSource` (gate bounds),
   `FactKind` (tool facts), `FormulaConstant` (constants). A gate reading, a cut force, an
   efficiency figure and an entry threshold carry no basis. "basis: sourced / calculated by a
   rule / no number" needs one of these to become the common one; `BoundSource::gates_export`
   and `setting` are behaviour that the others do not have.

## 7. What surprised me

- The dressup entry plunges at half of the first linear feed of the toolpath (else 500
  mm/min). It does not read the op's `plunge_rate`, which Suggest sizes through the
  vertical-rate limit (`entry_contract.rs:236-266`, `dressup_apply.rs:746`).
- A fallback ramp in the 3D planner uses the DRESSUP angle (3 deg), not the 3D op's angle
  (`adaptive3d/path.rs:2560`).
- Three ramp angles are in the product: 3, 10 and 12 deg. The 12 deg one has no dial.
- A drill entry in the simulation builds no `WalkChecks`, so the shank, holder and shoe zones
  are not walked for it (`compute/simulate.rs:2187-2222`). I did not find another check that
  covers them; the mesh holder check `check_collisions` may (not read).
- The chord law of the scallop exists in four places, with different radius reads for a
  tapered ball.
- No surface shows the cusp height that a stated stepover gives.
- Two measures of "how much of the commanded feed the machine reaches" exist
  (`ReachedFeedShare`, `ToolpathKinematicUtilization`), and only MCP shows the first.
- `cut_efficiency` does not use the K7 engagement or the `d_eng` door; it reads the nominal
  diameter and one planned width.
- The rubbing check uses `min(0.025, band minimum)` on the commanded advance before the
  simulation and the band minimum on the median peak chip after it.
- The nomogram's walls come from `MachineEnvelope`; the named RPM limits of P21
  (`RpmChoice::limits`) are shown only as a hover and by MCP and the CLI.
- `SpeedsPoint` holds `Sourced` values and serialises them as bare numbers.
- `ProjectVerdict` has no variant for a load criterion that exceeds.
- Opinion: `Capabilities` is already the "one declared table" that P28 section 4 sketches,
  but keyed on the op. Most of its metric fields (`axial_envelope`, `move_count`,
  `air_cut_high_threshold_pct`, `model_gouge`) would move to the cut or become a function of
  it; the fields that describe dressup order and territory would stay on the op.
- Opinion: the entry rules are the largest part of item 4 that P28 did not reach. They need
  an operator ruling of their own (which angle, which pitch, which plunge feed), because no
  source prints any of them (rulings Q8, Q9 are named in the code).
