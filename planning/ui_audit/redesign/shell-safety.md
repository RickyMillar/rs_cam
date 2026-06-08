# Redesign SPEC — Shell, safety wiring & cross-cutting cleanup

Domain owns findings: **P6-003**, **P6-001**, **P1-004**, **P2-005**.

Two of these (P6-003 wiring, P6-001 delete) require a **code / data-model change**, not
just layout. They are flagged inline. The other two (P1-004, P2-005) are spec-only
boundary decisions that the UI must reflect.

Legend for change class:
- `[CODE]` — requires a Rust / data-model change to be correct. Spec describes target.
- `[SPEC]` — IA decision only; no functional change, only which surface is authoritative.

---

## P6-003 — Fixture Z must reach collision checking, or the Z fields must not exist

**This is the highest user-risk finding and is a FUNCTIONAL gap.**

### Confirmed current state (evidence)
- `CollisionCheckRequest` is exactly `{ toolpath, tool, mesh }`
  (`crates/rs_cam_core/src/compute/collision_check.rs:12-16`). No fixture field.
- `run_collision_check` builds a spatial index from `request.mesh` only and the tool
  assembly — fixtures never enter the holder/shank check
  (`collision_check.rs:54-70`).
- Both call sites pass only the model mesh:
  - `crates/rs_cam_core/src/session/compute.rs:1659-1664`
  - `crates/rs_cam_viz/src/compute/worker/helpers.rs:99-106`
- Fixtures are consumed **only** as an XY footprint rectangle that ignores
  `origin_z` / `size_z` (`crates/rs_cam_core/src/session/mod.rs:356-363`).
- Yet `fixture-properties-panel` (`crates/rs_cam_viz/src/ui/properties/setup.rs`) lets
  the user edit `set-fixture-position` (XYZ), `set-fixture-size` (XYZ), and
  `set-fixture-clearance` and persists them.

The Z extent is editable, persisted, and renders a 3D box — but moves nothing in the
safety check. **A holder crashing a clamp is never flagged.** The Z fields are a
false-assurance affordance.

### Decision: WIRE IT (do not amputate)

We keep fixture Z and make it real. Amputating the Z fields would make fixtures a pure
2D keep-out and lose the one mechanism that can catch the most dangerous, hardest-to-spot
crash (holder/shank into a low clamp). The redesign principle P6 ("every control earns
its place") is satisfied by **wiring the orphan**, not removing it.

### Required code/data-model change `[CODE]`

1. **Extend the request** — `CollisionCheckRequest` gains a fixtures slot:
   ```
   pub struct CollisionCheckRequest<'a> {
       pub toolpath: &'a Toolpath,
       pub tool: ToolDefinition,
       pub mesh: &'a TriangleMesh,
       pub obstacles: &'a [CollisionObstacle],   // NEW
   }
   ```
   `CollisionObstacle` is an AABB-or-mesh-box derived from each **enabled** fixture's
   `origin_{x,y,z}` + `size_{x,y,z}` + `clearance` (clearance inflates all six faces, so
   the box used for collision is the same dimensions the 3D viewport draws plus
   clearance). Keep-out zones may also feed this list later (full-Z box) — out of scope
   for this domain but the slot is shaped to accept them.

2. **Check against obstacles** — `run_collision_check` checks the holder/shank assembly
   against the model mesh **and** every obstacle box. Reuse the existing interpolated
   move walk; add a second collision target. A collision against an obstacle is reported
   with a distinct `CollisionKind::Fixture { fixture_id }` so the UI can attribute it.

3. **Populate at both call sites** — `session/compute.rs:1659` and
   `worker/helpers.rs:99` build `obstacles` from the active setup's enabled fixtures.
   This is the single place fixture Z geometry becomes load-bearing.

### UI consequence (this domain owns the affordance)

The fixture panel does not need new fields — it needs its existing Z fields to be
**legible as safety inputs**, and it needs a point-of-need readout that proves the check
ran. P5 ("UI wins, not prose"): replace any "fixtures are decorative" silence with an
affordance that shows whether the holder check covered this fixture.

- Each fixture row in the panel carries a **clearance-check status pill** (see mockup):
  - `◇ not checked` (grey) — collision check not run since last edit.
  - `✓ clear` (green) — last holder check found no holder/shank intrusion into this
    fixture's box.
  - `✗ hit` (red, clickable) — collision; clicking seeks the viewport to the marker.
- The Z fields (`Z`, `Height`, `Clearance`) get a small **shield glyph** column header
  marking them as the inputs the safety check consumes — distinguishing them from the
  XY-only keep-out form (which has no shield), so the two near-clone panels (P4 from the
  surface record) now read as different jobs.
- The `Check Holder Clearance` command (menu-bar `Simulation` submenu,
  `menu_bar.rs:202`) gains a **mirror trigger** as a point-of-need button at the top of
  the fixture panel: `Run holder clearance ▸`. One authoritative event
  (`AppEvent::RunCollisionCheck`); the panel button is an explicit mirror (P1).

**Provenance (P7):** the status pill is computed from the *actual last collision result*
keyed by `fixture_id`, never recomputed from a fresh guess — same "where did this come
from" discipline the feeds domain applies. Stale (geometry edited after last check) =
`◇ not checked`, never a falsely-green pill.

---

## P6-001 — Edit › Delete Selected: wire it (one authoritative path)

### Confirmed current state
- `menu_bar.rs:144-147` hardcodes
  `ui.add_enabled(false, Button::new("Delete Selected").shortcut_text("Del"))` —
  permanently disabled, emits nothing, advertises a `Del` shortcut.
- The **working** delete path is keyboard-only: `app/input.rs:439-446` pushes
  `AppEvent::RemoveToolpath(id)` on `Delete`/`Backspace` when a Toolpath is selected.

So the capability exists and is reachable; the menu item is a dead, misadvertised mirror.

### Decision: WIRE the menu item to the existing event `[CODE]`

Make the menu item an honest mirror of the working keyboard path. One authoritative
home for the *capability* (`delete-toolpath`), with the menu and the keyboard as two
equal mirrors of the same event:

```
let can_delete = matches!(state.selection, Selection::Toolpath(_));
if ui.add_enabled(can_delete, Button::new("Delete Selected").shortcut_text("Del"))
      .clicked()
{
    if let Selection::Toolpath(id) = state.selection {
        events.push(AppEvent::RemoveToolpath(id));
    }
    ui.close_menu();
}
```

- Enabled state is **derived from selection**, not a literal `false`. When nothing
  deletable is selected the item greys out for a real reason, and the `Del` shortcut text
  is now truthful (the same key does the same thing).
- This also resolves the P4-flavoured confusable in the surface record (a greyed control
  advertising a shortcut it never performs): enabled + wired = distinct, honest role.

No new event, no new data model — reuses `RemoveToolpath`. The only reason this is
`[CODE]` not `[SPEC]` is the literal `false` must change to selection-derived enablement
plus a click handler.

---

## P1-004 — Post-config sync boundary: declare the session canonical `[SPEC]`

### Confirmed current state
- `post-processor-panel` mutates `gui.post.format` (and spindle/safe-z/etc.) directly
  with no inline `session.set_post_config` (`properties/post.rs:13-20+`).
- The export **wizard** path writes both `gui.post` and the session
  (`app/input.rs:241-254`, `WizardSetPost`).
- The session is reconciled from `gui.post` only at IO boundaries — on save
  (`controller/io.rs:152-155`) and on load (`io.rs:173`).
- GUI export reads `gui.post` directly, so GUI export is correct; the only staleness
  window is **GUI-vs-MCP** between an unsaved panel edit and the next save (MCP reads
  the session directly).

### Decision: ONE writer, sync on every edit (P1)

The audit softened this to low because GUI export is unaffected — but the redesign
principle is "one concern, one home". Today there are two write disciplines: the wizard
syncs immediately, the panel defers to save. That asymmetry is exactly the divergence
P1 forbids.

Target boundary:
- **`session.post_config()` is the single source of truth for post settings.**
- `gui.post` becomes an explicit **edit buffer / mirror**, not a parallel home.
- Every post-panel edit routes through a `SetPostConfig`-style event (mirror of what
  `WizardSetPost` already does) that writes the session immediately. The panel no longer
  mutates `gui.post` in place and waits for save.
- The save-time reconcile in `io.rs:152-155` becomes a no-op safety net rather than the
  *only* sync point.

This closes the GUI-vs-MCP window and makes the wizard and the panel use **one**
write discipline. `[CODE]` to add the event routing, but the *boundary decision* is the
spec deliverable here; the panel layout does not otherwise change (see P2-004 — Safe Z
relocation — owned by the heights/feeds domain, not this one).

UI consequence: none visible beyond the panel now reflecting a value MCP/session agree
with instantly. No new control, no prose. (P5 — nothing to explain because the two homes
collapse to one.)

---

## P2-005 — Two-sided trigger: one editor, two declared mirrors `[SPEC]`

### Confirmed current state
- `AppEvent::SetupTwoSided` fires from **two** places:
  - `properties/setup.rs:100` — "Add alignment pins for this flip" button, shown when a
    flipped setup has no pins.
  - `properties/stock.rs:187` — "Two-sided setup" button, shown when no flipped setup
    exists and stock has no pins.
- The actual editing of `flip_axis` / `alignment_pins` lives **only** under
  `stock.rs` (alignment-pins-section). Setup only *reads* pin state
  (`setup.rs:230`: "Alignment pins are now defined on the stock").

The audit confirmed this is deliberate stock-level consolidation; both triggers fire the
identical event; editing is already single-home. The only residue is the dual trigger.

### Decision: KEEP both triggers, but make them obviously mirrors, not rivals (P1, P4)

This is the correct IA — a two-sided job is conceived from *either* the Setup view ("I
want to flip this setup") or the Stock view ("this stock is two-sided"). Removing one
would hurt discoverability. The fix is **labeling discipline**, not removal:

- Editing home is **Stock › Alignment pins** — authoritative, unchanged.
- Both triggers use **one consistent label and treatment**: `＋ Two-sided setup`
  (same glyph, same wording, same button style) so a user recognizes them as the *same
  action* surfaced in two contexts — directly satisfying P4 ("same-state controls get one
  consistent label/treatment"). Today they read as two different features
  ("Add alignment pins for this flip" vs "Two-sided setup").
- Each trigger carries a one-word point-of-need destination chip rather than the current
  explanatory caption ("Creates flipped Setup 2, sets flip axis, places 2 pins"):
  clicking jumps to **Stock › Alignment pins** where the editing lives. P5: replace the
  caption prose with the affordance that takes you to the one editor.

`[SPEC]` only — no data-model change. The event is already shared; this is a relabel +
navigation chip so the two triggers read as mirrors of one capability whose home is
Stock.

---

## Depth tiers across this domain (P3)

| Surface | Summary (always visible) | Behind disclosure |
|---|---|---|
| fixture-properties-panel | name, enabled, **clearance-check status pill**, `Run holder clearance ▸` | XYZ position / size / clearance grid under a `Geometry (safety-checked) ▾` header with the shield marker |
| menu-bar Edit | Undo / Redo / **Delete Selected (enabled-by-selection)** | — |
| post-processor-panel | Format, the canonical post fields | (Safe-Z affordance redesign owned by heights domain) |
| stock + setup two-sided | one `＋ Two-sided setup` button + destination chip | editing under Stock › Alignment pins |

## Distinct-role separation summary (P4)

- **Fixture panel vs keep-out panel** (near-clones today): fixture Z fields get a shield
  marker + clearance-check status pill; keep-out has neither (XY-only, full-Z by
  definition). The two forms now look like different jobs.
- **Delete Selected**: enabled-by-selection + wired click, so it never masquerades as a
  live control while doing nothing.
- **Two-sided triggers**: identical label/treatment so they read as one capability, not
  two features.

## Provenance summary (P7)

Only the fixture clearance-check pill carries provenance in this domain, and it uses the
same discipline as the feeds redesign: the pill state is read from the **actual last
collision result** keyed by `fixture_id`. An edit after the last check resets the pill to
`◇ not checked` — never falsely green. No recompute-from-scratch optimism.
