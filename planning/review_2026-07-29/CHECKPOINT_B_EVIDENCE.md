# CHECKPOINT B EVIDENCE — no default changed; human approval required before any shared-helper/policy default change

Date: 2026-07-29
Basis: `planning/review_2026-07-29/TECH_DEBT_RESEARCH_AND_FIX_PLAN.md` §H3
("Research harness" / "Required metrics" / "Candidate architectures"), §3.3
Checkpoint B, and **§A.0** (area is not a safe invariant — report label-grid
topology).
HEAD at time of run: `90d1292`.
Instrument: `crates/rs_cam_core/tests/checkpoint_b_resolution_ab.rs`.

Reproduce (fast subset, default CI, **23 s**):
`cargo test -p rs_cam_core --test checkpoint_b_resolution_ab`
Reproduce (full grid, `#[ignore]`, **354 s**, DEBUG):
`cargo test -p rs_cam_core --test checkpoint_b_resolution_ab -- --ignored --nocapture`

**Status: research only.** No production default moved; no policy selector
function changed. Every number below came from the single full-grid run named
above — nothing is estimated, interpolated or carried over from an earlier
session.

**Build profile: DEBUG.** Debug was fast enough (354 s for the whole grid), so
per the standing constraint no `--release` run was made. Absolute seconds below
are therefore *not* production timings; the **ratios between arms** are the
load-bearing figures and they are measured on one profile, one machine, one
run, sequentially.

---

## 1. Method

### 1.1 The one variable

Only the **generation-surface cell size** varies. Tool, tolerance, planner
dials, boundaries, feeds and every operation parameter are pinned. The cell is
varied by naming a `FinishResolutionPolicy` (PR-3) and handing it to the
generator, **not** by moving the tolerance and not through the cell-size
adapter — so the two named arms keep their own `mode` / `cell_source`
provenance instead of collapsing to `Explicit`.

| arm | mode | cell (mm) | cell source |
|---|---|---|---|
| envelope/4 (legacy — today's shipped default for all three ops) | `LegacyEnvelopeQuarter` | 0.7500 | `EnvelopeRadius` |
| intermediate (geometric mean of the two named arms) | `Explicit` | 0.3062 | `Explicit` |
| cusp/4 | `CuspQuarter` | 0.1250 | `CuspRadius` |
| tolerance-driven | `Explicit` | 0.1000 | `Explicit` |

Tool: Ø1 tip / 7° half-angle / Ø6 shank tapered ball — `envelope_radius_mm()`
3.0, `cusp_radius_mm()` 0.5, a 6× split. Tolerance pinned at **0.10 mm** on
every arm, chosen so the `.max(tolerance)` floor never binds on either named
arm (0.75 and 0.125 both clear it) — otherwise two arms would silently become
the same grid. `arms_are_distinct_grids_with_honest_provenance` asserts this.

The two `Explicit` arms are explicit *honestly*: neither has a tool scale
behind it. The intermediate is a bisection probe; the tolerance arm is "what
the floor would give if it bound".

### 1.2 Fixtures

All 16 × 16 mm height fields at 0.25 mm vertex spacing (so the FIXTURE is never
the thing limiting feature fidelity), padded by one envelope radius per side →
22 mm grids.

| fixture | geometry | why |
|---|---|---|
| narrow ridge | 2 mm wide, 4 mm tall triangular crest, flanks 76° | feature ~2.7 envelope-cells wide; the coarse grid cannot represent it |
| narrow valley | the inverse: 2 mm wide, 4 mm deep V-groove in a plateau | the coarse grid BRIDGES it instead of clipping it |
| mixed-slope ribbon | ~6° / 60° / 85° bands across X, Y-invariant | every class present and wide enough to be a region |
| patches + hole | two disconnected 50° domes plus a 62° conical pit | region COUNT can move independently of area |

**Wanaka: DEFERRED to the checkpoint session.** No read-only wanaka
characterisation was attempted — `planning/airrun_2026-06-01/wanaka.toml` is
user-modified in the working tree and no existing headless pattern fit inside
the time box. Nothing below claims anything about a real part.

### 1.3 Production source touched (and why)

Three **additive** pass-through entry points, one per generation consumer:

* `scallop::scallop_toolpath_structured_annotated_with_resolution`
* `ramp_finish::ramp_finish_toolpath_structured_annotated_with_resolution`
* `steep_shallow::steep_shallow_toolpath_split_with_resolution`

Each takes the `FinishResolutionPolicy` the op would otherwise resolve
internally; the pre-existing public function is now a one-line wrapper that
passes `<op>_generation_resolution(cutter, params.tolerance)` — literally the
value it used before. Without this seam the harness cannot hold everything
else fixed while moving the cell, because each op resolves its own policy
inside its own body.

Evidence that this changed nothing: PR-3's three pre-refactor FNV toolpath
fingerprints (`finish_resolution_policy_pr3.rs`, 8/8) and the PR-2 cell-source
tripwire (`tool_scale_semantics_pr2.rs`, 8/8) are unchanged and green.

### 1.4 Measurement domains (M1)

| metric | domain | honest limits |
|---|---|---|
| `residual` | Z deviation of an emitted CUTTING move endpoint from the **tool-centre offset surface**, bilinearly sampled from a pinned reference grid at **0.05 mm**. Negative = tool centre below the reference = gouge. | Heightmap-sample domain. NOT a dexel-stock measure and NOT a surface-normal distance. See §5.1 — this column is only a genuine resolution-error signal for RampFinish/SteepShallow. |
| `cusp p50/p95` | flat-ground cusp implied by the measured spacing between points on **adjacent rings**, `h = R − √(R² − (d/2)²)`, R = `cusp_radius_mm()` | an estimate on the tool-centre field, not a measured stock cusp. Comparable BETWEEN ARMS, which is what Checkpoint B needs. Scallop only. |
| `uncut core mm²` | `ScallopReport::uncut_core_mm2` — the same number `ToolpathStats::standing_material_mm2` carries (projected XY area, ring-cascade-residual stage) | hole-blind upper bound (known, A/M9) |
| `rapid grazes` | rapids sampled at 0.05 mm whose Z drops below the reference tool-centre surface | a **reference-field probe, not the dexel simulator**. Lower bound on collisions. `SimulationMetrics::rapid_collision_count` was NOT run — see §7. |
| label topology | connected components (4-connectivity) + per-class cell coverage over COVERED cells, and per-cell disagreement against the 0.05 mm reference labels | §A.0's invariant. Area appears only as a caveated column. |
| `min seg mm` | shortest non-zero 3D cutting segment | accel-friendliness proxy |

Reference grid: 441 × 441 = **194,481 cells** on every fixture, built in
16.4–17.3 s. It is an `Explicit` cell on purpose: the reference is not a
candidate policy, it is a ruler.

### 1.5 Determinism

`legacy_arm_is_deterministic` runs the legacy arm twice and asserts identical
FNV fingerprints, move counts and standing material. Without it every delta
below could be run-to-run noise. It passes.

`coarse_and_fine_arms_differ_on_the_narrow_ridge` is the non-vacuity guard in
the other direction: if envelope/4 and cusp/4 ever produce byte-identical
scallop output, every table here is comparing a grid with itself.

---

## 2. §A.0 compliance — label-grid TOPOLOGY (not area)

Region counts are connected components of each class on each arm's **own**
grid; disagreement is per-cell, walking the 0.05 mm reference grid and looking
the arm's label up by nearest cell (so every arm is scored on the same
population of points).

### 2.1 Region count vs the reference

| fixture | reference (0.05) mid / very | envelope/4 | intermediate | cusp/4 | tolerance |
|---|---|---|---|---|---|
| narrow ridge | 10 / **8** | 1 / **0** | 2 / 1 | 4 / 2 | 8 / 6 |
| narrow valley | 6 / **4** | 2 / **0** | 2 / 0 | 2 / 2 | 6 / 4 |
| mixed-slope ribbon | 3 / 2 | 1 / 2 | 2 / 1 | 2 / 1 | 3 / 2 |
| patches + hole | 39 / 0 | 4 / 0 | 3 / 0 | 3 / 0 | 3 / 0 |

**The VerySteep class does not exist at envelope/4 on either narrow fixture.**
The truth has 8 and 4 disconnected very-steep regions; the shipped grid finds
zero. Every arm finer than envelope/4 recovers at least one.

### 2.2 Per-class coverage and disagreement

| fixture | arm | cov shallow | cov mid | cov very | disagree vs reference |
|---|---|---|---|---|---|
| narrow ridge | reference | 0.875 | 0.075 | 0.050 | — |
| | envelope/4 | 0.773 | 0.227 | **0.000** | 16367/103041 = **15.9%** |
| | intermediate | 0.830 | 0.169 | 0.000 | 10268/103041 = 10.0% |
| | cusp/4 | 0.860 | 0.047 | 0.093 | 8342/103041 = 8.1% |
| | tolerance | 0.876 | 0.062 | 0.062 | 5136/103041 = 5.0% |
| narrow valley | reference | 0.925 | 0.050 | 0.025 | — |
| | envelope/4 | 0.905 | 0.095 | **0.000** | 12517/103041 = **12.1%** |
| | intermediate | 0.925 | 0.075 | 0.000 | 5774/103041 = 5.6% |
| | cusp/4 | 0.922 | 0.016 | 0.062 | 5132/103041 = 5.0% |
| | tolerance | 0.925 | 0.037 | 0.037 | 2568/103041 = 2.5% |
| mixed ribbon | reference | 0.854 | 0.112 | 0.034 | — |
| | envelope/4 | 0.795 | 0.198 | 0.006 | 10903/103041 = **10.6%** |
| | intermediate | 0.849 | 0.113 | 0.038 | 3206/103041 = 3.1% |
| | cusp/4 | 0.845 | 0.109 | 0.047 | 2243/103041 = 2.2% |
| | tolerance | 0.851 | 0.112 | 0.037 | 963/103041 = 0.9% |
| patches + hole | reference | 0.855 | 0.145 | 0.000 | — |
| | envelope/4 | 0.789 | 0.211 | 0.000 | 13747/103041 = **13.3%** |
| | intermediate | 0.832 | 0.168 | 0.000 | 6305/103041 = 6.1% |
| | cusp/4 | 0.817 | 0.183 | 0.000 | 5334/103041 = 5.2% |
| | tolerance | 0.822 | 0.178 | 0.000 | 4909/103041 = 4.8% |

Envelope/4 mislabels **10.6–15.9%** of the surface. Most of that error is gone
by the intermediate cell (3.1–10.0%); cusp/4 recovers a little more (2.2–8.1%).
The disagreement curve is steepest between 0.75 and 0.31 mm, not between 0.31
and 0.125 mm.

### 2.3 The area caveat, independently reproduced

The plan's §A.0 says area's direction flips between fixtures and is therefore
unsound as a gate. On these four fixtures the coarse grid **overstates**
non-shallow area on every one, while simultaneously **collapsing** topology:

| fixture | reference non-shallow mm² | envelope/4 | error |
|---|---|---|---|
| narrow ridge | 32.1 | 61.9 | **+93%** |
| narrow valley | 19.3 | 25.9 | +34% |
| mixed-slope ribbon | 37.7 | 55.7 | +48% |
| patches + hole | 37.4 | 57.4 | +53% |

An acceptance gate phrased "steep/non-shallow area must increase" would have
**passed the broken coarse grid on all four fixtures** — it already reports
34–93% more non-shallow area than the truth. Region count and per-cell
disagreement are the invariants that behave correctly here, exactly as §A.0
requires. All areas in this document are derived and caveated; no verdict rests
on one.

*(Caveat on region count itself: it is scale-sensitive for BAND-shaped classes.
On `patches + hole` the 0.05 mm reference fragments the domes' 45° annulus into
39 components where every arm sees 3–4. Topology is still the right invariant,
but a future gate should carry a minimum-component size or read annuli as
annuli — logged in §8.)*

---

## 3. Per-operation results

### 3.1 Scallop (commanded cusp height 0.020 mm)

| fixture | arm | gen s | moves | cutting | min seg mm | \|res\| p50 / p95 / p99 / max | deepest gouge | gouge>50µm | rapid grazes | rings | uncut core mm² | cusp p50/p95 | fingerprint |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| narrow ridge | envelope/4 | 3.88 | 1999 | 1923 | 0.0400 | 0.0000 / 0.0135 / 0.0314 / 0.0750 | −0.0750 | 4 | 1 | 38 | **0.00** | 0.0105 / **0.0291** | `1a3c047381a330f4` |
| | intermediate | 8.16 | 4941 | 4857 | 0.0400 | 0.0000 / 0.0276 / 0.0846 / 0.0846 | −0.0846 | 141 | 1 | 42 | **0.00** | 0.0064 / 0.0200 | `f8f84181cc05fbbd` |
| | cusp/4 | 14.62 | 8042 | 7940 | 0.0400 | 0.0000 / 0.0276 / 0.0846 / 0.0846 | −0.0846 | 202 | 0 | 51 | **19.32** | 0.0026 / 0.0047 | `11d51d0c0d524b8d` |
| | tolerance | 17.32 | 9253 | 9151 | 0.0398 | 0.0000 / 0.0155 / 0.0846 / 0.0846 | −0.0846 | 202 | 0 | 51 | **54.23** | 0.0013 / 0.0023 | `92aa1c0b441003ca` |
| narrow valley | envelope/4 | 3.73 | 1836 | 1770 | 0.0400 | 0.0000 / 0.0282 / 0.0437 / 0.0810 | −0.0810 | 5 | 0 | 33 | 0.00 | 0.0144 / **0.0276** | `cbec1f864b434401` |
| | intermediate | 8.97 | 4890 | 4808 | 0.0400 | 0.0000 / 0.0314 / 0.0437 / 0.0848 | −0.0848 | 6 | 1 | 41 | 0.00 | 0.0066 / 0.0194 | `3ee45711979a8466` |
| | cusp/4 | 12.27 | 6041 | 5945 | 0.0400 | 0.0000 / 0.0314 / 0.0437 / 0.0783 | −0.0783 | 9 | 1 | 48 | 0.00 | 0.0050 / 0.0158 | `cd4e0d4cf48724a3` |
| | tolerance | 11.95 | 4650 | 4580 | 0.0400 | 0.0000 / 0.0282 / 0.0437 / 0.0437 | −0.0437 | 0 | 1 | 35 | 0.00 | 0.0107 / 0.0158 | `2183b83e49b95da7` |
| mixed ribbon | envelope/4 | 3.85 | 1792 | 1728 | 0.0400 | 0.0001 / 0.0155 / 0.0314 / 0.0581 | −0.0581 | 9 | 12 | 32 | **0.00** | 0.0141 / **0.0313** | `d4e590555099c718` |
| | intermediate | 10.46 | 5128 | 5042 | 0.0400 | 0.0000 / 0.0041 / 0.0314 / 0.0407 | −0.0407 | 0 | 5 | 43 | **0.00** | 0.0064 / 0.0161 | `acad72795920859a` |
| | cusp/4 | 17.95 | 8107 | 8005 | 0.0400 | 0.0002 / 0.0030 / 0.0267 / 0.0314 | −0.0248 | 0 | 12 | 51 | **33.24** | 0.0019 / 0.0060 | `493e9791accd752d` |
| | tolerance | 19.47 | 8150 | 8048 | 0.0400 | 0.0000 / 0.0026 / 0.0248 / 0.0314 | −0.0270 | 0 | 10 | 51 | **31.03** | 0.0010 / 0.0110 | `727996b63b5a07a6` |
| patches + hole | envelope/4 | 3.66 | 1626 | 1564 | 0.0400 | 0.0000 / 0.0041 / 0.0099 / 0.0214 | −0.0214 | 0 | 2 | 31 | 0.00 | 0.0171 / **0.0276** | `fb05ee7cb8cf2ae2` |
| | intermediate | 7.33 | 3678 | 3616 | 0.0400 | 0.0000 / 0.0041 / 0.0120 / 0.0277 | −0.0277 | 0 | 3 | 31 | 0.00 | 0.0182 / 0.0200 | `c5a84457fac4daf2` |
| | cusp/4 | 11.75 | 4755 | 4675 | 0.0400 | 0.0000 / 0.0047 / 0.0123 / 0.0217 | −0.0217 | 0 | 6 | 40 | 0.00 | 0.0077 / 0.0200 | `6e68258c80d47593` |
| | tolerance | 13.37 | 4888 | 4802 | 0.0400 | 0.0000 / 0.0046 / 0.0139 / 0.0380 | −0.0380 | 0 | 6 | 43 | 0.00 | 0.0064 / 0.0200 | `8f7436218702a908` |

**Findings.**

1. **The shipped grid overshoots the commanded cusp on every fixture.** p95
   achieved cusp at envelope/4 is 0.0291 / 0.0276 / 0.0313 / 0.0276 mm against
   a commanded 0.020 — **138–157% of the dial**. Every finer arm lands at or
   below the dial. This is a real quality defect of the legacy cell, and it is
   invisible in the residual columns.
2. **But cusp/4 over-corrects into over-machining.** Achieved cusp p50 falls to
   0.0026 / 0.0050 / 0.0019 / 0.0077 mm — 2.6× to 10× *tighter* than commanded
   — with 2.9–4.5× the moves and 3.2–4.7× the generation time.
3. **cusp/4 makes scallop leave standing material where envelope/4 leaves
   none.** Ridge 0.00 → 19.32 mm², ribbon 0.00 → 33.24 mm² (tolerance arm:
   54.23 and 31.03). Mechanism is the one `scallop.rs` already documents:
   `max_rings` is budgeted from the FLAT-ground stepover while the loop selects
   a smaller one per ring; a finer grid sees the real convex curvature, shrinks
   the stepover further, and the cascade runs out of rings with the interior
   still standing. **A resolution change on scallop is therefore gated behind
   Checkpoint C, not independent of it.**
4. **The intermediate cell is the only arm that gets the cusp on-dial without
   the truncation**: cusp p95 0.0161–0.0200 (at or under the dial) and
   `uncut_core_mm2` 0.00 on **all four** fixtures, at 2.0–2.7× time.
5. `min seg mm` is 0.0400 on every scallop arm — scallop's shortest segment is
   NOT cell-driven, so a finer grid costs move *count* but not segment
   *shortness*.

### 3.2 RampFinish

| fixture | arm | gen s | moves | cutting | min seg mm | \|res\| p50 / p95 / p99 / max | deepest gouge | gouge>50µm | rapid grazes | fingerprint |
|---|---|---|---|---|---|---|---|---|---|---|
| narrow ridge | envelope/4 | 0.24 | 64 | 50 | 1.0570 | 0.1741 / 0.2859 / 0.2889 / 0.2889 | **−0.1621** | **2** | 0 | `4144a588359cd940` |
| | intermediate | 0.62 | 85 | 71 | 0.4645 | 0.1767 / 0.2837 / 0.2840 / 0.2840 | **0.0000** | **0** | 0 | `c11471f1719acbe2` |
| | cusp/4 | 2.81 | 80 | 66 | 0.2265 | 0.1764 / 0.2817 / 0.2834 / 0.2834 | 0.0000 | 0 | 0 | `c488b564d948f52d` |
| | tolerance | 4.36 | 78 | 64 | 0.1917 | 0.1787 / 0.2814 / 0.2831 / 0.2831 | 0.0000 | 0 | 0 | `64a67c1ace5a3ed9` |
| narrow valley | envelope/4 | 0.98 | 137 | 113 | 1.2953 | 0.2492 / 2.3939 / 2.3939 / 2.3939 | **−2.3939** | **2** | 1 | `343c2e5a20a82f8c` |
| | intermediate | 1.38 | 181 | 155 | 0.5529 | 0.1800 / 0.2813 / 0.2813 / 0.2813 | **0.0000** | **0** | 0 | `c5c4b09f6bb84e37` |
| | cusp/4 | 3.47 | 161 | 135 | 0.2320 | 0.1927 / 0.2803 / 0.2803 / 0.2803 | 0.0000 | 0 | 0 | `a6fe7f507a1f41c6` |
| | tolerance | 4.99 | 151 | 125 | 0.1897 | 0.1816 / 0.2805 / 0.2805 / 0.2805 | 0.0000 | 0 | 0 | `b33c7e0c48411675` |
| mixed ribbon | envelope/4 | 1.26 | 219 | 185 | 1.2264 | 0.0189 / 0.2964 / 0.4347 / 0.4347 | −0.4347 | 4 | 0 | `ebd159d8723ea76a` |
| | intermediate | 1.64 | 257 | 219 | 0.5342 | 0.0313 / 0.3029 / 0.4347 / 0.4347 | −0.4347 | 3 | 2 | `d54cf436f6d9e317` |
| | cusp/4 | 3.91 | 242 | 204 | 0.2255 | 0.0305 / 0.3037 / 0.6185 / 0.6185 | −0.6185 | 1 | 1 | `95d33644d196ccd4` |
| | tolerance | 5.58 | 237 | 199 | 0.1851 | 0.0313 / 0.3041 / 0.5554 / 0.5554 | −0.5554 | 2 | 2 | `68877448d3029600` |
| patches + hole | envelope/4 | 0.91 | 165 | 123 | 0.3473 | 0.0759 / 2.8550 / 3.9776 / 4.2195 | −4.2195 | 45 | 4 | `06291307d00bc534` |
| | intermediate | 1.21 | 223 | 179 | 0.2706 | 0.0616 / 1.8881 / 4.1195 / 4.2293 | −4.2293 | 67 | 5 | `d33b5a9c3a924205` |
| | cusp/4 | 3.54 | 236 | 184 | 0.2358 | 0.0512 / 1.7587 / 4.1537 / 4.2260 | −4.2260 | 57 | 8 | `4653d775201a4ce4` |
| | tolerance | 4.94 | 247 | 189 | 0.1891 | 0.0543 / 1.8884 / 4.1051 / 4.2339 | −4.2339 | 66 | 9 | `5f25fd2373ceeb14` |

**Findings.**

1. **RampFinish reads Z off the generation grid** (unlike scallop — §5.1), so
   its residual column is a genuine resolution-error signal. On the narrow
   valley the legacy grid produces a **2.39 mm gouge**; on the narrow ridge a
   0.162 mm gouge. **Every arm finer than envelope/4 eliminates both
   completely** (deepest gouge exactly 0.0000, zero samples past 50 µm).
2. **The win arrives at the intermediate cell, not at cusp/4.** 0.306 mm is
   already gouge-free on both fixtures, for **1.4–2.6×** the time; cusp/4 buys
   nothing further and costs **3.1–11.7×**.
3. **Machine time is barely affected**: move count grows only 1.11–1.43×
   cusp/4-vs-legacy. RampFinish is the cheapest place in the codebase to buy a
   resolution safety win.
4. **`patches + hole` gouges 4.2 mm at EVERY resolution** (45–67 samples past
   50 µm). That is not a grid artefact — it is the ramp descending into a 62°
   cone the tool profile cannot enter. Resolution does not fix it and no
   diagnostic reports it (§8).
5. `min seg mm` tracks the cell almost exactly (1.06 → 0.19 mm on the ridge,
   ~1.5–1.9× cell). A finer RampFinish grid is an accel-cost, not a move-count
   cost.

### 3.3 SteepShallow

| fixture | arm | gen s | moves | cutting | min seg mm | \|res\| p50 / p95 / p99 / max | deepest gouge | steep-half moves | fingerprint |
|---|---|---|---|---|---|---|---|---|---|
| narrow ridge | envelope/4 | 0.42 | 2381 | 2209 | 0.1220 | 0.0000 / 0.1890 / 0.1890 / 0.1890 | 0.0000 | 328 | `0fd20e5505494359` |
| | intermediate | 0.81 | 2445 | 2277 | 0.1220 | identical | 0.0000 | 328 | `d3c9b885d853eba5` |
| | cusp/4 | 3.18 | 2474 | 2306 | 0.1220 | identical | 0.0000 | 328 | `90302ec130256036` |
| | tolerance | 4.70 | 2474 | 2306 | 0.1220 | identical | 0.0000 | 328 | `90302ec130256036` |
| narrow valley | envelope/4 | 1.23 | 2727 | 2611 | 0.5000 | 0.0000 / 0.0000 / 0.1890 / 0.1890 | 0.0000 | 612 | `57d4571919892168` |
| | intermediate | 1.54 | 2823 | 2709 | 0.5000 | 0.0000 / 0.1890 / 0.1890 / 0.1890 | 0.0000 | 708 | `f7c3d79dd17b6ea5` |
| | cusp/4 | 3.87 | 2823 | 2709 | 0.5000 | identical | 0.0000 | 708 | `f7c3d79dd17b6ea5` |
| | tolerance | 5.46 | 2823 | 2709 | 0.5000 | identical | 0.0000 | 708 | `f7c3d79dd17b6ea5` |
| mixed ribbon | envelope/4 | 1.48 | 2925 | 2723 | **0.0009** | 0.0000 / 0.0542 / 0.3073 / 0.3073 | −0.0215 | 938 | `ee45de2b94049c55` |
| | intermediate | 1.90 | 3084 | 2900 | 0.0009 | identical | −0.0215 | 938 | `3ea4f1f541c86f81` |
| | cusp/4 | 4.33 | 3084 | 2900 | 0.0009 | identical | −0.0215 | 938 | `88818e78947789cf` |
| | tolerance | 5.94 | 3086 | 2902 | 0.0009 | identical | −0.0215 | 940 | `8708a2778caff890` |
| patches + hole | envelope/4 | 1.08 | 2557 | 2379 | 0.2393 | 0.0000 / 0.0390 / 0.1048 / 0.1154 | −0.0074 | 636 | `2d8b434e1a5e8d4e` |
| | intermediate | 1.48 | 2562 | 2390 | 0.2393 | identical | −0.0074 | 609 | `ac78689d4deef857` |
| | cusp/4 | 3.76 | 2591 | 2429 | 0.2393 | identical | 0.0000 | 582 | `fee143a9dea394da` |
| | tolerance | 5.31 | 2591 | 2429 | 0.2393 | identical | 0.0000 | 582 | `fee143a9dea394da` |

**Findings.**

1. **SteepShallow's emitted quality is resolution-INSENSITIVE on all four
   fixtures.** Every residual quantile is bit-identical across all four arms on
   every fixture. Move counts move 1–5%. Cost to go to cusp/4: **2.9–7.6×**.
2. **The output CONVERGES well above cusp/4.** Fingerprints are byte-identical
   between intermediate/cusp/4/tolerance on the narrow valley, and between
   cusp/4/tolerance on the ridge and patches. Beyond ~0.3 mm this op is
   producing the same G-code from a 10× finer grid.
3. **This is a dissociation worth flagging, not a clean "leave it alone".**
   The op *classifies* steep vs shallow on this same grid, and §2 shows that
   classification is 10.6–13.3% wrong at envelope/4 with the VerySteep class
   entirely absent on two fixtures — yet the path barely changes. Either these
   fixtures have bands too large to discriminate (likely: all four have
   contiguous steep territory much wider than a 0.75 mm cell), or the op's
   downstream dilation/overlap machinery is swamping the label error. Both
   readings argue for **deferring SteepShallow until a discriminating fixture
   exists**, not for concluding it is fine.
4. The 0.9 µm minimum segment on the mixed ribbon is present at **every**
   resolution — a degenerate-output defect independent of H3 (§8).

---

## 4. Runtime and memory scaling

### 4.1 Surface build alone — linear in cells, as expected

Cell 0.75 → 0.125 is 6× linear = **36× cells**. Measured build-time factor:

| fixture | 0.75 (961 cells) | 0.306 (5,329) | 0.125 (31,329) | 0.100 (48,841) | factor 0.75→0.125 |
|---|---|---|---|---|---|
| narrow ridge | 0.08 s | 0.46 s | 2.61 s | 4.08 s | **32.6×** |
| narrow valley | 0.07 s | 0.43 s | 2.69 s | 4.11 s | **38.4×** |
| mixed ribbon | 0.08 s | 0.50 s | 2.79 s | 4.39 s | **34.9×** |
| patches + hole | 0.07 s | 0.45 s | 2.58 s | 4.13 s | **36.9×** |

Mean **35.7×** against a 32.6× cell-count ratio (961 → 31,329) — the drop-cutter
grid build is **linear in cell count** with no super-linear term. The reference
grid (194,481 cells, 202× the legacy grid) built in 16.4–17.3 s, i.e. ~210×,
which confirms linearity out to the largest grid measured.

### 4.2 Whole-operation time factor, cusp/4 vs envelope/4

| op | narrow ridge | narrow valley | mixed ribbon | patches + hole | median |
|---|---|---|---|---|---|
| Scallop | 3.77× | 3.29× | 4.66× | 3.21× | **3.5×** |
| RampFinish | 11.71× | 3.54× | 3.10× | 3.89× | **3.7×** |
| SteepShallow | 7.57× | 3.15× | 2.93× | 3.48× | **3.3×** |

Whole-op factors are far below the 36× grid factor because the surface build is
2% of op time at 0.75 mm and only ~18% at 0.125 mm; the rest of the growth is
downstream geometry work (scallop's ring decimation spacing is 0.75 × cell, so
ring point count also scales). **Every op is >20% slower at cusp/4 and therefore
requires documented justification under the repository performance policy** —
that justification exists only for RampFinish (§3.2) and only at the
intermediate cell.

Intermediate-cell factor (the cheaper candidate): Scallop 2.0–2.7×, RampFinish
1.3–2.6×, SteepShallow 1.3–1.9×.

### 4.3 Memory

**Peak RSS was not instrumented** — no allocator hook or `/proc` sampling was
added, so this is an analytic figure, stated as such. Per cell the surface
carries `SurfaceHeightmap` (8 B `z` + 1 B `covered`) and `SlopeMap` (24 B
normal + 8 B angle + 8 B curvature) ≈ **49 B/cell**:

| grid | cells | ≈ bytes |
|---|---|---|
| envelope/4 on a 16 mm fixture | 961 | 47 KB |
| cusp/4 on a 16 mm fixture | 31,329 | 1.5 MB |
| reference 0.05 on a 16 mm fixture | 194,481 | 9.5 MB |
| envelope/4 on a 300 × 200 mm part | ~107,000 | 5.2 MB |
| **cusp/4 on a 300 × 200 mm part** | **~3,840,000** | **~188 MB** |

The last row is the scaling warning: a global cusp-scaled grid on a real part
is a ~190 MB allocation for the surface alone, before rings, polygons or the
toolpath. It is projected from the cell arithmetic, not measured.

---

## 5. Instrument honesty

### 5.1 The residual column does not mean the same thing for all three ops

Scallop's ring Z comes from a **per-point `point_drop_cutter` query against the
mesh** (`scallop.rs` `ring_to_3d`), i.e. it is exact and resolution-independent;
the grid affects scallop only through stepover selection, ring decimation
spacing and the ring budget. RampFinish and SteepShallow read Z off
`surface.heightmap`, so their emitted Z carries the grid's error directly.

Consequence: for scallop the residual column is dominated by the **reference
grid's own** bilinear interpolation error at the points the path happens to
visit, and it rises with a finer arm simply because a denser path visits more
sharp-feature cells (ridge: 4 → 202 samples past 50 µm while `deepest gouge`
barely moves). It is reported for completeness and **is not used for any
conclusion about scallop**. Scallop's conclusions rest on achieved cusp, ring
count and standing material. For RampFinish and SteepShallow the column is
load-bearing and is used.

Any future "surface residual" gate on scallop must not be built on a sampled
reference field.

### 5.2 What was skipped, and why

| plan metric | status |
|---|---|
| peak memory | **skipped as a measurement**; analytic estimate given (§4.3). No allocator instrumentation was added for a research harness. |
| rapid collisions (`SimulationMetrics::rapid_collision_count`) | **skipped**. A dexel sim per arm × op × fixture (48 runs) does not fit the time box, and the fine-vs-coarse *simulation* resolution trap (memory: "never clear collisions across mismatched resolutions") would need its own controlled design. Substituted with a reference-field `rapid grazes` probe, declared in §1.4 as a lower bound and NOT as a collision count. |
| commanded-vs-measured cusp distribution | **delivered as p50/p95 only** (not a full distribution), scallop only, on the tool-centre field. |
| curvature disagreement vs the reference | **skipped**; slope-class disagreement is reported instead. Curvature enters the result indirectly through the achieved-cusp column, which is what curvature drives. |
| Wanaka characterisation | **DEFERRED to the checkpoint session** (§1.2). |
| UnifiedFinish MidSteep band | **covered by identity, not by a separate run.** `unified_finish_mid_steep_generation_resolution` *is* `scallop_generation_resolution` (PR-3 asserts identity, not equality), so §3.1 is the MidSteep band's A/B. No separate UnifiedFinish end-to-end arm was run. |
| UnifiedFinish VerySteep waterline / Shallow raster controls | **asserted structurally.** `only_three_consumers_can_see_the_generation_resolution` walks the whole `src` tree and asserts the set of files that build a generation surface is exactly {`scallop.rs`, `ramp_finish.rs`, `steep_shallow.rs`}. The waterline and raster bands build no generation surface, so there is no API through which the H3 variable could reach them — they are resolution-insensitive by construction, and the test fails if that ever stops being true. |

### 5.3 Controls that passed

* **Ball control** — on a Ø3 ball `cusp_radius_mm() == envelope_radius_mm()`, so
  `LegacyEnvelopeQuarter` and `CuspQuarter` resolve to the same 0.375 mm cell
  and produce **byte-identical scallop output** while keeping different modes
  and different `cell_source` tags. H3's "Ball fixtures remain unchanged where
  the policy resolves to the legacy cell size" gate, stated as an equality.
* **Determinism** — legacy arm twice, identical fingerprints (§1.5).
* **Non-vacuity** — envelope/4 and cusp/4 scallop fingerprints differ on the
  ridge; every arm's `cell_source` matches its policy.

---

## 6. RECOMMENDATION

> **Recommendation only — human decision at Checkpoint B.** Nothing in this
> section has been applied. No selector function, no shared helper and no
> default was changed by this work.

Ranked in the plan's own candidate order:

### Rank 1 — Explicit per-consumer resolution policy *(recommended)*

The strongest result in this pack is that **the three consumers respond
qualitatively differently to the same variable**:

| op | what a finer grid buys | what it costs | verdict |
|---|---|---|---|
| RampFinish | eliminates a 2.39 mm and a 0.16 mm gouge outright, on 2 of 4 fixtures, **at the intermediate cell already** | 1.3–2.6× gen time at 0.306 mm; move count +11–43% | **move it** |
| Scallop | achieved cusp from 138–157% of the dial down to on-dial | 2.0–2.7× time, 2.3–2.9× moves at 0.306 mm — and **at cusp/4 it creates 19–33 mm² of standing material that the legacy cell did not** | **hold; gated behind Checkpoint C** |
| SteepShallow | nothing measurable — every residual quantile identical, output converges above 0.3 mm | 2.9–7.6× | **hold; needs a discriminating fixture first** |

No single global cell is correct for all three. That is precisely the case
per-consumer policy exists to express, and PR-3 already shipped the mechanism,
so adopting it costs one edit per selector function and moves nothing else.
Suggested *values* for the human to accept or reject:

* **RampFinish → the intermediate scale.** Either a new named mode (a
  geometric mean of envelope/4 and cusp/4 is honest about being a compromise)
  or an explicit op-owned cell. The gouge elimination is a safety result, the
  cost is small, and the move count barely moves.
* **Scallop → HOLD at `LegacyEnvelopeQuarter`** until Checkpoint C fixes the
  `ring_stepover` min-across-ring collapse and the flat-ground `max_rings`
  budget. Moving scallop first would trade a cusp overshoot for uncut material,
  which is the worse defect.
* **SteepShallow → HOLD**, and add a fixture whose steep/shallow boundary has
  structure at the 0.5–1.5 mm scale before re-testing.

### Rank 2 — Local/adaptive slope sampling along rings/contours

Second because §5.1 shows scallop's Z is *already* exact per-point; only the
stepover dials read the grid. A local slope/curvature probe at ring points
would deliver fine-grid stepover selection **without** the global grid — the
36× cell cost and the 190 MB real-part figure both disappear. It is ranked
below (1) only because it is a new algorithm, it is scallop-specific, and it
would still expose the ring-budget truncation, so it too is gated behind
Checkpoint C. If Checkpoint C lands first, this becomes the strongest
scallop-side option.

### Rank 3 — Split coverage and differential fields

The data say the right thing is being split: the **differential** channel
(slope/curvature → labels 10.6–15.9% wrong; achieved cusp 38–57% over dial) is
what needs the fine cell, while the **coverage** channel (grid extent, the
`covered` mask, envelope padding) shows no measured resolution sensitivity.
Ranked third only because it is the general form of (2) with more machinery,
and nothing measured here yet requires the generality.

### Rank 4 — Globally cusp-scaled generation grid *(not recommended)*

Measured cost: 32.6–38.4× surface build, 2.9–11.7× whole-op time, ~4× scallop
move count, ~190 MB projected surface memory on a 300 × 200 mm part — and on 2
of 4 fixtures it makes scallop **strictly worse** (0 → 19–33 mm² standing
material). It is the plan's own "correctness-first fallback", and this evidence
does not promote it. Revisit only if Checkpoint C removes the truncation and
(2)/(3) both fail their quality gates.

### What must NOT be done

Change the formula inside the shared helper. §2 shows the label error is real
and large, which makes "just make it finer everywhere" tempting; §3.1 shows
that exact move is a regression for the one op with the strongest fidelity
instrument.

---

## 7. Open questions for the human

1. **Is Checkpoint C a hard prerequisite for any scallop resolution change?**
   At cusp/4 scallop leaves 19.3 mm² (ridge) and 33.2 mm² (ribbon) standing
   where envelope/4 leaves none. Alternatively: should a `max_rings` budget
   derived from the *selected* rather than the flat-ground stepover land inside
   H3, since it is the thing blocking the H3 decision?
2. **Achieved cusp at envelope/4 is 138–157% of the commanded dial on every
   fixture.** Is that the mechanism behind the P2.f "the dial reads ~3× off"
   history? If so, the quality case for a finer scallop grid is materially
   stronger than the residual columns suggest, and this number — not residual —
   should be the scallop acceptance gate.
3. **RampFinish's win arrives at 0.306 mm, not at cusp/4.** Do you want a third
   named mode (keeps provenance, but names a compromise), or an op-owned
   `Explicit` cell (honest that no tool scale justifies it)?
4. **SteepShallow's labels are 10.6–13.3% wrong yet its output is unchanged.**
   Fixture limitation or genuine insensitivity? Should H3 explicitly defer this
   consumer with a written reason rather than silently leaving it on legacy?
5. **The 0.100–0.125 mm band is unstable for scallop.** A 20% cell change there
   moves standing material by 12–35 mm² and achieved cusp by 2×. If either
   `CuspQuarter` or a tolerance-floor policy is ever made a default, that
   sensitivity needs to be understood first — right now `max(cusp/4, tolerance)`
   and `tolerance` land within 20% of each other on the shipped tool but do not
   produce comparable results.
6. **Memory**: is ~190 MB of finish surface acceptable on a 300 × 200 mm part,
   or does that alone retire candidate 4?
7. **Wanaka**: do you want the read-only characterisation before the decision,
   or is the synthetic evidence sufficient to choose a direction and validate
   on wanaka afterwards?

---

## 8. Adjacent defects found (logged, not fixed)

1. **`steep_shallow` emits 0.9 µm cutting segments** on the mixed-slope ribbon
   at *every* resolution (`min seg 0.0009 mm`). Degenerate G-code, unrelated to
   H3.
2. **RampFinish gouges 4.2 mm on `patches + hole` at every resolution** (45–67
   samples past 50 µm) — the ramp descends into a 62° cone the tool profile
   cannot enter. A genuine reach failure with **no diagnostic channel**; a user
   would ship it.
3. **Region count is scale-sensitive for band-shaped classes.** The 0.05 mm
   reference fragments the domes' 45° annulus into 39 components where every
   arm sees 3–4. §A.0's topology invariant is still the right one, but a gate
   built on it needs a minimum-component size or an annulus-aware reading.
4. **`ScallopReport` carries `uncut_core_mm2` but no ring count.** The harness
   had to count `ScallopRuntimeAnnotation`s to report rings — the cascade's own
   report should carry the number that explains the residual.
5. **The scallop residual trap** (§5.1): a sampled reference field cannot score
   a path whose Z is an exact drop-cutter query. Worth writing into
   `MEASUREMENT_DOMAINS.md` before someone builds a gate on it.
6. **PR-3's tolerance-floor / `comparable_to` false negative is still live** —
   two policies with different modes but a bound tolerance floor resolve to the
   same cell yet compare as different sources.
7. **`SteepShallow` classifies on its own envelope-scaled generation grid**
   (PR-3 already noted this): §2 quantifies the consequence — on the narrow
   ridge and valley it decides "there is no very-steep territory here" when the
   truth has 8 and 4 regions.

---

# ADDENDUM — 2026-07-30 — `max_rings` experiment + SteepShallow deferral

Added by the H3 wave (PR-8c), under the approved Checkpoint B. **Evidence
only: no production scallop behaviour changed in this wave, whatever the
numbers below say.** Adopting a scallop ring budget is its own decision and
belongs with Checkpoint C context (M4).

HEAD at time of run: `074789e` (post PR-8a / PR-8b).
Instrument: `checkpoint_b_resolution_ab::max_rings_budget_experiment`
(`#[ignore]`, 238 s DEBUG) plus its fast non-vacuity guard
`the_three_ring_budgets_are_three_different_numbers`.

Reproduce:
`cargo test -p rs_cam_core --test checkpoint_b_resolution_ab max_rings_budget_experiment -- --ignored --nocapture`

## A.1 What was asked

Checkpoint B's ruling: *"`max_rings` budget derived from SELECTED stepover
as an H3-scoped EXPERIMENT first (adopt only if standing → 0 with no
over-cut regression; remember v3: naive cap raise was 34× worse)."*

§3.1 finding 3 is the target: at `cusp/4` scallop leaves **19.32 mm²**
standing on the narrow ridge and **33.24 mm²** on the mixed ribbon where
`envelope/4` leaves none, because `max_rings` is budgeted from the
FLAT-GROUND (widest) stepover while the ring loop selects a smaller one per
ring on sloped terrain.

## A.2 The three budgets

New research seam `ScallopRingBudget` (additive; every production entry point
resolves to `FlatGroundStepover`, so no default moved). On the shipped
Ø1-tip / 7° / Ø6-shank taper at a 0.020 mm commanded cusp:

| budget | stepover the cap is sized from | vs shipped |
|---|---|---|
| `FlatGroundStepover` — **shipped** | 0.2800 mm | 1.00× |
| `ReachPolicyStepover` — the ruling's candidate | 0.2500 mm | **1.12×** more rings |
| `LoopClampFloor` — v3's naive raise, as CONTROL | 0.0250 mm | 11.2× more rings |

`ReachPolicyStepover` reads "the SELECTED stepover" through
`reach::suggested_offset_stepover_mm` — PR-6a's canonical answer to "how far
apart may two passes of this cutter sit" — rather than as the loop's
`cusp_r * 0.05` clamp FLOOR, which is exactly the naive raise v3 measured.
The control is included so the v3 result is *reproduced on these fixtures*
rather than cited.

## A.3 Results — cusp/4 arm, all four fixtures

| fixture | budget | gen s | rings | moves | **uncut core mm²** | deepest gouge | gouge>50µm |
|---|---|---|---|---|---|---|---|
| narrow ridge | flat-ground (shipped) | 15.66 | 51 | 8042 | **19.32** | −0.0846 | 202 |
| | reach policy | 16.14 | 55 | 8330 | **12.79** | −0.0846 | 218 |
| | loop clamp floor (v3 control) | 17.41 | 70 | 8908 | **0.00** | −0.0846 | 276 |
| narrow valley | flat-ground (shipped) | 12.66 | 48 | 6041 | 0.00 | −0.0783 | 9 |
| | reach policy | 12.78 | 48 | 6041 | 0.00 | −0.0783 | 9 |
| | loop clamp floor | 12.65 | 48 | 6041 | 0.00 | −0.0783 | 9 |
| mixed-slope ribbon | flat-ground (shipped) | 17.56 | 51 | 8107 | **33.24** | −0.0248 | 0 |
| | reach policy | 18.29 | 55 | 8458 | **25.78** | −0.0255 | 0 |
| | loop clamp floor (v3 control) | 21.00 | 79 | 9651 | **0.00** | −0.0731 | 1 |
| patches + hole | flat-ground (shipped) | 10.59 | 40 | 4755 | 0.00 | −0.0217 | 0 |
| | reach policy | 10.20 | 40 | 4755 | 0.00 | −0.0217 | 0 |
| | loop clamp floor | 10.29 | 40 | 4755 | 0.00 | −0.0217 | 0 |

`ring_count == cascade_ring_count` on **every** row (wave D3's two counters):
the cascade is being TRUNCATED by the cap, not producing rings that a keep
predicate then discards. On the two fixtures where nothing stands, all three
budgets emit identical output — the cap never binds there, so those rows are
controls, not results.

**Over-cut caveat, load-bearing here.** §5.1 applies: scallop's ring Z is an
exact per-point drop-cutter query, so the residual column is dominated by the
0.05 mm REFERENCE field's own interpolation error at whatever points the path
visits, and a denser path visits more sharp-feature cells. It is usable to
detect a LARGE regression (v3's 34×) and must not be read as an absolute
over-cut count. Ring count, move count and generation time carry the rest.

## A.4 Verdict — **REJECT `ReachPolicyStepover`**

The ruling's adopt condition was *standing → 0*. It is not met, and not
nearly:

* narrow ridge 19.32 → 12.79 mm² (−34%), mixed ribbon 33.24 → 25.78 mm²
  (−22%). Material still stands on both.
* Cost is small (+3% time, +4% moves, gouge>50µm 202 → 218 on the ridge) —
  but a 34% reduction in a defect the ruling wanted eliminated is not worth
  a production behaviour change that has to be re-validated on wanaka.

**The arithmetic says why it could never have worked.** The cap sizes as
`(max_extent / stepover) · 0.5 + 10`. `ReachPolicyStepover` is 0.25 mm
against the flat-ground 0.28 mm — a **12%** wider budget, 51 → 55 rings. The
control shows the cascade *collapses naturally at 70 rings* on the ridge and
79 on the ribbon, i.e. the requirement is **1.4–1.6×** the shipped budget.
No tool-scaled stepover lands there, because the shortfall is not a tool
scale: it is `ring_stepover` taking the MIN across a whole ring, so one steep
sample sets the advance for every ring that touches slope. A budget derived
from any *nominal* stepover is describing a spacing the loop does not use.

**The control cannot be adjudicated on these fixtures, and that is itself a
finding.** `LoopClampFloor` drives standing to 0.00 everywhere for +11–20%
time and +11–19% moves, and the ribbon's deepest gouge grows 3× (−0.0248 →
−0.0731 mm) with gouge>50µm on the ridge +37% (202 → 276). That is a
*directional* match to v3's regression but nowhere near its magnitude (+92%
time, 34× over-cut on wanaka ×2). The reason is fixture scale: these are
16 × 16 mm height fields where the cascade collapses at ~70 rings, while
v3's uncapped run produced *thousands* of rings at ~25 µm spacing on real
relief. **These fixtures do not have the power to test the naive raise**, so
nothing here weakens v3's measured result and nothing here should be used to
argue for it.

**Recommendation, unchanged from §6 and now with numbers behind it: the fix
is in `ring_stepover`, not in the budget.** Per-segment advance instead of
min-across-ring (plus the chord refinement v3 identified) removes the reason
the cascade crawls; the cap then stops binding on its own. `scallop.rs`
already documents this in place. A budget change is a workaround for a
mechanism nobody has fixed, and the best of the two candidates tested here
recovers a third of the defect.

**No production change was made.** `ScallopRingBudget::FlatGroundStepover`
remains the only value any production caller passes, and PR-3's scallop
fingerprint is unchanged.

## A.5 SteepShallow — the written deferral (Open question §7.4)

§7.4 asked: *"SteepShallow's labels are 10.6–13.3% wrong yet its output is
unchanged. Fixture limitation or genuine insensitivity? Should H3 explicitly
defer this consumer with a written reason rather than silently leaving it on
legacy?"*

**Deferred, with the reason written into the code** — see the doc comment on
`steep_shallow::steep_shallow_generation_resolution`, which is where a
future reader looks and where prose in a planning file would not be found.
It states: the measured zero delta (§3.3 — every residual quantile
bit-identical on all four arms, output converging above 0.3 mm, 2.9–7.6×
cost to move); the measured label error that does NOT absolve it (§2.2 —
10.6–13.3% wrong, VerySteep absent on two fixtures where the truth has 8 and
4 regions); and the two readings that fit the dissociation.

### Investigation note: what a discriminating fixture must do

All four Checkpoint B fixtures share a property that makes them unable to
answer §7.4: **contiguous steep territory far wider than a 0.75 mm cell**.
The op dilates the steep region by `overlap_distance` (2.0 mm default, ~2.7
legacy cells) and erodes the shallow region by `wall_clearance` (1.0 mm),
so a mislabelled boundary cell is absorbed before it can reach an emitted
move. The label error is real and lands where the morphology hides it.

A fixture that would discriminate:

* **Interdigitated steep fingers narrower than `overlap_distance`** — e.g.
  alternating 45°/85° ribs at ~1.0–1.5 mm pitch. A cell that mislabels one
  rib cannot be dilated away, because the dilation reaches its neighbour.
* **Isolated very-steep islands smaller than one legacy cell**, so the class
  either exists or does not (§2.1 already shows envelope/4 finding ZERO
  VerySteep regions where the truth has 8 — but on fixtures where the
  waterline half of the op has plenty of other territory to cut).
* Scored on **which half emitted the moves**, not only on residuals: the
  `SteepShallowSplit` move ranges are the op's own topology report and are
  the signal §3.3's residual column cannot carry.

Until that fixture exists and is run, moving this consumer would be changing
a default on the strength of an experiment that could not have detected the
change. Filed as an H4 ledger item.
