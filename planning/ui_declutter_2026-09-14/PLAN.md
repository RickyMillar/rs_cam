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
| F-5 | Every project load warning renders at ONE severity, because the typed `ProjectLoadWarning` is flattened to `Vec<String>` at load time. |
| F-6 | The Heights diagram draws a red `BZ` floor on EVERY operation, including the ones that do not read a pinned Bottom Z. |

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

---

## 6. Rulings for this phase

The design lead rules on every question a package would otherwise block on.
Each ruling names the alternative it rejects and why.

### R23 — the per-toolpath `C` / `R` toggles go into `…`, NOT into Overlays

DC1 said "move to the Overlays panel". That is wrong, and reading the code
shows why. `viewport.toolpath_move_visibility` is keyed **per toolpath**;
the Overlays panel's cutting/rapid rows are **global**. A move from a
per-object control to a global one is not a move. It is a deletion.

The two glyphs leave the card and become two check items in the card's `…`
menu. The card loses two elements. The capability stays.

### R24 — one state dot replaces the chip, the `▶` and the `Sim` button

The card carried a `StatusChip`, a `▶` generate button and a `Sim` button,
all three driven by the same `FreshnessState`. One dot carries the state,
its word arrives on hover, and a click generates when the state asks for
generation. `Generate All` and the `…` menu keep the bulk and the explicit
route, so no command loses its last surface.

### R25 — the `MAN` and `TRACE` badges leave the card

`MAN` is a property of the operation TYPE, so every 3D operation carries it
and it separates nothing. `TRACE` is generator-debug provenance and belongs
on the simulation debug surface. Neither changes what gets cut, so Rule 4 of
§4 does not protect them.

### R26 — the swatch IS the drag grip

The card opened with a 10-point grip and a 6-point swatch side by side: two
rectangles, one job each. The swatch takes the drag. One element, no new
drag semantics, and the thing you grab is the thing that names the row.

### R27 — the stats row leaves the card for the inspector

Three numbers per card times nine cards is 27 numbers competing with nine
names. The inspector is open whenever a card is selected and has room. This
is also what lets Rule D hold: the card is ONE height, always, because
nothing on it varies with state.

**Correction, 2026-09-14, after the package landed.** R27's premise was
only partly true. The inspector does NOT carry the card's three figures. It
carries ONE — `{n} moves` — and it draws it unconditionally, so an edited
operation's stale count renders unmarked. The card marked the same figure
`old: ` and faded it (F2.2), for the stated reason that confident numbers
outweigh a quiet state indicator beside them.

Time and distance are not lost: readiness, pre-flight, export and the setup
sheet all state the cycle time without a hover. Only the move count needed
a guard, and DC5 adds it. The lesson is narrow and worth keeping: **"it
moves to a surface that already has room" is a claim about that surface,
and it must be READ before it is ruled.** The inspector had room. It did
not have the figures.

### R28 — the eye is the only always-visible action

Click shows or hides. Double click isolates. The bullseye glyph is deleted.

### R29 — the Tool Library moves to the Setup workspace

The operator asked for "its own tab". A fifth workspace tab adds an element
to the busiest strip in the product, and this phase's rule says a change
that adds must delete two. Setup already holds the project's RESOURCES —
stock, machine, models. A tool is a resource. The Tool Library joins that
list and leaves the Toolpaths workspace completely.

### R30 — a Danger badge keeps its chip; every other badge becomes a dot

Rule E asks for an indicator, not a word. Safety keeps its voice, which is
the rule UP3 already established and its sentry already pins. A Caution or
Ok badge becomes a 6-point dot on the tab's top-right corner with the count
on hover.

### R31 — the warnings surface is a modal, not a floating window

F-4 is caused by a non-modal `egui::Window` that the operator can leave open
over the tab bar. A modal cannot be left over the tab bar, because it is a
thing you open, read, and close. That is Rule A's legitimate use of a
window.

### R32 — the card is ONE row

`swatch · dot · name · tool · eye · …`, at a fixed height. Two rows per card
times nine cards is what made the panel a wall.

### F-5 — the load warnings lose their severity before the GUI sees them

Found 2026-09-14 while verifying DC7, and it is NOT a DC7 regression. It is
older and it sits upstream of the GUI.

`controller/io.rs:521` and `:566` assign `self.load_warnings =
warning_messages`, a `Vec<String>`. The typed `ProjectLoadWarning` enum has
**eight** variants and they do not carry one severity:

- `MissingToolReference`, `MissingModelReference`, `ModelImportFailed` and
  `MissingModelFile` mean an operation CANNOT generate. The project is
  broken until the operator acts.
- `MachineRefFallback` and `UnknownToolType` are warn-and-default. The
  project loaded and it works. The operator may want to know.

DC7 renders all eight as `Role::Caution`, which is the only honest choice
available to it — the type is already gone by the time the controller hands
the GUI a `&[String]`.

**Why this matters more after DC7, not less.** `NoticeStack` caps what it
draws, and its first rule is that severity outranks the cap so every
`Danger` renders. With no warning ever reaching `Danger`, that rule is
inert here: on a project with 30 warnings, a missing tool reference can sit
behind `Show all` under 29 cosmetic ones. The bounded renderer is doing its
job correctly on input that has already lost the distinction it needs.

The fix is to carry `ProjectLoadWarning` to the GUI instead of its
`to_string`, and to map its variants onto `Role`. That is a controller and
`io` change, so it is not this phase's work. Recorded here so the next
phase does not re-derive it.

---

## 7. A finding: UP1 killed a distinction and the sentry did not notice

Found 2026-09-14, while DC5 checked a ruling of mine rather than obeying it.

F2.2 marked an edited operation's figures on the toolpath card in TWO
channels: the prefix `old: `, and a fainter colour. The code said so:

    .color(if is_stale { theme::TEXT_FAINT } else { theme::TEXT_DIM })

**Both arms are the same colour, and have been since UP1.** `theme.rs` maps
`TEXT_DIM` and `TEXT_FAINT` onto one token:

    pub const TEXT_DIM: Color32 = tokens::TEXT_FAINT;
    pub const TEXT_FAINT: Color32 = tokens::TEXT_FAINT;

UP1 collapsed the two names deliberately — §2.4 ruled that two names one
step apart were not two rungs — and it recorded the collapse. What nobody
checked was whether any call site was USING the step as a signal. One was.
So F2.2's two-channel marking silently became one channel, and the card
looked like it faded a stale figure while drawing it at the same weight.

**The sentry stayed green the whole time.** `freshness_surfaces_g_freshrender`
asserted the PREFIX and never the colour, so it could not see the loss.

Two lessons, and the second is the one that generalises:

1. A token collapse is a behaviour change wherever a call site read the two
   names as a CONTRAST. Grep for conditionals that select between the two
   collapsed names before collapsing them, not after.
2. **A sentry that pins one channel of a two-channel signal reports the
   signal as healthy when the other channel dies.** Pin the distinction, not
   one of its carriers.

Nothing is broken today. The card's stats row was the only site that named
both, and DC1 deleted it under R27; the surviving move count in the
inspector marks staleness with the prefix and a hover. DC5 was right to
refuse the fade: restoring the contrast would mean giving the FRESH count a
stronger token, which adds emphasis, and this phase removes it.

### F-6 — the Heights diagram contradicts the sentence printed beside it

Found 2026-09-14 by the F-1/F-2 agent, which correctly did NOT fix it: the
fix changes what the drawing MEANS, so it needed a ruling.

The diagram draws a `BZ` line from `resolved.bottom_z` on every operation,
in `DANGER` red, at the same weight as the real limits and with no
qualifier. But `OperationType::honors_pinned_bottom_z()` answers TRUE for
only three operations — `Adaptive3d`, `UnifiedFinish` and `Waterline`
(`crates/rs_cam_core/src/compute/catalog.rs:618`). Every other operation
floors its cut at `top_z` minus its OWN depth dial and never reads the pin.

`draw_heights_params` already prints `bottom_z_pin_note` beside the row. So
on a pocket the panel says **"Bottom: Not used by this operation. The
projected surface sets the floor."** and the diagram directly under that
sentence draws a red floor at `BZ 26.0`.

Two adjacent surfaces, one operation, opposite answers about where the cut
stops. Red is the strongest thing on the canvas and it is on the line that
does not apply.

**Ruling R33. The diagram agrees with the sentence.** When
`honors_pinned_bottom_z()` is false the `BZ` line is NOT a floor and must
not be drawn as one: it renders in `UNKNOWN`, not `DANGER`, labelled
`BZ (not used)`, with the reason on hover.

It is not deleted, and that is deliberate. The operator SET that value and
can see it in the field above; a diagram that silently omits it invites the
reading "the pin did not take". The honest picture shows the value and says
it does not drive this operation — which is what the sentence beside it
already says.

---

## 8. The finding DC5a was worth doing for

`Apply selected` on the project feeds rollup applied to NOTHING whenever the
per-operation modal was shut. At HEAD:

    fn apply_feeds_project_selected(&mut self) {
        let ids = self.state.feeds_modal.as_ref()
            .map(|m| m.project_selected.iter().copied().collect())
            .unwrap_or_default();
        self.apply_feeds_batch(&ids, "selected toolpaths");
    }

`unwrap_or_default()` on a `Vec` is the EMPTY vector. So with no modal open
the batch ran over zero toolpaths and reported "selected toolpaths" — a
write that succeeds and writes nothing.

**This is the fifth consumer of the modal's project state, and it is the
one that WRITES A RECIPE.** The other four — sort, row toggle, scatter,
select-all — are view controls; a dead view control annoys the operator. A
dead `Apply` changes no feed and no speed while reporting that it did, and
feeds and speeds are what the machine cuts at.

It was LATENT at HEAD rather than live: the button sits inside the modal,
so the modal was open whenever it was clicked. Moving the rollup to
Readiness would have made it live and silent on the first click. DC5a
caught it at the only moment it was cheap to catch.

Three things this teaches, in order of how much they generalise:

1. **`unwrap_or_default()` on a collection is an abstention rendered as a
   pass.** It converts "I could not find the set" into "the set is empty",
   and an empty set is a legal input to a batch. Every other abstention in
   this repo is typed — `None` means NOT MEASURED, `Measurability` names its
   reason. This one had no type to carry the distinction.
2. **State that lives on the wrong object survives the move that exposes
   it.** The rollup's state sat on `FeedsModalState`. Moving the DRAW code
   without the state would have kept every symptom and hidden the cause.
3. **Count the consumers before you move the state.** Four were found by
   reading the handler list. The fifth was found by grepping the FIELD. The
   handler list is the obvious census and it was incomplete.

---

## Follow-up, not scheduled in this phase — one string-aware source stripper

Every source-scanning sentry in `rs_cam_viz` strips `//` comments with its
own private helper, and none of them models string literals. That is not a
style nit. It changes what the scans MEASURE, and it bit twice on 2026-09-14
in opposite directions:

| Scanner | Error | Consequence |
|---|---|---|
| DC5a's unused-import audit | **over-counted usage** — `.radio(…, "Match chart")` satisfied `\bchart\b` | four dead imports read as used and survived the audit; the compiler caught them |
| The sentries' `strip_comments` | **under-counts code** — a `//` inside a string ends the line early | on a `!contains(…)` arm this is a silent false PASS |

**The combination to watch is a negative assertion plus an under-counting
stripper**, because that is the only pairing nobody investigates: a
`contains(…)` arm that under-counts fails noisily, and somebody looks.

Two traps to know before touching this:

- **`tests/panels_read_the_token_module_up1.rs` documents the same blind
  spot and reasons that "an under-count cannot make a failing budget pass".
  That is correct for a BUDGET arm — a `<=` comparison — and FALSE for a
  `!contains` arm, where an under-count is precisely what makes it pass.**
  The sentence is true of its own use and licenses the bug if copied. A
  correct comment is the hardest kind of trap to see.
- An indent marker has the same shape. `inspector_header_wraps_g_reachwrap`
  slices on `"\n    }\n"`, which occurs 103 times in `properties/mod.rs`;
  the cut is correct only because it takes the nearest occurrence after the
  start, an invariant upheld by rustfmt and therefore by the format gate.
  Both its failure modes shorten the slice, and its assertions are negative.

Measured 2026-09-14: no file any of these sentries scans contains a `//`
inside a string literal, so every one of them is sound on today's sources.
That is a property of the sources, not of the scanners.

**The fix is one string-literal-aware stripper shared by every scanner**,
with UP1's converted alongside. It is deliberately NOT done here: it adds a
helper to four or more files, and this phase is judged by what it deletes.
The caveat is recorded at each scanner instead, naming the direction that
scanner errs in and which of its arms are negative.
