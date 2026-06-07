# rs_cam_viz IA Audit — Layer B: Diagnosis

Findings ranked **by severity (high → low), then by number of surfaces touched**, grouped by lens P1–P7. Refuted findings are excluded. Softened findings carry their `refute_note`.

---

## Executive summary — the 5 worst IA problems, in plain language

1. **Feeds & speeds has four-plus competing editing homes for one set of fields.** The same recommended feed/plunge/RPM/DOC/WOC values can be applied per-field or all-at-once from the Params tab, the Feeds-tab legacy card, the Feeds modal, and a third "Suggest all" button — all routing into the identical `apply_feeds_result_to_op` / operation setters. The code itself admits this is an unfinished Phase-4 migration that kept the legacy card alive next to the new modal. Users cannot tell which surface is canonical, and a feeds action silently rewrites cut **geometry** (DOC/WOC). *(P1-001/002/003, P2-001/002.)*

2. **Two project-level load rollups show different numbers for the same concept.** The Verdict HUD counts load verdicts *per gate-criterion* (up to 3× per toolpath) while the Inspector Project Overview counts *per toolpath* — so "exceeds 3" in one and "TPs exceeding 1" in the other are irreconcilable. The HUD tooltip even mislabels its per-criterion count as a toolpath count. The overview's own code comment acknowledges the per-gate fold bug and was migrated to fix it; the HUD was never migrated. *(P4-001/002.)*

3. **Applied feed/speed values carry zero provenance.** `OperationConfig` stores feeds as bare scalars — no field records whether a value came from vendor LUT, the sim optimizer, the nomogram what-if, or a hand edit. After apply, the origin is unrecoverable; the pill colour shown later is recomputed from a fresh LUT lookup, not from what actually produced the stored value. Worse, a single `ChiploadSource` enum is overloaded to label four independently-derived fields, so a vendor RPM can show as amber "formula fallback" and stepover/DOC pills claim vendor provenance describing the chipload lookup, not the geometry. *(P7-001/002/004.)*

4. **A safety check gives false assurance: fixture Z geometry is orphaned from collision avoidance.** Fixture height/Z position and clearance are editable and persisted, but `CollisionCheckRequest` has no fixture field at all — the holder/shank collision check only ever receives the model mesh + tool. Fixtures are consumed only as an XY footprint subtraction (ignoring `origin_z`/`size_z`). A holder crashing into a clamp will never be flagged; the Z fields just move a decorative box. **Highest user-risk finding in the set.** *(P6-003.)*

5. **Viewport visibility state has duplicate write paths with mismatched labels, and one mode is dead.** `show_stock`/`show_cutting`/`show_rapids` are writable from both Inspector › View and the Viewport overlay toolbar — same backing fields, different labels ("Show cutting moves" vs "Paths (cutting)"). Separately, `StockVizMode::ByOperation` has a live GPU render branch but no UI writer ever selects it (combobox maps it to "Solid"), and the enum has no serde derive so it can't enter via load either — dead code masquerading as a fallback. *(P4-005, P6-002.)*

### Needs live-GUI screenshot confirmation (code alone can't fully judge)

These P4/P5 findings hinge on *visual* glance-level confusability or spatial adjacency that source review can only partially settle:

- [ ] **P4-003** — does the ⚡ glyph (single-field / Suggest-all / bare pill) actually read as the same control at a glance, despite differing adjacent text/hovers? *(feeds-card, toolpath-tab-feed-params)*
- [ ] **P4-005** — confirm the relabel ("Show cutting moves" vs "Paths (cutting)") genuinely prevents users recognising the same toggle. *(Inspector › View, Viewport overlay)*
- [ ] **P4-006** — verify the two "Apply" buttons (feeds modal vs optimize modal) are never visually adjacent / confusable in practice. *(feeds-modal-toolpath, optimize-modal)*
- [ ] **P4-007** — confirm there's no visual cue distinguishing actionable vs read-only rows in the feeds-card grid. *(feeds-card)*
- [ ] **P5-001** — do the non-clickable count pills look clickable enough that users try to click them? *(Verdict HUD, Boundary timeline)*
- [ ] **P5-004** — confirm the Deviation-mode warning has no co-located re-run affordance and the user must navigate away. *(Inspector › View)*
- [ ] **P6-001** — confirm the greyed "Delete Selected" menu item reads as a real (just-disabled) control advertising a non-functional "Del". *(menu-bar)*

---

## P1 — One concern, one home (duplication)

### P1-001 · **HIGH** · feeds-card, params-tab-suggest-all, feeds-modal-toolpath
Three "apply all LUT feeds" buttons for one toolpath, all routing into `rs_cam_core::feeds::suggest::apply_feeds_result_to_op`. feeds-card "⚡ Suggest all" at `properties/mod.rs:1228-1246`; Params-tab "⚡ Suggest all (LUT)" at `properties/mod.rs:2828-2853` (comment at `:2808` says it "Mirrors the same-named button in the Feeds tab"); modal "⚡ Apply all" at `feeds_modal.rs:587-596` → `AppEvent::ApplyFeedsAll` → controller `apply_feeds_all` (`events/mod.rs:660,706`).
**Impact:** Three identical bulk-overwrite buttons across Params, Feeds-tab card, and the modal — user cannot tell whether they differ, eroding trust that any one is authoritative.

### P1-002 · **HIGH** · feeds-card, params-tab-feed-params, feeds-modal-toolpath
Per-field feed/plunge/DOC/WOC/RPM suggestion has three homes, all writing the same operation setters. feeds-card per-field "⚡ Suggest" at `properties/mod.rs:1149-1189`; Params-tab inline ⚡ pill via `dv_pill` (`properties/mod.rs:3365`, used across `operations/*`) plus `draw_spindle_rpm_row` pill (`operations/mod.rs`); modal per-row Apply via `AppEvent::ApplyFeedsField` (`feeds_modal.rs:618-628,700-714` → `events/mod.rs:620-655`, identical `set_*` setters).
**Impact:** Same recommended values applied per-field from three surfaces with different glyphs — no clear source of truth.

### P1-003 · **HIGH** · feeds-card, feeds-tab, feeds-modal-launcher, feeds-modal-toolpath
The Feeds tab (`properties/mod.rs:3006`) hosts BOTH the modal-launch button (`:3018-3028`) AND the full legacy feeds card (`draw_feeds_card`, `:1120-1246`). Comment at `:3013-3016`: the modal "Stays alongside the legacy feeds card so existing muscle memory keeps working until the modal is fully promoted (Phase 4)." `feeds_modal.rs` (3174 lines) reimplements the same comparison/power/MRR rows.
**Impact:** Two full feeds/speeds editors ship side by side in the same tab — double the surface to learn, two places values can drift.

### P1-006 · **LOW** · vendor-lut-viewer, feeds-modal-toolpath
Embedded vendor rows render in two surfaces from the same `embedded_vendor_lut`. Feeds-tab viewer at `properties/mod.rs:1321+`; modal at `feeds_modal.rs:295,1011-1014`.
**Impact:** Same vendor evidence table in two places, no single canonical home. (Sub-symptom of the P1-003 Phase-4 migration.)

### P1-004 · **LOW** *(softened med→low)* · post-processor-panel, export-wizard, mcp-read-path
post-processor-panel mutates `gui.post.format` directly with no inline `session.set_post_config` (`post.rs:13-20`); the wizard's `WizardSetPost` writes both (`app/input.rs:241-253`). `gui.post` is reconciled into the session at IO boundaries (`controller/io.rs:153-155,173`), and GUI export reads `gui.post.format` directly (`io/export.rs:139,178,235,290`).
**Impact:** Panel edit leaves `session.post_config()` stale until next save; only MCP (which reads the session directly) can briefly disagree. GUI export is unaffected.
**Refute note:** Core claim (no inline sync) true, but the wrong-export implication is mostly wrong — GUI export uses `gui.post` and save reconciles the session. Only genuine staleness window is GUI-vs-MCP between an unsaved edit and the next save. Narrow; lowered med→low.

### P1-005 · **LOW** *(softened med→low)* · post-processor-panel, params-tab-spindle-override, feeds-modal-toolpath
Two RPM fields: post panel "Spindle Speed:" → `post.spindle_speed` (project default, `post.rs:23-29`); per-op "Spindle RPM:" override → `operation.spindle_rpm` via `draw_spindle_rpm_row` (`operations/mod.rs:68+`). Precedence IS documented on the per-op side (hover + "(uses project default)" hint).
**Impact:** Near-identical labels in separate panels; per-op documents precedence but the post panel has no cross-reference to the override.
**Refute note:** The "neither states which wins" claim is false — the per-op side documents it. Residual is one-directional (post panel silent on override) + label similarity. Lowered med→low.

### P1-007 · **LOW** *(softened)* · params-tab-scallop-op, feeds-modal-dropcutter
Two scallop-height inputs drive **different** op types: Scallop op's `ScallopConfig` geometry param (`operations/surface_3d.rs:381-388`) vs DropCutter override `SetDropCutterScallopHeight` (`feeds_modal.rs:728-755`, ball-tip-gated).
**Impact:** "Scallop height" as a term scatters across two op families.
**Refute note:** Largely refuted as true duplication — these are different ops/different events; over-merging would be wrong IA. Kept as a soft conceptual-scatter low.

---

## P2 — One home (scatter of one concern across panels)

### P2-001 · **HIGH** · Params Tab, toolpath-tab-feed-params, feeds-card, feeds-tab, feeds-modal-toolpath, feeds-modal-project, toolpath-tab-suggest-all
Four+ feeds-editing surfaces over one `OperationConfig`: (1) Params editable fields via `draw_feed_params` (`operations/mod.rs:43-68`); (2) Feeds-tab legacy card per-field Suggest (`properties/mod.rs:1154,1165,1176,1187`) + Suggest-all (`:1236`); (3) modal launcher (`:3017-3029`); (4) the modal itself (`feeds_modal.rs:596,627` → `events/mod.rs:646-649,706`). Code admits the overlap (`properties/mod.rs:3013-3016`, Phase 4).
**Impact:** Feeds has four-plus editing homes — edits in one place look stale/contradicted in another.
**Refute note:** Stands. All surfaces write the same `OperationConfig` (no divergent state) — mild mitigation only; multiplicity of affordances is the real problem.

### P2-002 · **HIGH** · Params Tab, feeds-card, feeds-modal-toolpath, optimize-modal
feeds-card has DOC/WOC "Suggest" buttons writing `set_depth_per_pass`/`set_stepover` (`properties/mod.rs:1176,1187`); shared `apply_feeds_result_to_op` writes `set_stepover`/`set_depth_per_pass` (`core feeds/suggest.rs:695-696`). Suggest-all and modal `ApplyFeedsAll` both route through it.
**Impact:** DOC/WOC (cut-geometry, set in Params) are overwritten from feeds surfaces — a feeds Suggest silently changes cut geometry, blurring "how deep/wide" with "how fast".
**Refute note:** Stands. Verified at core level — `apply_feeds_result_to_op` unconditionally rewrites stepover + depth-per-pass. No role-separation defense.

### P2-004 · **HIGH** · Heights Tab, Params Tab, post-processor-panel, export-wizard
Z clearance/retract scattered over four surfaces with overlapping names: Heights owns 5 Z planes (`operations/mod.rs:244-293`); drill Params has its own "Retract Z" R-plane (`drill.rs:50,131`, apology tooltip `properties/mod.rs:3461`); Post has "Safe Z" (apology tooltip `post.rs:32-33`); export wizard adds a per-export safe-Z override (`export_wizard.rs:397-417`).
**Impact:** Users can't tell which plane governs a given rapid; two surfaces ship apology-text to compensate for the split.
**Refute note:** Stands. Drill `retract_z` (G-code R-plane) and the export override are legitimately different concepts, but they reuse the same labels ("Retract Z", "Safe Z") across four panels. The two apology tooltips are direct evidence the split is confusing.

### P2-003 · **HIGH** · Dressups Tab, Params Tab
Two entry-style state fields for adaptive3d: `Adaptive3dConfig`'s own Plunge/Helix/Ramp in Params (`surface_3d.rs:120-168`) vs the generic `DressupConfig` entry-style in Dressups (`properties/mod.rs:3555-3608`). The Dressups combo is greyed only when `strip_all_reason` is `Some`; adaptive3d's `DressupPolicy::FORCE_NO_ENTRY` sets `strip_all_reason:None` but `entry:ForceNone` (`catalog.rs:961-967,1548`) — so the Dressups entry-style **stays editable but compute coerces it to None**.
**Impact:** "How the tool enters material" is split; adaptive3d's Dressups entry-style is editable but silently inert (a live no-op control).
**Refute note:** Stands, stronger than stated — the no-op-duplicate is real because grey-out is gated on `strip_all_reason` (None for adaptive3d), so the combo is enabled while functionally inert.

### P2-007 · **LOW** · post-processor-panel, Params Tab, Feeds Tab
`post.high_feedrate`/`high_feedrate_mode` live in Post behind the "Safe Rapids (G0→G1)" checkbox (`post.rs:72-92`), separate from per-op feeds.
**Impact:** A "feed rate" value sits in the post panel, a fourth location for the "feed" vocabulary.
**Refute note:** Stands at low. High-feedrate is a rapid-replacement post concern (defensibly in Post), but reuses "feed" vocabulary away from op feeds.

### P2-005 · **LOW** *(softened high→low)* · setup-properties-panel, alignment-pins-section, stock-properties-panel
`AppEvent::SetupTwoSided` fires from both `setup.rs:100` and `stock.rs:188`; `flip_axis`/`alignment_pins` editable only in `stock.rs:185-365`. Setup only reads pin state and offers a convenience trigger (`setup.rs:230` documents "pins are now defined on the stock").
**Impact:** Two-sided workflow triggerable from Setup or Stock, but edited only under Stock.
**Refute note:** Deliberate stock-level consolidation; both triggers fire the identical event, editing lives in exactly one place. Only residue is a dual trigger affordance. Lowered high→low.

### P2-006 · **LOW** *(softened high→low)* · post-processor-panel, Params Tab, Feeds Tab, feeds-modal-toolpath
Project-default spindle (`post.spindle_speed`, `post.rs:23-29`) + per-op override `Option<u32>` (`draw_spindle_rpm_row`, `operations/mod.rs:80-145`); tooltip documents precedence, "(uses project default)" hint when off.
**Impact:** User could miss that the override takes precedence over the Post default.
**Refute note:** Textbook default + opt-in-override hierarchy; the disambiguation the lens says is missing is present in the UI. Lowered high→low.

---

## P3 — Dig deeper, don't dump (flatness)

### P3-002 · **MED** · Params Tab
The Params dispatch (`mod.rs:2897-2941`) calls ~24 `draw_*_params`, each a single flat grid interleaving geometry, `draw_feed_params`, and general fields with no internal grouping (`draw_pocket_params boundary_2d.rs:78-135`, `draw_inlay_params :444-505`, `draw_zigzag_params :515-547`). Contrast: `draw_dressup_params` has named sections ("Entry & Exit"/"Path Quality"/"Optimization"/"Safety" at `mod.rs:3548/3646/3693/3783`), `draw_profile_params` nests collapsing "Tabs" (`boundary_2d.rs:200`), `tool.rs:205` nests "Holder / Shank".
**Impact:** Params reads as a dump with no summary→detail progression, breaking the visual grammar the same codebase uses elsewhere.
**Refute note:** Stands. Internal-inconsistency is the strongest part — the codebase demonstrably knows how to group and chooses not to here.

### P3-001 · **MED** *(softened high→med)* · Params Tab (draw_adaptive3d_params), toolpath-tab-feed-params
`draw_adaptive3d_params` (`surface_3d.rs:59-270`) renders one `Grid::new("a3d_p")` with ~20 controls and no `CollapsingHeader`/section labels.
**Impact:** Large single grid of geometry/feeds/entry/strategy/finishing with no labels; user must scan the whole grid.
**Refute note:** Flatness holds, but not a pure dump — lines `138-168`/`236-269` conditionally reveal sub-fields (a form of disclosure the finding understates), and this is the most advanced op where most params are legitimately in-scope. Lowered high→med.

### P3-004 · **LOW** · setup-properties-panel
Five sub-concerns (Orientation `setup.rs:53`, Datum/Alignment `:107`, Models `:247`, Fixtures `:295`, Keep-Out `:327`) as one always-expanded stack delimited only by bold `RichText`; no `CollapsingHeader`.
**Impact:** Editing one aspect forces scrolling past every other always-open section.
**Refute note:** Stands at low. Some conditional content exists; labeled sections already give grouping; setup panels are short and infrequently edited.

### P3-005 · **LOW** *(softened, near-refuted)* · Feeds Tab
After the modal-launch button, the Feeds tab unconditionally renders a 4-5 line formula breakdown at 9.5pt then the engagement diagram (`mod.rs:3046-3097`). The Feeds card (`:1128`) and vendor viewer (`:1392`) ARE behind collapsing headers.
**Impact:** Formula breakdown is a fixed always-visible block rather than an expand-on-demand summary.
**Refute note:** Comment at `:3047` reads "Formula breakdown — always visible, the key teaching tool"; only 4-5 short lines, deliberately compact, and the detailed parts ARE collapsed. Defensible style preference, not a real IA failure. Kept at low only because a one-line collapsible summary would still help.

### P3-003 · **LOW** *(softened med→low)* · Params Tab (inlay)
`inlay_p` grid lays out the five inlay-fit params then `draw_feed_params` + Tolerance in one flat grid, no label (`boundary_2d.rs:444-505`).
**Impact:** Fit controls not visually separated from boilerplate cutting params (though positionally contiguous at top).
**Refute note:** Specific instance of P3-002 (partial double-count); the inlay geometry fields ARE positionally clustered at the top, so impact is "no label to confirm grouping". Lowered med→low.

---

## P4 — No confusable controls (same look, different role / same role, different look)

### P4-001 · **HIGH** · Verdict HUD, Inspector › Project overview
`sim_timeline.rs:168-190 verdict_counts()` iterates per-toolpath × 3 gates, incrementing per-criterion (`:174-189`); pills render "load/unmodeled/exceeds" (`:117-134`). `sim_diagnostics.rs:507-514` uses `load_report.summary()` which folds per-toolpath (`verdict.rs:355-405`), labeled "TPs within/exceeding/fully unmodeled" (`:619-637`).
**Impact:** Same load concept shows different numbers in two project rollups — "exceeds 3" (HUD) vs "TPs exceeding 1" (overview) can't be reconciled.
**Refute note:** Stands. The overview comment (`sim_diagnostics.rs:499-502`) explicitly acknowledges the per-gate fold problem and switched the overview to `summary()` to fix it — the HUD was never migrated.

### P4-002 · **HIGH** · Verdict HUD
`sim_timeline.rs:119-121` pill `"✓ load {ok}"` with hover "Toolpaths within modeled load limits." but `{ok}` comes from `verdict_counts` (`:174-184`) which increments once per (toolpath,gate) pair across 3 gates — a per-criterion count, not a toolpath count.
**Impact:** The tooltip claims a toolpath count while the value is a per-criterion count — actively mislabeling a safety-relevant load metric.
**Refute note:** Stands. Distinct self-contained mislabel (subset of P4-001's cause). Actively incorrect label on a safety metric → high.

### P4-003 · **MED** · feeds-card, toolpath-tab-feed-params
Same `\u{26A1}` glyph means "suggest this one field" (`properties/mod.rs:1150,1161,1172,1183`), "suggest all" (`:1229,:2829`), and the bare inline single-field pill (`suggest_pill :3342`, single-field overwrite `:3396`) — distinguished only by adjacent text/hover.
**Impact:** Same lightning glyph carries three scopes; user can overwrite all feeds when they meant one (or vice-versa).
**Refute note:** Glyph overload confirmed. Mitigations exist (text "Suggest all"/"Suggest all (LUT)", hovers, "overwrite this field only"), so scope is recoverable from text — but the icon itself carries no scope distinction. Destructive-but-reversible → med.

### P4-005 · **MED** · Inspector › View, Viewport overlay toolbar
Inspector View writes `viewport.show_stock/show_cutting/show_rapids` labeled "Show stock"/"Show cutting moves"/"Show rapid moves" (`sim_diagnostics.rs:58,76,78`); the Viewport overlay "Show ▼" menu writes the SAME fields labeled "Stock"/"Paths (cutting)"/"Rapids" (`viewport_overlay.rs:108,112,113`).
**Impact:** Same backing fields, different labels in two places — user may not recognise it as the control they already changed.
**Refute note:** Stands/strengthened. Same-state two-labels case (inverse of P4-004). Toggling one updates the other, but differing labels mask that.

### P4-006 · **LOW** *(softened)* · feeds-modal-toolpath, optimize-modal
feeds-modal "Apply" (`feeds_modal.rs:622-627`, vendor-LUT source) and optimize-modal "Apply"/"Apply ⭐" (`optimize_modal.rs:508-515` → `apply_optimize_candidate`, sim source) overwrite overlapping feed/stepover/doc/rpm fields from two different engines.
**Impact:** Two "Apply" buttons in different modals accept different engines' numbers; the affordance alone doesn't say which.
**Refute note:** Modals disambiguated at container level (distinct titles, separate windows, never side-by-side; optimize adds ⭐ and disables on exceed). Engine recoverable from the opened window. Residual: bare word "Apply" reused + overlapping fields. Lowered to low.

### P4-007 · **LOW** · feeds-card
In the `feeds_card` grid, RPM/Chip Load/Power/MRR rows emit `ui.label("")` in the action column (`properties/mod.rs:1139-1146,1193-1222`) while Feed/Plunge/DOC/WOC rows emit a "⚡ Suggest" button (`:1147-1190`) — all in the same 3-column layout with no cue distinguishing them.
**Impact:** Some "label: value" rows are actionable and some read-only with no visual cue; user can't tell at a glance which can be applied.
**Refute note:** Stands at low. Discoverability nit; the actionable rows ARE the legitimately-tunable ones (RPM/chipload/power/MRR are derived).

### P4-004 · **LOW** *(softened)* · Inspector › View, Operations Queue Panel
Inspector View checkbox "Show cutting moves" → `viewport.show_cutting` (global, `sim_diagnostics.rs:76`); toolpath-row "C" button → `entry.show_cutting` (per-toolpath, `toolpath_row_controls.rs:39-60`); same for rapids.
**Impact:** Two cut/rapid visibility toggles at different scopes on the same lines; toggling one and seeing no full effect (because the other scope still gates) is confusing.
**Refute note:** Partially refuted on "near-identically-named" — labeled checkbox vs single-letter "C" button, hovers explicitly scope ("in the 3D viewport" vs "for this toolpath"). Closer to a legitimate global/per-item hierarchy; residual is layering-semantics surprise, not name/role confusability. Lowered to low.

---

## P5 — UI wins, not prose (text-crutch)

### P5-001 · **MED** *(softened high→med)* · Verdict HUD, Boundary timeline
The exceeds/collisions count pills are plain `ui.label` with hover only — no `Sense::click`, no events (`sim_timeline.rs:133,150,193-196`). The HUD (`@83`) and timeline (`@924`) are separate widgets; the redirect target (red timeline lines) IS clickable and seeks via `nearest_safety_marker_move` + `SimJumpToMove` (`:1100-1113`).
**Impact:** Count pills look clickable but do nothing; a tooltip sentence redirects the user to hunt for thin red lines on a separate widget.
**Refute note:** Core claim (non-interactive pills) stands. Lowered high→med because the redirect target works — user can act, just via a worse affordance than a clickable pill.

### P5-004 · **LOW** · Inspector › View
When `stock_viz_mode == Deviation` and `display_deviations` is `None`, a plain WARNING `ui.label` "No deviation data — re-run simulation to compute" renders with no button (`sim_diagnostics.rs:125-133`); the mode selector that triggered the requirement is right there (`:106-113`); Run lives on a different panel (`sim_op_list.rs`).
**Impact:** Selecting Deviation produces a prose warning with no inline re-run affordance, forcing the user off-surface.
**Refute note:** Stands at low. Genuine point-of-need gap (unlike P5-003 where the action was on the same panel); consequence mild (one navigation).

### P5-003 · **LOW** *(softened med→low)* · Verification empty-state card, Setup & run
The ready-to-sim empty-state card's only content is "Use Run Simulation above…" (`sim_op_list.rs:146`); the sibling no-toolpaths branch gives a real "Go to Toolpaths" button (`:157-169`). But a prominent full-width "Run Simulation" button sits one separator above on the SAME panel (`:107-116`).
**Impact:** The card is prose-only while the sibling branch has a button.
**Refute note:** The "above" is literal and accurate — the primary CTA is one of the most prominent controls on the panel. The sibling needs its own button because that action isn't available above; duplicating Run inside the card would be redundant. Minor polish. Lowered med→low.

---

## P6 — Every control earns its place (orphans / dead controls)

### P6-003 · **MED (treat as floor)** · fixture-properties-panel, Export Readiness modal
Fixture Z extent + clearance are editable and persisted (`setup.rs:425 origin_z`, `:467 size_z`, clearance) but the holder/shank collision check never receives fixtures: `CollisionCheckRequest` has exactly `{toolpath, tool, mesh}` (`collision_check.rs:13-17`); `run_collision_check` uses mesh + tool only (`:55-70`); both call sites pass only model mesh (`session/compute.rs:1659-1664`, viz `helpers.rs:99-106`). Fixtures consumed only as XY footprint subtraction via `Fixture::footprint()` which ignores `origin_z`/`size_z` (`session/mod.rs:357-363`).
**Impact:** A holder crashing into a clamp is never flagged; the Z fields only move a decorative box — a safety check giving false assurance.
**Refute note:** Stands/strengthened. No missed code path could feed fixtures into the holder check. **Highest user-risk finding; med should be a floor, not a ceiling.**

### P6-001 · **MED** · menu-bar (Edit menu)
`menu_bar.rs:144-147` hardcodes `add_enabled(false, Button::new("Delete Selected").shortcut_text("Del"))` — emits no event, unconditionally disabled (literal `false`, no enable logic). The real Delete path is separate at `app/input.rs:440-446` (pushes `RemoveToolpath` when a Toolpath is selected).
**Impact:** Edit › Delete Selected is permanently greyed and advertises a "Del" shortcut it never performs — a dead, misadvertised control.
**Refute note:** Stands. Capability reachable via keyboard, so impact is bounded to discoverability/false-affordance → med.

### P6-002 · **MED** · Inspector › View
`StockVizMode::ByOperation` is defined (`state/simulation.rs:380`) with a live render branch (`app/gpu_upload.rs:83 → operation_placeholder_colors`), but the combobox never offers it: `selected_text` maps it to "Solid" (`sim_diagnostics.rs:100`, "// placeholder: treated as Solid") and the `selectable_value` list (`:104-119`) only has Solid/Deviation/ByHeight. The enum has no serde derive (`simulation.rs:373`) so it can't enter via load either.
**Impact:** A whole stock-coloring mode and its GPU path can never be selected — dead code masquerading as a Solid fallback.
**Refute note:** Stands/strengthened. Repo-wide `rg ByOperation` returns exactly three hits (def, GPU arm, placeholder) — no writer ever assigns it. Default is Solid. Genuinely unreachable.

### P6-004 · **LOW** · Verdict HUD
`draw_verdict_hud` takes `_events: &mut Vec<AppEvent>` (underscore-prefixed, unused, `sim_timeline.rs:89`); all pills are `info_pill` with no click handler (`:116-164`); the mut sink is threaded from the caller (`:69`) but never written.
**Impact:** The HUD is plumbed as if it could emit actions but is purely read-only — dead plumbing.
**Refute note:** Stands. Tooltips invite clicking the separate boundary timeline, so this is a vestigial parameter, not a broken on-HUD affordance. Low.

---

## P7 — Provenance is legible (recommendation attribution)

### P7-001 · **HIGH** · toolpath-tab-feed-params, feeds-card, feeds-modal-toolpath, optimize-modal, feeds-modal-nomogram-explore
`set_feed_rate` (`operation_configs.rs:940`), `set_plunge_rate:946`, `set_stepover:952`, `set_depth_per_pass:958`, `set_spindle_rpm:967` all write bare `f64`/`u32` — no per-field source/provenance field anywhere in `OperationConfig`. The modal reads them back into `CurrentValues` (`feeds_modal.rs:365,401-424`) which has no source field.
**Impact:** After apply — from LUT pill, sim optimizer, nomogram what-if, or hand-typing — the stored value records nothing about which system produced it; origin is unrecoverable.
**Refute note:** Stands. Any later pill colour derives from a recomputed `FeedsResult.chipload_source`, not from stored provenance — once in the config, origin is gone.

### P7-002 · **HIGH** · toolpath-tab-feed-params, Params Tab
RPM pill passes `&result.chipload_source` (`operations/mod.rs:132`); stepover/DOC pills pair `r.radial_width_mm`/`r.axial_depth_mm` with `&r.chipload_source` (`surface_3d.rs:16,57,58`; `boundary_2d.rs:17,18`). `pill_color_for_source` keys solely off `ChiploadSource` (`properties/mod.rs:3311-3328`). But `feeds/mod.rs:882-899` keeps `result.rpm_nominal` while setting `FormulaFallback` for a vendor row with `chip_load_mm==0`, and `:925` overrides RPM from the vendor row regardless of chipload source.
**Impact:** A vendor-LUT RPM is shown amber "formula fallback"; stepover/DOC pills claim "vendor LUT (obs_id)" describing the chipload lookup, not the geometry they label — mis-attributed provenance.
**Refute note:** Stands. A single `ChiploadSource` enum is overloaded as the provenance label for four independently-derived fields.

### P7-004 · **MED** · optimize-modal, feeds-modal-nomogram-explore, toolpath-tab-feed-params, feeds-card
`apply_feeds_explore` (`events/mod.rs:793-818`) sets feed+rpm only — no source; `apply_optimize_candidate` (`:442-531`) replaces the whole `OperationConfig` via `apply_toolpath_param_snapshot` — no source; `apply_feeds_all` routes through `apply_feeds_result_to_op` (`:706`). All converge on source-less storage; the later pill derives only from a recomputed `chipload_source`.
**Impact:** Three engines (vendor LUT, sim optimizer, manual nomogram) deposit values that afterward look identical and carry the LUT-flavoured pill — a sim-validated value can't be told from a raw LUT suggestion.
**Refute note:** Stands at med. Corollary of P7-001 from the apply side; engines genuinely differ in trust level yet render identically.

### P7-003 · **MED** · toolpath-tab-feed-params, feeds-card, feeds-modal-toolpath
Vendor-LUT state painted three colours: inline pill `rgb(80,180,80)` (`properties/mod.rs:3314`), card "Source:" line `rgb(100,160,200)` blue (`:1254`), modal `theme::SUCCESS rgb(100,180,100)` (`feeds_modal.rs:294,1037`). Formula fallback: pill/card `220,180,60` match, modal `theme::WARNING_MILD 200,170,60` (`feeds_modal.rs:302,310,1253`).
**Impact:** The same vendor-LUT signal renders as three colours and formula-fallback as two ambers depending on surface — the colour can't be learned as a reliable cue.
**Refute note:** Stands/strengthened. The two greens are genuinely different RGB (not "two greens that match"). Inconsistency/learnability, not data-correctness → med.

### P7-005 · **LOW** *(softened)* · feeds-modal-toolpath, feeds-card, toolpath-tab-feed-params
Full provenance lives in the modal: toggle-gated "How is this calculated?" disclosure (`feeds_modal.rs:986-1080`, collapsed by default) and always-open "Why is the recommendation here?" (`:1208-1262`). Inline pills expose provenance only via hover (`properties/mod.rs:3343-3357`); legacy card shows a one-line "Source: {observation_id}" (`:1252`).
**Impact:** Editing feeds on the Toolpath/Feeds tab gives only a hover or one-line source; to see vendor-row-vs-formula the user must open the modal.
**Refute note:** Evidence citation corrected (`:1214-1259` is the always-open block; the toggle-gated detail is `:986-1080`). Kept at low — modal is reachable from the editing surface and the one-line source + hover give a usable shallow cue. Depth/locality gap, not hidden provenance.
