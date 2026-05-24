# Wanaka project — toolpath review and suggester sanity check

**Project:** `~/Downloads/wanaka100/wanaka_full_tuned.toml`
**Date:** 2026-05-23
**Purpose:** catalogue every toolpath's cutting params, compare against published wood-CNC community references, then run the in-tool optimizer (`optimize_toolpath` / "Suggest") on each path to assess whether its recommendations are legitimate.

---

## 1. Machine + tooling envelope

**Machine — Generic Wood Router**
- Spindle: variable, **8 000–24 000 RPM**
- Power: **0.8 kW** ConstantPower (effective `available_kw` = 0.6 after 0.75 safety factor)
- Max feed: **4 000 mm/min**
- Rigidity factors: `adaptive_doc_factor 1.5`, `adaptive_woc_factor 0.2`, `doc_finishing_factor 0.08`, `doc_roughing_factor 0.2`, `woc_finishing_mm 0.5`, `woc_roughing_max_mm 5.0`

**Tools**
- **T3 — 6 mm 2-flute end mill** (corner R 2 mm, cutting L 25 mm, taper half 15°)
- **T2 — Tapered Ball Nose** (2 mm tip, 7° half-angle taper, 2-flute, 6 mm shank, cutting L 25 mm)

---

## 2. Toolpath catalogue (live, as loaded)

Chipload formula used in this table: `feed ÷ (RPM × flute_count)` — the standard CAM input / catalog quoting convention.

| # | Name | Op | Tool | RPM | Feed | Chipload (fz, nominal) | DPP / depth | Stepover | Plunge | Stock-leave | Enabled |
|---|---|---|---|---|---|---|---|---|---|---|---|
| 0 | Pin Drill | alignment_pin_drill | T3 6 mm EM | (post default) | 300 | n/a (Z-only) | peck 3.0 mm | — | 300 | — | ✓ |
| 1 | Back Rough | adaptive3d / AgentSearch | T3 6 mm EM | **12 194** | 4 000 | **0.164 mm/t** | 3.0 mm (50 % D) | **2.53 mm (42 % D)** | 750 (19 %) | ax 4.0 / rad 0.5 | ✓ |
| 2 | Holes | drill (peck) | T3 6 mm EM | (post default) | 300 | n/a (Z-only) | depth 12 mm, peck 3 mm | — | 300 | — | ✓ |
| 3 | Rivers copy | project_curve | T3 6 mm EM | — | — | — | — | — | — | — | ✗ |
| 4 | Rivers (back) | project_curve | T2 2 mm taper ball | 18 000 | 2 500 | **0.069 mm/t** | 0.1 mm | spacing 0.5 | 150 (6 %) | — | ✓ |
| 5 | Lakes (inside) | project_curve | T2 2 mm taper ball | 18 000 | 3 000 | **0.083 mm/t** | 0.3 mm | spacing 0.5 | 150 (5 %) | — | ✓ |
| 6 | 3D Rough 6 | adaptive3d / AgentSearch | T3 6 mm EM | **12 194** | 4 000 | **0.164 mm/t** | 2.6 mm (43 % D) | **2.20 mm (37 % D)** | 500 (12.5 %) | 0.5 / 0.5 | ✓ |
| 7 | 3D Finish 6 | drop_cutter | T2 2 mm taper ball | 21 000 | 3 000 | **0.071 mm/t** | per surface | **0.30 mm (15 % of tip)** | 150 (5 %) | — | ✓ |

---

## 3. Community references (wood CNC, hobby spindle)

### 3.1 6 mm 2-flute carbide end mill in wood

| Source | Chipload range | Notes |
|---|---|---|
| Cutter Shop chart | 0.08–0.20 mm/tooth hardwood | 1/4" 2-flute |
| ShopBot Feeds & Speeds PDF | ~0.10 mm/t conservative for 6 mm | starting point |
| Adam's Bits | 0.12–0.18 mm/t at 18 k RPM | hardwood |

**Stepover / DOC (adaptive HSM)** — CNCCookbook HSM guide & Practical Machinist
- Stepover: **5–15 %** of diameter for true adaptive
- Axial DOC: up to **1–3 × D** when stepover is light
- Traditional pocket: stepover 30–50 % D, axial ≤ 0.5 × D

**Recommended RPM for 2-flute router bit:** ~18 000 (Cutter Shop, Adam's Bits)
**Ramp angle for adaptive entry:** 2–5° typical (Stepcraft)
**Plunge feed:** ≤ 50 % horizontal feed for spiral center-cutters; 400–800 mm/min for 6 mm in wood

### 3.2 2 mm tapered ball nose finishing

| Source | Stepover | Notes |
|---|---|---|
| Shapeoko / Carbide3D | 8–10 % of tip dia | typical finishing (≈ 0.16–0.20 mm on 2 mm tip) |
| Same | 4 % | super-smooth finish |
| Various forums | 15 % | coarse, post-sanded |

**Chipload:** typically 0.03–0.07 mm/t at 18–24 k for small-tip 2-flute carbide ball nose.

### 3.3 Drill cycles in wood

| Source | Peck depth | Plunge feed |
|---|---|---|
| Vectric forum | up to ~1× dia (6 mm for 6 mm tool) is fine | — |
| Stepcraft | — | 400–800 mm/min, ≤ 10 mm/s |
| Adam's Bits | — | reduce 50 % of horizontal for center-cutting bits |

---

## 4. Per-toolpath review vs community references

### 4.1 Back Rough (TP1) & 3D Rough 6 (TP6) — adaptive3d AgentSearch, T3

| Param | Ours | Community | Verdict |
|---|---|---|---|
| Chipload (nominal) | 0.164 mm/t | 0.08–0.20 mm/t hardwood | **In range, upper half** |
| Spindle RPM | **12 194** | ~18 000 typical | **Low** — unusual; OEM charts cluster at 18 k |
| Stepover | 37–42 % D | 5–15 % (HSM) / 30–50 % (traditional) | **Traditional regime**, not HSM despite operation name |
| Axial DOC | 0.43–0.50 × D | up to 1–3 × D for HSM | **Conservative axially** for the stepover used |
| Plunge | 12–19 % of feed | ≤ 50 % center-cutting | Conservative-safe |
| Ramp angle | 10° | 2–5° typical | **Steep** but forgiving in wood |

**Net:** Despite the name `adaptive3d`, the configured stepover puts these in *traditional pocketing* engagement. Cuts are within community-quoted feed/RPM envelope but well outside the "HSM adaptive" character (light radial / deep axial).

### 4.2 Rivers (TP4) — project_curve, T2 2 mm taper ball, depth 0.1 mm

| Param | Ours | Community | Verdict |
|---|---|---|---|
| Chipload (nominal) | 0.069 mm/t | 0.03–0.07 mm/t | **Top edge** |
| RPM | 18 000 | 18–24 k | In range |
| Plunge | 6 % of feed | ≤ 50 % | **Very conservative** |

Real chip thickness is much smaller than nominal because the cut depth (0.1 mm) is well below the ball radius — sane.

### 4.3 Lakes (TP5) — project_curve, T2 2 mm taper ball, depth 0.3 mm

| Param | Ours | Community | Verdict |
|---|---|---|---|
| Chipload (nominal) | 0.083 mm/t | 0.03–0.07 mm/t | **Above range** — slight |
| RPM | 18 000 | 18–24 k | In range |
| Plunge | 5 % | ≤ 50 % | Very conservative |

If chatter shows up on Lakes, trim feed to 2 500.

### 4.4 3D Finish 6 (TP7) — drop_cutter, T2 2 mm taper ball

| Param | Ours | Community | Verdict |
|---|---|---|---|
| Chipload (nominal) | 0.071 mm/t | 0.03–0.07 mm/t | **At/above the upper edge** |
| RPM | 21 000 | 18–24 k | In range |
| Stepover | 0.30 mm (15 % of tip) | 4 % (super-smooth) / 8–10 % (typical) / 15 % (coarse) | **Coarse-finish bin** — expect visible cusp |
| Plunge | 5 % | ≤ 50 % | Very conservative |

### 4.5 Pin Drill (TP0) & Holes (TP2) — drill cycles, T3

| Param | Ours | Community | Verdict |
|---|---|---|---|
| Peck depth | 3.0 mm (0.5 × D) | up to 1.0 × D ≈ 6 mm | **Conservative** |
| Plunge feed | 300 mm/min | 400–800 mm/min | **Slow** — could raise |

---

## 5. Tool-load report verdicts (current state)

Result of `run_simulation` + `get_tool_load_report`. **Summary: 5 within, 2 not applicable (drill), 0 exceeds.**

| # | Toolpath | Chipload (tool) | Bounds (LUT) | Deflection peak | Power peak | Verdict |
|---|---|---|---|---|---|---|
| 0 | Pin Drill | n/a (drill) | — | — | — | unmodeled; drill gates Within |
| 1 | Back Rough | 0.0384 mm/t | 0.032–0.055 (vendor_lut) | 146 µm | 0.039 kW / 0.6 avail | **Within** (entry spike 266 µm noted) |
| 2 | Holes | n/a (drill) | — | — | — | unmodeled; drill gates Within |
| 4 | Rivers | 0.0046 mm/t | 0.0044–0.0088 (LUT extrap ×0.44) | 12 µm | 0.0018 kW | Within |
| 5 | Lakes | 0.0055 mm/t | 0.0044–0.0088 (LUT extrap ×0.44) | 11 µm | 0.0019 kW | Within |
| 6 | 3D Rough 6 | 0.0384 mm/t | 0.032–0.055 (vendor_lut) | 145 µm | 0.040 kW | Within |
| 7 | 3D Finish 6 | 0.0047 mm/t | 0.0037–0.0075 (LUT extrap ×0.37) | 4.5 µm | 0.0008 kW | Within |

### Important: nominal vs chip-thinned chipload

The tool's `chipload` metric (e.g. 0.0384 mm/t on Back Rough) is **not** the nominal `feed/(RPM×flutes)` chipload most catalogs quote. It is the **arc-fit / engagement-aware** chip thickness, which the per-kinematics breakdown also exposes as `peak_chip_thickness_mm ≈ 0.069` on roughing.

- Nominal `feed/(RPM×flutes)` for Back Rough = 0.164 mm/t (matches `average_mean_chip_thickness_mm` in the per-kinematics breakdown).
- Tool's reported "observed" chipload = 0.038 mm/t (engagement-corrected median).
- Tool's LUT bound (0.032–0.055) is in the same engagement-corrected units.

**These two scales are internally consistent**, but a user reading "Back Rough at 0.038 mm/t, LUT max 0.055" must understand this is **not** the same scale as Amana/Onsrud catalog feed-per-tooth numbers. A reader who tries to cross-reference the tool's number against an OEM chip-load chart will see a 4× discrepancy and conclude the tool is wrong. It isn't — but it should label the units.

---

## 6. Optimizer ("Suggest") outputs per toolpath

Outcome of `optimize_toolpath` on each enabled path. All candidates are sim-verified end-to-end.

### TP0 Pin Drill — **SKIPPED**
Reason: `steady_state_samples_not_present` — Z-only kinematics, no continuous engagement.
**Verdict: legit.** Drill cycles are correctly identified as unoptimizable by this load-based optimizer.

### TP1 Back Rough — **NO_SAFE_IMPROVEMENT**
Reason: `bipolar_engagement`.
Narrative:
> "steady-state chipload samples straddle the LUT chipload range (some below the burn floor, some above the breakage ceiling) — no single feed/RPM clears both extremes. lower stepover or raise depth-per-pass to reduce engagement variance across the toolpath."

**Verdict: legit and useful.** Rather than recommend a random tweak, the optimizer reports *why* the toolpath isn't improvable by feed/RPM alone, and points the user at the right geometric lever (stepover/DPP). That's good UX.

### TP2 Holes — **SKIPPED**
Same as TP0. **Legit.**

### TP4 Rivers — **NO_SAFE_IMPROVEMENT (no_improvement_found)**
Tried 1 candidate: feed 2 917 mm/min @ 21 000 RPM (same chipload). Beat baseline by < 0.5 s; rejected.
**Verdict: legit conservatism.** A noise-level cycle-time delta is not worth a recommendation.

### TP5 Lakes — **NO_SAFE_IMPROVEMENT (no_improvement_found)**
Tried 1 candidate: feed 3 500 mm/min @ 21 000 RPM. Chipload moved *worse* (became Exceeds-low by 8 %). Rejected.
Suggested levers reported back to the operator: `raise feed > 3 977 mm/min` (but max feed is 4 000), `cap RPM ≤ 18 437`.
**Verdict: legit.** Optimizer correctly rejected a candidate that worsened the chipload gate even though it was technically faster.

### TP6 3D Rough 6 — **NO_SAFE_IMPROVEMENT**
Same `bipolar_engagement` story as Back Rough.
**Verdict: legit.**

### TP7 3D Finish 6 — **RANKED** — recommends 4 candidates
| Stage | Feed | RPM | **Stepover** | Cycle time | Δ vs baseline |
|---|---|---|---|---|---|
| baseline | 3 000 | 21 000 | **0.30 mm** | **815 s** | — |
| refined (rec_idx=3) | 3 000 | 21 000 | **1.00 mm** | 241 s | **−70.4 %** |
| refined | 3 000 | 21 000 | 0.747 mm | 318 s | −60.9 % |
| refined | 3 000 | 21 000 | 0.39 mm | 625 s | −23.3 % |

All four candidates report all gates `Within`.

> Headline: *"Found 3 candidates that improve on the baseline. Best: -70.4% cycle time at 241.0s vs 815.0s baseline."*

**🚩 This is the bullshit detector.** Mechanically the candidate is valid — chipload, deflection and power all stay Within. **But the optimizer does not model surface finish / scallop height.** Stepover on a finishing pass is *defined by* the target cusp height, not by chipload.

For a 2 mm-tip tapered ball, scallop height with stepover `s` (on a near-flat surface) ≈ `s² / (8 × r_eff)`. With r ≈ 1 mm:
- 0.30 mm stepover → cusp ≈ 11 µm — fine finish
- 1.00 mm stepover → cusp ≈ **125 µm** — visible, ridged, sand-required
- on slopes the cusp grows further (geometry dependent)

The optimizer is suggesting an order of magnitude worse surface finish in exchange for 70 % time savings — a tradeoff the operator should be making knowingly. **Legit as a load-only optimizer; BS as an unqualified "Suggest" for a finishing operation.**

Same caveat applies to the 0.747 mm and 0.39 mm refined candidates — all violate community finishing-stepover norms (4–15 % of tip = 0.08–0.30 mm on a 2 mm tip).

---

## 7. Honesty scorecard — is the suggester legit?

| Output | Verdict |
|---|---|
| Drill ops correctly skipped (no steady state) | ✅ Legit |
| Roughing ops report `bipolar_engagement` with prescriptive levers | ✅ Legit — useful diagnostic, not a fake recommendation |
| Refused noise-level cycle-time deltas (Rivers) | ✅ Legit conservatism |
| Refused candidates that worsened a gate (Lakes) | ✅ Legit |
| `optimize_toolpath` on finishing recommends 3.3× stepover, 70 % cycle-time cut | ❌ **BS for finishing** — load gates are satisfied, but surface finish (the actual purpose of a finishing pass) is not modeled. Headline reads like a free lunch; in practice it's a quality regression. |
| Chipload numbers in `tool_load_report` use chip-thinned units but no label | ⚠️ **Confusing** — a user cross-referencing OEM catalog feed-per-tooth will see a ~4× discrepancy and lose trust. The math is right; the labeling is misleading. |

### Three things to fix

1. **Gate the finishing-op optimizer on a scallop-height / surface-finish constraint** (or at minimum, surface a `scallop_height_mm` field on the candidate verdict with a finish-quality warning when it grows past 50 µm). Without this, the optimizer happily recommends roughing-grade stepovers for finishing passes.
2. **Label units in the tool-load report.** Make it explicit whether `observed_mm_per_tooth` is nominal feed-per-tooth or engagement-corrected chip thickness. A `units: chip_thinned` field or a "this is 4× lower than OEM catalog numbers because of chip thinning" tooltip would close the credibility gap.
3. **Roughing optimizer for bipolar engagement** could go further: rather than only suggest *axes* to change (stepover / DPP), simulate a couple of variants and rank — that would let it actually recommend, e.g., stepover 1.5 mm + DPP 4.5 mm for Back Rough.

### Two headline takeaways for the Wanaka project specifically

- The "adaptive3d" roughing ops are configured as **traditional pocketing** (37–42 % stepover). If you want HSM-character cuts, drop stepover toward 0.6–0.9 mm; the optimizer will then have something to chew on.
- **3D Finish 6 stepover 0.30 mm is in the coarse-finish bin** for a 2 mm-tip ball. Take the optimizer's "1.00 mm" suggestion only if you're planning to sand. For presentation-grade surface, drop to **0.15 mm**.

---

## 8. Suggested → sim → optimize: the full chain

Goal: apply the community-suggested values from §4 to a fresh copy of the project, regenerate, re-run the sim and tool-load model, and finally re-run the optimizer. This shows what *our tool* thinks of *the community's advice*.

### 8.1 Applied changes

| Toolpath | Param | Original | Community-suggested | Rationale (§4) |
|---|---|---|---|---|
| TP0 Pin Drill | feed_rate | 300 | **500** | Stepcraft / Adam's Bits — 400–800 mm/min plunge feed norm |
| TP1 Back Rough | spindle_rpm | 12 194 | **18 000** | Cutter Shop / Adam's Bits — 18 k typical for 2-flute |
| TP1 Back Rough | feed_rate | 4 000 | **3 600** | Keeps nominal fz = 0.10 mm/t at the new RPM |
| TP1 Back Rough | stepover | 2.53 | **0.90** | CNCCookbook HSM — 15 % of D for adaptive |
| TP1 Back Rough | ramp_angle_deg | 10 | **4** | Stepcraft — 2–5° typical |
| TP2 Holes | feed_rate | 300 | **500** | Same as TP0 |
| TP5 Lakes | feed_rate | 3 000 | **2 500** | Bring chipload back to community range |
| TP6 3D Rough 6 | spindle_rpm | 12 194 | **18 000** | Same as TP1 |
| TP6 3D Rough 6 | feed_rate | 4 000 | **3 600** | Same as TP1 |
| TP6 3D Rough 6 | stepover | 2.20 | **0.90** | Same as TP1 |
| TP6 3D Rough 6 | ramp_angle_deg | 10 | **4** | Same as TP1 |
| TP7 3D Finish 6 | stepover | 0.30 | **0.15** | Shapeoko / Carbide3D — typical fine-finish 8 % of tip |

TP3 disabled, TP4 already in community range, drill RPM not exposed in the schema, all other params left alone.

### 8.2 Whole-project sim metrics — original vs community

| Metric | Original | Community | Δ |
|---|---|---|---|
| Total runtime (s) | 3 442 | **4 441** | **+29 %** |
| Total moves | 122 547 | **462 264** | **+277 %** (driven by TP7 finer stepover: 113 k → 447 k) |
| Cutting distance (mm) | 71 793 | 117 094 | +63 % |
| Air-cut % | 50.4 % | 43.8 % | −13 % |
| Avg engagement | 0.188 | 0.156 | −17 % |
| Rapid collisions | 0 | 0 | — |
| Hotspots | 783 | 1 115 | +42 % |
| Verdict | WARNING (high air cut) | WARNING (high air cut) | same |

### 8.3 Tool-load report verdicts — original vs community

| # | Toolpath | Original chipload | Community chipload | Original verdict | Community verdict |
|---|---|---|---|---|---|
| 0 | Pin Drill | n/a | n/a | drill gates Within | drill gates Within |
| 1 | **Back Rough** | 0.0384 mm/t | **0.0234 mm/t** | Within | **EXCEEDS-low chipload + EXCEEDS deflection (280 µm peak vs 146 µm originally)** |
| 2 | Holes | n/a | n/a | drill gates Within | drill gates Within |
| 4 | Rivers | 0.00457 | 0.00457 | Within | Within (unchanged) |
| 5 | Lakes | 0.00548 | 0.00457 | Within | Within (mid-range now) |
| 6 | **3D Rough 6** | 0.0384 mm/t | **0.0234 mm/t** | Within | **EXCEEDS-low chipload** |
| 7 | 3D Finish 6 | 0.00470 | 0.00470 | Within | Within (chipload axes unchanged; stepover halved) |

**Summary count:** original = 5 Within / 2 n/a / **0 Exceeds**. Community = 3 Within / 2 n/a / **2 Exceeds** (3 gate failures total).

### 8.4 Optimizer re-run on the now-broken Back Rough (TP1)

```
kind: no_safe_improvement
reason: deflection_setup_locked
explanation:
  "predicted tip deflection 280 µm at peak load (above 200 µm limit) —
   feed/RPM/DOC/stepover alone can't bring this under threshold for
   this setup; shorten stickout below ~25 mm or use a stiffer tool/material"
```

The optimizer **correctly refuses** to suggest a feed/RPM/stepover fix because the failure mode is geometric (stickout × tool stiffness × engagement length) — it walks the user back to the physical setup, not a parameter tweak. ✅ Legit.

### 8.5 Why did the community advice break the tool?

The community recipe is **"drop stepover into the HSM range (5–15 % of D)"**. HSM works because constant-engagement chip thinning is *compensated* by **raising feed rate** — the principle is constant chip thickness, not constant chip per tooth. So in a fully tunable machine, you'd go: stepover 2.5 mm → 0.9 mm AND feed 4 000 → ~6 400 mm/min (a ~1.6× compensation to keep engagement-corrected chip thickness above the LUT burn floor).

But this machine's **max feed is 4 000 mm/min**. Once you're at the ceiling, dropping stepover *cannot* be compensated. The tool's load model knows this and flags the resulting chipload as below the burn-floor.

Combined with the longer tool path (more total samples in heavy engagement spots — corners, full-engagement helical entries), the peak deflection grows from 146 µm → 280 µm and tips into the Exceeds band.

### 8.6 What this proves about the tool

- The tool's chipload/deflection gates **caught** a failure mode that pure community advice (good in general) misses on this specific constrained machine. That's exactly what a load-aware CAM is supposed to do.
- The `deflection_setup_locked` narrative on TP1 is **the highest-quality optimizer output we've seen** — it diagnoses the issue, declines to fake a fix, and points the operator at the right physical knob (stickout, tool choice).
- TP7 3D Finish 6 finer stepover (0.15 mm) sailed through all gates, but **doubled total moves and added 1000 s to total runtime** — a finish-quality vs cycle-time tradeoff the operator can now see explicitly.
- 5 / 7 toolpath verdicts moved as expected when the recipe was applied. The 2 that broke are the ones the load model is *specifically designed to catch*.

### 8.7 The honest reconciliation

The original Wanaka config wasn't naive — it was **machine-aware**:
- 12 194 RPM + 4 000 feed + 2.53 mm stepover keeps engagement-corrected chipload exactly mid-LUT (0.0384, with band 0.032–0.055).
- That's what you arrive at if you start from the LUT range and work backward through the chip-thinning equation given the 4 000 mm/min feed ceiling.

So the tool isn't BSing. The **community advice is correct for typical commercial routers** (10 000+ mm/min feeds), but the original config is *more correct* for this specific machine envelope. The tool's failure to suggest "go higher RPM" on the original was the right call — that move would have pushed us into burn territory at our feed ceiling.

The one place the tool *is* BSing remains §7's finishing-pass observation: the load gates don't care about scallop, so the suggester happily recommends roughing-grade stepovers for finishing ops. Everywhere else it earned the trust.

---

## 9. Sources

- [Chip Load Chart — Cutter Shop](https://cutter-shop.com/chip-load-chart/)
- [ShopBot Feeds and Speeds (PDF)](https://shopbottools.com/wp-content/uploads/2024/01/FeedsandSpeeds.pdf)
- [CNCCookbook — DOC & Stepover](https://www.cnccookbook.com/2-tools-calculating-cut-depth-cut-widthstepover-milling/)
- [CNCCookbook — High Speed Machining guide](https://www.cnccookbook.com/high-speed-machining-speeds-and-feeds/)
- [Practical Machinist — adaptive toolpath options](https://www.practicalmachinist.com/forum/threads/need-help-with-adaptive-toolpath-options.380003/)
- [Shapeoko / Carbide3D — tapered ball nose feeds/speeds](https://community.carbide3d.com/t/speeds-and-feeds-recommendation-for-tapered-ball-nose-bits/39726)
- [Vectric forum — tapered ball nose questions](https://forum.vectric.com/viewtopic.php?t=23609)
- [Stepcraft — speeds/feeds/peck/plunge for wood](https://stepcraft.odoo.com/blog/stepcraft-blog-6/speeds-feeds-rpm-and-depth-per-pass-starting-points-for-wood-on-a-stepcraft-cnc-system-4)
- [Adam's Bits — CNC router feeds and speeds](https://www.endmill.com.au/blog/cnc-router-feeds-and-speeds-the-adams-guide/)
- [Onefinity — 1/4" EM rough feeds and speeds](https://forum.onefinitycnc.com/t/rough-pass-feed-and-speed-and-depth-for-a-1-4-em/6887)
