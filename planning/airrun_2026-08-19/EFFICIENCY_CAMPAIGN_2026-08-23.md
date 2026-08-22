# wanaka200 efficiency campaign — plan (2026-08-23, operator-commissioned)

Operator ask: per-toolpath efficiency sweep. Specific questions to answer:
(a) can Lakes and Rivers be cut "in one move"; (b) better linking / height
selection — "a lot of the moves in the height selector show all the levels
above the stock, which makes no sense"; (c) is there a way to pick optimal
rough stepdown/stepover — can the optimizer be used more; (d) tool sizes:
chuck takes up to 6 mm shank, drawer has Ø5 down to Ø0.5 — would a 3 mm ball
+ rest finishing beat the R1.5 everywhere, or is R1.5 already right?

## Where the seconds are (unified candidate, 17,697 s = 4.92 h)

| op | ≈ runtime | share | dominant cost (runtime_by_intent) |
|---|---|---|---|
| Unified finish (R1.5) | ~7,000 s | 40% | cutting (chipload-clamped) + 31% air |
| Back Rough (optimized) | ~4,400 s | 25% | cutting + 881 s plunge entries + 855 s air blob |
| Pencil (R0.5) | ~2,900 s | 16% | still entry-dominated (150 mm/min cap) |
| Front Rough | ~2,570 s | 15% | **54% air-cut** — worst ratio in the project |
| Rivers (V-bit) | ~700 s | 4% | only 158 s CUTTING; 209 s entry + 338 s rapids |
| Lakes + drills | ~180 s | 1% | negligible |

Campaign baseline = C0 re-measures this table exactly (per-op
runtime_by_intent from the cut trace) before anything moves.

## Standing rules (unchanged from the overnight)

Sim at 0.15 mm (0.1 OOMs this board). One variable per iteration, except
optimizer-recommended joint moves (E4/E5 taught us why both directions).
Quality bars: 0 collisions/rapids, 8/8 gates with real populations,
crosses-standing not worse, scallop math stated for any finish change,
surface RENDERED before accepting a candidate (G-UNIFIEDBOTTOMZ was caught
by a report + narration, not by aggregates). The full rest-chain ladder
after any upstream change. Operator's `wanaka200.toml` untouched; all work
on `wanaka200_fast_unified.toml` lineage. NEW first-class metric: **tool
changes** (manual on this machine — each costs the operator minutes and a
re-zero), reported per candidate.

## C0 — Baseline instrument pass (~20 min)

Load `wanaka200_fast_unified.toml`, generate_all @0.15, sim, pull per-op
`runtime_by_intent` + air% + entry counts. This table arbitrates everything
below and replaces the ≈ estimates above.

## C1 — The height-ladder audit (operator's own observation) (~30 min)

The operator saw the height selector offering Z levels ABOVE the stock.
Two known-real mechanisms make this plausible rather than cosmetic:
- G-UNIFIEDBOTTOMZ (yesterday): auto heights resolution picked bottom_z =
  stock TOP. The sibling failure — top_z resolving ABOVE the true stock
  top — would make every rough ladder start with pure-air levels.
- The front rough's worst air blobs sit at z 5.4–6.7 against a local stock
  top of 7.0 — consistent with air levels or near-air first passes.
Method: for each rough + finish, narrate the Z ladder and compare the top
ladder levels against the emission-frame stock top; per-level cutting-vs-air
from the narration. If air levels exist, pin top_z per op and re-measure.
Also answers the operator's UI confusion with a concrete mechanism either
way. Expected: 0–800 s + possibly a G-finding on auto top_z.

## C2 — Front rough: kill the 54% air (~45 min)

In order (stop when the ratio is sane):
1. `optimize_toolpath` on it — never yet run on this op; sim-verified
   feed/RPM/DOC candidates for free (the back rough gave −39% this way).
2. C1's top_z pin if the audit implicates air levels.
3. `mill_shallow_areas` / `detect_flat_areas` A/B — also reduces what the
   finish crosses (the 3.2 mm standing bites), a quality co-benefit.
4. Boundary tighten (model_silhouette+offset) only if 1–3 leave air >35%.

## C3 — The tool-size question: finishing cascade study (~2 h, the big one)

Scallop math at the 30 µm bar (s = sqrt(8·R·h)):
| ball | stepover for 30 µm | path vs R1.5 |
|---|---|---|
| R1.5 (current) | 0.60 mm | 1.00× |
| R2.0 (already a project tool) | 0.69 mm | 0.87× |
| R2.5 (Ø5 from drawer, needs tool entry) | 0.77 mm | 0.78× |

But path length is only half the story: (i) a bigger ball can't reach into
river curvature < its radius → the REST left for the pencil grows (E2
taught us the pencil taxes every micron the finish leaves); (ii) the vendor
row match can change the clamped feed (that's why unified beat drop_cutter);
(iii) a taper's reach in deep narrow valleys differs from a plain ball.
Experiments, each full-chain simmed:
- C3a: unified with R2.0 tapered @ 0.69 (tool already in project; ZERO new
  tool changes) — measure finish − pencil-growth net.
- C3b: unified with Ø5 ball @ 0.77 + set pencil reference_tool to match —
  one added tool change; only run if C3a nets positive.
- C3c (operator's "3mm + rest finishing"): the current R1.5 IS Ø3 at the
  tip — the answer to "would a 3 mm ball + rest finishing help" may be
  "that is what we have, minus the explicit rest stage". The cascade
  variant worth testing: coarse unified R2.0 → **rest-targeted** R1.0/R1.5
  pass (pencil with num_offset_passes raised, or a scallop/rest op) →
  pencil R0.5. Only if C3a shows the big-ball rest is pencil-affordable.
Verdict criterion: total(finish+pencil) + tool-change count, gates green,
crosses-standing not worse.

## C4 — Rivers + Lakes: "one move?" and the linking disease (~40 min)

They cannot be ONE op — different cutters (20° V-bit vs R1.0 tapered). But
the real cost isn't two ops: Rivers spends 158 s cutting inside 700 s of
runtime — the pencil disease again (entry + rapids around sparse curves).
Levers: project_curve link/hookup distance, retract_strategy, and
`optimize_rapid_order` (already on — verify it actually fires here);
re-order ops within the setup so same-tool ops are adjacent (tool-change
count). Target: Rivers+Lakes ≤ 450 s and no added tool changes.

## C5 — Back rough residuals (~30 min)

881 s of plunge entries (entry_style=plunge @ 541 mm/min) + 855 s air blob.
A/B entry_style helix (pitch 2.0) vs plunge; if the air blob at
(127, 46) survives C1, chase its mechanism (it's one advisory, 15% of the
op). The optimizer already set feed/DOC/RPM — don't re-litigate those.

## C6 — Pencil second pass (~20 min)

hookup_distance 15→25 A/B (diminishing, but entries still dominate its
~2,900 s). If C3 changes the finish tool, reference_tool_diameter MUST
follow it (currently Ø3 = R1.5 ✓) — that's a correctness item, not a tune.

## C7 — Optimizer sweep of everything eligible (~45 min)

`optimize_toolpath` on: front rough (C2), unified finish, rivers, lakes.
Each returns sim-verified candidates or an honest NoSafeImprovement. The
back-rough result says the Stage-1/2 DOC sims find joint moves that
one-variable probing provably misses.

## C8 — Consolidate (~40 min)

Winner config → full-chain regen + sim @0.15 → gates + triage + composite
render + side-by-side vs current → save `wanaka200_fast2.toml` → export +
byte-verify → scoreboard, commit, push. Every negative result logged with
its mechanism (E4 has already paid for itself once).

## Expected landing zone

Honest guess: 17,697 s → 14,500–15,500 s (4.0–4.3 h), dominated by C2+C3.
The stretch case (~3.8 h) needs C3b to net positive AND C1 to find real air
levels. Anything past that is fighting the chipload band, which is physics
(or at least vendor-published physics) — the remaining lever there would be
a Ø6 ball's bigger band, which is C3b's data point.

## Execution mode

Sequential iterations, ~15–25 min each, ladder-disciplined; memory watchers
re-armed first (console restart killed them). Estimated wall: 5–7 h of GUI
lane time. Findings filed as they surface; scoreboard updated per iteration.
