# P27 brief: a chip-load model from fundamentals, validated against the vendor tables

Written by the lead on 2026-11-11 for a dedicated session. The operator's direction, in their
words:

> "Instead of trying to do it perfectly. Could we just try and make math that gives us similar
> results to the existing lookup tables everywhere. Solely based on geometry, materials of the tool
> and the stock. If it matches the tables, then I think we can assume it will do a good job finding
> the other tables. [...] Because if we have that, all cuts can go through our own pipeline, based
> off fundamentals, and then we just use the vendor data as validation."

## 1. Where the app stands

- The target chip per tooth is the ONE looked-up input. A vendor row gives it; the size law, the
  hardness transfer and the family transfer extend it.
- Everything else is calculated: force, power and deflection (the Curti 2021 density law, helix 0,
  `material/force_line.rs`), the limits (power, the force budget, the depth caps, the safe
  envelope), the constant-chip RPM (`calculator/spindle.rs`), the chip-thinning lift, the Pocket
  width, the slot-ring feed.
- When no row exists, an unsourced rule-of-thumb formula gives a chip and an RPM. It is not the
  force model. Operator ruling 6 of `P25_DESIGN.md` is open on it; the lead treats it as "no row,
  no number" until this model exists.
- The row lookup is being rebuilt as categories (`P25_DESIGN.md`, rulings 1 to 5 given).

## 2. The goal

A function `chip_per_tooth(tool, stock, cut, machine) -> (chip, binding limit, each limit's
value)` built from physical limits. Each limit is a formula with named inputs. The chip is the
lowest of the limits. The function reproduces the vendor tables to inside the spread that the
vendors show between themselves. Then Suggest reads the model first, and the vendor rows become
the validation set and a warning ("the model is outside the printed band of <row>").

## 3. Candidate limits (hypotheses, to test against the data; none is assumed)

| Limit | What it says | Inputs | Likely to bind for |
|---|---|---|---|
| Tool body stress | The bending stress at the flute root or the shank stays under a fraction of the material's rupture strength | force model, diameter, core diameter, flute length, stick-out, tool material | small tools (under about 4 mm), long reach |
| Edge (tooth) strength | The load on one tooth stays under what its wedge carries | chip, depth, wedge angle (rake, relief), tool material | brittle edges, hard stock |
| Chip room | The chip volume per tooth fits the flute space | chip, depth of cut, width, flute count, core diameter, a fill factor | deep cuts, many flutes, large chips in wood |
| Minimum chip | Under about the edge radius the tool rubs | edge radius or a stated sharpness | finishing, high RPM at low feed |
| Deflection | The tip stays inside a tolerance | force model, beam model (exists) | long thin tools, finishing |
| Machine | Power at the RPM, the force budget, the axis rates | machine profile (exists) | large tools on a hobby machine |
| Surface or tear-out | The scallop along the feed, the fibre tear-out | chip, diameter, grain | finishing; may be out of scope for a first model |

Dimensional check to start from: with the force proportional to chip times depth, depth
proportional to diameter and reach proportional to diameter, a constant body stress gives a chip
proportional to diameter. The stored tables scale more slowly (the app's generic size law is
diameter to the power 0.61, `CHIPLOAD_DIAMETER_EXPONENT` in `feeds/vendor_lookup.rs`). So at least
two limits must act across the size range. Find which.

## 4. The data

- 567 vendor rows with their stored source texts: `crates/rs_cam_core/data/vendor_lut/`
  (`observations/*.json`, `sources/`, `source_manifest.json`). They cover flat, ball, tapered ball,
  V and facing tools, straight and spiral, hardwood, softwood, MDF, plywood, plastics, aluminium.
- The size-law study of P13 (`the_industrial_rows_are_a_baseline_not_a_lookup_p13`, 33 groups,
  k 0.5701) and the extrapolation programme (`planning/extrapolation_2026-09-24/`).
- The force model sources: Curti 2021 Table 5 (stored: `planning/extrapolation_2026-09-24/fetch/
  G7/sources/marcon2021_generalized_force_model.txt`), Woodson 1979 (see `research/
  large_straight_bit_roughing_2026-11-10.md`).
- Tool facts the app holds: `compute/tool_config.rs` (diameter, flute length, flute count, shank,
  stick-out, tool material, flute form, centre cutting). Facts it does NOT hold and the model may
  need: core diameter, rake and relief angles, edge radius. Decide for each: a stated fact, a value
  derived from a published standard geometry, or a fitted family constant. Never a typed guess.
- The spread between vendors is large: the app prints "flat-end vendors spread x0.80-1.56 at 2x"
  the row size. That spread is the accuracy target.

## 5. How success is judged (fix this before fitting anything)

1. Split the rows before the first fit: by vendor and by size, a training part and a held-out part
   (for example hold out one vendor per tool family and the largest and smallest size of each
   chart). Record the split in the repo.
2. The model has few fitted constants, each with a physical meaning and a stated plausible range
   (for example "the allowed fraction of rupture strength"). Count them. A fit with more constants
   than tool families is a lookup table in disguise.
3. On the held-out rows the model's error is at or under the spread between vendors for the same
   tool class. State both numbers per tool family.
4. The binding limit changes with size in a way that makes physical sense (body stress for small
   tools, the machine or chip room for large ones), and the model says which binds on each row.
5. The model fails loudly where it has no basis: a tool class with no rows in training gets "no
   model", not an extrapolation.
6. Known hard checks: the straight 18 mm router bit (Onsrud 48-000 and Freud straight rows), the
   2 mm single-flute, the 1 mm tapered ball, the 6 mm plain spiral, a 60 degree V-bit.

## 6. Deliverables (the operator's standing preference)

- An interactive HTML page that explains the model: the limits as curves over diameter and over
  chip, the vendor rows as points with their spread, the binding limit shaded, the held-out rows
  marked. A script asserts every claim the page makes.
- A design note with the formulas, each constant with its source or its fit and range, the split,
  the errors per family, the failures.
- No product code in the first session. The integration plan comes after the operator has seen
  the page: where the model enters (`feeds/` beside the row lookup, one door), what Suggest prints
  (model value, binding limit, and the vendor row as "validation: inside or outside the printed
  band"), and how ruling 6 of P25 closes.

## 7. Rules that carry over

- Constants are derived by math or printed in a source stored in the repo; a fitted constant
  states its data and its fit. No bench rig exists.
- One definition: reuse the force line, the beam model and the machine limits that exist; do not
  write a second force model.
- Every finding warns; the app never refuses a job (Suggest may give no number; generation and
  export still work).
- The lead decides and verifies; agents implement and never commit; every cargo command through
  `scripts/cargo_lane.sh`; prose in ASD-STE100.
- Be honest about what the model is: calibrated physics. Vendors hold safety factors that no
  source prints, so some constants are fitted. The held-out test is what makes it trustworthy.

## 8. Risks the session should name early

- The tables may encode the machine class of the vendor's customers (industrial routers) more
  than the tool. Then the "tool" limits never bind for large tools and the fit is weak there. That
  is a finding, not a failure: for a hobby machine the machine limits are the true answer.
- Wood-specific limits (tear-out, burning) have little published quantitative data.
- The force model is tested for mean chips of 0.04 to 0.10 mm; chip-load tables for large tools
  sit above that.
- Tool geometry facts (core diameter, rake) are rarely printed; a family constant may be the only
  option, and it must be labelled as one.
