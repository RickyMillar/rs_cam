surface: menu-bar
file: crates/rs_cam_viz/src/ui/menu_bar.rs
kind: menu
job: Provide the top-level application command menus (File/Edit/Toolpath/Tools/Workspace/Simulation/View/Help) and register global keyboard shortcuts.
opens-from: always visible (TopBottomPanel "menu_bar"); also handles raw key input each frame
controls:
  - import-stl
  - import-svg
  - import-dxf
  - import-step
  - open-job
  - save-job
  - export-gcode-wizard
  - export-gcode-direct
  - export-combined-gcode
  - export-setup-gcode
  - export-setup-sheet
  - export-svg-preview
  - quit
  - undo
  - redo
  - delete-selected
  - generate-all
  - optimize-project
  - open-tool-library
  - switch-workspace
  - run-simulation
  - reset-simulation
  - run-collision-check
  - reset-view
  - set-view-preset
  - show-shortcuts
reads-state: state.session.list_setups(), state.simulation.has_results(), state.is_optimizing, ctx.input modifiers/keys
writes-state: none directly (emits AppEvent); keyboard handler pushes Undo/Redo/SaveJob/OpenExportWizard/ExportGcode/OpenJob
confusable-with: workspace-bar (Workspace menu duplicates the workspace tabs); menu-file-direct-export (two export paths); project-tree (Tool Library opener, import buttons, setup export all duplicated)
recommendation-sources-touched: none
health: yellow — "Delete Selected" is hard-disabled (add_enabled(false)) i.e. an orphan control (P6); the Workspace submenu fully duplicates the workspace-bar tabs (P1/P2); two parallel export entry points (wizard vs direct) live side by side (P1/P4).
