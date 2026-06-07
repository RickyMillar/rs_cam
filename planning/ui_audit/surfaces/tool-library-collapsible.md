surface: Tool Library (collapsible)
file: crates/rs_cam_viz/src/ui/toolpath_panel.rs
kind: panel
job: List defined tools and add new tools by type.
opens-from: CollapsingHeader "Tool Library" at the bottom of the Operations Queue Panel
controls:
  - select-tool
  - add-tool
reads-state: session.tools, selection
writes-state: emits Select(Tool) / AddTool events
confusable-with: none
recommendation-sources-touched: none
health: green — small, single-purpose, collapsed by default (good summary->detail).
