# Tool Library Design

## Problem

Tools are currently per-project only. Users must recreate their tool library for each new project. Common tools (1/4" end mill, 1/8" ball nose) should persist across projects.

## Architecture

### Storage

```
~/.rs_cam/tool_library.yaml
```

YAML file with the same ToolConfig structure used in project files. Each tool has a unique `library_id` (UUID) in addition to the per-project `ToolId`.

### Data Model

```rust
struct ToolLibrary {
    tools: Vec<LibraryTool>,
}

struct LibraryTool {
    library_id: Uuid,          // persistent across projects
    tool: ToolConfig,          // same struct used in projects
    tags: Vec<String>,         // user-defined categories ("roughing", "finishing")
    notes: String,             // free-form notes
    last_used: Option<String>, // ISO date
}
```

### Project Integration

Projects reference library tools by `library_id`. On project load:
1. If a library tool exists, merge any updated properties (name, holder dims)
2. If a library tool is missing, the project's copy is used standalone
3. Projects always embed a full copy of each tool for portability

### UI

- Setup workspace: "Tool Library" section in left panel (below Models)
- Browse, search, add to project from library
- Import/export library as YAML
- "Save to library" button on tool editor

### Migration

- No breaking changes. Existing projects continue to work.
- On first run with library support, library file created empty.
- Users opt in by saving tools to library.

## Status

Architecture designed. Implementation deferred to a future phase.
