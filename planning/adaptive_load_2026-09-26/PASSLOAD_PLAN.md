# G-ADAPTPASSLOAD — 2D Adaptive's load measure admits ~2x the commanded width (design, 2026-09-26)

Status: design only (read-only investigation at 9ac09bfa). **Awaiting two
operator decisions** (§5). The quadrature numbers below come from a scratch
calculation; per the repo rule they must become an asserting unit test before
they are cited as fact (sentry S2 does this).

## 1. The measures

- Target (ops/adaptive_shared.rs:10-14): f* = acos(1 − s/R) / 2π, an ANGLE
  fraction. s = 2, R = 3: f* = 0.19591; accept ceiling f*·1.05 = 0.20571
  (adaptive/search.rs:223, 228); in radial units
  `radial_woc_fraction_from_leading_arc` (:37) = 0.36264. Commanded s/D = 0.333.
- DiskArea (default; search.rs:25-64): material cells ÷ disc cells at the next
  position. With disc(cur) already cleared this is the crescent the step
  removes, ≈ (w/π)·fill for step L = R/2 (path.rs:261, 310). Steady side cut,
  R 3, L 1.5 (quadrature): s = 1 / 2 / 3 / 4 / 6 → 0.051 / 0.104 / 0.158 /
  0.211 / 0.315. The band [0.186, 0.206] is met at s ≈ 3.7-3.9 mm:
  **w ≈ 0.62-0.65, about 1.9x the commanded 0.33** (the quantitative form of
  finding F1, PROGRESS_HISTORY.md:1757). Thin material lets w rise further: a
  head-on bite of p = 1.5 mm reads 0.1955 (in band) at sideways spread 0.866.
- LeadingArc (search.rs:84-106): in-material share of the leading semicircle,
  x0.5. Exact on one-sided side cuts; head-on it admits w = sin 37° = 0.60; a
  sliver inside the new disc reads 0.
- Sim radial (dexel_stock/stamping.rs:1816-1824, 1392-1397): sideways extent of
  fresh material in the subsegment's midpoint disc ÷ D. A full slot reads
  ≤ 5.625/6 = 0.9375 on 0.5 mm cells.
- Physics (stored source: planning/UNIFIED_LOAD_MODEL_2026-06-18.md §4): h =
  fz·sinθ; mean tangential force ∝ ∫sinθ dθ = sideways extent / R; MRR =
  v_f·ap·extent. **The sim's w is the mean-force fraction of a slot and the
  normalised MRR.** Peak chip thickness saturates (sin 74° = 0.96 at w =
  0.36) and cannot define load. Define load as radial immersion w = sideways
  material extent ÷ D.

## 2. The 0.93 is real full-width contact

Its sources: slot-clearing lines (path.rs:320-356, default on,
operation_configs.rs:476; on the fixture, full slots at feed 1500 on both
levels); the out-of-band fallback `best_any` (search.rs:468, 508); gradient
mode (path.rs:612, no load read); contour-parallel residue loops (path.rs:1400);
the mop walk step (path.rs:1260); forced clear (path.rs:698) wipes 2R on the
planner grid with no motion, so planner stock runs ahead. Steady agent passes
sit at w ≈ 0.62-0.65 by design of the band itself. The trace separates slot
lines and entries (semantic_item_id, source_intent, move_index) but not agent
/ gradient / fallback / contour loop / mop (no Marker).

## 3. Options

- A. Default LeadingArc: partial; saved projects keep DiskArea (serialised);
  slots, mop, gradient, fallback and loops stay unchecked.
- **B. `SweptWidth` (recommended)**: the sim's radial model on the planner grid
  (w = max − min sideways coordinate of material cells in disc(next) ÷ D), one
  predicate `step_within_pass_load` with ceiling 0.3626 for agent accept
  (fallback becomes a refusal), gradient steps, contour loops (split over-cap
  spans), the mop walk (a capped search toward residue), and
  `feed_link_within_pass_load`. It applies to 2D whatever engagement_measure
  says; Adaptive3d keeps the historical rule (byte parity unmoved).
  Cycle time goes UP (up to ~x1.9 on bulk agent passes: the passes honour the
  commanded stepover).
- C. Hold load by feed (v_f · min(1, 0.3626/w)): smallest time cost but it does
  not stop ploughing as geometry; backstop only.

Plus trace markers `ResidueContour` / `ResidueMop` and gradient / fallback
counters (trace only).

## 4. Sentries

- S1 `a_clearing_cut_holds_the_pass_load_g_adaptpassload` (six-island
  fixture): every ClearingCut Linear/Arc sample ≤ 0.3626 + (0.5 + 0.5 +
  0.1)/6 = 0.546 (reuse the G-ADAPTLINKLOAD functions, moved to common).
  Red today (the mop walk: 1362 samples; slot lines ≈ 0.93; agent median
  predicted ≈ 0.62).
- S2 unit (search.rs, closed form): a head-on wall at p = 1.5 mm; DiskArea
  reads in band (documents the defect); no accepted step has SweptWidth >
  0.3626 + cell/D (red today: 0.866). Oracle: SweptWidth reads s/D ± cell/D
  for s ∈ {0.5, 1.5, 3, 6}.
- S3 (optional): per-kind peaks via the new markers.
- Guards: G-ADAPTLINKLOAD stays; adaptive3d byte parity and perf_golden
  unmoved.

## 5. Operator decisions

1. Approve B: 2D Adaptive passes hold the commanded stepover as real radial
   immersion; slower (up to ~1.9x on clearing), lower tool load.
2. Slot-clearing lines: off by default (Adaptive3d already passes false,
   clearing.rs:2843), or keep them as a declared full-slot load at the feed
   that holds the same mean force, 1500 x 0.3626 / 0.9375 ≈ 580 mm/min
   (limit ÷ the sim's slot reading).
