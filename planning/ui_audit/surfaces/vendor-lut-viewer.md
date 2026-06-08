surface: vendor-lut-viewer
file: crates/rs_cam_viz/src/ui/properties/mod.rs
kind: panel
job: Show the raw embedded vendor cutting-data rows (the LUT) for the current tool family/diameter as a read-only reference table.
opens-from: "Vendor Cutting Data" collapsing header at the bottom of the Feeds tab (draw_vendor_lut_viewer, properties/mod.rs:1381 / called 3100). Default-collapsed.
controls:
- view-vendor-lut (read-only-display of embedded_vendor_lut observations filtered by tool family)
reads-state: rs_cam_core::feeds::embedded_vendor_lut(); tool_type; tool_diameter
writes-state: none
confusable-with: feeds-modal-toolpath Chart A/B (which plot the SAME vendor rows as min/max curves against diameter and hardness) and the provenance disclosure (which names the matched row)
recommendation-sources-touched: vendor-lut
health: green — single clear job (read-only provenance reference); only mild redundancy with the modal's Charts A/B which visualise the same rows.
