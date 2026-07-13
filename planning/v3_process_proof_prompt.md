# v3 PROCESS PROOF — cascade rest-clearing on scaled wanaka (handoff)

Paste the short prompt at the bottom into a fresh session; this file is
the operational brief. Full spec: `planning/unified_v3_design.md` §0.a
(the reframed end goal) — read it first. Branch `experiment/adaptive-spiral`.

## The goal (user, 2026-07-13)

Prove the PROCESS, not tune wanaka: **ball all-over finish + ONE unified
rest-clearing pass (mixed strategies over rest islands) beats
all-over-tip on time at equal COLUMNS quality**, Region spans proving
raster/scallop/pencil each cut their own territory. Ball size is a free
parameter; the model may be scaled (×2) to fix tool-vs-texture ratio.

## Build list (each slice: gates green → commit)

1. **wanaka ×2 fixture**: scale the mesh in the harness (load wanaka,
   scale vertices ×2, stock/heights accordingly — or a standalone scaled
   chain: one rough + Op A + Op B is enough; the full 7-op chain is NOT
   required for the proof).
2. **Region-level territory filter** in the unified op: decompose FIRST,
   then drop whole conditioned islands whose measured rest share (vs
   `territory_stock`, the existing telemetry loop) is below a dial.
   NEVER mutate `covered` per cell — measured 2026-07-13: even 0.2 %
   true skips at dendritic necks fragment the decomposition 4→35
   regions (see `unified_finish.rs` Step 2.5 comments).
3. **Stock-referenced crease claims for finish-quality references**: when
   `territory_stock` is a FINISHED (ball) stock, run `detect_rest_valleys`
   with `RestReference::Stock` (the R2-validated config) instead of the
   analytic self-probe; keep additive emission (no corridor carving).
   The "claims are geometric" rule (`ClaimsConfig` docs) applies to
   ROUGH references only — distinguish by chain intent (simplest: a
   `ClaimsConfig` flag the cascade harness sets; or infer nothing and
   dial it). Self-probe claiming for single-tool ops is self-defeating
   BY CONSTRUCTION (+22.5 % time, zero quality — measured).
4. **Cascade A/B harness**: Op A = Ø3 ball all-over scallop; Op B =
   unified rest clearer (tip tool, FromRemainingStock after Op A).
   Baseline = all-over tip at equal effective cusp. Score: total time,
   group-filtered COLUMNS (on-size AND '>+.5' tails — per band), 0
   collisions, Region-span mix table. Then sweep ball Ø ∈ {2,3,4}.

## Traps (all hard-won this week — do not relearn)

- `planning/airrun_2026-06-01/wanaka.toml` is USER-MODIFIED: never
  commit, revert, or `save_project` over it.
- Compare quality/collisions ONLY at matched sim resolutions (GUI
  auto-res is 0.1 mm for the Ø1 tip; headless default 0.5). Known
  pre-existing: ~20 fine-res rapid grazes in the live op = descents
  through uncut slivers between rough passes that only exist on fine
  grids (pads can't bound; sliver-aware descents = future work; NOT a
  regression signal for the cascade, whose Op B follows a FINISHED
  reference).
- Never compare across regenerations (air-cut ladder variance, ~40
  moves); measure within one run; index columns by `ColumnDeviation`
  row/col, never inverse-transform XY.
- `UnifiedFinishReport` does NOT cross the session boundary — claims
  telemetry is invisible to harness/session consumers (cost hours: a
  false-negative label check hid that claims were firing). Carry what
  the harness needs onto the annotated result (rest_grid/rest_regions
  already carried) or assert via spans; consider a report slot.
- Skipped territory shows up as the '>+.5' TAIL, not the on-size share
  — gate both (s1_claims_ab has the pattern).
- Verdicts attribute by the op's OWN Region spans, never re-derived
  (dilated) band maps.
- Heavy cargo jobs exclusive (`free -g` first); core lib tests
  `--no-fail-fast` past the 3 known reds; zero-warning clippy;
  `cargo fmt` never bare rustfmt; long runs → log file + background.
- `s1_claims_ab` is a KNOWN-RED time gate documenting the self-probe
  dead end; `pencil_claims` defaults false (experimental).

## Current state (2026-07-13, HEAD ≈ 488f0cd)

S1 shipped + reined in: claims machinery (additive-only, default off),
Region spans + region_table, rest_grid/rest_regions carry-through,
per-group FIDELITY-COLUMNS, staleness-chain invalidation
(`invalidate_result_chain`), descent pad, enabled-finish-op harness
targeting. Live-validated: full coverage restored, quality == pre-S1.
Instrument trustworthy (three-way probe). Measured basis for scaling:
at scale 1 every ball Ø2–6 leaves 87–96 % of the mid-steep band as rest
area; Ø3 covers ~89 % of shallows; the tip runs ~3.2× over
deflection-safe feed (the ball's feed advantage is real).

## Paste-prompt for the next session

> Read `planning/v3_process_proof_prompt.md` and `planning/unified_v3_design.md`
> §0.a, then run the process-proof campaign in build-list order: wanaka ×2
> fixture → region-level territory → stock-referenced claims for
> finish-quality references → cascade A/B (ball Ø3 + unified rest clear
> vs all-over tip), then the ball-size sweep. Gates green → commit per
> slice. wanaka.toml is user-modified — never commit, revert, or save
> over it.
