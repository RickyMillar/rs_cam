surface: menu-file-direct-export
file: crates/rs_cam_viz/src/ui/menu_bar.rs
kind: menu
job: Bypass the export wizard and emit g-code straight to a file dialog (all toolpaths, combined-with-pauses, or per-setup).
opens-from: File menu › "Direct export (skip wizard)" submenu; also Ctrl+Alt+E for the all-toolpaths variant
controls:
  - export-gcode-direct
  - export-combined-gcode
  - export-setup-gcode
reads-state: state.session.list_setups() (combined/per-setup entries only shown when >1 setup)
writes-state: none (emits ExportGcode / ExportCombinedGcode / ExportSetupGcode)
confusable-with: export-wizard (does the same end goal via a different, validation-skipping path); menu-bar File › Export G-code (wizard)
recommendation-sources-touched: none
health: red — a second, validator-bypassing export pipeline parallel to the wizard; "skip wizard" power-user escape hatch is exactly the duplicate-home / confusable-roles smell (P1/P4) the audit hunts for. Per-setup export is also a third place (alongside project-tree setup context menu and the wizard's per-setup layout).
