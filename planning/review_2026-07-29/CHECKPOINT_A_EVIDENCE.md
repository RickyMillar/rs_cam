# CHECKPOINT A EVIDENCE — no behavioral change made; human approval required before any H2 routing PR

Date: 2026-07-29
Basis: `planning/review_2026-07-29/TECH_DEBT_RESEARCH_AND_FIX_PLAN.md` §H2
("Research experiments") and §3.3 ("Checkpoint A");
`planning/review_2026-07-29/TOOL_SCALE_SEMANTICS.md` (the oracle).
HEAD at time of run: `ae10cb2`.
Instrument: `crates/rs_cam_core/tests/checkpoint_a_valley_matrix.rs` — 12 tests,
**4.5 s** wall clock, default CI (no `#[ignore]`).
Reproduce: `cargo test -p rs_cam_core --test checkpoint_a_valley_matrix -- --nocapture`

**Status: research only.** No production source file was touched. Every number
below came from an actual run of the file named above; nothing is estimated.

---

## 0. What Checkpoint A has to decide

Four production sites answer *"how wide a band around this valley centreline can
the cutter actually work?"* with `cutter.radius()` — the ENVELOPE, 3.0 mm on the
shipped Ø1-tip/7°/Ø6-shank taper. The candidates the plan names:

| code | model | radius fed to the shipped fit equation |
|---|---|---|
| **ENV** | envelope baseline (today) | `envelope_radius_mm()` |
| **CUSP** | tip-sphere baseline | `cusp_radius_mm()` |
| **ENG** | engagement at local rest depth | `engagement_radius_mm(δ)` |
| **CLR** | profile clearance, vertical-slot reading (`TOOL_SCALE_SEMANTICS.md` §4.5) | `engagement_radius_mm(δ)` + `height_at_radius(w)` reach guard |
| **CLR+θ** | profile clearance with the local wall angle | exact two-wall solve over `width_at_height` |

The shipped fit equation (`crease_paths::centerline_cut_paths`) is
`n = round((half_width_mm − r) / offset_stepover).max(0).min(cap)`, and the same
scalar `r` also drives the pencil/clearing routing rule
`pencil ⟺ half_width_mm ≤ route_width_factor × r`.

## 1. Method and error bound

**Valley.** A straight V groove in a block whose top is `z = 0`: rim edges at
`x = ±w`, walls inclined `θ_l` / `θ_r` from horizontal, apex where they meet at
depth `D = 2w/(cot θ_l + cot θ_r)`, lateral position `x_apex = D/tan θ_l − w`.

**Truth.** The tool is queried only through `MillingCutter`. With
`H(u) = height_at_radius(u)`, a tool whose axis stands at `x` interferes iff
some profile point sits below the surface, so

```
deepest reachable tip depth at x:   z*(x) = max_u [ V(x+u) − H(|u|) ]
fits at rest depth δ:               z*(x) ≤ −δ
max lateral offset at rest depth δ: X(δ) = max{ q ≥ 0 : z*(x_apex ± q) ≤ −δ }
```

`X(δ)` is exactly the quantity the shipped equation approximates by
`half_width_mm − r`. This is a morphological erosion of the surface by the
cutter — the same operation the drop-cutter performs — evaluated by dense
sampling of the tool's own profile. No new geometric model was introduced.

**Sampling bound.** `H` is sampled at `DU = 0.00025 mm` over `[0, R_env]`.
`V` is piecewise linear with `|V′| ≤ L = max(tan θ)`, and `H` is monotone, so
the sampled maximum understates the true maximum by at most `L·DU`, one-sided.
At the steepest wall in the matrix (85°, `tan = 11.43`) that is **0.0029 mm**.
The fit predicate subtracts that bound as a guard, so the reported ground truth
is *conservative*: it never claims a fit the real tool would not have. Against a
0.5 mm stepover it can move a pass count only within 2.9 µm of an exact
boundary. Truncation of the profile scan at `H(u) > δ` is exact, not
approximate (`V ≤ 0` everywhere, so those samples can never bind).

**Independent cross-check.** The same fit has a closed form,
`X_closed(δ) = min over p ∈ [0,δ] of [ (D−p)/tan θ_near − width_at_height(δ−p) ]`.
`closed_form_matches_profile_erosion_sampler` compares it against the erosion
sampler on all 176 cells × 3 tools:

```
taper Ø1/7°/Ø6 :  176 cells cross-checked, worst |Δ| = 0.00025 mm
taper Ø1/15°/Ø6:  176 cells cross-checked, worst |Δ| = 0.00025 mm
ball  Ø3       :  176 cells cross-checked, worst |Δ| = 0.00025 mm
```

Worst disagreement is exactly one sampling step. The two methods agree, so
CLR+θ is an independently computed model, not a restatement of the oracle.

**Matrix axes** (plan §H2 verbatim): wall angles 30/45/60/75/85°; rim
half-widths 0.25 (below-tip) / 0.5 (tip) / 1.0 (2×tip) / 2.0 (cone) / 3.0
(shaft) / 5.0 (beyond-shaft) mm; rest depths 0.05 / 0.1 / 0.2 / **0.4391**
(the taper's ball/cone tangency) / 0.6 / 1.0 / 2.0 mm. 210 combinations per
tool, of which **34 are N/A** (the valley is shallower than the commanded rest
depth), leaving **176 scored cells** per tool. Tools: Ø3 ball control,
Ø1-tip/7°/Ø6 taper (the shipped one), Ø1-tip/15°/Ø6 taper (second angle).
`stepover = 0.5 mm`, `cap = 4`, `route_width_factor = 2.0` — all shipped defaults.

**Scoring.** Pass counts are scored only on cells where the model actually
routes a pencil fan (a model that routes to clearing hands the region to another
strategy; scoring its moot fan would punish the conservative answer).

* **GOUGE** = `n_model > n_true` — a pass at a lateral offset the tool cannot
  hold at depth. In today's pipeline `paths_from_sampled` re-solves Z by
  drop-cutter, so the physical outcome degrades to an *air-cut* pass rather than
  a gouge; the model is still over-claiming, and any fixed-Z or claims-carving
  consumer of the same number would gouge. Scored as the conservative violation,
  per the plan's bar.
* **MISS** = `n_model < n_true` — reachable detail suppressed.
* **float-blind** = truth says the tool cannot hold the commanded depth on the
  apex at all (it wedges between the walls), yet the model routes a centreline
  there anyway.
* **coverage** = `Σ min(n_model, n_true) / Σ n_true` over the cells where truth
  says a pencil fan is the right strategy. Reported twice: with the routing
  verdict ignored (the fit equation alone) and as actually routed.

---

## 2. Matrix — shipped taper, Ø1 tip / 7° / Ø6 shank (envelope 3.000, cusp 0.500)

Entries are **gouge / miss / mean|Δpass|**, aggregated over the 7 rest depths in
each (angle, width) cell.

| wall θ | half-width | class | ENV | CUSP | ENG | CLR | CLR+θ |
|---:|---:|---|---|---|---|---|---|
| 30° | 0.25 | below-tip | 0/0/0.00 | 0/0/0.00 | 0/0/0.00 | 0/0/0.00 | 0/0/0.00 |
| 30° | 0.50 | tip | 0/0/0.00 | 0/0/0.00 | 0/0/0.00 | 0/0/0.00 | 0/0/0.00 |
| 30° | 1.00 | 2×tip | 0/3/0.75 | 1/0/0.25 | 0/0/0.00 | 0/0/0.00 | 0/0/0.00 |
| 30° | 2.00 | cone | 0/5/2.00 | 0/0/0.00 | 0/0/0.00 | 0/0/0.00 | 0/0/0.00 |
| 30° | 3.00 | shaft | 0/6/3.50 | 0/0/0.00 | 0/0/0.00 | 0/0/0.00 | 0/0/0.00 |
| 30° | 5.00 | beyond | 1/0/0.29 | 0/0/0.00 | 0/0/0.00 | 0/0/0.00 | 0/0/0.00 |
| 45° | 0.25 | below-tip | 0/0/0.00 | 0/0/0.00 | 0/0/0.00 | 0/0/0.00 | 0/0/0.00 |
| 45° | 0.50 | tip | 0/0/0.00 | 0/0/0.00 | 0/0/0.00 | 0/0/0.00 | 0/0/0.00 |
| 45° | 1.00 | 2×tip | 0/3/0.60 | 2/0/0.40 | 1/0/1.00 | 1/0/1.00 | 0/0/0.00 |
| 45° | 2.00 | cone | 0/6/2.33 | 0/0/0.00 | 0/0/0.00 | 0/0/0.00 | 0/0/0.00 |
| 45° | 3.00 | shaft | 0/7/3.43 | 0/0/0.00 | 0/0/0.00 | 0/0/0.00 | 0/0/0.00 |
| 45° | 5.00 | beyond | 0/0/0.00 | 0/0/0.00 | 0/0/0.00 | 0/0/0.00 | 0/0/0.00 |
| 60° | 0.25 | below-tip | 0/0/0.00 | 0/0/0.00 | 0/0/0.00 | 0/0/0.00 | 0/0/0.00 |
| 60° | 0.50 | tip | 0/0/0.00 | 0/0/0.00 | 0/0/0.00 | 0/0/0.00 | 0/0/0.00 |
| 60° | 1.00 | 2×tip | 0/3/0.50 | 3/0/0.50 | 2/0/1.00 | 2/0/1.00 | 0/0/0.00 |
| 60° | 2.00 | cone | 0/7/2.29 | 0/0/0.00 | 0/0/0.00 | 0/0/0.00 | 0/0/0.00 |
| 60° | 3.00 | shaft | 0/7/3.86 | 0/0/0.00 | 0/0/0.00 | 0/0/0.00 | 0/0/0.00 |
| 60° | 5.00 | beyond | 0/0/0.00 | 0/0/0.00 | 0/0/0.00 | 0/0/0.00 | 0/0/0.00 |
| 75° | 0.25 | below-tip | 0/0/0.00 | 0/0/0.00 | 0/0/0.00 | 0/0/0.00 | 0/0/0.00 |
| 75° | 0.50 | tip | 0/0/0.00 | 0/0/0.00 | 0/0/0.00 | 0/0/0.00 | 0/0/0.00 |
| 75° | 1.00 | 2×tip | 0/3/0.43 | 4/0/0.57 | 3/0/1.00 | 3/0/1.00 | 0/0/0.00 |
| 75° | 2.00 | cone | 0/7/2.43 | 0/0/0.00 | 0/0/0.00 | 0/0/0.00 | 0/0/0.00 |
| 75° | 3.00 | shaft | 0/7/4.00 | 0/0/0.00 | 0/0/0.00 | 0/0/0.00 | 0/0/0.00 |
| 75° | 5.00 | beyond | 0/0/0.00 | 0/0/0.00 | 0/0/0.00 | 0/0/0.00 | 0/0/0.00 |
| 85° | 0.25 | below-tip | 0/0/0.00 | 0/0/0.00 | 0/0/0.00 | 0/0/0.00 | 0/0/0.00 |
| 85° | 0.50 | tip | 0/0/0.00 | 0/0/0.00 | 0/0/0.00 | 0/0/0.00 | 0/0/0.00 |
| 85° | 1.00 | 2×tip | 0/4/0.57 | 3/0/0.43 | 3/0/1.00 | 3/0/1.00 | 0/0/0.00 |
| 85° | 2.00 | cone | 0/7/2.57 | 0/0/0.00 | 0/0/0.00 | 0/0/0.00 | 0/0/0.00 |
| 85° | 3.00 | shaft | 0/7/4.00 | 0/0/0.00 | 0/0/0.00 | 0/0/0.00 | 0/0/0.00 |
| 85° | 5.00 | beyond | 0/0/0.00 | 0/0/0.00 | 0/0/0.00 | 0/0/0.00 | 0/0/0.00 |

| aggregate | cells | ENV | CUSP | ENG | CLR | CLR+θ |
|---|---:|---|---|---|---|---|
| θ = 30° | 28 | 1/14/1.36 | 1/0/0.11 | 0/0/0.00 | 0/0/0.00 | 0/0/0.00 |
| θ = 45° | 32 | 0/16/1.28 | 2/0/0.17 | 1/0/0.14 | 1/0/0.20 | 0/0/0.00 |
| θ = 60° | 35 | 0/17/1.31 | 3/0/0.21 | 2/0/0.22 | 2/0/0.33 | 0/0/0.00 |
| θ = 75° | 39 | 0/17/1.23 | 4/0/0.22 | 3/0/0.23 | 3/0/0.43 | 0/0/0.00 |
| θ = 85° | 42 | 0/18/1.19 | 3/0/0.14 | 3/0/0.19 | 3/0/0.43 | 0/0/0.00 |
| **all** | **176** | **1/82/1.27** | **13/0/0.18** | **9/0/0.18** | **9/0/0.32** | **0/0/0.00** |

| | ENV | CUSP | ENG | CLR | CLR+θ |
|---|---|---|---|---|---|
| cells routed to a pencil fan | 176 | 74 | 49 | 28 | 89 |
| routing over-claim (pencil where truth says clearing) | **63** | 0 | 0 | 0 | 0 |
| routing under-claim (clearing where truth says pencil) | 0 | 39 | **64** | **64** | 0 |
| float-blind (centreline into material the tool cannot reach) | 24 | 24 | 24 | **3** | **0** |
| fit-equation coverage, routing ignored | **2%** | 100% | 100% | 98% | 100% |
| reachable-detail coverage as actually routed | **2%** | 15% | **0%** | **0%** | **100%** |

## 3. Matrix — second taper angle, Ø1 tip / 15° / Ø6 shank

Same envelope and cusp; only the cone angle differs. Included to show the
conclusions are not an artefact of α = 7°.

| aggregate | cells | ENV | CUSP | ENG | CLR | CLR+θ |
|---|---:|---|---|---|---|---|
| θ = 30° | 28 | 1/14/1.36 | 1/0/0.11 | 1/0/0.20 | 1/0/0.25 | 0/0/0.00 |
| θ = 45° | 32 | 0/16/1.28 | 2/0/0.17 | 2/0/0.25 | 2/0/0.33 | 0/0/0.00 |
| θ = 60° | 35 | 0/17/1.31 | 3/0/0.21 | 3/0/0.30 | 3/0/0.43 | 0/0/0.00 |
| θ = 75° | 39 | 0/17/1.23 | 4/0/0.22 | 3/0/0.21 | 3/0/0.38 | 0/0/0.00 |
| θ = 85° | 42 | 0/17/1.14 | 4/0/0.19 | 3/0/0.18 | 3/0/0.38 | 0/0/0.00 |
| **all** | **176** | **1/81/1.26** | **14/0/0.19** | **12/0/0.22** | **12/0/0.36** | **0/0/0.00** |

| | ENV | CUSP | ENG | CLR | CLR+θ |
|---|---|---|---|---|---|
| cells routed to a pencil fan | 176 | 74 | 54 | 33 | 88 |
| routing over-claim | 63 | 0 | 0 | 0 | 0 |
| routing under-claim | 0 | 39 | 59 | 59 | 0 |
| float-blind | 25 | 25 | 25 | 4 | 0 |
| fit-equation coverage, routing ignored | 2% | 100% | 100% | 98% | 100% |
| reachable-detail coverage as actually routed | 2% | 14% | 0% | 0% | 100% |

## 4. Matrix — ball control, Ø3 (envelope 1.500 == cusp 1.500)

| wall θ | half-width | class | ENV | CUSP | ENG | CLR | CLR+θ |
|---:|---:|---|---|---|---|---|---|
| 30° | 0.25 | below-tip | 0/0/0.00 | 0/0/0.00 | 0/0/0.00 | 0/0/0.00 | 0/0/0.00 |
| 30° | 0.50 | tip | 0/0/0.00 | 0/0/0.00 | 0/0/0.00 | 0/0/0.00 | 0/0/0.00 |
| 30° | 1.00 | 2×tip | 0/1/0.25 | 0/1/0.25 | 2/0/0.67 | 2/0/1.00 | 0/0/0.00 |
| 30° | 2.00 | cone | 1/3/0.83 | 1/3/0.83 | 3/0/1.00 | 0/0/0.00 | 0/0/0.00 |
| 30° | 3.00 | shaft | 1/3/0.83 | 1/3/0.83 | 0/0/0.00 | 0/0/0.00 | 0/0/0.00 |
| 30° | 5.00 | beyond | 0/0/0.00 | 0/0/0.00 | 0/0/0.00 | 0/0/0.00 | 0/0/0.00 |
| 45° | 1.00 | 2×tip | 0/1/0.20 | 0/1/0.20 | 2/0/0.50 | 2/0/1.00 | 0/0/0.00 |
| 45° | 2.00 | cone | 1/3/0.83 | 1/3/0.83 | 3/0/1.00 | 0/0/0.00 | 0/0/0.00 |
| 45° | 3.00 | shaft | 2/3/1.00 | 2/3/1.00 | 1/0/3.00 | 0/0/0.00 | 0/0/0.00 |
| 60° | 1.00 | 2×tip | 0/1/0.17 | 0/1/0.17 | 2/0/0.40 | 2/0/1.00 | 0/0/0.00 |
| 60° | 2.00 | cone | 1/3/0.71 | 1/3/0.71 | 3/0/0.75 | 0/0/0.00 | 0/0/0.00 |
| 60° | 3.00 | shaft | 1/3/0.71 | 1/3/0.71 | 1/0/2.00 | 0/0/0.00 | 0/0/0.00 |
| 75° | 1.00 | 2×tip | 0/1/0.14 | 0/1/0.14 | 2/0/0.33 | 2/0/1.00 | 0/0/0.00 |
| 75° | 2.00 | cone | 1/3/0.71 | 1/3/0.71 | 3/0/0.75 | 0/0/0.00 | 0/0/0.00 |
| 75° | 3.00 | shaft | 1/3/0.57 | 1/3/0.57 | 1/0/1.00 | 0/0/0.00 | 0/0/0.00 |
| 85° | 1.00 | 2×tip | 0/1/0.14 | 0/1/0.14 | 2/0/0.33 | 2/0/1.00 | 0/0/0.00 |
| 85° | 2.00 | cone | 1/3/0.71 | 1/3/0.71 | 3/0/0.75 | 0/0/0.00 | 0/0/0.00 |
| 85° | 3.00 | shaft | 1/3/0.57 | 1/3/0.57 | 1/0/1.00 | 0/0/0.00 | 0/0/0.00 |

(rows at 0.25 / 0.50 / 5.00 mm half-width are 0/0/0.00 for every model at every
angle and are elided; below-tip and tip-scale valleys support no offset pass on
a Ø3 ball, and 5 mm half-widths route to clearing under every model.)

| aggregate | cells | ENV | CUSP | ENG | CLR | CLR+θ |
|---|---:|---|---|---|---|---|
| θ = 30° | 28 | 2/7/0.52 | 2/7/0.52 | 5/0/0.45 | 2/0/0.67 | 0/0/0.00 |
| θ = 45° | 32 | 3/7/0.52 | 3/7/0.52 | 6/0/0.53 | 2/0/0.67 | 0/0/0.00 |
| θ = 60° | 35 | 2/7/0.39 | 2/7/0.39 | 6/0/0.39 | 2/0/0.67 | 0/0/0.00 |
| θ = 75° | 39 | 2/7/0.31 | 2/7/0.31 | 6/0/0.27 | 2/0/0.67 | 0/0/0.00 |
| θ = 85° | 42 | 2/7/0.29 | 2/7/0.29 | 6/0/0.24 | 2/0/0.67 | 0/0/0.00 |
| **all** | **176** | **11/35/0.39** | **11/35/0.39** | **29/0/0.35** | **10/0/0.67** | **0/0/0.00** |

| | ENV | CUSP | ENG | CLR | CLR+θ |
|---|---|---|---|---|---|
| cells routed to a pencil fan | 141 | 141 | 91 | 15 | 71 |
| routing over-claim | 15 | 15 | 0 | 0 | 0 |
| routing under-claim | 1 | 1 | 36 | 56 | 0 |
| float-blind | 56 | 56 | 56 | 0 | 0 |
| fit-equation coverage, routing ignored | 75% | 75% | 100% | **5%** | 100% |
| reachable-detail coverage as actually routed | 74% | 74% | 18% | 0% | 100% |

**ENV and CUSP are byte-identical on every ball cell** (pinned by
`ball_control_envelope_and_cusp_agree_on_every_cell`), which is what makes this
a control: the "existing Ball behavior remains unchanged" gate is satisfied by
any swap between those two, and is *not* satisfied by ENG, CLR or CLR+θ. Ball
behaviour therefore **does** move under the winning model — see open question 5.

## 5. Physics reference — shipped taper, symmetric V

The tip float column is the residual the pencil physically cannot remove. It is
a property of the valley, not of any dial.

| θ | w | valley depth | δ | apex reach | tip float | X_true | n_true |
|---:|---:|---:|---:|---:|---:|---:|---:|
| 30° | 1.00 | 0.577 | 0.20 | 0.500 | 0.077 | 0.519 | 1 |
| 30° | 2.00 | 1.155 | 0.20 | 1.077 | 0.077 | 1.519 | 3 |
| 30° | 3.00 | 1.732 | 0.60 | 1.655 | 0.077 | 1.827 | 3 |
| 30° | 5.00 | 2.887 | 2.00 | 2.809 | 0.077 | 1.402 | 2 |
| 45° | 0.25 | 0.250 | 0.20 | 0.067 | 0.183 | unreachable | 0 |
| 45° | 1.00 | 1.000 | 0.60 | 0.793 | 0.207 | 0.193 | 0 |
| 45° | 3.00 | 3.000 | 2.00 | 2.793 | 0.207 | 0.793 | 1 |
| 60° | 0.50 | 0.866 | 0.60 | 0.366 | 0.500 | unreachable | 0 |
| 60° | 2.00 | 3.464 | 2.00 | 2.964 | 0.500 | 0.556 | 1 |
| 75° | 0.25 | 0.933 | 0.20 | 0.067 | 0.866 | unreachable | 0 |
| 75° | 0.50 | 1.866 | 0.60 | 0.433 | **1.433** | unreachable | 0 |
| 75° | 1.00 | 3.732 | 2.00 | 2.299 | 1.433 | 0.080 | 0 |
| 85° | 0.25 | 2.858 | 2.00 | 0.067 | **2.791** | unreachable | 0 |
| 85° | 0.50 | 5.715 | 2.00 | 0.467 | **5.248** | unreachable | 0 |
| 85° | 1.00 | 11.430 | 0.20 | 4.540 | 6.890 | 0.600 | 1 |
| 85° | 3.00 | 34.290 | 2.00 | 20.828 | 13.462 | 2.312 | 4 |

24 of the 176 cells (α = 7°; 25 at α = 15°) are **tip-float** cells: the tool
wedges between the two walls and cannot hold the commanded depth on the apex at
all, with a worst float of **5.248 mm**. ENV, CUSP and ENG route a pencil
centreline into all 24; CLR's `height_at_radius(w)` guard catches 21 of 24;
CLR+θ catches all 24.

## 6. Asymmetric valleys — bisector position error

| θ_l | θ_r | w | depth | x_apex | δ | X_left | X_right | n_true | n_ENV | n_ENG | n_CLR+θ |
|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| 30° | 75° | 2.0 | 2.000 | +1.464 | 0.20 | 2.982 | 0.136 | 0 | 0 | 0 | 0 |
| 30° | 75° | 3.0 | 3.000 | +2.196 | 0.20 | 4.714 | 0.404 | 0 | 0 | **1** | 0 |
| 30° | 75° | 3.0 | 3.000 | +2.196 | 0.60 | 4.021 | 0.259 | 0 | 0 | **1** | 0 |
| 45° | 85° | 3.0 | 5.517 | +2.517 | 0.20 | 5.107 | 0.083 | 0 | 0 | 0 | 0 |
| 60° | 75° | 2.0 | 4.732 | +0.732 | 0.20 | 2.332 | 0.868 | 1 | 0 | **2** | 1 |
| 60° | 75° | 2.0 | 4.732 | +0.732 | 2.00 | 1.288 | 0.348 | 0 | 0 | **1** | 0 |
| 60° | 75° | 3.0 | 7.098 | +1.098 | 0.20 | 3.698 | 1.502 | 3 | 0 | 3 | 3 |
| 60° | 75° | 3.0 | 7.098 | +1.098 | 0.60 | 3.462 | 1.357 | 2 | 0 | **3** | 2 |
| 60° | 75° | 3.0 | 7.098 | +1.098 | 2.00 | 2.654 | 0.982 | 1 | 0 | **2** | 1 |

Two facts fall out:

1. **The apex is not the mask centre.** Max apex offset across these fixtures is
   **2.517 mm** — on a valley whose measured half-width is 3.0 mm. A centreline
   traced on the rest mask's medial axis stands up to that far from the trough.
2. **X_left and X_right differ by up to 22×** (2.982 vs 0.136 mm). But
   `paths_from_sampled` emits offsets symmetrically at `±k·stepover`, so a
   single scalar count is forced to the narrow side and the wide side is
   under-covered by construction. No candidate model can fix this; it needs an
   asymmetric fan (open question 4).

## 7. End-to-end confirmation probes (real routing path, not the matrix)

Three probes through `pencil_toolpath_structured_annotated` /
`detect_rest_valleys` on a synthetic trapezoidal groove (rim half-width 1.5 mm,
walls 70°, depth 1.2 mm) in a 40 × 24 mm block. Seconds each.

| probe | result |
|---|---|
| **1** — shipped envelope model on a 3 mm-wide valley the tip could ladder | 2 rest centrelines detected; **`offset_total = 1` on every chain — zero offset passes emitted**. Envelope `r = 3.000 ⇒ n = 0`; engagement at the local rest depth `r(δ=1.2) = 0.590 ⇒ n = 2`. The winning model would emit a 5-path fan where the shipped one emits a bare centreline. |
| **2** — Ø3 ball control on the same fixture | 1 centreline, `half_width_mm = 1.2`, 0 clearing regions; envelope == cusp confirmed at the routing layer. |
| **3** — the A8 / §7.1 defect, live | `reference_tool_diameter = 6.0` (**the shipped default**) ⇒ **1** chain; `= 12.0` ⇒ **2** chains. Same tool, same fixture. `resolve_reference_cutter` compares the nominal reference against `diameter()` = the Ø6 **shank**, so `6.0 > 6.0` is false and every tapered pencil op silently falls through to `SelfReferenced`. Confirmed in production code, not by inspection. |

Probe 1 is the headline the plan asked for, verbatim: *offset passes emitted = 0
under envelope on a 3 mm valley the tip could ladder*.

---

## 8. Verdict

### 8.1 The plan's bar

> "The winning model must be conservative against gouging without suppressing
> reachable detail."

**No single-scalar model clears that bar.** The honest answer is a hybrid, and
the reason is structural rather than a matter of tuning.

### 8.2 What each candidate does

| model | conservative against gouging? | suppresses reachable detail? | verdict |
|---|---|---|---|
| **ENV** (today) | Almost. 1 gouge cell — but only because it emits **nothing**: fit coverage **2%**, 82 miss cells, mean pass error 1.27. It is also the **worst** router: 63 over-claims (a single centreline into a valley truth says needs clearing) and 24 float-blind cells. | Catastrophically. **Dead code below the shank**: 0 of 107 sub-shank cells get a pass, though 48 of them physically support one. | **Reject.** Conservative only by omission, and its routing rule is the most wrong of all five. |
| **CUSP** | **No.** 13 gouge cells on the taper (14 at α = 15°), concentrated at half-width ≈ 2×tip where the fixed 0.5 mm tip radius over-states the cone's real width. | No — 0 miss, 100% fit coverage. | **Reject.** Confirms §4.3's "same error class, opposite direction" numerically. |
| **ENG** | **No.** 9 gouge cells (12 at α = 15°, **29** on the ball). Blind to all 24 tip-float cells. | No — 0 miss, 100% fit coverage. | **Not sufficient alone.** Correct on the width question, silent on the reach question. |
| **CLR** (§4.5's vertical-slot reading) | **Better but not clear.** Same 9 gouge cells as ENG (the guard fires only where `n` was already 0), but float-blindness drops **24 → 3** on the taper and **56 → 0** on the ball. | On the taper no (98% fit coverage). **On the ball, badly** — 5% fit coverage, because `height_at_radius(w) == None` for every `w > 1.5 mm` and the reading treats that as "refuse the fan". | **Adopt the guard, fix the `None` reading** — see 8.4. |
| **CLR+θ** | **Yes. 0 gouge, 0 miss, 0 route errors, 0 float-blind, 100% coverage on all three tools.** | No. | **The reference model.** Requires one input routing does not carry today: the local wall angle. |

### 8.3 The coupling nobody has costed yet — this is the biggest finding

`route_width_factor × r` and `(half_width − r)/stepover` are fed by the **same
scalar**. Shrinking `r` from the envelope to a depth-aware value fixes the fit
equation and **simultaneously collapses the pencil/clearing routing rule**:

| | ENV | ENG | CLR |
|---|---|---|---|
| routing under-claim (taper) | 0 | **64** | **64** |
| fit-only coverage | 2% | 100% | 98% |
| coverage **as actually routed** | 2% | **0%** | **0%** |

At the shipped `route_width_factor = 2.0`, a depth-aware radius routes almost
every truth-pencil branch to *clearing* instead, and the coverage gain the fit
equation bought is entirely given back. **An H2.1 PR that swaps the radius
without re-deciding the routing rule trades one defect for another, and a
Wanaka before/after would show it as "the pencil stopped cutting".** Pinned by
`swapping_the_radius_alone_collapses_the_pencil_clearing_routing_rule`.

CLR+θ scores **0/0** routing errors because it does not use a
`width ≤ factor × radius` rule at all. It routes on a **coverage criterion**:

```
pencil  ⟺  X_reach ≤ cap × offset_stepover
```

i.e. *"can a centreline plus the permitted offsets actually cover the reachable
band?"* That is the question the routing decision is trying to answer, and it is
dimensionally the right one — it compares a reachable width against the width
the op can emit, not against an unrelated tool scalar.

### 8.4 Recommended model (subject to human approval)

A three-part policy, in one shared function used by A1 (`pencil.rs`),
A3 (`unified_finish.rs`) and A7 (`compute/execute.rs`) per plan H2.5:

1. **Reach** — `X_reach`, the max lateral offset holding the local rest depth:
   * **where a local wall angle is available**, the two-wall profile solve
     `X = min over p∈[0,δ] of [ (D−p)/tan θ − width_at_height(δ−p) ]`.
     The rest-field grid already carries `surface_z` per cell
     (`RestGrid::surface_z`), so a per-branch wall angle is a finite difference
     on data the detector already computes — no new geometry pass.
   * **fallback where it is not**, `engagement_radius_mm(δ)` as the width term
     (this is `CLR` with the corrected `None` reading, and it degenerates to
     `ENG` plus the guard below).
2. **Float guard** — refuse the branch when
   `height_at_radius(half_width_mm)` is `Some(h)` and `h < δ`: the tool wedges
   before reaching the commanded depth. `None` means **the rim does not
   constrain the tool**, not "refuse" — see the ball's 5% column for the cost of
   getting that backwards.
3. **Routing** — `pencil ⟺ X_reach ≤ cap × offset_stepover`; wider ⇒ clearing.
   Retire `route_width_factor × pencil_radius`. Pass count
   `n = floor(X_reach / offset_stepover)`, capped.

Envelope-derived grid padding, trust erosion and region-polygon dilation
(`rest_field.rs:280/362/482`) stay exactly as they are — rule 4.

### 8.5 What the depth statistic must be

H2.1 asks whether to extend `RestCenterline` with median or peak rest depth.
The matrix settles the *direction* analytically rather than empirically:
`engagement_radius(δ)` is monotone non-decreasing in `δ` (pinned by
`tool_scale_semantics_pr2.rs`), so a **larger** depth statistic yields a
**larger** width term, hence **fewer** passes and **more** refusals — strictly
more conservative on both the fit and the guard.

Therefore: **peak (or a high quantile) rest depth is the conservative choice;
median is optimistic.** But a single per-centreline scalar is the wrong shape
either way — `X_reach` is a *local* quantity, and a branch that is 0.2 mm deep
at one end and 2 mm at the other has no honest single answer. Recommendation:
carry **both** `median_rest_mm` and `peak_rest_mm` on `RestCenterline` for the
routing decision (which is per-branch by construction), and evaluate the fit
equation **per sampled point** inside `paths_from_sampled`, where the local rest
depth is available and the pass can be truncated instead of dropped. That is a
larger change than H2.1 as scoped; the human should decide whether to take it
now or ship the per-branch scalar first.

---

## 9. Open questions for the human

1. **Depth statistic.** Peak (conservative, suppresses detail on long mixed-depth
   branches), median (optimistic — the matrix shows this direction produces
   gouge cells), or per-sample local depth (correct, but widens H2.1's scope to
   `paths_from_sampled` and the `RestCenterline` serde surface)? §8.5.
2. **Routing rule.** Retire `route_width_factor` in favour of the coverage
   criterion `X_reach ≤ cap × offset_stepover`? This changes the meaning of an
   existing user-facing dial (`PencilParams::route_width_factor`, serialised in
   project files) and couples routing to `num_offset_passes`, which is currently
   an independent cap. If it stays, it needs re-scaling — at a depth-aware
   radius, 2.0 under-routes 64/176 cells.
3. **Wall angle plumbing.** Accept a finite-difference wall angle off
   `RestGrid::surface_z` as the CLR+θ input (0 gouge / 0 miss / 100% coverage),
   or ship the angle-free fallback (0 miss but 9-29 gouge cells and 3 residual
   float-blind cells) and defer? The angle-free fallback is *not* conservative
   against gouging, so shipping it means accepting that the plan's bar is met
   only in the fixed-Z sense, not literally.
4. **Asymmetric valleys.** `half_width_mm` is a single scalar and
   `paths_from_sampled` emits `±k·stepover` symmetrically, while the matrix shows
   left/right reach differing by up to 22× and the apex sitting up to 2.517 mm
   off the mask centre. Options: (a) accept the narrow-side count on both sides
   (today's behaviour, under-covers the wide side); (b) carry per-side reach and
   emit an asymmetric fan; (c) leave asymmetric valleys to clearing. This is a
   *policy* decision, not a model decision — no candidate changes it.
5. **Ball regression tolerance.** The gate says "existing Ball behavior remains
   unchanged unless a separately justified correction is found." On the Ø3 ball
   the winning model **does** move behaviour: ENV/CUSP score 11 gouge / 35 miss /
   56 float-blind / 74% coverage; CLR+θ scores 0/0/0/100%. Is that the
   "separately justified correction", or must the ball path be pinned to today's
   output and only tapers migrate?
6. **Gouge severity.** Every "gouge" in this matrix degrades to an *air-cut* pass
   today because `paths_from_sampled` re-solves Z by drop-cutter. Should the
   acceptance gate be written on gouge (as scored here, protecting future
   fixed-Z / claims-carving consumers) or on wasted cutting time (the actual
   present-day symptom)? The two rank the candidates differently: on wasted time
   ENV is the worst by a wide margin; on gouge ENV looks near-clean.

---

## 10. Proposed API and migration table

The API proposal and the full site-by-site migration table are **not restated
here** — they live in `planning/review_2026-07-29/TOOL_SCALE_SEMANTICS.md`:

* §9 — the ADR (additive named methods vs typed newtypes) and its decision;
* §9.1 — the 18-row PR-2 migration table plus the "caller-supplied scalars PR-2
  must NOT yet delete" table;
* §6.A — the eight routing/reach sites (A1-A8) this evidence is about.

PR-2 landed rows 1-18 of that table (`606b8d5`), including the
`crease_paths::centerline_cut_paths` scalar deletion (row 18), so **A2/A4 are
now a one-line change inside one function**. This evidence gates that line.

One correction to record against §4.5: `height_at_radius(w) == None` is
documented there as "the valley is wider than the whole cutter — this is a
clearing job, not a pencil job". The matrix shows the second half of that
sentence is wrong for the ball control, where `w > 1.5 mm` is an ordinary
finishing case that still supports 2-4 offset passes. `None` means the rim does
not constrain the tool; the clearing decision must come from the coverage
criterion, not from the `None`.

---

## 11. H2.1 items this evidence gates

Both are prerequisites for the routing change, not consequences of it:

1. **The tapered-pencil `SelfReferenced` defect (task #12 /
   `TOOL_SCALE_SEMANTICS.md` §7.1 / A8).** `resolve_reference_cutter`
   (`pencil.rs:1051-1062`, called at `:1080`, `:1183`, `:1307`) compares the
   nominal reference against `cutter.diameter()` — the **shank**. On the shipped
   taper `diameter() = 6.0`, so the default `reference_tool_diameter = 6.0`
   fails `6.0 > 6.0 + 1e-6` and every tapered pencil op silently falls to the
   analytic self-probe. **Confirmed live by probe 3**: 1 chain at the default
   versus 2 at a reference above the shank, same tool and fixture. Until this is
   fixed, no rest measurement on a tapered tool means what it says, and any
   before/after this programme produces would be measuring the wrong reference.
   Required meaning is CUSP (`cusp_radius_mm() * 2.0`), or better, pass
   `&dyn MillingCutter` and compare `cusp_radius_mm()` directly.
2. **The `RestFieldParams::pencil_radius` seam (§7.3).** The field defaults to
   `0.5` (tip-scale — the *correct* intent) and all three production callers
   (`pencil.rs:1224`, `unified_finish.rs:646`, `compute/execute.rs:2122`)
   overwrite it with `radius()` (shank-scale). It is the single lever H2.1 needs
   to split *routing* from *padding*, which is why PR-2 deliberately did not
   delete it. Note also that every existing `rest_field.rs` unit test builds
   `pencil_radius: pencil.radius()` with **ball** cutters, where the two scales
   coincide — those green tests are inert as coverage for this change.

## 12. Adjacent defects surfaced by this work (report only, nothing fixed)

1. **Routing/fit scalar coupling** (§8.3). Not previously named: the oracle's A1
   records that grid padding and the routing decision read the same number by
   two different plumbing paths, but not that shrinking the number breaks
   routing while fixing the fan.
2. **`height_at_radius(w) == None` semantics** (§10). The oracle's §4.5 reading
   costs the ball control 95 percentage points of fit coverage.
3. **Symmetric fan on asymmetric valleys** (§6). `paths_from_sampled` emits
   `±k·stepover`; measured left/right reach differs by up to 22×.
4. **Tip float has no diagnostic channel.** 24 of 176 taper cells are physically
   unreachable at the commanded depth, with up to 5.248 mm of residual, and the
   op emits a centreline into all of them with no report. Same shape as the
   A/M9 standing-material gap.
5. **`unified_finish.rs:719-721`'s justifying comment** (§7.2) remains factually
   wrong; it steers H2.3 and should be corrected whenever that file is next
   touched.
