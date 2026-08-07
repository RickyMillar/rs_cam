# Unified load model — one feed-aware engagement→load function, every stage consumes it

**Status:** DESIGN (no code). Physics confirmed by deep research 2026-06-18
(17 verified claims, 3-0 across primary mechanistic-milling + wood sources; §4).
**Date:** 2026-06-18
**Why:** The feeds/load subsystem hits dead ends because the same physics
(deflection) is implemented in several places that **disagree on whether feed
affects deflection.** Until all load constraints live in one decision variable
(feed), the per-move "max MRR within limits" optimizer cannot be posed cleanly.
**North star UX:** *for this setup + this toolpath → these feeds & speeds* — one
solved operating point per path (feed schedule + binding constraint + MRR +
accel wall-clock), with the strategy choice emerging from comparing each path's
solved point. Builds on [[STRATEGY_ADVISOR_2026-06-17]],
[[KC_MILLING_CALIBRATION_2026-06-17]].

## 1. The problem, precisely

The optimizer's job is: **maximise feed (→ MRR) subject to `chipload ∈ band ∧
deflection ≤ limit ∧ power ≤ limit`**, per move, given the local engagement. For
that to be well-posed, every constraint must be expressed as a cap on the *same*
decision variable — feed. Today:

| Constraint | Feed-aware? | Consistent across consumers? |
|---|---|---|
| Chipload | yes (it *is* feed/tooth) | yes |
| Power | yes (`P = Kc_eff · ap · ae · vf / 60e6`) | yes — gate + modulation agree |
| **Deflection** | **NO in 5 of 6 consumers** | **NO** |

So you can't pose "max feed s.t. all three": two constraints respond to feed, one
(as modelled) does not. **That mismatch is the structural dead end** — every
prior attempt bolted another stage on top of it. Worse, the one place deflection
*is* feed-aware (the modulation optimizer) **disagrees** with the gate, so when
modulation lowers feed to "fix" deflection, the post-sim gate — feed-blind —
re-reads EXCEEDS and rejects the fix. Two stages, opposite verdicts on the same
cut. This is the same failure mode as the predictor↔gate drift fixed
2026-06-17 and the two-loader divergence — *duplicate physics that drifts.*

## 2. Current-state audit (grounded in the code, 2026-06-18)

**One shared cantilever** (good): `ToolDefinition::tip_deflection_mm(force_n,
axial, E)` — the integrated two-section beam. Caller supplies the force.

**Force into that cantilever = `Kc · axial · radial_width` (feed-blind) at FIVE
consumers**, all routing through `feeds::predict::tip_deflection_from_engagement`:

1. `feeds::predict::predict_peak_deflection_um` — Suggest's pre-gen predictor.
2. `tool_load::deflection::sample_tip_deflection_mm` — **post-sim gate** (radial
   from arc engagement).
3. `feeds::cutter_constraints::invert_deflection` — the axial-DOC **envelope**
   (binary-searches DOC at a fixed radial).
4. `tool_load::optimize::preflight` (preflight.rs:167) — an existing optimize path.
5. (predict + envelope share the same `tip_deflection_from_engagement`.)

**The divergent sixth — `feed_modulation.rs` (F-039 ConstrainedMax):**
- Uses its **own** cantilever `simple_tip_deflection` (not `tip_deflection_mm`).
- Computes a reference force `Kc · axial · radial`, then **scales feed by
  `bound / δ_ref`** — i.e. assumes `δ ∝ feed` (feed-aware), the *opposite* of the
  gate's feed-blind model.

So deflection diverges on **both** axes: the force model (feed-blind vs `δ∝feed`)
*and* the cantilever (`tip_deflection_mm` vs `simple_tip_deflection`).

**Power, by contrast, is already unified + feed-aware:** both the gate
(`tool_load::power::predicted_power_kw`) and modulation use
`GRAIN_ANISOTROPY_FACTOR · Kc · ap · ae · vf / 60e6`. **Deflection should be made
to look like power** — feed-aware, single source.

## 3. The convergence target

```
                 ONE feed-aware force model
   F_lat = force_lateral(Kc, ap, ae/D, fz)      ← [PHYSICS §4, pending research]
                          │
                 ONE cantilever
   δ = ToolDefinition::tip_deflection_mm(F_lat, ap, E)   ← already shared
                          │ every stage reads THIS, nothing reimplements force or beam
   ┌──────────────┬───────────────┬───────────────┬──────────────┐
   ▼              ▼               ▼               ▼              ▼
 predict        gate            envelope        preflight      modulation
 (Suggest      (validate;      (invert for     (optimize)     (per-move feed
  pre-gen)      AGREES now)     max DOC)                        = the optimizer)
                          │
                  feed schedule + binding-per-move  ──► compute_cycle_time (accel)
                          │
        strategy advisor = run per strategy, compare wall-clock (thin consumer)
```

The **"bomber feeds/speeds per path"** is literally the F-039 ConstrainedMax
output on the generated path against this unified model: a feed schedule, the
binding constraint per segment (the *why*), MRR, and accel wall-clock. The
strategy advisor becomes "run that per strategy, compare."

## 4. The force model (deep-research confirmed, 2026-06-18)

The mechanistic milling-force literature is unambiguous and directly contradicts
our `F = Kc · ap · ae`:

**(a) Force is driven by chip thickness, which is feed-per-tooth × immersion.**
Instantaneous uncut chip thickness `h = fz · sin(θ)` (θ = tooth immersion angle).
Per-element force = (cutting coeff · h + edge coeff) · ap, resolved into
tangential / radial / axial via KT/KR/KA; the **radial component is the lateral,
deflection-causing force**, tied to tangential by the KR/KT ratio.
[sciencedirect S100093611300054X, S2666496825000482 — both 3-0]. Our model has
no `h` (feed) term at all.

**(b) Radial width `ae` enters through the engagement ARC, not linearly.**
Total force integrates the elemental forces over the engaged arc; `ae` sets the
entry/exit angle limits of integration and the number of engaged teeth, via a
dimensionless `W/D` engagement factor — *not* a linear width multiplier
[S100093611300054X 3-0; ctemag 3-0]. Our `· ae` is the wrong handle.

**(c) Mean uncut chip thickness, partial immersion:**
`h_m = fz · sin(ψ/2)`, with engagement angle `cos ψ = 1 − ae/r` (r = radius)
[woodresearch.sk 201905/12, 2-1]. Radial engagement folds into `h_m` through ψ.

**(d) Wood is AFFINE in chip thickness with a non-zero edge intercept — the key
correction.** Quasi-orthogonal CNC wood milling: `Fc1z = 49.95·h_m + 5.30`
(conventional), `49.12·h_m + 5.94` (climb), R² ≈ 0.99 [woodresearch.sk, 3-0].
MDF: higher fz → monotonically higher force [mdpi 14/9/1085, 3-0]. The intercept
is a **fracture-toughness / edge-ploughing term** → **halving the feed does NOT
halve the bending force; there is a force floor** (the "Ft ∝ fz, so halving feed
halves force" claim was *refuted 0-3*). Cutting speed is second-order [mdpi 3-0].

**(e) Size-effect alternative:** `kc = kc1.1 · h^(−mc) · (1 − 0.01·γ)` (γ = rake°)
[machiningdoctor ×2, 3-0]. We could **not** pin `mc` for wood (the 0.2–0.3 range
was unconfirmed, 1-2), so the **affine wood fit (d) is the recommended form** — it
sidesteps the uncertain exponent and is directly wood-validated.

**Recommended model:**
```
ψ        : cos ψ = 1 − ae/r                  // engagement arc from radial WOC
h_eff    = fz · sin(θ_peak),  θ_peak = min(ψ, π/2)   // PEAK chip thickness (worst
                                              //   engaged tooth) for deflection
F_lat    = ap · (Ks · h_eff + F_edge) · k_dir
```
- `Ks` (slope, N/mm², wood ≈ 50) + `F_edge` (edge/fracture intercept, N) replace
  the single `Kc`. Feed enters via `h_eff`; immersion via θ_peak; ap linear.
- `k_dir` folds the radial/tangential split (KR/KT) into one calibrated constant.
- **Use PEAK chip thickness** (not the revolution mean) — peak instantaneous force
  is what bends the tool; this is also what makes a parallel corner spike (high
  immersion) read hot, matching the sim.

**Reconciles with power (answers Q3):** the existing `P = Kc · MRR`
(MRR = ap·ae·vf) is the *energy* average and stays correct — naturally feed-aware
through MRR. The force model needs the *instantaneous* chip-thickness term that
power integrates away. Same specific-energy `Kc` family, two roles: power =
energy/volume × swept volume; deflection = specific force × instantaneous chip
area (ap·h). **That is exactly why power was already feed-aware and deflection
wasn't** — not an oversight, a missing term.

**Confidence:** the *form* (affine in `fz·sin(immersion)`, edge intercept,
arc-not-width) is well-confirmed (3-0, multiple primary sources). The *wood
coefficients* (`Ks ≈ 50`, `F_edge ≈ 5 N`) come from one quasi-orthogonal study —
treat as a starting band, calibrate against our existing milling-Kc reference
point (§6), and label suggestions "verify on a test cut."

## 5. Migration plan (consumer by consumer)

1. **Add the force function** `feeds::force::lateral_cutting_force(Kc, ap, ae,
   fz, geometry) -> N` (canonical, feed-aware). One home, doc-cited.
2. **`tip_deflection_from_engagement`** gains `fz` (or chip thickness) and calls
   the new force fn. The 5 feed-blind consumers inherit feed-awareness for free
   (they already route through it) — but their *callers* must now pass fz:
   - predict: has feed/rpm/flutes → fz available.
   - gate (`sample_tip_deflection_mm`): per-sample fz from the sim sample.
   - envelope (`invert_deflection`): operates at a candidate feed → pass it.
   - preflight: pass its operating feed.
3. **Delete modulation's bespoke `simple_tip_deflection` + `δ∝feed` scaling**;
   route it through `tip_deflection_from_engagement` / the shared force fn. (Keep
   a fast path only if profiling demands it, and pin it equal to the integrated
   beam with a sentry.)
4. **Advisor**: run F-039 modulation per candidate before timing, so it compares
   *optimized* paths, not raw Suggest-feed paths. Regime label falls out of the
   per-move binding constraint, not a heuristic.

## 6. Recalibration plan — DECIDED: literature-absolute (2026-06-20)

Exposing fz changes the force magnitude at every operating point. The affine
form has **two** constants (`Ks`, `F_edge`) where today there was one (`Kc`),
so calibration must set both. Two anchors were on the table — pin to our
existing milling-Kc force (preserve the old hot readings), or anchor to the
literature absolute. **Decision: literature-absolute.** The question that
settled it was not "match the wanaka benchmark" (wanaka is just a playground,
not gospel) but "what is the best, most honest UX for *any* generic cut."

- **What landed (`feeds::force`):** `Ks = 49.95`, `F_edge = 5.30 N/mm`
  (woodresearch.sk affine fit, R²≈0.99) attached to our `GenericHardwood` Kc
  as the anchor wood and scaled to other materials linearly by `Kc/anchor_Kc`.
  Both shape (9.42:1) and magnitude come straight from the wood measurement.
- **Why it's the best generic UX:** this is the physically-honest
  *instantaneous* bending force (chip area `ap·h`), ~9× lower than the old
  milling-lifted `Kc·ap·ae` aggregate. So each load constraint binds where it
  physically should: **roughing is chipload/power-bound; deflection gates only
  long/thin tools and aggressive small-tool cuts.** Cross-checked numbers
  (a 3 mm DOC full-slot in hardwood): a stubby 6 mm @ 45 mm reads ~18 µm
  (Validated); a 3 mm tool at L/D 10 reads ~82 µm (Within, finish-degradation
  note); Exceeds (>200 µm) needs L/D ~14+ or a deep/over-fed cut. The old
  inflated model fired the
  deflection gate on routine roughing and hid the real limiter — bad feeds
  advice. The honest model is exactly the clean per-path decomposition the
  "bomber feeds/speeds" UX needs.
- **Consequence — the deflection gate goes quiet for normal tools.** That is
  correct, not a regression: a gate that cries wolf on every roughing cut is
  worse than one that fires only when deflection is genuinely the limiter. The
  recent F-031 "wanaka is deflection-tool-limited" framing was an artifact of
  the inflated force; it is reversed here (wanaka reads Within; chipload is the
  limiter — which matches the sim, power idle at ~13%).
- **Sentry re-baseline (done):** the wanaka/AS013 "tool-limited / Exceeds"
  sentries were *re-conceived* (not deleted) to assert the honest "deflection
  Within; chipload is the limiter"; the Suggest deflection-back-off code-path
  tests were *re-tooled* to long-reach fixtures (where deflection genuinely
  binds) so they keep exercising their path. Lib suite green; the F-024/27/28
  integration sims still read `< 0.2 mm` (cooler, pass), F-029 AS013 re-pointed
  to Within. Per [[feedback_run_integration_after_physics_change]] the `--test`
  sims were run, not just `--lib`.
- **Honesty bar:** anchored to one quasi-orthogonal wood study, so absolute
  magnitude stays "approximate / verify on a test cut" — same bar as the
  milling-Kc factor.

## 7. Sentries (lock the convergence so it can't re-drift)

- **Agreement sentry:** for a fixed (tool, material, engagement, feed), all
  consumers (predict, gate-sample, envelope-probe, modulation) return the *same*
  deflection within tolerance. This is the anti-drift lock — the thing missing
  every prior round.
- **Feed-sensitivity sentry:** halving fz materially lowers predicted deflection
  (pins feed-awareness; would fail today for 5 of 6 consumers).
- **Optimizer↔gate consistency:** a path feed-modulated to the deflection limit,
  then simulated, reads `Within` at the gate (today it would read EXCEEDS — the
  dead end this fixes).

## 8. Why this is architecturally sound (not another dead end)

- It **removes** models (deflection: 2 cantilevers + 2 force models → 1 + 1), it
  doesn't add — the consolidation discipline that has been the actual fix.
- **One model, many consumers**, pinned by the agreement sentry → cannot drift.
- The expensive parts already exist and are tested: the per-move constrained
  optimizer (F-039), the accel integrator (`compute_cycle_time`), the integrated
  cantilever, the strategy comparison. We **point them at one model**, we don't
  build new flows (honors the core+worker+UI guardrail).
- Deflection is made to mirror **power**, which already works this way.

## 9. Risks / open questions

- Recalibration is real work and shifts many baselines (sequence as its own
  commit, like the milling-Kc landing).
- **Resolved by research:** use PEAK chip thickness (not revolution-mean) for
  deflection; use the AFFINE form (`Ks·h + F_edge`), not the Kienzle power form,
  because the wood size-effect exponent `mc` could not be pinned (0.2–0.3 was
  unconfirmed) while the affine wood fit is R²≈0.99. The edge intercept gives a
  force floor — feed alone can't drive deflection to zero (see §6).
- Wood coefficients (`Ks≈50`, `F_edge≈5 N`) are from a single quasi-orthogonal
  study; the *shape* is solid, the *magnitude* is calibrated to our reference,
  so the absolute band stays "approximate / verify on a test cut" — same honesty
  bar as the milling-Kc factor.
- Per-move cost: the gate runs per sample and modulation per move; if the
  integrated beam is too slow there, keep a calibrated fast closed-form **pinned
  equal** by the agreement sentry (don't let it become divergence #2).
- Does the optimizer also need to choose DOC/stepover/RPM, or is per-move feed
  (with the axial envelope picking DOC) enough for v1? Propose: v1 = feed only;
  DOC via existing envelope; revisit.

## 10. Build order (after this doc is approved)

1. Land the unified feed-aware force fn + thread fz through
   `tip_deflection_from_engagement` (+ the agreement & feed-sensitivity sentries).
2. Recalibrate; re-baseline sentries (lib + integration sims).
3. Route modulation through the shared model; delete `simple_tip_deflection`.
4. Optimizer↔gate consistency sentry.
5. Advisor runs modulation-per-strategy; regime from binding constraint.
6. Surface "for this setup + this path → feeds & speeds" (the F-039 output) in
   the op UI / MCP.

When step 2 lands, add the §11 sources to `CREDITS.md` (new formula references).

## 11. Sources (deep research 2026-06-18, verified claims)

Primary (mechanistic milling force):
- Circular/standard mechanistic end-milling model, `h = fz·sin θ`, KT/KR/KA per
  element, integrate over engaged arc — sciencedirect `S100093611300054X`.
- Mechanistic force model, coefficients as power functions of instantaneous chip
  thickness (size effect), up/down-mill calibration — sciencedirect
  `S2666496825000482`.

Primary (wood):
- Quasi-orthogonal CNC wood milling: `Fc1z = 49.95·h_m + 5.30` (conv.) /
  `49.12·h_m + 5.94` (climb), R²≈0.99; `h_m = fz·sin(ψ/2)`, `cos ψ = 1 − e/r`;
  `kc` rises as chip thins (fracture-toughness term ∝ 1/h_m) — woodresearch.sk
  `201905/12`.
- MDF milling: feed-per-tooth dominant on force, cutting speed second-order —
  mdpi `2079-6412/14/9/1085`.
- Oak peripheral milling: force is non-linear, multivariable — bioresources
  (NCSU), "modelling of cutting forces in oak peripheral milling".
- Affine `Fc = (Ks·h + Int)·ap`, intercept = toughness component — springer
  `s00107-021-01667-5`.

Secondary (Kienzle reference):
- `kc = kc1.1·h^(−mc)·(1 − 0.01·γ)`; kc1.1 = force for 1mm² × 1mm chip at 0° rake
  — machiningdoctor (machining-power calculator + kc glossary).
- Radial WOC enters via engaged-tooth count + W/D engagement factor — ctemag,
  "Understanding tangential cutting force when milling".

Refuted / not used: "halving fz halves force" (Ft∝fz, 0-3); `mc ≈ 0.2–0.3` for
wood (unconfirmed, 1-2); square-root Orlicz exponents (0-3).
