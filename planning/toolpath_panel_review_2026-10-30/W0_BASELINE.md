# W0 baseline: the toolpath inspector in bad states

This file records the counts before the tidy programme (W1-W6). The test
`crates/rs_cam_viz/tests/the_inspector_bad_states_w0.rs` writes it. Do not
edit it by hand. To write it again, run this command:

```
W0_WRITE_BASELINE=1 scripts/cargo_lane.sh test -p rs_cam_viz -q --test the_inspector_bad_states_w0 -- --nocapture
```

## The tree that the test measured

The counts are true for this tree only. A file that differs from HEAD
changes the counts.

- HEAD: `e9e75762c planning: toolpath inspector UI/UX review and execution plan`
- Source files that differ from HEAD (viz `src/` and core `src/`):

```
 M ../rs_cam_core/src/maps/CLAUDE.md
 M ../rs_cam_core/src/maps/reach_map.rs
 M src/app/mcp/view.rs
 M src/mcp_server.rs
 M src/state/runtime.rs
 M src/ui/components/CLAUDE.md
 M src/ui/components/kv_row.rs
 M src/ui/components/mod.rs
 M src/ui/components/notice.rs
 M src/ui/components/tab_strip.rs
 M src/ui/components/value_row.rs
?? src/ui/components/notes_counter.rs
?? src/ui/components/prop_row.rs
```

## The method

The test renders the real inspector (`properties::draw`) at the 280-point
panel width of IC1. It renders three op families:

- Pocket (2D), on a 20 mm square outline, with a 6 mm flat end mill.
- 3D Rough (Adaptive3d), on a 10 mm hemisphere, with the flat end mill.
- 3D Finish (Scallop), on the hemisphere, with a 6 mm ball nose.

Each state has its own fresh fixture. The state applies to one op only.
The test renders each tab of `ToolpathTab::ALL` in each state. It renders
each tab two times on a fresh egui context and reads the second frame.

The states are these:

1. clean: the reference. The tool fits, the feeds and the floor are set,
and nothing refused.
2. no tool: the op has no tool.
3. feeds unset: the feed, the plunge and the spindle speed are empty. The
floor stays set.
4. wrong tool kind: the flat end mill on the Scallop.
5. last run refused: the run state holds the D53 refusal text of IC1.
6. reach map on: `show_reach_map` is on and the overlay holds a ready map
of the hemisphere (Ø6 ball, 0.5 mm cells, tolerance 0.0001 mm). The
overlay also holds the refined surface, so the scale row paints.
7. boundary on (Selection): the machining boundary is on, and its source
is the closed shapes of the 2D outline.
8. stale after edit: the op has a result, then the feed changes to
900 mm/min. The freshness is `EditedSince`.
9. feeds unset + refused (extra): state 3 plus a refusal in the shape of
the core `NotSet` text. This state is not in the W0 list.

These states do not apply, and the test skips them:

- Pocket / wrong tool kind: the registry row is ANY_TOOL: every tool kind fits.
- 3D Rough / wrong tool kind: the registry row is ANY_TOOL: every tool kind fits.
- Pocket / reach map on: `supports_reach_map` is false: no reach rows exist for the op.
- 3D Rough / reach map on: `supports_reach_map` is false: no reach rows exist for the op.

The test asserts only one rule: each bad state paints its marker text
more times than the clean state does. It does not assert the W6 rules yet.

## The measures

- Visible rows: the row bands of all galleys, merged where they overlap
(the IC1 measure). A row of label, value and unit is one row.
- Galleys: the painted text galleys on the screen.
- Truncated: a galley that elided its tail, or that the clip rectangle
cut.
- Repeat: the number of galleys on one screen with the same message key.
The key is the full galley text, with the whitespace normalised and a
leading glyph (for example `✕` or `⚠`) removed. The test counts only a
text of three words or more as a message. A shorter text is a label, a
unit or a value.
- Longest face: the most characters that one galley paints. The face is
shorter than the full text when the galley elided the tail.
- Longest text: the most characters in one full galley text.

The test prints the full galley inventory of each screen with
`--nocapture`. This file holds the tables only.

## The limits of the measures

- The repeat key is the text. It does not see one fact in two words. For
example, the wrong tool state paints the tool refusal as three different
texts on one screen, and the repeat count is 1. W6 must count a
diagnostic id (the W1 `anchor` and `face`), not a text.
- The three empty feed fields each paint the hint "set a value". The
repeat count is 3, but each hint belongs to its own field. W6 must not
count a field hint as a repeated message.
- `OneLine` elides each long text, so no face is long. The full text
behind the face can be long. The tables give both lengths.

## Worst values per state

| State | Screens | Worst repeat | Message of the worst repeat | Longest face | Longest text | Texts over 60 chars |
|---|---:|---:|---|---:|---:|---:|
| clean | 18 | 1 | Show reach map | 48 | 76 | 1 |
| no tool | 18 | 2 | No tool set. Set a tool on the Cut tab | 48 | 76 | 1 |
| feeds unset | 18 | 3 | set a value | 51 | 148 | 4 |
| wrong tool kind | 6 | 1 | Tool load: Scallop Finish requires a Ball Nose or Tapered Ball Nose tool | 51 | 108 | 4 |
| last run refused | 18 | 1 | Show reach map | 50 | 406 | 2 |
| reach map on | 6 | 1 | the grid over-states gaps by up to its floor (0.019 mm), so the true unreachable share is AT OR BELOW 0.0 %, and the 32.1 % unresolved is the band the grid cannot classify either way | 53 | 184 | 3 |
| boundary on (Selection) | 18 | 2 | Default: every closed loop (1) | 48 | 76 | 1 |
| stale after edit | 18 | 1 | old: 0 moves | 48 | 76 | 1 |
| feeds unset + refused (extra) | 18 | 3 | set a value | 52 | 148 | 7 |

## Repeated messages

Each row is a message that paints two or more times on one screen.

| Family | State | Max per screen | Tabs | Message |
|---|---|---:|---|---|
| Pocket | no tool | 2 | Cut | No tool set. Set a tool on the Cut tab |
| Pocket | feeds unset | 3 | FeedsSpeeds | set a value |
| Pocket | boundary on (Selection) | 2 | Geometry | 1 closed loop |
| Pocket | boundary on (Selection) | 2 | Geometry | Default: every closed loop (1) |
| Pocket | feeds unset + refused (extra) | 3 | FeedsSpeeds | set a value |
| 3D Rough | no tool | 2 | Cut | No tool set. Set a tool on the Cut tab |
| 3D Rough | feeds unset | 3 | FeedsSpeeds | set a value |
| 3D Rough | feeds unset + refused (extra) | 3 | FeedsSpeeds | set a value |
| 3D Finish | no tool | 2 | Cut | No tool set. Set a tool on the Cut tab |
| 3D Finish | feeds unset | 3 | FeedsSpeeds | set a value |
| 3D Finish | feeds unset + refused (extra) | 3 | FeedsSpeeds | set a value |

## Texts over 60 characters

Each row is a full galley text over 60 characters. W6 sets 60
characters as the face limit.

| Family | State | Chars | Face chars | Truncated | Tabs | Full text |
|---|---|---:|---:|---|---|---|
| Pocket | feeds unset | 140 | 51 | yes | Geometry, Cut, Heights, FeedsSpeeds, Linking, Dressup | Geometry: Pocket: no feeds data for this tool: set feed and RPM (not set: `feed_rate`, `plunge_rate`, `spindle_rpm`) — generation will fail. |
| Pocket | last run refused | 406 | 50 | yes | Geometry, Cut, Heights, FeedsSpeeds, Linking, Dressup | 'Edges' takes its boundary from the rest regions of 'Body rough' with Center containment, but its tool radius 5.000 mm is larger than the tool radius of 'Body rough', 3.000 mm. The regions are tool-centre positions of that tool, so this tool could cut up to 2.000 mm past the machining boundary of 'Body rough'. Set the boundary containment to Inside, or use a tool no larger than the tool of 'Body rough'. |
| Pocket | feeds unset + refused (extra) | 140 | 51 | yes | Geometry, Cut, Heights, FeedsSpeeds, Linking, Dressup | Geometry: Pocket: no feeds data for this tool: set feed and RPM (not set: `feed_rate`, `plunge_rate`, `spindle_rpm`) — generation will fail. |
| Pocket | feeds unset + refused (extra) | 102 | 51 | yes | Geometry, Cut, Heights, FeedsSpeeds, Linking, Dressup | Pocket: no feeds data for this tool: set feed and RPM (not set: feed rate, plunge rate, spindle speed) |
| 3D Rough | feeds unset | 145 | 49 | yes | Geometry, Cut, Heights, FeedsSpeeds, Linking, Dressup | Geometry: 3D Adaptive: no feeds data for this tool: set feed and RPM (not set: `feed_rate`, `plunge_rate`, `spindle_rpm`) — generation will fail. |
| 3D Rough | last run refused | 406 | 50 | yes | Geometry, Cut, Heights, FeedsSpeeds, Linking, Dressup | 'Edges' takes its boundary from the rest regions of 'Body rough' with Center containment, but its tool radius 5.000 mm is larger than the tool radius of 'Body rough', 3.000 mm. The regions are tool-centre positions of that tool, so this tool could cut up to 2.000 mm past the machining boundary of 'Body rough'. Set the boundary containment to Inside, or use a tool no larger than the tool of 'Body rough'. |
| 3D Rough | feeds unset + refused (extra) | 104 | 50 | yes | Geometry, Cut, Heights, FeedsSpeeds, Linking, Dressup | 3D Rough: no feeds data for this tool: set feed and RPM (not set: feed rate, plunge rate, spindle speed) |
| 3D Rough | feeds unset + refused (extra) | 145 | 49 | yes | Geometry, Cut, Heights, FeedsSpeeds, Linking, Dressup | Geometry: 3D Adaptive: no feeds data for this tool: set feed and RPM (not set: `feed_rate`, `plunge_rate`, `spindle_rpm`) — generation will fail. |
| 3D Finish | clean | 76 | 48 | yes | Heights | Bottom height: Not used by this operation. The model surface sets the floor. |
| 3D Finish | no tool | 76 | 48 | yes | Heights | Bottom height: Not used by this operation. The model surface sets the floor. |
| 3D Finish | feeds unset | 76 | 48 | yes | Heights | Bottom height: Not used by this operation. The model surface sets the floor. |
| 3D Finish | feeds unset | 148 | 51 | yes | Geometry, Cut, Heights, FeedsSpeeds, Linking, Dressup | Geometry: Scallop Finish: no feeds data for this tool: set feed and RPM (not set: `feed_rate`, `plunge_rate`, `spindle_rpm`) — generation will fail. |
| 3D Finish | wrong tool kind | 76 | 48 | yes | Heights | Bottom height: Not used by this operation. The model surface sets the floor. |
| 3D Finish | wrong tool kind | 108 | 50 | yes | FeedsSpeeds | Feeds unavailable: Scallop Finish needs a ball nose or tapered ball nose tool; this tool is a flat end mill. |
| 3D Finish | wrong tool kind | 61 | 49 | yes | Geometry, Cut, Heights, FeedsSpeeds, Linking, Dressup | Scallop Finish requires a Ball Nose or Tapered Ball Nose tool |
| 3D Finish | wrong tool kind | 72 | 51 | yes | Geometry, Cut, Heights, FeedsSpeeds, Linking, Dressup | Tool load: Scallop Finish requires a Ball Nose or Tapered Ball Nose tool |
| 3D Finish | last run refused | 406 | 50 | yes | Geometry, Cut, Heights, FeedsSpeeds, Linking, Dressup | 'Edges' takes its boundary from the rest regions of 'Body rough' with Center containment, but its tool radius 5.000 mm is larger than the tool radius of 'Body rough', 3.000 mm. The regions are tool-centre positions of that tool, so this tool could cut up to 2.000 mm past the machining boundary of 'Body rough'. Set the boundary containment to Inside, or use a tool no larger than the tool of 'Body rough'. |
| 3D Finish | last run refused | 76 | 48 | yes | Heights | Bottom height: Not used by this operation. The model surface sets the floor. |
| 3D Finish | reach map on | 76 | 48 | yes | Heights | Bottom height: Not used by this operation. The model surface sets the floor. |
| 3D Finish | reach map on | 177 | 53 | yes | Geometry, Cut, Heights, FeedsSpeeds, Linking, Dressup | full colour at one tip radius R = 3.00 mm · grid floor 0.019 mm = 0.01 R: a reported gap under the floor is discretisation, not geometry · grey: not measured (walls, undersides) |
| 3D Finish | reach map on | 184 | 51 | yes | Geometry, Cut, Heights, FeedsSpeeds, Linking, Dressup | ⚠ the grid over-states gaps by up to its floor (0.019 mm), so the true unreachable share is AT OR BELOW 0.0 %, and the 32.1 % unresolved is the band the grid cannot classify either way |
| 3D Finish | boundary on (Selection) | 76 | 48 | yes | Heights | Bottom height: Not used by this operation. The model surface sets the floor. |
| 3D Finish | stale after edit | 76 | 48 | yes | Heights | Bottom height: Not used by this operation. The model surface sets the floor. |
| 3D Finish | feeds unset + refused (extra) | 105 | 52 | yes | Geometry, Cut, Heights, FeedsSpeeds, Linking, Dressup | 3D Finish: no feeds data for this tool: set feed and RPM (not set: feed rate, plunge rate, spindle speed) |
| 3D Finish | feeds unset + refused (extra) | 76 | 48 | yes | Heights | Bottom height: Not used by this operation. The model surface sets the floor. |
| 3D Finish | feeds unset + refused (extra) | 148 | 51 | yes | Geometry, Cut, Heights, FeedsSpeeds, Linking, Dressup | Geometry: Scallop Finish: no feeds data for this tool: set feed and RPM (not set: `feed_rate`, `plunge_rate`, `spindle_rpm`) — generation will fail. |

## Raw rows in the source

The count of `ui.label(` and `ui.horizontal(` sites per file under
`crates/rs_cam_viz/src/ui/properties/`. The scan ignores a line comment.

| File | `ui.label(` | `ui.horizontal(` | Total |
|---|---:|---:|---:|
| `machine_panel.rs` | 27 | 6 | 33 |
| `toolpath_panel.rs` | 14 | 14 | 28 |
| `operations/boundary_2d.rs` | 17 | 3 | 20 |
| `tool.rs` | 16 | 4 | 20 |
| `operations/surface_3d.rs` | 18 | 1 | 19 |
| `stock_changes.rs` | 10 | 6 | 16 |
| `setup.rs` | 9 | 6 | 15 |
| `stock.rs` | 11 | 3 | 14 |
| `feeds_speeds.rs` | 11 | 0 | 11 |
| `model_sim_panels.rs` | 5 | 6 | 11 |
| `head.rs` | 5 | 4 | 9 |
| `mod.rs` | 9 | 0 | 9 |
| `tab_badges.rs` | 4 | 4 | 8 |
| `obstacles.rs` | 3 | 4 | 7 |
| `linking_dressup.rs` | 5 | 1 | 6 |
| `operations/project.rs` | 4 | 0 | 4 |
| `operations/finishing.rs` | 3 | 0 | 3 |
| `post.rs` | 3 | 0 | 3 |
| `operations/drill.rs` | 2 | 0 | 2 |
| `operations/engrave.rs` | 2 | 0 | 2 |
| `operations/mod.rs` | 2 | 0 | 2 |
| `operations/registry.rs` | 2 | 0 | 2 |
| `selection_section.rs` | 0 | 2 | 2 |
| `operations/bore.rs` | 1 | 0 | 1 |
| `operations/height_diagram.rs` | 0 | 1 | 1 |
| `pins/mod.rs` | 1 | 0 | 1 |
| `geometry_tab.rs` | 0 | 0 | 0 |
| `operations/shape_diagrams.rs` | 0 | 0 | 0 |
| `operations/validate.rs` | 0 | 0 | 0 |
| `panel_apply.rs` | 0 | 0 | 0 |
| `pills.rs` | 0 | 0 | 0 |
| `pins/drills.rs` | 0 | 0 | 0 |
| `pins/edit.rs` | 0 | 0 | 0 |
| `pins/flip_view.rs` | 0 | 0 | 0 |
| `pins/plan_view.rs` | 0 | 0 | 0 |
| `pins/status.rs` | 0 | 0 | 0 |
| `shoe_row.rs` | 0 | 0 | 0 |
| `tests.rs` | 0 | 0 | 0 |
| **all 38 files** | 184 | 65 | 249 |

## Each screen

| Family | State | Tab | Visible rows | Galleys | Truncated | Max repeat | Longest face | Longest text |
|---|---|---|---:|---:|---:|---:|---:|---:|
| Pocket | clean | Geometry | 8 | 22 | 0 | 1 | 33 | 33 |
| Pocket | clean | Cut | 19 | 37 | 0 | 1 | 33 | 33 |
| Pocket | clean | Heights | 19 | 43 | 5 | 1 | 33 | 33 |
| Pocket | clean | FeedsSpeeds | 21 | 58 | 10 | 1 | 33 | 33 |
| Pocket | clean | Linking | 18 | 29 | 0 | 1 | 33 | 33 |
| Pocket | clean | Dressup | 9 | 17 | 0 | 1 | 33 | 33 |
| 3D Rough | clean | Geometry | 5 | 13 | 0 | 1 | 33 | 33 |
| 3D Rough | clean | Cut | 18 | 37 | 0 | 1 | 33 | 33 |
| 3D Rough | clean | Heights | 17 | 40 | 5 | 1 | 33 | 33 |
| 3D Rough | clean | FeedsSpeeds | 21 | 58 | 11 | 1 | 33 | 33 |
| 3D Rough | clean | Linking | 18 | 29 | 0 | 1 | 33 | 33 |
| 3D Rough | clean | Dressup | 9 | 17 | 0 | 1 | 33 | 33 |
| 3D Finish | clean | Geometry | 6 | 14 | 0 | 1 | 33 | 33 |
| 3D Finish | clean | Cut | 17 | 31 | 0 | 1 | 33 | 33 |
| 3D Finish | clean | Heights | 19 | 42 | 6 | 1 | 48 | 76 |
| 3D Finish | clean | FeedsSpeeds | 21 | 50 | 5 | 1 | 46 | 50 |
| 3D Finish | clean | Linking | 19 | 30 | 0 | 1 | 33 | 33 |
| 3D Finish | clean | Dressup | 10 | 18 | 0 | 1 | 33 | 33 |
| Pocket | no tool | Geometry | 8 | 22 | 0 | 1 | 38 | 38 |
| Pocket | no tool | Cut | 19 | 35 | 0 | 2 | 38 | 38 |
| Pocket | no tool | Heights | 18 | 42 | 5 | 1 | 38 | 38 |
| Pocket | no tool | FeedsSpeeds | 10 | 23 | 0 | 1 | 38 | 38 |
| Pocket | no tool | Linking | 17 | 28 | 0 | 1 | 38 | 38 |
| Pocket | no tool | Dressup | 8 | 16 | 0 | 1 | 38 | 38 |
| 3D Rough | no tool | Geometry | 5 | 13 | 0 | 1 | 38 | 38 |
| 3D Rough | no tool | Cut | 18 | 35 | 0 | 2 | 38 | 38 |
| 3D Rough | no tool | Heights | 16 | 39 | 5 | 1 | 38 | 38 |
| 3D Rough | no tool | FeedsSpeeds | 10 | 23 | 0 | 1 | 38 | 38 |
| 3D Rough | no tool | Linking | 17 | 28 | 0 | 1 | 38 | 38 |
| 3D Rough | no tool | Dressup | 8 | 16 | 0 | 1 | 38 | 38 |
| 3D Finish | no tool | Geometry | 6 | 14 | 0 | 1 | 38 | 38 |
| 3D Finish | no tool | Cut | 17 | 31 | 0 | 2 | 38 | 38 |
| 3D Finish | no tool | Heights | 18 | 41 | 6 | 1 | 48 | 76 |
| 3D Finish | no tool | FeedsSpeeds | 11 | 24 | 0 | 1 | 38 | 38 |
| 3D Finish | no tool | Linking | 18 | 29 | 0 | 1 | 38 | 38 |
| 3D Finish | no tool | Dressup | 9 | 17 | 0 | 1 | 38 | 38 |
| Pocket | feeds unset | Geometry | 8 | 22 | 1 | 1 | 51 | 140 |
| Pocket | feeds unset | Cut | 19 | 37 | 1 | 1 | 51 | 140 |
| Pocket | feeds unset | Heights | 19 | 43 | 6 | 1 | 51 | 140 |
| Pocket | feeds unset | FeedsSpeeds | 19 | 45 | 6 | 3 | 51 | 140 |
| Pocket | feeds unset | Linking | 18 | 29 | 1 | 1 | 51 | 140 |
| Pocket | feeds unset | Dressup | 9 | 17 | 1 | 1 | 51 | 140 |
| 3D Rough | feeds unset | Geometry | 5 | 13 | 1 | 1 | 49 | 145 |
| 3D Rough | feeds unset | Cut | 18 | 37 | 1 | 1 | 49 | 145 |
| 3D Rough | feeds unset | Heights | 17 | 40 | 6 | 1 | 49 | 145 |
| 3D Rough | feeds unset | FeedsSpeeds | 19 | 45 | 6 | 3 | 49 | 145 |
| 3D Rough | feeds unset | Linking | 18 | 29 | 1 | 1 | 49 | 145 |
| 3D Rough | feeds unset | Dressup | 9 | 17 | 1 | 1 | 49 | 145 |
| 3D Finish | feeds unset | Geometry | 6 | 14 | 1 | 1 | 51 | 148 |
| 3D Finish | feeds unset | Cut | 17 | 31 | 1 | 1 | 51 | 148 |
| 3D Finish | feeds unset | Heights | 19 | 42 | 7 | 1 | 51 | 148 |
| 3D Finish | feeds unset | FeedsSpeeds | 18 | 36 | 3 | 3 | 51 | 148 |
| 3D Finish | feeds unset | Linking | 19 | 30 | 1 | 1 | 51 | 148 |
| 3D Finish | feeds unset | Dressup | 10 | 18 | 1 | 1 | 51 | 148 |
| 3D Finish | wrong tool kind | Geometry | 8 | 16 | 2 | 1 | 51 | 72 |
| 3D Finish | wrong tool kind | Cut | 18 | 32 | 2 | 1 | 51 | 72 |
| 3D Finish | wrong tool kind | Heights | 20 | 43 | 8 | 1 | 51 | 76 |
| 3D Finish | wrong tool kind | FeedsSpeeds | 14 | 27 | 3 | 1 | 51 | 108 |
| 3D Finish | wrong tool kind | Linking | 20 | 31 | 2 | 1 | 51 | 72 |
| 3D Finish | wrong tool kind | Dressup | 11 | 19 | 2 | 1 | 51 | 72 |
| Pocket | last run refused | Geometry | 9 | 23 | 1 | 1 | 50 | 406 |
| Pocket | last run refused | Cut | 20 | 38 | 1 | 1 | 50 | 406 |
| Pocket | last run refused | Heights | 20 | 44 | 6 | 1 | 50 | 406 |
| Pocket | last run refused | FeedsSpeeds | 22 | 59 | 11 | 1 | 50 | 406 |
| Pocket | last run refused | Linking | 19 | 30 | 1 | 1 | 50 | 406 |
| Pocket | last run refused | Dressup | 10 | 18 | 1 | 1 | 50 | 406 |
| 3D Rough | last run refused | Geometry | 6 | 14 | 1 | 1 | 50 | 406 |
| 3D Rough | last run refused | Cut | 19 | 38 | 1 | 1 | 50 | 406 |
| 3D Rough | last run refused | Heights | 18 | 41 | 6 | 1 | 50 | 406 |
| 3D Rough | last run refused | FeedsSpeeds | 22 | 59 | 12 | 1 | 50 | 406 |
| 3D Rough | last run refused | Linking | 19 | 30 | 1 | 1 | 50 | 406 |
| 3D Rough | last run refused | Dressup | 10 | 18 | 1 | 1 | 50 | 406 |
| 3D Finish | last run refused | Geometry | 7 | 15 | 1 | 1 | 50 | 406 |
| 3D Finish | last run refused | Cut | 18 | 32 | 1 | 1 | 50 | 406 |
| 3D Finish | last run refused | Heights | 20 | 43 | 7 | 1 | 50 | 406 |
| 3D Finish | last run refused | FeedsSpeeds | 22 | 51 | 6 | 1 | 50 | 406 |
| 3D Finish | last run refused | Linking | 20 | 31 | 1 | 1 | 50 | 406 |
| 3D Finish | last run refused | Dressup | 11 | 19 | 1 | 1 | 50 | 406 |
| 3D Finish | reach map on | Geometry | 10 | 20 | 2 | 1 | 53 | 184 |
| 3D Finish | reach map on | Cut | 21 | 37 | 2 | 1 | 53 | 184 |
| 3D Finish | reach map on | Heights | 23 | 48 | 8 | 1 | 53 | 184 |
| 3D Finish | reach map on | FeedsSpeeds | 25 | 56 | 7 | 1 | 53 | 184 |
| 3D Finish | reach map on | Linking | 23 | 36 | 2 | 1 | 53 | 184 |
| 3D Finish | reach map on | Dressup | 14 | 24 | 2 | 1 | 53 | 184 |
| Pocket | boundary on (Selection) | Geometry | 12 | 33 | 0 | 2 | 33 | 33 |
| Pocket | boundary on (Selection) | Cut | 24 | 46 | 1 | 1 | 48 | 60 |
| Pocket | boundary on (Selection) | Heights | 19 | 43 | 5 | 1 | 33 | 33 |
| Pocket | boundary on (Selection) | FeedsSpeeds | 21 | 58 | 10 | 1 | 33 | 33 |
| Pocket | boundary on (Selection) | Linking | 18 | 29 | 0 | 1 | 33 | 33 |
| Pocket | boundary on (Selection) | Dressup | 9 | 17 | 0 | 1 | 33 | 33 |
| 3D Rough | boundary on (Selection) | Geometry | 9 | 24 | 0 | 1 | 33 | 33 |
| 3D Rough | boundary on (Selection) | Cut | 23 | 46 | 1 | 1 | 48 | 60 |
| 3D Rough | boundary on (Selection) | Heights | 17 | 40 | 5 | 1 | 33 | 33 |
| 3D Rough | boundary on (Selection) | FeedsSpeeds | 21 | 58 | 11 | 1 | 33 | 33 |
| 3D Rough | boundary on (Selection) | Linking | 18 | 29 | 0 | 1 | 33 | 33 |
| 3D Rough | boundary on (Selection) | Dressup | 9 | 17 | 0 | 1 | 33 | 33 |
| 3D Finish | boundary on (Selection) | Geometry | 10 | 25 | 0 | 1 | 33 | 33 |
| 3D Finish | boundary on (Selection) | Cut | 22 | 40 | 1 | 1 | 48 | 60 |
| 3D Finish | boundary on (Selection) | Heights | 19 | 42 | 6 | 1 | 48 | 76 |
| 3D Finish | boundary on (Selection) | FeedsSpeeds | 21 | 50 | 5 | 1 | 46 | 50 |
| 3D Finish | boundary on (Selection) | Linking | 19 | 30 | 0 | 1 | 33 | 33 |
| 3D Finish | boundary on (Selection) | Dressup | 10 | 18 | 0 | 1 | 33 | 33 |
| Pocket | stale after edit | Geometry | 8 | 23 | 1 | 1 | 33 | 33 |
| Pocket | stale after edit | Cut | 19 | 38 | 1 | 1 | 33 | 33 |
| Pocket | stale after edit | Heights | 19 | 44 | 6 | 1 | 33 | 33 |
| Pocket | stale after edit | FeedsSpeeds | 21 | 59 | 10 | 1 | 33 | 33 |
| Pocket | stale after edit | Linking | 18 | 30 | 1 | 1 | 33 | 33 |
| Pocket | stale after edit | Dressup | 9 | 18 | 1 | 1 | 33 | 33 |
| 3D Rough | stale after edit | Geometry | 5 | 14 | 1 | 1 | 33 | 33 |
| 3D Rough | stale after edit | Cut | 18 | 38 | 1 | 1 | 33 | 33 |
| 3D Rough | stale after edit | Heights | 17 | 41 | 6 | 1 | 33 | 33 |
| 3D Rough | stale after edit | FeedsSpeeds | 21 | 59 | 11 | 1 | 33 | 33 |
| 3D Rough | stale after edit | Linking | 18 | 30 | 1 | 1 | 33 | 33 |
| 3D Rough | stale after edit | Dressup | 9 | 18 | 1 | 1 | 33 | 33 |
| 3D Finish | stale after edit | Geometry | 6 | 15 | 1 | 1 | 33 | 33 |
| 3D Finish | stale after edit | Cut | 17 | 32 | 1 | 1 | 33 | 33 |
| 3D Finish | stale after edit | Heights | 19 | 43 | 7 | 1 | 48 | 76 |
| 3D Finish | stale after edit | FeedsSpeeds | 21 | 51 | 5 | 1 | 46 | 50 |
| 3D Finish | stale after edit | Linking | 19 | 31 | 1 | 1 | 33 | 33 |
| 3D Finish | stale after edit | Dressup | 10 | 19 | 1 | 1 | 33 | 33 |
| Pocket | feeds unset + refused (extra) | Geometry | 9 | 23 | 2 | 1 | 51 | 140 |
| Pocket | feeds unset + refused (extra) | Cut | 20 | 38 | 2 | 1 | 51 | 140 |
| Pocket | feeds unset + refused (extra) | Heights | 20 | 44 | 7 | 1 | 51 | 140 |
| Pocket | feeds unset + refused (extra) | FeedsSpeeds | 20 | 46 | 7 | 3 | 51 | 140 |
| Pocket | feeds unset + refused (extra) | Linking | 19 | 30 | 2 | 1 | 51 | 140 |
| Pocket | feeds unset + refused (extra) | Dressup | 10 | 18 | 2 | 1 | 51 | 140 |
| 3D Rough | feeds unset + refused (extra) | Geometry | 6 | 14 | 2 | 1 | 50 | 145 |
| 3D Rough | feeds unset + refused (extra) | Cut | 19 | 38 | 2 | 1 | 50 | 145 |
| 3D Rough | feeds unset + refused (extra) | Heights | 18 | 41 | 7 | 1 | 50 | 145 |
| 3D Rough | feeds unset + refused (extra) | FeedsSpeeds | 20 | 46 | 7 | 3 | 50 | 145 |
| 3D Rough | feeds unset + refused (extra) | Linking | 19 | 30 | 2 | 1 | 50 | 145 |
| 3D Rough | feeds unset + refused (extra) | Dressup | 10 | 18 | 2 | 1 | 50 | 145 |
| 3D Finish | feeds unset + refused (extra) | Geometry | 7 | 15 | 2 | 1 | 52 | 148 |
| 3D Finish | feeds unset + refused (extra) | Cut | 18 | 32 | 2 | 1 | 52 | 148 |
| 3D Finish | feeds unset + refused (extra) | Heights | 20 | 43 | 8 | 1 | 52 | 148 |
| 3D Finish | feeds unset + refused (extra) | FeedsSpeeds | 19 | 37 | 4 | 3 | 52 | 148 |
| 3D Finish | feeds unset + refused (extra) | Linking | 20 | 31 | 2 | 1 | 52 | 148 |
| 3D Finish | feeds unset + refused (extra) | Dressup | 11 | 19 | 2 | 1 | 52 | 148 |
