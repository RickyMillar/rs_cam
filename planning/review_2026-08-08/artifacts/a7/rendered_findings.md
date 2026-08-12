# A-7 rendered evidence — the typed findings and the clamped verdict, as text

Captured 2026-08-13 through `diagnostics::adapters` — the CORE renderer the CLI
report and MCP `get_diagnostics` both consume. The GUI's own strings are in
`properties/mod.rs` and `feeds_modal.rs`; a live GUI capture is recorded as NOT
EXERCISED in the wave entry (the operator's session runs an `e66962a`-era binary
that does not contain this code).

## K-(c2)/(d2) — clamped vs exceeded, side by side

```text
=== ON the band ceiling (rubbing-floor clamp put it there) ===
[Info] DiagnosticId("load.chipload.within"): Observed feed-per-tooth 0.0115 mm is ON the 0.0115 mm/tooth band ceiling — CLAMPED there by the engine's own rubbing-floor rule, not exceeded. The whole derated band sits below the chip-formation floor, so no feed clears rubbing without leaving the band; expect burnishing [median of steady-state samples, mm of linear advance per tooth; commanded 0.0115 mm/tooth advance — clamped to the vendor band ceiling 0.0115 mm/tooth — the whole derated band sits below the 0.0250 mm/tooth chip-formation floor, so the recipe rests ON the breakage bound, ×1.0000 achieved/commanded feed] (row amana-tapered-hardwood-scallop-3175-2f; scaled ×0.4802 diameter × ×1.0000 hardness from Ø3.175 mm; pass role SemiFinish substituted for the requested Finish)
[Info] DiagnosticId("load.power.within"): Power: model deferred (machine profile not provided to evaluator)
[Info] DiagnosticId("load.deflection.within"): Tip deflection within limit (4 µm / 200 µm)

=== 5 % over the same ceiling (genuine exceedance) ===
[Caution] DiagnosticId("load.chipload.high"): Feed-per-tooth too high — breakage risk: 0.0121 mm [peak steady-state sample, mm of linear advance per tooth; commanded 0.0121 mm/tooth advance, ×1.0000 achieved/commanded feed] (row amana-tapered-hardwood-scallop-3175-2f; scaled ×0.4802 diameter × ×1.0000 hardness from Ø3.175 mm; pass role SemiFinish substituted for the requested Finish)
[Info] DiagnosticId("load.chipload.commanded_above_band"): Commanded feed-per-tooth 0.0121 mm/tooth is 1.1× the matched band maximum 0.0115 mm/tooth — same unit, same stage, so this comparison needs no conversion (row amana-tapered-hardwood-scallop-3175-2f; scaled ×0.4802 diameter × ×1.0000 hardness from Ø3.175 mm; pass role SemiFinish substituted for the requested Finish)
[Info] DiagnosticId("load.power.within"): Power: model deferred (machine profile not provided to evaluator)
[Info] DiagnosticId("load.deflection.within"): Tip deflection within limit (4 µm / 200 µm)
```

## K-(c2)/(d2) — the same case as the fixture reports it

```text
c2: observed 0.01152537835462983 vs ceiling 0.01152537835462983 → Within + ceiling_advisory (burn_advisory false)
d2: commanded stage clamped_to = clamped to the vendor band ceiling 0.0115 mm/tooth — the whole derated band sits below the 0.0250 mm/tooth chip-formation floor, so the recipe rests ON the breakage bound
```

## K-(a3) — the RPM-anchor disclosure

```text
=== a3 disclosure, as the operator reads it ===
[Info] DiagnosticId("feeds.vendor_row_publishes_no_chipload"): Vendor row whiteside-1540-vgroove-60deg-quarter-rpm is an RPM anchor and publishes no chipload column — the recommended 0.0168 mm/tooth is the empirical formula's, not this vendor's, and this recommendation carries no band. The post-simulation gate resolves a different, chipload-bearing row, so its verdict is judged against bounds this recipe never saw.
[Caution] DiagnosticId("feeds.chipload_clamped_to_floor"): Chipload clamped to rubbing floor: 0.0126 → 0.0250 mm/tooth (post-derate chipload below chip-formation threshold; expect honest output above floor instead of ploughing recipe)
```

## K-(a4) — the reroute census, re-measured

```text
**0 of 3024 (query, reroute) pairs resolve to DIFFERENT rows.** Suggest ÷ gate band maximum: min ×NaN, median ×NaN, max ×NaN.
**756 pairs are REFUSALS** (ProjectCurve on bull-nose / V-bit: `lut_query_for` returns `None`). Post-a4 BOTH sides refuse — the gate as `Unmodeled(NoVendorData)`, Suggest as `FeedsWarning::NoVendorRowsForRoutedOperation`.
Through `feeds::calculate` on the refused surface: **12 of 12 recommendations were vendor-banded pre-a4 (routing disabled), 0 are post-a4.** Pre-a4 the full census counted 378 such pairs: an operator got a vendor-backed number on a surface where the gate had declined to judge at all.
```
