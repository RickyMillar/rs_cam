# Review: Operation Creation & Generation Flow

## Scope
User flow from "Add operation" through configuration to "Generate" and viewing results.

## What to review

### Creation flow
1. Select operation type from menu/panel
2. System assigns default tool, model, parameters
3. User configures in properties panel
4. User clicks "Generate"
5. Compute worker processes → result appears

### Configuration UX
- Are defaults sensible for each operation type?
- Does the properties panel show only relevant fields per operation?
- Are operation-specific parameters clearly labeled?
- Can the user see a preview before generating?

### Generation flow
- Status indicators: Pending → Computing → Done/Error
- Can user edit while computing? What happens?
- Queue behavior: generate all vs generate one
- Stale detection: does editing mark result as stale?

### Result inspection
- Toolpath visualization in viewport
- Statistics display (time, distance, moves)
- Isolation mode for single toolpath
- Visibility toggle

### Operation ordering
- Drag-to-reorder in project tree
- Move between setups
- Does order affect output? (G-code emission order, rest machining dependency)

### Gaps
- No auto-generation on parameter change
- No toolpath preview (only full generation)

## Output
Write findings to `review/results/37_flow_operation.md`.
