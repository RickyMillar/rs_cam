surface: Feeds & Speeds Modal (launcher reference)
file: crates/rs_cam_viz/src/ui/properties/mod.rs (button ~3017-3028) → modal implemented in crates/rs_cam_viz/src/ui/feeds_modal.rs (OTHER cluster)
kind: modal
job: Open the redesigned full-screen Feeds & Speeds view (current-vs-recommended comparison, machinist charts, Apply buttons) — a third editor of the same feed/plunge/rpm/doc/woc values.
opens-from: "📊 Open Feeds & Speeds modal" button in the Feeds Tab (emits AppEvent::OpenFeedsModal)
controls:
  - open-feeds-modal
  - request-feeds-suggestion-all (inside the modal — owned by feeds_modal.rs, not this cluster)
reads-state: (modal owns its own reads; launcher only emits the event with entry.id)
writes-state: (modal applies feeds to the operation — same fields the Params/Feeds tabs write)
confusable-with: Feeds Tab and Params Tab (all three edit the same feeds/speeds operation fields)
recommendation-sources-touched: vendor-lut, sim-feedback (per modal; out of this cluster's source scope)
health: red — code comment states it "stays alongside the legacy feeds card... until the modal is fully promoted (Phase 4)", i.e. a knowingly-transitional THIRD home for feeds/speeds editing (P1, P6). Full audit belongs to the feeds_modal.rs cluster; flagged here as a cross-surface duplication of this cluster's feeds capabilities.
