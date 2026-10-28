# Step 7.8 design: the template "Geodesic body + edges"

Author: I4. Date: 2026-10-28. Base: `22e96b652`. Approved by the lead
(D-T1 to D-T4 as recommended; C4 on for the edge op, see §4 and §6).

The operator ruling (OPERATOR_DECISIONS_2026-10-28.md, midday):

> one GUI action, "Geodesic body + edges", with the edge op optional; the
> edge tool is no larger than op 1's, with Inside-safe containment.

The operator's later notes on the same day bear on it:

- G-RESTDOC: the operator "likes the edge contour pass".
- C4: "Contour on each level" is one visible 3D Rough option, off by
  default.
- W4: a through cut ends with a tabbed Profile.

The one-line rule holds: the action adds no always-visible panel line.

## 1. What the action does

It adds two operations to one setup, in this order:

| # | Op | What it is |
|---|---|---|
| 1 | "Geodesic body" | a 3D Rough, `clearing_strategy = GeodesicAdaptive`, with its rest analysis on, so it keeps the regions it leaves |
| 2 | "Geodesic edges" (optional) | a 3D Rough, `clearing_strategy = ContourParallel`, on the remaining stock, with its machining boundary on the rest regions of op 1 |

This is the D47 channel pair: the pair that the D44, D48, D50 and D53
sentries build by hand (`tests/a_larger_rest_consumer_tool_refuses_d53.rs`,
`session_on`). The template makes the same pair through the product doors.

## 2. One command, the AddToolpath path

A new core command row: `Command::AddGeodesicBodyAndEdges(args)` in
`session/command.rs`. Its door is in `session/mutation/toolpath.rs`, beside
`add_bore_from_drill`.

```text
AddGeodesicBodyAndEdgesArgs {
    setup_index: usize,
    tool: ToolChoice,          // FirstFitting (GUI) or Exact (MCP tool_index)
    model: ModelChoice,        // Rule { selected } (GUI) or Exact (MCP model_id)
    edges: bool,               // the optional edge op
}
```

The door:

1. It builds op 1 with `ProjectSession::draft_toolpath` (S11, the one draft
   door of the GUI, MCP and the CLI: Suggest, `DressupConfig::for_op`,
   `draft_boundary`). Then it sets `clearing_strategy` and
   `rest_analysis.enabled`.
2. It adds op 1 with `add_toolpath_impl`, inside one `try_with_effects`
   window. That is the function the `AddToolpath` row calls, so the
   validation is the same: `floor_conflict`, `domain::validate_toolpath`
   and the R2 tool fit. This is the `add_bore_from_drill` precedent.
3. If `edges`, it builds op 2 with `draft_toolpath(.., ToolChoice::Exact(op 1's
   tool))`. It sets the four fields of §4, using op 1's new id, and adds
   op 2 with `add_toolpath_impl`.
4. It returns one `Effects`: `created` is op 1's index, and op 2 is the next
   index of the same setup (`Effects::created` holds one index). The whole
   action is ONE undo step.

There is no second door. The GUI and MCP send the one command through
`ProjectSession::apply`, and the controller's add handler copies its GUI
part: the runtime entry, the selection and the toasts.

**Decision D-T1 for the lead.** You wrote "every op must be added through
the existing AddToolpath command path". There are two readings:

- (a) one composite row that calls `add_toolpath_impl`, as above (the
  `AddBoreFromDrill` precedent; one undo step);
- (b) the surfaces send two `Command::AddToolpath`. The second needs op 1's
  new id, so the controller and MCP each build op 2 after op 1 lands. That
  is two copies of the pair rule and two undo steps.

I recommend (a): it is one rule in core, and the surfaces only call it.

## 3. Where the action lives (GUI)

It lives in the existing add menu: the `+` of a setup row and the empty
queue's `Add toolpath` button share `add_menu_items`. The 3D group gains
one item after the 3D ops. Nothing goes in the inspector.

```
┌ + ──────────────────────────────────────────┐
│ 2.5D (from a drawing or STEP faces)         │
│   Face  Pocket  Profile  …                  │
│ ─────────────────────────────────────────── │
│ 3D (from a mesh: STL or STEP)               │
│   3D Rough                                  │
│   Drop Cutter  Waterline  …                 │
│   Geodesic body + edges            ← new    │
└─────────────────────────────────────────────┘
   hover: "A Geodesic Adaptive rough, then a Contour Parallel rest op on
           the regions it leaves, with the same tool. The edge op is an
           ordinary op: untick it in the queue to skip it."
```

The item is enabled under the 3D ops' rule (`add_menu_offers`: a model with
a mesh). It is disabled with the same hover reason when there is none.

**The edge op is optional (D-T2 for the lead).** I recommend that the item
always adds both ops, with the edge op enabled. The edge op is then an
ordinary op, and its existing enable box in the queue skips it: one click,
no new widget. On wanaka op 1 clears almost all, and the edge op costs about
940 s; the operator who wants the body alone unticks it or deletes it. The
two other forms add clutter, so I do not recommend them:

- a submenu with "body + edges" and "body only";
- a modal with an "edges" checkbox.

MCP keeps the choice as an argument (`edges`, §7).

### Line count

| Surface | Before | After |
|---|---|---|
| Inspector panel | unchanged | unchanged (the action is in the menu) |
| Add menu, 3D group | the 3D op items | + 1 item |
| Toolpath queue after the action | — | + 2 rows (the two ops; + 1 with `edges` false over MCP) |

## 4. The defaults, and the source of each value

No number is typed. Each value comes from a rule or a door that exists.

**Op 1, "Geodesic body"**

| Field | Value | Source |
|---|---|---|
| op type | `Adaptive3d` (3D Rough) | the template |
| tool | the first fitting tool (GUI); the stated tool (MCP) | `draft_toolpath`, `ToolChoice` (R2) |
| model | the model rule (GUI); the stated model (MCP) | `draft_toolpath`, `ModelChoice` (Q7) |
| stepover, Depth/Pass, feeds | Suggest's values | `draft_toolpath` → `suggest_for` (S11). Q12 (the Geodesic stepover rule) is still open; the template takes whatever Suggest gives |
| `clearing_strategy` | `GeodesicAdaptive` | the template's name; the operator ruling |
| `rest_analysis.enabled` | `true` with the edge op; `false` without it | D44: the runner keeps the regions only on demand. A consumer is the demand; `auto_enable_rest_analysis_for_source` writes the same flag |
| boundary | the draft boundary: model silhouette, `Center`, offset = tool diameter | `draft_boundary` → `BoundaryConfig::for_3d_op` |
| stock source | `Fresh` | the draft |
| name | "Geodesic body" | the template |

**Op 2, "Geodesic edges"**

| Field | Value | Source |
|---|---|---|
| op type | `Adaptive3d` | the template |
| tool | op 1's tool (`ToolChoice::Exact`) | the ruling ("no larger than op 1's"). An equal tool is the largest tool that D53 allows, and op 1's regions are the centre positions of that very tool (D48), so `Center` keeps the tool inside op 1's boundary |
| stepover, Depth/Pass, feeds | Suggest's values for that tool | `draft_toolpath` |
| `clearing_strategy` | `ContourParallel` | the ruling ("CP edges"); `Adaptive3dConfig::default()` |
| `stock_source` | `FromRemainingStock` ("Rest machining: from previous ops") | the D47 pair; D50 starts it at the left stock top |
| boundary | `{ enabled, DerivedRestRegions { op 1's id }, Center, offset 0 }` | the D53 sentry pair (`session_on`: `offset: 0.0`). The regions are already tool-centre positions, so the draft's tool-diameter offset would shrink them by a whole tool |
| containment | `Center` | "Inside-safe": with an equal tool, D53 refuses nothing and `Center` is exact; `Inside` would inset by the tool radius and can collapse narrow regions (G-BOUNDCOLLAPSE) |
| `contour_each_level` (C4) | `true` (lead ruling at approval; the line lands in a follow-up patch once C4 is committed) | G-RESTDOC, D61/D68: this pair is the G-RESTDOC case (D61: Corne op 5 cut 8.96 mm at a Depth/Pass of 6); "off by default" is the field default, and the template sets the edge op's own value |
| name | "Geodesic edges" | the template |

**"Inside-safe."** The template never writes `Inside` itself. With an equal
tool, `Center` is the safe containment, and no collapse check can run before
op 1 generates, because there are no regions yet. If the operator later
picks a larger edge tool, G-C does the rest: `Center` and `Outside` go grey,
`Refused · see Cut` shows, and `[Use Inside]` writes `Inside` only when
`boundary_collapse_with` says it keeps area.

## 5. No suitable edge tool

The edge tool is op 1's tool, so whenever op 1 has a tool, the edge op has
one that is allowed.

| Case | What happens |
|---|---|
| No project tool fits a 3D Rough | R2: op 1 is added with no tool, and so is the edge op. One toast: the existing R2 text "Added with no tool: … Set a tool on the Cut tab." Both refuse to generate until a tool is set; then G-C guards the edge tool size. |
| MCP states an `edge_tool_index` that is larger than op 1's | The command refuses before it adds anything, with the D53 sentence (`LargerRestConsumer::refusal("Center")`). Nothing is added: one command, one answer. |
| MCP states an equal or smaller `edge_tool_index` | That tool, with `Center` (D53 allows it). |
| The tool has no feeds data | S11: added with no feeds, and the existing feeds toast. |
| No mesh | The menu item is off (§3), and MCP refuses with the draft's model refusal. |

## 6. C4 and W4

- **C4 (`contour_each_level`)**: changed at approval. The template sets
  it ON for the edge op (source: G-RESTDOC, D61/D68), so it never makes a
  pair with a known over-Depth/Pass cut. C4 is not committed yet: the one
  line and its test land in a follow-up patch.
- **W4 (the tabbed Profile)**: the template does NOT offer it. A tabbed
  Profile ends the job: it cuts after every finishing op (the D69 ruling
  "the job ends with a tabbed Profile"). The template makes the roughing
  pair at the start, so a Profile here would sit in the wrong place in the
  plan order. W4's own Caution, `geom.waterline_part_unheld`, names the
  missing Profile and its fix when the job needs one. That signal already
  exists and needs no new text.

## 7. MCP parity

A new MCP tool, `add_geodesic_body_and_edges`, beside `add_bore_from_drill`:

```text
{ setup_index, tool_index?, model_id?, edges?: bool = true, edge_tool_index? }
```

- It sends the same command. `tool_index` and `model_id` mean what they
  mean on `add_toolpath` (Exact, else the rule).
- `edge_tool_index` absent means op 1's tool.
- The reply: `{ body_index, edges_index | null, no_tool, feeds_refusal }`,
  with the `add_toolpath` envelope.
- The wire snapshot gains the param type and the tool description.
- `build_info().features` gains `geodesic_body_and_edges`.

The CLI job file has no `DerivedRestRegions` boundary today (rg: no hit in
`rs_cam_cli/src/job.rs`), so a job cannot state the pair. That is not in
this step; tell me if you want a job form.

## 8. Tests

- Core `--test the_template_adds_the_channel_pair_s7_8`:
  - The command adds two ops in order, with the values of §4.
  - It is one undo step: `Undo` removes both.
  - With `edges: false` it adds op 1 alone, with its rest analysis off:
    no op needs the regions (D44: on demand only).
  - A larger `edge_tool` refuses with the D53 text, and adds nothing.
  - With no fitting tool, both ops are added with no tool.
  - The pair generates on the D53 channel fixture: op 2 is not refused, and
    holds a result. This reuses `tests/common` `height_field`; about 30 s,
    so I would gate it under `heavy-tests`.
- Viz lib:
  - With a mesh, the 3D group shows the item enabled; with none, disabled
    with the reason.
  - The click sends one event, and the controller adds two ops and selects
    op 1.
- Viz `mcp_add_door_parity`: the GUI action and MCP `add_geodesic_body_and_edges`
  give the same two configs (the `every_add_door_takes_one_model_and_tool_q7`
  pattern).
- `mcp_wire_surface_pin` re-blessed for the new tool; `command_registry_completeness`
  for the new row.

## 9. Files

- Core:
  - `session/command.rs`: the row and its args;
  - `session/mutation/toolpath.rs`: the door;
  - `session/mod.rs`: the re-export of the args (I5 edits this file for
    W2b; I will ask first).
- Core tests: one new sentry.
- Viz:
  - `ui/toolpath_panel.rs` (`add_menu_items`: one item);
  - `ui/mod.rs` (`AppEvent::AddGeodesicBodyAndEdges`);
  - `controller/events/toolpath.rs` (the handler, sharing the add
    handler's GUI part);
  - `mcp_server.rs`, `app/mcp/commands*` (the tool);
  - `tests/snapshots/mcp_wire_surface.json`.
- MCP types: `rs_cam_mcp/src/server.rs` (`AddGeodesicBodyAndEdgesParam`,
  the feature token).
- Docs: `FEATURE_CATALOG.md` (the 3D Adaptive row).

## 10. Decisions for the lead

- **D-T1**: one composite command row (recommended) or two `AddToolpath`
  sends from each surface.
- **D-T2**: the edge op is optional through its enable box in the queue
  (recommended), or through a submenu or modal choice.
- **D-T3**: the edge tool is op 1's tool (recommended: the largest tool
  that D53 allows, and the regions are its centres), or the smallest
  fitting tool in the library.
- **D-T4**: MCP `edge_tool_index` refuses a larger tool (recommended),
  or adds it with `Center` and leaves it to the G-C blocker.
