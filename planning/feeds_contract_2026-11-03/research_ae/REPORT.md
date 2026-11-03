# Stepover (ae), depth (ap) and load - research report v2 (2026-11-04)

Read-only. No repository file changed. No cargo run. HEAD `acfbc7c5f`.
Script `ae_claims.py` (same folder) asserts every derived number. It prints "ALL CLAIMS HOLD" and
"PART 2 CLAIMS HOLD". `fz = 0.05` is an example value only. Scope: flat end mills first (operator,
2026-11-04). Paths are under `crates/rs_cam_core/src/` unless stated.

## Summary

1. Stepover needs better modelling: yes. ae enters power and force, but three envelope defects hide it (A1-A3).
   At the same ap and feed a slot is more load than 0.2 D: shear power x5, edge power x3.39, edge-in-cut time x3.39.
   At the same MRR the model prefers a shallow slot. The reasons not to slot (heat, chip evacuation,
   conventional-side rubbing, local wear) are not in the model.
2. "20 %": no repo source prints 20 % as an optimal load for wood. The operator's 20 % is
   `adaptive_woc_factor = 0.20` (`machine/mod.rs:732`; rivmap toml `:54`), an unsourced rule of thumb from
   the first commit (`machine/mod.rs:74`). Other "20 %" values: section 3.
3. One combined model or separate effects? One engagement-angle force model with helix covers most outputs:

   | Output | ap | ae | Covered by one model |
   |---|---|---|---|
   | chip thickness h(theta) = fz.sin theta | no (flat) | yes (psi) | yes |
   | force magnitude | yes | yes | yes |
   | force direction | no | yes (entry/exit angle; up/down) | yes (needs K_r) |
   | power | yes | yes | yes |
   | deflection | yes (force, lever arm) | yes (magnitude, direction) | yes |
   | edge heat | weak | yes (time in cut psi/2pi, chip size) | partly (observed only) |
   | chip evacuation | yes | yes (slot walls) | no; no repo source |

   The repo holds the parts: psi (`feeds/force.rs:57`), the Curti affine line, and a helix-wrap term
   (`tool/mod.rs:250-255`). Today the force is one tooth at the peak chip over the full ap, with no helix.
   At 0.2 D that is 4.0x the mean force over the engaged edge; in a slot 1.16x. This one error pushes every
   limit against deep, light cuts.
4. Chip-first flow: yes for flat end mills, in stages (section 6).
   (a) The chip falls with depth from the vendor depth table (`feeds/geometry.rs:155-165`): 0.5 above 3 x D;
   25/6 = 4.17, so 0.131 -> 0.0655.
   (b) The table has no stated radial condition and no stated reason: a force proxy at unknown engagement.
   (c) At ae 0.2 D the derated chip gives a mean chip of 0.028 mm. The floor 0.025 is unsourced. 0.028 is
   below the Curti fit range 0.04-0.10. This is the operator's "we burn" case.
   (d) Hold the chip in its band, size the feed for the actual chip, and let force, power and deflection at
   that chip bound ap x ae.
5. 1.125 kW available is plausible for the 1.5 kW constant-torque VFD (toml `:38-40`). Not investigated further.

## 1. Where ae enters the model today

| Place | Formula | Does ae enter? |
|---|---|---|
| Feed | `rpm x chip x Z x depth_tier` (`feeds/mod.rs:2442`) | No. `rctf` computed (`:2412`), not applied (`:595-608`); "A stepover change therefore no longer moves the feed" (`:2409-2411`) |
| Power | `A(Ks.MRR + F_edge.ap.Vc.z.psi/2pi)/6e7` (`tool_load/power.rs:13`, `:162-182`) | Yes: section `ap x ae` (`feeds/mod.rs:2522`) and psi (`:1592`) |
| Side force | `ap(Ks.fz.sin(min(psi,pi/2)) + F_edge)` (`feeds/force.rs:21-26`, `:94-97`) | Partly: saturates at D/2; one tooth; no helix |
| Tip deflection | stepped cantilever, load at `stickout - ap/2` (`tool/mod.rs:749-770`) | Only through the force |
| Slot rule | `ap <= 0.25 D where ae > 0.85 D` (`feeds/mod.rs:2330-2346`) | Yes, a step. Both constants unsourced (`:1651-1661`) |
| Pass 6b (aggressiveness) | one common scale on dpp and stepover; chip does not move (`feeds/suggest.rs:672-700`) | Yes, through force and power |
| Post-sim power gate | width `(arc/pi).2r` (`tool_load/power.rs:266-273`) | A second width rule; P9 names it (D-W) |
| `ChipGeometry` | `h_max = fz.sin(arc)` for arc < pi; mean `(2h_max/arc)(1-cos(arc/2))` (`tool/mod.rs:239-248`) | A third engagement definition. For pi/2 < arc < pi it gives h_max < fz, where `force.rs` gives fz. Add to P9 |
| `BandMid` modulation (legacy) | `band.mid / sqrt(woc)` (`dressup/feed_modulation.rs:763-765`) | Yes: the one thinning lift G-CHIPTHIN-HALFFIX did not remove. Clamped to the band ceiling; not the default (`:87`) |
| Adaptive planners | `acos(1 - ae/R)/2pi` (`ops/adaptive_shared.rs:10-14`) | Geometry only; no load target. The "Optimal load" slider was removed (`rs_cam_viz/src/ui/properties/operations/surface_3d.rs:71-72`) |

### Defects that hide the ap-ae trade

- A1. The grid shows power at a capped depth (confirmed by code). `calc_at` sets ap (`feeds/envelope.rs:527`).
  `calculate` lowers ap at the flute guard (`feeds/mod.rs:2294-2302`; 0.8 x 25 = 20 mm on rivmap,
  toml `:70`) and at the slot cap (`:2330-2336`). `SpeedsPoint::from_result` stores the requested ap and the
  power of the lowered ap (`envelope.rs:359-377`, `:695-699`). `power_limit` bisects on the same results
  (`:1189`). So above 0.85 D every point with ap > 1.5 mm shows the power at 1.5 mm, and the power edge cannot
  cross there.
- A2. The flat 42.75 mm deflection edge is the search bracket: 0.95 x 45 mm stickout (toml `:78`;
  `feeds/cutter_constraints.rs:87`). `invert_deflection` returns the upper bracket when that point is in the
  limit (`:316-318`). The model reads ae; it finds no limit in the span. The envelope still draws an
  `ApCurve` (`envelope.rs:1293-1302`): an absence shown as a limit. The bracket also passes the 25 mm flute.
  The search assumes deflection grows with ap. For a uniform beam with the load at `a = L - ap/2`,
  deflection ~ `ap.a^2(3L - a)` peaks at ap = 32.9 mm, 7.4 % above its value at 42.75 mm. A cut between the
  brackets could exceed and still pass. A unit test on the stepped beam must decide.
- A3. The power edge does not reach the plot. A1 removes the deep-and-wide corner, where power is largest.

## 2. What the repo's sources print about ae

- Wood: "No wood source in the shipped LUT publishes a radial-engagement condition for its chipload column"
  (`planning/review_2026-08-04/CHIPLOAD_LITERATURE_VERDICT.md:283-289`; N-8 `:818`). The wood `ae_rule`
  strings are repo-authored windows (`:291-299`; `CREDITS.md:373-377`).
- Garr chip thinning (`CHIPLOAD_LITERATURE_VERDICT.md:226-233`; `CREDITS.md:357-361`): "Radial chip thinning
  occurs when the width of cut is less than 50% of the cutter diameter"; "the feed rate is increased to
  maintain a co[n]stant chip thickness relative to the radial depth of cut".
- Garr aluminium, the one printed ap-ae-chip trade (`crates/rs_cam_core/data/vendor_lut/observations/garr_aluminum.json`):

  | Row | ap | ae | chip mm/tooth | Vc |
  |---|---|---|---|---|
  | slot (`:4-29`) | .5 x D | 1 x D | 0.03-0.09 | SFM 400-600 |
  | profile (`:88`) | 1 x D | .5 x D | 0.06-0.12 | SFM 500-650 |
  | HEM (`:256-279`) | 2 x D | 30-40 % D | 0.12-0.18 | max RPM |

- Helical (`helical_aluminum.json:4`, `:33`): "HEM ADOC ... 200% x D" / "HEM RDOC ... 40% x D" against
  "Traditional ... 100% x D" / "50% x D". Wording not verified (N-4, `CHIPLOAD_LITERATURE_VERDICT.md:814`).
- Mechanistic force: "ae sets the entry/exit angle limits of integration and the number of engaged teeth"
  (`planning/UNIFIED_LOAD_MODEL_2026-06-18.md:106-110`, S100093611300054X).
- Wood line: Curti 2021, affine, density-scaled, valid "mean chip 0.04-0.10 mm" (`CREDITS.md:426-435`).
- Specific energy `u = Ks + F_edge.D.psi/(2.ae.fz)`, "a wear proxy ... heating the edge"
  (`planning/load_model_2026-09-16/ADVICE.md:109-121`).
- Feed-force ratio "0.79 to 0.97" in ae/D 0.05-0.20 (`planning/load_model_2026-09-16/DERATE_SPEC.md:176-182`).
- Heat: Freud "If your chip is very small, or just sawdust, then it will not carry enough heat away from the
  edge of the bit" (`planning/extrapolation_2026-09-24/fetch/G6/sources/freud_router_bit_feed_speed_cnc_20170822.txt:51-53`).
  Slot heat/evacuation is repo text or judgement only (`tool_load/metric_guide.rs:91`;
  `planning/feeds_blind_review_2026-11-03/review_C.md:101`). Onsrud prints only the tool choice: "Upcut
  Spiral ... Use for grooving or slotting, for upward chip evacuation" (`fetch/G3/sources/onsrud_oc06_catalog.txt:6062`).

## 3. Every "20 %" candidate

| Candidate | Where | Status |
|---|---|---|
| `adaptive_woc_factor 0.20` | `machine/mod.rs:732`; rivmap toml `:54` | The operator's 20 %. Unsourced, first commit `172d6de60`, "RULE OF THUMB with no published source" (`:74`). Default profile 0.25 (`:175`). Lifts wood Adaptive ae (`feeds/mod.rs:3797-3806`) |
| "The current target for wood is 20 %" | `compute/validate.rs:281-296` | Restates the factor |
| `doc_roughing_factor 0.20` | `machine/mod.rs:726` | A depth. Unsourced (`planning/feeds_matrix_2026-09-23/EVIDENCE.md:327`) |
| Pocket SemiFinish `ae_factor 0.20` | `feeds/mod.rs:3277-3279` | Unsourced (`:3247`) |
| Garr "20% of diameter for basic engagement parameters" | `garr_aluminum.json:382-383`, `garr_fiberglass.json` | Printed; aluminium and fibreglass |
| Amana `ae_rule` "8% to 20%D" | `amana_flat_end.json` | Repo-authored (`EVIDENCE.md:356`) |
| IDC V-bit clear-pass 20 %; IDC metric ball 20 % (imperial 40 %) | `EVIDENCE.md:155`, `:110` | Retailer; self-contradicting |
| Onsrud finish allowance "approximately 20%" | `EVIDENCE.md:320` | A finish allowance |
| Blind review "20 % reserve" | `planning/feeds_blind_review_2026-11-03/review_A.md:64` | Torque reserve; judgement |
| `aggressiveness` 0.85 | `machine/mod.rs:609` | Ruling R4 Q1; not 20 % |

## 4. Physics (repo formulas; numbers from the script)

Oak line Ks 58.52, F_edge 4.595 (`tool_load/power.rs:901`), D 6, Z 2, helix 30 deg (toml `:71`).
Mean chip `fz(1 - cos psi)/psi` (`feeds/operating_point.rs:242`).

| fz 0.05 | slot | 0.5 D | 0.2 D | 0.1 D |
|---|---|---|---|---|
| psi (rad) | 3.142 | 1.571 | 0.927 | 0.644 |
| teeth in cut z.psi/2pi | 1.00 | 0.50 | 0.30 | 0.20 |
| peak chip / fz (= 1/RCTF) | 1.00 | 1.00 | 0.80 | 0.60 |
| mean chip (mm) | 0.0318 | 0.0318 | 0.0216 | 0.0155 |
| u (N/mm^2 = mJ/mm^3) | 202.9 | 202.9 | 271.6 | 354.2 |
| one-tooth model force per mm ap (N) | 7.52 | 7.52 | 6.94 | 6.35 |

- Same ap, same feed: slot vs 0.2 D = shear power x5, edge power x3.39, model force x1.08.
- Same MRR (slot at ap/5): shear power equal; slot edge power 0.68x; model force about 0.22x; u lower in the
  slot (thicker chip). The model, as written, prefers a shallow slot.
- Chip thinning: at 0.2 D the peak chip is 0.8 fz (RCTF 1.25, `feeds/geometry.rs:17-31`). If the vendor chip
  is a peak chip (Garr), the feed can rise by RCTF at the same peak force. The repo removed this for wood
  only because no wood chart states its radial condition (N-8).
- Helix: at 0.2 D one tooth's arc spans 4.82 mm of flute length. Helix wrap equals the flute pitch at
  ap = 16.3 mm (`tool/mod.rs:252-253`); above that the engaged-edge force is smooth, near its mean. The
  one-tooth model gives 4.0x that mean at 0.2 D and 1.16x in a slot.
- Not in the model (no repo source): edge-in-cut share per revolution (slot 0.5, 0.2 D 0.15); chip packing in
  a slot; conventional-side rubbing (Curti up/down lines are enveloped, `CREDITS.md:430-433`); wear on a short
  flute length; force direction (needs K_r; the repo has only spruce "below 0.2", `DERATE_SPEC.md:186`).

## 5. Proposal (ae)

1. A1 (no new source): store the actual ap/ae per grid point, or mark points the calculator moved.
   `power_limit` evaluates power at the requested depth; the slot cap and flute guard stay separate Limits.
2. A2 (no new source): emit "no limit in span" when the inversion returns its bracket; clamp the bracket at
   the flute length; add a test for non-monotonic deflection (`feeds/cutter_constraints.rs:267`).
3. P9 (`planning/feeds_contract_2026-11-03/phases/P9.md`): make `Engagement` the combined model - psi from
   `immersion_angle`, helix wrap from `tool/mod.rs:250-255`, force integrated over the engaged edge, and the
   `ChipGeometry` peak/mean chip moved to the same definition. A hardwood K_r needs a new printed source.
4. Slot rule 0.85 D / 0.25 D: find a printed wood value, or keep it as an operator ruling marked Unsourced.
5. Heat and evacuation: show psi/2pi and u as observed values; a limit needs a printed wood burn threshold.
6. `BandMid`: remove or rule on the `1/sqrt(woc)` lift.

## 6. Chip-first flow

(a) Where the chip falls with depth. `doc_derating_scale` (`feeds/geometry.rs:155-165`) via
`depth_tier_multiplier` (`:287-292`) multiplies the feed (`feeds/mod.rs:2440-2442`). The same scale de-rates
the chip band (`:2979-3002`) and the post-sim chipload gate. Rivmap: 25/6 = 4.17 -> 0.5, so 0.131 -> 0.0655,
at every ae. Above 3 x D no vendor prints a factor; the code holds 0.5 (`geometry.rs:167-177`). Printed text:
- Freud: "The chip loads in the table above are based a cutting depth that is equal to or less than the bit
  diameter ... If Cut Depth is 2X the bit diameter, reduce the Chip Load by at least 25% / If Cut Depth is 3X
  the bit diameter, reduce the Chip Load by at least 50%" (`freud_router_bit_feed_speed_cnc_20170822.txt:102-106`).
- Onsrud: "1 x D Use recommended chip load / 2 x D Reduce chip load by 25% / 3 x D Reduce chip load by 50%"
  (`CREDITS.md:363-371`). Amana: the same rule on the feed rate.

(b) Proxy? Very likely. No vendor states a radial condition (N-8) or a reason. The charts describe profiling
or slotting cuts (`planning/wanaka200_feeds_check_2026-09-19/FINDINGS.md:315-318`). At a fixed, unstated ae,
force grows with ap, and a lower chip is the vendor's single control. An engagement model can hold the chip
and lower ae instead: with the helix term, a narrow cut's force grows with `ap.z.psi/2pi`, not with ap.
The crossover chip `F_edge/Ks` is 0.079 mm (`tool_load/power.rs:27`). Below it a thinner chip removes little
force and adds edge energy (u rises as 1/fz). In wood that is the wrong control.

(c) Rubbing and burn evidence. `RUBBING_FLOOR_MM_TOOTH = 0.025` is "a repo rule with no primary source"
(`feeds/mod.rs:1176-1192`); the band minimum can lower it (`:1250-1263`). "No wood tooling vendor publishes a
numeric minimum chip thickness" (`planning/load_model_2026-09-16/MACHINIST_REFERENCE_CHECK.md:505`;
`FINDINGS.md:319-323`). Freud, PreciseBits and ShopBot describe heat from small chips
(`MACHINIST_REFERENCE_CHECK.md:508-530`). ToolsToday: "If you slow the feed rate too much while keeping RPM
high, the bit may start making dust instead of chips. That can increase heat, dull the tool, and cause
burning" (`fetch/G6/sources/toolstoday_understanding_cnc_feeds_and_speeds.txt:137`). The floor tests the
advance per tooth (0.0655, passes). The real chip at ap 25, ae 0.2 D: peak 0.052 mm, mean 0.028 mm - near
the floor and below the Curti fit range 0.04 (`material/force_line.rs:78`).

(d) Recommendation: chip-first for flat end mills.
1. Take the chip from the vendor band at 1 x D and hold it. Pass 6b already holds the chip for the dial
   (`feeds/suggest.rs:672-675`); extend the rule to depth.
2. Size the feed so the actual peak chip `fz.sin(theta_peak)` sits in the band (chip thinning at ae < 0.5 D,
   backed by Garr's >= 50 % rule). This needs an operator ruling over N-8, and the post-sim gate changes in
   the same commit.
3. Bound ap x ae by force (helix engagement model), power, deflection and the flute guard at that chip. The
   envelope then shows the ap-ae trade.
4. Keep the vendor depth table as a visible Limit ("vendor ceiling at unknown ae"), not a feed multiplier.
   Warn when the plan is deeper than the table at an ae wider than the model trusts.
Preconditions: A1, A2; P9 `Engagement` with helix; a sourced hardwood K_r (or deflection stated as magnitude
only); an operator ruling on chip thinning vs N-8. No new source for the depth table: it stays a printed Limit.

Constants that need a printed source before they gate anything: `SLOTTING_THRESHOLD`, `SLOTTING_DOC_CAP`,
`adaptive_woc_factor`, `RUBBING_FLOOR_MM_TOOTH`, hardwood K_r, any burn threshold.
