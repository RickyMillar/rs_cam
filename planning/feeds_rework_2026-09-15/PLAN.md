# Feeds and Speeds rework — plan

Evidence: `FINDINGS.md` in this directory. Read it before changing anything
here; every work item below names the finding it answers.

Owner decisions taken 2026-09-15:

1. **Charts A and B are deleted.** The Explore window becomes the nomogram
   alone.
2. **Direct feed / plunge / RPM control returns to the inspector's Feeds &
   Speeds tab**, above the recommendation card. The modal stays a read-and-
   explore surface.

---

## The two rules this rework is judged against

**R1. A container holds one question.**

The Explore window's question is *"if I move feed and RPM, where do I land
relative to the vendor band and my machine's limits?"* The nomogram answers
it. Charts A and B answer a different question — *"how does the vendor table
vary across tools and materials I am not using?"* — which is reference data,
and which the inspector's `Vendor Cutting Data` table already answers in the
right form. (F-6)

**R2. An explanation belongs to the thing it explains.**

The operator reads the recommendation one row at a time, and the question
that arises is always about one row: *why is my DOC being tripled?* A
disclosure that explains the recommendation as a whole cannot answer that,
however short it is. So "why" moves onto the row whose number changed. (F-7)

---

## Work items

### W1 — restore direct feed / plunge / RPM control (F-1)

The regression from `d323cabb`. Highest priority: the product currently
cannot set a feed rate.

- A `SPEED` block at the top of the Feeds & Speeds tab, above the
  recommendation: Feed, Plunge, and a spindle RPM override.
- RPM uses `components::PrecedenceField`, which exists, is documented, and
  has had no call site since UR4. The project default must stay visible and
  reachable — an override that hides what it overrides is how the old
  hardcoded-18 000 row misled people (W3.1).
- Writes go through the same command path as every other inspector edit.
  This block sets values; it does not consult the calculator. `⚡ Apply all`
  remains the only route from a recommendation into the operation.
- Drill operations are Z-only: `plunge_rate` IS `feed_rate`, so the Plunge
  field is read-only there rather than a duplicate no-op (the W3.2 rule the
  deleted section already carried).

### W2 — the recommendation explains itself row by row (F-7)

- Delete the `Why is the recommendation here?` `CollapsingHeader` and the
  five `why::draw_*` calls under it.
- Every comparison row carries `tokens::GLYPH_DETAIL` and a hover that
  explains **that row's** number: where it came from, and what moved it.
- A row whose value did not change still explains itself; the operator asked
  for the hover on change, and an unchanged row's hover costs one glyph and
  answers "why is it NOT moving", which is the same question inverted.
- Warnings stay on the page as lines. They are warnings; a warning behind a
  hover is a warning that was deleted.
- The vendor row's identity stays on the context chip's `Source` badge,
  which already hovers.

`why.rs` keeps the per-row explanation builders and loses the section
renderers. It stops being a panel and becomes a library of sentences.

### W3 — the power gauge becomes a power statement (F-2)

- Delete the `ProgressBar`. Measured peak utilisation across the entire
  shipped matrix is 23.6 %; typical is 1 %. A bar that never leaves its left
  quarter reports nothing.
- Keep the number, as one line, with the spindle it is quoted against named
  on its hover — a headroom figure quoted against a guessed 0.8 kW default
  is worse than no figure.
- The line takes the caution colour only when utilisation is high enough to
  act on.

### W4 — the Explore window is the nomogram (F-6, R1)

- Delete `draw_chart_a`, `draw_chart_b`, their legends and their helpers.
- The body becomes: spindle policy row, the nomogram, its legend, and the
  band / current / recommended readout.
- Re-derive the window's default size for the smaller body. The screen cap
  and `vscroll` from `g_feedsfit` stay; they are correct and independently
  pinned.

### W5 — the legends stop wrapping one letter per line (F-3)

- Rebuild the legend as a vertical list of `horizontal_wrapped` rows, not an
  `egui::Grid`.
- The mechanism, recorded so it is not reintroduced: in a `Grid` cell the
  default `TextWrapMode::Extend` asks for infinite width (the UR1 defect),
  and `.wrap()` — the obvious correction — collapses to the narrowest
  possible break because the cell has no width to wrap against. **Both Grid
  cell wrap modes are wrong for a label whose width matters.** Do not reach
  for a third; do not use a Grid.

### W6 — the nomogram's annotations stop colliding (F-5 class)

Chart C draws its callouts at fixed pixel offsets, so they overlap each
other and the axis labels at some data positions. Place them so they cannot
collide with the plot edges, and move the multi-clause status sentence
("commanded advance/tooth … · BURN risk … · past spindle cap") out of the
plot and into the readout below it, where it has a line to itself.

---

## Sentries

| Sentry | Pins |
|---|---|
| `the_gui_can_set_a_feed_g_fscontrol` (new) | W1. The Feeds tab holds a writable feed, plunge and RPM control. **Negative-plus-positive**: the editors exist AND they write through the command path. UR4 deleted these silently because nothing asserted they existed. |
| `the_feeds_why_is_a_summary_g_whylines` (rewrite) | W2. The disclosure is gone; every changed row carries a hover that names what moved it. Keeps its arm 2 — every explanation that left the page is still reachable. |
| `the_feeds_modal_holds_one_scope_dc5a` (extend) | W4. The window draws the nomogram and nothing else. |
| `the_legend_reads_as_words_g_legendwrap` (new) | W5. No legend line breaks inside a word. Rendered, not source-scanned: the defect is a layout outcome. |
| `the_feeds_window_fits_the_screen_g_feedsfit` | unchanged; must stay green through the resize. |

## Out of scope

- The 0.8 kW default machine profile. W3 makes its provenance visible; it
  does not change the default, which is a machine-library question.
- The vendor LUT table's own formatting. It is collapsed by default and was
  not reported.
- The four tests failing at `e2ecf697` (simulation staleness, freshness,
  and the `RunSimulation` producer ruling). Unrelated, pre-existing, and
  they need an owner decision rather than a patch.
