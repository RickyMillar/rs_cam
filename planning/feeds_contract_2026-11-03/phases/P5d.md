# P5d: the envelope plot and the nomogram, second operator review

Status: DONE 2026-11-04 (report `P5d_REPORT.md`; design below by the lead, 2026-11-04; card by the lead, 2026-11-04, from the operator's live review of P5c
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

## Design (lead, 2026-11-04; decided, do not reopen)

Live numbers, rivmap100 3D Rough (MCP `get_safe_envelope 1`, GUI built 13:10 after
13c35395e): D 6; Vendor ap 6.0 (amana-flat-hardwood-pocket-6000-2f-spektra); Rigidity 9.0;
Slot 1.5 where ae > 5.1; flute guard 20; cutting length 25; deflection 42.75 (flat);
Runtime ae floor 0.043; chip band per point. Stated = proposal = (ap 3.1, ae 2.1).

### D1. Which limits draw (reverses the P5c "near a mark" rule)

A non-frame limit draws a line and a label when its edge enters the view. Delete
`NEAR_SHARE` and the `near` test. Exceptions, each for a geometric reason, not for clutter:
- An edge within `MARK_CLEARANCE` px of the frame (example: Runtime at ae 0.043 of 6) draws
  no line; the hover names it. It is the axis, not a line in the plot.
- A per-point limit (chip band) has no edge; as today.
- An edge above the frame top (deflection 42.75 > 20) cannot bind inside the tool's reach;
  no line.

### D2. The plot range ("that could just be the size of the graph")

Width: unchanged (0..D). Depth: `ap_max = min(hard_top, VIEW_HEADROOM x max(every flat ap
ceiling of a non-frame limit that is below hard_top, the marks))`. A curve does not set the
range. rivmap: max(6, 9, 1.5, 3.1) x 1.3 = 11.7, under the guard 20. When `ap_max ==
hard_top`, the frame top carries a label: "Cut length" (CuttingLength) or "Flute guard".
The view still never follows the candidate.

### D3. One mark rule on both charts

| Mark | Envelope plot | Nomogram |
|---|---|---|
| White filled dot (`TEXT_STRONG`; `CAUTION` outside) | setpoint: the candidate, else the stored point | setpoint: `candidate_speeds` while a candidate exists, else the stored speeds; label "Now" |
| Hollow `TEXT_MUTED` ghost | stored point, only while a candidate exists | stored speeds, only while a candidate exists; no label |
| Quiet ACCENT (1 pt outline, no fill, `wash` alpha as for faint lines) | proposal diamond, when not at the setpoint | Recommended ring at `explain.recommended` (never moves on drag); the suggested chip line also stays at the recommendation, 1 pt, quiet |

The nomogram drops the painted "Recommended" label (it never painted); the hover names the
ring. No blue mark moves during a drag on either chart.

### D4. Line colour and names

- Binding limit: `TEXT_BODY` 2 pt. Crossed: `CAUTION` 2 pt. Every other drawn limit:
  `TEXT_MUTED` 1.5 pt (still above the `BORDER` grid). The label takes its line's colour.
- `short_label`: VendorAxial -> "Vendor DOC {k}×D" with k = mm / D (trim to at most 2
  decimals: "1×D", "0.5×D"; "Vendor DOC" when D is unknown); Rigidity -> "Rigidity DOC";
  Slot -> "Slot DOC"; CuttingLength -> "Cut length"; Runtime -> "Min width (moves)". The
  others keep their names. The hover keeps the full law, the mm and the basis.

### D5. Follow-ups

- The "6" tick at D: pad the drawn x bounds a few px past D (the clamp and the frame stay
  at D; draw the D frame edge as a line in `BORDER` strength or stronger) so egui_plot
  paints the edge tick. Same for the depth top when it is the hard top.
- 240 pt: the nomogram RPM axis names at least two ticks (0 and one more), and "Now" /
  "Max RPM" do not touch (the label layout rejects an overlap; a moved label takes a leader).
- 240 pt: every drawn envelope line has its label (a muted label may sit inside the plot
  along its line; it may cross the grid, never a mark).

### Mock (340 pt, rivmap 3D Rough at rest)

```
 SAFE CUT                                   ⚡
 11.7┤
     │
    9┤───────────────────────── Rigidity DOC   (muted)
     │
    6┤───────────────────────── Vendor DOC 1×D (body, binds)
     │
    3┤            ●  (white, stored = setpoint; proposal at Now: no diamond)
 1.5┤                                  ┌─── Slot DOC (muted, ae > 5.1)
    0┼─────────┬─────────┬─────────┬────┴─┤
     0         2         4               6
```
Lines before (P5c, at rest): 1 line, 0 labels. After: 3 lines, 3 labels (Rigidity, Vendor,
Slot). The agent measures the counts at 340 and 240 pt and records them here.

Measured (p5d-impl, 2026-11-04; rivmap100 3D Rough at rest, the inspector on the Passes tab;
before = HEAD 6b7d81e3c, after = the P5d working tree; `every_limit_is_drawn_and_named_p5d`
prints the after row with `--nocapture`). A line is an open painted path inside the chart band.

| Chart | Width | Lines before | Labels before | Lines after | Labels after |
|---|---|---|---|---|---|
| Envelope | 340 | 1 | 1 (Vendor max DOC) | 4 (3 limits + the D edge) | 3 (Vendor DOC 1×D, Rigidity DOC, Slot DOC) |
| Envelope | 240 | 1 | 1 (Vendor max DOC) | 4 (3 limits + the D edge) | 3 (the same) |
| Nomogram | 340 | 5 | 5 (Max feed, Max RPM, Rubbing, Min RPM, Now) | 5 | 5 (the same) |
| Nomogram | 240 | 5 | 3 (Max feed, Max RPM, Now) | 5 | 3 (the same) |

The fixture P5c point drew one label (the card above says 0). RPM ticks at 240: before "0" only,
after "0" and "20000". "Recommended" painted at neither width before or after on this job.

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
