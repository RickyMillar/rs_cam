# UI review pass — status

Tracker for `PLAN.md`. The orchestrator edits this file. Never delete a row.

Operator rulings, 2026-09-14:
- UR5: "ill take the reccomendation" — what is drawn is the selection; the
  eye appears only in All-toolpaths mode; the isolate pin and its two
  command rows are deleted.
- UR8 is in scope.

The first agent on this plan (a different account) implemented UR1, UR2,
UR6 and UR7 in the main checkout, uncommitted, and stopped on a usage limit
with two UR1 test arms red. The main session took the tree over from there,
repaired the UR1 harness, implemented UR3 in-session after its writer and
fixer rungs hit usage limits, and committed the verified state. The
`wt_ur3` / `wt_ur5` / `wt_ur8` worktrees were checked and are clean — that
session landed nothing in them.

| Package | State | Evidence |
|---|---|---|
| UR1 inspector cannot be widened by content | DONE | `inspector_width_is_tab_independent_up4.rs`: real warning-shaped Feeds-tab fixture renders inside the panel; source scans ban bare horizontal labels |
| UR2 every badge is a dot, Danger included | DONE | `the_workspace_bar_is_a_strip_dc3.rs` arm 2 flipped; one shared "N safety" badge text |
| UR3 one Run Simulation, sim card reads the kit | DONE | dc6 census: exactly one producer, named; off-workspace allowlist; `the_sim_row_offers_its_visibility_control_ur3.rs`; metric staleness DERIVED from `SimulationRunMeta::accepted_metric_options_revision` (the bool is gone); controller tests incl. `the_primary_and_the_builder_agree_about_a_runnable_project` |
| UR4 finish DC5a: modal is Explore only | DONE | Explore-only title/body + DC5a sentry; canonical Compare and one Why disclosure now live in the Feeds inspector; legacy card module/test and split applies deleted; full viz tests + clippy green |
| UR5 one visibility model | TODO — scout recon complete (full isolate/command migration map); operator ratified the selection-only model |
| UR6 button label sits in one place | DONE | `component_contracts_up2.rs`: one centered paint in every state, disabled semantics + WidgetInfo preserved |
| UR7 resource row has one affordance | DONE | `the_setup_rail_is_one_weight_dc4.rs`: chevron/menu mutually exclusive; "none" empty states route to import / library; classifier shared with File menu |
| UR8 Readiness leads with the first unmet action | TODO — Run-sim affordances already gated on `simulation_request_is_buildable` (UR3 overlap); the reorder itself is not done |
