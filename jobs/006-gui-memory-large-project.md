---
id: 006
state: queued
commit: 5b34e710
needs_ricky_ok: true
---
## Commands
Use the release rs_cam_gui that job 004 builds on master 5b34e710 (contains
the G-SIMMEM memory fix 75aa2869; does NOT contain the drop-cutter kernel
fix yet). Ricky drives the GUI; the runner watches memory.

1. Start the GUI under a memory cap so the desktop cannot go down again:
   systemd-run --user --scope -p MemoryMax=16G -p MemorySwapMax=0 target/release/rs_cam_gui
2. Ricky loads his large project (rivmap350.toml, 380 x 510 x 26 mm stock,
   8 toolpaths, the ~8 h R1.0 scallop) and runs the full simulation at 0.5 mm
   (Auto if it picks 0.5).
3. The runner samples the GUI's RSS every 5 s during generation + simulation
   (e.g. `while sleep 5; do ps -o rss= -p <pid>; done > rss.log`).

## Report back
Peak RSS (GB) and when it peaked (generate / simulate / artifact write),
total simulation time, whether the GUI stayed responsive, rapid collision
count and cycle time shown, and any error or panic. Before the fix this
project went 2.3 -> 19.4 GB in 30 s.

## Notes
The steep-wall visual check waits for the kernel-fix package (a later job).
Ricky's own list (model-outline boundary + holes, tier weld dial, flute-reach
advisory, G-EMPTYADD, etc.) can run in the same GUI session; report anything
odd he sees as free text.
