# Step G-E (3): the rest-boundary refusals have short faces and an action

Implementer: I4. Date: 2026-10-28. Base: `2881bab01`. Design:
`gui_fix_design.md` §7.7 row 2, with the fit rules of §7.3.

Patch: `stepGE3_tree_vs_2881bab01.patch` (8 files: 4 core, 4 viz). The
tree also holds other agents' uncommitted core edits. These are the
Waterline `helical` rename in `finish_raster.rs`, `operation_configs.rs`
and `perf_golden_sim_metrics.rs`. The patch leaves them out.

## What changed

### Core (`session/compute/generation.rs`; the lead allowed the edit)

- New `pub enum RestBoundaryRefusal { SourceMissing, OwnRegions,
  NotGenerated, NoRegions }`. The `NotGenerated` and `NoRegions` variants
  carry the source id and name. It is re-exported as
  `rs_cam_core::session::RestBoundaryRefusal`.
- Its `Display` is the four refusal texts of today, word for word. The
  wire, MCP and the generation error do not change.
- `derived_rest_regions` (private) is now the one rule of the derived
  regions, and it returns the regions or the typed refusal.
  `resolve_derived_rest_region_polys` maps the refusal to the same
  `SessionError::OperationFailed(text)` as before.
- New read door `ProjectSession::rest_boundary_refusal(index) ->
  Option<RestBoundaryRefusal>`. It gives `None` when the boundary is off,
  is not `DerivedRestRegions`, or resolves.

### Viz

- `ToolpathPanelInputs::rest_boundary_refusal` reads the core door.
- The G-E1 face line under the Generate row now checks for a typed
  refusal first. When the last run's text equals the typed refusal's
  `Display`, the panel draws the short face, and its hover is the full
  text. Otherwise the panel draws the refusal text, truncated, as in G-E1.
- The text compare means a face always describes the refusal the run gave.
  A face never comes from a guess about the present state.

| Refusal | Face (one line) | Action |
|---|---|---|
| `NotGenerated` | `✕ Generate '{source}' first` | `⟳` small button, hover `Generate '{source}'`, sends `AppEvent::GenerateToolpath(source)` |
| `NoRegions` | `✕ '{source}' left no regions` | the same `⟳` (regenerate the source) |
| `OwnRegions` | `Refused · source is this op` | none (the GUI source list leaves the op out; MCP can reach it) |
| `SourceMissing` | the refusal text, truncated (the generic face) | none |

- The face uses `tokens::GLYPH_DANGER` (`✕`), not the design's `⛔`, which
  is not a token. A lib test checks that the `⟳` glyph (U+27F3) is in the
  app fonts.
- The face and its action share one row. The action takes its width
  first (right to left), and the face truncates in the rest (§7.3: "a long
  source name cuts the face, never wraps it").

## Panel line count

A rest refusal state is the clean panel plus 1, as in G-E1. Only the
words of that line change. The design's estimates: 5 to 7 rows before
G-E, 1 now.

## Tests

- Core `--lib the_rest_boundary_refusal_is_the_generation_refusal_g_e`:
  - With no source result, the door gives `NotGenerated`, and its
    `Display` is the exact text `generate_toolpath` refuses with.
  - With no regions it gives `NoRegions`, again with the same text.
  - With regions it gives `None`; with no derived boundary it gives `None`.
- Viz `--lib the_rest_refusal_faces_are_short_g_e`: the four faces, the
  actions, and the `⟳` glyph in the app fonts.
- Viz `--test the_inspector_text_is_bounded_ic1`
  `a_rest_refusal_has_a_short_face_and_an_action_g_e`:
  - The setup: the 3D Finish takes the 3D Rough's rest regions through
    `Command::SetBoundaryConfig`, and the last run failed with the core
    text.
  - The checks: the face `✕ Generate '3D Rough' first` and `⟳` on one
    row, the full text not on the face, no wrap, and no excess width.

## Gates (green)

- `cargo test -p rs_cam_core --lib -- session::compute`: 79 passed. Also
  `-- derived_rest the_rest_boundary_refusal`: 9 passed.
- `cargo test -p rs_cam_core --test a_larger_rest_consumer_tool_refuses_d53`:
  passed.
- `cargo test -p rs_cam_viz --lib`: 602 passed, 2 ignored.
- `cargo test -p rs_cam_viz --test the_inspector_text_is_bounded_ic1`
  (4 tests), `inspector_header_wraps_g_reachwrap`,
  `the_inspector_nests_once_dc5`: green.
- `cargo clippy --no-deps -p rs_cam_core -p rs_cam_viz --lib --tests -- -D warnings`:
  clean.
- `cargo fmt -p rs_cam_viz -p rs_cam_core -- --check`: clean. The two core
  files were formatted alone with rustfmt.
