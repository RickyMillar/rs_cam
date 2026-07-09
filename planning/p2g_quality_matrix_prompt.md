# Handoff prompt — P2.g: win the fine-quality tier (collar fix + band economics)

Paste everything below into a fresh session.

---

Continue the unified finishing pass: P2.g. Read the 2026-07-09 entries in
`planning/finishing_stack_review_2026-07.md` and the P2.f rows in
`planning/unified_finishing_pass_plan.md` first. Branch
`experiment/adaptive-spiral`.

## Where P2.f ended (all committed, gates green)

Overnight + morning 2026-07-09, five fixes landed and were live-validated
in the GUI (wanaka via MCP, project NEVER saved):

1. Scallop **chord refinement** (8976401) — feed chords between exact
   ring points no longer behead sub-chord terrain.
2. **UnifiedFinish strip-all dressup policy** (2bd2718) — role-default
   ramp entries were carving 3° diagonal trenches on MCP-added ops.
3. **Region serpentine** (6540045) — segmented rasters stay down between
   adjacent rows.
4. **Tip-radius cusp math** (this session) — scallop spacing used the
   SHANK radius (Ø6) instead of the 1 mm tip: real cusp was 3× the dial
   on tapered tools. Now `cusp_radius()` (geometry_hint TaperedBall arm).
   Exactly correct for slopes < 90°−taper (~83°).
5. **Ring min-stepover** (this session) — ring spacing was the ring-MEAN
   of sampled stepovers; now the MIN, so the cusp guarantee holds at the
   steepest stretch of every ring.

Live parity was CONFIRMED: live entry_s/rapid_s match headless to 0.1 s
(150.3 / 940.7). One open live-only item: 5 rapid collisions inside the
live unified op (headless 0) at local moves 2413/42560/77235/80814/81854.

## The quality matrix (fidelity instrument, true dial h=0.011, tip-radius
## correct, `target/p2f_fidelity/matrix.log`)

| branch | finish s | vs A | mid-steep on-size | mid +0.01–0.05 leftover |
|---|---|---|---|---|
| A all-over raster 0.3 | 6883 | — | 29133 | 23752 (steep cusp ≈0.034) |
| B75 unified | 8855 | +28.6% | 29944 | 22538 |
| B65 unified + contour 65–75 | 11973 | +73.9% | 29686 | 22698 |
| D all-over scallop | 9390 | +36.4% | **43345** | **8186** |
| B yesterday (effective h≈0.033) | 5486 | −20.3% | ≈A parity | speed tier |

Verdicts already drawn (do not re-litigate):
- The unified op's −20% is real at the SPEED tier (steep ≈ A quality).
  At the FINE tier it currently loses to plain scallop D on quality and
  barely beats it on time. The user's skepticism was data-confirmed.
- Waterline/contour is DEAD on this terrain at any threshold (B65:
  +45 points time over B75, zero measured quality delta — its per-level
  plunge cycles cost 1606 s entry). Revisit only on a wall-heavy part.
- Big-tool→small-tool cascade (user idea) is REAL for shallows only:
  rest-share probe (`p2f_ball_rest_share_probe`): mid-steep rest share
  is 87–96% for every ball Ø2–6 (the flank is un-cascadable; the 1 mm
  tip owns it), but shallow rest share is 8.6–33.5% (Ø3 covers 89% of
  shallows; √R stepover + the tip currently runs 3.2× over its
  deflection-safe feed recommendation → ball carries feed legitimately).
  Parked as a shallow-band feature; planner classification is already
  tool-independent, `DerivedRestRegions` boundary already ships.

## Task 1 — the COLLAR FIX (top): make B75 match D's band quality

B75's mid-steep is diluted by the 2 mm `overlap_mm` collar: around every
dendritic region boundary the RASTER covers mid-steep cells at
raster-on-slope cusp (~0.034) instead of scallop's 0.011. On a dendritic
decomposition that collar is a large fraction of the band (D 43345
on-size vs B75 29944; the +0.01–0.05 leftover bin: 8186 vs 22538).
Candidates: (a) asymmetric overlap — the FINER strategy's regions keep
the dilation, the coarser one gets eroded at shared seams (finer owns
the seam); (b) shrink overlap_mm and measure seam integrity; (c) cut
order so the finer pass runs second where both cover. Acceptance: B75
mid-steep table ≈ D's, at meaningfully less than D's +36.4% time,
collisions 0, sweep anchor re-check.

## Task 2 — band economics: why is B75 only 6% faster than D?

B75's raster shallows should be much cheaper than D's rings on the same
ground (serpentine rows vs rings). Attribute finish time PER BAND
(Region spans exist on the unified toolpath; the instrument has band
ownership) and find where the expected raster win went — suspects:
dendritic per-region ring cascades (many tiny rings + entries), the
collar being double-cut, region count/min_area conditioning for the
r=1 tip. Fix what the data indicts. Expose `UnifiedFinishReport.route`
debuggably while in there (old Task 3 tail).

## Task 3 — product posture + tails

- Decide the shipping default: unified as the SPEED-tier default with
  the h dial honest (user picks fine tier explicitly), vs D-style
  all-over scallop for fine work. Update FEATURE_CATALOG honestly.
- Live tail: the 5 rapid collisions in the live unified op (headless 0).
- Old Task 3 remainder: crease/pencil integration into the op (pencil
  YES on creases — R2 pattern; contour NO), min_area floor for tapered
  tools, waterline sampling clamp (only matters if contour returns),
  offset_polygon root fix, C rim-guard, clip-aware serpentine for
  post-clipped rasters (branch A's silhouette clip re-fragments rows —
  census probe `p2f_a_move_census`).
- Then P3 morphed spiral — both the appearance problem (rings on
  dendritic regions look chaotic even at correct cusp) and the
  10× junction-physics time lever point there.

## Standing rules (unchanged)

- wanaka (`planning/airrun_2026-06-01/wanaka.toml`) LIVE-ONLY; never
  `save_project`; commit only when the user asks (overnight autonomous
  sessions: commit gate-green milestones, precedent P1/P2.f).
- Cargo: heavy jobs exclusive — `free -g` + `pgrep -f
  "bin/[c]argo|[r]ustc --crate-name"` (bracket a char) before launch;
  never workspace-wide `cargo test`; core lib needs `--no-fail-fast`
  past the 3 known reds (adaptive3d peck / rapid_segment /
  planner_sim_dexel_parity). Zero-warning clippy; fmt pre-commit.
- LONG RUNS: redirect to a file + Monitor; never pipe through `tail`.
- The harness is `tests/p2c_headless_ab_wanaka.rs`: pinned A
  (8919.5/6883.4 — still valid, A verified byte-identical through all
  P2.f changes), fidelity instrument (per-band histograms + deviation
  PNGs + f32 dumps in `target/p2f_fidelity/`), matrix branches
  (B/B65/D/C), probes (census, rest-share, offset-cascade, cost-curve).
- THE LESSON (now 4×): the user's eyeball beats averages — coastline,
  "identical" stocks, smoosh, coarse steep stepover. Per-band
  histograms before quality claims, always.

---

## STATUS UPDATE 2026-07-09 evening — Task 1 investigation (read this FIRST)

Task 1's premise is DEAD and partially resolved. The collar theory was
REFUTED by measurement (B-bad/D-good cells are interior — median 14 mm
from shallow seams, only 3.9 % inside the 2 mm collar — on a 1.41 mm
lattice = 2 classification cells). What the investigation established:

1. **The toolpaths are geometrically EQUIVALENT.** Bare-mesh calls,
   session-conditioned dumps (emission frame = mesh rot90:
   sess_x = 126.25 − mesh_y, sess_y = mesh_x + 21.25, z + 20), and
   ball-floor queries exactly at the bad cells with the correct 0.5 mm
   tip radius: B75 ≈ D within ~1 µm (re-verified on post-fix dumps).
   Real mid-steep ranking: B75 ≈ D at the 0.011 dial ≫ A at 0.034,
   with B75 6 % faster than D.
2. **Two REAL sim bugs found and fixed** (probe-proven, in tree):
   - `RadialProfileLUT` tip undersampling: dist²-uniform bins over the
     SHANK radius gave the Ø1 tip ~7 of 256 bins → tool read up to
     17.6 µm high at 70–75° ball-side contact. Fix: `LUT_SAMPLES=4096`
     (41 call sites) + sentry `tapered_tip_lut_error_bounded`.
   - Stamper z-blind subsegments: cells stamp at z(t_closest)+h(d),
     ignoring z-drop along ≤0.25 mm subsegments (production step is
     clamped `.max(0.25)`, compute/simulate.rs:450) → 5–35 µm
     under-removal on ~25 % of steep-finish columns. Fix:
     `MAX_SUBSEGMENT_Z_DROP_MM=0.02` z-aware subdivision
     (dexel_stock/simulation.rs). Proven by `p2g_stamp_probe`:
     B75 24.7 %→3.1 % of columns >5 µm above envelope, D 22.4 %→0 %.
3. **The fidelity-instrument gap PERSISTS anyway** (B75 mid-steep
   on-size 30 713 vs D 44 366 post-both-fixes; on-size + first
   leftover bin sums nearly equal → it's still a ~5–10 µm shift at
   the 0.01 edge). With op-8 geometry equal and isolated stamping now
   envelope-exact, the divergence enters between "op-8 stamps on
   fresh stock" and "full-chain measurement deviations". Suspects,
   in order: (a) roughing interaction (final = min(rough, finish);
   rough toolpaths are ladder-regenerated per branch), (b) the
   NON-IDENTITY per-setup frame — wanaka is a rot90 setup, and F-024
   explicitly left "zero-rooted effective bbox" frame consistency
   for non-identity setups unresolved, (c) the deviation/meshing
   step (dev vertices are a deterministic branch-independent grid —
   totals exactly equal at 119 334).
4. **Next probe (designed, not built)**: branch-context stock
   extraction — swap op, ladder, measurement sim, then read the
   sim's final stock top per column in the bad window and diff
   against the fresh-stock op-8 stamp + rough floor. Whichever step
   introduces the +5–10 µm for B75 is the mechanism. `p2g_stamp_probe`
   is the template; `p2g_session_op8_dump` provides move dumps.
5. **B75's residual real defects** (small): ~8 % of the mid band is
   ring-uncovered (cascade skips dendritic slivers; raster collar
   covers 78.6 % of that at raster quality); `set_toolpath_operation`
   does NOT apply the registry dressup policy on op swap (branch ops
   ran with inherited arc_fitting=true; same gap class as the F75
   ramp bug); B75 region-scallop generation 670 s vs D all-over
   405 s (Task 2 lead).

Probes added to the harness (all `#[ignore]`): `p2g_ring_dump`,
`p2g_session_op8_dump`, `p2g_measurement_aliasing_probe`,
`p2g_lut_error_probe`, `p2g_stamp_probe`. Artifacts in
`target/p2f_fidelity/`: `pre_lutfix_*`, `lutfix_only_*`, current
`p2f_B/p2f_D` = post-both-fixes; session dumps `p2g_sess_*` (current)
and `prev_p2g_sess_*` (pre-fix; differ by ~40 moves via the air-cut
filter's changed snapshot).

THE LESSON (now 6×): measure before theorizing — the LUT fix was
committed to a 2-hour acceptance rerun on a magnitude match at the
wrong slope percentile. The stamp probe (60 s, isolated, exact) found
the real stamping defect immediately. Build the cheap decisive
experiment FIRST.

---

## TASK 1 SOLVED 2026-07-10 — the gap was the MEASUREMENT MESH

`p2g_chain_stage_probe` (new, in the harness) ran the real chain per
branch and read every intermediate per dexel column in the bad window:
rough floor (`prior_stocks[op8]`), post-op8 floor (faithful re-stamp on
the prior snapshot — op8 is the LAST op, no successor snapshot exists),
exact envelope of the generated moves, mesh top, deviation. Cross-branch
per-cell deltas by stage:

| stage | B75−D p10/p50/p90 µm | verdict |
|---|---|---|
| rough | 0 / 0 / 0 | ladder identical |
| env   | −55.6 / −0.3 / +54.3 | ±55 µm TEXTURE PHASE, median 0 |
| post  | −55.8 / +0.3 / +54.8 | stamps track env sub-µm in chain |
| dev   | −1.5 / **+5.4** / +13.8 | the instrument's shift — STRIPED |

Facts established:

1. **The real machined surfaces are statistically identical.** The
   ±55 µm pointwise envelope difference is ring/chord lattice phase
   (expected between two different ring families); its local mean is
   0.0–0.2 µm at every smoothing scale 0.75–3.25 mm, and the
   vertex-anchored true-leftover quantile comparison (env at vertex −
   exact model_z from `vz − dev`) puts B75−D within ±18 µm at every
   percentile, mean +0.2 µm. Earlier "equivalent within 1 µm" python
   floor checks were median/limited-sample statements — directionally
   right, blind to the phase texture.
2. **The instrument's +5.4 µm B75 shift does not exist in the stocks.**
   Correlation between measured dev delta and true stock delta is ZERO
   at every scale (raw and box 3–13); the dev delta has horizontal
   stripe structure absent from the stock. Mechanism: deviations are
   measured on MESH VERTICES. The z-grid mesh places vertex heights at
   corner-bilinear averages of 2×2 dexel tops
   (`dexel_mesh_mc::z_grid_marching_cubes`), and steep flanks are
   additionally meshed by the X/Y side grids. Mesh-vertex sampling
   filters machined texture by its phase coherence vs the grid:
   B75's grid-locked ridges survive the averaging (its 10–50 µm bin
   counts are honest-ish), D's phase-diverse silhouette rings CANCEL
   (its "on-size 92 %" is fake smoothness). "D wins the fine tier" was
   an instrument artifact — resolution-independent, which is why the
   0.21 mm rerun kept it.
3. **Fix (in tree)**: `SimulationResult::column_deviations` — pointwise
   per-dexel-column top vs model (world frame, per-setup transform,
   same relevance/nearest-surface semantics as the vertex pass), a
   FIDELITY-COLUMNS table in the harness `fidelity_report`, and sentry
   `column_deviations_pointwise_against_flat_model`. Vertex deviations
   stay for GUI display. Quality verdicts must read COLUMNS from now on.
4. **Dead leads closed**: `set_toolpath_operation` dressup-policy gap is
   INVALID — `normalize_for_op` runs on swap (since 06468a8), the dumps
   show entry/lead/link stripped; `arc_fitting: true` surviving is by
   design and branch-symmetric. Roughing interaction and rot90 frame
   are exonerated by the stage table (rough delta exactly 0; env is
   frame-independent ground truth).
5. **Honest band-wide verdict (COLUMNS instrument, colfix acceptance)**:
   mid-steep columns — B75 on-size 32 959 / +.05-bin 20 557 vs D
   on-size 45 952 / +.05-bin 7 051. This gap is REAL (columns are exact
   pointwise leftovers) and is NOT the in-window story: dense 0.05 mm
   ground truth over the bad window (70 k points, `p2g_dense_env_probe`)
   puts B75−D at ±11 µm per quantile, mean −1.4 µm — equal where rings
   run. The +.05 excess = 13 506 columns = 844 mm² = 16.3 % of the band,
   matching the known ~8 % ring under-coverage + ~8 % collar share, both
   cut at raster-on-slope cusp (~0.034 → the 10–50 µm bin). So: B75
   holds D's quality on the ~84 % of the band its rings cover and hands
   ~16 % to raster/collar quality, at 6 % less time. The ORIGINAL Task 1
   (collar/coverage fix) is resurrected as the real remaining lever,
   now precisely sized: close the 16 % and B75 ≈ D quality band-wide at
   lower cost. The vertex instrument's version of the gap conflated this
   real 16 % with the averaging artifact (its in-window "bad cells",
   1.41 mm lattice, and D's 92 % on-size were artifact).

Artifacts: `p2g_chain_{b75,d}_cells.txt` (per-cell stage dump),
`p2g_chain_{b75,d}_verts.txt` (per-vertex model_z + env ground truth),
`p2g_chain_delta_*.png` (stage delta maps), chain_stage_probe2.log.

THE LESSON (7×): when two measured populations disagree, correlate the
instrument against ground truth PER LOCATION before believing either.
Histograms hid that the "winner" was invisible to the instrument, not
better.

---

## STATUS 2026-07-09 late night — the verdict is OPEN again; three-way probe designed

The evening squeeze (coverage overlay + paired columns) REVISED the
Task-1 closure. Read this before trusting the section above.

**Facts (all from paired same-lattice, group-filtered column dumps on
the CURRENT project — see below re: the file change):**

1. **Coverage attribution DEAD**: 99.2 % of mid-steep columns lie within
   0.3 mm of a ring pass, and the +10..50 µm columns are exactly as
   ring-covered (99.3 %) as the on-size ones. The "16.3 % = 8 %
   under-coverage + 8 % collar" story does not survive; the excess sits
   UNDER the rings, in ~1.5–2 mm stripes across the whole band
   (`p2g_plus_share_map.png`).
2. **The gap is real in the sim's stocks and reproduces on the live v2
   op**: group-1 (top setup) paired columns — unified on-size 48.5 % /
   +.05 30.0 % vs D 67.7 % / 10.1 % band-wide; in the old analysis
   window 63.9 %/36.1 % vs 95.9 %/0.8 %.
3. **But it contradicts the exact envelopes**: the dense 0.05 mm
   envelope delta over the same window is symmetric ±55 µm with mean
   −1.3 µm at EVERY 0.25 mm lattice phase (aliasing refuted), while the
   sim's paired stock delta there is mean +6 µm, p10 −3.2 / p90 +17.
   Per-branch re-stamps matched their own envelopes to ≤1 µm p90
   (chain-stage probe). Three individually-validated measurements
   disagree pairwise. The real tool matches the probes' cutter model
   exactly (tool id 2: Ø1 ball / 7° / Ø6 shank / 25 cl) — not the cause.
4. **Prime suspect**: the air-cut filter (or any post-sim conditioning)
   MUTATES the stored toolpath after simulation — so the toolpath the
   sim STAMPED is not the toolpath later read for envelopes/re-stamps.
   Known signature: regenerated dumps differ by ~40 moves. Whether ~40
   edited moves can move 20-30 % of a band's columns by 10-50 µm is the
   open question (they're plunge/entry-adjacent segments — possibly
   high-leverage on ring starts).
5. **Instrument fixes landed meanwhile** (92ceb85): ColumnDeviation
   carries the setup-group ordinal — the entire `<-.5` "gouge" tail
   (11.2 k) in every FIDELITY-COLUMNS table was the BOTTOM setup's
   columns cross-attributed against the top surface. Filter by group.
   FIDELITY-COLUMNS in fidelity_report still needs the group filter.

**THE NEXT PROBE (three-way, decisive)**: extend `p2g_chain_stage_probe`
to record, per window cell, in ONE run: (a) exact envelope of the
toolpath AS READ post-sim, (b) my re-stamp of that toolpath, (c) the
SIM'S OWN final stock top (sim.column_deviations, group-filtered), and
(d) the envelope of the toolpath captured BEFORE the first simulation
(pre-air-cut-filter). Whichever pair diverges identifies the mutation.
If (d) ≠ (a), the air-cut filter is the mechanism and the honest
verdict must be computed from the AS-STAMPED toolpath.

**Project-file trap (cost an hour)**: wanaka.toml changed on disk at
13:27 — "3D Finish 6" is now DISABLED; the enabled finish is
"Unified Finish 6 (live v2)" (same 127 235 moves as the harness B75 —
dial-identical). Probes that swap by the historical name silently
measure the live op twice. The overlay probe now targets the enabled
finish op with an assert; the OTHER FINISH_OP_NAME-pinned tests
(acceptance/fidelity/chain probes) still need the same fix before any
rerun. Do NOT commit or revert the user's wanaka.toml.

LESSON (8×): a "ground truth" validated only against artifacts computed
from the same inputs is circular. The sim's stock and the stored
toolpath are not guaranteed to be the same object — measure the
as-stamped geometry.
