# G-WLCLEANSPAN: the 3D Rough's semantic items get their move ranges (pre-registration)

Date: 2026-10-28. Base: `geodesic-integration` `dbdf8f111` (C4b). Measured
in an isolated worktree at that commit, with a temporary probe test (deleted
before any patch). No product code changed yet.

## 1. What the trace records today (measured)

Two records describe the 3D Rough structure:

1. **Structural spans** (`compute/spans.rs:520`
   `spans_from_adaptive3d_annotations` → `push_adaptive3d_spans`). A
   `DepthPass` or `WaterlineCleanup` span runs from its event's move index
   to the NEXT level or cleanup event (`spans.rs:547-560`, `:575-598`); a
   `Region` span runs to the next `RegionStart` (`:600-625`). These are
   correct.
2. **Semantic items** (`compute/annotate.rs:217` `annotate_adaptive3d`,
   called from `compute/execute/rough_3d.rs:232`). Each item ends at
   `move_end(&move_indices, i, tp_len)` (`annotate.rs:20`): the NEXT
   annotation of ANY kind. The first child of a level or of a cleanup is a
   `PassEntry` at the same move index (the entry of its first cut), so the
   range is empty, `bind_to_toolpath` (`trace/semantic_trace.rs:898`, via
   `range_geometry`) binds nothing, and the item has `move_start: None`.
   The `RegionStart` arm (`annotate.rs:241`) never binds at all.

Probe (two-pocket fixture of C4b, plus Geodesic), items by kind (count,
linked):

| Case | Cleanup | DepthLevel | Region | Entry | Operation |
|---|---|---|---|---|---|
| CP, Global | 3, 0 | 3, 0 | — | 20, 20 | 1, 1 |
| Geodesic, Global | 3, 0 | 3, 0 | — | 28, 28 | 1, 1 |
| CP, By Area (bottom run only) | 1, 0 | (as below) | 2, 0 | | |
| CP, By Area, C4b masked runs on | 7, 0 | 6, 0 | 2, 0 | 17, 17 | 1, 1 |

The structural `WaterlineCleanup` spans of the same runs have their ranges
(for example CP Global: (72, 104), (146, 175), (212, 239)). The rapid
reorder and every other dressup were ruled out: with all dressups off, the
items are still unlinked.

**So G-WLCLEANSPAN is wider than the cleanup.** Every Region, DepthLevel and
Cleanup item of a 3D Rough has no move range; only Entry items (which carry
an explicit end, `entry_end_move_idx`) and the operation root are linked.

## 2. The fix (no new span or item kind)

`annotate_adaptive3d` binds each Region, DepthLevel and Cleanup item to the
STRUCTURAL span made from the same events, through the existing
`bind_span_scope` (`annotate.rs:204`). One rule then gives both records,
by construction:

- compute the spans once (`spans_from_adaptive3d_annotations(events,
  tp_len)`, the function `rough_3d.rs::spans` already calls);
- the k-th `RegionZLevel`/`GlobalZLevel`/`WaterlineCleanup` event binds to
  the k-th `DepthPass`/`WaterlineCleanup` span in start order (the spans
  are built from exactly those events, in order; an empty span is skipped
  by both, so the item stays unbound there, which is correct: it covers no
  move);
- a `RegionStart` binds to its `Region` span;
- `PassEntry`, `PassSummary` and the others keep their binding as today.

This covers all three paths: Global, the By Area bottom run, and the C4b
masked runs (each is a `WaterlineCleanup` event).

## 3. What can move (predicted)

- **Toolpaths: nothing.** The moves, the structural spans and every
  `spans_hash` stay as they are. Byte parity and the op golden do not
  change.
- **Semantic consumers** that read item ranges see the levels, regions and
  cleanups for the first time:
  - the cut trace's per-sample `semantic_item_id`
    (`stock/simulation_cut.rs:185`) can point at a level or cleanup item
    in place of the operation;
  - `narrate_toolpath` and the span tools (MCP `inspect_spans`,
    `get_cut_trace`) can place and time the cleanup.
- No checked-in golden stores semantic move ranges (search: no `.json`,
  `.toml` or `.txt` fixture holds `move_start` or `semantic_trace`). The
  MCP wire snapshot lists tools, not data, so it does not change.
- Tests that assert on 3D Rough semantic items or narration text may move.
  Predicted re-bless list: **empty**. I verify it with a run of every light
  core and viz test that names `narrate`, `semantic` or `semantic_item_id`
  (about 30 binaries), and the sim goldens. Any red is reported, not
  re-blessed, until you rule.

## 4. Sentries

1. `the_rough_semantic_items_carry_their_moves_g_wlcleanspan` (session):
   CP Global, Geodesic Global, CP By Area with "Contour each level" on (the
   C4b masked runs). Every Cleanup, DepthLevel and Region item has a move
   range, and it equals its structural span (start, end − 1). Non-vacuity:
   each case has at least one Cleanup item that covers at least one move.
2. Determinism: unchanged (sentry 1 runs twice in one process and compares
   the item ranges).
3. Planted red: `annotate_adaptive3d` keeps `move_end` for levels and
   cleanups. Then sentry 1 fails with "no move range" (today's state).

## 5. Size

About 40 lines in `compute/annotate.rs`, 1 test file, plus the
`DEFECTS.md` row (closed with the wider cause) and the folder notes if they
name the gap.
