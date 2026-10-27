# Integration step 7.7: the C12 judge on the product arm

Date: 2026-10-28. Branch `geodesic-integration`, base `5a72cc07d` (7.5b).
The implementer did not commit.

Patch: `step77_tree_vs_5a72cc07d.patch`, md5
`3ad979e9c908eacada6ff2f36e425417`. It holds 2 files: the new judge
`tests/geodesic_adaptive_holds_the_pass_load_c12.rs` (1296 lines) and the
`adaptive3d/CLAUDE.md` sentry line. No planning file is in it.
`git apply --check -R` passes.

Source: R `tests/adaptive3d_geodesic_arm_c12.rs` at 3682d1dcc (2602
lines). The judge runs on `ClearingStrategy3d::GeodesicAdaptive` with no
`research` feature and no environment variable. It reads only product
output: the toolpath, its annotations and `Move::feed_cap`.

No 3D research selector is left on M: `RESEARCH_ARM_ENV` and the other
`RS_CAM_RESEARCH_*` variables were deleted in 7.4a (`rg` finds none in
`crates/`). The research C12 file is not on M (it never landed), so
nothing is deleted.

## 1. One definition of the load

R's `exact_width_at` summed its own per-strip chip area,
`Σ cutter.mrr_cross_section_mm2(h, pitch) / cutter.mrr_cross_section_mm2(d, D)`.
The product judge calls `tool_load::load_fraction::chip_load_fraction`
(step 7.1) with `cutter.geometry_hint()` on the same strips (index by
`round(u / pitch)`, depth the deepest stock over the cutter). The one
function integrates by the trapezoid between strip centres, so a band
reads its extent between the outer centres: at most one pitch (0.05 mm,
0.008 of D) under R's rectangle sum.

## 2. The exemptions

As R, with one change: R exempted turn moves and departure-fed moves by
matching its turn and departure records to moves by position. Those
records were deleted with part F (7.4a). The product exempts every move
that carries `Move::feed_cap` (a planned turn or a plunge departure) and
judges it by the feed rule instead: feed ≤ cap, cap < the commanded feed
(`cap_faults`). The geometric departure exemption (`departing_moves`), the
entry, vertical, waterline and AgentSearch perimeter-band exemptions are
R's. For the screened capped moves the readout prints the peak of the
exact load times cap over feed (the removal rate as a fraction of the
steady rate); it is not asserted.

## 3. The tests (ported)

| Product test | From R | Result | Time |
|---|---|---|---|
| `geodesic_adaptive_holds_the_pass_load_at_two_levels` (ignored, heavy) | `the_geodesic_arm_generates_and_holds_the_pass_load_at_two_levels` | ok: 2063 moves, judged at Z 4.5…10.5, 0 over the screen, 0 over the exact bound, no link over the bar, 919 capped samples (exact load × cap/feed peak 0.368), feed rule held. Control AgentSearch: 225 over the exact bound (printed) | 514 s |
| `geodesic_adaptive_feeds_its_turns_down_at_the_level` | `the_geodesic_arm_feeds_its_turns_down_at_the_level` | ok: 32 capped turn moves at the level, 96 capped departure moves, feed rule held | 6 s (with the channel test) |
| `no_held_link_cuts_standing_stock_with_the_group_filter_on` (ignored, heavy) | `no_held_link_crosses_standing_stock_with_the_group_filter_on` | ok: filter 15 mm, no fed link over the bar | 498 s |
| `the_stock_read_split_holds_the_exact_bound_on_draped_steps` (ignored, heavy) | the same name | ok: 0 over the exact bound | 35 s |
| `the_stock_read_split_adds_no_over_move_on_the_shelf` (ignored, heavy) | `the_width_read_splits_a_wide_draped_step` | ok: 0 over moves (stock read) against 3 (path shape) | 149 s (with the next) |
| `geodesic_adaptive_leaves_the_channel_to_the_edge_pass` | `the_research_arm_leaves_the_channel_to_the_edge_pass` | ok: the plan is empty on the channel (0 moves, as R's pin of 7.4a); the D44 sentries read the regions it states. No longer ignored | (above) |
| `no_entry_plunges_more_than_a_pass_into_stock` (ignored, heavy) | `no_split_entry_plunges_more_than_a_pass_into_stock` | ok: deepest stroke 6.000 mm on both fixtures and both splits (pass 6) | (above) |

## 4. Dropped, with the reason

Tests:

| R test | Reason |
|---|---|
| `c34_frame_ring_grow_probe` | an instrument (prints, asserts nothing) that called `research_grown_outlines`, a research name that is `grown_outlines` (private) on M |
| `the_slab_field_holds_the_load_on_a_sloped_floor` | its subject, `SlabField`, is not ported (design §3, part A) |
| `the_slab_field_reads_the_floor_band_beside_a_shelf` | the same |

Assertions inside the ported tests:

| R assertion | Reason |
|---|---|
| per-option loops (`C5_CURV` … `C37_SPIRAL_HEADON`) and "`geodesic_curv` and `geodesic_curv_split` differ" | the options are deleted (7.3d); the product is one preset |
| every entry has a recorded cause (C40) | the entry-cause record was a planner instrument, deleted in 7.4a |
| `held_link_faults` empty (C22a) | the same; the simulation link check (`link_over`, a fed link removes no more than Depth/Pass + one cell) holds C22a on the product, in the two-level judge and in the group-filter test |
| core zones: at least one, no re-entry inside (C35) | the core-zone record was an instrument |
| join lines (C37), region openings (C31: every region opened with the grown outline) | instruments. C31 stays judged indirectly: the product arm gets no perimeter-band exemption, and 0 samples read over in the band |
| turns planned > 0, `turn_faults` against the turn summary, departure records fed (`departure_faults`) | the summaries were matched to moves by position; part F replaced them with caps on the cut. Replaced by `cap_faults` and by "at least one capped turn and one capped departure" |
| split counters: at least one step avoided (§16.7), width splits > 0 (§16.9), split entries over the pass (§19.3) | `SplitStats` was an instrument. The property assertions (no judged sample over; no more over moves than path shape; no stroke over the pass) stay |
| the router does not run with `KeepDownRoute::Off` | `RouteStats` was an instrument |

## 5. Gates

| Gate | Result |
|---|---|
| `cargo fmt --all -- --check`, full clippy (heavy-tests, research, test-support) | ok |
| the judge, plain run | 2 ok, 5 ignored |
| the judge, each ignored test by name | all ok (§3) |
| `folder_maps_name_real_files_f140`, `every_ignore_states_its_kind_f152`, `gate_tests_read_no_operator_paths_f155`, `the_research_arms_are_feature_gated_fin01` | ok |

No product code changed, so byte parity and opgolden are not affected.
