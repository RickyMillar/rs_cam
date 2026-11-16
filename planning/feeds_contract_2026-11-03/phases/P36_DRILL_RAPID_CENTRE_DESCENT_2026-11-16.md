# P36: the drill rapid into stock and the centre descent of the front rough (2026-11-16)

Job: `planning/rivmap_block_job/nz_wanaka_21_lead_proposal.toml` at `fa77ebc2f`.
Instrument: the release CLI built from `fa77ebc2f` (`project --setup .. --emit-gcode`),
0.2 mm cell, cap only. All positions are in the setup frame unless the text says G-code frame
(G-code frame = setup frame + (18.5, 8.5)).

Both findings are code defects. This note gives the measured cause, the rule and the
predictions of each fix. The predictions were written before the code.

## 1. Finding 1: `project.rapid_collision` on "Drill light holes 6 mm end mill" (id 4)

### 1.1 What the stock holds at the holes

The probe: a scratch copy with the Drill on the remaining stock and an automatic Top Z. The
Drill then puts each R-plane 5 mm above `column_top` (the highest stock top in the tool radius
plus one cell). The R-plane in the G-code less 5 mm is the stock top under the tool.

| Holes | Stock top under the Ø6 footprint |
|---|---|
| 38 holes | 16.50 (the cavity floor) |
| (327.13, 379.02) | 16.79 |
| (327.13, 345.87) | 17.19 |
| (327.13, 312.72) | 17.60 |
| (327.13, 412.17), the corner hole | 25.50 (the stock top, not cut) |

The model floor is Z 16.5 at all 42 holes (`back_target_s1.stl`). The cavity walls are at
X 333.5 and Y 418.5 with a 3 mm corner radius (`cavity_d9.00.dxf`).

### 1.2 Why the wood is there (the job)

- "Back bulk rough 18 mm" (Geodesic Adaptive): the nearest fed tool centre to the corner hole is
  (321.37, 403.62), 10.32 mm away. The tool radius is 9 mm, so the hole centre is not cut. Along
  the wall X 333.5 the tool centre stops at X 322.7; the cut ends at X 331.7.
- "Back rest 6 mm": its fed moves stay in X 30.2..347.9, Y 34.2..266.6. It makes no fed move
  near the cavity wall above Y 266.6. Its log has 12 lines "C38 piece left to the rest pass
  z=16.55 .. why=no field".
- Research C38 section 16 (lead ruling 2026-10-27) leaves edge pieces to a rest contour pass.
  This job has no contour pass on the cavity. The cavity corner and a strip at the wall stay at
  their height after setup "1 Back". This is a job gap, not the cause of the rapid: section 1.3.

### 1.3 Cause A1: the R-plane follows a pinned Top Z below the standing wood (code)

`compute/execute/drilling.rs::drill_hole_columns`:

- Fresh stock: `retract_z = top_z + clearance`. The pinned Top Z is 16.5, so the R-plane is
  21.5 at each hole. The stock box top is 25.5.
- Remaining stock: `hole_top = column_top.min(top_z)`, `retract_z = hole_top + clearance`. The
  cap makes the same 21.5 under a column top of 25.5. Measured: the scratch copy with the
  remaining stock and the 16.5 pin gives R 21.50 at all 42 holes and the same finding (local
  move 13).

The rule exists for every other reader: `dressup::fresh_stock_top_z` (G-PECKSPLIT, ruling R1):
"a pinned top BELOW the stock top does not lower" a rapid plane, because material stands up to
the stock top. The Drill did not read it.

### 1.4 Cause A2: the rapid reorder takes a peck cycle apart when the R-plane is the safe Z (code)

The lead set the Top Z to 25.5 and got 11 collisions. Measured (scratch copy, fresh, pin 25.5):
11 rapids `30.5 -> 13.0`, each at a different hole. The G-code of one such hole
(G-code frame X182.942 Y404.098):

```
G0 Z30.5 / G0 Z13.0 / G1 Z8.5      (the LAST peck comes first)
G0 Z30.5 / G0 Z19.0 / G1 Z12.5
G0 Z30.5 / G0 Z25.0 / G1 Z18.5
G0 Z30.5 /            G1 Z24.5
```

The pecks of the hole are in the reverse order. The first rapid goes to Z 13.0 through wood
that no peck has cut.

`dressup/tsp.rs::split_into_segments` keeps a rapid inside a segment only when its target is
below the ceiling (`internal_link_ceiling_z`, the safe Z for a drill cycle). With the pin at
25.5 the R-plane is 30.5, the safe Z. Each retract to the R-plane then frames a segment. Each
peck is its own segment, and the reorder is free to put the pecks of one hole in any order.
The condition is: the retract plane of the op (automatic: the `safe_z` of the project, 30.5
here) is equal to the R-plane (the hole top + 5 mm, or the Drill `retract_z` when that is
higher). A project whose `safe_z` is 5 mm above its stock top has it on every Drill with an
automatic Top Z.

### 1.5 The rules

- R-A1 (one rule, `drill_hole_columns`): the R-plane of a hole is the clearance above the
  material that stands over the hole. With stock evidence that is the column top
  (`standing_material::column_top`), not capped by the Top Z. With no evidence (fresh stock, or a
  grid that does not see the hole) it is `dressup::fresh_stock_top_z(stock box top, Top Z)`.
  The cut start (`DrillHole::top_z`) and the hole floor do not change (F19).
- R-A2 (`split_into_segments`): a rapid that stays at the XY of the move before it never frames
  a segment of a canned cycle (a ceiling is given). The reorder exists to cut XY travel; a
  vertical rapid in place is the cycle's own motion. A hole is one segment at any R-plane.
- No refusal. Both rules change motion only.

### 1.6 Predictions

- P-A1 (sentry, fresh): a Drill with the Top Z pinned 9 mm below the stock top puts each
  R-plane at the stock top + 5. Red before A1.
- P-A2 (sentry, remaining stock): a hole under a column that stands above the pinned Top Z gets
  its R-plane 5 mm above that column top; a hole in the cleared pocket keeps the R-plane 5 mm
  above the pocket floor. Red before A1.
- P-A3 (sentry): with the R-plane on the retract plane of the op and a grid of holes, the fed
  descents of each hole are in falling order of Z, and no rapid ends below the lowest point
  that an earlier feed of that hole reached (or below the stock top before the first feed).
  The hole order still changes when it saves travel. Red before A2.
- P-A4 (lib, `tsp`): a peck cycle whose retracts are on the ceiling stays one segment per hole.
- P-A5 (job, setup "1 Back", the proposal file as it is): `project.rapid_collision` 1 -> 0.
  Each of the 42 holes: R-plane 21.5 -> 30.5, fed descents `[24.5, 18.5, 12.5, 8.5]` (4 pecks,
  3 before). Fed length per hole 14.0 -> 23.5 mm; 42 x 9.5 mm at 1000 mm/min = +24 s of feed.
- P-A6 (job, scratch copy with the Top Z at 25.5): 11 -> 0.
- P-A7 (job change, scratch copy `p36_remaining_drill.toml`: the Drill with
  `stock_source = "from_remaining_stock"`, the 16.5 pin kept): 0 collisions; 38 holes keep
  R 21.5 and 3 pecks, the corner hole has R 30.5 and 4 pecks, the three wall holes R 21.79,
  22.19, 22.60.
- P-A8: no op golden row of a Drill changes (the fixtures have no Top Z pinned below the
  stock). If a row changes, the lead rules.

## 2. Finding 2: `safety.non_centre_cutting_descent` on "Front bulk rough 18 mm" (id 20)

### 2.1 The move

Setup "3 Front" alone gives the same finding (move 18235, 1.03 mm). The tool: core Ø8 (c = 4),
blade protrusion p = 1. The straight-line limit is `atan(1/8)` = 7.125 deg.

Move 18235 is a cut move (`ClearingCut`), not an entry and not a link. It is in a run of short
cut pieces that the P22f-2a stock read splits and caps (`capped_split_3d`,
`departure_window`): the pieces end at moves 18222, 18223, 18230, 18231, 18235, 18236. Each
piece descends at the straight-line limit while the path turns from +X to -X (about 180 deg in
14 mm).

### 2.2 Cause B: the dwell rule reads one piece at a time (code)

The check is on the G-code, with no grid: at each vertex, the highest point of the fed path
before it inside one XY diameter of 2c (the window of `CoreZone::clamp_descents`), less the
vertex Z.

| G-code vertex (core move) | Window descent |
|---|---|
| 18226 (18224) | 1.069 mm |
| 18236 (18234) | 1.058 mm |
| 18237 (18235) | 1.057 mm |

The limit is 1.000 mm. So the emitted path breaks the rule of its own generator; the finding is
not a resolution effect of the 0.2 mm cell. The kernel slack is 0.02 mm. The printed 1.03 mm
is under the 1.057 mm of the G-code; the cause of that difference is not measured (the ring
of the same move cuts a part of the wood).

`adaptive3d::path::emitted_cut_path` clamps each cut piece with one anchor point (the tool
position). `CoreZone::clamp_descents` says: "The rule reads only this path: the history before
`anchor` is the caller's". No caller supplies the history. A window that starts in the piece
before sees only the anchor, so two pieces that each hold the straight-line limit can together
go down more than p in one window when the path turns. `emitted_link_path` has the same gap
(no anchor at all).

### 2.3 The rule

- R-B: the dwell rule reads the fed chain. `CoreZone::clamp_descents` takes a `FedLead`: the
  points fed before the path since the last entry, the last one where the tool stands. The
  window walks back through the path and then through the lead. The emitter and the planner
  stamp keep the same lead through one type, so the stamp is still the path that ships
  (G-PHANTOMSTAMP). A cut and a link extend the lead; an entry starts it again at the entry end.
- Pattern: the clamp takes no bare point. `emitted_cut_path` and `emitted_link_path` need a
  `&FedLead`, so a new caller cannot leave the history out (the compiler holds it).
- Limit, stated: the window does not go back through an entry (a ramp, a helix, a peck). The
  entry emitters hold their own core rules, and the planner models an entry as a vertical
  descent, so the lead cannot hold the entry moves and stay equal in both readers.
- A point only rises. A centre-cutting tool has no core: its path keeps every bit.

### 2.4 Predictions

- P-B1 (sentry, kernel): two cut pieces, +X then +Y, each a descent at the straight-line
  limit, clamped one by one with only the anchor: the simulation gives a core finding (the
  defect). The same pieces clamped with the lead: no core finding, and no point is lower.
- P-B2 (lib, `adaptive3d`): `emitted_cut_path` on the two pieces with the lead holds a window
  descent of at most p at each vertex; with a lead of one point (the old read) it does not.
- P-B3: a lead of one point gives the bits of the old anchor read; a centre-cutting tool keeps
  its bits (`adaptive3d_emission_byte_parity`, the op golden).
- P-B4 (job, setup "3 Front"): `safety.non_centre_cutting_descent` on id 20: 1 -> 0. On the
  G-code, the window descent at each vertex of a cut and link chain is at most 1.000 mm plus
  the 0.001 mm rounding of the file (two values).
- P-B5 (job): the path of ids 1 and 20 (the 18 mm tool, 3D Rough) can change, because raised
  points leave wood that the planner sees. The change of the cut time is reported, not
  predicted. Ids with a centre-cutting tool: no move changes, except the Drill of fix A.
- P-B6: no op golden row changes (no fixture has a tool with a core in a 3D Rough). If a row
  changes, the lead rules.

## 3. Results

See the hand-over report of the P36 agent; the lead adds the measured table here.
