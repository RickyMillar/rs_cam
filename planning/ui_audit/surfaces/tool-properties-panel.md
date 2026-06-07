surface: tool-properties-panel
file: crates/rs_cam_viz/src/ui/properties/tool.rs
kind: panel
job: Edit the selected project tool's geometry, material, cut direction, holder/shank, preview it, and save it to a library catalog.
opens-from: Selecting a tool in the project tool list (right properties dock).
controls:
  - set-tool-name
  - set-tool-type
  - set-tool-diameter
  - set-tool-cutting-length
  - set-tool-flutes
  - set-tool-helix
  - set-tool-corner-radius
  - set-tool-material
  - set-cut-direction
  - set-vbit-included-angle
  - set-taper-half-angle
  - set-shaft-diameter
  - set-holder-diameter
  - set-shank-diameter
  - set-shank-length
  - set-stickout
  - view-tool-preview (read-only)
  - save-tool-to-library
  - view-holder-not-configured-warning (read-only)
reads-state: tool (name, tool_type, diameter, cutting_length, flute_count, helix_deg, corner_radius_mm, corner_radius, tool_material, cut_direction, included_angle, taper_half_angle, shaft_diameter, holder_diameter, shank_diameter, shank_length, stickout), egui temp (catalog name, save status)
writes-state: tool.* (all fields above, mutated directly via &mut ToolConfig); calls tool_library::append_tool for save
confusable-with: tool-library-modal edit form (shares draw_tool_fields, so the in-place tool editor and the library editor are pixel-identical)
recommendation-sources-touched: none
health: yellow — single tool-editing job, but no feeds/speeds at all live here (chipload/rpm/feed-rate are authored elsewhere) while spindle RPM lives in the Post panel — feeds&speeds concern is scattered (P2). draw_tool_fields is shared with the library modal (good reuse) yet makes the two surfaces confusable (P4); two distinct "Corner Radius" fields exist (corner_radius_mm for EndMill vs corner_radius for BullNose) under the same label (P4).
