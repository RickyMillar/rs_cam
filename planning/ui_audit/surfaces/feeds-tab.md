surface: Feeds Tab
file: crates/rs_cam_viz/src/ui/properties/mod.rs (ToolpathTab::Feeds ~3006-3101, draw_feeds_card ~1120, calculate_and_apply_feeds ~1083)
kind: tab
job: Show the computed feeds/speeds recipe (RPM, chipload, feed, plunge, DOC, WOC, power, MRR) with per-row Apply/Suggest buttons, a formula breakdown, an engagement diagram, and a vendor LUT table.
opens-from: Feeds tab in the Toolpath Tab Bar
controls:
  - open-feeds-modal
  - request-feeds-suggestion-feed-rate
  - request-feeds-suggestion-plunge-rate
  - request-feeds-suggestion-doc
  - request-feeds-suggestion-stepover
  - request-feeds-suggestion-all
  - view-vendor-lut
reads-state: entry.feeds_result (computed each frame via feeds_result_for_operation), tool_configs (diameter/type/flute_count), material, machine, workholding, spindle_strategy
writes-state: entry.operation.set_feed_rate / set_plunge_rate / set_depth_per_pass / set_stepover via per-row Suggest buttons; apply_feeds_result_to_op via "Suggest all"; entry.feeds_result cache; entry.stale_since
confusable-with: Params Tab (same feed/plunge/doc/stepover edits via inline pills); Feeds Modal (the "Open Feeds & Speeds modal" button leads to a third, redesigned editor of the same values — explicitly kept "alongside the legacy feeds card" per code comment)
recommendation-sources-touched: vendor-lut (ChiploadSource shown as "Source: {observation_id}" / formula / edge-radius-floor; vendor LUT table viewer), suggest-icon (⚡ per-row buttons)
health: red — duplicates every feed/speed write the Params tab already offers AND links out to a third modal editor the code admits is a transitional duplicate (P1, P6 dead-duplicate); read-out values vs editable Apply buttons sit in the same grid (P4 distinct-roles), and three representations of one recipe (card + prose formula + engagement diagram) compete (P3/P5).
