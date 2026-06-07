surface: toolpath-tab-suggest-all
file: crates/rs_cam_viz/src/ui/properties/mod.rs
kind: inline-widget
job: One button in the Toolpath tab that bulk-overwrites feed, plunge, depth-per-pass, stepover, and RPM from the LUT without switching to the Feeds tab.
opens-from: rendered inline at top of the Toolpath tab params (properties/mod.rs:2808-2882, "⚡ Suggest all (LUT)").
controls:
- apply-all-feeds (⚡ Suggest all (LUT) → apply_feeds_result_to_op; writes feed/plunge/doc/stepover/rpm)
- request-feeds-suggestion (hover + preview line "→ feed.., plunge.., DOC.., WOC..")
reads-state: operation; tool_cfg; material; machine; workholding; spindle_strategy; computes/caches entry.feeds_result
writes-state: operation.feed_rate, plunge_rate, depth_per_pass, stepover, spindle_rpm (direct mutation, sets stale)
confusable-with: feeds-card "⚡ Suggest all" (identical capability, same apply_feeds_result_to_op call, different tab), feeds-modal-toolpath "⚡ Apply all"
recommendation-sources-touched: vendor-lut
health: red — third "apply all LUT feeds" button for one toolpath (also in feeds-card and the modal). Comment at properties/mod.rs:2808 explicitly says it "Mirrors the same-named button in the Feeds tab" — acknowledged duplication (P1/P6).
