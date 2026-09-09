# R04 source track — support agent report (v1, PARTIAL, received by message 2026-09-09 16:28–16:36 +12)

Read-only Claude Code `Explore` subagent `r04-source`. Two messages arrived truncated by the
message channel; the parts below are what reached the orchestrating session `rs-cam-b9`.
A second agent (`r04-source-2`) writes the full report to `r04_source_track.md`.
The orchestrator spot-checked: `ValueRow::suggest` writes `round_suggestion_value(s.recommended)`
straight into the field with no clamp (`components/value_row.rs:112-135`) ✔; `dv_pill` feeds it
`r.axial_depth_mm` (`properties/mod.rs:4879-4900`, `operations/boundary_2d.rs:128`) ✔.

---

**Headline (agent).** The Checkpoint I "one application funnel" holds for the four recipe-scoped Apply buttons and for the MCP tool. It does **not** hold for the **24 per-field ⚡ suggest pills** still shipping on the inspector's Geometry and Feeds tabs. Those pills write raw calculator output with no clamp, reproducing the exact defect the 2026-08-12 ruling deleted from the modal, in a file no sentry reads. Separately, the inspector's Feeds card displays pre-clamp recommendation values under bare labels, so on a fresh Pocket it advertises a 3.50× DOC change that its own Apply button will not make.

## Route inventory (partial)

| # | Route | Entry point | Write path |
|---|---|---|---|
| R1 | Inspector Feeds tab, speed recipe | `⚡⚡ Apply recommended speeds` | `apply_speeds_to_op` — `properties/mod.rs:2083-2093` |
| R2 | Inspector Feeds tab, cut recipe | `⚡ Apply cut geometry` | `apply_cut_geometry_to_op` — `properties/mod.rs:2125-2135` |
| R3 | **Inspector Feeds tab, per-field pills** | small `⚡` on Feed / Plunge | `ValueRow::suggest` writes `*self.value` direct — `components/value_row.rs:112-135`, called `properties/mod.rs:2019, 2034` |
| R4 | **Inspector Geometry tab, per-field pills** | small `⚡` on Stepover / Depth/Pass / Max depth | `dv_pill` → same `ValueRow::suggest` — `properties/mod.rs:4879-4900`; 22 call sites in `operations/{boundary_2d,surface_3d,finishing,drill}.rs` |
| R5 | Inspector manual drag | any `DragValue` | `entry.operation.set_*`, flushed by `write_entry_config_to_session` |
| R6 | Inspector stale-default Fix | `✓ Fix (set to N)` banner | `validate::apply_stale_default_to_op` — `properties/mod.rs:4024, 3227` |
| R7 | Feeds modal, single toolpath | `⚡ Apply all — changes the cut` | `AppEvent::ApplyFeedsAll` → funnel — `feeds_modal.rs:647-657` |
| R8 | Feeds modal, explore chart | `✓ Apply explored values` | `ApplyFeedsExplore`, scope Speeds — `feeds_modal.rs:2200-2209` |
| R9 | Feeds modal, project row | `Apply` per row | `ApplyFeedsAll(r.id)` — `feeds_modal.rs:2984` |
| R10 | Feeds modal, project batch | `⚡ Apply selected — changes the cut` | `ApplyFeedsProjectSelected` — `feeds_modal.rs:2886` |
| R11 | Feeds modal, project batch all | `⚡⚡ Apply all toolpaths — changes the cut` | `ApplyFeedsProject` — `feeds_modal.rs:2897` |
| R12 | Optimize modal, candidate | `Apply` / `Apply ⭐` (enabled only when `!verdict.any_exceeded()`) | `ApplyOptimizeCandidate` — `optimize_modal.rs:849-861`, handler `events/mod.rs:519-643` |
| R13 | Optimize modal, axis suggestion | `Apply & re-optimize` | `ReoptimizeWithAxisOverride` → `resolve_operation_invariants` — `optimize_modal.rs:964-978`, handler `events/mod.rs:655-770` |
| R14 | Optimize project rollup | `Apply selected` | `ApplyOptimizeProject` — `optimize_project.rs:229-232`, handler `events/mod.rs:1160-1295` |
| R15 | MCP `apply_feeds` | tool call, `scope` argument | funnel — `app/mcp.rs:4884-4959` |
| R16… | (truncated) | | |

## 3.5 The Feeds card labels a recommendation, not the stored operation (agent, verified by the orchestrator against `properties/mod.rs:2047, 2105` and `get_suggest_rationale`)

**The `⚡ Apply cut geometry` button on that Pocket is a no-op, while the line directly above it advertises 4.20 mm.**

(a) `result` is `entry.feeds_result`, cloned at `properties/mod.rs:1965`, written at `:1657` from `feeds_result_for_operation` (`feeds/suggest.rs:779-791`) which calls `feeds::calculate` (`feeds/mod.rs:1045`). `enforce_invariants` (`suggest.rs:1897`, private) is NOT applied, so `result` carries none of the DPP-moving passes (`suggest.rs:1943-1951`: `pick_axial_envelope`, `clamp_dpp_to_rigidity`, `clamp_dpp_to_cutting_length`, `backoff_dpp_for_deflection`, pass 9). The rigidity cap is `clamp_dpp_to_rigidity` (`suggest.rs:2110-2137`, `cap = factor × tool.diameter`). `calculate` honours a caller-supplied DOC (`feeds/mod.rs:1511-1513`) but Pocket is in the explicit no-hint arm of `OperationConfig::feeds_hints` (`catalog.rs:2501-2519`), so the calculator never sees the stored 1.2 and derives 4.2 from LUT / diameter-factor defaults. The `WOC:` row is equally detached from the stored stepover. Only Scallop, UnifiedFinish, DropCutter, Waterline, SteepShallow, VCarve and RampFinish feed their own value back in (`catalog.rs:2470-2500`).

(b) `apply_cut_geometry_to_op` writes **1.2**, not 4.2: both cut-geometry routes reach `apply_feeds_subset` (`suggest.rs:856`), which writes the raw recommendation into a scratch clone (`:868-872`), runs `enforce_invariants` (`:916`), then copies out (`:931-932`). Add-time Suggest uses the same funnel (`suggest_params` `:658` → `suggest_for_operation` `:680` → `apply_feeds_result_to_op` `:694` → `apply_feeds_subset(ApplyScope::Both)`), so the button writes the value already stored. **The button does not bypass the cap. The label does.**

(c) [truncated in transit — see `r04_source_track.md` from the second agent]

(d) [truncated — HYPOTHESIS by the orchestrator: 0.0435 is `result.chip_load_mm` at the calculator's own recommended feed, before the rubbing-floor/derate path that produced the stored 750 mm/min]
