# P5c: the envelope plot and the nomogram, operator review fixes

Status: NOT STARTED (card by the lead, 2026-11-04, from the operator's live review of P5b on
rivmap100 3D Rough) | Contracts: K2b (view) | Numbers move: no (view only, unless item 4
changes the plot range that the envelope reports)

## Goal

The operator can drag a candidate on the envelope plot and read the plot without a legend.
The plot shows only what limits the cut. Read the memory rule "no text clutter in the UI":
design first with a line count and a mock; prefer chips, colour and disabled controls.

## Operator feedback (2026-11-04, verbatim points)

1. "dragging the dot drags the recommendation". The proposal must stay fixed. Only the
   candidate ring moves.
2. "the graph scrolls when you drag off the screen too far ... it just fires into the void".
   Scroll is acceptable only when it is controlled and stays inside the plot bounds. A drag
   past the plot edge clamps the candidate to the edge; the panel does not scroll.
3. "the 'vendor' line is not intuitive? is that a recommendation?" It is the vendor axial
   bound (`LimitKind::VendorAxial`), a printed ceiling. The name must say it is a limit.
4. "we cannot actually cut more than a slot so the graph should hard stop there. same with
   the depth and the cut length of the bit." The width axis ends at D
   (`LimitKind::Diameter`) and the depth axis ends at the cutting length
   (`LimitKind::CuttingLength` / `FluteGuard`). These are the plot frame, not lines in it.
5. "diameter and moves labels ... do they add more than the axis does?" Diameter goes with
   item 4. Moves (`LimitKind::Runtime`, the move budget) stays only if it binds in view; it
   needs a clearer name or a hover note, not more text.
6. "there are maybe too many lines on the chart, cluttering it". Draw the binding limit and
   the limits near the view; drop or grey the rest. Count the lines before and after.
7. "the colour changing of the lines is hard to tell what it means". Define one colour rule
   (example: binding = one colour, crossed by the candidate = amber, others = grey). The
   hover names the limit. Write the rule in the card before code.
8. "feed chart has no y axis numbers". The nomogram feed axis needs tick numbers.

## Tests

- GUI state test: a drag moves the candidate and leaves the proposal field unchanged (item 1).
- A drag that ends outside the plot clamps the candidate to the frame; the scroll offset does
  not change (item 2).
- The plot range: the width max equals D; the depth max equals the cutting length (item 4).
- The line count on the rivmap100 3D Rough fixture before and after (item 6).
- The feed axis paints tick labels (item 8).
- Screenshots before the commit (operator review), from the live GUI through MCP.

## Not in this card

- Stepover does not move the feeds and speeds (radial chip thinning is observed, not applied,
  G-CHIPTHIN-HALFFIX), and the depth/stepover trade exists only as the Power and Deflection
  edges. The operator asked whether a higher DOC and a lower stepover "cancel". That is a
  research question (`STATUS.md`, "Not in a phase"), not a view fix.
