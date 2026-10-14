# Operation architecture: orchestration plan (2026-10-14)

Source: `AUDIT.md` in this folder (the verdict, six kernel shapes, defects
D1–D11). This file says who does what, in which order, and how each step is
proved.

## Rules for every agent

- Agents edit and test. Agents never commit, push, stash, reset or checkout.
  The lead reviews each change by path and commits it with explicit paths.
- Work on branch `op-arch` (made from master by the lead). The lead merges to
  master at the end of each wave, after the review and the live check.
- One cargo job at a time: `timeout <s> scripts/cargo_lane.sh <args>`.
- No large gates: named sentries, `--lib <filter>`, the folder sentries, then
  the full clippy line and `cargo fmt --all -- --check`. Ask before any test
  run over 3 minutes.
- One agent edits the checkout at a time (shared checkout, one build lane).
  Research agents are read-only and can run in parallel with an editor.
  No worktrees (disk is at 87 %; a worktree target is 10–55 GB).
- Constants are derived from existing constants or a cited source.
- STE prose in comments, docs and reports. No legacy shims; state breaks.
- Each fix gets a sentry. Each sentry is proved red once with the defect
  put back, then restored.
- An agent that cannot finish an item cleanly stops and reports it open.

## Safety net first: W0 golden toolpath fingerprints

Before any refactor, one agent builds
`tests/every_op_keeps_its_toolpath_g_opgolden.rs`:

- one small fixture per `GeometryRequirement` (Stock, Polygons, Mesh, Both)
  and per tool kind the registry row requires;
- every op in `OperationType::ALL`, fresh stock and rest stock where the op
  reads it;
- a fingerprint per op (move count, cut length, bbox, a hash of the moves via
  `export/fingerprint.rs`), stored as a blessed file;
- a bless switch for an intended change, which the lead approves by path.

Every later step that claims "no change in output" proves it with this test.
A step that changes output on purpose re-blesses only the ops it names.

## Waves

### W1 — defects that can leave material or make an unsafe link

Sequential, one editor each. Order by risk.

| Item | Defect | Work | Proof |
|---|---|---|---|
| W1.1 | D1 two link ceilings disagree | Research the two rules; the lead rules (R1); one `LinkCeiling` factory that both families call | sentry: same inputs → same ceiling on both families; G-opgolden re-bless for the changed family only |
| W1.2 | D2 2D Adaptive keeps only `machinable_vec[0]` | plan every piece | sentry: two-island region → both islands cleared (sim, no material above floor + eps) |
| W1.3 | D4 Chamfer side | simulate first; fix only if confirmed | sentry: chamfer width and side measured on the sim stock |
| W1.4 | D3 Inlay plug in the workpiece | prove with a sim sentry; write the split proposal (R2); implement after the ruling | sentry: the female op cuts nothing outside the female region |
| W1.5 | D11 GUI prior-op check stops at the setup | walk core `carve_order_before` | viz test: rest op first in setup 3 names the setup-2 op |
| W1.6 | D9 red schema test | make `diameter` consistent with the schema (serde default or unexposed decision) | the red test turns green |
| W1.2b | D2 in Adaptive3d slices (same planner, still plans only `machinable_vec[0]`) | reproduce on a mesh with a split slice; fix in the planner or the 3D adapter | sentry + G-OPGOLDEN |
| W1.3b | Operator 2026-10-14: Chamfer on inside edges too (pocket walls and holes) | a side setting on Chamfer (outside / inside), or chamfer every edge of the outline including holes; same bevel rule | sentry: inside bevel width and depth on a pocket and a hole |
| W1.4b | VCarve's Finish lead-in cuts 238 cells up to 0.75 mm outside the outline (found in W1.4) | strip or contain the lead on VCarve as on Inlay | sentry: no cell outside the outline |
| W1.7 | R3: retire the Rest op | delete the op on every surface (core, GUI, MCP, CLI, catalog, docs); state the load break | G-OPGOLDEN loses its Rest cases on purpose; build + clippy clean |
| W1.1b | R1 follow-up: links are not scored against the stock | a simulation check that flags a fed link whose tool passes through remaining stock | sentry: a link forced below the stock top is flagged |

Live check after W1: rebuild release, load `testpiece_mini`, generate all,
simulate, compare with commit 7a9f5d84 (expect no change: it uses none of the
fixed ops except links).

### W2 — shared services

| Item | Service | Removes |
|---|---|---|
| W2.1 | One rest-reference resolver (overlap guard + one fallback) | 3 copies (`finish_3d.rs`, `dressup_apply.rs`, pencil `detectors.rs`) |
| W2.2 | Kernel prologue: cancel checked FIRST, `require_mesh`/`require_index`, tool refusals from `tool_constraints` | the copies in every adapter and `from_static_checks.rs:202` |
| W2.3 | Findings through a returned report struct | hand-written offset-failure / truncated-core / rest-level records |
| W2.4 | `fn capabilities(&self) -> Capabilities` on the config, static row as default; move the membership lists into it | D6 (`predict.rs` fail-open and `== Drill`), `narrate.rs` lists, the three catalog matches |
| W2.5 | Result provenance tag `Dressed | Modulated { sim_rev }`; export and narrate state it | D10 |

Proof: G-opgolden shows no change for W2.1–W2.4. W2.5 changes only text.

### W3 — the Layered shape for the 2.5D family

- W3.0: trait `LayeredOp` and `run_layered` with `LevelInfo { z, index,
  is_final }`, `LevelStock`, a `ladder()` hook, region-first default plus
  `region_ordering` (R4), the
  prepare cache per region, spans and findings owned by the framework.
- W3.1: move Pocket onto it. G-opgolden: no change. The G-RESTLEVELS sentry
  stays green.
- W3.2: Zigzag, Trace, Profile (no output change on fresh stock).
- W3.3: Face onto the framework ladder; fixes D7.
- W3.4: `RestUse` per op: Zigzag `LevelClip`, Profile/Trace `LevelSkip`,
  Face `LevelClip` on the stock rectangle, 2D Adaptive `LevelMask` (one stock
  read shared with the clip), Drill `StockColumn` (fixes D8). Each gets a case
  in the G-RESTLEVELS sentry. (The Rest op is gone after W1.7.)
- W3.5: Chamfer and Waterline as Layered ops (Waterline: `ContourFilter`).

Live check after W3: the mini project; then Z22/Z23 as Zigzag pattern and a
rest Profile on a fixture.

### W4 — conformance suite over all ops

Generated from `for_each_op!`: cancel first, rest (gated on the instance
capability), empty (`empty_is_feature_selective`), flipped frame (where the
setup transform is read), determinism, schema round trip. The existing
cancel-flag test moves into it.

### W5 — params derive

Ruling R5 = A: a new proc-macro crate (`[lints] workspace = true`; errors through `syn::Error`). `#[derive(OpParams)]` gives field
list, type names, ranges, help, aliases, `unexposed`. The GUI `dv()` reads
the derived range (D5 gets a direct fix in W1 first); MCP validates against it. `OpUiRow::draw` stays
for layout and custom widgets. The text-parsing test
`param_defs_cover_every_config_field` is deleted.

### W6 — 3D shapes

Height field, composed surface, volumetric, curve. Plan this wave after W3
and W4, from what they measure. It is not scheduled now.

## Rulings (operator, 2026-10-14)

| # | Ruling | Effect on the plan |
|---|---|---|
| R1 | **D**: one `LinkCeiling` builder. Stock = remaining stock when known; fallback top = `max(stock_bbox.max.z, heights.top_z)` (the G-PECKSPLIT rule); radius = `envelope_radius_mm()`; each op states in that builder whether it needs a ceiling on fresh stock (engraving: always; finishing: only with remaining stock) | W1.1. Added W1.1b (a simulation check that scores fed links against the stock: today a link through stock is a silent plough) and backlog B1 (shank and holder in the ceiling; pencil searches only the tip radius) |
| R2 | **A**: split Inlay. "Inlay" cuts only the V walls of the pocket; the floor is a normal Pocket with a flat end mill; a new "Inlay Plug" op (mirrored, start depth) runs in its own project for the plug board | W1.4. Also fix: plug not mirrored; glue gap wrong for bits that are not 90°; default flat tool is imperial |
| R3 | **Retire the Rest op**, unless the reach map uses it. Checked: the reach map does not use `ops::rest` (only a doc mention in `surface/rest_field.rs:37`). Operator note: the reach map is a visual, not planning, and must not live with planning code | W1.7: delete the Rest op on every surface; state the load break |
| R4 | **B**: region-first default plus the `region_ordering` option (the 3D Rough enum) on every Layered op | W3.0. 3D Rough keeps its own default (Global, level-first) |
| R5 | **A**: a proc-macro crate for `#[derive(OpParams)]`, "clean now" | W5 uses a derive. The GUI range drift (D5) and the pin-drill schema test (D9) still get their own direct fixes in W1 |

## Per-item flow

1. Read-only research agent (only when the item needs one), in parallel with
   the current editor.
2. Editor agent: code + sentry + red proof + gates; report ≤ 500 words.
3. Lead: review the diff by path, run the item's sentry once, commit.
4. Next item.

## Status

| Wave | State |
|---|---|
| W0 | DONE: G-OPGOLDEN, 37 cases, 24/24 ops, 2 s |
| W1 | W1.1 DONE 877c93e5; W1.2 DONE b5dc7ed4; W1.3 DONE 0c8f562a (chamfer cut outside the edge); W1.4 DONE 08458617 (FM1 bless pending) |
| W2–W6 | not started |

## Backlog (not scheduled)

- B1: shank and holder radius in the link ceiling; pencil link search uses the
  tip radius, not the envelope (R1 research).
- B2: a second board in one project (would join the Inlay pocket and plug).
- B3: D5 GUI ranges read `ParamDef.range` (can land before W5).
