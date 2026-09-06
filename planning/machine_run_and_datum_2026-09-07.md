# Machine-run setup + datum plan (2026-09-07)

Captured before compaction. Context for the plywood test run of wanaka200 and
the datum-location fixes the operator wants.

## PRIORITY FIX — export must CONSUME the setup datum (IMPLEMENTED, 2026-09-07)

**Operator ruling:** the work origin / datum (where the machine is zeroed) is a
property of the SETUP, NOT an export-time choice.

**Correction to the premise (found during implementation):** the datum ALREADY
lives in the setup and always did. `SetupData.datum` carries `XYDatum`
(`CornerProbe(Corner)` / `CenterOfStock` / `AlignmentPins` / `Manual`) and
`ZDatum` (`StockTop` / `MachineTable` / `FixedOffset(f64)` / `Manual`), it is
edited in the Setup Properties panel next to face/rotation
(`crates/rs_cam_viz/src/ui/properties/setup.rs`), and it round-trips through
project IO (`tests/setup_datum_round_trip_p2.rs`). Export Wizard step 3 holds
WCS (G54..G59) + units + safe-Z, NOT the datum — there was nothing to demote.

**The real gap:** export never CONSUMED `setup.datum`. `export_datum_shift`
hardcoded XY = stock-min-corner and **Z = 0 (world frame)**, ignoring the
setup's `z_method`. So a 3D job whose stock top sits above world Z0 (wanaka
terrain at +7) emitted Z0 at the MODEL origin, 7 mm below the stock top — the
operator's "the z should be stock top!" and the reason the .nc needed Z-surgery.

**What landed (Phase 1):**
- `SetupEvalContext` gained a `z_datum` field (from `setup.datum.z_method`);
  `export_datum_shift` now sets Z from it. `ZDatum::StockTop` (the default)
  puts the emitted-frame stock top at program Z0 (`-heights_stock_bbox.max.z`).
  On a 2D stock already at world Z0 the shift is 0 → byte-identical. A flipped
  setup zeroes to its presented up-facing surface (`-stock_h`) instead of the
  old implicit spoilboard datum.
- **XY is unchanged** (stock-min-corner = the default `CornerProbe(FrontLeft)`),
  deferred deliberately: XY is never re-zeroed between setups, so a per-setup XY
  datum would reintroduce the G-EXPORT-DATUM two-datum defect. XY needs a
  cross-setup consistency guard before `CenterOfStock`/other corners can ship.
- **`MachineTable` / `FixedOffset` do NOT auto-shift Z yet.** They RAISE the
  frame, which would drive the emitter's fixed positive Z literals (post
  `safe_z` retract, postamble Z, dry-run clamp) INTO the stock (the SAFEZ-LOCAL
  class). `StockTop` only LOWERS the frame, so it is safe. Wiring the raising
  methods needs every non-toolpath Z emission audited first. The export header
  states the declared datum for these; the operator zeroes to it.
- **MCP export inherits it for free** (`io/export.rs` routes through the same
  `export_datum_shift_for_toolpath`), so secondary fix #1 below is DISSOLVED:
  no MCP datum parameter is needed; the datum comes from the setup.
- Files: `crates/rs_cam_core/src/session/eval_context.rs`,
  `crates/rs_cam_core/src/gcode/mod.rs` (module note),
  `crates/rs_cam_viz/src/app/mcp.rs` (split-export header wording). Sentries:
  `tests/export_datum_setup_frame.rs` (rewrote the Z arm + added a 3D-stock arm)
  and `wizard_e2e.rs`. Verified: targeted core + viz export tests green; full
  gate pending.

**Still open (follow-ups, NOT in Phase 1):**
- The Setup panel's datum controls exist but there is no viewport origin gizmo;
  add one showing the chosen zero.
- XY: consume `xy_method` with a cross-setup consistency guard (warn/refuse when
  enabled setups resolve different XY zeros).
- Audit every non-toolpath Z literal, then wire `MachineTable` / `FixedOffset`.

## SECONDARY FIXES (found this session)

1. **MCP `export_gcode` datum — DISSOLVED by the priority fix.** MCP export now
   inherits the setup's datum through the shared shift helper; no coordinate
   parameter is needed. (Was: "MCP export silently uses model-origin.")
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
