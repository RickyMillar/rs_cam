# S1 results — shipped-G-code rapid replay (2026-08-28)

Instrument: `crates/rs_cam_core/tests/rapid_replay_shipped_gcode_s1.rs` (v2),
run `--release` against the two shipped wanaka programs at HEAD. Full logs:
v1 74 s, v2 299 s (fine tier 295 s, 1,430 adjudications, budget not hit).

## VERDICT: STRIKES FOUND — live, but not where the indictment pointed

Descending rapids enter standing material **systematically in the finishing
link descents** (ops 7 and 8). The op the phase indicted — adaptive3d's
rapid-descent floor (S-a) — was **not exercised** by this job and is neither
convicted nor exonerated. Setup 1 is fully clean.

| op | tool | descents probed | strikes | worst (shaved) |
|---|---|---|---|---|
| 1 Pin Drill | T1 | 18 | 0 (18 kerf-grazes, +0.5 own-hole re-entry — correct) | +0.500 |
| 2 Back Rough | T1 | 0 | 0 | — |
| 3 Holes | T1 | 0 | 0 | — |
| 4 Rivers (V-bit) | T20 | 0 | 0 | — |
| 5 Lakes | T8 | 0 | 0 | — |
| 6 3D Rough (adaptive3d) | T1 | 0 | 0 — see "not exercised" below | — |
| 7 3D Finish (drop_cutter) | T15 | **601** | **601 (100%)** | **−1.298 mm** |
| 8 Pencil detail | T6 | 811 | **381** (+153 near-miss, 277 clean) | −0.604 mm |

Strike depth histogram (shaved-radius margin; instrument conservatism stacks
to ~0.10–0.15 mm, so the first two rows are noise-floor):

```
(-0.05,  0.00]   72   at/below conservatism floor
(-0.15, -0.05]  276   inside conservatism budget
(-0.50, -0.15]  492   beyond discretisation — REAL
(-1.00, -0.50]  134   beyond discretisation — REAL
      <= -1.00    8   beyond discretisation — REAL
```

**634 strikes beyond any discretisation explanation.** All 982 are DESCENTS;
zero traverses struck.

## The mechanism, read from the shipped motion

Op 7's worst strike (`wanaka200_2_Setup_2___front.nc:447072`):

```
G1 X167.000 Y220.000 Z2.212      cutting the raster row
G0 X167.000 Y220.000 Z12.000     retract
G0 X168.200 Y220.000 Z12.000     1.2 mm hop
G0 X168.200 Y220.000 Z2.211   ←  rapid straight down to the RESUME POINT'S Z
G1 X168.500 Y220.000 Z2.210      resume cutting
```

The link descent's floor is **the next cut point's surface Z with zero
clearance** — and that Z is the *design* surface. What actually stands there
on a first visit is design + op 6's `stock_to_leave_axial = 0.5`
(`wanaka200.toml:721`) + rough-raster cusps. So the R1.5 taper tip
rapid-plunges through 0.5–1.3 mm of white oak **at G0 rate, on every
inter-run link** — 601 times in op 7 alone. Op 8 (pencil) repeats the
pattern into op 7's rest material (shallower: pencil exists where material
stands proud; 277 of its descents land where op 7 truly finished and are
clean).

Contrast the SAFE patterns in the same programs, which is what isolates the
defect to the finishing link planners:

- op 7's **initial entry**: `G0 Z9` (stock top + 2) then `G1 Z7 F270` — feed
  plunge, correct;
- ops 4/5 (project_curve family on remaining stock): `G0 Z13` then
  `G1 … F496` plunge feed, correct;
- op 1/3 pecks: rapid re-entry to +0.5 above own hole floor, correct
  (adjudicated KERF-GRAZE, margin@shaved exactly +0.500).

Physical read: a fine tapered-ball tip driven through ~1 mm of hardwood at
rapid (5,000 mm/min, no feed-override protection) once per link. Tip
chipping/breakage and burn risk, repeated ~980 times across the two
finishing ops of this one job.

## What this changes in the phase plan

1. **S-a (adaptive3d point-probe floor) was NOT TESTED by this artifact.**
   Op 6's descents all stop at stock top + 2 on this fresh-stock job — the
   suspect code path never emitted a below-top descent. It remains a code-read
   finding (latent class), not a measured one. Do not cite this run as either
   conviction or exoneration of `clearing.rs:1338`.
2. **The measured live defect is the finish link descent floor** (drop_cutter
   links, pencil links): descends to the resume point's surface Z with no
   clearance and no awareness of remaining stock. S3's emitter work should
   target this site FIRST — it is the one with 634 measured strikes — with
   `clearing.rs` second.
3. **S-b/S-d get a concrete falsification target for S2** — and the shipped
   record is now cited, not assumed. The airrun reported **zero collisions on
   all 8 ops** (`planning/airrun_2026-08-19/RUN_LOG.md:16`), and the RUN_LOG
   itself already called `rapid_collision_count` "**vacuous for Setup 2** …
   the collision test ran against a misplaced volume" (`RUN_LOG.md:1243-1245`)
   — while `:130` claims collision checks read the correctly-framed
   `group_stock`. Those two statements conflict; S2 resolves them. More: **the
   operator suspected rapid gouging at the time**, and the ad-hoc audit that
   cleared it (`RUN_LOG.md:1230-1247`) counted only **lateral** rapids below
   stock top (found 0) — a Z-only descent has no lateral travel, so the
   entire striking class was structurally invisible to that check. The
   suspicion was right; the instrument was blind.

   S2's diagnosis priority also shifts: these strikes are **near-axis**
   (margin@env == margin@shaved on the big ones — material under the tip),
   so even a zero-radius point probe at the descent endpoint would see
   1.3 mm of standing material. Point-probe blindness (S-b's original
   theory) cannot explain the silence. Lead hypotheses, in order: (1)
   **timing/stock-state** — if rapids are checked against the op's *final*
   stock, this class vanishes, because the feed move immediately after each
   descent cuts exactly the material the rapid plunged through; (2) the
   misplaced-volume frame defect named at `RUN_LOG.md:1243`; (3) S-d
   (mesh, not remaining stock); (4) a tolerance eating the depth. The fixed
   detector must evaluate each rapid against the stock state **at the time
   of the rapid** — the semantics this instrument just demonstrated — and
   the falsification test is that a normal pipeline run of wanaka200.toml
   then flags ops 7/8.
4. **Convergence with the finishing programme**: these striking links are the
   same retract-hop links Track G's detour observation and the ceiling work
   (§0g/§0h) are about. The links that cost TIME are the links that plunge.
   Fixing the link planner addresses both; the tracks should share the fix
   site.

## Instrument history (honesty trail)

- **v1 reported 651 STRIKES; 10 of 10 inspected were artefacts** — ascending
  Z-only retracts evaluated at their buried start point. v2 exempts
  retract-ascents on a monotone-profile proof (body at higher tip Z occupies
  a strict subset of the column; holder strikes are Phase S4 scope, out of
  this instrument's question). STRIKE now means descent or traverse only.
- Every number above is v2, full adjudication (budget 20,000, 0 dropped).
- **Self-calibration**: op 1's peck re-entries adjudicate at margin@shaved =
  **+0.500 exactly** — the replayed hole floor matches the G-code peck floor
  to sub-cell precision, verifying fine-tier stamping on vertical plunges.
  The op-7/8 strikes show margin@env == margin@shaved, proving near-axis
  material rather than a rim artefact of the conservative disc; and op 8's
  strike/near/clean mix tracks where op 7 finished vs. where rest material
  stands, which a systematic under-removal bug could not produce. (Setup 1
  and op 6 being clean came mostly from the early-out, so the +0.500
  calibration is what carries the stamping-fidelity weight.)
- Setup 2 replayed on conservative fresh stock (its op 6 IS fresh per
  `wanaka200.toml:711`, so this is exact, not conservative, for ops 6–8's
  chain).
- Known conservatisms (~0.10–0.15 mm): cell half-diagonal, half-cell disc
  dilation, `conservative_top` sub-cell over-read. Applied in the histogram;
  strikes ≤ 0.15 mm are not claimed.
- Single job measured. The 100% strike rate on op 7 links makes the finding
  structural rather than statistical, but magnitudes are job-specific
  (they track stock_to_leave + cusp height).
