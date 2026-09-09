# R04 source track — feeds recommendations, apply semantics, optimization

Source investigator report. Read-only. All paths are repo-relative. Every
line number was read in this session from the working tree on branch
`isoclip-rapid` (2026-09-09). Statements marked **HYPOTHESIS** are inferred
and were not read or executed.

Method: `Read`, `rg`, `sed`. No cargo, no GUI. The section-3 arithmetic uses
constants read from source and the numbers the live observer reported.

---

## 1. Route inventory

Every user-facing route that changes feed, plunge, RPM, stepover, DPP or
dressups on an operation.

| # | Entry point (visible label) | Surface | Trigger | Handler |
|---|---|---|---|---|
| R1 | `⚡⚡ Apply recommended speeds` | Inspector, Feeds & Speeds tab, SPEED section | click | `apply_speeds_to_op` — `crates/rs_cam_viz/src/ui/properties/mod.rs:2077-2098` |
| R2 | `⚡ Apply cut geometry` | Inspector, Feeds & Speeds tab, CUT section (only when the op has stepover or DPP) | click | `apply_cut_geometry_to_op` — `properties/mod.rs:2119-2136` |
| R3 | Per-field `⚡` pill on `Feed:` and `Plunge:` | Inspector, Feeds & Speeds tab | click | `ValueRow::suggest` writes the value directly — `properties/mod.rs:2016-2045`, `crates/rs_cam_viz/src/ui/components/value_row.rs:102-138` |
| R4 | Per-field `⚡` pill on `Stepover:` and `Depth/Pass:` (and family equivalents) | Inspector, Geometry tab | click | `dv_pill` → `ValueRow::suggest` — `properties/mod.rs:4877-4898`, `crates/rs_cam_viz/src/ui/properties/operations/boundary_2d.rs:16-17,33-51,68-69,85-100` and 20 further `dv_pill` sites in `operations/{drill,finishing,surface_3d,boundary_2d}.rs` |
| R5 | `Spindle:` override checkbox + DragValue | Inspector, Feeds & Speeds tab | edit | `PrecedenceField` — `properties/mod.rs:2058-2075`, `components/precedence.rs:62-115` |
| R6 | `Feed:` / `Plunge:` DragValue (hand edit) | Inspector, Feeds & Speeds tab; also the `SPEED — how fast (manual)` fallback when the engine refuses the pairing | drag/type | `properties/mod.rs:2027-2030,2043-2045,1681-1700` |
| R7 | `⚡ Apply all — changes the cut` | Feeds & Speeds modal, "This toolpath" tab | click | `AppEvent::ApplyFeedsAll` → `apply_feeds_all` → funnel — `crates/rs_cam_viz/src/ui/feeds_modal.rs:647-657`, `crates/rs_cam_viz/src/controller/events/mod.rs:960-971` |
| R8 | `✓ Apply explored values` (after `⊕ Start exploring`, drag on Chart C or sliders) | Feeds & Speeds modal | click | `AppEvent::ApplyFeedsExplore` → `apply_feeds_explore` → funnel with `ApplyScope::Speeds` — `feeds_modal.rs:2128,2200-2215`, `events/mod.rs:1093-1112` |
| R9 | `⚡ Apply selected — changes the cut` | Feeds & Speeds modal, "All toolpaths" tab | click | `ApplyFeedsProjectSelected` → `apply_feeds_batch` — `feeds_modal.rs:2876-2887`, `events/mod.rs:1009-1018,1040-1085` |
| R10 | `⚡⚡ Apply all toolpaths — changes the cut` | Feeds & Speeds modal, "All toolpaths" tab | click | `ApplyFeedsProject` → `apply_feeds_batch` over `enabled` toolpaths — `feeds_modal.rs:2888-2898`, `events/mod.rs:1021-1031` |
| R11 | `Apply` / `Apply ⭐` per candidate row | Optimize modal (single toolpath) | click, enabled only when `!verdict.any_exceeded()` | `ApplyOptimizeCandidate` → `apply_optimize_candidate` — `crates/rs_cam_viz/src/ui/optimize_modal.rs:854-862`, `events/mod.rs:519-652` |
| R12 | `Apply & re-optimize` under "Try this" | Optimize modal | click | `ReoptimizeWithAxisOverride` → `reoptimize_with_axis_override` — `optimize_modal.rs:964-983`, `events/mod.rs:655-760` |
| R13 | `Apply selected` | Optimize project rollup | click, enabled when any checked row has a first-safe candidate | `ApplyOptimizeProject` → `apply_optimize_project` — `crates/rs_cam_viz/src/ui/optimize_project.rs:229-232`, `events/mod.rs:1170-1245` |
| R14 | MCP `apply_feeds(index, scope)` | MCP | agent call | `mcp_apply_feeds` → `apply_feeds_recommendation` → funnel — `crates/rs_cam_viz/src/app/mcp.rs:4884-4960`, `events/mod.rs:943-949` |
| R15 | MCP `set_toolpath_param` | MCP | agent call | raw `session.set_toolpath_param` — `mcp.rs:3301-3320` |
| R16 | MCP `add_toolpath` | MCP | agent call | `suggest_params` fills the new op with a funnelled recipe and stamps provenance — `mcp.rs:3495-3547` |

Entry points to the surfaces:

- Inspector Feeds & Speeds tab: `ToolpathTab::FeedsSpeeds` — `properties/mod.rs:2987`.
- Feeds & Speeds modal: `📊 Open Feeds & Speeds modal` button on the Geometry tab — `properties/mod.rs:4660-4668`.
- Optimize modal: `Optimize this op` small buttons in the diagnostics panel — `crates/rs_cam_viz/src/ui/sim_diagnostics.rs` (~266, ~323); `Review ▸` in the project rollup — `optimize_project.rs:416-421`.
- Optimize project: the exceeding-pill click and `⚡ Optimize project for cycle time` (hover: "Search every enabled toolpath for a faster, still-safe set of feeds & speeds.") — `sim_diagnostics.rs:447-450,480-493`; Toolpath menu — `crates/rs_cam_viz/src/ui/menu_bar.rs:181`.

Routes that do **not** exist:

- No route changes **dressups** from a feeds or optimizer action. Every optimizer apply passes the existing `dressups` clone unchanged — `events/mod.rs:585,599-604,1216-1222`. No "recommended linking/dressups" button was found (`rg` for `dressup` in `feeds_modal.rs`, `optimize_*.rs`: none).
- No per-field Apply button in the modal. `ui/mod.rs:40-51` records why it was removed; `compare.rs:92-101` is read-only.

---

## 2. Action → fields changed → evidence source → invalidated work → undo matrix

### 2.1 The funnel (what "validated" means)

`apply_feeds_subset` — `crates/rs_cam_core/src/feeds/suggest.rs:856-957`:

1. Clone the op to a scratch. Write **all** of the calculator's feed, plunge, stepover, DPP (rounded) and RPM into the scratch (`:870-874,890-893`).
2. Run `enforce_invariants` on the scratch (`:925`), in this order (`:1897-1975`): axial envelope → plunge ≤ feed → stepover ≤ diameter → stepover runtime back-off → **DPP ≤ rigidity factor × D** (`:2110-2136`, roughing only) → DPP ≤ cutting length (`:2143-2160`) → DPP deflection back-off → adaptive3d entry style → plunge-entry warning → **pass 9: re-derive the feed at the final DPP/stepover** (`:2542-2700`).
3. Copy only the requested `ApplyScope` back (`:927-942`): `Speeds` = feed, plunge, RPM; `CutGeometry` = stepover, DPP; `Both`.
4. Stamp per-field provenance for the written subset from `FeedsResult::provenance()` (`:947`, `crates/rs_cam_core/src/feeds/provenance.rs:147-172,288-323`).

Consequence: a `Speeds` apply computes its feed against the **clamped scratch DPP**, not the DPP the operation keeps. Pass 9 only moves the feed when the depth-tier factor differs between the calculator's DPP and the clamped one (`suggest.rs:2583-2604`); the tier is 1.0 for any DPP ≤ 1 × D (`crates/rs_cam_core/src/feeds/geometry.rs:261-275`).

`apply` (`suggest.rs:1213-1233`) is the single controller-side write. `FeedsPreview::applicable` returns `None` when `validate_tool_for_operation` refuses, so the modal, batch, explore and MCP routes cannot write on a refused pairing (`:1043-1073`).

### 2.2 The matrix

Sources: **V** = matched vendor row (`ProvenanceSource::VendorLut` with observation id); **F** = formula fallback (`Formula`); **E** = edge-radius floor; **M** = Manual; **O** = Optimizer. Per-field labelling rule: feed and plunge inherit the chipload source; RPM, DOC, WOC are `VendorLut` only when the row publishes that column, else `Formula` — `provenance.rs:288-323`. The D^0.61 / Janka^-0.5 scaling is not a provenance kind; it is exposed only as `chipload_diameter_scale` / `chipload_hardness_scale` / `is_extrapolated` on the matched row.

| Route | Fields written | Fields NOT written | Values pass `enforce_invariants`? | Provenance stamped | Sets stale? | Undo entry? |
|---|---|---|---|---|---|---|
| R1 Apply recommended speeds | feed, plunge, RPM (RPM only if `result.rpm` finite > 0) | stepover, DPP, dressups | Yes (scratch, full point; copy speeds) | V/F/E per field, speeds only (`provenance.rs:157-163`) | Yes — `entry.stale_since = now` (`properties/mod.rs:2097`) | Panel snapshot path (see 2.3) |
| R2 Apply cut geometry | stepover, DPP | feed, plunge, RPM | Yes | V/F per field, geometry only (`:164-171`) | Yes (`:2135`) | Panel snapshot path |
| R3 Feed/Plunge `⚡` pill | that one field | everything else | **No.** Writes `round(result.feed_rate_mm_min, 1.0)` straight into the field (`value_row.rs:127-136`); no clamp, no plunge ≤ feed check | **None written by the pill.** At the flush, `detect_manual_edits` sees value moved with provenance unchanged and stamps **Manual** (`provenance.rs:190-217`, `properties/mod.rs:3446-3452`) | Yes (`edited` includes `suggested`, `value_row.rs:163`) | Panel snapshot path |
| R4 Stepover/DPP `⚡` pill (Geometry tab) | that one field | everything else | **No.** Writes `round(result.axial_depth_mm, 0.001)` — the **raw calculator DOC before the rigidity, cutting-length and deflection clamps** (`boundary_2d.rs:17,69`, `value_row.rs:134-135`) | **Manual** (same mechanism) | Yes | Panel snapshot path |
| R5 Spindle override | RPM (`Some`/`None`) | rest | n/a (operator value) | Manual at flush | Yes (`:2074`) | Panel snapshot path |
| R6 Hand edit feed/plunge | that field | rest | n/a | Manual at flush | Yes | Panel snapshot path |
| R7 Modal Apply all | feed, plunge, RPM, stepover, DPP | dressups | Yes (`ApplyScope::Both`) | V/F/E per field, all | Yes — `rt.stale_since` (`events/mod.rs:924-927`) | **No history push** |
| R8 Apply explored values | feed, RPM (operator's), plunge (clamped ≤ feed) | stepover, DPP | Clamps yes; feed/RPM **not re-solved**; chipload band dropped; pass-9 rescale withheld (`suggest.rs:1178-1189`, `:910-924`) | Stamped from `result.provenance()` → reads **V/F**, although the feed is the operator's drag (`suggest.rs:947`; the `speeds_explored` flag does not reach the stamp) | Yes | **No history push** |
| R9/R10 Modal batch | as R7 per toolpath; refused rows skipped and named in a notification (`events/mod.rs:1040-1085`) | dressups; disabled toolpaths (R10 filters `tc.enabled`, `:1027`) | Yes | as R7 | Yes per toolpath | **No history push** |
| R11 Optimizer candidate Apply | whole `OperationConfig` snapshot of the candidate (feed/RPM/stepover/DPP/scallop as searched) | dressups, face selection (cloned through, `:585-586`) | **No, by design** — candidate was scored against a simulated trace (`suggest.rs:1058-1066`) | **Optimizer** on every dimension whose value differs from baseline (`provenance.rs:224-250`, `events/mod.rs:596`) | Yes; also auto-regenerates and re-simulates when a baseline sim existed (`:628-651`) | **No history push** |
| R12 Apply & re-optimize | one axis (feed / RPM / stepover / DPP / scallop) to the suggested value | rest | Clamps only via `resolve_operation_invariants` (`suggest.rs:1248-1265`, `events/mod.rs:707-725`); clamp reported by notification (`:745-760`) | Optimizer on the changed axis (`:733`) | Yes; then re-runs the search | **No history push** |
| R13 Project Apply selected | candidate snapshot per checked `Ranked` row with a first-safe candidate (`:1191-1197`) | non-Ranked rows even if checked; dressups | No (as R11) | Optimizer | Yes per toolpath; reconciliation sim follows (`:1237-1245`) | **No history push** |
| R14 MCP apply_feeds | per declared scope | per scope | Yes | per scope | Yes via `mcp_apply_stale` (`mcp.rs:4926-4928`) | **No history push** |
| R15 MCP set_toolpath_param | one param | rest | No | **Not stamped** — no provenance write in the handler (`mcp.rs:3301-3320`; `rg feeds_provenance` in `mcp.rs` hits only `:1478,1484,3495,3547,7075`). The stored label from a prior apply stays on a value the agent overwrote. Contradicts the `Manual` doc comment "GUI DragValue or MCP/programmatic set-param" at `provenance.rs:37` | Yes | No |

Sentries: `crates/rs_cam_viz/tests/apply_contract_a3.rs`.
`per_field_apply_affordance_no_longer_exists` (`:301-329`) checks strings in
`ui/mod.rs`, `compare.rs`, `feeds_modal.rs` and the controller.
`no_apply_path_writes_the_raw_preview_value` (`:492-530`) drives
`ApplyFeedsAll` and greps the controller source. Neither test looks at
`ValueRow` or `dv_pill`, so **R3/R4 are outside the sentry net**. The fixture
fingerprint pinned at `:626` is feed 3000 / plunge 794 / RPM 18000 / WOC 2.222
/ DOC 1.27 on the default Ø6.35 2-flute end mill (DOC 1.27 = 0.20 × 6.35).

`crates/rs_cam_viz/src/ui/mod.rs:34-51`, quoted:

```text
// NOTE (A-4, Checkpoint I-1, 2026-08-12): the `FeedsField` enum and the
// `AppEvent::ApplyFeedsField` variant that used to live here are GONE, and
// their absence is a safety property, not a tidy-up. They backed six per-row
// `Apply` buttons in the Feeds & Speeds modal that wrote
// `FeedsExplain::recommended` straight into the operation — no validation of
// the tool × operation pairing, and no `enforce_invariants`, so none of the
// plunge/stepover clamps, the rigidity and cutting-length DOC clamps, the
// deflection back-off or the rounding ran. Measured on the shipped default
// Ø6.35 2-flute flat end mill in a Pocket op, the per-field DOC `Apply` wrote
// **4.445 mm** where the funnel writes **1.27 mm** (3.50×).
// Every surviving apply goes through `rs_cam_core::feeds::suggest::apply` with
// an explicit `ApplyScope`. Do not reintroduce a field-grained apply event
// without re-reading `planning/review_2026-08-08/APPLY_CONTRACT_CENSUS.md` §3.4
// — sentried by `apply_contract_a3::per_field_apply_affordance_no_longer_exists`.
```

The Geometry-tab DPP pill (R4) is the same write this note describes, on a
different widget: `dv_pill(... dpp_sugg)` with `dpp_sugg = r.axial_depth_mm`
(`boundary_2d.rs:69,94-102`) → `*self.value = round_suggestion_value(4.2, 0.001)`
(`value_row.rs:134-135`). On the observed fixture that is 4.20 mm where the
funnel writes 1.20 mm (3.5×).

### 2.3 Undo

`UndoHistory` — `crates/rs_cam_viz/src/state/history.rs:15-42,85-111`. Variants:
`StockChange`, `PostChange`, `ToolChange`, `ToolpathParamChange` (op + dressups
+ face selection), `MachineChange`. Cap 100. Menu `Undo` (Ctrl+Z) / `Redo` —
`menu_bar.rs:132-143`; handled at `events/mod.rs:228-229`.

The only writer of `ToolpathParamChange` is the properties panel:

- A snapshot of the selected toolpath is taken once, when the toolpath is
  selected and no snapshot is pending (`properties/mod.rs:406-415`).
- The snapshot is pushed as one undo entry **only when the selection moves
  away from that toolpath** (`flush_toolpath_snapshot`, `:184-199`).

Consequences:

- Every inspector action (R1–R6) on the selected toolpath is folded into one
  pending diff. Ctrl+Z while the toolpath is still selected does not undo it;
  it pops the previous entry (**HYPOTHESIS**, follows from `:184-199` and
  `:406-415`; needs a live test).
- Controller-side routes (R7–R15) push nothing. If a snapshot is pending for
  the same toolpath, the later flush captures their effect inside the panel
  diff (**HYPOTHESIS**). Otherwise they are not undoable.
- Optimizer applies also trigger regeneration and a re-simulation
  (`events/mod.rs:628-651`); an undo does not roll those back.
- Explore has `⟲ Reset to current` and `✕ Close explore` for the preview point
  (`feeds_modal.rs:2217-2242`); they act on the modal state, not on the op.
  Preview never writes until `✓ Apply explored values`.

### 2.4 Stale

Every route sets `stale_since` on the GUI runtime. The optimizer modal reads
`state.simulation.is_stale(edit_counter)` and shows
`⚠ Results stale (params changed) — re-run sim` (`optimize_modal.rs:37-51`,
`components/freshness.rs:41-47`). The MCP reply for `apply_feeds` says
"CHANGED THE CUT (DOC/WOC) — re-simulate before trusting any gate verdict"
for non-speed scopes (`mcp.rs:4947-4951`).

---

## 3. Live observation to explain

Observed on a fresh Pocket op: stored feed 750, RPM 15000, 2 flutes, DPP 1.2,
stepover 2.1. Card printed `Commanded advance/tooth: 0.0435 mm/tooth` and
`DOC: 4.20 mm`. `get_suggest_rationale` says DPP capped 4.2 → 1.2 by the
rigidity factor.

### (a) What computes `result`, and does it include the rigidity cap?

`result` is `entry.feeds_result` (`properties/mod.rs:1965-1967`), filled by
`feeds_result_for_operation` (`:1647-1657`, and again on the Geometry tab at
`:4046-4055`). That function is `validate_tool_for_operation` +
`feeds::calculate` (`suggest.rs:771-795`). **It never runs
`enforce_invariants`.** The rigidity cap lives only in the apply funnel
(`clamp_dpp_to_rigidity`, `suggest.rs:2110-2136`) and in the rationale path
(`cutter_op_profile` → `CutterOpProfile::for_combo`, `session/mod.rs:1459-1489`),
which the MCP tool and the modal's "Why these values?" read
(`mcp.rs:1504-1533`, `feeds_modal.rs:248-249,484-503`). The inspector card
has no rationale call (`rg rationale properties/mod.rs`: none).

Where 4.20 comes from: Pocket declares no DOC hint (`catalog.rs:2503-2504`),
so `calculate` takes the operation default profile: Pocket/Roughing
`ap_factor 0.70`, `ae_factor 0.35` (`feeds/mod.rs:1487-1489,2029-2032`).
4.20 = 0.70 × 6.0 and 2.1 = 0.35 × 6.0, so the tool is Ø6.0
(**HYPOTHESIS** on the diameter; consistent with both figures). The cap is
0.20 × 6.0 = 1.2 on the `Generic Wood Router` preset
(`doc_roughing_factor: 0.20`, `machine.rs:166`; the struct default is 0.25,
`:61`).

The card prints `result.axial_depth_mm` (`:2106`) with no note that Apply
will write a different number. The modal's Recommendation card prints the
same raw value in its `DOC` row (`feeds_modal.rs:565-571`).

### (b) Does `apply_cut_geometry_to_op` write 4.2 or 1.2?

**1.2.** The scratch gets 4.2 (`suggest.rs:873`), `clamp_dpp_to_rigidity`
lowers it to 1.2 (`:2127-2133`), and only the scratch's DPP is copied back
(`:938-940`). `panel_cut_geometry_apply_goes_through_the_invariant_funnel`
and the 1.27 mm fingerprint pin this on Ø6.35 (`apply_contract_a3.rs:587-610,626`).

The Geometry-tab `Depth/Pass:` `⚡` pill (R4) writes **4.2** (section 2.2).
The stored provenance then reads `✎ manual`.

### (c) Does `apply_speeds_to_op` write the feed behind 0.0435, or the floored one?

It writes `round(result.feed_rate_mm_min, 1.0)`, then whatever pass 9 does.
Pass 9 checks whether the depth-tier factor moved between the calculator's
DPP (4.2 / 6 = 0.70 → tier 1.0) and the clamped one (1.2 / 6 = 0.20 → tier
1.0) (`suggest.rs:2594-2604`, `geometry.rs:261-275`). The factors are equal,
so pass 9 returns without touching the feed. The written feed is therefore
the calculator's final feed — the one **after** its own derates and the
Step-9b rubbing-floor clamp — not `0.0435 × 15000 × 2 = 1305`.

### (d) Why 0.0435 ≠ 0.025

`result.chip_load_mm` is the **target** chipload: the LUT midpoint or the
formula value **before any derate** (`feeds/mod.rs:1989` assigns
`chip_load_mm: chip_load`; the same value is `derates.target_chip_load_mm`,
`:1973`). The quantity the row's label and hover describe — "feed ÷ (RPM ×
flutes) at the recommended feed" (`properties/mod.rs:2048-2053`) — is
`derates.effective_chip_load_mm()` = target × combined applied factor
(`feeds/mod.rs:598-627`).

Chip thinning is **not** the cause: since 2026-08-19 the radial and axial
thinning factors are computed and reported but not multiplied into the feed
(`feeds/mod.rs:1577-1631`, `:509-516`).

Arithmetic that reproduces the stored 750 (**HYPOTHESIS**, uses factors read
from source; the L/D and workholding inputs were not observed):

```text
raw_feed  = rpm × chip_load × flutes × depth_tier          (mod.rs:1634)
          = 15000 × 0.0435 × 2 × 1.0 = 1305
× L/D factor 0.88 (4 < stickout/D ≤ 6, :1638-1651)  → 1148
× workholding Low 0.85 (:1653-1661)                   → 976
× safety_factor 0.75 (Generic Wood Router, machine.rs:174; Step 9, mod.rs:1769) → 732
advance = 732 / (15000 × 2) = 0.0244 mm/tooth  < RUBBING_FLOOR 0.025 (mod.rs:858)
Step 9b clamps the feed up to the floor (mod.rs:1795): 0.025 × 30000 = 750
```

So the stored 750 / 0.025 is the calculator's own floored output, and 0.0435
is a number the recommendation never commands. If this reproduction is
right, the card also carried a `Commanded advance/tooth below rubbing
floor: 0.024 -> 0.025 mm/tooth` warning line (`properties/mod.rs:2199-2212`)
and the stored feed was written by a funnelled apply (MCP `add_toolpath`
runs `suggest_params`, `mcp.rs:3495-3547`), which is why a "fresh" op
already sat at the clamped DPP and floored feed. A live test can confirm all
of this from the modal's `How is this calculated?` drawer and Chart C.

Labelling defect, independent of the arithmetic: the inspector row labelled
`Commanded advance/tooth` (`quantities.rs:54` defines that string as
"feed ÷ (rpm · flutes)") prints the pre-derate target. The modal's
Recommendation card does the same in its `Commanded advance/tooth` row
(`feeds_modal.rs:573-580`, `explain.recommended.chip_load_mm`). The
OPERATING POINT card prints the real commanded figure from the gate,
`explain.commanded.feed_per_tooth_mm` (`properties/mod.rs:1834-1858`).
Two rows on one tab carry the same label and different quantities.

---

## 4. Provenance labelling

`crates/rs_cam_viz/src/ui/components/provenance.rs`:

- Kinds (`:32-41`): `VendorLut`, `Formula`, `EdgeRadiusFloor`, `Manual`,
  `Optimizer`, `AutoCorrect`, plus display-only `Inherited`. 1:1 with core
  `ProvenanceSource` (`feeds/provenance.rs:30-43`).
- Glyph / colour / label (`:49-87`): `▣ vendor LUT` green (`SUCCESS_BRIGHT`);
  `▲ formula fallback` and `⌊ edge-radius floor` both amber (`WARNING`, same
  colour, glyph differs); `✎ manual` muted; `◆ sim-optimized` optimizer
  blue; `⟲ auto-corrected` info; `〈〉 inherited` faint.
- `ProvenanceBadge` (`:154-165`): small text, hover `Source: <label> (<ref>)`.
  `compact()` shows the glyph only.
- There is **no kind** for "repo-derived scaling" or "clamped". A row scaled
  by D^0.61 / Janka^-0.5 still renders `▣ vendor LUT`. The scaling is visible
  only in the modal: context chip `approx ×N` when `is_extrapolated`
  (`feeds_modal.rs:307-321`) and the `How is this calculated?` drawer
  (`Scaling: diameter ×a · hardness ×b (approximate)`, `:975-996`). The
  exponents themselves are not shown anywhere in the GUI (`rg 0.61` in
  `rs_cam_viz/src/ui`: none).

Where the badge appears:

- Inspector `Feed:` / `Plunge:` rows: compact badge from the **stored**
  `feeds_provenance` (`properties/mod.rs:1988-2000,2024-2026,2040-2042`,
  `value_row.rs:140-146`). The `⚡` pill beside it is coloured by the **live
  recommendation's** source (`speed_kind` from `result.chipload_source`,
  `:2005`). Two colours on one row can therefore disagree, which is correct
  but needs the hover to decode.
- RPM, stepover, DPP have stored provenance slots (`provenance.rs:113-126`)
  but **no badge renders them**: the CUT section shows read-only
  recommended values only (`:2101-2113`), and the Geometry tab uses
  `dv_pill` without `.prov()` (`:4886-4897`).
- Modal context chip: badge for the **recommendation** (`▣` with row id, or
  `▲`), not for the stored values (`feeds_modal.rs:300-325`).

After a manual edit of an applied field: `detect_manual_edits` at the flush
(`provenance.rs:190-217`, called at `properties/mod.rs:3452`) stamps
`Manual` on any dimension whose value changed while its provenance slot did
not. This is correct for DragValue edits. It also fires for the `⚡` pill
(R3/R4), so a value the user accepted **from the recommendation** is
labelled `✎ manual`. The MCP `set_toolpath_param` path bypasses the flush
and leaves the old label in place (section 2.2, R15).

`PrecedenceField` (`components/precedence.rs:62-115`): renders
`[☑ override] [value] · default 〈 N 〉` or `〈 N 〉 RPM` with hover
"Project default — enable override to set a per-operation value."

---

## 5. Weak-evidence surfaces

`FeedsWarning::ChiploadClampedToFloor { requested, floor, band_capped_from }`:

- Inspector (`properties/mod.rs:2199-2212`): `band_capped_from: None` →
  `Commanded advance/tooth below rubbing floor: R -> F mm/tooth`;
  `Some(g)` → `Commanded advance/tooth raised to vendor band ceiling: R -> F
  mm/tooth (band is entirely below the g rubbing floor)`.
- Modal (`feeds_modal.rs:1081-1096`): same two shapes, the `Some` arm adds
  `— expect burnishing`.

`FeedsWarning::VendorRowPublishesNoChipload { observation_id, formula_chipload_mm, floor_band_from }`:

- Inspector (`:2226-2240`): `Vendor row X publishes RPM only — N mm/tooth is
  the formula's, no vendor band; rubbing floor from ROW` / `… no vendor band`.
- Modal (`:1111-1131`): longer sentences, adds "which is the chipload-bearing
  row the post-sim gate also resolves" / "and no chipload-bearing row matched
  either".

`NoVendorRowsForRoutedOperation` (`:2219-2225`, modal `:1103-1110`):
`No vendor data for OP on a FAMILY cutter — formula-derived, no band (…)`.
The modal drawer with no matched row says `No vendor LUT row matched.
Recommendation is from the empirical formula. Re-check against vendor data
before use.` (`feeds_modal.rs:1032-1043`).

Rendering: both surfaces emit each warning as **one `ui.label` of
`RichText::small()`**, prefixed `! ` (inspector, `:2242-2248`) or `⚠ `
(modal, `:1144-1150`). Neither sets a wrap mode or a max width. egui wraps a
label in a vertical layout only when the parent gives it a finite width; the
1400×900 clip the capture showed is consistent with the panel granting
unbounded width there (**HYPOTHESIS** — the enclosing `ScrollArea` kind was
not located in this pass; check the properties panel container in
`crates/rs_cam_viz/src/app/` or `ui/toolpath_panel.rs`). The `Some(global)`
arm of `ChiploadClampedToFloor` is the longest line (about 110 characters
in the inspector).

Refusal: when `validate_tool_for_operation` refuses, the inspector replaces
the whole card with `Feeds unavailable: <error>` in red plus a manual SPEED
section (`properties/mod.rs:1667-1700`); the modal keeps every chart and
replaces only the Apply column with `Cannot apply — this tool cannot run
this operation` + the error + a three-line explanation (`feeds_modal.rs:616-645`).

Gate-side weak evidence: the diagnostics panel prefixes an extrapolated-row
chipload verdict with `ADVISORY (extrapolated LUT row)`
(`sim_diagnostics.rs:1106,1132-1133`).

---

## 6. Planned vs emitted

`kinematic_utilization::FeedsProvenance::{Planned, Emitted}` reaches the GUI
in one place: the simulation operation list's kinematic pill —
`crates/rs_cam_viz/src/ui/sim_op_list.rs:953-957,972-1030`. Label `⚙ NN%`
gains the suffix ` planned` when `Planned` (`:1015-1020`); the hover ends
with `(<qualifier>)` (`:1014`). The inspector Feeds & Speeds card, the modal
and the optimizer do not show this qualifier.

MCP `narrate_toolpath` prints `planned — this surface narrates the
pre-modulation plan; the emitted reading is in get_tool_load_report after a
simulation` for the planned arm and the shared `qualifier()` for emitted
(`mcp.rs:1478-1489`).

The other `basis.qualifier()` calls (`export_wizard.rs:982`,
`readiness_panel.rs:147`, `preflight.rs:128`, `toolpath_panel.rs:517`,
`sim_timeline.rs:1125`, `sim_diagnostics.rs:376`, `setup_sheet.rs:127`)
qualify the **cycle-time basis**, a different enum (**HYPOTHESIS**: not
read; the call sites format "Estimated cycle time (…)").

The OPERATING POINT card is labelled `— measured` and shows the gate's
commanded and achieved advance per tooth, `Feed vs commanded: +N% median`,
`Modulated: a / b cuts`, `Strategy:` (`properties/mod.rs:1722-1800`). It
appears only when a load verdict carries a feed explanation or a modulation
summary (`:2155-2159`).

---

## 7. Optimizer

Search space (`crates/rs_cam_core/src/tool_load/optimize/`):

- `KnobAxis`: `Feed`, `SpindleRpm`, `Stepover`, `DepthPerPass`,
  `ScallopHeight` (`narrative.rs:217-223`).
- Stage F closed-form feed/RPM solve (headroom scale-up or per-gate
  retarget), then an axis grid over DOC × stepover × scallop anchored on
  the stage-F candidate (`optimize/mod.rs:7. Stage F` comment at ~`:289-320`;
  `strategy/grid.rs:1-14,45-80`). Candidates are simulated and scored by the
  same gates as diagnostics; ranked by measured cycle time
  (`optimize_modal.rs:735`).

Preconditions / refusals (`optimize/mod.rs:176-190,235`; text from
`crates/rs_cam_core/src/tool_load/mod.rs:160-199`):

- No baseline simulation → `Skipped(SimulationRequired)`: "no simulation has
  been run yet — Optimize needs a baseline sim to score against".
- Drill kinematics → `Skipped(SteadyStateSamplesNotPresent)`: "no steady-state
  cutting samples — typically a drill cycle or all-ramp toolpath, which
  Optimize cannot tune".
- `Material::Custom` → `Skipped(MaterialUnvalidated)`.
- Pre-flight refusals (`DeflectionSetupLocked`, `BipolarEngagement`,
  `ChiploadBandNarrowerThanHeadroom`, …) become `NoSafeImprovement` with a
  narrative.

Disabled operations: `optimize_project` walks only `tc.enabled` toolpaths
(`optimize/mod.rs:850-857`). A disabled op is **absent** from the rollup, not
listed as skipped. The rollup header adds `+ N toolpath(s) not estimated
(skipped)` for `Skipped` rows and excludes them from both totals
(`optimize_project.rs:276-296`); `baseline: current simulation` (`:299-304`).

What Apply changes: the whole candidate `OperationConfig` (section 2.2,
R11/R13). Rows are bucketed `APPLY NOW` (Ranked with a safe candidate,
checkbox), `NEEDS YOUR CALL` (TradeOff / MarginalSafe, `Review ▸` opens the
modal), `Not optimized (N)` collapsed (`optimize_project.rs:146-222`).

Labels that imply safety:

- Per-candidate `Apply` is enabled iff `!verdict.any_exceeded()`
  (`optimize_modal.rs:854`); the variable is named `safe`.
- The recommended row shows `⭐` and `Apply ⭐`.
- Entry button hover: "Search every enabled toolpath for a faster,
  still-safe set of feeds & speeds." (`sim_diagnostics.rs:487-488`).
- `MarginalSafe` header `Verify on a scrap` (`optimize_modal.rs:212`);
  `TradeOff` header `Trade-off candidates` (`:258`); `NoSafeImprovement`
  header `No improvement found` (`:140`).
- The run-provenance drawer `How these numbers were taken` (`:312-340`) adds
  the disclosure `Candidates were scored with feed modulation OFF while
  normal simulation runs it ON. "Safe" and "faster" here mean safe and
  faster in the unmodulated commanded-feed model.` when the candidate sims
  diverge from the library default (`:438-446`). Cell sizes for rank/report
  are listed (`:402-409`).
- Nothing says "generated ≠ simulated ≠ Within ≠ physically safe" in words;
  the gate vocabulary (`Within`, `Exceeds`) is the only evidence label.

Auto-verify: a single-candidate apply regenerates and re-simulates when a
baseline sim existed (`events/mod.rs:628-651`); a project apply moves the
view to `Reconciling` and then `Reconciled` with a read-only table
(`optimize_project.rs:550-609`).

---

## 8. Comparison support

- `AppEvent::DuplicateToolpath` (`ui/mod.rs:164`; `Duplicate` button,
  `toolpath_panel.rs:593-594`). Copies operation, dressups, heights,
  boundary, rest analysis, stock source, face selection, debug options and
  `feeds_provenance`; new name `<name> (copy)`; fresh runtime with no
  result (`controller/events/toolpath.rs:180-233`). This is the only
  baseline-retention facility besides undo.
- `components/compare.rs` `CompareRow` is **current vs recommended** for one
  operation (`:92-140`), not two operations.
- The optimizer keeps its own baseline as candidate 0 (`Current` card,
  `optimize_modal.rs:779-813`) and the project header shows `Current` /
  `Optimized` totals (`optimize_project.rs:250-274`). `BaselineRestoreGuard`
  is internal (`optimize/mod.rs:6.`).
- No A/B view, no side-by-side of two toolpaths' metrics, no saved
  baseline of a simulation. `rg -i "compare|baseline"` in `rs_cam_viz/src/ui`
  finds nothing else relevant.

Comparison of two feed variants therefore means: duplicate, edit, generate
both, simulate the project (one trace), and read per-toolpath rows in the
simulation panel or via MCP `get_tool_load_report` / `narrate_toolpath`.
Both duplicates cut the same stock in sequence in one simulation, so the
second one cuts air where the first already removed material
(**HYPOTHESIS**; depends on `StockSource` and ordering — a live test should
check air-cut seconds on the copy).

---

## 9. Fastest expert route, and live-test questions

### Fastest expert route worth preserving

1. Inspector → Feeds & Speeds tab → read the SPEED/CUT recommendation and
   the warning lines → `⚡⚡ Apply recommended speeds` (funnelled, speed
   only, stamps per-field provenance, keeps the cut).
2. Geometry tab → hand-type DOC/stepover (stamps `✎ manual`), or Feeds tab
   `⚡ Apply cut geometry` for the funnelled DOC.
3. `📊 Open Feeds & Speeds modal` → `How is this calculated?` (row id,
   vendor, calibrated diameter, scaling ×, band, RPM range) and `Why these
   values?` (the clamp list that Apply will perform, incl. `DPP capped to
   rigidity factor (1.20 mm)` — `rationale.rs:208-217`).
4. For an agent: `get_suggest_rationale(index)` then `apply_feeds(index,
   "speeds"|"cut_geometry"|"both")`; the reply lists the written fields.
5. Explore: `⊕ Start exploring`, drag, read the live `→ commanded
   advance/tooth … · verdict` line (`feeds_modal.rs:2153-2170`), `✓ Apply
   explored values` (clamps run, values kept).

Capabilities to preserve: speed/cut split; refusal replaces the write, the
explanation survives; explore apply keeps the operator's point; stored
per-field provenance; skipped rows named in batch notifications; the
optimizer's run-provenance drawer.

### Questions only a live test can answer

1. Does the inspector show a `Commanded advance/tooth below rubbing floor:
   0.024 -> 0.025` line on the observed fixture? (Confirms the section-3
   arithmetic and that stored 750 is a floored feed.)
2. Click `⚡` on `Depth/Pass:` on the Geometry tab: does DPP become 4.20 and
   does the row then read `✎ manual`? Does `Why these values?` in the modal
   still list the rigidity cap afterwards?
3. Click `⚡` on `Feed:` when the recommended feed is below the stored plunge
   rate: does plunge stay above feed?
4. Ctrl+Z immediately after `⚡⚡ Apply recommended speeds` with the toolpath
   still selected: what is undone?
5. Ctrl+Z after modal `⚡ Apply all` or an optimizer `Apply`: anything?
6. Does the `Some(global)` arm of the floor warning wrap or clip at 1400×900
   in the inspector, and which container is responsible?
7. On the OPERATING POINT card, do `Commanded advance/tooth` (gate) and the
   SPEED row `Commanded advance/tooth` (target) show different numbers side
   by side after a simulation?
8. `Apply explored values` then read the Feed row badge: does it say
   `▣ vendor LUT` for a dragged feed?
9. MCP `set_toolpath_param(feed_rate)` then read the inspector badge: does
   the old `▣ vendor LUT` label persist on the agent-set value?
10. Optimize project on a mixed job: is the disabled op absent from every
    bucket, and does the drill row appear under `Not optimized` with
    `skipped: no steady-state cutting samples …`?
11. Does `Apply ⭐` followed by the auto re-sim change the verdict of a
    neighbouring toolpath (the reconciliation trace is whole-project)?
