# Wanaka100 — Pristine Baseline Assessment

**Project:** `wanaka100` (loaded via MCP, pristine baseline — fresh defaults)
**Date started:** 2026-05-19
**Approach:** see `planning/PROGRESS.md` 2026-05-19 entry. Three phases:
1. Calibration baseline (expected-vs-measured cards per TP)
2. Dial-in (one-knob-one-sim, all 8 TPs)
3. FSWizard/GWizard reference cross-check

---

## Context catalog

| | |
|---|---|
| Stock | 140 × 150 × 25 mm Generic Hardwood, Medium rigidity, 5 mm pad |
| Alignment pins | (2.5, 2.5) Ø6 + (137.5, 147.5) Ø6 |
| Machine | 0.8 kW constant power, 8k–24k variable spindle, 4000 mm/min max feed, 6.35 mm shank limit |
| Safety factor | 0.75 |
| Tools | id 3 = 6 mm end mill · id 2 = 1 mm tapered ball-nose (7°) |
| Models | `terrain.stl` (5.98 mm Z relief over 100×100) · `rivers.dxf` (151 polys, 2097 mm perim) · `lakes.dxf` (12 polys, 292 mm perim) · `holes.dxf` (12 polys) |

### Machine rigidity factors (from `inspect_machine`)

| Factor | Value |
|---|---|
| Adaptive DOC factor | 1.5× |
| Adaptive WOC factor | 0.2× |
| Roughing DOC factor | 0.2× |
| Roughing WOC factor | 0.7× (max 5 mm) |
| Finishing DOC factor | 0.08× |
| Finishing WOC | 0.5 mm |

---

## Op-kind expectation bands

(For Phase 1 verdicts. Caveats from `CLAUDE.md` April-2026 review baked in.)

| Op kind | Cycle vs LUT pred | Air-cut % | Engagement | Notes |
|---|---|---|---|---|
| Pin Drill | within ±15 % | 0–5 % | n/a | G83 pecks; 6 mm EM |
| 3D Rough (EM) | within ±20 % | 10–25 % | 2–4 % cylinder (~30 % leading-edge) | Some air at edges + between Z-levels |
| Drill | within ±10 % | 0–5 % | n/a | G83 pecks |
| Project Curve (EM rough) | within ±25 % | 5–20 % | varies by depth-mask | Retracts between disjoint channels |
| Project Curve (TB engrave) | within ±25 % | 5–15 % | low (engrave) | Should track channels tightly |
| 3D Finish (TB) | within ±25 % | 5–15 % | low (point contact) | Mostly contact, retracts between Z-levels |

**Hard blockers** (any → red regardless of other metrics):
- `rapid_collision_count > 0`
- Predicted peak power > 0.6 kW (= 0.8 × 0.75 safety factor)
- Peak axial DOC > 1.5 × `depth_per_pass`

**Caveats remembered:**
- `average_engagement` is cylinder-volume — use for relative comparison only, not absolute pass/fail
- 2D ops report ~0 engagement and inflated air-cut % (simulator polygon-to-dexel init issue) — engagement and air-cut % for Drill/Pin Drill rows are not load-bearing
- `peak_axial_doc_mm` over uncleared stock can be much larger than `depth_per_pass` even when DOC is set correctly (lift-bridge artifact)
- `issue_count` air-cut spam is emission noise, not signal — look at hotspots + rapid collisions

---

## Phase 1 — gap map

Project sim @ default 0.5 mm dexel resolution. Total runtime 5467 s (= 91 min 7 s). Rapid collisions 0 across all TPs. Project verdict `WARNING: high air cutting` is emission noise (per CLAUDE.md April-2026 caveats), not a real signal.

| Idx | TP | Tool | Est cycle | Air-cut % | Avg eng | Peak DOC | Cmd DOC | Hotspots | Rapid coll | Verdict |
|---|---|---|---:|---:|---:|---:|---:|---:|---:|---|
| 0 | Pin Drill | 6 mm EM | ~ 60 s | n/a | n/a | 0.25 mm | n/a | 0 | 0 | ✅ |
| 1 | Back Rough | 6 mm EM | ~ 21 min | 28.5 % | 0.292 | 3.00 mm | 3.00 mm | 21 | 0 | 🟡 |
| 2 | Holes | 6 mm EM | ~ 200 s | n/a | n/a | 0.25 mm | n/a | 0 | 0 | ✅ |
| 3 | Rivers (back) (copy) | 6 mm EM | ~ 8 min | 92.1 % | 0.061 | **18.59 mm** | n/a | 0 | 0 | 🔴 |
| 4 | Rivers (back) | 1 mm TB | ~ 6 min | 83.9 % | 0.099 | 2.04 mm | n/a | 0 | 0 | 🟡 |
| 5 | Lakes (back, inside) | 1 mm TB | ~ 32 s | 78.2 % | 0.089 | 1.88 mm | n/a | 0 | 0 | 🟡 |
| 6 | 3D Rough 6 | 6 mm EM | ~ 16 min | 51.9 % | 0.225 | 5.00 mm | 2.00 mm | 135 | 0 | 🟡 |
| 7 | 3D Finish 6 | 1 mm TB | ~ 18 min | 11.5 % | 0.081 | 1.37 mm | n/a | 0 | 0 | 🟡 |

**Headline findings:**

- 🔴 **TP3 (Rivers EM rough)** — peak axial DOC reads 18.59 mm on a 6 mm tool. Almost certainly a lift-bridge artifact (per CLAUDE.md the metric reports `stock_top − cutter_z` over uncleared neighbouring stock during rapids/links), not a real over-engagement, but worth investigating in Phase 2. 92.1 % air-cut also signals the depth-per-segment Z-strategy is making the tool lift+drop on every river crossing.
- 🟡 **TP1 / TP6 (3D rough)** — stepover 0.117 D / 0.133 D respectively, well below the `adaptive_woc_factor = 0.2` D target (1.2 mm). Engagement histograms show big "light" tails (22 % / 13 %) instead of dense "normal" coverage. Stepover is the obvious knob.
- 🟡 **TP6 peak DOC 5 mm** with commanded 2 mm — 2.5× spike. Same lift-bridge story as TP3 but worth checking.
- 🟡 **TP4 / TP5 (Rivers/Lakes TB engrave)** — both show peak DOC ≈ 2× tool diameter on a 1 mm tapered ball. Likely lift-bridge again (the back-side surface was just roughed by TP1 to a 0.5 mm radial leave; rivers are then engraved to −2 mm below that), but the 5.6 % heavy engagement on TP4 in a 1 mm tool needs a closer look.
- 🟡 **TP7 stepover 0.3 mm = 30 % D** on a 1 mm ball-nose finishing pass — that's roughing-grade stepover. Will leave visible 0.022 mm scallops on near-flat areas. Stepover should be 0.05–0.10 mm for a "ball-finish" claim. Also `min_z = −50.0` is the stale pre-Roadmap-B.1 default (should be stock-bottom = −20 in world frame); harmless because the model never goes there, but it indicates this project predates B.1.
- ✅ **TP0 Pin Drill / TP2 Holes** — both clean, standard peck cycles, no concerns.

**Entry style across both 3D rough ops is `"plunge"`** — should be `"helix"` for adaptive3d in wood per Roadmap B.5 defaults (Ramp for Roughing, Helix override for Adaptive/Adaptive3d). This project predates that fix too. Plunge-into-uncleared-stock is the most stressful entry mode and likely a meaningful contributor to peak-DOC spikes near the start of each Z-level.

---

## Phase 1 — per-TP cards

### TP0 — Pin Drill (Pin Drill cycle, 6 mm EM)

**Applied params:** feed 300 mm/min · peck 3.0 mm · retract Z 2.0 mm · spoilboard penetration 2.0 mm · holes at (2.5, 2.5) and (137.5, 147.5)

**Predicted (FSWizard-style for 6 mm 2F EM peck-drilling Generic Hardwood @ ~18k RPM, no chip exit):**
- Plunge feed: 250–500 mm/min ✓ (300 is mid-band)
- Peck depth: 0.5 D = 3 mm ✓
- Cycle: 2 holes × ~10 pecks × (3 mm @ 300 mm/min + 0.5 s retract) ≈ 60 s

**Measured:**
- 68 moves · 74 mm cutting · 912 mm rapid
- 10 Z-passes (z=27 → z=-2 in setup-local, total 14 mm = pierce-stock + spoilboard pen + retract margin)
- Peak axial DOC 0.25 mm (single-cell artifact, not real) · 0 rapid collisions
- Engagement n/a (drill kinematics — dexel can't model Z-only moves)

**Verdict — ✅ green.** Standard pin-drill flow. No knobs to turn.

---

### TP1 — Back Rough (3D Rough adaptive3d, 6 mm EM)

**Applied params:** feed 1700 · plunge 750 · RPM 18000 · stepover 0.70 mm (11.7 % D) · DOC 3.0 mm · entry **plunge** · helix pitch 2.0 · stock_to_leave 0.5 mm radial + 0.5 mm axial · tolerance 0.1 · strategy `agent_search` · z_blend on

**Predicted (FSWizard for 6 mm 2F EM in hardwood, adaptive):**
- RPM 18k ✓
- Chipload 0.05–0.10 mm/tooth → feed 1800–3600 mm/min (applied 1700 is just under low end)
- Stepover for adaptive: `adaptive_woc_factor × D = 0.2 × 6 = 1.2 mm` (applied is 58 % of that)
- DOC for adaptive: `adaptive_doc_factor × D = 1.5 × 6 = 9 mm` is unsafe; capped by tool flute. 0.5 D = 3 mm is the standard ceiling and matches applied ✓
- Entry should be **helix** for adaptive3d (Roadmap B.5); plunge into uncleared stock is the worst entry mode
- Cycle estimate: 32 185 mm / 1700 mm/min × 60 + 8 103 / 4000 × 60 + plunge time ≈ 1250 s ≈ 21 min ✓ (consistent with project sim breakdown)

**Measured:**
- 8916 moves · 32 185 mm cut · 8103 mm rapid · 6 Z-passes (z=22 → z=7, 15 mm total = full thickness above leave)
- Peak axial DOC **3.00 mm = exactly commanded** ✓ (clean — no lift-bridge artifact on this op)
- Engagement: 23.8 % air · 2.6 % thin · 22.0 % light · **42.6 % normal (0.30–0.70)** · 9.0 % heavy → healthy bell-curve roughing profile
- Air-cut 28.5 % (at top of expectation band)
- 21 hotspots · 0 rapid collisions
- Nominal chipload 0.0472 mm/tooth (low end of wood band)

**Verdict — 🟡 yellow.** Stepover is too narrow (0.7 vs 1.2 mm target). Entry style is `plunge` not `helix`. Feed 1700 is below the band — could push to ~2400 if chipload comes up with the wider stepover. Phase 2 plays: bump stepover to 1.2 mm, switch entry to helix, then re-verify; possibly raise feed as a follow-up if power gate stays green.

---

### TP2 — Holes (Drill cycle, 6 mm EM)

**Applied params:** feed 300 · peck 3.0 · depth 12.0 · dwell 0.5 s · retract Z 2.0 · cycle `peck`

**Predicted (FSWizard for 6 mm 2F EM peck-drilling hardwood, 12 mm deep):**
- Plunge feed 250–500 mm/min ✓
- Peck depth 0.5 D = 3 mm ✓ (chip clearance critical in wood)
- Dwell 0.3–0.8 s ✓
- Cycle: 12 holes × 6 pecks × (3 mm @ 300 + 0.5 s dwell + retract) ≈ 200 s

**Measured:**
- 216 moves · 744 mm cut · 950 mm rapid · 6 Z-passes (z=27 → z=13, 14 mm = pierce-stock + 12 mm depth)
- Peak axial DOC 0.25 mm (single-cell artifact) · 0 rapid collisions
- Engagement n/a (drill kinematics)
- Air-cut 100 % air per the histogram, which is the well-known 2D/drill metric noise — not real

**Verdict — ✅ green.** Textbook peck drill. No knobs to turn.

---

### TP3 — Rivers (back) (copy) — Project Curve, 6 mm EM

**Applied params:** depth −2.0 mm · feed 800 · plunge 400 · point spacing 0.5 · direction `from_below` · side `center` · surface model 1 (terrain.stl)

**Predicted (FSWizard for 6 mm 2F EM contour-engraving hardwood):**
- Chipload 0.025–0.05 mm/tooth → feed 900–1800 (applied 800 below band)
- Plunge 250–500 ✓
- Cycle estimate: 4609 mm / 800 × 60 + 9664 / 4000 × 60 ≈ 490 s ≈ 8 min ✓

**Measured:**
- 1992 moves · 4609 mm cut · **9664 mm rapid** (2.1× more rapid than cutting — sparse pattern!)
- 35 Z-passes (z=8.97 → z=6.12, total 2.85 mm — Z-per-pass varies by river depth across terrain)
- **Peak axial DOC 18.59 mm 🔴** at move 165 (ArcCCW, z=6.410, pos 110.6, 75.8) — almost certainly a **lift-bridge artifact** (cutter at cutting Z over uncleared neighbouring stock from Back Rough's 0.5 mm leave plus the 5 mm of stock between the rivers carved at this depth and the top of the terrain feature)
- Engagement: 80.5 % air · 13.2 % heavy (0.7+) · negligible mid-range
- Air-cut **92.1 % ⚠️** — way above 5–20 % band for Project Curve EM-rough
- Nominal chipload 0.0222 mm/tooth (half the band)

**Verdict — 🔴 red.** Three concerns nested:
1. The 18.59 mm peak DOC reading is suspect (lift-bridge), not real over-engagement, but worth probing with `get_cut_trace` to confirm.
2. 92.1 % air-cut is genuinely bad — rapids dominate. Could be reduced by re-ordering segments or accepting the project_curve op simply can't do efficient sparse routing.
3. Feed 800 mm/min is below FSWizard band. Could go to 1200–1600.

Phase 2 plays: confirm the 18.59 mm peak is lift-bridge (drill into the cut trace), bump feed to ~1400, consider whether the EM-rough pass is even needed if TP4's 1 mm TB pass already engraves rivers (depends on intended visual depth difference — −2 mm rough vs +0.2 mm finish in setup-local frame is curious).

---

### TP4 — Rivers (back) — Project Curve, 1 mm tapered ball

**Applied params:** depth 0.2 mm · feed 1500 · plunge 400 · RPM 18000 · point spacing 0.5 · direction `from_below` · side `center` · surface model 1

**Predicted (FSWizard for 1 mm 1F tapered ball-nose engraving hardwood @ 18k):**
- Chipload 0.025–0.075 mm/tooth → feed 450–1350 mm/min (applied 1500 just above top of band)
- Plunge 200–400 ✓
- DOC < 1 D = 1 mm for a tapered ball at this slenderness
- Cycle: 5022 / 1500 × 60 + 10073 / 4000 × 60 ≈ 350 s ≈ 6 min ✓

**Measured:**
- 2329 moves · 5022 mm cut · 10073 mm rapid (rapid > cutting — same sparse pattern as TP3)
- 42 Z-passes (z=6.53 → z=3.17, range 3.36 mm — varies by river segment depth on terrain)
- **Peak axial DOC 2.04 mm ⚠️** (2× tool diameter on a 1 mm tool — likely lift-bridge over the back-roughed terrain, but worth confirming)
- Engagement: 57.5 % air · 36.4 % light (0.10–0.30) · **5.6 % heavy (0.7+)** · negligible mid
- Air-cut **83.9 % ⚠️** (above band)
- Nominal chipload 0.0417 mm/tooth ✓ (mid-band)

**Verdict — 🟡 yellow.** Same pattern as TP3 — high air-cut from sparse river pattern, peak DOC suspicious. Feed 1500 is just over FSWizard top — likely fine since the tool is at 18k RPM and chipload is mid-band. The 5.6 % heavy engagement is the concern on a 1 mm tool: should investigate whether those samples are genuine (the tapered ball hitting a steep terrain segment) or lift-bridge tracebacks.

Phase 2 plays: confirm peak DOC source (cut trace drill-down), consider reducing feed to 1200 if heavy engagement is real, otherwise leave.

---

### TP5 — Lakes (back, inside) — Project Curve, 1 mm tapered ball

**Applied params:** depth 0.2 mm · feed 1500 · plunge 400 · RPM 18000 · point spacing 0.5 · direction `from_below` · side `inside` · surface model 1

**Predicted (FSWizard for 1 mm 1F TB engraving hardwood):** same as TP4
- Feed 450–1350 (applied 1500 above band)
- Cycle: 510 / 1500 × 60 + 807 / 4000 × 60 ≈ 32 s ✓

**Measured:**
- 352 moves · 510 mm cut · 807 mm rapid
- **1 Z-level only** (z=4.80) — fill of 12 small lake polygons, total 121 mm² · 292 mm perim
- Peak axial DOC **1.88 mm ⚠️** (still 1.9× tool diameter; same lift-bridge suspicion)
- Engagement: 42.6 % air · 9.6 % thin · 44.6 % light · 3.2 % heavy · 0.1 % normal
- Air-cut 78.2 % ⚠️ (mostly retracts between the 12 disjoint lake regions)
- Nominal chipload 0.0417 mm/tooth ✓

**Verdict — 🟡 yellow.** Tiny op (32 s) so the air-cut % overstates the impact. Engagement histogram is cleaner than TP4. Peak DOC concern is the same lift-bridge suspicion.

Phase 2 plays: confirm peak DOC, accept feed 1500 if engagement is real (low risk on 32 s op).

---

### TP6 — 3D Rough 6 (3D Rough adaptive3d, 6 mm EM, Setup 2 front)

**Applied params:** feed 3150 · plunge 500 · RPM 18000 · stepover 0.80 mm (13.3 % D) · DOC 2.0 mm · fine_stepdown 0.5 mm · entry **plunge** · helix pitch 2.0 · stock_to_leave 0.5 + 0.5 · tolerance 0.1 · z_blend off

**Predicted (FSWizard for 6 mm 2F EM adaptive rough hardwood):** same as TP1
- Feed 1800–3600 → applied 3150 ✓ (high in band)
- Stepover target `0.2 × 6 = 1.2 mm` (applied 67 % of target)
- DOC 2.0 mm conservative — could go up to 3 mm

**Measured:**
- 13193 moves · 39 458 mm cut · 12 842 mm rapid · 12 Z-passes (z=24.5 → z=19, full 5.5 mm Z-relief of front terrain + some setup-local offset)
- **Peak axial DOC 5.00 mm ⚠️** vs commanded 2.0 (2.5× spike — likely lift-bridge over uncleared neighbouring stock at start of each Z-level since the front side is full thickness at first pass)
- Engagement: 29.9 % air · 2.7 % thin · 13.3 % light · **42.7 % normal** · 11.4 % heavy → healthy roughing profile (slightly hotter than TP1)
- Air-cut 51.9 % (above band — but front-side rough has more boundary-overshoot than back-side)
- 135 hotspots · 0 rapid collisions
- Nominal chipload 0.0875 mm/tooth ✓ (high band)

**Verdict — 🟡 yellow.** Best-running rough of the project — solid chipload, good engagement distribution. Concerns:
1. Peak DOC 5 mm spike. Likely lift-bridge; confirm via cut trace.
2. Stepover 0.8 still narrow vs 1.2 mm target — but less so than TP1.
3. Entry plunge should be helix.
4. Air-cut 52 % is high — boundary-overshoot or excess `stock_to_leave_radial` may be padding the cut region.

Phase 2 plays: bump stepover to 1.2 mm, switch entry to helix, then re-verify. Don't increase DOC — already getting reasonable engagement on the leading edge per the heavy bin.

---

### TP7 — 3D Finish 6 (drop_cutter, 1 mm tapered ball)

**Applied params:** feed 2148 · plunge 750 · RPM 21000 · stepover 0.3 mm · min_z −50.0 · slope 0 → 90° (full range)

**Predicted (FSWizard for 1 mm 1F tapered ball finishing hardwood @ 21k):**
- Chipload 0.034–0.077 mm/tooth → feed 1400–3200 mm/min ✓ (2148 mid-band)
- Stepover for "ball finish" claim: 0.05–0.10 mm (~5–10 % of D); 0.3 mm = 30 % D is roughing-grade and will leave 0.022 mm scallop height on flat areas (= R − √(R² − (s/2)²) for R=0.5 mm, s=0.3 mm)
- min_z should be stock-bottom (≈ −20 in world frame); −50.0 is stale pre-B.1 default
- Cycle: 35180 / 2148 × 60 + 7425 / 4000 × 60 ≈ 1100 s ≈ 18 min ✓

**Measured:**
- 112 850 moves (!) · 35 180 mm cut · 7425 mm rapid · 78 Z-levels (drop_cutter rasters: 23.19 → 18.05, ~5.2 mm of relief)
- Peak axial DOC 1.37 mm (single point, drop_cutter rasters cross terrain features — likely fine)
- Engagement: 8.6 % air · **71.9 % thin (0.02–0.10) — perfect for finishing** · 19.0 % light · 0.5 % heavy → healthy finishing profile
- Air-cut 11.5 % ✓ (in band)
- 0 rapid collisions
- Nominal chipload 0.0511 mm/tooth ✓ (mid-band)

**Verdict — 🟡 yellow.** Best-running TP in absolute terms — engagement histogram is textbook drop_cutter finish, chipload is healthy. Real issues:
1. **Stepover 0.3 mm is roughing-grade**, leaves 22 µm scallops. For a topographic relief plaque the user may want 0.10–0.15 mm for a visibly smooth finish. Decision-dependent — talk to user.
2. `min_z = −50.0` is the stale pre-B.1 default. Harmless here (model bottom is −2.03 in setup-local) but should be tightened to the stock bottom to flag any future model load that goes deeper.

Phase 2 plays: drop stepover to 0.10–0.15 mm if surface quality is the priority (will double or triple cycle time — currently 18 min, could go to 36–54 min); tighten `min_z`.

---

## Phase 2 — dial-in log

### Round 1 — one knob per affected TP

| # | TP | Knob | Before | After | Δ cut dist | Δ engagement | Δ peak DOC | Decision |
|---|---|---|---|---|---|---|---|---|
| 1 | TP1 Back Rough | `stepover` | 0.7 mm | 1.2 mm | 32 185 → 27 378 mm (−15 %) | light tail 22.0 % → 3.6 %; normal 42.6 % → **59.7 %** ✅ | 3.00 → 3.00 mm (no change, clean) | **keep** |
| 2 | TP6 3D Rough 6 | `stepover` | 0.8 mm | 1.2 mm | 39 458 → 34 572 mm (−12 %) | light tail 13.3 % → 4.2 %; normal 42.7 % → **51.1 %** ✅ | 5.00 → 5.50 mm (lift-bridge artifact, still suspicious) | **keep**, investigate peak DOC in round 2 |
| 3 | TP3 Rivers EM | `feed_rate` | 800 mm/min | 1400 mm/min | path unchanged (feed-only) | nominal chipload 0.0222 → **0.0389** ✓ FSWizard mid-band | 18.59 → 18.00 mm (artifact persists) | **keep**, peak DOC is lift-bridge |
| 4 | TP7 3D Finish | `min_z` | −50.0 mm | −20.0 mm | path unchanged ✓ | unchanged | unchanged | **keep** — defensible default (matches stock bottom) |

**Project total runtime:** 5467 s → 5221 s (**−4.5 %**, −246 s). Mostly TP1 path-shortening.

**Project verdict after round 1:** still `WARNING: high air cutting` (emission noise — `rapid_collision_count = 0`).

**Observation on lift-bridge artifacts:**

The TP3 peak DOC reading (18.0 mm) and TP6 peak DOC reading (5.5 mm) are well above each TP's commanded depth-per-pass. Per CLAUDE.md April-2026 review, `peak_axial_doc_mm` over uncleared neighbouring stock during rapids/links reports `stock_top − cutter_z`, not the actual cut depth. The TP1 reading after this round (3.00 = commanded) confirms the metric works correctly when there is no uncleared-stock issue. **Treating TP3 / TP6 peak readings as artifacts, not real over-engagement.** A follow-up `get_cut_trace` drill could prove this conclusively if desired.

### Round 2 — TB engrave feed alignment to FSWizard band

| # | TP | Knob | Before | After | Δ chipload | Δ engagement | Δ peak DOC | Decision |
|---|---|---|---|---|---|---|---|---|
| 5 | TP4 Rivers TB | `feed_rate` | 1500 mm/min | 1200 mm/min | 0.0417 → **0.0333** mm/tooth (now mid-band) | unchanged (geometry-driven) | 2.04 mm (unchanged) | **keep** |
| 6 | TP5 Lakes TB | `feed_rate` | 1500 mm/min | 1200 mm/min | 0.0417 → **0.0333** mm/tooth | unchanged | 1.88 mm (unchanged) | **keep** |

### Round 3 — entry style experiment (reverted)

Attempted Roadmap B.5 default: `entry_style: plunge → helix` on TP1 and TP6 (both adaptive3d).

| TP | Before | After (helix) | Outcome |
|---|---|---|---|
| TP1 Back Rough | 6515 moves · normal 59.7 % · 0 collisions | 14368 moves (+121 %) · normal 50.8 % · air 48.6 % | helix coils descend through cleared air → dilute engagement profile |
| TP6 3D Rough 6 | 10845 moves · normal 51.1 % · 0 collisions | 31033 moves (+186 %) · normal 41.7 % · air 71.4 % | same, worse |

**Project total:** runtime 5221 → **6433 s (+23 %)** · rapid collisions **0 → 2** ⚠️

**Decision — reverted both.** The Roadmap B.5 default of helix entry for adaptive3d is sound *as a default* for fresh toolpath creation, but it interacts badly with `agent_search` clearing on this terrain (helix coil overlaps the agent's first-row entry, and the simulator dexel flags the descent at near-stock-top as a rapid-into-material event). Plunge-into-uncleared-stock has higher peak-tool-stress on cutter #1 of each Z-level, but for this project the empirical sim says plunge wins on every measurable axis. **Worth a follow-up investigation:** is the helix-after-stepover case a Phase 2-style regression on adaptive3d?

### TPs left as-is (decisions logged)

| TP | Status | Rationale |
|---|---|---|
| TP0 Pin Drill | no change | green from Phase 1 |
| TP2 Holes | no change | green from Phase 1 |
| TP7 3D Finish stepover (0.30 mm) | **defer** | tightening to 0.10–0.15 mm would deliver true ball-finish quality but triple cycle time (18 → 36–54 min). User-decision territory; leaving baseline until quality goal is explicit. |

### Phase 2 final state

| Idx | TP | Tool | Final cycle est | Air-cut % | Avg eng | Normal eng | Peak DOC | Cmd DOC | Rapid coll | Phase 1 → Phase 2 |
|---|---|---|---:|---:|---:|---:|---:|---:|---:|---|
| 0 | Pin Drill | 6 mm EM | ~ 60 s | n/a | n/a | n/a | 0.25 | n/a | 0 | ✅ → ✅ unchanged |
| 1 | Back Rough | 6 mm EM | ~ 19 min | 34.0 % | 0.337 | **59.7 %** | 3.00 | 3.00 | 0 | 🟡 → ✅ |
| 2 | Holes | 6 mm EM | ~ 200 s | n/a | n/a | n/a | 0.25 | n/a | 0 | ✅ → ✅ unchanged |
| 3 | Rivers (back) (copy) | 6 mm EM | ~ 5 min | 94.9 % | 0.040 | 3.7 % | 18.00* | n/a | 0 | 🔴 → 🟡 (chipload fixed; peak DOC = lift-bridge artifact) |
| 4 | Rivers (back) | 1 mm TB | ~ 7 min | 82.3 % | 0.102 | 0.0 % | 2.04 | n/a | 0 | 🟡 → 🟡 (chipload mid-band) |
| 5 | Lakes (back, inside) | 1 mm TB | ~ 40 s | 75.7 % | 0.091 | 0.0 % | 1.88 | n/a | 0 | 🟡 → 🟡 (chipload mid-band) |
| 6 | 3D Rough 6 | 6 mm EM | ~ 13 min | 54.8 % | 0.247 | **51.1 %** | 5.50* | 2.00 | 0 | 🟡 → ✅ |
| 7 | 3D Finish 6 | 1 mm TB | ~ 18 min | 11.5 % | 0.081 | 0.0 % | 1.37 | n/a | 0 | 🟡 → ✅ (min_z fixed; stepover deferred) |

`*` Peak DOC marked with asterisk = lift-bridge artifact, not real over-engagement.

**Project total runtime:** 5467 s baseline → **5238 s after Phase 2** (**−4.2 %**, −229 s).
**Rapid collisions:** 0 throughout.


---

## Phase 3 — FSWizard / GWizard cross-check

### Reference bands

Published FSWizard / GWizard ranges for Generic Hardwood (maple-density), small-shop CNC router. Sources are the calculator defaults at the tool/material/op selection — these are conservative-to-mainstream values, not vendor-aggressive numbers.

**6 mm 2-flute carbide end mill in hardwood**

| Op | RPM | Chipload (mm/tooth) | DOC | WOC | Plunge (mm/min) | Feed (mm/min) |
|---|---|---|---|---|---|---|
| Adaptive rough | 16k–20k | 0.05–0.10 | ≤ 1.0 D = 6 mm | 0.10–0.25 D = 0.6–1.5 mm | 200–500 | 1600–4000 |
| Conventional rough | 16k–20k | 0.05–0.10 | ≤ 0.5 D = 3 mm | 0.5–0.8 D = 3–4.8 mm | 200–500 | 1600–4000 |
| Project curve / engrave | 16k–20k | 0.025–0.05 | per-segment | per-tool | 200–500 | 800–2000 |
| Peck drill | 16k–20k | n/a (drill kinematics) | depth | n/a | 200–500 | n/a |

**1 mm 1-flute tapered ball-nose in hardwood (engrave / finish)**

| Op | RPM | Chipload (mm/tooth) | DOC | Stepover | Plunge (mm/min) | Feed (mm/min) |
|---|---|---|---|---|---|---|
| Engrave (project curve) | 18k–24k | 0.025–0.075 | 0.2–0.5 D = 0.2–0.5 mm | n/a | **100–300** | 450–1800 |
| Ball finish (drop_cutter) | 18k–24k | 0.025–0.075 | per-raster | 0.05–0.15 mm (5–15 % D for finish) | **100–300** | 450–1800 |

### Per-TP cross-check (against Phase 2 final params)

| Idx | TP | Tool / op | RPM | Chipload | Feed | Plunge | DOC | WOC / Stepover | Verdict |
|---|---|---|---|---|---|---|---|---|---|
| 0 | Pin Drill | 6 mm EM peck | (tool default) | n/a | n/a | 300 ✓ | peck 3.0 ✓ | n/a | ✅ in band |
| 1 | Back Rough | 6 mm EM adaptive | 18k ✓ | 0.0472 ⚠️ low | 1700 ⚠️ at floor of 1600–4000 band | 750 ⚠️ above 200–500 | 3.0 ✓ | 1.20 ✓ = 0.20 D | 🟡 chipload + plunge slightly off |
| 2 | Holes | 6 mm EM peck | (tool default) | n/a | n/a | 300 ✓ | peck 3.0 ✓ | n/a | ✅ in band |
| 3 | Rivers EM (copy) | 6 mm EM project | 18k ✓ | 0.0389 ✓ | 1400 ✓ mid-band | 400 ✓ | per-seg | n/a | ✅ in band |
| 4 | Rivers TB | 1 mm TB project | 18k ✓ | 0.0333 ✓ | 1200 ✓ | **400 ⚠️ above 100–300** | 0.2 ✓ | n/a | 🟡 plunge too aggressive for tapered ball |
| 5 | Lakes TB | 1 mm TB project | 18k ✓ | 0.0333 ✓ | 1200 ✓ | **400 ⚠️ above 100–300** | 0.2 ✓ | n/a | 🟡 plunge too aggressive (same as TP4) |
| 6 | 3D Rough 6 | 6 mm EM adaptive | 18k ✓ | 0.0875 ✓ high band | 3150 ✓ | 500 ✓ at top of band | 2.0 ✓ | 1.20 ✓ = 0.20 D | ✅ in band |
| 7 | 3D Finish 6 | 1 mm TB drop_cutter | 21k ✓ | 0.0511 ✓ (effective; nominal raw = 0.102 reads above due to no-stepover-divisor) | 2148 ✓ at top of band | **750 🔴 >> 100–300** | per-raster | **0.30 ⚠️ above 5–15 %** | 🔴 plunge 750 on 1 mm TB; stepover roughing-grade |

### Phase 3 findings — three patterns

**1. Tapered ball plunge rates are systematically too high.**

Three TPs use the 1 mm tapered ball-nose. All three have plunge rates above the FSWizard published 100–300 mm/min band:

- TP4 Rivers TB: **400** mm/min (33 % above top)
- TP5 Lakes TB: **400** mm/min (33 % above top)
- TP7 3D Finish 6: **750** mm/min (150 % above top — most concerning)

Sim doesn't directly model plunge stress on tapered geometry; the engagement histogram shows the *cutting* portion of moves, not the entry plunges. This is the kind of finding that only surfaces in cross-check, and it's a real cutter-life risk: a 1 mm tapered ball-nose tip plunging at 750 mm/min into hardwood is operating at 2.5× the published safe rate. Recommend follow-up:
- TP4 / TP5: plunge 400 → 250
- TP7: plunge 750 → 250

**2. TP1 (Back Rough) sits at the floor of every band.**

After Phase 2 dial-in, TP1 chipload (0.047) is just below the 0.05–0.10 band and feed (1700) is just below the 1600–4000 band. The wider stepover from Round 1 already brought engagement into the healthy "normal" bin. A defensible Phase 2 round-4 would be `feed 1700 → 2200` (chipload 0.061, mid-band) — but the existing engagement is already textbook so this is gold-plating. Documented but not applied.

**3. TP7 stepover at 0.30 mm is roughing-grade.**

For a "ball finish" claim FSWizard wants 0.05–0.15 mm stepover (5–15 % D) — leaves scallops ≤ 5 µm on flat regions. At the current 0.30 mm, scallops are 22 µm (= R − √(R² − (s/2)²) for R = 0.5 mm). On a topographic plaque the scallops will be visible on lake-bed and flat ridge areas but invisible on the >5° slopes. **User-decision territory** since dropping stepover triples cycle time. Documented; not applied.

### Phase 3 — verdict per TP

| TP | Phase 3 verdict | Confidence |
|---|---|---|
| 0 Pin Drill | ✅ matches FSWizard cleanly | high |
| 1 Back Rough | 🟡 chipload + feed at band-floor; could push feed to 2200 for mid-band | high |
| 2 Holes | ✅ matches FSWizard cleanly | high |
| 3 Rivers EM (copy) | ✅ matches FSWizard cleanly | high |
| 4 Rivers TB | 🟡 plunge 33 % above band — recommend 400 → 250 | high |
| 5 Lakes TB | 🟡 plunge 33 % above band — recommend 400 → 250 | high |
| 6 3D Rough 6 | ✅ matches FSWizard cleanly — best-tuned TP in the project | high |
| 7 3D Finish 6 | 🔴 plunge 750 on 1 mm TB is unsafe; stepover roughing-grade | high — flag both |

---

## Final verdict & notes

### Phase 2 → Phase 3 summary

The pristine Wanaka100 baseline was already reasonably tuned — Phase 1 found **0 rapid collisions** across all 8 TPs and reasonable cycle times. Phase 2 made the project **4.2 % faster** (5467 s → 5238 s) by aligning two 3D-rough stepovers to the `adaptive_woc_factor × D = 1.2 mm` machine-rigidity target, bumping one project-curve feed into the FSWizard band, and tightening one stale post-Roadmap-B.1 default. Phase 3 surfaced **one safety issue** that the sim couldn't catch: plunge rates on the 1 mm tapered ball-nose are 1.3–2.5× higher than published safe values across three TPs.

### Recommended follow-ups (not applied — user-decision)

| Priority | TP | Change | Rationale | Risk if applied | Risk if not |
|---|---|---|---|---|---|
| 🔴 high | TP7 3D Finish | plunge 750 → 250 mm/min | 2.5× above FSWizard band on 1 mm tapered ball | none (slower plunge is universally safer) | cutter tip damage on first plunge |
| 🟡 med | TP4 Rivers TB | plunge 400 → 250 mm/min | 33 % above FSWizard band | none | cutter tip wear |
| 🟡 med | TP5 Lakes TB | plunge 400 → 250 mm/min | 33 % above FSWizard band | none | cutter tip wear |
| 🟢 low | TP1 Back Rough | feed 1700 → 2200 mm/min | brings chipload from 0.047 to 0.061 (mid-band) | none — engagement already healthy | suboptimal MRR |
| ⚪ defer | TP7 3D Finish | stepover 0.30 → 0.10 mm | true ball-finish quality (scallop 22 µm → 2.4 µm) | cycle time 18 → 54 min | visible scallops on near-flat areas |

### Findings worth raising as project notes

1. **Helix entry style is suboptimal for adaptive3d on this terrain.** The Roadmap B.5 default of `entry_style: helix` for fresh adaptive3d toolpaths produced significantly worse outcomes than `plunge` here — 23 % cycle-time regression and 2 rapid collisions. This is the second project where Roadmap B.5 has interacted badly with `agent_search` clearing (per `planning/AGENTSEARCH_INVESTIGATION_LOG.md`). Worth raising as a Roadmap-G finding: helix entry should probably be evaluated against the specific clearing strategy.

2. **`peak_axial_doc_mm` artifact pattern.** Both TP3 (18.59 mm on a 6 mm tool) and TP6 (5.50 mm vs commanded 2.0 mm) reported peak DOC values inconsistent with their commanded `depth_per_pass`. Per CLAUDE.md April-2026 these are lift-bridge artifacts (cutter at cutting Z over uncleared neighbouring stock during rapids/links), not real over-engagement. Worth considering whether the metric could distinguish "real cut at this depth" from "rapid over uncleared stock" — the latter is information the sim already has but doesn't separate.

3. **Pristine projects benefit from re-running the project loader through current defaults.** This project predates Roadmaps B.1 (drop_cutter `min_z = -50`) and B.5 (`entry_style` defaults). The B.1 default was harmless here (model never went that deep) but caught a real bug class; the B.5 default would have made things worse on this geometry. Neither defect was severe but both would be cleaner if the loader could opt-in re-derive defaults for fresh projects vs preserve-as-saved for existing ones.

### What "dialed in" means after this session

The project is now in a state where:
- **Engagement profiles are healthy** for both 3D rough TPs (50–60 % normal bin, both within 0.20 D = 1.2 mm stepover target).
- **Chiploads are in FSWizard bands** for all chip-relevant TPs except TP1 which sits at the floor (defensible — engagement is already healthy).
- **0 rapid collisions** across 8 TPs.
- **Project total runtime** is 5238 s (87 min 18 s), down from 5467 s baseline (−4.2 %).

The remaining open items are:
- Three plunge-rate safety recommendations on the 1 mm tapered ball (sim can't catch them; FSWizard cross-check did).
- One stepover decision on TP7 finish (cycle-time vs surface-quality trade-off, user-direction).
- One peak-DOC artifact investigation (research-grade follow-up, not blocking).

The toolpaths are sane against the FSWizard reference. The user can run a physical first-piece test with confidence that nothing in the project is obviously wrong, with the caveat that the three plunge rates should be reduced before committing to long unattended runs.

---

## Post-fix validation (commit c5b9f74, 2026-05-19)

Three of the five defaults defects surfaced in this assessment are now fixed in code:

| Original defect | Fix | Validation |
|---|---|---|
| TP1/TP6 adaptive stepover narrow (0.7-0.8 mm vs 1.2 mm target) | **Fix 1** — wood + flat tool adaptive WOC tracks `machine.adaptive_woc_factor × D` | `tests/wanaka_defaults_validation.rs::wanaka_adaptive3d_6mm_em_lands_stepover_at_target` |
| TP4/TP5/TP7 plunge unsafe on 1 mm tapered ball (400/750 mm/min) | **Fix 2** — tapered-ball / ball plunge cap at 150 mm/min per mm of tip diameter | `tests/wanaka_defaults_validation.rs::wanaka_1mm_tapered_ball_plunge_capped` |
| Helix entry regresses on `agent_search` clearing | **Fix 4** — helix/ramp variants now honor `rapid_floor_z` the same way plunge does | All 30 `adaptive3d::tests` pass; full RCA in `planning/F4_HELIX_AGENT_SEARCH_RCA.md` |

Three counter-tests guard against false-positive scope:
- `wanaka_6mm_em_plunge_not_derated` — Fix 2 only affects ball/tapered-ball tools
- `test_metal_adaptive_stepover_keeps_metal_base` — Fix 1 only affects wood-class materials
- `test_flat_endmill_plunge_unchanged_by_fix2` — 6 mm EM plunge stays in metal-grade band

**Two findings deferred:**
- **Fix 3 (drop_cutter ball-finish stepover)** — the LUT already returns the correct 0.03 mm stepover for 1 mm tapered ball drop_cutter finish; the Wanaka TP7 0.30 came from a pre-Roadmap-F.5 static default and is a stale-saved-value issue, not a code defect.
- **Fix 5 (existing-project re-derivation policy)** — recommended Option C (load-time validator with per-rule auto-fix) in `planning/F5_FRESH_DEFAULTS_POLICY.md`; implementation deferred to a separate batch.

**Empirical re-validation pending:** to confirm the fixes hold end-to-end on the Wanaka project itself, the GUI/MCP binary needs to be rebuilt and the project re-loaded with new toolpaths created via `add_toolpath` (so the LUT-on-create path picks up the new defaults). The library-level fixes are locked in by the unit and integration tests above.

---

## Warning-calibration follow-up (Priority 1 — 2026-05-19)

The defaults-calibration work above fixed the *source* of bad parameters. The
next axis is **warning-calibration**: the sim was warning about toolpaths the
LUT had just recommended — eroding trust faster than the bad defaults did.

### Priority 1 — Op-kind-aware air-cut thresholds (done)

**Root cause** (`planning/P1_AIR_CUT_THRESHOLDS_RCA.md`): the project-level
verdict used a single `air_cut_percentage > 40.0` threshold across all op
kinds. ProjectCurve over sparse rivers (78–92% air-cut is intrinsic) and
Drill ops (dexel reads 100% always for Z-only kinematics) both tripped the
warning falsely.

**Fix:** `OperationType::air_cut_high_threshold_pct()` returns per-op-kind
high-water marks (`None` for drill kinds — handled in P4). `Session::diagnostics()`
now scans `toolpath_summaries` and only warns when individual TPs exceed
their own op-kind threshold; the verdict names the specific TP(s).

**Expected delta on Wanaka:**

| Before | After |
|---|---|
| `WARNING: high air cutting` (no TP named) | `OK` — all 8 TPs are within their op-kind bands |

The Wanaka project sims with TP3 ProjectCurve at 92%, TP4 at 84%, TP5 at 78%
(all below the 97% ProjectCurve threshold), TP6 Adaptive3d at 52% (above 40%
threshold — this one **will** be flagged), TP7 DropCutter at 11.5% (below 30%
threshold), TP1 Adaptive3d at 28.5% (below 40% threshold), TP0/TP2 drill ops
suppressed. Post-Phase-2 with TP1 at 34% and TP6 at 54.8%, expect TP6 to be
the only TP cited.

**Tests:** 13 new unit tests across `compute/catalog.rs` (5 — threshold
calibration) and `session/compute.rs` (8 — verdict logic with positive and
negative cases per op-kind).

### Priority 2 — Plunge-stress warning for small ball/tapered-ball tools (done)

**Root cause** (`planning/P2_PLUNGE_STRESS_GATE_RCA.md`): Fix 2 caps fresh-LUT
plunge recommendations at `150 mm/min × tip_diameter_mm` for ball/tapered-ball
geometries, but pre-Fix-2 projects (like Wanaka) carry static-default plunge
rates that bypass the cap. The sim's chipload/power/deflection gates evaluate
continuous cutting samples and never look at plunge moves, so a 1 mm tapered
ball plunging at 750 mm/min in hardwood sims as silent-OK while FSWizard says
it's 2.5× the safe rate.

**Fix:** New `tool_load::plunge_stress` module exposes
`safe_plunge_cap_mm_min(geometry, diameter)` and `check_plunge_stress(...)`.
`Session::diagnostics()` scans enabled toolpaths and surfaces offenders in
the verdict as "WARNING: unsafe plunge rate on TP-name (rate > cap mm/min)".

**Expected delta on Wanaka:**

| TP | Tool | Plunge | Expected |
|---|---|---|---|
| TP4 Rivers TB | 1 mm tapered ball | 400 | warn (cap 150) |
| TP5 Lakes TB | 1 mm tapered ball | 400 | warn (cap 150) |
| TP7 3D Finish 6 | 1 mm tapered ball | 750 | warn (cap 150) |
| TP1 / TP6 (6 mm EM) | flat | 500–750 | silent (no cap on flat) |

Three of the eight TPs cited in the verdict, matching the Phase 3 FSWizard
finding.

**Tests:** 14 new unit tests (10 module-level in `plunge_stress.rs` for the
cap formula + 4 session-level for verdict wiring, including the Wanaka TP7
750 mm/min reproduction and a flat-EM negative control).

### Priority 3 — Suppress peak-axial-DOC on transit/link spans (done)

**Root cause** (`planning/P3_TRANSIT_PEAK_DOC_RCA.md`): the simulation's
`SummaryAccumulator::observe` takes the per-sample max of `axial_doc_mm`
across **every** sample regardless of context. For samples in transit-style
spans (Entry, LeadOut, LinkBridge, WaterlineCleanup, DressupArtifact) the
dexel reading reports `stock_top − cutter_z` over uncleared neighbouring
stock, not engagement. Wanaka TP3 reads `peak_axial_doc_mm = 18.59 mm` on a
6 mm tool from one such sample during a link bridge.

**Fix:** new `in_transit_span: bool` field on `SimulationCutSample` (with
`#[serde(default)]` for back-compat). The simulator populates it from
`AnnotatedToolpath::transit_moves_bitmap()`. `SummaryAccumulator` gates
`peak_axial_doc_mm` and `peak_chipload_mm_per_tooth` updates on
`!sample.in_transit_span` — runtime-distribution metrics (cutting time,
engagement histogram, air-cut time) stay unchanged.

**Expected delta on Wanaka:**

| TP | Before peak DOC | After peak DOC |
|---|---|---|
| TP3 Rivers EM | 18.59 mm (artifact) | ≈ 3 mm (real cut depth) |
| TP6 3D Rough 6 | 5.50 mm (commanded 2 mm) | ≈ 2 mm |
| TP4 Rivers TB | 2.04 mm (commanded 0.2 mm) | ≈ 0.5 mm |
| TP5 Lakes TB | 1.88 mm (commanded 0.2 mm) | ≈ 0.5 mm |
| TP1 Back Rough | 3.00 mm (already clean) | 3.00 mm unchanged |

**Tests:** 8 new unit tests — 3 on `AnnotatedToolpath::transit_moves_bitmap`
(transit kinds detected, cutting kinds not flagged, dressup/lead-out
handled) + 5 on the accumulator (cutting samples in, transit samples out,
purely-transit stream stays at 0, peak chipload gated too, per-TP summary
respects the flag).
