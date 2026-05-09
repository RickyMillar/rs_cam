# Post Reference Notes — Fusion `.cps` survey

**Source:** `cam.autodesk.com/posts/posts/{grbl,linuxcnc,mach3mill}.cps`
(local copies in `reference/posts/`, gitignored — see `GCODE_EXPORT_OVERHAUL.md` Phase 0 license decision)

**Revision pinned for these notes:**
- `grbl.cps` rev 44214 / 2026-02-17, 2114 lines
- `linuxcnc.cps` rev 44220 / 2026-04-01, 2570 lines
- `mach3mill.cps` rev 44214 / 2026-02-17, 2788 lines
- `grblhal.cps` — **not published by Autodesk.** Will source a community post in Phase 4.

All three posts share the same Fusion HSM kernel — `commonFunctions.cpi`, `coolant.cpi`, `writeWCS.cpi`, `writeToolCall.cpi`, `startSpindle.cpi`, `onRapid_fanuc.cpi`, `onLinear_fanuc.cpi`, `onCircular_fanuc.cpi` are inlined into each. **Differences are surprisingly small and live almost entirely in `onOpen` / `onClose` / `extension` / format decimals / coolant table / a handful of properties.** This is excellent news for the data-driven `PostDefinition` plan: the emitter is essentially one function with per-dialect knobs.

---

## Cross-dialect at a glance

| Aspect                       | Grbl              | LinuxCNC          | Mach3 (mill)      |
|------------------------------|-------------------|-------------------|-------------------|
| Filename extension           | `.nc`             | `.ngc`            | `.tap`            |
| Wraps program in `%`         | No                | **Yes** (begin & end) | No            |
| XYZ decimals (mm / inch)     | 3 / 4             | 3 / 4             | 3 / 4             |
| Feed decimals (mm / inch)    | 1 / 2             | 2 / 3             | **0 / 1**         |
| Default `useM06`             | `false`           | `true` (hardcoded)| user property, default true |
| Default `splitFile`          | `none` (configurable: tool / toolpath) | n/a       | n/a               |
| Default `showSequenceNumbers`| `false`           | n/a (own property) | n/a              |
| `maximumSequenceNumber`      | unlimited         | 99999             | unlimited         |
| Comment chars                | `(...)`, ignoreCase | `(...)`, **upperCase** | `(...)`, ignoreCase |
| Default highFeedrate (mm/min)| 5000              | 5000              | 5000              |
| Coolant FLOOD (M-code)       | `M8`              | `M8`              | `M8`              |
| Coolant MIST (M-code)        | unsupported       | `M7`              | `M7`              |
| Coolant OFF                  | `M9`              | `M9`              | `M9`              |
| Program end                  | `M30`             | `M30` then `%`    | `M30`             |
| Length compensation (`G43`/`H`) | **off**        | on by default     | on by default     |
| Radius compensation (`G41/G42`) | **off**        | on                | on                |
| `useRadius` option (R-format arcs) | no            | yes (default off) | yes (default off) |
| 3D arcs (G2.4/G3.4 fallback) | no               | optional          | optional          |
| Tolerance default (mm)       | 0.002             | 0.002             | 0.002             |

**Identical across all three:**
- Min chord 0.25 mm, min arc radius 0.01 mm, max arc radius 1000 mm, min sweep 0.01°, max sweep 180°
- Helical arcs allowed, all planes (G17/G18/G19) allowed
- ABC format: 3 decimals (degrees)
- RPM format: 0 decimals
- Dwell (`G4 P`) format: 3 decimals (seconds)

---

## 1. Grbl (`grbl.cps`)

### Preamble
```
(programName)
(programComment)
(machine vendor / model / description)
(tool list, one per tool: T1 D=6.35 CR=0 ...; with global Z range if 3D job)
G90 G94
G17
G21                     ; or G20 for inch
```
**No initial `%`. No length-comp init (G43 is disabled). No mode-cancel header (no G40/G49/G91.1).**

### Per-section opening
```
                        ; blank line
(operation comment)
G53 Z0                  ; or G28 / clearance height — depends on safePositionMethod prop
T1                      ; only if useToolCall=true; M6 only if useM06=true
(CHANGE TO T1)          ; comment when not using M6
(tool comment)
S18000 M3               ; spindle on, before WCS
G17 G90 G94             ; redundant-safe modal restate
G54
M8                      ; coolant flood (only if assigned)
G0 X10 Y10              ; preposition XY
G0 Z5                   ; preposition Z (if needed)
```

### Tool change sequence (when `useM06=true`)
1. `M9` (coolant off, if was on) — issued by `onSection` block before retract
2. `M5` (spindle stop)
3. `G53 Z0` (retract Z)
4. *(blank line)*
5. `(operation comment)`
6. `T<n> M6`  *(or just `T<n>` + comment if useM06=false)*
7. `(tool comment)`
8. `S<rpm> M3` (spindle restart with new speed)
9. modal restate, WCS, coolant on, preposition

**No spindle warmup dwell.** Grbl 1.1 typically auto-handles spindle ramp internally. (Note for Validator Phase 1: this is per-machine tuning — make warmup dwell a `PostDefinition` setting, not assumed-required.)

### Motion conventions
- `onRapid` writes `G0 X.. Y.. Z..` — all axes in one block, no per-axis splitting
- `onLinear` writes `G1 X.. Y.. Z.. F..` — `F` only when changed (modal elision via `feedOutput.format`)
- Axis word elision: any axis whose value is unchanged is omitted (modal `xOutput`/`yOutput`/`zOutput`)
- Motion mode (`G0`/`G1`/`G2`/`G3`) is modal — emitted only on change (`gMotionModal`)
- Plane (G17/18/19) emitted on plane change

### Arc conventions
- **IJK only** (no R-format option)
- Helical arcs: emit XYZ + IJK in one line (line 439)
- Full circle: linearizes if helical; otherwise emits with IJK only (no end-point words)
- IJK are computed as `cx-start.x`, `cy-start.y`, `cz-start.z` (incremental from arc start)
- I/J/K outputs are CONTROL_FORCE — always written, even if zero
- After every arc, `forceCircular(plane)` resets X/Y/I/J so the next move re-emits them

### Program end (`onClose` → `writeProgramEnd`)
```
                        ; blank line
M9                      ; coolant off
G53 Z0                  ; retract Z
G53 X0 Y0               ; home XY (settings.retract.homeXY.onProgramEnd)
M5                      ; spindle stop
M30
```

### Notable Grbl-specific properties
- `splitFile` — `"none"` / `"tool"` / `"toolpath"`. When `tool`, emits a master file plus one sub-file per tool change; when `toolpath`, one sub-file per operation.
- `safePositionMethod` — `"G28"` / `"G53"` / `"clearanceHeight"`
- `useToolCall` — disable `T<n>` output entirely
- `useM06` — disable `M6` (Grbl ignores it; some controllers don't)

---

## 2. LinuxCNC / EMC2 (`linuxcnc.cps`)

### Preamble
```
%                       ; <-- LinuxCNC requires program tape begin
(programName)
(programComment)
(machine info)
(tool list)
G90 G94 G17 G91.1       ; G91.1 = arc IJK are INCREMENTAL (LinuxCNC default is absolute IJ!)
G21                     ; or G20
```
**The `G91.1` is critical** — without it LinuxCNC interprets I/J as absolute coordinates of the arc center, which breaks every Fusion-style arc. Grbl and Mach3 default to incremental IJK, so they don't need it; LinuxCNC does.

### Per-section opening
Identical structure to Grbl, but:
- Tool call line is always `T<n> M6` (`useM06 = true` is hardcoded as a `var` at the top of file, not a user property)
- Length compensation: `G43 H<n>` would be supported here but `outputToolLengthCompensation` is unset (defaulted true via kernel?) — check actual output to confirm
- `setCoolant` may emit `M7` (mist) when assigned, in addition to `M8` (flood)

### Tool change sequence
Same retract → coolant-off → spindle-stop → (blank) → comment → `T<n> M6` → comment → `S<rpm> M3` → modal restate → WCS → coolant → preposition.

### Motion / arc conventions
- Same Fanuc-style `onRapid` / `onLinear` / `onCircular` as Grbl (literally the same `.cpi` includes)
- **Adds R-format support** via `useRadius` user property (default off): when on, emits `G2/G3 X.. Y.. R<radius> F..`. Negative R for sweeps > 180°. Full circles linearize in R mode.
- **Adds 3D-arc fallback** via `allow3DArcs` property: emits `G2.4`/`G3.4` (LinuxCNC extension) for arcs whose plane isn't axis-aligned. Defaults off.
- Has parametric feeds (`#100=...` then `F#100`) via `useParametricFeed` property — keep in mind for the validator (treat `F#nnn` as opaque).

### Program end
```
                        ; blank line
M9                      ; coolant off
M5                      ; spindle stop
G53 Z0                  ; retract Z
M30                     ; program end
%                       ; <-- closing tape mark
```

---

## 3. Mach3 (mill, `mach3mill.cps`)

### Preamble
```
(programName)
(programComment)
(machine info)
(tool list)
G90 G94 G91.1 G40 G49 G17    ; explicit cutter-comp cancel + length-comp cancel
G21                          ; or G20
```
**No `%` wrapping.** Includes G40/G49 in header — Mach3 sometimes boots with stale modal state, so explicit cancels protect against the previous program leaving comp on.

### Per-section opening
Same overall structure. Notable difference: `writeToolCall` always uses M6 if `useM06` property is true (default true) and supports `preloadTool` (writes `T<next>` after the M6 to preload the carousel).

### Tool change sequence
- `M9` coolant off (if on)
- `M5` spindle stop
- Z retract
- *(blank line)*
- comment
- `T<n> M6` (and optional `T<next>` preload)
- `S<rpm> M3`
- modal restate, WCS, coolant, preposition

### Motion / arc conventions
Identical Fanuc-style core. Same `useRadius` and `allow3DArcs` properties as LinuxCNC. **Decimals on feed are different**: Mach3 emits `F600` (integer, mm/min) instead of `F600.0`. This is the #1 thing to get right when comparing byte-for-byte.

### Program end
```
                        ; blank line
M9
M5
G53 Z0
M30
```
No `%`. No XY home by default.

---

## What this means for `PostDefinition`

The TOML schema needs at minimum these fields to cover all three dialects without code:

```toml
name           = "grbl"
extension      = "nc"
units_default  = "mm"

[wrapper]
header_percent = false        # LinuxCNC: true
footer_percent = false        # LinuxCNC: true

[decimals]                    # by units
xyz_mm   = 3
xyz_inch = 4
feed_mm  = 1                  # LinuxCNC: 2, Mach3: 0
feed_inch = 2                 # LinuxCNC: 3, Mach3: 1
abc      = 3
rpm      = 0
seconds  = 3

[modal]
include_g91_1_in_preamble  = false   # LinuxCNC: true
include_g40_g49_in_preamble = false  # Mach3: true
emit_redundant_motion_mode = false   # all three: false (modal elision)
emit_redundant_axis_words  = false   # all three: false
group_xyz_in_one_block     = true    # all three: true
restate_modals_each_section = true   # all three: true (G17 G90 G94 line)

[arc]
format             = "ijk"           # alternatives: "r", "auto"
ijk_force_output   = true            # always emit I/J/K even if zero (CONTROL_FORCE)
ijk_increment_from = "start"         # incremental from arc start point
helical_supported  = true
full_circle_action = "ijk_no_endpt"  # alternatives: "linearize", "split"
max_sweep_deg      = 180
min_radius_mm      = 0.01
max_radius_mm      = 1000

[tool_change]
use_m6                 = false       # LinuxCNC: true, Mach3: true
sequence = [
    "coolant_off",
    "spindle_stop",
    "z_retract",
    "blank_line",
    "operation_comment",
    "tool_select",                   # T<n> [M6]
    "tool_comment",
    "spindle_start",                 # S<rpm> M3/M4
    "modal_restate",                 # G17 G90 G94
    "wcs",                           # G54..G59
    "coolant_on",
    "preposition_xy",
    "preposition_z",
]
warmup_dwell_ms        = 0           # most posts skip; expose as knob

[program_end]
sequence = [
    "blank_line",
    "coolant_off",
    "z_retract",
    "spindle_stop",                  # Note: Grbl does spindle_stop AFTER home_xy; LinuxCNC/Mach3 do it before retract
    "home_xy",                       # only if config asks
    "m_program_end",                 # M30
]
m_program_end = 30                   # alternative: 2

[coolant.flood]      on = "M8"  off = "M9"
[coolant.mist]       on = "M7"  off = "M9"   # Grbl: not supported
[coolant.off]        on = ""    off = "M9"

[comments]
prefix     = "("
suffix     = ")"
case       = "ignore"                # LinuxCNC: "upper"
allowed    = " a-z 0-9 . , = _ - * : "
max_length = 80

[limits]
max_sequence_number = 0              # 0 = unlimited; LinuxCNC: 99999
max_tool_number     = 9999
```

Per-dialect TOMLs in Phase 3 only need to override the fields that differ — the bulk is shared. **No template engine needed for any of these decisions** — `str::replace` on simple `{var}` placeholders in coolant/program-end command strings is sufficient.

## Things we do NOT need to support in Phase 3

These appear in the .cps but are out of scope for our 3-axis-router use case:

- 5-axis (`onRapid5D`/`onLinear5D`/`positionABC`/work-plane tilt) — we're 3-axis only
- Tilted work plane / Euler conventions — same
- Rigid tapping (`G33.1`, Mach3) — out of scope
- Drilling cycles (`G81`/`G82`/`G83`/`G84`/`G85`/`G86`/`G87`/`G89`) — we don't emit canned cycles, we emit point-to-point linears
- TCP / radius compensation — disabled in Grbl, irrelevant for our IR
- Subprograms — Mach3 emits `M98 P<n>` blocks; we emit flat g-code
- Probing macros — out of scope
- Inverse-time feed (`G93`) — only used for 5-axis
- Smoothing toggles (LinuxCNC `G64 P0.05`) — could add later as a knob

The reference output we GENERATE in step 4 of Phase 0 should use a fixture job that intentionally avoids all of the above so the diffs are tractable.

## Open questions for fixture generation (Phase 0 step 4)

1. Should the Fusion fixture jobs use **mm or inch**? Recommend mm (matches our default and most rs_cam projects).
2. Do we want fixtures with `useToolCall=true, useM06=true` (Grbl) so the tool-change sequence is exercised, or stick with Grbl defaults? Recommend **enable M6 in Grbl fixture** so all three dialects exercise the same code paths.
3. Coolant on/off in fixture — recommend **flood on** for all three (forces the coolant table to differ visibly between dialects).
4. WCS — Fusion defaults to G54. Keep that.
5. Sequence numbers — leave off for all three (matches all three defaults; one less source of diff).
