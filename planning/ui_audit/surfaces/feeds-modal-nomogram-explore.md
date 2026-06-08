surface: feeds-modal-nomogram-explore
file: crates/rs_cam_viz/src/ui/feeds_modal.rs
kind: inline-widget
job: Let the user drag/slide a point on the feed-vs-RPM nomogram (Chart C) to explore a what-if operating point, see the resulting chipload verdict and power, then apply it.
opens-from: "⊕ Start exploring" button under Chart C inside feeds-modal-toolpath (SetFeedsExplore(Some)); also clicking inside the plot.
controls:
- set-feeds-explore-point (RPM/feed sliders + plot click → SetFeedsExplore)
- apply-feeds-explore (✓ Apply explored values → ApplyFeedsExplore writes feed + rpm)
- reset-feeds-explore (⟲ Reset to current; → Snap to recommended; ✕ Close)
- request-feeds-suggestion (read-only: recommended diamond + target pre-derate marker + vendor band overlay)
reads-state: modal.explore (NomogramExplore{rpm, feed_mm_min}); current feed/rpm; FeedsExplain.recommended; machine envelope; matched_row chipload band
writes-state: (via events) operation.feed_rate + operation.spindle_rpm (ApplyFeedsExplore); modal.explore
confusable-with: optimize-modal (also proposes feed/rpm operating points and gates them by a chipload/power/deflection verdict — but search-based, not a manual drag); the per-row Apply in the comparison card above it
recommendation-sources-touched: vendor-lut
health: yellow — a third way to set feed+rpm on the same toolpath (after the comparison-card Apply rows and the inline toolpath-tab pills); powerful but adds a manual operating-point editor on top of two existing recommend-and-apply paths (P1/P6).
