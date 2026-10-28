# W2 pre-registration: slope-aware Waterline ladder on vertical walls (D69 §3, rank 2)

Written before the code, on HEAD 3cc6b0dc2 (W1 a884902e6, W1b 3cc6b0dc2). Lead answers of 2026-10-28: points 1–4 of
my proposal, the 50 µm bound, I edit `session/compute.rs`.

## The rule

Inside a W1 chain (helical on, `WaterlineConfig::helical = true` only), the chain may skip a ring `i` when all hold:

1. The rings from the last kept ring `a` to ring `i + 1` coincide: each lies inside the other to within the op's
   `sampling` (the W1 containment test `ring_inside`, both ways). No new number.
2. The gap `z(a) − z(i + 1)` is at most the rough's axial step: `ExecutionContext::rest_axial_step_mm`, the smallest
   `OperationConfig::depth_per_pass()` of the earlier enabled ops of the setup, set only when the op cuts the
   remaining stock.
3. The deflection check passes at that gap: `feeds::predict::tip_deflection_from_engagement(tool, material, ap, ψ, fz)`
   (the shared force and cantilever model of the tool_load deflection gate, `tool_load/deflection.rs`
   `sample_tip_deflection_mm`; the cantilever is `ToolDefinition::tip_deflection_mm`, `tool/mod.rs`) is at most
   `tool_load::deflection::WITHIN_BOUND_MM` (50 µm, the gate's "Within, Validated" bound). Inputs:
   - `ap` = the gap;
   - `ψ` = `feeds::force::immersion_angle(ae, R)`, with `ae` the widest radial skin that the remaining stock shows at
     ring `i + 1` (each point: `R + cell/2 − d`, `d` the distance to the nearest cell that holds material above the
     level within the rule's reach `R + cell`; a cell off the grid counts as a full slot, `ae = 2R`);
   - `fz` = op feed / (`spindle_rpm` × `flute_count`).
4. The first and the last ring of a chain always stay.

No widening on fresh stock, or with no material, no `spindle_rpm`, or no rough step. Then the ladder is the W1 ladder,
move for move. A skipped ring changes only the descent: the lap from the last kept ring descends over the wider gap,
and the lap count follows the W1 ramp rule (`DRESSUP_RAMP_ANGLE_DEG`).

## New context fields

`ExecutionContext::material: Option<&Material>` and `ExecutionContext::rest_axial_step_mm: Option<f64>`, default
`None` in `ExecutionContext::new`. The session fills them beside `rest_top_z` (`session/compute.rs` and
`session/compute/generation.rs`). The session part waits until the uncommitted D53 work in those files is committed.

## Goldens and fixtures that may change

- `op_golden_g_opgolden.toml` rows `waterline/fresh` and `waterline/rest`: NOT expected to change. Fresh has no rest
  stock (rule 2 and the skin need it). On rest, the cap fixture's rings are not coincident (a sloped cap), so rule 1
  fails at every ring.
- No other golden holds a helical Waterline on rest stock with a material and an rpm.
- Re-bless: none expected. A move of any golden is examined first, not re-blessed.

## Sentries (new file `a_waterline_widens_its_step_on_a_vertical_wall_w2.rs`)

1. A vertical-walled chain on rest stock with a rough step D: the kept rings are at most D apart, fewer than the W1
   ladder; the first and last rings stay.
2. With the deflection check failing (a long, thin tool), no ring is skipped.
3. A drafted wall (rings not coincident): no ring is skipped.
4. With no rough step, or no material: the output equals the W1 output, move for move.
