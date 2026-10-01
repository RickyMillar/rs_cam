---
id: 009
state: queued
commit: 0617bf8e
needs_ricky_ok: true
---
## Commands
On master 0617bf8e (contains the kernel fix fd06f407 and memory waves 1-3).
Ricky's rivmap350.toml (not in the repo), release build, under the
MemoryMax=16G cap as before.

A. The 8 rapid collisions in "3D Rough 8" (seen in jobs 006/007). Run the
   project through the CLI or MCP and dump every rapid collision: toolpath,
   move index, start/end XYZ, the tool, the move intent/span kind, and the
   resolution used. Then re-simulate only that op's chain at 0.25 mm and
   0.125 mm (or the finest that fits the budget) and say for each collision
   whether it persists (a true strike) or disappears (a sim artefact).
   If the CLI `project` command has a diagnostics/collision dump flag, use
   it; otherwise MCP get_simulation_issues / the rapid-collision list.

B. The wall texture (job 007): re-simulate the finishing chain at 0.25 mm
   and screenshot the same wall view as results/007-walls-0.2mm.png. A
   moire changes spacing or direction between 0.2 and 0.25; a path defect
   stays. Ricky's eye is the verdict.

## Report back
A: a table (op, move, XYZ, tool, intent, 0.2 / 0.25 / 0.125 persists?).
B: the screenshot and Ricky's verdict.
Also: does "3D Rough 8" use a tool / strategy that fd06f407 changed
(ball / bull / tapered drop-cutter, arc fit, TSP)?

## Notes
If collisions persist at finer cells, they are real: do NOT run that G-code
on the machine before the lead has looked.
