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
| W1.6 | D9 red schema test | make `diameter` consistent with the schema | the red test turns green |

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
  is_final }`, `LevelStock`, a `ladder()` hook, region-first order (R4), the
  prepare cache per region, spans and findings owned by the framework.
- W3.1: move Pocket onto it. G-opgolden: no change. The G-RESTLEVELS sentry
  stays green.
- W3.2: Zigzag, Rest, Trace, Profile (no output change on fresh stock).
- W3.3: Face onto the framework ladder; fixes D7.
- W3.4: `RestUse` per op: Zigzag `LevelClip`, Profile/Trace `LevelSkip`,
  Face `LevelClip` on the stock rectangle, 2D Adaptive `LevelMask` (one stock
  read shared with the clip), Drill `StockColumn` (fixes D8). Each gets a case
  in the G-RESTLEVELS sentry. The Rest op's `prev_tool_radius` model waits for
  ruling R3.
- W3.5: Chamfer and Waterline as Layered ops (Waterline: `ContourFilter`).

Live check after W3: the mini project; then Z22/Z23 as Zigzag pattern and a
rest Profile on a fixture.

### W4 — conformance suite over all ops

Generated from `for_each_op!`: cancel first, rest (gated on the instance
capability), empty (`empty_is_feature_selective`), flipped frame (where the
setup transform is read), determinism, schema round trip. The existing
cancel-flag test moves into it.

### W5 — params derive

Ruling R5 first (a new proc-macro crate). `#[derive(OpParams)]` gives field
list, type names, ranges, help, aliases, `unexposed`. The GUI `dv()` reads
the derived range (fixes D5); MCP validates against it. `OpUiRow::draw` stays
for layout and custom widgets. The text-parsing test
`param_defs_cover_every_config_field` is deleted.

### W6 — 3D shapes

Height field, composed surface, volumetric, curve. Plan this wave after W3
and W4, from what they measure. It is not scheduled now.

## Rulings the operator owns

| # | Question | When |
|---|---|---|
| R1 | The one link-ceiling rule (tool or envelope radius; stock top or `top_z`; always or only with stock) | before W1.1 code |
| R2 | Inlay: two ops in two setups, or one op that emits two programs | before W1.4 code |
| R3 | The Rest op: keep its `prev_tool_radius` model beside real-stock rest, or retire it | before W3.4 touches Rest |
| R4 | Region-first as the default order for Layered ops | before W3.0 |
| R5 | A new proc-macro crate for the params derive | before W5 |

The lead brings each ruling with a recommendation and evidence.

## Per-item flow

1. Read-only research agent (only when the item needs one), in parallel with
   the current editor.
2. Editor agent: code + sentry + red proof + gates; report ≤ 500 words.
3. Lead: review the diff by path, run the item's sentry once, commit.
4. Next item.

## Status

| Wave | State |
|---|---|
| W0 | not started |
| W1 | not started |
| W2–W6 | not started |
