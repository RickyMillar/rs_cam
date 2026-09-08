surface: Viewport overlay toolbar
file: crates/rs_cam_viz/src/ui/viewport_overlay.rs
kind: menu
job: Strip above the 3D viewport for camera presets, projection mode, the Overlays panel button, isolation, compute activity, and per-workspace actions.
opens-from: Drawn at the top of the 3D viewport in every workspace that renders one
controls:
  - set-view-preset
  - reset-view
  - toggle-projection
  - open-overlays-panel
  - toggle-isolate-toolpath
  - clear-isolation
  - cancel-compute
  - generate-all
  - run-simulation
  - reset-simulation
reads-state: AppState (workspace, viewport.isolate_toolpath, the overlay flags the Overlays button counts), projection, lanes[]
writes-state: overlays.open; emits SetViewPreset, ResetView, ToggleProjection, ToggleIsolateToolpath, ClearIsolation, CancelCompute, GenerateAll, RunSimulation, ResetSimulation
confusable-with: sim-setup-and-run + staleness card (RunSimulation also there)
recommendation-sources-touched: none
health: amber — P6 (2026-09-08) retired the two menus that made this surface red. `Show ▼` was a flat popover of twelve unrelated visibility toggles and is replaced by the `Overlays (n)` button, which opens the overlays-panel surface; `Shaded ▼` is gone because its `Wireframe` arm drew nothing (no wireframe pipeline: it hid an STL model and ignored a STEP one). No overlay flag is written here any more, so the duplicate-write finding against Inspector › View is closed. What remains amber: RunSimulation is still one of several homes for the run action, and the strip still mixes camera, isolation, compute-cancel and workspace actions in one row.
