# Operator decisions, 2026-10-28

The lead (session rs-cam-85) ran the research and the integration overnight. This file lists what is done and what only the operator can decide. The page for the G2 decision is https://claude.ai/artifact/AvKopJ9PFDNXzPbyLA5Ud9 (version 4).

## Done

- **G2 recommendation: promote**, with the conditions below.
- **Integration branch `geodesic-integration`** (from master ab208f83a; master has no code commits since). Commits in order:

  | Commit | Step |
  |---|---|
  | 5d700703a | step 1: the per-move feed cap |
  | 5561fdb5f | step 1c: the export gate fix |
  | a78ede314 | 7.1: the one load function |
  | cfe2c36db | 7.2: the shared split, the keep-down proof and the router |
  | c981145f5, d639bb8f1, a82b79d70 | 7.3a–d: the `adaptive/geodesic/` kernel; research file cut to test support |
  | 22557a199 | 7.4a: the 3D engine, with turn feeds on `feed_cap` |
  | 26d901943 | 7.4b: the strategy in the config and GUI |
  | 5657a7b96 | 7.4c: caps in the sim trace |
  | c09e0e238 | the fixture feed fills |
  | f915d37dd, 5a72cc07d | 7.5a–b: rest regions with the frame clip, the Z top, the depth guard and the larger-tool refusal |
  | 7ee3d3226 | 7.7: the pass-load judge on the product |

- **Every existing strategy is byte-identical** on the integration branch: no golden was re-blessed.
- **The product equals the research branch.** D58 and D60 show that op 1's geometry, load, plunges and volume match exactly. Every edge op is byte-identical to a research run. 787 capped moves, and none is fed over its cap.
- **Research branch `adaptive-instruments`:** the defect fixes D47, D48, D50 (with the addendum), D53 and D55.

## Results (acceleration clock, against contour-parallel)

| Part | Stepover | Job / CP | Peak load (geodesic / CP) | Note |
|---|---|---|---|---|
| wanaka200 | 2.0 mm | 0.57 | 0.72 / 2.62 | leaves about CP's stock (D51) |
| wanaka200 | 3.5 mm | 0.34 | 0.99 / 2.62 | leaves about twice CP's area over 0.6 mm on the unfinished back face |
| Corne | 2.1 mm | 1.15 | 1.00 / 1.03 (p99 0.49 / 1.01) | the 6 mm roughing is 1.07× CP; the rest of the gap is the 3 mm rest op (G-LEVELPHASE) |

## Safety finding: read this first

**G-RESTDOC (D61)** blocks machining the "body + edge op" pair. The edge op, a contour-parallel rest op on remaining stock, cuts deeper than one Depth/Pass in its first level:

| Part | Edge op | Max axial cut | Depth/Pass | Volume deeper than one DPP |
|---|---|---|---|---|
| Corne | op 5 | 8.96 mm | 6 mm | 12.7 % |
| wanaka 3.5 mm | op 7 | 16.2 mm | 5.46 mm | 14.7 % |

- The export only warns about it ("Exceeded without refusing"), and does not refuse.
- The earlier D54 numbers came from an even deeper cut (12 mm, 36 %), so they are void.
- D62 checks whether shipped CP-only jobs breach one DPP in their rest ops too.
- **D62:** shipped CP-only jobs anchor correctly. But **D63** shows that a CP + CP pair with mismatched boundaries can breach it too. The rest op's boundary clear removes a band its tool still reaches.
- **D63 (research 805368f98):** the start-level part is fixed. The cut part needs your choice, set out in `phase3/d63/README.md` §6:
  1. keep the band in the rest op's planner stock (G-CLIPAIR for every strategy): holds the rule, moves shipped output;
  2. an edge pass per level where the band stands: smaller; C24's recommended next step;
  3. refuse rest ops whose simulated cut exceeds one DPP, and warn for other ops (shipped CP has local over-DPP spots, G-AXIALDOC). C24 recommends this as a guard beside 1 or 2.
- Until G-RESTDOC is fixed, test-cut op 1 (Geodesic Adaptive) alone, or with an edge op on a conservative setup that you check by eye.

## Decisions for the operator

1. **Merge `geodesic-integration` into master?** It ships "Geodesic Adaptive" as new and not yet machine-tested. Recommendation: yes, after one test cut.
2. **One test cut on the machine.** Suggested: wanaka200 back rough at 2.0 mm with the edge op, or a small part.
3. **Q12: the default stepover rule.** 3.5 mm is 3× faster than CP but leaves a rougher unfinished face; 2.0 mm matches CP's surface.
4. **Q-T: the "body + CP edges" template (step 7.8).** Decide:
   - whether it is one GUI action or a job-file pair;
   - which edge tool it uses. It must be no larger than the rough's tool, or use Inside containment; otherwise D53 refuses.
   - whether the edge op is optional. On wanaka op 1 clears almost everything, and the edge op costs about 940 s to remove 1.8 cm³.
5. **D55 on master** (the plunge entry pecks from the contact read). It changes shipped output for every 3D rough with plunge entries, CP included: XY moves are identical, and only the air re-feeds and retracts go. Fixture move counts fall, for example 378 → 332 on AgentSearch. Recommendation: yes.
6. **G-FEEDMAX.** Feed modulation runs cutting moves above `feed_max_rate` (3000) up to the machine ceiling: 4000 on Corne, up to 5245 on wanaka. The doc scopes `feed_max_rate` to the optimiser only. Decide: change the doc and GUI label, or make the modulator obey it.
7. **Q5 / Q-L: when to mark AgentSearch legacy** in the GUI and MCP.
8. **Parked, no action needed now:**
   - G-EDGEBAND: about 216 s of wanaka edge-op air feed, from a cell-quantisation sliver; fix (a) is recorded.
   - G-LEVELPHASE, G-CPRAPID, G-AIRRAMP, G-SIMFEED: shipped defects that also slow CP.
   - G-MASTERREDS-1027: two shipped adaptive sentries red on master.
   - The slab field.

## Operator rulings, 2026-10-28 morning

- **Q5, decided:** FULL removal of the AgentSearch, ContourSpiral and Adaptive (original 3D) clearing strategies. Geodesic Adaptive and Contour Parallel stay. D64 measures all five first ("just in case"). I1 designs the removal (`integration/removal_design.md`).
- **Op 1 re-entry rows** (the turn row at the Corne ledge, and the LargeZ rows at the walls): the operator asks whether these can be tidied. C24 designs a capped-turn admission in place of retract and re-entry (`phase3/d65/README.md`).
- **Removal load rule, decided:** option B, refuse at load. A project or job file that names a removed strategy refuses as a whole, with a message that names the fix (RETIRED_VALUES precedent, rulings 2026-09-16 and 2026-10-15).
- **Advisor, decided:** it compares Contour Parallel and Geodesic Adaptive; suggest heuristic B points at geodesic_adaptive. The wording follows D64.

## Operator rulings, 2026-10-28 midday

- **G-BOUNDCOLLAPSE:** refuse generation when a boundary's inset collapses (all strategies).
- **G-FEEDMAX:** the feed modulator obeys feed_max_rate (shipped output changes where modulation boosted above it).
- **D55:** port to the integration branch, with the fixture re-bless listed move by move.
- **Template (7.8):** one GUI action, "Geodesic body + edges", with the edge op optional; the edge tool is no larger than op 1's, with Inside-safe containment.
- **Advisor:** about 38 s is acceptable for now.
- **Merge:** merge now, test-cut later. Done: master cc00aa0c3 merges geodesic-integration.
- **G-RESTDOC:** the operator likes the edge contour pass. They asked what the region-side fix would do to the numbers, and how either option behaves when there are many boundaries or regions. Study first (C24), then decide.
- **Core full gate:** not run yet; the operator deferred it (2026-10-28). C1 can make a core test that generated through a collapsed boundary refuse; only the full gate would show it.
