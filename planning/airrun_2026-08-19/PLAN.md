# wanaka200 from-scratch run — plan (2026-08-19)

The operator's next live session: author a NEW project from scratch over MCP
using the regenerated wanaka200 export, in white oak. This is both a real
part and the UX-journey / N-3 validation run agreed post-TD3. The July
`~/Downloads/wanaka200/wanaka200.toml` is the *reference recipe* (op shapes,
height conventions) — do NOT load or reuse it; the point is fresh authoring.
The committed `planning/airrun_2026-06-01/wanaka.toml` remains read-only and
is not part of this run.

## 1. Source files (verified 2026-08-19)

`/home/ricky/Downloads/wanaka200/rivmap_export/` — regenerated 2026-08-19
(NOT the geometry the July project referenced: land peak 7.0 mm vs 6.0,
hole grid 53 mm spacing vs 40, coastal buffer 50 vs 30):

| file | role | measured |
|---|---|---|
| `terrain.stl` | 3D model | binary STL, **661,212 triangles**, bbox (0,0,−2.814)..(200,200,7.0) — 200×200 mm, 9.814 mm total relief (waves dip to −2.81, peaks +7.0) |
| `rivers_aligned.dxf` | river polylines, mesh-aligned | back-pierce curves |
| `lakes.dxf` | lake polygons | back-pierce, `side=inside` |
| `holes.dxf` | Ø16 circles, offset grid, 53 mm pitch, ocean-only | intent to confirm (§5) |
| `machinable_edge_band.dxf` | edge band | optional boundary/keep-out |
| `rivmap_data.toml` | generator settings | provenance only |

## 2. Stock, machine, material

- **Material: `SolidWood { species: WhiteOak }`** (Janka 1360 — medium
  hardwood). This is the big delta from the July reference (Baltic birch ply):
  every feed number must come from Suggest + sim on the hardwood band, not
  from the reference file.
- **Stock**: 240 × 250 × *t* mm, origin (−20, −25, 7.0 − *t*) so **stock top
  = model top = Z +7.0** (the convention the reference uses; keeps the
  terrain fully inside stock with 20/25 mm clamp margins). *t* = actual
  dressed board thickness, measured at the machine — plan assumes **25 mm**;
  minimum workable is ~19 mm (10 mm relief + 4 mm shell + back-rough floor
  clearance). `workholding_rigidity = Medium`.
- **Machine**: Shapeoko Pro XXL profile as in the reference (max feed 10 000,
  max shank 6.35, VFD 1.5 kW, per-axis accel 500/500/270).
- **Alignment pins**: Ø6 at stock-local (2.5, 2.5) and (237.5, 247.5) —
  outside the model, diagonal, for the flip.

## 3. Op chain

### Setup 1 — back face up (`face_up = bottom`)

1. **Pin Drill** — `alignment_pin_drill`, 6 mm EM, peck, spoilboard
   penetration 2.0. Registration for the flip.
2. **Back Rough** — `adaptive3d`, 6 mm 2F carbide EM, `stock_source=fresh`,
   boundary `model_silhouette/inside`. Thin the back leaving
   **`stock_to_leave_axial = 4.0`** (the shell the pierce ops cut through).
   Feeds/stepover/DPP from Suggest on white oak — expect the axial envelope
   to clamp DPP at 4.2 (vendor_ap on the pocket-family row; same mechanism
   as the G-WANAKA-DPP baseline).
3. **Holes** — on `holes.dxf`, pending §5 intent ruling: `drill` (6 mm at
   centroids, as the reference did) or `pocket` (true Ø16 through-holes).
4. **Rivers (back)** — `project_curve` on `rivers_aligned.dxf`,
   `direction=from_below`, `surface_model = terrain`, `depth = 0.1`,
   `stock_source=from_remaining_stock`. **Tool: the 20° V-bit** (creative
   choice this run): a ~4.1 mm pierce through the shell yields a kerf ≈
   1.4 mm at the cavity side narrowing to a **hairline at the terrain
   surface** — a crisper backlit river line than the reference's 2 mm-tip
   tapered ball (which leaves a ~2 mm opening). Fallback if the sim shows
   V-bit trouble: R1.0 tapered ball as per reference.
5. **Lakes (back)** — `project_curve` on `lakes.dxf`, `from_below`,
   `side=inside`, `depth = 0.3`, R1.0 tapered ball (broader opening reads
   better for lake areas than a hairline).

### Setup 2 — front (`face_up = top`, z_rotation 90)

6. **3D Rough** — `adaptive3d`, 6 mm EM, `from_remaining_stock`,
   `stock_to_leave` radial 0.3 / axial 0.5.
7. **3D Finish** — **`unified_finish`** with the **R1.5 tapered ball**
   (creative delta from the reference's drop_cutter: exercises the newest
   finishing surface — shallow raster / mid scallop / steep waterline bands,
   `stock_to_leave` honoured in all three since 2026-08-06 — and the 41°
   edge walls land in the waterline band naturally). Stepover ~0.3.
8. **Pencil / detail** — `pencil` with **R0.5 tapered ball**, reference
   tool = the R1.5, `detector=rest_depth` — valley and river-mouth detail
   the R1.5 can't reach.

Sub-Ø2 caution (VSBS probe D): the R0.5 (Ø1 tip) and R1.0 (Ø2 tip) are in
the range where Suggest can command over the band ceiling — **let the
post-sim chipload gate arbitrate their feeds**, don't trust the pre-sim
number.

## 4. Session protocol

- Kill any stale `rs_cam_gui` instance, then `/mcp` (release binary rebuilt
  and verified 2026-08-19; `.mcp.json` launches it directly).
- Author everything over MCP from scratch: `import_model` ×4 →
  `set_stock_config` → `add_tool` (6 mm EM, R0.5/R1.0/R1.5/R2.0 tapered
  balls, 20° V-bit — specs in the July reference file are accurate) →
  `add_setup`/`set_setup_face` → `add_toolpath` per §3 → `apply_feeds`
  from Suggest → `generate_all` (fixpoint, `simulation_resolution_mm`
  required — model is 200 mm, tip radii to 0.5 mm; start 0.4, refine where
  rest ops need it) → `run_simulation` → `get_diagnostics` triage →
  optimize where offered → `export_gcode`.
- Save the authored project as `planning/airrun_2026-08-19/wanaka200.toml`
  (new file; do not touch `planning/airrun_2026-06-01/wanaka.toml`).
- UX journey notes: record every point of confusion or friction as N-3/TD4
  intake — that's half the purpose. Watch-list: heat-map legend absence,
  cursor-jump, inert aggressiveness dial, rest-ops-after-resim.
- If the operator is at the desktop: drive window states (occlude, unfocus,
  lock, monitor change) during the long generates — N-3 agenda item 1.

## 5. Open questions for the operator (ask at session start)

1. **Holes intent**: Ø16 through-holes (pocket) or 6 mm pilot drills at the
   centroids (what the July file did)? What are they physically for
   (LED mounts / hanging / decor)? Depth: through or partial?
2. **Board thickness**: actual dressed dimension of the white oak blank.
3. **Rivers tool ruling**: V-bit hairline vs tapered-ball 2 mm glow line —
   plan recommends V-bit; operator may prefer the wider line.
4. **Lakes depth**: 0.3 as reference, or deeper for stronger glow?
