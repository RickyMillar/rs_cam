# UX Testing Session — Prompt for next session

Paste the block below into a fresh Claude Code session (after the GUI is
running with `--mcp` and the rs-cam MCP is connected).

---

We're running the UX testing session described in
`planning/UX_TESTING_SESSION_PLAN.md`. Read that plan first — it defines the
12 journeys (J1–J12), the common-negatives watchlist, the deliverable, and
the severity scale. The fixtures are already in place (`test_data/ux_*.toml`
for skeletons; `~/Downloads/wanaka100/...` for the mature project).

Your job: walk through the journeys, exercise the GUI via the rs-cam MCP
tools, **and read the GUI source** when an MCP call doesn't tell you what
the user would actually see (panel layout, button labels, field tooltips,
warning rendering). Use SocratiCode (`codebase_search`, `codebase_symbol`)
to find the right surface before reading. The MCP shows you what the engine
does; the code shows you what the user sees.

For each journey, record findings in `planning/UX_PAIN_POINTS_2026-05-11.md`
as bullets tagged 🔴 / 🟡 / 🟢. Bullet format:

> 🟡 **[J6, sim graph]** Hotspot list shows "1247 issues" with no severity
> breakdown. User can't tell if any are dangerous or all are air-cut noise.
> _Where_: `crates/rs_cam_viz/src/ui/sim_diagnostics.rs:142`. _Fix idea_:
> split count by severity, link the dangerous bucket to its first move.

Each bullet must include: severity emoji, journey tag, surface name,
one-line problem statement, source path:line where you found it, and a
short fix idea (one line — don't design the fix, just point at the shape).

**No timing logs.** Findings only.

**No edits to source code.** This session is observation-only — we land
fixes in subsequent PRs based on what you find. The only writes are to
the deliverable file.

### Order

1. Read `planning/UX_TESTING_SESSION_PLAN.md` end-to-end.
2. Skim `crates/rs_cam_viz/src/ui/` directory listing so you know which
   files map to which surfaces (params panel, sim diagnostics, viewport
   overlays, optimizer panel, tool-load report, stock panel, tool panel).
3. For each journey J1–J12, in order:
   - Identify the fixture(s) the journey needs.
   - Drive the MCP to set up the scenario.
   - Read the GUI source for the surface(s) the user sees.
   - Take a screenshot when the visual layout matters (`screenshot_simulation`,
     `screenshot_toolpath`).
   - Record findings as bullets in the deliverable.
4. After all 12 journeys, write the summary table at the bottom of the
   deliverable (rows = surface, columns = 🔴/🟡/🟢 counts) and pick the
   single surface that should be the next investment target.

### Watchlist (the patterns that matter most)

The plan lists 10 — keep all in mind, but the three the user cares most
about right now are:

1. **Silent state mutation** — value flips with no UI feedback. The
   spindle_rpm coercion fix (F2) was triggered by exactly this pattern.
2. **Lists where graphs would be honest** — the F6 reframe targeted this.
   J6/J7/J9 should specifically test whether the reframe landed cleanly
   or whether more lists still need to become graphs+bands.
3. **Verdict without next step** — every red badge needs an answer to
   "what do I do about it?" Even if the answer is "ignore for this op
   type", the UI must say so.

### Stop conditions

- If a journey can't be run because the engine errors or the GUI crashes:
  log it as a 🔴 with as much detail as the panic/error message gives,
  then continue to the next journey.
- If the MCP disconnects mid-session: stop and tell the user.
- If you finish all 12 in one pass: do a second pass on the most-painful
  surface from the summary table to look for issues you missed the first
  time.

### What success looks like

A deliverable file with at least 30 bullets covering all 12 journeys,
honest severity tagging (mostly 🟡 and 🟢 — 🔴 should be rare and earned),
each bullet pointing at a real source location, and a summary table that
makes it obvious where the next UX investment should go.

---

End of pasted block.
