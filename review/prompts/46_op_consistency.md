# Review: Operation Consistency

## Scope
Do all 22 operations handle shared concerns (heights, boundaries, tools, errors) the same way?

## What to review

### Parameter handling
- Heights (clearance, retract, feed, top, bottom): applied consistently across all operations?
- Stock boundary clipping: supported by all operations?
- Keep-out zones: respected by all operations?
- Tool type support: which operations work with which tools? Is this documented?

### Dressup support
- Entry style (plunge/ramp/helix): supported by all relevant operations?
- Dogbone: only profiles? Or also pockets?
- Tabs: only profiles?
- Lead-in/out: which operations?
- Link moves: which operations?
- Arc fitting: all operations?

### Error handling
- Do all operations return Result or do some panic?
- Are error messages consistent in style?
- What happens with invalid parameters (zero depth, negative stepover)?

### Output format
- Do all operations produce the same Toolpath IR structure?
- Statistics: do all operations report the same stats?

### GUI config
- Does each operation's properties panel show exactly the relevant fields?
- Are defaults consistent (e.g., same default stepover for similar operations)?

### Method
- Create a matrix: 22 operations × shared concerns
- Fill in: supported / not supported / not applicable / inconsistent

## Output
Write findings to `review/results/46_op_consistency.md` with the consistency matrix.
