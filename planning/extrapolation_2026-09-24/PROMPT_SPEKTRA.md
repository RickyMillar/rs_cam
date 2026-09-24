# Prompt for the Spektra sizes session (B5 clean-up, data only)

Paste this as the first message of a new session.

---

You transcribe and verify the rest of the Amana Spektra Spiral Plunge chart
(v24) for the rs_cam extrapolation programme. The orchestrator session is
"rs-cam-2f" (ListAgents); it owns the code and the LUT. A second session
("rs-cam-e2") owns compute/, session/, adaptive3d/ and dressup/. You write
documents, JSON candidate files and python scripts only. No Rust edits, no
cargo, no edits to `crates/rs_cam_core/data/vendor_lut/`: the orchestrator
loads the rows after a ruling, because rows move numbers.

Read first:
- `planning/extrapolation_2026-09-24/B5_PLAN.md` (decision 2: "the other
  Spektra sizes, 1/4 in and up, are a later transcription job") and
  `EXTRAPOLATION_G6.md` (§5, the landing record).
- `planning/extrapolation_2026-09-24/fetch/G6/`: FETCH_NOTES.md,
  sources.json, verified_rows.json (the 8 Ramp Down cells verified at 1/8 in
  and 6 mm), candidate_rows.json, lut_discrepancies.md. The PDF is in
  fetch/G6/pdf/ if it is still on disk; otherwise re-download it from the
  URL in sources.json and compare the sha256.
- `crates/rs_cam_core/data/vendor_lut/observations/amana_flat_end.json`
  (the Spektra side rows now in the LUT: ids like
  `amana-flat-softwood-pocket-6000-2f-spektra`) and the
  `amana_spektra_spiral_plunge_v24` entry of `source_manifest.json`.
- `crates/rs_cam_core/src/feeds/extrapolation/drill.rs` (`DRILL_RULES`,
  `range_mm: (3.175, 6.0)`) and `feeds/ramp.rs`: the G6 drill claim and the
  ramp feed read these rows. A wider range needs the rows first.

The task:
1. List every size, flute count and material column that the chart prints
   (1/4 in / 6.35 mm and up, and any metric size), with the printed Feed
   Rate, Chip Load, RPM and "Ramp Down" values.
2. Compare each cell with the rows already in `amana_flat_end.json`. Say
   which cells are in the LUT, which are missing, and every discrepancy.
3. Write `fetch/G6/candidate_rows_spektra_all.json` in the LUT's
   observation schema (copy the shape of an existing Spektra row exactly:
   ids, tool_subfamily `spektra_spiral_plunge`, filed under (Pocket,
   Roughing), `rpm_nominal` 18000, notes). Mark nothing as derived unless
   you computed it; the Ramp Down figure stays a check, not a row field.
4. Verify independently: a script `scripts/spektra_verify.py` re-reads the
   PDF text (pdftotext) and checks every transcribed number and the
   Ramp Down = Feed Rate / flutes identity on every row. Report the result.
5. Write `fetch/G6/SPEKTRA_SIZES.md`: the table, the verification result,
   the proposed new `range_mm` for `DRILL_RULES`, the flute counts printed,
   and the question for the operator ("load these N rows and widen the G6
   range to X mm?").

Rules:
- Simplified Technical English in prose.
- Commit only your own files, by explicit path; run
  `git diff --cached --name-only` before every commit. Never `git add -A`,
  `git stash` or `git reset`. Do not touch `.mcp.json` or `.pi/`.
- PDFs and raw HTML stay out of git (the folder's .gitignore covers
  fetch/*/pdf, raw, html).
- Nothing is pushed.
- Keep the token use small: one agent, no workflow.
- When done, send a short summary to "rs-cam-2f" (SendMessage) with the
  file list and the operator question.
