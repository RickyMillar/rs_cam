# Slice 1b — apply AFTER 1a is committed (same file)

## Edit 1 — after the `target_chipload` guard in `target()`

```rust
        // ── Checkpoint P (1b) — the same COMPARISON, not just the same band ──
        //
        // P-(1a) made the retargeter read the gate's band. This asks the
        // gate's own predicate whether the headroom target actually lands
        // inside it. `ChipBounds::contains` is the inclusive-with-epsilon
        // comparison the gate decided this very `Exceeds` with (Checkpoint K
        // b1), so retargeter and gate can no longer disagree about the edge.
        //
        // **Report-only, deliberately.** Turning this into a refusal or a
        // clamp would move a measured **86 of the 235** two-sided shipped LUT
        // rows (36.6 %; 48 of them single-point rows where max == min): on any
        // row narrower than the headroom factor both `min * 1.20 > max` and
        // `max / 1.20 < min`, so BOTH headroom targets fall outside the band —
        // the §2.5 defect class reached through the headroom policy instead of
        // the DOC derate. That population is far outside the movement this
        // wave is authorised for, so A-8i measures it, says it in the
        // rationale the operator reads, and hands the branch to a checkpoint.
        // Evidence: `planning/review_2026-08-08/artifacts/a8i/narrow_band_census.{py,txt}`.
        let headroom = match side {
            Side::Burn => self.low_headroom,
            Side::Breakage => self.high_headroom,
        };
        let target_in_band = bounds.contains(target_chipload);
```

## Edit 2 — F1 shortfall comparison

`if achieved_observed > 0.0 && achieved_observed < target_chipload` becomes

```rust
            // Checkpoint P (1b): the gate's low-side comparison, not a bare
            // `<`. NOTE, so this is not over-claimed: it is provably a no-op
            // on reachable inputs. `achieved / target == 5000 / raw_target`
            // exactly (both sides are `peak * clamped / baseline` over
            // `target`), so `achieved` sits within the 8-ulp boundary slack of
            // `target` only when `raw_target` is within ~9e-12 of the feed cap
            // — and `was_clamped` needs `|clamped - raw| > 1e-6` to be true at
            // all. The two epsilons cannot both bind. Routed anyway: "no gate
            // and nothing re-deciding a gate writes a bare bound comparison"
            // is the contract, and an unreachable exception is still an
            // exception.
            if achieved_observed > 0.0
                && crate::tool_load::boundary::below_low(
                    achieved_observed,
                    target_chipload,
                    0.0,
                )
```

## Edit 3 — rationale carries the out-of-band statement

```rust
        let mut rationale = format!(
            "{side:?}: scale feed by {multiplier:.2}× to move sample peak from \
             {peak:.4} to {target_chipload:.4} — the gate's own DOC-derated \
             band with headroom"
        );
        if !target_in_band {
            rationale.push_str(&format!(
                ". WARNING: {target_chipload:.4} is OUTSIDE that band \
                 [{min}, {max:.4}] — the band is narrower than the \
                 {headroom:.2}× headroom, so this candidate cannot reconcile \
                 no matter how the feed moves",
                min = bounds
                    .min_mm_per_tooth
                    .map_or_else(|| "none".to_owned(), |m| format!("{m:.4}")),
                max = bounds.max_mm_per_tooth,
            ));
        }
```

## Tests to add

* `a_target_outside_a_narrow_band_is_reported_not_silently_emitted` — band
  [0.09, 0.10] (ratio 1.111 < 1.20), breakage peak 0.20, headroom 1.20 →
  target 0.08333 which `contains` rejects. Assert the rationale carries
  "OUTSIDE" **and** the primary patch value is unchanged from the bare
  arithmetic (no number moved).
* `a_target_inside_the_band_says_nothing_extra` — the wide-band control.
