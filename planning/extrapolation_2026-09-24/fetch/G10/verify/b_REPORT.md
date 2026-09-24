# G10 verify, part `hobby` (verifier b), 2026-09-25

Scope: `parts/hobby_sources.json` (21 records, 16 with a stored text),
`parts/hobby_statements.json` (143 statements), `parts/hobby_NOTES.md`.
Machine-readable verdicts: `verify/b_verdicts.json`. Fresh bytes and scratch
scripts: `verify/b_work/`.

## 1. Counts

| Verdict | Statements |
|---|---|
| confirmed | 142 |
| grade_wrong | 1 |
| wrong | 0 |
| not_found | 0 |

Skipped: none. The verifier checked all 106 Sienci rows, not only a sample.
A script parsed both wood pages of the fresh PDF (own `pdftotext -layout`)
and compared page, tier, feed, plunge, RPM, verbatim, material and the
plunge/feed fraction of each statement. All 106 agree. The verifier checked
the tier blocks on a render of page 1. The 36 rows whose fraction is not
exactly 0.50 or 0.33 (0.498-0.505, 0.330-0.335, and the 0.334 row
g10-hobby-031) are in that set.

All 37 other statements were checked by hand against the fresh copy (or the
stored PDF where the site is unreachable).

## 2. Non-confirmed statement

- **g10-hobby-019** (Fusion, `helix_diameter_max_frac` = 0.8), `grade_wrong`.
  The captions "Value of 1.8 x the Dia" and "Value of 0.8 x the Dia" occur
  as printed. But 0.8 x D is an example of a good value, not a maximum. The
  printed maximum is 1.0 x D (g10-hobby-018). Record 0.8 x D as an example,
  grade c.

## 3. Notes on confirmed statements (no verdict change)

- g10-hobby-030, 032, 033, 083, 085 (Sienci tapered ball): the PDF values
  are correct. The newer live web table differs (see §4.1).
- Sienci round-groove rows: `ball_nose` and `centre_cutting: true` are
  inferences. The chart prints only a plunge rate.
- Sienci "22mm Surfacing Bit" rows: `flat_end_mill` is an inference. Keep
  these rows out of an end-mill fit.
- g10-hobby-004 / 005: the material labels are incomplete. The pages also
  list metals (EM2E4) or G-10/FR-4, aluminium and hardwood (EM2E8). The class
  `any` is correct.
- g10-hobby-027: "p. ~40" is exactly PDF page 40.

## 4. Flagged points

1. **Sienci page against Sienci chart.** The page says: "we usually
   recommended lower plunge rates (100mm/min to 300mm/min) for most
   materials". This is for 2-flute 1/8 in end mills. The live page has
   datePublished 2021-04-19 and dateModified 2025-11-18. The chart
   (FeedsSpeedsMetric.pdf, created 2022-11-08) prints plunge = about 0.50 x
   feed, e.g. "1/8" Flat End Mill UC 1620 810". The chart is newer. Sienci
   names it as its guide in two places:
   - the blog of 2022-11-28: "standardized testing method ... downloadable
     PDF guides";
   - the current page resources.sienci.com/view/cnc-feeds-speeds/
     (dateModified 2026-05-13), which links the same PDF (same sha256).

   Use the chart. **New finding:** that live web table differs from the PDF
   on 5 tapered-ball rows. The web plunge is lower:
   - soft 1/4 in Coarse: 2470/820 (PDF 3170/1580);
   - soft 1/8 in Coarse: 2000/670 (PDF 2000/1000);
   - soft 1/8 in Fine: 3640/1210 (PDF 3640/1820);
   - hard 1/4 in Coarse: 1630/815 (PDF 3170/1580);
   - hard 1/8 in Coarse: 1290/645 (PDF 1290/650).

   On the soft page, all four web tapered rows are now about 0.33. The other
   feed and plunge values are the same on the web and in the PDF.
2. **Carbide 3D staff ramp-feed post.** The author is robgrz, post 1 of
   /t/50824, 2022-09-29T12:07Z. The Discourse JSON gives staff=true,
   admin=true and moderator=true (the staff badge). The post begins "We just
   posted an update to CC to include ramping in Pro". The rule describes the
   Carbide Create Pro behaviour at that build. It is not advice, and later
   builds can change it.
3. **Fusion helix diameter.** The "boss standing" sentence and both
   captions are confirmed. Fusion's value is the helix path diameter ("an
   optimal value causes the tool to overlap it's center"). A boss stays
   when the path radius is more than the tool radius. Thus "path radius <=
   0.5 x D" is a fair reading, and exact for a flat-bottom tool. **Extra:**
   the Ramping Angle figure prints "2°" (alt "ramping angle - 2 degrees").
   The dead-end table says the Fusion help prints no ramp angle. The help
   does show 2 deg, as an example only.
4. **Carbide Create 20 deg default.** Only a user post gives it
   (machinemoney, staff=false, 2024-03-27). No staff reply gives an angle.
   One search and the two Carbide 3D toolpath guide pages found no
   Carbide 3D document.
5. **CNCCookbook OSG 10-20 deg.** One search found no OSG primary. The
   value stays grade c, second hand.
6. **PreciseBits V-tip and tapered-ball pages.** EM2E4: "Plunge rate = 40
   IPM to 250 IPM". EM2E8: "Plunge rate = 5 IPM to 25 IPM" and "plunge style
   tip geometry". Tapered ball 2F: "Plunge rate = depends on speed (RPM)",
   with no number. No page has a ramp rule, a helix rule or a rule that
   forbids a plunge.
7. **Carbide 3D shared rows.** The rendered headers are "#201 .25" Square /
   #202 .25" Ball" (S3) and "#101 .125" Square / #102 .125" Ball" (Nomad).
   One FEED/PLUNGE column pair serves both tools. All ten wood rows and all
   fractions are confirmed. The charts do not print flute counts.
8. **Text headers.** All 16 stored texts have `accessed_on: 2026-09-25`.
   The sha256 of each stored text equals `text_sha256`. All raw files in
   `html/` and `pdf/` equal `raw_sha256`.
9. **Sienci tiers (notes §5.1).** The render of page 1 confirms the tiers:
   "1650 830" is in Reduced and "2190 1100" is in Regular.

## 5. Sources that changed or are unreachable

| Source | Result |
|---|---|
| carbide3d_community_ramping_cc_pro | `changed` (Discourse dynamic HTML). The quotes occur verbatim in the fresh HTML and JSON. |
| carbide3d_community_ramp_entry_angle | `changed` (the same cause). The quotes occur verbatim. |
| carbide3d_s3_feeds_250 | `unreachable`: web.archive.org timed out / refused the connection. The live URL gives a 38134 B HTML page. The stored PDF is byte-identical to `fetch/G9/pdf/S3_feeds_250.pdf` and equals the G9 `pdf_sha256`. |
| carbide3d_nomad883_feeds_125 | `unreachable` (the same cause). The stored PDF is byte-identical to `fetch/G9/pdf/Nomad883_feeds_125.pdf`. **But** `fetch/G9/sources.json` has no Nomad record. The header claim "sha256 equals the G9 record" is thus unsupported; only the G9 file matches. |
| shopbot_user_guide_2015 | `unreachable`: shopbottools.com gives nginx 403 for every URL from this host. The verifier checked the statement against the stored PDF. |

The other 11 sources: the fresh bytes match `raw_sha256`. The Sienci chart
also has a copy at the vendor URL
`resources.sienci.com/wp-content/uploads/2022/10/FeedsSpeedsMetric.pdf`,
with the same sha256.
