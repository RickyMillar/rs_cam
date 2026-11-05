# Feeds sanity-check review (STATUS item 4e, brief section 5)

Date 2026-11-06. HEAD `ec5cbe4a9` (hand-off `f0150aeb5` plus P15). The reviewer changed nothing
under `crates/`. `python3 -I assert_claims.py` checks every number in this file, in
`DISAGREEMENTS.md` and on `index.html`. The page is published as a private artifact (link at the
end).

## 1. Process record

- The judgement came first. `PREREGISTRATION.md` fixed the pass rule and the cells.
  `JUDGEMENT.md` holds the reviewer's safe cut per toolpath from printed sources only.
  `reference.py` checks that each of its 50 printed values is on its cited line.
- Disclosure: before the judgement the reviewer saw three sets of engine numbers: the brief's
  "Face 5" observation, the first two FM1 rows, and the numbers that WHAT_IS_DONE section 4
  quotes. `PREREGISTRATION.md` names them.
- GUI check: the feeds lead built `rs_cam_gui` from HEAD (binary time 11:43). The reviewer
  called `gui_restart`. The new GUI started at unix 1793918703, after the binary mtime
  1793918597. Every MCP number in this review comes from that process.
- CLI: the release CLI was older than HEAD. The reviewer rebuilt it through
  `scripts/cargo_lane.sh`. The build took 245 s (`runs/cli_build.txt`). That is over the
  3-minute line, and the reviewer did not ask first. The reviewer expected a short build
  because the lead had just built the release core.
- Agents: three read-only agents collected the evidence for the reviewer items by path. The
  reviewer wrote every verdict.

## 2. Method

1. The judgement (`JUDGEMENT.md`) gives depth, width, chip, rpm and feed per toolpath of both
   fixtures, and general rules G1-G12 for the matrix.
2. The engine: `suggest --json` on both fixtures and the softwood row (`runs/`); 67 matrix
   cells on the rivmap100 Shapeoko table (`matrix/run_cells.py`, `matrix/cells.json`); 25
   power-ladder cells (`matrix/power_cells.py`); 5 post-sim cells through the CLI `project`
   door and the MCP load report (`matrix/postsim.py`, `matrix/gate_mcp.json`); aggregates of
   the FM1 golden (`fm1_stats.py`).
3. Two layers. Layer V compares with the vendor row of the same tool, size and material (the
   pass rule of the brief). Layer H compares with the hobby charts (Sienci, Carbide 3D),
   because both fixtures run a hobby table.
4. The CLI `tp_*.json` record holds no load field. The CLI `summary.json` holds only the
   envelope placement (`lineage.measured`). The chip, depth and deflection verdicts come from
   MCP `get_tool_load_report`.

## 3. Results per toolpath kind (the 4e comparison)

Verdicts: **Pass** (inside the printed band and limit), **Finding** (a row in
`DISAGREEMENTS.md`), **No reference** (no printed row covers the cut).

| Kind | Fixture toolpath | Engine (shipped) | Judgement | Verdict |
|---|---|---|---|---|
| Face | rivmap100 Face 5, 6 mm 2F, white oak | chip 0.1311 mm, feed 5550 mm/min at 18000 rpm | chip 0.114-0.127 mm | Finding D1: the chip is 3.3 % over the one printed Spektra chip 0.127 mm |
| Face | wanaka Face back / front, 18 mm 2F | 0.5 mm at 0.4 D; the 6000 mm/min cap lowers rpm | Sienci 22 mm: 1.0 mm at 0.4 D | Pass; the cap is D9 |
| Pocket | wanaka Face clamp pads, 6 mm 2F | Spektra row with the D1 scale; stated 0.5 mm | Sienci pocket 5.6 mm at 0.45 D | Pass on depth; chip D1 |
| Pocket | wanaka Lake pockets, 2 mm 1F | chip 0.0699 mm | 0.0508-0.0584 mm (1/16 in and 3/32 in rows) | Finding D10 |
| Profile | wanaka Outline profile, 6 mm 2F | 1.5 mm per pass (stated), Onsrud 60-200 band, 10610 rpm (stated) | slot 2.5 mm per pass | Pass V (inside Onsrud); D1 scale applies |
| Drill | wanaka Pin drill, Drill light holes | plunge 1835 mm/min at 14000 rpm | 1370-2286 mm/min | Pass |
| Project Curve | wanaka River channel, V rivers | calculator depth 0.36 mm and 0.19 mm; lineage -4.0 mm | the real depth into the stock | Finding D7 |
| 3D rough | rivmap100 3D Rough; wanaka bulk and rest roughs | peak chip at the band; depths stated | no printed wood adaptive (G10) | No reference for depth and width |
| 3D finish | rivmap100 and wanaka scallop, 2 mm tip tapered | chip 0.0644 mm, Amana ZrN 1/16 in row | SpeTool 2.0 mm tip 0.0762 mm | Pass (lower, other vendor) |
| Trace / V | rivmap100 Project Curve 20 deg; wanaka V rivers | chip 0.127 mm | Amana 18/30 deg 1F 0.0762-0.178 mm | Pass |
| Trace / V | wanaka V60 slit and labels | feed 2743 mm/min | Amana 90 in/min = 2286 mm/min | Finding D11 |

## 4. The cases the lead named

### 4.1 Brief section 2: does the chart print a larger hardwood chip?

No. Spektra v24 prints one Wood/Plywood chip: 0.127 mm at 6 mm. The softwood cell ships
0.127 mm and 5375 mm/min. The white oak cell ships 0.1311 mm and 5550 mm/min. The ratio is
1.0326. The cause is the hardness transfer, not the material family. The Spektra rows carry the
engine proxy Janka 1450, and white oak is 1360. The transfer scales by the square root of
1450 / 1360, which is 1.0326. No chart prints the proxy. A one-column chart has no slope
inside its column. The missing term is a hardness span per row (D1).

The same proxy flips the row. The lookup score gives up to 80 points for hardness. The Spektra
row gets 80 for generic hardwood and 75 for white oak. The Freud row prints no hardness and
gets 0, but it gets the grade-A and "exact" bonuses. For a 6.35 mm 2F tool in white oak the
engine ships `freud-solid-carbide-quarter-hardwood` at 0.2492 mm and 10026 rpm. In generic
hardwood it ships Spektra at 0.127 mm. Freud prints 0.2032-0.2794 mm; Spektra and Onsrud print
about half that. The post-sim gate then judges the white oak cut against the Freud band (max
0.2885 mm after the 1.0326 scale). The transfer reads a missing hardness as the family default;
the score reads it as 0 points. For generic hardwood the two rows tie and Q3 (the lower chip)
picks Spektra; for white oak the hardness points break the tie before Q3 runs. The missing
terms: one reader of a missing hardness, and an agreement term across vendors (D2).

### 4.2 R37: 3.175 mm hardwood Pocket at 2.2 x D

The free proposal is 6.985 mm deep and 2.32 mm wide at chip 0.0889 mm (Freud 1/8 in row). Sienci
prints 5.6 mm deep at 1.45 mm for a 1/8 in hardwood pocket. The proposal is deeper and wider.
The vendor depth table binds at the 2.2 x D point, so layer V passes at the limit.

Suggest and the gate disagree on the same cut. On the simulated cut the depth gate reports
"exceeds": the force-budget depth is 5.00 mm at 0.1096 mm/tooth. The Sienci point passes the
same gate (budget depth 9.02 mm). Suggest sizes the depth at the commanded advance 0.0889 mm.
The depth gate reads the sample's own feed field; the chip gate reads the achieved advance. The
depth gate's value is the same to every digit in both cells, although their op feeds differ,
so it may not be a property of the cut. Which door is right is open.

Chip clearing and breakage: no printed source in the repo gives a chip-clearing limit other
than the depth table. The verdict: open. The pick is deeper and wider than the hobby chart, and
it meets the vendor table at its limit. The term that closes the disagreement: one feed for
Suggest and both gates (K7, D3).

### 4.3 R36: the hobby budget and the hardwood slot

The 6.35 mm generic hardwood proposal is 3.39 mm deep at 0.85 D. Sienci prints a hardwood slot
of 2.5 mm; Carbide 3D prints Mahogany at 2.54 mm. At the Sienci feed the engine prints 24.67 N
for the hardwood slot and 30.25 N for the softwood slot (P13.md). Per mm of depth the engine
separates hardwood from softwood by 1.76. Sienci separates them by a depth ratio of 2.16 at the
same feed and rpm. The force line has a grain factor of 1.0. The missing term: a species factor
beyond density (D4). The VFD power ladder (part B) measures power, not peak force, so it cannot
decide this alone; a hardwood cut at the engine depth with a power reading is the nearest test.

### 4.4 R30: the bull nose below the corner radius

Closed. A 6 mm bull nose with r 1 mm ships the same feed at 0.5 mm and 3 mm deep, width 0.5 D.
No false lift remains below the corner radius.

### 4.5 R41, R38, R42: light widths

- R38: Harvey 809500 (a wood chart) prints Finishing at 0.764 x Slot-Rough, at 0.1 D radial and
  3 D axial. The no-thin reading is 0.5; the thinned reading is 0.833. The printed point is
  nearer the thinned reading. Garr's metal finish row prints a low chip. Verdict: a light
  finish pass in wood should thin, with the depth table; Harvey is the only wood witness (D13).
- R42: a stated 0.05 D width lifts the feed by 2.29. The simulated pocket meets a peak radial
  fraction of 0.83 at its opening cut and corners. The depth gate exceeds: budget 1.85 mm
  against 6.0 mm. The depth gate reads the commanded lifted feed; the chip gate reads the
  achieved advance and stays within. Two terms can explain it: the engagement spread of the
  planned path, or one feed for both gates (D6). A cap on the lift is neither.
- R41: the Waterline width assumption 0.1 D is an Unsourced default. Verdict: read the width
  from the stated stepover, and take the D6 term for the engagement spread.

### 4.6 Plywood (MODELS.md M4)

Baltic birch has no force line. The 6.35 mm proposal is 10.58 mm deep at 0.85 D, row
`freud-solid-carbide-quarter-plywood-hardwood`, power 0.0. Sienci prints 2.5 mm for "Hardwood,
Hard Plywood". The result is Unmodeled, not a pass. The depth has no force or power limit (D5).

## 5. The matrix

- FM1 (generic machine): 524 of 1000 cells ship; the other cells refuse an op that the tool
  cannot cut. No generic hardwood cell ships a larger chip than its softwood pair, because the
  generic species sits on the proxy. FM1 has no species cell, so it cannot see D1 or D2.
- FM1 sim subset: no gate exceeds, but the chip gate is Unmodeled on 24 of 38 rows. The gate
  judges few of the engine's own picks on the small fixtures. The post-sim cells of this review
  (D3, D6) show two depth-gate exceeds on engine picks; both rest on the depth gate's feed.
- Power ladder: with a shank that the machine holds, the feed falls smoothly as the depth rises
  (`matrix/power_cells.json`). The jump of D8 (62 mm/min against 5212 mm/min at 4.04 kW) shows
  only in cells that also carry `ShankTooLarge`.
- The default cutting cap 6000 mm/min binds on the 18 mm and 3F cells. It has no stored source
  and no F201 row (D9).
- Rigid table: no printed spec gives the rigidity block. The review scaled the hobby budget by
  the Sienci stiffness ratio (AltMill / LongMill MK2), to 182.8 N. The vendor depth table then
  binds at 1 x D. Every rigid result is Unsourced.

## 6. Verdicts on the reviewer items (WHAT_IS_DONE section 4)

Vocabulary: **Closed**; **Open** (confirmed at HEAD); **Open + term** (the review adds a term or
evidence); **Keep** (the reviewer rules no change); **Not judged** (the reason is given).

### 6.1 Engine defects and model terms

| # | Verdict | Note |
|---|---|---|
| R30 | Closed | §4.4 |
| R31 | Open | Delete `BandMid` (`dressup/feed_modulation.rs:78-83`); one test reads it |
| R32 | Open | Rename; no path refuses (Q6) |
| R33 | Open | Delete `StockToLeave`; no producer |
| R34 | Keep | 0.002 mm is under the 0.005 mm convergence step; document the inside end |
| R35 | Open | One feed per FM1 row, then re-bless |
| R36 | Open + term | §4.3, D4: a species factor beyond density |
| R37 | Open + term | §4.2, D3: Suggest and the depth gate disagree; one feed for Suggest and both gates |
| R38 | Open + term | §4.5, D13: Harvey supports thinning a light finish |
| R39 | Open + term | D10: interpolate between the bracketing printed rows |
| R40 | Open | The LUT holds Amana ball rows from 1/16 in to 3/4 in for hardwood, softwood and MDF, so a size gap does not explain the formula cells. This review did not trace their route |
| R41 | Open + term | §4.5 |
| R42 | Open + term | §4.5, D6: the engagement spread or one gate feed, not a cap |
| R43 | Open | Per-element radius; no number tested here |
| R44 | Open | No printed wood `K_r` found |
| R45 | Open | No printed helix found |
| R46 | Keep | The doc bounds the step (0.75 mm or less on an 18 mm flute); document |
| R47 | Open | Read `d_eng` |
| R48 | Open | One depth field |
| R49 | Not judged | No wood source on axial thinning |
| R50 | Open | No printed carbide strength in the repo |
| R51 | Open | Paint the peak chip |
| R52 | Keep | Keep `doc_derating_scale` as the printed table that the inverse reads |
| R53 | Open + term | Sienci prints a pocket stepover of 0.45 D (G5); the 0.85 D threshold has no source |
| R54 | Open | Store the derived budget as derived |
| R55 | Open | Move the stored text under `crates/rs_cam_core/data/` |
| R56 | Open | Not measured |
| R57 | Open | Not measured |
| R58 | Open + term | Sienci 0.45 D pocket; Fusion prints no wood adaptive width |
| R59 | Open | No printed `k` |
| R60 | Open | Compliance per move |
| R61 | Open + term | §5: the chip gate is Unmodeled on 24 of 38 FM1 sim rows; D3 and D6 exceed |
| R62 | Open + term | D6 shows the same mechanism on a 2D pocket |
| R63 | Open + term | The face starts at the pinned `top_z` (12) under the stock top (14). Term: the face ladder reads the stock above its top |
| R64 | Keep | No floor; a floor is a new Unsourced constant |
| R65 | Open: the cutter is wrong | `VBitEndmill::width_at_height` ignores the `tip_diameter` that `build_cutter` sets; a tipped bit cuts wider. Fix the cutter (moves toolpaths) |
| R66 | Open | One shank source: the cutter's shaft is the physical fact; one K3 fact |
| R67 | Open | Read `d_eng` per candidate depth; flat tools do not move |

### 6.2 Surface and record items

| # | Verdict | Note |
|---|---|---|
| R1 | Not judged | No wanaka generation run in this review |
| R2 | Open | Delete the words; nothing reads the target |
| R3 | Not judged | GUI |
| R4 | Open + term | Rename. A material with no force line must not count as inside (D5) |
| R4b | Not judged | Plunge rows not tested |
| R5 | Not judged | Frame cost not measured |
| R16 | Not judged | Performance |
| R17 | Open | Needs a test on the hash |
| R18 | Open | Rename |
| R19 | Not judged | Draw rule |
| R20 | Closed | No depth moves under `DrawnAsLimits` (`calculator/cut.rs:91`, `:153`) |
| R21 | Not judged | No deflection run |
| R22 | Open + term | One-value rows (Spektra) bind the table at 1 x D whenever the chip equals the point; that matches the chart's 1 x D condition |
| R23 | Not judged | Cost |
| R24 | Closed | The test doc names the half chip on purpose |
| R25 | Open | One source |
| R26 | Open | Same population as the gate |
| R27 | Open | One width per move |
| R28 | Open | Rewrite the module doc |
| R29 | Not judged | GUI |
| R29b | Keep | Sienci trades the other way (narrower and deeper); no safety defect |
| R29c | Open | Rename |
| R6 | Superseded in part | P13c covers the roughing rows; the 6 mm `k` rule remains for other axial fields |
| R7 | Open | No code builds the shank or stick-out fact |
| R8 | Not judged | Needs a diff review |
| R9 | Not judged | GUI |
| R10 | Open | Fire only on a computed width |
| R11 | Closed | The sentry pins the fallback |
| R12 | Open | Four `retired_lift_*` tests, not three |
| R13 | Open | 47 lines |
| R14 | Open | No token |
| R15 | Closed in part | Depth and width closed; the 0.25 D slot cap stays (D12) |

### 6.3 Accepted reds

The agents found each red at HEAD as listed. The reviewer adds one note:
`_litmatrix_rubbing_floor_clamp` (the Ipe row) depends on the same lookup score as D2.

## 7. What the review asks of the next session

1. D2 and D1 together: one reader of a missing or proxy hardness; a hardness span per row.
2. D7: the Project Curve depth sign in the cut spec.
3. D3 and D6: one feed for Suggest and both gates; then the engagement spread.
4. D5: a plywood force line, or a refusal of the depth.
5. D4: the species term; part B can add a power witness.

## 8. Files

Page: https://claude.ai/artifact/TuzkiQeYRcDipDief2CxHM (private; source `index.html`, built by
`build_page.py` from `page_src.html` and `claims.json`).

`PREREGISTRATION.md`, `JUDGEMENT.md`, `reference.py`, `judgement.json`, `DISAGREEMENTS.md`,
`REPORT.md`, `index.html`, `assert_claims.py`, `claims.json`, `summarize.py`, `fm1_stats.py`,
`runs/`, `matrix/`.
