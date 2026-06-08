surface: project-tree
file: crates/rs_cam_viz/src/ui/project_tree.rs
kind: panel
job: Present the project hierarchy (stock, post, machine, models, tool library, setups with workholding + toolpaths) as the primary selection/navigation tree, with per-item context-menu actions.
opens-from: always visible left-side panel
controls:
  - select-stock
  - select-post-processor
  - select-machine
  - select-model
  - reload-model
  - remove-model
  - open-tool-library
  - select-tool
  - add-tool
  - add-tool-from-library
  - duplicate-tool
  - remove-tool
  - select-setup
  - add-setup
  - remove-setup
  - export-setup-gcode
  - select-fixture
  - remove-fixture
  - select-keep-out
  - remove-keep-out
  - select-toolpath
  - add-toolpath
  - toggle-toolpath-visibility
  - toggle-toolpath-enabled
  - duplicate-toolpath
  - move-toolpath-up
  - move-toolpath-down
  - remove-toolpath
  - inspect-toolpath-in-simulation
  - import-stl
  - import-svg
  - import-dxf
  - import-step
reads-state: state.session (name, stock_config, machine, models, tools, list_setups, get_toolpath_config), state.gui.post.format, state.selection, state.gui.toolpath_rt (status/visible/result)
writes-state: none (emits Select* / Add* / Remove* / Toggle* / Move* / Export* / Import* / Open* / Reload* / Duplicate* events)
confusable-with: menu-bar (import buttons, Tool Library opener, setup export, Add Toolpath/Tool all also reachable from menus); the per-toolpath status dot vs. the color swatch are two different visual codes side by side
recommendation-sources-touched: sim-feedback (toolpath status dot + trace badge reflect compute/sim state)
health: yellow — dense but legitimately the navigation hub; smells: import + tool-library + per-setup-export duplicate the menu bar (P1/P2); the row packs a palette swatch, a status dot, an enabled/visible dim, an index, a name and a trace badge — distinct codes that look alike (P4); Add Tool / Add Toolpath / Add Setup / import are scattered add-affordances at different nesting depths (P2).
