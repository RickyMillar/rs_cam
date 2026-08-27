# S1 recon — replay-instrument groundwork (read-only, 2026-08-28)

Repo at HEAD `1c353d9f`, branch master. All paths relative to repo root unless
absolute. Target G-code: `planning/airrun_2026-08-19/wanaka200_1_Setup_1.nc`
(17,167 lines) and `wanaka200_2_Setup_2___front.nc` (489,243 lines), generated
from `planning/airrun_2026-08-19/wanaka200.toml`.

## Q1 — Export frame contract

Stock (`wanaka200.toml:7-14`): `x=240, y=250, z=25`, origin
`(-20, -25, -18)`, so **world** stock bbox = min `(-20,-25,-18)`, max
`(220,225,7)`. Post: `safe_z = 10.0` (`:34`).

The exporter applies a per-toolpath translation at emit time only
(`crates/rs_cam_core/src/gcode/mod.rs:308-321`), computed by
`SetupEvalContext::export_datum_shift` (`crates/rs_cam_core/src/session/eval_context.rs:175-187`):

```rust
if self.local_to_global.is_some() {           // non-identity: already stock-relative
    P3::new(0.0, 0.0, 0.0)
} else {                                      // identity: world -> stock-relative
    P3::new(-self.world_stock_bbox.min.x, -self.world_stock_bbox.min.y, 0.0)
}
```

Applied by `toolpath_in_export_datum` as a pure `m.target + shift`
(`gcode/mod.rs:272-285`). Z is deliberately never shifted (`gcode/mod.rs:232-253`;
pinned by `tests/export_datum_setup_frame.rs:364-387`).

- **Setup 2 "front" (`face_up=top`, z_rot 0 — identity)**: toolpath is generated
  in WORLD frame; exported = world + `(+20, +25, 0)`. Stock box in exported
  coords: X `[0,240]`, Y `[0,250]`, **Z `[-18, 7]`, stock top at Z=7**.
- **Setup 1 (`face_up=bottom`, z_rot 0 — non-identity)**: shift is zero; the
  toolpath is generated AND exported in the zero-rooted setup-local frame.
  `world_to_local` chain = `-stock_origin` then face flip
  (`crates/rs_cam_core/src/compute/transform.rs:282-295`); Bottom arm is
  `P3::new(p.x, stock_d - p.y, stock_h - p.z)` (`transform.rs:105`). Concretely:
  **exported = (x_w + 20, 225 − y_w, 7 − z_w)**. Stock box in exported coords:
  `[0,240]×[0,250]×[0,25]`, **stock top at Z=25** (the board's world underside).

Safe/retract Z: `safe_z = effective_safe_z(post.safe_z, heights_stock_bbox.max.z)`
(`eval_context.rs:112`), where `effective_safe_z = raw.max(stock_top + 5.0)`
(`SAFE_Z_CLEARANCE_MM`, `crates/rs_cam_core/src/compute/config.rs:1402-1411`).
Setup 1: `max(10, 25+5)` = **Z30**; Setup 2: `max(10, 7+5)` = **Z12** — both
match the files' `G0 Z30.000` / `G0 Z12.000` retracts (line 7 of each).

## Q2 — Emitter grammar

(a) **I/J are INCREMENTAL (start → centre)**. Emit site
`crates/rs_cam_core/src/gcode/emitter.rs:358-364`:
```rust
"{} X{x:.xyz$} Y{y:.xyz$} Z{z:.xyz$} I{i:.ijk$} J{j:.ijk$} F{feed:.feed_dp$}", dir.word()
```
Pinning code (not a doc): the degeneracy guard computes
`let center = (sx + i, sy + j);` from the arc START (`emitter.rs:382`), and the
IR source is `MoveType::ArcCW { i, j, .. }` — "I/J are offsets from start to
center" (`crates/rs_cam_core/src/toolpath.rs:35-38`). `toolpath_in_export_datum`
translates targets only and leaves i/j alone (`gcode/mod.rs:268-283`).

(b) A `Statement::Rapid` always writes all three axes in one block
(`emitter.rs:236-238`: `"G0 X{x:.xyz$} Y{y:.xyz$} Z{z:.xyz$}"`), so a combined
XY+Z rapid is representable. Measured in BOTH shipped files: **zero** G0 blocks
change XY and Z together (awk position-tracking scan, this recon); rapids are
Z-then-XY or XY-then-Z sequences.

(c) X/Y/Z always fully spelled on G0/G1/G2/G3, even when unchanged. Two
exceptions: `Statement::SafeZRetract` emits Z-only `G0 Z{z}` (`emitter.rs:274-276`;
3 per file), and **F is modal-elided** via `Statement::LinearModal`
(`emitter.rs:248-250`; tracker `gcode/modal.rs:24,46-49`) — 453,269 of setup 2's
G1 lines carry no F; arcs always carry F. Parser rule: carry XYZ+F as modal state.

(d) Drill pecks are **pre-expanded G0/G1 — no canned cycles** (each file's only
`G8x` is the preamble `G80`). `drill_peck_full_retract`
(`crates/rs_cam_core/src/drill.rs:202-232`) walks `fed_descents` (R-plane
rooted, `drill.rs:166-198`) emitting feed to `descent.to_z`, retract rapid to
`params.retract_z`, re-entry rapid to `to_z + 0.5` (`PECK_REENTRY_CLEARANCE_MM`,
`drill.rs:125`). The emitted R-plane is NOT toml `retract_z=2`:
`retract_z: effective_safe_z(cfg.retract_z, ctx.stock_bbox.max.z)`
(`crates/rs_cam_core/src/compute/execute.rs:955`, also `:1002`) = max(2, 25+5)
= **Z30** in setup 1 — matching the file (30→27, re-entry 27.5, 27.5→24, …).

(e) Comment formats: `format!("LOAD: {} [T{}]", tool.label, tool.number)`
(`gcode/program_builder.rs:369-379`) and
`format!("TOOL CHANGE: {label} [T{tool_number}]")` (`gcode/post.rs:317`), each
wrapped in the post's comment style → `(LOAD: … [Tn])`. The label is
`t.name.as_str()` from the tool config (`gcode/mod.rs:339-343`) — **verbatim
toml `name`** (verified: `(LOAD: 6mm 2F Carbide End Mill [T1])`,
`(TOOL CHANGE: R1.5mm x 6mm x 30.5mm 2F Tapered Ball [T15])`). Caveat:
`sanitize_comment_text` maps parens→brackets inside comments (`post.rs:313-320`),
so the PHASE label `"6 3D Rough (front)"` prints as `(6 3D Rough [front])` —
don't key a parser on verbatim op names.

## Q3 — Clearance primitive

`crates/rs_cam_core/src/dexel_stock/mod.rs:486-544`:

```rust
pub fn max_clearance_tip_z_for_profile(
    &self,
    cx: f64,
    cy: f64,
    radius: f64,
    cutter: &dyn crate::tool::MillingCutter,
) -> Option<f64> {
```

Rule (doc `:432-484`): lowest tip Z clearing everything under the disc —
`tip_z >= max over cells of [conservative_top(cell) − height_at_radius(r_near)]`.

- **`None` does NOT mean "no material"**: `None` only when the disc is entirely
  off-grid (`clamped_cell_bbox` → `None`, `:503-510`) or every visited cell lies
  past the envelope (`:481-484`). An on-grid disc over fully-cut cells still
  returns `Some(bound)` from `conservative_top` — which starts at
  `bbox.max.z` and only ever lowers (`crates/rs_cam_core/src/dexel.rs:294,394`,
  `conservative_top_at :517-518`, `lower_conservative_top :527-529`), so an
  emptied cell may keep a stale-high (safe-direction) bound.
- **Frame of cx/cy**: the grid's own frame — `origin_u/origin_v = bbox.min.x/y`
  of the bbox the stock was built over (`dexel.rs:378-395`). Build the stock in
  exported coordinates and pass exported coordinates.
- **Half-diagonal conservatism** (`:512-515,:532`):
  `let half_diag = cs * FRAC_1_SQRT_2;` … `r_near = (dist_sq.sqrt() − half_diag).max(0.0)`
  — profile evaluated at the closest point a cell's material could be, so the
  query "may only ever err high" (`:479`). Plus the flat-disc conservatisms:
  `conservative_top` reading and half-cell disc dilation (`reach = radius + cs*0.5`, `:497`).
- **Radius**: cells past the whole envelope are skipped, never clamped
  (`:534-537`), so any radius is safe in the no-panic sense — but a radius
  BELOW `envelope_radius_mm()` shrinks the scanned disc and under-scans
  off-axis material. Conservative use = pass the envelope radius. Flat-endmill
  byte-identity vs `max_conservative_top_z_in_disc` pinned by
  `tests/profile_link_ceiling.rs::flat_endmill_profile_ceiling_is_byte_identical` (`:452-460`).

## Q4 — Dexel replay API

Constructor (`crates/rs_cam_core/src/dexel_stock/mod.rs:145-173`) — any bbox,
any cell size:
```rust
pub fn from_bounds(bbox: &BoundingBox3, cell_size: f64) -> Self       // Z-grid only
pub fn from_stock(x_min, y_min, x_max, y_max, z_min, z_max, cell_size) -> Self
```

Per-segment public entries (`mod.rs:206-257`):
```rust
pub fn stamp_tool_at(&mut self, lut: &RadialProfileLUT, radius: f64, cx, cy, tip_z, direction: StockCutDirection)
pub fn stamp_linear_segment(&mut self, lut: &RadialProfileLUT, radius: f64, start: P3, end: P3, direction: StockCutDirection)
```
A pure-vertical plunge needs no special call: `stamp_segment_on_grid` detects
`seg_len_sq < 1e-20` and stamps a point at min depth
(`dexel_stock/stamping.rs:659-664`). There is **no public arc stamper** — the
replay linearizes arcs itself: `replay_moves`
(`dexel_stock/simulation.rs:99-186`) matches `MoveType::ArcCW { i, j, .. }`,
calls `linearize_arc_into(&mut arc_buf, start, end, i, j, cw, cell_size)`
(`crate::arc_util`), and stamps each window pair. Toolpath-level drivers:
`simulate_toolpath(toolpath, cutter, direction)` (`simulation.rs:36-44`, builds
the LUT internally) → `simulate_toolpath_with_lut_cancel` (`:59`) →
`replay_moves`, the "single playback enumerator" (`:81-99`).

Two load-bearing facts for the instrument: `replay_moves` **skips rapids**
(`MoveType::Rapid => {}`, `simulation.rs:137`) — parse the whole .nc into one
`Toolpath` (`rapid_to`/`feed_to`/`arc_cw_to`/`arc_ccw_to`,
`toolpath.rs:137-152`; arc i/j are start→centre, matching G-code I/J directly),
replay it, and probe rapids separately. And out-of-window stamps are clean
no-ops (`stamping.rs` tests `stamps_entirely_outside_the_grid_are_a_clean_skip`
`:1971`, `a_toolpath_that_leaves_the_stock_does_not_panic` `:2091`) — which is
what makes PLAN.md's windowed fine-grid replay sound.

**LUT required: yes** at the stamping level (`&dyn MillingCutter` is not
accepted there). Build:
`RadialProfileLUT::from_cutter(cutter, crate::radial_profile::LUT_SAMPLES)`
(`crates/rs_cam_core/src/radial_profile.rs:35`, `LUT_SAMPLES = 4096` `:16` —
use 4096, not 256: small-tip taper accuracy). Direction for both files as
replayed in their exported frames: `StockCutDirection::FromTop`.

## Q5 — Tool construction

Production mapping is `compute::cutter::build_cutter`
(`crates/rs_cam_core/src/compute/cutter.rs:8-53`): `end_mill` →
`FlatEndmill::new(diameter, cutting_length)`; `v_bit` →
`VBitEndmill::new(diameter, included_angle, cutting_length)`;
`tapered_ball_nose` → `TaperedBallEndmill::new(diameter, taper_half_angle,
shaft_diameter, cutting_length)` — argument order per
`tool/tapered_ball.rs:41-46`: `(ball_diameter, taper_half_angle_deg,
shaft_diameter, cutting_length)`. Each then gets `cutter.helix_deg = tool.helix_deg`.

| toml id | name / T# | constructor call | envelope r (mm) |
|---|---|---|---|
| 0 (`:76-97`) | 6mm 2F Carbide End Mill, T1 | `FlatEndmill::new(6.0, 25.0)` | 3.0 |
| 1 (`:99-120`) | R0.5 tapered ball, T6 | `TaperedBallEndmill::new(1.0, 7.1, 6.0, 20.0)` | 3.0 |
| 2 (`:122-143`) | R1.0 tapered ball, T8 | `TaperedBallEndmill::new(2.0, 5.7, 6.0, 20.0)` | 3.0 |
| 3 (`:145-166`) | R1.5 tapered ball, T15 | `TaperedBallEndmill::new(3.0, 2.8, 6.0, 30.5)` | 3.0 |
| 5 (`:191-212`) | 20 deg V-bit 5.5mm 2F, T20 | `VBitEndmill::new(5.5, 20.0, 15.0)` | 2.75 |

Envelope: `envelope_radius_mm()` = `radius()` = `diameter()/2`
(`tool/mod.rs:252-267`); `TaperedBallEndmill::diameter()` returns
`shaft_diameter` (`tapered_ball.rs:102-104`), hence 3.0 for all tapers;
`VBitEndmill::diameter()` returns `diameter` (5.5 → 2.75). `height_at_radius`
is total (Some) across the full envelope for all five: flat `Some(0.0)` for
`r <= radius` (`flat.rs:67-73`), tapered ball ball-then-cone for
`r <= shaft_radius` (`tapered_ball.rs:147-163`), V-bit `r/tan(half)` for
`r <= radius` (`vbit.rs:113-119`). Tool 4 (R2.0, T10, `:168-189`) is defined
but unused by any toolpath in this project. Files use: setup 1 = T1, T20, T8;
setup 2 = T1, T15, T6 (LOAD/TOOL CHANGE comments, this recon).

## Q6 — Setup 2 initial stock

Setup 2's first op **"6 3D Rough (front)" uses `stock_source = "fresh"`**
(`wanaka200.toml:711`) — it does NOT see Setup 1's removals. Stock thickness
**z = 25.0** (`:10`). All 8 ops:

| setup | toolpath | stock_source |
|---|---|---|
| 1 | 1 Pin Drill (`:266`) | fresh |
| 1 | 2 Back Rough (`:354`) | fresh |
| 1 | 3 Holes (6mm pilot) (`:458`) | fresh |
| 1 | 4 Rivers (back, V-bit) (`:540`) | from_remaining_stock |
| 1 | 5 Lakes (back) (`:620`) | from_remaining_stock |
| 2 | 6 3D Rough (front) (`:711`) | **fresh** |
| 2 | 7 3D Finish (R1.5 drop_cutter) (`:817`) | from_remaining_stock |
| 2 | 8 Pencil detail (R0.5) (`:903`) | from_remaining_stock |

Implication for the replay: back-side (Setup 1) removals reach at most
`depth 12` holes + `spoilboard_penetration 2` through-pin-drills + back rough
(depth_per_pass 4.2, stock_to_leave_axial 4.0) into the 25 mm board. Front-side
rapid floors were computed against fresh 25 mm stock, so where Setup 1 already
removed material the front rapids are merely conservative (air where the model
said material) — the safe direction. The dangerous direction (material present
where the emitter's model says absent) can only come from within each file's
own chain, which is what the replay measures per file.

## Q7 — Print opt-in and skip-if-missing patterns

`crates/rs_cam_core/tests/power_ceiling_parity_f2.rs` — the file opens with a
long `//!` doc comment (`:1-83`); the opt-in attribute block sits at `:84-90`
(NOT the first 5 lines):
```rust
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::print_stderr
)]
```
(`thin_organic_island_widths.rs:63-68` is the same shape with
`clippy::print_stdout`.)

Skip-if-missing guard, `crates/rs_cam_core/tests/thin_organic_island_widths.rs:244-252`
(mesh const `WANAKA_MESH = "/home/ricky/Downloads/wanaka200/rivmap_export/terrain.stl"` at `:86`):
```rust
#[test]
#[ignore = "evidence run — needs the operator's wanaka mesh (not in repo)"]
fn wanaka_tier_and_band_region_widths() {
    let path = Path::new(WANAKA_MESH);
    if !path.exists() {
        println!("SKIP: {WANAKA_MESH} not present on this machine.");
        return;
    }
```

## OPEN items

None blocking. Two soft notes: (1) the LinearModal F-elision decision site was
cited via `modal.rs` state only, not walked end-to-end (empirics from both .nc
files confirm the grammar); (2) Setup 1's per-phase heights (`mode = "auto"`
throughout) were verified against emitted motion + `effective_safe_z` call
sites rather than by walking every heights-resolution branch.
