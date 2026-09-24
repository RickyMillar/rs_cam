# G6 LUT discrepancies

Date: 2026-09-24. Group: G6 (drill).

No discrepancy found.

- The LUT holds 0 drill rows (389 observations; no `operation_family`
  "drill" and no drill `tool_family`). G6 has nothing to compare against.
- The G6 fetch re-downloaded two charts that the LUT manifest already lists.
  Both PDF hashes match the manifest, and the `pdftotext -layout` output of
  the Spektra chart is byte-identical to the stored LUT text:
  - `amana_spektra_spiral_plunge_v24`: sha256
    `5b6fef854b2cf6b422e2eb5bbc86e29b4dcab5b19195b7ff228b70f99cf86d3a`.
  - `amana_ball_nose_v7`: sha256
    `e851a270a9958a9affc17a24c8e6a8ef51d63a813f384b7043670d676cae3563`.
- G6 did not re-check the LUT's Spektra side chip-load rows. G6 used only
  the "Ramp Down" column, which no LUT row carries.
