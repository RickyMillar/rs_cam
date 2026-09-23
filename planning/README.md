# Planning index

This directory holds the current status, the open plans and the evidence that
live sentries cite. It is not a single specification. Read `CLAUDE.md` in this
directory for the rules, then `PROGRESS.md` for the snapshot.

## Deleted material and how to get it back

The structure purge of 2026-09-17 removed 1010 files. Nothing was archived —
the operator ruling of 2026-09-16 retires archiving. Every removed file stays
retrievable at an annotated tag:

```
git show planning-pre-purge-2026-09-17:planning/<path>
```

`DELETED_INDEX.md` lists each deleted package, what it decided and why it went.
A `planning/…` path in a doc comment or in `PROGRESS.md` that no longer exists
is retrievable the same way.

## Start here

| File | Purpose |
|---|---|
| [`PROGRESS.md`](PROGRESS.md) | Current snapshot, newest first |
| [`CLAUDE.md`](CLAUDE.md) | How to use this directory |
| [`TECH_DEBT_REGISTER.md`](TECH_DEBT_REGISTER.md) | Open debt no gate can fail on |
| [`AGENT_CODEMAP.md`](AGENT_CODEMAP.md) | Where each subsystem lives |
| [`DELETED_INDEX.md`](DELETED_INDEX.md) | What the purge removed, and why |

## Open packages

| Package | State |
|---|---|
| [`linking_2026-09-09/`](linking_2026-09-09/) | G-LINKSTAGE spec, paused |
| [`island_clip_2026-09-09/`](island_clip_2026-09-09/) | Spec; the experiments wait on the GUI |
| [`ui_review_2026-09-14/`](ui_review_2026-09-14/) | UR4/UR5 open; `crates/rs_cam_viz/CLAUDE.md` points here |
| [`roughing_strategy_ab_2026-09-07/`](roughing_strategy_ab_2026-09-07/) | Measured; an operator ruling on the bar is open |
| [`arch_consolidation_2026-09-09/`](arch_consolidation_2026-09-09/) | Live tracker; five G- items open |
| [`structure_2026-09-17/`](structure_2026-09-17/) | This programme |
| [`gen_sim_rest_ux_2026-09-18/`](gen_sim_rest_ux_2026-09-18/) | Plan; seven rulings open (generate ↔ simulate ↔ rest, one path, one indicator) |
| [`corne_case_analysis_2026-09-18/`](corne_case_analysis_2026-09-18/) | Analysis; eight rulings open (silhouette holes make the rough weave, waterline Auto ladder is one level, a diagram drag pinned Z −1.37) |
| [`feeds_matrix_2026-09-23/`](feeds_matrix_2026-09-23/) | Plan; not started (every tool type on every operation: what fires, what ships, what backs it; a `FeedsSupport` declaration that refuses an unbacked suggestion) |

## Other packages held under this directory

Two 2026-09-16 programme records: `duplicate_sweep_2026-09-15/` and
`tech_debt_2026-09-16/`. Four packages another account owns:
`load_model_2026-09-16/`, `feeds_rework_2026-09-15/`,
`feed_modulation_calibration/` and `ui_premium_2026-09-13/`.

The rest are closed campaigns kept because a live sentry names them as its
pre-registration, because `CREDITS.md` names them as attribution for shipped
data, or because a test reads a file in them at run time. Two directories a
test reads directly: `toolpath_acceptance/` (the CLI smoke CSV) and
`gcode_current_outputs/` (the post-processor captures).

## Conventions

- Product-facing capability docs belong in the repo root (`README.md`,
  `FEATURE_CATALOG.md`, `CREDITS.md`), not here.
- AI analysis reference: `research/AI_MACHINIST_ANALYSIS_REFERENCE.md`.
- A dated report is evidence of what was measured then. It is not an
  evergreen instruction.
