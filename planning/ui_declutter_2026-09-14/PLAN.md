# rs_cam GUI — the declutter phase

Date: 2026-09-14. Follows `planning/ui_premium_2026-09-13/`, which is
complete.

**This phase SUBTRACTS.** The previous one could not: rule 1 of its plan was
*"No behaviour change. No control moves, appears or disappears."* Every
package was therefore forbidden from removing anything, so it made each
element better and the product denser. The operator's verdict was *"it has
just polished the clutter"*, and that is structurally correct.

**The rule for this phase is the inverse.** A package here is judged by what
it REMOVES. A change that adds an element must delete two.

---

## 1. What is wrong, as patterns

Twenty-six separate complaints were collected, from the operator and from a
fresh-eyes review of the 2026-09-14 captures. They are six patterns, and
fixing a pattern fixes its instances.

### Pattern A — three nesting mechanisms compete at one level

**`Geometry` is a collapsible disclosure AND a tab, at the same time, holding
different content.** The tab strip sits BELOW the disclosure. The toolpath
inspector's reading order is:

> name field → Generate button → a sentence → `▾ Geometry` disclosure
> (3 fields) → `Hints (1)` disclosure → **tab strip (5 tabs)** → fields

A reader cannot tell what contains what. The app has tabs, disclosures and
hover-reveal, and uses all three at the same level for the same kind of
content.

Instances: Geometry as tab and disclosure; `Hints` as a long disclosure at the
TOP; the Simulation page dense at one level with no dig-deeper; cards showing
all 13 elements always.

**Rule A. One nesting mechanism per level.** Tabs are peer views of ONE
object. A disclosure is for genuinely optional detail and never duplicates a
tab. Hover reveals nothing that is not also reachable another way.

### Pattern B — status is loud, controls are hidden

The status chips are bordered, tinted, glyphed and always on screen. The six
actions on a card are 12-point icons that **appear only on hover**. Meanwhile
the `Sim` / `▶` button is not an action anyone takes, it is a state; and the
`2/5` beside a setup is a state rendered as almost nothing.

The two are inverted: **what you READ is shouting, what you DO is hidden.**

Instances: `OK` / `TRACE` / `PEND` chips overwhelming; six hover-only icons;
`Sim`/`▶` button that should be a status; `2/5` that means nothing obvious;
tab badges taking a tab's worth of space.

**Rule B. Status is quiet and always present. An action is visible and never
hover-only.** One control may carry two actions when the second is a
modifier — one eye, click to show/hide, double-click to isolate.

### Pattern C — the app narrates instead of showing

Prose where a state and an action would do:

- the Simulation Inspector is a **help paragraph** telling you the controls
  are in another panel
- the workspace hint strip, top right, nobody reads twice
- *"This operation requires manual generation. Press G or click Generate."*
- *"No cutting metrics captured — enable capture and re-run."*
- *"Bottom: Not used by this operation. The projected surface sets the floor."*
- the **Project Load Warnings window**, which floats over the workspace tabs
  and covers Setup, Toolpaths and Simulation — you cannot navigate while it is
  open

**Rule C. A sentence becomes a state plus an action.** A set of warnings
becomes a count that opens: `11 warnings`. Nothing that explains where a
control lives survives; the control moves instead.

### Pattern D — the same kind of object is drawn at different weights

Stock and Machine are full cards. A Setup is a full card. A **Tool is a `▸`
disclosure at the bottom of the panel.** The two setup cards are different
heights from each other, because one carries an extra line.

**Rule D. One object kind, one weight, one height.** A card's height does not
vary with its content; overflow goes behind the `…`.

### Pattern E — navigation does not read as navigation

The workspace tabs read as pills: same rounding on all four, the active one
differing only in fill, no shared baseline joining them to the panel below.
The badges sit BETWEEN tabs, so `6 pending` renders between Toolpaths and
Simulation and belongs to neither.

**Rule E. A tab strip is a strip.** Shared baseline, joined to the panel
below, and a badge sits ON its tab as a small indicator with the count on
hover — never as a word in the strip.

### Pattern F — containers clip their content

The Heights diagram is cut off at its container edge, with `Stock` / `Model`
sliced in half, and its model profile draws blank. The Feeds & Speeds tab
overflows its panel. The load-warnings window overlaps the tab bar.

**Rule F. A container sizes to hold its content or scrolls.** Clipping is a
defect, never a layout choice.

---

## 2. The packages

Each names what it DELETES. A package that only moves things is not done.

### DC1 — the toolpath card

**Deletes 8 of 13 elements.** Today: drag handle, colour swatch, status chip,
`MAN` badge, name, tool name, `▶`/`Sim` button, and six hover icons. Nine
cards is about 80 elements in one panel.

Becomes: **swatch · state dot · name · tool · `…`**, with one always-visible
eye. Click shows/hides, double-click isolates.

- `C` and `R` (cutting/rapid visibility) **move to the Overlays panel**, which
  already exists and is where every other viewport filter lives.
- Duplicate, and the rest, go behind `…` and right-click.
- The `▶` / `Sim` button becomes part of the state dot.
- The card is ONE height for every card.

**Operator ruling, 2026-09-14: bury it.** The `⏻` enable toggle moves into
`…` with the rest. The CONTROL is buried; the STATE is not — a disabled
operation already reads `OFF` on its state dot, and that indicator stays
always-visible. That keeps the thing this rule existed to protect: nobody
walks to the machine unaware an operation is off, because the card still says
so. What goes away is the one-click toggle, which is correct — disabling an
operation is not something you should be able to do by brushing a 12-point
icon.

### DC2 — the operations panel header

**Deletes the `Operations` heading and its rule.** `Generate All` takes that
space at full section width, with a spinner on anything generating.
`+ Add` becomes `+`. The `2/5` counter becomes an indicator on the setup card
rather than a fraction in a header.

### DC3 — the workspace bar

**Deletes the badge words from the strip.** Tabs gain a shared baseline and
join the panel below. Each tab carries a small state indicator; the count is
on hover. The right-hand hint strip is deleted outright (Pattern C).

### DC4 — the setup workspace

**Deletes the Stock and Machine cards.** Each becomes ONE line that opens the
right sidebar — they are navigation, not content. Setup cards take one fixed
height. **Tool Library is promoted out of its `▸` disclosure**, either to its
own tab or to parity with setups (Pattern D).

### DC5 — the toolpath inspector

**Deletes one of the two nesting mechanisms.** `Geometry` stops being both a
disclosure and a tab. `Hints` moves off the top and becomes a count.
The three prose lines above the tab strip become one state row.

Also fixes the Feeds & Speeds tab overflowing its container.

#### DC5a — the feeds modal is FOUR tools, at TWO scopes

Measured: `feeds_modal.rs` is **3 450 lines**, and its draw functions divide
cleanly into four jobs that share nothing but a window.

| Job | Functions | Scope | Where it belongs |
|---|---|---|---|
| **Compare & apply** | `comparison_card`, `apply_column`, `context_chip` | this operation | the **Feeds tab**. It is per-operation editing, which is what the inspector is FOR. |
| **Why** | `provenance_disclosure`, `rationale`, `chipload_breakdown`, `warnings`, `chipload_*_attestation`, `engaged_diameter_row` | this operation | an **expand** from the tab. Optional detail is exactly what a disclosure is for (Rule A). |
| **Explore** | `chart_a`, `chart_b`, `chart_c`, `explore_controls`, `scallop_control` | this operation | a **helper modal**. A nomogram you open, drag, and close is a focused tool and a legitimate modal. |
| **Project rollup** | `project_view`, `bottleneck_callout`, `project_scatter`, `machine_envelope` | **the whole project** | NOT here at all. |

**The finding is the fourth row.** `FeedsModalMode` is `{ Toolpath, Project }`
— the modal holds two different SCOPES behind a mode flip. A project-wide
view of every toolpath is reachable only by selecting one toolpath, opening
its modal, and switching mode. That is why the split between the tab and the
modal feels arbitrary: the modal is not "more detail about this operation",
it is a container holding **two scopes and three levels of detail**.

So the operator's instinct — smaller helper modals, or expands — is right,
and the rule that produces it is Rule A: **one nesting mechanism per level,
and a container holds one scope.**

- The Feeds tab becomes authoritative for this operation.
- "Why" is an expand on that tab, not a window.
- "Explore" stays a modal, because it is a tool.
- The project rollup **leaves the modal** and becomes a project-scope surface
  of its own. Readiness is the candidate home, since that workspace already
  answers project-wide questions.

### DC6 — the simulation workspace

**Deletes the Inspector's help paragraph.** The page becomes summary-first:
one verdict line, then dig-deeper. The orphaned bottom-left strip — transport,
three chips, an italic sentence, a button — becomes one grouped bar near the
viewport it drives.

### DC7 — the load-warnings window

**Deletes the window.** It becomes `11 warnings` in the status bar, opening a
bounded `NoticeStack`, which already exists and already has this as one of its
six intended consumers.

---

## 3. Defects, not styling

Triage separately. These are wrong, not ugly.

| Id | Defect |
|---|---|
| F-1 | The **Heights diagram draws no model** for setup-1 operations 2–5, only the stock. |
| F-2 | The Heights diagram is **clipped** at its container; `Stock` / `Model` are cut in half. |
| F-3 | The Feeds & Speeds tab **overflows** its panel. |
| F-4 | The load-warnings window **covers the workspace tabs**, blocking navigation. |

---

## 4. What this phase must not do

- It must not change what a number MEANS, any threshold, or anything in
  `rs_cam_core`.
- It must not remove the last route to a command. The WP23 census
  (`command_surface_completeness.rs`) stays green: a control may move, but the
  command stays reachable.
- It must not hide a state that changes what gets cut. Enabled/disabled stays
  legible.
- It must not undo the token work. Every colour stays a token; the budget
  sentry stays at 1.

## 5. How it is judged

Not by a screenshot pair. **By a count.** Each package states elements before
and after on its surface, and the acceptance is the operator looking at the
surface and finding less on it.

The baseline, measured 2026-09-14 on the wanaka200 capture:

| Surface | Elements |
|---|---|
| One toolpath card, selected | **13** |
| One toolpath card, at rest | **8** |
| The operations panel, 9 cards | **~80** |
| The inspector, above the first parameter | **12**, three of them prose |
