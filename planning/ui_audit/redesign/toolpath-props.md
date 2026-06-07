# Redesign SPEC — Toolpath properties: group by concern + progressive disclosure

Domain owner findings: **P2-003, P2-004, P3-001, P3-002, P3-003, P3-004, P3-005, P1-005, P1-007, P2-006.**
(Cross-domain dependency noted where a feeds-domain finding — P1-001/002/003, P7-001/002 — constrains a control's home; the feeds redesign owns those, this spec only consumes the contract it must satisfy.)

This SPEC is the TARGET information architecture for the per-toolpath properties surfaces. No Rust here — see `toolpath-props_mockups.md` for the ASCII layout that demonstrates it.

---

## 0. The core move

The Toolpath Tab Bar today is `[Params] [Feeds] [Heights] [Dressups]`, where **Params** is a flat dump that interleaves five different concerns and **Feeds** is a competing editor of fields Params already owns. The same codebase already groups properly two tabs over: `draw_dressup_params` has named sections; `draw_profile_params` nests collapsing Tabs; `tool.rs` nests Holder/Shank (P3-002 evidence).

The redesign **renames and re-charters the tabs to the five concern groups from P2**, and makes every tab a **summary tier + progressive disclosure** surface (P3) instead of a flat grid:

```
OLD:  [Params] [Feeds] [Heights] [Dressups]
NEW:  [Geometry] [Feeds & Speeds] [Linking] [Heights] [Dressup]
```

Each tab owns exactly ONE concern (P2). "Params" as a catch-all is **retired** — its fields are redistributed to Geometry (what-to-cut) and Feeds & Speeds (how-fast). Nothing is deleted; every former Params field gets a named home below.

---

## 1. Concern groups — one home each (P2, P1)

| Tab (new) | Concern | Owns (authoritative home) |
|-----------|---------|---------------------------|
| **Geometry** | *What to cut* | op pattern/strategy, cut direction, profile side, stepover, depth, depth-per-pass, total depth, all op-specific shape params (inlay fit, drill cycle, scallop height, slope/threshold, pencil, tabs-as-geometry, chamfer, project-curve, boundary geometry) |
| **Feeds & Speeds** | *How fast* | feed rate, plunge rate, spindle RPM (override), the LUT recipe read-out, vendor LUT evidence, what-if/optimizer entry points |
| **Linking** | *How moves connect* | entry style (plunge/helix/ramp) + ramp angle + helix radius/pitch, lead-in/out, link moves, retract strategy, arc-fitting, rapid-order optimization |
| **Heights** | *Z reference planes* | the 5 Z planes (clearance/retract/feed/top/bottom) + the side-view diagram. **Becomes the single authoritative home for every Z-clearance concept** (P2-004). |
| **Dressup** | *Cosmetic / post edge work* | dogbone overcuts, holding tabs (as a finishing dressup), path-quality tolerance, feed-rate optimization smoothing |

> Note on the Geometry/Linking split of today's "Dressups" tab: the current Dressups tab is **already well-grouped** but mixes two concerns — *entry/linking* (Entry & Exit, Optimization, Safety/retract) and *cosmetic* (Path Quality/dogbone). The redesign splits it: linking-of-moves → **Linking**; cosmetic edge work → **Dressup**. The named-section grammar it pioneered is the template for all five tabs.

---

## 2. Depth tiers — summary first, detail behind a drill (P3)

Fixes **P3-001 (adaptive3d 20-control grid), P3-002 (24 flat draw_*_params), P3-003 (inlay), P3-004 (setup stack), P3-005 (Feeds formula block)**.

Every tab opens at a **Summary tier**: the 4–6 fields a user touches on most jobs, always visible, no scrolling. Everything else lives behind named **CollapsingHeader** sections, collapsed by default. The rule that replaces the dump:

> **Summary = the fields that change per-job. Drill = the fields that change per-tool-or-rarely.**

### 2.1 Geometry tab tiers (per op family)

Each `draw_*_params` is restructured from one flat `Grid` into:

- **Summary (always open):** the op's defining shape controls + the cut-geometry trio (stepover / depth-per-pass / total depth). For pocket: pattern, climb/conv, stepover, DOC, total depth. For adaptive3d: strategy, stepover, DOC. For inlay: the 5 fit params (P3-003 — now under a labelled "Inlay Fit" summary section instead of bleeding into boilerplate).
- **▸ Advanced (collapsed):** tolerance, min-cut-radius, finishing/spring passes, stock-to-leave, rest-machining, slot-clearing, sampling resolution, region ordering, fine-stepdown — the per-tool / set-once fields.
- **▸ Conditional sub-fields (collapsed, auto-revealed):** fields gated on a toggle (e.g. adaptive3d helix radius/pitch shown only when relevant) keep their existing conditional reveal (P3-001 refute_note credited this) but now sit inside a named section header so the user knows what's hidden.
- The **pattern diagram** (P5-strong, params-pattern-diagrams) stays pinned at the bottom of the Summary tier as the live what-you-cut affordance.

This is the exact grammar `draw_dressup_params` already uses (Entry & Exit / Path Quality / Optimization / Safety) — applied uniformly so the codebase stops contradicting itself (P3-002 refute_note: "the codebase demonstrably knows how to group and chooses not to here").

### 2.2 Feeds & Speeds tab tiers

- **Summary (always open):** feed / plunge / spindle-RPM editable rows (the authoritative inputs, see §3), each with its inline Suggest pill, plus a one-line **recipe summary chip** (RPM · chipload · MRR).
- **▸ How is this calculated? (collapsed):** the formula breakdown — fixes **P3-005** by putting the always-on formula block behind a one-line collapsible summary instead of an always-rendered 4–5 line slab.
- **▸ Engagement diagram (collapsed).**
- **▸ Vendor cutting data (collapsed):** the vendor LUT evidence table — single canonical home (resolves the P1-006 two-viewers split for this domain's surface).

### 2.3 Heights tab

Already well-grouped (heights-tab health: yellow). Keep the grid + draggable diagram as co-equal editors of the same 5 fields (acceptable — both write `entry.heights`, one is the P5 affordance for the other). New responsibility in §4.

---

## 3. Spindle RPM precedence — one legible treatment (P1-005, P2-006)

Today: project default lives in **Post panel** ("Spindle Speed:"), per-op override lives in the **Feeds rows** ("Spindle RPM:"). The per-op side *documents* precedence (hover + "(uses project default)" hint) but the Post side is silent, and the two near-identical labels sit in unrelated panels (P1-005, P2-006 refute_notes: the disambiguation is partly present but one-directional + label collision).

**Target — one precedence widget, shown on the Feeds & Speeds tab (the override's home):**

A single **two-state RPM row** that always shows BOTH numbers and which one wins, replacing prose with an affordance (P5):

```
Spindle  [ Project 18000 ]  ◯ override → [ ____ RPM ]   ⚡
         └ effective pill ─┘  └─ opt-in toggle ──────┘
```

- When override is OFF: the effective value reads from the project default and is shown as a **read-only "Project 18000" pill** (provenance style = project-default, see §5). The override DragValue is disabled. No prose hint needed — the pill *is* the explanation.
- When override is ON: the project pill dims to a strikethrough mirror, the override DragValue activates, and the effective pill switches to "override 22000" in the hand-edit/override provenance style.
- The **project default itself** stays editable in Post (it is genuinely a project/output concern), but the Post panel "Spindle Speed:" field gets a **read-only point-of-need link pill** "→ per-op override on Feeds tab" so the precedence is legible from both ends (closes the one-directional gap in P1-005).
- P4 distinctness: the Post default field and the per-op override are now **visually distinct roles** — Post's is a bare DragValue labelled "Project spindle default"; the per-op is the two-state precedence widget above. No two near-identical "Spindle …:" DragValues in separate panels anymore.

---

## 4. Z-plane scatter — collapse to one home (P2-004)

Today Z-clearance scatters over **four** surfaces with colliding labels, **two of which ship apology tooltips** to compensate (P2-004 evidence: Heights' 5 planes; drill Params "Retract Z" R-plane w/ apology tooltip; Post "Safe Z" w/ apology tooltip; export-wizard per-export safe-Z override).

These are **not all the same concept** (P2-004 refute_note is explicit): the drill R-plane and the export override are legitimately different. So the fix is not "merge into one field" — it is **one authoritative home per distinct Z concept, with the apology prose replaced by point-of-need affordances and distinct visual roles (P4/P5):**

| Z concept | Authoritative home (target) | Treatment |
|-----------|------------------------------|-----------|
| The 5 rapid/clearance/feed/top/bottom planes | **Heights tab** | Stays. Single home for op clearance geometry. |
| Drill R-plane ("Retract Z") | **Geometry tab → Drill cycle section**, relabelled **"Peck retract (R-plane)"** | Relocate-in-place + rename. It is a *cutting-cycle* parameter (how far the bit backs out between pecks), not a clearance plane. The apology tooltip is **deleted** — the new label + its position inside the Drill cycle group carries the meaning (P5). |
| Post "Safe Z" | **Heights tab → read-only "Effective safe-Z" mirror row** + the editable value stays in Post as the *project* safe-Z | Heights gains a read-only mirror pill showing the post-clamp effective safe-Z so the user sees, at the point they edit clearance planes, what the post will actually clamp to. Apology tooltip on Post **deleted**, replaced by the mirror affordance. |
| Export per-export safe-Z override | **Export wizard** (out of this domain's tabs, stays) | Keep. Distinct role — relabel to "Export safe-Z override (this export only)" so it never reads as the project Safe-Z. |

Net: every Z value has ONE editable home; the other surfaces show **read-only mirrors with provenance styling** (§5) instead of duplicate editors or apology text. The two apology tooltips are retired (P5 win).

---

## 5. Provenance — one visual language (P7, where this domain touches it)

This domain doesn't own the provenance data model fix (P7-001/002 — `OperationConfig` storing per-field source — belongs to the feeds/core domain). But every surface in this domain that *displays* a value-with-origin must use **one consistent provenance vocabulary** so the user learns it once. The contract this domain consumes and renders:

| Provenance | Visual role (consistent everywhere) |
|------------|-------------------------------------|
| Vendor LUT | green pill, "vendor" + obs-id on hover |
| Sim optimizer | blue pill, ⭐ marker |
| Nomogram what-if | violet pill, "what-if" |
| Hand edit / override | neutral/grey pill, no badge |
| Project default (inherited) | hollow/outline pill, "project" label |

Applied in this domain: the spindle precedence widget (§3) uses the **project-default hollow pill** vs **override neutral pill** — that IS the precedence cue. The Z mirror rows (§4) use the hollow/inherited style to read instantly as "this is a mirror, edit it at its home." This domain commits to *rendering* the shared vocabulary; it does not invent a second one.

---

## 6. Confusable controls made distinct (P4)

| Confusion (finding) | Fix |
|---------------------|-----|
| **P2-003** adaptive3d entry-style: live-but-inert Dressups combo duplicates Adaptive3dConfig's own Plunge/Helix/Ramp | Entry style has ONE home: the **Linking tab**. For adaptive3d, the Linking entry-style control reads the op's `Adaptive3dConfig` entry directly. The generic `DressupConfig` entry-style combo is **removed for adaptive3d** (its `FORCE_NO_ENTRY` policy made it a no-op anyway — P2-003 refute_note: enabled-but-inert). For other ops, the Linking entry-style writes `DressupConfig`. No surface shows two entry-style controls for one op. The inert combo is **retired**, not greyed. |
| **P1-005 / P2-006** two near-identical "Spindle …:" DragValues | §3 — distinct roles: Post = "Project spindle default" bare field; Feeds = two-state precedence widget. |
| **P2-004** "Retract Z" / "Safe Z" reused across 4 panels | §4 — each relabelled to its distinct concept ("Peck retract (R-plane)", "Effective safe-Z (mirror)", "Export safe-Z override"). No two panels share a Z label. |
| **P1-007** "Scallop height" means two different things (Scallop op geometry vs DropCutter ball-tip override) | Soft-conceptual (refute_note: legitimately different ops). Fix is **labelling, not merging**: Scallop op's field stays "Scallop height" under Geometry; the DropCutter override is relabelled **"Finish scallop limit (ball-tip)"** and lives in DropCutter's Geometry → Advanced section, so the term no longer collides at a glance. |

---

## 7. Controls retired or rewired (P6)

| Control | Action | Reason |
|---------|--------|--------|
| Generic `DressupConfig` entry-style combo **for adaptive3d** | **Retire** | Live-but-inert no-op (P2-003); `FORCE_NO_ENTRY` coerces it to None. Entry style for adaptive3d is owned by the Linking tab reading `Adaptive3dConfig`. |
| Drill "Retract Z" apology tooltip | **Retire (prose)** | Replaced by relabel + group placement (P5, §4). |
| Post "Safe Z" apology tooltip | **Retire (prose)** | Replaced by the Heights effective-safe-Z mirror pill (P5, §4). |
| Feeds tab always-on formula slab | **Demote** to collapsible "How is this calculated?" | P3-005; one-line summary + drill. |
| "Params" tab as a catch-all | **Retire (as a tab)** | Re-chartered into Geometry + Feeds & Speeds; no field lost (§1). |

Nothing in this domain is a dead orphan beyond the inert adaptive3d entry combo; the rest of the work is relocate/rename/group.

---

## 8. Finding → fix traceability

- **P2-003** → §6 (entry-style one home: Linking; inert combo retired §7)
- **P2-004** → §4 (Z one-home-per-concept; 2 apology tooltips retired)
- **P3-001** → §2.1 (adaptive3d grid → Summary + named Advanced/Conditional sections)
- **P3-002** → §0, §2.1 (uniform Summary+drill grammar across all draw_*_params)
- **P3-003** → §2.1 (inlay fit becomes a labelled Summary section)
- **P3-004** → §2 (setup stack → CollapsingHeader sections; same grammar — applies to setup-properties even though it's an adjacent panel, kept consistent)
- **P3-005** → §2.2, §7 (formula slab demoted to collapsible)
- **P1-005** → §3 (two-state precedence widget + Post→override link pill)
- **P1-007** → §6 (DropCutter scallop override relabelled)
- **P2-006** → §3, §5 (project-default hollow pill vs override pill IS the precedence cue)
