# Step 7.8: the template "Geodesic body + edges"

Implementer: I4. Date: 2026-10-28. Base: `d131559dd`. Design:
`step78_design.md`, approved by the lead. D-T1 to D-T4 are as
recommended; C4 is on for the edge op, in a follow-up patch.

Patch: `step78_tree_vs_d131559dd.patch` (16 entries: 14 changed files and
2 new sentries). Checked with `git apply --check` on a clean `d131559dd`.
I5's W4 is committed, so the tree held only my edits when I froze.

## What changed

### Core

| File | Change |
|---|---|
| `session/op_draft.rs` | `GeodesicPairRequest` (Copy), `GeodesicPairDraft`, `GEODESIC_BODY_NAME`, `GEODESIC_EDGES_NAME` and `ProjectSession::draft_geodesic_pair`. Each op goes through `draft_toolpath` (S11). The template then sets: op 1 `GeodesicAdaptive`, with the rest analysis on only when the edge op exists; op 2 `ContourParallel`, `FromRemainingStock`, the boundary `{enabled, DerivedRestRegions{placeholder}, Center, offset 0}`, and op 1's tool (or the stated `edge_tool`). New `built_tool_radius_mm(tool_id)`: the radius rule of the containment. |
| `session/command.rs` | The row `AddGeodesicBodyAndEdges` ("add_geodesic_body_and_edges"; GUI and MCP reached; the CLI skips it because a job file has no rest-regions boundary). `AddGeodesicBodyAndEdgesArgs { setup_index, body, edges }` and its dispatch arm. |
| `session/mutation/toolpath.rs` | The door `add_geodesic_body_and_edges`. It runs the AddToolpath checks (`floor_conflict`, `domain::validate_toolpath`) on both configs. It refuses a larger edge tool with `LargerRestConsumer::refusal("Center")` BEFORE any add, because the effects window does not roll back. Then it calls `add_toolpath_impl` twice in one `try_with_effects`, and writes op 1's new id as the edge op's source. `created` is op 1's index; the edge op is the next index. |
| `session/mod.rs` | The re-exports. I checked first: no other edit was in the file, and I told I5. |

### Viz and MCP

| File | Change |
|---|---|
| `ui/toolpath_panel.rs` | `add_geodesic_pair_menu_item`: one item, "Geodesic body + edges", after the 3D ops in `add_menu_items` (the setup `+` and the empty-queue `Add toolpath`). Its hover is `MENU_GEODESIC_PAIR_HOVER`. It is enabled under `add_menu_offers(Adaptive3d)`; else it is off, with the hover and "Requires 3D mesh (STL/STEP)". |
| `ui/mod.rs`, `controller/events/mod.rs` | `AppEvent::AddGeodesicPair` and its dispatch. |
| `controller/events/toolpath.rs` | `add_target` (shared with the op add: the setup, the selected model and the picks, Q7). `handle_add_geodesic_pair`, and `add_geodesic_pair`: the core draft, then ONE command, then the op add's GUI part (a runtime entry per op, op 1 selected, one R2 toast and one S11 toast). |
| `rs_cam_mcp/src/server.rs` | `AddGeodesicBodyAndEdgesParam { setup_index, tool_index?, model_id?, edges?, edge_tool_index? }`, and the feature token `geodesic_body_and_edges`. |
| `mcp_server.rs`, `mcp_bridge.rs`, `app/mcp/commands.rs` | The tool `add_geodesic_body_and_edges` on the core route. `core_add_geodesic_pair` drafts with the same core door: an absent tool and model take the core rules; a bad `tool_index` or `edge_tool_index` refuses. The describe arm makes the runtime entries and replies `{ body_index, edges_index, no_tool, feeds_refusal }`. |
| `tests/snapshots/mcp_wire_surface.json` | Re-blessed: +55 lines, the new tool `add_geodesic_body_and_edges` with its param schema. No other row moves. |
| `FEATURE_CATALOG.md` | The rest-regions row states the template and its sentries. |

## Deviations from the design

- **Undo (§2 said "one undo step").** The GUI has no undo for an added
  toolpath: the plain `AddToolpath` pushes no history entry either. Only
  `AddBoreFromDrill` has one (`BoreAdded`). So the template is one command
  and one effects window, but it is no more undoable than a single add.
  Undo removes neither op. An undo entry for adds would be a new feature
  for every add; tell me if you want it.
- **The D53 radius rule.** The door compares the radii with the new
  `built_tool_radius_mm`. `larger_rest_consumer` (in `generation.rs`)
  still has the same rule as a local closure. I did not edit
  `generation.rs` (I5's file at the time). One follow-up line makes
  `larger_rest_consumer` call `built_tool_radius_mm`, so there is one
  definition.
- **Generation of the pair.** The edge op reads the remaining stock, so it
  generates only after a simulation of op 1 (the GUI's generate plan does
  this). The core sentry simulates between the two generations, as the D50
  sentry does.

## Line counts

| Surface | Before | After |
|---|---|---|
| Inspector panel | unchanged | unchanged |
| Add menu, 3D group | the 3D op items | + 1 item ("Geodesic body + edges") |
| Toolpath queue after one action | — | + 2 rows (+ 1 with MCP `edges: false`) |

## Tests

New core sentry `--test the_template_adds_the_channel_pair_s7_8`, on the
D53 channel:

1. The pair has the values of the design §4. The setup order is `[0, 1]`.
   The edge source is op 1's id, and `larger_rest_consumer` is `None`.
   Op 1 generates; after a simulation, the edge op generates and holds a
   result.
2. `edges: false` adds op 1 alone, with its rest analysis off.
3. A Ø8 edge tool with a Ø6 body refuses with the D53 Center sentence, and
   the command adds nothing.
4. With no tools, both ops are added with no tool.

New viz sentry `--test the_template_has_one_door_s7_8`:

- The GUI event stores the core draft (name, model, tool, params, stock
  source, rest analysis) for both ops. The edge source is the new id, op 1
  is selected, and both have a runtime entry.
- A source scan checks that the MCP arm uses `FirstFitting`, `Rule`,
  `draft_geodesic_pair` and `Command::AddGeodesicBodyAndEdges`, and that
  the add menu draws the item and offers it under the 3D Rough rule.

## Gates (green)

All cargo runs went through `scripts/cargo_lane.sh`, in the foreground.

- `cargo clippy --no-deps -p rs_cam_core -p rs_cam_viz -p rs_cam_mcp --all-targets --features rs_cam_core/heavy-tests,rs_cam_core/research,rs_cam_core/test-support -- -D warnings`:
  clean.
- Core `--test` sentries, all passed:
  - `the_template_adds_the_channel_pair_s7_8`
  - `command_registry_completeness` (10)
  - `a_larger_rest_consumer_tool_refuses_d53` (2)
- Core `--lib -- op_draft mutation`: 49 passed.
- `cargo test -p rs_cam_mcp`: 36 passed. It includes the `build_info`
  freeze and the F26 unknown-field scan.
- `cargo test -p rs_cam_viz --lib`: 607 passed.
- Viz `--test` sentries, all passed:
  - `the_template_has_one_door_s7_8` (2)
  - `mcp_wire_surface_pin`
  - `mcp_core_arm_describes_every_row`
  - `mcp_authoring_surface`
  - `mcp_escape_hatches`
  - `every_add_door_takes_one_model_and_tool_q7`
- `cargo fmt --check` on core, viz and mcp: clean.

## Follow-up (queued)

- **C4**: when I3's `contour_each_level` lands, set it to `true` on the
  edge op in `draft_geodesic_pair` (source: G-RESTDOC, D61/D68). Add one
  assert to `the_template_adds_the_channel_pair_s7_8`.
- **One radius rule**: `larger_rest_consumer` calls
  `built_tool_radius_mm`.
