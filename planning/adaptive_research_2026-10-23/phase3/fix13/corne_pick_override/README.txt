DO NOT SEND program_REFUSED_ANALYSIS_ONLY.nc TO A MACHINE. EVER.

This folder is an ANALYSIS-ONLY rerun (D23, lead-approved 2026-10-26) of the
Corne spiral_pick run on the D23 build (fixture worktree at 3cc257c1f +
phase3/d23/tree_vs_3cc257c1f.patch). The normal run, fix13/corne_pick/, was
REFUSED by the export gate. This rerun used --accept-exceeded-tool-load, the
known-dangerous override, only so that the program text can be audited.
The plan is the same: summary.json and the toolpath files equal the refused
run apart from the export block and the timing fields.

The refusal, word for word (fix13/corne_pick/summary.json gcode_export.refused):

G-code export refused: tool load exceeded on toolpath(s):
  toolpath 3: deflection=stiffness
Exceeded without refusing the export, because the bound does not gate one:
  toolpath 1: depth of cut, peak 12.0000 mm, limit 9.0000 mm, from the machine rigidity factor 1.50 times the engaged diameter 6.00 mm at that depth, which is 9.00 mm; a rule of thumb with no published source
  toolpath 3: depth of cut, peak 12.0000 mm, limit 4.5000 mm, from the machine rigidity factor 1.50 times the engaged diameter 3.00 mm at that depth, which is 4.50 mm; a rule of thumb with no published source
To override (a known-dangerous override): MCP `accept_exceeded_tool_load: true`, CLI `--accept-exceeded-tool-load`, or the GUI pre-flight checkbox "Accept EXCEEDED criteria".

Code identity (checked 2026-10-26): the lead later committed the D23 code as
6aefca969. `git diff 3cc257c1f 6aefca969` equals the D23 patch this run was
built from (the tracked files and research_outer_band.rs, index lines aside),
so this run is the 6aefca969 code.
