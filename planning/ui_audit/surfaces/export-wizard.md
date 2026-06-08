surface: export-wizard
file: crates/rs_cam_viz/src/ui/export_wizard.rs
kind: modal
job: Walk the user through a gated, multi-step g-code export (post, output layout, coordinate/units, tool-change/spindle, setup pauses, preview/validate, save).
opens-from: File › Export G-code... / Ctrl+Shift+E (AppEvent::OpenExportWizard); stepper + Back/Next/Cancel nav; closes on Cancel/X (CloseExportWizard)
controls:
  - wizard-set-step
  - wizard-set-post
  - wizard-set-output-layout
  - wizard-set-filename-template
  - wizard-set-wcs-override
  - wizard-set-units-override
  - wizard-set-safe-z-override
  - wizard-set-dry-run
  - wizard-set-spindle-warmup
  - wizard-set-setup-pause-message
  - wizard-set-allow-validator-errors
  - wizard-save
  - display-post-metadata
  - display-tool-change-summary
  - display-coolant-summary
  - display-gcode-preview
  - display-validator-findings
  - display-export-summary
reads-state: state.show_export_wizard, state.wizard_active_step, state.gui.post (format, spindle_speed, safe_z), state.session.wizard() (output_layout, filename_template, wcs_override, units_override, safe_z_override, dry_run, spindle_warmup_secs, pause_message, allow_validator_errors), state.session (list_setups, toolpath_configs, tools, name), state.gui.toolpath_rt (stats), state.simulation
writes-state: none directly (emits Wizard* events that write session.wizard() fields + state.gui.post.format)
confusable-with: menu-file-direct-export (alternate export path that skips this entire gated flow); project-tree per-setup export
recommendation-sources-touched: vendor-lut (post-definition decimals/limits/WCS shown as authoritative read-only post metadata); sim-feedback indirectly (preview/save summary use simulated toolpath stats)
health: yellow — well-staged summary→detail flow overall, but: step_post mutates the *project* post format (state.gui.post.format) as a side effect of an export-time dropdown (P1 — same value the project-tree Post Processor item selects); spindle RPM / feed appear here only as read-only clamped warnings while their true edit home is the Feeds modal / tool config (P1/P7); coolant and pre/post snippets are shown read-only with prose pointing the user elsewhere ("edit in the toolpath inspector") instead of a link/affordance (P5).
