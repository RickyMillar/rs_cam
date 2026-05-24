# F-016 — Drill `chip_welding` threshold not material-aware

- **Stage:** sim
- **Severity:** medium
- **Status:** open
- **First found in:** round-01 (2026-05-24)
- **Effort:** S (one-liner suspected)
- **Linked PRs:** —
- **Source audits:** smoke-run evidence

## Evidence

AS011 drill smoke case: stock was hardwood (Generic Hardwood). The
drill_gates report came back with:

```
chip_welding: { kind: within, observed: 4.0, threshold: 8.0 }
```

But `8.0` is the **softwood** D/d threshold per `CLAUDE.md`. Hardwood
should be `5.0`. The gate happened to pass either way (observed 4 <
hardwood threshold 5 < softwood threshold 8), but the threshold
lookup appears to ignore the stock material.

Per `CLAUDE.md` "Drill-specific thresholds" block:

| Material | depth_to_diameter fail threshold |
|---|---:|
| softwood / softwood plywood | 8 |
| hardwood / MDF / hardwood plywood | 5 |
| plastic | 4 |

## Acceptance test

1. **Unit test**: drill toolpath with stock=hardwood, depth=20mm,
   d=3mm → threshold returned must be 5.0, not 8.0.
2. **Unit test**: drill toolpath with stock=softwood, same geometry →
   threshold 8.0.
3. **Unit test**: drill toolpath with stock=plastic → threshold 4.0.
4. **Smoke verification**: re-run AS011 with stock changed to
   hardwood-equivalent; the `chip_welding.threshold` field must be
   5.0.

## Files

Probable suspects (read before fixing):

- `crates/rs_cam_core/src/tool_load/drill_gates.rs` — likely the
  threshold lookup
- `crates/rs_cam_core/src/tool_load/verdict.rs` — alternative location
- `crates/rs_cam_core/src/drill.rs` — drill cycle types

## Fix shape

One-liner (probably): make the threshold lookup branch on
`material.family()` instead of hardcoding `8.0`.

## Risk

Tiny.

## Notes

- **Out of scope:** changing the thresholds themselves. Just plumb
  the material correctly.
- Look for `peck_adequacy` and `plunge_feed` thresholds — they may
  have the same hardcoded-material issue.
