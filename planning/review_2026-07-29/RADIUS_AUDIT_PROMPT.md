# Independent audit: `radius()` vs `cusp_radius()` in the finishing path

Repo `rs_cam`, branch `experiment/adaptive-spiral`. Relevant commits:
`5732f57` (planner dials), `32c5e48` (classification cell size). Full
narrative in `planning/unified_v3_design.md` §14q and §14r.

**You are auditing conclusions reached by an agent with a poor track record
on this specific subsystem. Disagreeing is the useful outcome, not the
awkward one.** See "calibration" at the end before you start.

---

## The defect class

`TaperedBallEndmill::diameter()` returns the **shaft** diameter:

```rust
fn diameter(&self) -> f64 {
    self.shaft_diameter // effective cutting diameter at widest point
}
```

So `radius()` is **3.0 mm** for a Ø1 tip on a 6 mm shank — 6× the 0.5 mm
tip. Anything deriving a *feature scale* from `radius()` is therefore 6×
too large in length and 36× in area, on tapered tools only.

`MillingCutter::cusp_radius()` was added beside `radius()`
(`crates/rs_cam_core/src/tool/mod.rs`): tip sphere via `geometry_hint()`,
falling through to `radius()` for everything else.

## The rule being audited

Stated in `scallop.rs` and not previously applied elsewhere:

> **Physical extent** — padding, grid coverage, collision, clearance,
> swept volume — keeps `radius()`.
> **Feature scale** — cusp height, cell size, minimum region area,
> morphological close radius, claim floor — uses `cusp_radius()`.

**Q1. Is this rule correct and complete?** If a case doesn't fall cleanly
on one side, that case is the finding.

## What was changed

| site | was | now |
|---|---|---|
| `FinishPlannerParams::for_tool` → `min_region_area_mm2` | `(2·radius())²·4` = 144 mm² | `(2·cusp)²·4` = 4 mm² |
| `for_tool` → `close_radius_mm` | `radius()/2` = 1.5 mm | `cusp/2` = 0.25 mm |
| `for_tool` → `pencil_claim_floor` | `radius()·0.25` | `cusp·0.25` |
| `finish_setup.rs:130` classification `cell_size` | `radius()/4` = 0.75 mm | `cusp/4` = 0.125 mm |
| `execute.rs` claim-floor override | passed `radius()·0.25` — a **no-op that read as a fix**, with a comment claiming it used "the REAL tip radius" | deleted |
| `scallop.rs::cusp_radius` free fn | private duplicate | consolidated onto the trait method |

Call sites switched: 1 in `compute/execute.rs`, 5 in `unified_finish.rs`.
Padding in `finish_setup.rs` (`origin_x = bbox.min.x - tool_radius`)
deliberately still uses `radius()`.

**Q2. Are there other `radius()` uses in the finishing path that violate
the rule?** Three were found and fixed; there is no confidence that is all
of them. Suggested sweep:
`rg -n '\.radius\(\)' crates/rs_cam_core/src` and classify each against
the rule. Fast, because the criterion is mechanical.

## The one deliberately NOT changed

`finish_setup.rs:95`, `build_finish_surface_with_cancel` — the
**generation** surface — still uses `(cutter.radius()/4.0).max(tolerance)`.

This is P2.f's "smooshed band" fidelity defect under its original name
("scallop/waterline resample at shank-r/4 cells"), which was worked around
via scallop adaptive chord refinement rather than fixed at source. Changing
it moves every tapered-tool finish toolpath.

**Q3. Should it change, and what evidence should decide?** This is the
largest open call and it was explicitly left for you.

## Evidence to check or overturn

Measured on wanaka (`planning/airrun_2026-06-01/wanaka.toml` — **read-only,
never save over it**; it is user-modified).

Reproduce:
```
cargo test -p rs_cam_core --release --test finish_planner_wanaka_decompose \
  -- --ignored --nocapture wanaka_band_mix_vs_cusp_radius
```

| cusp_r | grid | sample_s | Shallow n/mm² | MidSteep n/mm² | VerySteep n/mm² |
|---|---|---|---|---|---|
| 3.00 (before) | 143² | 0.7 | 2 / 5585 | 1 / 4215 | **0 / 0** |
| 1.00 | 425² | 5.2 | 8 / 5448 | 3 / 4215 | 2 / 286 |
| **0.50** (Ø1 tip) | 849² | **19.2** | 19 / 5725 | 4 / 3937 | **10 / 313** |
| 0.25 | 1697² | 77.0 | 64 / 5975 | 26 / 3663 | 48 / 337 |

Ground truth from the STL itself (`terrain.stl`, area-weighted face slope,
219 944 triangles, 100×100 mm, relief 5.98 mm): **38.6% of area ≥45°**,
**25.4% ≥55°**, **3.7% (482 mm²) ≥75°**. Independently matches a comment
already in `finish_planner_wanaka_decompose.rs` ("38.5% of true area
≥45°").

**Q4. Is 313 / 482 mm² the right stopping point?** The claim is that the
residual is legitimate conditioning (hysteresis 10°, close, min-area floor)
because recovery asymptotes near 337 mm² at 4× the cost. Test that claim.

**Q5. Is the 27× classification cost (0.7 s → 19.2 s) acceptable, or
optimisable?** The user's position: *"if we need to kill performance to get
a real result, then either we need to optimise or just live with it. The
worse option is to lie."* So the cost is accepted — but not necessarily
unavoidable.

**Q6. Is "ball-nose tools are unaffected" correct?** It rests on
`diameter()` being honest for ball noses, so `cusp_radius() == radius()`.
If that is wrong the blast radius is every finishing op, not just tapered
ones.

## Gates currently green

`cargo clippy --workspace --all-targets -- -D warnings`;
`cargo test -p rs_cam_core --release --test param_sweep -- --ignored` (56/56);
`cargo test -p rs_cam_viz --lib` (216/216);
`cargo test -p rs_cam_core --lib` — **3 pre-existing adaptive3d reds are
expected** (`peck_plunge_progresses_when_depth_per_pass_equals_retract_clearance`,
`rapid_segment_lifts_to_safe_z_before_traverse`,
`planner_sim_dexel_parity_agent_search`).

**Q7. Do the gates actually cover this?** The 56 sweeps were *unchanged* by
both fixes, which suggests the sweep fixtures never exercise a tapered tool
on ribbon terrain. If so, that is a coverage gap worth its own sentry.

## What these fixes invalidate

Every strategy-value conclusion measured this session ran on a
decomposition that could not produce a steep region. Specifically **not
established**, despite being asserted in earlier sections:

- "contour and pencil have nothing to do on this part"
- "scallop wins every time"
- the band-mix tables in §14, §14a and §14m–§14p

They are untested, not disproven — the comparison could not have come out
any other way. Re-running them is downstream work, not part of this audit,
but do not treat them as established while auditing.

## Calibration — the agent's error record on this subsystem

In one session, on this one operation, four successive diagnoses were wrong
and each was killed by measurement rather than argument:

1. "`territory_clip` does not confine it" — measured at 0.5 mm dexel with a
   0.5 mm tool tip. Retracted.
2. "phantom rest from coarse quantisation" — refuted when a 0.1 mm re-run
   landed within 2.5% of the 0.5 mm one.
3. "`min_rest_depth_mm` sits below the 22.5 µm upstream cusp" —
   arithmetically true (0.30²/(8×0.5)), causally irrelevant: raising the
   dial moved cutting 0.5%.
4. "this terrain has no steep faces" — inferred from bulk relief; the STL
   says 25.4% is steeper than 55°.

Plus a structural error: a hypothesis built on gaps between rows of a
**filtered** table (only dropped spans were listed; the gaps held the
surviving ones).

The pattern: finding *a* mechanism that explains the evidence and stopping
there. The root cause in each case was found only by an instrument that
localised *where* rather than argued about *why* — the decisive one was an
engagement histogram showing 99.4% of cutting samples in air, which
indicted the field deciding WHERE and exonerated every dial deciding HOW
MUCH.

**Audit accordingly: prefer a measurement that could falsify, over a story
that fits.**
