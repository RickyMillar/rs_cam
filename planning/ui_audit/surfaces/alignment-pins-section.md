surface: alignment-pins-section
file: crates/rs_cam_viz/src/ui/properties/stock.rs
kind: inline-widget
job: Configure two-sided flip axis and the stock's alignment pin positions/diameter (shared across setups).
opens-from: "Alignment Pins" collapsing header inside stock-properties-panel (default open).
controls:
  - setup-two-sided
  - set-flip-axis
  - set-pin-diameter
  - set-pin-position
  - mirror-pin
  - remove-pin
  - add-pin
  - auto-place-pins
  - set-auto-place-pin-count
  - view-pin-symmetry-warning (read-only)
  - view-pin-bounds-warning (read-only)
reads-state: stock.flip_axis, stock.alignment_pins (x, y, diameter), stock.x, stock.y, stock.padding, has_flipped_setup
writes-state: stock.flip_axis, stock.alignment_pins[*].x/y/diameter (add/remove/mirror/auto-place); emits SetupTwoSided / StockChanged
confusable-with: setup-properties-panel (its "Add alignment pins for this flip" / two-sided button and pin-count read-out)
recommendation-sources-touched: none
health: yellow — feature-rich single concern (pin layout), but the two-sided/flip workflow is split: SetupTwoSided is also triggered from setup-properties-panel, and pin presence is reported there while edited here (P1/P2). Dense flat list with two validation warnings rendered as text (P5).
