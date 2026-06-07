surface: workspace-bar
file: crates/rs_cam_viz/src/ui/workspace_bar.rs
kind: tab
job: Switch between the three workspaces (Setup / Toolpaths / Simulation) and surface a readiness/pending/collision badge per workspace.
opens-from: always visible, drawn below the menu bar
controls:
  - switch-workspace
reads-state: state.workspace, state.session.toolpath_configs(), state.gui.toolpath_rt, state.gui.edit_counter, state.simulation.has_results(), state.simulation.is_stale(), state.simulation.checks.holder_collision_count, state.simulation.checks.rapid_collisions
writes-state: none (emits SwitchWorkspace)
confusable-with: menu-bar Workspace menu (identical switch-workspace capability)
recommendation-sources-touched: sim-feedback (badges derive stale/collision state from simulation results)
health: yellow — clean single-purpose switcher, but switch-workspace is duplicated in the menu bar (P1); the per-tab badges encode three different concerns (compute-pending, sim-stale, collisions) in tiny colored text, leaning on prose/color instead of a clearer affordance (P3/P5).
