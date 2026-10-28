# G-WLCLEANSPAN: the 3D Rough's semantic items carry their move ranges

Date: 2026-10-28. Built in an isolated worktree at `dbdf8f111` (C4b), own
lane script and target. The patch applies to the shared HEAD `09559c003`;
the core changes since the base are not in my files (help text, template).
The implementer did not commit.

Patch: `g_wlcleanspan_tree_vs_dbdf8f111.patch`, md5
`11153c59c07523d4833fd5e98db6646a`. 2 files: `compute/annotate.rs`
(+143/−3) and the new sentry
`tests/the_rough_semantic_items_carry_their_moves_g_wlcleanspan.rs`.
Pre-registration: `g_wlcleanspan_PREREG.md` (md5
`764ef6d1e61e3b065db1a6a115f1b79b`), approved for all three kinds, with
match by index in place of the ordinal pairing.

## 1. Change

- New `adaptive3d_structure_span(spans, events, i, len)`: the structural
  span of a Region, DepthLevel or Cleanup event, matched by KIND, START and
  END. End is the next event of the same family (a level or cleanup; a
  region start), the rule of `spans::push_adaptive3d_spans`. An empty event
  has no span and stays unbound. A non-empty event with no matching span is
  a disagreement of the two builders: unbound, and a `debug_assert` stops a
  debug build.
- `annotate_adaptive3d` computes the structural spans once
  (`spans_from_adaptive3d_annotations`) and binds the RegionStart,
  RegionZLevel, GlobalZLevel and WaterlineCleanup items through the existing
  `bind_span_scope`. Entry, PassSummary and the others keep their binding.
- Why match by start AND end: an empty level shares its start with the next
  family event, so a start-only match would bind it to its neighbour's
  moves. The unit test has that case.

## 2. Sentries

| Sentry | Result |
|---|---|
| `--lib an_empty_level_leaves_the_next_items_on_their_moves_g_wlcleanspan` | empty level unbound; next level (0..5); cleanup (6..9); entries at the same moves do not end them |
| `the_rough_semantic_items_carry_their_moves_g_wlcleanspan` | CP Global 6 items, 3 linked cleanups; Geodesic Global 6, 3; CP By Area + C4b 15, 7; every Region, DepthLevel and Cleanup item equals a span of its kind |
| `the_rough_semantic_items_are_deterministic_g_wlcleanspan` | ok |
| planted red (the matcher returns no span) | all three fail: "DepthLevel item 'Global Z 7.00' has no move range", "Region item 'Region 1' has no move range", and the unit test |

## 3. Re-bless check (no re-bless)

| Run | Result |
|---|---|
| 17 light core binaries that read the semantic trace or the narration and build a 3D Rough, plus the sim goldens | all ok except `perf_golden_sim_metrics`: the base red, same numbers (air-cut 1.9083 against 1.9295) |
| 3 light viz binaries of the same kind | all ok |
| the 46 light 3D Rough core binaries (the `debug_assert` check) | all ok except the 5 S11 "feeds not set" base reds (before any motion) |
| byte parity, op golden, C4 and C4b sentries | 6/6, 1/1, 5/5, 2/2, 2/2 |
| `rs_cam_core --lib` | 2730 ok |
| clippy all targets with full features, fmt | clean |

The re-bless list is empty. No golden stores semantic ranges, and the moves
and structural spans are unchanged.

Not run: the core full gate.

## 4. Records

`DEFECTS.md` G-WLCLEANSPAN row: marked fixed (pending commit), with the
wider cause.
