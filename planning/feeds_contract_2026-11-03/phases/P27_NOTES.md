# P27 notes: first thoughts from the earlier lead (use or discard)

These are hypotheses, not findings. Nothing here was tested.

## Candidate limits on the chip

| Limit | What it says | Likely to bind for |
|---|---|---|
| Tool body stress | The bending stress at the flute root or the shank stays under a fraction of the material's rupture strength | small tools, long reach |
| Edge (tooth) strength | The load on one tooth stays under what its wedge (rake, relief) carries | brittle edges, hard stock |
| Chip room | The chip volume per tooth fits the flute space | deep cuts, many flutes, large chips in wood |
| Minimum chip | Under about the edge radius the tool rubs and burns | finishing, high RPM at low feed |
| Deflection | The tip stays inside a tolerance (the beam model exists) | long thin tools |
| Machine | Power at the RPM, the force budget, the axis rates (all exist) | large tools on a hobby machine |
| Surface or tear-out | The scallop along the feed, fibre tear-out | finishing |

## One dimensional clue

With the force proportional to chip times depth, depth proportional to diameter and reach
proportional to diameter, a constant body stress gives a chip proportional to diameter. The stored
tables scale more slowly: the app's generic size law is diameter to the power 0.61
(`CHIPLOAD_DIAMETER_EXPONENT`, `feeds/vendor_lookup.rs`). So more than one limit acts across the
size range, or the tables encode something other than the tool.

## Things that surprised the earlier sessions

- For large tools the chart chips may mostly reflect the vendors' customers' machines (industrial
  routers). On a hobby machine the machine limits are then the true answer, and the model could be
  better than the chart there.
- The force model is tested for mean chips of 0.04 to 0.10 mm; chart chips for large tools sit
  above that. Woodson 1979 (white oak, thicker chips) agrees with the line within about 19 %.
- Every simulated gate reads the force at helix 0 (a straight edge) for every tool. The paper's
  spiral fits are much lower and less reliable; the operator ruled to keep one line.
- Vendors almost never print helix, rake, core diameter or edge radius.
- Two rules of thumb in the product have no source and may fall out of a real model: the depth
  rule "2 x diameter" and the plunge-rate base rule.
- The 3D adaptive ops have no width rule (the Pocket has one); see `P26_survey.md` if it exists.
- A wide spread exists between vendors for the same tool class; measure it before judging a fit.

## Hard cases worth checking early

The straight 18 mm router bit on a 6 mm shank (Onsrud 48-000 and Freud straight rows), the 2 mm
single-flute, the 1 mm and 0.5 mm tapered balls, the 6 mm plain spiral, a 60 degree V-bit, a
facing bit.
