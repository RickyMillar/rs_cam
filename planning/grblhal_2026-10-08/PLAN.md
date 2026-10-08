# Full grblHAL support (2026-10-08)

Owner: the operator. Lead: the local runner session. Status: ACTIVE.

## Why

The Shapeoko XXL moved from Grbl 1.1 to a grblHAL board with a BitZero
(work zero) and a BitSetter (tool length). The operator wants the board, not
gSender, to run the tool change (`$341=3`, automatic touch off at G59.3), so a
new computer needs no gSender set-up. The review of 2026-10-08 found that the
grblHAL post is the Grbl post less the M6/M7 filter, and that no test checks
the output against grblHAL. Every grblHAL claim cites grblHAL/core@c3a887e
(the sources the review fetched).

## Operator rulings (2026-10-08)

- The board runs the tool change. rs_cam writes `M6 T<n>` on grblHAL.
- Implement every item below. The export UI holds every setting that
  grblHAL needs.

## Packages, in order

Each package has its proving test. A package that changes emitted output
re-blesses the goldens of G9 in the same commit.

| ID | Package | Proof |
|---|---|---|
| G1 | SAFETY: refuse cutter compensation "In Control" when the post has `supports_cutter_comp = false` (the cut is off by the tool radius now). All posts. | an export test that expects the refusal |
| G2 | Tool change on grblHAL: `M6 T<n>` with the tool name in a comment, after M5 and a safe-Z retract. A post option `tool_change = m6 | pause` (grblHAL default `m6`, Grbl stays `pause`). Correct the wrong M6 comment in `posts/grblhal.toml`. | emitter unit tests, golden |
| G3 | Setup pauses (flip, epoxy pour): grblHAL cannot jog in an M0 feed hold (`gcode.c:4981`, `system.c:241`). Two-sided jobs export one file per setup by default on grblHAL. A single-file export keeps M0 and says why in a `(MSG,...)`. GUI per-setup files get the FLIP/DATUM header that MCP `split_setups` writes (parity). | viz parity test, golden |
| G4 | Operator messages: `(MSG,<text>)` at tool changes and setup pauses on grblHAL (`gcode.c:1139`). ASCII only. | golden |
| G5 | Spindle: an option "the controller waits for the spindle" (`$340` at-speed or `$394` delay). On, the post writes no G4 warm-up dwell. S is clamped to the profile's `$30`/`$31` range with a warning. The cycle-time estimate adds the spindle wait from the profile. | unit tests |
| G6 | M7 mist is opt-in on grblHAL (the board rejects M7 without a mist output, `gcode.c:1857`). | `shipped_post_unsupported_mcodes`; `post_format_round_trip_p1` gets a new observable |
| G7 | Drill cycles: the G82 dwell emits `G4` on every expanded post (it is lost now). grblHAL option: native G81/G82/G83 (`gcode.c:1606`), default off; the simulation still uses the expanded motion. | new `f17_drill` captures |
| G8 | Validator rules for grblHAL: line length (Grbl 80, grblHAL 256 with comments counted, `protocol.h:36`), ASCII only, the grblHAL word set (no G90.1, no G64 unless enabled, M7 only with the mist flag). | validator unit tests |
| G9 | Byte goldens of the current emitter output for each dialect (non-ignored), so a change to the output fails a test. | the goldens |
| G10 | `$$` import: detect grblHAL from the welcome line (`GrblHAL`) or `$I` (`[FIRMWARE:grblHAL]`, `report.c:312,1111`). Parse and store `$30 $31 $32 $340 $341 $342 $394` in the machine profile. Apply `$30/$31` to the post limits. The profile selects the dialect (it is only a project `PostConfig` now). Warn when `$341=4` (M6 ignored) and the post writes M6. | `from_grbl_settings_parses_grblhal_dump` with a synthetic dump now, the real dump when the operator pastes it |
| G11 | Export UI: a grblHAL section in the export dialog: tool change (M6 / pause), the controller waits for the spindle, mist output present, native drill cycles, one file per setup. The values default from the machine profile. MCP `export_gcode` and the CLI take the same options (surface parity). | viz tests; MCP schema test |
| G12 | Docs: FEATURE_CATALOG, the CLI "Supported:" list (`rs_cam_cli/src/job.rs:354`), the post comments, and a short operator note: BitZero per setup, BitSetter at each M6 with `$341=3`, the G59.3 position. | review |

## Open, needs the operator

- ~~The real `$$` and `$I` output of the controller~~ Received 2026-10-08:
  `crates/rs_cam_core/tests/fixtures/grblhal_dump_2026-10-08/` (G10 proof).
  The operator sets `$341=3`, G59.3 on the BitSetter, `$342=40`
  (`OPERATOR_NOTE.md`).
- The rivmap350 project still selects `format = "grbl"` with no saved
  kinematics: the limits set through MCP on 2026-10-01 were never saved.
  After G10, import the dump once into the machine library.
