# Review: Architecture Conformance

## Scope
Does the actual code follow the stated architectural guardrails?

## Guardrails to verify (from CLAUDE.md)
1. "Keep the core library independent from GUI concerns"
2. "Treat the toolpath IR as the boundary between planning and post-processing/output"
3. "Keep import, tool modeling, operation generation, dressups, simulation, and export as distinct layers"
4. "Prefer extending existing core + worker + UI wiring path instead of creating parallel one-off flows"

## What to review

### Core independence
- Does rs_cam_core depend on any GUI crate? (Check Cargo.toml)
- Does core import anything from rs_cam_viz?
- Are there any "viz" or "render" types leaked into core?
- Does core have any egui dependency?

### Toolpath IR boundary
- Do operations produce Toolpath IR and nothing else?
- Does G-code generation consume only Toolpath IR?
- Does simulation consume only Toolpath IR?
- Are there operations that pass extra data around the IR?

### Layer separation
- Import: standalone module, no dependency on operations?
- Tool modeling: standalone, used by operations and simulation?
- Operations: depend on tools and import, produce toolpaths?
- Dressups: consume and produce toolpaths, no operation knowledge?
- Simulation: consume toolpaths and tool geometry only?
- Export: consume toolpaths only?

### Wiring path
- Is there a single path: UI event → controller → compute worker → core → result → state → render?
- Are there shortcuts or backdoors (UI directly calling core, etc.)?

### Architecture docs
- `architecture/high_level_design.md` — does it match reality?
- Any stale claims in architecture docs?

## Output
Write findings to `review/results/45_architecture.md` with a conformance matrix: Guardrail | Status | Violations (if any).
