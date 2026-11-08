# P22 — Does the simulation model the centre core of a non-centre-cutting tool?

Status: investigation only (2026-11-09, HEAD f168ffab9). No product code changed.
Question from the operator: wanaka tool 6 (18 mm 2F surfacing bit, centre core
8 mm, blade protrusion 1 mm; `planning/rivmap_block_job/nz_wanaka_21_operator.toml`
tool id 6, `centre_cutting = "no"`). Tool 6 cuts five wanaka toolpaths: ids 0, 1,
17, 18, 20 (the faces and the two bulk roughs).

## Answers

1. **Material model: NO.** The tri-dexel stamp cuts the full flat disc. The core
   is not in the stamp. Every cut where the core passes over wood deeper than the
   protrusion shows that wood removed.
2. **Detection: PARTIAL.** A core check exists in the simulation walk
   (G-CENTRECUT, `RapidClearanceCheck::core_strike`). It sees one straight move
   that is steeper than the limit angle. It is blind to a helix with a radius
   smaller than the core radius, and to any descent that the toolpath splits
   into short moves. `tool_load/` has no core check (`rg -n 'centre|core'
   src/tool_load/` gives no hit on the core).

## Evidence: the stamp (fault 1)

- `stock/radial_profile.rs:35` `RadialProfileLUT::from_cutter` samples
  `cutter.height_at_radius(r)` over `0..R`. The stamp reads only this LUT.
- `tool/mod.rs:911` `ToolDefinition::height_at_radius` forwards to the bare
  shape (flat: 0 for every r ≤ R). `tool/mod.rs:312,666,857`: the core fact
  rides on `ToolDefinition::non_centre_cutting`, but only the entry emitters and
  the core check read it (`rg 'non_centre_cutting\(\)'`: 6 hits, 3 files;
  none in `dexel_stock/` stamping, `stock/radial_profile.rs`, `tool_load/`).
- The probe prints the stamp profile of tool 6: `h(0) = h(2) = h(3.9) = h(4.1)
  = h(8.9) = 0.0`. The core (r < 4) stamps as a cutting face at the tip.

## Evidence: the core check (fault 2)

- `stock/collision.rs:1054` `core_descent_candidate`: only a move steeper than
  `atan(protrusion / core)` (7.125 deg for tool 6) is a candidate.
- `stock/collision.rs:1074` `core_strike`: reads the stock as it stands before
  the move, in the lens of the core discs at the tip and one `h` back along the
  SAME move (`back` clamps at the move start, line 1100). The model is correct
  for one straight move over stock that no earlier move touched.
- `dexel_stock/simulation.rs:651-678` (also 738-761, 833-856): the walk flushes
  the stamps, runs the check, then stamps the move. The check reads the
  simulated stock, and fault 1 made that stock flat under the core of each
  earlier move. So the history under the core is lost at every move boundary:
  a move strikes only when its own drop into fresh wood is more than the
  protrusion.
- `dressup/entry_descent.rs:943-948` `emit_helix`: the G-CENTRECUT clamp limits
  each 10-degree chord to the limit slope. The straight-line limit does not
  apply to a helix whose radius is smaller than the core radius: the helix
  centre stays inside the core for the whole entry, no blade passes over it,
  and the tool descends one pitch per turn (1.569 mm per turn at r = 2 mm, more
  than the 1 mm protrusion). Nothing refuses or warns on `radius < core / 2`.
- Export: `session/compute/export.rs:207-224` (`Gate::CentreCutDescent`) gives a
  `safety.non_centre_cutting_descent` warning (`diagnostics/ids.rs:605`,
  `warn(Danger::Tool)`). It warns; it does not refuse. The function name
  `non_centre_cutting_refusal_rows` (`compute/simulate.rs:1057`) is older than
  the K6 ruling; the name only is wrong.

## The probe

Temporary file `crates/rs_cam_core/tests/zz_probe_sim_core_tmp.rs`, run with
`scripts/cargo_lane.sh test -j 2 -p rs_cam_core -q --features test-support
--test zz_probe_sim_core_tmp -- --nocapture` (0.34 s). **The file is deleted.**
It used the live walk of the shipped `the_gate_sees_the_core_go_into_wood`
helper (`tests/a_non_centre_cutting_tool_never_plunges_into_stock_g_centrecut.rs:165`):
tool 6 geometry, stock top Z 12, cell 0.25 mm, target floor Z 7 (5 mm into
stock), stock top read in a 0.3 mm disc at the centre after the run.

| Case | Real tool | Sim strikes | Sim stock top at centre |
|---|---|---|---|
| A. one 5 mm plunge at (0,0) | core hits wood at 1 mm; wood stays at Z 12 | 1 (5.00 mm over tip) | 7.000 |
| B. the same plunge as 11 moves of 0.5 mm | same as A | **0** | 7.000 |
| C1. helix r = 2 mm, clamped pitch 1.569 mm/turn | post under the core; centre wood at Z 12 | **0** | **7.000** |
| C2. helix r = 3 mm, clamped pitch 2.353 mm/turn | post, as C1 | **0** | **7.000** |
| C3. helix r = 5.4 mm (0.3 D), clamped pitch 4.236 mm/turn | cuts clean (centre in the blade annulus) | 0 | 7.000 (correct) |
| D. helix r = 2 mm, pitch 6 mm/turn (each chord 25 deg, steeper than the limit) | post, as C1 | **0** | **7.000** |
| E1. 30 deg ramp, one move | core strike | 1 (1.150 mm over tip) | — |
| E2. the same ramp as 22 moves | core strike | **0** | — |

Read: C1, C2 and D leave a 5 mm post in real wood (the centre never leaves the
core); the simulation shows it cut and says nothing. B and E2 show that the
check depends on how the toolpath splits a descent, not on the motion. D shows
that even chords steeper than the limit pass, because each chord drops
0.167 mm into wood that the previous stamp flattened.

## What inherits the error

The cut trace and the stock come from the same stamp, so every reader after a
core error reads the wrong stock:

- The stock mesh, the rest-stock readings and `standing_islands` (a post is
  missing).
- The cascade: a later op with `stock_source = machined` starts from a stock
  without the post. Its engagement, air-cut %, `crosses_standing_material` and
  link-plough checks are all low. A later tool can strike the post with no
  finding.
- The sample engagement of the faulty move itself: the arc and the section
  count the core disc as cut (`Engagement::from_sample`), so power, chipload,
  deflection and the force card read a slot that the tool does not cut. For a
  plunge this overstates the load; the real load is a stall or a burn.
- The simulation gate `non_centre_cutting_descents` (the one source of the
  export warning) misses the cases above, so the export is silent.

## Fix proposal

### (a) The stamp of a non-centre-cutting tool

The stamp must remove only what the blade annulus `core_r ≤ r ≤ R` sweeps
(the protrusion band is in the annulus; the core face sits `protrusion` above
the tip). The core cuts nothing.

1. The profile: give the LUT a second channel, not a changed `height_at_radius`.
   `h_blade(r) = 0` for `core_r ≤ r ≤ R`, `+inf` (no cut) for `r < core_r`.
   `height_at_radius` stays the envelope for collision, drop-cutter and the
   finish planners; only the stamp reads the blade channel.
2. The kernel: the shipped segment stamp evaluates `z(t_closest) + h(d)`
   (`dexel_stock/simulation.rs:1128`, `stamping.rs:212` `segment_cell_coverage`,
   `swept.rs`). That is exact only for a profile that rises with r. An annulus
   does not. A closed form exists: a cell at lateral distance `d` from the
   segment line
   - with `d ≥ core_r`: as now (the annulus covers it at the closest point);
   - with `d < core_r`: it is cut at the segment parameters where its distance
     to the axis equals `core_r`, i.e. `t_closest ± sqrt(core_r² - d²) / len`,
     clamped to `[0, 1]`; take the lower of the tip Z at the two in-range
     points. No point in range: the move does not cut the cell.
   A helix is many short segments, so each chord is a segment and the closed
   form is per chord. C1 then keeps the post; C3 still cuts clean.
3. The air-skip and the mip: `RadialProfileLUT::profile_is_nonneg_total`
   (`stock/radial_profile.rs:101`; read at `dexel_stock/simulation.rs:324`,
   `tile_mip.rs:30`) assumes a non-decreasing profile. The blade channel is
   not. Turn the S2 air-skip off for a blade-channel stamp, or bound it with
   the envelope (the envelope is a lower bound on the blade stamp, so it stays
   conservative). The metric route and the playback route must use the same
   kernel (`dexel_stock/CLAUDE.md`, invariant 3).
4. The core as a body: after (2), wood under the core stays in the stock. The
   core check then reads true history, because no stamp flattens the core.
   Optional for the stamp: cut the wood above the core face (`tip + protrusion`)
   as "crushed", so the stock does not show wood inside the tool. Recommend
   NOT to: the strike finding reports it, and the stock keeps the wood that the
   real job keeps after a stall.

### (b) A generic finding: wood under the core deeper than the protrusion

- Replace the candidate rule (steeper than the limit, one move) with a check
  on every fed move of a non-centre-cutting tool, against the true stock of
  (a): `max_top_z_in_disc(tip, core_r) - tip.z > protrusion + tol`. With (a)
  the history is in the stock, so no lens model and no limit-angle candidate
  are needed. Cost: one disc read per sample step on such a tool only; keep
  the flush-on-candidate shape, with "the tool is non-centre-cutting" as the
  candidate.
- Without (a), a partial guard: carry the lens across move boundaries (a
  per-cell "inside the core since Z" record over the core disc). It is more
  code than (a) and still leaves the stock wrong. Not recommended.
- The finding: one per move, as now (`NonCentreCuttingDescent`: position,
  wood over the tip, angle), on every op, through the same
  `safety.non_centre_cutting_descent` warning. Never a refusal (operator rule:
  every finding warns; generation and export always produce output). Rename
  `non_centre_cutting_refusal_rows` to `…_warning_rows`.
- The generation side: `emit_helix` must state the helix-radius limit. A helix
  of radius `r_h < core_r` leaves a post at any pitch. The emitter cannot
  refuse; it warns (feasibility, beside the G-CENTRECUT plunge warning) and
  the op can raise `r_h` to `core_r` where the containment allows.
  Pre-register the bound from geometry: the centre is in the blade annulus at
  every phase exactly when `core_r ≤ r_h ≤ R`.

## Blast radius (`rg` counts over `crates/`, `*.rs`)

| Symbol | Hits | Files |
|---|---|---|
| `RadialProfileLUT::from_cutter` | 86 | 38 |
| `profile_is_nonneg_total` | 14 | 5 |
| `fn stamp_` | 33 | 14 |
| `segment_cell_coverage` | 8 | 2 |
| `lut_h_with_edge_fallback` | 7 | 2 |
| `core_strike` / `core_descent_candidate` | 6 / 6 | 2 / 2 |
| `non_centre_cutting()` | 6 | 3 |
| `SAFETY_NON_CENTRE_CUTTING_DESCENT` | 11 | 9 |

The stamp change touches `dexel_stock/stamping.rs`, `swept.rs`,
`band_batch.rs`, `tile_mip.rs`, `simulation.rs`, `playback.rs` and
`stock/radial_profile.rs`. A centre-cutting tool must stay bit-identical
(G-OPGOLDEN, `band_stamping_determinism_s3`, `swept_stamping_s1`).

## Sentries to add

1. `a_helix_inside_the_core_leaves_a_post_g_simcore` (red today): case C1. The
   stock top at the helix centre is at least `floor + protrusion` (real: Z 12),
   and the simulation records at least one core strike. Inverse case C3: no
   strike, and the centre is cut to the floor.
2. `a_split_descent_strikes_like_one_move_g_simcore` (red today): cases B and
   E2 give the same strike set as A and E1 (one strike per toolpath at least,
   on the first move that pushes the core more than the protrusion into wood).
3. `a_centre_cutting_stamp_is_unchanged_g_simcore`: the stamp of every shipped
   centre-cutting shape is bit-identical before and after (stock hash and
   sample hash on the `swept_stamping_s1` fixtures).
4. `a_straight_pass_of_a_non_centre_bit_cuts_clean_g_simcore`: a level slot at
   0.5 mm depth after a 0.5 mm plunge leaves no wood in the slot (the annulus
   sweep covers the core area) and gives no strike.
5. Keep `the_gate_sees_the_core_go_into_wood` and
   `a_rough_with_a_non_centre_cutting_bit_keeps_its_limits` green; the wanaka
   tool-6 ops are an evidence run (`#[ignore = "evidence: .."]`), minutes long.
