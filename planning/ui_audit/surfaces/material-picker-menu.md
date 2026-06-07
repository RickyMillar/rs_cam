surface: material-picker-menu
file: crates/rs_cam_viz/src/ui/properties/stock.rs
kind: menu
job: Pick the stock material via a hierarchical Category -> (Wood -> Softwood/Hardwood) -> species drilldown with per-leaf filtering.
opens-from: Clicking the "Material: <name> ▼" menu button in stock-properties-panel.
controls:
  - set-stock-material
  - filter-wood-species
reads-state: stock.material (label, category, janka_lbf), Material::materials_by_category()
writes-state: stock.material; returns changed -> caller emits StockMaterialChanged
confusable-with: none
recommendation-sources-touched: none
health: green — exemplary summary->detail drilldown with search filter and Janka annotations; one concern (material selection), well structured (replaced a flat 33-entry combo).
