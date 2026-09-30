---
id: 003
state: queued
commit: 6aa77df9c56ca686e0d6e5038fa9b62066f628e8
needs_ricky_ok: true
---
## Commands
Branch origin/claude/dazzling-noether-jhue57 at 6aa77df9 (a PRE-FINAL
snapshot of the kernel-fix package: drop-cutter edge contact, arc-fit
combined tolerance, fed-chord refinement; the TSP no-regression guard may
still be missing). Run after job 001 ends.

    git fetch origin claude/dazzling-noether-jhue57
    git checkout --detach 6aa77df9c56ca686e0d6e5038fa9b62066f628e8
    scripts/cargo_lane.sh test -p rs_cam_core --features heavy-tests,research,test-support --no-fail-fast -- -q

## Report back
Same format as 001: lib result line; ok / FAILED counts; each failing
target + test + first panic line; wall time; build errors. Then a
two-column list: targets that fail here but PASS in job 001 (these are the
package's regressions — most important), and targets failing in both.

## Notes
Expected: capability_link_moves_safety may fail (2 tests; known, being
fixed). heatmap_two_arc_divergence_a1 and scallop_trace_survives_relink
fail on master too.
