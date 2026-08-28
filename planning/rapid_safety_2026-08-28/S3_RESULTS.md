# S3 results — the emitter stops planning strikes (2026-08-28)

## The emitter, positively identified

Not `adaptive3d::sample_stock_top_at` (the phase's original indictment — S1
never exercised it and it is untouched). The chain, each link verified this
session:

1. `raster_toolpath_from_grid` (toolpath.rs:607-747) emits a SAFE link at
   every junction: retract rapid → linking rapid at safe_z → **fed
   EntryPlunge**.
2. `dressup::filter_air_cuts` then classified that plunge "all air" with a
   **zero-radius centerline probe** — its `tool_radius` parameter was
   documented *"reserved for future per-cell radius checks"* and never read
   — and converted it to a rapid ending at the resume Z.
3. The filter runs only when a prior stock reaches the dressup chain
   (compute/execute.rs step 7), which is why every measured strike lived in
   the `FromRemainingStock` ops (4/5/7/8) and the fresh ops were clean.
4. The pre-S2 detector shared the identical point-probe blindness — emitter
   and detector masked each other, precisely PLAN.md's S-a/S-b prediction,
   two sites over from where it pointed.

## The fix

`filter_air_cuts` / `swept_path_is_all_air` now take the cutter. A sample is
air for the TOOL, not the centerline: two-stage —

- stage 1: the existing cheap centerline test; MATERIAL verdicts return
  immediately (sound: the centre cell is inside the disc, `h(0) = 0`,
  `conservative_top ≥ ray_top` — a centerline hit implies a disc hit);
- stage 2 (air-side only): `max_clearance_tip_z_for_profile(px, py,
  envelope_r, cutter) + tolerance <= pz`.

The all-or-nothing whole-move rule is unchanged, so a plunge that touches
crest material anywhere stays **fed in its entirety**; the later
`optimize_entry_descents` pass re-splits its airborne top against a
stock-derived ceiling (already profile-safe direction: envelope disc,
conservative). Callers migrated (11 sites; the GUI worker now builds the
cutter unconditionally — it previously only existed when feed-opt stock did,
and the filter would have silently lost tool-awareness on the GUI path).

Sentries: `tests/air_filter_tool_aware_s3.rs` — the measured class stays
fed (red against pre-fix code); a genuinely-tool-clear link still converts
(efficiency guard); taper flank clears what a flat of the same envelope
does not (tool-aware, not envelope-paranoid); end-to-end raster link over a
crest → fed plunge → S2 live check reports zero, while a hand-rebuilt
pre-S3 emission is flagged by the same detector (vacuity guard).

## Falsification — all three legs

**1. Pipeline (fixed detector watching):** same CLI command as S2's run —

| op | S2 (pre-S3) | post-S3 |
|---|---|---|
| 5 Lakes | 1 | **0** |
| 7 3D Finish | 179 | **0** |
| 8 Pencil | 22 | **0** |
| all others | 0 | 0 |

Not silence — the S2 live profile-aware detector was watching, and the
motion moved the right way: cutting distance 405,496 → 420,726 mm, rapids
178,339 → 147,485 mm (the plunges returned as feeds).

**2. Independent instrument:** the S1 replay pointed at the fresh emission
(split of `--emit-gcode`, via the instrument's new `S1_SETUP1_NC` /
`S1_SETUP2_NC` overrides): **982 strikes → 2**, both inside the
instrument's own conservatism noise floor (1 in (−0.05, 0], 1 in
(−0.15, −0.05], **zero beyond discretisation**). By the histogram
discipline S1_RESULTS established, that is clean. Note: the banner still
prints `STRIKES FOUND` because the verdict counts any nonzero — read the
histogram, not the banner; the 826 near-misses (< 0.5 mm clearance) are
the safe pattern operating close to stock **by design** (rapid to ceiling +
clearance, feed the rest).

**3. Sentries both directions:** the S2 crest sentries stay green (the
detector still flags the class S3 no longer emits — the fix did not pass by
weakening the detector), and S3's own red-first sentry pins the emitter.

## Costs and honest notes

- **Cycle-time estimate +9.6%** (74,843 → 82,056 s) and air-cut% 55.6 →
  60.6 on this project: fed air returned where conversion was unsafe. Some
  is recoverable by profile-tightening `optimize_entry_descents`' ceiling
  (finishing programme Track B2, not S3). Air-cut% on tapered tools also
  carries Phase M's caveat.
- **Scope wider than planned, in the conservative direction**:
  `prior_stock` reaches the filter UNGATED by stock source
  (session/compute.rs:1823-1830) — a Fresh op regenerated after any
  simulation also runs it. Consequence: a Fresh op's air-cut%/rapid splits
  can differ between first-generate and regenerate-after-sim. Pre-existing
  routing, unchanged by S3; the classifier it feeds is simply correct now.
- **Perf**: no measurable blowup — generate phases within seconds of the
  pre-fix run (stage-1 short-circuit carries the weight). The tile-mip
  early-out remains pre-scoped if a pathological case appears.
- The S1 instrument run on the fresh emission took 579 s vs 299 s on the
  shipped files — more airborne motion survives to be adjudicated
  (5,460 retract-exempt vs 4,206; 1,485 fine adjudications). Instrument
  cost, not product cost.

## Phase state after S3

- S-a (adaptive3d floor): still a code-read finding, unexercised by this
  job, unfixed — now the LIVE detector would catch it on any job that
  exercises it, which is the mitigation PLAN.md wanted from S2.
- S-c (rapids exempt from holder/fixture checks) and S-d
  (`check_collisions*` vs mesh only): open → S4.
- S5 sentry list: largely discharged by the S2/S3 sentry files + the
  instrument; remaining: a sub-cell sliver red-first case.
