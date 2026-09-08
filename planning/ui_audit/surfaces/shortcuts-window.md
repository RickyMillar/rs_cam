surface: shortcuts-window
file: crates/rs_cam_viz/src/ui/shortcuts_window.rs
kind: modal
job: Display a static read-only reference of keyboard shortcuts grouped by General / Toolpaths / Overlays / Simulation.
opens-from: Help menu › "Keyboard Shortcuts..." (AppEvent::ShowShortcuts); closed via the window's open bool
controls:
  - display-shortcut-reference
reads-state: none (hardcoded shortcut table; only the `show` bool)
writes-state: none (toggles the show bool)
confusable-with: none
recommendation-sources-touched: none
health: amber — the table is still a hand-maintained string list rather than a projection of the bindings, so it can drift from `app/input.rs` (P5). The Overlays block added by P6 (O, Shift+O, S, P, R, X, `,` / `.`) matches `RsCamApp::handle_overlay_shortcuts`, which is bound in both the editor and the simulation handlers; the earlier claim that the General / Toolpaths / Simulation rows were unregistered is stale — they are registered in `app/input.rs`, not in `menu_bar.rs`.
