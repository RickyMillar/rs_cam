surface: Engagement Diagram
file: crates/rs_cam_viz/src/ui/properties/mod.rs (draw_engagement_diagram ~1549)
kind: inline-widget
job: Visualize the recommended radial WOC / axial DOC engagement against the tool cross-section.
opens-from: Rendered inside the Feeds Tab below the formula breakdown when a feeds_result exists
controls: (none — read-only visualization)
reads-state: entry.feeds_result (radial_width_mm, axial_depth_mm), tool_diameter, tool_type
writes-state: none
confusable-with: Params-tab stepover/spiral pattern diagrams (also illustrate WOC/stepover)
recommendation-sources-touched: vendor-lut (visualizes the LUT-derived WOC/DOC)
health: green — clean read-only widget; mild overlap with the Params pattern diagrams that also depict stepover.
