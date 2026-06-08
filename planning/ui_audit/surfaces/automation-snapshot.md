surface: automation-snapshot
file: crates/rs_cam_viz/src/ui/automation.rs
kind: inline-widget
job: Non-visible infrastructure — record widget label/rect/enabled into a per-frame egui temp snapshot so external automation/tests can locate controls.
opens-from: not user-facing; populated by begin_frame() + automation::record() calls (e.g. status-bar lane chips)
controls:
  - record-widget-state
reads-state: egui ctx temp data (UiAutomationSnapshot)
writes-state: egui ctx temp data only (no AppState)
confusable-with: none
recommendation-sources-touched: none
health: green — single-purpose test/automation harness, not a real UI surface; included for completeness. Note: only a handful of widgets (status-bar lanes) actually call record(), so coverage is partial but that is by design.
