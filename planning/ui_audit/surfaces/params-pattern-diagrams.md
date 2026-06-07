surface: Params Pattern Diagrams
file: crates/rs_cam_viz/src/ui/properties/operations/mod.rs (draw_stepover_diagram, draw_spiral_diagram, draw_radial_diagram, draw_outline_diagram, draw_point_set_diagram, draw_pencil_diagram, draw_steep_shallow_diagram, draw_ramp_finish_diagram, draw_inlay_diagram, draw_tab_diagram, draw_dogbone_diagram, draw_lead_in_out_diagram)
kind: inline-widget
job: Render a top-down/side-view minimap of the active operation's cut pattern (zigzag/contour/spiral/radial/outline/points/pencil/steep-shallow/ramp/inlay) driven by the current param values.
opens-from: Rendered at the bottom of the Params Tab after the per-op param grid; tab/dogbone/lead-in diagrams render inside their dressup/profile sub-sections
controls: (none — read-only visualizations parameterized by op fields)
reads-state: entry.operation fields (stepover, angle, z_step, scallop_height, num_offset_passes, offset_stepover, threshold_angle, max_stepdown, pocket_depth/glue_gap/flat_depth, tab_count/width/height, dogbone_angle, lead_radius, profile side, trace compensation)
writes-state: none
confusable-with: Engagement Diagram (Feeds tab) which also depicts stepover/WOC
recommendation-sources-touched: none
health: green — read-only, single illustrative purpose, value-reactive (strong P5); just note the engagement diagram overlaps conceptually across tabs.
