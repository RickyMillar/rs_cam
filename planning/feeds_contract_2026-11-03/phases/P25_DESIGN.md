# P25 design: a tool is what it is; a number is sourced or calculated, else there is none

Design and measurement only, 2026-11-11. No product code. No cargo run was made for this note:
every count below comes from the row files and the FM1 fixture CSV. Items that need a run say so.

## 0. The direction

The operator, 2026-11-11: "instead of 'Standard' and 'special' can we not just treat tools as
what they are? just have categories with generic functions that live between them? ... We
either source an exact number or we calculate one, and if there isn't a match, or the math, we
don't do it." And: "I think we just have 'straight flute' and 'Spiral flute' categories? Would
this also play into upcut and downcut bits? but keep the defaults generic for those."

Words used in this note:

- **Row**: one printed cell of a vendor chart (a tool size, a material, a chip per tooth).
- **Category**: what a tool is, in the terms a chart is printed for.
- **Sourced**: a row printed for the tool's category, material, operation and size.
- **Calculated**: a value made from printed rows by a rule that has a name.
- **Score**: today's point system. It adds points per row and the highest total wins.

## 1. Today: every rule that decides which row or value ships

The class column uses the operator's terms. "Preference" means: a rule that picks between
candidates by points or by rank, not by a source or a named calculation.

| # | Rule | File and function | What it does | Class |
|---|---|---|---|---|
| 1 | Operation match | `vendor_lookup.rs` `passes_must_match` | A row must be printed for the operation, or a transfer rule (31, 33) carries it | sourced |
| 2 | Material class | `passes_must_match`, `material_category` | Solid wood, plywood, MDF-class, plastic, aluminium never mix | sourced |
| 3 | Tool family, with borrowing | `tool_family_compatible` | A bull nose may read a flat row, a flat tool a bull row, a tapered ball a ball row | preference |
| 4 | Straight rows only for a straight tool (P24a) | `series.rs` `row_admits_query` | The one exclusive series | sourced |
| 5 | Size sanity gate | `passes_must_match` (`SCALE_CLAMP_LO/HI`) | Row size within 0.1x to 10x of the tool | preference |
| 6 | Score: tool family | `tool_family_score` | 220 own family; 120, 110, 100 for a borrowed family | preference |
| 7 | Score: row kind | `ObservationKind::score` | exact 120, derived 70, fallback 30 | preference |
| 8 | Score: evidence grade | `EvidenceGrade::score` | a 60, b 30, c 10 | preference |
| 9 | Score: flute count | `score_observation` | same 80, off by one 30, else -20 | preference |
| 10 | Score: size nearness | `score_observation` | 0 to 200 by distance in size | preference |
| 11 | Score: hardness nearness | `score_observation` | 0 to 80 | preference |
| 12 | Score: series key | `score_observation` | +50 for a row of the tool's series | preference |
| 13 | Score: pass role | `score_observation` | +45 same role, -25 other role | preference |
| 14 | Score: exact material | `score_observation` | +100 for the same material family | preference |
| 15 | Tie: nearer size | `beats` | On an equal score the nearer size wins | preference |
| 16 | Tie: printed before transferred (A3) | `beats` | A row printed for the operation beats a transferred row | sourced before calculated |
| 17 | Tie: lower chip (Q3) | `beats` | The lower printed chip wins | preference (safe side) |
| 18 | Tie: row id | `beats` | Makes the result repeatable | preference |
| 19 | Row series | `series.rs` `row_series`, `SERIES_RULES` | Names a row straight, compression, down-cut, ZrN, O-flute, single flute, rougher, or standard | classification |
| 20 | Series rank | `prefer_series`, `series_rank` | Own series, then standard, then other specialty, but only between rows of one material and size | preference |
| 21 | Band members | `with_band`, `cheap_exclusion`, `size_admits` | The printed rows of several charts for one tool, size, material | sourced (and form A) |
| 22 | Band: lowest ships (P16) | `BandRule::Lowest` | The lowest member ships | preference (safe side) |
| 23 | Band: exact size first (P23c) | `BandRule::ExactSize` | A row at the tool's size beats a lower interpolated member | sourced before calculated |
| 24 | Band: winner ships | `BandRule::WinnerShips` | The score winner ships when no member is below it (no rise) | preference |
| 25 | Band: no member | `BandRule::NoMember` | The score winner ships alone | preference |
| 26 | Hobby charts stay out | `BandExclusion::Hobby` | A hobby feed chart is never a member | sourced |
| 27 | Weak rows last | `is_weak_evidence` | Grade c or fallback rows join only with no grade a or b member | preference |
| 28 | Cross-family note (P18) | `VendorBand::cross_family` | Names a borrowed tool family on the card | report |
| 29 | Size law form A | `extrapolation/size.rs` | Between two printed sizes of one chart series (log-log) | calculated, named |
| 30 | Size law form B | `size.rs` | One printed step past the series, by the series' own slope | calculated, named |
| 31 | Size law form C | `size.rs` (`CHIPLOAD_DIAMETER_EXPONENT` 0.61) | From the nearest row within 0.5x to 2x, by the fitted law | calculated, named |
| 32 | Micro and tip floor | `support.rs` `micro_extrapolation_refusal`, `tapered_tip_floor_refusal` | Under 1.5 mm only a near row; no tapered tip under 0.5 mm | refusal |
| 33 | Hardness transfer (G2) | `extrapolation/hardness.rs` | Another species or family of one material class, by the Janka law, with a printed cap | calculated, named |
| 34 | Family rules (G3) | `extrapolation/family.rs` `FAMILY_RULES` | A chart that names no operation serves the 3D families | calculated, named, with a witness |
| 35 | Routes | `vendor_normalize.rs` `lut_query_for` | A flat finish reads its roughing row; a chart with no operation is (Pocket, Roughing) | calculated, named |
| 36 | Drill rule (G6) | `extrapolation/drill.rs` | A plunge reads the side row x 1/Z | calculated, named |
| 37 | Plunge and ramp (G10) | `extrapolation/plunge.rs`, `ramp.rs` | Fractions of the shipped feed | calculated, named |
| 38 | V-bit angle | `find_best_vbit_row` (tolerance 20 degrees, bonus 200) | The nearest printed angle wins by points | preference |
| 39 | Formula with no row | `support.rs` `formula_backing`, `FeedsSupport::FormulaOnly` | A repo formula `k0 D^p (1/H)^q` answers where no row exists | calculated, no source |
| 40 | RPM with no printed RPM | `calculator/speed.rs` | The surface-speed constant gives the RPM | calculated (out of scope here) |
| 41 | Extrapolated flag | `is_extrapolated_for_ratios` | Marks a scale beyond 40 % | report |

Count: 41 rules. 20 are preferences.

## 2. The proposed model

### 2.1 The category

A tool has one category. A row has one category. A tool reads only rows of its own category.

| Field | Values | From the tool | From the row |
|---|---|---|---|
| Shape | flat end, ball nose, bull nose, tapered ball, V-bit, facing bit | `ToolConfig::tool_type` (today's `lut_family`) | `VendorObservation::tool_family` |
| Flute form | straight, spiral, O-flute | `BitCutDirection` (`Straight`, else spiral) | by rule from the subfamily (today's `row_series`, renamed) |
| Spiral kind | plain, down-cut, compression | `BitCutDirection` | by rule from the subfamily |
| Flute count | 1, 2, 3, ... | `flute_count` | `flute_count` |

Rules for the fields:

- The flute form applies to a flat end, a bull nose and a ball nose. A V-bit and a facing bit
  have no spiral: their category is the shape alone (a V-bit adds its angle, as today).
- **Plain spiral is the default.** `BitCutDirection::UpCut` means "plain spiral" (an up-cut is
  the ordinary spiral). No new "not stated" value is needed. The GUI label can read "Spiral".
- A row with no stated geometry is a plain spiral row (Spektra, Freud solid carbide, the
  Onsrud 60-000 group, Garr). 210 of the 350 flat-end rows state nothing or state "upcut".
- Coating is not a category field (see 3.3).

One definition: `ToolSeries` becomes `FluteForm` + `SpiralKind` in `feeds/series.rs`
(renamed `feeds/category.rs`). `row_series` becomes `row_category`. `SERIES_RULES` stays as the
table for rows whose subfamily does not state the geometry. `ZrnCarving`, `SingleFlute` and
`Rougher` are deleted. `LookupQuery::tool_subfamily` (a string) becomes a typed category.

### 2.2 The resolution

For one tool, material, operation and size:

1. **Take the rows of the tool's category**, in the material's class. A down-cut or
   compression tool takes the rows of its kind and the plain spiral rows.
2. **Sourced.** A row printed for this material and operation at the tool's size ships. For a
   down-cut or compression tool, a row of its own kind at the size ships before a plain row.
3. **Calculated.** If nothing is sourced, try the named rules in this fixed order. Stop at the
   first that gives a value: one transfer at the printed size (hardness, family, route or
   drill rule); the size law form A; form B; form C; then a size law joined with one transfer.
4. **Several charts at the same step**: the lowest chip ships, and the card names every chart
   (today's band, rule 22). A hobby chart is never a source (rule 26). Grade c rows count only
   when no grade a or b row is at that step (rule 27).
5. **Nothing**: no number. Suggest names the gap ("no chart prints a plain spiral flat end
   mill in plywood at 18 mm; nearest printed size 12.7 mm"). Generation and export still work.

### 2.3 What "calculated" admits

Kept, each with its name on the card: size law A, B and C (29-31); hardness transfer (33);
family rules (34); routes (35); drill rule (36); plunge and ramp (37).

Dropped: the formula with no row (39); tool-family borrowing (3), unless ruled as a named
rule (3.5); the fallback rows as a source (3.7).

### 2.4 What happens to the score

The score is deleted. Its nine terms go to these places:

| Term | Goes to |
|---|---|
| Tool family (6), series key (12) | The category: a hard match |
| Flute count (9) | The category field (3.2) |
| Size nearness (10), tie on size (15) | The size law order: exact, A, B, C |
| Hardness nearness (11), exact material (14) | Sourced in the family, else the hardness transfer |
| Pass role (13) | The operation match and the routes |
| Row kind (7) | Exact = sourced. Derived = calculated, and the row names its rule. Fallback = not a source |
| Evidence grade (8) | Only rule 27 remains |

Also deleted: the size sanity gate (5), the series rank (20), "winner ships" (24), "no member"
(25) and the extrapolated flag (41: "calculated by" says it). The V-bit points (38) change:
the angle becomes a key, exact or inside a named tolerance.

**Rules that go away: 16** (5-15, 20, 24, 25, 39, 41). Two more (3, 4) are absorbed into the
category. 23 stay.

### 2.5 What the card and MCP print

One of three, on every surface:

- `sourced: <chart>, <row>` (and the other charts at the same size).
- `calculated: <rule name> from <rows>` (the rule, its range, its residual: today's claim text).
- `no number: <the gap>`.

`FeedsSupport` keeps its claims and is renamed to match: `VendorBacked` -> `Sourced`;
`Extrapolated`, `FamilyTransferred`, `DrillTransferred` -> `Calculated { rules }`;
`FormulaOnly` is deleted; `Refuse` -> `NoNumber { gap }`.

## 3. The hard cases

| # | Case | Proposed answer | Alternative |
|---|---|---|---|
| 3.1 | A spiral tool at a size that no spiral chart prints (the 18 mm bit: Spektra stops at 12.7 mm) | Keep the size law, forms A, B and C, as named calculations. The card says "calculated: size law from Spektra 1/2 in". | Only form A (between two printed sizes). Then the 18 mm plain spiral tool has no number, and 154 FM1 cells lose theirs. |
| 3.2 | Flute count; single-flute and O-flute tools | Flute count is a category field. O-flute is a flute form (13 rows, none for wood). A single-flute spiral is a spiral with 1 flute (3 rows); no category of its own. | Flute count stays a preference. 59 FM1 tapered-ball cells ship a row of another flute count today; under the proposal they need a named rule ("the chart prints one flute count for this series") or lose the number. |
| 3.3 | ZrN-coated carving rows (21 rows; 10 for wood) | A coating is not a flute form. A ZrN row joins the category its geometry gives. The lowest chip ships, so a value can only stay or fall. | A `coating` field on the tool; a ZrN row serves only a tool that states ZrN (this is the 2026-10-31 ruling in new words). |
| 3.4 | The 6 mm wanaka tool reads the Onsrud 60-200 row | 60-200 is a down-cut series. A plain spiral tool does not read it. The tool reads the Spektra 6 mm row (0.127). | State the tool as down-cut, if it is one. Or keep ruling P2 (b) of 2026-11-08 as one written exception. |
| 3.5 | A bull nose reads flat-end rows (46 FM1 cells) | A named rule with its condition, if the operator accepts one ("a bull nose reads the flat row of its diameter"). No source states it, so the default is no number. | Keep the borrowing as it is, named on the card (today's P18 note). |
| 3.6 | Chipbreaker roughers (2 rows), facing bits (9 rows) | Roughers: both rows are Whiteside fallback rows; see 3.7. A facing bit is a shape of its own, as today. | A "chipbreaker" flute form, when a printed chart arrives. |
| 3.7 | The 13 Whiteside fallback rows (from a tool library, not a chart) | They leave the lookup and stay as evidence, as the industrial rows did in P13. No FM1 cell ships one today. | Keep them as a last source, named "tool-library value". |
| 3.8 | V-bits, tapered balls, ball noses | No flute form field. The category is the shape (a V-bit with its angle; a tapered ball at its tip). | A form field for ball noses, when a chart prints a straight ball. |
| 3.9 | A tool that states no form | Plain spiral. This is today's default (`up_cut`) under a clearer name. | A separate "not stated" value that reads the plain rows and says so on the card. |
| 3.10 | The formula with no row (28 FM1 cells: ball nose 3D finishes in solid wood, V-bit 2D ops in softwood) | No number. The formula has no source. | Keep it as "calculated: repo formula", named as unsourced. |

## 4. Measurement

### 4.1 Rows per category (567 rows; from the row files)

| Shape | Form and kind | Rows | Of these, wood |
|---|---|---|---|
| Flat end | plain spiral | 210 | 185 |
| Flat end | straight | 54 | 54 |
| Flat end | compression | 39 | 38 |
| Flat end | down-cut | 17 | 16 |
| Flat end | O-flute | 13 | 0 |
| Flat end | ZrN (today's series) | 12 | 4 |
| Flat end | single flute | 3 | 0 |
| Flat end | rougher | 2 | 2 |
| Ball nose | plain 41, ZrN 8 | 49 | 45 |
| Bull nose | plain 7, ZrN 1 | 8 | 7 |
| Tapered ball | - | 74 | 72 |
| V-bit | - | 77 | 74 |
| Facing bit | - | 9 | 9 |

Row kinds: exact 371, derived 183, fallback 13. Grades: a 251, b 256, c 60.

### 4.2 FM1 (1000 cells; all tools are 2-flute plain spiral), by reading the fixture

Today: 524 cells ship a value, 476 refuse.

| Group | Cells | Under the model | Needs |
|---|---|---|---|
| Refused today | 476 | Stay "no number" (the gap text changes) | - |
| A plain tool ships a down-cut row (12) or a compression row (20) | 32 | Changes row, or no number | a run |
| A bull nose ships a flat-end row (40 plain, 6 compression) | 46 | Ruling 3.5: named rule, or no number | ruling, run |
| A tapered ball ships a row of another flute count | 59 | Ruling 3.2: named rule, or no number | ruling |
| Formula with no row | 28 | Ruling 3.10: no number | ruling |
| Other cells | 359 | Same row expected | a run to confirm |

Inside the 359: 142 cells are form C size-law values today. They keep their value only under
the proposed answer of 3.1 (the 12 form B cells are in the group of 32). 47 cells ship a
derived grade c row; their value stays and their label becomes "calculated". No cell gains a value by reading; the run checks.

### 4.3 Wanaka (18 toolpaths, from the acceptance table)

| Toolpaths | Tool | Today | Under the model |
|---|---|---|---|
| idx 0, 1, 11, 12, 14 | 18 mm, plain | Spektra 1/2 in x size law, 0.1791 | Same under 3.1. No number if only form A is kept. As a straight tool: 0.2204 (P24a). |
| idx 2, 15 | 6 mm, plain | 0.127; the table names Onsrud 60-200 | 0.127 from the Spektra 6 mm row. The row changes, the value does not (needs a run). |
| idx 5, 17 | 6 mm, plain | 0.14919 from Onsrud 60-200 (form B) | Likely 0.127 from the Spektra row through the flat-finish route: about -15 % (needs a run). |
| idx 3, 4 | 6 mm drill cycles | Spektra side row x 1/Z | Same |
| idx 6, 7, 8 | 2 mm | Spektra, form A | Same |
| idx 9, 10, 13 | V-bits | printed angle rows | Same |
| idx 16 | 2 mm tapered ball | SpeTool 2.0 mm, exact | Same |

### 4.4 The run (waits for the lane)

One probe test, no product change: a prototype of 2.2 written in the test file over
`EMBEDDED_LUT`, run on every FM1 cell and on the wanaka tools. It prints, per cell: today's
row and chip, the model's row and chip, and the class (same, row changed, value changed, lost,
gained). It runs once per ruling option of 3.1, 3.2, 3.3 and 3.5, so each ruling has its count.

## 5. Phase plan

| Step | What | Sentry | Predicted pin moves | Ruling it needs |
|---|---|---|---|---|
| P25-0 | The probe of 4.4; the counts go into this note | none (a measurement) | none | the go on the lane |
| P25-1 | The category type; `row_category`; `ToolConfig::category`. No behaviour change | every row has one category; the 4.1 counts | none (FM1 identical) | 3.2 flute form list, 3.9 |
| P25-2 | Every category is exclusive (the P24a rule for all). A down-cut or compression tool reads its kind, then plain | a plain tool never ships a row of another kind; a kind tool ships its own row at its size | FM1: up to 32 cells; wanaka idx 5, 17; `a_generic_tool_reads_the_standard_row_first_g_series` rewritten | 3.3, 3.4 |
| P25-3 | The ordered resolution replaces the score; delete rules 5-15, 20, 24, 25 | each FM1 cell names "sourced", "calculated by" or "no number"; no cell ships a value that 2.2 does not give | FM1 columns `band_rule`, `band_score_winner`; `lookup_parity`; `lut_resolver_census_a6`; q3 and p16 sentries | 3.1 |
| P25-4 | Flute count and tool-family borrowing by their rulings | a named rule or no number for each of the 59 and 46 cells | FM1: up to 105 cells | 3.2, 3.5 |
| P25-5 | Fallback rows out of the lookup; the formula with no row deleted | no fallback row is a candidate; no `FormulaOnly` arm | FM1: 28 cells to "no number"; row count pin 567 -> 554; F201 formula rows | 3.7, 3.10 |
| P25-6 | Card and MCP text: sourced, calculated by, no number | one text on card, MCP and FM1 | MCP wire snapshot; card snapshots | none |
| P25-7 | Delete the old names; docs; `FEATURE_CATALOG.md` | search guard: no `series_rank`, no score terms | none | none |

### Earlier rulings

| Ruling | Under the model |
|---|---|
| 2026-10-31, series: "a specialty row serves only a tool of that series; a generic tool uses standard rows first and falls back to a specialty row" | Replaced. No "standard" and "specialty". No fall-back to another category. |
| P16 item 8, the band: the lowest chip of several charts ships; no rise above the score winner | Kept: the lowest chip at one step ships, hobby charts stay out. Replaced: the no-rise rule against a score winner (there is no score winner). |
| P23c, 2026-11-10: a row printed at the tool's size is a source and beats an interpolated value | Kept, and made the general order: sourced before calculated. |
| P24a, 2026-11-10: a straight row serves only a straight tool | Kept, and made general: every category is exclusive. |
| P2 (b), 2026-11-08: the Onsrud 60-200 row stands for the unbranded 6 mm tool | Replaced, unless the operator keeps it (3.4). |
| G1, G2, G3, G6, G10 claims; the micro and tip-floor refusals; S11 "no raw fallback" | Kept. |

## 6. Rulings the operator must give

1. 3.1: is the size law (forms B and C) a calculation, or only form A?
2. 3.2: is flute count a hard match? If yes, is "the chart prints one flute count" a rule?
3. 3.3: do ZrN rows join their geometry's category, or does a tool state its coating?
4. 3.4: the 6 mm tool: plain spiral reading Spektra, or down-cut?
5. 3.5: may a bull nose read a flat-end row by a named rule?
6. 3.10: does the formula with no row go?

## 8. Operator rulings (2026-11-11, the operator's words; recorded by the lead)

1. Size law forms B and C: "yeah. keep it." They stay as named calculations, labelled as past the
   printed sizes.
2. Flute count: "flute count should match! I'm pretty sure it changes a lot right?" A hard match.
   The lead's rider (to confirm with the operator when the P25-0 list is in): a chart that prints
   one flute count for a tool serves that tool only at that count; no borrowing across counts.
3. ZrN rows join the category of their geometry: "yes".
4. The wanaka 6 mm tool: "its up cut - I do own downcut bits". A plain spiral tool. It no longer
   reads the Onsrud 60-200 down-cut row; this replaces ruling P2 (b) of 2026-11-08. The operator's
   down-cut bits are separate tools that state `down_cut`.
5. A bull nose reads a flat-end row: "no".
6. The formula with no row: OPEN. The operator asked for the state of the math against the lookup
   ("I thought we did a huge amount of force modelling to try and make this all based on math, not
   just look up, then validate the math with the lookups"). The lead's answer: the chip per tooth
   is the one looked-up input; force, power, deflection, the limits, the RPM, the lifts and the
   pocket width are calculated; the no-row formula is an unsourced rule of thumb, not the force
   model. Options put to the operator: (a) no number; (b) keep the formula; (c) a force-model
   fallback (the largest chip inside the power, force-budget and deflection limits), as its own
   designed phase. Lead recommendation: (a) now, (c) designed next.
Spiral force (P24b): one force line for every tool stays ("I've never seen a helix quoted").
