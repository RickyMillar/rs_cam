# rs_cam GUI — design specification

Date: 2026-09-13. Toolkit: egui 0.34.3 + wgpu 29. The toolkit stays.
Companion documents: `AUDIT.md` (evidence), `PLAN.md` (work packages).

This specification defines the visual language only. It changes no
behaviour, no control, no number and no threshold.

---

## 1. What the product is

`rs_cam` is an instrument. An operator uses it to decide whether a program
is safe to cut a real board with a real spindle. The interface must be
**dense, calm, legible at a glance and dark by default**.

Four principles follow. Every rule in this document answers to one of them.

1. **The workpiece is the subject.** The 3D scene is the only saturated
   thing on screen. The chrome around it is neutral and recedes.
2. **One colour, one meaning.** Colour is a scarce resource. It is spent on
   verdicts and on one accent. It is never spent on decoration, on ordering
   or on category.
3. **Absence is a state, and it is designed.** "Not measured" must look
   different from "measured and zero", and neither may look like "clean".
4. **Weight before position.** Hierarchy comes from size, weight and colour
   first, and from indentation and rules second.

### The look to avoid

Two house styles are common and both are wrong here.

- Cream ground, a serif face and a terracotta accent. It is warm, editorial
  and slow. This product is neither warm nor editorial.
- Near-black ground with one acid accent on every interactive element. It
  reads as a marketing site for a developer tool. It also spends a
  high-chroma hue on chrome, which principle 2 forbids.

The target reads as **anodised instrument metal**: a cold low-chroma
graphite, one cool accent, and a warm workpiece inside it. The contrast
between a cool shell and a warm part is the product's identity and it is
already latent in the renderer.

---

## 2. Tokens

All tokens live in one module, `crates/rs_cam_viz/src/ui/tokens.rs`.
`ui/theme.rs` keeps its existing public names and re-exports from
`tokens.rs`, so no call site breaks in the token package.

### 2.1 Grid

The base unit is **4 points**. Seven steps, and no others.

| Token | Points | Use |
|---|---|---|
| `SPACE_0` | 0 | flush |
| `SPACE_1` | 2 | inside a chip, between a glyph and its word |
| `SPACE_2` | 4 | between rows of one group |
| `SPACE_3` | 8 | between groups, panel side padding |
| `SPACE_4` | 12 | between sections |
| `SPACE_5` | 16 | panel top padding, window inner margin |
| `SPACE_6` | 24 | between major blocks on a page |
| `SPACE_7` | 32 | above a page-level action row |

`item_spacing` becomes `(SPACE_2, SPACE_2)` — `(4, 4)`, replacing today's
`(6, 4)`.

**Row rhythm is a token of its own, and it is not on the spacing scale.**

| Token | Points | Use |
|---|---|---|
| `ROW_DENSE` | 22 | a parameter row, a key-value row, a table row |
| `ROW_ACTION` | 26 | a row carrying a control, and the minimum control height |

A parameter row sets a **fixed line box of 22 points**. It does not derive its
height from `item_spacing` plus a text ascent, because that drifts with the
font and with any caption inside the row. Eight parameter rows occupy exactly
176 points.

**Density is the point of an instrument.** The first drawn specimen of this
specification ran parameter rows at 26 points, from `item_spacing` alone, and
the operator's verdict was that it "looks a bit too spaced apart" and should
"retain some density". At 26 points an inspector reads as a settings dialog;
at 22 it reads as a control panel. A section header takes `SPACE_4` above and
`SPACE_2` below, not `SPACE_5` / `SPACE_3`, for the same reason.

This is the one place where legibility and density genuinely trade against
each other, and the resolution is that **the type grows and the leading does
not**. Caption text rises from 9 points to 11, which is where the legibility
win is, while the row it sits in gets tighter.

Retire 6.0, 10.0, 14.0, 18.0, 28.0, 40.0 and 48.0. Map each to the nearest
step; 6 goes to 8, not to 4, so groups do not collapse.

### 2.2 Radius

Two values. No others.

| Token | Points | Use |
|---|---|---|
| `RADIUS_SM` | 4 | chips, buttons, inputs, combo boxes, swatches |
| `RADIUS_MD` | 8 | cards, windows, popovers, banners |

Set `Visuals::widgets.*.corner_radius` to 4 and `window_corner_radius` and
`menu_corner_radius` to 8, so egui's own defaults (2 and 6) stop competing.
The asymmetric workspace-tab radius stays, because a tab really does join
the panel below it; express it as `RADIUS_SM` on the two top corners.

### 2.3 Elevation

Four planes. Elevation is carried by **fill and shadow**, never by a border
alone.

| Token | Fill | Shadow | On screen |
|---|---|---|---|
| `SURFACE_SUNKEN` | `#15171A` | none | viewport ground, text-input wells, timeline track |
| `SURFACE_BASE` | `#1B1E22` | none | panels, status bar, menu bar |
| `SURFACE_RAISED` | `#22262B` | none | cards, list rows, grouped fields |
| `SURFACE_OVERLAY` | `#282D33` | `SHADOW_OVERLAY` | windows, popovers, toasts, menus |

`SHADOW_OVERLAY` is `egui::epaint::Shadow { offset: [0, 8], blur: 24,
spread: 0, color: rgba(0, 0, 0, 110) }`. It is the product's only shadow.

A modal window additionally paints a scrim: a full-viewport rect at
`rgba(8, 9, 11, 140)` behind the window. The pattern already exists once, at
`ui/optimize_project.rs:572-574`; it becomes the rule.

### 2.4 The neutral ramp

The bias is **cold, toward blue-green**, at very low chroma (HSL saturation
6 % to 10 %, hue 205). It replaces today's accidental violet, which came
from two unrelated literals (`AUDIT.md` D-07).

| Token | Hex | Use |
|---|---|---|
| `INK_00` | `#0F1113` | pure ground, behind the scrim |
| `INK_05` | `#15171A` | `SURFACE_SUNKEN` |
| `INK_10` | `#1B1E22` | `SURFACE_BASE` |
| `INK_15` | `#22262B` | `SURFACE_RAISED` |
| `INK_20` | `#282D33` | `SURFACE_OVERLAY` |
| `INK_25` | `#31373E` | hairline rules, disabled fills |
| `INK_35` | `#454D56` | control borders, chip strokes |
| `INK_50` | `#737C87` | `TEXT_FAINT` — units, provenance |
| `INK_65` | `#8C959F` | `TEXT_MUTED` — secondary text, labels |
| `INK_80` | `#B4BCC5` | `TEXT_BODY` — default label text |
| `INK_95` | `#E2E7EC` | `TEXT_STRONG` — values, names, headings |

Contrast is **computed, not estimated** (WCAG 2.1 relative luminance).
Ratios against the four surfaces:

| Token | SUNKEN | BASE | RAISED | OVERLAY |
|---|---|---|---|---|
| `INK_50` `TEXT_FAINT` | 4.24 | 3.95 | 3.60 | 3.28 |
| `INK_65` `TEXT_MUTED` | 5.91 | 5.51 | 5.01 | 4.57 |
| `INK_80` `TEXT_BODY` | 9.36 | 8.71 | 7.93 | 7.23 |
| `INK_95` `TEXT_STRONG` | 14.43 | 13.44 | 12.23 | 11.15 |

`INK_50` was `#6B747E` in the first draft and fell to **2.92** on
`SURFACE_OVERLAY`, below even the 3:1 large-text floor. It is lightened to
`#737C87`, which clears 3:1 on every surface. **`TEXT_FAINT` is still
forbidden below 12 points and forbidden for any sentence.** It exists for a
unit suffix beside the number it belongs to.

For comparison, the palette shipping today fails twice. `theme::TEXT_FAINT`
`(100, 100, 115)` reads **2.85** against the panel fill and is used for
9-point text. `theme::ERROR` `(220, 80, 80)` reads **4.19**, below the
normal-text floor, and it is the colour of the `ERR` chip and of collision
text.

### 2.5 The accent

**One accent. `ACCENT = #5B9DD9`.** A cool blue at moderate chroma. It
carries three things and nothing else:

- selection (a list row, a tab, a tree item),
- keyboard focus (a 2-point ring at `ACCENT`, always, on every control),
- the primary button fill.

Two derived tokens:

| Token | Hex | Use |
|---|---|---|
| `ACCENT_QUIET` | `#2C4459` | selected row fill, active tab fill |
| `ACCENT_PRESSED` | `#4A85BB` | primary button, pressed |

The accent must never carry a verdict. A selected row that is also in error
shows the error on its chip and the selection on its left rule; the two do
not blend.

### 2.6 Semantic colours

Five roles. Each has a text tone and a quiet fill for chips. Nothing else
in the product may use these hues.

| Role | Text | Chip fill | Worst ratio | Means |
|---|---|---|---|---|
| `OK` | `#5FBF7A` | `#1C3323` | 5.96 | within a band, current, clear, pass |
| `CAUTION` | `#E0A83C` | `#3A2E12` | 6.23 | stale, waiting, elevated, review |
| `DANGER` | `#E87B77` | `#3A1D1E` | 4.98 | exceeds, collision, error, refusal |
| `INFO` | `#5B9DD9` | `#1E2E3D` | 4.80 | informational, the accent reused |
| `UNKNOWN` | `#8D9AA8` | `#23282E` | 4.84 | **not measured** |

"Worst ratio" is the lowest contrast that role reaches on any of the four
surfaces or on its own chip fill. Every value clears 4.5. Two tones were
lightened after the first draft failed: `DANGER` was `#E2635F` and read 4.48
on a card, and `UNKNOWN` was `#7E8A96` and read 4.22 on its own chip.

`UNKNOWN` is new and it is the most important addition in this section.
Today "not measured" is drawn as an em dash in whatever grey is nearby, so a
gate that abstained and a gate that passed look alike at a glance. `UNKNOWN`
gives abstention its own tone, one step cooler and flatter than `TEXT_MUTED`,
and always with the glyph `—` and never with a number.

Three rules follow.

1. A verdict colour is never used for ordering, for category or for
   decoration. The Heights diagram (`AUDIT.md` D-20) therefore stops using
   red, green, amber and yellow for five heights; see §7.2.
2. A safety row and a procedure row never share a colour. "Not run" is
   `UNKNOWN`, not `CAUTION`. `CAUTION` means a measurement came back and it
   needs review.
3. Colour is never the only channel. Every verdict also carries a glyph:
   `✓` OK, `!` CAUTION, `✕` DANGER, `—` UNKNOWN.

### 2.7 Structure, surfaces and tints

Derived from the call sites, not invented. A colour census of the crate found
**40 distinct intents** across 445 literals, and `theme.rs`'s 20 constants
cover **13**. Everything in §2.7 to §2.9 exists to close the other 27.

| Token | Hex | Sites today | Values today |
|---|---|---|---|
| `HAIRLINE` | `#31373E` | 20 | **11** |
| `BORDER` | `#454D56` | — | — |
| `INPUT_WELL` | `#15171A` | 2 | 1 |
| `SCRIM` | `rgba(8, 9, 11, 140)` | 4 | 4 |

**Surface tints.** A `Banner`, a table row and a readiness row all need a
tinted ground, and today four of those are hand-mixed per site.

| Token | Hex | Sites today | Values today |
|---|---|---|---|
| `TINT_OK` | `#1C3323` | 1 | 1 |
| `TINT_CAUTION` | `#3A2E12` | 9 | **8** |
| `TINT_DANGER` | `#3A1D1E` | 6 | **5** |
| `TINT_INFO` | `#1E2E3D` | — | — |
| `TINT_UNKNOWN` | `#23282E` | — | — |

These are the same five values §2.6 uses as chip fills. One family serves
both; a chip is a small tinted surface.

### 2.8 The diagram palette

The operation preview thumbnails, the entry diagram and the height diagram
are drawings, not UI. They carry **41 literals over 5 intents** today, all
outside `theme.rs`, and they are the reason `TEXT_FAINT` collides with an
op-diagram dim colour at 11 sites.

| Token | Hex | Means |
|---|---|---|
| `DIAGRAM_CANVAS` | `#14171A` | the drawing's ground |
| `DIAGRAM_INK` | `#5B9DD9` | the path being described |
| `DIAGRAM_MATERIAL` | `#2A2F36` | stock or material body |
| `DIAGRAM_TOOL` | `#8C959F` | the cutter body |
| `DIAGRAM_DIM` | `#454D56` | construction lines, retired geometry |

`DIAGRAM_INK` takes the accent because a diagram is explanatory, not a
verdict. It must never take `OK` or `DANGER`.

### 2.9 Data scales

Three ordered scales. **A data scale is derived from one hue by lightness,
never assembled from separate hues**, because a category wheel spends the
semantic palette (see §7.1 and `AUDIT.md` D-23).

This section named three scales and gave no values. **UP1 derived them and
they are now recorded here.** Every step was checked against all four
surfaces at the 3:1 WCAG floor for a graphical object, because these are
drawn marks rather than text.

**`SPAN_SCALE`** — six steps, cool blue to cool cyan, hue 210 to 186 with
lightness rising. It colours span kinds, which today carry **17 distinct
values over 24 sites** and are the worst drift in the product.

| Step | Hex | Worst ratio |
|---|---|---|
| 0 | `#4482C1` | 3.44 |
| 1 | `#5A98C6` | 4.45 |
| 2 | `#70ACCB` | 5.59 |
| 3 | `#86BED1` | 6.81 |
| 4 | `#9BCDD8` | 8.01 |
| 5 | `#AFDADE` | 9.19 |

The first draft started at lightness 46 and put step 0 at **2.88**, under
the 3:1 floor. The scale was lifted. Adjacent steps now separate by 1.15 to
1.29, which is what makes the order readable rather than merely present.

**`CHART_SERIES`** — four steps, hue 205 to 175. The simulation signal strip
and the feeds charts today carry 9 values over 11 sites.

| Step | Hex | Worst ratio |
|---|---|---|
| 0 | `#3D8BC2` | 3.75 |
| 1 | `#64B0C9` | 5.68 |
| 2 | `#89CBD1` | 7.62 |
| 3 | `#ADDCD8` | 9.25 |

**`LANE_SCALE`** — the four compute-lane states, retuned onto the ramp and
onto the semantic roles rather than onto four unrelated hues.

| State | Token | Hex |
|---|---|---|
| idle | `INK_65` | `#8C959F` |
| queued | `ACCENT` | `#5B9DD9` |
| running | `CAUTION` | `#E0A83C` |
| cancelling | `DANGER` | `#E87B77` |

A lane state is a PROCEDURE state, not a verdict, so reusing `CAUTION` and
`DANGER` here is the one deliberate exception to §2.6's rule that nothing
else may use those hues: "running" and "cancelling" are the same escalation
the roles already encode, and giving them private hues would spend two more
slots on the same meaning.

### 2.10 Coverage rule

**Every new colour is a token or it does not ship.** UP1's sentry counts
literals outside the token modules and UP8 drives that count to zero.

Twelve values currently serve two intents each. Four of those are load
bearing and the token set separates them:

| Value | Intent A | Intent B |
|---|---|---|
| `#DC3C3C` | error text | the viewport X-axis gizmo |
| `#DC5A5A` | the `exceeding` verdict pill | the chipload band-max line, and the Confirm-delete button |
| `#50B450` | `SUCCESS_BRIGHT` | the feeds vendor-band fill |
| `#787882` | `TEXT_DIM` | `SpanKind::RapidOrderBarrier` |

An axis gizmo that is the same red as an error, and a vendor band that is the
same green as a pass, are exactly what principle 2 exists to stop.

---

## 3. Type

### 3.1 Faces

| Family | Face | Licence | Use |
|---|---|---|---|
| `Proportional` | **Inter** (Regular, Medium, SemiBold) | SIL OFL 1.1 | all prose, labels, headings, buttons |
| `Monospace` | **JetBrains Mono** (Regular, Medium) | SIL OFL 1.1 | every measured value, every code and G-code string |

Both ship as `.ttf` files in `crates/rs_cam_viz/assets/fonts/`, loaded with
`include_bytes!`, exactly as the two Noto symbol fonts are today
(`app.rs:1085-1111`). **Neither adds a Cargo dependency.** Keep both Noto
symbol fallbacks and keep their order.

Inter replaces Ubuntu-Light. The weight axis matters more than the shape: a
light face has no room below it, so `.strong()` has to do all the work.
Regular, Medium and SemiBold give three real rungs.

If the licence review rejects either face, the fallback is to keep egui's
Hack for monospace and to raise every size in §3.2 by one point. The scale
is the load-bearing part, not the face.

### 3.2 The scale

Eight styles. **Five map onto egui's built-in `TextStyle` slots and apply
automatically. Three do not exist as global styles and must be helper
functions.** See §10.3 for why: a custom `TextStyle::Name` is a legal map key
but `ui.label()` never reads it.

| Name | egui slot | Size | Weight | Family | Use |
|---|---|---|---|---|---|
| `Heading` | `TextStyle::Heading` | 15 | SemiBold | Inter | panel title, modal step title |
| `Body` | `TextStyle::Body` | 13 | Regular | Inter | default label and sentence |
| `ButtonText` | `TextStyle::Button` | 13 | Medium | Inter | every button |
| `Numeric` | `TextStyle::Monospace` | 13 | Regular | JetBrains Mono | **every measured value** |
| `Caption` | `TextStyle::Small` | **11** | Regular | Inter | units, provenance, supporting sentence |
| `Display` | helper | 20 | SemiBold | Inter | a window title, a page title. Rare. |
| `Subhead` | helper | 12 | SemiBold | Inter | section header |
| `Micro` | helper | 10 | SemiBold, upper, +0.8 pt tracking | Inter | chip text only |

**The single highest-leverage line in this whole specification is raising
`TextStyle::Small` from 9 to 11.** All 516 `.small()` call sites read that
slot, so one assignment in `configure_theme` lifts the product's entire
caption layer off the floor **without editing a single call site**. UP1
delivers most of the legibility win on its own.

**The floor is 11 points.** Nothing renders below 11 except `Micro`, which
is a chip: short, upper-case and high contrast by construction.

Line height is **17 points for `Body` and 15 for `Caption`** (about 1.3),
set per call site through `RichText::line_height`. It cannot be set
globally on 0.34; see §10.2.

### 3.3 Migration of `.small()`

516 sites carry `.small()`. They divide into four groups.

| Today | Becomes | Editing needed |
|---|---|---|
| A unit, a provenance stamp, a `configured N` note | `Caption` at 11 | **none** — `.small()` already reads that slot |
| A whole sentence | `Body`, or `Caption`, never below 11 | drop `.small()` where it is a sentence |
| A section header (`.small().strong()`, 41 sites) | `Subhead` helper | call site |
| A chip or badge word | `Micro` helper | call site |

So of 516 sites, the great majority need **no edit at all** once the `Small`
slot moves to 11. Only the 41 header sites and the chip sites are hand work.

The sites concentrate in the modals, not the inspector: `feeds_modal.rs` 98,
`optimize_modal.rs` 59, `sim_diagnostics.rs` 56, `multitool_planner.rs` 56,
`optimize_project.rs` 53. Those five hold 322 of the 516, and four of them
are resizable windows with room to grow. The toolpath inspector holds 44.
**The density risk this specification was most worried about is small, and
it lands on the modals.**

The tool-load caution, the readiness cycle-time note and the workspace hint
(`AUDIT.md` D-01, D-39) are all in the last group.

### 3.4 Numbers

**Every measured value renders in `Numeric`.** That is a hard rule and it
has a toolkit reason: egui 0.34 exposes no OpenType feature switch, so
tabular figures cannot be enabled on a proportional face. Alignment comes
from the monospace family or from nowhere.

Rules:

- A value column is right-aligned when the rows share a unit, and
  left-aligned when they do not.
- A unit is `Caption` in `TEXT_FAINT`, one `SPACE_1` after the number,
  never inside the `Numeric` run.
- Group thousands with a thin space: `661 212`, not `661212`.
- A value the product did not measure is the em dash `—` in `UNKNOWN`.
  It is never `0`, never `0.0` and never blank.

---

## 4. Components

Every component lives in `crates/rs_cam_viz/src/ui/components/`, beside the
nine that already exist. Existing components are extended, not replaced.

### 4.1 `SectionHeader`

**The one section mark.** Replaces the four forms in `AUDIT.md` D-09.

Drawn as: `Subhead` text in `TEXT_MUTED`, then `SPACE_2`, then a 1-point
hairline in `INK_25` running to the panel edge. `SPACE_4` above, `SPACE_2`
below. An optional trailing slot holds one right-aligned action at
`Button::Quiet`.

A collapsible section uses the same mark plus a 10-point chevron on the
left, which rotates over 160 ms.

`UiExt::named_section` becomes a thin call into `SectionHeader`, so the
existing call sites gain the treatment without being rewritten.

### 4.2 `Card`

Replaces `theme::card_frame`.

`SURFACE_RAISED` fill, `RADIUS_MD`, `SPACE_3` inner margin, no border.
Selected state: fill `ACCENT_QUIET`, plus a **3-point left rule in
`ACCENT`** running the full card height. The left rule is what makes
selection readable at a glance in a list of twelve cards; a fill change
alone is not.

Hover raises the fill by one ramp step over 120 ms. A card is never
outlined; outline is reserved for focus.

### 4.3 `StatusChip`

The rendered form of `toolpath_panel::status_chip`, whose seven-state pure
function stays exactly as it is (`AUDIT.md` §8).

Drawn as: `Micro` text on the role's chip fill, `RADIUS_SM`, padding
`(SPACE_2, SPACE_1)`, a 1-point stroke in the role's text colour at 40 %
alpha. Minimum width 34 points so a column of chips aligns.

| State | Word | Role |
|---|---|---|
| `NoResult` | `PEND` | `UNKNOWN` |
| `Regenerating` | `GEN` | `INFO` |
| `Current` | `OK` | `OK` |
| `EditedSince` | `STALE` | `CAUTION` |
| `WaitingOnUpstream` | `WAIT` | `CAUTION` |
| `Disabled` | `OFF` | `UNKNOWN` |
| `Error` | `ERR` | `DANGER` |

`PEND` moves from `TEXT_DIM` to `UNKNOWN`, because "never generated" is
exactly an unmeasured state. Every other colour keeps its current meaning.
The hover text is unchanged.

`TRACE` and every other provenance mark uses the same chip form at
`UNKNOWN`, so the row no longer draws the weaker signal as the stronger
shape (`AUDIT.md` D-08).

### 4.4 `CountPill` — extended, not replaced

`ui/components/pill.rs` keeps its `Verdict` / `Observation` and `ReadOnly` /
`Actionable` axes. Three changes:

- it adopts the `StatusChip` geometry, so a chip and a pill are one shape;
- `Actionable` gains a chevron `›` and an `ACCENT` hover, so it reads as a
  target;
- the Simulation inspector's `collisions 5` becomes a `Verdict` pill at
  `DANGER` instead of loose red text (`AUDIT.md` D-28).

### 4.5 `Button`

Four variants. Minimum height 26 points, set **once** through
`Style::spacing::interact_size.y`, which egui documents as "the default
height of button, slider, etc." No per-call `.min_size()` is needed
(§10.6).

| Variant | Fill | Text | Use |
|---|---|---|---|
| `Primary` | `ACCENT` | `INK_05` | **one per screen.** Generate, Run simulation, Next, Apply. |
| `Default` | `INK_15`, 1 pt `INK_35` border | `TEXT_BODY` | every ordinary action |
| `Quiet` | transparent | `TEXT_MUTED` | inline actions, row actions, a header's trailing slot |
| `Danger` | transparent, 1 pt `DANGER` border | `DANGER` | delete, discard, remove |

Hover lifts the fill one ramp step over 120 ms. Focus draws a 2-point
`ACCENT` ring outside the shape on every variant. **The ring is drawn by the
component, not by the theme** — egui 0.34 draws no focus indicator by
default and renders a focused widget in its `active` visuals instead, which
reads as "pressed" (§10.5). Disabled drops to
`INK_25` fill and `INK_50` text, and **a disabled control always carries a
hover that states the reason** — the rule the Overlays registry already
keeps for 40 rows (`AUDIT.md` D-38).

`Primary` is the most constrained token in this document. §6 names the one
primary action per screen. Two primaries on one screen is a defect.

### 4.6 `KeyValueRow` — the extension of `ValueRow`

`ui/components/value_row.rs` already owns the labelled input and the ⚡
suggest pill. It gains a fixed four-slot geometry.

```
[ label ........ ][ value ][ unit ][ trailing ]
  TEXT_MUTED       Numeric  Caption  pill / chip / ⚡
  Body, left       right    FAINT    right-aligned
```

- The label column width is **one value per panel**, computed once from the
  longest label in that panel and held for every row. This removes the
  two-indent defect (`AUDIT.md` D-15).
- `trailing` never overflows. When the panel is too narrow the trailing slot
  wraps to a second line at `Caption` rather than clipping. That closes
  `AUDIT.md` D-16 for value rows. **A global default exists and UP1 should
  set it**: `Style::wrap_mode` takes an `Option<TextWrapMode>` and changes
  the default for every label at once (§10.7). The component rule then
  handles the cases the global default gets wrong.
- A read-only row and an editable row differ by the value slot: an editable
  value sits in a `SURFACE_SUNKEN` well with `RADIUS_SM`; a read-only value
  has no well. The difference is then visible at the value, which is where
  the eye already is (`AUDIT.md` D-13).
- The row is `ROW_DENSE`, 22 points, as a fixed line box (§2.1). The well is
  18 points inside it, so a column of wells has 2 points of air above and
  below and the rows still read as a stack rather than a list of boxes.
- A well has a **minimum width**, so a column of values with different digit
  counts keeps one right edge.

### 4.7 `DataTable`

A thin wrapper over `egui::Grid`. The crate has 89 `Grid::new` sites, no
`egui_extras` and no `TableBuilder`, so this is a styling wrapper and not a
new dependency.

- Header row: `Subhead` in `TEXT_MUTED`, hairline below.
- Zebra: even rows at `SURFACE_RAISED`, odd rows transparent.
- Numeric columns right-aligned in `Numeric`.
- Row height `ROW_DENSE`, 22 points, matching `KeyValueRow` so a table and a
  parameter list share one rhythm. `SPACE_3` between columns.
- Hover tints the whole row one ramp step.

The `Current / Recommended / Δ` table in the Feeds modal is the reference
implementation (`AUDIT.md` §8, item 5). It becomes a `DataTable` and every
comparison in the product uses the same form.

### 4.8 `EmptyState`

Replaces the 49 italic one-liners. **The reference implementation already
exists** at `ui/sim_op_list.rs:128-170`: a filled frame, a strong headline,
and guidance that branches on the situation. This component generalises it.


Centred in the available space, at most 280 points wide:

1. a 24-point outline glyph in `INK_35`,
2. `SPACE_3`,
3. one `BodyStrong` line in `TEXT_MUTED` saying what would be here,
4. `SPACE_2`,
5. one `Caption` line in `TEXT_FAINT` saying how to fill it,
6. `SPACE_4`,
7. **at most one** `Button::Default` that does step 5, when one exists.

Italic is retired from empty states entirely. Italic in this product means
"a machine-generated note", and an empty state is a designed screen.

### 4.9 `Banner`

The full-width message block. One per surface, at the top of it.

`RADIUS_MD`, `SPACE_3` inner margin, the role's chip fill, a **3-point left
rule** in the role's text colour, a leading glyph, `BodyStrong` title, and
an optional `Caption` line. An optional trailing `Button::Quiet`.

A `Banner` never renders below 11 points. The tool-load caution, the
readiness cycle-time note and the load warnings are all sentences, and
§3.3 puts every sentence at `Body` or `Caption`.

The Readiness banner, the `NOT MEASURED` strip and the load warnings all
become `Banner`.

**One banner per surface, whatever N is.** When more than one condition holds,
the banner takes the **worst** severity and its title names the population:
`3 problems — 1 collision, 2 cautions`. It carries at most two lines of
detail. Everything else goes to a `NoticeStack` beneath it. Two stacked
banners is a defect; the second one is always a stack row.

### 4.10 `Toast`

`SURFACE_OVERLAY` fill, `SHADOW_OVERLAY`, `RADIUS_MD`, a 3-point left rule
in the role colour, `Body` text, maximum width 380 points.

It slides in from the right over 140 ms with an opacity ramp, and fades out
over 200 ms. The repaint interval drops from 1 s to 16 ms while any toast is
alive, so a toast leaves when its TTL says so (`AUDIT.md` D-05).

**The visible stack caps at four**, per `NoticeStack`. A fifth arrival
coalesces the oldest non-danger toasts into one counter row reading
`+3 more · View`, which opens the notification list. A `DANGER` toast never
coalesces. A burst from one operation — nine toasts from one `generate_all` —
collapses to one row with a `×9` multiplier rather than filling the corner.

Two things this does **not** change: every toast keeps its own TTL and
severity exactly as today, and `get_notifications` keeps publishing the whole
stack. This is a rendering cap on one surface, not a change to the
notification model, and a test asserting what the operator saw still reads the
full stack.

### 4.11 `NoticeStack` — the bounded renderer for N of anything

**No surface in this product renders an unbounded list.** Six places can carry
an arbitrary number of items, and two of them are unbounded in code today:
the toast stack iterates every active notification (`app.rs:932`) and the load
warnings window iterates every warning (`app.rs:911`). The others are triage
advisories, simulation hotspots (293 on the reference job), the per-toolpath
finding set, and inspector cautions.

Core already solved this once and solved it well. `SimulationTriage` caps
advisories at 10 per toolpath and 50 per project, dedupes on a spatial key,
and publishes `truncated` alongside a **true pre-cap** `total_matching`.
`NoticeStack` is that discipline made visual, and every one of the six
surfaces uses it.

**Ordering.** Severity first — `DANGER`, `CAUTION`, `UNKNOWN`, `INFO` — then
stable within a severity: newest first for toasts, subject order for
findings. Ordering never depends on arrival order across severities.

**Collapsing.** Items sharing an id and a dedupe key render as one row with a
`×N` multiplier, expandable to the individual items. Five rapid collisions at
five different moves are one row reading `Rapid collision ×5`, not five rows.

**The cap, and the one rule that outranks it.** A stack renders at most
`visible_cap` rows — 4 in a panel, 3 in a window, 4 for toasts.
**Every `DANGER` item renders, always, past the cap.** The cap applies to
`CAUTION` and below. A stack of 3 dangers and 40 cautions with a cap of 4
shows all 3 dangers, 1 caution and an overflow row — never 4 cautions. On a
machine that cuts real material, a count may be truncated and a severity may
not.

**The overflow row states the true total.** It reads
`Showing 4 of 293 · Show all`, and 293 is the pre-cap population, not the
rendered count and not a remainder. This product has already been bitten by a
count that meant three different things under one name; an overflow row that
says `+289 more` invites the same error. Where the population is itself
capped upstream, the row says so: `Showing 4 of 50+ · Show all`.

**Grouping.** Above 20 items the stack groups by subject — by operation, by
setup — and each group header carries its own count. 293 hotspots is not a
list, it is a distribution, and it reads as one line per operation.

**Empty is nothing.** A stack with no items renders no header, no frame and no
height. It does not become an `EmptyState`; an absent problem is not an empty
screen.

### 4.12 `NotMeasured`

The rendered form of principle 3. An inline run of `—` in `UNKNOWN` at
`Numeric` size, with a hover that names the reason. A row of them reads as a
deliberate row of blanks, not as missing text.

---

### 4.13 Rulings on the gaps and contradictions §4 left open

UP2 read §4 against §2 and §3 and found **six contradictions and thirteen
underspecified points**. The operator delegated these on 2026-09-13 ("you
decide on all the things that need my call"). Each is ruled below with its
reason, so a later reader can overturn a ruling rather than re-derive it.

#### The six contradictions

**R1. The overflow row reads `Showing 4 of 293 · Show all`, everywhere.**
§4.10 said `+3 more · View` and §4.11 forbids exactly that form. §4.11 wins,
including for toasts. A remainder hides the true total, and hiding the true
total is the defect §4.11 was written to stop. §4.10's wording is retired.

**R2. A chip's stroke is `BORDER`.** §4.3 asked for the role's text colour at
40 % alpha; §2.4 assigns `INK_35` the use "chip strokes" and §2.7 names it
`BORDER`. The token wins. A per-role alpha blend is a computed value that the
literal sentry cannot see and that differs at every call site — the exact
pattern §2.7 exists to end. The chip already carries its role three times
over, in fill, text and glyph.

**R3. `CountPill::Actionable` does NOT take the accent on hover.** §2.5 rules
that the accent carries selection, focus and the primary button fill "and
nothing else", and a fourth carrier would dilute it. An actionable pill lifts
its fill one step (R9) and shows a pointer cursor. That is enough.

**R4. The disclosure chevron is PAINTED, not set as type.** §3.2 floors text
at 11 points and §4.1 wanted a 10-point chevron. A triangle drawn with
`Painter` is a shape and the floor does not apply to it. This also drops a
glyph-coverage dependency.

**R5. The hairline token is `HAIRLINE`, never `INK_25`.** Same hex, but a
component names the semantic token, not the ramp position. §4.1 and §4.7 are
amended.

**R6. `BodyStrong` is defined: 13 points, Inter Medium.** §4.8 and §4.9 both
require it and §3.2 never defined it. It is the emphasis rung INSIDE body
text. Medium rather than SemiBold, because SemiBold at 13 would compete with
`Heading` at 15 SemiBold and flatten the hierarchy. It is a helper, not an
egui slot, so the scale is still five built-in slots plus four helpers.

#### The thirteen underspecified points

**R7. A value well has a minimum width of 58 points.** §4.6 requires one and
gives no number. 58 is the value the drawn specimen used, and the operator
approved that specimen's density on 2026-09-13.

**R8. `Button::Primary` text is `INK_05`, and the ratios are now published.**
`INK_05` on `ACCENT` is **6.22**; on `ACCENT_PRESSED` it is **4.59**. Both
clear the 4.5 floor. The alternative was tested and rejected: `INK_95` on
`ACCENT` reads **2.32**, so a white-on-blue primary button would have failed
§9 outright.

**R9. "One ramp step" means the next SURFACE token, not the next ink.** The
ramp is unevenly spaced, so a lightness delta and a token step differ. The
lift is defined over the surface ladder only:
`SUNKEN → BASE → RAISED → OVERLAY → OVERLAY` (it clamps), and a transparent
fill lifts to `SURFACE_RAISED`. Any other fill does not lift; a tinted chip
signals hover with its stroke instead.

**R10. `NoticeStack` orders five roles, and `OK` sorts last.**
`DANGER`, `CAUTION`, `UNKNOWN`, `INFO`, `OK`. §4.11 listed four and omitted
`OK`. An `OK` notice may enter a stack — "this one passed" is a legitimate
notice — and it needs the least attention, so it sorts behind everything.

**R11. Toast-out easing is `cubic_in`.** §5 names `cubic_out` for ease-out and
says only "ease-in" for the toast leaving. `cubic_in` is the symmetric
partner.

**R12. `NoticeStack` expands INLINE and lifts its cap when expanded.**
`Show all` renders every item in place, still ordered and still deduplicated.
Expanding a `×N` group behaves the same way. No separate surface, and no new
control: UP2 to UP8 add none, and a new surface would need a registry row.

**R13. `NoticeStack` row geometry.** A row is at least `ROW_DENSE` and grows
when its text wraps. Padding is `SPACE_2` vertical and `SPACE_3` horizontal.
The severity glyph leads, then the text at `Body`. The row's ground is the
severity's `TINT_*`. A group header is `Subhead` in `TEXT_MUTED`. The
overflow row is `Caption` in `TEXT_MUTED` on no fill.

**R14. A card that is both selected and hovered keeps `ACCENT_QUIET` and
gains a 1-point `ACCENT` stroke.** Selection owns the fill, because selection
is state and hover is transient. The stroke is the hover channel, and it is
the accent because the card is already selected, so no fourth carrier is
introduced.

**R15. `CountPill`'s two families are separated by ground, not by hue.** A
`Verdict` pill takes the role's `TINT_*` ground, the role's text and the
role's glyph. An `Observation` pill takes `SURFACE_RAISED`, `TEXT_MUTED` and
no glyph. An observation therefore cannot borrow a verdict colour, which is
§2.6 principle 1 applied to the pill.

**R16. `CountPill` moves onto the grid.** It currently synthesises its fill
with `linear_multiply(0.10)` and its stroke with `linear_multiply(0.55)`, and
uses `corner_radius(6)` and `Margin::symmetric(5, 1)` — none of which is a
token. It takes `TINT_*`, `BORDER`, `RADIUS_SM` and `SPACE_1` / `SPACE_2`.

**R17. `DataTable`'s header rule is `HAIRLINE`.** Follows R5.

**R18. A row-shaped component states whether it calls `end_row`.** Three
existing components (`ValueRow`, `PrecedenceField`, `CompareRow`) call
`ui.end_row()` themselves and therefore only work inside a `Grid`.
`KeyValueRow` does NOT call it, so it works in a plain vertical layout, and
`DataTable` owns row termination for its own rows. The rule: a component
named `*Row` that is a grid cell calls `end_row`; one that is a standalone
line does not, and its doc comment says which it is.

**R19. `StatusChip` adds the glyph channel.** `status_chip`'s seven states
collapsed onto three colours under UP1's palette — `GEN`, `STALE` and `WAIT`
are all `CAUTION`, and `PEND` and `OFF` are both `INK_50` — so colour alone
no longer separates five of the seven. §2.6 rule 3 already requires a glyph
beside every verdict, and the chip is where it lands. `PEND` additionally
moves to `UNKNOWN`, which the plan already called for. The WORDS do not
change, and neither does the pure function that produces them.

## 5. Motion

`ctx.animate_bool_with_time_and_easing` and `ctx.animate_value_with_time`
carry all of it. The crate uses neither today.

**Use the `_and_easing` variant.** Plain `animate_bool_with_time` hardcodes
`emath::easing::linear`, so every "ease-out" below is
`emath::easing::cubic_out`, passed explicitly. `emath::easing` ships 22
functions in 0.34 (§10.4).

| Event | Duration | Curve |
|---|---|---|
| Hover fill | 120 ms | ease-out |
| Focus ring | 90 ms | linear |
| Selection move | 140 ms | ease-out |
| Disclosure open and close | 160 ms | ease-out |
| Tab switch indicator | 160 ms | ease-out |
| Toast in | 140 ms | ease-out |
| Toast out | 200 ms | ease-in |
| Window scrim | 120 ms | linear |
| Progress spinner | continuous | linear |

Three rules.

1. **Nothing moves position under the pointer.** A layout that changes size
   when a state changes reserves the space instead. That closes
   `AUDIT.md` D-17: the manual-generation line keeps its height and swaps
   its text.
2. **No animation exceeds 200 ms.** This is an instrument.
3. **Playback and the 3D scene are never animated by the UI layer.** They
   have their own clock.

---

## 6. Hierarchy per screen

Each screen names one primary action and one primary reading. Everything
else is `Default` or `Quiet`.

### Setup

- Primary reading: the setup card list, left.
- Primary action: `+ Add Setup`.
- The right panel opens with a `Heading` naming the selected object, as the
  Machine panel already does (`AUDIT.md` D-12). Stock, Machine, Model and
  Setup panels all adopt it.
- Empty right panel becomes an `EmptyState`: "No selection", "Choose a
  setup, stock, machine or model on the left."

### Toolpaths

- Primary reading: the operation list, left.
- Primary action: `Generate` in the inspector header. `Generate All` on the
  viewport strip is `Default`.
- The inspector header is a fixed block that never changes height: name,
  `StatusChip`, `Generate`, then the tool-load `Banner` when one exists.
- Inside a tab, the first `SectionHeader` group holds the rows that change
  the cut. Remaining rows keep their order under a second `SectionHeader`.
  **No row moves between tabs and no row is hidden.** Which rows are primary
  is an IA question (D3) and stays open; this specification only says that
  the group boundary is drawn.
- The six row-action squares gain `Micro` labels and a `Quiet` treatment,
  and group into two pairs with `SPACE_3` between (`AUDIT.md` D-19).

### Simulation

- Primary reading: the verdict `Banner` at the top of the right Inspector.
- Primary action: `Re-run`.
- The badge set is drawn **once**, in the Inspector, as `CountPill`s. The
  transport bar keeps the transport only (`AUDIT.md` D-28).
- The operation card's three facts split onto three `KeyValueRow`s, safety
  first at `Body`, the rest at `Caption` (`AUDIT.md` D-29).
- The `View` help paragraph moves into the hover of an `Overlays` button
  (`AUDIT.md` D-27).
- The Inspector's pre-simulation state (`ui/sim_diagnostics.rs:41-48`, one
  9-point italic line for a full-height panel) becomes an `EmptyState`.
- **Open question, operator's to answer:** this is the one workspace with no
  status bar (`AUDIT.md` D-42). The bar carries an actionable collisions
  chip, so adding it is a behaviour decision. This specification does not
  make it.

### Readiness

- Primary reading: the verdict `Banner`.
- Primary action: **`Export G-code…` as `Primary`** when every safety row is
  `OK`, and `Run simulation` as `Primary` otherwise. Exactly one is
  `Primary` at any moment (`AUDIT.md` D-33).
- The content column widens from 560 to **880 points** and the rows become a
  `DataTable` with a fixed action column, so the buttons align
  (`AUDIT.md` D-30, D-32).
- Rows split into two `SectionHeader` groups, **Safety** and **Program**.
  Safety holds rapid collisions and holder clearance. Program holds
  operations, simulation, tool load and cycle time.
- A row that has not been run is `UNKNOWN`, not `CAUTION` (`AUDIT.md` D-31).

### Windows

- Every window gets `SURFACE_OVERLAY`, `SHADOW_OVERLAY` and `RADIUS_MD`.
- **The scrim should come from `egui::Modal`, which exists in 0.34 and the
  crate has never used** (§10.1). It paints its own backdrop and blocks
  input to what is behind it. Blocking input is a *behaviour* change, so
  adopting it needs the operator's word, and it must not be applied to all
  twelve. Task windows take it: Export Wizard, Export Readiness, Feeds,
  Optimize, Tool Library, Machine Library, the planner, Unsaved Changes.
  **Overlays, Keyboard Shortcuts and Project Load Warnings stay plain
  windows**, because an operator is meant to keep working with them open.
  Where `Modal` is refused, a hand-painted scrim gives the look without the
  input change, as `ui/optimize_project.rs:566-580` already does once.
- A window title is `Heading`, left-aligned, with the close control right.
- A window's footer is one row: `Quiet` cancel on the left, `Default` and
  then `Primary` on the right, in that order.
- The Export Wizard's breadcrumb becomes a step rail: a completed step is a
  filled `OK` dot with its number, the current step is an `ACCENT` dot with
  its name at `BodyStrong`, a future step is an `INK_35` dot with its name
  at `Caption` (`AUDIT.md` D-36).
- The Project Load Warnings window anchors to the **viewport centre**, not
  the top-left corner, so it cannot cover the workspace switcher
  (`AUDIT.md` D-35).

---

## 7. The 3D layer

`crates/rs_cam_viz/src/render/colors.rs` is already the token module for
this layer. Its structure stays. Its values are retuned.

### 7.1 Move palette

The eight-hue category wheel is replaced by a **six-step sequential ramp in
one hue family**, cool blue to cool cyan, with lightness carrying the
toolpath index. Two consequences:

- adjacent toolpaths are told apart by lightness, which survives a
  colour-vision difference, where hue does not;
- red, amber and green leave the move palette entirely, so a red line in the
  viewport can only ever mean a collision.

Cutting moves render at 85 % opacity, rapids at 35 % as a dashed line, and
the unselected toolpaths — when the operator asks for all of them — at 20 %.
The selected toolpath always draws at full strength. WP27 already made
"selected only" the default; this makes the selected one *look* selected.

### 7.2 Height planes

The five height planes stop using the verdict hues (`AUDIT.md` D-20). They
become one cool ramp — clearance lightest, bottom darkest — and the diagram
in the Heights tab uses the same five values, so the panel and the viewport
agree. The two-letter labels `CZ`, `RZ`, `FZ`, `TZ`, `BZ` expand to their
words at `Caption`.

### 7.3 Stock

`STOCK_SOLID_FACE` desaturates by about 30 % and darkens slightly, to a
muted oak. It stays warm, because warm against the cool shell is the
product's identity, but it stops being the brightest thing on screen
(`AUDIT.md` D-23).

Deviation and by-height colouring keep their ramps and gain a legend built
from the same colour function, which the Overlays panel already requires.

### 7.4 Viewport ground

`SURFACE_SUNKEN` (`#15171A`) replaces both `rgb(26, 26, 38)` and the panel
fill's near-twin, from a single token, in all four layouts
(`AUDIT.md` D-07).

---

## 8. State rules

### Loading

A surface that is computing shows, in place and at the same size as its
result: a `Micro` label naming the stage, an elapsed-seconds counter in
`Numeric`, a 14-point spinner, and a `Quiet` cancel where one exists. It
reserves the result's height so nothing below it moves
(`AUDIT.md` D-18, §5 rule 1).

A surface never shows a skeleton placeholder. A machinist's instrument does
not pretend to have an answer.

### Empty

`EmptyState`, per §4.8. Every empty surface in the product gets one:
the properties panel with no selection, the toolpath list, the tool list,
the catalog detail pane, the simulation inspector before a run.

### Error and refusal

A refusal is a `Banner` at `DANGER` with the exact refusal sentence the core
produced. The product must not paraphrase a refusal, and it must not shorten
one. Where the refusal names a fix, the banner carries a `Default` button
that performs it — the Overlays rule again.

### Many

`NoticeStack`, per §4.11. The question "what happens at N = 300?" has an
answer on every surface that can reach it, and the answer is never "render
them all" and never "silently show the first few".

### Not measured

`NotMeasured`, per §4.12. A gate that abstained, a metric below its
measurability floor, a figure not yet computed. Never a zero, never a blank,
never a green tick.

### Stale

A stale surface keeps its numbers and dims them one ramp step, and its
`StatusChip` reads `STALE` at `CAUTION`. The rule already in
`toolpath_panel.rs` — prefix the figures and fade them — is correct and
generalises.

---

## 9. Accessibility floor

- Body text meets 4.5:1 against its own surface. Caption and chip text meet
  4.5:1. `TEXT_FAINT` meets 3:1 on every surface and is restricted to §2.4's
  rule. Every ratio in §2.4 and §2.6 is computed, and the worst value
  anywhere in the semantic set is 4.80.
- Colour is never the only channel. Every verdict carries its glyph (§2.6
  rule 3).
- The focus ring is 2 points, `ACCENT`, drawn outside the shape, on every
  interactive control, always. It is a component-layer guarantee, not a
  theme setting (§10.5).
- The hit target minimum is 26 x 26 points. Today's row-action squares are
  18 (`AUDIT.md` D-19).
- The layout holds at 1280 x 800 with no clipped text, which is one step
  below the 1400 x 900 bar the earlier review used.

---

## 10. Toolkit constraints, verified against egui 0.34.3

Every claim here was read in
`~/.cargo/registry/.../egui-0.34.3` and `epaint-0.34.3`. The first draft of
this specification assumed four things that are wrong, and they are marked.

**10.1 `egui::Modal` exists** — `egui-0.34.3/src/containers/modal.rs:16`,
exported from `containers/mod.rs`. It paints its own backdrop, default
`Color32::from_black_alpha(100)`, settable through `backdrop_color`, and it
blocks input behind it. The crate uses it zero times.

**10.2 Line height is per call site, in points** —
`TextFormat::line_height: Option<f32>`
(`epaint-0.34.3/src/text/text_layout_types.rs:381`) and
`RichText::line_height`. **There is no line-height field on `Style`.** A
global version, `extra_text_line_spacing`, arrived in egui 0.36 and is not
available here. *First draft said a global ratio; wrong.*

**10.3 A custom `TextStyle::Name` is never picked up by a plain widget** —
`Label` passes `FontSelection::Default` (`widgets/label.rs:185`), which
resolves to `TextStyle::Body` unless `Style::override_font_id` or
`override_text_style` is set (`style.rs:158-167`). `Button` falls back to
`TextStyle::Button` (`widgets/button.rs:48`). Only the five built-in slots
apply automatically. *First draft said three rungs could be `TextStyle::Name`
entries; wrong.* **Weight travels on the `FontFamily`, not the text style**:
register one named family per weight, each with the symbol fallbacks
appended, then point the built-in slots at those families.

Use `ctx.all_styles_mut` (`context.rs:2167`), not `set_global_style`
(`:2142`), which writes one theme and leaves the other on egui's defaults.
The crate's own `configure_theme` uses the single-theme calls today, so the
light theme is entirely unstyled. `ctx.style_mut` is deprecated in 0.34.

**10.4 Easing is available, but not by default** — `emath::easing` ships 22
functions. `Context::animate_bool_with_time` hardcodes
`emath::easing::linear` (`context.rs:3220-3224`). Use
`animate_bool_with_time_and_easing` (`:3236`) or `animate_bool_with_easing`
(`:3212`). `animate_bool_responsive` (`:3206`) already applies `cubic_out`.
The easing flips when the target goes false (`:3258-3260`), so a curve is
symmetric in both directions and no separate ease-in is needed.

**10.5 There is no default focus indicator, but a global route exists** —
`Visuals::show_focused_widget` defaults to `false` (`style.rs:1390`) and is a
debugging aid. A focused widget otherwise renders in its `active` visuals
(`style.rs:1255`), so it looks pressed, and `Visuals` carries no focus field.
*First draft implied a theme setting; wrong.* **But it need not be per call
site either**: a `Plugin` (`egui-0.34.3/src/plugin.rs:13`) registered with
`Context::add_plugin` (`context.rs:1968`) can paint the ring in `on_end_pass`
from `Memory::focused`, using `StrokeKind::Outside`
(`epaint-0.34.3/src/stroke.rs:109`). One registration covers every widget.
**Caveat, and UP2 must measure it:** the ring lands outside the widget rect,
so a parent `Ui` with no margin clips it. Whether this GUI's layouts leave
that margin everywhere is NOT MEASURED. UP2 tries the plugin first and falls
back to per-component rings where it clips.

**10.6 Minimum control height is global** — `Style::spacing::interact_size`
is a `Vec2` and its `y` is documented as the default height of a button,
slider and similar (`style.rs:417-420`). `Button` floors its height there
(`widgets/button.rs:299`), so one assignment covers every button. *First
draft called for `.min_size()` on every button; unnecessary.* Two exceptions:
`Button::small()` skips the floor (`button.rs:298`), which is correct for
the row-action squares only if they are re-sized deliberately; and the same
field raises sliders, drag values and colour pickers, which its own doc calls
intended.

**10.7 A global default wrap mode exists, and it names the clipping cause** —
`Style::wrap_mode: Option<TextWrapMode>` (`style.rs:317`), read by
`Ui::wrap_mode()` (`ui.rs:696`); the older `Style::wrap` (`style.rs:309`) is
deprecated. `Label` also takes `wrap_mode`, `wrap`, `truncate` and `extend`
(`widgets/label.rs:56-79`). **`Extend` sets an infinite max width and grows
the `Ui`** (`label.rs:232-247`), which is the likely mechanism behind the
trailing text clipped at the panel edge in `AUDIT.md` D-16: the label does
not wrap, the `Ui` grows past the panel, and the panel clips it. Setting the
global default reaches the whole class at once.

**10.8 Letter spacing is per call site, in points** —
`TextFormat::extra_letter_spacing`
(`epaint-0.34.3/src/text/text_layout_types.rs:372`) and
`RichText::extra_letter_spacing`. Not on `Style`, `FontId` or `FontTweak`.
The unit is points, so `Micro`'s tracking is **0.8 pt at 10 pt**, not an em
value, and only a chip helper can keep it consistent.

**10.9 `Shadow` is integer-valued** — `offset: [i8; 2]`, `blur: u8`,
`spread: u8`, `color: Color32` (`epaint-0.34.3/src/shadow.rs:10-27`). No
fractional blur. `SHADOW_OVERLAY` is therefore
`Shadow { offset: [0, 8], blur: 24, spread: 0, color: Color32::from_black_alpha(110) }`.
Globals are `Visuals::window_shadow` and `Visuals::popup_shadow`.

**10.10 `CornerRadius` is per corner and per widget state** — fields
`nw`, `ne`, `sw`, `se`, each `u8`. `Visuals::window_corner_radius` and
`menu_corner_radius` are single fields, but the widget radius lives on each
`WidgetVisuals`, so §2.2's value of 4 must be written to all of
`noninteractive`, `inactive`, `hovered`, `active` and `open`.

**10.11 There is no OpenType feature switch** — nothing in `epaint`'s text
module mentions font features or tabular figures. §3.4's rule stands:
numeric alignment comes from the monospace family or from nowhere.

### What an upgrade would buy, for the operator to weigh

egui is at **0.36.2**; this crate pins **0.34**. Two additions bear on this
work. 0.35 added `Classes` on `UiBuilder` and widgets, a styling mechanism
close to CSS classes, and `AtomLayout`, which handles layout inside a widget
and is what chips and buttons need. 0.36 added the global
`extra_text_line_spacing` that §10.2 says is missing. Building the component
set on 0.34 and upgrading later would mean rebuilding part of it. Changing
the pin touches `Cargo.toml`, which this specification does not do.

---

## 11. Out of scope

Named so no package drifts into them.

- A light theme. The product is dark by default and this specification does
  not define a light ramp.
- A density switch. `Comfortable` and `Compact` would be a new control and a
  new registry row. Recorded as a follow-on, not adopted.
- Any change to what a control does, where a control lives, what a number
  means, or which rows appear on which tab. Those are IA items D1-D6 and
  they stay with `planning/ui_fix_2026-09-09/`.
- Icons. The product uses Unicode glyphs through two Noto fallbacks. An icon
  set is a separate decision with its own licence question.
