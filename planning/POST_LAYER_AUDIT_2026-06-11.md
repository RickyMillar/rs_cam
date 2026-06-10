# Post-layer (G-code) audit — 2026-06-11

**Trigger:** a static scan of one real WANAKA export found four defects
(fixed in 4544e9a: per-op RPM flattening, tool-number-keyed change
detection, raw M6 on GRBL, nested-paren comments, diagonal rapids).
This audit swept the whole layer for more, before first machine run.
**Method:** full read of `gcode/` + posts TOMLs + validator + all four
export consumers; every finding verified against code on disk.
Emulator-validation suite has 96/97 tests ignored — coverage on this
layer is thin; that's why a casual scan kept finding things.

## CRITICAL

- **C1 — export gates silently unenforced on GUI/MCP/CLI-job paths.**
  `export_gcode_phases_with_overlay_checked` / multi-setup variant take
  `_sim_trace, _policy` and ignore them (TODO stub, gcode/mod.rs
  ~603/663). Every viz export, the MCP handler, and CLI main.rs route
  through these — accept_unmodeled / accept_exceeded toggles do
  nothing; an Exceeds project exports without refusal. Only
  `export_gcode_checked` (session/CLI-project path) enforces.
- **C2 — postamble hardcodes `G0 Z10.000`** (grbl/grblhal/mach3
  TOMLs): WCS-frame literal with no frame knowledge. On positive-Z
  3D jobs (WANAKA: terrain above Z10) it commands a Z-only rapid DOWN
  into the part with a stopped spindle. Fix: SafeZRetract-style
  builder statement / `{safe_z}` token (LinuxCNC's `G53 G0 Z0` is the
  right pattern).
- **C3 — dry-run doesn't clamp rapids.** `dry_run_safe_z` clamps
  Linear/Arc only; drill peck re-entry RAPIDS go to just above the
  previous peck bottom — in dry-run the material isn't removed, so
  G0s drive into solid stock. Clamp Rapid/SafeZRetract too.

## CAUTION

- **A1 — inch mode is cosmetic**: G20 swaps the word, coordinates stay
  mm (25.4× error). Wizard shows an inline warning only. Decide:
  scale in emitter or hard-refuse.
- **A2 — pre/post_gcode Raw splices corrupt modal state**: snippet F
  breaks F-elision for following LinearModal; G91 left active turns
  the rest of the program incremental; G20/G53/WCS words shift later
  phases; snippet motion invalidates rapid-split prev_pos. Resync
  modal state after any splice + warn on dangerous words.
- **A3 — coolant changes suppressed between consecutive same-tool
  phases** (guard inverted: keys on "tool unchanged" instead of
  "tool change fired this phase").
- **A4 — no spin-up dwell after tool-change/M0 resume** (warmup only
  applies after first preamble; mach3 alone has G4 in preamble).
- **A5 — mach3 post has no WCS line at all**; wizard WCS override
  no-ops; validator flags permanent MissingWcs on every Mach3 export.
- **A6 — validator vs linuxcnc post mutually inconsistent** (G91.1, %
  brackets, M30-vs-M2): three permanent Errors on every LinuxCNC
  export; blocks wizard save unless overridden.
- **A7 — M6 posts + colliding display T-numbers**: change fires on
  config id but renders `M6 T1` → `M6 T1`; controller skips the
  physical swap. Warn/refuse at export when distinct tools share a
  T-number on an M6 post.
- **A8 — rapid-split traverses at max(prev_z, target_z), not a known
  safe height.** Latent (generators retract first). Needs safe_z
  threaded into the builder. Backlog with A9.
- **A9 — machine-safety findings (`validate_machine_safety`) are
  log-only**; wizard preview runs format rules only. Surface them.
  (G53-unaware "ends below clearance" false-positive noted.)
- **A10 — ToolChangeMode::Suppress removes the M0 too** — multi-tool
  program flows into the next op with the wrong tool. Keep a stop or
  refuse when >1 distinct tool id.
- **A11 — first tool never announced**; multi-setup seeds
  initial_tool from the first tool-ful phase anywhere (divergent from
  build_phased). Preamble "LOAD: <label> [T<n>]" comment.

## CLEANUP

- `replace_rapids_with_feed`: sound for emitter output (modal-F reset
  after rapids makes the F-insert safe); gaps — converts the postamble
  retract to a feed move, no max_feed clamp, F0.0 when high_feedrate
  is 0, misses G00/lowercase in user snippets.
- mach3 postamble leaves G91 active (G28 G91 Z0, no G90 restore).
- filter_raw denylist bypass: `M6(msg)` / `T1M6` tokens fail the u32
  parse and pass through. Use word-scanning.
- Arc linearization degeneracies: sub-threshold full-circle arc →
  zero-length chord (geometry dropped); tiny-sweep arc with equal
  rounded endpoints reads as full circle to GRBL.

## Leads cleared (verified non-issues)

- IJ 3dp rounding vs GRBL radius check: worst case ≈0.002 mm, below
  the 0.005 mm gate; 0.05 mm linearize threshold sufficient (except
  full-circle degenerate above).
- Modal F-elision resets for rapids/tool-change/setup boundaries all
  correct (only the A2 Raw-splice gap).
- 4544e9a fixes verified: pause-style change, brackets, byte baselines.
- Ordering: pure project order, no grouping (UX gap, not safety —
  candidate for the UI pass).

## Fix order (decided)

1. C1 gate enforcement → 2. C3 dry-run rapid clamp → 3. C2 postamble
safe-Z → 4. A2 splice resync → 5. A5+A6 post/validator consistency →
6. A4 resume dwell → 7. A3 coolant guard → 8. A7 T-collision warning,
A10 suppress-keeps-stop, A11 first-tool comment → 9. cleanups.
A1 = hard-refuse inch for now (no silent 25.4×), proper conversion
backlogged. A8+A9 backlogged together (safe-Z threading + surfacing
machine-safety findings).
