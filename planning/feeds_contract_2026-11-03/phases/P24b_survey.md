# P24b survey: a force line that reads the tool's helix

Read-only survey, 2026-11-10. No code and no cargo run. The operator rules before any code.

## The question in plain words

The engine computes the cutting force of every wood tool from one line. The line comes from a
paper that measured three tools: a straight blade, a 15 degree spiral and a 30 degree spiral.
The engine uses the straight-blade numbers for every tool, because they are the largest.

A spiral tool cuts with less force than a straight blade. So today the 6 mm spiral end mill is
charged the force of a straight blade. Since P24a a tool states its flute form and its helix.
Should the force line read them?

## The numbers

Source: Curti et al. 2021, Table 5, as stored in
`planning/extrapolation_2026-09-24/fetch/G7/sources/marcon2021_generalized_force_model.txt`
(lines 772-793). The stored text holds all six fits: helix 0, 15 and 30, up- and down-milling.
Nothing here is typed from memory, and no fetch was necessary.

The rule is the rule the code uses today for helix 0: for each term, take the largest value
over the grain angle (0 to 180 degrees), then the larger of up- and down-milling.

| Helix | Ks term (per kg/m3) | Edge term (per kg/m3) | Ks ratio to helix 0 | Edge ratio to helix 0 |
|---|---|---|---|---|
| 0 (today) | 0.076833 | 0.006033 | 1.000 | 1.000 |
| 15 | 0.069833 | 0.000200 | 0.909 | 0.033 |
| 30 | 0.050000 | 0.000408 | 0.651 | 0.068 |

Force per mm of cutting edge in white oak (density 761.6 kg/m3), at four chip thicknesses:

| Chip (mm) | Helix 0 (N/mm) | Helix 15 | Helix 30 | Helix 30 / helix 0 |
|---|---|---|---|---|
| 0.02 | 5.77 | 1.22 | 1.07 | 0.19 |
| 0.04 | 6.94 | 2.28 | 1.83 | 0.26 |
| 0.07 | 8.69 | 3.88 | 2.98 | 0.34 |
| 0.10 | 10.45 | 5.47 | 4.12 | 0.39 |

So the paper's own data give a 30 degree spiral about one fifth to two fifths of the
straight-blade force. Most of the drop is the edge term, which is almost zero for a spiral.

## Four cautions from the paper

1. The edge term of the spiral fits is negative at most grain angles (15 degree down-milling
   is negative at every angle). The paper says: "The fact that the intercept is slightly
   negative makes the empirical model very hazardous to extrapolate for thinner chips". The
   envelope rule keeps the largest value (a small positive number at one end of the range).
2. The fit error is large. Table 6 prints an error of 9 % to 34 % for helix 15 and 30, by
   species.
3. A spiral pushes along the tool axis. Table 4: the axial force is 25 % to 45 % of the cutting
   force at helix 30, and 0 % to 5 % at helix 0. The engine does not model this force.
4. All three test tools are 20 mm, 2 flutes, rake 25 degrees. The 6 mm tool is outside that.

## 1. Where the line is derived and stored

- `crates/rs_cam_core/src/material/force_line.rs`: the four printed helix 0 quadratics
  (`CURTI_UP_HELIX0_KS_N`, `CURTI_UP_HELIX0_INT_N`, `CURTI_DOWN_HELIX0_KS_N`,
  `CURTI_DOWN_HELIX0_INT_N`), `GrainQuadratic::max_over_grain`, `envelope_term`,
  `curti_helix0_envelope` (0.0768333 and 0.0060333), and `ForceLine::curti_density_law`
  (`Ks = Ks_n x density`, `F_edge = Int_n x density`).
- Density: `WoodSpecies::fpl_density` and `fpl_eq_4_11_g12` (FPL tables, 12 % moisture).
- `Material::force_line()` (`material/mod.rs:1314`) is the one door. It takes no tool.
- `fetch/G7/verified_rows.json` also holds the helix 15 and helix 30 rows.
- The helix 15 and 30 fits are not in the code.

## 2. Who reads the line

About 30 call sites of `Material::force_line()`, in these groups:

- Power: `feeds/mod.rs` (`power_force_line`), `feeds/calculator/power.rs`, `close.rs`,
  `feeds/operating_point.rs`, `tool_load/power.rs`.
- Force and deflection: `feeds/engagement.rs` (`edge_force`), `feeds/predict.rs`,
  `tool_load/cut_force.rs`, `tool_load/deflection.rs`, `tool_load/distribution.rs`.
- The force budget and the depth cap: `feeds/force_budget.rs` (four sites).
- The envelope: `feeds/envelope.rs` (three sites).
- The efficiency figure: `feeds/efficiency.rs`.
- The modulator and the simulation: `dressup/feed_modulation.rs`, `compute/simulate.rs`,
  `session/compute.rs`.
- Surfaces: `rs_cam_viz` `ui/properties/stock.rs`, `ui/feeds/compare.rs`, `app/mcp/generation.rs`.

A helix line needs the tool at the door, so every one of these sites changes its call.

## 3. The rule options

- (a) Keep helix 0 for every tool. Today's rule. The force is too high for a spiral, on the
  safe side.
- (b) The tool's helix picks the nearest measured fit at or below it: below 15 reads helix 0,
  15 to below 30 reads helix 15, 30 and above reads helix 30.
- (c) Interpolate between the three fits.

### What (b) moves

- Nearly every tool. The helix of a tool is a plain number with the default 30
  (`tool::DEFAULT_HELIX_DEG`), not a stated fact. A tool that states nothing would read the
  helix 30 line. A default would then cut the force to a third.
- FM1: the power and force columns of every solid-wood cell with a helical tool (most of the
  1000 cells). MDF does not move (the Goli line has no helix).
- The hobby force budget (`HOBBY_SLOT_POINTS`) is computed with the same line and the default
  helix, so the budget drops with the force. For a spiral tool the depth cap then moves
  little. For a straight tool (helix 0) the force stays and the budget drops: its depth cap
  gets much smaller. This is reasoning from the code, not a run.
- The acceptance table: the power row of every wood milling op (the table uses Ks and F_edge
  at helix 0).
- F201: four new printed quadratics per helix (eight constants), all Cited.
- Sentries that pin the line: `the_force_line_is_printed_per_family_b6`, `literature_parity`,
  `literature_matrix`, `the_hobby_force_budget_recomputes_from_the_printed_rows_p13c`,
  `the_engaged_edge_force_sums_the_helix_p11a`, `one_engagement_before_and_after_the_simulation_k7`,
  `power_reads_the_true_engaged_section_p20d`, the lib test
  `deflection_chain_matches_hand_calc_from_published_formulas`, and viz
  `the_power_row_names_its_force_line_b6`.
- The earlier check "Woodson 1979 white oak agrees with the force line within 19 %" was made
  against the helix 0 line.

## 4. Wanaka, the 6 mm tool (idx 2), by arithmetic

Today (acceptance table, idx 2): feed 6841, 24000 rpm, ae 1.638, ap 6, white oak.

| | Helix 0 (today) | Helix 15 | Helix 30 |
|---|---|---|---|
| Shear power (kW) | 0.06557 | 0.05960 | 0.04267 |
| Edge power (kW) | 0.07276 | 0.00241 | 0.00492 |
| Total power (kW) | 0.13833 | 0.06201 | 0.04759 |
| Ratio to today | 1.00 | 0.45 | 0.34 |
| Force per mm at the peak chip 0.127 (N/mm) | 12.03 | 6.91 | 5.15 |
| Deflection ratio (follows the peak force) | 1.00 | about 0.57 | about 0.43 |

The power ratio to the spindle goes from 0.092 to about 0.03. Power does not bind this op
today, so no feed or RPM moves on idx 2. The deflection figure and the force budget depth are
the values that would move. The absolute deflection needs a run.

## 5. Recommendation

Keep (a) for now. Reasons:

1. The helix of a tool is a default (30), not a stated fact. Under (b) a default would cut
   every force to about a third. First make the helix a stated fact, as P13a did for the
   other tool facts.
2. The change is in the unsafe direction (lower force, deeper cuts), and the spiral fits
   have a near-zero or negative edge term and an error of up to 34 %.
3. Nothing binds on the 6 mm tool today, so (b) buys no feed there.

If the operator wants the spiral force: choose (b), not (c). (b) uses only printed fits, and
"at or below" keeps the safe side between the measured angles. Conditions for (b): only a
stated helix reads a spiral fit; the hobby budget keeps one stated rule for its own tools; a
straight tool keeps today's line.

One smaller step is possible without (b): show the helix 15 and 30 values on the card as
information, with no effect on any limit.
