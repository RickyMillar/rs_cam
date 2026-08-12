# A-7 artifacts — Checkpoint K execution, 2026-08-13

| file | what it is |
|---|---|
| `cli_modulation_state.md` | K-(g1)/(g2) rendered evidence — both arms of the CLI modulation-state line, captured verbatim, plus the `summary.json` fields |
| `rendered_findings.md` | K-(a3)/(c2)/(d2)/(a4) rendered evidence — the typed findings and the *clamped vs exceeded* pair as the core diagnostics adapter prints them (the surface CLI + MCP consume), and the re-measured a4 census |
| `suite_pre_a7_summary.txt` | full core suite at parent `e41bdfde` |
| `suite_mid_a7_summary.txt` | full core suite after slices 1–6 (`0bc972c9`) |
| `suite_post_a7_summary.txt` | full core suite after a4 (`b7234d2f`) — carries the one a4 attribution |
| `suite_final_a7_summary.txt` | full core suite after the a4 re-pin (`16786d3b`) |

## About the four suite files

Each was collected as an **unbounded** `cargo test -p rs_cam_core
--no-fail-fast` capture (≈ 279 kB, ≈ 5 300 lines each). What is committed
here is a **reduction**, not the raw file: every `Running <binary>` line,
every `test result:` line, every `failures:` block and every failure name
is kept; the passing tests' names and the panic *bodies* are dropped
(1.1 MB → ~150 kB total).

The one panic body that mattered — the a4 attribution in
`suite_post_a7_summary.txt` — is quoted **in full** in the A-7 wave entry
in `ORCHESTRATION_LOG.md` and in commit `16786d3b`'s message, including
the moved band and both fixtures' old/new feeds. Reducing the artifact
after the fact was a judgement call about repository weight and it is
recorded here rather than left for a reader to discover as an absence.

Reproduce any of them:

```text
cargo test -p rs_cam_core --no-fail-fast
```

Expected red set at `16786d3b`: `arc_raster_full_dressups_fingerprint`,
`face_full_chain_fingerprint`, `three_pass_full_dressups_fingerprint`
(G-XFP, red at master itself — not attributable to TD3) and
`wanaka_suggest_baseline` (environmental). Nothing else.
