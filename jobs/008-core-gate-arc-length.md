---
id: 008
state: queued
commit: 0015dcfcb8a4e5df620bbeae7ed361fe6c0b9e80
needs_ricky_ok: true
---
## Commands
Branch origin/claude/dazzling-noether-jhue57 at 0015dcfcb8a4e5df620bbeae7ed361fe6c0b9e80: master fd06f407 plus
ONE change: the cycle-time model times an arc by its arc length (it used
the chord). This moves many time pins; I need the full list.

    git fetch origin claude/dazzling-noether-jhue57
    git checkout --detach 0015dcfcb8a4e5df620bbeae7ed361fe6c0b9e80
    scripts/cargo_lane.sh test -p rs_cam_core --features heavy-tests,research,test-support --no-fail-fast -- -q

## Report back
Same format as 001. For EVERY failing test also give the full assertion
message lines (the old and the new value), because each is a time pin to
re-pin with its cause. Mark which targets also fail at fd06f407 if you
know (001 was 5b34e710: scallop_trace_survives_relink and
crease_own_region_pr6b are known pre-existing).

## Notes
Also useful but lower priority, after the gate:
 and
 on the same commit.
