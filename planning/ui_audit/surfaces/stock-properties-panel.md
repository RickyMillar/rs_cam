surface: stock-properties-panel
file: crates/rs_cam_viz/src/ui/properties/stock.rs
kind: panel
job: Edit stock material, dimensions, origin, auto-from-model behaviour, and alignment pins.
opens-from: Selecting "Edit stock dimensions" / Stock in setup-list-panel (right properties dock).
controls:
  - set-stock-material
  - view-material-hardness-index (read-only)
  - view-material-kc (read-only)
  - set-stock-dimensions
  - set-stock-origin
  - toggle-auto-from-model
  - set-stock-padding
  - (embeds alignment-pins-section controls)
reads-state: stock (material, x, y, z, origin_x/y/z, auto_from_model, padding, flip_axis, alignment_pins), has_flipped_setup
writes-state: stock.material, stock.x/y/z, stock.origin_x/y/z, stock.auto_from_model, stock.padding (+ flip_axis & alignment_pins via embedded section); emits StockChanged / StockMaterialChanged
confusable-with: setup-list-panel stock summary card (shows effective dims, read-only)
recommendation-sources-touched: none
health: yellow — coherent stock home, but the material hardness/Kc read-outs are prose-style explanatory rows (P5), effective-dims live in a different surface (P1), and it hosts both raw stock geometry and the large alignment-pins workflow in one flat scroll (P2/P3).
