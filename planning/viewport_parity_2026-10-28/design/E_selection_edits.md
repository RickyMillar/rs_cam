# E. Multi-item selection edits: core and MCP side

Design only. HEAD b5aa2a45, 2026-10-28. No product file changes. No cargo run.
Scope: REPORT A.1 SEL-3, SEL-4, SEL-7, SEL-12, SEL-13; B.2; D.1 steps 9 and 11.

## Files read (file:line)

| File | Lines | What it gives |
|---|---|---|
| `crates/rs_cam_core/src/selection/rule.rs` | 23-57, 130-196, 284-337 | `KindFilter`, `DiameterRange`, `Filter` (with `z`, `face`, `diameter`), `Rule` struct, `Rule::layer_rules` |
| same | 346-430, 431-467 | `Selection`, `SelectionEdit`, `SelectionEdit::for_toolpath_model`, `SelectionEdit::add_rules` |
| same | 600-617, 656-710 | `is_default_of`, `holds_default`, `Selection::apply` (O-PICK) |
| `crates/rs_cam_core/src/selection/key.rs` | 24-64, 115-136 | `KEY_MATCH_EPS_MM`, `CAD_ROUNDING_MM`, `MatchKey`, `matches` |
| `crates/rs_cam_core/src/selection/item.rs` | 99-111, 585-600 | `FaceItem` (`plane_z`, `normal`), `Item::z`, `Item::plane_z` |
| `crates/rs_cam_core/src/selection/resolve.rs` | 152-159, 547-600, 692-713, 750-830 | `LostPick`, the frame helpers, `face_frame`, the rule and pick loops |
| `crates/rs_cam_core/src/selection/refusal.rs` | 464-570, 581 | `RefusalCause` (`LostPick`, `AmbiguousPick`, `NoSuchRule`), `SelectionRefusal::new` |
| `crates/rs_cam_core/src/session/command.rs` | 580-650, 2047-2069, 2846-2848 | registry rows (`EditSelection`, `ListItems`), `EditSelectionArgs`, the apply arm |
| `crates/rs_cam_core/src/session/mutation/entities.rs` | 252-385 | `edit_toolpath_selection` (GT-Q8 pre-resolve, O-PICK, Q2, R-A, R-B) |
| `crates/rs_cam_core/src/session/selection_status.rs` | 1500-1560, 1622-1670 | `pick_edit`, `hole_rim_key`, `rule_drop_edit` |
| `crates/rs_cam_core/src/session/rule_offers.rs` | 1-205 | GT-3 `rule_offers`, `Z_HALF_RANGE_MM = CAD_ROUNDING_MM` |
| `crates/rs_cam_core/src/geometry/enriched_mesh.rs` | 308 | `HORIZONTAL_FACE_MIN_NORMAL_Z = 0.95` |
| `crates/rs_cam_mcp/src/server.rs` | 1840-1945, 1948-2096, 2218-2231, 3781-3885 | wire `FilterParam`, `RuleParam`, `MatchKeyParam`, `SelectionEditParam`, `EditToolpathSelectionParam`, `selection_wire_is_the_file_json_p2_7` |
| `crates/rs_cam_viz/src/mcp_server.rs` | 715-730 | `edit_toolpath_selection` tool description |
| `crates/rs_cam_viz/src/app/mcp/commands/selection.rs` | 360-430, 935-1005 | `plan_edit_selection`, `describe_edit_selection`, `a_pick_replaces_the_default_on_both_doors_o_pick` |
| `crates/rs_cam_viz/src/controller/events/mod.rs` | 100-138, 190-300, 515-610 | `apply_selection_edit`, `apply_tab_edit` (own undo step), `AddPick`/`TogglePick`/`PickLayer` arms |
| `crates/rs_cam_viz/src/ui/properties/mod.rs` | 1616-1633 | the toolpath undo snapshot capture |
| `crates/rs_cam_viz/src/ui/properties/panel_apply.rs` | 488-518 | `flush_toolpath_snapshot` |
| `crates/rs_cam_viz/src/state/history.rs` | 15-40, 140-150 | `UndoAction::ToolpathParamChange`, `toolpath_snapshot` |
| `crates/rs_cam_viz/src/controller/tests/selection.rs` | 205-300 | `edit_in_one_step`, `each_selection_section_edit_undoes_in_one_step_p4_3` |
| `crates/rs_cam_viz/src/state/pick.rs` | 1-115 | `DEFAULT_CIRCLE_RANGE`, `PickFilter` |
| `crates/rs_cam_viz/src/ui_command.rs` | 1-60 | the view registry: no row may mutate `ProjectSession` |
| `crates/rs_cam_viz/src/app/input.rs` | 420-450, 540, 600-680 | Escape ends the pick mode (direct state write); no Ctrl binding |
| `crates/rs_cam_viz/tests/mcp_wire_surface_pin.rs` | 1-80 | what the pin holds (`name` + `inputSchema`) |
| `FEATURE_CATALOG.md` | 123, 125, 126, 130 | selection claims ("one undo step", MCP edits, O-PICK, pick mode) |
| `planning/geometry_selection_2026-10-16/DESIGN.md` | 30-55, 845-856 | rule 4 "No silent fallback"; R2 (a topological id is never the key) |
| `planning/geometry_selection_2026-10-16/PLAN.md` | 265-275, 835-842 | D2/D13, O-PICK |

## 0. Four facts that change the brief

1. **`Rule` is a struct, not an enum.** `Rule { source: SourceRef, filter: Filter }`
   (`rule.rs:284`). "Same Z" exists: `Filter::z: Option<ZRange>` for STEP edge
   items and `FaceFilter { facing, z }` for faces, both in the setup frame.
   GT-3 `ProjectSession::rule_offers` (`rule_offers.rs:59`) already derives
   "same diameter", "up faces at this Z" and "items of this kind at this Z" from
   one picked item. The tolerance is `CAD_ROUNDING_MM` (L-13) for both. So
   SEL-7 needs **no new rule kind and no new project-file key**.
2. **A viewport pick is not its own undo step today.** `apply_selection_edit`
   (`controller/events/mod.rs:109`) pushes no `UndoAction`. The step comes from
   the inspector snapshot: `draw_toolpath_selection` captures it once
   (`ui/properties/mod.rs:1624`), and `flush_toolpath_snapshot` pushes it only
   when the GUI selection leaves the toolpath (`panel_apply.rs:489`). N clicks on
   one toolpath give ONE undo step at navigate-away. The test helper
   `edit_in_one_step` (`controller/tests/selection.rs:223`) does the flush by
   hand, so no test sees this. `FEATURE_CATALOG.md:123` says "one undo step"
   per edit; that claim is true per inspector visit only.
3. **No door checks that a pick key names an item.** `Selection::apply` stores
   any key. `resolve` reports a key with 0 or 2+ matches as `LostPick`
   (`resolve.rs:821`), and status and generation refuse later. So MCP
   `add_pick` of a wrong key succeeds and stores a lost pick.
4. **The wire converts one way only.** `From<SelectionEditParam> for
   SelectionEdit` exists (`server.rs:2084`); the reverse does not. A new core
   variant compiles with no wire variant. That breaks the F166 rule of
   `crates/rs_cam_mcp/CLAUDE.md` and the operator rule "compiler-enforced >
   sentry > doc".

## 1. Batch edits

### 1.1 Compare

| Question | `AddPicks` / `RemovePicks` | `Batch(Vec<SelectionEdit>)` |
|---|---|---|
| Undo | One command, one `ToolpathParamChange` (after the 4.2 fix) | Same |
| O-PICK | One test: `is_default_of` → `from_picks(keys)` | Order decides: the first `AddPick` replaces, a later `AddRule` adds; a mixed batch is hard to state |
| GT-Q8 | `rule_drop_edit` takes the key list once | `edit_toolpath_selection` must unwrap, run `rule_drop_edit` per inner `RemovePick` against the state in between, and re-wrap |
| Duplicate keys | `matches` dedups, as `AddPick` does | Same |
| Wire schema | Two flat variants | Recursive enum; `#[schemars(inline)]` cannot inline a recursive type (the tool schema has no `$defs`) |
| Box select needs | Exactly this | More than this |

**Decision:** `AddPicks(Vec<MatchKey>)` and `RemovePicks(Vec<MatchKey>)`. No `Batch`.

### 1.2 Core types (`crates/rs_cam_core/src/selection/rule.rs`)

```rust
pub enum SelectionEdit {
    Replace(Box<Selection>),
    AddRule(Rule),
    RemoveRule(usize),
    AddPick(MatchKey),
    RemovePick(MatchKey),
    /// Add every key as one edit (box select, SEL-4). O-PICK: on a
    /// selection that is still the op default, the keys replace it.
    /// A key that `matches` an earlier key of the list is dropped.
    AddPicks(Vec<MatchKey>),
    /// Remove every key as one edit. Each key acts as `RemovePick`.
    RemovePicks(Vec<MatchKey>),
    Reset,
}

// for_toolpath_model (rule.rs:399): two new arms, no wildcard.
SelectionEdit::AddPicks(keys) => SelectionEdit::AddPicks(
    keys.into_iter().map(|k| k.for_toolpath_model(toolpath_model)).collect()),
SelectionEdit::RemovePicks(keys) => SelectionEdit::RemovePicks(
    keys.into_iter().map(|k| k.for_toolpath_model(toolpath_model)).collect()),

// Selection::apply (rule.rs:670): two new arms.
SelectionEdit::AddPicks(keys) if keys.is_empty() => {
    return Err(SelectionRefusal::new(None, RefusalCause::NoKey));
}
SelectionEdit::AddPicks(keys) if self.is_default_of(op_default) => {
    *self = Self::from_picks(dedup_by_match(keys));
}
SelectionEdit::AddPicks(keys) => {
    for key in keys { self.add_pick(key); } // the body of the AddPick arm, moved into a fn
}
SelectionEdit::RemovePicks(keys) if keys.is_empty() => {
    return Err(SelectionRefusal::new(None, RefusalCause::NoKey));
}
SelectionEdit::RemovePicks(keys) => {
    for key in keys { self.remove_pick(key); } // the body of the RemovePick arm
}
```

`add_pick` and `remove_pick` are private fns that the single arms call too, so
one body holds each rule.

### 1.3 Session door (`crates/rs_cam_core/src/session/mutation/entities.rs:285`)

```rust
// GT-Q8, generalised: selection_status.rs:1622
pub(crate) fn rule_drop_edit(
    &self, tc: &ToolpathConfig, target: SelectionTarget, keys: &[MatchKey],
) -> Result<Option<SelectionEdit>, SessionError>;
// names_key = |k| keys.iter().any(|key| k.matches(key)); a rule whose members
// hold ANY key becomes picks of its members less ALL keys.

// R5 door: THE resolution, no second matcher (the pattern of rule_offers.rs:80).
pub(crate) fn check_keys_name_items(
    &self, tc: &ToolpathConfig, target: SelectionTarget, keys: &[MatchKey],
) -> Result<(), SessionError>;
// resolve_spec_frames(tc, &Selection::from_picks(keys.to_vec()), &ctx,
//   FaceAccess::Peek, target_uses(tc, target)); the first LostPick refuses:
// SessionError::Selection(SelectionRefusal::new(None,
//   RefusalCause::PickNamesNoItem { key: Box::new(key), matches }).for_op(label))
```

Door order in `edit_toolpath_selection`:

1. `AddPick(k)` and `AddPicks(ks)`: `check_keys_name_items` first. A refusal writes nothing.
2. `RemovePick(k)` and `RemovePicks(ks)`: `rule_drop_edit(tc, target, keys)`; `None` gives the plain edit.
   A remove of a key that names no item stays legal, because the Geometry tab removes a lost pick row.
3. `Selection::apply`, then Q2 / R-A / R-B as today. `adds_model_area(&before, &next)` compares the two
   selections, so a batch needs no change there.

New cause in `refusal.rs`: `PickNamesNoItem { key: Box<MatchKey>, matches: usize }`
(0 = no item, 2+ = equal keys) and `NoKey`. Both are edit-time causes, as
`NoSuchRule` is: add them to the "status = generation, except" list of
`selection/CLAUDE.md`. Fix text: "Copy the key from `list_model_items`".

**Break (state it in the commit):** MCP `add_pick` of a key that names no item
now refuses. It stored a lost pick before. The GUI never sends such a key.
`Replace` (the file JSON and the CLI job) keeps no check: a file may hold a
lost pick after a drawing change, and status names it.

### 1.4 Project file and CLI

None. `SelectionEdit` is not stored; the `Selection` is, and its shape does not
change. The CLI sends `Replace` only (`rs_cam_cli/src/job.rs:1295`).
`format_version` stays 3.

### 1.5 MCP wire (`crates/rs_cam_mcp/src/server.rs:2067`)

The single forms stay; the list forms are additive. No break needed.

```rust
pub enum SelectionEditParam {
    Replace(SelectionParam),
    AddRule(RuleParam),
    RemoveRule(usize),
    AddPick(MatchKeyParam),
    RemovePick(MatchKeyParam),
    /// Pick every item of the list in one edit (one undo step). Each key
    /// must name one item, or the whole edit refuses and writes nothing.
    AddPicks(Vec<MatchKeyParam>),
    /// Remove every item of the list in one edit.
    RemovePicks(Vec<MatchKeyParam>),
    #[default]
    Reset,
}

// NEW, F166: the reverse direction, exhaustive. The next core variant
// does not compile until the wire carries it.
impl From<rs_cam_core::selection::SelectionEdit> for SelectionEditParam { /* match, no `_` */ }
// needs From<Selection> for SelectionParam, From<Rule> for RuleParam,
// From<Filter> for FilterParam (full destructure, no `..`, F118).
```

JSON: `{"add_picks": [<key>, <key>]}`. Core serde gives the same spelling
(`rename_all = "snake_case"`), so the p2_7 "wire = file JSON" rule holds.
Add a `build_info().features` token `selection_batch` (a new capability).

## 2. "Select similar" (SEL-7)

### 2.1 The rule kinds exist

```rust
// rule.rs:130 today, unchanged by this design.
pub struct Filter {
    pub layers: Vec<String>,
    pub kinds: KindFilter,
    pub diameter: Option<DiameterRange>, // circles only
    pub face: Option<FaceFilter>,        // faces only: Facing + plane ZRange
    pub z: Option<ZRange>,               // STEP edge items only, setup frame
}
```

| Brief asks for | Already written as | Offer in `rule_offers` | Resolver path |
|---|---|---|---|
| `AtZ` for STEP rings, edges, points | `Filter { kinds: Only([k]), z: Some(z ± CAD_ROUNDING_MM) }` | yes (`rule_offers.rs:127`) | `Filter::admits` `z_ok` ← `item_z_in_setup_frame` |
| `AtZ` for faces | `FaceFilter { facing: Up, z: Some(..) }` | yes, up faces (`:113`) | `face_ok` ← `face_in_setup_frame` |
| `AtZ` for drawing items | meaningless: every drawing item is in the drawing plane, and `Filter::z` admits no drawing item | "on layer L", "all K on this model" | `layer_ok`, `kind_ok` |
| Same diameter | `DiameterRange { nominal: d, tolerance: CAD_ROUNDING_MM }` | yes (`:100`) | `diameter_ok` |
| `FaceNormal` | not written | — | — |

**`FaceNormal`: recommend no.** A face that a 2.5D op cuts must be flat:
`view` refuses `FaceNotFlat` under `HORIZONTAL_FACE_MIN_NORMAL_Z = 0.95`. On a
3-axis router "parallel faces" is "up faces" or "down faces" (`Facing`). Two
small additions in `rule_offers` cover it, with no new key:

```rust
// rule_offers.rs, Shape::Face arm: two more offers.
// "All up faces on this model": FaceFilter { facing: Up, z: None }
// "All down faces at Z {z}" (boundary target only; view refuses a down Regions face):
//   FaceFilter { facing: Down, z: Some(z_range(face.plane_z)) }
```

Project-file key: none new. `Filter.face.z` and `Filter.z` are stored today.

### 2.2 The gaps are the entry points

1. **GUI:** the offers are only in the Geometry tab row "Rule…" menu
   (`selection_section.rs:729`, `SelectionUi::offers_for`). The viewport has no
   entry. One line: in pick mode, a right-click on an item opens an egui context
   menu. Row 1 is the current layer rule (`PickLayer`). The next rows are
   `rule_offers(tc, target, key)`. A row sends its `RuleOffer::edit` through
   `AppEvent::EditToolpathSelection`.
2. **MCP:** an agent composes the same `Filter` by hand from `list_model_items`
   (`key.z`, `measure`) and sends `{"add_rule": ..}`. The stored rule is the
   same; the agent does not see the label or the count. Option: a `Query` row
   `RuleOffers` → tool `list_rule_offers(index, target?, key)` that returns
   `{label, rule, count, edit}`. See question 7.2.

MCP `add_rule` wire shape for "same Z as this ring" (exists today):

```json
{"index": 2, "edit": {"add_rule": {"source": "toolpath_model",
  "filter": {"kinds": {"only": ["loop"]}, "z": {"min": 19.99, "max": 20.01}}}}}
```

## 3. Keyboard (SEL-13)

| Key | Act | Route | Parity |
|---|---|---|---|
| Ctrl+A in pick mode | add the rule of every admitted kind | `AppEvent::EditToolpathSelection { edit: add_rules(..) }` → `Command::EditSelection` | MCP `add_rule` / `replace` with the same rules |
| Escape | end the pick mode | exists (`input.rs:430`), a view-state write | none needed: MCP has no pick mode |
| clear (Reset) | no key | the Selection section Reset button exists | MCP `"reset"` |
| Tab ("select other") | cycle the candidates under the pointer | GUI only; the chosen key goes through `pick_edit` | MCP `list_model_items { near }` gives the same list |

A `UiCommand` is the wrong home for Ctrl+A. The view registry declares no row
that mutates `ProjectSession` (`ui_command.rs:47`). Ctrl+A is a core command,
so parity holds by construction through `edit_toolpath_selection`.

"Every item of kind K on this model" is one rule. Generalise `layer_rules`, do
not copy it:

```rust
// rule.rs: layer_rules becomes a thin call of this fn.
pub fn kind_rules(
    source: SourceRef,
    layer: Option<&str>,          // None = every layer
    kinds: &[ItemKind],           // PickFilter::effective_kinds
    circle: Option<DiameterRange>, // PickFilter::circle; circles get their own rule
) -> Vec<Rule>;
```

**O-PICK and Ctrl+A.** Under the default, `add_rules` replaces `Served` by
`Only(kinds)`. That is a real change: a rule that names kinds refuses
`KindNotCut`, `Served` notes and skips (D2/D13). Recommend: under the default,
Ctrl+A writes nothing and shows "The default already selects every item that
the op cuts". After a pick, Ctrl+A adds the rule (one `AddRule` or one
`Replace`).

## 4. Undo (SEL-12)

1. A batch is one `Command::EditSelection`, so the core change is atomic.
2. The GUI step is NOT one per act today (fact 0.2). Fix it in the controller:

```rust
// controller/events/mod.rs: move the snapshot logic of apply_tab_edit
// (lines 263-305) into one helper. apply_tab_edit and apply_selection_edit call it.
fn push_act_step(
    &mut self,
    toolpath_id: ToolpathId,
    before: ToolpathParts,   // (operation, dressups, boundary, feeds_provenance)
    after: ToolpathParts,
);
// 1. Push the panel edits since the snapshot as their own step (if any moved).
// 2. Push before -> after as the act's step.
// 3. Move toolpath_snapshot to `after`.
```

Then every viewport selection act (click, shift-click, box, right-click rule,
Ctrl+A, offer row) is one `ToolpathParamChange`. D11 holds: the variant carries
the boundary on both sides. This change updates `FEATURE_CATALOG.md:123` from
"true per visit" to "true per act".

3. The tree `Selection::Face/Faces` has no undo. **Confirm: leave it.** It is
   GUI view state; it writes no op and no file. An undo step for it would push
   view noise between real edits on the stack.

## 5. Parity tests

Names follow the `<claim>_<tag>` convention. Tags: `sel4`, `sel7`, `sel12`, `sel13`.

| Test | Where | Claim |
|---|---|---|
| `a_batch_pick_replaces_the_default_once_o_pick` | `selection/rule.rs` tests | `AddPicks([a,b])` on the default gives `from_picks([a,b])`; on picks it adds; a dup key adds once |
| `an_empty_batch_refuses_sel4` | same | `AddPicks([])` refuses `NoKey` |
| `a_batch_with_a_lost_key_writes_nothing_sel4` | `crates/rs_cam_core/tests/a_batch_pick_is_one_edit_sel4.rs` | one bad key: `PickNamesNoItem`, the stored selection is byte-equal before and after; also for single `AddPick` |
| `a_batch_remove_turns_each_rule_into_picks_once_gt_q8` | same file | `RemovePicks([a,b])` on a CUT rule gives the members less a and b, no exclude |
| `the_down_face_offer_holds_the_down_faces_at_its_z_sel7` | `crates/rs_cam_core/tests/rule_offers_read_one_picked_item_gt3.rs` (extend) | new offers match the items that share the fact |
| `a_box_and_an_mcp_batch_give_one_selection_status_sel4` | `rs_cam_viz/src/app/mcp/commands/selection.rs` tests | `stored(&gui) == stored(&mcp)` and equal `selection_status` after `edit(AddPicks)` vs `wire_edit({"add_picks": ..})` |
| `an_mcp_batch_with_a_lost_key_changes_nothing_sel4` | same | the reply is an error; `stored` is unchanged |
| `a_batch_replaces_the_default_on_both_doors_o_pick_sel4` | same | the pattern of `a_pick_replaces_the_default_on_both_doors_o_pick` |
| `an_mcp_rule_and_a_gui_offer_give_one_selection_sel7` | same | the `RuleOffer::edit` of a STEP ring at Z 20 = wire `add_rule` with `filter.z` |
| `every_viewport_selection_act_is_one_undo_step_sel12` | `controller/tests/selection.rs` | two clicks with NO flush: two `Undo`s, each restores one click |
| `a_box_is_one_undo_step_sel4` | same | one `AddPicks` of two keys: one `Undo` restores both |
| `ctrl_a_under_the_default_writes_nothing_sel13` | same | notice, no edit |
| `selection_wire_is_the_file_json_p2_7` | `rs_cam_mcp/src/server.rs:3781` (extend) | add the two variants to the edit list; add a no-wildcard `match` on `SelectionEdit` that builds one sample per variant |
| `mcp_wire_surface_pin` | `rs_cam_viz/tests/` | re-bless `tests/snapshots/mcp_wire_surface.json` with `RS_CAM_UPDATE_WIRE_SNAPSHOT=1`; the `edit_toolpath_selection` schema gains `add_picks` and `remove_picks`; read the diff |
| `an_unknown_mcp_field_is_refused_f26` | `rs_cam_mcp` | no change; new variants live in an enum that has `deny_unknown_fields` |

The box rectangle test itself (egui harness, `pick_items_in_rect`) belongs to
the GUI design of step 9, not here.

## 6. Commit sequence

Each commit builds and passes alone. Run the folder sentries, the crate tests,
then clippy.

| # | Commit | Files | Gate |
|---|---|---|---|
| 1 | core: `AddPicks`/`RemovePicks`, the key check, GT-Q8 for a list | `selection/rule.rs`, `selection/refusal.rs`, `session/selection_status.rs` (`rule_drop_edit`, `check_keys_name_items`), `session/mutation/entities.rs`, `selection/CLAUDE.md`, `session/CLAUDE.md`, `crates/rs_cam_core/tests/a_batch_pick_is_one_edit_sel4.rs` | `--lib selection`, the new test, `a_pick_replaces_the_default_rule_o_pick`, viz `--lib`, `rs_cam_mcp` tests (no exhaustive core match outside core, so viz and mcp still build) |
| 2 | core: two face offers | `session/rule_offers.rs`, `tests/rule_offers_read_one_picked_item_gt3.rs` | that test |
| 3 | mcp: wire variants, reverse `From`, feature token | `crates/rs_cam_mcp/src/server.rs`, `crates/rs_cam_mcp/CLAUDE.md`, `crates/rs_cam_viz/src/mcp_server.rs` (description), `crates/rs_cam_viz/tests/snapshots/mcp_wire_surface.json`, `FEATURE_CATALOG.md:125` | `rs_cam_mcp` tests, `mcp_wire_surface_pin`, viz MCP parity tests |
| 4 | viz: undo per act | `controller/events/mod.rs` (`push_act_step`), `controller/tests/selection.rs`, `FEATURE_CATALOG.md:123` | viz `--lib controller` |
| 5 | viz: right-click offers, Ctrl+A | `ui/pick_bar.rs`, `app/viewport.rs`, `app/input.rs`, `rule.rs` (`kind_rules`), shortcuts window text, `FEATURE_CATALOG.md:130` | viz `--lib`, `pick_mode_picks_by_kind_p4_5` |
| 6 | viz: parity tests | `app/mcp/commands/selection.rs` tests | viz `--lib selection` |

Commit 1 states the break: "MCP `add_pick` of a key that names no item now
refuses `PickNamesNoItem`". Commit 3 needs no break line (additive).

Tool description, new text after the `remove_pick` clause: "`{\"add_picks\":
[<key>, ...]}` and `{\"remove_picks\": [<key>, ...]}` (one edit, one undo
step; a key that names no item refuses the whole edit and writes nothing)".

Invariants kept:

- Every write is `ProjectSession::apply(Command::EditSelection)`. No new door.
- Rules before indices (R2): no rule, offer or key holds a `FaceGroupId` as its
  identity; `MatchKey.brep` stays a tie-break only.
- No silent fallback (rule 4): a bad key refuses; an empty list refuses; a
  dropped duplicate is not a fallback (the same item, once).
- O-PICK lives in `Selection::apply` only. GT-Q8 lives in `rule_drop_edit` only.
- One stored spelling per item (P2.7): `for_toolpath_model` maps each key.

## 7. Open questions for the lead

| # | Question | Recommendation |
|---|---|---|
| 7.1 | A batch with one bad key: refuse all, or apply the rest with a note? | **Refuse all.** Rule 4 forbids a silent skip, and a note on a partial write leaves the agent to diff the result. Apply the same check to single `AddPick` (a stated break). The GUI box drops keys that match 2+ items in its own item set before it sends, and its toast names the count. |
| 7.2 | Add an MCP `list_rule_offers` query row? | **Yes, if the lead wants the label and the count on the wire** (number parity, operator rule 2026-09-23). Without it, parity of the stored rule holds, but the agent must derive `z ± 0.01` itself. Cost: one `Query` row, one wire param, one handler. |
| 7.3 | `AtZ` tolerance | **Settled:** `Z_HALF_RANGE_MM = CAD_ROUNDING_MM` (`rule_offers.rs:40`, L-13). No new constant. |
| 7.4 | `FaceNormal` rule | **No.** `FaceNotFlat` refuses every face that is not near-horizontal; `Facing` + `FaceFilter::z` carry "same plane". |
| 7.5 | Ctrl+A at all, given O-PICK? | **Keep it, small.** A no-op with a notice under the default; one `add_rules` edit after a pick. Value is low (SEL-13 ★). Drop it if step 11 runs short. |
| 7.6 | "Select other" cycle: core or GUI? | **GUI only.** The cycle chooses a key before the click; the edit is the ordinary `pick_edit` result. Step 10's ID list feeds it. |
| 7.7 | Undo per act (4.2): accept the change of click undo from "per visit" to "per act"? | **Yes.** It makes the catalogue claim true and gives the box one step. |
| 7.8 | Plain left drag in pick mode: add or toggle? | **Add.** A toggle per key needs per-key resolution and gives a mixed `AddPicks` + `RemovePicks`, so two commands or one `Replace`. Ctrl+drag removes (`RemovePicks`). |
