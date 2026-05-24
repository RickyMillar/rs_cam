# F-019 — Stepover semantic cardinality across op families

- **Stage:** suggest
- **Severity:** low
- **Status:** deferred — re-evaluate after F-003 lands
- **First found in:** round-01 (2026-05-24)
- **Effort:** M (~150 LOC, type system + op-by-op branching)
- **Linked PRs:** —
- **Source audits:** suggest-pipeline

## Evidence

`apply_feeds_result_to_op` writes `result.radial_width_mm` into
`set_stepover` regardless of whether the op's stepover semantically
means:

- **"tool-fraction"** — drop_cutter ball stepover, scallop direction
  spacing
- **"scallop-derived"** — scallop_height converted to a stepover
- **"width of cut"** — pocket / adaptive radial WOC

For V-carve specifically, the LUT lookup uses tip-Ø ≈ 0; the LUT-
derived stepover may not correspond at all to the V-bit's actual cut
width which depends on depth.

Result: drop_cutter and scallop accept the same numeric value with
different physical intent. Looks reasonable but disagrees with the
vendor row's exact recommendation in subtle ways.

## Acceptance test

This is a code-clarity finding more than a numeric correctness one.
Acceptance test shape:

1. **Type test**: each op's stepover is reachable through a typed
   wrapper that names its semantic (`StepoverPctDiameter`,
   `StepoverScallopHeight`, `StepoverRadialWidthMm`, ...).
2. **Conversion test**: the LUT-side recommendation flows through a
   semantic-aware converter rather than a blind `set_stepover` write.

## Files

- `crates/rs_cam_viz/src/ui/properties/mod.rs:95` —
  `apply_feeds_result_to_op` (deleted by F-003)
- `crates/rs_cam_core/src/compute/operation_configs.rs` — per-op
  `set_stepover` impls (where they exist)
- `crates/rs_cam_core/src/feeds/calculate.rs` — recommendation
  output that needs typed conversion

## Fix shape

After F-003's `suggest_params` lands and the old
`apply_feeds_result_to_op` is gone, audit each op's stepover
semantics. Introduce typed wrappers if the variation across ops is
genuinely large. Otherwise, document the existing convention and add
a per-op converter helper.

## Risk

Medium. Type-system refactor. Don't attempt before F-003 lands —
many of the old call sites will disappear and the surface to refactor
will be smaller.

## Notes

- **Deferred until after F-003.** Re-evaluate scope once F-003 is in.
- **Out of scope:** changing what each op's stepover physically
  controls. Only the encoding.
