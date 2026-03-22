# Review: Simulation & Export Flow

## Scope
User flow from "Run Simulation" through playback, inspection, collision check, to G-code export.

## What to review

### Simulation flow
1. User clicks "Run Simulation"
2. All enabled toolpaths collected and grouped by setup
3. Tri-dexel simulation runs on Analysis lane
4. Result: mesh, boundaries, checkpoints
5. Workspace switches to Simulation
6. Playback: play/pause/scrub/step

### Playback UX
- Timeline scrubber: smooth? responsive?
- Per-operation navigation: jump to op start/end
- Per-setup navigation
- Tool visualization during playback
- Cutting vs rapid color coding

### Collision check
- Separate from simulation — is this confusing for users?
- How are results displayed?
- Does the user need to re-run on parameter changes?

### Simulation staleness
- If user edits a toolpath, is simulation marked stale?
- Visual indicator?
- Auto-re-run?

### Export flow
1. Pre-flight checklist (preflight.rs)
2. File → Export G-code
3. Dialect selection (GRBL/LinuxCNC/Mach3)
4. File dialog
5. G-code written

### Export UX
- Single vs combined vs per-setup export
- Setup sheet generation
- SVG preview
- Is there a "verify before cut" workflow?

### Gaps
- Simulation deviation coloring not wired to renderer
- Rapid collision rendering not visible
- No simulation comparison (before/after parameter change)

## Output
Write findings to `review/results/38_flow_sim_export.md`.
