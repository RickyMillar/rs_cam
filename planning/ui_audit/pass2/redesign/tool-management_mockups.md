# Pass-2 Mockups — Tool management

ASCII only; intent is to show affordances, not pixels.

---

## M1 — Project Tools list (toolpath panel) — TOO-001, TOO-002

Before (shipping): select-only list + "+ Add Tool". No delete, no duplicate, no import, no manage.

```
▼ Tool Library
   ◦ 6mm flat endmill ⌀6.00
   ◦ 3mm ball ⌀3.00
   [ + Add Tool ▾ ]
```

After: each row is a manager; full action set lives here.

```
▼ Project Tools                              [ Manage Library… ]
   ⬚ 6mm flat endmill        ⌀6.00  4fl
   ◗ 3mm ball nose           ⌀3.00  2fl   ◀ selected
   ▽ 60° V-bit               60°
        └ right-click ▸ ┌──────────────┐
                        │ Duplicate     │   → DuplicateTool
                        │ Delete   (Del)│   → RemoveTool
                        └──────────────┘
   [ + Add Tool ▾ ]  [ + From Library ▾ ]
                          └ endmills ▸ 1/4" upcut ⌀6.35
                            ball     ▸ R1.5 ⌀3.00
```

- Per-row context menu + `Del` key on a selected tool revive `DuplicateTool` / `RemoveTool` (TOO-001).
- `+ From Library ▾` and `Manage Library…` bring import + catalog manager into the tool home (TOO-002).
- Glyph differs by tool type (⬚ endmill, ◗ ball, ▽ V-bit) so rows are distinguishable at a glance.

---

## M2 — Tool editor: consistent commit + grouped geometry — TOO-003, TOO-005

Before (properties panel): flat form, live-apply, "Shaft Diameter" and "Shank Diameter" both visible as sibling-ish rows; no commit state shown.

After: grouped by concern, draft + Apply/Revert, shaft/shank separated by group header and relabelled.

```
┌ 3mm ball nose ──────────────── ● modified ┐
│ Name  [ 3mm ball nose            ]          │
│ Type  [ Tapered ball nose      ▾ ]          │
│                                             │
│ ── Cutter geometry ──────────────           │
│   Diameter            [ 3.00 ] mm           │
│   Cutting length      [ 18.0 ] mm           │
│   Flutes              [ 2 ]                  │
│   Helix               [ 30 ] deg            │
│   Taper half-angle    [ 3.0 ] deg           │
│   Upper shaft ⌀ (taper top) [ 6.00 ] mm     │  ← was "Shaft Diameter"
│   Material            [ Carbide          ▾] │
│   Cut dir             [ Climb            ▾] │
│                                             │
│   [ cross-section preview ]                  │
│                                             │
│ ▶ Holder / Shank  ⚠ no holder — collision   │  ← badge ON HEADER (TOO-006)
│                      check skipped           │
│        (expanded:)                           │
│        Holder ⌀          [ 0.00 ] mm         │
│        Shank ⌀ (in collet)[ 6.00 ] mm        │  ← was "Shank Diameter"
│        Shank length     [ 30.0 ] mm          │
│        Stickout         [ 25.0 ] mm          │
│                                             │
│              [ Revert ]   [ Apply ]          │  ← explicit commit (TOO-003)
└─────────────────────────────────────────────┘
```

- Header `● modified` + `[Revert] [Apply]` = one commit model matching the modal's Save/Cancel. No more "did this save?" ambiguity (TOO-003).
- "Upper shaft ⌀ (taper top)" sits under **Cutter geometry**; "Shank ⌀ (in collet)" sits under **Holder / Shank**. Different group, role-bearing label — no longer confusable (TOO-005).

Holder configured → header badge clears:

```
│ ▶ Holder / Shank   ⌀ 25.0 mm ✓               │
```

---

## M3 — Library save without silent duplicates — TOO-004

Before: free-text catalog box + "Save" → unconditional append; vendor/product-id read-only in modal, uneditable everywhere.

After: catalog-aware save (add-or-replace), and vendor/product-id editable in the catalog editor.

Panel export:

```
[ Save to library… ]
      └ ┌──────────────────────────────┐
        │ Catalog [ endmills        ▾ ] │
        │ Tool "6mm flat endmill"       │
        │ ⚠ exists — will replace        │   ← dedupe by (name, product_id)
        │            [ Cancel ] [ Save ] │
        └──────────────────────────────┘
```

Modal catalog editor gains a metadata group (these fields exist on ToolConfig and were read-only-only before):

```
│ ── Catalog metadata ─────────────             │
│   Vendor      [ Amana            ]            │  ← now editable (TOO-004)
│   Product ID  [ 46202-K          ]            │  ← now editable (TOO-004)
```

- Save replaces an existing entry instead of piling up duplicates the Dedupe button must later sweep (TOO-004).
- Vendor / Product ID are correctable provenance, not display-only dead-ends (TOO-004).
