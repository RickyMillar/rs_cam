# Round-04 delta — 2026-05-25

**Vs baseline:** round-02 `rounds/round-02-2026-05-25/delta.md` +
round-03 `rounds/round-03-2026-05-25/delta.md`.

**Smoke type:** focused deflection probes (AS001 + AS013), 3 MCP
rebuilds, 4 smoke-verifications.

**Auditor:** autonomous Claude session.

## Headline

**F-024 fully verified on AS001**: deflection.peak_mm dropped from
**0.374 → 0.076 mm** (Exceeds → Within). Collisions 36→0. Air-cut
77%→37%. Engagement 0.08→0.28. Per-pass volumes finally plausible
(6.6k mm³ each pass instead of 40k on first pass and 287 on second).

**AS013 unchanged** — `ux_3d_terrain.toml` has `stock.origin_z = 0`,
so F-024's frame mismatch doesn't apply (the broken zero-rooted bbox
IS the world bbox for that case). AS013's deflection over-fire is
either genuinely real (181k-move 3D rough at 2.7× recommended feed on
full DOC) or a different latent bug — not addressable by F-024.

## The three-rebuild saga

F-024 needed fixes at THREE sites; each rebuild flushed out the next.
This is the round's most important learning:

| Smoke | Binary has | AS001 deflection.peak_mm | Auditor diagnosis |
|---|---|---|---|
| v1 | `d82bd4d` (core only) | 0.374 (unchanged) | Viz worker doesn't route through `ProjectSession::run_simulation` |
| v2 | + `0c907a6` (viz worker) | 0.374 (unchanged) | Viz controller drops `stock.origin_*` when building world bbox |
| v3 | + `67de558` (viz controller) | **0.076 (Within!)** | All three sites patched; production path consistent |

Each implementer's local acceptance test passed on their PR. Each
fix was real and necessary. **None moved the production needle alone.**

This is a meta-finding worth absorbing into the loop docs: in
multi-pipeline codebases, per-fix acceptance tests need to probe
through to the user-facing entry point — otherwise multiple
local-test-green fixes can stack up before a smoke run reveals they
don't compose. See "Loop process learning" below.

## Verdict by finding

### Verified — closing this round

| Finding | Commits | Smoke evidence |
|---|---|---|
| **F-024** (3 sites) | `d82bd4d` + `06a9a2a` + `0c907a6` + `56e9ec2` + `67de558` | AS001 deflection.peak_mm 0.374→0.076. peak_axial_doc_mm.linear 12.0→2.0 (matches commanded). collisions 36→0. Per-pass volumes 40k+287+118→6.6k+6.7k+6.8k (now realistic). |

### Reframed mid-round

| Finding | Resolution |
|---|---|
| **F-024** Original scope was "core fix only". Round-04 smoke flushed out three production sites that all needed the same identity-setup conditional + stock-origin-aware world bbox. Final scope: 5 PRs across core + viz worker + viz controller. |

### Acceptance bars status

| Bar | Round-02 | Round-04 v3 | Status |
|---|---|---|---|
| Sim chipload calibration (3D ops) | 1/1 (AS013) | 1/1 (AS013) | stable |
| Sim chipload calibration (2D ops) | 2/2 (AS001/AS002) | 1/1 (AS001) | F-001 verified, AS013 still Exceeds_LOW (real chip-thinning at low engagement, not a frame bug) |
| **Sim deflection calibration** | 0/4 Within | **AS001 Within (0.076)**, AS013 still Exceeds 0.573 | **F-024 partial — AS001 fixed; AS013 needs separate investigation (different stock origin)** |
| Optimizer refusal correctness | 1/1 (AS015 byte-identical) | not re-tested | stable per round-02 |

The deflection bar went from 0/4 to 1/4 verified Within. Not a full
close, but a clean direction-correct move. AS001 is now passable on
all three bars (chipload, power, deflection) modulo real chip-thinning.

### Unchanged signals (collateral wins)

- **AS001 rapid_collision_count: 36 → 0.** The rapid-collision flagger
  was tripping because the grid was at the wrong Z; rapids over
  "uncleared" stock weren't actually crossing material. F-024 also
  closes part of F-017 territory for the cases F-024 applies to.
- **AS001 air_cut_percentage: 77% → 37%.** Same story — air-cut was
  inflated by the broken grid reporting cells as "uncut" when they
  were below the cutter trajectory.

These are NOT independent improvements; they're all the same Z-frame
bug surfacing through different metrics. Worth flagging that F-017's
"rapid collisions everywhere" finding will shift after F-024 — AS013
(844 collisions) won't move (origin_z=0), but AS001/AS002 will be
clean now.

## Open queue trim after this round

Recommend re-ranking when round-05 starts:

- **F-024**: close (5 PRs across 3 sites, smoke-verified on AS001).
- **F-017** (rapid collisions): re-frame. AS001's 36 collisions
  disappeared along with the frame fix. AS013's 844 remain — that's
  the actual unsolved class of bug, not the 36-pocket case.
- **F-018** (test templates) and **F-014** (docs) — closed.
- Need an AS013-deflection investigation finding (F-026? F-027?)
  if a round-05 smoke confirms AS013's 573µm is real overload at
  the commanded feed — or another latent bug if not.

## Loop process learning

**For autonomous_auditor.md / implementer_contract.md updates:**

When a finding's fix lives in a multi-pipeline codebase (core + viz +
CLI variants), the implementer's PR acceptance test should:

1. Test through the FRONT door, not the implementation-layer they
   touched. If the production path is MCP / GUI, the test should
   call the same entry point the MCP / GUI uses.
2. If that's not possible from a unit test (Controller state etc.),
   the implementer should flag it as a hard stop — not declare done
   on an implementation-layer test alone.

Round-04 burned three rebuild cycles because each fix had a
green-test that didn't exercise the path my smoke probe took.
Healthy implementer discipline didn't catch it — only smoke did.

Will draft an addition to `implementer_contract.md` and
`autonomous_auditor.md` in a separate doc PR.

## What's next

Pause. User-facing acceptance loop is in a good state:
- AS001 is the canonical Within-deflection case
- F-024 + F-015 + F-023 + F-016 + F-014 all verified through smoke
- 11 commits landed across rounds 02-04

Next round (round-05) should:
1. Re-sweep the full AS001-AS018 matrix to update the deflection bar properly
2. Investigate AS013's residual 573µm — open new finding if not just real overload
3. F-020 (Optimizer Ranked-BS path) if a test fixture spec is ready
4. F-017 reframe per above
