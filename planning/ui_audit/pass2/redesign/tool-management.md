# Pass-2 Redesign SPEC — Tool management

Scope: the GUI surfaces that create, edit, organise, and import tools.
Surfaces in scope:

- `tool-library-collapsible` — the live "Tool Library" CollapsingHeader inside the toolpath panel (`toolpath_panel.rs:172-192`).
- `tool-properties-panel` — the right-side editor for the selected project tool (`properties/mod.rs:221-231` → `properties/tool.rs`).
- `tool-library-modal` — the Tools ▸ Tool Library… catalog manager (`tool_library_modal.rs`).
- `project_tree` (dead) — `ui/project_tree.rs`, declared `pub mod` at `ui/mod.rs:10`, never wired into any panel.

OUT OF SCOPE (pass-1, do not touch): per-toolpath property tabs / Feeds & Speeds / Heights / Dressups / Linking; vendor-LUT / nomogram; spindle precedence; fixture-collision fix; Verdict-HUD reconciliation. Tool-side feeds defaults are not addressed here.

---

## Findings addressed

| id | sev | one-line |
|----|-----|----------|
| TOO-001 | high | delete/duplicate a project tool only exists in a never-rendered panel — unreachable in shipping UI |
| TOO-002 | high | the panel where tools are managed has no path to library manager or to importing a saved tool |
| TOO-003 | med | identical-looking editor applies live in the properties panel but needs Save/Cancel in the modal |
| TOO-004 | med | vendor / product-id shown but never editable; panel "Save to library" silently appends duplicates |
| TOO-005 | low→ | "Shaft Diameter" (grid) and "Shank Diameter" (Holder section) co-visible and confusable |
| TOO-006 | low | "collision check skipped" safety state shown only as grey italic prose below a collapsed section |

---

## Root cause

Tool management has **two competing homes that were never reconciled**, plus a **third, fully built but disconnected one** (`project_tree`). The disconnected panel is where the complete CRUD set was implemented (duplicate, delete, "Manage library…", "From library" import). When the live home (`toolpath_panel` Tool Library collapsible) replaced it, only *create* + *select* were carried over; *delete*, *duplicate*, *import*, and *manage* were left behind in dead code (TOO-001, TOO-002). The result: the shipping panel is a read-mostly list, and the only complete tool manager is the modal — which a user must reach from the top menu bar.

The redesign establishes **one home for project-tool CRUD** (the panel list), **one home for catalog management** (the modal), and makes the boundary between them explicit, with consistent commit semantics across both (TOO-003).

---

## Target structure

### Home 1 — Project Tools list (in the toolpath panel) — *one concern: tools that belong to this project*

Replace the current select-only collapsible. Each row is a full management affordance, not just a label.

Tier 1 (always visible, the list):
- one row per project tool: colour-agnostic glyph by tool type + `tool.summary()` + selection state.
- a **per-row right-click context menu**: `Duplicate`, `Delete` (revives the dead `DuplicateTool` / `RemoveTool` paths that already exist in `controller/events/mod.rs:50-51`; the only thing missing is a live emitter — this restores TOO-001).
- a **keyboard path**: extend the Delete/Backspace handler at `input.rs:441` to also match `Selection::Tool`, so a selected tool deletes like a selected toolpath does today (closes the keyboard half of TOO-001).

Tier 1 actions row (below the list):
- `+ Add Tool ▾` — blank-by-type (unchanged behaviour, `AddTool`).
- `+ From Library ▾` — the catalog→tool submenu emitting `AddToolFromLibrary`, ported verbatim from the dead `project_tree.rs:142-159`. This puts import where tools are managed (closes the import half of TOO-002).
- `Manage Library…` — small button emitting `OpenToolLibrary`, ported from dead `project_tree.rs:99-105`. The library manager is now reachable from the tool home, not only the menu bar (closes the manage half of TOO-002). The menu-bar entry (`menu_bar.rs:166`) stays as a secondary route.

This makes the panel the single home for **everything you do to a tool that lives in the project**: add, add-from-library, duplicate, delete, select-to-edit, and jump to the catalog manager.

### Home 2 — Tool Library modal — *one concern: the reusable catalogs*

Keep as the catalog manager (browse / edit-in-catalog / move / delete / dedupe / import-into-project). No structural change to its role. Two fixes:

- **Make vendor / product-id editable** (TOO-004). They already render read-only in the modal grid (`tool_library_modal.rs:485-490`) and are absent from the shared `draw_tool_fields`. Add a "Catalog metadata" subsection (Vendor, Product ID text fields) to the editor — see below — so library tools can carry correctable provenance.
- **Retire the panel-side "Save to library" silent-append.** See "Retired", below.

### Shared editor — `draw_tool_fields` — consistent commit semantics (TOO-003)

The same form renders in two places with opposite commit models: live-apply (properties panel, mutates `tools_mut()` directly) vs draft-then-Save (modal, edits a clone). The fix is **make both draft-commit**, because draft-commit is the one that generalises (the modal already needs it, and a Cancel affordance is strictly more capable than relying on Ctrl-Z):

- The properties panel adopts the modal's pattern: edit a `draft` clone of the selected tool; commit on an explicit **Apply** (or auto-commit on focus-loss is acceptable as a fallback, but the form must show a "modified — Apply / Revert" affordance when the draft differs from the committed tool). The existing undo snapshot (`properties/mod.rs:222-227`) stays as a safety net but is no longer the *only* way back.
- Result: a user reads one editor with one rule everywhere — edits are pending until committed, and a visible Apply/Revert pair shows the state. The "is this saved?" ambiguity (TOO-003) is gone.

### Confusable distinction (TOO-005)

`draw_tool_fields` currently shows **Shaft Diameter** (in the params grid, TaperedBallNose only, `tool.rs:185`) and **Shank Diameter** (in the Holder/Shank section, `tool.rs:219`) at the same time, two near-identical names for different geometry. Distinguish by **co-locating and disambiguating**, not by renaming-in-prose:

- Move the TaperedBallNose **Shaft Diameter** out of the generic params grid and **into the cross-section preview's labelled callouts** / a "Cutter geometry" group, while **Shank Diameter** stays in the "Holder / Shank" group. Distinct roles now live under distinct group headers ("Cutter geometry" vs "Holder / Shank"), so the two diameters are visually separated by section rather than stacked as sibling rows.
- Rename for role clarity in-affordance: Shaft → **"Upper shaft ⌀ (taper top)"**, Shank → **"Shank ⌀ (in collet)"**. Short, role-bearing labels; the geometry each drives is now legible from the label + its group.
- The two "Corner Radius" labels (EndMill `corner_radius_mm` vs BullNose `corner_radius`) are mutually exclusive match arms and never co-visible — no change required (this is why TOO-005 softened to low). Note in code that the duplication is intentional-by-arm.

### Safety state made a control state, not prose (TOO-006)

The "Holder not configured — collision check will be skipped" grey italic below the collapsed Holder section is invisible. Promote it onto the **section header itself**:

- The "Holder / Shank" CollapsingHeader carries a **status badge** in its header text: a warning glyph + "no holder — collision check skipped" when `holder_diameter < 0.01`, rendered in the theme WARNING colour, so the state is visible **without expanding the section**.
- When a holder is configured, the badge shows a neutral/OK marker (or nothing). The free-floating italic prose below the section is removed — the state now lives on the affordance (UI wins, not prose).

---

## Retired

- **`ui/project_tree.rs` (whole module) + its `pub mod project_tree;` at `ui/mod.rs:10`.** It is dead (zero callers). Once its three unique affordances are ported to the panel (per-row Duplicate/Delete context menu, `+ From Library` submenu, `Manage Library…` button — all listed under Home 1), the module has nothing left that the live UI doesn't have. Delete it to remove the "complete tool manager hiding in dead code" trap that produced TOO-001/TOO-002. (The `DuplicateTool`/`RemoveTool` event variants and their handlers stay — they are now driven by the live panel + keyboard.)
- **Panel-side "Save to library" silent append** (`properties/tool.rs:13-49` → `tool_library::append_tool` → unconditional `push`, TOO-004). Replace with a **"Save to library…"** action that routes through the modal's catalog-aware path (choose catalog, then add-or-replace by name with the existing dedupe semantics) instead of an unconditional append from a free-text catalog box. The one-way export affordance stays, but it no longer silently piles up duplicates that only the modal's Dedupe button can clean. If a lighter touch is preferred, at minimum the append must become add-or-replace-by-(name, product_id) using the same logic that backs `DedupeToolCatalog` (`mod.rs:119`).

---

## Why this satisfies the principles

- **One concern / one home**: project-tool CRUD → panel list; catalog management → modal. The dead third home is deleted.
- **Distinct roles look distinct**: Shaft vs Shank now sit under different group headers with role-bearing labels (TOO-005).
- **UI wins, not prose**: the collision-skip state is a header badge, not italic explanatory text (TOO-006).
- **Every control earns its place**: dead `project_tree` removed; its working controls migrated to the live home.
- **Consistency**: one commit model (draft + Apply/Revert) across both editor instances (TOO-003).
- **Provenance legible**: vendor / product-id become editable so catalog tools carry correctable source metadata (TOO-004).
