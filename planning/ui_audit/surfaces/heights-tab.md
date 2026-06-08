surface: Heights Tab
file: crates/rs_cam_viz/src/ui/properties/mod.rs (ToolpathTab::Heights ~3103-3109) + operations/mod.rs draw_heights_params ~232, draw_height_diagram ~1431
kind: tab
job: Edit the five Z planes (clearance, retract, feed, top, bottom) as reference+offset rows, plus an interactive draggable side-view diagram.
opens-from: Heights tab in the Toolpath Tab Bar
controls:
  - set-clearance-height
  - set-retract-height
  - set-feed-height
  - set-top-height
  - set-bottom-height
reads-state: entry.heights (HeightsConfig), height_ctx (stock_top_z/bottom_z, model_top/bottom_z, safe_z, op_depth)
writes-state: entry.heights.clearance_z / retract_z / feed_z / top_z / bottom_z (HeightMode), written by BOTH the grid rows and the diagram drag
confusable-with: none (Z-step fields like waterline z_step live in Params, conceptually adjacent)
recommendation-sources-touched: none (auto-promote of Auto/Manual modes to FromReference uses computed defaults, not a recommendation engine)
health: yellow — the five heights have two co-equal editors (grid rows + draggable diagram) writing the same fields (P1 within one tab); otherwise well-grouped and the diagram is a strong P5 affordance.
