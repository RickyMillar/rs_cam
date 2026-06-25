# Multi-tool finishing — roadmap (A/B committed, C/D on paper)

**Goal:** finish freeform reliefs (wanaka-class) faster without losing detail, by
not machining the whole surface with one tiny tool. Research basis + citations:
`research/multitool_finishing_optimization.md`. Verified core findings: rest
machining / tool-set selection is solved for 2.5D (Veeramani Voronoi-mountain) and
3D roughing (Sarma; Lin & Gian — both reject "largest tool that fits"); constant-
scallop beats raster ~7–21% (Lin & Koren 1996; Feng & Li 2002); Elber classifies
convex/concave/saddle regions → tool-type per region. **Open gap = reach-radius
spectrum → finishing tool-kit, scored by accel-aware cycle time (rs_cam holds all
three ingredients).**

**Hard machine constraint (drives everything):** target is a Shapeoko (low accel
~500 mm/s², **MANUAL tool changes** — each added tool = minutes of human time +
re-zero). So linking overhead AND tool-change cost are both heavily weighted →
the sweet spot is likely **2 tools (bulk + detail)**, not 4–5. The whole point of
B is to measure whether even 2 wins here.

**Engine primitives already shipped** (FEATURE_CATALOG): steep/shallow
(`steep_shallow.rs`), scallop/constant-cusp (`scallop.rs`), pencil (`pencil.rs`),
rest (`rest.rs`), IPW "use remaining stock" (pre-sims prior ops), and an
**acceleration-aware cycle-time integrator** (strategy advisor). A/B need NO new code.

---

## wanaka200 reference state (for fast pickup)

- Project: `/home/ricky/Downloads/wanaka200/wanaka200.toml`, plywood (Baltic Birch),
  roughs on contour_parallel, feeds tuned (18k/3000 on 6mm EM). 0 collisions, OK.
- Front setup (Setup 2): `3D Rough 6` (id 10, 6mm EM) → **`3D Finish 6` (id 11,
  drop_cutter, tool 2 = 1mm tapered ball, stepover 0.3, F4000, 19k rpm)** — the
  finish is the dominant cost: **447,515 moves, ~150 m cutting, the bulk of the
  ~5.5 hr job.** THIS is what A/B target.
- Tool library candidates: tool 12 = 6mm ball nose; tool 13 = 3mm ball; tool 8 =
  R1.5 tapered ball (3mm); tool 2 = 1mm tapered ball; tool 7 = 0.5mm tapered ball.

---

## PATH A — Committed: reconfigure wanaka200 finish as bulk + rest (no new code)

**Do:** replace the single 1mm finish with a tool pair on the front surface, both
`stock_source = from_remaining_stock` so each only cuts what's left:
1. **Bulk finish:** 6mm ball nose (tool 12), `scallop` (constant-cusp) over the
   front surface. (Steep/shallow variant tested in B.)
2. **Detail/rest finish:** 1mm tapered ball (tool 2), restricted to leftovers —
   try BOTH (a) existing drop_cutter/scallop with `from_remaining_stock`, and
   (b) `pencil` (valley-seam) — B decides which.

**Open risk to confirm in A:** does the small finish op, with `from_remaining_stock`,
actually RESTRICT its cutting area to the rest region — or does it still raster the
whole surface (just at the already-cut height)? If it doesn't self-restrict, use
`pencil` for the detail tool and/or a boundary. This is the make-or-break mechanic.

**Acceptance:** generates clean (0 errors), 0 collisions, detail in the valleys
preserved (visually + scallop check), bulk surface scallop acceptable for plywood.

## PATH B — Committed: measure the variants on the sim (baseline + decide)

Sim each, capture: `total_runtime_s` (accel-aware), per-op move_count /
cutting_distance / rapid_distance, air_cut %, load gates (chipload/deflection/
power), collisions. **One variable at a time.**

| # | Variant | Isolates |
|---|---------|----------|
| 0 | **Baseline** — current single 1mm drop_cutter (id 11 as-is) | reference |
| 1 | 1mm **constant-scallop** (swap drop_cutter→scallop, same tool) | path strategy alone (free, no tool change) |
| 2 | 6mm-ball scallop (bulk) + 1mm rest (`from_remaining_stock`) | multi-tool rest win |
| 3 | 6mm-ball **steep/shallow** + 1mm **pencil** valleys | hybrid + valley-only detail |

**Decision gates:**
- If **#1 < #0** materially → constant-scallop > raster holds on a Shapeoko: adopt
  it as a free default regardless of anything else.
- If **#2/#3 < #0** by more than the manual tool-change cost (~few min) → multi-tool
  rest wins HERE → C/D are worth building.
- If **#2/#3 ≥ #0** (linking + tool-change eat the gain) → **multi-tool finishing
  does NOT pay on low-accel manual-change machines**; stop pursuing D, keep #1.
  (This negative result is itself the key deliverable.)

**Gain from A/B:** immediate speed-up on wanaka200 if any variant wins; and the
empirical go/no-go for C/D — answers the user's core worry ("do the linking moves
make it worse?") with numbers on the real machine, not literature on ATC mills.

---

## PATH C — On paper: "reach-radius map" diagnostic (first novel ingredient)

**What:** compute, per surface point, the **largest ball radius that can finish it
without gouging** (≈ local concave radius of curvature / inscribed-sphere fit in
the valley), then surface the **area-vs-tool-radius histogram** + a colored GUI
overlay. NOT the optimizer — just the analysis. This is the user's original
instinct ("ratio of areas within a given radius") made literal.

**Build sketch (reuses existing machinery):**
- For each candidate radius r, run the existing drop-cutter/gouge logic to find the
  area an r-ball can TOUCH the design surface (vs bridge over a valley). Reachable
  area(r) is monotone ↑ in 1/r.
- Histogram: area added per radius band → "72% reachable by 6mm, 23% needs ≤3mm,
  5% needs ≤1mm." The complement at each r = the rest region for smaller tools.
- Output: histogram + heat-map overlay; expose over MCP for agent reasoning.

**Gain:** turns tool selection from guess → data, BEFORE cutting. Tells you which
tools are worth loading and how much each does. Useful standalone; de-risks D.
**Effort:** moderate (reuses dexel/drop-cutter/gouge). **Risk:** low.

## PATH D — On paper: full reach-radius → tool-set → accel-aware optimizer

**Formulation:** given the reach-radius field (C) and the tool library, choose an
ordered tool subset T1>T2>…>Tk and per-tool regions, where Ti's region = (area Ti
can reach) − (area larger tools already did) [IPW cascade], minimizing:

    total = Σ_i finish_time(Ti, region_i)            ← accel-aware integrator
          + (k−1) · tool_change_cost                 ← MANUAL change = large on Shapeoko
          + linking/retract cost                     ← accel-aware

**Algorithm:**
1. Reach-radius field (Path C).
2. Per candidate tool: reachable region + accel-aware finish-time estimate
   (region area ÷ strip-width ÷ effective feed, with cornering/junction limits).
3. Tool count is small (library ~5–10 finish tools) → near-enumerate subsets or
   greedy/DP; score each by the total above.
4. Emit the winning kit as a sequence of existing finish ops, each
   `from_remaining_stock` + region boundary.

**Why it's novel (verified):** the three ingredients — medial/reach-radius spectrum
(Veeramani, 2.5D), curvature-region tool selection (Elber), and accel-aware
cycle-time models (Altintas/Erkorkmaz, mature) — exist but have **never been joined
for 3D finishing with an accel-aware objective.** Defensible contribution / paper.

**Gain:** automatic, machine-aware multi-tool finishing — every relief auto-gets the
fastest tool combo for THIS machine's kinematics + manual-change cost. **Effort:**
significant (feature + optimization layer). **Risk:** higher. **Depends on:** C (the
field), B (proof economics hold on low-accel), and per-op region-boundary handoff.

**Key open design questions for D:**
- Region-boundary handoff fidelity: can ops be cleanly restricted to an assigned
  patch via boundary config + IPW, without seams/double-cut at the patch edges?
- Estimator-vs-sim agreement: the optimizer's accel-aware time estimate must track
  the real sim or the chosen kit is wrong.
- Tool-change cost calibration (manual Shapeoko): likely dominates → biases toward
  k=2. Make it a user param.

---

## RESULTS — A/B run 2026-06-24 (wanaka200, front subset: front rough idx6 + finish)

Method: back setup + drills disabled (independent of front finish) so each variant
sims fast and compares apples-to-apples. `total_runtime_s` is accel-aware. Finish
tool = 1mm tapered ball (id 6) unless noted. Front-subset baseline below; whole-job
baseline (#0, all ops on) = **29,732 s ≈ 8.26 hr** at 0.3 stepover.

| # | Variant | runtime_s (front subset) | vs #0 | finish moves | finish cut (m) | collisions | verdict |
|---|---------|--------------------------|-------|--------------|----------------|------------|---------|
| 0 | 1mm drop_cutter @ 0.3 stepover (real default) | **20,509** | — | 447,515 | 146 | 0 | OK |
| 1 | 1mm **continuous constant-scallop**, cusp 0.0225 (= #0 worst-case) | **6,029** | **−70.6% (3.40×)** | 70,993 | 56 | 0 | OK |

**#1 = clean adopt (gate #1 triggered).** Same tool, same worst-case cusp, NO tool
change. Coverage + detail visually confirmed (rivers/terrain all present, no uncut
bands). The win is far bigger than the literature's 7–21% (Lin & Koren) because
drop_cutter's fixed-XY-grid raster *oversamples along every line* — pathological on a
low-accel machine where each extra move costs an accel/decel cycle.

**Linking gotchas found (important, reusable):**
- Scallop with `continuous=false` retracts between offset rings → 29 per-ring lead-in
  rapid-collisions on the tall relief. Setting `continuous=true` stitches one spiral:
  rapid distance 1665 → 245 mm, **collisions → 0**, and runtime dropped further.
- `set_toolpath_heights` clearance plane is in the **setup-local emission frame**: for
  this Top setup the stock top is **Z=+27** (relief cuts down to ~Z+17), NOT Z=0.
  Setting clearance below the part (Z=5) plows every rapid through it (29→531). Keep
  clearance above the stock top (Z≥28). [trap worth remembering for future ops]

**MAKE-OR-BREAK RESOLVED (the load-bearing question): `from_remaining_stock` does
NOT restrict a scallop's cut area.** A 1mm scallop with `stock_source=from_remaining_
stock`, generated AFTER a 6mm bulk scallop, produced **70,993 moves / 56.3 m —
byte-identical** to the same 1mm scallop on fresh stock (fresh toolpath id, not a
cache hit). So stock_source affects sim/engagement metrics, not the generated path
geometry — scallop always rasters the whole surface. ⇒ **Variant #2 (6mm bulk + 1mm
*scallop* rest) is strictly worse than #1**: you cut the entire 1mm scallop anyway,
plus a redundant 6mm pass, plus a manual tool change. Dead.

⇒ The only way multi-tool finishing can win here is a detail tool that is
**intrinsically region-restricted by geometry, not by stock_source** — i.e. `pencil`
(concave valley seams only). That is variant #3, and it is now the sole multi-tool
contender. (If a boundary-restricted scallop is ever wanted, it needs an explicit
machining boundary via `set_boundary_config`, not stock_source — untested.)

**Variant #3 (6mm bulk scallop + 1mm pencil valleys) — pencil is unusable on organic
relief.** 6mm bulk scallop (cusp 0.05, continuous) = 25,223 moves / 28.3 m, clean.
Then 1mm pencil for the valleys:
- `bitangency_angle=160` (default): **366,672 moves / 1,323 m cutting / 335 m rapid** —
  catches a "seam" on every micro-ripple of the textured mesh and hooks up thousands
  of disconnected fragments. Catastrophic on a low-accel machine.
- `bitangency_angle=120, min_cut_length=10`: **85 moves / 0.17 m** — catches almost
  nothing. Bistable. No stable middle exists because an organic rivermap has a
  *continuous spectrum of concavity*, not discrete sharp concave fillets (the shape
  pencil was designed for). ⇒ pencil cannot express the rest region on this surface.

## A/B VERDICT (2026-06-24)

- **#1 single-tool continuous constant-scallop: clean 3.40× win — ADOPTED.** Gate #1
  fired. Same tool, same worst-case cusp, no tool change, 0 collisions, full coverage.
  This is the deliverable speed-up.
- **#2 / #3 multi-tool rest with OFF-THE-SHELF ops: do NOT pay on organic relief.**
  Not because of tool-change/linking economics (the plan's predicted failure mode) but
  for a deeper reason: **today's ops can't express the rest region on a continuous-
  concavity surface.** `from_remaining_stock` doesn't restrict scallop; `pencil` is
  bistable (all-or-nothing). The 6mm bulk genuinely is fast and clean — the gap is
  purely *"what region does the small tool cut?"*, which neither mechanism answers.
- **This SHARPENS the case for Path C** (reach-radius map → an explicit machining
  boundary for the detail tool) rather than killing it: the 2-tool idea is sound, but
  it needs a reach-radius-derived boundary fed to a *boundary-restricted scallop*
  (via `set_boundary_config`), NOT pencil and NOT stock_source. C is now a *prerequisite*
  for any multi-tool win here, not an optional enhancement. D still waits on a positive
  C result. (Caveat: the test surface is one organic relief; pencil-rest may still pay
  on mechanical parts with discrete fillets — not this product line.)

## Sequencing / decision

1. **NOW: A then B** — cheap, speeds up wanaka200 if any variant wins, and produces
   the go/no-go data for C/D. Variant #1 (constant-scallop vs raster) is the free
   control that separates "better path" from "more tools."
2. **If B is positive →** build C (useful alone), then D.
3. **If B is negative →** adopt #1 if it helped, shelve D, record the negative
   result (low-accel + manual-change kills multi-tool finishing) in the research log.

Do NOT build D speculatively — let B's measurement earn it.
