# Phase 0 Fixture Corpus

Six small toolpath fixtures used to anchor encoder behavior against Fusion reference output. Each is **deterministic, tiny, and exercises one encoder concern**. Geometry is intentionally trivial so a human can manually replicate it in Fusion CAM (or hand-craft an equivalent Manual NC operation).

All fixtures use:
- Units: **mm**
- Default spindle: **18000 RPM**
- WCS: **G54**
- No coolant unless noted
- Tool: 6.35 mm flat end mill (T1) unless noted

Fixtures are defined in code at `crates/rs_cam_core/tests/gcode_phase0_capture.rs`. Run with:

```bash
cargo test --test gcode_phase0_capture -- --ignored --nocapture
```

Capture writes one `.nc` per (fixture × dialect) to `planning/gcode_current_outputs/<fixture>_<dialect>.nc`.

---

## F1 — `basic_lines`
**Purpose:** preamble, postamble, basic G0/G1, modal feed elision.

```
rapid_to(  0,   0,   5)
feed_to ( 10,   0,  -2, F=600)
feed_to ( 10,  10,  -2, F=600)   # F should elide (modal)
feed_to (  0,  10,  -2, F=600)   # F should elide
feed_to (  0,   0,   5, F=1000)  # F changes → emit F1000
```
**Watches:** `F` modal elision; the final ramp-out at higher feed.

---

## F2 — `arcs_xy`
**Purpose:** arc encoding (G2/G3 with IJK), plane selection (G17), arc center math.

```
rapid_to( 10,   0,   5)
feed_to ( 10,   0,  -2, F=600)
arc_cw_to ( 0,  10,  -2,  I=-10, J=  0, F=600)   # quarter CW arc, R=10
arc_ccw_to(-10,  0,  -2,  I=  0, J=-10, F=600)   # quarter CCW arc, R=10
feed_to (-10,   0,   5, F=1000)
```
**Watches:** `G2 X0 Y10 I-10 J0` form; `G3` form; whether `Z` is suppressed (it's unchanged at -2); whether `G17` is emitted.

---

## F3 — `helical_ramp`
**Purpose:** helical arc encoding (X+Y+Z+IJK in one block); whether helical detection works.

```
rapid_to( 10,   0,   5)
feed_to ( 10,   0,   0, F=300)
arc_cw_to ( 0,  10,  -1,  I=-10, J=  0, F=600)   # quarter helix down 1mm
arc_cw_to (-10,  0,  -2,  I=  0, J=-10, F=600)
arc_cw_to (  0, -10,  -3,  I= 10, J=  0, F=600)
arc_cw_to ( 10,   0,  -4,  I=  0, J= 10, F=600)
feed_to ( 10,   0,   5, F=1000)
```
**Watches:** Z must appear in the arc block (not as a separate G1); helical descent of 4 × 1mm.

---

## F4 — `profile_multipass`
**Purpose:** multiple Z-stepped passes; modal Z behavior; how the encoder handles repeated XY rectangles at descending Z.

```
for z in [-2.0, -4.0, -6.0]:
    rapid_to(  0,   0,   5)
    feed_to (  0,   0,   z, F=300)
    feed_to ( 20,   0,   z, F=600)
    feed_to ( 20,  10,   z, F=600)
    feed_to (  0,  10,   z, F=600)
    feed_to (  0,   0,   z, F=600)
```
**Watches:** at each new pass, only `Z` should change in the plunge G1; subsequent XY moves should suppress Z.

---

## F5 — `two_tool_changes`
**Purpose:** tool-change sequence between phases (M5 → retract → T+M6 → S+M3 → return).

```
phase 0: tool 1, RPM 18000, label "Op 0 — pocket T1"
    rapid_to(  0,   0,   5)
    feed_to ( 10,   0,  -2, F=600)
    feed_to ( 10,  10,  -2, F=600)

phase 1: tool 2, RPM 24000, label "Op 1 — finish T2"
    rapid_to( 20,   0,   5)
    feed_to ( 30,   0,  -1, F=300)
    feed_to ( 30,  10,  -1, F=300)
```
**Watches:** between phases — coolant off (none here), spindle stop (M5), Z retract, T2 + M6, S24000 M3, return to safe height. Order matters; this is the safety-critical sequence.

---

## F6 — `two_setups`
**Purpose:** multi-setup boundary; M0 pause between setups; safe-Z retract.

```
setup "Top":
    phase 0: tool 1, RPM 18000, "Pocket"
        rapid_to(  0,   0,   5)
        feed_to ( 10,   0,  -2, F=600)
        feed_to ( 10,  10,  -2, F=600)

setup "Bottom":
    phase 0: tool 1, RPM 18000, "Profile"
        rapid_to( 20,   0,   5)
        feed_to ( 30,   0,  -1, F=300)
        feed_to ( 30,  10,  -1, F=300)

safe_z = 25.0
```
**Watches:** retract to Z=25 between setups, M0 pause comment ("Setup change: Bottom"), spindle restart on the second setup.

---

## How to reproduce in Fusion (for reference output)

For each fixture, the simplest Fusion path is **Manual NC operations** chained together:

1. Open Fusion → Manufacture workspace.
2. Create a new Setup with a 50×50×30 mm rectangular stock, WCS at top-front-left corner, output coordinate system G54.
3. For each move in the fixture, add a *Manual NC* operation:
   - Type: `Pass through`
   - Text: the literal g-code line, e.g. `G1 X10 Y0 Z-2 F600`
4. For tool-change fixtures (F5/F6): create the second op with a different tool *before* the second Manual NC group; Fusion will inject its own tool-change sequence (which is what we want to compare against).
5. For multi-setup (F6): clone the setup; the second setup will get its own preamble.
6. Post-process with the target post (Grbl / LinuxCNC / Mach3). Save as `tests/posts/<dialect>/<fixture>.expected.nc`.

**Caveat — Manual NC operations bypass Fusion's planner.** That gives us *encoding-only* parity tests, which is exactly what Phase 4 wants. Algorithm-level parity (does Fusion's pocket strategy match ours?) is **not** a goal — we have different planners.

**Alternative — Autodesk's HSM Post Editor (`Autodesk/cam-posteditor`, MIT-licensed VS Code extension):** ships sample `.cnc` intermediate files that can be posted through any `.cps` without needing the full Fusion installation. This may be a faster path for whoever generates the references. Investigate in Phase 4.
