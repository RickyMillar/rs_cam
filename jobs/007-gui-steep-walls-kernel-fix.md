---
id: 007
state: queued
commit: fd06f407
needs_ricky_ok: true
---
## Commands
master fd06f407 carries the drop-cutter kernel fix (ball / bull / tapered
tools sat 0.3-1.8 mm too low on sloped mesh edges), fed-chord refinement,
the arc-fit 3D tolerance and the TSP guard.

    git fetch origin master && git checkout --detach fd06f407
    scripts/cargo_lane.sh build --release -p rs_cam_viz -p rs_cam_cli

Then Ricky opens rs_cam_gui (under MemoryMax=16G as in 006), loads the
project whose steep walls showed bumps/gouges, regenerates all finishing
toolpaths and simulates at 0.2 mm.

## Report back
- Ricky's visual verdict on the steep walls vs before (screenshots of the
  same view before/after if he has a before).
- From the GUI or MCP: rapid collisions, cycle time per finishing op, and
  the simulation's deviation / overcut figures if shown.
- Peak RSS during generate and simulate.

## Notes
The previous GUI build (job 004) predates this fix. Tiered-finishing
defaults are unchanged (weld dial 3, arc tolerance 0.05) until Ricky
approves the changes.
