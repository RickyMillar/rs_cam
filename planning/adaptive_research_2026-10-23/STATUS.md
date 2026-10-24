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
| C7 area measure | RUNNING 00:00 (24th): exact area-per-mm step measure on the planner grid under the SAME ceiling c·D; research-only replay variant (`PassLoadMeasure` threaded through path.rs, shipped sites unchanged; byte parity is the proof); columns geodesic_curv_area, agent_area, contour_spiral_area; the L-step-at-a-wall unit test is the claim the ruling rests on | `P2_C2_geodesic_fixes.md` §C7 |
| C7 area measure | DONE 00:50, committed 8deecf98 (not built): measure = standing stock in the step capsule outside the start disc × cell² per mm; same ceiling c·D; threaded `PassLoadMeasure` (shipped sites SweptWidth). **Result: the ruling's premise fails** — head-on steps exceed the ceiling in both measures; circular fronts read identically (area/mm = s + s(R − s/2)/ρ = the C3 swept-width formula). The measures differ only on slivers either side or a grazed lump. Columns geodesic_curv_area, agent_area, contour_spiral_area with both readings. | `P2_C2_geodesic_fixes.md` §C7 |
| D7 verifier | DONE 01:30 at 8deecf98: all green, no fixes; shipped neutrality proven (byte parity, 14 adaptive3d sentries, both clippy configs); master reds unchanged. The area columns are NOT evidence: the per-mm reading is unbounded on short steps (raw up to 7.85 D), entries rose 1.3–4.8× under it → moved to EXTRA with a step-length floor (C8 first item). Was RUNNING 00:55: shipped neutrality of the threading (byte parity, 8 adaptive + 14 adaptive3d sentries, clippy with and without research), C7 tests, harness controls + area columns | `P15_D_verify.md` §D7 |
| C8 link + lead-in | DONE 00:40 (24th), committed 71f4cf0b (not built): a lead-in tangent at the arc START cannot work against a wall (it would begin beyond the wall, or leave a growing corner wedge), so each band is entered MID-ARC: link along the cut arc, lead-in tangent at P, cut P→F, link back to P, cut P→E (the leftover wedge's bite grows from zero). Lead-in radius derived from the swept-width ceiling: ρ_L ≥ R + (h² − m²)/(2m) ≈ 25.7 mm on a straight band (the tool radius FAILS: 3.67 vs 2.18), ≈ 23–24 mm on curved fronts, checked on exact circles (worst width 2.1755 vs 2.1756); the span is long (12–32 mm), and inside ρ* there is no margin so no lead-in. Honest prediction: few star lobes fit → star 75–90, pocket 60–80 entries; one plunge per lobe NOT expected. Area columns parked (EXTRA, one-cell step floor). Was RUNNING 01:35: one-direction lobe arcs, keep-down return LINK through the just-cut band (existing link check, zero load), tangential LEAD-IN arc into the next band (radius derived from s and the ceiling); column geodesic_curv_lead; target one plunge per lobe | `P2_C2_geodesic_fixes.md` §C8 |
| D8 verifier | DONE 01:00 (24th) at 71f4cf0b: green after a test-only fix (match arms for LobeMorph::Lead in a C4 test); controls exact. **The lead-in never applied**: 0 lead-ins on both shapes, every band fell back to the seam (radius range "0.0 to 0.0"); geodesic_curv_lead = 82 / 73 entries only because of the core spiral. Open: find the guard that rejects every lead-in before reading this as evidence for or against the FreeCAD lead-in. Pocket mop still 2 steps over (0.4677). Was RUNNING 00:45 (24th): C8 gates + harness, geodesic_curv_lead row | `P15_D_verify.md` §D8 |
| Operator look at farside_ideas v1/v2 (24th) | "Harmonic looks amazing but overlaps the same area a lot — could a post-process make it more efficient? The pure geodesic ones look very very nice; (a) exact geodesic, curvature read locally, looks like perfect lines if linked well. Distance ratio: not sure it is a fair comparison; bugs out on the pocket (does not wrap the circle). (a) with the round union (c) looks really nice." Lead: the ratio's pocket failure is its second basin, not a code bug; harmonic overlap is intrinsic to level sets (trimming re-creates open arcs). **Plan: C16 = (a) spacing in Rust after D11, then C14 turns on (a), then (c); C17 = streamline march prototype (harmonic direction, even gap)** | — |
| C17 streamline march | v3 DONE (24th), lead re-ran `assert_c17.py` (OK): one closed loop per level on star (30) and pocket (49, one loop along the island front, fronts stop in air either side of a corridor); interior width 0.3622 D star / 0.3741 D pocket (within one cell); 73 / 38 samples over c, all within R of a wall (max 0.5133 D); coverage as (e). **But no gain in length:** 1.00× (e) on the star, 0.98× on the pocket (6.1× / 8.2× (a)); 74–82 % of the length is wall and corridor runs, because each closed loop retraces the wall earlier loops reached. Verdict: the overlap comes from the closed loop itself, not the field; not a Rust candidate. **Additions DONE (page v3 §4–§5; lead re-ran asserts OK):** engaged/air at the cut feed 1500 mm/min (fixture_feeds; the 2D keep-down link feeds at the cut feed, path.rs; a separate link feed exists only in the link-moves dressup, 500 mm/min): star (a) 933 / 9 mm, 46 retracts, 0.63 min; star (e) 2389 / 3119 mm, 0 retracts, 3.67 min; pocket (a) 1317 / 8, 27 retracts, 0.88 min; pocket (e) 3852 / 6776, 0 retracts, 7.09 min (the march: 3.67 / 6.88 min, cut parts over c where loops leave walls). Family u_λ = (1−λ)T/Tmax + λ·u_harm: **every λ < 1 needs retracts** (≥ 5 star, ≥ 14 pocket; late levels reach walls as open arcs with no keep-down route) → λ* = 1 = (e). Break-even: (a) beats (e) unless a retract costs more than ~4.0 s (star) / ~13.6 s (pocket). (a)'s retract count here is without C14 turns (FreeCAD's rule). `sweep_paths.npz` (11 MB) not committed; `sweep.py` rebuilds it. Brief was loops advanced along ∇u streamlines of the harmonic field by the C5 law gap per point; points stop on walls; closed loops, even gap? **Added (operator: the harmonic overlap is keep-down travel that sways back and forth chipping at the cut; retracts should not happen):** split length into ENGAGED vs AIR with the raster replay, time at cut feed and link feed (information only), retract count by R1 §4's routing rule. **Second addition (operator: "dial it in so that overlap is only useful linking"):** each closed loop = CUT parts + keep-down LINK parts (two colours); family u_λ from geodesic (a) (λ 0) to harmonic (e) (λ 1); λ* per shape = minimum estimated time subject to no retract and cut parts ≤ c (computed, not typed); render λ 0 / λ* / λ 1. Operator rulings also: (c) with a smaller radius — lead proposal: radius from the machine's acceleration at the cut feed (F²/a, machine profile), capped by C15's load bound 2.68 mm; smoothing reuses `blend_corners_to_moves` (C14 gap: blend arcs at a turn keep full feed) | `P2_C17_streamline_march.md`, `farside_ideas_v3.html` |
| C15 far-side ideas | v1 DONE (24th), Python only; lead re-ran `assert_c15.py` (183 page + 51 note claims hold). Exact shortest paths at 0.1 mm. **Hypotheses:** (1) lattice only small (lattice wraps within 0.47 mm of exact, along-wrap SD ≤ 0.11 mm); the real cause behind the island is the SPACING: at ρ = R + T the exact wraps read 0.4129 D (C5's known limit) and start the cascade; (2) the jagged seed is the main cause of C11's wobble (along-wrap SD up to 1.29 mm); (3) the V is a corner (r 0.87 mm), costs arcs not shape. **Ideas:** (a) one field from the seed, each level spaced by its OWN curvature (3-point radius over the pass step) → 0.3574 D, +3 levels, no residue, far side = involutes continuing the near-side circles — RECOMMENDED; (b) harmonic shadow fill rejected (all wraps run into two points, 1181 mm cut); (c) round union: derived fillet bound r ≤ 2.68 mm (no V, holds load, +4 levels); (d) one front from the shadow line, 23 arcs, 0.3694 D; (a)+(c) lowest load 0.3280 D, fewest open arcs (63). Next Rust step proposed: a `Spacing` that keeps ONE T per level and bisects ΔT so every contour point is within min(s, s'(ρ_local)) of the cut. Arc ends read 0.63 D even on the near side → C14's class. Whole-region morphs DONE (§0 of the same page; lead re-ran asserts: 274 page + 51 note claims hold): (e) harmonic gives one closed loop per level on the star but 5.2× the v4 cut (the gap follows 1/|∇u|, so the widest gap forces crowding elsewhere) and 0.80 D near 4 tips; pocket 7.8× cut, late levels split round the island; (f) distance ratio fails load (0.90–0.91 D at corners) and on the pocket leaves 2120 mm² (a second basin). **Verdict: a level set of ONE field cannot give an even gap along a closed loop → neither is a next step; recommendation stays (a), then (c)** | `farside_ideas_v1.html`, `P2_C15_farside_ideas.md`, `phase2/c15/` |
| Branch | c3d38bf4: C12 (3D arm, load test ignored with `evidence:` D11) + C14 (turns) committed; D11 addendum: of 128 C12 over samples 81 filter gaps (22 vertical lifts, 25 wall pass at ~R, 34 per-sample plunge exemption), ≤ 2 RDP, 0 links-as-cuts, 47 real (worst: the starter lap after a straight plunge, helix radius 0 in the 3D slice); the AgentSearch control has 224 real | `P15_D_verify.md` D11 addendum |
| C19 march: clip, split, per arm | DONE for the march (24th), Python; lead re-ran `assert_c19.py` (402 quotes, 298 recomputed: ALL OK). Clip keeps the march shape (pieces on the C17 loops to 1.4e-14 mm); split: star at loop 8 into the 5 arms, pocket at loop 5 (stock splits at levels 31/33). **Finding: the wall-departure overload is a hidden turn** — each piece starts with a ~2 mm head along the wall into new stock (0.70 D star, 0.54 D pocket); made the counted turn (feed c / max(w, a)), but the march meets walls at θ ≈ 56–59°, so 51 / 37 of these turns exceed the C14 bound and need the slanted bound s/cos θ + (1+√2)·cell (it fits 51 / 34). Remaining 0.62 / 0.42 D just after the departure; proposed fix: extend the turn ~1.3 mm (fits 36/39, 26/31). **Card plan (head as turn, slanted bound, one-way), times at cut feed:** clipped march 1.51 / 2.27 min, plunges 8 / 7 (core ring seams + 2 turns), vs (a) with the same links 1.10 / 1.70 min, plunges 17 / 12 (Python replay reads harder than Rust: D12 has (a) 34 / 41 entries); full march 3.67 / 6.88. Air 4084 → 738 mm (star), 8584 → 1577 (pocket); engaged 1243 vs 893 (star), 1594 vs 1251 (pocket). With the literal C14 bound the march has 59 / 42 plunges. **CORRECTION (C19, page v6; v5 replaced by a pointer):** a builder defect (after a plunge the one-way return went to the piece start, not the wall departure point) chained turns over the bound; corrected, the slanted bound is needed by only march 2 / 5, harmonic 0 / 8, (a) 0 / 3 turns (star / pocket) → the lead's ruling (keep the C14 bound) stands. Card plan (head as turn, C14 bound): march 1.50 / 2.25 min, entries 74 / 65; harmonic 2.16 / 4.15 min, entries 35 / 23; (a) 1.10 / 1.68 min, entries 85 / 75; break-even per entry vs (a): harmonic 1.3 / 2.9 s, march 2.2 / 3.4 s; v6 asserts 758 quotes ALL OK. Superseded v5 text follows: **Harmonic ADDED (same page v5, 648 quotes, asserts ALL OK):** clipped (e) star 2.16 / pocket 4.20 min, entries 35 / 15 (fewest), median cut width 0.12 / 0.06 D, engaged 2.6× / 3.1× (a); pocket holds c everywhere. Break-even per entry vs (a): clipped (e) 1.3 / 2.6 s, clipped march 2.1 / 2.8 s (Python replay counts). `paths_c19.npz` (9 MB) not committed; `run_c19.py` rebuilds it | `P2_C19_march_arms.md`, `farside_ideas_v5.html`, `phase2/c19/` |
| C20 degenerate turn + 3D departure | CODE DONE (24th), not built; lead reviewed by path. Cause of the 0.048 / 0.035 mm unfed turns: a lattice cell with one corner inside the island's tool-centre disc loses its contour segment, so a level splits into a 64-point arc and a 3-point tail; the one-way turn IS that missing segment (engages nothing, factor 1), and the emitter's 1 µm collinear drop removes its vertex. **Readout fix, decided AFTER the D12 result (stated openly):** a turn with no move on it counts as unfed only when its feed factor is < 1 (factor 1 = full feed F, which every clearing move already carries); such turns go to a new count `full_feed_unmatched`. The zigzag gap turn (factor 0.7158) still counts. D13 must confirm the < 1 µm drop. New REQUIRED column `geodesic_local_zigzag`. 3D test: departure exemption now as the 2D walker (each move exempt while it starts within R of the vertical plunge; the first move starting at R or more ends it; path.rs:544/551/655/675); one documented difference (3D path does not mark where a 2D cut ends) | `P2_C2_geodesic_fixes.md` §C20 |
| D12 addendum | Heavy scallop tests GREEN at c60ed442 (heavy-tests,test-support, no research): scallop_isofield_gouge_m4 2 passed / 3 ignored (222 s), scallop_candidates_m4 3 passed / 4 ignored (139 s) → C16's split of iso_contour_where leaves the shipped iso-scallop finish unbroken (operator approved the run) | `P15_D_verify.md` D12 addendum |
| D-FIX1 (3D fixtures) | Corne DONE at cdc9814e (page `fixture3d_v1.html`, opened for the operator): geodesic_curv WITHOUT turns vs AgentSearch — cycle 2749 vs 2112 s, cut 65.1 vs 38.5 m, Rough 1 entries 400 vs 132, standing islands 928 (5250 mm²) vs 457 (6833 mm²), 0 collisions; geodesic wraps visibly ordered (concentric, follow the case). wanaka200 RUNNING (operator approved 900 s per run). Deviations: --accept-unmodeled-tool-load for the export only; Drill depth→floor migration in the NEW wanaka copy only | `P15_D_verify.md` D-FIX1, `phase3/fix1/` |
| D14 | QUEUED after wanaka: C22 + C23, then Corne with geodesic_local_zigzag / oneway at the new commit | — |
| C22 3D arm gets (a) + turns | CODE DONE (24th), not built; lead reviewed by path. Env values geodesic_local_oneway / geodesic_local_zigzag; `slice_segments` returns the turns, the 3D slice runs the 2D cleanup with them; `apply_slice_turn_feeds` writes F·factor on the 3D moves; **finding: C12's arm lifted links through cleared stock as air → entries (the cause of Corne's 400); fix `HeldLinks` (research arm only) keeps them at depth** — changes the geodesic_curv / split 3D outputs, no shipped arm. C12 test now 4 modes (~12 min, heavy); new plain test. Risk: 3D RDP 0.1 mm can drop a short turn's vertex| `P3_C22_3d_turns.md` |
| C23 pocket far corner + island-side plunges | CODE DONE (24th), not built; lead reviewed by path. Island plunges: behind the island the levels are involutes that run head-on into the island; the arc before leaves a lens that reads (R + Δ/2)/2R = 0.62–0.66 D; one-way climb puts the island at the END of every lower arc → refused last 0.6–2.1 mm → plunge (11 entries traced). Fix `ArcEnds::HeadOn`: start each arc at the end with the larger lens reading (threshold one reading step cell/D), so the turn cuts the lens. Far corner: the split meeting rule alternates upper/lower arcs → long links; fix `MeetingRule::Through` (the operator's queued merge) + head-on. No fillet (any r ≤ 2.68 would be tuned). Columns *_v2 and *_headon. Predicted: one-way v2 24–32, zigzag v2 22–32 pocket entries. Operator decision: some one-way arcs become conventional; zigzag v2 takes the return path in head-on lobes| `P2_C2_geodesic_fixes.md` §C23 |
| D13 verifier | DONE (24th), committed on the branch: **geodesic_local_oneway KEEP (34 = S1 J19 P14 / 41 = S1 J29 P11), geodesic_local_zigzag KEEP (38 = S1 J23 P14 / 44 = S1 J30 P13)**; every turn fed; only mop over steps; zigzag saves 716 / 764 mm keep-down for +4 / +3 entries; one zigzag star turn planned but not emitted (unexplained). C20's cause confirmed (every unmatched turn end < 1 µm off its holding move). Byte compare: exactly C20's 11 readout files differ, PNGs equal. **C12 3D oracle valid:** 0 over the exact bound in both modes, AgentSearch control 225; heavy (461 s). One clippy fix (manual_let_else in a C20 test) | `P15_D_verify.md` §D13 |
| D12 verifier | DONE (24th), branch c60ed442: all green; controls EXACT (30 .txt byte-equal to D11). geodesic_local 78 / 68; **geodesic_local_oneway 34 = S1 J19 P14 / 41 = S1 J29 P11**; geodesic_curv_oneway_whole (chaining fix only) 30 / 46. Whole chaining removes 9 entries in the broken arm (12 → 3). **Rule on local_oneway: (1) holds, (2) holds (the one over step is a residue-mop cut, 0.4638), (3) FAILS — one unfed turn on the pocket, (27.53, 25.00) → (27.50, 25.04), 0.048 mm, swept 0 (a new degenerate kind), (4) holds (C 0, wrap cascades 5) → DROP on rule 3 only.** Slanted turns: few would fit s/cos θ (star 0–1, pocket 3–6 per column); the long refused joins (6–69 mm) are lobe changes and jumps across the island, not turns. **C18: the 3D arm reads 0 over the screen and 0 over the exact bound in both modes; AgentSearch control 225.** Starter now one plunge then one continuous spiral. Caveat before un-ignoring: C18's departure exemption (every move starting within R of the last plunge, to the next entry) is broader than the 2D walker's (first path only). Master-side red: crease_own_region_pr6b fails on unset feeds since S11, so that byte pin checks no iso_contour today (G-CREASEPIN). Not run: heavy scallop_isofield_gouge_m4 (237 s), scallop_candidates_m4 (143 s) — operator go-ahead | `P15_D_verify.md` §D12, `phase2/d12_*` |
| Operator 2026-10-24 on C21 | "Fine to have the option to select if there really are use cases for each. wanaka200 has no obstacles to go round on the first levels, so maybe that is why." → (a), the clipped march and the clipped harmonic stay as selectable variants if each wins on a fixture | — |
| Page v7 | `g1_2d_renders_v7.html`: v4 seam vs (a) + one-way vs (a) + zigzag, real planner output (D13 harness). Lead read: one-way 34 / 41, zigzag 38 / 44 plunges, every turn fed, only mop over steps → both pass the rule pending D13's formal verdict; zigzag keep-down 251 / 211 mm vs one-way 967 / 975 mm. Star: all five arms clean (lower-left arm fixed). Pocket one-way: the far top-right corner is still fragmented, and arcs that start at the island's lower-right side each start with an entry | — |
| C21 entry cost | DONE (24th); lead re-ran `assert_c21.py` (179 quotes OK). Python port of the cycle-time model (kinematics.rs trapezoid, per-axis accel, junction deviation; adaptive3d entry path) reproduces the Phase 0 runtimes within 0.06 %. **Entry penalty at a 5 mm hop vs keep-down:** rivmap100 (helix) 4.41 s (0.74 into a cut column, 17.5 fresh at the deepest level); Corne (plunge) 0.68 s (1.57 s with the linker's 200 mm/s² default); wanaka200 (plunge) 1.40 s. The cost is the depth fed at the plunge feed, not the rapids (helix: 11.3 mm path per 1 mm descent). **Break-even:** on rivmap100 the clipped march and clipped harmonic beat (a) on both shapes; on Corne and wanaka200 (a) wins (2D shapes ≠ fixtures; order of magnitude only). **Phase 0 rivmap100: 237 entries take 2364 of 3263 s (72 %)**; the 36–40 % in PLAN.md is from step_ladder RESULTS.md (538 of 1379 s). Findings: a 3D keep-down link ends at the rapid floor and the entry style still cuts below it (path.rs:1219-1221), so a keep-down saves the lift and travel but not the helix; fixture files lack per-axis Z rates (the operator's preset has $112 = 1000 mm/min) | `P2_C21_entry_cost.md`, `phase2/c21/` |
| C16 local-level spacing | CODE DONE (24th), not built; lead reviewed by path (no bare float). `Spacing::LocalLevel`: one T per level, ΔT the largest with ΔT·s ≤ min(s, s'(ρ)) over the trial level set, ρ the signed 3-point radius over ±1 pass step, bisection to the op tolerance. **Lower-left star arm: CAUSE FOUND** — `chain_segments` (scallop_isofield) grows a chain at its tail only and drops 2-point chains, so a row-major scan cuts arcs that fall to the right into pieces and loses the segment between (18 of 20 star gaps in that arm, Python emulation matches D11 to 0.01 mm); research fix `Chaining::Whole` (opt-in), pure split `iso_segments_where` in the shipped file. **Unfed zigzag turn: same cause** — the turn bridges a gap on the arc's own line, its end 0.17 µm off the chord, under the emitter's 1 µm RDP limit, so the point is dropped. Slanted turns measured into new `.c16.txt` readouts (bound unchanged). Columns geodesic_local, geodesic_local_oneway, attribution geodesic_curv_oneway_whole. Predicted: local_oneway star 27–33, pocket 42–52 | `P2_C2_geodesic_fixes.md` §C16 |
| C18 3D oracle + starter | CODE DONE (24th), not built; lead reviewed by path, re-ran `assert_c18.py` (holds). Oracle on the 2D rules: vertical moves not judged; the wall pass found by its `WaterlineCleanup` annotation, counted not asserted; plunge exemption per move (2D sentry's plunge_origin); a screened sample (sim > 0.546) fails only if the move's EXACT peak > 0.3626 + (cell + tol + pitch)/D = 0.471. Starter cause: helix 0 is correct for a Plunge entry; the lifter's air test reads stock at the cutter CENTRE, so ~11 mm of starter spiral inside the plunge hole reads as air, is demoted to a rapid, and the lap then meets 0.75 D (derived). Fix research-gated: the arm reads stock under the whole disc (shipped keeps the centre read). Shipped proposals filed in DEFECTS.md: G-AIRCENTRE, G-3DSTARTERSLOT, G-WATERLOAD, G-PERIMTAG. Predicted: starter class clears; ~29 wraps (0.56–0.69) stay over → test stays ignored. Build waits for C16 | `P3_C18_3d_oracle.md`, `phase2/c18/` |
| D11 verifier | DONE (24th) at ea468814 + C12 + C14: green fmt, check both, byte parity both (5/5), 13 adaptive3d sentries, 5 more sentries, --lib adaptive 143, research_geodesic 17, all 3 clippy, harness; controls EXACT (26 .txt byte-equal to D10). Reds same as master: g_adaptpassload, g_adaptrapidlift; the 2 ignored research tests still red at ea468814 (the mop step now 0.4524, reason said 0.3669 → reason being fixed). **C12 RED:** 128 judged samples over 0.546 D on geodesic_curv (peak 1.000; 81 ClearingCut ≥ 8 mm from the wall at Z 7 and 4.5; 13 Linking at 1.000) — but the AgentSearch control fails the same bound (260 over, peak 0.928); oracle vs arm defect not settled → test to be ignored with that reason. **C14:** one-way star 39 / pocket 54 entries (turns 40 / 41, 4 / 10 over the bound; non-turn max 0.3586 / 0.3624, 0 over; keep-down 589 / 865 mm; cut 1193 / 1322; wrap cascades 11 / 11); zigzag 50 / 66 (one unfed turn per shape). Turn medians 0.70–0.73 D swept, feed 0.44–0.47. **Rule: both DROP** (star 39 and 50 > 36; zigzag also an unfed turn). Lead look: one-way clean on 4 of 5 star arms; the lower-left arm (fragmented since G1) carries most entries; zigzag plunges sit where arcs meet a wall at a slant (turn s/cos θ > bound). Page `g1_2d_renders_v6.html`. Fixture estimates (not measured): CLI build 3–8 min, Corne < 3 min, wanaka 1–3 min, rivmap100 unknown | `P15_D_verify.md` §D11, `phase2/d11_harness.txt` |
| C14 traditional arc linking | CODE DONE (24th), not built; lead reviewed by path: `GeodesicOptions.arcs` (Nearest default / OneWay / Zigzag); one-way = climb (CW spindle, stock on the right), keep-down return link per vertex then the turn arc-start → next arc-start; zigzag turns end → near end; turn bound s + (1+√2)·cell = 3.207 mm (= the producer's reach); turn feed F' = F·c / max(swept/D, area/D) (holds the steady removal rate, the shear term of tool_load/power.rs; also holds peak lateral force); shipped path.rs: `LinkLoad::turns` empty on every shipped path, so `admits_turn` is false (byte parity proves it); residue/wall/mop get no exemption; split base not built (own plan loop). Predicted: star 35–55 (rule 1 at risk: closed-to-closed seams stay). Brief was (24th). **Operator ruling 2026-10-24:** link arcs as a traditional adaptive does — one-way with a keep-down return link through the cut, and zigzag; the short step-over TURN at each arc end may exceed the steady ceiling as a counted research exemption (length ≤ derived bound, feed reduced by its measured engagement, every turn reported); shipped rule unchanged. Columns `geodesic_curv_oneway`, `geodesic_curv_zigzag`; rule: star entries ≤ 36, no non-turn step over the ceiling, every turn within its bound and feed. C14 owns research_geodesic.rs + the harness; C12 told to stay out | `P2_C2_geodesic_fixes.md` §C14 |
| C12 geodesic 3D arm | CODE DONE (24th), not built; lead reviewed by path: `#[cfg(feature = "research")]` variant `ClearingStrategy3d::Geodesic(GeodesicOptions)` on the PLANNER enum only (3 product sites; a config-enum variant would break viz under the research clippy line); selected by env `RS_CAM_RESEARCH_ADAPTIVE3D_ARM` = geodesic_curv | geodesic_curv_split, which replaces the agent_search arm; fixture runs use NEW files `*_c12_agent_search.toml` (originals untouched; same file without the env = AgentSearch control); same slice dispatch + emitter as ContourSpiral; 2D links emitted as 3D cuts after cleanup; test `adaptive3d_geodesic_arm_c12` (pocket + island, 2 Z, both modes, dexel oracle, bound ceiling + (cells + tol)/D as common::adaptive_islands; entries and the perimeter-sweep band not judged). Risks: RDP simplification before the stamp vs the 2D grid stamping the walked line → slivers; perimeter sweep untagged; F-038 forecast dry-runs the agent, not the field. Build waits for C14 (shared worktree). C12's only research_geodesic.rs lines: one block `// ── The 3D research arm (C12) ──` (`slice_segments`, `links_as_cuts`). **Known gap:** `slice_segments` drops `Produced::turns`, so a C14 turn preset in 3D would be refused by the 3D replay; the arm stays on C5_CURV / C11_SPLIT until a follow-up carries the turns | `P3_C12_geodesic_3d_arm.md` |
| C13 lobe margin | DONE (24th), Python only; lead re-ran `assert_c13.py` (all claims hold). Exact geometry (circles about the seed, real wall corners, width error ~1e-5 mm) + a one-to-one port of MaterialGrid. 35 star lobe bands (not 40). C10's law passes 12/35; with two fixes (tilt = min(C10, exact closed form); full bite at least l_c before the far wall, l_c = where a plain pass meets the far corner, 0.66–1.79 mm) 35/35 pass, Δ 1.958 → 1.445. **But the lattice cannot see it:** at cell steps it reads 0.02–0.71 mm UNDER the exact width and accepts everything (even the 4 bands exact geometry rejects); at 1.5 mm steps it refuses 25 of 35 good bands. No rule from R, s, cD, cell alone (l_c depends on the walls). Cost 13.7 mm cut per plunge removed. Prediction: star 73 → ~38 if all 35 become lead-ins. **Side finding (G-CELLREAD, shipped):** every plain lobe arc reads 2.25–2.70 mm at its wall arrival in exact geometry (0.08–0.52 over cD); the lattice reads 1.45–2.46. **Lead: lobe margin PARKED** behind G-CELLREAD; C14's turns are the simpler path to the same plunges | `P2_C13_lobe_margin.md`, `phase2/c13/` |
| D10 verifier | DONE (24th) at f576e51e + C11 (committed ea468814): all green, no fix, controls exact (every control .txt byte-equal to D9 except the reseed `joins` line, now real). Split, pocket: zone 927 mm² (3709 nodes), 2 fields, entries 84 = S1 J64 M3 P16, wrap cascades 640 → 60, max width 0.3609 with 0 over (mop over-step gone), cut 1344 → 1174 mm; star = geodesic_curv. **Rule: (1) holds, (2) FAILS (84 > 63), (3) holds → DROP.** Lead reading: C9 and C11 both solve problem 1 and fail the entry bar for one reason — the far side becomes open arcs, each with a wrap-end plunge (problem 2). Taken to the operator with page v5 (three pocket columns) | `P15_D_verify.md` §D10, `phase2/d10_harness.txt` |
| C11 split at the meeting line | CODE DONE (24th), not built, ~700 lines: zone = nodes whose source-tracked branch bends round a hole ring (the island's shadow; equals the law-error set at zero tolerance); first field with the zone removed (wraps end on the shadow rays at a right angle); ONE zone field from T = 0 on the cut (no plunge), law from ρ0 = R, one-side rule, the two zone fronts in separate branches; star zone empty by construction; reseed `joins` readout fixed. Predicted (not run): pocket entries 45–70, rule 2 at risk (the split cuts the long lower arcs). Prototype `phase2/c11/zone.py`. Lead review: no bare float outside test geometry. Brief was (24th): operator idea 1b after C9's drop; far-side zone derived from the field, cleared as its own region (seed: own entry or the shared cut edge, once); column `geodesic_curv_split`; same pre-registered rule (cascades ≤ 320, entries ≤ 63 / 73, width ≤ ceiling); also fixes the reseed `joins` readout | `P2_C2_geodesic_fixes.md` §C11 |
| D9 verifier | DONE (24th) at 609ce75b + C9: all green, no fix, controls exact. Star: 0 re-seeds, plan identical to `geodesic_curv` (73). Pocket: 47 re-seeds / 48 fields (39 place one wrap), 74 entries = S1 J63 P10 (M, C → 0), wrap cascades 640 → 18, max width 0.3563 with 0 over (mop over-step gone), cut 1344 → 1599 mm. **Rule: (1) holds, (2) FAILS (74 > 63), (3) holds → DROP.** Cause: the truncated wrap end re-triggers, one re-seed per ~1 mm along y 48.5. Readout defect: the reseed column's `joins` line reads 0 (JoinReport::default). Page `g1_2d_renders_v5.html` (lead looked: far side cut by clearing, not residue, but as vertical strokes and a fan — not one front) | `P15_D_verify.md` §D9, `phase2/d9_harness.txt` |
| C9 far-side re-seed | CODE DONE (24th), not built, ~865 lines: trigger = the replay model refuses a wrap step after that field cut stock; the wrap ends there, the rest of the field drops; new field T = 0 on tool-centre nodes whose disc meets no stock, each node its own pivot, branch table kept (one-side rule still works), spaced by the curvature law from ρ0 = R; stop: a field places every wrap, or a re-seeded field has no level; finite because material falls strictly. Risk: the truncated wrap's own end re-triggers (rule 2 at risk). Lead review: no bare float outside test geometry. Brief was (new lead session rs-cam-e3): operator idea 1a, re-seed the field from the cleared area after the one-side rule; column `geodesic_curv_reseed`; pre-registered rule: keep if pocket cascade wrap refusals ≤ half of `geodesic_curv`, entries ≤ 73 / 63, width ≤ ceiling (mop excepted); else idea 1b | `P2_C2_geodesic_fixes.md` §C9 |
| C10 lead-in guard | DONE (24th), read-only, lead spot-checked 2 citations (ok): C8's zero is a LIMIT plus a bug. Inside ρ* (22.78 mm) the margin is ~0, so no lead-in (every closed wrap, all pocket lobe bands, 15/39 star bands). Beyond ρ* a bug (mid-arc trace returns on a miss, the shift of P never runs) hides a second limit: star arm arcs ≤ 10.6 mm against spans ≥ 13.3 mm. The radius readout records success only. Patch in the note, NOT applied (predicted gain: pocket arc ends 1 → 10 of 158, star 0). Growing-bite variant fits 0/39 star bands at spacing s. **Lead ruling: drop the circular lead-in; the per-arc plunge in lobes stays the candidate's cost at this spacing.** The only open option (growing bite with a reserved band margin, ~40 star bands predicted) changes the spacing → operator question 3 below | `P2_C10_leadin_guard.md` |
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

- **00:20: adaptive-instruments LANDED on master 629748f1** (peer's c49414c5
  merged, no conflict, ff'd). Post-merge smoke on main: check core + viz,
  viz --lib 586 green, roughcov 9/9, roughgouge 6/6.

- **Instruments (Phase 1.5) are built and measured.** Gouge: clean on all
  eight Phase 0 roughs, exports succeed, no false fire. Coverage: reads
  every eye defect (37 + 2 hex-hole cores, the 13 mm spur, the outer band),
  terraces gone, headline names a free island; the ContourParallel zero
  control fails honestly on real wall strips (G-WALLSTRIP-PLAN) and small
  slivers. All on branch `adaptive-instruments` (7c75324d), gates green
  (P15_D_verify.md D2–D6). **ff of master blocked** by the peer's
  uncommitted registry change (they land it, then I merge + ff).
- **Candidate (a) geodesic (state at 01:10, 24th):** growth pattern accepted at
  G1; the curvature-spaced wraps (`geodesic_curv`) are the state to look at:
  `g1_2d_renders_v4.html` (served at http://127.0.0.1:8766/g1_2d_renders_v4.html
  while the session runs). Star 73 entries (from 92), pocket 63 (from 88),
  no mid-wrap splits, five like arms. Four join schemes were tried and
  measured tonight (ramp C3, morph C4, transition C6, mid-arc lead-in C8):
  none beat the plain seam under the swept-width rule; C7 showed the
  exact-area measure gives the same answer on circular fronts and at
  wall ends. The per-arc plunge in a lobe stands as the candidate's cost
  until a derived join is found; the C8 lead-in code never applied (guard
  to find). Verified record: P15_D_verify.md D2–D8, all on master.
- **The measure ruling is WITHDRAWN (C7, 00:50):** the lead's premise was
  wrong. C7 built the exact area-per-mm measure and showed that at a
  wall-end turn the area per mm is over the ceiling too (a head-on step
  clears a disc cap: 3.1 then 5.2 mm²/mm against 2.18), and that on every
  circular front the two measures are the SAME expression. So the per-arc
  plunge is real engagement, not a measurement artefact. What FreeCAD
  does instead (R1 §4): the next arc is reached by a keep-down LINK through
  the already-cleared band (zero load) and entered by a tangential LEAD-IN
  whose bite grows from 0 to s. That is C8, after D7 verifies C7.
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

## Lead ruling 2026-10-24: the turn bound stays s + (1+√2)·cell

D12 measured how many refused joins a slanted-wall bound s/cos θ + (1+√2)·cell would admit: star 0–1, pocket 3–6 per column. The long refused joins (6.3–68.6 mm) are lobe changes and jumps across the island; they need routed keep-down links (FreeCAD's, R1 §4), not a longer turn. Operator: "if it makes sense, run with it." Heavy scallop tests approved by the operator (D12 not-run list).

## Operator 2026-10-24: default cut direction

"Are we defaulting this to milling both directions too? That is generally how I do it." → `geodesic_local_zigzag` (LocalLevel + Whole chaining + zigzag) made REQUIRED in C20, under the same rule as `geodesic_local_oneway`; the operator picks the default when both are measured.

## Operator 2026-10-24 (evening, then away for the night)

- "(a) spacing + zigzag (both directions) + turns looks the closest" → `geodesic_local_zigzag` is the lead candidate.
- "Likely want an optional waterline clearing path on it to clear up the edges with a quick skim." → queued as an optional research pass (engagement held to the ceiling or the stock left; NOTE the shipped waterline cleanup reads 0.72–0.99 D, G-WATERLOAD, so it is not a skim today).
- "See what you can do without me." Lead's night plan: C22 + C23 → one verifier → commit; 3D fixture runs on Corne and wanaka200 only (each < 3 min; AgentSearch, zigzag, one-way); NO rivmap100 or other run over ~3 min without the operator; fixture pages per PLAN if the GUI can run the branch build; the edge skim brief once C23 frees research_geodesic.rs.

## Queued optimisations (operator, 2026-10-24; after C19 and the Rust columns)

- Where two fronts meet (the V behind an island, and where clipped march / harmonic pieces meet), merge the two segments into ONE clean arc through the middle instead of two arcs meeting in a corner. = C15 idea (c), the round union; its load bound is derived (r ≤ 2.68 mm on the pocket); the operator wants a smaller radius, from the machine's acceleration at the cut feed (F²/a, machine profile), capped by that bound.
- C19 also covers harmonic (e) with the same clip → split → per-arm order (operator: "the same thing could be done with harmonic too").

## Open questions for the operator

1. `TROCHOID_CAP_MULT_3D = 1.6` (`adaptive3d/clearing.rs`) and
   `default_trochoid_cap_mult()` (`compute/operation_configs.rs`) cite the
   June cap sweep, which lives only behind tag
   `planning-pre-purge-2026-09-17`. The peer's default-removal work may
   delete this default; ContourSpiral then refuses until the dial is set.
   The baseline uses 1.6 and says so.
3. (24th, C10; operator: study it in parallel with the 3D wiring → C13) Reserve a band margin in lobes (lobe spacing Δ' 1.57–1.94 mm
   instead of s = 2 at levels 8–15) so a growing-bite lead-in fits and
   most star wrap-end plunges go? Cost: more passes in lobes. Needs an
   exact-geometry width check first; inner margins (0.007 mm) are below
   the lattice resolution.
4. (24th) Far side of the island: C11 split or keep `geodesic_curv`? Operator: not enough information yet (did not know what C9 / C11 are) → plain explanation added to page v5; awaiting the look.
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

## (WITHDRAWN 00:50 by C7's measurement — kept for the record) Ruling on the engagement MEASURE at wall ends

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
