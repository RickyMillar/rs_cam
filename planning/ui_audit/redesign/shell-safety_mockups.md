# ASCII Mockups — Shell, safety wiring & cross-cutting cleanup

Demonstrates, not describes. Distinct roles get distinct treatments; prose captions are
replaced by affordances (pills, chips, point-of-need buttons).

Glyph legend (one consistent visual language, reused everywhere below):
```
  🛡  field is consumed by the holder-clearance safety check
  ◇  not checked (grey)     ✓ clear (green)     ✗ hit (red, clickable → seeks viewport)
  ＋  create / mirror-trigger action (identical wherever the same capability appears)
  ▸  navigates to the authoritative editing home
  ▾ / ▸  collapsed / expandable disclosure
```

---

## 1. fixture-properties-panel — P6-003 wired + P3 tiering + P4 separation

### BEFORE (false-assurance: Z editable, never checked, flat field wall)
```
┌─ Fixture ─────────────────────────────────┐
│ Name   [ Front clamp            ]          │
│ Kind   [ Clamp        ▼ ]                  │
│ ☑ Enabled                                  │
│ Position  X[  10.0] Y[  5.0] Z[  0.0]      │   ← Z edits a decorative box only
│ Size      X[  40.0] Y[ 20.0] Z[ 25.0]      │   ← never reaches collision check
│ Clearance [   3.0]                         │
└────────────────────────────────────────────┘
   (identical-looking to the keep-out panel; nothing says "safety")
```

### AFTER (summary-first; Z fields marked as safety inputs; status pill proves the check ran)
```
┌─ Fixture ──────────────────────────────────────────────┐
│ Name   [ Front clamp            ]   ☑ Enabled           │
│ Kind   [ Clamp        ▼ ]                               │
│                                                         │
│ Holder clearance:  ✗ hit            [ Run holder clearance ▸ ] │  ← point-of-need
│   └ clicking ✗ seeks viewport to the collision marker   │     mirror of menu cmd
│                                                         │
│ 🛡 Geometry (safety-checked)                         ▾  │  ← disclosure (P3)
│ ┌─────────────────────────────────────────────────────┐ │
│ │ 🛡 Position  X[ 10.0]  Y[  5.0]  Z[  0.0]            │ │  ← shield marks the
│ │ 🛡 Size      X[ 40.0]  Y[ 20.0]  Z[ 25.0]            │ │     fields the check
│ │ 🛡 Clearance [  3.0]   (inflates all 6 faces)        │ │     actually consumes
│ └─────────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────┘
```
Pill states (one consistent language — same as keep-out / feeds provenance discipline):
```
  Holder clearance:  ◇ not checked        (grey — edited since last run, or never run)
  Holder clearance:  ✓ clear              (green — last check covered this box, no hit)
  Holder clearance:  ✗ hit  →             (red, clickable → SimJumpToMove on marker)
```
Editing any 🛡 field flips the pill back to `◇ not checked` — never a falsely-green box.

### Contrast — keep-out panel stays a plain XY form (P4: the two no longer look alike)
```
┌─ Keep-out zone ────────────────────────────┐
│ Name   [ Vise body          ]  ☑ Enabled   │
│ Position  X[ 60.0]  Y[ 10.0]               │   ← no 🛡, no Z, no clearance pill:
│ Size      X[ 30.0]  Y[ 25.0]               │     full-Z by definition, different job
└────────────────────────────────────────────┘
```

---

## 2. menu-bar Edit menu — P6-001 Delete Selected wired

### BEFORE (permanently greyed, advertises a Del shortcut it never performs)
```
  Edit ▾
  ┌───────────────────────────────┐
  │ Undo                  Ctrl+Z  │
  │ Redo                  Ctrl+Y  │
  │ ───────────────────────────── │
  │ Delete Selected          Del  │ ← always greyed (literal false), emits nothing
  └───────────────────────────────┘
```

### AFTER (enabled by selection; click + Del do the same real thing)
```
  Edit ▾  (a toolpath is selected)          Edit ▾  (nothing deletable selected)
  ┌───────────────────────────────┐         ┌───────────────────────────────┐
  │ Undo                  Ctrl+Z  │         │ Undo                  Ctrl+Z  │
  │ Redo                  Ctrl+Y  │         │ Redo                  Ctrl+Y  │
  │ ───────────────────────────── │         │ ───────────────────────────── │
  │ Delete Selected          Del  │ ←active  │ Delete Selected          Del  │ ←greyed
  └───────────────────────────────┘  →emits  └───────────────────────────────┘  for a
        RemoveToolpath(id)                          real reason (no selection)
```
The `Del` label is now truthful: menu click and the keyboard both push `RemoveToolpath`.

---

## 3. Two-sided trigger — P2-005 one capability, two mirror triggers, one editor

### BEFORE (two differently-labelled buttons + explanatory caption = reads as 2 features)
```
  Setup panel:                          Stock panel:
  ┌──────────────────────────────┐      ┌──────────────────────────────────────┐
  │ [ Add alignment pins for     │      │ [ Two-sided setup ]                   │
  │   this flip ]                │      │ Creates flipped Setup 2, sets flip    │
  └──────────────────────────────┘      │ axis, places 2 pins                   │ ←prose
                                         └──────────────────────────────────────┘
```

### AFTER (identical label/treatment = obviously the SAME action; chip replaces prose)
```
  Setup panel:                          Stock panel:
  ┌──────────────────────────────┐      ┌──────────────────────────────┐
  │ [ ＋ Two-sided setup ]        │      │ [ ＋ Two-sided setup ]        │
  │   edit pins in  Stock ▸       │      │   edit pins in  Stock ▸       │ ←navigates to
  └──────────────────────────────┘      └──────────────────────────────┘   the one home
        both push SetupTwoSided                both push SetupTwoSided
```
Authoritative editing of `flip_axis` / `alignment_pins` lives only in
**Stock › Alignment pins** (unchanged). The `Stock ▸` chip is the affordance that
replaces the "Creates flipped Setup 2…" caption.

---

## 4. Post sync boundary — P1-004 (invisible, by design)

No layout change. The point of the fix is that the panel and the wizard write through
**one** discipline (session = source of truth, `gui.post` = mirror), so there is nothing
new to show — and nothing to explain (P5). The only observable difference: an unsaved
post edit is instantly visible to MCP / the session, not stale until next save.

```
  BEFORE:  panel edit ──→ gui.post ──(only at save)──→ session   ← MCP sees stale value
  AFTER:   panel edit ──→ SetPostConfig ──→ session ──(mirror)──→ gui.post   ← always agree
```
