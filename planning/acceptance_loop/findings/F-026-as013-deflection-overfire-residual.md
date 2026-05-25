# F-026 — Auto-from-model dexel grid doesn't extend above stock_top_z (was: AS013 deflection over-fire residual)

- **Stage:** sim
- **Severity:** **high** (was medium; round-05 confirmed wider scope)
- **Status:** landed (scope-trimmed; rapid-collision part of round-05 signature resolved, deflection residual carved out as F-027)
- **First found in:** round-04 (2026-05-25); confirmed round-05 (2026-05-25)
- **Effort:** S–M (same family as F-024; one bbox decision in core
  + likely a mirror in viz worker per the round-04 three-rebuild-saga)
- **Linked PRs:** —
- **Implementer note (2026-05-25):** investigation while landing found
  the round-05 audit's diagnosis ("dexel grid Z range stops at
  stock_top_z") was the **wrong layer** — the actual fix is at
  `ProjectSession::load`, not the simulator. With `auto_from_model =
  true` and stale TOML `stock.z`, the load path was using verbatim
  TOML values; F-024's identity-setup fallback then used a stock_bbox
  truncated to those stale dims. Reapplying `update_from_bbox` after
  models load (mirroring `add_model`'s runtime behaviour) restores the
  documented invariant and fixes the AS013 rapid-collision count
  cleanly (844 → 0). The deflection-Exceeds residual is a separate
  bug at the adaptive3d/simulator XY boundary — carved out as F-027
  with concrete repro evidence.
- **Source audits:** round-04 F-024 verification probe; round-05
  sweep AS013 + AS015 + AS004

## Resolution of the round-04 stub

Original stub posited three candidate root causes. **Round-05 smoke
confirmed candidate 2** (auto-from-model Z-frame anomaly) and
**rejected** candidates 1 (genuine overload) and 3 (wrong feed):

- Hotspot probe on AS013 (the cut_trace highest-volume hotspot, sample
  range 68755-315994, covering the deflection-triggering sample at
  297050) reported `peak_axial_doc_mm = 25.009` on a 6 mm endmill, at
  `representative_position = [86.5, 5.0, 32.375]`. **The cutter is at
  Z=32.375, above stock_top_z=30.** Stock spans world Z=[0,30] (auto-
  from-model, terrain extends to Z=52.6).
- A sibling hotspot in the same toolpath at
  `representative_position = [91.86, 5.0, 23.375]` (cutter inside the
  grid Z range) correctly reads `peak_axial_doc_mm = 3.0` — the
  commanded `depth_per_pass`.
- AS015 (scallop on the same terrain) also reads deflection 0.434 Exceeds
  and 52 rapid collisions — same root cause, different operation.
- AS004 (face on auto_from_model MDF plate, stock.z=12, model.z=10)
  reads `peak_axial_doc_mm = 9.14` on a 0.5 mm DOC pass, deflection
  0.204 Exceeds, 24 rapid collisions. **Auto_from_model class, not
  terrain-specific.**

## Root cause

The F-024 fix made identity setups use `request.stock_bbox` (world
frame) as the dexel grid bounds, replacing the broken zero-rooted
local bbox. For ordinary projects where `stock.z` fully contains the
toolpath Z range, that's enough.

For **auto_from_model** projects where the model has features above
`stock_top_z` (a terrain mesh peaks above the stock surface; a STEP
plate has stock padding above model top), the toolpath legitimately
emits cuts at world Z > stock_top_z (clearance moves above terrain
peaks, lead-in moves, retract-to-safe-Z transits). When the dexel
grid stops at `stock_top_z`, samples with the cutter above the grid
mis-register: the F.a sub-cell stamping (gated on coverage ≥ 0.95
per CLAUDE.md Step 4) treats the rays as material to be cleared even
when the cutter is far above any actual material, and `axial_engagement_mm`
reads the cutter-to-stock-top distance.

The mechanism is the **inverse** of F-024:
- F-024: cutter below the grid → every ray cleared → axial reads full
  stock height.
- F-026: cutter above the grid → every ray "still uncleared" → axial
  reads cutter-to-grid-top.

## Files

Primary fix site (per F-024 implementer's investigation notes from
round-03):
- `crates/rs_cam_core/src/session/compute.rs` — identity-setup branch
  that returns `local_stock_bbox = None`. Currently relies on
  `request.stock_bbox` (world). Needs to widen Z range to include the
  model bbox max Z when `auto_from_model = true`.
- `crates/rs_cam_core/src/compute/transform.rs::effective_stock_bbox`
  — alternative fix site if the bbox should be computed upstream.

Mirror sites (per round-04 three-rebuild-saga learning — must be
checked, may all share the same bbox source after F-024):
- `crates/rs_cam_viz/src/compute/worker/execute/mod.rs::build_core_simulation_request`
  — viz worker path; F-024 added an identity-setup conditional here.
- `crates/rs_cam_viz/src/controller/events/simulation.rs::build_world_stock_bbox`
  — viz controller helper; F-024 third-site fix.

## Fix shape

The dexel grid Z range needs to span `[stock_bbox.min.z,
max(stock_bbox.max.z, model_bbox.max.z) + small_margin]` for
auto_from_model setups. Equivalent framings:

- **Option A**: compute `effective_dexel_bbox` that extends the
  stock world bbox up to enclose all model max Z (and a clearance
  margin matching the toolpath generator's safe-Z policy).
- **Option B**: at toolpath generation, retract-to-safe-Z samples
  should be tagged `MoveIntent::Retract` (per Step 1 of the dexel-
  fidelity roadmap, 2026-05-19) and excluded from cutting metrics.
  This is a different shape — it fixes the measurement rather than
  the grid, but covers retract-only over-stock samples. Won't help
  cutting moves that legitimately go above stock_top.
- **Option A + Option B together** are likely needed — the auto_from_model
  case has BOTH retract moves above stock AND legitimate cutting moves
  above stock (clearing the terrain peaks).

## Acceptance test

**Must run through `ProjectSession::run_simulation`** to exercise the
production path (per round-04 loop learning, see
`rounds/round-04-2026-05-25/delta.md` "Loop process learning"):

1. Load `test_data/ux_3d_terrain.toml` (auto_from_model=true, model
   extends to Z=52.6 above stock.z=30).
2. Add adaptive3d on model_id=1 with baseline params (feed=2500,
   dpp=3, stepover=1.2).
3. Generate + simulate.
4. Assert: at the deflection-triggering hotspot (currently sample
   297050-297051, peak_axial_doc_mm=25.0), `peak_axial_doc_mm` is
   `<= depth_per_pass + 0.5 mm margin`.
5. Assert: `deflection.peak_mm < 0.2`.

Secondary regression test:
- Add face op on `test_data/ux_step_plate_mdf.toml` with baseline
  params from AS004 (depth=1, dpp=0.5, stepover=3.0, feed=2400).
- Assert `peak_axial_doc_mm <= 0.5 + 0.1` on the first pass.
- Assert `rapid_collision_count == 0`.

## Risk

S–M.

- S if the fix is purely "extend grid bbox to enclose model" (one
  source-of-truth function in `session/compute.rs` if F-024's identity-
  setup conditional already routes through there).
- M if the fix needs to mirror to viz worker and controller (per
  the round-04 three-rebuild-saga; may or may not be needed depending
  on whether `request.stock_bbox` reaches those paths via the same
  source).

## Notes

- **Cross-link**: F-017 (rapid collisions) is now a clean duplicate
  of F-026 for AS013-class and likely all the round-01 collision counts
  on auto_from_model ops. Recommend closing F-017 when F-026 lands and
  round-06 verifies.
- **Cross-link**: F-025 (non-identity setups, face_up=Bottom etc.) is
  a separate sibling — same family of Z-frame issue but for the local↔
  global transform path. Still open.
- **Sample evidence on AS013** captured 2026-05-25 in
  `target/acceptance_sweeps/agent_smoke_20260525_round05/results.csv`
  + cut_trace dump in
  `~/.claude-personal/.../tool-results/mcp-rs-cam-get_cut_trace-1779678551413.txt`.
- This finding subsumes the original "AS013 deflection residual"
  framing — same root cause, much wider blast radius.
