surface: Signal spine (stacked metric graphs)
file: crates/rs_cam_viz/src/ui/sim_timeline.rs
kind: panel
job: Plot per-sample chipload / arc-engagement / axial-DOC / MRR / feed tracks over move-index for the focused toolpath, with chipload envelope bands and gate-trip dots.
opens-from: Scroll area at the bottom of the bottom panel; rendered only when cut metrics were captured
controls:
  - read-chipload-signal
  - read-arc-engagement-signal
  - read-axial-doc-signal
  - read-mrr-signal
  - read-feed-rate-signal
  - read-chipload-envelope
  - focus-hotspot
  - seek-playback
reads-state: sim.results.cut_trace.samples (effective_chip_thickness_mm, arc_engagement_radians, axial_doc_mm, mrr_mm3_s, feed_rate_mm_min, engagement.radial_woc_fraction), sim.cached_chipload_envelopes(), sim.focused_toolpath(), sim.debug.span_scope, sim.playback.current_move, sim.hovered_x
writes-state: sim.hovered_x, sim.playback.scrub_drag_active, sim.debug.focused_hotspot; emits SimJumpToMove
confusable-with: Selected-span metric grid (same chipload/engagement/DOC/MRR as scalar aggregates), now-playing chipload badge (same envelope), boundary-timeline (separate X coordinate space)
recommendation-sources-touched: vendor-lut (chipload envelope min/max from LUT — the dashed cl_min/cl_max band + burn/breakage shading), sim-feedback (all sample tracks)
health: yellow — chipload/engagement/DOC/MRR appear here as continuous tracks AND in the Selected-span grid as scalars (P1 same values two homes); the feed track shows feed but feed-rate is not editable anywhere in the simulation workspace (authoritative-edit lives in another cluster), so provenance of the plotted feed is implicit.
