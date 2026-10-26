# Integration plan: the geodesic arm and its by-products (2026-10-27)

Author: a planning agent, read only. No code, no cargo, no commit.
Question (lead): how do the geodesic arm and its by-products fit the
shared toolpath architecture, which of them help the strategies that
already ship, and where does the research duplicate shipped code?

Path keys in this file:
- `AH/` = `.claude/worktrees/adaptive-head/crates/rs_cam_core/src/`, the
  working tree on 2026-10-27 (branch head 3181ae54f plus the uncommitted
  D27–D33 work). Line numbers move with that tree.
- `M/` = `crates/rs_cam_core/src/` on master.
- Planning files are relative to this folder.

## 0. The answer in short

1. Most by-products do not depend on the geodesic field. They depend only
   on the planner stock, the boundary and the emitted path (C38 §13.8).
   They belong in the shared 3D slice layer (`adaptive3d/`), the toolpath
   IR (`toolpath.rs`) and the feed stages (`dressup/`). The field, the
   spacing law and the core spiral belong in the 2D engine (`adaptive/`).
2. The three items that help contour_parallel (CP) NOW are small and do not
   need the field:
   - one keep-down proof for CP's side links (G-LINKPLOUGH). On Corne the
     two flagged CP links are a tolerance mismatch between the planner and
     the triage, not a missed column (§3, item D1);
   - G-CLIPAIR (a) for CP, a candidate cause of G-CPRAPID (unmeasured);
   - the per-move feed cap (`Move::feed_cap`): shipped-neutral today, and
     the contract every later load-set feed needs.
3. The research re-implements five shipped things. The worst is link
   feasibility: eight readers of "may the tool stay down here", with
   three stock reads and two typed tolerances (§3).
4. The research measured large gains for its own arm and almost none on a
   shipped strategy. Every benefit to CP, AgentSearch or ContourSpiral in
   §2 is a prediction until a step measures it. So the adoption plan
   (§4) starts with instruments that measure CP, not with code that
   changes CP.
5. The load definition now exists in three forms (box, per strip,
   integral). Fix ONE form in ONE function before any planner or judge
   adopts it (§3, item D5; question Q1).
6. Promotion of the arm is the last step. It needs one preset, a config
   variant, and the kernel moved out of the 10 935-line research module
   (`AH/adaptive/research_geodesic.rs`).

## 1. The map

Columns: the research component; where it is now; the shipped module it
belongs in; what it duplicates or extends; the interface another strategy
calls.

| # | Component | Now (research) | Home | Duplicates / extends | Shared interface |
|---|---|---|---|---|---|
| 1 | Geodesic field (source-tracked Eikonal solve, seed disc, islands as walls) | `AH/adaptive/research_geodesic.rs:343-1750` (`GeodesicField`, `FieldSolver` :401) | new `adaptive/geodesic/field.rs` | extends `finish::scallop_isofield::fast_sweep` (`M/finish/scallop_isofield.rs:325`); `FieldSolver::FirstOrder` IS that solver | `GeodesicField::solve(region, seed, cell) -> Field` |
| 2 | Core spiral (`T/s − φ/2π` while the level sets are star-shaped) | `core_spiral` `AH/adaptive/research_geodesic.rs:3539`, `star_levels` :1523, `CoreJoin` :2196 | `adaptive/geodesic/wraps.rs` | ContourSpiral's tuned trochoid insert (`spiral.rs`; cap 1.6, G0 ruling 3 in `STATUS.md`) does the same job with an underived constant | `core_spiral(field, seed, r0, s, tol) -> Vec<P2>`, any scalar field with a star test |
| 3 | Curvature spacing law `s'(ρ) = ρ + R − √(R² + ρ² + 2ρ(R − cD))`, `Spacing::LocalLevel` | `Spacing` :2053; C5, C16 in `P2_C2_geodesic_fixes.md` | `adaptive/` next to `pass_engagement_limit` (`M/adaptive/mod.rs:180`) | new; the closed form of the ceiling `c` the engine already uses | `level_step(s, R, c, rho) -> f64` |
| 4 | C31 grown outline (smooth, shortfall inset, grow by R − m, closing guard) | `research_grown_outlines` `AH/adaptive3d/clearing.rs:3786` | `adaptive3d/clearing.rs`, in place of the double inset of `planner_outlines` (`AH/…/clearing.rs:3182`) | replaces the shipped double inset and the perimeter sweep (G-PERIMTAG, G-OUTERBAND) | `planner_outlines(region, …) -> Vec<Polygon2>` with one inset |
| 5 | Stock-box clip and centre-clip reach of a grown outline | `clip_to_stock_box` `AH/adaptive3d/research_outer_band.rs:42`, `clip_to_centre_reach` :136 | `geometry/` (box clip); `adaptive3d/centre_clip.rs` (reach) | new | `clip_to_box(polys, bbox)`, `CentreClip::reach(margin)` (C38 §13.8) |
| 6 | G-CLIPAIR (a): the boundary clear keeps `boundary ⊕ R` | `boundary_reach` `AH/…/research_outer_band.rs:74`, call `AH/adaptive3d/path.rs:860-915` | `adaptive3d/path.rs` boundary clear, all strategies | fixes the shipped O5b clear (`M/adaptive3d/path.rs:829`) | `boundary_reach(boundary, R)` |
| 7 | G-CLIPAIR (b): regions only inside the boundary | `ResearchClip::restrict_regions` (first loop only after C38 §16.1) | `adaptive3d/clearing.rs` grid builder, or `geometry/region_mask.rs` | needed only once (a) lands; the shipped clear gives it for free today | `RegionReach::restrict(grid)` |
| 8 | Keep-down proof of a held link (swept disc of R, conservative top, slab top + clearance) | `keep_down_excess` `AH/…/clearing.rs:1312`; counter `check_held_link` :2914 | one `adaptive3d/keep_down.rs` | duplicates `link_footprint_ok` (:1251), `corridor_ceiling_z` (:1279), `is_clear_path_3d`, `stay_down_link_z` (`AH/adaptive3d/path.rs:1384`) — §3 D1 | `keep_down_excess(stock, from, to, R, ceiling) -> Option<(P2, f64)>` |
| 9 | Routed keep-down travel (A* over free cells; "replaces the travel, never the entry's cut") | `AH/adaptive3d/keep_down_route.rs:82`; `routed_entry` `AH/…/clearing.rs:1557`, hooked at :1504 and in `commit_pending` :2950 | `adaptive3d/keep_down.rs` with item 8 | new; the 2D `feed_link_to_legal` (`M/adaptive/path.rs:334`) is the 2D cousin | `keep_down_route(stock, &RouteRequest) -> Option<Vec<P2>>` |
| 10 | Feed cap | `Move::feed_cap` (`AH/toolpath.rs`, `pub feed_cap: Option<f64>`); binds last in `AH/dressup/feed_modulation.rs` (~:1016) and `feedopt.rs` | `toolpath.rs` + `dressup/` (already there, shipped-neutral) | extends the Stage 4 `predicted_cap` channel (`M/dressup/feed_modulation.rs:472`, ContourSpiral only, `M/session/compute.rs:1984`) | the field itself; sentry `a_feed_cap_binds_every_feed_stage_c38` |
| 11 | Turn, slot and departure records; exact and chain feed match | `slice_turn_records` `AH/adaptive/research_geodesic.rs:8527`, `TurnKind` :8630, `apply_slice_turn_feeds` :8770, `turn_feed_factor` :7749; `unfed_chains`/`apply_chain_feeds` `AH/…/research_outer_band.rs:227,287` | the segment IR: a factor carried by the cut itself (§3 D6). The match code then goes | re-derives by XY position (1e-6 mm, then the RDP tolerance) a fact the planner knew | `Adaptive3dSegment::Cut` with a per-span feed factor; RDP joins only equal factors |
| 12 | Replay after the short-group filter (C22a) | `filter_short_groups` `AH/…/clearing.rs:3574` + the research replay loop | one shared filter for every slice arm | the 2D cleanup already follows this rule (`replay_kept_segments`, `M/adaptive/path.rs:597`); the 3D F-038 filter does not | `filter_short_groups(segs, min_cut, params) -> (segs, dropped)` followed by the replay |
| 13 | SlabField (stock top per cell for the 2D planner) | `AH/adaptive/slab_field.rs` (untracked), `attach_slab` `AH/adaptive/material_grid.rs:142`, reads in `AH/adaptive/search.rs` (`swept_width_on` :762) | `adaptive/` (2D engine), opt-in per slice consumer | new (G-FACESLAB) | `AdaptiveParams.slab_field: Option<Arc<SlabField>>` |
| 14 | Area load `L` through `mrr_cross_section_mm2` | `SlabField::strips_load` `AH/adaptive/slab_field.rs:161`; judge in `phase3/fix24/` | ONE function in `tool_load/` beside the power gate (`M/tool_load/power.rs:287`), which reads `M/tool/mod.rs:508` | three forms exist now (§3 D5) | `chip_load_fraction(cutter, strips, dpp, D) -> f64` |
| 15 | C40 entry-cause instrument | `record_entry_cause` `AH/…/clearing.rs:3538`, `entry_records` `AH/adaptive3d/mod.rs:1031` | `trace/` (generation records) and the span annotations | new; complements `dressup/entry_audit.rs` | an `EntryCause` on each entry span, for every strategy |
| 16 | Audit scripts (descents into stock, lift-overs, turns, plunge-feed share, MRR, bands) | `phase3/fix11/audit.py`, `entries.py`, `fix24/area_oracle.py`, `load_split.py` | `stock/sim_triage.rs` as findings | `link_cuts_stock` (`M/stock/sim_triage.rs:1640`) covers fed links only | new triage observations, published measured or abstained |
| 17 | Part A stock-read split | `StepSplit` `AH/adaptive3d/mod.rs:374`, `step_axial_depth` / `step_swept_width` `AH/…/clearing.rs:1341,1370` | none now | — | recommended OFF (`P3_C22_3d_turns.md` §16.14); do not adopt |
| 18 | Disc read in the air classifier (G-AIRCENTRE) | `disc_read` `AH/…/clearing.rs:4394-4404` | `adaptive3d/clearing.rs`, all slice arms | the shipped centre read with `AIR_THRESHOLD_MM` (`M/adaptive3d/clearing.rs:2679`) | none new |
| 19 | Whole chaining of contour segments (G-ISOCHAIN) | `Chaining::Whole`, split `iso_segments_where` in `M/finish/scallop_isofield.rs` | `finish/scallop_isofield.rs` | fixes the shipped `chain_segments` tail-only rule | `chain_segments` |

Parts the research itself drops (`P3_C38_outer_band.md` §16.1): the
corridor opening and slots, the G-b judge and `RecordMark`, and the second
loop of `restrict_regions`. They are not in this plan.

## 2. Which strategies benefit, by how much, and the risk

"Measured" means a planning file gives the number. "Predicted" means only a
design or a reading of place supports it.

| # | Component | Beneficiary | Gain | Status | Risk |
|---|---|---|---|---|---|
| 1 | Field | geodesic arm; iso-scallop finish (solver accuracy) | arm: the base R4 calls sound (`R4_independent_review.md` §1.1). Finish: none measured | arm measured; finish predicted | the source-tracked solver over-reads ≤ 1.14 cells behind a wall (module doc :13-28) |
| 2 | Core spiral | geodesic arm; ContourSpiral and `spiral.rs` (replace the tuned insert in the star-shaped core) | arm: wanaka descents 3578 → 2016, Corne 1195 → 1076 (`fixture3d_v5.html`, D20 → C35) | arm measured; ContourSpiral predicted | the replay split the spiral until the chord-heading fix (`P3_C22_3d_turns.md` §12.6-12.7); three Corne hole floors left uncut (§12.9) |
| 3 | Spacing law | geodesic arm; ContourSpiral inner wraps (a pass round a small cleared disc reads `2 + 4/ρ` mm against 2.18, module doc :88-91). Not CP: CP rings close in on a shrinking island, the reverse geometry | 2D star: mid-wrap splits gone (D5, `STATUS.md`) | arm measured | the law assumes a front curving about the seed; behind an island it under-spaces (C5, C15) |
| 4 | Grown outline | AgentSearch and ContourSpiral: removes the double inset and the perimeter sweep (G-OUTERBAND, G-PERIMTAG) | arm: thick over-load 4.74 % → 0.85 % on Corne; inside 3–6 mm band 1017 → 448 mm² (R4 §1.1, §1.5) | arm measured; shipped arms predicted (the sweep is 1026 mm of the arm's over-bound length before C31, `STATUS.md` C30) | G-RINGGROW (a thin frame ring fills, closing guard refuses); G-ARCENDHOOK on the far-side modes |
| 5 | Box and centre-reach clips | every arm that grows an outline | wanaka: 72 "unfed" turns were clipped turns (`P3_C38_outer_band.md` §13.7) | predicted for D27+; the arm only | none known; geometry only |
| 6 | G-CLIPAIR (a) | ALL Adaptive3d strategies, CP included: every planner read near the boundary (entry floor, link proof, air test, level gate) | Corne copy op: 126/127 deep samples at its containment line; that move caused a 200 µm deflection and an export refusal (§11.1, §11.1b) | arm measured; CP and AgentSearch predicted. G-CPRAPID's link to it is a hypothesis (C38 §13.8) | changes every arm's output near a boundary; the band stock then stands until a later op cuts it |
| 7 | G-CLIPAIR (b) | same | required by (a) | — | none beyond (a) |
| 8 | Keep-down proof | CP side links (G-LINKPLOUGH); every held link; the C22a counter | Corne fed-link bars traced to centre-line reads (C38 §14.2) | arm measured; CP predicted (§3 D1) | a stricter proof turns some CP links into entries: more entries, more time |
| 9 | Routed travel | every slice arm, CP included (the hook is `plan_entry`'s door) | after the §16.7 fix the router took 0 routes on Corne (`P3_C22_3d_turns.md` §16.9 item 2). C42's estimate for CP: about 28 of 95 Corne entries and 449 of 1609 wanaka entries could route at the cut feed (§16.4) | predicted; the safe subset is small | `LowRapid` (a rapid at depth) needs its own safety ruling (§16.3) |
| 10 | Feed cap | every op whose planner lowers a feed by a load rule; today none ships | the modulator raised 1554 strip samples over the bound to F 4000 on Corne (`P3_C38_outer_band.md` §15.1-15.2) | arm measured | none; with no cap the output is byte-identical |
| 11 | Turn and departure feeds | geodesic arm; shipped 2D Adaptive and the slice arms, which exempt the step out of a straight plunge (`M/adaptive/CLAUDE.md`, "Exempt") | departures: P −36.8 mm of "other" over-load at most on Corne (§15.7) | predicted | a shipped turn exemption is a new shipped load rule (question Q3) |
| 12 | Replay after filter | AgentSearch and ContourSpiral (they run the same F-038 filter with no replay) | arm: the copy op's 301 fed-link samples on 193 moves (`P3_C22_3d_turns.md` §14) | arm measured; the shipped arms do not hold links, so for them only the cut-overload half applies, unmeasured | more entries where links fall to rapids |
| 13 | SlabField | geodesic arm, then AgentSearch slice; CP needs its own design (§17.3) | slope test: 172 → 0 over with 354 entries (width read), 2 over with 97 entries (load read) (§17.7, §18.7) | measured on test fixtures only; both tests RED | the cost in entries (4× the base on the slope); AgentSearch's first slot is not reached by the seed rule |
| 14 | Area load L | every judge and report; CP's comparison numbers move | CP area over 0.471: Corne 3158.9 → 2226.0 mm, wanaka 6201.7 → 2133.1; C35 Corne 86.9, wanaka 26.0 (`phase3/fix24/rescore.json`) | measured (re-score of existing sims) | three forms exist (§3 D5) |
| 15 | C40 causes | every slice arm and CP | showed LargeZ is 73.6 % (Corne) and 90.0 % (wanaka) of the arm's entries (`fixture3d_v5.html`, `P3_C22_3d_turns.md` §16) | measured on the arm | research-only `RefCell` on the context today |
| 16 | Audits | CP first (the operator's daily strategy) | the audit, not a span label, found the 1195 Corne plunges (R4 §3.2 item 1) | measured on the arm | thresholds in `entries.py` have no derivation (R4 §3.2) |
| 18 | Disc air read | AgentSearch, ContourSpiral: the starter lap inside its own plunge hole reads 0.75 D (derived, G-AIRCENTRE) | arm: starter class cleared (C18) | shipped predicted | more cut moves where runs were rapids |
| 19 | Whole chaining | iso-scallop finish (`iso_contour`) | none measured; heavy scallop tests stayed green with the split (`STATUS.md` D12 addendum) | predicted | finish output changes; heavy tests need the operator's go-ahead |

Not measured anywhere: the cause of G-CPRAPID (86 rapid collisions on
wanaka CP, `DEFECTS.md`), and where CP's 2226 mm / 2133 mm of area
over-load sits. Both come before any CP change (§4, step 0).

## 3. Duplication: which version wins

### D1. Link feasibility (eight readers)

| Reader | Where | Stock read | Tolerance |
|---|---|---|---|
| `is_clear_path` (2D) | `M/adaptive/path.rs:59` | 2D grid | the 20 % material-crossing rule (underived, R4 §4) |
| `feed_link_to_legal` (2D) | `M/adaptive/path.rs:334` | 2D grid, pass load | the ceiling |
| `is_clear_path_3d` | `M/adaptive3d/search.rs:112` | centre line | one DPP bite + 0.5 |
| `link_footprint_ok` (CP side links only) | `M/adaptive3d/clearing.rs:844` | `max_top_z_in_disc`, cell centres within `R − cell` | `DPP + LINK_DESCENT_NOISE_MM` (0.5, :866) |
| `plan_entry` stay-down proof | `M/…/clearing.rs:765`, `corridor_ceiling_z` :872 | conservative top, full disc | the stay-down clearance |
| `stay_down_link_z` (F-038b) | `AH/adaptive3d/path.rs:1384` | mesh at 10 samples + the stock ceiling | clearance, leave |
| `keep_down_excess` (research) | `AH/…/clearing.rs:1312` | conservative top, full disc | slab top + clearance |
| `bridge_corridor_is_swept` (dressup) and `relink_fragments` (finish) | `M/dressup/link.rs:96`, `M/finish/surface_link.rs:841` | move history; link ceiling | own |

**A fact the defect rows did not state.** The triage bar for a fed link is
the commanded step plus one sim cell (`link_cut_noise_floor_mm`,
`M/stock/sim_triage.rs:1528`): 6.30 mm on Corne op 1, 3.30 on the copy op.
`link_footprint_ok` admits a level link that bites `DPP + 0.5`: 6.50 and
3.50. The two Corne G-LINKPLOUGH lines remove 6.50 mm (limit 6.30) and
3.38 mm (limit 3.30) (`DEFECTS.md`). Both lie inside the planner's own
allowance. So on Corne the defect is two definitions of one tolerance. The
wanaka link (11.26 mm at Depth/Pass 5.46) is outside both, so it has a
different cause: the `R − cell` probe, the cell-centre read, or G-CLIPAIR.

**Winner.** In 3D, one swept-disc read on the conservative top:
`keep_down_excess` (it returns where and by how much). `corridor_ceiling_z`
is its max form and stays as a thin wrapper. `link_footprint_ok` goes. The
allowance for a level side link is one rule, shared with the triage bar
(question Q2 picks the value). The 2D readers and the finish linker stay
separate: they read other stock models. Put the 3D readers and the router
(item 9) in one new file, `adaptive3d/keep_down.rs`.

### D2. Entry floors and the stamp height

- The entry floor reads the conservative disc (`plan_entry`); the contact
  height reads the cell centres (`AH/…/clearing.rs:1196-1200`).
- `lift_to_level` lifts points to the surface heightmap plus the leave,
  but the emitter and the stamp drape with the drop cutter
  (`drape_point`, `AH/adaptive3d/path.rs:1572`). At Corne (91.3, −1.13) the
  planner read 17.45 where the sim stood at 18.0–18.5, and a rapid
  collided (`P3_C22_3d_turns.md` §16.12; the D32 probe of §16.13 is built,
  not run).
- **Winner:** one height function for the lift and the stamp (the drape,
  because it is what ships). This is a candidate cause of G-CPRAPID too,
  as CP drapes the same way. Run the D32 probe first.

### D3. Stock reads

`sample_stock_top_at` (centre, `AH/…/clearing.rs:2469`),
`max_top_z_in_disc` (cell centres), `max_conservative_top_z_in_disc`, and
the air classifier's centre read (`AIR_THRESHOLD_MM`). **Winner:** the
conservative disc for every proof (link, entry, route, air run). A centre
read stays only where its doc says why. The research already moved the
held-link counter from the centre line to the disc (C38 §14.3).

### D4. Swept width

The 2D `swept_width_on` (`AH/adaptive/search.rs:762`), the 3D
`step_swept_width` (`AH/…/clearing.rs:1370`, a second copy of the same rule
on the 3D stock), the sim's `radial_woc_fraction`, and the oracles in the
tests and scripts. **Winner:** the 2D planner form. `step_swept_width` and
`step_axial_depth` serve only Part A, which is off; delete them with Part A
(question Q6).

### D5. The load L (three forms)

- Box: `L = (ap/DPP)·(ae/D)` (`P3_C22_3d_turns.md` §18.6; the sim screen).
- Per strip: `Σ mrr_cross_section_mm2(d_i, strip) / mrr_cross_section_mm2(DPP, D)`
  (`AH/adaptive/slab_field.rs:161`, and the re-score of commit 2e7befc0f).
- Integral: `A = ∫ h(u) du` (`phase3/fix24/README.md` §1, still in the
  committed README text).

**Winner:** the per-strip form. It calls the power gate's own
`mrr_cross_section_mm2` (`M/tool/mod.rs:508`), it equals the radial fraction
at a full pass, and the planner already uses it. Move it out of
`SlabField` into one `tool_load` function. The planner, the sim screen
and the judge call it. Update the fix24 README so that its §1 states
that form. Operator confirmation: Q1.

### D6. Feed application

Four channels lower or set a feed after the planner:
- the exact match by XY (`apply_slice_turn_feeds`);
- the chain match by distance (`apply_chain_feeds`);
- the Stage 4 planner engagement for ContourSpiral (`predicted_cap`);
- the modulator's chip-load rule, which raised the planner's reduced feeds
  back to F 4000 (C38 §15.2).

The match code exists only because `Adaptive3dSegment::Cut(Vec<P3>)`
carries no feed. The planner knows each turn and departure when it
emits it, and the D15 fix (`split_at_turns`) already keeps each turn its
own cut through the RDP.

**Winner:** the cut segment carries its factor. The RDP joins only equal
factors, as arc fit and segment merge already do for `feed_cap`. The
emitter writes `feed_cap`. Then both match functions go, and the "unfed"
class of error goes with them. `turn_feed_factor` (`c / max(w, area)`) stays
as the one planner-side rule. The modulator still sets the feed, and the
cap only lowers it.

### D7. The time crossover

`air_run_crossover_mm` (`M/adaptive3d/clearing.rs:1865`) uses
`ASSUMED_RAPID_MM_MIN = 5000` (:1854, "a named guess").
`machine::kinematics::retract_saves_time` (`M/machine/kinematics.rs`,
about :929) uses the machine model, and the waterline and air-cut filters
already call it. **Winner:** `retract_saves_time`. The route of item 9
bounds its length by `stay_down_mm` (the CUT-05 dial). Keep that bound,
and decide route-or-entry by `retract_saves_time`.

### D8. Smaller items

- `PERIMETER_INSET_MARGIN_MM` is defined twice on master
  (`M/adaptive3d/clearing.rs:2467` as `_FORECAST`, and :2561). Make it one
  constant.
- The short-group filter (D12 in §1): one function for every slice arm,
  followed by the replay. The 2D engine already applies this rule.
- The region outline: the C31 grow (one inset) wins over the double inset
  of `planner_outlines` (R4 §7.1).
- Contour chaining: `Chaining::Whole` wins over the tail-only
  `chain_segments` (G-ISOCHAIN).

## 4. Adoption plan

Rules for every step:
- One step changes one strategy's output, or none.
- Before a step: pre-register the numbers on Corne and wanaka200 for the
  strategy it changes.
- After a step: byte parity on every other strategy. A re-bless names only
  the changed op (G-OPGOLDEN rule, `M/compute/CLAUDE.md`).
- An agent implements and the lead verifies. Agents never commit.

Sentries named below are the folder sentries in `M/adaptive3d/CLAUDE.md`,
`M/adaptive/CLAUDE.md`, `M/dressup/CLAUDE.md` and `M/stock/CLAUDE.md`, plus
`adaptive3d_emission_byte_parity` and `every_op_keeps_its_toolpath_g_opgolden`
in each step.

### Step 0. Measure CP before changing CP (no output change)

- **What:**
  1. A strategy-agnostic C40: an entry cause on each 3D entry for CP too.
     Move `record_entry_cause` out of `cfg(research)` and write the cause
     into the entry span (`trace/`).
  2. A triage observation for descents into stock (`entries.py`'s
     predicate). Derive its thresholds from the sim cell and Depth/Pass
     (R4 §3.2 item 1).
  3. A per-sample L (the D5 function) in the sim summary. Report it only.
  4. Run the D32 probe (§16.13) and a cause split of the 86 wanaka CP
     rapid collisions: G-CLIPAIR band, stamp/drape height, or other.
  5. Localise CP's area over-load: first ring, raster cleanup, links,
     draped wedges.
- **Files:** `adaptive3d/clearing.rs`, `adaptive3d/mod.rs`, `trace/`,
  `stock/sim_triage.rs`, `tool_load/`.
- **Checks:** byte parity on every op (no move changes); new triage
  sentries with a planted case each.
- **Operator decision:** Q1 (the one form of L) and whether the shipped
  report shows L beside the radial width.

### Step 1. The feed cap contract (shipped-neutral)

- **What:** land `Move::feed_cap` and its binding in the modulator,
  `feedopt`, arc fit and segment merge on master. No planner writes it.
- **Files:** `toolpath.rs`, `dressup/feed_modulation.rs`, `dressup/feedopt.rs`,
  `dressup/arcfit.rs`, `dressup/condition.rs`, the move copies listed in
  the worktree diff (`compute/transform.rs`, `compute/simulate.rs`,
  `finish/surface_link.rs`, viz render and upload).
- **Checks:** byte parity on every op; `a_feed_cap_binds_every_feed_stage_c38`;
  `constrained_max_modulation_f039`; the `dressup/` sentries.
- **Operator decision:** none (no output change).

### Step 2. G-LINKPLOUGH for CP: one keep-down proof

- **What:**
  1. Create `adaptive3d/keep_down.rs` with `keep_down_excess` and
     `corridor_ceiling_z` (no cfg).
  2. CP's `side_link_ok` uses it in place of `link_footprint_ok`.
  3. The level-link allowance becomes the triage's one rule.
- **Files:** `adaptive3d/clearing.rs` (the CP calls at `AH/…/clearing.rs:1919`
  and :2070 are the only callers), `adaptive3d/keep_down.rs`,
  `stock/sim_triage.rs` (if the allowance moves there).
- **Pre-register:** Corne CP fed-link lines 2 → 0; wanaka CP 0 stays 0;
  entries up by at most the number of refused links.
- **Checks:** `a_link_through_stock_is_flagged_g_linkplough`,
  `adaptive3d_keep_down_link_f038b`, `adaptive3d_entry_stock_aware`; byte
  parity on AgentSearch and ContourSpiral (they do not call
  `side_link_ok`); CP golden re-bless.
- **Operator decision:** Q2 (the allowance value).

### Step 3. G-CLIPAIR (a)+(b) for CP

- **What:** the boundary clear keeps `boundary ⊕ R`, and the region grid
  keeps centres inside the boundary, for CP first. The strategy test stays
  at the call site (`AH/adaptive3d/path.rs:875`).
- **Files:** `adaptive3d/path.rs`, `adaptive3d/clearing.rs`
  (`build_material_bool_grid`), `adaptive3d/region_map.rs` (By Area reads
  the same cell test, C38 §11.2).
- **Pre-register:** from step 0's cause split. If the G-CLIPAIR band holds
  most of the 86 wanaka rapid collisions, P: 86 → near 0.
- **Checks:** `adaptive3d_boundary_clear_parity` (re-blessed for CP only),
  `adaptive3d_boundary_clip_leaves_no_phantom_stock_g_boundaryphantom`,
  the two By Area sentries; byte parity on the other arms.
- **Operator decision:** CP first, then AgentSearch and ContourSpiral in
  one later step (Q4).

### Step 4. One height for the lift and the stamp (if step 0 names it)

- **What:** `lift_to_level` and the stamp read one drape height (D2).
- **Files:** `adaptive3d/clearing.rs`, `adaptive3d/path.rs`.
- **Checks:** `adaptive3d_plan_matches_merged_path_g_plansimgap`,
  `adaptive3d_planner_never_ahead_of_emitted_path`; every arm changes, so
  pre-register per arm and re-bless per arm.
- **Operator decision:** none beyond the measured cause.

### Step 5. The slice arms (AgentSearch, ContourSpiral)

One step per item, each parity-checked against CP:
- 5a. G-CLIPAIR for the slice arms.
- 5b. The grown outline, the box clip and the centre-reach clip in
  `planner_outlines`; remove the perimeter sweep (G-OUTERBAND, G-PERIMTAG).
  The C31 closing guard keeps the shipped path where it refuses.
- 5c. The disc air read (G-AIRCENTRE).
- 5d. The shared short-group filter with the replay (C22a).
- 5e. Departure feeds through the segment factor and `feed_cap` (D6).
- **Checks:** `agent_search_coverage`, `adaptive3d_plans_every_slice_piece_g_adapt3dpieces`,
  `contour_spiral_gcode_validity_phase0`, `adaptive3d_entry_coalescing_f038`,
  byte parity on CP.
- **Operator decision:** Q5. If the operator retires AgentSearch, skip 5
  for it. ContourSpiral then decides alone.

### Step 6. The routed keep-down travel (optional)

- **What:** `keep_down_route` behind `Adaptive3dLinking::keep_down_route`,
  cut feed only, decided by `retract_saves_time` (D7).
- **Pre-register:** CP entries routed (C42: at most 28 of 95 on Corne,
  449 of 1609 on wanaka), the entries kept, and the cycle.
- **Operator decision:** Q7 (no `LowRapid` without a safety ruling).

### Step 7. Promote the geodesic arm

1. **Freeze the options.** Keep one preset. The candidate is
   `C35_SPIRAL_PICK` (`AH/adaptive/research_geodesic.rs:2395`):
   - source tracking;
   - the split meeting rule;
   - `LocalLevel` spacing;
   - whole chaining;
   - zigzag arcs;
   - the replay pick;
   - the core spiral.

   Delete the losing enum values and presets (repo rule: delete, not
   archive; R4 §5 item 10). Decide the pick (Q8).
2. **Move the kernel.** Create `adaptive/geodesic/` with `field.rs` (items
   1, 3), `wraps.rs` (item 2, `ordered_wraps`, the meeting rule), `arcs.rs`
   (zigzag turns, the turn records) and `plan.rs` (`plan_piece` for the
   one preset), plus a `CLAUDE.md` of 40 lines or fewer. The readout
   (`pass_load_readout`, the cascade walk, the 2D harness door) stays in
   the research file as test support.
3. **Make the 3D arm a product variant.** Add a `ClearingStrategy` value
   in `compute/operation_configs.rs` (W5 schema), the dispatch in
   `compute/execute/rough_3d.rs`, the GUI row
   (`rs_cam_viz/src/ui/properties/operations/surface_3d.rs`) and the MCP
   wire type. The env selector `RESEARCH_ARM_ENV`
   (`AH/adaptive3d/mod.rs:123`) goes. The 3D held links (`HeldLinks`,
   `AH/adaptive/research_geodesic.rs:9059`) use the step 2 proof.
4. **Checks:** byte parity on every existing strategy (the new variant
   adds output only); the C12 test without `research`; the 2D harness
   controls; fixture runs against step 0's instruments.
- **Operator decision:** Q5, Q8, Q9, and the G2 ruling itself.

### Step 8. Later and separate

- SlabField for the slice arms, after the judge reads L and both §17
  tests are green (`P3_C22_3d_turns.md` §17.7, §18.7).
- A SlabField design for CP (§17.3 says it is a separate design).
- The core spiral and the spacing law in ContourSpiral, with the removal
  of `TROCHOID_CAP_MULT_3D` (G0 ruling 3).
- `Chaining::Whole` in the iso-scallop finish (heavy tests, operator's
  go-ahead).

## 5. Open questions for the operator

- **Q1.** Is the per-strip form of L the one load definition for the
  planner, the sim report and the judge? Does the shipped report show L?
  (CP's own numbers move: Corne 3158.9 → 2226.0 mm over the bound.)
- **Q2.** What is the one allowance for a level side link: the planner's
  `DPP + 0.5`, or the triage's `DPP + one sim cell`? The 0.5 has no
  derivation (R4 §4).
- **Q3.** May a shipped planner exempt a short turn or a plunge departure
  from the pass-load ceiling if it lowers that move's feed by
  `c / max(w, area)` and caps it? The 2026-10-24 ruling covered research
  only.
- **Q4.** Does G-CLIPAIR go to CP first, as this plan orders? The band
  stock then stands in the planner until a later op cuts it.
- **Q5.** Is AgentSearch retired when the arm ships, or kept? Is the arm
  body-only with a CP rest op for the edges (`P3_C38_outer_band.md` §16)?
  If yes, does the GUI offer that pair as one template?
- **Q6.** Part A (the stock-read split) is off and costs its gain back on
  wanaka (§16.14). Delete `StepSplit` and its two readers?
- **Q7.** Is a rapid at depth over cut ground (`LowRapid`) allowed? Until
  it is, the router runs at the cut feed only.
- **Q8.** Does the arm ship with the C28/C29 replay pick? R4 (§6) advises
  no further work on it. The pick doubles the plan work per piece.
- **Q9.** What do the GUI and MCP call the strategy, and which
  parameters are visible? The research presets have none visible.

## 6. The Python measurements: duplicates, new instruments, homes

(Operator addition, 2026-10-27.) `phase0/`, `phase2/` and `phase3/` hold
258 `.py` files. They read the CLI's artefacts (`simulation.json` from
`--sim-artifact`, `program.nc`, `tp_*.json`, `summary.json`, stdout log
lines) and re-derive quantities from them. Below, each family is classed
as a DUPLICATE (the Rust already computes it), NEW (worth an instrument
in the app and the CLI), or RESEARCH-ONLY (it joins research log lines;
it retires with the research).

### 6.1 What the Rust already gives (read before a script is written)

| Quantity | Rust source |
|---|---|
| Per-sample radial width, axial engagement, Z descent, removed volume, segment time, feed, intent | `SimulationCutSample` (`M/stock/simulation_cut.rs:63,151,164,169,183`) |
| Runtime by intent | `runtime_by_intent: CycleTimeBreakdown` (`M/stock/simulation_cut.rs:505`), from `machine/kinematics.rs` (trapezoid, junction deviation) |
| Standing stock over the plan's floor, islands, height, width, wall tag | `StandingField`, `standing_islands` (`M/stock/standing_islands.rs:57,218`); target from `adaptive3d::intended_floor_grid` (G-ROUGHCOV A3/A4) |
| Fed links through stock | `link_cut_observation` (`M/stock/sim_triage.rs:1640`) |
| Rapid collisions, gouge, deeper-than lines | the rapid check sentries in `M/stock/CLAUDE.md`; `compute/model_gouge.rs` |
| Power, chip load, deflection, histograms | `tool_load/` (`power.rs:287`, `distribution.rs`) |
| Stock heightmap images | CLI sweep (`rs_cam_cli/src/sweep.rs:345`); MCP `inspect_stock`, `screenshot_simulation` |

### 6.2 The script families

| Family (example scripts) | What it computes | Class | Recommended home |
|---|---|---|---|
| Stock rebuild and target: `fix9/stockmap.py`, `stockmap_d22…d30.py`, `fix9/render.py` stock maps, `fix18/lowtravel.py` step 2 | re-stamps a flat end mill along the trace samples on a coarse grid; target = closing of the part heightmap by a disc of R, plus the leave | DUPLICATE. The dexel stock and `intended_floor_grid` (an exact erosion) already exist. The Python rebuild is less exact: flat end mill only, sample spacing 0.3 / 1.0 mm. | Export the `StandingField` of each rough checkpoint from the CLI (one flag, JSON or NPY grid) and from MCP. Scripts then read it and stop stamping. |
| Distance bands of standing stock: `fix15/bands_d25.py` (R4 §1.5 table) | standing stock over 0.8 mm, split by distance from the hole-filled silhouette | NEW. It found G-OUTERBAND's size; no Rust reading splits by distance. The 0.8 mm has no derivation (R4 §3.2 item 6). | `stock/standing_islands.rs`: a band split of `StandingField` by the distance to the silhouette (`boundary::silhouette`), with the bar of the coverage finding (leave + one cell) in place of 0.8. Report it in the coverage finding ("inside the part" bar, "outer band" information, R4 §3.2 item 6). |
| Thin spurs: `phase0/tools/spurs.py` | tall thin material on a re-stamped heightmap | DUPLICATE since G-THINSTAND (A, A3, A4): islands with inscribed width and height | none; retire |
| Label-free kinematic audit: `fix11/audit.py` | from `program.nc`: vertical fed descents that remove stock, Z reversals, planar turn angles and reversals (> 90°), MRR by time, time split by motion (rapid / plunge feed / lift feed / cut / fed in air). Thresholds derived from the post's 3 decimals and the dexel volume quantum. | NEW. `runtime_by_intent` splits by LABEL; the audit splits by motion, which is the point (R4 §3.2 items 1-4). | (1) The descent count and the time split: a `stock/sim_triage.rs` observation over the trace samples, by predicate only, published measured or abstained, every strategy. (2) Turn angles, reversals and Z reversals: `machine/kinematics.rs`, beside the junction model the cycle time already walks; shown in `compute/toolpath_stats.rs` and `narrate_toolpath`. (3) The MRR histogram: `tool_load/distribution.rs`, which owns the histograms and their populations. Read the post-dressup `Toolpath` moves, not the G-code text: the export writes the moves as given. |
| Entry selection by span: `fix9/entries.py`, `entries_fix12.py`, `entries_d25.json` | entry spans by span style, their descent depth and removed volume | DUPLICATE of the audit's descent predicate, but label-based | retire for the audit's descent observation plus the C40 cause on the span (§1 row 15) |
| C40 joins: `fix15/c40_join.py`, `turn_place.py`, `parta_place.py` | joins research log lines to sim samples by XY | RESEARCH-ONLY. The join exists because the cause is in a log line, not in the trace. | none; with the cause on the entry span (§4 step 0) the join is a field read |
| Width and load oracles: `fix24/area_oracle.py`, `rescore.py`, `c32/thick_measure.py`, and the Rust test oracles `exact_width_at` (`tests/adaptive3d_geodesic_arm_c12.rs`) and `print_exact_width_along_move` (`M/adaptive/CLAUDE.md`) | the exact width and the per-strip load L at a 0.05 mm lattice | NEW as a measure; DUPLICATE as code (three oracles in two languages, and `thick_measure` is an older form that L supersedes) | The dexel stamp already finds the fresh cells of each disc for its radial width (`dexel_stock/stamping.rs`, perp extent). Make it also keep the deepest fresh depth per lateral strip and write L per sample with the one D5 function (C22 §18.3 item 4). Then the sim gives L itself, the triage counts "length over the bound", and the Python oracle remains a cross-check only. Merge the two Rust test oracles into one `test-support` module. |
| Load splits by producer: `fix21/load_split.py`, `fix23/why/why.py`, `nearA_*.py`, `d31_rise.py`, `drape_left.py` | over-bound length by move kind, by place, by an earlier higher pass | RESEARCH-ONLY as written (keyed to research log lines). One part is NEW: "over-bound length per producer" | Once the length over the bound is in the sim (row above), split it by the span's producer in `tool_load/distribution.rs`. That gives CP its own split (§4 step 0 item 5) with no script. |
| Feed checks: `fix21/feed_check.py`, `strip_feed.py`, `slotfeed.py`, `turnfeed.py` | program F against the planner's lowered feed | RESEARCH-ONLY. After §3 D6 the cap is on the move. | the sentry `a_feed_cap_binds_every_feed_stage_c38` already holds "feed ≤ cap" in Rust; retire the scripts |
| Held-link and peak probes: `held_links.py`, `peak_move.py`, `op1_deep.py`, `clipair_base.py`, `links.py` | which move cut what, at which depth | DUPLICATE in part: `link_cut_observation` names the worst link move; the deep-sample count is G-AXIALDOC's `crosses_standing_material` | none new; the triage findings carry the worst move |
| Cycle-time port: `phase2/c21/model.py`, `run_c21.py` | a Python port of `machine/kinematics.rs` and the adaptive3d entry emitter | DUPLICATE (the port reproduces Phase 0 runtimes within 0.06 %) | none; the CLI's cycle time with the label-free time split (audit row) answers the same question. Fixture files need per-axis Z rates for it to be right (`STATUS.md` C21) |
| Keep-down route prototype: `fix18/lowtravel.py` | a Dijkstra over free cells on a re-stamped stock | DUPLICATE since `keep_down_route.rs` | none; a "routable entries" count can come from the Rust router in a dry-run mode, if the operator wants it (§4 step 6) |
| Renders: `fix9/render.py` plane, drape and crop images; `fix17/fixture3d_v5.py` pages | strict-plane, drape and 10 mm crop images per column | NEW in part: a strict-plane filter (moves on one Z level) and a fixed crop for side-by-side columns | the GUI toolpath view and MCP `screenshot_toolpath`: a "this Z level only" filter. The comparison pages stay research tooling. |
| 2D prototypes and harness readers: `phase2/c13`, `c15`, `c17`, `c19`, `zone.py`, `sweep.py` | exact-geometry prototypes of design ideas | RESEARCH-ONLY | stay as evidence |
| Page asserts: `assert_*.py`, `make_readme.py` | assert each number a page quotes | RESEARCH-ONLY (the operator's rule for explanation pages) | stay |

### 6.3 What to port, in order

These are Step 0 of §4 (no output change). They give the operator the
readings in the GUI, MCP and CLI.

1. **The label-free descent and time split** (from `audit.py`) as a
   `sim_triage` observation. Every strategy. It is the reading that would
   have shown the 1195 Corne plunges at once (R4 §3.2 item 1).
2. **L per sample in the sim**, from the dexel stamp, with the one D5
   function, and "length over the bound" per op and per producer in
   `tool_load`. It replaces `area_oracle.py`, `rescore.py`,
   `load_split.py` and `thick_measure.py` for routine counts.
3. **The `StandingField` export and its distance-band split.** It replaces
   the stock rebuilds and `bands_*.py`.
4. **Turn angles, reversals and Z reversals** beside the cycle model, in
   `toolpath_stats` and `narrate_toolpath`. It measures the operator's
   "smoother in acceleration" report (R4 §0 item 5).
5. **The entry cause on the span** (§1 row 15). It retires every XY join
   script.

Sentries: one per observation, each with a planted case and a vacuity
check (the `gate_population_vacuity_xvac` pattern in `M/tool_load/CLAUDE.md`).

**Rule for future research (proposal):** a script may read the Rust
outputs above. It does not re-stamp stock, re-parse G-code for a quantity
the trace holds, or join log lines by XY. A new quantity starts as a
script, and it moves into Rust when a decision rests on it. The old
scripts stay as the evidence of what was measured then. The planning
rule says delete, not archive, for obsolete material only.

**Operator decisions:** Q10. Which of items 1–5 does the GUI show, and
which only the CLI and MCP give? Q11. Is the 0.8 mm band threshold replaced
by the coverage bar (leave + one cell)?

## 7. Lead defaults for Q1–Q11 (2026-10-27)

The operator delegated these questions to the lead ("I think you know better"). Each default holds until step 0's measurements or the operator change it.

- **Q1.** Yes. The per-strip L (`mrr_cross_section_mm2` summed per strip, over the same at a full pass) is the one load definition for the planner, the sim report and the judges. It moves into `tool_load/`. The app reports it.
- **Q2.** One derived allowance for a level side link: Depth/Pass + one planner cell, in the planner and the triage alike. The typed 0.5 goes. Step 0 measures what this changes on CP before step 2 ships it.
- **Q3.** Yes, but only with the reduced feed and the feed cap. An exempt move without a cap is not allowed. It ships per strategy after step 0 shows that the strategy has such moves.
- **Q4.** Yes. G-CLIPAIR goes to CP first (step 3), after step 0 shows how much of CP's deep cuts and rapid collisions it explains.
- **Q5.** AgentSearch stays until the geodesic arm ships, then it is marked legacy (not deleted) in the GUI and MCP. The arm ships as body-only, and the GUI offers "geodesic body + CP edges" as one template.
- **Q6.** Yes. `StepSplit` (Part A) and its readers are deleted in step 7, unless D34 shows a use for it.
- **Q7.** Not yet. The router runs at the cut feed. The low rapid waits for C21's acceleration-aware comparison (C42c) and a derived clearance.
- **Q8.** Step 7 measures the arm with and without the C28/C29 replay pick on both fixtures. The pick ships only if it wins by more than the run-to-run noise on load or cycle, because it doubles the plan work.
- **Q9.** Name: **Geodesic Adaptive**. "Geodesic" alone could be read as a finishing strategy, which some other CAM packages use the word for. "Adaptive" says it is a constant-engagement roughing strategy. Visible parameters are the usual roughing ones: tool, stepover, Depth/Pass, feeds and boundary. The research options stay hidden.
- **Q10.** The GUI shows a short summary per op: the length over the load bound, the plunge count, and a smoothness reading. The full tables stay in the CLI and MCP.
- **Q11.** Yes. The coverage bar (leave + one cell) replaces the 0.8 mm band threshold, because it is derived.
