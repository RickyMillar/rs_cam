# Multi-tool island finishing — orchestration plan

> Written 2026-08-23 by the investigation session. Inputs: `T1_FINDINGS.md`
> (reachability math), `T2_FINDINGS.md` (island machinery), `T3_FINDINGS.md`
> (UX), and the T4 empirical arm measured live in this session (§0). This is
> the plan for the IMPLEMENTATION session. Investigation prompt:
> `INVESTIGATION_PROMPT.md`.

## 0. What the empirical arm proved (T4, measured 2026-08-23)

Config-only two-tier on wanaka200 (`wanaka200_mt1.toml`, branched from the
C2 keeper): tier-A = the C2 finish retooled R1.5→R2.0 at equal 30 µm cusp
(raster_stepover 0.6→0.69), tier-B = new rest-driven R1.0 unified
(`claims_reference = machined_stock`, `min_rest_depth_mm 0.03`, proven
R1.0 feeds 1062/180/19000), pencil unchanged. Full fixpoint ladder + sim
@0.15, gates 9/9 Within with real populations, 0/0 collisions, verdict OK.

**Result: 36,892 s (10.25 h) vs C2's 17,088 s (4.75 h) — loses by 5.5 h.**
Attribution (per-op runtimes from the cut trace):

| Op | mt1 runtime | Comparison |
|----|------------|------------|
| tier-A R2.0 @30 µm | **5,019 s** | C2's whole finish+pencil ≈ 9,334 s → **tier-A alone saves ~4.1k s. The big-tool-on-flats half WORKS.** |
| tier-B R1.0 rest | **23,916 s** | of which **16,140 s is rapids** — 19,137 retract round-trips (19,132 *inside* routing nodes), plus 7,329 s cutting 89 km ≈ near-full re-coverage |
| pencil R0.5 | 203 s | (small: tier-B's R1.0 pre-cut most valleys the R1.5-referenced detector finds) |

Two costs ate the margin, and they are the SAME two the July selective-
finishing ledger recorded (cutting −80% but rapids 12×,
`finishing_stack_review_2026-07.md:744-745`):

1. **Over-selection.** `min_rest_depth 0.03` sits AT tier-A's own cusp
   height, and — the deeper cause, T1 §3 — the drop-cutter residual is a
   tool-CENTRE surface difference, biased by `R·(sec θ − 1)`: R2-vs-fine
   reads 0.62 mm at 45° on a slope both tools machine perfectly. The whole
   mid-steep band qualified: tier-B's scallop band was ONE region node of
   146,872 moves.
2. **Fragmentation.** The rest filter punches the pass into confetti
   *inside* regions; every fragment ends in a full retract (0.84 s each ×
   19k = the 16,140 s). Region count was NOT the problem (14 nodes, not
   1000s) — intra-region linking was.

A/B sharpener (tier-B `min_rest_depth` 0.03→0.05, above tier-A's cusp,
`wanaka200_mt1b.toml`): **byte-identical toolpath** — the dial is INERT
for unified_finish emission when `territory_clip = false`; the claims
mask is computed and never applied. So arm 1's tier-B was a FULL-BOARD
R1.0 finish, not a rest pass: "over-selection" was actually "no
selection", and the 19k retracts are unified's own emission at R1.0/0.49
on this terrain. Consequences for this plan: (a) the untested config-only
confinement levers are `territory_clip = true` (B2's welding trap
applies) and rest_analysis → `DerivedRestRegions` — run both as cheap
probes at the START of Phase T before building anything; (b) Phase F
gains a small F4: make `min_rest_depth_mm`/`claims_reference` either
effective or refused on unified_finish when territory_clip is off — a
dial that silently does nothing is the G-family pattern.

**Go/no-go verdict: GO, with a defined margin.** Tier-A + pencil = 5,222 s
against C2's 9,334 s leaves **≈ 4,100 s of budget** for a fine tier that
must also deliver ≥ C2 hill detail. A tier-B confined to true fine-territory
(mountains) with stay-down intra-island routing fits that budget on paper:
detection must stop charging the fine tool for slopes (fix #1) and routing
must stop paying full retracts per fragment (fix #2). Neither is
config-reachable today — that is the feature.

Measurability caveat, stated honestly: at 0.15 mm cells, 58% of tier-B's
material-removing samples read zero engagement (R1.0 tip < cell
resolution); collisions, removal, and DOC are unaffected. 0.1 mm OOMs this
board. Time/collision verdicts stand; engagement-grade claims about the
fine tier do not.

## 1. Design skeleton (what the findings force)

- **The primitive exists.** `rest_field::detect_rest_valleys`
  (`rest_field.rs:616-683`) IS the per-tool residual map
  (`rest = drop_z(reference) − drop_z(fine)`), polygon extraction included;
  `attach_generic_rest_analysis` (`execute.rs:3023-3091`) runs it for any op
  family; `BoundarySource::DerivedRestRegions` (`config.rs:1454-1456`)
  pre-clips a consumer op to the result — the ONLY boundary source that
  confines decomposition, not just post-clips (T2 §3,
  `session/compute.rs:1261-1276`).
- **The island machinery exists.** `finish_planner::decompose`
  (`finish_planner.rs:352-548`): coverage erosion → hysteresis →
  morphological close → label grid → min-area absorption → marching
  squares. `unified_finish` already takes `machining_boundary:
  Option<&RegionSet>` ANDed per-cell BEFORE decompose
  (`unified_finish.rs:1305,1345-1363`). Nothing morphological needs
  writing (T2 §6).
- **The operator's dials exist un-exposed.** `FinishPlannerParams`
  `close_radius_mm` (merge radius), `min_region_area_mm2` (min island),
  `hysteresis_deg` — all tool-derived via `for_tool`, none reachable;
  override site is four lines (`execute.rs:2138-2142`) (T3 §10).
- **Three defects/blockers to clear first:**
  - **B1 slope bias (T1 G2):** the mask stage has no `R·(sec θ − 1)`
    compensation; options = analytic compensation vs
    `RestReference::Stock` cascade. T4 shows machined-stock claims
    over-select too when threshold ≤ coarse cusp; the planner must use
    `threshold > coarse-tier cusp + slope term` or compensate analytically.
  - **B2 envelope-radius dilation (T2 §5, T1 G6):** derived-region dilation
    and erosion use the ENVELOPE radius (`rest_field.rs:830-836,137-145`)
    — 3.5 mm on a Ø1-tip taper, measured to weld dendritic masks into full
    coverage (`unified_finish.rs:1599-1603`). Must become tip/cusp radius
    for tapered balls.
  - **B3 intra-island routing:** fragments must link with stay-down moves
    under the stock-aware `LinkCeiling` (machinery landed with chaining +
    pencil links, commits 3ef08d32/4346cb7b), not full retracts. 19k
    retracts = 16.1k s is the measured price of not doing this.
- **Structure: op-chain, not one-op** (T3 §1, decisive): gates, G-code
  (M6 per phase boundary), feeds provenance, and stock chaining all key on
  one-op-one-tool; `pencil_claims`' one-op precedent measured +22.5% time
  for zero gain. Per-tier diagnostics come free as per-op rows.
- **Costs (T1 §5, honest):** per-tool map ≈ 8 s @0.6 mm, ≈ 31 s @0.3 mm,
  ≈ 125 s @0.15 mm on this mesh (200×200, 661k tri), 49 B/cell → plan
  tiers at 0.3–0.6 mm, NEVER 0.15. No per-tool cache exists; a new one
  must copy `geom_cache`'s key discipline (`Weak` + `Arc::ptr_eq`) and its
  memory discipline (5 tiers @0.15 = 463 MB on a board that OOMs at 0.1).

## 2. Phases

Each phase = red-first sentries, then implementation by parallel no-cargo
Opus editors, then the single verify lane (fmt/clippy/tests) — this
session's standing pattern. Worktree isolation ONLY where the touch lists
overlap (they mostly don't). File evidence: T1/T2/T3_FINDINGS.md.

### Phase F — Foundation seams (small, unblock everything)

1. **F1 — tip-radius dilation/erosion for tapered tools.**
   `rest_field.rs:137-145` (dilation = fine cutter envelope + margin),
   `:830-836`, and `erosion_radius()` (T1 G6: envelope blanks a 3 mm rim).
   Use the tool's TIP radius for tapered_ball; keep envelope for flat/ball
   where tip == envelope.
   *Sentry (red first):* a Ø1-tip/Ø6-shank taper's derived regions from a
   dendritic mask stay disjoint (assert region count > 1 and total area ≪
   board area); today they weld (`unified_finish.rs:1599-1603` documents
   the measured welding).
2. **F2 — expose `FinishPlannerParams` overrides** on
   `UnifiedFinishConfig`: `min_region_area_mm2`, `close_radius_mm`,
   `hysteresis_deg` as `Option<f64>` (None = `for_tool` derivation).
   Override site `execute.rs:2138-2142`; config struct
   `operation_configs.rs:955-987`; serde round-trip gets TOML+MCP free
   (`session/compute.rs:400-542`).
   *Sentry:* schema exposes the three fields; None ≡ today byte-identical.
3. **F3 — region-cap honesty.** `MAX_REST_REGIONS = 64`
   (`region_mask.rs:182-204`) silently truncates per band. Surface a typed
   report-only finding (follow the `ToolpathStats` contract: None = not
   measured) when the cap trims.
   *Sentry:* a 100-island synthetic mask reports `regions_truncated` with
   the true pre-cap count.

Agent split: one editor (all three are core, small, non-overlapping
files); verify lane runs the sentries red → green.

### Phase T — Tier-map planner (core, the new math)

1. **T1 — n-tool residual walk (T1 G1).** One grid walk, one max-radius
   spatial-index query per cell serving all ladder tools; output a per-cell
   tier label = smallest tool index whose residual ≤ tolerance.
   New module `tier_map.rs` beside `rest_field.rs`; reuse
   `point_drop_cutter` (`dropcutter.rs:16-43`) and the
   `rest_field.rs:685-690` rayon pattern (add cancellation polling — the
   existing walk has none).
2. **T2 — slope-bias treatment (T1 G2, the B1 decision).** Two candidate
   arms, A/B'd on wanaka200 before either merges:
   (a) analytic compensation — subtract `R_diff·(sec θ − 1)` using the
   slope grid (`slope.rs`) before thresholding;
   (b) stock-referenced residual — `RestReference::Stock` against the
   coarse tier's simulated stock (exact, but needs the generate→simulate
   cascade and inherits sim-cell floors).
   Decision rule: (a) wins if its tier map on wanaka200 assigns < 25% of
   board area to fine tiers AND visual overlay matches the operator's
   mountain/flat intuition; else (b).
3. **T3 — per-tool surface cache (T1 G4).** Key = (mesh identity via
   `Weak`+`Arc::ptr_eq`, tool geometry hash, cell_mm, grid origin/extent);
   capacity-bounded like `geom_cache` (`geom_cache.rs:19-53,88-93`).
   Planning resolution default 0.3–0.6 mm.
*Sentries (red first):* synthetic hemisphere+plane fixture — R2 tier claims
the plane, fine tier claims the bowl below its radius, NO tier claims a
45° planar ramp (kills the slope bias by construction); cost sentry:
tier-map build on the fixture < 5 s; cache sentry: second build with same
key does zero drop-cutter calls.

Agent split: two editors (T1+T3 one, T2 the other — different files),
verify lane holds the A/B measurement.

### Phase I — Island filtering + tier assembly

1. Feed the tier label grid through `finish_planner::decompose` steps
   2/4/6 (hysteresis / close / min-area — T1 confirms band-agnostic,
   `finish_planner.rs:352-548`) with the F2 dials; per-tier output =
   `RegionSet`.
2. Tier overlap band: dilate each finer tier's islands by `overlap_mm`
   into the coarser tier's territory (dial exists per-op, default 2.0);
   hold `stock_to_leave` EQUAL across tiers (T2 §7: a seam across a slope
   change prints a `stock_to_leave·Δcos θ` step — with 0.0 everywhere the
   term vanishes).
3. The 1000s-of-islands guard: after min-area absorption, if region count
   still > a planner cap (default 24 per tier), auto-raise
   `close_radius_mm` and re-close (bounded loop), and report what merged.
*Sentries:* island count on wanaka200 tier map ≤ cap with default dials;
seam-overlap fixture: every fine-tier island polygon is contained in the
coarse tier's coverage dilated by overlap_mm.

### Phase O — Op generation + routing

1. **Ladder → op chain.** A planner action ("Plan multi-tool finishing")
   takes the ladder (ordered tool ids + tolerance) and EMITS k enabled
   unified_finish ops, coarse→fine, each `from_remaining_stock`, each
   carrying its tier's `RegionSet` — plus a `planner_origin` provenance
   field on `ToolpathConfig` (new, T3 C1: today emitted tiers would be
   indistinguishable from hand ops). Reconciler precedent:
   alignment-pin-drill (`controller/events/model.rs:645-790`) but
   multi-instance.
2. **Boundary injection.** Either generalize `pre_boundary_regions` beyond
   `DerivedRestRegions` (`session/compute.rs:1261-1313`) with a new
   `BoundarySource::PlannedTierRegions`, or store the RegionSet per-op the
   way `DerivedRestRegions` resolves lazily. MUST enter the pre-decompose
   seam (`unified_finish.rs:1345-1363`), not post-clip.
3. **B3 — intra-island stay-down routing.** Inside an island, link
   fragments with `max_conservative_top_z_in_disc` ceiling links (the
   project_curve/pencil chaining machinery) instead of full retracts;
   between islands, keep retracts. Budget target from T4: tier-B's
   rapid_s must drop from 16,140 s to O(1,000 s) at equal coverage.
4. **GUI fixpoint parity (T3 C2).** The one-call rest-chain fixpoint is
   `#[cfg(feature = "mcp")]` (`controller/events/compute.rs:1387-1493`);
   the GUI is single-pass and tells the operator to loop by hand. A k-op
   emitted chain makes this untenable — wire GUI Generate All to the
   fixpoint.
*Sentries:* emitted chain round-trips project IO with provenance;
routing sentry (red first): a two-fragment island links stay-down under a
ceiling and never rapids to safe-Z between them; a fresh-stock op is
untouched (byte-identical golden).

### Phase U — UX

1. Ladder entry: planner dialog listing drawer tools (checkboxes, coarse→
   fine), tolerance, min island mm², merge radius — pre-filled from
   `for_tool`. One new panel; Optimize-project's cached-plan + per-row
   checkbox + "Apply selected" pattern (`ui/optimize_project.rs:222-236`)
   is the veto shape to copy (T3 §8; the strategy advisor is the named
   anti-pattern — MCP-only, zero GUI surface, no apply path).
2. Island preview VETO (operator decision point #1): render the tier map
   pre-generation through the rest-heatmap slot
   (`rest_heatmap_mesh.rs:52` → `gpu_upload.rs:1222-1230` → render) with
   per-tier colors; gate like height-planes (`app/viewport.rs:463-464`).
   Agent-visible twin: give `planned_regions_to_svg`
   (`finish_planner.rs:978`, finished, zero callers) a caller + an MCP
   `preview_tier_map` tool (clone `mcp_screenshot_toolpath` minus its
   generated-result guard, `app/mcp.rs:4562,4583-4586`). NO giant HTML
   dumps (the existing interactive HTML path measured 948 MB, T3 §7).
3. Per-tier diagnostics: free as per-op rows — no work beyond naming ops
   "<n> Finish tier k (Rx.y)".
*Sentries:* preview renders without generation (plan-time); veto path
leaves the project unmodified.

### Phase V — Validation on wanaka200 (acceptance)

Run the emitted ladder (R2.0 → R1.0, pencil kept) on `wanaka200_p2_c2`'s
job. ALL gates measured at sim @0.15, fixpoint ladder, real populations:

| Gate | Bar |
|------|-----|
| Total time | **< 17,088 s** (beat C2) — stretch < 15,376 s (beat P2) |
| Hill detail | fine-tier territory covers the mid-steep band the operator flagged (C1: 53% of finish moves), at ≤ 30 µm cusp params, fine tool ≤ R1.0 |
| Collisions | 0 / 0 |
| Load gates | green `Within` with non-vacuous populations (check `sample_range` — the vacuous-gate trap, CLAUDE.md) |
| Fine-tier rapids | rapid_s / cutting_s < 0.5 on the fine tier (T4's was 2.2) |
| Operator eyeball | tier-map overlay veto BEFORE generation; final render after (the C4 rule: the eye is the accepting gate for surface quality) |

Also re-measure pencil against the new cascade (its R1.5-referenced
detector partially overlaps tier-B's territory — decide keep/re-reference
/drop with evidence; T4's pencil collapsed to 203 s).

## 3. Operator decision points

1. **Tier-map veto** (Phase U preview) — see territory before any
   generation; reject re-opens the ladder dialog.
2. **Ladder choice** — which drawer tools participate; default proposal
   R2.0 → R1.0 (+ pencil R0.5 as today).
3. **B1 arm choice** ratified by overlay eyeball (Phase T A/B): analytic
   slope compensation vs stock-cascade residual.
4. **Pencil's fate** after tiers land (keep / re-reference / drop).
5. **Merge call** at the end, as always.

## 4. Agent orchestration summary

- Fable orchestrates; Opus agents implement; editors are no-cargo; ONE
  verify lane owns cargo (fmt → clippy → `-p rs_cam_core` tests → viz/cli/
  mcp as touched). One cargo job machine-wide (`pgrep -x cargo` +
  `/proc/<pid>/cwd`).
- Phase F: 1 editor. Phase T: 2 editors (T1+T3 / T2). Phase I: 1 editor.
  Phase O: 2 editors (core routing+boundary / viz reconciler+fixpoint
  parity — disjoint crates). Phase U: 1 viz editor + 1 mcp editor.
  Worktrees only if a split turns out to overlap `unified_finish.rs`.
- Sentries are written and RED before their phase's implementation
  (standing rule: commit instruments the moment they lint clean).
- Live validation (Phase V) runs in THIS session's GUI lane, pre-built
  release binary, memory watcher armed, sims @0.15.

## 5. Standing constraints (unchanged)

Never touch `.mcp.json`, `planning/airrun_2026-06-01/wanaka.toml`,
workspace `Cargo.toml`, `benches/hot_paths.rs`, `tests/perf_golden_*`,
`planning/perf_review_2026-08-19/`. No history rewriting; explicit
staging. Never pipe `cargo test` through `head`. Op-inventory check after
every generation; reload after any empty generation. Pre-built release
binary before MCP connect.

## 6. Post-compact continuation prompt (paste after /compact)

Continue rs_cam in /home/ricky/personal_repos/rs_cam (branch master).
Read planning/multitool_2026-08-23/ORCHESTRATION_PLAN.md FIRST — it is
the whole agenda; evidence in T1/T2/T3_FINDINGS.md beside it and the T4
measurement in planning/airrun_2026-08-19/P2_RUN_LOG.md (mt1 arm:
two-tier loses 36,892 s vs C2 17,088 s; tier-A R2.0 alone SAVES ~4.1k s;
the losses are slope-biased over-selection + 19k intra-region retracts =
16.1k s rapids — the plan's B1/B3). Candidates on disk:
planning/airrun_2026-08-19/wanaka200_p2_c2.toml (C2 keeper, 17,088 s,
0/0) and planning/multitool_2026-08-23/wanaka200_mt1.toml (the losing
two-tier arm, kept as the A/B baseline). Standing constraints: never-touch
(.mcp.json, wanaka.toml, workspace Cargo.toml, benches/hot_paths.rs,
tests/perf_golden_*, planning/perf_review_2026-08-19/), one cargo job
machine-wide (pgrep -x cargo + /proc cwd, never pgrep -f), no history
rewriting, don't pipe cargo test through head, sims @0.15 (0.1 OOMs this
board), Fable orchestrates + Opus implementation agents (editors
no-cargo, ONE verify lane), op-inventory check after every generation,
reload after any empty generation, pre-built release binary before MCP
connect, memory watcher for GUI work. Start with Phase F (foundation
seams — red-first sentries, then F1 tip-radius dilation, F2 planner-dial
exposure, F3 region-cap finding), then Phase T with the B1 A/B. The
operator decision points are listed in §3 — surface the tier-map preview
veto before any full-board generation.
