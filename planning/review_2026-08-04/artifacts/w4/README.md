# W4 — adversarial 2D campaign artifacts

Generated at `0e7d38b`, debug, 2026-08-04.

| file(s) | what |
|---|---|
| `fixture_<name>.svg` (22) | every hostile fixture, as authored. Exterior white, holes cyan, **vertices as orange dots** so sub-epsilon clusters read as thickening rather than as nothing. |
| `<op>_<fixture>.svg` (28) | emitted toolpath over its fixture. Cutting amber, rapids dashed red. |
| `fixture_mechanisms.md` | the measured non-vacuity table per fixture — reflex corners, min segment, sub-epsilon segments, min non-adjacent gap, min curvature radius, components, nesting depth, min area, self-intersection. |
| `campaign_matrix_partial.md` | the operation × fixture rows collected before the run was stopped. **33 of 198 cells.** |

## What is deliberately not here

**The v-carve and inlay renders were dropped.** At 95k–136k emitted moves each
they are 3–12 MB of SVG apiece — 44 MB in total — and a planning directory is
not a place to park them. They regenerate:

```bash
R2_ARTIFACT_DIR=/tmp/r2 cargo test -p rs_cam_core \
  --test adversarial_2d_campaign_r2 -- --ignored --nocapture adversarial_2d_full_campaign
```

The move counts are in `campaign_matrix_partial.md` and are worth reading on
their own: `inlay × dendrite` emits **136 489 moves** for 32.8 m of cutting on
a 200 × 80 mm fixture, and `vcarve × comb-16` **96 164**.

## Why the matrix is partial

Two reasons, both recorded rather than papered over:

1. **`inlay × rosette-24` ran past 129 s** against a pre-registered 60 s
   ceiling, CPU-bound, while recovering from four contained
   `cavalier_contours` panics (`pline_seg.rs:33`). That is finding **F-12** in
   `ADVERSARIAL_2D_FINDINGS.md`, and the run was stopped rather than left to
   hold the single Cargo slot.
2. **The machine's root filesystem reached 100%** (889 GB of 935 GB) during
   the wave, which ended Cargo availability entirely.

The 165 uncollected cells are stated as `NOT RUN`. They are not passing.
