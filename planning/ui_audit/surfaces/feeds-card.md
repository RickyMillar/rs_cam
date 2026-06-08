surface: feeds-card
file: crates/rs_cam_viz/src/ui/properties/mod.rs
kind: panel
job: Show the cached LUT feeds result (RPM, chipload, feed, plunge, DOC, WOC, power, MRR) inside the Feeds tab with a per-field "⚡ Suggest" button and a "Suggest all" button.
opens-from: "Feeds & Speeds" collapsing header inside the Feeds tab (draw_feeds_card via calculate_and_apply_feeds, properties/mod.rs:1128).
controls:
- request-feeds-suggestion (vendor-lut, read-only recommended values + Source line)
- set-feed-rate (⚡ Suggest → operation.set_feed_rate)
- set-plunge-rate (⚡ Suggest → operation.set_plunge_rate)
- set-doc (⚡ Suggest → operation.set_depth_per_pass)
- set-stepover (⚡ Suggest → operation.set_stepover)
- apply-all-feeds (⚡ Suggest all → apply_feeds_result_to_op)
reads-state: entry.feeds_result (cached FeedsResult); operation; tool; machine; material
writes-state: entry.operation.feed_rate / plunge_rate / depth_per_pass / stepover (direct mutation, sets entry.stale_since)
confusable-with: feeds-modal-toolpath (same recommendation, same five Apply/Suggest fields, same power bar + MRR + source line, but in a modal), toolpath-tab-feed-params (the actual editable inputs for these fields with the SAME inline pills)
recommendation-sources-touched: vendor-lut
health: red — direct functional duplicate of feeds-modal-toolpath's comparison card; comment at properties/mod.rs:3013 admits it is kept "alongside the legacy feeds card... until the modal is fully promoted." Two homes for the identical Suggest-this-field capability (P1/P6).
