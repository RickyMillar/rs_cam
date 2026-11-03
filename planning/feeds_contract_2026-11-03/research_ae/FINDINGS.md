# How stepover (ae) sets load: findings (2026-11-04)

Read-only research by agent ae-research at HEAD `acfbc7c5f`, checked by the lead.
`ae_claims.py` asserts each derived number (run `python3 -I ae_claims.py`; it
prints ALL CLAIMS HOLD and PART 2 CLAIMS HOLD). Inputs: white oak Curti line
(`tool_load/power.rs:901`), rivmap100 tool D 6, z 2, stickout 45, helix 30.
`fz = 0.05` is an example chip inside the Curti range (`material/force_line.rs:78`).

## Operator questions

- "Stepover needs better modelling?" Yes. ae enters power (shear term `ap x ae`,
  edge term `psi = acos(1 - ae/r)`), but not the feed (radial chip thinning is
  observed, not applied, G-CHIPTHIN-HALFFIX), and the side force saturates at
  ae >= D/2 and is one tooth's peak with no helix.
- "Otherwise we would full slot?" At the same ap and fz, a slot is 5x the shear
  power and 3.39x the edge power of 0.2 D. At the same MRR (slot at ap/5) the
  model favours the slot (edge power 0.68x, side force about 0.22x). The real
  reasons not to slot (heat per revolution, chip evacuation, rubbing, local
  wear) are not in the model. No wood source in the repo prints a radial
  condition (`CHIPLOAD_LITERATURE_VERDICT.md` N-8). Garr and Helical print the
  HEM trade for aluminium only.
- "Where does 20 % come from?" `MachineProfile::adaptive_woc_factor = 0.20`
  (`machine/mod.rs:732`), in the first commit `172d6de60`; its doc says "a RULE
  OF THUMB with no published source". No repo source prints it.

## Defects found

- D-A: the envelope speeds grid stores the requested ap but sizes power at the
  ap after the slot cap and the flute guard (`feeds/envelope.rs:359-377`,
  `feeds/mod.rs:2301/2336`). Above the slot threshold, power is flat in ap.
- D-B: the deflection edge at 42.75 mm is the search bracket 0.95 x stickout
  (`cutter_constraints.rs:87, :316-318`), drawn as a limit. Deflection vs ap
  under a load F = k ap peaks near ap 32.9 mm, inside the bracket.
- D-C: the one-tooth peak force overstates a deep narrow cut about 4.0x against
  the mean engaged-edge force (slot: 1.16x). A helix-wrap term exists
  (`tool/mod.rs:250-255`, `instantaneous_flutes_in_cut`) and is not used by
  the force model.
- D-D: the chip falls with depth through `doc_derating_scale`
  (`feeds/geometry.rs:155-165`: 1.0 to 1 x D, 0.75 at 2 x D, 0.5 at 3 x D,
  held at 0.5 above). Onsrud, Freud and Amana print the 1/2/3 x D steps; no
  vendor states a radial condition or a reason. On rivmap100 3D Rough at
  0.2 D and ap 25 mm the mean chip is 0.028 mm: near the 0.025 floor (an
  unsourced repo rule) and below the Curti fit range (0.04).
- Two width rules: the post-sim power gate uses `(arc/pi) 2r`
  (`tool_load/power.rs:266-273`), 0.295 D at 0.2 D (P9 D-W). The legacy feed
  modulation `BandMid` still lifts the feed by `1/sqrt(woc)`.

## Recommendation (agent, lead agrees)

Chip-first for flat end mills, in stages: hold the chip in its band; size the
feed for the actual chip (thinning); let force (summed over the engaged edge
with the helix wrap), power and deflection at that chip bound ap x ae, so the
envelope shows the ap-ae trade. The vendor depth tiers then become a check,
not the control. Heat and chip evacuation need new printed wood sources.
Correction by the lead: the agent wrote "pass 6b already holds the chip"; P5
deleted pass 6b (only its dead `SuggestWarning` variant remains).

The full report (203 lines) is `REPORT.md` in this folder. Its references to
pass 6b (`feeds/suggest.rs:672-700`) read a dead warning variant: P5 deleted
the pass. Section 6(d) needs an operator ruling on chip thinning against
N-8 (`CHIPLOAD_LITERATURE_VERDICT.md`) before step 2.
