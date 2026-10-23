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
- 44af62dc GUI pass; 74af0f8e S11 defaults; 5516aa8b chamfer stop-short;
  4240dfdd open sides; a68cf423 heights D6/D1/Q8; c49414c5 R-A outline.
- Planning: 5ee0fbbe, 49e8ce09, c924192b, 2e33dc7a and the H7/H6/S11
  rulings. Master also carries the adaptive research merges (609ce75b,
  3517d845: instruments + research geodesic; no overlap with this work).
- Peer coordination: the adaptive research session (rs-cam-b2) shares the
  release GUI binary and the cargo lane; tell it before a --release
  rebuild; it owns adaptive/, adaptive3d/, compute/model_gouge.rs.

## Queue, in order

1. DONE 44af62dc: GUI pass, audit rows P4, P3, D7,
   L3, L5 (checkbox), L6, L7.
2. DONE 74af0f8e (+0e6788b2 s6 fixture): underived defaults removed
   (S11); stepover and depth per pass raw values still ship when
   Suggest has no data (ruling text named feed and RPM only).
3. DONE 5516aa8b: Chamfer stop-short at a wall. DONE 4240dfdd: Pocket
   "Cut past open sides" (opt-in); Pocket (on) -> rest Chamfer works on
   stepped_block. Known: a grown higher area can be cut again in air by
   a lower face.
4. DONE a68cf423: D6 Bottom height on the Heights tab, D1 Model bottom,
   Q8 names. Open: the Geometry tab still has a read-only Floor line.
5. DONE c49414c5: R-A part outline item, default Profile Outside to the
   model bottom, no tabs, "frees the part" note. Open: after ModelBottom
   is written a face-ring pick on that Profile does not get Each face.
6. IN PROGRESS, UNCOMMITTED (2026-10-24 morning): R2 + Q7 + Q9 in one
   package. The working tree holds ~251 edited files (core, viz, cli,
   mcp) plus 4 new tests: an_op_with_no_tool_is_a_state_r2,
   a_new_op_takes_its_setup_model_q7, add_a_bore_takes_a_fitting_end_mill_q9,
   viz every_add_door_takes_one_model_and_tool_q7. Design (accepted):
   `ToolpathConfig.tool_id: Option<ToolId>` (no-tool state; generation
   and export refuse "Set a tool on the Geometry tab"; diagnostics
   `precondition.no_tool` "No tool fits: needs <kinds>"); one door
   `ProjectSession::tool_fit`; `draft_toolpath(op, ToolChoice, selected_model,
   setup, name)` picks the setup's selected/serving model
   (`LoadedModel::serves`); picks at add become the selection;
   AddBoreFromDrill takes the first fitting end mill (overturns P6.7b);
   a no-tool draft keeps the default boundary and the first
   SetToolpathTool sizes it. Breaks: file tool_id optional (F053
   changes), MCP add_toolpath tool_index/model_id optional, wire snapshot
   moves. The lead's agent was mid-gates when the session ended. A fresh
   session: run `git diff --stat`, build (`cargo check -p rs_cam_core
   -p rs_cam_viz -p rs_cam_cli`), then the gate list (core --lib, folder
   sentries of session/compute/selection, opgolden, viz --lib +
   properties/app/controller sentries, mcp, cli, fmt, clippy), fix reds,
   review by path, commit by explicit paths. Known master reds, not
   this work: g_adaptpassload, g_adaptrapidlift (adaptive research owns).
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
