# Next session: geometry selection, after 2026-10-23

You are the lead of the geometry selection programme. Read `PLAN.md` from
"Operator rulings 2026-10-23 morning" to the end first. Then read
`INDUSTRY_DEFAULTS.md` and `REFERENCE_AUDIT.md` (section 3).

## Standing rules

- Settle an open question by the industry default (Kiri:Moto and FreeCAD
  source, Fusion manuals). Build the obvious without asking. Ask the
  operator only on a real conundrum (the references disagree, no source,
  or a safety rule conflicts).
- A default with no derivation is removed: the field is empty and the op
  refuses until it is set.
- Ask before any push and before any test run over 3 minutes. P8 stays
  blocked until the operator has reviewed everything else on screen.
- Agents implement and never commit. The lead reviews by path and
  commits by explicit paths. One cargo job at a time through
  `timeout <s> scripts/cargo_lane.sh`. No large test gates. No worktrees.
  Never commit `.mcp.json`, `planning/rivmap_block_job/testpiece.toml`
  or `.pi/`.
- zsh: `cp` is aliased to `cp -i` and hangs an agent; use `\cp -f`.
- Check that the GUI binary is built from HEAD before any GUI look.

## Done (local on master, not pushed; origin is 2880e104)

- b5b8dd65 F26: every MCP param type refuses an unknown field.
- 7922e689 F31: an auto stock fits the union of every model.
- 88d18baf R-B, R-E, F19: a STEP Drill drills each rim to its floor;
  one hole is one row; the Heights panel owns the Drill top.
- Planning: 5ee0fbbe, 49e8ce09, c924192b, 2e33dc7a and the H7/H6/S11
  rulings.

## Queue, in order

1. GUI pass (in progress at the time of writing): audit rows P4, P3, D7,
   L3, L5, L6, L7. Review, gate, commit.
2. Underived defaults: the inventory is
   `UNDERIVED_DEFAULTS.md` (if it is absent, run it again). Remove the UNDERIVED ones; first the Pocket depth at add.
3. Chamfer stop-short: an edge that ends at a higher wall is trimmed by
   a clearance (Fusion "Chamfer Clearance"). The buried check and the
   gouge guard read the trimmed run. Live case: stepped_block, Pocket ->
   Waterline -> Chamfer "Rest machining: from previous ops" refuses
   today ("10.0 mm below the material that stands over it") because the
   edge band reaches the step wall.
4. D6 + D1: the floor is edited as "Bottom height" on the Heights tab
   under "Top height"; add the floor reference "Model bottom".
5. R-A: the outline item and the default Profile (design note
   `RA_OUTLINE_DESIGN.md`; decisions taken:
   exact outline from the mesh triangles, outline only, Outside, to the
   model bottom, no tabs, a "this cut frees the part" note; only Profile
   Outside takes it by default; area ops refuse it).
6. R2: an add with no fitting tool makes the op with the state "no tool
   fits: needs <kinds>" (Fusion). Medium: an op may exist with no tool.
7. T3 (delete the last tab, note that the part is freed) and L8 (run the
   held Profile near/far hole test, then pick "Island" or "Inner wall").
8. Q5 (error state on the op row), Q6 (refuse a move with a typed
   absolute height), Q7 (a new op takes the setup's models and the
   current selection), Q9 (the Bore takes a flat end mill that fits).
9. Live GUI look on a HEAD build of every item above; then the morning
   report for the operator. Then P7 (Q10: holes grouped by diameter).

## Flow facts seen live on 2026-10-23

- Pocket -> Waterline clears the wall round stepped_block (the sim shows
  the stepped part standing clean). The Waterline slots the 5 mm padding
  in 48 levels: heavy for a finish op.
- `add_toolpath_via_gui` binds the first project model (Q7 fixes it).
