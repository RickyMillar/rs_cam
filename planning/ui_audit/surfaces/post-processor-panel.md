surface: post-processor-panel
file: crates/rs_cam_viz/src/ui/properties/post.rs
kind: panel
job: Configure G-code output format, spindle speed, safe-Z clearance, and safe-rapids substitution.
opens-from: Selecting Post / output settings (right properties dock).
controls:
  - set-post-format
  - set-spindle-rpm
  - set-safe-z
  - view-effective-safe-z (read-only)
  - toggle-safe-rapids
  - set-high-feedrate
reads-state: post (format, spindle_speed, safe_z, high_feedrate_mode, high_feedrate), stock_top_z, effective_safe_z()
writes-state: post.format, post.spindle_speed, post.safe_z, post.high_feedrate_mode, post.high_feedrate
confusable-with: tool-properties-panel (a user expecting spindle RPM next to the tool finds it here instead)
recommendation-sources-touched: none
health: red — spindle-speed (a feeds/speeds value) and high-feedrate live in the Post panel, divorced from the tool and from per-operation feeds (P2 broken grouping); Safe Z requires a prose warning to explain the compute-time clamp instead of an affordance that shows/edits the effective value (P5); "Safe Z" vs per-operation "Retract Z" disambiguated only by tooltip text (P4/P5).
