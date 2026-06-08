surface: Operations Queue Panel
file: crates/rs_cam_viz/src/ui/toolpath_panel.rs
kind: panel
job: List the per-setup operation queue with status/visibility and let the user add, reorder, generate, and select toolpaths.
opens-from: Left panel of the Toolpath workspace (always present)
controls:
  - generate-all-toolpaths
  - add-toolpath
  - add-tool
  - select-toolpath
  - reorder-toolpath
  - move-toolpath-to-setup
  - generate-toolpath
  - inspect-toolpath-in-simulation
  - toggle-toolpath-visibility
  - toggle-toolpath-enabled
  - toggle-isolate-toolpath
  - toggle-cut-move-visibility
  - toggle-rapid-move-visibility
  - duplicate-toolpath
  - delete-toolpath
reads-state: session.list_setups, session.get_toolpath_config (id/name/enabled/tool_id/operation), gui.toolpath_rt (visible/auto_regen/status/result/stats), viewport.isolate_toolpath, viewport.toolpath_move_visibility, selection, session.tools, gui.mcp_highlights
writes-state: emits AppEvents only (no direct field writes); viewport.toolpath_move_visibility + viewport.isolate_toolpath via toolpath_row_controls
confusable-with: Simulation workspace op list (shares toolpath_row_controls eye/C/R/isolate row)
recommendation-sources-touched: none
health: yellow — single clear job (queue management) but each card stacks 4 rows (status chips, tool+badges, stats, eye/C/R/isolate controls) plus a context menu that fully duplicates the inline quick actions (Generate, Inspect, Isolate, Hide/Show); summary->detail is shallow (P3) and inline-vs-context duplication risks confusion (P1).
