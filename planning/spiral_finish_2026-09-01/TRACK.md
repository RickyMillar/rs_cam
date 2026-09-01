# Track C — shape-selected spiral: EDT offsets + F2 bridging (no slit map)

**Rank:** 3 of 6 (`planning/finishing_status_2026-09-01.md` §5 avenue C).
**Status: DONE (2026-09-01) — BAR MET on both fixtures: sphere 1.113×, dish 1.223× (bar 1.25×), 0 retracts, 0.0000 % unmachined, spacing 0.8 % / 0.7 % off spec, bridge overhead 5.6–5.8 %. Evidence in FINDINGS.md; module `spiral_finish_compact.rs`; instrument `spiral_finish_compact_c1.rs`.**

## The goal

A research-harness proof of the compact-region spiral: nested
iso-distance (EDT/medial) offset rings, bridged into ONE continuous path
with F2's log-rectangle bridging, gouge-checked, on compact geometry.
The conformal slit map is OUT OF SCOPE — it is the part measured 1.87×
slower and stays shelved (`FINDINGS_F2.md` §F2-4 D).

Why this candidate: the medial-axis field scored **1.036× floor,
0 retracts, meets spec** on the sphere — the programme's best honest
number (synthesis §9) — and bridging is field-agnostic (synthesis §2a).
The operator also values the LOOK: continuous spiral toolmarks, no
raster lines, no cell seams, on round showpiece work.

## Pre-registered bar

On each fixture: continuous single path (0 retracts), zero
self-intersections, CoverageAudit passes, achieved spacing within spec,
and total ≤ **1.25× floor** (medial rings 1.036× + bridging overhead
5–13 % gives the expected ~1.09–1.17×; the bar leaves margin without
letting a loser through). Report time under `relink_and_cost_under`'s
realistic regime beside the raster arm. Render SVG/toolpath images for
the operator's appearance judgement — appearance is a first-class
outcome here, not a footnote.

## Fixtures

1. Analytic sphere cap (exists) — convex dome.
2. NEW analytic concave dish (spherical-cap pocket), facets ≤ stepover/3,
   concavity radius comfortably above ball R (gouge-check must engage).
   This is the wood use case (bowls, dishes).

## Evidence rules

- Research module + instrument tests only. NO shipped-behavior change.
- Pre-register falsifiers in FINDINGS.md before each run.
- Score ×floor; count fragments/links/retracts; run the coverage audit.

## Evidence lands here

`planning/spiral_finish_2026-09-01/FINDINGS.md`.

## Productisation charter (2026-09-01, appearance approved by the operator)

Goal: an opt-in compact-region spiral strategy on the shipped surface.

**Revised 2026-09-01 (operator architecture question): the first door is
the EXISTING `SpiralFinish` operation, not a unified_finish dial.**
SpiralFinish today walks a circular Archimedean spiral from the bbox
centre — round rings whatever the region's shape, XY-uniform spacing.
`spiral_finish_compact` is that op's intent done properly: rings that
follow the boundary (EDT offsets), bridged into one continuous path,
spacing on the surface spec. Upgrade path: add a boundary-conformal mode
to SpiralFinish (default stays the legacy circular mode for byte-compat;
mode dial + typed refusal fallback + report). The unified_finish shape
gate (below) becomes step 2, later, reusing the same core module — one
module, two doors, per the house core+wiring rule. The multitool T3
precedent (diagnostics legibility argues for separate ops) supports the
standalone door leading.

Scope:
1. A dial on `unified_finish` (naming to match house style), default
   OFF, that routes a SHALLOW region through
   `spiral_finish_compact` when the region passes the shape gate:
   compactness/elongation (reuse C2's machinery), nested closed EDT
   level sets, and the module's own typed refusals. Any refusal falls
   back to the raster arm and is REPORTED (report-only finding, not a
   silent fallback).
2. The slope derate applies to the spiral's ring spacing the same way
   it applies to the raster stepover — spec is spec.
3. Wire: op panel, MCP schema, project IO, setup sheet, test
   initializers (the CLAUDE.md field-audit rule).
4. Gates: golden default-off byte-identity; sentries for gate/refusal
   arms; full heavy gate; a C4-style rendered pattern review by the
   operator on a real compact region BEFORE any default discussion.

Non-goals: the conformal slit map (measured 1.87× slower — stays
shelved); branched regions (typed refusal is the correct answer there);
any default change in this ticket.
