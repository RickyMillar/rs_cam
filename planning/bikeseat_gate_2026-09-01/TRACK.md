# Track D — bike-seat replication, gate stage only

**Rank:** 4 of 6 (`planning/finishing_status_2026-09-01.md` §5 avenue D).
**Status: CLOSED at the gate stage (2026-09-01). GATE 1 FAILED.**

Gate run (`crates/rs_cam_core/tests/bikeseat_gate_d1.rs`, analytic, ~4 s):

- Fixture: all requirements held (facets 0.82x the bar, relief 24.0 mm,
  `mean(s_max)/min(s_max)` 1.111, simply connected, up-facing).
- **Gate 1 FAIL:** prize ceiling +3.85 % vs the pre-registered 5 % bar
  (median W_max/W_min 1.0756; both anti-faceting rules clean).
- **Gate 2 PASS:** whole-sheet coherence 1.677 mm = 3.45 stepovers;
  regime w30 0.887 / 0.811, coverage 1.000.
- Negative control failed gate 2 (0.500 mm, w30 0.320), so the gate-2
  pass is meaningful.

The class is coherent but the prize is under the bar: a fixed-angle
raster already captures half of the pointwise anisotropy, and the
shipped per-region rotation (C2) eats into the rest. The full record,
the mechanism, and why no in-class construction can pass both gates is
`FINDINGS.md` §3. **No pathing, no field solve, no ×floor runs.**

## Scope of this stage

Build the missing fixture class and run the TWO EXISTING CENSUSES only.
No pathing, no field solve, until the gates pass:

1. Fixture: analytic swept-saddle / bike-seat-class sheet — genuinely
   varying curvature (`mean(s_max)/min(s_max)` well above 1), direction
   zones coherent over several stepovers, simply connected, facets ≤
   stepover/3. (Alternative: the paper's GrabCAD bike seat, if
   retrievable and licence-clean; the analytic sheet is the durable
   option.)
2. Gate 1: `wanaka_curvature_anisotropy`-style census → prize ceiling %.
3. Gate 2: `zone_coherence_census` → `w30 ≥ 0.70`, coherence length ≥
   several stepovers.

**Only if both gates pass** does the full F1 pipeline + ×floor + honest
raster comparison get scheduled (paper's Table 1 target: −13.0 % path on
the bike seat). If the analytic sheet cannot pass its own gates, that is
a finding: record it and stop.

## Evidence lands here

`planning/bikeseat_gate_2026-09-01/FINDINGS.md`.
