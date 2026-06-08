surface: Transport & scrubber
file: crates/rs_cam_viz/src/ui/sim_timeline.rs
kind: panel
job: Drive playback — step/play/pause, show elapsed/total time, and set playback speed multiplier.
opens-from: Top row of the Simulation-workspace bottom panel
controls:
  - sim-step-backward
  - toggle-sim-playback
  - sim-step-forward
  - read-elapsed-total-time
  - playback-speed
reads-state: sim.playback.{playing,current_move,speed}, sim.total_moves(), estimate_times()
writes-state: sim.playback.speed; emits SimStepBackward, ToggleSimPlayback, SimStepForward
confusable-with: none (unique transport)
recommendation-sources-touched: none
health: green — single-purpose transport; speed shown as a project-relative × multiplier with the moves/sec basis in the tooltip.
