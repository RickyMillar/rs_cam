# Multi-tool island finishing — investigation session prompt

> **Paste this whole file as the prompt for the next session.** That session's
> job is INVESTIGATION ONLY: survey the code, UX, and math that could enable
> multi-tool island finishing, run one cheap empirical arm, and WRITE THE
> ORCHESTRATION PLAN for the session after it. No feature code in the
> investigation session.

## The operator's idea (2026-08-23, verbatim intent)

Wanaka200 (240×250×25 white oak terrain) finishing today uses one R1.5
tapered ball everywhere + an R0.5 pencil. The operator wants: fine tools
(R1.0/R0.5, or a 1–2 mm tapered ball) scalloping ONLY the mountains/tight
terrain for real detail, larger balls (R2.0+) on the flats/open areas where
they're strictly faster at equal cusp height. Unified finish already groups
scallops/contours into islands — the ask is a technique that identifies
islands that "fit an x-size bit": rest stock + unified's math + filtering so
we don't end up with 1000s of islands, keeping uniform scallop per island.

## Working context

- Repo: /home/ricky/personal_repos/rs_cam, branch master. Read
  `planning/airrun_2026-08-19/P2_RUN_LOG.md` first (today's ledger: chaining,
  pencil entry ramps + LinkCeiling, entry_load gate, G-UNIFIEDCRASH fix that
  UNLOCKED fine scallop).
- Candidates on disk: `wanaka200_p2.toml` (4.27 h, hills at 100 µm cusps) and
  `wanaka200_p2_c2.toml` (4.75 h, whole surface at 30 µm — the current
  keeper). Multi-tool must beat C2 on time at ≥ C2's hill detail to earn in.
- Tool drawer: Ø6 end mill; tapered balls R0.5/R1.0/R1.5/R2.0 (6 mm shank,
  max 6.35); 20° V-bit. Machine: Shapeoko Pro XXL.
- Standing constraints (unchanged): never touch `.mcp.json`,
  `planning/airrun_2026-06-01/wanaka.toml`, workspace `Cargo.toml`,
  `benches/hot_paths.rs`, `tests/perf_golden_*`,
  `planning/perf_review_2026-08-19/`. ONE cargo job machine-wide
  (`pgrep -x cargo` + `/proc/<pid>/cwd`, never `pgrep -f`). No history
  rewriting; explicit staging. Never pipe `cargo test` through `head`. Sims
  @0.15 mm (0.1 OOMs this board); memory watcher armed for any GUI work;
  pre-built release binary before MCP connect. Fable orchestrates, Opus
  agents implement. Op-inventory check after every generation; reload after
  any empty generation (the refusal now catches these, but the discipline
  stays).

## Investigation tracks (fan out read-only agents; cite file:line evidence)

### T1 — Reachability math: "does an R-size ball fit here?"

The core primitive is a per-tool residual map: drop-cutter map with tool T =
the T-achievable surface; residual = achievable − true surface; islands =
regions where residual > tolerance. Establish:
- Where drop-cutter / point-drop machinery lives and what it costs at
  board scale (the generator already runs it per sample — is a full-grid
  map cheap enough at planning time? measure or estimate honestly).
- Whether `reach` (see `crates/rs_cam_core/src/reach.rs` — tip_float /
  reach residual, suggested_offset_stepover) already computes something
  equivalent per-tool.
- How rest analysis (`rest_analysis`, `min_valley_depth`, cell_mm) and the
  claims machinery (`claims_reference = machined_stock`, `min_rest_depth`,
  `pencil_claims`, `territory_stock`) express "what tool X left behind" —
  this is the two-tier version's engine and probably the n-tier one's too.
- The July selective-finishing work (memory: P2 selective finishing,
  commit 6a7e164) — what "selective" already means there and whether its
  region selection is reusable.

### T2 — Island machinery: decomposition, routing, filtering

- unified_finish's band/region decomposition and island routing (COLUMNS
  instrument, route_greedy, region nodes) — can a region set be driven by an
  EXTERNAL mask (the tier map) instead of slope classification? Where is the
  seam?
- Boundary machinery: `ToolContainment`, boundary regions, derived-rest
  boundaries (`DerivedRestRegions`) — can an island set become a machining
  boundary for a second unified op today?
- The 1000s-of-islands problem: what exists for morphological cleanup
  (dilate/merge/min-area)? rivmap/zones artifacts (planning/review_2026-08-04
  arp1_zones.pgm) suggest zone rasters existed in analysis tooling — find
  the code lineage.
- Tier-boundary blending: overlap band between tools so tier seams don't
  print as steps (stock_to_leave interplay, cos-A vertical-offset caveat in
  CLAUDE.md).

### T3 — UX

- How would an operator express a tool ladder ("R2.0 then R1.0 then R0.5,
  tolerance 30 µm, min island 50 mm², merge radius 5 mm")? One op with a
  ladder param vs auto-generated op chain (one per tier — better for
  per-tier feeds/sim/gates)? Look at how setup/op templates and the strategy
  advisor present choices today.
- Island PREVIEW before generation: what can render a tier mask (viewport
  overlay, composite PNG, screenshot surface)? An operator must be able to
  see "R2 territory vs R0.5 territory" and veto.
- Where do per-tier diagnostics land (per-op is free if tiers are separate
  ops — argue for/against on the diagnostics surface).

### T4 — Empirical arm (cheap, config-only — do this in the GUI while
agents read)

Two-tier TODAY with existing ops on wanaka200: R2.0 unified over everything
(wide stepover on flats at equal cusp) + rest-driven fine unified
(claims_reference = machined_stock, min_rest_depth ≈ 0.03, R1.0 or R1.5)
+ existing pencil. Measure against C2 (17,088 s / 4.75 h, 0/0, uniform
30 µm). Rest-measurement rules apply: claims_reference=machined_stock in the
cascade, sim cell well below fine-tool TIP radius (0.15 vs R1.0 tip is
marginal — state measurability honestly). Phase-1's C3a (R2.0 negative) is
STALE evidence: it lost on pencil entry economics that were fixed 2026-08-23
(entries −837 s). This arm is the go/no-go signal for the whole feature: if
config-only two-tier already beats C2, the planner mostly automates a win;
if it loses, find out which cost (rest detection, linking, tier overlap) ate
the margin before proposing anything.

## Deliverable — the orchestration plan for the SESSION AFTER

Write `planning/multitool_2026-08-23/ORCHESTRATION_PLAN.md`: phased plan
(foundation seams → tier-map planner → island filtering → op generation →
UX → validation on wanaka200), each phase with file-level touch points from
the investigation's evidence, agent task splits (parallel editors no-cargo +
single verify lane, worktree only if files overlap), red-first sentry
designs, measurable acceptance gates (time vs C2, detail ≥ C2 on hills,
0/0 collisions, gates green with real populations), and the operator
decision points (island preview veto, tier ladder choice). Include a
post-compact continuation prompt at the bottom, as
`planning/airrun_2026-08-19/EFFICIENCY_PHASE2_2026-08-23.md` did — that
pattern worked.

Also append investigation findings + the T4 measurement to
`planning/airrun_2026-08-19/P2_RUN_LOG.md` and commit both docs.
