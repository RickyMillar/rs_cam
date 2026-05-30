# Verification Report — Amana feeds/speeds staging (data_ingest_2026-05-29)

Independent verifier. Web research only. No JSON/code modified.

Method: All Amana PDFs re-fetched 2026-05-29 with
`curl -sL -A "Mozilla/5.0 (X11; Linux x86_64)" <url>` + `pdftotext -layout`
(the CDN 403-blocks WebFetch). Amana's HTML product pages are behind a
Cloudflare JS challenge (curl returns a "Just a moment..." stub and WebFetch
returns 403), so per-SKU dimensions were taken from `toolstoday.com` spec pages
(curl-readable) and corroborated with WebSearch result snippets. Inch→mm = inch × 25.4.

---

## TL;DR

| Item | Verdict |
|------|---------|
| 4 sampled chiploads (Mission 2) | **All 4 CONFIRM** — match the source PDFs exactly |
| Chipload material-independence question | **The chart genuinely holds chipload constant across all material columns; only Feed/IPM is shared too. The collector did NOT wrongly copy — it is faithful.** See caveats below for the *angle* dimension. |
| V-groove / engraving `diameter_mm` (Mission 1) | **The charts do NOT state a cutting diameter for ANY angle.** Diameters must come from the per-SKU product specs. See per-angle table. **Two of the rows have a tool-family mismatch** (chart "V-Groove" 30°/45° SKUs are actually conical ENGRAVING bits with no fixed cutting diameter). |

---

## Mission 1 — Cutting diameter per angle

### Critical structural finding (read first)

Neither speed chart prints a cutting-diameter column. The AMS-159 chart and the
Spektra chart are organized by **included angle** (and, for engraving, **tip
width**), with a "Tool Reference #'s" block listing one SKU per angle. The real
cutting diameter lives only on the individual SKU product pages.

When the SKUs are looked up, they split into two physically different tool types:

- **True V-groove bits** (carving liner / carbide-tipped "Zero Point"): have a
  flat, published cutting **Diameter "D"** (the bit reaches full width D at the
  point listed, then the V tapers to a point). Examples: 18° = 1/4" D,
  60° = 1/2" D, 90° = 3/8" D.
- **Conical engraving bits** (Spektra signmaking, and the 30°/45° SKUs the
  AMS-159 chart cites): specified by **tip width + included angle + cutting
  height (CH)**. Amana does **not** publish a single "cutting diameter" for
  these — the effective diameter ramps from the tip width up to the body
  diameter (~1/4") as cut depth increases. The max usable diameter is the body
  stub, ~0.25" / 6.35 mm, reached at full CH.

So "the cutting diameter for each angle" is only well-defined for the V-groove
bits. For the engraving bits I report (a) the tip width Amana publishes and
(b) the geometrically-derived max cutting diameter `D_max = tip + 2·CH·tan(½·angle)`,
flagged as derived, not quoted.

### AMS-159 V-Groove chart — per-angle SKU and diameter

Tool Reference row, verbatim from the PDF:
> "18° 30° 45° 60° 90° / Solid Carbide 1 Flute (18/30/45) / Carbide Tipped 2 Flute (60/90) / **45783 45771 45623 45707 45701**"

| Angle | Chart SKU | Actual SKU identity (product page) | Tool type | Cutting diameter | Verbatim quote | inch→mm |
|-------|-----------|------------------------------------|-----------|------------------|----------------|---------|
| 18° | 45783 | "Solid Carbide Carving Liner 18 Deg x **1/4 D** x 5/8 CH x 1/4 SHK" | true V/carving-liner | **1/4" = 6.35 mm** | "Amana Tool 45783 Solid Carbide Carving Liner 18 Deg x 1/4 D x 5/8 CH x 1/4 SHK x 2-1/2 Inch Long Single Flute" (toolstoday v-9829-45783) | 0.25 × 25.4 = 6.35 |
| 30° | 45771 | "Solid Carbide **30 Degree Engraving 0.005 Tip Width** x 1/4 SHK" | conical engraving | **no published D**; tip 0.005"=0.127 mm; derived D_max ≈ 0.23" ≈ **5.87 mm** (CH 0.440") | "Amana Tool 45771 ... 30 Degree Engraving 0.005 Tip Width ... Cutting Height: 0.440"" (toolstoday v-13.../WebSearch) | tip 0.005×25.4=0.127; D_max=0.005+2·0.440·tan15°=0.231"→5.87 |
| 45° | 45623 | "Solid Carbide **45 Degree Engraving 0.042 Tip Width** x 1/4 SHK", page lists "**1/4" diameter** ... 0.242" cutting height" | conical engraving | tip 0.042"=1.0668 mm; page-stated body **1/4" = 6.35 mm**; derived D_max = 0.242"→**6.15 mm** | "Amana 45623 features a 1/4" diameter with a 45° angle, 0.242" cutting height, 1/4" shank, 0.042" tip width" (toolstoday v-13785-45623) | tip 0.042×25.4=1.0668; D_max=0.042+2·0.242·tan22.5°=0.242"→6.15; body 0.25"→6.35 |
| 60° | 45707 | "Carbide Tipped Zero Point V-Groove 60 Deg x **1/2 D** x 13/32 CH x 1/4 SHK", 2 flute | true V-groove | **1/2" = 12.7 mm** | "Amana Tool 45707 Carbide Tipped Zero Point V Groove 60 Deg x 1/2 D x 13/32 CH x 1/4 Inch SHK ... 2 flutes" (toolstoday v-13394-45707) | 0.5 × 25.4 = 12.7 |
| 90° | 45701 | "Carbide Tipped Zero Point V-Groove 90 Deg x **3/8 D** x 1/2 CH x 1/4 SHK", 2 flute | true V-groove | **3/8" = 9.525 mm** | "Amana Tool 45701 Carbide Tipped Zero Point V Groove 90 Deg x 3/8 D x 1/2 CH x 1/4 Inch SHK" (toolstoday v-13392-45701) | 0.375 × 25.4 = 9.525 |

Representative diameter to record per angle (recommended): 18°=**6.35 mm**,
30°=**6.35 mm** (use the 1/4" body; or 5.87 mm derived), 45°=**6.35 mm** (page
states 1/4" body), 60°=**12.7 mm**, 90°=**9.525 mm**.

> NOTE / data-integrity flag: the AMS-159 chart mixes families. 18/60/90° are
> genuine V-groove bits; 30° (45771) and 45° (45623) are conical engraving bits
> that share SKUs with the Spektra engraving chart. For the staged V-bit rows,
> 30° and 45° do **not** have a clean fixed cutting diameter — treat them like
> the engraving rows (tip + angle + body ~1/4").

### Spektra Engraving chart — per-angle SKU and diameter

Tool Reference block, verbatim from the PDF:
> "15° 30° 45° 120° / 45606-K (120°) / 45611-K (15°) / 45620-K (30°) / 45622-K (45°) / 45630-K (30°) / 45771-K (30°) / 45773-K (30°) / 45774-K (30°) / 45632-K (45°)"

The chart lists **multiple SKUs per angle**, distinguished by tip width (the
column headers: 15°=0.005", 30°=0.005"–0.030", 45°=0.042", 120°=0.015").
**No cutting diameter is stated anywhere on the chart.** All these bits have a
~1/4" body, so D_max ≈ 1/4" = 6.35 mm at full cutting height; the working
diameter at any shallower depth is `tip + 2·depth·tan(½·angle)`.

| Angle | Chart tip width | Representative SKU | Tip width | Published body D | Derived D_max | Verbatim quote | arithmetic |
|-------|-----------------|--------------------|-----------|------------------|---------------|----------------|-----------|
| 15° | 0.005" | 45611-K | 0.005" = 0.127 mm | ~1/4" body | CH 0.93" → D_max = 0.005+2·0.93·tan7.5° = **0.250" = 6.35 mm** | "45611-K ... 15 Degree Engraving 0.005 Tip Width ... 0.93" cutting height, 1/4" shank" (WebSearch/amanatool) | 0.005×25.4=0.127; D_max→6.35 |
| 30° | 0.005"–0.030" | 45620-K (0.0108" tip) / 45771-K (0.005" tip) | 0.0108"=0.274 mm or 0.005"=0.127 mm | ~1/4" body | 45620: CH 0.413"→D_max 0.232"≈**5.9 mm**; 45771: CH 0.440"→**5.87 mm** | "45620 ... 30 Degree Engraving 0.0108 Tip Width ... cutting height 0.413""; "45771 ... 0.005 Tip ... 0.440" CH" | see derivations |
| 45° | 0.042" | 45623-K / 45632-K | 45623: 0.042"=1.0668 mm; 45632: 0.005"=0.127 mm | 45623 page: **1/4" = 6.35 mm** | 45623 CH 0.242"→D_max=**6.15 mm** | "45623 features a 1/4" diameter ... 0.242" cutting height ... 0.042" tip width" | tip 1.0668; D_max 6.15; body 6.35 |
| 120° | 0.015" | 45606-K | 0.015" = 0.381 mm | ~1/4" body (cone truncated by body) | tip+cone reaches 1/4" body within ~0.072" of depth; **practical D_max ≈ 1/4" = 6.35 mm** | "45606-K ... 120 Degree Engraving 0.015 Tip Width ... 0.575" cutting height, 1/4" shank" | tip 0.015×25.4=0.381; pure-cone formula overshoots body, so capped at body 6.35 |

> Engraving-diameter recommendation: record **tip_diameter_mm** (already present
> for some rows) plus a **diameter_mm = 6.35** (the 1/4" body / max cutting
> diameter) for every engraving row, with a note that the working diameter is
> depth-dependent for a conical bit. Do **not** invent a single mid-cone number.

### Diameter-field corrections the collector should apply

1. **30° and 45° V-groove rows** (`amana-vbit-*-30deg-1f`, `amana-vbit-*-45deg-1f`):
   no clean fixed D. These are engraving bits (45771, 45623). Either record
   `diameter_mm = 6.35` (1/4" body) with a conical note, or move them to the
   engraving family.
2. **Engraving rows** missing `diameter_mm` entirely: add `diameter_mm = 6.35`
   (1/4" body). The collector inconsistently set `tip_diameter_mm` on some
   rows (15°, 45°, 120°) but left 30° with no tip field at all — 30° tip should
   be 0.127 mm (45771, 0.005") or 0.274 mm (45620, 0.0108"); pick per the SKU
   actually intended.
3. `amana-engrave-softwood-trace-45deg-1f` has `tip_diameter_mm: 1.0668`
   (0.042", = SKU 45623) — **correct**. The matching 30° rows should likewise
   carry a tip width; they currently omit it.
4. 18°=6.35 mm, 60°=12.7 mm, 90°=9.525 mm are unambiguous and should be added as
   `diameter_mm`.

---

## Mission 2 — Adversarial re-verification of 4 sampled chiploads

All re-fetched from the source PDFs (verbatim chart rows below).

Compression chart (`Solid-Carbide-Compression-Spirals-v8.pdf`), **2-Flute** table, verbatim:
```
1/4"  110"  .0031" 55"  220"  .0061" 110"  110" .0031" 55"   110" .0031" 55"
1/2"  280"  .0077" 140" 400"  .0111" 200"  280" .0077" 140"  280" .0077" 140"
```
(columns: Wood | MDF/Laminate | Plywood | Plastic, each = Feed IPM / Chip Load / Ramp Down)

| # | Observation | Staged value | Source value | Verdict |
|---|-------------|--------------|--------------|---------|
| 1 | `amana-compression-wood-pocket-6350-2f` | 0.0787 mm (0.0031" @ 1/4" 2F Wood) | 1/4" 2F Wood = **.0031"** → 0.0031×25.4 = 0.07874 mm | **CONFIRM** |
| 2 | `amana-compression-mdf-pocket-12700-2f` | 0.2819 mm (0.0111" @ 1/2" 2F MDF) | 1/2" 2F MDF/Laminate = **.0111"** → 0.0111×25.4 = 0.28194 mm | **CONFIRM** |
| 3 | `amana-vbit-softwood-trace-45deg-1f` | 0.0762–0.1778 mm (0.003"–0.007") | AMS-159 Soft Wood @ 45° = **0.003"–0.007"** → 0.0762–0.1778 mm | **CONFIRM** |
| 4 | `amana-vbit-hardwood-trace-45deg-1f` | 0.0762–0.1778 mm (0.003"–0.007") | AMS-159 Hard Wood @ 45° = **0.003"–0.007"** → 0.0762–0.1778 mm | **CONFIRM** |

All four chipload values are faithful to the source.

---

## THE KEY INTEGRITY QUESTION — is chipload identical across material columns?

**Answer: YES — the charts genuinely hold chipload (and feed) constant across
all material columns. The collector did NOT erroneously copy one column onto the
others. The transcription is faithful to the source.**

### AMS-159 V-Groove — verbatim, all material rows for 18/30/45°:
```
Soft Wood     50" - 130"  0.003" - 0.007"   50" - 130" 0.003" - 0.007"   50" - 130" 0.003" - 0.007"
Hard Wood     50" - 130"  0.003" - 0.007"   50" - 130" 0.003" - 0.007"   50" - 130" 0.003" - 0.007"
Soft Plastic  50" - 130"  0.003" - 0.007"   50" - 130" 0.003" - 0.007"   50" - 130" 0.003" - 0.007"
Hard Plastic  50" - 130"  0.003" - 0.007"   50" - 130" 0.003" - 0.007"   50" - 130" 0.003" - 0.007"
Aluminum      50" - 130"  0.003" - 0.007"   50" - 130" 0.003" - 0.007"   50" - 130" 0.003" - 0.007"
Solid Surface 50" - 130"  0.003" - 0.007"   50" - 130" 0.003" - 0.007"   50" - 130" 0.003" - 0.007"
```
Every material row is **identical** for 18/30/45° — not just the chipload, but
the feed IPM too. The only material differentiation is that Aluminum and Solid
Surface are marked **N/A** at 60° and 90°.

### Spektra Engraving — verbatim, all material rows:
```
Soft Wood     50"-125" 0.003"-0.007"  50"-125" 0.003"-0.007"  50"-125" 0.003"-0.007"  40"-110" 0.002"-0.006"
Hard Wood     50"-125" 0.003"-0.007"  50"-125" 0.003"-0.007"  50"-125" 0.003"-0.007"  40"-110" 0.002"-0.006"
Soft Plastic  50"-125" 0.003"-0.007"  50"-125" 0.003"-0.007"  50"-125" 0.003"-0.007"  40"-110" 0.002"-0.006"
Hard Plastic  50"-125" 0.003"-0.007"  50"-125" 0.003"-0.007"  50"-125" 0.003"-0.007"  40"-110" 0.002"-0.006"
Solid Surface 50"-125" 0.003"-0.007"  50"-125" 0.003"-0.007"  50"-125" 0.003"-0.007"  40"-110" 0.002"-0.006"
```
Identical across all 5 materials for every angle.

### Nuance — chipload is NOT identical across ALL angles:

- **AMS-159 V-groove:** 18/30/45° = 0.003"–0.007" (range). 60/90° = single
  **0.003"** value (2-flute carbide-tipped). The staged 60/90° rows correctly
  record min=max=0.0762 mm. ✓
- **Spektra:** 15/30/45° = 0.003"–0.007"; **120° = 0.002"–0.006"** (different).
  The staged 120° rows correctly use 0.0508–0.1524 mm. ✓

So: **chipload is constant across materials, but varies by angle group.** The
staged rows respect both facts. No material-copy error detected. The provenance
note (lines 51–54, "the full table is identical across … materials") is accurate.

---

## Other data-integrity concerns

1. **Tool-family mismatch (most important).** AMS-159 chart's 30° (45771) and
   45° (45623) "V-groove" SKUs are actually conical **engraving** bits — the
   same SKUs that appear on the Spektra engraving chart. The staged
   `amana-vbit-*-30deg-1f` / `*-45deg-1f` rows are tagged
   `tool_subfamily: solid_carbide_vgroove` but the physical tool is an engraving
   bit with no fixed cutting diameter. For a safety-critical tool-load table,
   this matters: a "V-groove with 1/4" diameter" model would over/under-estimate
   engagement vs. a conical bit whose effective diameter ramps with depth.
   Recommend reconciling family + recording the conical geometry (tip + angle).

2. **No `diameter_mm` is derivable from the charts themselves.** The collector
   correctly omitted it rather than inventing one — good. But the field is
   REQUIRED, so the per-SKU diameters above (or the 6.35 mm body for engraving
   bits) must be merged in from the product specs, not the speed charts.

3. **`tip_diameter_mm` inconsistency in engraving rows.** 15°, 45°, 120° rows
   carry `tip_diameter_mm`; the 30° rows do not. They should — 30° tip is
   0.127 mm (45771, 0.005") or 0.274 mm (45620, 0.0108") depending on intended SKU.
   The chart's 30° column header is a *range* "0.005"–0.030"", so the SKU choice
   determines the tip; the chart alone is ambiguous for 30°.

4. **Compression "Wood" → `hardwood` mapping** (provenance lines 124–126): the
   chart's "Wood" column is generic. Mapping to `hardwood` is the conservative
   (stiffer) choice and is documented — acceptable, but note the chart does not
   distinguish hard vs soft wood, so a `softwood` consumer would inherit the
   same numbers. Not a transcription error, just a mapping judgment to be aware of.

5. **All four sampled chiploads and both chart material-independence claims
   checked out exactly.** Confidence in the collector's numeric transcription is
   high; the remaining issues are taxonomic (tool family) and a required-field
   gap (diameter), not bad numbers.

---

## Sources

- AMS-159 V-Groove chart PDF: https://www.amanatool.com/pub/media/productattachments/AMS-159-18-30-45-60-90-Degree-V-Groove-Speed-Chart-v2.pdf
- Spektra Engraving chart PDF: https://www.amanatool.com/pub/media/productattachments/Spektra-15-30-45-120-Degree-Engraving-Speed-Chart-v4.pdf
- Compression Spirals chart PDF: https://www.amanatool.com/pub/media/productattachments/Solid-Carbide-Compression-Spirals-v8.pdf
- 45783 (18° carving liner, 1/4 D): https://toolstoday.com/v-9829-45783.html
- 45771 (30° engraving 0.005" tip, CH 0.440"): https://www.amanatool.com/45771-solid-carbide-30-degree-engraving-0-005-tip-width-x-1-4-inch-shank-signmaking.html
- 45623 (45° engraving 0.042" tip, 1/4" dia, CH 0.242"): https://toolstoday.com/v-13785-45623.html
- 45620 (30° engraving 0.0108" tip, CH 0.413"): https://toolstoday.com/v-13436-45620.html
- 45707 (60° Zero-Point V-groove, 1/2 D): https://toolstoday.com/v-13394-45707.html
- 45701 (90° Zero-Point V-groove, 3/8 D): https://www.toolstoday.com/v-13392-45701.html
- 45611-K (15° engraving 0.005" tip, CH 0.93"): https://toolstoday.com/v-14595-45611-k.html
- 45606-K (120° engraving 0.015" tip, CH 0.575"): https://toolstoday.com/v-15279-45606-k.html
