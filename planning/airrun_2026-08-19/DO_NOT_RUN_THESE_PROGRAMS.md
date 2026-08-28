# ⚠ The two shipped `.nc` programs here are UNSAFE to run

`wanaka200_1_Setup_1.nc` and `wanaka200_2_Setup_2___front.nc` (exported
2026-08-22) carry the rapid-descent defect measured by Phase S
(`planning/rapid_safety_2026-08-28/S1_RESULTS.md`): **982 link descents
enter standing material at G0 rate, up to 1.3 mm deep** — ops 7 (3D
finish, 601/601 links) and 8 (pencil). Setup 1's program is clean, but do
not cherry-pick: re-export both.

**Before any real cut, re-export from a build at or after `79361f31`**
(S3: the air-cut filter no longer converts plunges through crest material
into rapids; verified 2026-08-28 — pipeline 202 → 0 collisions with the
fixed detector watching, independent replay clean).

The files stay in the tree deliberately: they are the measured evidence
for the S1 instrument (`tests/rapid_replay_shipped_gcode_s1.rs`), which
replays them byte-for-byte. Do not regenerate over them.
