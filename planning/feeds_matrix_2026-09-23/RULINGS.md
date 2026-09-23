# Feeds matrix Phase 3: the five rulings

Date: 2026-09-23. Status: RULED 2026-09-23 (see the operator rulings block). Evidence rows are in
`EVIDENCE.md`; the row label (for example 4.1-12) is the reference. No fix lands
before these rulings. The recommendations are the orchestrator's; the operator
decides.

## Operator rulings, 2026-09-23

Recorded verbatim in substance; the operator's words are in FINDINGS-style quotes.

- **R1: YES, gated.** "yes. but only once we have had some kind of sanity check that
  it makes sense. We need evidence backing up formula results, refuse if we are
  clueless (judgement made by agents not the code)." Reading: option (a), but the
  per-cell arm is a declaration that agents write from the evidence: `FormulaOnly`
  where a published figure corroborates the formula's range for that cell class,
  `Refuse` where no evidence backs it. The code reads the table; it does not judge.
  Threshold confirmed by the operator 2026-09-23 ("clueless sounds right to me"):
  a formula result outside 0.5x to 2x of the published band is CLUELESS and
  refuses. FORMULA_BACKING.md (e18d8155) is the judgement snapshot; the encoding
  waits until R4 and R5 land and the judgement is run again.
- **R2: NOT RULED.** "This is a blind spot for me. I have not seen any official docs
  on depth of cut. But I have seen people do it quite deep. tapered balls make it
  too complex for me to conceptualise." Needs a plain explanation and a smaller
  step before a ruling.
- **R3: YES.** "yes, delete. 1.5 is arbitrary. if we have data to back up real
  rules, go for it, but I think deflection is the main limiter." Delete the hint;
  keep the published depth ladder in one implementation; the deflection gate is
  the limiter the operator trusts.
- **R4: NOT RULED.** "We need a solid review on how they combine and what the
  value is!" A review package of the derate chain, stage by stage with numbers,
  comes before the ruling.
- **R5: YES.** "if they are broken that's a real f-up. yes please get those
  fixed."

## Landed under the rulings, 2026-09-23 evening

- R3: d8d8b0c7 (hint deleted), aef54c83 (one linear scale for the feed and
  the band; band at the shipped depth; 0.50 held above 3 x D with a Caution),
  dc18440e (FM3 sentry). Fact found on the way: under one continuous scale a
  depth clamp that starts at or below 2.5 x D cannot lift the shipped load
  over a ceiling the calculator's point satisfied; g_t15 and g_s2 re-derived
  on a 30 -> 24 mm crossing.
- R5: 3885c8bf..bab9a236 (stored chart text and hashes; printed Amana ball v7
  and Spektra v24 rows; reduced and unprinted rows relabelled derived/c; Onsrud
  77-100 tapered rows and the two plywood sheets; the MDF routing was a
  missing row), 827f383e (fifteen fixtures re-blessed to the printed rows),
  d884ab7a (matrix re-run: VendorBacked 328 -> 424, floor fires 333 -> 201,
  feed-cap clamps 30 -> 97).
- Consequence to rule on later (R5 follow-up): the Amana Spektra chart prints
  one value per size, so those rows carry a maximum only. On flat-end pocket
  and adaptive cells at 1/8 in, 6 mm and 1/4 in in every wood Suggest now
  shows no band, the burn gate is silent (its low side is advisory without a
  minimum, by existing policy) and modulation runs bandless. Options: accept;
  or author a derived band around the printed value with a named rule.
- R1, engine-rule part: LANDED (feat commit "Suggest refuses what the
  registry's own tool rule refuses" and the FM4 sentry). Suggest and the
  generator now refuse the same cells; Pencil and SpiralFinish carry
  Scallop's tool rule; the refusal text names the operation and the allowed
  kinds in words (rows 2-1, 2-6, 2-7, 4.4-15, 3.5-18 closed). Generation and the
  MCP operation schema moved with Suggest on Pencil and SpiralFinish: their
  generators now refuse a non-ball tool, as Scallop's always did. The
  evidence-backed refusals wait for FORMULA_BACKING_v2 (a fresh judgement on
  the post-R5 matrix) and the operator's look at its count.
- R2, R4: no change.

## The three findings that change the frame

Read these before the rulings. They were not in PLAN.md.

1. **The vendor-backed cells are not better sourced than the formula-only cells.**
   The Amana ball-nose rows at 3.175 mm and 6 mm sit at about 0.16 of the chart
   they cite; the manifest calls them "derived (reduced) for 3D-finish use" but the
   rows carry `row_kind exact, evidence_grade a` (4.1-12). The Amana spiral-plunge
   rows carry bands and depth ranges that are not on the fetched chart (5.2-19).
   The tapered 3.175 mm rows have no printed row in their cited PDF (4.1-15). The
   four bull rows cite a page with no bull row (3.3-17). The reduced ball band is
   the direct cause of the vendor-backed floor fires (4.1-12). This is the
   wanaka200 "3x to 7x low" reading, seen from the LUT side.
2. **The rubbing floor has no source.** None of the three citations in the code
   states 0.025 mm/tooth (4.1-1, 4.1-2, 4.1-3, 4.1-4). A wood vendor publishes
   recommended chiploads below it (4.1-26). The floor fires on 333 of 960 cells,
   and in 247 of them the recipe's own target clears the floor; the engine's own
   safety factor trips it (4.1-5). Where the whole band sits below the floor, the
   floor moves a mid-band recipe to the band maximum (4.1-6).
3. **The finish depth verdicts on the simulated fixture are not a gouge.** The depth
   gate's peak is the tallest column one stamp removes on unroughed stock, not the
   wall height and not the z-step (5.1-1). The instrument ran every finish on
   fresh stock. The verdicts measure roughing removal with a finish recipe.
   Separately, Suggest ships finish depths above the engine's own finish cap
   because the clamp acts on roughing roles only (5.1-2, 3.1-19, 3.2-10).

## R1. Does a formula-only recommendation ship? (closes H7)

Options in PLAN section 7: (a) ship with visible `FormulaOnly` provenance and a
Caution; (b) refuse unless the tool family has a vendor row in that operation
family; (c) refuse unless the exact cell has a row.

Evidence:
- The formula's k0 and p fit the community Shapeoko A-to-Z soft-wood table (6-1,
  6-2); q does not reproduce that table's hard-wood column (6-4); the fit record is
  in a gitignored file (6-1). k0 has no citation (3.1-24).
- Against vendor charts the formula is 0.2 to 0.5 of every band (3.1-1, 3.1-2,
  3.2-1, 3.3-1, 3.5-1, 6-6, 6-7); against the machine maker's measured chart it is
  2 to 2.5 above (6-3, 6-5). MDF and plywood are inverted: every chart puts MDF
  above wood, the engine puts it below (3.1-3, 5.2-10).
- After the engine's own derates the formula lands on the floor by construction on
  small hardwood cells (3.1-17, 3.3-9, 4.1-17).
- Option (b) refuses 584 of 960 cells today, including every ball and tapered-ball
  rough, every V-bit clearing pass and every drill cell (no drill row exists).
- Recipes ship next to the engine's own Critical "wrong tool" flag on
  EndMill/Pencil (3.1-22, 4.4-15) and a tapered ball on Chamfer fires nothing
  (3.5-18). SpiralFinish refuses a flat tool through its feeds family alone while
  RadialFinish accepts the same tool (2-1). The refusal text names the wrong
  operation and leaks Debug output (2-6). Bull on Scallop disagrees at three sites
  (2-7).

Recommendation: **(a)**, with three conditions that make it honest.
1. The `FormulaOnly` source string names the fit and the uncited k0 (Phase 0 did
   this). The GUI, MCP and CLI show the arm on every recipe.
2. Refuse where the engine's own tool rule is Critical: align
   `validate_tool_for_operation` with the registry `tool_constraints`, so
   EndMill/Pencil and a ball-tipped tool on Chamfer refuse instead of shipping a
   recipe under a Critical flag. That refusal is backed by the engine's own rule,
   which is the standard the operator set.
3. Fix the refusal routing and wording: SpiralFinish declares its real family or
   gains the scallop target; the error carries the `OperationType`; the V-bit arm
   prints a sentence.
Why not (b) or (c): the vendor-backed cells are not better sourced today (finding
1). Refusing the formula while shipping a reduced ball band as "backed" would
refuse the more honest number. Revisit (b) after R5 lands and "VendorBacked" means
what it says.

## R2. The depth criterion on radial-load finishes (PLAN 6a)

Evidence:
- No vendor states an axial finish cap as a fraction of D for wood (5.1-3,
  5.1-12). The published finish rule is radial: about 20 % of the roughing tool
  diameter (5.1-5). PreciseBits gives 1x tip per pass, 2x maximum, for a tapered
  ball (5.1-8, 3.5-8). DAPRA's 10 % of ball diameter is metal (5.1-9).
- The cap sets the depth on 100 % of 54 roughing recipes and sits at 0.2 of the
  published 1 x D (5.1-13, 5.1-4).
- Instrument defects that need no ruling: the pre-sim clamp uses the tip and the
  gate uses the shank on a tapered tool (5.1-7, 3.5-15); Profile reads Exceeds at
  peak equal to cap through an f32 difference (5.1-6, 5.2-16); a depth Exceeds is
  absent from the diagnostics list (5.1-15); an Unmodeled gate is reported under a
  `*.within` id (5.1-14).

Recommendation: split the criterion by role.
- Finish and SemiFinish roles: judge the RADIAL engagement against the one
  published radial rule (Onsrud, about 20 % of the roughing tool diameter as the
  remaining allowance), and report the axial peak as a measured reading with no
  bound. A wall-following pass engages the wall by construction; an axial cap on it
  is the wrong quantity.
- Roughing and Adaptive roles: keep factor x D, labelled the rule of thumb it is,
  until R3's published de-rate replaces the depth ceiling with the vendor's own 1
  to 3 x D ladder.
- Close the four instrument defects above under this ruling; each is an
  engine-versus-engine contradiction and needs no external source.
- Re-run the simulation subset on ROUGHED stock before any finish verdict is
  cited again.

## R3. The 1.5 x D static hint versus the Onsrud depth rule (PLAN 6b)

Evidence:
- The hint has no source and fired on 0 of 960 cells (5.2-1). Its only lineage is
  the literature row that CREDITS.md already names as misattributed (5.2-1).
- The published rule is a chip-load multiplier keyed to depth (1 x D full, 2 x D
  minus 25 %, 3 x D minus 50 %), identical in Onsrud, Freud and Techno; Amana
  reduces the feed, which is the same rule at fixed RPM (5.2-2, 5.2-17). The engine
  applies it in the feed and agrees at the printed points (5.2-4, 5.2-8 to 5.2-13).
- On the Shapeoko presets the engine applies the correct 0.75 at 2 x D and then
  trips its own hint (5.2-3).
- The engine holds two implementations of the rule that disagree between the
  printed points and above 3 x D (5.2-6, 5.2-5); the chipload band the UI shows
  never carries the rule (5.2-7).

Recommendation: delete the hint. Keep ONE implementation of the published rule,
cited to the three vendors, applied to the feed and to the shown band from the same
function. Between printed points use the step to the next printed point: Freud
prints "at least", and no vendor prints an in-between value, so the step is the
conservative published reading. Above 3 x D hold 0.50 and emit a Caution "beyond the
published table" instead of the uncited 0.45. Replace the hint with an information
row that names the de-rate applied and its vendors.

## R4. De-rate vocabulary: which published rules may scale a figure

Evidence:
- One published rule scales a figure: the depth ladder above (5.2-17, 5.2-18).
- Scales with no source: the 0.75 safety factor and the L/D factors 0.88 and 0.75 at
  4 x and 6 x D (5.2-14); the rigidity factors (5.1-12, 3.3-14); the drill
  multiplier 2.5 and the plunge envelope (6-8, 4.3-14, 3.1-16); the 0.45 tier
  (5.2-5).
- The rubbing floor: unsourced (4.1-1 to 4.1-4), contradicted by a vendor chart
  (4.1-26), tripped by the engine's own safety factor in 247 cells (4.1-5), and it
  lifts a mid-band recipe to the band maximum (4.1-6).
- The H1 mystery factor x0.704 is the depth ladder applied at the tapered tool's
  engaged cone diameter (6-10); the same row is scaled from two diameters by
  Suggest and by the gate (6-11, 3.5-7).

Recommendation: three classes, and the rule that an unpublished scale flags and does
not scale.
1. May scale: the vendor's own row (band, printed ap and ae), and the depth ladder
   (R3). Nothing else.
2. Limits, not scales: machine facts (max feed, max shank, RPM range, power). They
   clamp and say so.
3. Flag, do not scale: the safety factor becomes a visible operator margin with its
   provenance "repo margin, unsourced" and a default the operator sees; the L/D
   factors become a "long stickout" Caution; the rubbing floor becomes a Caution
   with no clamp and no band-max override; the drill multiplier and envelope stay
   named unsourced in the drill provenance string and gain a Caution.
The one-diameter rule: a tapered tool is scaled from one diameter, the engaged cone
diameter at the shipped depth, in Suggest and in the gate.

## R5. Coverage gaps to fill from the reference check

Evidence:
- Onsrud 77-100 (tapered ball) is printed in four wood sheets and is not in the LUT
  (3.5-1 to 3.5-4, 4.1-14, 4.1-16). No Onsrud plywood PDF is in the manifest
  (5.2-11, 3.3-4).
- Rows that do not match their cited chart: Amana ball reduced about x0.16 (4.1-12,
  4.1-8); Amana spiral-plunge bands and depth ranges (5.2-19); tapered 3.175 rows
  (4.1-15); the four bull rows (3.3-17, 3.3-16).
- MDF inverted against every chart (3.1-3, 5.2-10).
- V-bit: the anchor row publishes RPM only (4.4-1, 4.4-5); no MDF or plywood V-bit
  row exists (4.4-3, 4.4-4, 4.1-25); one tool gets two RPMs 2.2x apart by pass
  (4.4-7); the Adaptive depth runs past the 60 degree cone (3.4-8).
- Families with no published row anywhere: bull finishing (3.3-16), ProjectCurve on
  bull or V (4.4-8, 4.4-6), tapered ball on trace, chamfer and drill (3.5-14, 3.5-17,
  3.5-18), ball drilling (3.2-18).
- The verifier fetches were not stable; a URL is not a stable source for these
  vendors (EVIDENCE.md, verification note).

Recommendation:
1. Store a copy and a hash of every PDF in the manifest; re-transcribe from the
   stored copy.
2. Add: Onsrud 77-100 rows for hard wood, soft wood, MDF and hard plywood at 1/8
   and 1/4 in; the Onsrud Hard and Soft Plywood sheets; the Amana ball v7 rows as
   printed; the Amana spiral-plunge v24 rows as printed, with the chart's own
   material columns (Wood/Plywood, MDF/Laminate).
3. Re-grade: the reduced ball rows become `derived, grade c` with the reduction
   named, or are replaced by the printed rows; the four bull rows become derived
   from the named flat series or are deleted.
4. Map MDF to the printed MDF column, not to a Janka extrapolation.
5. Stay formula-only, with provenance, for bull finishing, ProjectCurve on bull or
   V, tapered ball on trace, chamfer and drill, ball drilling, and V-bit clearing.
6. One RPM per V-bit from its vendor anchor on every pass; a V-bit clearing depth
   stops at the cone height.

## Defects that need no ruling

Each is an engine-versus-engine contradiction. Phase 4 may close them under the
ruling named, without a new source.

| Row | Defect | Under |
|---|---|---|
| 2-1 | SpiralFinish refuses through its feeds family alone | R1 |
| 2-6 | Refusal text names the family, not the operation; Debug output leaks | R1 |
| 2-7 | Bull on Scallop: three sites disagree | R1 |
| 4.4-15, 3.1-22 | EndMill/Pencil ships a recipe under a Critical flag | R1 |
| 5.1-6, 5.2-16 | Profile reads Exceeds at peak equal to cap | R2 |
| 5.1-7, 3.5-15 | Tapered tool: clamp uses tip, gate uses shank | R2 |
| 5.1-15 | Depth Exceeds is absent from the diagnostics list | R2 |
| 5.1-14 | Unmodeled gate reported under a `*.within` id | R2 |
| 5.1-2 | Finish depths ship above the finish cap; clamp is roughing-only | R2 |
| 5.2-6, 5.2-7 | Two depth de-rate functions; the shown band carries neither | R3 |
| 5.2-18 | The "verbatim" quote in geometry.rs matches no ap_rule string | R3 |
| 4.1-1 to 4.1-3 | The floor's three citations do not state the floor | R4 |
| 3.4-13 | One V-bit, one role, two chiploads 4.3x apart by hint presence | R5 |
| 3.4-8 | V-bit Adaptive depth past the cone; the flute guard reads cutting_length | R5 |
| 4.1-12 | Ball rows marked exact/grade a are reduced x0.16 | R5 |

## What Phase 4 waits for

The five decisions above. Each fix will name the cells it moves and re-bless the
FM0 and FM1 sentries with the ruling as the cause. No number moves before then.
