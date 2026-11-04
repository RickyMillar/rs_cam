# P5d: the envelope plot and the nomogram, second operator review

Status: NOT STARTED (card by the lead, 2026-11-04, from the operator's live review of P5c
round 2, commit 13c35395e, rivmap100 3D Rough) | Contracts: K2b (view) | Numbers move: no

## Goal

The two charts on the Passes tab read as one picture. Every limit is on the plot and named.
The recommendation is quiet. One mark means one thing on both charts.

## Operator rulings (2026-11-04, verbatim points)

1. "The limits should be there and shown. That could just be the size of the graph." Every
   limit that bounds the cut is drawn and labelled, not only the limits near the point. The
   plot range is large enough to hold them (the slot step, the vendor depth, the rigidity cap;
   the cutting length is the frame top when it fits; the width frame is D). P5c drew a limit
   only when it was near a mark: the operator saw "Slot max DOC" only during a drag. Reverse
   that rule. Keep the clutter low with short names and a muted colour, not by hiding limits.
2. "Recommendations should be subtle." The proposal diamond on the envelope plot and the
   Recommended ring and line on the nomogram take a quiet style (thin, muted ACCENT). The
   operator's own point is the strong mark.
3. "The graphs should match (the white point means different on both)." One rule on both
   charts:
   - white dot = the setpoint that ⚡ writes (the draft while a candidate exists);
   - hollow grey ghost = the stored value, only while a draft exists;
   - quiet ACCENT = the recommendation.
   On the nomogram, during an envelope drag the white dot moves to the speeds the pick writes
   (`candidate_speeds`; P5 Apply writes the speeds of a pick), and a ghost stays at the stored
   rpm/feed. Today the nomogram's white "Now" is the stored speeds and the ACCENT ring follows
   the candidate, which is the opposite of the envelope plot.
4. Names (operator asked what the lines mean):
   - "Vendor max DOC" -> name that says it is the vendor's printed depth per pass and its
     base, e.g. "Vendor DOC 1×D" (it is a recommendation that warns, not the tool length).
   - "Slot max DOC" -> "Slot DOC" (the unsourced rule ap <= 0.25 D where ae > 0.85 D).
   - The cutting length is the frame top; label it "Cut length" where it shows.

## Follow-ups to fold in

- The nomogram "Recommended" label never paints (no place near the ring clears the walls and
  the suggested line). After item 2 it may need a leader, or the hover names it.
- At the 240 pt rail the nomogram RPM axis names only "0", and "Now" / "Max RPM" touch.
- The "6" tick at the width frame edge D does not paint (egui_plot).
- At 240 pt "Slot max DOC" found no room (P5c round 2).

## Tests

- Every limit in the plot range draws a line and a label (the rivmap 3D Rough fixture:
  Vendor DOC and Slot DOC at rest).
- The white dot on both charts is the setpoint; a drag moves both white dots; the ghosts
  exist only during a draft; Esc returns both.
- The recommendation marks use the quiet style.
- Line and label counts at 340 and 240 pt, before and after.
- Screenshots from the live GUI through MCP before the commit (operator review).

## Not in this card

The force model, chip-first and feed-driven limits: `P11.md`.
