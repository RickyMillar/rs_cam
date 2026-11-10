# Feeds and speeds acceptance tables

This folder holds independent acceptance tables. A table states what each
shipped feeds-and-speeds number SHOULD be, from printed sources in the repo and
physics. The app is graded against the table.

| File | Content |
|---|---|
| `wanaka_21.md` | The wanaka 21 table: rules, common inputs, operator questions, consistency checks, one row per toolpath per quantity |
| `wanaka_21.toml` | The same rows, machine-readable |
| `wanaka_21_header.md` | The fixed prose part of `wanaka_21.md` |
| `derive_wanaka_21.py` | Computes every expected value and grade; writes the two outputs |

## Rule: the table changes only with an operator ruling

- Do not change an expected value, a candidate or a status to match the app.
- A change to the table needs one of these:
  - an operator ruling on a P or U question (record the ruling and its date in `wanaka_21_header.md`);
  - a new printed source stored in the repo (quote the file and line).
- When the operator rules a POLICY question, delete the losing candidate from
  the script and change the status of those rows to DERIVED.
- Never type a number from memory. Each constant in the script quotes a file and line in `SRC`.

## How to re-grade a new build

1. Check the build. Call `gui_status`. Record `binary.mtime_unix` and
   `newer_than_process`. Run `git log -1 --format='%h %ct'` and
   `git status --short crates`. `newer_than_process` must be false, and one of
   these must hold:
   - the binary mtime is later than the HEAD commit time, and `crates/` has no
     uncommitted edit;
   - the binary is earlier than the commit of its own source (the build ran on
     the working tree, then the commit followed). The lead must then state that
     no product source changed between the build and the commit. Record that
     statement and both times in the header.
   If neither holds, the GUI is stale; do not grade.
2. Check that the wanaka 21 project is loaded. Call `list_toolpaths`. Expect 18
   toolpaths, index 0-17, with the names in the table.
3. For each index 0-17, call `get_toolpath_params` and read:
   - `operation.params.feed_rate`, `spindle_rpm`, `plunge_rate`, `ramp_feed_rate` (the shipped values);
   - `operation.params.stepover`, `depth_per_pass`, `trochoid_cap_mult`, `entry_style` or `dressups` (the inputs; if an input changed, the expected value can change: re-derive, do not copy).
4. For each index 0-17, call `get_suggest_rationale` and read:
   - `basis.shipped_power.feed_mm_min`, `.rpm`, `.required_kw`, `.available_kw`, `.case_ae_mm`, `.ap_mm` (Suggest feed, rpm and power);
   - `basis.plunge.value_mm_min` (Suggest plunge);
   - `basis.ramp.value_mm_min` (Suggest ramp or helix entry feed; null when the entry is a straight plunge);
   - `basis.vendor_band.score_winner_chip_mm` (the target chip per tooth).
   When `shipped_power` is `{"unmodeled": ...}` (idx 5 and 9), read `basis.rpm.feed_mm_min`,
   `basis.rpm.value`, `basis.rpm.plunge_mm_min` and `basis.rpm.ramp_feed_mm_min` (every op has
   them from P23b). The power row then has no number. The script has no fallback to a stored
   param: a missing Suggest feed or rpm stops the script.
5. Put the values in the `APP` dict of `derive_wanaka_21.py` (`sug` = step 4, `shp` = step 3).
6. Run `python3 -I derive_wanaka_21.py` in this folder. It rewrites
   `wanaka_21.md` and `wanaka_21.toml` and prints the status and grade counts
   and each FAIL row. The script reads no engine code and runs no cargo.
7. Compare the FAIL list with the previous grade. A FAIL that turns to PASS
   shows a fix. A new FAIL shows a regression.

These calls are read-only. Do not call `set_*`, `apply_feeds`, `generate_*`,
`save_project`, `load_project` or `gui_restart` while grading.

## Notes

- The tolerance is 1 % relative. The app rounds feeds to whole mm/min.
- The power ratio does not depend on RPM on this machine (constant torque
  below 24000 rpm). The power rows grade the model arithmetic at the app's own
  operating point.
- Not a feeds row: the project states `[job.machine.controller] rpm_max = 24000.0`
  (corrected 2026-11-09; the operator read `$30` = 24000 on the machine).
