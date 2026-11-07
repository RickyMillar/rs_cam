# Next session: feeds and speeds correctness (brief, 2026-11-08)

HEAD at the time of writing: `048516b85`. P0-P19b are committed. `STATUS.md` row 11 has the phase list.
The operator wants the next session to make every feeds and speeds suggestion correct. GUI work waits.

## Start here

1. Read `STATUS.md` (row 11, item 4c0), then `phases/P18.md` "Follow-ups" and `phases/P19.md`.
2. Read `planning/feeds_review_2026-11-06/RECHECK_bd72997c9.md`: the status of D1-D17 at bd72997c9.
3. Protocol: the lead decides and verifies; agents implement and never commit. Every cargo command
   goes through `scripts/cargo_lane.sh` in `timeout`. Gate per phase: clippy, G-OPGOLDEN (46 known
   rows), the viz suite (9 known red binaries), fmt (3 known files). FM1 re-blesses only after the
   predicted cells are listed; a rise is a stop.

## 1. Plunge, ramp and drill audit (first)

The operator's question (2026-11-07), project `planning/rivmap_block_job/nz_wanaka_21_operator.toml`,
toolpath "Drill light holes 6 mm end mill" (index 4), white oak, 6 mm 2F end mill, peck 6 mm, hole 8 mm:

| | Plunge mm/min | RPM | Axial chip per tooth |
|---|---|---|---|
| Operator setting | 1000 | 18000 | 0.028 mm |
| Suggest (MCP `get_suggest_rationale` index 4) | 1778 | 14000 | 0.0635 mm |
| Amana Spektra "Ramp Down", 6 mm 2F wood, at the chart RPM | 2286 | 18000 | 0.0635 mm |

- Suggest lowers the RPM to 14000 only by the drill RPM cap
  `feeds.drill_rpm_envelope_for_diameter.small_ceiling_rpm`, an F201 Unsourced rule. Find a source
  or remove the cap.
- Audit every plunge, ramp and drill producer (`feeds/plunge.rs`, `feeds/ramp.rs`,
  `extrapolation/drill.rs`, `DRILL_RULES`, `PLUNGE_RULES`) against its printed source and the
  machine Z limits (`max_rate_xyz_mm_min[2]`, Z acceleration). Use one table: cell, shipped value,
  source line, verdict.
- P18 D14 already made plunge and ramp read the feed before the radial lift (`UnliftedFeed`).

## 2. Open correctness items

- **4c (P18):** the chip gate and the depth gate must abstain ("not graded", with the reason and the
  fix) when `sim_measurability` marks the metric NotMeasurable (Checkpoint D Q2). Today the chip gate
  can say "within" on blind moves. Sentry: `P_thin_005` at 0.5 mm abstains; at 0.1 mm it grades.
- **3b (P18):** `FeedsResult` power becomes an Option; "not modelled", never 0.0
  (`diagnostics/tests.rs` literals at :668 and :1018).
- **FM1 `plunge_binding` column** (`feeds_matrix_instrument_fm1.rs` ~:400) still calls the raw
  `resolve_plunge` at the lifted feed. Switch it to `resolve_plunge_at` and re-bless.
- **HighAirCut headline** cannot name the withheld toolpaths (a new field on `ProjectVerdict`).
- **BandMid** has two reach-cap omissions (P14 says delete BandMid).

## 3. Review items still open (RECHECK)

- D2 first half BLOCKED: at 1/8 in hardwood, `amana-flat-hardwood-contour-3175-2f` prints 0.0227 mm
  and `onsrud-hardwood-60-100mw-1_8-finish` prints 0.2794 mm (12x, both compression). Read both
  source lines before any score rule changes.
- D3 and D6 partial: the deep pocket ring still exceeds the depth gate; the R27 0.5 % width margin
  lets it pass.
- D4 (a species factor beyond density) and D13 (thinning on a light finish pass) need sources.
- D5: plywood has no force line.

## 4. GUI (later, low priority)

P17b open items (`phases/P17.md` "P17b open items"): over-budget dots cover the heat cells; the
240 pt Passes labels drop or stack; contour labels crowd the 100 % column; data at 100 % meets the
frame; the playhead ring is dark. The explainer page (`explainer_p13c/`) has stale claims to refresh.
