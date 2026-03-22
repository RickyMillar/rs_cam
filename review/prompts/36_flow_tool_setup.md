# Review: Tool + Setup + Workholding Flow

## Scope
User flows for tool library management, setup configuration, fixtures, and keep-out zones.

## What to review

### Tool flow
1. Add tool → select type → configure geometry
2. Edit tool in properties panel
3. Assign tool to toolpath
4. Duplicate / delete tool
- Is there validation when deleting a tool that's referenced by toolpaths?
- Can tools be reordered?
- Preset tools?

### Setup flow
1. Add setup → select orientation (FaceUp)
2. Configure datum (X/Y offset, Z top)
3. Add fixtures → position/size
4. Add keep-out zones → bounds
5. Assign toolpaths to setup (drag-drop)
- Multi-setup coordinate transforms: are they correct?
- Minimum 1 setup enforced — UI feedback when trying to delete last?

### Workholding flow
- Fixture visualization: wireframe box in viewport
- Keep-out subtraction from machining boundary
- Rigidity hardcoded to Medium — user can't change in GUI?
- Clamp mode: Clamped vs Floating — what's the difference in behavior?

### Gaps and issues
- Tool library persistence (saved with project? global presets?)
- Setup orientation: all 6 faces work correctly?
- Fixture collision with toolpath

## Output
Write findings to `review/results/36_flow_tool_setup.md`.
