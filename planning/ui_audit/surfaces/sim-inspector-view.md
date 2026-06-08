surface: Inspector › View
file: crates/rs_cam_viz/src/ui/sim_diagnostics.rs
kind: panel
job: Control how the simulation looks in the 3D viewport (stock/path visibility, stock color mode, generator-step overlay).
opens-from: CollapsingHeader "View" inside the Inspector right panel (default open)
controls:
  - show-stock
  - stock-opacity
  - show-cutting-moves
  - show-rapid-moves
  - stock-color-mode
  - show-generator-steps
  - highlight-active-generator-step
reads-state: viewport.show_stock, viewport.show_cutting, viewport.show_rapids, sim.stock_opacity, sim.stock_viz_mode, sim.playback.display_deviations, sim.debug.enabled, sim.debug.highlight_active_item, gui.toolpath_rt[].{debug_trace,semantic_trace}
writes-state: viewport.show_stock, viewport.show_cutting, viewport.show_rapids, sim.stock_opacity, sim.stock_viz_mode, sim.debug.enabled, sim.debug.highlight_active_item; emits SimVizModeChanged
confusable-with: viewport-overlay Show ▼ menu (also toggles show_stock/show_cutting/show_rapids — SAME fields, two homes), sim-setup-and-run (comment explicitly contrasts "display toggles here vs recording toggles there")
recommendation-sources-touched: none
health: red — show_stock, show_cutting, show_rapids are each written here AND in the viewport Show ▼ menu (P1 violation: one concern, two homes writing identical viewport fields); StockVizMode::ByOperation is silently rendered as "Solid" (dead enum branch).
