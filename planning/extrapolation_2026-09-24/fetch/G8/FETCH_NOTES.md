# G8 fetch notes: long tool and small tool loads

Date: 2026-09-24. Agent: G8 research agent (Phase 1, fetch).
Builder: `planning/extrapolation_2026-09-24/scripts/g8_build.py`. The script
hashes every file and checks every verbatim string against its stored text.
Run it again after any change to the stored texts.

## 1. The finding

- No router-bit vendor that I could reach prints a stickout or L/D de-rate
  for wood. Onsrud and Amana print a **depth-of-cut** rule (2 x D -25 %,
  3 x D -50 %). The LUT already holds that rule in `ap_rule`.
- Metal-cutting vendors print length rules, but the rules disagree and do
  not define "long":
  - Harvey: a neck-length table (3x 120 %, 5x 100 %, 8x 80 %, 12x 65 %,
    15x 55 %). It is for undercutting tools, it uses the neck diameter, and
    its base is 5x.
  - Niagara: "Long and Extra" -50 % feed.
  - RedLine: -20 % feed, "up to 50 %" feed, and -10 % speed and chip load,
    all in one document.
  - RedLine p216 and Fullerton: -25 % SFM, which is a speed factor, not a
    chip-load factor.
- Two vendors print the physics instead of a factor: Helical ("rigidity
  ... third power (L3) ... fourth power (D4)") and the Harvey blog
  ("doubling the length ... 8 times more deflection"). This is the
  cantilever law that `feeds::force` and the deflection gate already use.
  It is a second witness for the engine's model.
- One wood chart prints a lower chip load for a long-reach tool: the Amana
  ZrN v8 "3 Flute Extra Long" block. Against the standard rows of the same
  chart, the maximum is x0.875 (3/8 in), x0.889 (1/2 in) and x1.0 (1/4 in
  against 6 mm). These ratios are **derived**. The extra-long tools have a
  relieved neck and a short flute. At 3/8 in both tools are 4 in overall.
  So the ratio is not a pure stickout effect. It is also a chip-load ratio,
  and the repo's 0.88 share acts on the load target, not on the chip load.
- Conclusion for the reconciler: the evidence supports PLAN §2 "a model
  the engine already has, to be preferred over a factor". No printed source
  gives the 0.88 / 0.75 share or its 4 x D / 6 x D thresholds. The share
  stays unsourced. The printed factors are metal factors on other axes.

## 2. Micro tools

- The minimum chip thickness against the edge radius is a metal and
  crystal result. The open paper (Micromachines 2020, PMC7600950) gives
  hmin = 0.17 rn for copper and cites 0.14-0.49 rn for other metals. I found
  no value for wood.
- The edge radius of a carbide router bit is not printed by any vendor I
  reached. Open wood papers print the edge radius of other carbide wood
  tools: 5.91-5.95 um (carbide drills in MDF) and 8 um (carbide saw teeth in
  spruce). HSS planer knives go from 2.08 um sharp to 13.42 um worn. A PCD
  cutter in WPC is about 10 um.
- Derived estimate (cross-material, grade c): hmin is about 0.8-3.9 um per
  tooth for a sharp carbide edge in wood. A 1 mm tool at the PreciseBits
  start point takes 30 um per tooth. Runout at the 2 % limit on the same
  tool is 20 um. On this estimate the edge-radius floor does not bind for
  wood micro tools; runout and breakage do. This is an inference.
- Two vendors print the same runout limit: TIR at most 2 % of the tool
  diameter (PreciseBits FAQ, Harvey blog).
- PreciseBits prints a test procedure, not a chart: start at 3 % of D per
  flute in all woods, plunge 2 x D / 1 x D / 0.5 x D by Janka band, and take
  0.75 x the failure feed. The start point is not a recommendation.

## 3. Output files

| File | Content |
|---|---|
| `sources.json` | 20 sources, manifest style, with sha256 and the hashed file |
| `candidate_rows.json` | 14 rows: Amana extra-long 3-flute, wood, exact, grade a |
| `derate_factors.json` | 21 entries: length factors, limits, the derived Amana ratio |
| `micro_rules.json` | 20 entries: hmin / rn, edge radii, runout, PreciseBits rules, one derived estimate |
| `lut_discrepancies.md` | no wrong LUT row; three manifest notes |
| `sources/` | stored texts |
| `pdf/` | downloaded PDFs (not committed) |
| `raw/` | downloaded HTML and JATS XML |

Candidate rows per source:

- `amana_zrn_3d_profiling_v8`: 12 rows (1/4, 3/8 and 1/2 in; ball nose
  parallel/finish and flat end pocket/roughing; softwood and MDF).
- `amana_spektra_3d_profiling_v6`: 2 rows (1/4 in 46490-K ball nose;
  softwood and MDF).

I did not force the length factors into the LUT observation schema. They
are factors on a ratio axis, mostly for metals. They are in
`derate_factors.json` with a `frame_note` each.

## 4. What I searched

Web searches (WebSearch):

1. `Harvey Tool undercutting end mill speeds feeds neck length multiple chip load 3x 120% 5x 100% 8x 80% pdf`: found SF_23200 on widen.net.
2. `router bit stickout reduce feed rate percent per diameter overhang Onsrud Amana`: only the depth-of-cut rule.
3. `Harvey Tool miniature end mill long reach reduced neck speeds feeds adjustment table L/D`: product pages; SF_846100 prints no reach factor.
4. `PreciseBits micro tool chip load guidance minimum chip load edge radius router`: PreciseBits tutorial and FAQ; PMC7600950.
5. `Kennametal solid end mill "L/D" feed reduction long reach table percent`: product pages and a Scribd copy only.
6. `Onsrud router bit tool stickout "cutting edge length" extension collet recommendation depth chipload reduce`: distributor pages only.
7. `Onsrud "CNC Production Routing Guide" pdf`: onsrud.com URL and the precisionboard.com mirror.
8. `"For Long and Extra Carbide Reduce Feed by 50%" ...`: the carbide3d.com forum upload (Niagara Cutter), RedLine p216, Tru-Edge.
9. `"For extra long endmills, reduce SFM by 25%" Redline ...`: RedLine p216, RedLine tech info, Fullerton.
10. `Amana Tool extra long router bit reduce feed rate "long" "reduce" chip load recommendation pdf`: the Spektra 3D profiling chart v6.
11. `Garr Tool long reach end mill speeds feeds reduction "length of cut" "reduce" percent`: Garr HP milling guides (no length rule).
12. `Kennametal "overhang" "reduce feed" solid carbide end mill catalog technical "L/D" "3xD" "5xD"`: master catalogs on productivity.com.
13. `Amana 46491 / 46494 46495 / 46493 46490 ...`: catalog geometry of the extra-long and standard tools.
14. `cutting edge radius new sharp tungsten carbide wood milling tool ...` and `minimum chip thickness wood cutting edge radius ratio ploughing ...`: metal papers and a Springer J Wood Sci paper.
15. `Harvey Performance In the Loupe miniature end mill runout chip load ...`: two Harvey blog posts.
16. Europe PMC REST search `"edge radius" AND (wood OR MDF OR particleboard) AND (milling OR cutting) AND OPEN_ACCESS:y`: four wood papers with a measured edge radius.

## 5. Dead ends

| URL | Result |
|---|---|
| `http://www.fastenal.com/content/documents/2016/06/FastenalClevelandCarbideEndMillBooklet.pdf` | "Access Denied" (Akamai), from a previous run; `pdf/fastenal_cleveland.pdf` is that HTML page |
| a Kennametal file on MSC (URL not recorded by the previous run) | Incapsula bot wall; `pdf/kennametal_msc_endmill.pdf` is that HTML page |
| `https://productivity.com/wp-content/uploads/2022/08/Kennametal-2023-Solid-Carbide-End-Milling-Inch-Master-Catalog-Interactive.pdf` | HTML page, not the PDF |
| `http://www.onsrud.com/files/pdf/LMT-Onsrud-CNC-Prod-Routing-Guide.pdf` | HTML page; the precisionboard.com mirror gave the PDF |
| `https://www.garrtool.com/doc/pdf/tech/TECH_MILLING_HP_f.pdf` and `..._m.pdf` | HTML page on 2026-09-24 |
| `https://www.amanatool.com/catalogsearch/result/?q=4649` | HTTP 403 |
| amanatool.com product pages (46491, 46494) | bot challenge "Just a moment..."; ToolsToday pages used instead |
| `https://link.springer.com/article/10.1007/s10086-017-1656-x` and the SpringerOpen copy | cookie redirect / "Client Challenge" |
| Harvey Machining Advisor Pro | interactive tool; no printed table to store |

## 6. Files left by a previous run

A previous run left PDFs in `pdf/` with no URL record. I recovered the URL
and matched the hash for these: `harvey_SF_23200.pdf`,
`helical_guidebook_2016.pdf`, `onsrud_cnc_prod_routing_guide.pdf`,
`c3d_hp_endmills.pdf` (Niagara Cutter), `redline_p216.pdf`. They are in
`sources.json`.

These are not in `sources.json`:

- `garr_milling_hp_f.pdf`, `garr_milling_hp_m.pdf`: no length rule (only
  "If tool displays chatter, increase feed (IPM) up to 30% and reduce speed
  (RPM) by 10%"). The URLs return HTML today, so I cannot match the hash.
- `harvey_SF_958300.pdf`: a hardened-steel ball finisher chart. ".8x Reach
  Multiple" is a product name. It prints no reach factor. URL not recovered.
- `iti_speeds_feeds.pdf`: prints only a depth rule ("AXIAL DEPTH OF CUT:
  ... NOT TO EXCEED 1-1/2 TIMES THE DIAMETER ... FEED PER TOOTH SHOULD BE
  REDUCED BY 50%"). URL not recovered.
- `truedge_endmills.pdf`: URL recovered
  (`https://www.tru-edge.com/wp-content/uploads/2019/09/Feeds-and-Speeds-Endmills.pdf`,
  hash matches) but it prints no length rule ("HSS Endmills Reduce SFM
  50%" only).
- `fastenal_cleveland.pdf`, `kennametal_msc_endmill.pdf`: HTML error pages.

## 7. Not found

- A stickout, overhang or L/D feed de-rate for router bits in wood. I
  reached the Onsrud routing guide and the Amana ZrN v8 and Spektra v6
  charts; none prints one. I did not search Whiteside or Freud for this
  axis.
- Any printed source for the repo's 0.88 / 0.75 share or for its 4 x D and
  6 x D thresholds.
- A Kennametal or Garr length de-rate in a document I could reach.
- The cutting-edge radius of a carbide router bit (vendor or paper).
- A minimum chip thickness for wood as a ratio of the edge radius.
- A PreciseBits chart with chip loads per micro-tool size (the site prints
  a test procedure and online calculators, not a table).

## 8. Engine context (read, not changed)

- `feeds::long_tool_load_share` (`crates/rs_cam_core/src/feeds/mod.rs`):
  1.0 up to 4 x D stickout, 0.88 above 4 x D, 0.75 above 6 x D; "repo rule,
  unsourced". Since ruling R4 Q7 (2026-09-24) Suggest pass 6b multiplies
  the aggressiveness (the load target) by it. The feed does not take it.
  `FeedsWarning::LongToolDerate` is the visible record.
- R4 spec §3.2: `ToolConfig::new_default` sets stickout 45 mm, so every
  tool of 7.5 mm or less is above 6 x D. The share fires on 470 of 498
  matrix cells.
- `feeds::force`: the lateral force is affine in the peak chip thickness
  (`F_lat = ap (Ks h_eff + F_edge)`), and the deflection cap
  `chipload_cap_for_deflection_with_reason` divides a tip-deflection budget
  by the compliance. The Helical and Harvey L^3 / D^4 statements are the
  same law in words.
