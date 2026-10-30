# Toolpath inspector: execution plan (2026-10-30)

Scope: the toolpath inspector and the toolpath rail. The UI result and the
code structure behind it. Input: `FINDINGS.md` (F1-F14, P1-P13, the design
rules) and the review page
<https://claude.ai/artifact/Gk47TzZUk4FnLZkRxk35e2>.

Owner-facing decision: R1-R5 RULED by the operator on 2026-10-30, all as proposed. Next action: W0.

## Why the last passes did not hold

The premium programme (UP1-UP8) built the parts, but it could not move a
control. The declutter phase moved controls, but it did not close the doors.
After 2026-10-13, 110 commits went into `ui/properties/`, and each new feature
used a raw row or a raw message beside the kit. This plan closes the doors
FIRST (W1), then moves the panel onto them (W2-W5), then makes the old doors
fail to compile or fail a sentry (W6).

## 1. Rulings (all ruled 2026-10-30, as proposed)

| # | Ruling | Proposal | Blocks |
|---|---|---|---|
| R1 | Feeds seed on empty | On op add and on tool change, write Suggest into EMPTY feed/plunge/spindle only, mark ⚡ "suggested", never overwrite a typed value, never lock. A carve-out from F.5 and from "no underived defaults" (a Suggest value is derived). | W3 Feeds |
| R2 | Tab set | Tool in the header. Tabs: Geometry, Passes, Feeds, Heights, Linking. A tab with no control for the op is not drawn. Dressup merges into Linking. Breaks the pinned order (sentry `_d7`). | W2, W3 |
| R3 | Reach map leaves the inspector | The dock and legend own it; the inspector keeps one row on the Geometry tab of a finishing op. | W2 |
| R4 | Inspector default width | 340 px (max 420 unchanged). | W2 |
| R5 | Scope | Toolpath inspector + rail now. Setup-side panels (machine 33 raw rows, tool 20, stock, setup) take the same doors in a second pass. | all |

## 2. Rules for the run

- Each package deletes something. A package that only adds is refused.
- The lead decides and verifies by path. Agents implement and never commit.
- One cargo job at a time through `scripts/cargo_lane.sh`. Agent test runs
  take a `timeout`.
- The gate of each package: its sentries, `cargo test -p rs_cam_viz --lib`,
  `cargo test -p rs_cam_mcp`, the folder sentries of each touched folder,
  clippy. No core full gate without asking.
- GUI and MCP report the same state (number parity). The run-state and the
  diagnostic placement are core functions that both surfaces read.
- The operator looks at the GUI on screen after W2 and after W3. Rebuild the
  release GUI first and check it was built from HEAD.

## 3. Waves

### W0 — Instruments first (commit before any change)

- **Bad-state fixtures.** Extend the IC1 fixture set: no tool, tool with no
  feeds, wrong tool kind, refused generation, reach map on, boundary on,
  stale after edit. Render every tab of a 2D, a 3D-rough and a 3D-finish op in
  each state.
- **Baseline counts** (recorded, not yet asserted): times each diagnostic id
  paints per screen; face length of each painted message; raw
  `ui.label` / `ui.horizontal` sites under `ui/properties/`; label widths in
  use.
- Deletes: the "+3" line caps (replaced in W6 by the new asserts).

### W1 — The doors (code structure; no visible change yet)

1. **Core `Diagnostic`:** `message` splits into `face` (≤ 60 chars, plain
   words) and `detail` (the derivation). A new `anchor: Anchor` names the
   owner: `Field(ParamKey)`, `Operation`, `Note`, `Project`, `View`. The
   `Scope` stays (which object). `OWNER_TAB` folds into `Field` (the field
   knows its tab). MCP returns `face`, `detail`, `anchor`. Breaking change
   to the wire, stated in the commit (no-legacy ruling).
2. **Core run-state:** one function `toolpath_run_state(session, id) ->
   RunState { NotReady, Ready, Generating, Done, OutOfDate, Failed }` plus the
   next action. A refusal belongs to the inputs that produced it, so it
   clears when they change. GUI header and MCP `list_toolpaths` read it.
3. **viz `ui/components/`:**
   - `PropRow` — label (`LABEL_COL_WIDTH`), control (`ROW_ACTION` high),
     unit as an inner suffix, read-only variant, field state outline from
     the diagnostics anchored to its key, the detail on hover.
   - `IssueRow` — icon, face, one Fix; max 3 in a stack.
   - `NotesCounter` — "i n" with a popover.
   - `TabStrip` — never wraps; shrinks to icons + tooltip, then a » menu.
4. **One placement function:** `diagnostics::place(&[Diagnostic]) ->
   Placement { by_field, issues, notes }`. It is the only reader of the
   list in the inspector.
- Deletes: nothing visible; the old renderers stay until W2-W3 move off them.

### W2 — The shell (header, issues, tabs) — operator looks after this

- Header: name; Tool dropdown (the only tool control; project tools, refused
  ones grey with the reason, then "Add from library…", "New tool…"); run-state
  word and one button from `RunState`.
- The first fitting tool added to the project binds an op with no tool.
- Issue stack under the header; notes counter in the status line.
- Op-aware tab set (R2) from the core op catalog; empty tabs are not drawn.
- Width 340 (R4).
- Deletes: the refusal line, the per-tab footer (findings, "Gates: …", hints
  expander), the Cut-tab Tool row and its no-tool label, the state toasts
  (a toast stays only for an event), the panel reach checkbox and its 4 rows
  (R3; one Reach row on Geometry), "planned/modulated" on the face.

### W3 — The tabs, one package each

| Package | Content | Deletes |
|---|---|---|
| W3a Geometry | Selection, floor and machining boundary in `PropRow`s. 3D ops: Surface row + boundary rows + Reach row. | Boundary section on the Cut tab; the lone "Pick faces" tab. |
| W3b Passes | Every op editor in `operations/` on `PropRow`. Entry style lives on Linking only. | Raw label+combo rows (`surface_3d.rs`, `boundary_2d.rs`, `finishing.rs`, `engrave.rs`, `project.rs`); the duplicate Entry Style. |
| W3c Feeds | R1 seed on empty with ⚡; a ⚡ button beside a typed value that differs; the recommendation table moves to the feeds modal / Details. | "set a value" boxes beside a recommendation; the face recommendation table; the fake 6 mm vendor-data fallback. |
| W3d Heights | One grid; the diagram reads the same numbers as the grid; line names on hover. | The second, misaligned grid; CZ/RZ/FZ/TZ codes on the face. |
| W3e Linking | Entry/exit, links, arc fit, dogbone (where it applies). | Dressup tab; "n/8 dressups active". |

### W4 — Messages

- Every adapter in `rs_cam_core/src/diagnostics/adapters/` (`from_feeds`,
  `from_generation`, `from_static_checks`, `from_tool_load`,
  `from_preconditions`, the rest) and every viz-side message: a `face`
  ≤ 60 chars in plain words, the right category, and an anchor.
- Reach grid-floor and over-statement notes: legend hover only.
- Deletes: developer prose on the face ("repo rule, unsourced", "AT OR
  BELOW", "common scale 0.688…"); category prefixes.

### W5 — Diagrams, rail, empty state

- Drill draws the picked holes. An op with no faithful picture draws none.
- Rail: "Generate All" stays a button (status is a separate word); row tool
  names do not truncate the op name.
- Empty project: the Getting started steps tick and link.
- Deletes: placeholder diagrams; the static Getting started text.

### W6 — Close the old doors (enforcement)

- `the_inspector_text_is_bounded_ic1` gets the W0 bad-state fixtures and
  asserts: each diagnostic id paints at most once per screen; no painted
  face over 60 chars; a truncated row counts its full text.
- Source sentry: under `ui/properties/`, no `ui.label(` / `ui.horizontal(`
  outside `ui/components/` (an allow-list that only shrinks), and no
  `Diagnostic.detail` read outside the placement function.
- Where the type system can hold it, make it hold it: the diagnostic text is
  not a `String` a panel can reach; `PropRow` is the only constructor of a
  field row in the inspector.
- Update `ui/CLAUDE.md`, `ui/properties/CLAUDE.md`,
  `ui/components/CLAUDE.md`, `diagnostics/CLAUDE.md`; delete stale doc
  comments (e.g. `state/runtime.rs:992` "The Geometry tab… print this one
  string").

### W7 — Code structure of the inspector

Runs alongside W2-W3 as each tab moves, not as a separate rewrite:
- `properties/toolpath_panel.rs` (1832 lines) splits into `header.rs`,
  `issues.rs` and one file per tab under `properties/tabs/`.
- `properties/mod.rs` (1898 lines) keeps `draw` and the snapshot/inputs
  builders; the panel-apply and the setup-side draws move out.
- `tab_badges.rs` (845 lines) is deleted when the placement function
  replaces it.
- `operations/shape_diagrams.rs` loses the placeholder pictures.

## 4. Done means

- The W0 fixtures pass the W6 asserts on every tab of every op family.
- No diagnostic id paints twice on one screen; no face is over 60 chars.
- Zero raw rows under `ui/properties/` (allow-list empty), or each left
  entry is named with a reason.
- GUI and MCP agree on run-state and on each diagnostic's face and anchor.
- The operator has looked at the GUI on screen after W2 and after W3 and
  signed off.
