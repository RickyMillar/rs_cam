# D69: per-level contour, helical and region-first Waterline, Waterline tabs, hole skip (Corne play job)

Tree: fixture worktree detached at `5974e642c`, release `rs_cam_cli`, no research feature. Line references are master
`53838c22b`. Prototypes were applied, built, run and removed; the worktree is clean at `5974e642c` and its release binary
is rebuilt from it. Nothing is committed. Every simulation.json is deleted. `assert_d69.py` asserts each number below
from the files in this folder (`python3 -I assert_d69.py` → "all D69 assertions hold").

Jobs: copies of the play projects (`jobs/corne_play_base.toml`, `jobs/wanaka_play_base.toml`, `--setup 0`; wanaka
`--resolution 0.6`). Corne ops: 1 = Geodesic 6 mm (DPP 6, stepover 2.1), 5 = CP rest edge op (6 mm, DPP 6),
3 = CP copy op (3 mm, DPP 3), 4 = Waterline (6 mm, z_step 1.0, sampling 0.5).

Clocks: "summary" = the acceleration clock (`trace.toolpath_summaries[].runtime_by_intent`, `opsum.py`). "sample" = the
commanded-feed sample clock, no acceleration. "nc-time" = `rs_cam_cli nc-time --rapid-feed 4000` (the F-034 integrator on
a G-code file). On the base Waterline nc-time gives 1773 s and the summary 1300 s; so a nc-time delta × 0.733 is the
summary estimate. Entries = runs of non-rapid samples after a rapid.

## 0. The baseline (clean build, Corne play job)

| op | summary s | rapid | entry | cut + link | entries | peak axial | removed mm³ |
|---|---|---|---|---|---|---|---|
| 1 Geodesic | 890.3 | 79.5 | 68.4 | 742.4 | 125 | 7.5 | 218067 |
| 5 CP rest edge | 453.7 | 199.6 | 13.3 | 240.8 | 97 | 8.96 | 53572 |
| 3 CP copy 3 mm | 1689.9 | 418.3 | 87.7 | 1184.0 | 311 | 14.5 | 4992 |
| 4 Waterline | 1299.9 | 410.5 | **518.3** | 371.0 | 282 | 9.84 | 7805 |
| job | 4333.8 | | | | | | |

The model (`model_map.py`, `holes.py`): plate top Z 3.5, plate bottom = stock bottom = 0. 88 holes: 83 hexagonal switch
holes of 55–56 mm², inscribed width 7.8–8.0 mm, and 5 holes of 2 mm that no tool of the job enters.

## 1. A contour pass on each level of a 3D Rough

### What exists today

- **Walls: the per-level waterline cleanup** (`adaptive3d/clearing.rs:1693 waterline_cleanup`). Global ordering runs it
  after each level (`adaptive3d/path.rs:1214`); By Area runs it once at the bottom only (`path.rs:1163`, F9). It traces
  the mesh contour at the level (the part walls), only where the slope is over 30°, at a fibre sampling of
  `max(R, 4 cells)`, and it skips a contour inside a piece that the Geodesic arm left to the rest pass. Each contour is
  one Rapid and one Cut (no link rule). On the Corne op 1 it traced 2 / 1 / 1 / 30 contours at Z 17 / 11 / 5 / 0.5
  (debug log). It is not a user option. Its semantic items carry no move range (`move_start: None`), so no tool can time
  it.
- **Boundary band: D68 option 2** (`../d68/option2_proto.patch`), a prototype only. It contours the containment line where
  the reach band stands, at Z = max(level, band top − DPP).

### Proposal: one option, "Contour each level", for CP and Geodesic

- **What it contours.** Two kinds of line, one rule: "where stock still stands in reach after the level's clearing".
  1. The part walls: the model contour at the level, offset by the radial stock to leave. This is today's cleanup, made
     visible, with its sampling derived from the op's tolerance and not from `max(R, 4 cells)`.
  2. The boundary: the containment line where the reach band stands (D68 option 2). On a fresh op with no band this kind
     gives no segment, so the option costs nothing there.
  Not "the band left" as a region: that is option 1 of D68 (a region rule), which cost +2703 s on Corne.
- **When.** After the level's clearing. Then the contour meets at most the cusp that the clearing left (radial load ≤ the
  stepover), not a fresh slot. The boundary kind must run before the NEXT level's clearing, because its job is to hold
  that level's axial depth to one DPP (G-RESTDOC).
- **Link and entry.** Each segment enters from the cleared floor of the level (the planner stock knows it), through
  `plan_entry`. Between segments, a keep-down link inside the stay-down distance (`max_stay_down_distance_mm`, the ONE
  stay-down dial) when the corridor is clear, else retract. Order: nearest end first, climb direction.
- **Short-segment merge rule (derived).** On one ring, a gap between two runs is fed through at its own draped Z when the
  feed through the gap is not slower than a retract, a rapid and a re-entry. That is the existing one rule
  `machine/kinematics.rs:929 retract_saves_time` (Waterline already uses it, `ops/waterline.rs:370`). The prototype used
  `clearing.rs:1953 air_run_crossover_mm` (the op's feed and plunge, the retract depth), because the adaptive3d context
  holds no machine kinematics. No typed length.

### Measured (prototype `d69_proto.patch` = D68 option 2 + the merge rule behind `RS_CAM_D69_MERGE`)

All option rows carry the anchor fix port of D68 (`a0`); D68 §0 says not to ship the anchor fix alone.

| job / variant | edge op s | entries | max axial (DPP) | over DPP mm³ | band segments per level | job s |
|---|---|---|---|---|---|---|
| Corne base (clean) | 453.7 | 97 | 8.96 (6.0) | 6750 (12.6 %) | — | 4333.8 |
| Corne a0 (anchor only) | 460.1 | 168 | 12.0 | 14062 (26.3 %) | — | 4339.6 |
| Corne option 2 | 767.3 | 327 | 12.0 | 3507 (6.2 %) | 26 / 6 / 6 / 6 | 4995.3 |
| Corne option 2 + merge | 745.2 | 312 | 12.0 | 3170 (5.6 %) | 4 / 4 / 3 / 3 | 4973.1 |
| wanaka base (clean) | 1041.1 | 756 | 1.91 (5.46) | 0 | — | 3120.7 |
| wanaka a0 | 928.3 | 679 | 1.20 | 0 | — | 3007.9 |
| wanaka option 2 | 1756.4 | 1289 | 5.46 | 0 | 40 / 94 / 160 / 16 | 3836.0 |
| wanaka option 2 + merge | 1691.7 | 1212 | 5.46 | 0 | 40 / 69 / 74 / 7 | 3771.3 |

- The merge rule cuts the band segments by 68 % on Corne (44 → 14) and 39 % on wanaka (310 → 190; under 5 mm: 247 → 111).
  But it saves only 22 s and 65 s. The rest of the option 2 cost is not in the band segments:
  - Corne op 5 gains 159 entries with option 2 for 44 band segments (30 after the merge).
  - The downstream copy op (3) costs +354.5 s with option 2 (entries 311 → 374), although its own band has 0 segments.
    D68 saw the same (+354 s): op 5 leaves less stock, and the copy op's first level moves from 15.19 to 13.35.
- The merge rule cannot join runs on different rings. On wanaka 71 rings stay 71 entries per level at most. A link
  between rings needs the keep-down link rule above, not a longer gap.
- Holes: in this job the edge op's band is the outer ring only (1 polygon, 2 rings). The 83 holes do not reach option 2.

### Risks

- The option 2 cost is real: +639 s on the Corne job (+15 %) and +651 s on wanaka (+21 %) against the clean build. It
  holds DPP (wanaka max 5.46 = DPP; Corne over-DPP volume 6750 → 3170 mm³). It is a safety fix, not a speed gain.
- The 12.0 mm samples (Corne) stay; D68 did not isolate their cause.
- The walls kind runs on every fresh rough when the option is on. Measure it on rivmap100 before a default changes.

### Size

- Option surface (one bool on `Adaptive3dConfig`, schema, GUI row, MCP): S, about 80 lines.
- Boundary kind (D68 option 2 made product, band in the planner stock, merge rule with `retract_saves_time`, entry and
  link through `plan_entry`): L, about 500 lines + 3 sentries (DPP held on the band, segment count after merge, fresh op
  unchanged).
- Walls kind: the cleanup exists; the work is the option gate, the derived sampling and the move range of the semantic
  item: S–M, about 150 lines.

## 2. Waterline in the small holes: a helical waterline

### What exists today

- `ops/waterline.rs:454 waterline_level` emits each closed contour as its own closed pass
  (`emit_closed_contour_with_intent`): retract, rapid, approach, entry, ring. `compute/execute/finish_raster.rs:291`
  walks the ladder level by level (`waterline_ladder`, `ops/waterline.rs:171`).
- `WaterlineConfig::continuous` (`compute/operation_configs.rs:1853`) exists and no generator reads it (only Scallop's
  `continuous` has an effect). It is the natural home of this option.
- Bore (`ops/bore.rs`, FEATURE_CATALOG "Bore"): a helix on a picked drawing circle of diameter d, D < d ≤ 2 D. These
  holes are mesh hexagons, not drawing circles. A Bore at the inscribed 8 mm leaves the corners out to the circumscribed
  9.24 mm that the waterline ring reaches. So it is not a Bore op.

### Measured cost of the holes today

Waterline passes (`wl_passes.py`): 282 passes. Levels 17.99 to 4: 2 rings (outer and inner wall). Levels 3, 2, 1: 84
rings (the outer wall + 83 holes). The 249 hole passes cost, on the sample clock, 217.7 s rapid + 120.8 s entry +
40.9 s cut for 697 mm³. Each entry is a 2.5 mm zig-zag ramp of 0.5 mm steps (`program.nc`), slow on the acceleration
clock: the summary entry time (518.3 s) is 3.1× the sample entry time (169.1 s).

### Proposal: the rule

A chain is the rings of one region over consecutive levels. Ring u (level k) and ring v (level k+1) chain when:

1. one lies inside the other (to within the op's own `sampling`, the accuracy of the weave), and
2. no third ring of the two levels lies between them, and
3. the match is the only one both ways.

The rule is topological; it has no distance threshold. A chain of n rings is ONE entry: ring 1 flat at its level, then
for each next level one lap of the LOWER ring that descends linearly from the upper level to its own, then one flat lap
of the last ring.

Why descend along the lower ring: in 3-axis the blocked set of the tool grows (or stays) as Z falls (the tool at a lower
tip Z holds every point the tool at a higher tip Z holds, shank included). So the lower ring at a higher Z is never in
the blocked set: the lap cannot gouge. Each point of the ring is cut by the next lap at a lower Z, and the last flat
lap closes the floor, so the lap leaves no more than the stepped levels do.

### Measured (`wl_gcode.py`: the shipped rings re-ordered and re-linked in the G-code, timed with nc-time)

| variant | entries | nc-time s | Δ nc-time | Δ summary (× 0.733) |
|---|---|---|---|---|
| base (re-written, same as shipped: 1773 s) | 282 | 1773 | — | — |
| region first only | 282 | 1720 | −53 | about −39 |
| helix in the holes (chains with a ring bbox under 15 mm) | 116 | 1053 | −720 | about −528 (−41 %) |
| helix in every chain | 94 | 969 | −804 | about −589 (−45 %) |

- Chains found: 83 of 3 levels (the holes), 2 of 11 (the walls), 2 of 2, 7 single rings. The topological rule found the
  hole chains and broke the wall chains at the level where the inner wall ends (Z 4 to 3), as it must.
- Hausdorff distance between chained rings: 0.000 mm in the holes (vertical walls), up to 1.58 mm on the walls (the top
  fillet). The only new XY moves are the links from the end of one lap to the nearest vertex of the next ring: 0.52 mm
  or less.
- The 15 mm bound in the "holes" row is only the measurement split; the product rule needs no size bound.

### Risks

- The G-code variant is not simulated. The XY positions are the shipped rings, so no new gouge is possible, but the lap
  descends 1 mm over a 7 mm ring (about 8°). That is near the shipped zig-zag entry angle (0.083 mm per 0.5 mm, 9.5°),
  but no entry rule checks it. The product must read the op's ramp angle rule.
- `optimize_rapid_order` keeps the depth order per level (`TRANSFORM_DEPTH_ORDER`, `compute/catalog/capabilities.rs:726`).
  A chain must carry its own span, or the TSP barriers undo the order.
- The rest filter (`RunFilter::keep`) can split a ring into open runs. A chain must break where a ring is not whole.

### Size

M: about 300 lines in `ops/waterline.rs` and the Layered runner (collect a region's rings over the levels before the
emit; the chain rule; the lap emitter), the `continuous` dial wired to it, spans per chain, and 2 sentries (a vertical
hole gives one entry per hole; the lap never enters the blocked set on a drafted wall).

## 3. Waterline ordering and air

### What exists today

- Level by level: `waterline_toolpath_at_levels` and the Layered runner. `WaterlinePass::region_ordering` returns
  `ByArea` (`finish_raster.rs:381`) but has no effect: the op has one input surface ("One surface, so the order changes
  nothing").
- The rest filter exists: `RestUse::ContourFilter`, `compute/execute/standing_material.rs:554 standing_near` (a point
  is kept when stock stands within R + cell above the level), and the air stretch bridge `ops/waterline.rs:370`.

### Region first

Measured above: region first alone saves 53 s on nc-time (about 39 s on the summary clock). Its value is that it makes
the helix of §2 possible; alone it is small, because each pass keeps its own retract and entry.

Safety:
- Shank and holder: the tool goes no deeper in one region than level-first takes it; only the order changes. What differs
  is the stock in the OTHER regions while the tool is deep. On a rest Waterline the other regions differ only by the skin
  that the op itself removes (here 0.5 mm on the walls, `passes_base_corne.json`), so the holder sees the same stock to
  within that skin. On Corne the holder is far clear: stickout 45 mm puts the holder face at Z 46 at the lowest level
  (stock top 23); the 6.35 mm shank starts 25 mm above the tip (Z 26).
- A Waterline on fresh stock is the case where the order matters (region first leaves full stock beside a deep region).
  Rule: allow region first only on a rest Waterline, or run the existing collision check on the result (it runs today on
  every toolpath).

### Air: what drives the 88.5 %

The trace measures air by radial engagement: a cut sample with `radial_woc_fraction` < 0.02
(`stock/simulation_cut.rs:249 AIR_CUT_RADIAL_WOC_FRACTION`, "Source: none printed"). On this run the trace gives
812.3 / 889.3 s = 91.3 % (the GUI's 88.5 % is the same measure on another run or resolution). The samples show:

| sample class | time s (sample clock) | removed mm³ |
|---|---|---|
| cut, WOC < 0.02, removes stock | 279.3 | 4018 (51 % of the op) |
| cut, WOC < 0.02, removes nothing | 56.6 | 0 |
| cut, WOC ≥ 0.02 | 60.0 | 3508 |
| entries | 169.1 | 278 (107.8 s remove nothing) |

So most of the "air" is a label defect: the flank of the tool cuts a 0.5 mm wall skin above the tip, and the radial
engagement reads 0. The true air is 14.3 % of the cut (56.6 / 395.9 s) plus the entries. Every wall level removes stock
(120–420 mm³ per ring, `passes_base_corne.json`), so rest-aware level skipping gains little here: the rest filter is
already on, and no wall level is empty.

What drives the time instead:
1. the entries (518.3 s summary): one ramp per ring per level; fixed by §2;
2. the levels on vertical walls: the outer wall is vertical from Z 16 down, the inner wall at all levels (constant bbox);
   on a vertical wall a flat end mill's flank cuts the whole skin at any z_step. A slope-aware ladder: keep the z_step
   where consecutive rings differ (the scallop needs it), and on a run of coincident rings space the levels by up to
   the rough's DPP (the rough cut that wall at that axial depth with a larger radial load). Estimate (pass-cost method,
   sample clock): walls 33 → 10 passes, about −280 s.
3. the label: report the air of a finishing op by removed volume, not by radial engagement (new defect, below).

### Size

Region first on its own: S (about 100 lines, the chain grouping without the lap). Slope-aware ladder: M (about 250
lines; needs the deflection gate on the result, since a finishing pass has no axial cap, FEATURE_CATALOG ruling R2).
Air label for finishing ops: S (about 60 lines in the cut accumulator and the summary).

## 4. Tabs and the stock bottom

### What exists today

- Tabs: Profile only. `dressup/tabs.rs:954 plan_profile_tabs` (placement), `dressup/tabs.rs:70 apply_tabs` (the lift),
  called in `ProfilePass::emit_level`; FEATURE_CATALOG Profile row (G-THROUGHCUT notice, P5, P5b).
- Waterline floor: `waterline_ladder` drops every level at or below the stock bottom + `STOCK_BOTTOM_FLOOR_EPS_MM`
  (`ops/waterline.rs:118`, 1e-6 mm).

### Measured

`web.py` (the lowest stock top on the best path of standing stock from the part rim to the stock border): after every op
of the play job the web stands at Z 0.5 (op 1's axial stock to leave). The Waterline's lowest level is 1.0, so it does
not cut the part free here. But the web the Waterline leaves is (top − k·z_step) mod the ladder phase: with top 18 and a
z_step that does not divide 18 (0.7 gives a last level at 0.2), it cuts the outline ring down to any value above
1e-6 mm. So the free cut depends on the ladder phase, not on a rule.

### Proposal

- Do not add tabs to Waterline. A tab on a finishing ring leaves a step in the finished wall at every level below the tab
  top, and the part still needs a separating cut.
- Give Waterline a through-cut floor rule: on a ring that reaches the stock bottom (the outline of a through part), the
  last level stays at or above the stock bottom + the web the job holds. The web is derived: the tab height of a later
  tabbed Profile in the job, else the lowest floor any earlier op left on that ring (here 0.5, op 1's axial leave).
  Show the G-THROUGHCUT style notice on the Waterline when the rule acts.
- End the job with a tabbed Profile on the outline (exists today).

### Size

Floor rule: S (about 80 lines + 1 sentry). Tabs on Waterline: M (about 200 lines; `apply_tabs` works per level on a
closed ring, but placement on mesh rings and the finishing step are new). Not recommended.

## 5. Skip the small holes in the 6 mm rough

### What exists today

- F-038 forecast: a region whose forecast cut length is under `min_region_cut_length_mm` (15 mm here,
  `compute/operation_configs.rs:1706`) is skipped (`adaptive3d/clearing.rs:2909`). The geodesic plan filters short
  groups (`clearing.rs:2469`, `filter_short_groups` `clearing.rs:2550`). A piece where the field does not apply is left
  to the rest pass (`leave_piece`, `clearing.rs:2406`).
- None of these sees the holes: at Z 0.5 op 1's region covers the plate and the holes together (the plate stock stands
  above its own floor of 4.0), and the plan dips into each hole by the drape. The holes are not regions or pieces.

### Measured: where the hole time goes (tool tip below the plate top 3.5, centre inside a hole, `hole_time.py`)

| op | sample s | removed mm³ | visits | holes |
|---|---|---|---|---|
| 1 Geodesic 6 mm | 49.1 | 11121 | 136 | 80 |
| 3 CP copy 3 mm | 351.8 | 3988 | 1132 | 83 |
| 4 Waterline | 105.4 | 685 | 249 | 83 |

The 6 mm rough is the efficient tool in the holes (about 227 mm³/s on the sample clock). The 3 mm op spends 7× the time
for a third of the volume.

The operator's plan, measured (`jobs/corne_play_op1floor4.toml`: op 1 bottom pinned at 4.0 = plate top + op 1's axial
leave; prototype binary with no variable = the anchor port, so compare with a0):

| | op 1 s | op 3 s | op 3 in holes | job s |
|---|---|---|---|---|
| a0 | 890.3 | 1689.7 | 351.8 s, 3988 mm³, 1132 visits | 4339.6 |
| op 1 floor 4.0 | 818.8 | 2766.9 | 587.2 s, 14522 mm³, 1868 visits | 5351.7 (**+1012**) |

In this job op 1 does not cut the outer edge (its web stays at Z 17.5; op 5 cuts the edge), so the height limit changed
only the holes. Moving the holes to the 3 mm tool costs +1012 s (+23 %).

A derived rule, prototyped (`RS_CAM_D69_NARROW`): a closed piece (no cleared cell on its rim) whose inscribed radius is
under R + stepover gets only slot cuts (no room for one pass at the stepover beside the first cut), so leave it to the
rest pass. Geodesic: 0 pieces skipped (the holes are drape dips, not pieces). CP: 9 closed pieces skipped, all in op 3 (op 5 is unchanged at 460.1 s)
(the 2 mm holes and small pits); job +7.7 s. For the hex holes the rule would say "skip" for the 6 mm tool
(8.0 < 6 + 2 × 2.1 = 10.2) and the measurement above says that costs +1012 s.

### Proposal

- Do not skip the holes in the 6 mm rough. Keep the slot: a 3 mm deep slot with a 6 mm tool at DPP 6 is in the load rule
  the geodesic arm already applies to a slot (the D65 feed cap).
- A narrow-pit rule is worth having only for the case where the slot breaks the load rule: skip a closed pit narrower than
  D + 2·stepover only when its depth under the rim is more than the axial depth that the load model admits for a full
  slot (`tool_load` chip load fraction at full width). On Corne no pit is skipped.
- Spend the effort on op 3's time in the holes (1132 visits for 3988 mm³ of corner rest). Its in-hole time spreads over
  every Z from 0.5 to 3.5 (its ramp entries). The helical idea of §2 applies to it too, and a rest op's corner
  slivers are a merge candidate for the §1 rule.

### Size

The prototype rule: S (about 120 lines in CP, about 15 in Geodesic) but not recommended as is. The load-based pit rule:
M (needs the pit detection on the drape floor, not on the pieces).

## Ranked optimisations

| rank | change | effect on the Corne play job | size | recommend |
|---|---|---|---|---|
| 1 | Helical Waterline on chained closed rings (§2), wired to the dead `continuous` dial | entries 282 → 116 (holes) / 94 (all); −720 / −804 s nc-time, about −528 / −589 s summary (−41 / −45 % of the op) | M | yes, holes first |
| 2 | Slope-aware Waterline ladder on vertical walls (§3) | estimate −280 s (walls 33 → 10 passes) | M | yes, after a deflection check |
| 3 | Air label for finishing ops by removed volume (§3) | 0 s; the 91 % "air" reads 14 % + entries | S | yes (defect) |
| 4 | Waterline through-cut floor rule (§4) | 0 s; removes the ladder-phase free cut | S | yes (safety) |
| 5 | Contour each level, boundary kind = D68 option 2 + anchor fix + merge rule (§1) | +639 s job (+15 %); over-DPP 6750 → 3170 mm³; merge saves 22 s | L | yes as the G-RESTDOC fix, not as a speed option; first find the copy op's +354 s |
| 6 | Region-first Waterline alone (§3) | −53 s nc-time, about −39 s | S | only as part of rank 1 |
| 7 | Narrow-pit skip / op 1 height limit (§5) | +1012 s | S | no |

## New defect candidates

- **G-WLAIRLABEL**: a finishing op's air share reads radial WOC; 279.3 s of Waterline samples remove 4018 mm³ (51 % of
  the op) with WOC 0. The GUI's "88.5 % air" overstates the true air (14.3 % of the cut) by about 6×.
- **G-WLCONTINUOUS**: `WaterlineConfig::continuous` is a schema dial that no generator reads.
- **G-WLCLEANSPAN**: the 3D Rough "Waterline cleanup" semantic items carry no move range, so its time cannot be measured.

## Open questions for the operator

1. Helical Waterline: on by default for chained closed rings, or behind the `continuous` dial? Holes only, or every
   chain (walls too)?
2. On a vertical wall, may the Waterline space its levels up to the rough's DPP (rank 2)? It changes the axial load of a
   finishing pass, which has no axial cap today (R2).
3. Contour each level: do you want the walls kind as a visible option too, or only the boundary kind (the G-RESTDOC fix)?
   The boundary kind costs about 15 % job time on Corne.
4. The through part: should the Waterline stop at the web the job holds and a tabbed Profile end the job (recommended),
   or do you want tabs on Waterline?
5. Holes: the data says keep them in the 6 mm rough. Do the hex holes need the 3 mm op's corners at all (R 1.5 against
   R 3)? If not, op 3 can skip them and save most of its 351.8 s in the holes.

## Files

- `run_play.sh` (one run of a play copy), `opsum.py` (per-op clocks → `opsum_<run>.json`), `wl_passes.py`
  (`passes_base_corne.json`), `wl_gcode.py` (the waterline G-code variants `wlg_base_*.nc`, `wlg_base_stat.json`),
  `nc_time_waterline.txt`, `stockmap.py`, `model_map.py`, `holes.py` (`holes_*.json`), `hole_time.py`
  (`hole_time_*.txt`), `web.py` (`web_*.txt`), `depth_by_level.py` (D68 copy, `depth_*.json`), `three_clocks.py`
  (copy without its write into fix26), `assert_d69.py`.
- `d69_proto.patch`: D68 option 2 (with the anchor port) + the merge rule (`RS_CAM_D69_MERGE`) + the narrow piece rule
  (`RS_CAM_D69_NARROW`). Prototype quality; the CP part leaves an unused-variable warning.
- `jobs/`: the play copies and `corne_play_op1floor4.toml`. Run folders: `base_*` (clean build), `a0_*` (anchor port),
  `opt2_*`, `opt2m_*`, `narrow_corne`, `floor4_corne` (each with `tree.txt`: HEAD and the diff md5).
