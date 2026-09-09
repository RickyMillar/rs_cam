# Small 3D baseline — rough then finish

Date: 2026-09-09. Owner: Astra. **MCP-assisted expert walkthrough**, not a completed
R01–R09 review or a machining recipe. No code/config/library changes, builds,
tests or commits. Concurrent developers' changes remain untouched.

## Scope / fixture derivation

Started from `test_data/ux_3d_terrain.toml` (zero ops; both STL and SVG), SHA-256
`603e125b1973b75275ae0a8d8afbea1304354a3f8bcbc31716da03e02c5df11f`.
Only scratch copies were changed: renamed REVIEW ONLY, removed the unrelated SVG
and alignment pins, rebased STL path to the existing repo asset. The unscaled
seed was loaded/inspected, NOT generated. Its model is 100×73.326×52.565 mm,
40,342 triangles—“small” is relative, and this was unnecessarily large for a UX
probe alongside other developers' builds.

A second scratch seed uses the supported project `ModelUnits::Custom(0.2)` and
1 mm padding. This is fixture preparation, **not a test of a user's scale UI**.
The model is 20×14.665×10.513 mm; auto stock is 22×16.665×11.513 at (−1,−1,0).
STL source SHA-256:
`c6a00e357b3ac4c31fb5414ea4ac0ce8b4351e47dc1b5be5d7fe7b24646756c4`.

Generic Hardwood, medium workholding, Generic Wood Router. One Top setup, no
fixtures/pins. Two tools inherited from the seed: Ø6 flat (tool ID1/index0), Ø3
ball (ID2/index1). Those are fixture values, not measured real tooling. Model ID1.

Authored via explicit MCP IDs:
- op0, `Review terrain rough`, Adaptive3d: contour_parallel, max dpp4.2,
  stepover1.2, feed1000, plunge527, rpm15000, axial/radial leave0.5.
- op1, `Review terrain finish`, DropCutter: initial stepover0.075,
  feed881, plunge263, rpm19000, min_z0. Set stepover **0.3** for this bounded
  walkthrough. Recommendation warning correctly reports 4× its recommendation;
  no claim of equal delivered finish or improved cutting recipe is made.
- Both stock sources remain fresh; the sequential simulator still removes the
  rough's material before the finish. This is NOT the remaining-stock generation
  dependency/fixpoint test from R05.

Full state: `scratch/REVIEW_ONLY_20mm_terrain_authored.toml`, SHA-256
`90180b62c6d8b692aa467951e337ee54576f69d38b8ba4730cfa50aaf1c0966e`.
Build metadata still 8a4df241-dirty / 2026-09-09T08:55:51+12:00. Same 1400×900
window, no resize. Re-read build/session on the next pass: source is changing.

## Outcome / trace

| Step | Action | Result / evidence |
|---|---|---|
| 1 | Load isolated unscaled zero-op seed | Model/stock inspected; no generation |
| 2 | Load 0.2-scale zero-op seed | Auto stock follows scaled model, two tools retained |
| 3 | Add rough using tool0; finish using tool1 | IDs0/1 pending; feed-floor cautions returned |
| 4 | Inspect finish Geometry | `evidence/01_finish_defaults.png`; precise raster purpose and parameters, but long warning clipped |
| 5 | Set finish stepover0.3 | Accepted; recommendation mismatch explicitly reported |
| 6 | Generate All, timeout20s | 2 generated, 1 round, no failures or simulations, no timeout |
| 7 | Simulate at0.25mm | 3788 moves, 243.502s estimated, zero reported collisions, degraded fine-contact measurements |
| 8 | Simulation workspace, jump to end | `evidence/02_simulation_end.png`; shows Must address → Annotation15, air47%, Within2/2 |
| 9 | Save authored scratch | Source seeds not overwritten |
| 10 | Backend export, no accept flags | Scratch NC written; G21/G54, spindle15000 then M0/tool-change region/spindle19000 |
| 11 | Inspect Readiness | `evidence/03_readiness.png`; Up to date, holder Not checked, Within2/2, 4:04, one tool change |

All PNGs were read. No actual clicks, file dialogs, keyboard focus, manual
camera fitting, undo, human predictions or unaided task timings were observed.
Camera remained zoomed well out at this very small scale; a real user can use
Fit, but that action was not driven here. Do not infer that geometry is impossible
to inspect from this state-navigation screenshot.

## Selected diagnostic observations (MCP-STATE excerpts, not raw full response)

- Rough: 501 moves, cutting619.683 mm, rapid423.891 mm.
- Finish: 3287 moves, cutting1541.371 mm, rapid35.495 mm.
- Total estimated runtime243.502377 s; air46.996947% of total runtime,
  57.364861% of cutting time. Do not confuse these denominators.
- 67,097 cut samples; zero rapid/other collisions reported at cell0.25 only.
- Radial/air/chip measurements **degraded**, reason
  `cell_too_coarse_for_tip_contact`: blind_fraction0.353061 (rough), 0.368464
  (finish). Fine-contact percentages are not ground-truth metrology.
- Current triage cautions: finish crosses standing material, peak4.3744mm,
  8.3% samples above its own3×median-bite threshold; rough also receives the
  “upstream op left standing” wording, despite being the first op. These are
  diagnostic findings, not an independently established geometry defect.
- Load display Within2/2 coexists with those cautions. An efficiency air warning
  is the overview headline, not an explanation of the standing-material findings.

## New findings / handoffs

### UX-R06-003 — “Must address: Annotation 15” is an unactionable primary summary

Type guidance/IA; S2; confirmed MCP-VIEW + CODE, human comprehension untested.
`02_simulation_end.png` shows a red **Must address** section whose only row is
**Annotation 15**. It gives no issue subject, scope, corrective action or visible
jump affordance there. This is not a useful answer to “what must I address?”

Source `crates/rs_cam_viz/src/ui/sim_diagnostics.rs:550-614` collects all
`SimulationIssueKind::Annotation` counts into the must-address bucket and renders
plain label/count cells. The change that gave annotations a home prevented
silent omission but did not make that home actionable. Detailed annotation
cards elsewhere may explain individual events; their discoverability is untested.

Proposal: surface the most consequential named annotation with operation and
locate/details action, then a bounded count of remaining notes. Do not simply
suppress annotations again or relabel every annotation as a physical danger.
Acceptance: a user can identify what the15 refer to and reach the associated
motion without learning the internal word “Annotation” or reading source.
Owner R06; R09 handles shared summary-to-location patterns.

### UX-R09-001 — Caution and reach detail are cut off at the normal review width

Type presentation; S2 for lost decision context; confirmed visible clipping.
`01_finish_defaults.png`: the current tool-load caution starts
“Chipload clamped to the matched band ceiling: 0.0098 → 0.0232 mm/tooth. The whol…”
and runs off the right edge. The complete backend message explains that the band
is below the0.025 formation floor, burnishing may remain, and names alternatives.
Those are the important limits, not decorative trailing text. Reach-map detail
also extends offscreen on a single line above it. No human hover/resize test yet.

Proposal: wrap a short readable caution summary and retain expandable full
rationale; do not require widening the entire app to learn the missing guarantee.
Acceptance: at1400×900 with this selected op, a user can read the constraint,
what is not promised and the next-step options without clipping or source access.
Preserve on-demand technical evidence and the compact operation form.

### W02 guidance lead — “upstream” on the first rough

MCP triage's standing-material action for op0 says “crosses material an upstream
op left standing.” There is no upstream op in this seed. Do not infer that the
measured heavy-bite statistic is false; the causal language may simply overreach.
R06 should verify diagnostic applicability/wording before telling the operator
to fix a nonexistent predecessor. Not yet promoted to a separate UI finding:
this wording was not visible in the captured primary panel.

## Strengths / limits

- Two distinct tool roles, purpose-labelled 3D strategies and stock-aware defaults
  can be represented without manual TOML operation authoring.
- Feed-floor messages explicitly describe a missing guarantee; preserve that
  honesty while improving layout. MCP-only visibility is not sufficient.
- Generate All completed the small two-op job without failure; op timeline shows
  a clear rough/finish split and tool-change count.
- Readiness retains **Holder clearance: Not checked**, rather than equating
  simulation completion with every clearance check passing.
- Backend NC emission was exercised only in a REVIEW_ONLY_NOT_FOR_MACHINING path.
  No wizard final step, native save, setup-sheet or controller execution test.

## Follow-on

Next human/desktop tasks: locate the annotation, find and understand the complete
feed warning, inspect the model with Fit/orbit, step through wizard Preview/Save
without production output. Then extend R03/R05 to incompatible initial tools and
a remaining-stock chain. Do not tune this miniature fixture into a production
recipe or compare strategies on its degraded metrics.
