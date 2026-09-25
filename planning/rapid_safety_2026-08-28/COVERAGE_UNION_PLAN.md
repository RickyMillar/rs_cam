# COVERAGE_UNION_PLAN — cells cleared by several partial stamps (design, 2026-09-25)

Status: design only, **deferred** (lead decision, operator delegated). The
false-flag class it removes has not been measured on a real project at Auto
resolution (pocket_lift_bridge_b1 and rivmap100 read 0 at 0.5 mm), and the fix
costs +35 % stock memory and an estimated +3-8 % on the sim kernel.
**Trigger to implement:** any rapid collision on a real project at Auto that
dumps as union residue (Step 0 below), or a low-channel-only flag of that kind.

## 1. The model (file:line at e32a5eec)

- `DexelGrid` (stock/dexel.rs:266-308): `rays` (ray_top = area-weighted mean,
  partial stamps blend via `ray_blend_above`, :97-127) and `conservative_top`
  (starts at the stock top, :415; lowered only by min, `lower_conservative_top`
  :519-523; read by `conservative_top_at` :509). Not serialized; no viz/GPU
  reader.
- Coverage: a 4x4 sub-sample grid per cell (stamping.rs:33-42);
  `point_cell_coverage` :54-91, `segment_cell_coverage` :220-271.
- Lowering sites: the shared driver (stamping.rs:699-704, only when coverage
  >= FULL_COVERAGE = 1 - 1/32, i.e. all 16 samples, to
  `cell_upper_bound_surface` = ub_depth + h(min(d + cs/√2, R)), :356-383),
  the plunge chunk (swept.rs:347-378), the swept chunk (swept.rs:677-687),
  `clear_above_at` (dexel_stock/mod.rs:621-628).
- Coverage is thrown away after each stamp, so a cell cleared by the union
  of several stamps keeps the lowest single-full-stamp value.
- Readers of the ceiling (13): dressup/air_cut.rs:94; dressup/mod.rs:290,
  :764, :776; dressup/entry_descent.rs:508 (`max_conservative_top_z_in_disc`);
  finish/surface_link.rs:139, :173; finish/pencil/emission.rs:230;
  adaptive3d/clearing.rs:728, :837; compute/execute/project_curve_chaining.rs:498,
  :549-550; stock/collision.rs:772.

Corrections to the brief: a false flag needs level L > τ + clearance (2.71 mm
at 1 mm cells, 1.60 mm at 0.5 mm) AND a union-only cell within R + cs/2; for a
straight pass such a cell exists only where passes overlap by less than
1.06·cs (at Auto with Ø6: stepover > ~91 % D, or corners, pass ends, ring
junctions). **The low channel carries the same residue**: two half stamps
leave ray_top = floor + L/4, which flags when L/4 > clearance on a cell wholly
inside the footprint. The fix must cover both channels.

Side findings (out of scope, unverified): a rib narrower than cs/4 straddling
a cell border can fall between the samples of two cells; swept.rs:683 lowers
with the centre bin's depth range but the whole chunk's coverage (possible
under-bound up to |dz/ds|·cs/√2 on a descending multi-bin chunk).

## 2. Options

**A (recommended when triggered): strict sub-square union mask, separate
channel.** Per cell `CoverUnion { mask: u16, depth: f32 }` + `union_top: f32`
(+INF); readers use `ceiling_at = min(conservative_top, union_top)`.
- Bit k = sub-square k (side cs/4, centred on sample k) wholly inside the
  footprint: dist(sample k) ≤ R − cs·√2/8 (1-Lipschitz; nudge down a few ULP).
  Strict, never "sample inside": the latter lowers a cell holding a real rib
  thinner than cs/4 between sample columns (the TP15/A/M10 sliver class).
- u = f32(`cell_upper_bound_surface`(lut, d², cs, ub_depth)); swept chunk
  u = max(sd, ed) + h(far); plunge: mask once, u per bin.
- Depth: storing "min over covering stamps" for the mask is UNSAFE. Keep one
  candidate (M, D) with invariant I: every bit in M has all its material ≤ D.
  absorb(bits, u), δ = 0.25·cs: (1) bits = 0 or !(u < ceiling) → return;
  (2) M ≠ 0 and M|bits = FULL → union_top = min(union_top, max(D, u));
  (3) M = 0, or D ≥ ceiling, or u < D − δ → restart M = bits, D = u;
  (4) u ≤ D + δ → M |= bits, D = max(D, u); (5) shallower → nothing;
  (6) M = FULL → union_top = min(union_top, D). I holds on every branch, a
  Z grid only loses material, and a completion bounds all 16 sub-squares.
- Rays and conservative_top stay byte-identical, so cell_can_remove, air-skip
  and the tile mip keep their exactness; the gate u < ceiling makes every
  route (per-stamp, band, whole-path, playback; mip on/off) bit-identical.
- Read side: mod.rs:426 and :607 read `ceiling_at`; the low channel (:594)
  reads min(ray_top, ceiling_at). air_cut.rs:68-75 assumes CT ≥ ray_top —
  still safe (errs to "material"); update its doc or read min(ray, ceiling).
- Memory 12 B/cell (+35 %): 96 MB at the 8M-column cap, paid by every stock
  clone (global, group, prior_stocks, checkpoints); sim_prefix.rs:591
  estimate 32 → 44 B. Lever: D as a u16 code rounded up (8 B/cell).
- Cost: est. +3-8 % kernel. Blast radius: stamping.rs:535-704,
  swept.rs:250-382 and :398-690, band.rs:43-200, constructors dexel.rs:310-323
  and :398-417, adaptive3d/tests.rs:75, collision.rs test helpers.

**B (refuted): re-derive from ray_top and neighbours.** A mean cannot bound a
maximum (a full-height rib inside one cell reads cleared). Unsafe.

**C1: fold the union into conservative_top and clip the ray** (moves goldens
and bit nets; a follow-up after A is measured). **C2: 4x4 per-sub-square u8
ceilings** (16 B/cell, exact per sub-square, quantisation up to 0.39 mm on a
100 mm stock). Reserve.

## 3. Sentries (A)

Step 0 (pre-registered, before any re-pin): pocket_lift_bridge_b1 at 1.0 mm,
moves 114/268/423 — dump the cell that sets `high` (row, col, d, CT, ray_top,
M, D). Prediction: M = FULL, D = previous floor (0, −4, −8). If not, this is
not union residue: stop and report.

New `tests/coverage_union_ceiling_g_coverunion.rs` — flat Ø6 (R 3), cs 1.0,
TOP 8, stock −4..8, a cell centre at x = 0, passes parallel to Y:
1. Passes x = ±2.9 at z 0: each covers 8/16 samples (f = 0.5); strict
   column at −0.125: 2.775 + 0.177 = 2.952 ≤ 3. At (0,0): CT 8.0, ray_top 2.0,
   ceiling 0.0 exactly.
2. A x = −2.9 z −4, B x = +2.9 z 0, both orders → 0.0; add C x = +2.9 z −4
   → −4.0.
3. A only → ceiling 8.0, union_top +INF.
4. x = −3.0 and +3.05 leave a real rib over [0, 0.05] → ceiling 8.0; with
   "sample inside" bits injected once it must go red.
5. Live check: Z-only rapid at (0,0) 19 → 0.5 on fixture 1: strikes before
   (high 8: 0.5 < 5.79; low 2.0), clear after; to −0.3 still strikes. Auto
   variant cs 0.5, TOP 3, passes ±2.9: before 0.5 < 3 − 1.10 strikes; after
   clear.
6. Routes agree bit for bit (extend the_six_cell_loops stamping.rs:2130-2143
   / 2396-2404, swept.rs:1259, band.rs:345-360,
   band_stamping_determinism_s3:125-133, sim_prefix_memo_s5:263 / :594).
7. Re-pin pocket_lift's 1 mm test to vec![] only if Step 0 confirms.

Must stay green: descent_resolution_stability_am10, g_rapidplungetol,
g_rapid6497, rapid_live_check_crest_s2, entry_descent_profile_b2,
air_filter_tool_aware_s3, profile_link_ceiling (reference loop :229 →
ceiling_at), rapid_collision_detector_population.
Perf guard: `cargo bench -p rs_cam_core --bench hot_paths` groups
sim_kernel_lateral, sim_kernel_plunge, sim_e2e_small, sim_playback_ab,
--save-baseline before / --baseline after; budget median ≤ +5 % lateral and
e2e, ≤ +2 % plunge; wanaka must stay under the 1.5 GB sim-prefix ceiling
(sim_prefix.rs:209). Over budget: drop the strict-ring loop, then u16 D.

## 4. What moves

pocket_lift's 1 mm pin (on purpose, after Step 0); profile_link_ceiling:229;
bit nets gain channels (old bits unchanged); sim goldens and rapid counts
expected unchanged (sim byte-identical). Generation reading prior stocks
(dressups, adaptive3d clearing, surface_link, pencil, project curve) gets
lower link/entry heights where union cells exist — less air; rapid counts
must not rise. Watch p1_headless_ab_wanaka, classification_columns_ab_m3,
union_coverage_m1, rest_cascade_g_restres, rivmap100 at 0.5 and 0.25 mm.
Docs: air_cut.rs:68-75, dexel.rs:276-307, collision.rs:644-663,
mod.rs:504-515, the pocket_lift header, PROGRESS 2a.
