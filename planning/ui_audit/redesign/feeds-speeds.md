# Redesign SPEC — Feeds & speeds consolidation + provenance

Domain owner findings: P1-001, P1-002, P1-003, P1-006, P2-001, P2-002, P2-007,
P7-001, P7-002, P7-003, P7-004, P7-005, P4-003, P4-006, P4-007.

This is a SPEC (no Rust). Mockups live in `feeds-speeds_mockups.md`.

---

## 0. The problem in one line

One `OperationConfig` is edited from 4+ feeds homes (Params inline pills,
Feeds-tab legacy card, Feeds modal, Suggest-all buttons in two tabs), a feeds
"Suggest" silently rewrites cut geometry (DOC/WOC), three engines (vendor LUT /
sim optimizer / nomogram what-if) deposit values that afterward look identical,
applied values store **zero provenance**, and one ⚡ glyph means three different
scopes.

---

## 1. Target structure — ONE canonical feeds home (P1, P2)

### 1.1 The five concern-groups for a toolpath (P2)

The Toolpath properties area regroups into exactly five clusters. This domain
owns group **B** and the feed-vocabulary leak in group **F**:

| Group | Concern | Owns |
|---|---|---|
| A | Geometry — *what* to cut | pattern, side, boundary, op-specific shape params |
| **B** | **Feeds-speeds & toolpath cut params — *how fast / how deep/wide*** | **feed, plunge, RPM, DOC, WOC, stock-to-leave, finishing passes** |
| C | Linking — how moves connect | lead-in/out, links, retract strategy |
| D | Dressup | tabs, dogbone, arc-fit |
| E | Heights | the 5 Z planes |
| F | Post | high-feedrate (G0→G1) — *flagged, not owned; see §6* |

### 1.2 The ONE home: the Feeds & Speeds panel (group B), inline in the Toolpath tab

There is exactly one authoritative editor for feed / plunge / RPM / DOC / WOC:
the **Feeds & Speeds section of the Toolpath tab** (today: `toolpath-tab-feed-params`,
already the P1 winner per the audit — it holds the authoritative `DragValue`
inputs). Everything else becomes a read-only mirror or is retired.

Authoritative home owns:
- `set-feed-rate`, `set-plunge-rate`, `set-spindle-rpm`,
  `set-stepover`, `set-depth-per-pass` (the editable inputs)
- the per-field **apply-recommendation** affordance (the redesigned pill, §4)
- the **apply-all** affordance (one button, §4)
- the **provenance chip** on every value (§5)

### 1.3 What happens to the other surfaces (P1-001/002/003/006)

| Surface | Fate | Why |
|---|---|---|
| `toolpath-tab-feed-params` | **KEEP — becomes THE home** | already authoritative inputs (P1 winner) |
| `feeds-card` (legacy card) | **RETIRE** | functional duplicate; code admits Phase-4 leftover (P1-003) |
| `toolpath-tab-suggest-all` | **CONSOLIDATE into home's one Apply-all** | dead duplicate (P1-001), comment admits "Mirrors the same-named button" |
| `Feeds Tab` (as a separate tab) | **REDESIGN → "Details" drawer** | the tab collapses into a drill-down off the home (§3) |
| `feeds-modal-toolpath` | **REDESIGN → read-mostly "Details" drawer** (P3) | no longer a second editor; it is the *drill* for charts/derate/provenance. Apply rows removed; editing happens in the home |
| `feeds-modal-nomogram-explore` | **KEEP, but explicit + attributed** | the what-if explorer is a distinct capability; its Apply now stamps `nomogram` provenance and is geometry-safe (§2) |
| `optimize-modal` | **KEEP, attributed** | distinct engine (sim); its Apply now stamps `sim-optimizer` provenance (§5) |
| `feeds-modal-project` | **KEEP** | project rollup; one batch home, attributed |
| `vendor-lut-viewer` | **KEEP (read-only ref)** + **mirror inside Details drawer** | single canonical raw-LUT table; the modal's Charts A/B reference it (P1-006) |

Net: **one editor**, plus drill-downs (Details drawer, what-if explorer,
optimizer, project rollup) that are visually marked as *different engines*, not
competing copies.

---

## 2. Separate "how fast" from "how deep/wide" (P2-002) — the headline fix

Today `apply_feeds_result_to_op` unconditionally writes
`set_stepover` + `set_depth_per_pass` (geometry) in the same call as
feed/plunge/rpm — a feeds "Suggest" silently changes cut geometry
(`crates/rs_cam_core/src/feeds/suggest.rs:695-696`, verified).

### Target rule (P2)

The home splits group B visually into **two sub-blocks**:

```
  SPEED      feed · plunge · RPM        ← "how fast"
  CUT        DOC · WOC · stock-to-leave ← "how deep/wide" (geometry)
```

- **Apply-all** in the home applies **SPEED only** by default.
- Any recommendation that *also wants to change geometry* (DOC/WOC) must surface
  that as an **explicit, separately-confirmable, separately-attributed** change.
  It is never silent and never bundled into a speed apply.
- The geometry sub-block's recommendation pills are a **distinct visual role**
  (different affordance + a "changes cut geometry" marker) from speed pills, so
  the user can never mistake a geometry rewrite for a speed tweak.

This makes every geometry change from a feeds action **explicit and
attributable** — exactly what P2-002 demands. (Data-model note: the apply
machinery must be able to apply the speed subset independently of the geometry
subset; the single bundled `apply_feeds_result_to_op` is the current blocker —
flagged to the core/wiring owner as `treatment=fix-wiring`.)

---

## 3. Depth tiers (P3) — summary first, detail behind a drill

### Tier 0 — SUMMARY (always visible, on the Toolpath tab home)
For each of the five fields, one row showing:
`[label]  [editable value]  [provenance chip]  [⚡ pill if a better value exists]`
Plus: one `Apply recommended speeds` button, one `Power ▓▓▓░ 62%` micro-bar,
and one `Details ▸` disclosure trigger. Speed and Cut sub-blocks are separated
by a thin labeled rule (§2). **No formula prose on the summary** (P5).

### Tier 1 — DETAILS drawer (one disclosure; replaces the modal + Feeds tab body)
Opens in place (no separate window). Contains, each behind its own collapsed
header:
- **Current vs recommended** table (read-only comparison — the old modal's
  comparison card minus its Apply rows; editing stays in Tier 0)
- **Why this value?** — the provenance/derate breakdown (chipload source,
  matched vendor row, derate chain). Replaces P7-005's modal-only depth.
- **Charts** (engagement, feed-vs-RPM nomogram, vendor band) — collapsed
- **What-if explorer** (the nomogram drag tool) — collapsed; its Apply is
  attributed `nomogram` and geometry-safe (§2)
- **Raw vendor LUT** (the canonical `vendor-lut-viewer` table, mirrored here)

### Tier 2 — Project rollup & sim optimizer (separate modals, unchanged scope)
`feeds-modal-project` and `optimize-modal` stay as full modals; they are
cross-toolpath / alternate-engine tools, correctly *not* on the per-op summary.

States explicitly: **on summary** = the 5 values + provenance chips + one
apply-speeds + power bar. **behind a drill** = comparison table, derate math,
charts, what-if, raw LUT.

---

## 4. Distinguish the confusables (P4)

### 4.1 The ⚡ glyph overload (P4-003) — three scopes, three distinct roles

Today the same ⚡ means single-field, suggest-all, and the bare pill. The
redesign gives each scope a **distinct visual role**:

| Scope | Old | New role (distinct affordance) |
|---|---|---|
| Apply ONE field | ⚡ pill / ⚡ Suggest button | a small inline **`↑ use 18000`** chip showing the *target value* in the chip itself — value-bearing, single-field. No bare lightning. |
| Apply ALL (speeds) | ⚡ Suggest all / ⚡ Suggest all (LUT) | one full-width labeled **button** `Apply recommended speeds` — clearly bulk, clearly speed-scoped (not geometry). |
| Read-only "where from" | (was a colored ⚡ doubling as status) | a **provenance chip** (§5) — a non-button, no action affordance. |

Result: a single field-apply *shows its value*, the bulk apply *is a labeled
button*, and provenance *is a chip you can't click to apply*. Three obviously
different things (P4).

### 4.2 Read-only vs actionable rows in the recipe grid (P4-007)

In the old feeds-card grid, RPM/chipload/power/MRR were read-only but looked
identical to the actionable feed/plunge/DOC/WOC rows (empty action column).
In the redesign:
- **Derived / read-out values** (chipload, power, MRR) render as **plain text
  with a provenance chip and NO apply affordance** and sit under a `Derived`
  subheading.
- **Editable values** (feed/plunge/rpm/doc/woc) render with a `DragValue` +
  apply chip.
The presence/absence of a `DragValue` plus the `Derived` grouping is the cue —
no more guessing which rows can be applied.

### 4.3 Two "Apply" buttons in two modals (P4-006)

`feeds-modal-toolpath`'s Apply rows are **removed** (editing moves to the Tier-0
home), so the only remaining cross-modal "Apply" pair is optimizer vs project.
Each Apply button is **prefixed with its engine + provenance glyph**, e.g.
`◆ Apply sim-optimized` vs `▣ Apply vendor LUT` — never a bare "Apply". The
provenance glyph (§5) is the same language that later marks the stored value.

---

## 5. ONE provenance visual language (P7) — the core deliverable

### 5.1 Data-model implication (P7-001/004) — bare scalars can't carry origin

`OperationConfig` stores feeds as bare `f64`/`u32`
(`operation_configs.rs:940-967`); there is **no field** recording origin, and the
pill colour shown later is *recomputed from a fresh LUT lookup*, not from what
produced the stored value. **The spec requires** each applied feed/speed/geometry
value to carry a small provenance stamp in the model:

```
ValueProvenance = { source, ref, when }
  source ∈ { VendorLUT, SimOptimizer, Nomogram, HandEdit, Default }
  ref     = e.g. vendor observation_id | optimizer candidate id | "" 
  when    = timestamp / generation
```

This is flagged to the core owner as `treatment=fix-wiring` on the affected
`set-*` capabilities — the UI cannot show legible provenance until the value
carries it. (Today's `ChiploadSource` is overloaded as the provenance label for
four independently-derived fields — P7-002 — so it cannot be reused as-is.)

### 5.2 ONE chip, FOUR sources, consistent EVERYWHERE (P7-002/003/004/005)

A single provenance-chip component, used on every surface (home summary,
Details comparison, optimizer, nomogram, project rollup, pills, Apply buttons):

| Source | Glyph | Color (single canonical RGB, used on ALL surfaces) |
|---|---|---|
| Vendor LUT | `▣` | green `(80,180,80)` |
| Sim optimizer | `◆` | blue `(90,150,220)` |
| Nomogram what-if | `◷` | violet `(170,120,210)` |
| Hand edit | `✎` | neutral grey `(150,150,150)` |
| Formula fallback / floor | `▲` | amber `(220,180,60)` |

Fixes:
- **P7-003** (one signal, three colors today: card blue, pill green, modal
  `SUCCESS` green; two ambers): there is now exactly ONE RGB per source, defined
  once, referenced everywhere. The chip looks identical on the home, the modal
  drawer, the optimizer, and the project rollup.
- **P7-002** (per-field mis-attribution: a vendor RPM shown amber "formula
  fallback"; DOC/WOC pills claiming chipload provenance for geometry): each field
  carries **its own** provenance stamp. RPM's chip reflects RPM's origin; DOC's
  chip reflects DOC's origin. No single enum standing in for four fields.
- **P7-004** (three engines deposit indistinguishable values): a sim-validated
  value reads `◆ sim-optimizer`, a raw LUT value reads `▣ vendor LUT`, a what-if
  reads `◷ nomogram`, a typed value reads `✎ hand edit` — distinguishable after
  apply, forever, because the origin is stored (§5.1).
- **P7-005** (provenance depth only in the modal): the chip is on the **summary**;
  click it to open the "Why this value?" drill in the Details drawer. Shallow cue
  on summary, full derate behind one click — no separate window needed.

### 5.3 Chip is not a button-to-apply (P4 reinforcement)

The provenance chip has a hover + click-to-open-detail, but it is visually a
**status badge**, distinct from the value-bearing single-field apply chip and the
full-width Apply button (§4.1). Three roles, three looks.

---

## 6. Retire / relocate the stragglers (P6, P2-007)

- **P2-007** — `post.high_feedrate` / `high_feedrate_mode` ("Safe Rapids G0→G1")
  reuses "feed" vocabulary in the Post panel. **Relocate label** out of the feed
  vocabulary: rename to **"Rapid-replacement speed"** under a **Linking/Rapids**
  heading (group C/F), so the word "feed" stops appearing in four places. It
  stays a post concern; it is no longer dressed as an op feed.
- **Retired controls**: the legacy `feeds-card` and the duplicate
  `toolpath-tab-suggest-all` button. Their capabilities survive in the home (the
  card's per-field Suggest → home pills; both Suggest-alls → the one
  `Apply recommended speeds`). Nothing is dropped — only de-duplicated.

---

## 7. Finding → fix traceability

| Finding | Fix |
|---|---|
| P1-001 | three Apply-all buttons → one `Apply recommended speeds` in the home (§1.3, §4.1) |
| P1-002 | per-field suggest has three homes → one set of pills in the home (§1.2, §4.1) |
| P1-003 | legacy card + modal side-by-side → card retired, modal becomes read-only Details drawer (§1.3, §3) |
| P1-006 | vendor LUT in two places → one canonical table, mirrored read-only in drawer (§3) |
| P2-001 | 4+ feeds editors → one home, rest mirrors/drills (§1) |
| P2-002 | feeds Suggest silently rewrites geometry → SPEED/CUT split, geometry change explicit + attributed (§2) |
| P2-007 | "feed" vocab leaks into Post → relocate + rename (§6) |
| P7-001 | bare scalars, no origin → ValueProvenance stamp on each value (§5.1) |
| P7-002 | one enum mis-labels four fields → per-field provenance stamp (§5.2) |
| P7-003 | one signal, 3 colors → one RGB per source everywhere (§5.2) |
| P7-004 | three engines indistinguishable after apply → stored source, distinct chip (§5.2) |
| P7-005 | provenance only in modal → chip on summary, drill on click (§5.2) |
| P4-003 | ⚡ glyph = three scopes → value-chip / labeled button / status badge (§4.1) |
| P4-006 | two bare "Apply" buttons → engine-prefixed + glyph; modal Apply rows removed (§4.3) |
| P4-007 | read-only vs actionable rows indistinct → Derived grouping, no DragValue = no apply (§4.2) |
