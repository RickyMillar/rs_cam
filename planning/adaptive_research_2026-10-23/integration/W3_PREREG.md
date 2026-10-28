# W3 pre-registration: G-WLAIRLABEL (the "air" label of a cut that removes stock)

Written before any code, on HEAD 3811e82d0 + W2a (uncommitted). For the lead's approval.

## The defect (D69 §3)

The trace calls a cutting sample "air" when its radial engagement is under 2 % of the diameter
(`stock/simulation_cut.rs:249 AIR_CUT_RADIAL_WOC_FRACTION`, through `SimulationCutSample::radial_engagement_class`,
the one partition). On the Corne Waterline, 279.3 s of such samples removed 4018 mm³ (51 % of the op): the flank cuts a
0.5 mm wall skin above the tip, and the radial channel reads 0. The trace says 91.3 % air; the true air is 14.3 % of
the cut (D69 `passes_base_corne.json`, `assert_d69.py`).

## The one existing definition to reuse

`stock/sim_measurability.rs:50` already names this population:

```text
removing = cutting samples with removed_volume_est_mm3 > 0
blind    = removing samples with engagement.radial_woc_fraction <= 0
```

W3 adds no threshold and no second form. It lifts that predicate onto the sample as one method,
`SimulationCutSample::removes_material()` (`is_cutting && removed_volume_est_mm3 > 0.0`), and
`sim_measurability` calls it. "Air that removes stock" = `RadialEngagementClass::Air` AND `removes_material()`.

## Two options

### Option B (recommended: the label only, small)

- The partition, `is_air`, the gates and `air_cut_time_s` do not change. No golden of an existing field moves.
- `SimulationToolpathCutSummary` gains `air_cut_removing_time_s` (serde default 0, skipped when 0 on write): the part
  of `air_cut_time_s` whose samples remove material. The accumulator fills it in the same `Air` arm.
- The narration air line (`trace/narrate.rs append_air_cut_anomaly`, about line 2162) reports the true air and names
  the rest in the same line, for example: "⚠ 91.3% of CUTTING time reads air-cut; 77.0 points of it remove stock at a
  radial engagement under 2% (a flank skin), so 14.3% removes nothing". The ⚠ bar reads the TRUE air share (air less
  removing) of the total runtime, so a Waterline that cuts a skin is not flagged.
- MCP: the toolpath summary (`rs_cam_mcp/src/response.rs`, viz `app/mcp/simulation.rs`) carries the new field next to
  `air_cut_time_s`. The GUI "air" figure is I4's; I tell I4 the field and do not edit viz UI.

### Option A (the partition)

- A new class `RadialEngagementClass::Unseen` (WOC under 2 %, removes material); `Air` then removes nothing.
  `air_cut_time_s` drops wherever a flank skin is cut (every Waterline, Steep and Shallow, Profile finish passes).
- Every shipped air-cut threshold (`Capabilities::air_cut_high_threshold_pct`, the GUI and CLI 40 %) was tuned on the
  old measure (`simulation_cut.rs` `AirCutRatios` doc), and the perf goldens, the air-cut calibration sentries
  (`air_cut_family_calibration_w5bf4`, `air_cut_denominators_lh1`) and the triage counts move. Not small.

## Predicted change and re-bless (Option B)

- No existing field or golden moves. `perf_golden_sim_metrics*.json` do not hold the new field (the test compares named
  fields). A trace written before W3 loads with the field at 0.
- The narration text changes only where `air_cut_removing_time_s > 0`. Search done before code:
  `rg -l 'of CUTTING time is air-cut' crates/*/tests crates/*/src` finds only `trace/narrate.rs` itself, so no
  narration golden holds the line. A narration test in `narrate.rs` that asserts the old line is re-checked, not
  re-blessed.

## Sentries (Option B)

1. A Waterline that cuts a wall skin (rest stock, a W1 fixture): `air_cut_removing_time_s > 0`, and
   `air_cut_time_s − air_cut_removing_time_s` equals the time of the Air samples with `removed_volume_est_mm3 == 0`.
2. A pass over cleared ground (removes nothing): `air_cut_removing_time_s == 0` (the `pocket-recut-air` case of
   `sim_measurability`).
3. `sim_measurability` counts the same removing samples as before (it reads the new method).
4. The narration line of case 1 names both shares and carries no ⚠ when the true air is under the bar.

## Questions for the lead

1. Option B (label only) or Option A (partition)?
2. With B, the ⚠ bar reads the true air share. Is that inside "the label", or must the bar stay on the old share?
