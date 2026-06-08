surface: shortcuts-window
file: crates/rs_cam_viz/src/ui/shortcuts_window.rs
kind: modal
job: Display a static read-only reference of keyboard shortcuts grouped by General / Toolpaths / Simulation.
opens-from: Help menu › "Keyboard Shortcuts..." (AppEvent::ShowShortcuts); closed via the window's open bool
controls:
  - display-shortcut-reference
reads-state: none (hardcoded shortcut table; only the `show` bool)
writes-state: none (toggles the show bool)
confusable-with: none
recommendation-sources-touched: none
health: red — the listed shortcuts are a hand-maintained string table that has drifted from the real bindings: e.g. it advertises F12 Screenshot, G/Shift+G generate, I/H toggles, Space/arrows/Home/End/[/]/Escape sim controls, none of which are registered in menu_bar.rs's input handler; this is prose standing in for the actual binding source of truth (P5) and an orphan-prone duplicate (P6).
