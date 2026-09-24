# Extrapolation programme: reasonable claims where the data stops

Status: PLAN, not started. Date: 2026-09-24. Owner: a whole session of its
own (see `PROMPT.md`). Operator's framing, 2026-09-24:

> How can we make a reasonable claim, to fill gaps our data does not,
> within reasonable limits — for each circumstance we need to fill,
> grouped in some way. Each tool/path needs an investigation into its own
> extrapolation; "impl" extrapolation per group and apply generic ones
> only when we confirm they make sense. Look for trends in vendor data,
> then match to equations; or sim data; or whatever.

## 1. Where the programme starts from

The feeds matrix programme (`planning/feeds_matrix_2026-09-23/`) made every
recipe state its basis. On the matrix of 2026-09-24 (960 cells: 5 tool
kinds × 2 sizes × 24 operations × 4 woods):

| State | Cells | Why |
|---|---|---|
| ship, vendor row | 366 | a printed chart row matched the tool, family, role and material category |
| ship, formula backed by a chart | 82 | `FORMULA_BACKING_v2` judged the formula inside 0.5–2× of a chart |
| refuse, registry tool rule | 192 | the operation's own rule (a flat end mill on Pencil, ...) |
| refuse, drill | 80 | no vendor prints a wood drill chipload; the 2.5 plunge multiplier is unsourced |
| refuse, CLUELESS | 220 | no published figure for the class (V-bit in MDF/plywood 64, bull nose finishes 48, ball nose in plywood/MDF 24, the rest below half of a chart) |
| refuse, never judged | 20 | a class the judgement never saw formula-only |

Two more gaps ship today but are extrapolations the card discloses:

- 38 shipping cells sit on an **extrapolated row** (the repo's diameter law
  `(d / D_row)^0.61` scaled a row to another size; the exponent was fitted on
  3–12 mm rows and carries no source).
- 83 vendor-backed cells have **no band** (Amana Spektra prints one value per
  size): Suggest shows one line, the burn gate is silent on the low side,
  the simulation modulates bandless, the strategy advisor does not optimise.

And two rules stand as stop-gaps until this programme replaces them:

- **The size rule** (87027060): a tool whose lookup diameter is under 1.5 mm
  and whose only row is more than 2× its size refuses. It stops the
  0.5–1.0 mm tapered balls shipping 5–7 % of the tip diameter per tooth.
  It also refuses the operator's wanaka "3D Finish 6" (1 mm tip).
- **The material categories** (87027060): softwood and hardwood share a
  category (the Janka scale models the difference and the card prints it);
  plywood and MDF/HDF/particleboard never substitute.

The rule of the whole programme is the R1 rule: **a claim ships only with
its evidence, its limits and its record on the card; outside the limits it
refuses.** Nothing here invents a number to close a gap.

## 2. The gap inventory, grouped

Each group is one investigation and, where it earns it, one implementation.
The groups are by the axis that is missing, not by the operator's tools.

| # | Group | What is missing | Cells today | Candidate evidence |
|---|---|---|---|---|
| G1 | **Size** (a row exists for the family but not at this diameter) | a chipload at another diameter, above or below the printed sizes | 38 ship extrapolated; every sub-1.5 mm tool refuses | multi-size printed rows: Amana Spektra 1/8", 6 mm, 1/4"; Amana ball 0.79–12.7; Onsrud tapered 1/8", 1/4"; V-groove 3.175–12.7; Amana/PreciseBits micro charts (sub-1 mm) |
| G2 | **Material category** (a row exists for another wood category) | plywood and MDF cells whose only rows are solid wood | 50 refuse (ball nose and V-bit in plywood/MDF) | vendor sheets that print both categories for one tool (Amana Spektra prints softwood, hardwood, both plywoods and MDF per size) → a per-category ratio with its spread |
| G3 | **Operation family** (a row exists for the tool in another family) | bull nose finishes; ball nose adaptive/pocket; V-bit adaptive and 3D finish | 32 + 24 + 16 + 8 refuse | Onsrud prints roughing and finish columns for one tool (77-100); the pass-role ratios the LUT already scores |
| G4 | **Band** (one value printed, no minimum) | a low bound for the burn gate, the modulator and the advisor | 83 ship bandless | the spread of the rows that print both limits, per family; Onsrud's "±" notes; the 0.5–2× judgement threshold itself |
| G5 | **Engaged geometry** (V-bit width, tapered cone, bull corner) | the diameter a chart figure applies to on a tool whose engaged width changes with depth | V-bit 16 + 16 refuse "at the engaged width"; every tapered cell is read at the cone | PreciseBits tapered guidance (1× tip per pass, 2× max); V-groove charts that print a depth; the R2 one-engaged-diameter rule |
| G6 | **Drill** | any wood drill chipload or plunge rate | 80 refuse | vendor drill charts for wood (Onsrud, Amana boring bits, Freud brad-point) — the 2026-08 audit found none for peck depth; chipload columns exist for some |
| G7 | **Material physics** (no Kc) | force and power for MDF, plywood, plastics | power on 208 of 448 cells only | Wood Handbook specific cutting energy, published Kc for MDF/plywood, or a sim-derived proxy with its spread |
| G8 | **Long tool and small tool loads** | the 0.88/0.75 long-tool share and the micro-tool concern the literature cell flags | 428 cells carry the long-tool share (repo rule) | deflection model already in the engine (`feeds::force`); vendor overhang notes; PreciseBits micro-tool rules |
| G9 | **Machine class** (the cutter vendor's chart is written for an industrial router) | a chip the FRAME can take, not only the cutter: on a Shapeoko a 6 mm two-flute in hardwood ships Amana's 0.131 mm/tooth (4720 mm/min at 18 000) where hobby practice is 0.05–0.09 | every vendor-backed cell on a hobby profile | the machine vendor's own published feeds (Carbide 3D's Shapeoko chart; Onefinity, Sienci charts), loaded as rows keyed by machine class so the profile matches its own vendor first and the card names it; the retired ×0.75 was this gap as an unsourced factor |

Groups G1–G4 are ratios inside vendor data (trend-then-equation). G5 and
G7 are geometry and physics (model-then-confirm). G6 and G9 are fetches
(G9 first: it is the gap the operator sees on every roughing card today).
G8 is a model the engine already has, to be preferred over a factor.

## 3. What one investigation produces

For each group, in this order, and nothing lands before step 4:

1. **The trend.** Pull every printed row the LUT holds along the missing
   axis (size, category, family, ...). Plot the ratios. State what the data
   shows and its spread: for example "the Spektra chipload scales as
   d^0.58 ± 0.05 between 3.175 and 6.35 mm across 5 materials and 2 flute
   counts; the Amana ball rows scale as d^0.71 between 0.79 and 12.7 mm".
   Where the LUT is thin, fetch more printed rows first (a research
   workflow like R5, with URL verifiers and stored chart text).
2. **The equation and its limits.** Fit the simplest form that the trend
   supports, per family where the families differ. State the valid range
   (the printed sizes and one modest step beyond), the residual, and the
   cases it must not serve. Compare with the repo's current rule (the 0.61
   law, the category ban, the pass-role scores) and say which wins.
3. **A second witness where one exists.** The simulation's chip and force
   samples on a real part (the wanaka fixture, the terrain fixture), the
   literature-matrix cells, or a physical rule (chip thickness against the
   edge radius for micro tools; deflection against the shank for long
   tools). A claim with two witnesses may be generic; a claim with one stays
   per group.
4. **The implementation.** One `Extrapolation` implementation per group
   (§4), with the claim, its evidence, its range and its confidence in code
   and on the card; refusal outside the range by the R1 rule. Sentries pin
   the fitted numbers and the range edges. The matrix instrument re-runs
   and the CSV is committed with the change.
5. **The record.** `EXTRAPOLATION_<group>.md`: the trend table, the fit,
   the witnesses, the rulings taken, and the cells that moved.

## 4. The implementation shape

```rust
/// One reasoned claim for one gap group. The engine asks the group that
/// owns the missing axis; a group answers only inside its range.
pub trait Extrapolation {
    /// The gap this claim fills (size, category, family, band, ...).
    fn gap(&self) -> Gap;
    /// The claim for this query, or None when the query is outside the
    /// range the evidence supports. Never a guess.
    fn claim(&self, query: &LookupQuery, anchor: &VendorObservation) -> Option<Claim>;
}

pub struct Claim {
    pub value: ChiploadBand,          // or the axis's own type
    pub rule: &'static str,           // "Spektra size law d^0.58, fitted 2026-10-xx on 30 rows"
    pub source_rows: Vec<ObservationId>,
    pub range: Range<f64>,            // where the claim is valid
    pub residual: f64,                // the fit's spread, so the card can say "± 8 %"
    pub confidence: Confidence,       // TwoWitnesses | OneWitness
}
```

- The vendor lookup stays the first door: a printed row inside its own
  range wins. An `Extrapolation` runs only when the lookup has no row for
  the query and an anchor row exists on the same tool family.
- `FeedsSupport` gains an arm `Extrapolated { claim }`, so the matrix,
  the card ("scaled ×0.32 from the 3.175 mm row by the Spektra size law,
  ± 8 %; valid 1.5–12.7 mm") and the diagnostics carry the claim with its
  rule and residual. No invisible calculation.
- A group that earns two witnesses may register as generic (applies to
  every family with an anchor); a one-witness group applies only to the
  families its trend covered.
- The stop-gap rules of §1 are replaced by the groups that supersede them
  (the size rule by G1, the category ban by G2) and deleted, not kept as
  fallbacks.

## 5. Phases

0. **Inventory** (read-only, this file's §2 checked against the current
   matrix and LUT; the CSV lists every refused and extrapolated cell).
1. **Fetch** (workflow like R5): the missing printed rows per group, URL
   verified, chart text stored with hashes, loaded as observations.
2. **Trends** (read-only agents, one per group): the ratio tables and
   plots from the LUT, written to `EXTRAPOLATION_<group>.md` §1.
3. **Fits and witnesses** (one agent per group): the equation, its range,
   the second witness where one exists; the recommendation "generic /
   per-family / refuse".
4. **Rulings** (operator): which groups ship, at which range and
   confidence; the default when a group has one witness.
5. **Implementation** (one editor per group, sequential where files
   overlap): the trait, the arms, the card line, the sentries, the matrix
   re-run per landing. The size rule and the category ban leave here.
6. **Close-out**: the matrix, the catalogue, the memory note; the wanaka
   "3D Finish 6" recipe is the acceptance case for G1.

## 6. What this programme does not do

- It does not author a vendor number. A fitted law is a claim about vendor
  numbers, and it says so.
- It does not lower the R1 bar: a class with no anchor row on its tool
  family still refuses.
- It does not reopen the dial, the depth ladder or the rubbing floor
  (ruled 2026-09-23/24).

## 7. Acceptance

- Every shipping cell states one of: a printed row, a formula judged
  against a chart, or an `Extrapolated` claim with its rule, range and
  residual on the card.
- Every refusal names the gap group that would fill it.
- The size rule and the category ban are gone, replaced by G1 and G2 with
  ranges, or kept with the trend evidence that says no law exists there.
- The matrix refusal count is lower than 512 only through claims that
  carry evidence; the report states the count per group.
