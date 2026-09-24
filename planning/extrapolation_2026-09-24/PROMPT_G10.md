# Prompt for the G10 session (entry parameters, Phases 0-2)

Paste this as the first message of a new session.

---

You run gap group **G10 (entry parameters)** of the rs_cam extrapolation
programme, Phases 0-2 only: inventory, fetch, trend. Another session
("rs-cam-e", the extrapolation orchestrator) owns the code; a third
("rs-cam-e2", the feeds/dial and adaptive3d session) owns compute/,
session/, adaptive3d/ and dressup/entry_descent.rs. You write documents
and python scripts only. No Rust edits, no cargo, no LUT or manifest
edits: loading rows moves numbers and waits for the operator's ruling.

Read first:
- `planning/extrapolation_2026-09-24/PLAN.md` (§3 the investigation shape,
  §4 the claim design) and `RULINGS.md` (the rulings blocks, "Work item
  G10", the ramp-feed proposal and its approval).
- One landed group as the model of a finished investigation:
  `EXTRAPOLATION_G6.md` (the drill) and `B5_PLAN.md`.
- `INVENTORY.md` (how Phase 0 was done) and `lut_axes.txt`.
- The code that holds today's entry defaults (read, do not edit): rg for
  `ramp_angle_deg`, `helix_radius_factor`, `helix_pitch`, `plunge_rate`,
  `ramp_feed_mm_min`, `plunge_rate_base`, `EntryStyle` in
  crates/rs_cam_core/src (compute/operation_configs.rs, feeds/mod.rs step 8,
  material/mod.rs, dressup/entry_descent.rs, feeds/suggest/adaptive_entry.rs).
- The FM1 matrix `planning/feeds_matrix_2026-09-23/matrix_2026-09-23.csv`
  (5 tool kinds x 2 sizes x 24 operations x 4 woods).

The question (operator, 2026-09-25): the entry parameters are probably not
generic across tools. For EVERY tool type and material in the matrix,
which cells have a printed or vendor source, which fall back, and what the
card says, for: ramp angle, helix pitch, helix radius, plunge rate, and
the new `ramp_feed_rate` (approved design: ramp feed = min(cutting feed,
axial_chip x rpm x Z / tan(ramp angle)), axial chip from the G6 printed
plunge chip where one exists, else the plunge rate).
Operator rules to respect: never helix through air; the entry style
(helix / plunge / ramp) is the operator's setting, not Suggest's; the
helix start clearance above material becomes a setting (default 0.5 mm).
Context: on the 6 mm flat end mill in hardwood a full-depth helix at the
plunge rate made roughs 2-2.6x slower.

Deliver, in `planning/extrapolation_2026-09-24/`:
1. `INVENTORY_G10.md`: every entry parameter, where the engine sets it
   today (file:line), its default, whether any source backs it, and the
   matrix cells it reaches (per tool kind, size, operation family).
2. `fetch/G10/`: vendor and literature guidance, stored with a text copy
   and a sha256 per document (PDFs and raw HTML stay out of git; the
   folder's .gitignore covers fetch/*/pdf, raw, html). Targets: vendor
   ramp-angle and helix guidance per tool family (Onsrud, Amana, Whiteside,
   Harvey, Helical, Garr, Kennametal, PreciseBits, Carbide 3D), helix
   diameter as a fraction of D, maximum ramp angle vs flute count and
   centre-cutting, plunge feed vs side feed, and any wood-specific
   statement. Transcribe only what a document prints; mark every computed
   value as derived. "Not published anywhere I could reach" is a valid
   result: say it plainly, with the searches you ran.
3. Verify each source independently (re-download, compare the hash and the
   transcribed numbers), as the Phase 1 workflow did.
4. `EXTRAPOLATION_G10.md` with §0 the gap (cells, current rules), §1 the
   trend (tables per tool family and parameter, the spread), §2 sources
   (id, what it prints, grade, verified), §3 for the rulings (the forms
   each parameter could take, their range, a second witness where one
   could exist, and a recommendation per sub-class: generic / per family /
   keep the repo default as a named rule on the card / refuse).

Rules:
- Simplified Technical English in prose.
- No cargo, no Rust, no LUT edits. Python scripts go in
  `planning/extrapolation_2026-09-24/scripts/` (name them g10_*.py).
- Commit only your own files, by explicit path; run
  `git diff --cached --name-only` before every commit. Another two
  sessions commit in the same checkout. Never `git add -A`, never
  `git stash`, never `git reset`.
- Do not touch `.mcp.json` or `.pi/`.
- Nothing is pushed.
- If you want a workflow with many agents, keep it small (one research
  agent per tool family plus verifiers); the token budget is tight.
- When done, send a short summary to the "rs-cam-e" session
  (SendMessage; ListAgents shows it) with the file list and the questions
  the operator must rule.
