# Independent audit: `radius()` vs `cusp_radius()`

Date: 2026-07-29

Scope: commits `5732f57` and `32c5e48`, the 3D finishing path, and the evidence in `unified_v3_design.md` §14q–§14r.

## Findings

- **[high] The two-way physical-extent/feature-scale rule is useful but incomplete.** A tapered cutter has a third class: **contact-width/reach at the actual axial engagement**. That quantity is neither the 3 mm shaft envelope nor always the 0.5 mm tip sphere; the existing `MillingCutter::engagement_radius(depth)` API models it. Routing a valley, deciding whether an offset pass fits, and deciding whether a crease needs clearing fall in this third class. `cusp_radius()` is exact for ball-tip cusp math, but its trait documentation overstates it as the answer to every “how small a feature can this cutter cut?” question. Cone/shank clearance can still make a tip-scale feature unreachable. `Adaptive3dParams` already provides the right precedent by carrying separate engagement and envelope radii.

- **[high] Several finishing-path feature/contact uses still consume the tapered shaft radius.** These violate the stated rule or fall into the missing contact-width class:
  - `crates/rs_cam_core/src/pencil.rs:1224` and `crates/rs_cam_core/src/unified_finish.rs:646` set the rest-field routing yardstick from `radius()`.
  - `crates/rs_cam_core/src/pencil.rs:1279` and `crates/rs_cam_core/src/unified_finish.rs:717` pass `radius()` into width-capped offset-pass counting.
  - `crates/rs_cam_core/src/unified_finish.rs:721` derives pencil offset stepover from `radius()`.
  - `crates/rs_cam_core/src/pencil.rs:1360` explicitly asks for the ball/corner contact radius, but a tapered ball falls through to the 3 mm shaft radius. This should use the 0.5 mm tip sphere for the current bisector model.
  - `crates/rs_cam_core/src/unified_finish.rs:868` passes `radius()` to the crease-own-region threshold. Creases are currently empty at this production call, so this one is dormant rather than harmless.
  - `crates/rs_cam_core/src/compute/execute.rs:2122` gives generic rest analysis the shaft-sized routing yardstick.
  - `crates/rs_cam_core/src/narrate.rs:840` scales the “suspiciously large arc” diagnostic from `radius()`. This is ambiguous rather than clearly wrong: if “large” is relative to the tool envelope it is correct; if it is relative to path-feature scale it should use the cusp/contact radius. The diagnostic contract needs to say which.

  A minimal correction is a narrowly named ball-tip/contact helper for the bisector and current routing heuristics, preserving `radius()` for rest-grid padding, boundary erosion, and tool-footprint dilation. A complete correction should use engagement-dependent width where the relevant depth is known.

- **[high] The generation-grid defect remains real; chord refinement does not cover all of it.** `crates/rs_cam_core/src/finish_setup.rs:95` still derives generation cell size from the shaft. In scallop generation that cell size controls slope/curvature input to `ring_stepover`, ring decimation spacing, and chord probe spacing. Chord refinement repairs Z error along already selected chords; it does **not** recover missed XY ring placement, missed slope/curvature samples, or an over-wide stepover selected from the coarse field. The behavior should change, but changing the shared helper globally without an A/B is too blunt: it also moves standalone Scallop, RampFinish, and SteepShallow and fine tapered drop-cutter grids are substantially more expensive than the tiny-probe classification grid.

  Evidence gate: hold classification/planner inputs fixed; compare shaft-grid vs cusp-grid generation on a synthetic sub-shank ribbon and Wanaka. Measure max/P95 surface residual or cusp-height error, standing material, collisions, move count, and generation time for Scallop, RampFinish, SteepShallow, and each UnifiedFinish band. If the fine grid wins, either adopt it or retain a coarse coverage grid while obtaining slope/decimation data through local/adaptive samples.

- **[high] The claimed “313 / 482 mm² recovered” and stopping point are not valid.** The 482 mm² reference is 3D triangle surface area. `PlannedRegion::polygon.area()` is XY-projected area. On the current STL, direct measurement gave:

  | threshold | true 3D face area | projected face area |
  |---|---:|---:|
  | ≥65° | 1302.0 mm² | 343.8 mm² |
  | ≥75° | 481.6 mm² | 48.1 mm² |

  The 0.125 mm classification grid had 124.7 mm² of raw projected cells ≥75°, while default decomposition emitted 313.3 mm² of VerySteep polygons because 10° hysteresis deliberately grows the band through connected ≥65° cells. Therefore 313 and 482 are different measures with different threshold semantics; “65% recovered” has no physical meaning.

  A fixed-grid isolation run at the real 0.5 mm cusp produced:

  | row | VerySteep regions / XY area |
  |---|---:|
  | default | 10 / 313.3 mm² |
  | no min-area absorption | 64 / 348.7 mm² |
  | no close | 7 / 202.6 mm² |
  | no hysteresis | 4 / 60.6 mm² |
  | no conditioning | 64 / 64.5 mm² |
  | 0.25 mm-radius dials on the same grid | 36 / 313.6 mm² |

  Thus the 313→337 change seen when halving cell size is sampling convergence, not evidence that the residual “is legitimate conditioning.” On the same grid, halving all radius-derived dials changes 313.3 to only 313.6. Also, morphological close increases rather than erases VerySteep area in this case. No stopping point has been established. Use projected/rasterized ground truth with the same hysteresis semantics, or compare converged label grids directly.

- **[medium] The diagnostic says it isolates dials, but its implementation now confounds dials and resolution.** `crates/rs_cam_core/tests/finish_planner_wanaka_decompose.rs:399` says the surface is held fixed. The loop at lines 425–432 rebuilds the surface with a different tapered tip and therefore a different grid for every row. The text was true after `5732f57` and became false in `32c5e48`. This is why §14q's dial-only table and §14r's combined table cannot be treated as the same experiment.

- **[medium] The regression gates do not cover either fix.** `wanaka_band_mix_vs_cusp_radius` is ignored, labels itself “Diagnostic, not a gate,” and has no behavioral assertion. All 56 parameter sweeps use ball cutters for the affected Scallop/RampFinish/SteepShallow fixtures; none references `TaperedBallEndmill` or UnifiedFinish. The five changed UnifiedFinish unit-test call sites also use ball cutters, so switching `radius()` to `cusp_radius()` is behaviorally inert there.

  Add a fast synthetic sentry that asserts:
  1. Ø1-tip/Ø6-shaft taper has `radius() == 3.0` and `cusp_radius() == 0.5`, including through `ToolDefinition`.
  2. Classification uses 0.125 mm cells while retaining 3 mm physical padding.
  3. A narrow steep ribbon survives as MidSteep/VerySteep under a tapered tool.
  4. End-to-end UnifiedFinish emits the expected non-shallow band(s).
  5. A ball nose retains identical `radius()`/`cusp_radius()` behavior.

- **[medium] Fine-grid cancellation is unresponsive.** `SurfaceHeightmap::from_mesh_with_cancel` checks cancellation only after the entire Rayon batch (`crates/rs_cam_core/src/slope.rs:115`). The audited Wanaka 0.5 mm-tip classification took 47.3 seconds locally, so cancel can appear dead for that whole period.

- **[low] `finish_setup` documentation is stale.** `crates/rs_cam_core/src/finish_setup.rs:117` says classification origin, extent, and resolution mirror the generation surface and align 1:1. Resolution no longer mirrors it: classification is 0.125 mm while generation remains 0.75 mm for the audited taper.

## Answers to the prompt

### Q1 — Is the rule correct and complete?

Correct as a first cut, not complete. Keep `radius()` for maximum swept envelope. Use `cusp_radius()` for ball-tip cusp equations and ball-tip-scaled heuristics. Add a third, engagement-dependent contact-width class for fit/reach/routing. Grid resolution should ultimately be error/tolerance-driven rather than universally tied to one tool radius.

### Q2 — Other violating `radius()` uses?

Yes. The Pencil, UnifiedFinish claims, generic rest-analysis, and ambiguous narration sites listed above remain. The rest of the production sweep was predominantly physical: drop/push cutter queries, simulation stamping, collision envelopes, bbox padding, rest-grid support erosion, and boundary dilation correctly keep `radius()`.

### Q3 — Should generation cell size change?

The coarse-grid behavior needs correction, but do not blindly substitute `cusp_radius()` in the shared helper without the focused output-quality A/B. Chord refinement is not a complete substitute. Prefer splitting coverage from feature/slope resolution or using local/adaptive slope sampling if a full 0.125 mm tapered-cutter offset grid is too expensive.

### Q4 — Is 313 / 482 the right stopping point?

No. It compares XY polygon area after hysteresis to 3D face area before hysteresis. The “residual is conditioning” conclusion is falsified by the fixed-grid run: 0.5 and 0.25 radius-derived dials both produce about 313 mm².

### Q5 — Is the cost acceptable or optimisable?

It is a correctness cost worth paying until replaced, but it is optimisable. Local reproduction was 1.0 s at 143², 9.6 s at 425², 47.3 s at 849², and 102.2 s at 1697² (164.7 s total diagnostic runtime). Production's 849² row was therefore ~47× the 143² row in this run, not 27×. Best opportunities:

1. Rasterize/top-project mesh triangles directly into classification cells instead of performing a tiny-ball drop at every cell.
2. Use adaptive/multiresolution refinement near slope thresholds and narrow connected features.
3. Cache classification fields by mesh, tolerance, and requested resolution.
4. Reuse one high-resolution true-surface field across planner consumers.
5. Poll cancellation per chunk rather than after the full Rayon batch.

### Q6 — Are ball noses unaffected?

Yes, narrowly for these two commits. `BallEndmill::diameter()` returns its actual ball/cutter diameter; the trait fallback makes `cusp_radius() == radius()`. `ToolDefinition` delegates `diameter()` and `geometry_hint()`, so the equality survives wrapping. There is no direct sentry for this equality yet.

### Q7 — Do the gates cover this?

No. The unchanged sweeps are explained by fixture choice, not proof of safety. Add the synthetic tapered ribbon sentry above and at least one tapered-tool parameter sweep/end-to-end UnifiedFinish case.

## Verification

Commands run:

```text
cargo test -p rs_cam_core --release --test finish_planner_wanaka_decompose \
  -- --ignored --nocapture wanaka_band_mix_vs_cusp_radius
# passed; rows: 1.0s, 9.6s, 47.3s, 102.2s; 164.72s total

cargo test -p rs_cam_core --lib tool::tests
# 22 passed

git diff --check 5732f57^..32c5e48
# clean
```

A temporary release-mode isolation harness produced the area/conditioning table above and was removed after the run. The user-modified `planning/airrun_2026-06-01/wanaka.toml` was read only and not changed by the audit.
