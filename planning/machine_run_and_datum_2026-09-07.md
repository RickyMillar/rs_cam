# Machine-run setup + datum plan (2026-09-07)

Captured before compaction. Context for the plywood test run of wanaka200 and
the datum-location fixes the operator wants.

## PRIORITY FIX — the work datum belongs in SETUP, not export

**Operator ruling:** the work origin / datum (where the machine is zeroed) is a
property of the SETUP (the physical placement of the board on the bed), NOT an
export-time choice. Today it lives in Export Wizard step 3 "Coordinate & units"
and is invisible everywhere else.

- **Move the datum into the setup model + UI.** A setup already carries
  `face_up` and `z_rotation` (the physical orientation); add the **work-zero**:
  - **Z-zero reference**: stock top (default) / model origin / stock bottom.
  - **XY origin**: stock min corner (default) / stock centre / model origin.
  Surface it in the Setup panel next to face/rotation, and show the resulting
  datum in the viewport (an origin gizmo at the chosen zero).
- **Export CONSUMES the setup datum** — the wizard's step 3 becomes a read-only
  echo (or an override), not the source of truth. This makes single-file and
  split exports agree by construction.
- **Why it matters here:** the single-file export datums at MODEL origin; the
  split export datums at the STOCK surface. They disagree because there is no
  single setup-owned datum. That divergence is the root cause of the whole
  datum confusion below.

## SECONDARY FIXES (found this session)

1. **MCP `export_gcode` must expose the datum** (Z-zero + XY origin), or read it
   from the setup once the priority fix lands. Right now MCP export has no
   coordinate control, so every agent/automation export silently uses
   model-origin.
2. **`split_setups` bugs:** (a) it FAILS when a setup has no enabled/computed
   toolpaths — a front-only run of a two-setup project can't get the stock
   datum. (b) it is IGNORED for single-setup projects, so you cannot force the
   stock-surface datum on a one-setup job. Both block reaching the correct
   datum through the sane path.
3. **wanaka terrain STL Z frame:** `terrain.stl` has Z0 at "sea level", ~7 mm
   BELOW its peak (the stock top). Until the setup datum lands, Z0 = stock top
   needs either the wizard (step 3) or a Z-shift on the .nc. Long-term: the
   setup Z-zero=stock-top setting fixes it with no geometry edit.

## PLYWOOD TEST RUN — current state (front side of wanaka200)

Recipe: front `6 3D Rough (front)` (6 mm EM) → `ISO Scallop (R1.5)` (R1.5 ball,
fine 0.03 mm cusp). Pencil + rivers disabled (saved for later). Material set to
**Baltic Birch Plywood**; feeds RE-APPLIED for plywood via `apply_feeds`
(rough 1100/16500, scallop 925/18500).

**Verified:** tool-load gates all WITHIN, 0 exceedances; 0 collisions /
0 rapid-through-stock at 0.2 mm (fine); rough entries safe (rapids are travel,
helical entries pass the `entry_load` check; deflection 8 µm, power 0.02 kW).
Runtime ≈ 168 min. Machine: Shapeoko Pro XXL, 1.5 kW, 24k rpm.

**Deliverables (scratchpad):**
- `wanaka_plywood_test_front.toml` — the project.
- `wanaka_plywood_test_front.nc` — MCP export (model-origin Z, i.e. stock top
  at Z+7; modulated feeds up to F1807).
- `wanaka_plywood_front_Z0stocktop.nc` — SAME program with Z shifted −7 so
  **Z0 = stock top** (rapids +5, cuts to −9.8). Interim datum fix. Tool change
  + arcs preserved.

**Still open for the run (operator choice):**
- Datum properly: set Z-zero=stock top (and XY=stock corner) in Export Wizard
  step 3 and re-export from the GUI — OR use the Z-corrected .nc. (Permanent
  home = the SETUP fix above.)
- Feeds: exported feeds are modulated to the plywood band MAX (within, but no
  margin). Conservative base feeds (F1100) available on request — MCP can't
  toggle modulation, so this needs the GUI export or a CLI
  `--no-adaptive-feed-modulation` re-export (verify CLI tool-change handling).
- XY datum still = terrain corner (20 mm X / 25 mm Y inset from stock corner);
  can normalise to stock corner.
- **Air-cut first** (raise Z origin ~20 mm) to confirm datum + motion.

## Other durable state (this session)

- **`pencil-entry-ramp-12deg` branch** (NOT merged, NOT pushed): commit
  `52c2d054` = ENTRY_RAMP_MAX_ANGLE_DEG 8→12 (sentries+lint green);
  `9a593b52` = pencil investigation record.
- **Pencil investigation** fully recorded in
  `planning/pencil_linking_2026-09-04.md` + memory
  `project-pencil-entry-and-define-dont-finish`. Headline: pencil is
  entry-dominated by FRAGMENTATION; the win is "define, don't finish"
  (rough + projected rivers = 5.4× vs rough+scallop+pencil). Rivers-definition
  fixture: `scratchpad/wanaka_front_rivers.toml`.
- Uncommitted, leave alone: `.mcp.json`, `wanaka200_iso_scallop.toml`
  (foreign/pre-existing modifications).
