surface: toolpath-tab-feed-params
file: crates/rs_cam_viz/src/ui/properties/operations/mod.rs
kind: inline-widget
job: Be the authoritative editable inputs for a toolpath's feed rate, plunge rate, spindle-RPM override, stepover, and depth-per-pass, each with an inline ⚡ Suggest pill carrying the LUT recommendation.
opens-from: rendered inside the "Toolpath" properties tab per operation (draw_feed_params + draw_spindle_rpm_row + per-op dv_pill calls in operations/boundary_2d.rs, surface_3d.rs, finishing.rs, drill.rs, engrave.rs, etc.).
controls:
- set-feed-rate (DragValue, authoritative; ⚡ pill → round_suggestion_value)
- set-plunge-rate (DragValue, authoritative; ⚡ pill)
- set-spindle-rpm (override checkbox + DragValue, authoritative; ⚡ pill enables override + writes LUT rpm)
- set-stepover (dv_pill, authoritative; ⚡ pill)
- set-doc (dv_pill on depth-per-pass / max-depth, authoritative; ⚡ pill)
- request-feeds-suggestion (the pills' source = FeedsResult.chipload_source, colour-coded green=vendor-lut amber=formula)
reads-state: operation.feed_rate / plunge_rate / spindle_rpm / stepover / depth_per_pass; entry.feeds_result (for pill values + source colour)
writes-state: operation.feed_rate, plunge_rate, spindle_rpm, stepover, depth_per_pass (direct mutation)
confusable-with: feeds-card and feeds-modal-toolpath (both expose Suggest/Apply for the EXACT same five fields with the same FeedsResult); the per-field pill (⚡) looks identical to the feeds-card per-field "⚡ Suggest" button and the toolpath-tab "⚡ Suggest all (LUT)" but acts on one field
recommendation-sources-touched: suggest-icon, vendor-lut
health: yellow — correctly the source of truth (P1 winner for these fields), but the same ⚡ glyph is overloaded across single-field pill, card Suggest, Suggest-all, and modal Apply, making roles hard to tell apart (P4/P7).
