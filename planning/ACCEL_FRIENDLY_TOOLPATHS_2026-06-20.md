# Accel-Friendly Toolpath Conditioning

**Status:** spec / not started · **Branch:** `experiment/adaptive-spiral` · **Authored:** 2026-06-20

**Goal:** make dense 3D and contour-spiral toolpaths actually *runnable* — and fast —
on a low-acceleration Grbl/Shapeoko machine. Condition the toolpath **geometry**
(fewer, longer moves; real arcs; smooth corners), not the feeds. One path
(contour spiral) currently **errors the controller** when run; that correctness bug
comes FIRST, before any speed work.

Cross-refs: `planning/UNIFIED_LOAD_MODEL_2026-06-18.md` (§4/§11 confirmed physics),
memory `project_unified_load_model`, `project_spiral_wallclock_structural`,
`project_strategy_advisor`.

---

## 1. Why this exists (the discovery)

We shipped a per-toolpath "operating point" readout (unified load model, steps 1–6).
Running it on the real **wanaka** project (`planning/airrun_2026-06-01/wanaka.toml`,
Shapeoko Pro XXL, `terrain.stl`) revealed that **every 3D cutting pass is entirely
acceleration-bound**:

- binding constraint = **kinematic-reach** on 99–100% of cuts
- feeds pulled DOWN 66% (rough) to 90% (finish)
- deflection idle (≤25 µm vs 200 µm limit), power idle (≤4%)

The machine literally cannot accelerate to the commanded feed on the short 3D moves.
Feeds can't fix this — the **path** has to get cheaper to accelerate through. This
reverses the old "wanaka is deflection/tool-limited" framing (that was an artifact of
the pre-step-2 inflated force model).

The user also reported the **contour-spiral** strategy is "very acceleraty" and slow,
and that **"the machine just errored"** when they ran it.

## 2. The physics (one number, then the corners)

**Segment-length floor.** To reach feed `F` on acceleration `A`, a move must be at
least `L_min = F² / (2A)` long (the distance to ramp from 0 to F). On the Shapeoko
(`A ≈ 350 mm/s²`, target `F = 4000 mm/min = 66.7 mm/s`):

```
L_min = 66.7² / (2 · 350) ≈ 6.3 mm
```

Our 3D / spiral paths emit ~0.5–1.5 mm segments. Each one tops out at the triangular
profile peak `v_peak = √(A · L) ≈ √(350 · 1.0) ≈ 590 mm/s²-derived ≈ 1100 mm/min` and
**never reaches the commanded feed**. The optimizer correctly reports this as
"kinematic-reach" binding and feeds it down.

**Planner starvation.** Grbl looks only **16 blocks** ahead. A wall of sub-millimetre
segments fills the planner buffer with moves that each decelerate to a near-stop at
their far end, so the look-ahead can never build a sustained high-velocity plan even
where geometry would allow it. Fewer, longer moves = deeper effective look-ahead.

**Cornering cap (junction velocity).** Grbl's junction-deviation model caps speed
through a corner at:

```
v_junction = √(A · R),   R = δ · sin(θ/2) / (1 − sin(θ/2))
```

where `δ` = `$11` junction deviation (default 0.01 mm) and `θ` is the *included* angle.
A near-reversal (θ→0) gives R→0 → full stop. A gentle bend keeps speed up. Smoothing
tight corners into arcs / blends is what keeps `v_junction` off the floor.

**Arcs are a bandwidth win, not magic.** Grbl supports only G0/G1/G2/G3 (no splines)
and **re-tessellates** G2/G3 internally by `$12` (arc tolerance, default 0.002 mm).
So an arc becomes many tiny internal segments anyway — the win is fewer blocks *over
the wire* (buffer/look-ahead headroom) and a single smooth feed, **provided we advise
a looser `$12` (~0.01–0.02 mm)** in the post/setup sheet. Arcs do not bypass the
segment-length physics by themselves.

## 3. What the engine already has (verify before building — don't reinvent)

| Capability | Where | State |
|---|---|---|
| RDP simplify (3D) | `crates/rs_cam_core/src/toolpath.rs:323` `simplify_path_3d(points, tolerance)` | active for adaptive3d (`clearing.rs:~465`) via `tolerance` (default 0.1 mm) |
| RDP simplify (2D) | `crates/rs_cam_core/src/adaptive/path.rs:~1433` `simplify_path` | active for 2.5D adaptive (`path.rs:~1514`) |
| Arc fitting → real G2/G3 | `crates/rs_cam_core/src/arcfit.rs:34` `fit_arcs(annotated, tolerance, tool_radius)` (Kåsa LSQ + `$12` endpoint correction + sagitta guard + radius cap) | called in dressup pipeline `compute/execute.rs:~1955` (`if cfg.arc_fitting`); ON by default for Roughing/SemiFinish/Finish (`config.rs:~410/417/424`, `arc_tolerance` 0.05). **Only fits constant-Z XY runs → 3D Z-varying paths stay dense G1. KEY GAP.** |
| Corner blend (2D, native arc) | `adaptive/path.rs:~1516` `blend_corners_to_moves` | gated `min_cutting_radius`, **default 0.0 (OFF)** |
| Corner blend (3D, linearized) | `adaptive3d/search.rs:~228` `blend_corners_3d` (`clearing.rs:~466`) | linearized, not real arcs; `min_cutting_radius` default 0.0 |
| Kinematics | `crates/rs_cam_core/src/machine_kinematics.rs` `MachineKinematics { acceleration_mm_s2, jerk_mm_s3: Option, max_junction_velocity_mm_min: Option }` | `junction_velocity` (`:468`) is a **dot-product** model, NOT Grbl junction-deviation. Presets: shapeoko_xxl_stock 250, generic_wood_router 200, shapeoko_xxl_ricky_tuned 350 |
| Contour spiral | `crates/rs_cam_core/src/adaptive/spiral.rs` | EDT → iso-contours → resample at `WRAP_SAMPLE_CELLS=1.5` cells (`:41`) → trochoid loops (`emit_trochoid_loop`, 20-pt 360°). Knobs: `trochoid_cap_mult` (1.6), `min_cutting_radius` (0.0) |
| G-code emit | `gcode/emitter.rs`, `gcode/post.rs` | passes ArcCW/ArcCCW as G2/G3 verbatim; `ArcLinearize` safeguard off (threshold 0.05). No conditioning at emit |

**MISSING entirely:** minimum segment-length floor · max-direction-change handling ·
Grbl junction-deviation model · 3D/helical arc fitting.

## 4. Phased plan (correctness first, then impact-ranked)

### Phase 0 — Diagnose the contour-spiral machine error — **CLOSED 2026-06-21**
**Conclusion: NOT a G-code defect.** The user confirmed the failure was a controller
**ALARM (limit) caused by the spiral accelerating too hard**, and it predates the
recent feed-modulation / kinematic-reach work (which has likely already stopped the
hard alarm). So there is no malformed-G-code bug to fix — the failure mode is the path
being too aggressive for the low-accel machine to physically track. The cure is the
structural work below (Phases 1-2 geometry conditioning + Phase 4 realistic junction
model), not a correctness patch.

Evidence (sentry `tests/contour_spiral_gcode_validity_phase0.rs`): drove the real
`fit_arcs(tol 0.05) → emit_gcode(grbl)` path on contour-spiral output and validated the
emitted text against GRBL's rules. All clean:
- arc endpoint radius consistency (`$12`/`error:33`): worst mismatch **0.0001 mm** ≪
  0.010 limit (and arcs barely fire on the 2D spiral, 0-2);
- undefined feed (`error:22`): feed is modal, first cut feed valid, none undefined;
- non-finite coords: none.

The one real signal it surfaced is **density**: 5-12% of cut moves are below the
accel-ramp length even at 1500 mm/min (at wanaka's 4000 mm/min `L_min`≈6.3 mm, so the
fraction explodes). That is the Phase-1 target; the sentry now doubles as the density
gauge (watch `short_cut` %% drop as conditioning lands).

Also observed while reproducing: contour-spiral generation self-times at ~9s but the
GUI/MCP `generate` stalls **40-50 min** — entirely *downstream* of compute (step-6b
sim/modulation or debug-trace assembly choking on the dense path). Tracked as a
separate perf bug; Phases 1-2 (which shrink the path) should relieve it. Revisit if it
persists after conditioning.

### Phase 1 — Segment merge (conditioning) — **LANDED 2026-06-21**
Toolpath-conditioning pass `condition::merge_linear_runs(at, tolerance)`: per maximal
run of consecutive **same-feed linear cut moves** (never crossing a `RapidOrderBarrier`
/ `DepthPass`), RDP-simplify at a conditioning tolerance so every dropped point stays
within `tolerance` of the retained chord. Span-aware exactly like `arcfit::fit_arcs`
(remaps spans through the N-to-M collapse). Slotted into `apply_dressups` right **after
arc-fit** (curves become G2/G3 first; this cleans the residual linears).

- Reused `simplify_path_3d` by extracting `simplify_path_3d_keep_mask` (so survivors map
  back to source moves for feed/intent) — byte-identical to the old RDP.
- `DressupConfig.segment_merge` (bool, `#[serde(default)]` false) +
  `segment_merge_tolerance` (0.3 mm). **Default on for the Roughing role only** — finish
  leaves ≈0 stock, roughing leaves ≥0.5 mm so a 0.3 mm merge never touches the surface.
  Follows the same per-op-serialized convention as `arc_fitting`: **new** roughing ops
  get it; **existing serialized projects keep their saved value** (opt-in by re-creating
  the op or toggling the field — e.g. via MCP `set_dressup_field`). This is why the
  wanaka cycle-time number is unchanged by the flag (the loaded back-rough op has the
  field absent → false).

**Why a pure floor isn't enough on its own (measured):** RDP at the generation tolerance
(0.1 mm) merges nothing; at 0.3 mm sub-ramp density drops 5-12% → 2-4% and block count
4-15%; at 0.5 mm → 0-1% / 16-40%. Genuine curvature can't be straightened without
exceeding tolerance — that's Phase 2's arc-fitting job. On the 2D spiral arc-fit barely
fires today (0-2 arcs), so merge is the active lever until Phase 2.

**Verified:** sub-ramp 58-79% lower on the spiral sentry (square 38→16, star5 92→19,
star7 81→32) with G-code still GRBL-valid; 19 `condition` unit tests + the Phase-1
sentry green. Full `rs_cam_core` suite (1901 lib + 80 integration targets) shows **zero
new failures** — the only two reds (`wanaka_suggest_baseline`,
`modulated_cycle_time_prediction_within_25_percent_of_machine`) **fail identically on
clean HEAD** and are pre-existing unified-load-model drift (deflection recalibration /
modulation-through-shared-model), needing the user's re-bench — out of Phase-1 scope.

Sentries: `condition::tests::*` (merge/corner/feed-boundary/barrier/span-invariants),
`tests/contour_spiral_gcode_validity_phase0.rs::phase1_merge_cuts_subramp_and_stays_grbl_valid`.

### Phase 2 — Helical arc fitting — **LANDED 2026-06-21**
`arcfit::fit_arcs` no longer splits runs on Z change. `try_fit_arc` now accepts a
Z-varying run when it is a true **helix** — XY points on the circle (as before) AND Z
linear with cumulative swept angle within `tolerance` (GRBL interpolates Z linearly over
a G2/G3 arc, so a helix emits as one `G2/G3 … Z… I… J…`). Constant-Z runs pass trivially
(unchanged). Non-helical Z wander (terrain contours, ramp jitter) is rejected → falls
back to linear, so no bad arcs. The emitted arc carries the run's **end Z**.

**Why it matters (measured):** the guaranteed beneficiary is the **helix entry** —
`dressup::emit_helix` emits **36 linear G1s per revolution** (10°/step, ~0.35 mm each at
r≈2 mm), i.e. every roughing plunge is a wall of sub-ramp segments. The structural test
(`test_fit_arcs_helix_entry_structure`, replicating emit_helix's center-anchor + orbit +
return) confirms the greedy fitter recovers from the off-circle anchor and collapses a
73-move helix to <36 — a couple of G2/G3 helical arcs instead of dozens of G1s. Clean
helical spiral descents benefit likewise. (Spiral *wraps* within a Z-level were already
constant-Z; their offset-contour geometry isn't circular, so arc-fit still mostly skips
them — that's Phase 3's `min_cutting_radius` job, not this one.)

Reused `simplify_path_3d_keep_mask` is unrelated here; the change is entirely in
`arcfit.rs` (run detection + helix check + end-Z emit). **Verified:** 23 `arcfit` unit
tests incl. `test_fit_arcs_helix_descent` / `_entry_structure` / `_rejects_nonlinear_z`,
existing `test_fit_arcs_different_z_breaks_arc` still green. Full `rs_cam_core` suite
(1904 lib + 80 integration targets) shows **zero new failures** — the more-aggressive
arc-fit broke no fingerprint/move-count sentry; only the same 5 pre-existing reds remain.

TODO (deferred): emit a recommended `$12` (~0.01–0.02 mm) in the post / setup sheet so
the controller's arc re-tessellation matches; and consider `emit_helix` emitting native
arcs directly (sidesteps the off-circle-anchor recovery). Neither blocks the win.

Sentries: `arcfit::tests::test_fit_arcs_helix_descent` / `_helix_entry_structure` /
`test_fit_arc_rejects_nonlinear_z` (22 `arcfit` unit tests total).

### Phase 3 — Default `min_cutting_radius` ON for the spiral *(~0.3× tool radius)*
Smooths tight inner-wrap corners into native G2/G3 (2D path) so `v_junction = √(A·R)`
isn't ≈0. Sensible non-zero default for contour-spiral roughing; verify it composes
with Phases 1–2.

### Phase 4 — Grbl junction-deviation kinematic model
Replace the dot-product `junction_velocity` with `v = √(A·R)`,
`R = δ·sin(θ/2)/(1−sin(θ/2))`, `δ` from a new `junction_deviation_mm` field on
`MachineKinematics` (default ~0.01). Makes the operating-point card's predicted feeds +
F-039 modulation match the real Shapeoko, and makes "kinematic-reach" honest.
Re-baseline F-034/F-035 cycle-time sentries (this changes predicted feeds).

## 5. Verification (the measurement loop)

The **operating-point card is the instrument.** After each phase, sim wanaka via the
rs-cam MCP (`load_project wanaka.toml` → `run_simulation` → `get_tool_load_report`) and
watch the **kinematic-reach binding fraction DROP** and **per-toolpath wall-clock**
(cut-trace summary) fall on the 3D Rough / Back Rough / contour-spiral paths. Add
sentries that pin: segment-length floor honored, arc emission on 3D paths, spiral
G-code validates, and the junction model matches the closed-form `v = √(A·R)`. Use a
real fixture (the f036b AS001 pocket harness, or a small terrain) — not just unit math.

## 6. Prior art (for citation when implementing)

- **Grbl** `planner.c` / `gcode.c` — 16-block look-ahead, trapezoidal planner,
  junction-deviation cornering (`v=√(a·R)`), `$11` junction deviation, `$12` arc
  tolerance, G2/G3-only motion.
- **LinuxCNC G64 P/Q** path-blending / naive-cam-detector — tolerance-bounded
  trajectory smoothing across short segments.
- **Fusion 360 / HSMWorks "Smoothing"** — fits arcs/splines to linearized 3D toolpaths
  within a tolerance to cut block count.
- **ArcWelder** (Prusa/Marlin ecosystem) — post-hoc G1→G2/G3 arc consolidation;
  reference for tolerance + endpoint-correction handling.

## 7. Hard constraints (these have crashed the PC / broken builds)

- **ONE cargo job at a time.** Before every cargo launch: `free -g` +
  `pgrep -af "[c]argo (build|test)"`. Concurrent heavy cargo has hard-crashed this PC
  3×. Sub-agents must NEVER run cargo. Prefer `-p rs_cam_core -j2`.
- **Zero-warning clippy** (16 deny lints, see CLAUDE.md). Clean before each commit.
- **Run integration `--test` sims** after physics/kinematics changes, not just `--lib`
  (Phase 4 especially).
- **Consolidate, don't patch** — extend the existing dressup pipeline / `arcfit` /
  kinematics; don't bolt on parallel flows. No worktree isolation (central files).
- `cargo fmt` cascades to `strategy_advisor.rs` + `strategy_advisor_smoke.rs`
  (pre-existing drift) — commit ONLY your files; `git checkout --` those two.
- Commit each phase separately; end messages with
  `Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>`.
- Baseline `cargo test -p rs_cam_core --lib` ≈ 1900+ pass / 3 pre-existing adaptive3d
  fails (peck_plunge, rapid_segment_lifts, planner_sim_dexel_parity) — ignore those.
