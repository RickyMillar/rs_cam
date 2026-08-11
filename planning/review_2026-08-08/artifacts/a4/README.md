# A-4 artifacts — the fixed Feeds & Speeds modal, rendered

TD3 Lane A wave A-4 (execute Checkpoint I). Plan §0 rule 3: any claim about a
visible surface ships with a screenshot.

## Provenance

Own GUI instance, not the operator's. **Debug** build of `rs_cam_gui --mcp`
(`cargo build -p rs_cam_viz --bin rs_cam_gui --features mcp`; no release build
was made, per §0.8), launched with `WAYLAND_DISPLAY` unset so winit takes
XWayland — the remedy Checkpoint L-6 corrected (`WINIT_UNIX_BACKEND` is inert
on winit 0.30). Driven over MCP stdio with B-1's dependency-free client,
`../b1/mcp_stdio_client.py`. Tree at the two commits `9026ddb` (core funnel) +
`2801654` (viz reroute), plus `0321677` (the MCP apply tool) — i.e. the
screenshots are of the shipped fix, not of a scratch build.

Fixture: a copy of `test_data/ux_3d_terrain.toml` with the two model paths
absolutised (the shipped file's relative paths resolve against its own
directory). One toolpath added over MCP: `drop_cutter` on tool index 0, the
**Ø6 mm 2-flute flat end mill**. The operator's `wanaka.toml` was never opened.

## The two states

| File | State | What it shows |
|---|---|---|
| `a4_modal_valid_pairing.png` | flat end mill on a plain DropCutter — a pairing `validate_tool_for_operation` accepts | The comparison grid's trailing column is **blank on every row** (the six per-field `Apply` buttons are gone, I-1), and the single write is `⚡ Apply all — changes the cut` (the attribution I-1 required on a geometry-touching apply). |
| `a4_modal_refused_pairing.png` | same toolpath after `set_toolpath_param scallop_height = 0.01` — a scallop-targeted DropCutter on a flat tip, the second arm of the validator | The modal is **still open and every chart is still drawn** — RPM 10610, feed 1606, plunge 527, WOC 0.18, the nomogram, the legend — and the whole Apply column is replaced by `Cannot apply — this tool cannot run this operation` / `scallop requires curved tip (need ball\|bull\|tapered_ball; got Flat on Parallel)` / the sentence explaining why the numbers are still shown. That is Checkpoint I-3 exactly: the explanation survives, the write becomes impossible. |

The `⚡ Apply all` button is **absent** in the second image. Compare the two at
the same pixel region under `MRR: 173 mm³/min`.

## The MCP tool, on the same instance

`apply_feeds` was exercised live in the same session (transcript in the run
log, quoted here verbatim):

- on the **refused** pairing, `{"index": 0, "scope": "both"}` →
  `ok: false`, `"Error: nothing applied to toolpath 0 — this tool cannot run
  this operation: scallop requires curved tip (need ball|bull|tapered_ball;
  got Flat on Parallel)"`. The agent gets the engine's own words, not a
  successful no-op.
- on the **valid** pairing, `{"index": 0, "scope": "speeds"}` → `ok: true`,
  `applied: {"changes_the_cut": false, "feed_rate": 1606.0,
  "plunge_rate": 527.0, "spindle_rpm": 10610, "stepover": 0.18,
  "depth_per_pass": null}`, summary `"… Cut geometry (DOC/WOC) unchanged."`
  Stepover 0.18 is the pre-call value — the speed-only scope kept its promise.

## NOT EXERCISED

- **The explore-chart Apply, in either state.** `✓ Apply explored values` only
  renders once `⊕ Start exploring` has been clicked, and MCP cannot inject a
  button click (`set_ui_view` opens modals and tabs, not widgets inside them).
  Both its behaviours are sentried instead —
  `explore_apply_takes_the_clamps_but_keeps_the_dragged_point` and
  `explore_apply_refuses_a_refused_pairing` in
  `crates/rs_cam_viz/tests/apply_contract_a3.rs`.
- **The project-rollup tab** (`All toolpaths`), where a refused row is marked
  `⚠` and shows `refused` in place of its Apply. The tab is switched by
  `AppEvent::SetFeedsModalMode`, which no MCP tool emits — `set_ui_view`'s
  `modal` parameter opens the modal but cannot select the tab inside it.
  Sentried by `project_apply_all_skips_a_refused_toolpath_and_reports_it`.
- **The scope-string parsing in `mcp_apply_feeds`** (`"speeds"` /
  `"cut_geometry"` / `"both"` / unknown → error). Two of the four arms were hit
  live above; there is no automated test, because the function is a private
  method on the egui `App` and this crate has no App-level harness. The
  `ApplyScope` values behind them are sentried at the controller
  (`agent_apply_honours_its_declared_scope`).
