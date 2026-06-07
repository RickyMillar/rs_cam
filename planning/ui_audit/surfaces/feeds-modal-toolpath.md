surface: feeds-modal-toolpath
file: crates/rs_cam_viz/src/ui/feeds_modal.rs
kind: modal
job: Show one toolpath's current-vs-recommended feeds/speeds with provenance, derate breakdown, and three machinist charts, and let the user apply recommended values.
opens-from: "📊 Open Feeds & Speeds modal" button in the Feeds tab (properties/mod.rs:3019); AppEvent::OpenFeedsModal(toolpath_id). Also reachable by toggling the "This toolpath" header tab inside the modal.
controls:
- request-feeds-suggestion (vendor-lut, read-only recommendation column)
- set-spindle-rpm (Apply RPM row → ApplyFeedsField{Rpm})
- set-feed-rate (Apply Feed row → ApplyFeedsField{Feed})
- set-plunge-rate (Apply Plunge row → ApplyFeedsField{Plunge})
- set-doc (Apply DOC row → ApplyFeedsField{Doc})
- set-stepover (WOC Apply row → ApplyFeedsField{Woc})
- apply-all-feeds (⚡ Apply all → ApplyFeedsAll)
- set-scallop-height (DropCutter only → SetDropCutterScallopHeight)
- set-spindle-strategy (Match chart / Max speed radios → SetSpindleStrategy)
- toggle-feeds-provenance (How is this calculated? → ToggleFeedsProvenance)
reads-state: operation.feed_rate / plunge_rate / spindle_rpm / depth_per_pass / stepover / scallop_height; tool.flute_count; stock.material; stock.workholding_rigidity; machine (rpm_range, power); post_config.spindle_strategy; FeedsExplain (re-derived each frame); modal.show_provenance
writes-state: (via events) operation.feed_rate, plunge_rate, spindle_rpm, depth_per_pass, stepover, scallop_height; post_config.spindle_strategy; modal.show_provenance
confusable-with: feeds-card (legacy collapsible in Feeds tab showing the same RPM/feed/plunge/DOC/WOC recommendation + Suggest buttons), toolpath-tab-feed-params (the authoritative numeric inputs for the same fields)
recommendation-sources-touched: vendor-lut
health: yellow — does many jobs (comparison table + scallop control + power/MRR + provenance disclosure + chipload-math breakdown + rationale + spindle policy + 3 charts) in one window; heavy summary→detail nesting helps (P3) but it overlaps the legacy feeds-card almost field-for-field (P1/P6).
