# J8 GUI half — F1.18 / G-DEPTHSTOCKCORE: the GUI consumes the core depth rule

Date: 2026-09-10. Branch: `ui-fix/lane-j78`. Worktree:
`/home/ricky/personal_repos/rs_cam_wt_p3`. Base: `9292287a`.

The core half is `reports/J8.md`. This report covers the GUI switchover.

## The head was already red, and the switchover is what makes it green

The merge that brought the core half onto the UI branch left BOTH rules
firing. `collect_diagnostics` called `diagnose_toolpath_inputs` — which since
F1.18 emits `geom.depth_beyond_stock` from `heights_checks` — and then
appended the GUI's own copy under the same id. The operator read the
identical Safety sentence twice.

The orchestrator ran the gate on `9292287a` and sent me the output. **I did
not run it.** Verbatim:

```
---- a_pocket_deeper_than_the_stock_cautions_on_the_header_and_does_not_block stdout ----
thread 'a_pocket_deeper_than_the_stock_cautions_on_the_header_and_does_not_block' panicked at crates/rs_cam_viz/tests/depth_beyond_stock_cautions_g_depthstock.rs:197:5:
assertion `left == right` failed: (a) expected one depth caution on the header, got [Diagnostic { id: DiagnosticId("geom.depth_beyond_stock"), ..., evidence: None, ... }, Diagnostic { id: DiagnosticId("geom.depth_beyond_stock"), ..., evidence: Some(GeometryCompare { lhs_label: "bottom_z", lhs_value: -7.0, rhs_label: "stock_bottom_z", rhs_value: 0.0, unit: "mm" }), ... }]
all: ["Depth exceeds stock thickness by 7.00 mm", "Depth exceeds stock thickness by 7.00 mm"]
  left: 2
 right: 1

---- a_the_rule_reads_the_excess_and_the_diagnostic_carries_its_id stdout ----
thread 'a_the_rule_reads_the_excess_and_the_diagnostic_carries_its_id' panicked at crates/rs_cam_viz/tests/depth_beyond_stock_cautions_g_depthstock.rs:246:5:
assertion `left == right` failed: (a) one diagnostic under the rule's id
  left: 2
 right: 1

test result: FAILED. 7 passed; 2 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

That IS the red-first evidence for this half, and it is stronger than one I
would have constructed, because nobody wrote it to catch this.

### Two corrections to the reading of that output

1. **The evidence block belongs to the GUI rule, not to the core rule.** The
   handover message said the opposite. `collect_diagnostics` extends with the
   core list FIRST and pushes the GUI finding SECOND, so the first entry
   (`evidence: None`) is core's `heights_checks` push and the second
   (`GeometryCompare { lhs_label: "bottom_z", … }`) is
   `depth_beyond_stock_diagnostic`, the deleted GUI builder — the only site
   in the workspace that ever wrote that label pair. So the switchover would
   have LOST an operator-visible line, not gained one. I carried the evidence
   into core's push rather than let it vanish; see below.
2. **`stock_bottom_z: 0.0` is measured, not an unset default.** The fixture
   builds `StockConfig { z: 18.0, auto_from_model: false, ..default() }`, and
   `StockConfig::default()` sets `origin_z: 0.0`
   (`compute/stock_config.rs:483`) while `bbox()` returns
   `min.z = origin_z`, `max.z = origin_z + z` (`:532-542`). The board spans
   `0 .. 18` in that fixture, so the stock bottom really is `0.0`. The same
   test asserts `stock_thickness_mm == 18.0` and that assertion passes. There
   is no NOT-MEASURED value being reported as a number here.

## What changed

| Site | Change |
|---|---|
| `crates/rs_cam_core/src/diagnostics/adapters/from_static_checks.rs` | The core push carries the `GeometryCompare` evidence the GUI rule used to carry. No logic change. |
| `crates/rs_cam_viz/src/ui/properties/operations/mod.rs` | DELETED: `DEPTH_BEYOND_STOCK_ID`, the `DepthBeyondStock` struct and its `message()`, the local epsilon, `depth_beyond_stock_applies`, the rule body of `depth_beyond_stock`, `depth_beyond_stock_diagnostic`, and the append in `collect_diagnostics`. |
| same | ADDED: a re-export of core's `DepthBeyondStock`, `diagnostics_heights` (the snapshot builder), and a `depth_beyond_stock` that carries no rule and only adapts inputs. |
| `crates/rs_cam_viz/src/ui/properties/mod.rs` | `DEPTH_BEYOND_STOCK_ID` left the re-export list. `depth_caution_row`'s hover no longer sends the operator to the Bottom Z field. |
| `crates/rs_cam_viz/tests/depth_beyond_stock_cautions_g_depthstock.rs` | Case (d) re-pinned; the id comes from `ids::GEOM_DEPTH_BEYOND_STOCK`. |

The caution row's hover used to end "Check the depth, the stock thickness on
the Stock panel, or the Bottom Z on the Heights tab." The rule no longer reads
that field, and on every operation this caution fires for the pin moves no
motion at all, so the clause sent the operator to a dial that cannot fix the
problem. It now reads "Check the operation's Depth field, or the stock
thickness on the Stock panel."

The per-operation forms (`boundary_2d.rs`, `drill.rs`, `engrave.rs`) and
`depth_caution_row` are untouched: they read only `message()`, `excess_mm`
and `stock_thickness_mm`, and core's struct carries all three. The deleted
`bottom_z` field is now `cut_floor_z`, and nothing outside the deleted
diagnostic builder read it.

### The evidence line, and the one label that changed

Core's push now carries
`GeometryCompare { lhs_label: "cut_floor_z", rhs_label: "stock_bottom_z" }`.
The inspector ribbon renders that arm
(`ui/properties/mod.rs:3472`), so the operator keeps the line.

The left label changed from `bottom_z` to `cut_floor_z`. `bottom_z` read as
the Heights tab's Bottom Z, which is exactly the confusion J7 removed; the
value was never that, it is the floor the operation emits. The MCP wire gains
this evidence object where it previously carried `null`.

## The behaviour changes, and there are three

### 1. The duplicate is gone

One producer, one row. This is the change that turns the gate green.

### 2. The pinned Bottom Z is no longer read — case (d)

The GUI rule took the DEEPER of two bottoms: the operation's depth dial and
the Heights tab's resolved Bottom Z. The core rule reads the depth dial
alone, because F1.19 measured that a pinned bottom reaches no emitted motion
on any operation this rule answers for.

So a 6 mm pocket with Bottom Z pinned 3 mm below an 18 mm board **stops
cautioning**. That is the intended correction. A 6 mm cut into an 18 mm board
is not a cut through the board, and the old caution described a cut the
machine does not make. The operator is not left in the dark: the Heights tab
now says the pin is not used by that operation (J7 half, `bottom_z_pin_note`).

### 3. On a PINNED config the four Z-order checks can now fire

This one follows from the snapshot, not from the depth rule, and I list it
because "two" would have been a false count. `ResolvedHeights::from_context`
projects `top_z`/`feed_z` onto the stock top, `bottom_z` onto the stock
bottom and both `retract_z`/`clearance_z` onto `safe_z`, so
`geom.bottom_above_top_z`, `geom.feed_z_below_top_z`,
`geom.retract_z_below_feed_z` and `geom.clearance_z_below_retract_z` could
never fire from it whatever the operator pinned. They can now.

The clearest case: a Bottom Z pinned ABOVE the Top Z raises
`Critical: Bottom Z (…) is above Top Z (…). No material will be cut.` on the
inspector ribbon, where the ribbon used to say nothing. That row is TRUE, and
the Heights tab badge already turns red for the same condition on the same
resolved heights (`compute_tab_badges`), so this closes a
badge-versus-ribbon disagreement rather than opening one. It is also the case
that made me ANNOTATE rather than disable the Bottom row in J7: the operator
must keep the control that clears such a pin.

For an all-Auto config nothing fires, as the table above derives.

### What did NOT change, and why I had to work for it

**A pinned TOP Z still deepens the caution.** The core rule reads
`h.top_z - depth`, and the snapshot the caller hands it decides what `top_z`
is. `ResolvedHeights::from_context` — core's own fallback "when only the
context is in hand" — projects `top_z` onto the stock top, so it drops a
pinned top. Had the GUI switched onto that snapshot, this case would have
gone silent:

> pocket 15 mm, Top Z pinned 5 mm below the stock top, 18 mm board. The
> emitted floor is 2 mm under the board. The GUI cautions today.

That is a Safety surface going quiet, and the same form already reads the
same pin for its through-cut line
(`profile_through_cut`, sentried by
`a_top_z_pinned_below_the_stock_top_counts_towards_the_through_cut_g_throughcut`);
the two lines share a boundary and must not disagree on it.

So `collect_diagnostics` now builds its snapshot with `diagnostics_heights`,
from the entry's own `HeightsConfig`. Core's `ResolvedHeights` doc asks for
exactly that: "consumers that have the resolved heights should call the
struct constructor directly."

**That snapshot also feeds the four Z-order checks, and for an `Auto` config
the delta is zero** (change 3 above is the pinned case). I derived this by reading `effective_safe_z`
(`compute/config.rs:1497`), which floors `safe_z` at `stock_top + 5`:

| Field | `from_context` | entry resolve, all Auto |
|---|---|---|
| `top_z` | `stock_top` | `stock_top` |
| `bottom_z` | `stock_bottom` | `top - op_depth` |
| `feed_z` | `stock_top` | `retract - 2 ≥ stock_top + 3` |
| `retract_z` | `safe_z` | `safe_z` |
| `clearance_z` | `safe_z` | `retract + 10` |

- `bottom_z > top_z`: false under both (`op_depth ≥ 0`).
- `feed_z < top_z`: false under both.
- `retract_z < feed_z`: false under both.
- `clearance_z < retract_z`: false under both.

For a PINNED config the four checks now read the operator's own numbers
instead of a projection. That is the same snapshot the Heights tab badge
already resolves (`compute_tab_badges`, `ui/properties/mod.rs:3204-3212`), so
the ribbon and the badge now agree where they used to be able to disagree. I
read this; I ran nothing.

## The F1.6 sentry: what I re-pinned, and what I did not touch

One test changed its expectation:

- **`d_a_bottom_z_pinned_below_the_stock_bottom_is_a_caution`** →
  **`d_a_bottom_z_pinned_below_the_stock_bottom_is_not_a_caution`**.
  - BEFORE: `rule(...)` returns `Some`, `excess_mm == 3.0`, message
    `Depth exceeds stock thickness by 3.00 mm`.
  - AFTER: `rule(...) == None`, plus a NEW assertion that the header carries
    no `geom.depth_beyond_stock` row either.
  - WHY: stated above and in the file's own header, which now carries the
    full argument so the next reader does not have to find this report.

**I removed no other assertion in that file.** Line by line, every other arm
is untouched: (a) both arms, (b), (b2), (c), (c2), (d2) and the flipped-setup
arm keep their assertions exactly as written. The only other edits are the
module doc header, the import of `GEOM_DEPTH_BEYOND_STOCK` in place of the
deleted `DEPTH_BEYOND_STOCK_ID`, and one ADDED test
(`d_the_heights_tab_says_the_pin_is_not_used_on_a_pocket`) that pins the
other half of the sentence on the surface that now carries it.

The two arms the orchestrator's gate reported red — case (a)'s
`cautions.len() == 1` and `by_id.len() == 1` — are unchanged. They are the
acceptance for this fix.

## The recurrence guard

`crates/rs_cam_viz/tests/one_depth_caution_g_depthstockgui.rs`, crate
`rs_cam_viz`, test binary `one_depth_caution_g_depthstockgui`. Added AFTER
the fix and labelled as a guard, not as red-first evidence. Three arms over a
five-case walk:

1. `no_diagnostic_id_appears_twice_on_one_toolpath` — generic. It does not
   name the depth rule, so the next surface that appends a finding core
   already produces fails here. Non-vacuity: the walk must cover every case
   and at least one case must produce a diagnostic.
2. `the_row_and_the_header_answer_the_same_way` — the Operations card row and
   the Safety header agree case by case, including the sentence. Non-vacuity
   on both sides: at least one case cautions and at least one is silent.
3. `a_top_z_pinned_below_the_stock_top_still_cautions` — the reading the
   switchover must not lose, pinned at `2.00 mm`.

RED-FIRST OUTPUT: pending verifier run

## Follow-ups — named, not done

- **The session route still builds `from_context`.**
  `crates/rs_cam_core/src/session/compute.rs:4269` hands
  `ResolvedHeights::from_context(&height_ctx)` to
  `diagnose_toolpath_inputs`, so MCP `get_toolpath_diagnostics` and the CLI
  `project` report drop a pinned Top Z where the GUI now reads it. The fix is
  one expression — resolve `tc.heights` against the context the line above
  already builds — but it is `session/**`, it changes the MCP wire, and it
  changes the inputs of four more checks on a surface I cannot exercise this
  round. GUI and MCP therefore agree on every Auto config and can differ on a
  pinned one. This gap EXISTS TODAY for all five heights checks; the
  switchover neither widens nor closes it.
- **The three pin-honouring operations still abstain.** `Adaptive3d`,
  `UnifiedFinish` and `Waterline` really can have a pinned Bottom Z as their
  floor, and the adapter snapshot carries no pin flag, so core answers for
  none of them. This is the follow-up J8.md named and I did not fold it in.
  Note the snapshot the GUI now builds DOES carry the resolved pinned bottom
  in `bottom_z` — what is missing is the `bottom_pinned` flag that would let
  the rule tell a pin from the stock bottom.
- **`gui_and_mcp_diagnostic_ids_match` is now a stale mirror.** That in-crate
  test (`operations/mod.rs`, PR-6 polish) replicates `collect_diagnostics` by
  hand with `from_context` and calls `diagnose_toolpath_inputs` directly, so
  it no longer exercises what the GUI does. It still passes — its fixture is
  an all-Auto Pocket — and I left it alone rather than widen this change.

## Not done

- No threshold moved. `DEPTH_BEYOND_STOCK_EPS_MM` is core's equality epsilon
  and the GUI's local copy was the same value; the GUI now names core's.
- No `CLAUDE.md` edit. The file carries no existing caveat paragraph on this
  caution.
- Nothing was run. No cargo command, no GUI, no MCP call. Every claim above
  is either a read of the source or the orchestrator's gate output quoted as
  theirs.
