# S2 recon — why `rapid_collision_count` was silent (2026-08-28)

> READ-ONLY recon at HEAD `50c8db55`. No cargo run; every claim is a code read
> with file:line citations. The headline is uncomfortable and stated plainly:
> **by code read, the HEAD detector should already flag the S1 class.** No
> mechanism found in the current checker explains the recorded zero. The
> falsification run (Q7) is therefore the first S2 action, not the last.

## Q1 — timing / stock state (lead hypothesis): timing is CORRECT — (a)

Call seam, `crates/rs_cam_core/src/compute/simulate.rs:1018-1027`:

```rust
// Check rapid collisions against the *current* stock state
// (after all previous toolpaths, before this one carves).
{
    let rapids =
        check_rapid_collisions_against_stock(entry_toolpath, &group_stock.z_grid);
```

Ordering inside the per-entry loop (`simulate.rs:994-1099`): pre-carve
snapshot `Arc::new(group_stock.clone())` at `:1010` → **collision check
`:1020-1027`** → stamping afterwards (`apply_drill_op` `:1045`, or
`simulate_toolpath_with_lut_metrics_cancel` `:1089`). So op 7 is checked
against the post-op-6, pre-op-7 stock — option **(a)**, and it has been since
the site was introduced (`eaa56158`, unchanged through `1d886be6`/`090959b7`
per `git log -L`). The check is a **batch walk against one frozen snapshot**,
not interleaved with stamping — `collision.rs:433-435`: "The grid passed in is
a *snapshot* — it is not updated as the toolpath progresses". That frozenness
is why the F3/ascent heuristic carve-outs exist (false-positive suppression
for the op's own just-cut columns), but for the S1 class the snapshot is the
RIGHT stock: op-6's 0.5 mm leave + cusps ARE in it when op 7 is checked.

**Therefore hypothesis (1) of S1_RESULTS §3 is refuted at HEAD**: the class
would NOT vanish from timing. Walking the wanaka link through the code:
descent `G0 (168.2,220,12)→(168.2,220,2.211)` is `dz<0`, so the ascent skip
(`collision.rs:470-476`) does not apply; the F3 walk-back (`:483-511`) steps
over the same-XY hop rapid, hits the retract at a different XY
(`dxk*dxk+dyk*dyk >= 0.01` → `break None`, `:493-495`), so no exemption; the
sampler (`:517-521`) includes the endpoint (`for step in 0..=n_steps`), and
`2.211 < ~2.7` trips `pz < stock_top` (`:524-531`). **Code says ~601 hits.**

Residual explanations for the recorded zero (Q7 decides). **R1 (stored ≠
emitted) is now NEARLY CLOSED against itself**: the G-code exporter maps G0
strictly from `MoveType::Rapid` (`gcode/program_builder.rs:74-83` —
`MoveType::Rapid => push_rapid`, `Linear => G1`), so the shipped
`G0 … Z2.211` PROVES the stored toolpath holds a Rapid ending at the
resume-point Z. Caveat on reading the `.nc`: `push_rapid` **dog-legs a
diagonal falling rapid** into "traverse at the current height, then plunge
straight down" (`program_builder.rs:20-31, 52-56`), so the three G0 lines in
S1's excerpt may be TWO stored moves (vertical retract + one diagonal
falling rapid) — either shape is sampled to its endpoint by the checker and
flags. What remains genuinely OPEN inside R1: **no core emitter read here
produces that stored shape.** `raster_toolpath_from_grid`
(`toolpath.rs:607-747`) emits unlinked junctions as `Retract` rapid →
`Linking` rapid at `safe_z` → **fed** `EntryPlunge` (`:694-710`);
`relink_fragments`' retract arm likewise feeds the plunge
(`surface_link.rs:692-703`); TSP's `rebuild_group` emits only safe-Z rapids
and clones fed moves verbatim (`tsp.rs:466-524`, "**Cutting moves are still
only cloned. No fed move is synthesized**", `:460-462`); and
`dressup::optimize_entry_descents` always keeps a fed tail below its
`ceiling + PLUNGE_CLEARANCE_MM` rapid (`dressup.rs:180-216`) — its sole
production caller is the **viz worker**
(`rs_cam_viz/src/compute/worker/execute/mod.rs:906-914`). Something between
those and export turns the whole plunge into a rapid at zero clearance —
find it in the viz worker's transform chain (that file is locally modified,
uncommitted — mind that in any A/B). (R2) the airrun's "zero on all 8 ops"
(`RUN_LOG.md:16`) was a stale/early reading — note triage reads the RAW
`rapid_collisions` (`sim_triage.rs:299`), not the boundary-attributed
counts, so an attribution bug cannot silence both wires; a truly empty
checker return or a stale read are the only shapes left. (R3) resolution
under-read (Q2/AM10) — cannot eat a uniform 0.5 mm leave, only sub-cell
classes.

## Q2 — what the probe compares

`crates/rs_cam_core/src/collision.rs:450-544`, signature
`(toolpath: &Toolpath, z_grid: &DexelGrid) -> Vec<RapidCollision>` — **no
cutter, no radius, no tolerance, no pad**. Per rapid move: samples every
≤1 mm along the segment (`n_steps = (dist / 1.0).ceil().max(1.0)`, `:513-514`),
both endpoints included (`for step in 0..=n_steps`, `:517`). At each sample:
`z_grid.world_to_cell(px, py)` (XY only, cell-centre rounding, `None` outside
the grid — an out-of-grid rapid is **silently unchecked**, `dexel.rs:450-462`)
then a strict zero-radius point compare:

```rust
z_grid.top_z_at(row, col)
    .is_some_and(|stock_top| pz < f64::from(stock_top))
```

(`:524-531`). `top_z_at` is `ray_top` of the topmost dexel segment
(`dexel.rs:492-495`) — NOT `conservative_top_at` (`dexel.rs:512-519`), so
sub-cell slivers stay invisible (S-b proper). One `RapidCollision` per rapid
move (first hit breaks, `:516-540`) — the count is "rapids that collide",
not strike points. Exemptions: **(1) pure-vertical ascending rapids are
skipped entirely** (`dz > 0.0 && xy_dist_sq < 0.01`, `:470-476`) — so a fixed
detector inherits the S1-v2-proven-safe behaviour for Z-only retracts for
free, though the skip is asserted ("no stock can be present"), never proven
per-column; **(2) F3 same-XY descent re-entry** (`:478-511`): walks back
through same-XY rapids to the most recent non-rapid at that XY; if that feed
ended at/below the descent's end Z, skip — matches drill pecks, does NOT
match the wanaka retract→hop→descend links (hop XY ≠ retract XY). Descents
and traverses are otherwise treated identically. Resolution sensitivity is
real and measured: `tests/descent_resolution_stability_am10.rs:18-21` — "the
same generated chain measured **0 / 15 / 20** rapid collisions at
0.5 / 0.25 / 0.1 mm" (TP15 RCA), driven by `ray_blend_above` sub-cell
blending + the strict no-tolerance compare; that class is entry-descents
planned against a coarse snapshot, not the S1 link class.

## Q3 — the "misplaced volume" conflict: RESOLVED in favour of :130

`RUN_LOG.md:1243-1245`'s "the collision test ran against a misplaced volume"
points ("for the reason above") at **G-SIM-IDENTITY-FRAME**
(`RUN_LOG.md:60-135`): the **global playback/display stock** was zero-rooted
while identity-setup toolpaths stamped in world frame. But the rapid check
never reads that object — it reads `group_stock` (`simulate.rs:1022`), and
`RUN_LOG.md:130-133` says exactly that: "per-toolpath metrics, engagement and
collision checks all read `group_stock` … which is correctly framed per
setup. Only the display/playback stock is wrong." The code agrees at HEAD:
identity setups get `local_stock_bbox = None` (F-024/F-030,
`session/compute.rs:2481-2508`, `sim_local_stock_bbox()` returns `None`) →
`run_simulation` falls back to world-frame `request.stock_bbox`
(`simulate.rs:936-939`), the same frame the untransformed toolpath is stamped
and checked in. The playback-stock defect was fixed in-session (`6bcc3c97`,
2026-08-19); nothing since touched the check's query object — `git log` on
`collision.rs` shows nothing after `8a17cdd2` (W0.1), and the lateral-setups
commits (`686be424`, `65994496`) touched playback/scrub, not this seam.
**Verdict: at HEAD the rapid check's query frame is correct for identity
setups with non-zero stock origin; `RUN_LOG:1243` is a conflation and does
not explain the zero either.** (Note also: even a misframed grid would be
self-consistent between stamp and check — both use the same grid — except
for out-of-grid XY, where `world_to_cell → None` silently passes.)

## Q4 — producer and consumers

**Sole producer confirmed**: the only production call site is
`simulate.rs:1022` (all other refs are tests: `collision.rs:776-987` module
tests, `descent_resolution_stability_am10.rs:125`). Where the count lands:
- `SimulationOutputs.rapid_collisions` / `rapid_collision_move_indices`
  (`simulate.rs:376-378`, returned `:1382-1383`), snapshotted into
  checkpoints (`:1228-1229`) and the S5 **prefix cache**
  (`sim_prefix.rs:507-508` — a resumed prefix restores the collisions from
  when it was first computed; skipped entries `k < skip_entries`
  (`simulate.rs:994-999`) are not re-checked).
- Per-toolpath attribution by move-index-in-boundary
  (`session/compute.rs:3487-3534`) → `ToolpathDiagnostic.rapid_collision_count`
  (`:3605`) and the project total (`:3877`); serialized on both wire shapes
  (`session/mod.rs:1830, 1921`).
- Triage safety rows (`sim_triage.rs:261, 299`); diagnostic id
  `PROJECT_RAPID_COLLISION` (`diagnostics/ids.rs:37`).
- CLI `project` report: verdict branch on `rapid_collision_count > 0`
  (`rs_cam_cli/src/project.rs:494, 540-543, 568`); also `smoke.rs`.
- MCP `get_diagnostics` `per_toolpath` rows (TD3 B-5 core parity) and the GUI
  banner via `ProjectDiagnostics`.
- **Export gate: OPEN** — not verified whether anything blocks export on the
  rapid count (drill gates do block; the rapid count appears verdict-only in
  everything read here). Check `export` gating before widening the detector.
- Note: `sim_measurability.rs:134` names a `RapidCollision` metric, but the
  count wire traced above does not consult measurability — it gates triage
  presentation, not the count. Not a silence candidate.

## Q5 — the fix seam

At `simulate.rs:1020-1032` the seam already has everything: `entry`
(`SimToolpathEntry` — `entry.tool` is the boxed `MillingCutter` from
`build_cutter`, `session/compute.rs:2425`), `entry_toolpath`, `group_stock:
TriDexelStock`, and four lines BELOW the check: `let lut =
RadialProfileLUT::from_cutter(&entry.tool, LUT_SAMPLES); let radius =
entry.tool.radius();` (`:1031-1032`) — hoist those two above the check and
pass them in. The clearance primitive exists:
`TriDexelStock::max_clearance_tip_z_for_profile` (`dexel_stock/mod.rs:486`),
already validated by S1's instrument. Minimal fix: change the signature to
take `&TriDexelStock` + cutter (or LUT) + radius and replace the point
compare with `pz < max_clearance_tip_z_for_profile(px, py, radius, cutter)`
per sample (this also swaps in `conservative_top`-based reads, closing S-b's
sliver blindness). The **replay enumerator exists**:
`dexel_stock/simulation.rs:99 fn replay_moves` (called from `:70` and
`:649` — "Since SIM w6 this is `Self::replay_moves` with a never-firing
cancel"); riding it gives (i) true per-rapid timing — each rapid evaluated
against the live stock mid-toolpath — making the F3/ascent carve-outs
correctness-redundant (keep as perf skips only). Adapter needed: OPEN —
`replay_moves` visibility/shape unread; it needs a rapid-move arm (it stamps
feeds today) or a per-move callback, and the detector's collision output must
then be produced inside `simulate_toolpath_with_lut_metrics_cancel`'s walk
instead of the separate pre-pass. Module tests at `collision.rs:776-987`
build bare `DexelGrid`s and will need `TriDexelStock` fixtures or a shim.

## Q6 — sentry landscape

Direct pins on the detector: `collision.rs:776-987` module tests (vertical
retract exempt, diagonal flagged, F3 peck re-entry exempt, raster pattern
clean — all break on a signature change and must be migrated, not deleted);
`tests/descent_resolution_stability_am10.rs` — asserts count is
resolution-stable AND `== 0` (`:201-216`): a conservative/profile-aware
detector will likely flag its coarse-planned descent at ALL resolutions →
**red-as-evidence (M2-style)** until S3 fixes `optimize_entry_descents`;
`tests/rapid_collision_detector_population.rs` (unread — OPEN what it pins);
`tests/end_to_end.rs:485` (a positive-detection sentry — comment credits the
check with catching a regression). Files referencing `rapid_collision` that
may assert zero on link-bearing finish fixtures (each is a "green turns red"
candidate — OPEN, needs a per-file pass): `swept_wanaka_ab_s1.rs`,
`p1_headless_ab_wanaka.rs`, `smoke_baseline_regression_f037.rs`,
`perf_golden_sim_metrics{,_3d}.json` + `perf_golden_sim_metrics.rs` (golden
counts move if fixtures contain wanaka-class links), `air_cut_denominators_lh1.rs`,
`strategy_comparison_h4.rs`, `checkpoint_b_resolution_ab.rs`,
`{pocket,adaptive3d,vcarve}_lift_bridge_b1.rs`, `scallop_intra_pass_relink_am7.rs`,
`sim_prefix_memo_s5.rs`, `adaptive_feed_modulation_pipeline_f036b.rs`,
`classification_columns_ab_m3.rs`, `adaptive3d_planner_stock_xy_f027.rs`,
`face_stock_top_frame_f028.rs`, plus S1's own
`rapid_replay_shipped_gcode_s1.rs` (the phase's falsification instrument).
CLI unit fixture asserts a count of 2 (`project.rs:885, 924`) — unaffected.

## Q7 — falsification path

Cheapest real-pipeline read:
`cargo run -p rs_cam_cli --release -- project planning/airrun_2026-08-19/wanaka200.toml`
— the `project` subcommand "Loads the GUI project TOML … executes all"
(`rs_cam_cli/src/main.rs:146-153`) and its report prints per-toolpath
`rapid_collision_count` in both the human verdict ("N rapid moves pass
through stock", `project.rs:540-543`) and the JSON block (`:568, :924`).
OPEN: exact flag names for simulation resolution / fixpoint on that
subcommand (`main.rs:153-248` shows overrides exist; run
`cargo run -p rs_cam_cli -- project --help` first). The project has four
`from_remaining_stock` ops (`wanaka200.toml:540, 620, 817, 902`), so the run
must resolve the chain in order with real simulated prior stock — the CLI
executes ops in plan order, but confirm it simulates between generates
(fixpoint semantics) rather than generating flat. Mind the standing rule:
one cargo job at a time, check `free -g` first. **Decision table**: HEAD
reports ~601 for op 7 → the silence was historical/stale and S2 reduces to
the profile/conservative_top upgrade; HEAD still reports 0 → the divergence
is stored-vs-emitted motion (R1): dump the stored moves around one op-7 link
(or point S1's instrument at `result.annotated().toolpath` instead of the
`.nc`) and diff against `wanaka200_2_Setup_2___front.nc:447069-447073`.
