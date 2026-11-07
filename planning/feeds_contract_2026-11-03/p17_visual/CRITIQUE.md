# P17 visual critique

One entry per image. Each entry: what looks good, what looks wrong, what changed after it.
The style bar is the histogram card in the Simulation inspector (`ui/components/histogram.rs`).
All captures are dark: the app has one INK palette (`ui/tokens.rs:449`, lead ruling 2026-11-06).

## Before (binary ec5cbe4a9, 2026-11-06)

### before_envelope_rough_dark.png (3D Rough, Passes tab, SAFE CUT)

Good: every limit is named on its line; the dot and the frame edge D read at a glance.
Wrong: the `BORDER` grid lattice is as strong as the muted limit lines; the tick labels are
the egui_plot default (a larger grey body text than the histogram's caption labels); the
rotated "Depth (mm)" title is heavy body text; the label chips sit at odd heights with no
relation to each other; no baseline, so the plot floats; the look has nothing in common with
the histogram cards two tabs away.

### before_envelope_face_dark.png (Face 5)

Good: the width-assumption caution on the header row reads well.
Wrong: the same grid and font problems; the "Slot DOC" and "Corner width" labels crowd the
top-right corner; the 2.5 mm x tick sits between 0 and 6 with no relation to the stepover
the operator types.

### before_envelope_scallop_dark.png (Scallop Finish, SAFE WIDTH bar)

Good: the bar is quiet and the "Tool diameter" end is named.
Wrong: the bar's end tick and the dot are the only marks; the bar font differs from the
nomogram ticks under it.

### before_sim_cards_rough_dark.png (Simulation Metrics, five cards)

Good (the style bar): caption-size labels in `TEXT_MUTED`, the band washes, the dashed
limit with its cap, the off-scale arrow with the bound and its unit, one hairline baseline,
even spacing between cards.
Wrong: five cards is one screen; the Depth of cut card has a line (9.00 mm) that is the
force budget one step removed; the Engagement card has no line and reads in degrees, which
the operator does not type anywhere.

### before_sim_cards_face_dark.png (Face 5)

The same reading as the 3D Rough cards.

## After

(entries follow each build)

### Agent B, envelope at 340 (`envelope_340.png`, headless egui_kittest, 2 px/pt, build of 2026-11-06)

Good: the plot now reads as the histogram's sibling: caption-size `TEXT_MUTED` ticks on 1-2-5
steps (0, 25, 50, 75 and the edge "100 %"; 0 to 10 mm), `TEXT_FAINT` small-caps titles,
one hairline baseline on each axis with short outward ticks, a faint reference line at each
depth step and no grid, the safe region as one soft `OK` wash under the Vendor DOC line, the
binding line heavier than the muted one, each label a chip on its own line in its line's
colour. The "100 %" edge label ends at D and is never clipped; both "0" labels paint at the
corner.
Wrong, first build: the chart allocated past the panel content (a bare `Panel::right` on a
screen of the panel's width) and the right edge was cut; the "DEPTH (mm)" title printed as
"DEPTH (MM)"; the y "0" label dropped under the x "0". Fixed: the snapshot draws through the
app's own `side_panel`, the kit keeps the case of a unit, the y tick labels take `SPACE_2`
of air and place first.
Left: the label chips are the overlay surface at alpha 235, as the nomogram's; on the wash
they read a shade green. A chip of the plot ground would hide the wash less and the line
more; the operator's call.

### Agent B, envelope at 240 (`envelope_240.png`)

Good: the x axis keeps 0, 50 and "100 %" (the 25 steps drop under the 45 pt spacing rule,
as the P5c rule read), every depth tick stays, the two depth labels fit on their lines, the
dot and the wash read as at 340. Nothing is clipped at the rail.
Wrong: the "Min width (moves)" line (the move budget, at 2 % of D) draws but its label has
no place between the Rigidity and Vendor lines at this width, so it drops: the hover still
names it. The before plot dropped it at 240 too (P5d counts); no regression, no fix here.

### Agent B, envelope at 420 (`envelope_420.png`)

Good: the same layout with more air; every label and tick of the 340 image, the labels
spread along their lines. Nothing crowds.
Wrong: nothing by eye.

### Agent B, envelope at 340, a draft (`envelope_340_draft.png`)

Good: the white dot moves to the candidate (30 % of D, 1 mm), the hollow `TEXT_MUTED` ghost
marks the stored point, the quiet `ACCENT` proposal diamond sits under the ghost at the same
point (the proposal is the stored point on this op), the labels keep clear of the marks.
Wrong: nothing by eye; the diamond under the ghost is faint by design (P5d D3, 1 pt at the
vendor-range alpha).

### Agent B, envelope at 340, a hard top (`envelope_340_hard_top.png`, the fixture Pocket at 1000 mm)

Good: the y arm of the D5 rule renders: the frame top is the flute guard at 20 mm, its
`TEXT_FAINT` edge line carries the "Flute guard" label through the kit and the depth edge
tick "20" paints on the top line with the 15 tick clear of it; the amber dot is held on the
frame top and paints whole in the edge room; the crossed rigidity curve is amber; the
quiet proposal diamond and the "Corner width" line read as at rest.
Wrong: nothing by eye. The "Flute guard" label sits under its line beside the amber dot,
the one open place; a wider plot would move it right.

### Agent B, round 2 (lead review of round 1; every PNG overwritten)

Fix 1, the chips: the envelope labels paint as plain text on the plot ground
(`LabelGround::None` on the kit's `Label`; the nomogram keeps `Overlay` through
`paint_labels`; a label that has to cross a soft line keeps its chip). By eye at 340, 240 and
420: "Rigidity DOC", "Vendor DOC 1×D" and "Min width (moves)" now sit beside their lines as
the histogram's labels sit under its bars; the green tint on the wash is gone; the label
colour alone carries the binding-versus-muted hierarchy, and it still reads.
Fix 2, the height: `PLOT_HEIGHT` 160; the drawn frame is about 120 points tall (160 minus
the 7 point edge room and a 33 point bottom gutter), against about 97 before and about 110
with egui_plot. The depth ticks keep 2.5 mm steps at every width; the labels have more air
between the two depth lines. The snapshot screen grew to 220 points so the x title is in
the image.
Draft and hard top: unchanged in behaviour; the "Flute guard" and "Corner width" labels
read as plain text beside their lines, the amber crossed curve and dot as before.
Left: nothing by eye. The 240 rail still drops the "Min width (moves)" label (no place).

Against the mock (`mock_p17.png`, left panel): the fonts, greys, baseline, ticks and wash
match; the mock shows label chips, the lead ruled plain text (round 2); the frame is about
120 pt against the mock's 130 (`PLOT_HEIGHT` 160 with the gutters inside it).

## After, round 1: the envelope in the running GUI (binary built 23:41 from the A + B tree)

Side-by-side crops at 2x: `side_envelope_rough.png`, `side_envelope_face.png`,
`side_envelope_scallop.png` (before at the left, after at the right).

### after_envelope_rough_dark.png (3D Rough, SAFE CUT)

Good: the plot is now the histogram's sibling at the real inspector width (about 330 pt):
caption-size `TEXT_MUTED` ticks on 0, 25, 50, 75 and the edge "100 %", small-caps
`TEXT_FAINT` titles, one hairline baseline per axis with short ticks, a faint reference line
per depth step and no lattice, the safe region as one soft OK wash under the Vendor DOC line,
plain labels in their line's colour beside their lines, the binding line heavier. Nothing is
clipped; the "100 %" label ends at the frame edge.
Wrong: the "Min width (moves)" line (the move budget at 1.7 % of D) sits about 4 pt from the
y baseline and reads as a doubled axis; its label explains it, and the P5d D1 clearance rule
(4 pt) keeps it; left as it is. The Slot DOC step of the before image is gone: P16 removed
`LimitKind::Slot` from the core envelope (`git diff ec5cbe4a9 bd72997c9 -- feeds/envelope.rs`),
not P17. The nomogram under the plot keeps the egui_plot look (larger ticks, a grid): the two
charts no longer match; the nomogram is out of P17's scope (an open item for the lead).

### after_envelope_face_dark.png (Face 5)

Good: the one "Corner width" line at 88 % of D with its label beside it, the dot at 23 %
and 0.81 mm, the wash up to the frame; the caution on the header row reads as before.
Wrong: nothing by eye. The depth range is 0 to 1 mm now (the Slot DOC flat ceiling that set
the old 1.5 mm range went with P16), so the 0.25 steps are fine-grained but right.

### after_envelope_scallop_dark.png (Scallop Finish, SAFE WIDTH bar)

Good: unchanged in form; the bar font is the kit caption font, so the bar and the plot agree.
Wrong: nothing by eye.

### Agent C, sim cards at 340 (`sim_cards_340.png`, headless egui_kittest, 2 px/pt, a hand-built Pocket set, 2026-11-07)

The set: the fixture Pocket envelope (6 mm end mill, the default machine and material), 500
prototype samples in the four classes of the P13c explainer section 9 (steady 80 %, corners
12 %, slot 5 %, ramp 3 %), each card from core's own law at each sample (the peak chip, the
grid power, the tip deflection, the peak edge force), the chip band from the calculator, the
force budget from the machine's depth-cap rule.
Good: four cards, one style: the caption-size `TEXT_MUTED` labels, the dashed limit with its
cap, the off-scale arrow with the bound and its unit, one hairline between cards. The force
card reads "peak 27 % of budget" with its ✓, the budget 35.3 N as the ceiling marker at the
right, and the two readings under it ("Depth 0.17 to 1.50 mm · peak 1.50 mm", "Width 33 to
100 % of D · peak 100 %") in the caption colour, no chip, no line. The chipload band of the
fixture is a point row (a ceiling only), so the card marks it at the slot chip, "peak 100 % of
limit".
Wrong, first build: the chipload card read "no limit" (the fixture's row prints one chip, not
a band; the set read `chipload_bounds` only); the width readings line was cut by the screen
height; the density cells of the chart were almost unseen. Fixed: the set reads the point
chip as the ceiling, the screen is 500 points, the cells (below).
Left: the three criterion cards have no limit row in this snapshot (the set carries no
verdict), so their status hover reads "No gate judges this metric" and their peak reads from
the histogram ceiling; in the app the row supplies both. The look is the same.

### Agent C, sim cards at 240 (`sim_cards_240.png`)

Good: every card keeps its chart, its two end labels and its bound; the readings fit on
their two lines.
Wrong: the header peak truncates ("peak 100 % of…", "peak 5 …") beside the flip and open
buttons: the card header's own truncation at the Simulation rail width (before P17 too, see
`before_sim_cards_rough_dark.png` at a wider rail). Not changed here; the hover carries it.

### Agent C, sim cards at 420 (`sim_cards_420.png`)

Good: the 340 image with more air; every text whole. Wrong: nothing by eye.

### Agent C, sim chart at 340 (`sim_chart_340.png`)

Good: the plane is the envelope plot's twin: the same ticks, titles, baseline, "100 %" edge,
the OK wash, the "Corner width" line and label. The planned white dot carries the ACCENT
playhead ring; the "steady passes · 81 %" label sits at the densest cell in `TEXT_BODY`; the
corner samples read as a faint grey streak along the planned depth and the slot and ramp
samples as a column at D; the header caption "500 moves · 81 % steady"; the legend row of
four painted swatches reads under the plot.
Wrong, first build: the cells were almost unseen (the square-root alpha alone put a cell of a
few samples at 6 %), and the legend was below the screen. Fixed: the lightest bucket is the
histogram's out-of-band wash (12 %) and the densest five times it (60 %), in eight equal
steps; the screen is 280 points.
Left: the steady cluster of this prototype is one cell under the planned dot (every steady
sample sits at the planned point, ± 2 %), so the densest cell itself is hidden by the dot and
the ring. A real run spreads it. The marks paint over the limit lines (the kit's order); a
dot on the Corner width line hides a pixel of it.

### Agent C, sim chart at 240 (`sim_chart_240.png`)

Good: the plot keeps 0, 50 and "100 %", every depth tick, the dot, the ring, the cluster label
and the Corner width label; nothing is clipped at the rail.
Wrong, first build: the header caption truncated to "81 % …" and the legend wrapped mid-entry
(a swatch at the end of one row, its caption on the next). Fixed: the caption drops the move
count when the two do not fit beside the title ("81 % steady"), and the legend lays its
entries in rows by measured width, an entry never split (two rows at 240).

### Agent C, sim chart at 420 (`sim_chart_420.png`)

Good: the 340 image with more air; the legend on one row. Wrong: nothing by eye.

### Agent C, sim chart, the over-budget dots (`sim_chart_340_over_budget.png`)

The fixture cut sits at 27 % of its budget, so the main images paint no dot. This image is
the same set with the budget at a fifth (a what-if, the cards are not part of it).
Good: the CAUTION dots mark the corner samples along the planned depth and the slot and ramp
samples at D; they read as "these samples, here"; the legend's "over force budget" entry
names them. Wrong: nothing by eye. Not shown: a run with more than 400 over-budget samples
(the cap; the legend entry's hover then counts the rest).

## After, round 1: the cards and the chart in the running GUI (binary built 00:37 from the A + B + C tree)

The window was 1400 x 1380 for these shots (a temporary `~/.config/rs_cam/settings.toml`
with `window_height = 1380.0`, removed after the capture), because the chart sits below
the fold at the default 900 pt height: layout B puts it after the four cards and the
abstention table. Zooms at 2x: `zoom_after_sim_cards_face.png`, `zoom_after_sim_chart_face.png`,
`zoom_after_sim_chart_rough.png`.

### after_sim_cards_rough_dark.png and after_sim_cards_face_dark.png (the four cards, 900 pt window)

Good: four cards in the one histogram style; the Cut force card on Face 5 reads "peak 53 %
of budget" with the 35.3 N marker at the right (the ForceBudget rule); on the 3D Rough
(Factor rule) it reads "no limit", "peak 18.8 N", and the caption "no force limit on this op
family"; the two readings "Depth 0.19 to 2.00 mm · peak 2.00 mm" and "Width 3 to 60 % of D ·
peak 60 %" sit under the force chart in the caption colour, no chip, no line; the depth of
cut row paints once in "Other limits".
Wrong: at the default window height the chart is below the fold (layout B); the operator
scrolls to it. Not a defect of the drawing; recorded for the lead.

### after_sim_chart_face_dark.png (Face 5, the width x depth chart)

Good: the envelope plot's twin (the same ticks, titles, baseline, "100 %" edge, the OK
wash, the "Corner width" line and label); the cut time reads as grey columns at 22 % of D;
the "steady passes · 75 %" label at the densest cell; the white planned dot; the legend row
of painted swatches; the header caption "1256 moves · 75 % steady".
Wrong: (1) the depth range is the envelope view (0 to 1.0 mm) while the samples reach
2.00 mm, so the deeper samples clamp into the top edge cell and paint a bright false cell
at 1.0 mm; (2) the four legend entries wrap at the 300 pt rail and "playhead" sits alone on
a second row. Sent to agent C: the y range covers the sample maximum with the envelope's
headroom (never past the hard top); "over force budget" becomes "over budget".

### after_sim_chart_rough_dark.png (3D Rough)

Good: the density band at 2 mm across 2 to 97 % of D reads as the steady passes; the
label "steady passes · 43 %" names it; the Rigidity DOC and Vendor DOC lines frame it; the
ACCENT playhead ring at 70 % of D near zero depth; a three-entry legend (no line, so no
"over budget" entry) on one row.
Wrong: nothing by eye beyond the shared range point above (the 3D Rough samples fit the
view, so it does not bite here).

### Agent C, round 2 (lead review of the live GUI, 2026-11-07): `sim_chart_240/340/420.png`, `sim_chart_340_over_budget.png`, `sim_cards_*.png`

The lead found on rivmap100 Face 5 that samples deeper than the envelope's own view
(2.00 mm on a 0 to 1.0 mm plot) clamped into the top edge cell and painted a bright false
cell at 1.0 mm, and that at the real rail (about 300 points) "over force budget" pushed
"playhead" onto a second legend row.
Changed: the chart's scene is `EnvelopeScene::with_depth_mark` (the one envelope.rs
helper of this round): the deepest sample joins the marks the view range reads, so the
depth range is `VIEW_HEADROOM` over it and never past the hard top, and the limit lines,
the fill and the frame edges draw on that range. The legend entry reads "over budget".
The hand-built set gains 2 % of samples at one and a half times the planned depth (not
an explainer class) so the images show the range.
Good: the 2.25 mm samples sit as one cell well inside the frame on a 0 to 2.9 mm range
(the ticks move to 0.5 steps); the readings under the force card say "Depth 0.17 to
2.25 mm · peak 2.25 mm" in agreement; the legend is one row at 340 and 420; the cluster
label takes the clear side (below-left at 240 and 340, above-right at 420) with no
collision.
Wrong: at 240 the legend still needs two rows ("playhead" alone on the second), the
rule the lead kept for narrower rails. Nothing else by eye.

## After, round 2: the chart in the running GUI (binary built 03:40 from the final tree)

`after_sim_chart_face_dark.png`, `after_sim_chart_rough_dark.png` (1400 x 1380 window),
zooms `zoom_after_sim_chart_face.png`, `zoom_after_sim_chart_rough.png`,
`zoom_after_sim_cards_face.png`, `zoom_after_sim_cards_rough.png`.

### Face 5

Good: the depth range is now 0 to 2.5 mm, so the samples at 2.00 mm show as their own
column at 20 % of D with the ACCENT playhead ring on them; no false edge cell; the planned
dot at 23 % and 0.81 mm; "steady passes · 70 %" at the densest cell; "Corner width" at
88 %; the legend reads "over budget".
Wrong: the legend still wraps at the real rail (about 290 pt): three entries on the first
row and "playhead" alone on the second. A 2 x 2 grid would read as intended rather than as
an orphan; left for the lead (a nit; the kittest 340 render shows one row).

### 3D Rough

Good: unchanged from round 1 (the samples fit the view): the 2 mm band across 2 to 97 % of
D, the cluster label "steady passes · 43 %", the Rigidity and Vendor DOC lines above it, a
three-entry legend on one row.
Wrong: nothing by eye.

## The independent image-only judge (2026-11-07; saw the images and the goal sentence only)

Verdict: "Not yet" with three blockers and a should-fix list. The lead agent's disposition
of each item:

Blockers:
1. "The Slot DOC edge that was visible before is not drawn after; the Face 5 depth range
   now stops at 1.0 mm." Not a P17 defect: P16 removed `LimitKind::Slot` from the core
   envelope (`git diff ec5cbe4a9 bd72997c9 -- crates/rs_cam_core/src/feeds/envelope.rs`;
   MCP `get_safe_envelope` on the 3D Rough lists no slot limit). The depth range follows
   the flat limits in view, so it is 1.0 mm on Face 5 now. Recorded; no change.
2. "The nomogram under the new chart still uses the old style: two kits on one panel." An
   explicit deferral: the nomogram is out of P17's scope (the card, "Out of scope"); an
   open item for the lead.
3. "The width x depth chart is not readable without explanation: the squashed rough range,
   the playhead ring over the planned dot, the unanchored cluster label, a shade with no
   gradient." Sent to agent C as round 3: the depth range follows the samples with the
   flat limits above it as off-scale markers at the top edge (the histogram's idiom), the
   ring as the quietest mark, the cluster label anchored with a leader, 16 alpha buckets.

Should-fix items, disposition:
- "Min width (moves)" on the same text row as "Vendor DOC 1xD": a label-placement nit of
  the rivmap rough; the hover names each; recorded.
- The fill reads as a background: it is the true safe region (everything under the lowest
  depth limit); its top edge is the limit line. No change.
- Caps axis titles: the kit uses the app's `SectionHeader` small-caps language (the "SAFE
  CUT" header above the chart); the nomogram's sentence case is the egui_plot default. The
  operator's call; recorded.
- The two "steady" numbers (75 % and 70 %): two builds, not one state (the round-2 range
  change moved the grid cells). No defect.
- The readings under the Cut force card with no header: the lead asked for plain readings
  under the force card. No change.
- The three N/A rows (measurability), the truncated peak at 240 pt, the chart below the
  fold at 900 pt: pre-existing or layout B; recorded for the lead.
- Marks at 100 % of D on the frame: the kit has `EDGE_ROOM` for the envelope marks; the
  cells at 100 % sit on the edge by value. Nit; recorded.
Nits recorded: "% of limit" beside "% of budget"; the legend wrap; no value on a limit
label (the hover has it); the section header styles ("SAFE CUT" caps, "Width x depth"
sentence case) differ.

### Agent C, round 3 (the independent image judge, 2026-11-07): `sim_chart_240/340/420.png`, `sim_chart_340_over_budget.png`

The judge found the depth range squashed the samples under far limits (a 0 to 11.7 mm
range for a cut under 2 mm), the playhead ring the loudest mark over the planned dot, and
the cluster label without an anchor.
Changed: the depth range is `VIEW_HEADROOM` over the deeper of the deepest sample and the
planned depth, capped at the scene's own range (never past the hard top); a flat depth
limit above it is an off-scale marker at the top edge in the histogram's idiom (the kit's
new `LabelAt::OffScaleTop`: a small arrow pointing out of the frame, the name and the
value, "Rigidity DOC 9.00 mm", in the line's colour; a second marker takes the next
caption row); a vertical limit stops at the top, a curve keeps its points under it. The
playhead ring is a 1 pt ring at the recommendation colour under the planned dot; the
legend swatch follows. The cluster label is anchored to the densest cell with the cell's
half diagonal as its radius (the kit's leader when it moves). Sixteen alpha buckets.
The chart images now draw the rivmap100 3D Rough envelope (two flat limits above a 2 mm
cut; the Adaptive family has no force line, so no dot and a three-entry legend); the
over-budget image keeps the Pocket set.
Good: the band at 2.0 mm reads with its structure (the corner streak, the slot and ramp
column at D, the deep cell at 3.0 mm) on a 0 to 3.9 mm range; both markers paint at every
width with their arrows at a third and two thirds of the width; "Min width (moves)" and
"steady passes · 79 %" sit clear; the planned dot reads over the quiet ring.
Wrong, first build of this round: the marker label was a mark anchor and took a leader
through its own text ("6.00|mm"); then, as one short line anchor, the second marker
dropped at 240 and 340. Fixed: the anchor is the top edge about the arrow plus one run a
caption row under it, so two markers stack. Nothing else by eye.

## After, round 3: the chart in the running GUI (binary built 05:03 from the final tree)

`after_sim_chart_rough_dark.png`, `after_sim_chart_face_dark.png` (1400 x 1380), zooms
`zoom_after_sim_chart_rough.png`, `zoom_after_sim_chart_face.png`.

### 3D Rough

Good: the depth range is 0 to 2.6 mm and the density now shows its structure: the 2 mm
steady band across the width, the shallower cuts spread under it, the deeper edge cells at
the right; the two flat limits above the range are off-scale arrows at the top edge in the
histogram's idiom; the planned dot over the quiet playhead ring; the "steady passes · 40 %"
label beside its cell.
Wrong: only the first marker has its label ("Vendor DOC 1×D 6.00 mm"); the second arrow
("Rigidity DOC 9.00 mm") lost its label at the real rail. Sent to agent C as round 4 (each
marker's label at its own end of the top edge).

### Agent C, round 4 (the GUI capture at the real rail, 2026-11-07): `sim_chart_240/340/420.png`

The lead found at about 290 points that the second off-scale marker kept its arrow but
lost its label. Changed: the kit places each off-scale label at its own end of the top
edge (the first left-aligned at the left corner, the second right-aligned at the right
corner, on the first row under the edge), the arrow at the label's inner end; a label
that would overlap one on its row takes the next row (three rows at most) and is dropped
only then; `PlaneChart::keep_clear` holds the band (the densest row's cells) clear of the
cluster label. Good: both labels paint at every width (one row at 420, two rows at 340
and 240, the sentry holds 290 too); "steady passes · 79 %" sits beside the band with the
kit's leader to its cell; the planned dot reads over the quiet ring. Wrong (a nit): at
340 the two arrows sit side by side near the middle of the edge, each at its own label's
inner end, and the left label's first letter touches the "Min width (moves)" line that
runs at 2 % of D next to the axis.

## After, round 4: the chart in the running GUI (binary built 05:11 from the final tree)

`after_sim_chart_rough_dark.png`, `after_sim_chart_face_dark.png` (1400 x 1380), zooms
`zoom_after_sim_chart_rough.png`, `zoom_after_sim_chart_face.png`, `zoom_after_sim_cards_rough.png`,
`zoom_after_sim_cards_face.png`.

### 3D Rough

Good: both off-scale markers keep their labels at the real rail ("Vendor DOC 1×D 6.00 mm"
left on the first row, "Rigidity DOC 9.00 mm" right on the second, the arrows at their
inner ends); the density band at 2 mm and the shallower spread read as the cut; the
"steady passes · 40 %" label sits beside the band; the planned dot over the quiet ring.
Wrong: the second marker label's row runs over the top of the band at the right (a nit:
the band is at 2.0 mm, the row at about 2.1); the two arrows sit near the middle of the
edge rather than over anything in the plot (they mark "above", not a width). Recorded.

### Face 5

Unchanged from round 3 (no off-scale limit on Face 5): the columns at 22 % of D, the
planned dot at 0.81 mm, the quiet ring at 2.0 mm, the label beside the column.

## The judge, second look (round 4 chart images only, 2026-11-07)

Verdict "not yet" for the chart alone. What works: the depth axis fits the data, the 2 mm
steady row is the brightest band, the over-budget dots read first, the header matches the
card headers, the legend matches the marks. Defects and the lead agent's disposition:
1. Must fix: the off-scale limit labels sit inside the plot over the 2 mm band, so a cold
   reader takes "Vendor DOC 1×D 6.00 mm" as the name of the band; the grey triangles at a
   free x carry no meaning. Round 5 (agent C): the marks move to a gutter above the frame
   in the histogram's off-scale idiom (arrow beside the text, right-aligned, the limit's
   line colour), nothing inside the plot.
2. Must fix: the "steady passes" label drawn across the density columns (Face 5). Round 5:
   the label keeps clear of every cell at a quarter of the densest or more.
3. Should fix: the cluster label has no visible anchor. Round 5: a 1 pt outline around
   the cluster block and a leader drawn always.
4. Should fix: the playhead ring concentric on the planned dot in the headless renders
   (the playhead at the start of the path). Recorded: the ring is now the quietest mark;
   a ring on the dot says the playhead is at the planned point.
5. Should fix: the density column and dots at 100 % cut by the border. Round 5 nit: the
   frame edge paints under the cells.
6. Should fix: the legend lists "over budget" with no dot in the chart. Round 5: the entry
   only when a dot painted. The two-row wrap at the rail stays (recorded).
7. Nits: "Corner width" is the loudest line (the P5d binding colour; the hover has the
   value); the green tint of the fill (the OK wash, the histogram's own); the caps axis
   titles (the operator's call, recorded above).

### Agent C, round 5 (the image judge's "not yet", 2026-11-07): `sim_chart_240/340/420.png`, `sim_chart_340_over_budget.png`

The judge found the off-scale labels inside the plot read as the band's name and the
free-x triangles carried no meaning; the cluster label had no visible anchor and crossed
the density columns; the legend named "over budget" with no dot.
Changed: the kit's off-scale marker is now `PlaneChart::off_scale_top`: one caption row
per off-scale limit in a gutter ABOVE the frame (allocated only then, three rows at
most), right-aligned to the frame's right edge, a painted up-arrow at the text's left
(`CAP_HALF_WIDTH`, the histogram's `paint_off_scale` idiom), in the limit's own line
colour; `LabelAt::OffScaleTop` is deleted. The cluster is a 1 pt `TEXT_MUTED` outline
about its nine-cell block; the label keeps clear of every cell at a quarter of the
densest or more and points at the block with a leader always drawn (`Label::leader`,
1 pt `TEXT_MUTED`). The legend lists "over budget" only when a dot painted. The frame
edges paint under the cells (`ChartLine::beneath`), so a sample at 100 % of D is not cut
by the border.
Good: "▴ Vendor DOC 1×D 6.00 mm" (the binding line's body colour) and "▴ Rigidity DOC
9.00 mm" (muted) stack above the frame at every width, outside the data, and read as
limits past the top; the outlined block at the planned point with "steady passes · 79 %"
beside it on a short leader; the planned dot over the quiet ring; the three-entry legend
on the rough chart, four on the over-budget image. Wrong: nothing by eye. Nit: the block
is one cell wide in this prototype (every steady sample sits at the planned point), so
the outline hugs the dot; a real run spreads it.

## After, round 5: the chart in the running GUI (binary built 05:26 from the final tree)

`after_sim_chart_rough_dark.png`, `after_sim_chart_face_dark.png` (1400 x 1380), zooms
`zoom_after_sim_chart_rough.png`, `zoom_after_sim_chart_face.png`, `zoom_after_sim_cards_rough.png`,
`zoom_after_sim_cards_face.png`.

### 3D Rough

Good: the two flat limits above the range are gutter rows above the frame, right-aligned,
each with its arrow and value ("▴ Vendor DOC 1×D 6.00 mm", "▴ Rigidity DOC 9.00 mm"):
nothing inside the plot names the band; the cluster is an outlined block at the planned
point with "steady passes · 40 %" on a leader; the density reads as the cut.
Wrong: the label still runs along the 2 mm band to the right of the block (those band
cells are under the quarter-of-densest threshold, so the layout may cross them). A
follow-up nit for the operator's demo ruling.

## The judge, third look (round 5 chart images only, 2026-11-07)

Verdict: "Ship with the must-fix items." What works: the off-scale rows read as "above
this range"; the cluster is an outlined block with its label on a leader; the legend
matches the marks; the Face 5 chart shows the planned-versus-steady gap a reader can act
on; the chart degrades cleanly at 240 pt. The lead capped the iterations here (2026-11-07);
every item below is a follow-up for the operator's demo ruling:
1. Must fix (`sim_chart_340_over_budget.png`): with dots present the cluster label moves
   below the block and shows no leader.
2. Must fix (`sim_chart_340_over_budget.png`): over-budget dots paint over the planned dot
   and break the block outline; the planned mark must stay on top.
3. Should fix: the density shade reads as a presence map (one visible level); the band is
   faint in the headless renders.
4. Should fix: the chart's limits are grey with an up-arrow; the cards' limits are red
   with a right-arrow (the P5d rule colours the envelope limits by binding, not verdict).
5. Should fix: Face 5 shows no Vendor DOC or Rigidity DOC rows (its limits are inside the
   range as lines, and the Slot DOC went with P16); a reader asks why the two ops differ.
6. Should fix: "Min width (moves)" sits mid-plot at 2 % of D and its label reads as a
   region name; "Corner width" sits left of its line over the 60 % column on Face 5.
7. Nits: dots at 100 % clipped by the frame; the empty block glyph looks like a button; no
   legend entry for the limit lines or the rows; the two rows right-aligned with a stagger.

## P17b agent C (items 1, 2, 3, 5; headless egui_kittest, 2 px/pt, 2026-11-07)

Images: `sim_chart_{240,340,420}.png` (the rivmap100 3D Rough envelope, the prototype
classes, no budget), `sim_chart_340_over_budget.png` (the Pocket set at a fifth of its
budget: dots and contours), `sim_chart_340_spread.png` (new, look only: the Pocket set
with the steady passes spread over four heat cells each way on a triangular density, no
dot, so the ramp and the contours read alone).

### sim_chart_340_spread.png (item 1, the heat map)

Good: the band reads "brighter where more cuts" at a glance: a bright core at the planned
point, a grey ring, faint edge cells that fade into the wash (32 steps on the square-root
ramp from the histogram's wash to the full body rung); the white planned dot on its ground
halo reads whole over the bright core; no cluster box, no leader, no "steady" text; the
header is the move count alone; the legend swatch is a four-square ramp, faint to bright.
The "25 %" contour is named on its line, quiet, in the line's colour (the only level
under the axis top: the planned Pocket cut sits near a quarter of its budget).
Wrong: nothing by eye. Nit: "Corner width" floats left of its line (the P17 label kit;
agent B's on-line anchor is the fix, pending).

### sim_chart_340.png, sim_chart_420.png, sim_chart_240.png (item 2, the depth axis)

Good: the axis extends to the Vendor DOC (6 mm), which draws as a line INSIDE the frame
with "Vendor DOC 1×D" named under it in the binding colour; the Rigidity DOC (9 mm) keeps
one quiet `TEXT_MUTED` row above the frame; the heat grid stays over the data range (the
2 mm band and the 3 mm deep cells), the extension is empty space with the limit through
it; the 240 rail keeps both.
Wrong: the band is one cell tall under a 6.9 mm axis: the extension to the lowest limit
squashes it (the operator's own ask for item 2; the judge's round-3 note stands as the
cost). The prototype's steady samples share one cell under the planned dot, so the ramp
does not show here (see the spread image). "Min width (moves)" still floats mid-plot
(agent B's anchor, pending).
Changed from the P17b before images: the up-arrow gutter row for the Vendor DOC is gone;
the cluster box, its leader and the "40 % steady" header text are gone.

### sim_chart_340_over_budget.png (items 3 and 5)

Good: the four iso-force contours from core's law at the planned advance per tooth, the
25/50/75 % lines faint (`TEXT_FAINT` 1 pt) under the fills, the "100 % budget" line in
`CAUTION` 1.5 pt the strongest, the same colour as the dots over it, so "the dots are
over THIS line" reads; the curves are the non-linear force the operator asked to see (a
wide cut is heavy at a shallow depth, a narrow cut at a deep one); the planned dot paints
last on its halo, whole over the orange dots at the same point, with the playhead ring
under it.
Wrong: the "25 %", "50 %", "75 %" labels take background chips where their only free
place crosses a neighbouring soft line (the kit's rule), so three of the four labels read
as chips, not as quiet text on the line; "100 % budget" turns along the steep part of its
curve. Both wait on agent B's on-line label anchor (the brief: keep the current labels
until it lands). Nit: the dots at 100 % D are still cut by the frame (P17 nit, open).

## P17b item 2: the limit names ON their lines (`envelope_240/340/420.png`, `envelope_340_draft.png`, `envelope_340_hard_top.png`; headless, 2 px/pt, 2026-11-07)

The operator: "I cant see what the axis lines are (limits) ... 'corner width' kind of works
but its not obvious its labelling the axis line." The kit gains `LabelAt::LineEnd` on
`LabelGround::Ground`: a label sits ON its line at the line's end, in the line's colour, on a
chip of the chart ground, so the line runs into the text and out the other side; a vertical
line takes the text horizontal beside its top, touching it; a step or a curve at its last
point; when the end place is taken, the other end, then the old beside-the-line places.
Good, 340 and 420: "Rigidity DOC" and "Vendor DOC 1×D" read as the names of their lines:
each sits on its line, right-aligned at the frame, the line visibly entering and leaving the
text; "Min width (moves)" hangs from the top of its vertical line, touching it. The binding
line's heavier weight still reads through its label colour. Nothing is clipped.
Good, 240: the two depth names keep their place at the right ends; the move-budget line
hugs the frame at this width and does not draw (P5d D1, as before), so it has no label.
Draft (`envelope_340_draft.png`): the white dot sits at the candidate, the hollow ghost at
the stored point, and the labels keep clear of both as at rest.
Hard top: "Flute guard" sits at the LEFT end of the top line (the right end is crossed by
the "Corner width" line, so the kit took the other end); "Rigidity DOC" sits along the
start of its curve (its end crosses the Corner width line); "Corner width" hangs beside the
bottom of its line (the top end sits under the crossed curve). Each label still touches the
line it names.
Wrong, by design: the ground chip under a level label punches a notch in the OK wash
where the label straddles the limit line (above the line: the ground; below: the wash); the
chip also masks the faint reference hairline it crosses ("Min width (moves)" over the
10 mm line at 340). Both are what "the line masked under the letters" asks for; recorded
for the lead. The 240 rail still drops the move-budget line (no change).

### P17b agent C, round 2 (agent B's on-line anchor landed; the same five images re-rendered)

Changed: every limit label and every contour label is now ON its line at the line's end
in the line's colour on a chip of the chart ground (`LabelAt::line_end`,
`LabelGround::Ground`); the playhead ring paints between the halo and the planned dot
(the advisor's catch: a halo over the ring hid "the playhead is at the plan"); the chart
snapshot screen is 300 pt so the 240 rail shows its second legend row.
Good (`sim_chart_340_over_budget.png`): "100 % budget" sits on the amber curve at its
upper end, "50 %" on its line at the left, "75 %" and "25 %" on their lines at the right
edge, each in its line's colour with the line running into the chip; no floating chip;
"Corner width" at the top of its line; the blue ring reads around the white dot over
the orange dots. (`sim_chart_340.png`, 420, 240): "Vendor DOC 1×D" on its line at the
right end, inside the frame; "Min width (moves)" at the foot of its line, no longer
mid-plot; the Rigidity DOC row above the frame. (`sim_chart_340_spread.png`): the heat
core, the ring of grey, the faint edge; "25 %" on its line at the right.
Wrong: nothing by eye. Nits: the dots at 100 % D are cut by the frame (P17, open); the
2 mm band under a 6.9 mm axis is thin on the rough (the cost of item 2's extension).

## P17b item 4: the nomogram on the chart kit (`passes_240/340/420.png`; headless, 2 px/pt, the whole Passes tab of the rivmap100 3D Rough, 2026-11-07)

The operator: "the passes tab on the toolpath ... still has the old feeds/speeds chart,
looks out of place now." `explore.rs` `draw_chart_c` draws through `PlaneChart` (no
`egui_plot` left in `explore.rs` or `shared.rs`; the readiness scatter keeps it). The content
is the same: the corridor lines, the one suggested line, the band lines where the row
publishes both, the three walls, the dashed target line and its ring, the quiet ring, the
ghost, the white dot, the hover readout on the frame only, the ranges `0..1.15 x cap` and
`0..1.05 x cap` with the wall label room; only the look moved to the kit.
Good, 340: the two charts now read as one kit: the same caption ticks on 1-2-5 steps
(RPM 0, 5000 .. 25000; feed 0, 2500 .. 10000), the same small-caps titles ("FEED
(mm/min)", "RPM"), one hairline baseline per axis with short outward ticks, a faint
reference line per feed step and no lattice, the origin at the frame corner (the egui_plot
5 % margin is gone). "Max feed" sits ON the feed wall at its right end in the wall's red;
"Rubbing" sits on the rubbing ray at its end, turned with the ray; "Min RPM" hangs from the
top of the amber wall, touching it; "Max RPM" takes the old beside-the-wall place (its top
end is under "Max feed", its bottom end over "Rubbing"); "Now" and "Before derate" are plain
text beside their marks. The quiet ring, the ghost and the dot are the kit's marks.
Good, 240: every line and mark keeps its place; "Max feed", "Max RPM", "Rubbing" and "Now"
paint; "Min RPM" drops (no room; as before W3k at 240). The feed ticks keep 2500 steps,
the RPM axis names 0, 10000, 20000.
Good, 420: the 340 image with more air; "Max RPM" moves beside the wall at mid height.
Wrong, by eye: the room above the feed wall (kept for the one wall label's fallback at the
240 rail, `with_wall_label_room`) leaves a 20-point band above "Max feed" with nothing in
it; a half-chip room would do when the end place is free, but the Pocket fixture at 240
needs the beside place (both ends taken: the ring at the corner, the Min RPM wall at the
axis). Recorded. The Passes tab text rows (ic1) moved with the chart texts, no prose row:
P17 (the envelope plot) 36 -> 39 and 35 -> 38; P17b (the nomogram: a feed tick every 20
points, each limit label on its own row) 39 -> 40, 38 -> 37, 28 -> 29.

## P17b finish (agent D, 2026-11-07): one live look, then headless only

The lead stopped the live GUI work after one look (the operator needed the GUI). The live
images below are round 0: the release binary of the tree that agent D found (agents A-C),
before the fixes of this round. Every later image is headless (`p17_visual_snapshots.rs`,
2 px/pt). The headless images include the uncommitted P19 core edits of a peer session
(`rs_cam_core/src/feeds/envelope.rs`, `predict.rs`): the "Min width (moves)" line is out
of the rivmap rough view in them.

### Live round 0 (`p17b_live_r0_*.png`, 1400 x 1380 and 1920 x 1165, dark)

- `p17b_live_r0_zoom_sim_rough.png`: the heat map reads as one flat grey sheet. Most of
  the cut time is in one cell at 2 mm, and the ramp on the share of the densest cell puts
  every other cell at the faint end. The cells are about 1 pt tall and 5 pt wide (32 rows
  over the data range under a 6.9 mm axis), so they read as streaks, not cells. The
  Vendor DOC name sits on its line inside the frame; the Rigidity DOC (9 mm, more than 3x
  the data range) keeps one quiet off-scale row. No contour: the 3D Rough has the Factor
  rule (no force budget), as specified.
- `p17b_live_r0_zoom_sim_face.png`: wrong. Face 5 has a force budget (35.3 N, the card
  reads "peak 53 % of budget"), but no contour draws. Cause: the force field covered only
  the envelope's own view top (1.06 mm); the samples go to 2 mm, the field peak under
  1.06 mm is 8.6 N, under the 25 % level (8.8 N). The faint cells read as a few grey
  bars.
- `p17b_live_r0_zoom_passes_rough.png`: the nomogram on the kit reads as one kit with the
  SAFE CUT plot. "Max RPM" hangs under "Max feed" at the corner of the two red walls;
  neither name reads as its own line's.

### Fixes of this round

1. The heat ramp is the RANK of the cell's time among the cells with time (histogram
   equalisation, datashader `eq_hist`), not the share of the densest cell: one heavy cell
   no longer fades the rest.
2. The heat cells are square on the screen (`square_rows`: 48 columns, the rows follow
   the data range's height in the frame, at most 32).
3. The force field spans the top of the chart's own scene (`with_depth_mark` at the
   deepest sample), so Face 5 gets its 25 % and 50 % contours.
4. The label kit: a steep line whose two ends are taken tries places touching its side
   before the free places; a label may name its line from either end (`is_own_line` reads
   both orders). The feed wall names itself at its low-RPM end, so "Max feed" and "Max
   RPM" no longer stack at 340 and 420 (at 240 the low end is taken by the Min RPM wall,
   so they still stack there).

### Headless after (`p17b_after_headless_*.png`, `sim_chart_*.png`, `passes_*.png`)

- `sim_chart_340.png`: square cells; the ramp shows (bright and grey cells along the 2 mm
  row). The prototype set holds every steady sample at one depth, so the row still reads
  as a dashed band; live data spreads over the plane.
- `sim_chart_340_over_budget.png`: the four contours, each named on its line; the planned
  dot whole over the orange dots; the cells now show under the dots.
- `sim_chart_340_spread.png`: a soft blob, bright core, faint edge.
- `passes_340.png`, `passes_420.png`: "Max feed" on its line at the left end, "Min RPM"
  and "Max RPM" hang from the tops of their walls, each touching its wall.

### A process defect in looks 1 and 2 (read this first)

Agent D ran the snapshot test with the RELATIVE directory
`planning/feeds_contract_2026-11-03/p17_visual`; cargo runs a test in the crate
directory, so the renders went to `crates/rs_cam_viz/planning/...` (moved here and the
stray directory deleted). The `p17b_after_headless_*` copies, `sim_chart_340_spread.png`
and `passes_240.png` that looks 1 and 2 saw were therefore the renders of agents A-C,
WITHOUT fixes 1, 2 and 4 of this round. Look 3 is the first look at the final images.

### Judge, look 1 (images: sim_chart 340, over budget, spread, passes 340; stale renders)

Verdicts: sim chart mostly; over budget no; spread mostly; passes: SAFE CUT mostly,
feed x RPM no. Must fix: the unlabelled quiet ring and the unlabelled corridor line on
the nomogram; "Max feed"/"Max RPM" stacked at the corner; the 2 mm row reads as a dashed
guide line; the orange dots hide the cells; contour labels at mixed ends; the cells and
dots at 100 % cut by the frame. Acted on: the corner stack (fix 4). Not acted on, by
rule: the ring and the corridor have no painted label since W3k/P5d D3 (the hover names
them; item 4 keeps the content); the dots stay orange (item 3); the labels sit ON their
lines (item 2).

### Judge, look 2 (the same images plus passes 240; stale renders)

Verdicts: sim chart mostly; over budget no; spread mostly; passes 340: SAFE CUT mostly,
feed x RPM no; passes 240 no. Must fix: at 240 "Min RPM" and "Before derate" drop;
the orange dots cover the cells; "Max RPM" still reads as a second label of the feed line
(it hangs left of its wall, under the feed wall); the two-ring "Before derate" glyph; the
100 % data cut by the frame; the thin data on the prototype rough; contour labels at mixed
ends; "Min width (moves)" placement; the playhead ring is dark on the dark ground. Nits:
red walls when nothing is crossed, the stub of line past each label, chip on one label and
not another, the blank band under the legend.
Not acted on (rule-bound or a design choice for the lead): the label drops at 240 (as
before P17b, W3k); the glyphs and colours of the nomogram (item 4: look only, content
kept; the red walls are the machine walls since W3k); the orange dots (item 3); the frame
at D (D5).

### Judge, look 3 (the final images: `p17b_after_headless_sim_chart_340.png`, `..._over_budget.png`, `sim_chart_340_spread.png`, `p17b_after_headless_passes_340.png`, `passes_240.png`)

Verdicts: sim chart mostly ("the data marks look like broken dashes"); over budget no
(too busy: dots and contour labels); spread YES; passes 340 mostly (SAFE CUT fine, the
feed x RPM chart busy); passes 240 no.
Must fix, as the judge ranks them: (1) at 240 "Min RPM" and "Before derate" drop; (2) at
240 "Max feed" and "Max RPM" stack at the corner; (3) the orange dots cover the heat cells
and the planned dot sinks into them; (4) "75 %" and "25 %" sit on the 100 % column; (5)
the 100 % column looks clipped by the frame; (6) the playhead ring is dark blue on the
dark ground; (7) the prototype 2 mm row reads as dashes. Nits: one chip style ("Vendor
DOC" has a chip, "Rigidity DOC" seems not to), contour labels at mixed ends, "25 %" says
not of what, the double line at 100 %, heavy small-caps axis titles.
The three-look cap is reached; agent D stops here. Each item is open for the lead (P17.md
"P17b open items").
