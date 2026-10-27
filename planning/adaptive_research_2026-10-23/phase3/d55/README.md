# D55: G-AIRPECK, a plunge entry pecks down through cleared air

Tree: worktree `adaptive-head`, base `3682d1dcc` (D53 committed). Not committed.
Patch: `tree_vs_3682d1dcc.patch`, md5 `b855a8da7523f8427cb3c3c2b82e8f86`. The pre-registration is `PREREG.md`, written before the change and before any parity run.

## 1. Cause (line numbers at `3682d1dcc`)

The chain:
1. `plan_entry` (`adaptive3d/clearing.rs:1177`) reads two values. The first is the entry's rapid floor, `max_conservative_top_z_in_disc` (`:1185`). This is the sliver-safe stock top over every cell whose square touches the tool disc (`dexel_stock/mod.rs:415`, `reach = radius + cs/2`). The second is the contact read, `contact_z` (`:1203`): the cell-centre read over the same disc.
2. The emitter rapids to `descent_start_z` = floor + 0.5 (`adaptive3d/path.rs:1646`, used at `:1863`).
3. For `Plunge` it calls `emit_peck_plunge(&mut tp, entry, descent_floor, params)` (`:1918`). The ladder starts at `ladder_start = min(start_z, max(entry.z, stock_top + ENTRY_CLEARANCE))` (`:174`) and pecks in Depth/Pass strokes down to the entry.
4. The contact read never reaches the peck. The helix and the ramp do take it (`EntrySafety::contact_top`, `dressup/entry_descent.rs:432` `rapid_to_entry_top`, G-AIRRAMP): they feed straight through the air to `contact_top + entry_clearance_mm` and start their cut there.

Why wanaka and not Corne:
- On wanaka, op 1's boundary is the silhouette with `Inside` containment and offset 0. So the uncut holding frame starts at the outline.
- After D48, op 7's tool centres stop one radius inside op 1's containment, so op 7's disc ends at the outline. The sliver-safe read takes the frame cells whose squares touch the disc, so the rapid floor is the stock top.
- The op 7 boundary is `DerivedRestRegions` with many regions, so there is no single pre-boundary, and the boundary clear (`path.rs:965`) does not clear the frame from op 7's planner stock.
- The ladder then pecks from the stock top down to the entry, through the band that op 1 cleared.

From `fix37/wanaka_s2p0/program.nc` (op 7; `peck_sites.py`):

| Site position | Sites | Descents per site | Ladder top |
|---|---|---|---|
| Edge of op 7's reach (≤ 1 mm) | 274 | 3 or more | Z 0.5 = stock top + 0.5 |
| Interior (> 10 mm) | 53 | 1 | low |

- On Corne, op 1's boundary is the silhouette with `Center` containment and offset +6 mm. Op 1 clears 9 mm past the outline, so no full-height stock stands beside op 5's discs. Op 5 has a median of 1 descent per site, and its ladder tops are low (`fix37/corne_B_s2p1/program.nc`).
- The lead's suspect, "the planner stock does not know op 1's cuts", is refuted. The interior sites read op 1's cut stock and descend once. Only the sites whose disc touches the uncut frame read the stock top.

The mechanism is not specific to rest ops. Any plunge entry whose disc edge touches stock higher than the column it cuts (a wall, a frame, a step) pecked from that higher stock. The parity fixtures show it on fresh stock too (below).

## 2. Fix (the shared entry code)

- `emit_peck_plunge` (`adaptive3d/path.rs`) takes the contact read (`contact_top: Option<f64>`, the `RapidWithFloor::contact_z` the helix and the ramp already get).
- The rapid still stops at the sliver-safe floor (unchanged).
- The ladder starts at `peck_top = min(ladder_start, max(contact_top, entry.z) + entry_clearance_mm)`, the start the helix and the ramp use.
- When the column below `peck_top` needs pecks, one straight feed goes through the air down to `peck_top`, and the pecks follow, each at most one Depth/Pass.
- When it needs none, one feed goes from the rapid floor to the entry.
- With no contact read (a plain `Rapid` entry, off the planner grid), `peck_top` is the old ladder start, so the moves are as before.
- No new number is added: the clearance is the op's `entry_clearance_mm`, which the helix and the ramp read.
- The planner stamp is unchanged: the planner stamps the vertical column from the descent floor to the entry, not the peck shape.

## 3. Sentries

- `crates/rs_cam_core/tests/a_rest_entry_does_not_peck_through_cleared_air_d55.rs` (session, not ignored, no research arm):
  - Fixture: op 0 (contour parallel, 1 mm leave) clears a pocket; op 1 roughs it on the remaining stock with no leave and `Plunge`.
  - Every entry site is one fed stroke. Non-vacuity: a site's column from its rapid floor is deeper than one Depth/Pass.
  - Red without the fix (`sentry_red_session.txt`): the site `(2, 7.5, 4.0)` was 2 strokes. Green: `(1, 7.5, 4.0)`.
- Emitter tests in `adaptive3d/path.rs` (`--lib _d55`):
  - `a_plunge_through_cleared_air_is_one_stroke_d55`: wall top 10, contact 0.2. One stroke from the rapid floor 10.5 to the entry, and no peck retract.
  - `a_plunge_into_real_stock_still_pecks_d55`: the planted case. Contact = floor = 10, and every stroke is at most one Depth/Pass (plus the re-entry from the retract).
  - `air_above_real_stock_feeds_then_pecks_d55`: contact 6. The first feed ends at the contact start 6.5, and the pecks below are each at most one Depth/Pass.
  - Red without the fix (`sentry_red_units.txt`):
    - the air column pecked `[(10.5, 7.5), (8.0, 4.5), (5.0, 1.5), (2.0, 0.0)]`;
    - the first feed ended at 7.5, not at 6.5;
    - the real-stock test passes in both trees, because it guards that the fix keeps pecking.

## 4. Parity: shipped output changes (pre-registered, NOT re-blessed)

Changed: every `ClearingStrategy3d` with `Plunge` entries (the 3D Rough default), where an entry's contact start is below its rapid floor. The shipped AgentSearch and ContourParallel change, and so do the 2D-in-3D strategies (adaptive, contour spiral) and the research arm. Helix, Ramp and every 2D op do not change.

`adaptive3d_emission_byte_parity`, 4 of 5 cases moved. From the bless text, captured and then restored (`parity_text/` holds the new text; `xy_only.py` compares):

| Case | Moves | XY moves (bitwise) | Removed |
|---|---|---|---|
| adaptive | 591 → 553 | 389 → 389, identical | 13 `EntryPlunge` feeds, 25 `Retract` rapids |
| agent_search | 378 → 332 | 201 → 201, identical | 20 feeds, 26 retracts |
| contour_parallel | 433 → 422 | 329 → 329, identical | 5 feeds, 6 retracts |
| contour_spiral | 995 → 857 | 325 → 325, identical | 66 feeds, 72 retracts |

`every_op_keeps_its_toolpath_g_opgolden`, 2 rows (16 field changes):

| Row | Moves | Rapids | Cutting length (mm) | Rapid length (mm) | Spans |
|---|---|---|---|---|---|
| `adaptive3d/adaptive` | 297 → 288 | 17 → 12 | 1040.712 → 1038.212 | 113.999 → 111.499 | 61 → 56 |
| `adaptive3d/fresh` | 379 → 368 | 18 → 12 | 1958.852 → 1955.852 | 105.62 → 102.62 | 86 → 80 |

The hashes move with these.

As predicted:
- Only vertical entry moves change. Every XY move is bit-identical, and so is the planner (`adaptive3d_planner_never_ahead_of_emitted_path` and `adaptive3d_plan_matches_merged_path_g_plansimgap` pass).
- The cutting-length drops are the removed re-feeds after the peck retracts.

Two points do not match the pre-registration exactly:
- `contour_parallel`, which I expected to have the fewest such entries, changed the least (5 feeds).
- The `fresh` row changed, so the defect occurs on fresh stock too. Its entries at later levels beside a wall pecked from the wall top.

The fixtures and goldens are left as they were, so these two tests are RED on this tree until the lead decides. No other golden moved: 20 fingerprint tests pass, and so do `crease_own_region_pr6b` and `finish_resolution_policy_pr3`.

## Gates (all through `scripts/cargo_lane.sh`, in the foreground)

| Gate | Result |
|---|---|
| `cargo fmt --all -- --check` | ok |
| clippy, full features with and without `research` | ok |
| `adaptive3d_emission_byte_parity` | RED by design: 4 cases (above) |
| `every_op_keeps_its_toolpath_g_opgolden` | RED by design: 2 rows (above) |
| the 13 adaptive3d folder sentries (`--features test-support`) | all ok |
| D55 session sentry; `--lib _d55` (3) and the old peck tests | ok |
| D53, D47, f140, the 4 rest-consumer sentries | ok |
| D44 x2, D48, D50 (arm env, `--ignored`) | 1/1 each |
| `rs_cam_core --lib`, plain and research | 2704 and 2755 ok |
| `rs_cam_cli` | 92 ok |
| `rs_cam_viz -j 2 --no-fail-fast` | 1329 ok, 7 failed: the same 7 that fail at base (D47 README) |

Not measured: the time saved on wanaka op 7. Expected: of the 916 descents at 339 sites (s2.0), the 274 edge sites lose the peck strokes and retracts above their contact start. The feed through the air from the rapid floor to the contact start remains, because it is the sliver-safe rule. That feed is about the same length as the old ladder, so most of the 537 s of entry time may remain. A re-run of fix37 on this tree measures it.

## 5. Re-bless (lead ruling, research branch only)

The 4 parity fixtures were set to exactly the text in `parity_text/` (copied, not re-generated). The op golden was blessed with `RS_CAM_BLESS_OPGOLDEN=1`; its diff is the 16 fields of the 2 rows below and nothing else. Both tests are green after the re-bless (parity 5/5 plain and research, opgolden 1/1). The port to master waits for the operator.

| File | Moves | First differing move (old → new) |
|---|---|---|
| `tests/fixtures/adaptive3d_emission/adaptive.txt` | 591 → 553 | move 76, `EntryPlunge` at (−1.750, −11.000): Z 9.500 → 9.000 |
| `tests/fixtures/adaptive3d_emission/agent_search.txt` | 378 → 332 | move 41, `EntryPlunge` at (−5.450, −3.376): Z 9.500 → 9.458 |
| `tests/fixtures/adaptive3d_emission/contour_parallel.txt` | 433 → 422 | move 130, `EntryPlunge` at (5.250, −9.500): Z 6.500 → 6.000 |
| `tests/fixtures/adaptive3d_emission/contour_spiral.txt` | 995 → 857 | move 92, `EntryPlunge` at (−4.389, 4.874): Z 9.500 → 9.391 |
| `tests/fixtures/op_golden_g_opgolden.toml`, row `adaptive3d/adaptive` | 297 → 288 | rapids 17 → 12, linears 280 → 276, cutting 1040.712 → 1038.212 mm, rapid 113.999 → 111.499 mm, spans 61 → 56, both hashes |
| `tests/fixtures/op_golden_g_opgolden.toml`, row `adaptive3d/fresh` | 379 → 368 | rapids 18 → 12, linears 361 → 356, cutting 1958.852 → 1955.852 mm, rapid 105.62 → 102.62 mm, spans 86 → 80, both hashes |

In each parity case the first difference is a plunge feed at the same XY. The old feed ended at a peck step (the floor less one Depth/Pass), and the new one ends at the contact start or at the entry. The XY moves stay bit-identical (§4).
