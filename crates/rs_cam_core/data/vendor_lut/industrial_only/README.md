# Industrial-only vendor LUT observations

Files in this directory document vendor chipload/feed data that is
**not loaded** by `VendorLut::embedded()`. Feeds contract P13 item 5
(operator ruling D13, 2026-11-05): these rows are evidence for the model,
not a lookup to ship from. The baseline sentry
`crates/rs_cam_core/tests/the_industrial_rows_are_a_baseline_not_a_lookup_p13.rs`
loads them, checks that no row of theirs is in the shipped lookup, and
checks the in-series size law against them.

History: commit 51edd494d (2026-06-01) had copied the four Freud rows into
`observations/freud_solid_carbide.json`, so from then until P13 they
shipped although this file said they did not. P13 removed the copies.

## Why a sibling directory instead of `observations/`?

The invariant for `crates/rs_cam_core/data/vendor_lut/observations/` is
that every file in it ships in `embedded()`. Industrial-CNC data
calibrated for ~10–15 kW spindles would propose feeds the runtime
power gate would then have to clip on a hobby spindle, so we keep it
out of the embedded set until a per-spindle gate exists. A separate
directory makes the boundary explicit at the path layer rather than
hiding it behind a softened validator threshold.

## Files

| File | Rows | Reason out of `embedded()` |
|------|------|----------------------------|
| `freud_solid_carbide_industrial.json` | 4 | Evidence for the size law (P13 item 5). 1/2" chiploads 0.46–0.69 mm/tooth; calibrated for industrial CNC, would breach `validate_lut.py`'s 0.5 mm/tooth `chipload max suspicious` threshold and demand unreachable spindle power on a Shapeoko-class router. See `planning/tool_kinematics_chipload_audit_2026-05-31.md` carry-forward item #1. |
