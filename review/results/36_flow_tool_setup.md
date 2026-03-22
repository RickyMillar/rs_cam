# Review: Tool + Setup + Workholding Flow

## Summary

The rs_cam project implements tool, setup, and workholding management with comprehensive UI support and project persistence. Tools are stored in a flat per-project library with full editing capabilities. Setups support multi-face stock orientation and datum configuration, with fixtures and keep-out zones for workholding. However, there are critical gaps in validation, particularly around tool deletion orphaning toolpaths, missing UI constraints for critical operations, and hardcoded workholding parameters.

## Findings

### Tool Management

**Data Structure (crates/rs_cam_viz/src/state/job.rs:192-278)**
- `ToolConfig` struct holds complete tool definition: geometry (diameter, cutting length, corner radius for bullnose, included angle for vbit, taper angle for tapered ball), holder/shank dimensions, flute count, material, cut direction
- Five tool types supported: EndMill, BallNose, BullNose, VBit, TaperedBallNose
- Tools serialized with project; no global presets system
- Tool IDs monotonically auto-incremented (JobState::next_tool_id)

**UI Flow (crates/rs_cam_viz/src/ui/)**
- Tool editing: Full properties panel (tool.rs) with type-specific parameter visibility
- Tool library displayed in project tree with Add/Duplicate/Delete context menu (project_tree.rs:85-119)
- Tool selector as combo box in toolpath panel (properties/mod.rs:763-777)

**Tool Deletion Without Validation**
- RemoveTool event (controller/events.rs:75-80) directly removes tool from `job.tools` array
- NO validation checking if tool is referenced by active toolpaths
- Toolpath still contains stale `tool_id` after deletion
- When toolpath generates G-code, it will fail to find tool config (likely panic or silent default)
- Project load detects orphaned tool references and emits `MissingToolReference` warning (project.rs:755-759, 1033-1037) but doesn't auto-reassign

**Missing Features**
- No tool reordering or drag-drop support
- No global tool presets (only per-project tools)
- No "preview tool" geometry visualization before use
- Tool library not collapsible/expandable with summary counts

### Setup Configuration

**Data Structure (crates/rs_cam_viz/src/state/job.rs:379-980)**
- `Setup` struct with FaceUp orientation (6 faces: Top, Bottom, Front, Back, Left, Right)
- `ZRotation` for secondary Z-axis rotation (0/90/180/270 degrees)
- `DatumConfig` supports 4 XY methods (CornerProbe at 4 positions, CenterOfStock, AlignmentPins, Manual) and 4 Z methods (StockTop, MachineTable, FixedOffset, Manual)
- `AlignmentPin` positions for multi-setup registration
- Fixtures and keep-out zones nested in Setup

**Coordinate Transforms**
- Setup::transform_point() (lines 928-945): world -> stock-relative -> FaceUp flip -> ZRotation
- Setup::inverse_transform_point() (lines 955-974): reverses the chain correctly
- Effective stock dims properly calculated per orientation (lines 948-951, 490-496)
- Transform functions for meshes, heightmaps, and polygons provided (lines 983-1052)

**Validation**
- Minimum 1 setup enforced: RemoveSetup only fires if `setups.len() > 1` (controller/events.rs:90)
- NO UI feedback shown when attempting delete on last setup (button simply doesn't respond)
- Setup orientation UI accessible in properties panel (properties/setup.rs:28-63)

### Workholding (Fixtures & Keep-Out Zones)

**Fixture Structure (crates/rs_cam_viz/src/state/job.rs:778-846)**
- `Fixture` stores position (origin_x/y/z), size (size_x/y/z), clearance margin
- `FixtureKind` enum: Clamp, Vise, VacuumPod, Custom (visual/semantic only, no behavior difference)
- `bbox()` returns physical bounding box; `clearance_bbox()` inflates by clearance margin for tool avoidance
- `footprint()` generates 2D polygon for boundary subtraction

**Keep-Out Zones (crates/rs_cam_viz/src/state/job.rs:848-896)**
- Rectangular XY-only regions with full Z extent (origin_x/y, size_x/y)
- `bbox()` computed from stock Z bounds
- `footprint()` generates 2D polygon for boundary subtraction

**Visualization (crates/rs_cam_viz/src/render/fixture_render.rs)**
- Fixtures and keep-out zones rendered as wireframe boxes
- Each box = 24 vertices (12 edges as line list)
- GPU buffer created per render frame (no persistence optimization)
- Extra lines (alignment pins) supported via `from_boxes_and_lines()`

**Hardcoded Workholding Rigidity**
- SetupContext::workholding_rigidity hardcoded to `Medium` (properties/mod.rs:613)
- NO UI control for rigidity (Soft/Medium/Hard/VeryHard per rs_cam_core::feeds::WorkholdingRigidity)
- Feeds calculation always assumes Medium rigidity, affecting DOC/feedrate recommendations
- User cannot tune for flexible vs rigid fixtures

**Limitations**
- `FixtureKind` is metadata only; no behavioral difference (Clamp vs Vise never used)
- No "clamp mode" vs "floating" distinction (not implemented)
- No fixture collision detection with toolpath (only passive visualization)
- No automatic fixture avoidance in tool motion generation

### Persistence & Serialization

**Project Format (crates/rs_cam_viz/src/io/project.rs)**
- Version 2 format with tools, models, setups, toolpaths as top-level sections
- Setup section contains nested fixtures and keep-out zones (ProjectSetupSection:235)
- Toolpath section stores tool_id and model_id by reference
- ToolConfig fully serialized (name, type, all dimensions, material, cut_direction, vendor info)
- Alignment pins stored as array of (x, y, diameter)

**Load-Time Validation**
- Missing tool reference detected and warned (lines 755-759, 1033-1037)
- Missing model reference detected and warned (lines 761-765, 1039-1043)
- Tool ID fallback: uses first tool if toolpath's tool_id invalid (line 1000)
- Model ID fallback: uses first model if toolpath's model_id invalid (line 1004)
- NO repair: orphaned toolpaths silently keep invalid IDs; user must manually reassign in UI

**Tool Library Persistence**
- Tools saved with project, not in global library
- Duplicate tool creates new ToolId; old tool data remains (DuplicateTool event, controller/events.rs:64-74)
- No "import from file" or "export tool library" features

## Issues Found

| # | Severity | Description | Location |
|---|----------|-------------|----------|
| 1 | High | Deleting a tool that is referenced by toolpaths leaves orphaned tool_id; no validation or cascading removal | controller/events.rs:75-80 |
| 2 | Medium | Last setup deletion: button appears clickable but RemoveSetup is silently ignored if only 1 setup; no UI feedback | project_tree.rs:155-158, controller/events.rs:89-106 |
| 3 | Medium | Workholding rigidity hardcoded to Medium; no UI control to adjust for fixture flexibility | properties/mod.rs:613 |
| 4 | Medium | Fixture clearance bounding boxes calculated but not integrated into collision detection during toolpath generation | state/job.rs:827-845 |
| 5 | Medium | Keep-out zone footprints not confirmed to be enforced during operation boundary calculation; may be visualization-only | state/job.rs:848-896 |
| 6 | Medium | Multi-setup coordinate transforms lack integration tests; 24 orientation combinations not systematically validated | state/job.rs:928-945 |
| 7 | Low | Project load warns on missing tool references but no UI guidance for reassignment; toolpath defaults to ToolId(0) if no tools exist | project.rs:998-1005, 1033-1037 |

## Test Gaps

- No tests for tool deletion validation (orphaned references)
- No tests for coordinate transform round-trips (all 6 faces x 4 rotations = 24 combos)
- No tests for setup boundary calculations with setups containing fixtures
- No tests for fixture clearance polygon generation
- No tests for loading project with missing tool references
- No tests for deleting last setup (should be blocked)
- No tests for multi-setup alignment pin registration

## Suggestions

1. **Add tool-in-use validation** before deletion: scan all toolpaths, warn user if tool is referenced, offer to cascade-delete dependent toolpaths or auto-reassign to first available tool
2. **Disable or hide delete-setup button** when `setups.len() <= 1`; add inline message "At least one setup required"
3. **Expose workholding rigidity as setup property**: add Rigidity dropdown in Setup properties, pass to feeds calculator via SetupContext
4. **Integrate fixture clearance into collision detection**: when building stock mesh, subtract clearance_bbox() polygons; test that tool avoids fixtures
5. **Verify keep-out zones are active**: search compute/worker code for uses of `setup.keep_out_zones`; if none found, document as visualization-only or implement subtraction from machining boundary
6. **Add test suite for coordinate transforms**: parametrized test for all FaceUp/ZRotation combinations; verify transform . inverse_transform = identity
7. **Improve project-load UX**: when toolpath references missing tool, highlight in project tree (red text or warning icon); auto-open properties panel to allow reassignment
8. **Document clamp mode not yet implemented**: add note to FEATURE_CATALOG.md that Clamped vs Floating distinction is not yet wired (FixtureKind is enum but unused)
