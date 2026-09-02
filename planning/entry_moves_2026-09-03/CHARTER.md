# G-RAMPTERRAIN — stock-aware entry moves (charter, 2026-09-03)

> Status 2026-09-03: family member 1 (ramp/helix entries) **FIXED,
> sentry standing.** Fix (a) shipped — ramp legs and helix turns clip
> to the drop-cutter floor; lost contact degrades to the plunge
> (finish door). The audit, bars, measurements, and two design
> amendments are in `FINDINGS.md`. In progress under the widened
> scope: member 3 (lead-in/out) and member 2 (sagging refit arcs).
> Open after that: triage safety-row promotion, live wanaka
> re-measure (the GUI-owning lane).

## Operator ruling

> "All entry moves should be stock aware."

Not an op-type refusal list — the design target is that NO entry motion
(ramp, helix, plunge, lead-in) may pass through standing stock or the
model surface blind.

## The defect, as found (operator-caught in the viewport)

The ramp-entry dressup (`entry_style = "ramp"`, 3°) replaces each
plunge with straight zigzag legs between two points, TERRAIN-BLIND. On
the wanaka ISO scallop op (33+ entries): **877 feed chords, 19–21 mm
long, ~1.0 mm drop each** (3° over ~19 mm), sawing straight through
ridges. Measured from exported G-code: chord midpoints up to 2.6 mm
below the local terrain z-max. Repro fixture:
`planning/multitool_2026-08-23/wanaka200_iso_scallop.toml` — set the
ISO op's `entry_style` back to `"ramp"` and regenerate; the G-code
chord analysis is in the session record
(`planning/metrology_2026-09-02/FINDINGS.md`, G-ISOCHANNEL addendum).

## SCOPE WIDENED same day — three stock-blind classes, one family

Follow-up A/B analysis attributed the operator's observations to THREE
dressup-layer classes (record: `planning/metrology_2026-09-02/FINDINGS.md`,
"stock-blind approach FAMILY"): (1) ramp entries — 20 mm saw-legs;
(2) `arc_fitting` — sagging refit arcs, source of the entry_load
CRITICAL and likely the fillet speckles; (3) `lead_in_out` — horizontal
approach at ring depth through standing terrain. The ruling covers the
family: ramps, helixes, lead-in/out, and arc refits near entries must
all be stock-aware (or refuse loudly). The buried-chord G-code
analysis found all three; build it as the standing sentry first.

## Scope notes for the next session

- **This is OLD and broad, not an iso-field defect.** The scallop op's
  DEFAULT dressups carry `entry_style = "ramp"` + `lead_in_out = true`
  (other ops in the same project default to `"none"`). The legacy
  cascade scallop has shipped with ramping entries; it merely has few
  enough entries that nobody saw the channels. Audit which op types
  default to ramp and which projects carry it.
- Two detection blind spots let it through, both now recorded:
  the rapid checker only audits rapids (these are feeds), and the
  envelope-residual coverage ruler only reads positive residual.
  The triage `entry_load` critical DID fire — it is currently the
  only wire that sees this class. Consider promoting deep-biting
  entry chords to a SAFETY row, not a load caution.
- The helical-link fix (`62237834`) is a separate, real, already-fixed
  class — do not conflate.
- Candidate fixes, in ascending ambition: (a) ramp legs clipped to the
  drop-cutter surface along their XY line (the dressup layer needs a
  surface probe handed in); (b) entries routed along the machined
  surface (the surface_link machinery already does gouge-checked
  linking — reuse it for entries); (c) refuse+warn where no surface
  context exists. The operator ruling wants (a)/(b), with (c) only as
  the fallback.
- Immediate mitigation in place: the wanaka ISO project is saved with
  `entry_style = "none"` (vertical plunges — stock-aware by
  construction of the drop-cutter target z, at plunge feed).
- Verification for any fix: the G-code chord analysis (long feed
  chords vs terrain z-max) — the operator's eye found it; the analysis
  script reproduces it in seconds. Turn it into a standing sentry:
  no cutting-intent chord's midpoint may sit below the local envelope
  by more than tolerance.
