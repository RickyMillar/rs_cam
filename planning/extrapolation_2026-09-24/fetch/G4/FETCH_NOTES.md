# G4 fetch notes: the band (one value printed, no minimum)

Date: 2026-09-24. Agent: G4 research. Status: Phase 1 fetch complete.
No cargo run. No LUT, manifest or Rust file changed.

Files in this folder:

- `sources.json`: 9 entries (8 new stored texts, 1 verification of v24).
- `sources/*.txt`: the stored texts. PDFs are in `pdf/` (not committed).
- `candidate_rows.json`: empty. See §5.
- `lut_discrepancies.md`: D1 (repo-authored Spektra bands), D2 (two
  encodings of one value), D3 (Spektra v24 superseded by v41).
- `band_shape_derived.txt`: the band shape per source and family, DERIVED
  from the LUT by `scripts/g4_band_shape.py`.

## 1. Target 1: does Amana say what its one printed value means?

**No. Amana prints no range, no "start at", no "+/- x %", no "maximum" and
no "minimum" for the Spektra or compression values.**

- Spektra v24 (the LUT's source; stored text in the crate; the PDF
  re-downloaded today has the manifest's sha256). The only notes are the
  depth-of-cut derate and: "Disclaimer: It is important to understand that
  these values are only recommendations."
- Spektra v41 (the highest version the URL pattern serves, 2024-06-15). The
  same notes. Still one value per cell.
- Compression spirals v8. The same notes. One value per cell.
- Amana "Router Bit Technical Information" page (Wayback 2024-05-28; the
  live page returns 403). Hand-router text, qualitative only: "There is no
  set feed speed when using a router ... feed the workpiece too slow ...
  causes the bit to overheat and mar the workpiece ... feeding too fast may
  leave a rough, washboard surface."
- ToolsToday (Amana retailer, grade c): "these feed and speed charts are
  Amana's recommended starting calculations and each CNC machinist may want
  to adjust their inches per minute depending on the size, power and
  stability of their machine." Its worked example runs a 3/8" Spektra
  compression bit at 0.020" per tooth against the chart's 0.0072".

Verdict for the refusal record: **a range around the Amana single value is
not published anywhere I could reach.**

## 2. Target 2: how do vendors say to use a band?

| Source | Grade | What it prints | Verbatim |
|---|---|---|---|
| Freud CNC chart 2017-08-22 | a | Start at the low end of the printed range | "You should start your tests with the lower feed rates yielded by our formulas to reduce the chance of bit breakage. (Freud’s chart contains recommended starting points, and does not warranty against tool breakage)" and, under each table, "Start your tests with the lower feed rates yielded by our formulas." |
| Freud (same) | a | Both edges named, no number | "If your chip is very small, or just sawdust, then it will not carry enough heat away from the edge of the bit. Excessive heat will prematurely dull the edge ... If the chip is too large, it will leave a rough surface or edge on your work piece." |
| Onsrud FeedSpeeds page (Wayback 2018-06-20) | b | Both edges named, no number, no start point | "If the chip is too small, the heat is transferred to the cutting tool causing prematurely dulling. Too high of a chipload will cause an unsatisfactory edge finish, or part movement." |
| Onsrud Cutting Data page (live) | b | Calls the data starting values | "recommended starting speeds and chip-load guidance" |
| Onsrud per-material sheets (LUT sources) | a/b | No band-use note; only "1 x D Use recommended chip load", the 2xD/3xD derate and the formulas | (in the crate's stored texts) |
| ShopBot 2016, restating Onsrud | c | Start at the middle (arithmetic mean) | "Start with the middle of the range of recommended chip load provided on the chart ((.006+.004)/2=.005)." |
| ShopBot 2016, "the strategy that bit manufacturer Onsrud suggests" | c | Climb from the chart until finish or hold-down fails, then back off | "Increase the cutting speed (feed rate) until the quality of the part’s finish starts to decrease or the part is starting to move from hold downs. Then decrease speed by 10%." |

Findings:

1. One vendor (Freud) prints a start point: the **low** end. No vendor
   prints "middle". "Middle" is ShopBot's text, not Onsrud's. I found no
   Onsrud document that prints either ShopBot sentence.
2. Every vendor text calls its numbers **starting points or
   recommendations**. None calls the printed value or the printed upper edge
   a maximum or a breakage limit.
3. No vendor prints a band width, a "+/- %", or a rule to make a band from
   one value.
4. Both edges have a named physical meaning in the vendor text: too small ->
   heat and dulling (burn); too large -> finish and part movement. This
   supports the engine's reading of min as the burn side. It does not give
   the burn side a number.

## 3. Target 3: what the engine does with a row that has no minimum

Code read on 2026-09-24 (commit 4098e937). Two encodings exist for "one
value printed" (lut_discrepancies.md D2). The table covers both.

| Consumer | Entry point | min absent (60 Spektra rows, 13 Whiteside) | min == max (26 Spektra long-tail, 5 compression, ...) |
|---|---|---|---|
| Burn / breakage gate | `tool_load/chipload.rs` ~L585, `derate_chipload_bounds(.., AllowHalfBand)` | High side hard: the printed value is the breakage bound. Low side not modelled: `ChipBounds::below_low` returns `None` (`tool_load/verdict.rs` ~L1388), `approach_to_min` is `None`. Source class `VendorLutMissingAe` or `VendorLut`. | High side hard. The row is `ChipBoundsSource::VendorLutPointPreset` (L600-613), so `low_side_is_advisory()` is true (`verdict.rs` L1292): any run below the printed value gives a burn advisory, not `Exceeds(Low)`. |
| Envelope resolver (viewport colour, sim, advisor) | `tool_load/mod.rs` `chipload_envelope_for_toolpath` L302, `RequireBoth` ~L364 | Returns `None`. The printed value is dropped with the missing minimum. | Returns `v..v`, an empty range. |
| Simulation modulator | `session/compute/simulation.rs` L542-544, `ChiploadBand::new` | No band: the BANDLESS arm (`dressup/feed_modulation.rs` L766-778) keeps the commanded feed, clamped to the machine ceiling, then the plunge guard. No chipload target and no chipload cap. | `ChiploadBand::new(v, v)` succeeds (it rejects only `max < min`, L127-139). ConstrainedMax targets `band.max` (L443) and floors at `band.min x rpm x flutes` (L586): target and floor are one feed where no other limit binds lower. |
| Strategy advisor | `session/compute.rs` `optimized_candidate` L1496, band fetch L1531-1540 | `chipload_envelope_for_toolpath` returns `None`, so the `?` returns `None`. The advisor times the raw path with the Suggest-warning regime; it does not optimise. | A zero-width band; the candidate is modulated as in the row above. |
| Suggest (recipe) | `feeds/mod.rs` L1627-1636, `RequireBoth` | The chip load is the printed value; `bounds` is `None`, so the feed-up loop does not run and the card shows one line. The rubbing floor falls back to the repo constant 0.025 mm (`RUBBING_FLOOR_MM_TOOTH`, L1142; `effective_rubbing_floor`, L1229). | `bounds` is `v..v`. The rubbing floor is the published min (ruling R4 Q9) when it is below 0.025 mm. |

Summary of §3: with the min absent, the printed value reaches only the burn
gate (as a breakage bound) and the Suggest chip load. The modulator, the
advisor and the viewport discard the printed maximum together with the
missing minimum. With min == max, every consumer gets the value, but the
modulator has no room to move and the gate softens the low side.

## 4. The framing question for the reconciler (not ruled here)

The LUT reads the one Amana value as a maximum: the Spektra rows say
"single printed CPT ... encoded as chipload_max only". No printed text
calls it a maximum. The texts found say:

- Amana chart: "these values are only recommendations."
- Freud chart (grade a): "Freud’s chart contains recommended starting
  points"; "start your tests with the lower feed rates".
- ToolsToday for Amana (grade c): "Amana's recommended starting
  calculations"; its example runs 2.8x above the printed value.

So the evidence reads a single printed value as a **start point**, not as
an upper bound. A G4 claim that builds a minimum below the printed value
(for example from the Onsrud/Freud min/max ratio) assumes the printed value
is the top of a band. The evidence does not say that. The reconciler must
state which reading it takes and cite these three quotes.

## 5. Candidate rows

`candidate_rows.json` is empty. G4's targets are statements, not printed
rows. The Freud and compression rows are already in the LUT and equal the
chart text. The new Spektra v41 sizes (4mm, 8mm, 10mm, 4 Flute) and the
1.5 mm / 3 mm revisions belong to G1; lut_discrepancies.md D3 lists them
with the verbatim lines.

## 6. The band shape the LUT gives (DERIVED, not printed)

From `band_shape_derived.txt` (script `scripts/g4_band_shape.py`):

- Onsrud flat-end sheets print a band of a **constant absolute width**:
  0.002 in in almost every cell (a few 0.001 in or 0.003 in). The min/max
  ratio therefore rises with size (0.67-0.92). The Onsrud 77-100 tapered
  rows are also 0.002 in wide (ratio 0.60-0.71).
- Freud flat-end: width 0.002-0.004 in; ratio 0.40-0.88 (1/8" hardwood
  .002"-.005" is the widest).
- Amana ball v7 / ZrN v8: width 0.0020 in at most; ratio 0.50-0.78.
- V-bit charts (AMS-159, Spektra engraving): width 0.004 in; ratio 0.33-0.43.
- Exclude the 14 grade-c Spektra "band" rows (D1): repo-authored.

This is an input for Phase 2, not a fit. Note: a constant-width band and a
constant-ratio band give different minimums on small tools. A Spektra 1/32"
value (.0010") minus 0.002 in is negative.

Cross-check of this programme's inventory: lut_axes.txt "flat 0.88" is the
Onsrud-dominated median. Its "86 one value" Spektra rows are 60 min-absent
plus 26 min == max.

## 7. What I searched

Web searches (2026-09-24):

1. `Onsrud chip load "start" recommended range router bit wood increase feed until`
2. `Amana Tool chip load chart "start" recommended chipload adjust increase decrease burning`
3. `onsrud.com "chip load" "middle of the range"` -> the ShopBot PDF.
4. `Onsrud "Increase the cutting speed" "until the quality" finish "decrease" RPM router` -> the ShopBot PDF.
5. `amanatool.com chip load "starting point" CNC router bits feed speed guide`
6. `Whiteside router bits chip load chart "start" "low end" OR "middle" recommended chipload range` -> no Whiteside text on band use.

Documents fetched and kept: see `sources.json`.

Other documents fetched:

- `https://onsrud.com/images/H%20Wood%20Cutting%20Data2.pdf` (sha256
  e3df6499...): an image-only older Onsrud Hard Wood sheet. No tesseract on
  this machine; I viewed it as an image at 80 dpi. It carries the same
  depth-of-cut note and formulas and no band-use note. No text copy, no
  numbers taken from it.
- Spektra v24 PDF: re-downloaded, sha256 equals the manifest.
- Spektra v25-v33, v35-v40: PDFs exist (hashes of v25-v28 recorded below);
  I compared v28 and v41 to v24 only. Not stored.
  v25 7e4dac8b..., v26 4b785b11..., v27 7117d4d9..., v28 97cbfaa9....

Dead ends:

- `https://www.amanatool.com/router-bit-technical-information`: Cloudflare
  403 (curl and WebFetch). Used the Wayback copy.
- `http://www.onsrud.com/xdoc/FeedSpeeds` (cited by ShopBot): 404 today.
  Used the Wayback copy.
- `http://s3.amazonaws.com/fablab-uc/.../Feeds_and_Speeds_Chart.pdf`: returns
  an XML error, not a PDF.
- Spektra URL pattern v34 and v42-v60: returns an HTML page, not a PDF.
- Amana product pages (to confirm which Spektra version is linked today):
  403.

## 8. Not found

- Any vendor "+/- %" or band width rule for a single printed value.
- Any Onsrud document that prints "start at the middle" or the
  "increase until finish degrades, then -10 %" strategy (only ShopBot's
  restatement, grade c).
- Any vendor statement that a single printed value is a maximum.
- Any vendor number for the burn side (the minimum) of a one-value chart.
