# F-036b1 — Validate modulation invariants against IR or re-simulate (deferred from F-036b)

- **Stage:** sim ↔ modulation feedback loop
- **Severity:** low (loop calibration is unaffected; modulator algorithm-layer invariants are already pinned)
- **Status:** landed 2026-05-26 (also fixed a second sequencing bug: kinematics cycle-time integrator now re-runs after modulation; trace's `total_runtime_s` reflects modulated feeds)
- **First found in:** F-036b implementation session, 2026-05-26
- **Effort:** S-M
- **Linked PRs:** (cb9d853 + this commit)
- **Workstream:** Feed Modulation
- **Depends on:** F-036b landed

## Symptom

F-036b's `modulated_path_never_emits_below_min_chipload` (AB5) failed against the
production pipeline: 2748 cutting samples read chipload below the LUT band floor
(worst 0.0053 vs floor 0.0320 mm/tooth on a softwood-pocket AS001 variant).
The modulator's algorithm-layer invariant DOES hold (F-036 unit test
`modulation_never_emits_below_min_chipload` passes), so what AB5 caught is an
architectural seam, not an algorithm bug.

## Hypothesised root cause

`SimulationCutSample::chipload_mm_per_tooth` is computed during simulation from
the *commanded* `feed_rate`. F-036b's modulation runs AFTER `run_simulation`,
rewriting per-move `feed_rate` in the IR. The simulator does not re-execute,
so the cached sample stream still reflects pre-modulation feeds.

AB5's invariant ("modulator never emits below band floor") is correct; reading
it from `sample.chipload_mm_per_tooth` is what's wrong.

## Fix shape — two options

### Option A — inspect IR directly (S effort, recommended)

Rewrite AB5 to walk the modulated `Toolpath` IR:

```rust
for move_ in &toolpath.moves {
    let feed = move_.move_type.feed_rate();
    let chipload = feed / (rpm * flute_count as f64);
    assert!(chipload >= band.start * 0.95);
}
```

This is the algorithm-layer invariant, restated against the IR. F-036b's other
acceptance tests already pin chipload-band Within from a different angle
(`modulated_gates_within_constant_chipload_band` AB3); AB5 becomes a
strict-floor sister test.

### Option B — second simulator pass (M effort)

After modulation, re-run `run_simulation` so the sample stream reflects the
modulated feeds. Cleaner test semantics but doubles simulation cost for any
modulated project. Probably not worth it.

**Recommend Option A.** The re-sim cost is paid once at G-code emission today
and not amortised across multiple post-pass operations.

## Acceptance test

Edit the existing `modulated_path_never_emits_below_min_chipload` to:

1. Read the modulated toolpath IR via the post-`run_simulation` accessor.
2. For each cutting move, compute `chipload = feed_rate / (rpm * flute_count)`.
3. Assert `chipload >= band.start * 0.95`.
4. Remove the `#[ignore]` attribute.

## Files

- `crates/rs_cam_core/tests/adaptive_feed_modulation_pipeline_f036b.rs` —
  unignore + rewrite AB5
- (Possibly) `crates/rs_cam_core/src/session/mod.rs` — expose a
  `toolpath_ir(idx) -> &Toolpath` accessor if not present already

## Risk

S. The algorithm-layer invariant is already pinned by F-036's unit tests;
F-036b1 just connects the production-pipeline integration test to the right
read path.

## Notes

- Don't bundle with F-036c (Shapeoko cycle-time calibration). They're orthogonal.
- If a future workstream adds a "re-sim after modulation" mode for accuracy
  (e.g. validating modulated chipload against the LUT band post-hoc inside
  the simulator), Option B becomes relevant — but that's a separate finding.
