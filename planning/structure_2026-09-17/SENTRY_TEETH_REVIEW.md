# Sentry teeth review — the viz file splits of 2026-09-17

**Verdict: TEETH 14 / VACUOUS 1 — four fixes (section 3).** Fourteen of the
fifteen edited sentry files still fail on the defect they guard. One sentry —
`production_writes_go_through_apply_wp15a.rs` — stays green on its own defect,
because its skip list misses `src/app/mcp/tests.rs`. Three more sentries lose
coverage without going fully vacuous. The four fixes are in section 3.

Scope: reviewed at `5067b5b5`. `git diff --name-only 5845f295..5067b5b5 --
crates/rs_cam_viz/tests` lists fifteen files. The commits are
`e6a5476f..857709cd` (`app/mcp.rs`), `b3690b6c..ee25a1a3`
(`ui/properties/mod.rs`), `5358ec27` (`ui/properties/operations/mod.rs`) and
`5602e1c0` (`state/simulation.rs`).

The last two splits edited no sentry. No sentry reads `state/simulation.rs`.
One un-edited sentry reads `ui/properties/operations/mod.rs`:
`bottom_z_pin_note_g_bottompin.rs` line 45. Its anchor
(`pub(super) fn draw_heights_params`) and its end marker
(`// ── Stepover Pattern Diagram`) both stayed in `mod.rs`, and its slice is
3592 bytes before and after the split. That sentry is intact.

Method: a throwaway worktree at `5067b5b5`, one injected defect at a time, and the
pre-built sentry binary run directly. Most of these sentries read the source at
run time, so an injection needs no rebuild. Two sentries bake the source with
`include_str!` and were rebuilt. The baseline is 15 files, 15 test targets, all
green, 0 failures.

## 1. Injection results

| Sentry file | Edited arm | Injected defect | Expected | Observed |
|---|---|---|---|---|
| `command_surface_completeness.rs` | `MCP_SOURCES` skip list gains six rows, `app/mcp/tests.rs` among them | rename `Command::RemoveToolpath(` in `controller/events/{toolpath,model}.rs` | RED | **RED** |
| `effects_are_stamped_wp19.rs` | `JUSTIFIED` row repointed to `app/mcp/generation.rs` | rename `Command::SetToolpathDebugOptions(` there | RED | **RED** |
| `effects_are_stamped_wp19.rs` | new `MCP_SURFACE` list, H3 negative scan | `stamp_stale` call in `app/mcp/view.rs` | RED | **RED** |
| `effects_are_stamped_wp19.rs` | same list, coverage probe | `stamp_stale` call in `app/mcp/tests.rs` | RED | **GREEN** — finding 2 |
| `freshness_surfaces_g_freshrender.rs` | `properties_src()` reads the folder | rename `match freshness {` in the F2.2 block | RED | **RED** |
| `freshness_surfaces_g_freshrender.rs` | export-wording negative scan widened to the folder | blocking sentence in `properties/machine_panel.rs` | RED | **RED** |
| `freshness_surfaces_g_freshrender.rs` | same scan, exemption probe | the same sentence, plus a `blocking_toolpath_message` mention in `properties/pills.rs` | RED | **GREEN** — finding 3 |
| `inspector_header_wraps_g_reachwrap.rs` | `function_body` end marker reads the visibility prefixes | `truncate` inside `wrapped_small_label` | RED | **RED** |
| `inspector_header_wraps_g_reachwrap.rs` | header slice over the folder | bare `ui.label` with a two-sentence caveat in the header | RED | **RED** |
| `inspector_header_wraps_g_reachwrap.rs` | `reach_block` over the folder | a reach reading moved into the checkbox row | RED | **RED** |
| `inspector_width_is_tab_independent_up4.rs` | `properties_src()` reads the folder | bare `ui.label` in a horizontal row inside `draw_feeds_card` | RED | **RED** |
| `inspector_width_is_tab_independent_up4.rs` | same | remove `ui.set_max_width(ui.available_width());` from the panel root | RED | **RED** |
| `non_egui_sites_write_through_commands_wp6b.rs` | `is_mcp` repointed to `app/mcp/generation.rs` | `toolpath_configs_mut` call in `app/mcp/view.rs` | RED | **RED** |
| `non_egui_sites_write_through_commands_wp6b.rs` | the same allowance, ceiling arm | a second `toolpath_configs_mut` call in `app/mcp/generation.rs` | RED | **GREEN** — pre-existing, section 4 |
| `open_guard_asks_before_discarding_g_openguard.rs` | `MCP_SRC` repointed to `app/mcp/project.rs` | rename the guard call in `mcp_load_project` | RED | **RED** |
| `overlays_registry.rs` | `mcp_set_ui_view` end marker removed; the body runs to the end of `app/mcp/view.rs` | hoist `registry::apply_overlays` above the reach pump | RED | **RED** |
| `overlays_registry.rs` | same, decoy probe | delete the pump from the body, append a decoy `fn` that calls it at the end of the file | RED | **RED** |
| `overlays_registry.rs` | `inspector_source()` reads the folder | retired `"Cut"` checkbox in `properties/machine_panel.rs` | RED | **RED** |
| `production_writes_go_through_apply_wp15a.rs` | `MCP_SOURCES` skip list gains 5 `app/mcp/*` children | rename `Command::RemoveToolpath(` in `controller/events/{toolpath,model}.rs` | RED | **GREEN** — finding 1 |
| `production_writes_go_through_apply_wp15a.rs` | same, control run | the same rename, plus `app/mcp/tests.rs` | RED | **RED** |
| `the_gui_can_set_a_feed_g_fscontrol.rs` | `properties_src()` reads the folder | rename `PrecedenceField::new("Spindle:"` | RED | **RED** |
| `the_inspector_nests_once_dc5.rs` | `inspector_src()` concatenates the folder in name order | a second `"{} hints"` above the tab-strip call | RED | **RED** |
| `the_recommendation_explains_each_row_g_whyrow.rs` | `feeds_sources()` gains the properties children | rebuild the Why disclosure in `properties/machine_panel.rs` | RED | **RED** |
| `ui_string_hygiene.rs` | `scanned_paths()` gains 5 `app/mcp/*` children | a space run inside a literal in `app/mcp/view.rs` | RED | **RED** |
| `viewport_draws_selected_only_wp27.rs` | end marker reads the visibility prefixes | read `toolpaths_to_draw` inside `mcp_screenshot_toolpath` | RED | **RED** |
| `workspace_menu_complete_g_wsmenu.rs` | `MCP_TESTS_SRC` added to the round-trip arm | stop the round-trip test iterating `Workspace::ALL` | RED | **RED** |
| `workspace_menu_complete_g_wsmenu.rs` | `MCP_PROJECT_SRC` repointed | push the camera fit past the 800-byte window in `app/mcp/project.rs` | RED | **RED** |
| `workspace_menu_complete_g_wsmenu.rs` | `PROPERTIES_SRC` **not** repointed | duplicate the stale wording in `properties/toolpath_panel.rs` | RED | **GREEN** — finding 4 |

## 2. Slice sizes, before and after

The three end-marker changes are honest. Each number is the byte length of the
slice the arm reads. "Bare marker" is what the old marker returns against the
new source.

| Sentry | Arm | 5845f295 | 5067b5b5 | Bare marker at 5067b5b5 |
|---|---|---|---|---|
| `viewport_draws_selected_only_wp27.rs` | `mcp_screenshot_toolpath` body | 3858 | 3857 | 15677 (the rest of `view.rs`) |
| `inspector_header_wraps_g_reachwrap.rs` | `wrapped_small_label` body | 1149 | 1160 | 6500 |
| `inspector_header_wraps_g_reachwrap.rs` | `render_diagnostic_row` body | 4024 | 4024 | 4024 |
| `inspector_header_wraps_g_reachwrap.rs` | header slice | 6150 | 6150 | — |
| `inspector_header_wraps_g_reachwrap.rs` | `reach_block` | 5137 | 5137 | — |
| `overlays_registry.rs` | `mcp_set_ui_view` body | 10465 | 10467 | — |

No slice shrank. The two "bare marker" rows show the edit was necessary: every
assertion in those arms is negative, so the longer slice would have passed more
easily.

The `overlays_registry.rs` body now runs to the end of `app/mcp/view.rs`,
because the handler is the last item there. The decoy probe above shows the arm
still fails today. The risk is forward: a new method appended after
`mcp_set_ui_view` extends the slice.

## 3. The fixes

### Finding 1 — `production_writes_go_through_apply_wp15a.rs` is vacuous

The skip list omits `src/app/mcp/tests.rs`, so the scan reads that test module
as production view source. `app/mcp/tests.rs` constructs
`Command::AddToolpath(` and `Command::RemoveToolpath(`. Property 2 — "every row
whose `gui` reach reads `Reached` is constructed in the view" — therefore
passes on test code.

Proof: the injection renamed every production construction of
`Command::RemoveToolpath(` outside the MCP surface. `command_surface_completeness.rs`
failed and named the row `remove_toolpath`. This sentry passed. A second run
that also renamed the construction in `app/mcp/tests.rs` failed with the same
message.

Before the split this hole did not exist: the constructions sat in
`app/mcp.rs`'s inline `mod tests`, and `app/mcp.rs` was skipped whole.

Fix: `crates/rs_cam_viz/tests/production_writes_go_through_apply_wp15a.rs`,
`MCP_SOURCES` at line 103. Add the row

```rust
    "src/app/mcp/tests.rs",
```

after `"src/app/mcp/simulation.rs",`. The doc comment above the list says "The
list matches `command_surface_completeness.rs`", and that file already carries
the row.

### Finding 2 — `effects_are_stamped_wp19.rs` H3 scans less than it did

`MCP_SURFACE` lists `app/mcp.rs` and five children. It omits
`app/mcp/tests.rs`. Before the split, `strip_comments(read("app/mcp.rs"))` read
the inline `mod tests` too, so the negative scan covered it.

Proof: a `stamp_stale` call in `app/mcp/view.rs` fails the arm; the same call in
`app/mcp/tests.rs` does not.

Severity is low. A second stamp site inside a test module is not a production
defect. Repair it for the same reason the rest of the wave repointed paths: the
arm asserts a property of "the MCP surface", and the surface has a file the list
does not name.

Fix: `crates/rs_cam_viz/tests/effects_are_stamped_wp19.rs`, `MCP_SURFACE` at
line 677. Add

```rust
    "src/app/mcp/tests.rs",
```

### Finding 3 — the freshness export-wording exemption now covers 14 files

`no_freshness_surface_writes_its_own_export_blocking_sentence` skips a surface
whose source contains `blocking_toolpath_message`. The inspector's source is now
the concatenation of every `.rs` file directly in `src/ui/properties/`. One
child that calls the shared builder therefore exempts all fourteen files.

Proof: a blocking sentence in `properties/machine_panel.rs` fails the arm. The
same sentence passes once `properties/pills.rs` mentions
`blocking_toolpath_message`.

No properties file mentions the builder today, so the arm is live, not vacuous.
The amplification is new: before the split the exemption covered one file.

Fix: `crates/rs_cam_viz/tests/freshness_surfaces_g_freshrender.rs`, lines
226–236. Read the inspector as one row per file, not one row for the folder, so
the `continue` applies per file. Give `properties_src()` a sibling that returns
`Vec<(String, String)>` of name and text, and extend the loop:

```rust
    for (label, src) in [
        ("ui/toolpath_panel.rs", PANEL_SRC),
        ("readiness_panel.rs", READINESS_PANEL_SRC),
        ("workspace_bar.rs", WORKSPACE_BAR_SRC),
    ]
    .into_iter()
    .chain(properties_files().iter().map(|(n, s)| (n.as_str(), s.as_str())))
```

### Finding 4 — `workspace_menu_complete_g_wsmenu.rs` guards the old file only

Line 46 still reads

```rust
const PROPERTIES_SRC: &str = include_str!("../src/ui/properties/mod.rs");
```

The same commit repointed `MCP_SRC`, `MCP_TESTS_SRC` and `MCP_PROJECT_SRC` in
that file, and every other properties sentry moved to a folder reader. Two arms
here are negative:

- `the_empty_inspector_names_what_can_be_selected` asserts the string "Select
  an item in the project tree" is absent, and its message says "must be gone,
  not duplicated".
- `getting_started_reviews_before_it_exports` asserts "Generate and export
  G-code" is absent.

Both now read 53 KB instead of the whole 350 KB inspector. The wording they
forbid can come back in any of the thirteen sibling files.

Proof: `const ZZ_TREE: &str = "Select an item in the project tree";` appended to
`properties/toolpath_panel.rs` leaves the sentry green.

The positive arms are unaffected: the four anchors they read all stayed in
`mod.rs`.

Fix: `crates/rs_cam_viz/tests/workspace_menu_complete_g_wsmenu.rs`, line 46.
Replace the `include_str!` with a folder reader, as
`inspector_width_is_tab_independent_up4.rs` lines 63–81 already do, and read
`PROPERTIES_SRC` through it at each of the seven use sites (lines 186, 190, 206,
211, 215, 218). Keep `operations/` out of the read, as the other sentries do.

## 4. One pre-existing residual, not a defect of this wave

`non_egui_sites_write_through_commands_wp6b.rs` allows `app/mcp/generation.rs`
to name `toolpath_configs_mut` once. No file names it any more; only a comment
in `generation.rs` mentions it. A second call injected into `generation.rs`
therefore passes. The allowance is a dead ceiling.

This is by design and it predates the split. `MCP_GENERATE_ARM_ALLOWANCE` is
documented as a ceiling — "this scan stays green when it goes" — and
`git show 5845f295:crates/rs_cam_viz/src/app/mcp.rs` shows the site was already
a comment then. The P4 edit repointed the shield faithfully. Consider setting
`MCP_GENERATE_ARM_ALLOWANCE` to `0` now that WP11b has removed the site.

## 5. Evidence

- Worktree: a detached checkout of `HEAD` under the session scratchpad, removed
  after the run. The main checkout stayed clean.
- Baseline: 15 test targets, 0 failures, before any injection and again after
  the last restore.
- The run-time-read injections ran one at a time. The four `include_str!`
  injections ran in one build; their results are per test function.
- Every red run names the injected token, so no result comes from cross-talk.
- HEAD moved `5067b5b5` → `8db15b75` during the run. That commit changes two
  `CLAUDE.md` files only, so the review holds.

## Outcome (2026-09-17)

All four fixes and the wp6b note landed in `7665b5fc`, each with a red proof. The wp6b ceiling could not be set to 0 (`absurd_extreme_comparisons` rejects a `usize` minimum comparison), so the allowance, its branch and its assertion were deleted; a `toolpath_configs_mut` call in `app/mcp/generation.rs` now lands in the offence list with its line.
