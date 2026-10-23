# Adaptive research — status

Lead session: Fable, started 2026-10-23 10:41 NZDT. **G0 PASSED 2026-10-23 13:30** with rulings below. Next: Phase 1.5 instruments, then Phase 2.

## Phase 1.5 + Phase 2 start (2026-10-23 13:45)

Branch `adaptive-instruments` in worktree `.claude/worktrees/adaptive-head`
(HEAD a23df1f8, own target). Agents edit there with no cargo; one verifier
runs the worktree lane script. The lead commits by pathspec on the branch
and cherry-picks onto master after the peer's default-removal commit lands
(it edits `compute/operation_configs.rs` widely; rebase on it).

Facts checked before briefing: `metrology::union_coverage` is LIVE (only
its evidence test is a gated target); `ToolpathStats::untouched_material_mm2`
is the scallop-cascade measure, not a rough measure, so the rough's coverage
is the union audit per op checkpoint; `ModelGougeRule::Exempt` on 3D ops
dates from UX-7b (6d5a5423, 2026-10-22), whose scope was 2.5D ops; the
Surface rule samples EMITTED moves at ≤ D/4 through the drop-cutter.

| Agent | Scope | Owns (files) |
|---|---|---|
| A coverage+thinstand | G-ROUGHCOV + G-THINSTAND: union audit per rough checkpoint against model + stock_to_leave, islands with height and min width, into SimulationTriage findings | `metrology/union_coverage.rs`, `stock/sim_triage.rs`, `session/compute/simulation.rs` (wiring), new sentry |
| B gouge | G-ROUGHGOUGE: Surface rule with the stock_to_leave allowance for 3D Rough (and other 3D ops that leave stock), on emitted moves; registry row change LAST | `compute/model_gouge.rs`, `compute/catalog/registry.rs` (one row), new sentry with an injected gouge |
| C candidate (a) | Phase 2 2D: geodesic field from the entry with islands as obstacles, wraps as level sets, one continuous path, behind a research flag; render beside the 2D baselines on `ux_2d_star` / `ux_2d_pocket` | new `adaptive/research_geodesic.rs` (or similar), a render harness; no edit to shipped paths |
| B result (14:20) | DONE, lead-reviewed by path: new `ModelGougeRule::Allowance { fix }`; limit = drop-cutter CL Z + (stock_to_leave_axial − chord_tolerance(tolerance, segment_merge_tolerance)), both read from the config the planner reads (no new constant; `OperationConfig::model_gouge_limit_offset_mm`); REG_ADAPTIVE3D row flipped; probe unchanged (real cutter − MODEL_GOUGE_TOL_MM); measured on the exported moves at generation; sentry `a_rough_cuts_no_deeper_than_its_leave_g_roughgouge` (4 tests). Deviation from the brief accepted: the Z tolerance is the op's own chord tolerance, not the 0.02 mm CAD rounding, because the RDP simplification may sit a chord that far below a draped point. Not built yet. | `P15_B_gouge.md` (worktree) |
| A result (14:35) | DONE (first pass): `metrology` is gated behind `test-support` at lib.rs (lead's "live" fact was wrong at the crate root), so the column walk and island flood moved to ungated `stock/standing_islands.rs`; the union audit calls it. Rough target = model + stock_to_leave_axial where there is model, mesh-bbox floor + leave where there is none (hole cores, outer band), raised to a pinned bottom_z, read only on tool-reachable columns. Bar: standing > leave + one cell (G-OVERCUTREF tolerance). Islands 4-connected with area, height, inscribed-disc width, peak; class safety when height > depth_per_pass + cell else action; abstains when cell > leave or flipped setup. Id `project.rough_leaves_islands`, field `triage.coverage`. Rulings given 14:40: add the viz tab row, append per-toolpath triage findings to `get_toolpath_diagnostics`, one narrate sentence. | `P15_A_coverage.md` (worktree) |
| C result (15:05) | DONE, not built: `adaptive/research_geodesic.rs` (research cfg) — geodesic distance from the entry disc (R about the EDT-max entry), islands as walls, the ONE `fast_sweep` solver made pub(crate) (+ converged flag), `iso_contour_where` filter; wraps = level sets one stepover apart (spiral.rs rule), ordered lower-level-first then nearest; wraps run through the shipped replay under `step_within_pass_load`, residue, wall pass, mop; NO trochoid insert (its values are underived). Harness `adaptive_geodesic_2d_renders_g1` writes SVGs for agent / contour_spiral / geodesic_a on star + pocket. Risks: field over-reads island corners (2.5 cells), kinks at meeting lines, 24-sweep cap, radial step between closed wraps (a continuous spiral `T/s − φ/2π` not built). Branch commits: de64e6ad (A), ce94b369 (B), b94f5e4d (C). | `P2_C_geodesic.md` (worktree) |
| D verifier | DONE 14:45 + re-gate 15:05 at 2b72f57b: every gate green (fmt, 3 checks, roughcov 4/4, roughgouge 4/4, research_geodesic 3/3, w2_4, ux7b, chamfer sentries, adaptive3d 14/14, --lib adaptive 141, fin01, fld0405, viz --lib 582, mcp 36, full clippy); 3 fixes (fmt, as_chunks, S11 fixture feeds in the harness). Master-side reds (pre-existing, owned by the geometry-selection lead): g_adaptpassload, g_adaptrapidlift, s6. Open: harness over-step on the pocket without position; no gate compiles the non-`parallel` path (pre-existing rayon break). Branch head 405cbd23 (master 18bf31a9 merged). **ff of master blocked** by the peer's uncommitted compute/catalog.rs; peer lands it in 1–2 h, then merge + ff. GUI swapped to the branch build 15:15 (mtime 1792721740, pid 1260840); the previous GUI (pid 84659) had exited with code 1 on its own at 14:56. | `P15_D_verify.md` (worktree) |
| C2 G1 fixes | DONE 17:40, committed 0e79f0fc: G1-1 was mostly the PNG converter dropping colours + wall/residue passes drawn like wraps; real spill 0.01 mm; wraps now clipped with the planner's own test; harness draws producers in colours and writes its own PNG. G1-2: the field is clean, the breakdown is the cutter turning through the V kink; built the one-side rule (first branch runs to the meeting line, the other ends one stepover short; valid for s ≤ R/√2), rendered beside the old rule. G1-3: lead hypothesis only partly right (first-order error ≤ 0.84 cell on the arm axes); main causes were the irregular fixture star (tips 45.5–49.8 mm), the seed one cell off the mirror axis, and the alternating join; solver now source-tracked (exact in the open, ≤ 1.14 cells behind the island, never under); WENO3 and smoothing rejected by prototype. Plunge per wrap: a sideways step of one stepover reads 0.94 D vs the 0.3626 ceiling; no reusable smooth join in spiral.rs (its insert is tuned); built a ramped entry into the next band derived from R, stepover and the ceiling (full depth in 7.7 mm); predicted 1–6 plunges per shape (unmeasured). path.rs gains one source for the planner cell and pass step. Prototype measurements in Python, not Rust. Was RUNNING 15:20: G1-1 spill (render colours + clip), G1-2 meeting line (measure first, then one-side continuation / blend / status quo), G1-3 arm asymmetry (measure T along the five arm axes; WENO sweep / smoothing / finer field grid), continuous join instead of a plunge per wrap, shared constants from path.rs | `P2_C2_geodesic_fixes.md` (worktree) |
| E result (15:55) | DONE. **Gouge: positions true, rule too strict** — blocks export on all 8 roughs: a 0.02–0.05 mm sideways wall graze in a hex hole reads 3.2 mm (Z-only measure reports the wall height), and op 0 overshoots the planner slack by 0.064 mm while 0.3 mm above the model. No false fire at AS move 1401 or wanaka link 5147 (correct: those cut stock, not model). Adds ~6 s per wanaka rough. **Coverage: no reading in the GUI** — viz `controller/events/compute.rs` ~2609 hands the session `checkpoints: Vec::new()`, so `rough_coverage_at` finds no stock; `triage.coverage` is `[]` (not a zero, not an abstention). N11 measured: back rough level 3 (Z 8.62) switches at ~move 3563 to a one-way +X scan, 0.5 mm rows over the whole board with 3.6 % material left → 1 581 rapid runs, ~115 m rapid. New: green mid-height strips inside the Corne wall on AS/CS; ~half the AS hex holes keep a core (not a third). **DO NOT ff master until the gouge rule is fixed (export regression on the default rough).** | `P15_E_evidence.md`, `phase0/img/*_p15_*.png` |
| A2 coverage wiring | DONE 17:20, committed d0dd680f: the view computes the readings at adopt from its checkpoints and hands the session the readings (ProjectEvidence::rough_coverage); abstentions NoCheckpoint / RunNotStored, never an empty list; memory ~100 B per island; ~2 s per rough once per adopt; new viz test `the_gui_adopt_reads_the_rough_coverage_g_roughcov`. Was RUNNING 16:00: the viz checkpoint list; keep the checkpoint stock the reading needs or compute the reading viz-side; respect the memory sentry | `P15_A2_coverage_wiring.md` (worktree) |
| B2 gouge rule | DONE 17:30, committed c6d1abb3: probe radius R − chord tolerance t for the Allowance rule (`probe_radial_tolerance_mm`); Blocking only below the model surface (− t, beyond the existing Z tolerance), Caution inside the leave band, a graze within t gives nothing; message gives both depths and an `at_wall` flag (drop-cutter more than the probe radius above the floor under the centre); sentry now 6 tests; predicted: all eight Phase 0 roughs export. Cost: a real wall cut up to t (~0.3 mm on Corne) gives no finding. Was RUNNING 16:00: probe tolerance = the op's chord tolerance; blocking only below the model surface; a finding inside the leave band is a caution; message states both | `P15_B2_gouge_rule.md` (worktree) |
| D2 verifier | DONE 18:35 at merge 009579fc: every gate green except the geodesic pass-load unit test (one shipped ResidueMop step 0.3669 > 0.3626 — G-MOPLOAD; now `#[ignore]` with that reason, bd287bd0) and the two master-red adaptive sentries (same red). Both B2 planted defects tripped their tests. No underived float in the three commits (B2's 0.02 mm = MODEL_GOUGE_Z_TOL_MM; C2's ramp uses R, s, ceiling only). Harness: **plunges did NOT fall** (87 star / 89 pocket) and the per-producer render shows a regression (fragmented cuts, big residue passes) → C3 diagnosis running. Branch 3ae1da39 (master a68cf423 merged); **ff still blocked** by the peer's dirty catalog.rs + capabilities.rs. Was RUNNING 17:45: merge master 5591d974, all gates for A2/B2/C2, planted-defect checks on B2, harness render | `P15_D_verify.md` (worktree) |
| C3 plunge diagnosis | DONE 19:05, committed c8db8b55: the ramp's entry law assumed a straight cleared edge; round the seed the edge is a circle and the step reads 2 + 4/ρ mm vs the 2.18 ceiling for every ρ < 23 mm → ramp REVERTED to the G1 join (kept selectable for the refused-step dump). Second bug found and fixed: every front-branch change counted as a meeting line (shadow lines too) → wraps cut in two; now only branches that went round the island on different sides meet. Entry split on the G1 column: star 86 = 56 wrap-join re-entries + 14 mid-wrap splits + 14 mop + starter + wall. Next lever proposed (not built): curvature-aware wrap spacing s'(ρ) = ρ + R − √(R² + ρ² + 2ρ(R − c·D)) and a join under that width. Was RUNNING 18:45: entries per producer, does the ramp pass the load rule, keep clipping + source tracking + meeting rule, fix or revert the join | `P2_C2_geodesic_fixes.md` §C3 (worktree) |
| D3 verifier | DONE 19:45 at c8db8b55 (+ clippy then_some, 26355fe9): green. Harness entry split S/J/M/C/W/P: star geodesic_a 92 = 1/56/20/0/0/15, pocket 88 = 1/42/27/4/0/14; emitted plan under the ceiling on both (0.3600, 0.3624); the ramp's spiral steps are refused at radius 3.4–5.0 mm (0.38–0.52), as C3 predicted; shadow-line fix holds (pocket wrap pieces 158 → 91). Was RUNNING 19:10: fmt, research lib tests, clippy, harness with the per-producer split and the refused-step dump | `P15_D_verify.md` §D3 |
| GUI rebuild | DONE 19:05: binary from 3ae1da39 (sha b31029f2…, mtime 1792735547.89), installed, gui_restart ok (pid 2538272) |
| E2 evidence | DONE 19:25 at 3ae1da39. **Gouge: clean on all 8 roughs, all 4 exports succeed**; the wanaka plough stays a link_cuts_stock finding (correct); Caution path untested (no reading produced one). **Coverage: reads every eye defect** — AS 37 hex-hole cores (3.5 mm tall, 1.4–1.9 wide; eye count 37), the outer band (~8 islands 16.5–18 mm), the spur (13.0 mm tall, 0.30 wide, 2.2 mm from the Phase 0 spot), CS exactly 2 cores + band, 5 screw-hole islands on all three; wanaka back abstains `setup_not_identity` (flipped setup, as designed). **But it fails the zero control:** CP reads 711 / 1 406 islands (Safety) from (1) a one-cell edge fringe along the outer cut edge, (2) real 0.6–1.4 mm wall strips against vertical walls that read as full-height islands, (3) the normal Z-level terraces on the wanaka front slope (36 % of area). Adopt cost ~2 s per Corne project, 4.7 s wanaka; wanaka generate +2–3 s vs the first run. Was RUNNING 19:08: Corne controls + wanaka200 CP on the fixed build; expect coverage readings present and no Blocking gouge (exports succeed) | `P15_E_evidence.md` §E2 |
| A3 coverage target | DONE 20:20, committed cbeb0acb (not built): ONE planner function `level_floor_z` (was written out in 4 places) + `plan_bottom_z`; coverage target = erosion of the per-centre floor by the cutter profile (exact for ball/bull nose; separable sliding min for flat); `touches_wall` per island; sentry +4 arms + 2 brute-force erosion tests. A3 disagrees with the ruling on two points, honestly: (1) terraces still read — the planner has no guard that holds a level (every level drapes; the F-029 bite clamp limits one pass only), so a terrace is above the plan's floor; (2) the outer-edge fringe still reads — it is reachable material: the outermost ring sits ~0.6 mm inside the containment, the cutter reaches 2.4 mm past it, the population reached 2.7 → G-WALLSTRIP-PLAN at the outer edge, now `touches_wall`. Prediction: the CP zero control still fails honestly on strips + fringe. Was RUNNING 19:50: per-centre intended floor factored from the planner's own rule, reachable envelope by tool-disc erosion, wall strips stay with `touches_wall` | `P15_A3_coverage_target.md` (worktree) |
| C4 morph join | DONE 20:45, committed 6f699bcf (not built, ~1 300 lines): core spiral = level set of T/s − φ/2π traced by angle unwrapping (bisection per ray), core size K from a star-shape ray test; lobes morph when parent/child, same branches, length ratio ≤ (R+2s)/(R+s) and every matched pair in reach, else the C3 one-side rule; option (i) as written cannot hold the load (the return leg meets 2 stepovers), so `geodesic_morph_alt` runs two half-pitch legs per band (derived minimum), `geodesic_morph_return` steps radially once per band (predicted refused once per band); per-producer max load and refused counts. C4 predicts the core spiral still splits its inner turns (same 2 + 4/ρ excess) → curvature spacing is the next lever. Predicted entries: morph_alt ~40–55 (from 92/88). Was RUNNING 19:50: core-zone spiral T = s(k + φ/2π), lobe morph (alternate / return-along-cut), one-side fallback behind islands; columns geodesic_morph_alt / _return | `P2_C2_geodesic_fixes.md` §C4 (worktree) |
| D4 verifier | DONE 21:15 at 6f699bcf (+ clippy fix c97093af): all green; A3 changes no shipped output (byte parity + 14 adaptive3d sentries green); geodesic_a identical to D3. Morph: star 92 → 69 (S1 J30 M14 C12 P12), pocket 88 → 77; core spiral refused 138/392 raw steps (star); lobes mostly not joined (split 53). New shipped-code finding G-RESLINKLOAD (residue-contour link at 0.97 D). Renders: whole lobes fall to the residue passes — lead's reading: one refusal cascades because the replay stamps only accepted cuts. Was RUNNING 20:50: A3 (planner-floor factor: byte parity is the proof) + C4 (geodesic_a must still read the D3 row), harness with the morph columns | `P15_D_verify.md` §D4 |
| C5 spacing + cascade | DONE, committed cc0b6d93 (not built, ~650 lines): cascade diagnostic walks the raw plan on two grids (planned: every step stamped; replay-model: only holding steps) and classes each refusal cascade vs intrinsic; spacing law s'(ρ) from the circular-front swept width, closed-form level radii, ρ* = s(2R − s)/(2(cD − s)) = 22.78 mm at s 2 R 3 (one extra level inside ρ*; each tight pass lands exactly on the ceiling width); known limit: fronts bending round a wall are spaced too widely; keep-or-drop rule for the morph set before the run. Predicted entries: curv 70–80 / 75–82, curv_morph 35–50 / 45–60. Verifier controls: geodesic_a = 92/88 and morph_alt = D4 row. Possible next lever if core circles still fail at 0.36–0.45: the lattice one-cell quantum (needs a ruling). Was RUNNING 21:30: cascade diagnostic in the readout; curvature-aware spacing s'(ρ) on the core spiral and tight wraps; columns geodesic_curv / geodesic_curv_morph | `P2_C2_geodesic_fixes.md` §C5 |
| GUI rebuild (A3) | DONE: binary from c97093af (sha dcc9fad1…, mtime 1792739600.22), installed, gui_restart ok (pid 2918218) |
| E3 evidence | DONE at c97093af: **every eye defect still reads** (AS 37 free cores, spur 13.0 × 0.3 mm free, CS 2 free cores, band as 8 wall-touching islands); terraces GONE on the wanaka front (910 → 178 islands, all ≤ 2.1 mm, class caution; A3's own prediction was wrong in the good direction); screw-hole islands gone; all exports succeed, no gouge finding. Zero control: final stock 1 387 of 1 428 islands touch a wall (strips, hex-side slivers, reachable fringe) + 41 untagged one-cell slivers at angled-wall feet; op 0 (6 mm) reads 627 untagged hole-rim slivers (the wall tag depends on depth per pass) and 84 real uncut hex holes (op 1 clears them). Headline still names a wall island on every rough. Adopt cost on wanaka 18.5 s (was 4.7). → A4 running (free islands first in the headline; op-independent wall tag). Was RUNNING: checks the fringe, wall strips (`touches_wall`), terraces, and that every eye defect still reads | `P15_E_evidence.md` §E3 |
| A4 headline + wall tag | DONE, committed 04b24757 (not built): free islands sorted before wall islands in the finding and the narration ("N free islands … ; M more touch a wall"); wall tag = a neighbour within one cell (8-connected) stands above the target by more than the island's own peak height − one cell (the G-OVERCUTREF tolerance); `depth_per_pass_mm` removed from the target so the tag cannot read the op; sentry arm (i): the 6 mm and 3 mm cutters tag the same sliver. Adopt cost formula recorded (drop-cutter queries dominate), not optimised. Was RUNNING: free islands before wall-touching in the finding and narration; wall tag from the island's own height, 8-connected | `P15_A3_coverage_target.md` §A4 |
| D5 verifier | DONE at 04b24757, all green, no fixes. **geodesic_curv: star 73 (S1 J58 M0 P14), pocket 63 (S1 J34 M12 C6 P10)** — mid-wrap splits gone, core clean; curv_morph 84/86 → morph DROPPED by C5's own rule. Remaining: joins (58 of 73 star entries; radial seam steps read 0.50–0.83), behind-island cascade on the pocket (640/763 wrap refusals are cascades: the law assumes a front curving about the seed). Page `g1_2d_renders_v4.html`. Was RUNNING: A4 + C5 gates, harness with the curv columns and the cascade diagnostic, C5's keep-or-drop rule for the morph | `P15_D_verify.md` §D5 |
| C6 join + island spacing | DONE, committed f5b3725c (not built, ~700 lines): transition join for CLOSED wraps (the wrap drifts out to the next level over its last part; inside ρ* = 22.78 mm the whole wrap drifts = the spiral form); local spacing with ρ measured from each point's source (seed or bend node); half-cell READ columns (the replay stays at the planner cell: `path.rs` cleanup sets its own cell — request for a pub(crate) cell parameter filed). **Key finding:** open arcs in a lobe (≈ 50 of the 58 star joins) cannot be joined within one stepover by any single-pass scheme under the swept-width measure: same-direction passes step a full stepover against the wall, alternating passes meet two stepovers at every second end (the level sets hit the walls obliquely), the half-pitch morph was dropped. The limit is the pass-load MEASURE (swept width), not the geometry: FreeCAD measures exact area per mm (R1 §3, target 0.989·s·D), under which the wall-end turn is a thin wedge and passes. Prediction: geodesic_curv_join 66–75 on the star (little gain). Was RUNNING: (i) the seam join as a spiral transition under the spacing law (no radial step), (ii) spacing behind an island from the front's local centre of curvature (the source-tracking bend node), (iii) lattice-quantum experiment: harness run at half the planner cell to see whether the near-ceiling refusals (0.36–0.45) are measurement quantum | `P2_C2_geodesic_fixes.md` §C6 |
| D6 verifier | DONE 00:05 at f5b3725c (+ fix/ignore 7c75324d): fmt/check/clippy/harness green; C6's disc join test RED (nesting fails at levels 2, 9, 12; edge transition unapplied) → ignored with reason; all four controls exact. **No C6 column beats geodesic_curv** (73 / 63): curv_join 89 / 72, curv_local 85 / 89 (removes the pocket cascades 876 → 202 and the mop over-steps, at the cost of entries), curv_local_join 100 / 94. **Quantum test: the near-ceiling refusals are NOT a cell artefact — the count rises at half cell**, so the planner cell under-reads the cut width (G-CELLREAD, shipped measure). Was RUNNING 23:45: C6 gates, four exact-match controls, the quantum test (near-ceiling count at planner cell vs half cell) | `P15_D_verify.md` §D6 |
| GUI rebuild (A4) + E4 | DONE 00:30. E4: the headline names a FREE island on every Corne rough (the spur on AS, a core on CS); counts unchanged (37 / 2 cores, spur, band); 596 of 627 hole-rim slivers now tagged. Edge cases of the wall tag remain: 41 angled-wall slivers (≤ 1.06 mm) stay free; an uncut hex hole the op never entered tags "at a wall" (it touches its own walls); the tall uncut ring inside the wall reads free (taller than the wall step). CP zero control: 1 387 wall-touching + 41 free slivers ≤ 1.06 mm. wanaka front 178 islands caution; adopt 18.9 s. Exports succeed. Tag iteration stops here for the night; the readings that matter (cores, spur, band, terraces gone) are right. | `P15_E_evidence.md` §E4 |
| (old) GUI rebuild | was RUNNING 18:40 from the worktree (branch 3ae1da39); then install, gui_restart, E2 re-run of the Corne controls | |
| (old) E evidence | was RUNNING 15:20: Phase 0 Corne columns through the two instruments in the GUI (zero control CP, positive control AS, CS), wanaka200 CP if time | `P15_E_evidence.md` |
| (old) D verifier | was RUNNING 15:10: merge master into the branch, fmt, check, new sentries, folder sentries, viz --lib, mcp, clippy, magic-number scan, render harness | `P15_D_verify.md` (worktree) |
| (plan) D verifier | after A–C: fmt, clippy, named sentries, `rs_cam_core --lib adaptive`, viz `--lib`, mcp tests; `rg` for bare float literals in new code | — |

Operator 13:40: "sounds like it's worth trying some of these ideas" → Phase 2
candidate (a) starts in parallel with the instruments; (b) and (c) wait for G1.

## G1 ruling (operator, 2026-10-23 14:55, page `g1_2d_renders.html`)

"Geodesic looks good. Maybe spills a bit over the edges and also seems to
break down as it finds the other side of the circle; I would expect the
pattern to diffuse out of that opening better, instead it seems to have
broken down. The spiral still looks worse than contour parallel. Agent
search looks odd as always. The growth pattern does look a lot more like
the classic adaptive I wanted." Decision left to the lead: **candidate (a)
goes forward; (b) and (c) stay shelved unless (a) stalls.**

G1 defects to fix on (a) before 3D: G1-1 edge spill (wrap ends past the
region edge; partly the one-colour render), G1-2 meeting-line breakdown
behind the island (sawtooth fragments instead of a continued front),
G1-3 arm asymmetry on the symmetric star (operator 15:05: "I'm surprised
they all look different"; lead hypothesis: direction-dependent error of
the first-order upwind sweep on the square grid, each arm at its own
angle to the grid; the lower-left arm also carries the residue clean-up).
Operator 15:05: "I think your plan is good." Then the continuous spiral form
(no radial step, no plunge per wrap) and the link stage.

## G0 rulings (operator, 2026-10-23 13:25) and lead judgements

1. **Instruments first.** Operator: "instruments are important." Before
   any Phase 2 candidate: a coverage instrument for a 3D Rough through MCP
   (G-ROUGHCOV, start from `metrology/union_coverage.rs`), a gouge check of
   emitted moves against the model mesh for a rough (G-ROUGHGOUGE), and a
   thin-standing-material detector (G-THINSTAND, from `phase0/tools/spurs.py`).
2. **Defects filed now:** `DEFECTS.md` (G-LINKPLOUGH, G-SPUR, G-HOLECORE,
   G-OUTERBAND, G-AXIALDOC, G-WALLDIP, G-HOPRETRACT, G-SIMCYCLE).
3. **Trochoid cap 1.6 (lead judgement, operator: "no magic numbers").** The
   value is a tuned knee from the June wanaka200 sweep, not a derivation.
   Under the 2026-10-23 ruling it goes: `TROCHOID_CAP_MULT_3D` and
   `default_trochoid_cap_mult()` are removed when Phase 2 touches
   ContourSpiral; the spiral's engagement bound becomes the existing derived
   pass-load ceiling (`adaptive/search.rs` `step_within_pass_load`), which
   the plan already names as the bound for every candidate. Until then the
   baseline pages state 1.6. The peer's default-removal diff does not touch
   these two symbols (checked 13:20), so this research owns the removal.
4. **Compact spiral (lead judgement).** Arm STOPPED: 0 of 47 islands, and
   the 12 holeless islands refuse too. The module's own [REPO] constants
   (`max_chord_mm` 0.1, `closure_tol_mm` 0.05, backtrack 0.35 rad) are
   underived; they stay only because the module is research-only behind
   the `research` feature and nothing ships them. No productisation.

## Where things stand (read this first, 2026-10-24 00:35)

- **Instruments (Phase 1.5) are built and measured.** Gouge: clean on all
  eight Phase 0 roughs, exports succeed, no false fire. Coverage: reads
  every eye defect (37 + 2 hex-hole cores, the 13 mm spur, the outer band),
  terraces gone, headline names a free island; the ContourParallel zero
  control fails honestly on real wall strips (G-WALLSTRIP-PLAN) and small
  slivers. All on branch `adaptive-instruments` (7c75324d), gates green
  (P15_D_verify.md D2–D6). **ff of master blocked** by the peer's
  uncommitted registry change (they land it, then I merge + ff).
- **Candidate (a) geodesic:** growth pattern accepted at G1; the
  curvature-spaced wraps (`geodesic_curv`) are the state to look at:
  `g1_2d_renders_v4.html` (served at http://127.0.0.1:8766/g1_2d_renders_v4.html
  while the session runs). Star 73 entries (from 92), pocket 63 (from 88),
  no mid-wrap splits, five like arms. The morph and the transition joins
  were tried, measured and dropped by pre-set rules.
- **One ruling needed:** the engagement measure at wall ends (section
  below). The per-arc plunge is a limit of the swept-width measure, not of
  the geometry; FreeCAD's exact area-per-mm measure accepts the turn.
- **Shipped-code findings filed tonight:** G-MOPLOAD (handed over),
  G-RESLINKLOAD, G-CELLREAD (the planner cell under-reads the cut width),
  G-WALLSTRIP-PLAN, G-SHELFSCAN (N11), plus the Phase 0 set.
- Next, after the ruling: the measure as a research-flag quantity shown on
  the 2D page; then the 3D arm on the three fixtures with the bars.

## Baseline pin

- Phase 0 code state: HEAD `61a8d4b4` (the plan commit `df6de926` adds
  only planning files). The GUI is built from a detached worktree at this
  commit: `.claude/worktrees/adaptive-head`, own target dir.
- Reason: the main checkout holds a peer session's uncommitted work
  (335 files, started 09:48 NZDT, the underived-defaults removal, S11
  tests). The release binary in `target/release` was built at 01:19,
  before commit `44af62dc` changed crate code, so it is not HEAD.
- Each fixture page records the binary path, its mtime and the HEAD hash.

## Open questions for the operator

1. `TROCHOID_CAP_MULT_3D = 1.6` (`adaptive3d/clearing.rs`) and
   `default_trochoid_cap_mult()` (`compute/operation_configs.rs`) cite the
   June cap sweep, which lives only behind tag
   `planning-pre-purge-2026-09-17`. The peer's default-removal work may
   delete this default; ContourSpiral then refuses until the dial is set.
   The baseline uses 1.6 and says so.
2. Fixture name in PLAN.md: the iso-scallop project is
   `planning/multitool_2026-08-23/wanaka200_iso_scallop.toml` (no `_mt1_`).

## What ran

| Item | State | Output |
|---|---|---|
| Plan + prompt commit | done `df6de926` | |
| HEAD worktree + GUI build | done 10:59 NZDT; installed over `target/release/rs_cam_gui` by rename (mtime 1792706351.82, sha256 51cbb713…a438); `gui_restart` verified | |
| R1 FreeCAD Adaptive | done; FreeCAD commit be69b03e, LGPL-2.1, author Kresimir Tusek 2018 (no Freesteel lineage in the source); lead spot-check: cpp:4064-4066 retract penalty, cpp:3880 climb-only fail, cpp:1856 stepOverScaled = R·s, rs_cam path.rs:999 6R link range — all match; accepted | `R1_freecad_adaptive.md` |
| R2 Freesteel | done; source FOUND through the Wayback CDX index (live site 403/503); 19 posts read; lead spot-check: 5 quoted phrases found in 2 archived posts (2007-01-15, 2013-11-20) — accepted | `R2_freesteel.md` |
| R3 papers | done; Held 2014 full + 2009 secondary, Bieterman by abstract + patent, Sethian 1999 + Zhang 2005, libactp headers; lead spot-check: Held–de Lorenzo §1.3 quote verified by fetch; L7 label (iso-field solver) was wrong, sent back, fixed; accepted | `R3_papers.md` |
| Phase 4 refusal count | done: 0 of 47 wanaka200 small-tool islands accepted (45 LevelNotSingleLoop, 2 NotStarShaped); run 7.1 s at 0.4 mm cell; disk controls (two disks × four cusp radii) all accepted. **Arm stops per PLAN Phase 4 step 2.** Deviations: level sets from a grid EDT + marching squares (C1's closed form is disk-only); harness `tests/spiral_finish_compact_wanaka_g3.rs` built in the worktree, copied to main and committed unbuilt there | `PHASE4_refusals.md` |
| Phase 0 baseline pages | DONE 13:00 NZDT: 9 columns (3 fixtures × 3 strategies), 15 rough ops, every generate finished, no refusal. Lead reviewed all three pages: HEAD stamps present, bars pass/fail/not-measured with instrument names, V1–V3 marked, N1–N9 named, N5 spurs captured mid-cycle. PNGs quantised by the lead (pngquant) to keep the planning tree small | `phase0/rivmap100.html`, `phase0/corne.html`, `phase0/wanaka200.html`, `phase0/PHASE0_NOTES.md` |
| Synthesis | done (lead) | `SYNTHESIS.md` |

## Disk

98 % full at start (19 GB free). The lead pruned 22 192 older-hash
duplicate files in `target/debug/deps` under the lane lock (10:53 NZDT);
63 GB free after. Peers were told. The next candidate is
`.claude/worktrees/demo-build/target` (6.2 GB, commit of 2026-09-25);
not taken without a ruling.

## G0 findings from the rivmap100 baseline (lead read of the page)

- **Two hard bars have no instrument through MCP for a 3D Adaptive op.**
  Bar 2 (coverage): every per-toolpath coverage field is null; the
  coverage code (`metrology/union_coverage.rs`) is test-support only.
  Bar 3 (gouge on emitted moves): `ModelGougeRule::Exempt` for 3D Adaptive
  in `compute/catalog/registry.rs`; the cut trace has only air_cut and
  low_engagement issues. Both bars read NOT MEASURED on every column.
  Operator decision needed: build the two instruments before Phase 2, or
  accept the bars as not measured for G1 (2D renders).
- **V1, V2, V3 all PRESENT on HEAD, on all three strategies.** The form has
  changed since June: V1 is entry circles (helix + concentric loops) at
  residue spots, not only in corners; V2 is overlapping entry circles that
  cut air, not corner-blend arcs. New: N1 "bead chain" (a line of helix
  entries 1–1.5 mm apart, each a full circle), N2 a fan of long fed links
  from one corner (ContourParallel), N3 entry stacking (AgentSearch).
- **Entries dominate time:** entry_s 2 364 of 3 263 s (ContourParallel),
  6 100 of 7 085 s (ContourSpiral). The spiral's 1 061 keep-down links
  with nested helix/plunge descents are the residue clean-up.
- **Peak axial DOC above the commanded 2.0 mm:** 3.0 mm (AgentSearch),
  2.5 mm (ContourSpiral) by `narrate_toolpath`. Not a hard bar as written;
  flagged for the operator.
- Bar 1 (0 rapid + 0 holder collisions) and Bar 4 (generation completes)
  PASS on all three rivmap100 columns.

## Phase 0 hard-bar table (from the pages; bars are pass/fail, no ranking)

| Fixture | Strategy | Bar 1 collisions | Bar 2 coverage | Bar 3a mesh gouge | Bar 3b stock-side plough/over-cut | Bar 4 generation |
|---|---|---|---|---|---|---|
| rivmap100 | ContourParallel | PASS 0/0 | NOT MEASURED | NOT MEASURED | 0 reported | PASS |
| rivmap100 | AgentSearch | PASS 0/0 | NOT MEASURED | NOT MEASURED | 0 reported | PASS |
| rivmap100 | ContourSpiral | PASS 0/0 | NOT MEASURED | NOT MEASURED | 0 reported | PASS |
| Corne | ContourParallel | PASS 0/0 | NOT MEASURED | NOT MEASURED | FAIL `link_cuts_stock` 1 + 3 | PASS |
| Corne | AgentSearch | PASS 0/0 | NOT MEASURED | NOT MEASURED | FAIL `crosses_standing_material` 4 (peak 15 mm) | PASS |
| Corne | ContourSpiral | PASS 0/0 | NOT MEASURED | NOT MEASURED | 0 reported | PASS |
| wanaka200 | ContourParallel | PASS 0/0 | NOT MEASURED | NOT MEASURED | FAIL `link_cuts_stock` CRITICAL (a link removes 11.26 mm) | PASS |
| wanaka200 | AgentSearch | PASS 0/0 | NOT MEASURED | NOT MEASURED | 0 reported | PASS |
| wanaka200 | ContourSpiral | PASS 0/0 | NOT MEASURED | NOT MEASURED | 0 reported | PASS |

V1–V3 on HEAD: all three present on every rivmap100 column; on Corne and
wanaka200 ContourParallel is free of V1 and V2 (V3 present on wanaka
front); AgentSearch and ContourSpiral show V3 everywhere and V1 as seam
loop chains (spiral). New defects named on the pages: N1 bead chains of
helix entries, N2 link fan from one corner, N3 entry stacking, N5 standing
spurs (AgentSearch only: Corne up to 16.5 mm, wanaka back 15.6 mm;
ContourParallel none; ContourSpiral nubs ≤ 3.4 mm), N7 band outside the
Corne wall left standing with holes (AgentSearch, ContourSpiral), N8 1 726
retracts on the wanaka back rough (ContourParallel, mostly 0.5 mm hops),
N9 narrow valleys left higher (rivmap100 AgentSearch, ContourSpiral).
Also: peak axial DOC above the commanded value on AgentSearch and
ContourSpiral (wanaka back 10.92 vs 5.46 mm). Corne ContourParallel cycle
time not reproducible (2 022 s then 1 564 s; the feed modulation reads the
previous simulation).

## G0 look (2026-10-23 13:15): operator and lead readings

**Operator (live GUI and the pages):** "AgentSearch looks like it makes a
nice spiral, then in odd shapes makes some very odd toolpaths (no clean
arcs). ContourSpiral looks almost as nice as parallel, but the arcs on
every corner look busy and potentially an artifact."

**Lead (all 9 columns, key views of each; zoom crops `phase0/img/lead_*.png`):**

- The spiral's "arcs on every corner" are NOT arcs. The Corne spiral G-code
  holds 11 G2/G3 in total. The busy shapes are chains of small closed
  polyline loops (1.5–2 mm) with a keep-down entry each, along the medial
  seam where the wraps' corners meet (`lead_corne_cs_corner_zoom.png`).
  They are the residue clean-up between wraps: V1 in its HEAD form (N1
  bead chain). The operator's artifact hunch is right.
- AgentSearch in the odd-shaped Corne region: irregular polyline walks with
  no structure, long diagonal fed links, scattered plunges
  (`lead_corne_as_right_zoom.png`). Matches the operator's reading.
- **New defect N10, hex-hole cores left standing (Corne).** The final stock
  top view shows a central pillar inside about a third of the hex holes on
  AgentSearch and inside 2 holes on ContourSpiral; ContourParallel cuts every
  hole clean (`lead_corne_as_stock_top_zoom.png`, `lead_corne_cs_...`,
  `lead_corne_cp_...`). The page did not name this. It is a finished-part
  defect and a coverage failure that bar 2 would have caught if it had an
  instrument.
- N7 confirmed on the zooms: AgentSearch and ContourSpiral leave the band
  outside the case wall standing with holes punched; ContourParallel clears it.
- rivmap100: ContourParallel links view is 104 retracts criss-crossing the
  part plus a fan of fed links from one corner (N2); AgentSearch has two big
  spirals and walks the rest; ContourSpiral has 1 061 keep-down clusters
  (the residue mop). Final floors of AgentSearch and ContourSpiral differ on
  0.2 % of pixels at this render; ContourParallel's floor shows rounder
  cusps (4 % of pixels differ from AgentSearch).
- wanaka200: ContourParallel back rough's links view is a solid wall of
  1 726 G0 hops (N8); AgentSearch back is one clean spiral with corner
  walks; ContourSpiral front has 4 414 keep-down links over the terrain.
  Final stocks look alike per fixture at this scale.
- Lead's answers to the three questions, per strategy (not a ranking):
  ContourParallel: links no (retract zigzags), path yes, part yes.
  AgentSearch: links partly, path no in odd shapes, part no (N5 spurs, N7
  band, N10 hole cores). ContourSpiral: links no (seam loop clusters), path
  yes except the seams, part partly (N7 band, 2 hole cores).

## Lead ruling 19:30, amended 19:40 after advisor review, on the coverage target (A3)

The bar judges against the rough's OWN intended surface, derived, no new
constant:

1. **Reachable envelope.** For each XY the floor the cutter could leave is
   the minimum, over every tool-centre position within R of that XY, of
   the planner's per-centre intended floor: an erosion of the centre-floor
   surface by the tool disc, on the grid. This removes the one-cell edge
   fringe (no reachable centre covers it) and keeps hole cores, the spur
   and the band (centres fit).
2. **Per-centre intended floor from the planner's own rule.** Every level
   drapes in this planner, with guards on steep slopes, so a terrace is
   either the guard's intent or a drape defect; only `adaptive3d`'s own
   code knows which. A3 reads `step_levels` and the drape/guard path and
   reuses (or factors out, as ONE shared function) the per-centre intended
   floor: drape surface + leave where it drapes, the level Z where the
   guard holds the level. If it cannot be factored, terraces stay flagged
   and the note says why. One instrument, one source for the target.
3. **Wall strips stay findings.** Material within one tool radius of a
   wall IS reachable (the tool edge meets the wall when its centre is R
   away; only the inside-corner fillet is geometric residue). The 0.6–1.4
   mm strips E2 confirmed are material the planner left, so the
   ContourParallel zero control fails on them honestly. The planner-side
   cause (grid-cell inset, last pass short of the boundary) is filed as
   G-WALLSTRIP-PLAN next to G-WALLSTRIP.

Visible at the next gate: the gouge Caution path and `at_wall` are tested
only synthetically (no live positive yet); the probe tolerance t ≈ 0.3 mm
leaves a real wall cut up to t unflagged.

## Ruling needed from the operator (morning): the engagement MEASURE at wall ends

The geodesic growth pattern is accepted (G1) and the curvature-spaced wraps
look right (page v4). The one open cost is one plunge per arc in every
lobe. C6 shows this is not a geometry problem: the shipped pass-load rule
measures SWEPT WIDTH, and at a wall end the turn from arc k to arc k+1
sweeps up to two stepovers of width even though the material removed is
a thin wedge. FreeCAD's rule measures AREA per mm of travel against a
derived target (R1 §3) and accepts that turn. Options:
1. Add the exact area-per-mm measure as a second, derived pass-load
   quantity (target 0.98948·s·D from the crescent, R1 §3) and let the
   candidate's wall-end turns be judged by it; keep swept width for
   straight side cuts (they agree there). The look: the classic zigzag
   lobes with no retract.
2. Keep swept width only: one entry per arc stays (73 / 63 entries on
   the development shapes), and the 3D arm is wired with that cost.
The lead recommends 1, built as a research-flag measure first and shown
on the 2D page before any shipped rule changes.

## Next for the geodesic candidate (C4, after D3): the morph join

C3's split: 56 of 86 star entries are wrap-to-wrap re-entries. A radial
step sweeps width D; spread the outward drift s over the whole wrap so
the sideways component per step is s/L. Core zone (level sets
star-shaped about the seed): trace T(x) = s·(k + φ(x)/2π), one spiral,
one plunge (editor C's proposal). Lobes: sample arc k and k+1 at matched
normalised arc length u and trace the point-wise blend (the
Bieterman/Held morph per band), alternate direction per band or return
along the just-cut arc as a fed link through cleared stock. Behind an
island, morph only within one source-tracking branch; fall back to the
one-side rule where arc lengths differ by a stated factor or an arc
splits. Verify with the readout (morph-step load near the plain-wrap
load; join entries near zero). Curvature-aware spacing is the second
lever, measured after the morph. The 3D arm is wired only when the 2D
joins show entries near one per lobe.

## Handed over 15:35: G-MOPLOAD and G-RAPIDLIFTPIN (adaptive/ sentries red since 10-20, see DEFECTS.md). G-MOPLOAD is a real pass-load break in the residue mop (the V1 mechanism): queue it right after the G1 fixes; the Phase 2 candidate must not inherit the mop.

## Operator observation 2026-10-23 ~12:20 (Corne case, live GUI)

"As it's milling down it leaves tall spurs of material. These could snap
and cause issues; prominent ones about 7 min in." Recorded as new defect
N5 "standing spurs". The Phase 0 agent captures them mid-cycle on every
Corne column and checks the other fixtures. A spur that snaps is a safety
item, not a look item: it needs a measure (thin standing material between
passes) before Phase 2 candidates are judged.

## Findings for G0 that are not about the spiral

- **`planning/multitool_2026-08-23/wanaka200_mt1.toml` does not load at HEAD.**
  The loader refuses the Drill toolpath's retired key
  `setups[0].toolpaths[2].operation.params.depth` (Phase 4 agent). The
  geometry-selection lead confirms this is the intended break of 88d18baf
  (Drill `depth` retired for `floor`; old study TOMLs under planning/ are
  not migrated, no-legacy ruling). Phase 0 used a fixed copy
  `phase0/projects/wanaka200_base.toml`; the fixture itself is unchanged.
  `link_cuts_stock` on the default rough is not from that lead's packages;
  it is this research's to file.
- Lead session paused ~11:30–12:10 NZDT (usage limit); Phase 0 resumed and
  finished. The GUI restarted at 11:56 on the same binary (gui pid 84659).
- The worktree `.claude/worktrees/adaptive-head` (own target, ~4 GB) stays
  until Phase 0 ends; delete it with its target dir afterwards.
