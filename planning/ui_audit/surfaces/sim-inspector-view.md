surface: Inspector › View
file: crates/rs_cam_viz/src/ui/sim_diagnostics.rs
kind: panel
job: Point at the viewport Overlays panel, which owns every display control this section used to hold.
opens-from: The foot of the Inspector right panel, below the Project / Toolpath / Span sections
controls:
  - display-overlays-pointer
reads-state: none
writes-state: none
confusable-with: none
recommendation-sources-touched: none
health: green — P6 (2026-09-08) moved the stock opacity slider, the Solid / Deviation / By Height stock colour modes and the two generator-step toggles into the Overlays panel as registry rows, so the P1 duplicate-write finding against the viewport `Show ▼` menu is closed on both sides. Two defects went with the section. The W4.3 comment claimed the stock show/hide toggle "lives in the viewport Show ▼ menu (one home for visibility)" — it did not, and `show_sim_mesh` read the workspace and `has_results()` only, so nothing could hide the simulated stock; it has its own row now. And `Show generator steps` was HIDDEN until a trace existed, which is the opposite of the Deviation precedent ten lines above it; it is now always listed, disabled, with "switch on Record generator trace and regenerate" plus the button.
