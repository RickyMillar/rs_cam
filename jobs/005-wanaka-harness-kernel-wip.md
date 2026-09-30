---
id: 005
state: queued
commit: 6aa77df9c56ca686e0d6e5038fa9b62066f628e8
needs_ricky_ok: true
---
## Commands
After 003 (same checkout, release profile):

    scripts/cargo_lane.sh test --release -p rs_cam_core -q --test p1_headless_ab_wanaka -- --ignored --nocapture 2>&1 | grep -E "rapid_collisions|op=|test result|panicked"

## Report back
rapid_collisions (must be 0), the per-op lines, pass/fail, wall time.

## Notes
The kernel fix moves every ball / bull / tapered drop-cutter path, so this
is the real-project collision check for it.
