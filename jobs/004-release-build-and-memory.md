---
id: 004
state: queued
commit: 5b34e710
needs_ricky_ok: true
---
## Commands
On master 5b34e710 (contains the G-SIMMEM memory fix 75aa2869).

    git checkout --detach 5b34e710
    scripts/cargo_lane.sh build --release -p rs_cam_cli -p rs_cam_viz
    /usr/bin/time -v target/release/rs_cam_cli project planning/fixtures/rivmap100/rivmap100_memory_repro.toml --resolution 0.5 2>&1 | tail -30

Cap the second command so it cannot hurt the PC:
systemd-run --user --scope -p MemoryMax=10G -p MemorySwapMax=0 <the command>

## Report back
Build ok / first error; build wall time. For the repro: exit code, wall
time, "Maximum resident set size" (the cloud measured 4.59 GB peak, 514 s,
on 4 cores; before the fix it aborted on a 9.54 GB allocation), and the
project summary lines (collisions, cycle time). Leave target/release/rs_cam_gui
built for a later GUI check.

## Notes
Do not start the GUI for this job.
