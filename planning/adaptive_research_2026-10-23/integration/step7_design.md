# Step 7 design: the "Geodesic Adaptive" product variant

Date: 2026-10-27. Design only; this step changes no code.
Inputs: INTEGRATION_PLAN §4 step 7, §7 (lead defaults Q1–Q11), §8 (lead
update after D43–D47).

Tree names in this file:
- **R** = research branch `adaptive-instruments` at `453d52571` (D48, the
  last commit). The working tree of `adaptive-head` holds uncommitted work
  after it; this design does not read that work.
- **M** = `geodesic-integration` at `5561fdb5f` (master `ab208f83a` +
  step 1 + step 1c).
- Paths are under `crates/rs_cam_core/src/` unless they name a crate.

## 0. Where the two trees stand

| Part | R | M |
|---|---|---|
| 2D research module `adaptive/research_geodesic.rs` | 10 935 lines, presets G1…C37 | 6 028 lines, presets G1…C6_LOCAL_JOIN (:1554); the merge-base copy |
| 3D arm `ClearingStrategy3d::Geodesic` | `adaptive3d/mod.rs:108`, `cfg(research)` | absent |
| research gates in the 3D layer | `cfg(research)`: 93 in `adaptive3d/clearing.rs`, 41 in `path.rs`, 33 in `mod.rs` | 0 |
| `adaptive3d/research_outer_band.rs`, `adaptive3d/keep_down_route.rs`, `adaptive/slab_field.rs` | 558, 209, 280 lines | absent |
| `Move::feed_cap` (step 1), D47 export gate (step 1c) | present | present |
| `Produced::rest_regions`, `BoundarySource::DerivedRestRegions` | 3D rough may publish (D44, D48) | finishing rest detector only (`trace/produced.rs:47`, `finish/rest_detector.rs:138`) |

So step 7 on M is not a "move": the 3D arm and its 3D-layer hooks do not
exist on M. Each piece is ported from R onto M's current 3D layer, without
the research gates, in the order of §5.

## 1. The preset to freeze, and what to delete

### 1.1 The preset

Freeze `GeodesicOptions::C35_SPIRAL_PICK` (R `adaptive/research_geodesic.rs:2395`).
Resolved through its chain (`C28_ZIGZAG_PICK` :2389 → `C20_LOCAL_ZIGZAG`
:2357 → `C16_LOCAL` :2336 → `C2` :2249):

| Option | Value | Enum (R line) |
|---|---|---|
| solver | `SourceTracking` | `FieldSolver` :401 |
| meeting | `Split` | `MeetingRule` :1929 |
| join | `Seam` | `JoinRule` :1946 |
| spacing | `LocalLevel` | `Spacing` :2053 |
| reseed | `Off` | `Reseed` :2089 |
| arcs | `Zigzag` | `ArcLink` :2127 |
| chaining | `Whole` | `Chaining` :2075 |
| ends | `Default` | `ArcEnds` :2149 |
| pick | `Replay` in R; **`Off` in the product** (Q8, D52) | `ArcPick` :2170 |
| core | `Spiral` | `CoreJoin` :2196 |

**Q8 is settled (D52, INTEGRATION_PLAN §8):** the replay pick does not
ship. On the measured jobs it kept plan A in all 9 regions, and the runs are
deterministic. The product preset is C35 plan A: the table above with the
pick `Off` and the default ends (`pick_variants(C35_SPIRAL_PICK).0`, R
:2411). Plan B (`ends: HeadOn`, `meeting: Through`) is not product code.

### 1.2 What the product keeps and deletes

The product module (§2) holds no options struct. It holds the C35 path as
plain code, with no switch at all (Q8: no pick). The
research file keeps `GeodesicOptions` for its own tests only until step
7.6 deletes it (§5).

Delete (repo rule: delete, not archive):

| Value or preset | Code it removes (R lines, approximate) |
|---|---|
| `FieldSolver::FirstOrder` | the first-order path in `GeodesicField` (:662–1766). INTEGRATION_PLAN §1 item 1: it is `finish::scallop_isofield::fast_sweep`; the field calls that solver if a first-order solve is ever needed |
| `JoinRule::{Ramp, Morph, Transition}`, `LobeMorph` | `transition_fraction`/`transition_wrap` :3327–3487; `lobe_length_ratio` … `morph_plan` :3718–4567 |
| `Spacing::{Plain, Curvature, CurvatureLocal}` | `curvature_levels` :3488 and the per-option branches; keep `law_spacing`/`law_next_radius` (:3278–3293) if `LocalLevel` calls them (the compiler shows it) |
| `Reseed` (whole enum) | `cleared_nodes` :5950, `reseed_segments` :6047, `seam_joins` :6189, `far_side_zone` … `split_segments` :6239–6977 (C9, C11) |
| `ArcLink::{Nearest, OneWay}` | their branches in the arc order |
| `Chaining::TailOnly` | the tail-only branch; `chain_whole` :1660 stays |
| `CoreJoin::Seam` | the seam core branch; `core_spiral`/`held_core_spiral` :3539–3717 stay |
| `ArcPick` (whole enum), `ArcEnds::HeadOn`, `MeetingRule::Through`, `pick_variants`, `slice_variants` | Q8 (D52): no pick. The pick score (`pick_score` :4724, `pick_kept` :4753) and the pick's use of `ReplayModel` go; `ReplayModel` (:5728) goes too if no other C35 path reads it (7.3 part c checks) |
| presets `G1` … `C37_SPIRAL_HEADON` (:2234–2406) | all 25; the C12 judge (R `tests/adaptive3d_geodesic_arm_c12.rs`) runs the product arm instead |
| `RESEARCH_ARM_ENV` and `research_arm_override` | R `adaptive3d/mod.rs:123`, :128–165, and the `AgentSearch` hijack at :193 |
| `RESEARCH_ROUTE_ENV`, `RESEARCH_STEP_SPLIT_ENV`, the slab-field env | R `adaptive3d/mod.rs:439–528` |

## 2. The module layout of `adaptive/geodesic/`

The 2D engine (`adaptive/`) owns the field, the wraps, the arcs and the
per-piece plan (INTEGRATION_PLAN §0 item 1). The 3D slice layer reaches it
through one door, `slice.rs`. Sizes are estimates from the R line spans
minus the deleted options; the move step measures them.

| File | Holds | From (R `adaptive/research_geodesic.rs`) | Est. lines |
|---|---|---|---|
| `mod.rs` | the door: `pub fn plan_piece_c35(region, params, cancel)` (no pick); the module doc (the "Derived constants" list, :255–296, cut to the kept rules) | :1–340 (doc), :4976 `planner_cell` | 250 |
| `field.rs` | `Lattice` (:357), `Walls`, `Branches`, source tracking (`track_sources` :525, `sweep_sources` :564), `GeodesicField` (:662) with `radius_at`, `star_levels` (:1523) | :343–1600 | 1 100 |
| `wraps.rs` | chaining (`coincide` :1602, `join_fragments` :1609, `chain_whole` :1660, `Chain` :1767–1924); the meeting line (`collect_chains` :2542, `parents_of` :2575, `meeting_trim` :2601); `ordered_wraps` :2845, `beside_core` :2881, `ordered_wraps_beside_core` :2898; the core spiral (`SpiralCore` :2868, `core_spiral` :3539, `trace_core_spiral` :3585, `held_tilt` :3630, `held_core_spiral` :3670); `EntryLaw` :3124; the spacing law :3251–3300 | as listed | 900 |
| `arcs.rs` | arc ends and lenses (`end_at_shock` :2453 … `nearest_on` :2511, `climb_oriented` :2653, `right_growth` :2663, `arc_end_lens` :2712, `wall_normal` :2741, `end_lens_at` :2792); zigzag order (no pick score: Q8) | as listed | 500 |
| `plan.rs` | `clearance` :5002, `append_path` :5022, `geodesic_segments` :5062, `plan_piece` :5128–5727 (`ReplayModel` :5728–5949 only if a C35 path reads it without the pick) | as listed | 800 |
| `slice.rs` | the 3D-facing door: `SlicePlan` :8194, `CoreZone` :8214, `core_rapids` :8287, `plan_rapids` :8302, `classify_reentries` :8351, `slice_segments` :8381, `slice_variants` :8469, `slice_cleanup` :8485, `slice_replay` :8500, `slice_turn_records` :8527, `TurnKind` :8630, `SliceTurns` :8655, `link_length` :8904, `slice_load_margin` :8943, `split_at_turns` :8971, `HeldLinks` :9059 | :8194–9102 | 850 |
| `CLAUDE.md` | 40 lines or fewer: file map, the C35 rules, the invariants (§4.6), the sentries | new | ≤40 |

No file passes about 1 100 lines; `field.rs` and `plan.rs` are the
largest. If `plan_piece` stays one 600-line function, split its zigzag
and core phases into `plan.rs` helpers at the move; do not change logic in
the same patch.

**What stays in `adaptive/research_geodesic.rs` as test support**
(`cfg(research)`): the reports (`FieldReport` :4568, `ArcReport` :4619,
`JoinReport` :3371, `LobeReport` :3740, `SplitReport` :4831), the readout
(`PassLoadReadout` :6826, `pass_load_readout` :6978, `Refusal` :7175),
the cascade (`CascadeReadout` :7249, `CascadeWalk` :7314,
`cascade_readout` :7427), the spans (`spans_of` :7547), `TurnRecord` :7653,
`holding_move` :7700, and the 2D harness door (`run_2d` :7992,
`run_2d_with` :8024). These read the product module through its `pub(crate)`
items. After step 7.6 the file has no plan code of its own.

**The feed records.** `turn_feed_factor` (:7749) is the one planner-side
feed rule (INTEGRATION_PLAN D6). It moves to `slice.rs`. `TurnRecord` and
the two match functions (`apply_slice_turn_feeds` :8770,
`apply_chain_feeds` R `adaptive3d/research_outer_band.rs:357`) are the
part D6 replaces: the cut segment carries its factor and the emitter
writes `Move::feed_cap` (§3, part F). Until part F lands, the arm keeps
the match functions in `slice.rs`, without `cfg`.

## 3. The shared parts M lacks

"Shared" means every slice arm (AgentSearch, ContourSpiral, Geodesic) or
every 3D rough may call it; its strategy test sits at the call site
(C38 §13.8). "Arm-only" means only the geodesic variant calls it.

| Part | What (R) | Home on M | Shared or arm-only | Default for shipped arms | Order |
|---|---|---|---|---|---|
| A. Load function (D43, Q1) | `strips_chip_load` R `adaptive/slab_field.rs:187`; caller `step_load` R `adaptive3d/clearing.rs:1388`, :1449 | `tool_load/` beside the power gate, one function (INTEGRATION_PLAN §1 item 14). `SlabField` (the per-cell top for the 2D grid, :35) is NOT ported: it is off by default (D34) and C35 + D43 do not read it | shared | not called | 1 |
| B. `StepSplit::StockDepth` (D43, Q6 reversed) | enum R `adaptive3d/mod.rs:374`, `effective_step_split` :530, split sites R `adaptive3d/clearing.rs:4608`, :4656 | `adaptive3d/mod.rs` + `clearing.rs` | shared enum; the arm's default is `StockDepth` | `PathShape` (today's rule) | 2 |
| C. Keep-down proof (step 2) | `keep_down_excess` R `adaptive3d/clearing.rs:1317`; held-link proof `check_held_link` :2960; `HeldLinks` (§2, `slice.rs`) | new `adaptive3d/keep_down.rs` (INTEGRATION_PLAN D1). `corridor_ceiling_z` (M `adaptive3d/clearing.rs:872`) becomes its max-form wrapper | shared | CP switches only in step 2 proper (its own re-bless); here the arm calls it, CP keeps `link_footprint_ok` (M :844) until step 2 | 3 |
| D. Routed keep-down travel | `keep_down_route` R `adaptive3d/keep_down_route.rs:82`; `routed_entry` R `clearing.rs:1584`; `KeepDownRoute` R `mod.rs:548` | `adaptive3d/keep_down.rs` with part C | shared; arm default `CutFeed` (Q7: no low rapid) | `Off` | 4 |
| E. Outer band and clip (C38 §16, C31) | `clip_to_stock_box` R `research_outer_band.rs:42`, `clip_to_centre_reach` :182, `boundary_reach` :120, `ResearchClip` :131, `research_grown_outlines` R `clearing.rs:3867` | box clip → `geometry/`; reach → `adaptive3d/centre_clip.rs`; `boundary_reach` → `adaptive3d/path.rs` boundary clear (M :811) | geometry shared; the call is arm-only now. G-CLIPAIR for CP is step 3, not here | not called | 5 |
| F. Feed factor on the cut (D6) | today: `apply_slice_turn_feeds`, `apply_chain_feeds` (match by XY) | `Adaptive3dSegment::Cut` carries a factor; the RDP joins equal factors; `segments_to_toolpath` (M `adaptive3d/path.rs:1429`) writes `Move::feed_cap` | shared IR; only the arm writes a factor | factor 1 → `feed_cap: None` → byte parity | 6 (may follow 7.4; see §5) |
| G. C22a replay after the short-group filter | `filter_short_groups` R `clearing.rs:3655`, replay loop R :3446–3520 calling `slice_replay` | `adaptive3d/clearing.rs`, one filter for every slice arm (INTEGRATION_PLAN §1 item 12) | shared filter; the replay is arm-only until a measure shows it for AgentSearch/ContourSpiral | AgentSearch/ContourSpiral keep today's filter output | 7 |
| H. Rest-region publication (D44 + D48 + D50) | see §3.1 | `trace/produced.rs`, `adaptive3d/`, `compute/execute/`, `session/compute/generation.rs` | the channel is strategy-neutral (§8 step 1b); the producer (left pieces) is arm-only, because only the arm records left pieces | none publish | step 1b, before 7.5 |

### 3.1 Part H in detail: the rest-region channel (D44, D48, D50)

M already has the channel's consumer side: `Produced::rest_regions`
(M `trace/produced.rs:47`) and `BoundarySource::DerivedRestRegions`, fed
today by the finishing rest detector (M `finish/rest_detector.rs:138`).
Part H adds a 3D Rough as a producer and bounds what it publishes in XY
(D48) and in Z (D50).

| Piece | What it does | R location | M home |
|---|---|---|---|
| D44 collect | each level records the pieces the arm left to a rest pass, with the level Z | `ClearZLevelContext::research_left_pieces` R `adaptive3d/clearing.rs:470`, push :3276, read :2429; carried out R `adaptive3d/path.rs:612`, :1290 | `adaptive3d/clearing.rs` context; renamed `left_pieces` (no `research_`) |
| D44 publish | the union over the levels, grown by one planner cell, becomes `Produced::rest_regions` | `left_regions` R `adaptive3d/research_outer_band.rs:84`; call R `adaptive3d/mod.rs:1406–1418` | a new `adaptive3d/rest_regions.rs` (geometry only, C38 §13.8), called from `adaptive3d/mod.rs` |
| D44 precedence | the 3D Rough's own regions win over the generic rest analysis; the analysis then attaches only its grid. The rough publishes only when a `DerivedRestRegions` boundary names it | R `compute/execute.rs`, `run_volumetric` R `compute/execute/volumetric.rs:142` | the same two places on M (M `volumetric.rs:142`) |
| D48 XY frame (G-RESTFRAME) | every producer publishes TOOL-CENTRE positions. The rough clips its union to its own containment polygons (`Adaptive3dGeometry::centre_boundary`, already on M), so a consumer with `Center` containment and a tool no larger than the rough's stays inside the rough's boundary | R `research_outer_band.rs:84–118`; doc R `trace/produced.rs:52–61` | `rest_regions.rs`; the doc on `Produced::rest_regions` |
| D50 Z span (in progress on R, uncommitted at 453d52571) | each left piece carries the top Z of the slab it leaves (`slab_top_z`: the next level above, or the level anchor). `left_regions` also returns the highest top of a piece that meets the clipped regions. `Produced` gains `rest_regions_top_z: Option<f64>` (one value for the set, shifted by `shift.z` in the datum move). The consumer reads it through `ProjectSession::derived_rest_top_z` (`session/compute/generation.rs`) into `Adaptive3dDepth::rest_top_z`; on remaining stock the level plan starts no higher (a ceiling on the G-LADDERANCHOR anchor, `adaptive3d/path.rs`). On fresh stock it is not read | R working tree: `research_outer_band.rs` (`slab_top_z`), `trace/produced.rs`, `adaptive3d/mod.rs`, `path.rs`, `compute/execute.rs`, `rough_3d.rs`, `volumetric.rs`, `session/compute.rs`, `generation.rs`; sentry `a_rest_consumer_starts_at_the_left_stock_top_d50` | port only after R commits D50; reread then. `slab_top_z` → `rest_regions.rs` |

Order inside part H (step 1b, then 7.5):
1. Step 1b (strategy-neutral): the `Produced` field `rest_regions_top_z`,
   its datum shift, the precedence in `compute/execute.rs` and
   `run_volumetric`, the consumer read (`rest_top_z` into the 3D Rough's
   depth), and the D48 frame rule in the `Produced` doc. No producer yet,
   so every op's output is unchanged: byte parity. Sentries: the D48 and
   D50 tests run with a hand-set `Produced` (a planted source), since no
   rough publishes yet.
2. Step 7.5: the arm's producer (collect, `left_regions`, `slab_top_z`).
   Sentries: R's D44, D48 and D50 tests ported.

Open points for part H:
- D50 keeps ONE top for the whole set. A region far below the highest
  piece then starts its levels too high and cuts air down to its own
  slab. A per-region top is the exact form. Record as a later item unless
  D50's measurement shows the air time matters.
- The consumer must not cut below its own floor because of `rest_top_z`:
  it is a ceiling on the start, never a floor. D50's sentry must state
  that.
- 7.5 test: a consumer with a LARGER tool than the rough's can still leave
  the rough's boundary (D48 holds for "no larger" only). The GUI template
  (Q-T) must pick the edge tool with that rule, or the consumer refuses.

Notes:
- Part H has one typed number: the grow `tool_radius / 6.0` (R
  `adaptive3d/mod.rs:1413`). Its comment says "grown by one planner cell".
  Use `planner_cell(params)` (R `research_geodesic.rs:4976`). Do not port
  the typed `6.0`.
- Part A replaces the box and integral forms of L (INTEGRATION_PLAN D5).
  The arm's planner, the sim report and the C12 judge call the one
  function.
- Part G's 3D replay is the arm's G-departure fix. AgentSearch and
  ContourSpiral have the same 3D filter. Their adoption is step 5 (slice
  arms), with its own measurement.
- The disc read in the air classifier (INTEGRATION_PLAN item 18, R
  `clearing.rs:4473` beside `AIR_THRESHOLD_MM`) is arm-only in R. Port it
  arm-only. The shipped centre read (M `clearing.rs:2679`) stays.

## 4. The product wiring

Every surface reaches the variant through `ProjectSession::apply(Command)`
and the existing op config. No new command and no parallel flow: the
variant is one more value of an existing parameter.

| Layer | Change (M lines) |
|---|---|
| Config enum | `ClearingStrategy` (`compute/operation_configs.rs:136`): add `GeodesicAdaptive` after `ContourSpiral`. Serde `snake_case` gives `geodesic_adaptive`. The `ParamEnum` derive (`rs_cam_derive/src/lib.rs:79`) writes the W5 schema string `enum:…|geodesic_adaptive`; nothing is typed by hand |
| Planner enum | `ClearingStrategy3d` (`adaptive3d/mod.rs:72`): add `GeodesicAdaptive` (no `cfg`, no payload: Q8 removed the pick). `From<ClearingStrategy>` (:102) maps it |
| Dispatch | `compute/execute/rough_3d.rs:153` needs no change (`.into()`). `adaptive3d/path.rs:469`: the new arm joins `AgentSearch \| ContourSpiral` in `clear_z_level_agent_2d_slice` (`clearing.rs:2922`), with the per-piece generator `adaptive::geodesic::slice` (as R `path.rs:491`, R `clearing.rs:3147`) |
| Every exhaustive match | the compiler lists them: `machine/strategy_advisor.rs:110` (`is_constant_engagement`: yes), :117 (label "Geodesic Adaptive"); `adaptive3d/clearing.rs` slice matches; `feeds/` readers that test `ContourParallel` (`feeds/suggest/adaptive_entry.rs:74`) |
| Advisor | `ADVISOR_CANDIDATE_STRATEGIES` (`session/compute.rs:1417`) stays `[CP, ContourSpiral]` in 7.4. Add the variant in a later patch with its own measurement |
| GUI | `rs_cam_viz/src/ui/properties/operations/surface_3d.rs:180–215`: one `selectable_value` row "Geodesic Adaptive", hover text from Q9 (body-only; pair with a CP rest op). Mark "Agent Search" legacy in its hover text (Q5) in the same patch. Visible parameters: the existing roughing ones only |
| GUI edit path | the combo writes `cfg.clearing_strategy`; the existing properties → `Command` path carries it. No new controller code |
| MCP | no wire enum exists: `rs_cam_mcp` has no strategy type, and `mcp_wire_surface.json` (`rs_cam_viz/tests/snapshots/`) pins tool names and argument shapes, not enum values. The value reaches MCP through `get_operation_schema` and `set_toolpath_param`. Expect no snapshot change; `mcp_wire_surface_pin` proves it |
| CLI | job files take `clearing_strategy = "geodesic_adaptive"` through serde; the schema table prints the new value |
| Project file | serde round trip: extend `clearing_strategy_serde_round_trip` (`compute/operation_configs.rs:3808`). An older build reading a new file refuses the unknown variant; record that in the release note |
| FEATURE_CATALOG.md | the 3D Adaptive row (:33): one sentence for the variant with its sentry; state "Experimental" until the operator test cut (§8 condition 3) |
| Docs | `adaptive/CLAUDE.md`, `adaptive3d/CLAUDE.md` (40-line limit), new `adaptive/geodesic/CLAUDE.md` |
| Template "geodesic body + CP edges" (Q5) | design in §6 Q-T; not in the first wiring patch |

### 4.6 Invariants the variant carries

- The arm clears the body only (C38 §16). Its left pieces go to a rest op
  through part H.
- Every feed the arm lowers carries `Move::feed_cap` (step 1). A census
  sentry checks that every dressup keeps the cap (lead note 2, step 1).
- No typed constant. Each rule in the module doc names its derivation.

## 5. The steps

One reviewable patch per step. Each step lists its parity proof. "Byte
parity" means `adaptive3d_emission_byte_parity` and
`every_op_keeps_its_toolpath_g_opgolden` pass unchanged, with no re-bless.

| Step | Patch | Gates | Parity proof |
|---|---|---|---|
| 7.0 | DONE by D52 (research branch): the pick kept plan A in all 9 regions; the runs are deterministic. Q8: no pick | — | — |
| 7.1 | Part A: `tool_load::chip_load_fraction` (the one L). Unit tests from R `slab_field.rs:215–275`, the full-pass identity `w/D` | `tool_load/` sentries, clippy | no caller changes output |
| 7.2 | Parts B + C + D, shared, default off: `StepSplit`, `adaptive3d/keep_down.rs` (`keep_down_excess`, `corridor_ceiling_z` wrapper, `keep_down_route`), `KeepDownRoute` | `adaptive3d/` sentries; planted cases for `keep_down_excess` (one link through stock, one over cut ground) and the router (R `keep_down_route.rs` tests) | every new switch defaults to today's rule; byte parity |
| 7.3a | The research reference on M: R's `research_geodesic.rs` (607e52730) less `SlabField` and the 3D slice layer; R's `adaptive/path.rs` turn support (`turn_bound`, `LinkLoad::with_turns`/`admits_turn`, `subdivide_each`, the `_turns` cleanups) less `SlabField`; R's `scallop_isofield.rs` (`iso_segments_where`); R's 2D harness; the pin test `the_geodesic_preset_keeps_its_plan_s7_3` (C35 plan A on the star and pocket, checked equal on R and M) | `adaptive/` sentries, research lib tests, the pin test | `turns` empty on every shipped path: byte parity |
| 7.3b | DONE: `adaptive/geodesic/` `mod.rs` (door `geodesic_toolpath`, a `Toolpath`, because `AdaptiveSegment` is `pub(crate)`) + `field.rs` + `wraps.rs` + `plan.rs` + `CLAUDE.md`, cut to C35 plan A by subtraction (dead-code lint, then the options folded). No `arcs.rs`: with the default ends only the head-on rule reached the arc lenses; the zigzag turn is 15 lines in `plan_piece` | the door equals the reference (feeds aside) on the star and the pocket | no product caller; byte parity |
| 7.3c | merged into 7.4: `slice.rs` (the 3D door, with `replay_after_group_filter`) lands with its caller | — | — |
| 7.3d | `research_geodesic.rs` cut to test support (readout, cascade, reports, `run_2d`) calling the product; its losing presets and their tests go (7.7 work brought forward, if the lead agrees) | pin test; research lib tests | byte parity |
| 7.4 | The variant: parts E and G, the `ClearingStrategy` value, the planner enum, the dispatch, the GUI row, the catalog line. Feeds by the match functions (part F not yet) | new golden `tests/fixtures/adaptive3d_emission/geodesic_adaptive.txt`; the C12 judge without `research`; `schema_enum_values_g_schemaenum`; `every_op_states_its_capabilities_w2_4`; `mcp_wire_surface_pin`; the serde round trip; viz `-j 2` filtered to `surface_3d` | byte parity on the four existing strategies: the variant adds output only |
| 7.5 | Part H producer for the variant (after step 1b lands the channel, §3.1): left pieces with their slab tops, D48 clip, D50 top | R `tests/a_rough_publishes_the_regions_it_leaves_d44.rs`, `a_derived_rest_rough_cuts_only_the_leftovers_d44.rs`, `a_rest_consumer_stays_inside_the_source_boundary_d48.rs`, `a_rest_consumer_starts_at_the_left_stock_top_d50.rs` (when R commits it), ported | byte parity on all strategies; a CP op without a `DerivedRestRegions` consumer publishes nothing |
| 7.6 | Part F: the factor on `Adaptive3dSegment::Cut`; the emitter writes `feed_cap`; delete `apply_slice_turn_feeds`, `apply_chain_feeds`, `TurnRecord` matching | the step-1 sentry; a new census: every capped cut keeps its cap through every dressup; the arm's golden re-bless (only this op) | byte parity on the four existing strategies (factor 1 → `None`, proved by step 1's `Debug` test) |
| 7.7 | Delete `GeodesicOptions`, the presets and the losing values (§1.2); move the C12 judge to the product arm | `research` clippy; the C12 judge | byte parity on all five strategies |
| 7.8 | Template "geodesic body + CP edges", if Q-T says yes | its sentry: the pair cuts the R D47 volume within the sim cell | byte parity |

Order rule (§8): step 1b goes before 7.5. Steps 0 and 2–6 for CP and the
slice arms come after 7.8. Step 2 then moves CP to `keep_down.rs` with its
own re-bless.

Measurements before 7.4 ships (pre-register in the 7.4 report):
- Corne and wanaka200, geodesic op 1 against CP: cycle (acceleration
  clock), load max and p99 by part A, length over the bound, plunge count;
- the M results must match R's D45/D47 numbers within the run-to-run
  spread. A larger gap is a port defect.

## 6. Questions for the operator

- **Q12. The default stepover.** The arm holds its bound at every stepover
  measured (D37, D47), and its time gain grows with the stepover. Candidate
  rule: the largest stepover whose p99 load (part A) equals a stated
  fraction of the tool's bound. The fraction is the operator's choice; the
  rule then derives the stepover per tool and material. Is that the rule,
  and what is the fraction?
- **Q8. The pick.** Settled by D52: no pick (§1.1).
- **Q-T. The "body + CP edges" template.** Is it one GUI action that adds
  two ops (the arm, then a CP op on `DerivedRestRegions`), or a preset op
  pair in the job file only? Does the CP edge op take the same tool?
  (D45 used a smaller tool on Corne; the job was ×1.14 of CP for that
  reason.)
- **Q-N. The name in the GUI.** Q9 says "Geodesic Adaptive". The 3D op is
  already called "3D Adaptive", and one strategy value is "Adaptive". Is
  "Geodesic" alone clear enough inside the 3D Adaptive strategy list?
- **Q-L. AgentSearch legacy (Q5).** Marked legacy in 7.4 (hover text), or
  only after the operator test cut?
- **Q-F. An older build and a new project file.** An older build refuses a
  project with `geodesic_adaptive`. Is that acceptable, or does the file
  need a version note?

## 7. Risks

- `plan_piece` (R :5128–5727) is 600 lines and reads options in many
  places. Cutting it to C35 by hand can change output. 7.3 pins C35's R
  output on the 2D controls before any cut, and each cut must keep the pin.
- The 3D hooks in R `clearing.rs` (93 `cfg` sites) interleave with the
  shipped arms. Each hook ported in 7.2/7.4 needs its shipped branch
  unchanged; byte parity is the proof, per step.
- M has moved since the merge base (257 crate files). R line numbers in
  this file are a map, not a patch. Each step rereads M before it ports.
