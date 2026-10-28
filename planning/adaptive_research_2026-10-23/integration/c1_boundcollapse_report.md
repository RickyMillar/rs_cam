# C1, G-BOUNDCOLLAPSE: a collapsed boundary refuses generation

Date: 2026-10-28. Branch `geodesic-integration`, base `f098d1623`.
The implementer did not commit.

Patch: `c1_boundcollapse_tree_vs_f098d1623.patch`,
md5 `87034be67284d1be628d883038d62902` (re-frozen after the lead's D53
ruling; the first freeze was `d820db22…`). 15 files (14 changed, 1 new).
`git apply --check -R` passes. The patch holds no `rs_cam_viz` file.
The start point was I2's WIP (`c1_WIP_from_I2.patch`), kept nearly as it was.

## 1. The D53 interaction (lead ruling 2026-10-28: do a and b)

The first freeze turned `a_larger_rest_consumer_tool_refuses_d53` red. Its
case "Ø8 with `Inside` generates" is a collapsed boundary: the Ø6 rest
region of the 4.8 mm half-width channel (about 249 mm²) has no area left
after a 4 mm inset. At HEAD that consumer generated with NO boundary clip
(the empty set passed through). So the D53 fix text led into the
G-BOUNDCOLLAPSE exposure.

- a. The sentry now has three groups:
  1. Ø8 `Center` and `Outside` refuse with D53 (as before; the needle
     `Ø6.000` is added);
  2. Ø6 and Ø4 `Center` generate (as before);
  3. Ø8 `Inside` on the narrow channel refuses with the G-BOUNDCOLLAPSE
     sentence, equal to `boundary_collapse(1)`; Ø8 `Inside` on a channel of
     half-width `WIDE_CHANNEL_HALF_WIDTH = LARGER_DIAMETER` (16 mm wide,
     twice the Ø8 tool) generates, and the read door gives `None`.
  The wide channel is a planted fixture variant (`session_on`). A probe
  (deleted) measured its rest region: one region of about 3339 mm²; Ø8
  `Inside` keeps area for half-widths 8, 10 and 12, and collapses at 4.8.
- b. The D53 refusal (`refuse_larger_rest_consumer`) now ends: "Use a tool
  no larger than the tool of '<source>' (Ø6.000), or Inside containment if
  the regions are wider than this tool (Ø8.000)." The radii stay in the
  sentence. No other file holds the old text (rg over the tree, `planning/`
  excluded).

## 2. Change

- `session/compute/generation.rs`:
  - `clip_containment` (from the WIP): the one computation of the
    post-generation clip set, for `Single` (stock, silhouette) and
    `Regions` (selection, rest regions, tier regions).
  - `apply_boundary_clip` and `apply_boundary_clip_multi` refuse an empty
    set (`collapsed_containment_refusal`, which replaces
    `resolve_collapsed_containment`). The `findings` parameter goes: the
    clip records no finding now.
  - NEW read door `pub fn boundary_collapse(&self, index) ->
    Result<Option<BoundaryCollapse>, SessionError>`. It resolves the
    generation inputs and the post-clip regions, then calls
    `boundary_collapse_of`. `None`: the boundary is off, the op is never
    clipped (Pin Drill, D2), the set has area, or an offset FAILED (the
    clip refuses that with the failure text).
  - NEW `pub struct BoundaryCollapse { op_name, source_label, containment,
    tool_diameter_mm, offset_mm }` with `message()`: the op, the boundary,
    and the fix (smaller tool, less boundary offset, another containment).
    Re-exported as `rs_cam_core::session::BoundaryCollapse`.
  - `post_clip_regions` (moved out of `capture_generate_handle`, code
    unchanged) so the read door and generation share it.
- `session/compute.rs`: `capture_generate_handle` refuses with
  `SessionError::OperationFailed(collapse.message())` after it resolves
  the post-clip regions, before any generator work. The same error class
  as D53. `clip_and_split_entries` loses its unused `findings` parameter.
- Docs: `geometry/boundary.rs` (the empty-set contract), `geometry/CLAUDE.md`
  (the invariant), `toolpath_stats.rs` (the finding is no longer recorded),
  the Inlay `pocket_depth` help text (it said "read boundary_clip_dropped"),
  `FEATURE_CATALOG.md` (the Inlay row and the boundary-clipping line).

Cost of the read door for the GUI: one `resolve_generation_inputs` (the
WP11b frame-loop resolver) plus the region resolution. A `PlannedTierRegions`
source forces the spatial index and reads the memoised tier map. The GUI
must memoise the answer per revision.

## 3. Not changed (follow-up for the lead)

`BoundaryClipDroppedFinding`, `ToolpathStats::boundary_clip_dropped`, its
diagnostic id `geom.boundary_clip_dropped`, the narration field and the viz
badge stay. No shipped path records the finding now. Their removal touches
`rs_cam_viz` (`ui/properties/tab_badges.rs`), so I did not do it.

## 4. Tests

| Test | Change |
|---|---|
| NEW `a_collapsed_boundary_refuses_g_boundcollapse` | CP and Geodesic, `Stock` boundary: `Inside` with a 4 mm square left under a Ø6 tool, and `Center` with an offset past the stock, refuse; the read door gives the same sentence; no result is stored. `Inside` offset 0 generates and the read door gives `None`. 2/2 |
| planted red | `boundary_collapse_of` returns `None`: the sentry fails ("the read door finds no collapse"); file restored |
| `--lib apply_boundary_clip_multi_all_regions_collapsed_refuses` | was `..._returns_original`; now asserts the refusal |
| `boundary_clip_escape_f1` | `a_genuine_collapse_still_passes_through_and_now_says_so` becomes `a_genuine_collapse_refuses_g_boundcollapse` (the cause is still `None`, not a failure); the failure arms lose the finding checks |
| `skipped_boundary_offset_f8` | `a_collapsed_user_offset_reaches_the_clip_as_a_reported_pass_through` becomes `a_collapsed_user_offset_refuses_at_the_clip` |
| `boundary_clip_invalidates_spans` | the `findings` argument goes |
| `a_larger_rest_consumer_tool_refuses_d53` | §1 |
| `adaptive3d_boundary_clear_parity` | comments only ("ships UNCLIPPED" becomes "refuses") |

No golden or fixture is re-blessed.

## 5. Gates

| Gate | Result |
|---|---|
| new sentry, f1, f8, spans | 2/2, 4/4, 2/2, 4/4 |
| `rs_cam_core --lib` | 2720 passed, 13 ignored |
| `adaptive3d_emission_byte_parity`, `every_op_keeps_its_toolpath_g_opgolden` | 6/6, 1/1 (1 ignored): shipped output unchanged |
| `adaptive3d_boundary_clear_parity`, `findings_transport_join_h21`, `silhouette_boundary_has_no_holes_r1` | 4/4, 3/3, 5/5 |
| `a_larger_rest_consumer_tool_refuses_d53` | 1/1 after §1 |
| `rs_cam_cli -q` (every CLI test binary) | all ok; no CLI fixture generated through a collapse |
| `cargo fmt --all -- --check` | ok |
| clippy `rs_cam_core --all-targets`, heavy-tests, research, test-support | ok |
| `cargo check -p rs_cam_viz -p rs_cam_cli -p rs_cam_mcp --all-targets` | ok |

Not run: the core full gate. Another core test that generates through a
collapsed boundary would now refuse. The full gate finds such a test; it
needs the operator's go-ahead.
