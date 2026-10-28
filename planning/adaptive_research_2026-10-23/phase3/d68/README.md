# D68: G-RESTDOC options measured (option 1 region side, option 2 edge pass, option 3 refuse)

Tree: master `cc00aa0c3`, release `rs_cam_cli`, no research feature, no override. The fixture worktree was detached at
`cc00aa0c3`. Each prototype was applied, built through its `scripts/cargo_lane.sh`, run, and then removed. The worktree is
clean again, and its `target/release/rs_cam_cli` is the master build. Nothing is committed. All simulation.json files are
deleted.

Jobs: fix41's D60 job files (Corne B s2.1 body + edges, `--setup 0 --skip 4`; wanaka s2.0, `--setup 0 --resolution 0.6`).
Edge op: Corne toolpath 5 "3D Rough 1 (edges, CP rest)" (CP, 6 mm tool, DPP 6.0, derived rest regions, `Center`, offset 0).
Wanaka toolpath 7 (CP, DPP 5.46, derived rest regions).

## 0. Two facts that change the question

1. **The anchor fix (D63, `805368f98`) is not on master.** It is on `adaptive-instruments` only. Options 1 and 2 need it,
   so each prototype carries a port of it (`a0_anchor_port.patch`). Alone, it makes Corne WORSE:
   - the first level moves from 14.04 to 17.0, so the band at 23 is cut at Z 11 by the disc edge;
   - max axial 8.96 → 12.0 mm; volume over one DPP 6750 → 14062 mm³ (12.6 % → 26.3 %).
   - Do not merge the anchor fix alone.
2. **Wanaka s2.0 does not show G-RESTDOC today.** Its edge op has 72 derived regions. They do not collapse to one polygon,
   so the op has no pre-boundary and no boundary clear ("Cleared cells outside boundary polygon" is absent). Max axial is
   1.91 mm against DPP 5.46. The wanaka breach of D61 was s3.5, a different mechanism (D63 §2d). Wanaka s2.0 is therefore
   the MANY-REGION case of this study, not a breach case.

## 1. What each option changes

| option | what changes | which ops | shipped outputs that move |
|---|---|---|---|
| 1 region side | On prior stock, the boundary clear keeps the reach band (the G-CLIPAIR reach clear, for every strategy). A level region is the containment cells with stock under the centre, OR whose disc (R + cs/2) meets stock outside the containment above the level. Cells outside the containment are never region cells. | every 3D Rough on remaining stock with a containment: CP rest ops, CP copy ops, Geodesic rest ops | the edge op and the copy op of the template; any shipped CP rest job with stock in the band; multi-region rest ops change even with NO band (the region grid is now cut to the containment, see wanaka) |
| 2 edge pass | At each level, after the regions, one contour on the containment line (0.02 mm inside the clip) where the reach band stands above the cut. The pass Z is the level, draped, and never more than one DPP under the highest band top in reach. The band state is lowered by every emitted cut. | every 3D Rough on remaining stock with a containment and stock in its reach band | only ops with band stock; ops without band stock are unchanged (the Corne copy op: band 8506 cells, 0 segments) |
| 3 refuse | Generation or export refuses a rest op whose simulated axial depth exceeds one DPP (form (c) of D63 §6) | every rest op | no toolpath; refusals |

Prototype files (for the record; not product quality):
- `a0_anchor_port.patch`: the D63 anchor fix ported to master (path.rs only; the unit tests are not ported).
- `option2_proto.patch` (`apply_option2.py`): the anchor port + the edge pass, behind `RS_CAM_D68_OPTION2`.
- `option2b_proto.patch`: the same, plus a band read before the border clear (`RS_CAM_D68_BORDER`). It changed little.
- `option1_and_2_proto.patch` (`apply_option1.py`): all of the above + option 1 behind `RS_CAM_D68_OPTION1`.
  With no variable set, the binary equals the anchor port (checked: run `a0chk_corne` = `a0_corne` to the mm³).

## 2. Numbers (measured)

Edge op only, acceleration clock (`fix26/three_clocks.py`, the summary integrator). Entries = entry moves of the clock;
over DPP = removed volume of samples with axial > DPP + 0.05 (`depth_by_level.py`). Corne band left = `corne_op5.py`
"left after op 5" in mm² at 0–3 / 3–6.2 / 6.2–9 mm outside the silhouette. Wanaka band left = `op7_wanaka.py` inside
0–9 mm. Rows: `summary_rows.json`.

### Corne B s2.1 (op 5, DPP 6.0)

| variant | edge op s | entries | rapids | max axial | over DPP mm³ (share) | over DPP+3 mm³ | band left mm² | job s |
|---|---|---|---|---|---|---|---|---|
| today (master) | 453.7 | 132 | 413 | 8.96 | 6750 (12.6 %) | 0 | 193 / 0 / 18 | 3034 |
| anchor fix alone | 460.1 | 195 | 762 | 12.0 | 14062 (26.3 %) | 8163 | 193 / 0 / 18 | 3040 |
| option 2 (+ anchor) | 767.6 | 293 | 1181 | 12.0 | 2982 (5.3 %) | 268 | 192 / 0 / 0 | 3702 |
| option 1 (+ anchor) | 3156.6 | 577 | 5233 | 12.0 | 3530 (6.2 %) | 549 | 170 / 0 / 0 | 5737 |
| option 3 (c) | 453.7 | 132 | 413 | 8.96 | 6750 | 0 | 193 / 0 / 18 | REFUSED (op 5; the copy op too, see below) |

Over DPP by excess (`axial_hist.py`, mm³):

| variant | DPP+0.05..0.5 | +0.5..1 | +1..3 | > +3 |
|---|---|---|---|---|
| today | 1555 | 107 | 5088 | 0 |
| anchor fix alone | 3629 | 890 | 1380 | 8163 |
| option 2 | 2619 | 79 | 16 | 268 |
| option 1 | 2955 | 14 | 12 | 549 |

- Both options remove the band class: the samples whose centre is 5–7 mm outside the silhouette and meet stock at 23
  (`why_over.py`: 10430 mm³ in the anchor-only run, 420 mm³ with option 2, 0 with option 1).
- What stays is mostly 0.05–0.5 mm over DPP at Z 11, centres 3–5 mm outside the silhouette. That is the drape and
  tolerance class (G-AXIALDOC), not the band. A few samples still reach 12.0 mm (option 2: 19 samples, 268 mm³; option 1:
  44 samples, 549 mm³). Their cause is not isolated; in option 2 they are probably ring points where the band test and the
  sim disagree by a cell.
- Option 2 time: +314 s on op 5 (cutting +12 s; entry +150 s; rapid +157 s). The pass is 1517 mm long over 4 levels
  (25 / 6 / 6 / 6 segments). The cost is the entries and rapids, not the cut.
- Option 1 time: +2703 s on op 5. The reach ring inside the containment is narrower than CP's contour start, so CP falls
  to its cell-row cleanup raster: 5233 rapids. This is today's CP meeting the new region rule. A CP that plans the ring as
  one contour would cost about what option 2 costs (that contour IS the edge pass).
- Job: option 2 also moves the copy op (+354 s; its first level moves 15.19 → 13.35 because op 5 leaves less stock). The
  copy op has no band pass of its own.

### wanaka s2.0 (op 7, DPP 5.46; 72 regions, no boundary clear)

| variant | edge op s | entries | rapids | max axial | over DPP | removed mm³ | band left mm² | job s |
|---|---|---|---|---|---|---|---|---|
| today | 1041.1 | 916 | 1468 | 1.91 | 0 | 1331 | 3650 | 3121 |
| anchor fix alone | 928.3 | 925 | 1063 | 1.20 | 0 | 1339 | 3649 | 3008 |
| option 2 (+ anchor) | 1756.4 | 1724 | 2279 | 5.46 | 0 | 2625 | 3599 | 3836 |
| option 1 (+ anchor) | 412.6 | 340 | 705 | 3.68 | 0 | 886 | 3673 | 2492 |
| option 3 (c) | 1041.1 | 916 | 1468 | 1.91 | 0 | 1331 | 3650 | passes |

- The anchor fix reads the frame stock in reach (25.0) and the D50 guard keeps it, so op 7 gets 4 levels in place of one.
  It is 113 s faster.
- Option 2 holds one DPP (max 5.46 = DPP) but adds 828 s: 310 band segments over 4 levels, 1721 mm of pass.
- Option 1 is faster (−629 s) because the region grid is cut to the containment: today CP plans contours over material
  outside the 72 regions and the centre clip cuts them into fragments. It removes 445 mm³ less and leaves 23 mm² more
  band. This gain is from the "never outside the containment" half of option 1, not from the band.

### Option 3 on these jobs

Form (c) with no margin refuses the Corne template today twice: op 5 (8.96 against 6.0) and the copy op (12.0 against
3.0, 153 mm³ over DPP+3; D62 saw the same). After option 2 it still refuses both (op 5: 268 mm³ over DPP+3; copy op:
131 mm³). So option 3 needs a margin or a volume share, and every such number must be derived (D63 §6). It passes wanaka
s2.0.

## 3. Many boundaries and many regions

Band segments per level (option 2 log "D68 band edge: level"; a segment is one run of the containment line where the band
stands):

| case | containment | ring length | segments per level | of them < 5 mm |
|---|---|---|---|---|
| Corne op 5 | 1 polygon, 2 rings | 1194 mm | 25 / 6 / 6 / 6 | 10 / 0 / 0 / 0 |
| wanaka op 7 | 72 polygons, 71 rings | 1998 mm | 40 / 94 / 160 / 16 | 32 / 76 / 124 / 15 |
| Corne copy op (3 mm tool) | 1 polygon, 1 ring | 680 mm | 0 at every level | — |

The planted 83-hole case did not work. `jobs/corne_holes_silhouette0.toml` bounds op 5 by the model silhouette, but the
silhouette fills the switch holes (1 ring, 656 mm). That run shows only that option 1 can create a new refusal: its copy op
got a 16.5 mm cut (depth limit 4.5) and the export refused, while today, the anchor fix and option 2 export.

Measured cost per segment (option 2 against the anchor port; cut time = length / feed, the rest is overhead):
- Corne: +307.5 s for 43 segments and 1517 mm at 4000 mm/min (23 s cut), so 6.6 s overhead and 2.3 entries per segment.
- wanaka: +828.1 s for 310 segments and 1721 mm at 1000 mm/min (103 s cut), so 2.3 s overhead and 2.6 entries per segment.

Estimate for 83 switch holes, if band stock stood in each (method: the per-segment costs above; each hole one closed ring of
about 56 mm, 4 levels):
- option 2: 332 segments, about 18600 mm of pass (279 s cut at 4000 mm/min) + 760–2190 s overhead: about 1000–2500 s;
- option 1 with today's CP: op 5's measured +2703 s for 1194 mm of ring scales to about 10500 s for 4650 mm per level
  (linear in ring length; the raster rows scale with ring area);
- option 3: no added time; it refuses or passes the op as a whole.

Where each option scales badly:
- **Option 2:** with the NUMBER of segments, not their length. On wanaka 75 % of the segments are under 5 mm, and each costs
  about 2.3 s of entry and rapid for a few mm of cut. It needs a merge rule (join runs on one ring with a short gap, drop runs
  that remove no stock) before it ships. Its cost grows with levels × segments.
- **Option 1:** with ring AREA on today's CP (the cleanup raster), and with every code path that reads the stock under the
  centre only (the CP cleanup Z clamp, entries, drape). The holes run shows that path: a 16.5 mm cut and a refusal on the
  copy op. On many regions without a band it helps (wanaka −629 s), because it stops planning outside the containment.
- **Option 3:** it does not scale with geometry, but it refuses whole jobs, and the residual G-AXIALDOC spots make a
  no-margin rule refuse the template even after a fix.

## 4. Recommendation

**Option 2 with the anchor fix, in one change, plus a short-segment rule; option 3 (c) as a warning first, not a refusal.**

- Option 2 holds the band class on Corne: over DPP+3 falls 8163 → 268 mm³ (against the anchor port) and the 6.2–9 mm band
  left goes 18 → 0. It moves only ops with band stock. Its cost is +314 s on Corne op 5 (+68 %) and +828 s on wanaka op 7
  (+89 %). Most of that is entries on short segments, which a merge rule can cut.
- Option 1 holds the rule too, but on today's CP it costs +2703 s on Corne and created a 16.5 mm cut and a refusal in the
  planted run. Its "region never outside the containment" half is worth its own study for many-region rest ops
  (wanaka −629 s).
- Ship the anchor fix only together with option 2. Alone it doubles the Corne breach.

Risks:
- The prototype pass has no own load check. The pass is a slot-like cut on the disc edge (up to R radial, one DPP axial).
- About 268 mm³ at up to 12 mm stays on Corne op 5 after option 2, cause not isolated. The sentry for the product change
  must name a measured bound for it, not "zero".
- The option 2 prototype orders segments nearest-first per level, with no link rule of its own. Entry counts in the
  product will differ.
- Option 2 also changes the copy op's levels downstream (+354 s on Corne). Measure the whole template, not op 5 alone.
- The prototype band state is a planner-side estimate. The product must stamp the band in the planner stock (the G-CLIPAIR
  reach clear) so the plan, the sim and the entries read one stock.

## Files

- `run_one.sh` (fix46's, job mode, with a label and a `holes` part), `summ.py` → `summary_rows.json`,
  `axial_hist.py`, `why_over.py`; copies of `depth_by_level.py`, `entries.py` (5th argument: toolpath), `corne_op5.py`,
  `op7_wanaka.py`, `corne_reach.py`, `tpsplit.py`.
- Run folders `<variant>_<part>/` (stdout, stderr, summary, program.nc, per-toolpath json, `tree.txt` with HEAD and the diff
  md5). Variants: `base`, `a0` (anchor port), `a0chk` (option binary, no variable), `opt2`, `opt2b`, `opt1`.
