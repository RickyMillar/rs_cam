# R2 — Freesteel blog (Julian Todd, Martin Dunschen)

Reading agent R2, 2026-10-23. Ideas only. rs_cam takes no code from this
source. One post (2014-02-11) contains C++; this note copies none of it.

## Verification result: FOUND (through the Wayback Machine only)

| Attempt | URL | Response |
|---|---|---|
| Direct, HTTPS (WebFetch) | `https://www.freesteel.co.uk/wpblog/` | HTTP 403 |
| Direct, HTTPS (curl) | `https://www.freesteel.co.uk/wpblog/` | HTTP 503 |
| Direct, HTTP (curl) | `http://www.freesteel.co.uk/wpblog/` | HTTP 301 to the HTTPS address |
| Wayback availability API | `https://archive.org/wayback/available?url=freesteel.co.uk/wpblog` | `"archived_snapshots": {}` (a false negative) |
| Wayback CDX index | `https://web.archive.org/cdx/search/cdx?url=freesteel.co.uk/wpblog/*&filter=statuscode:200&collapse=urlkey&fl=original,timestamp` | 6710 rows, 888 distinct post slugs (2005–2019) |

The working method:

1. Query the CDX index (the last row of the table).
2. Take the timestamp of the post.
3. Fetch `https://web.archive.org/web/<timestamp>id_/http://www.freesteel.co.uk/wpblog/<yyyy/mm/dd/slug>/`.
   The `id_` suffix returns the raw archived page.

The archive throttles fast requests. Wait a few seconds between fetches.

The category `machining/adaptive-clearing` exists. Snapshots of its pages:
20091216041739 (page 1), 20141216002641 (page 2), 20211025142122 (page 3).
The category holds only some of the adaptive posts. The 2007 and 2013–2014
adaptive posts are found by slug.

### Posts read (19), with snapshot timestamps

All URLs have the form `https://web.archive.org/web/<snapshot>/http://www.freesteel.co.uk/wpblog/<date>/<slug>/`.

| Date | Slug | Snapshot | Relevant? |
|---|---|---|---|
| 2005-09-12 | adaptive-clearing-online | 20160712121020 | little (web demo) |
| 2006-01-11 | intelligent-machining-algorithms | 20140507043017 | general planning essay |
| 2006-06-06 | global-use-of-adaptive-clearance | 20160712203742 | yes (stock model, in comments) |
| 2006-11-16 | a-new-machining-strategy-is-born | 20160712125254 | little (Z-level rest idea) |
| 2007-01-15 | embarrassing-toolpath-ordering | 20160712123613 | **yes** |
| 2007-06-16 | demonstration-of-staging-links | 20160712090919 | **no — UN votes; the slug misleads** |
| 2008-05-16 | machining-the-competition | 20160712184600 | little (competitor survey) |
| 2009-04-14 | suggested-caption-this-new-machining-technique-… | 20140317090443 | little (Mastercam Dynamic) |
| 2009-12-07 | another-year-another-euromold | 20160712131117 | little |
| 2012-12-17 | another-day-another-revolutionary-new-roughing-strategy | 20160712062753 | yes |
| 2013-01-10 | the-toolpath-collision-point-with-an-engagement-sweep | 20160712073624 | **yes** |
| 2013-08-28 | delcams-18-months-of-patenting-secrecy-are-up | 20131109121119 | yes (contrast only) |
| 2013-11-20 | a-stay-down-shortest-linking-motion | 20140131093729 | **yes** |
| 2013-12-10 | homing-in-on-an-a-structure | 20140131092521 | yes |
| 2014-01-10 | a-minor-a-improvement | 20140131092852 | little (A* heuristic) |
| 2014-02-11 | rolling-a-star-linking | 20140318042841 | **yes** |
| 2014-06-26 | rolling-the-wrong-way-on-the-linking-motions | 20141216002620 | **yes** |
| 2014-07-31 | solve-a-problem-and-make-a-problem | 20141216072109 | **yes** |
| 2015-08-26 | change-you-can-engage-with | 20150904044337 | **no — politics** |

### Not read, possibly relevant

The lead can decide on these. Each is in the CDX index.

- 2005-09-27 `weave-subdivision` and 2005-10-10 `subdivision-in-weaves`. The
  "weave" is the area model that the engagement and the A* links run on.
  These posts can firm up the Q2 answer.
- 2005-11-23 `patent-schmatent` and 2005-12-14 `patent-part-deux`. These can
  describe the 2004 algorithm.
- 2014-01-24 `a-routing-on-a-flexibly-subdivided-weave`, 2014-05-12
  `2d-sideways-offset-using-weaves`, 2014-09-21 `shedloads-of-code`.

The blog never got the promised write-up of the pass reordering ("a really
simple trick", 2014-06-26). No later slug matches it.

## Answers

### Q1 — What did they try that failed?

1. **Start-point order by recency only.** The first plan picked "the most
   recently left potential starting point". At the start of a job the tool
   was "hopping around at random sniffing the shape out". It visited every
   corner first. Fix: go to the closest start point. They state that the
   best fix is a real link to each candidate, then the shortest.
   (2007-01-15, embarrassing-toolpath-ordering)
2. **Aim at a stock corner first.** On rectangular stock the code aimed for
   the corner as the best spiral-in start. When the part walls touch the
   stock, the corner is the "worst possible place": it lands in a
   disconnected stock fragment. Fix: aim at "the middle of one side" and
   follow the side to the stock. (2007-01-15)
3. **Reorder-safety test, first versions.** Version 1: bounding boxes apart
   by more than the sum of the two tool radii. It was good enough only as a
   demonstration. Version 2: use the measured engagement at each node. It
   failed because no sample rate guarantees that the engagement does not
   spike between nodes. Version 3: arcs on path Q as short line segments. It
   was rejected: "What tolerance do you use? There's no right answer." Final:
   assume the maximum engagement angle all along P (a controlled
   over-estimate), and solve the disc-against-arc contact analytically.
   (2013-01-10, the-toolpath-collision-point-with-an-engagement-sweep)
4. **Stay-down links were started to prove them slower.** Todd planned the
   stay-down path to show users "it's a lot slower than a straightforward
   no-nonsense retract motion". Users wanted it anyway. He notes "a
   difference between being the fastest toolpath and looking good".
   (2013-11-20, a-stay-down-shortest-linking-motion)
5. **A\* structure.** Four complete rewrites. Paths as point lists failed;
   paths as edge lists worked. A large case took 5 minutes until a priority
   queue replaced the search. (2013-12-10, homing-in-on-an-a-structure)
6. **A\* tile sides.** Two paths cannot claim the same tile side. The search
   then took a long way round. They accepted it because a straight line is
   still found when one fits. (2014-02-11, rolling-a-star-linking)
7. **Link direction guards.** The shortest link sometimes turned 180°
   back on itself. Fix 1: an exclusion circle at roll-off and roll-on. The
   link then went round the wrong side of the circle. Fix 2: a radial
   blocking line. The line itself became "a liability".
   (2014-07-31, solve-a-problem-and-make-a-problem)
8. **Schedule.** The stay-down link work ran "6 months beyond" the estimate
   of "a couple of months". (2014-06-26)
9. **Z-level in rest areas** (2006-11-16): an idea, not adaptive. A later
   pingback says new ideas may "void all that excitement".

### Q2 — How do passes grow? (PARTIAL)

No read post describes the pass-growth rule directly. The evidence:

- The adaptive code stores no stock model. "Given any point, I can quickly
  calculate whether it is inside or outside the remaining stock" (Todd, in
  the comments of 2006-06-06, global-use-of-adaptive-clearance).
- The code keeps a list of potential start points and tries "to cut
  forward from it". Seven or more start types exist: "on contour, from
  helix, from corner, towards contour, away from contour, double cut
  breakthrough, towards vanishing point on last level". (2007-01-15)
- The code measures the engagement "at every node of the path". It samples
  the path "at the widest rate possible specifically to not exceed this
  engagement angle". It has an optimum and a maximum engagement; a wider
  gap between them computes faster. (2013-01-10)
- The weave stores stock engagement per fibre (2014-02-11).
- The "initial clearing spiral" with a variable pitch is "on the right
  track" (2012-12-17, about a competitor). Inference: their own job opens
  with a spiral, then switches to forward cutting.

**Inference (labelled):** passes grow step by step forward from a start
point. Each step tests the engagement against the implicit remaining
stock. Passes do not come from a family of offsets of the boundary.

**Do not attribute to Freesteel:**

- The Delcam Vortex patent abstract (quoted in 2013-08-28) is an
  offset-family method. It makes offsets per corner region, joins them, and
  adds "backfill cutting paths". Todd reads it as a way to round off "the
  sharp corner collapses in an offset clearing toolpath". It is a contrast.
- The medial-axis pocketing in the comments of 2012-12-17 is Anders
  Wallin's openvoronoi work. He calls it "input-geometry driven, not a
  cutting-simulation".

### Q3 — How do they link passes, and what do they test?

- **Default link (to 2014):** "If it can't go nearly directly through the
  partially cleared pocket, it rolls off and does a retract to a roll-on
  to the next starting point." (2014-06-26)
- **Retract steps** existed "from the start in our original 2004" version
  (2012-12-17).
- **Stay-down link (2013–2014):** A\* search on the weave cells. The search
  "will always find a connection no matter how complicated" (2013-11-20).
  The code then pulls the path tight inside three limits (2014-02-11):
  1. the contour slice,
  2. the stock engagement along each weave fibre,
  3. extra exclusion circles at the key turns.
  Then it subdivides the path in each cell to the sample tolerance.
- **Tangent ends.** The first and last link segments must be tangent to the
  pass. A tighter corner is acceptable just after a sharp incoming corner,
  because the machine is already slow there (2013-12-10, a reader's
  suggestion that Todd accepts).
- **Test along the link:** the tool disc sweeps along the link. The code
  tests it against the material removed by earlier passes. A removed-material
  segment is "the red arc" plus two parallel lines. Five contact cases
  exist. A cutting path needs a yes/no. A link path is trimmed back to its
  safe part. A new link then "avoids all previously uncut stock".
  (2013-01-10)
- **Choice of next pass:** at the end of a pass, pick one of about "five
  next paths" by distance. Better: build a full link to each and pick the
  shortest. (2013-11-20)
- **Reordering breaks links:** after the reorder, the good links are
  shredded and must be rebuilt. This is why the stay-down linker was
  needed. (2014-06-26)
- **The "wrong side" link:** a link can pass on the "wrong" side of the
  last cut path. Todd shows that this is correct. The real defect: the
  cutting cycle skips almost-empty sections, and the linker sees the small
  residue as an obstacle. It then goes the wrong way round the roll-off arc.
  He refuses a tolerance and plans to "barge through" the start of the
  link. (2014-06-26)

**Note for the lead:** the reorder-safety test (Q before P is safe if Q's
swept disc misses P's removed material) and the relink after reorder match
rs_cam's G-LINKSTAGE and retract-count work.

### Q4 — Corners and the end of a region (THIN)

No read post describes the toolpath in a concave corner.

- "From corner" is one of the start-point types (2007-01-15).
- On rectangular stock, a corner start is wrong when the walls touch the
  stock. Start at the middle of a side instead (2007-01-15).
- "Towards vanishing point on last level" and "double cut breakthrough"
  are start types (2007-01-15). They suggest special cases for a region
  that closes and for two fronts that meet. The posts do not explain them.
- End of region: "the cutting cycle skips out sections when there is almost
  nothing left to cut" (2014-06-26). The small residue stays. No post
  describes a mop-up of it.

### Q5 — V1, V2, V3 (from PLAN.md)

No post names these defects. Each answer below is inference from the posts.

- **V1 (corner mop loops, each a plunge-cut-retract):** the Freesteel cycle
  does the opposite. It skips sections with almost no material
  (2014-06-26). It does not run separate mop loops. The residue then
  blocks the stay-down link. This is the same mechanism as V1 in rs_cam,
  where residue rejects the stay-down links.
- **V2 (full circles, half in air):** their default link is a roll-off arc,
  a retract and a roll-on (2014-06-26). The links go through the cleared
  area when they can. The posts do not discuss full circles in air.
- **V3 (does not grow into spaces):** the passes grow from engagement
  against the remaining stock, not from the level sets of the distance to
  the wall. So, by construction, they do not follow the walls (inference
  from 2006-06-06, 2007-01-15, 2013-01-10). The start rule "middle of one
  side, follow it till meeting the stock" (2007-01-15) also starts from the
  stock, not from the wall. This is the direct contrast to "The gap the
  June review left open" in PLAN.md: rs_cam's EDT wraps follow the walls
  because the field is distance to the boundary.
