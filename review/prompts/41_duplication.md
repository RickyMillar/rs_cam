# Review: Duplication & Abstraction Opportunities

## Scope
Identify copy-paste patterns, missed abstractions, and redundant code across the codebase.

## What to review

### Operation duplication
- 22 operations: how much code is shared vs duplicated?
- Common patterns: depth stepping, boundary clipping, dressup application, result construction
- Is there a shared "operation runner" or does each operation wire its own pipeline?

### Compute worker dispatch
- The giant match in execute.rs: how much boilerplate per arm?
- Parameter extraction patterns: repeated across operations?

### GUI property editors
- 6+ property files under properties/: shared patterns?
- DragValue, ComboBox, label patterns — abstracted or repeated?

### Tool type dispatch
- 5 tool types: how is tool-specific logic dispatched?
- Is there a trait method for everything or lots of match arms?

### Import paths
- STL, SVG, DXF: shared patterns in import.rs?
- Error handling patterns

### Quantify
- Identify the top 5-10 duplication hotspots by approximate duplicated LOC
- For each, assess whether abstraction would be net positive or premature

## Output
Write findings to `review/results/41_duplication.md` with specific code locations and a recommendation (abstract / leave as-is / consolidate) for each.
