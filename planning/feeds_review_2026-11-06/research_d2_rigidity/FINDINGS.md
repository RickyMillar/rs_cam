# D2: is rigidity in the hobby chip or in the hobby depth?

Author: D2 research agent, 2026-11-06. HEAD `cb40af153`. Read-only. No cargo run.
`build_pairs.py` writes `pairs.csv` and `stats.json`. `python3 -I assert_claims.py`
re-derives every number below from the stored files (96 claims).

## 1. Verdict: undecided

Outcome (a) fails: the ratio is not steady. Outcome (b) fails: the chips are not alike.

- Three of four hobby sources print chips well below the industrial chips:
  source medians 0.235 (Shapeoko 3), 0.274 (IDC), 0.645 (Sienci). This leans to (a).
- The Nomad 883 prints chips at the industrial level (median 1.09) at a very
  shallow depth (0.112 D to 0.272 D). This is (b).
- The three low sources differ by a factor of 2.74 (0.645 / 0.235). One stored
  derate cannot hold all three.
- Sienci (chip 1.0, depth 0.463) and Shapeoko 3 (chip 0.96, depth 0.25) keep
  the chip and cut the depth for the harder wood. The Nomad halves both
  (chip 0.51, depth 0.467).

Thus the place of rigidity is a choice of the chart author, not one class term.

Constraint independent of the statistics: the engine force budget
(`HOBBY_SLOT_POINTS`, CREDITS.md:937-958) reads Sienci lines 79 and 170 and
Shapeoko 3 lines 63, 64 and 66. Those rows already carry the low hobby chips
(Sienci 0.114 mm, Shapeoko 3 pine 0.0302 mm). A chip derate fitted from them
uses the same evidence twice. The budget solves the depth from the force. A
derated chip thus gives a deeper budget depth. The chart's (chip, depth) pair
comes back only if the engine reads both values from the same chart.

## 2. Ratio statistics (hobby chip / industrial chip mid)

60 hobby rows, 214 pairs, 115 primary pairs (Sienci "Full Speed - Roughing"
tier plus all rows of the other charts, single-flute rows out). The primary
set uses 31 hobby rows and 55 industrial rows. 3 hobby rows have no partner
(Sienci 1/16 in hardwood, lines 129, 153, 177).

Primary: median 0.368, quartiles 0.244 to 0.749, range 0.0947 to 2.34,
fraction below 1 = 0.913.

| Group | n | median | quartiles |
|---|---|---|---|
| Sienci | 46 | 0.645 | 0.521 to 0.782 |
| Shapeoko 3 | 9 | 0.235 | 0.12 to 0.261 |
| Nomad 883 | 14 | 1.09 | 0.801 to 1.83 |
| IDC | 46 | 0.274 | 0.185 to 0.325 |
| vs Amana | 64 | 0.578 | |
| vs Freud | 22 | 0.312 | |
| vs Onsrud | 29 | 0.284 | |
| softwood | 44 | 0.292 | |
| hardwood | 36 | 0.333 | |
| MDF | 19 | 0.625 | |
| plywood | 16 | 0.624 | |

Envelope (one count per hobby row and wood, against the band of all matched
industrial rows): 41 below, 11 inside, 0 above. Sienci 14 below, 8 inside;
Shapeoko 3 5 below; Nomad 2 below, 3 inside; IDC 20 below.

Sienci tiers (46 pairs each): reduced 0.387, regular 0.516, full 0.645.

Decision rule: IDC lines 64-65 print "back the feeds off by about 20%". A
ratio in 0.8 to 1.25 is "alike". (a) needs the pooled median outside the band,
every source median below 1 and both quartiles below 1. The Nomad median is
inside the band, so (a) fails. (b) needs the pooled median (0.368) inside the
band, so (b) fails.

## 3. Depth comparison

The industrial chips are at 1 x D (Amana spektra line 3, Onsrud sheet line 11,
Freud line 57).

| Hobby row | depth / D | chip mm |
|---|---|---|
| Sienci 1/4 in flat, soft, slot (line 79) | 0.85 | 0.114 |
| Sienci 1/4 in flat, hard, slot (line 170) | 0.394 | 0.114 |
| Sienci 1/8 in flat, soft, slot (line 83) | 1.73 | 0.0794 |
| Sienci 1/8 in flat, hard, slot (line 174) | 0.819 | 0.0794 |
| Shapeoko 3 Pine, slot (line 63) | 1.6 | 0.0302 |
| Shapeoko 3 Mahogany (line 64) | 0.4 | 0.029 |
| Shapeoko 3 Plywood (line 65) | 1.0 | 0.0447 |
| Shapeoko 3 MDF (line 66) | 1.2 | 0.0398 |
| Nomad Pine (line 61) | 0.24 | 0.203 |
| Nomad Mahogany (line 62) | 0.112 | 0.104 |
| IDC 1/4 in down cut, 40 % stepover (line 219) | 1.0 | 0.0468 |

The Shapeoko 3 cuts pine and MDF deeper than 1 x D with a chip near one
quarter of the industrial chip. Across 29 hobby rows (facing out), the rank
correlation of depth / D with the chip ratio is -0.406: weak, and mostly
between sources.

## 4. Prose witnesses

- IDC lines 59-65: a machine with flex must back the feed off by about 20 %.
- ToolsToday video lines 79-81: chart 260 ipm, "tabletop CNC" 120 ipm, ratio 0.462.
- ToolsToday guide lines 176-179: chart 400 ipm, desktop 120 ipm, ratio 0.3.

## 5. Limits

- Four hobby charts. Two are from one maker (Carbide 3D).
- The Sienci flute count is assumed (2, CREDITS.md:952). At 3 flutes each
  Sienci chip is 0.667 of the value here.
- The Carbide 3D flute counts come from a forum post (G9 post lines 5 and 7).
- IDC prints one wood column; its pairs repeat under softwood and hardwood.
- IDC imperial rpm is used: line 245 prints 19000, metric line 462 prints 15000.
- The industrial side is not one level: the Onsrud compression rows sit well
  above the Amana Spektra rows at one size (see pairs.csv). The pairs are not
  independent.
- Hobby rpm comes from router dials; low-rpm rows (Nomad) move the chip.
- Freud 1/2 in rows have no hobby partner. Freud names no machine class.

## 6. A hobby chart states a feed, not a chip (lead analysis, 2026-11-06)

The primary set has 115 pairs. In it the four hobby sources print similar feeds: the source
median feeds go from 1778 mm/min (IDC) to 2720 mm/min (Sienci), a ratio of 1.53. Their
spindle speed times flute count (rpm x Z) goes from 8000 to 63000, a span of 7.88.

The log of the hobby/industrial chip ratio correlates -0.714 with the log of rpm x Z, and 0.396
with the log of the feed. So the hobby chip falls where the chart's spindle runs fast: the
chart fixes a feed and a depth at its router speed, and the chip is only feed / (rpm x Z).
A hobby chart is feed and depth evidence, not chip evidence.

Operator ruling (2026-11-06, P16 item 8): a hobby feed chart (Sienci, Carbide 3D Shapeoko
and Nomad, IDC, and any chip derived as feed / (rpm x Z) from a hobby preset) never enters a
chip band. The force budget may keep reading those rows, because it reads their real feed,
rpm and depth.

(The lead's first count of the rpm x Z span, 9000 to 56850, differs from the value above,
which is over the 115 primary pairs of `pairs.csv`.)
