# Known reds: triage (2026-11-09)

Author: the triage agent (no code edit, no git write). Tree: HEAD `90e03440b`
with the P22-0c working-tree diff of another agent on top. Each red ran once,
alone, through `scripts/cargo_lane.sh test -p <crate> -q -j 2 --test <name>`.

Classes:

1. Stale test. A ruling or a commit changed the product, and the test pins the old behaviour.
2. Product defect. The test is right.
3. Environment or timing. The red depends on the machine, the load or the feature set.
4. Waits on a named phase.

"Parallel-safe" means that the fix does not touch `stock/`, `tool/`,
`compute/collision_check.rs`, `compute/obstacle.rs`, `compute/simulate.rs`,
`maps/tier_flute_reach.rs`, `session/compute/measured.rs`, the viz viewport,
the tool panel or `sim_render`. It also means that the result does not read
the simulation that P22-0c changes now.

## A. rs_cam_viz

### A1. `an_unbacked_cell_still_adds_its_operation_fm5` (3 tests): class 1

```text
an_unbacked_cell_still_adds_its_operation_fm5: exactly one Warning toast: []  left: 0 right: 1
a_tool_rule_refusal_still_stops_the_add_fm5: unexpected toast: Cannot add Scallop Finish toolpath: import geometry first
the_mcp_add_reply_carries_the_refusal_fm5: anchor: the MCP add door has an Unbacked arm
```

- Cause. Three commits changed the add door:
  - `74af0f8e1` (S11, operator ruling 2026-10-23) moved the `Unbacked` arm into core `ProjectSession::draft_toolpath` (`session/op_draft.rs:661`). The MCP door stores `draft.feeds_refusal`. `commands.rs` has no `Unbacked` arm now.
  - `e1683dc5c` (W2b) removed the toast for a draft with no feeds. Missing feeds is a state, and the inspector shows it.
  - A wrong tool now gives a draft with no tool (R2 `e53629116`). The geometry rule then stops the add. `add_toolpath_via_gui_g_guiadd.rs:242` pins the new "import geometry first" text.
- The new contract has a sentry: `one_draft_for_every_add_door_s11.rs` claims 1 and 2 cover the Unbacked add, the "no toast" rule and the MCP door.
- Fix. Delete `crates/rs_cam_viz/tests/an_unbacked_cell_still_adds_its_operation_fm5.rs`. Alternatively, rewrite it to pin "op added, `feeds_refusal` set, no toast".
- Blast radius: 1 file. No other file names the binary.
- Parallel-safe: yes.

### A2. `panel_drafts_leave_egui_memory_ui09` (2 tests): class 1

```text
Over budget: ui/load_refusal_modal.rs 3 (allowed 0), ui/properties/toolpath_panel/issues.rs 2 (allowed 0)
ui/properties/selection_section.rs is allowed 2 egui-memory sites and holds 0.
```

- Cause. The ratchet list did not follow three commits:
  - `5974e642c` (G-D) added the `expand_id` bool to the load-refusal modal.
  - `d34a2db7e` (W7) moved the issue stack "expanded" bool into `toolpath_panel/issues.rs`.
  - `ddb2f6d0d` (W3h) removed the 2 sites from `selection_section.rs`.
- All 5 new sites are disclosure toggles. The rule of the test allows a view toggle when it is named with its reason.
- Fix. In `EGUI_MEMORY_ALLOWANCE` (`tests/panel_drafts_leave_egui_memory_ui09.rs`), do these steps:
  1. Add `ui/load_refusal_modal.rs` with 3 sites and `ui/properties/toolpath_panel/issues.rs` with 2 sites. Give the reason "view toggle: the expanded state of a disclosure".
  2. Remove the `selection_section.rs` row.
- Blast radius: 1 file (5 hits).
- Parallel-safe: yes.

### A3. `effects_are_stamped_wp19` (2 tests): class 1

```text
the `#[cfg(test)] mod` resolver excluded 43 file(s). ... More than 42 means it hides production code from the scan.
```

- Cause. `6bb0d4820` (W3j, 2026-10-30) added `#[cfg(test)] mod glyph_tests;` in `ui/components/mod.rs`. That file is the 43rd test file. The ceiling was set to 42 by W2c `14285edf6` on the same day.
- Fix. Set `MAX_EXCLUDED_FILES` to 43 in `tests/effects_are_stamped_wp19.rs:140`. Name `ui/components/glyph_tests.rs` in its doc comment.
- Blast radius: 1 file (3 hits).
- Parallel-safe: yes.

### A4. `new_3d_op_boundary_is_silhouette_plus_tool_r2` (`both_creation_doors_call_the_shared_constructor`): class 1

```text
src/controller/events/toolpath.rs: the B.7 has-mesh arm is gone; the scan below is vacuous
```

- Cause. S11 (`74af0f8e1`) moved the boundary rule into the core draft door. `BoundaryConfig::for_3d_op` and `m.mesh.is_some()` are now in `rs_cam_core/src/session/op_draft.rs:206,257`. The GUI door and the MCP door call `draft_toolpath`, and `one_draft_for_every_add_door_s11` pins that.
- Fix. Point the scan of this arm at `../rs_cam_core/src/session/op_draft.rs`, which holds the anchor and the constructor call. Keep the "no pre-R2 literal" check on the two viz door files.
- Blast radius: 1 file. The anchor text `m.mesh.is_some()` is in 18 files, but only this test reads it as an anchor.
- Parallel-safe: yes.

### A5. `generate_all_skips_disabled_g_genalldisabled`: class 1

```text
the plan submits its first step and waits for the completion: []  left: [] right: [ToolpathId(0)]
```

- Cause. W2c (`14285edf6`) makes an automatic plan step skip an op that is not ready (`controller/events/compute.rs:1945`). The fixture builds `OperationConfig::new_default(Face)` with model 0 and no model. After S11 (`74af0f8e1`), that op has unset feeds, so it is "Not ready" and the plan submits nothing.
- The enable filter still holds: the plan walk gives `[0, 2]`, and that assertion passes.
- Fix. In `tests/generate_all_skips_disabled_g_genalldisabled.rs` `toolpath()`, make the ops Ready. Call `rs_cam_core::compute::fixture_values::fill_unset(&mut op)`, as the core `common::session::toolpath_config` does. Then check that op 0 has no other "Not ready" cause.
- Blast radius: 1 file.
- Parallel-safe: yes.

### A6. `the_inspector_text_is_bounded_ic1`: class 1, but the lead must decide

```text
Pocket / Passes: 41 visible text rows, cap 40.
3D Rough / Passes: 38 visible text rows, cap 37.
3D Finish / Passes: 30 visible text rows, cap 29.
Tool: 18 visible text rows, cap 17.
```

- Each surface is over by exactly one row. The caps are the measure with no margin (W6 rule). The Passes caps were last set by P17b (`8a50ee9fb`, 2026-11-07).
- I did not find which row is new. On the Passes tab, "Ramp feed" is in the speed section, but `baca127f5` (P5b) added it before the P17b measure. On the Tool panel, P22-0c-1 (`92e0d71dc`) changed only the preview draw, not a label.
- The inventory is in the scratchpad: `/tmp/claude-1001/-home-ricky-personal-repos-rs-cam/7a1b80aa-de3a-4f94-9eb3-ffbff208a378/scratchpad/ic1.txt`. Regenerate it with `-- --nocapture`.
- Fix. Choose one of these two:
  - Find the new row by a run at `8a50ee9fb` in a worktree, and diff the inventories.
  - Raise the four caps by 1 in `tests/the_inspector_text_is_bounded_ic1.rs:419-452`, with the row named.
- The operator rule "no text clutter in the UI" applies, so the lead decides. The fix does not edit the cap blind.
- Blast radius: 1 file.
- Parallel-safe: no for the Tool cap, because the P22-0c diff edits `ui/properties/tool.rs`. Measure the Tool cap after P22-0c. Yes for the Passes caps.

### A7. `the_selection_section_writes_edits_only_p4_3`: class 1

```text
no painted text '⋯' (#1); painted: [..., "layers.dxf", "…", "…", ..., "Selection", "×", "×", "…", "…", ...]
```

- Cause. W3j (`6bb0d4820`) changed the list menu glyph from U+22EF to U+2026, because the font has no U+22EF (`ui/components/list_box.rs:25` `MENU`). The test still has `const MENU: &str = "\u{22EF}";` at line 419.
- Fix. Set `MENU` to `"\u{2026}"`, or import `rs_cam_viz::ui::components::list_box::MENU` if it is public.
- Caution: egui also paints U+2026 for elided text, and the painted list holds each text twice. Before you accept the fix, check the `nth` index of each of the 4 menu clicks (lines 348, 394, 399, 408).
- Blast radius: 1 file. Two files hold `u{22EF}`; the other is `glyph_tests.rs`, which is the red proof.
- Parallel-safe: yes.

### A8. `the_gui_can_set_a_feed_g_fscontrol` (`the_spindle_override_shows_the_project_default_g_fscontrol`): class 1

```text
the spindle row no longer uses PrecedenceField, so the project default it overrides is not on screen
```

- Cause. S11 (`74af0f8e1`, operator ruling 2026-10-23, "If ther is no logical reason for a default, then dont have it") removed the RPM fallback to the post spindle speed. It also removed the `PrecedenceField::new("Spindle:", ..)` call. The project now has no default RPM to show.
- Fix. Delete arm 3 of `tests/the_gui_can_set_a_feed_g_fscontrol.rs` (from line 299).
- `PrecedenceField` now has no production call site: it has 12 hits in 5 files, and those are the component, its re-export, docs and one comment. A separate tidy can delete it. This triage does not propose that.
- Blast radius: 1 file.
- Parallel-safe: yes.

### A9. `loose_executor_is_not_named_in_viz_wp12`: class 1

```text
These lines have none: .../ui/properties/toolpath_panel/tabs/geometry.rs:276
```

- Cause. W3 (`20f25205d`) wrapped the comment again. The allowlist `line_marker: "one generation uses"` is now on line 275, and `session/compute.rs` is on line 276.
- Fix. In `tests/loose_executor_is_not_named_in_viz_wp12.rs:137`, set the `line_marker` to "`session/compute.rs` clones `tc.boundary`". That text is on the line that names the module. This is a test-only edit; the alternative is to wrap the comment again.
- Blast radius: 1 file (1 hit).
- Parallel-safe: yes.

### A10. Flake `render_pipelines_headless_g_pipesmoke`: class 3

- The test passed alone tonight: 8 passed in 0.76 s.
- It asks for a software wgpu adapter first (`fixtures/gpu.rs`). The SIGSEGV occurs only in the full parallel viz run, so it is a driver crash under load.
- Fix: none in product code. Choose one of these two:
  - Run it in a separate step of the viz gate.
  - Mark it `#[ignore]` and run it with `--ignored` in the gate script.
- Parallel-safe: yes.

## B. rs_cam_core

### B1. `every_op_conforms_w4` check 6: class 2 (a schema annotation)

```text
adaptive3d/schema: the required param `contour_each_level` names no field the config holds; a write to it writes nothing
```

- Cause. `e8072f172` (C4) added `#[serde(default, skip_serializing_if = "std::ops::Not::not")]` to `contour_each_level`. A default config therefore does not serialize the key. The schema still publishes the param as required.
- The precedent is `cut_past_open_sides` (`compute/operation_configs.rs:1008-1009`), which has the same serde attribute and `#[param(.., optional)]`.
- Fix. Add `optional` to `#[param(help = HELP_CONTOUR_EACH_LEVEL)]` at `compute/operation_configs.rs:1895`.
- After the edit, run these tests:
  - `every_op_conforms_w4`
  - `contour_each_level_is_off_by_default_c4`
  - viz `mcp_wire_surface_pin` and `mcp_authoring_surface`, because the published schema changes one flag.
- Blast radius: 1 line in `compute/operation_configs.rs`. The name `contour_each_level` is in 18 files in the repository.
- Note: the P22-0c diff edits the test file `every_op_conforms_w4.rs` (4 lines), but not this arm.
- Parallel-safe: yes. The edit is in `compute/operation_configs.rs`, not in the listed files.

### B2. `a_clearing_cut_holds_the_pass_load_g_adaptpassload` (`a_straight_plunge_entry_is_exempt_only_leaving_its_hole`): class 2

```text
non-exempt ClearingCut samples over 0.5460 that the exact oracle also reads over (move, sim, exact):
[(2810, 0.8329, 0.9404), (6159, 0.8329, 0.9404)];
per producer: ... "Residue mop": (2, 0.8329, 53.1) ...
```

- The exact oracle agrees with the simulation. A "Residue mop" step reads 0.94 of D, and the pass ceiling is 0.546. The step occurs at two depth levels.
- This is a planner defect in the 2D Adaptive mop walk (`adaptive/`), not a simulation artefact. It was already red at `c6826ad05` (2026-11-03).
- Fix size: M. Do these steps:
  1. Bisect for the commit that moved the mop step.
  2. Hold the mop walk to `step_within_pass_load` for the step that reads 0.94.
- Blast radius: `adaptive/` only.
- Parallel-safe: yes for the code. No for the oracle, because it reads the session simulation, which P22-0c changes. Verify the result after P22-0c lands.

### B3. `a_rapid_leaves_cut_depth_straight_up_g_adaptrapidlift`: class 4 (P22-0c)

```text
sim-only rapid flags moved (move, exact clearance mm): [(6930, 0.24951132908296492)]  left: 1 right: 0
```

- The exact oracle clears every rapid: no rapid enters material. The simulation flags one clear rapid, move 6930, with 0.25 mm exact clearance. The file names this false positive: the cell `conservative_top` reads a wall about 0.35 + 0.25 mm closer than it stands. `SIM_ONLY_FLAGS` pins 0.
- The rapid check is the contact kernel that P22-0c-3 and P22-0c-4 replace on the live stock.
- Fix. After P22-0c, run the test again. Then choose one of these two:
  - Set `SIM_ONLY_FLAGS` to the new count, with the move named.
  - Leave the pin at 0 if the new kernel stops the false flag.
- Blast radius: 1 file.
- Parallel-safe: no. The result reads the rapid check of P22-0c.

### B4. `lateral_setup_end_to_end` (3 of 4 tests): class 1 (S11 group)

```text
generate the Front-setup pocket: Operation(NotSet(NotSet { op: "Pocket", kind: Feeds, fields: ["plunge_rate", "spindle_rpm"], tool: None }))
generate the Front-setup drill: Operation(NotSet(NotSet { op: "Drill", kind: Feeds, fields: ["spindle_rpm"], tool: None }))
```

- The local `toolpath_config` at line 209 does not call `fill_unset`. The shared fix is in section C.
- Parallel-safe: no. The P22-0c diff edits this file now (`.mounted(..)` at about line 380). Do it after P22-0c is committed.

### B5. `perf_golden_sim_metrics`: class 1 (probable), not a timing pin

```text
[Profile].air_cut_time_s: got 1.9082943185, golden 1.9294586420 (rel err 1.097e-2 > 1e-3)
[Profile].per_kinematics[Arc].cutting_runtime_s: got 3.0221452739, golden 3.0433095974
[Profile].per_kinematics[Arc].average_radial_woc_fraction: got 0.4518534188, golden 0.4487110595
(6 fields; the test runs in 0.33 s)
```

- Correction to STATUS. These numbers are simulated machine times. They are not wall-clock times, so this is not a budget pin. The red is deterministic.
- About 0.021 s of Arc cutting time is now air time on the Profile arm. One arc sample, or part of one arc, now reads as air.
- The golden was last blessed at `b0a133614` (2026-10-21). STATUS shows the red on 2026-11-04. The candidate commits touch Profile arcs and entries in that window: P3 `c0db8dcb6` (K4 entry) and P6.0c (the lead side).
- Fix size: M. Do these steps:
  1. Bisect `b0a133614..HEAD` in a worktree. The test is fast.
  2. Name the commit.
  3. Re-bless with `UPDATE_PERF_GOLDENS=1`, with that commit and its reason in the message.
- Blast radius: 1 fixture JSON (`tests/fixtures/perf_golden_sim_metrics.json`).
- Parallel-safe: no. The metric reads `compute/simulate.rs`, which the P22-0c diff edits. Bisect and bless after P22-0c.

### B6. `arc_fit_disposition_a5` (`retired_lift_leaves_feed_at_the_calculator_value`): class 4 (P21)

```text
DC-2 Ø2-tip tapered ball / WhiteOak / small router: Suggest must ship the un-lifted feed 2499 ± 0.5, got 2317
```

- The reason is confirmed. `phases/P21_DESIGN.md` section 6.3 (line 393 onward) gives 0.06437 x 2 x 18000 = 2317 on `MachineProfile::default()`.
- The arm returns to 2499 only when the DC-2 fixture opts into `ConstantChip { rpm_ceiling: Some(24000) }` with stated rates. Then n = floor(2500 / (0.06437 x 2)) = 19419, and F = 2499.
- Do not chase this red. P21 owns it.
- Parallel-safe: n/a.

### B7. `the_envelope_meets_its_time_budget` (core `--lib`): class 3

- The test passed alone: 1 passed, and the binary ran in 0.28 s.
- `ENVELOPE_TIME_BUDGET_MS` is 300 ms. The doc records 121 to 148 ms alone, and 515 ms in the parallel `--lib` run beside a viz suite (`feeds/envelope.rs:37-63`). The red is load only.
- Fix. Choose one of these two:
  - Move the clock assertion behind `#[ignore]`, and run it alone in the gate.
  - Keep the assertion on the point count and the `Runtime` limit, and print the time without an assertion.
- Do not raise the budget again: it is already about 2x the measured time alone.
- Blast radius: 1 test in `feeds/envelope.rs` (3 hits).
- Parallel-safe: yes.

### B8. `_litmatrix_rubbing_floor_clamp` (`ipe_pocket_ships_its_computed_feed_above_the_floor_after_r4`): class 1

```text
the Ipe cell's row moved  left: "amana-flat-hardwood-pocket-6000-2f-spektra" right: "amana-compression-wood-pocket-6350-2f"
```

- The live record from the `record_the_cell` instrument: Ipe and Ipe long tool give rpm 18000, feed 2938.57 and point 0.081627 mm/tooth. The pin is 0.048864.
- STATUS saw a third row (`freud-solid-carbide-quarter-hardwood`) at `c0db8dcb6`. The row moved twice.
- P13c intends the new row. The claim check `explainer_p13c/assert_claims.py:182` asserts "FM1 reads the Spektra row, not the upcut row" for 6 mm hardwood.
- Likely commits: `b57dab8de` (P13c) or `d82985059` (P13 items 4-5, "industrial rows are baselines").
- Fix size: M. The file doc demands a derived chain, not a blind bless ("A scaling-law change must re-pin this file"). Do these steps:
  1. Re-derive the chain table for the Spektra row.
  2. Re-pin `IPE_ROW_ID`, `IPE_DERATED_POINT_MM_TOOTH` and `IPE_MOVED_CELL_ADVANCE_MM_TOOTH`.
  3. Check that the "above the floor" arm still holds. It holds: 0.0816 > 0.025.
- Blast radius: 1 file.
- Parallel-safe: yes. The fix is in a test file and reads only `feeds/`.

### B9. The "others" list

#### `a_lead_curls_away_from_the_part_p6_0c` (`only_the_surface_ops_state_no_side_p6_0c`): class 1

```text
`RingMaterial::NotStated` is written outside the surface ops: ["ops/chamfer.rs"].
```

- `ops/chamfer.rs:173` names `NotStated` in a match pattern. It reads the variant and does not write it.
- P1 (`b01777df5`, deny `wildcard_enum_match_arm`) made the arm name every variant. The source scan cannot tell a pattern from a write.
- Fix. Make the scan skip a match-pattern line, for example a line that holds `=>` or starts with `|`. Edit the test, not chamfer.
- Blast radius: 1 test file. The needle has 10 hits in 8 files.
- Parallel-safe: yes.

#### `a_model_bottom_floor_reads_the_model_d1`: class 3 (feature set)

```text
STEP support not compiled into this build (enable the `step` feature)
```

- Fix. Put `#[cfg(feature = "step")]` on the STEP arm `a_model_bottom_floor_cuts_to_the_model_bottom_d1`, as five other STEP tests do. One example is `a_step_shell_faces_out_at_import_p3_4.rs`.
- The red has been there since `a68cf4234`.
- Blast radius: 1 file.
- Parallel-safe: yes.

#### `a_rough_cuts_no_deeper_than_its_leave_g_roughgouge` (`a_cut_into_the_leave_is_a_caution_g_roughgouge`): class 1

```text
... [Measured at generation ... the export does not warn on it.]   (test expects "export does not refuse")
```

- `c6826ad05` (P2 commit B, K6: "no finding refuses a job") changed the text.
- Fix. In `tests/a_rough_cuts_no_deeper_than_its_leave_g_roughgouge.rs:233`, expect "export does not warn on it". Also correct the doc line 17.
- Blast radius: 1 file (2 hits).
- Parallel-safe: yes.

#### `a_signature_serialises_every_op_f065`: class 1

```text
Face: the signature must open with the op text
```

- S11 (`74af0f8e1`) made `common::session::toolpath_config` call `fill_unset`. The stored op is therefore not the default op that the test serializes.
- Fix. Compare with `serde_json::to_string(&tc.operation)` in `tests/a_signature_serialises_every_op_f065.rs:52`.
- Blast radius: 1 file.
- Parallel-safe: yes.

#### `an_op_change_keeps_its_selection_p2_3` (`a_change_to_an_op_with_no_selection_reports_the_drop`): class 1

```text
a default selection holds no user choice
```

- W3h (`ddb2f6d0d`) made `Selection::empty()` the default selection of a new 2.5D op (`compute/catalog.rs:1779`). `Selection::drawing()` is now a user choice, so the drop is reported, which is correct.
- Fix. In the second half of the arm (line 100), use `Selection::empty()`.
- Blast radius: 1 file.
- Parallel-safe: yes.

#### `layered_framework_owns_the_ladder_w3` (2 tests): class 1 (probable)

```text
the Contour pocket changed  left: (232, 9145900763367787069) right: (232, 18123423408609281905)
changed: ["zigzag", "trace", "profile tabs", "profile tabs spring"]
```

- The move counts are equal. Only the hash moved.
- `move_fingerprint` is FNV-1a over `Debug` of the moves (`tests/common/fingerprint.rs:44`). A new `Move` field or `MoveIntent` variant moves the hash, and the geometry does not need to change. P3 `c0db8dcb6` added `EntryApproach`, for example.
- The level-first arms compare `sorted_cuts`, and they pass.
- Fix size: M. Do these steps:
  1. Run once with the pins re-measured at `68226ec5` and `86692c56` in a worktree.
  2. Confirm that the positions are equal.
  3. Re-pin, or change the fingerprint to the geometry only (move type and target).
- Blast radius: 1 file.
- Parallel-safe: yes.

#### `setters_have_rows_wp15a`: class 1

```text
I read 1 setter(s) that no row calls: .../session/mutation/toolpath.rs:847: bind_tool_impl
```

- `4d684209c` (W1-core + W2a) added `bind_tool_impl`, the raw half of `set_toolpath_tool`. `add_tool` also calls it.
- Fix. Add `"bind_tool_impl"` to `CRATE_PRIVATE_HELPERS` in `tests/setters_have_rows_wp15a.rs:143`. The list already exempts `*_impl` halves.
- Blast radius: 1 file.
- Parallel-safe: yes.

#### `vendor_lut_sub_1mm` (2 tests): class 1

```text
expected 502 embedded observations ...  left: 567 right: 502
expected the 1mm 2-flute ZrN v8 tapered row to win for a 1mm 2-flute query
  left: "spetool-tapered-softwood-parallel-1000-2f" right: "amana-tapered-softwood-parallel-1000-2f-zrn-v8"
```

- The lib pin `feeds::vendor_lut::tests::test_embedded_loads_all_observations` reads 567 since P24a (`910dcf6d5`, + 53 straight rows; it read 514 at the triage, P16 `bd72997c9`). This integration copy still reads 502. Updated 2026-11-11 (sweep `phases/SWEEP_2026-11-11.md`).
- The winner moved under the vendor-row rules of P13. `d82985059` is the likely commit ("industrial rows are baselines").
- Fix. Do these steps:
  1. Copy the count and its chain text from the lib pin.
  2. Re-pin the winner after you confirm against the P13 items 4-5 ruling that SpeTool should win.
- Blast radius: 1 file.
- Parallel-safe: yes.

## C. The S11 "feeds not set" fixtures (17 targets named in STATUS)

Each of the 17 targets in the STATUS list builds its own `ToolpathConfig { .. }`
in the test file, or builds no config there:

- 15 targets have 1 to 4 local literals.
- `modulated_feeds_read_their_own_prefix_b5` and `tier_arcfit_burial_f4` build the config in a helper elsewhere.
- None of the 17 calls `fill_unset`, and none uses `common::session::toolpath_config`.

One shared fix covers all of them. Route each local builder through
`rs_cam_core::compute::fixture_values::fill_unset(&mut op)` and
`fill_unset_dressups`, which already hold the values from before S11. Or replace
the local builder with `common::session::toolpath_config`. Do not make the
harness apply Suggest. Suggest output moves with every feeds phase, and the
pins would move with it. The fixture values are fixed and come from before S11.
`adaptive3d_lift_bridge_b1` needs no feature: `Cargo.toml` does not gate it, and it ran in the sweep of 2026-11-11 (this note said `heavy-tests` before). `lateral_setup_end_to_end`
must wait until P22-0c is committed, because the P22-0c diff edits it now.
The fix size is M, because it touches 17 files. Each edit is mechanical.

## D. Table and order

| # | Red | Class | Fix size | Files | Parallel-safe |
|---|---|---|---|---|---|
| 1 | viz `an_unbacked_cell_still_adds_its_operation_fm5` (3) | 1 | S | delete the test file | yes |
| 2 | viz `panel_drafts_leave_egui_memory_ui09` (2) | 1 | S | the test file (allowance list) | yes |
| 3 | viz `effects_are_stamped_wp19` (2) | 1 | S | the test file (`MAX_EXCLUDED_FILES` 43) | yes |
| 4 | viz `new_3d_op_boundary_is_silhouette_plus_tool_r2` (1) | 1 | S | the test file (scan `op_draft.rs`) | yes |
| 5 | viz `generate_all_skips_disabled_g_genalldisabled` (1) | 1 | S | the test file (fixture `fill_unset`) | yes |
| 6 | viz `the_selection_section_writes_edits_only_p4_3` (1) | 1 | S | the test file (`MENU` = U+2026, check `nth`) | yes |
| 7 | viz `the_gui_can_set_a_feed_g_fscontrol` (1) | 1 | S | the test file (delete arm 3) | yes |
| 8 | viz `loose_executor_is_not_named_in_viz_wp12` (1) | 1 | S | the test file (`line_marker`) | yes |
| 9 | viz `the_inspector_text_is_bounded_ic1` (1) | 1, lead decides | S | the test file (4 caps +1 after the row is named) | Passes yes; Tool no |
| 10 | viz `render_pipelines_headless_g_pipesmoke` (flake) | 3 | S | the gate script, or `#[ignore]` | yes |
| 11 | core `every_op_conforms_w4` check 6 | 2 | S | `compute/operation_configs.rs:1895` (`optional`) | yes |
| 12 | core `a_rough_cuts_no_deeper_than_its_leave_g_roughgouge` | 1 | S | the test file | yes |
| 13 | core `a_signature_serialises_every_op_f065` | 1 | S | the test file | yes |
| 14 | core `an_op_change_keeps_its_selection_p2_3` | 1 | S | the test file | yes |
| 15 | core `setters_have_rows_wp15a` | 1 | S | the test file | yes |
| 16 | core `a_lead_curls_away_from_the_part_p6_0c` | 1 | S | the test file (scan skips patterns) | yes |
| 17 | core `a_model_bottom_floor_reads_the_model_d1` | 3 | S | the test file (`cfg(feature = "step")`) | yes |
| 18 | core `the_envelope_meets_its_time_budget` (lib) | 3 | S | `feeds/envelope.rs` test | yes |
| 19 | core `vendor_lut_sub_1mm` (2) | 1 | S | the test file | yes |
| 20 | core `_litmatrix_rubbing_floor_clamp` | 1 | M | the test file (re-derive the chain) | yes |
| 21 | core `layered_framework_owns_the_ladder_w3` (2) | 1 (probable) | M | the test file (+ worktree check) | yes |
| 22 | core S11 group (17 targets, incl. `lateral_setup_end_to_end`) | 1 | M | 17 test files | yes, except `lateral_setup_end_to_end` |
| 23 | core `a_clearing_cut_holds_the_pass_load_g_adaptpassload` | 2 | M | `adaptive/` mop walk (+ bisect) | code yes; verify after P22-0c |
| 24 | core `perf_golden_sim_metrics` | 1 (probable) | M | the golden JSON (bisect, then bless) | no (reads `compute/simulate.rs`) |
| 25 | core `a_rapid_leaves_cut_depth_straight_up_g_adaptrapidlift` | 4 (P22-0c) | S | the test file (`SIM_ONLY_FLAGS`) | no |
| 26 | core `arc_fit_disposition_a5` | 4 (P21) | — | P21 owns it | n/a |

Recommended order:

1. Rows 1-8 and 12-17: the S stale-test edits. They are all test-only, each is one file, and none touches a P22-0c file. One implementer can do them in one pass. The gate is each named binary.
2. Row 11: the one-line schema `optional`, with the MCP pin tests run after it.
3. Rows 18 and 10: the gate hygiene for the two load-only reds. The gate gets two lines quieter.
4. Rows 19, 20 and 21: the re-pins that need a short derivation or a worktree check.
5. Row 22: the S11 fixture sweep, without `lateral_setup_end_to_end` until P22-0c is committed.
6. After P22-0c: row 9 (Tool cap), row 25 (count the sim-only flags again), row 24 (bisect and bless), and row 23 (the mop-walk defect, verified on the new simulation).
7. Row 26 stays with P21.

Not run here: G-OPGOLDEN (46 rows; it has its own procedure) and the older STATUS viz names that are not in tonight's list (`apply_contract_a3`, `folder_maps_name_real_files_f140`, `mcp_authoring_surface`, `mcp_wire_surface_pin`). The lead's run at `b3895d544` did not list them as red.
