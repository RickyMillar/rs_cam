# R03 — shallow operation census

Source: `crates/rs_cam_core/src/compute/catalog.rs` registry entries
(labels/descriptions at lines 1916–2404, `ALL_2D` 259–271, `ALL_3D` 273–286,
`ToolConstraints` policy test 2725–2760) and the GUI forms under
`crates/rs_cam_viz/src/ui/properties/operations/`. Evidence label: **CODE**,
except the rows marked LIVE, which were added and generated in this review.
Menu availability is decided by `GeometryRequirement` alone
(`toolpath_panel.rs:671-710`): a greyed entry's hover says the description plus
"Requires 2D geometry (SVG/DXF)" / "Requires 3D mesh (STL/STEP)" / "Requires
both 2D curves and 3D mesh". The menu does not look at the tools.

Tool precondition column: **any** = no restriction (19 ops); **V-bit** =
validator blocks Generate after add ("… requires a V-Bit tool");
**ball/tapered** = Suggest refuses a Flat or V-bit tool at add time, op never
created; a **Bull Nose passes the add door** (`feeds/mod.rs:822-841` accepts
`Bull`) and is refused only at Generate (`execute.rs:2146-2153`), with the
Generate button still enabled because the static validator has no
Scallop/Unified arm. Verified 2026-09-09 by the second-run source agent
(`../W03/support/r03_census_verification.md` §1) against every registry
entry: **no correction to any row**. Two refinements: 3D Finish is `any` at
add time but its Feeds tab reads "Feeds unavailable: scallop requires curved
tip …" once a scallop height is set with a flat tool (`feeds/mod.rs:823-824`,
`properties/mod.rs:1668-1677`); Rest's earlier op must be enabled, in the
same setup, on the same model, with the previous tool, and a fresh Rest op
always starts blocked with "Previous tool not selected"
(`operations/mod.rs:1879, 1976`).

## 2.5D menu — header "2.5D (from SVG)"

| Label (enum) | Description shown on hover | Needs | Tool | Pass role | Form | Live |
|---|---|---|---|---|---|---|
| Face (`Face`) | Level the stock top surface | Stock | any | rough | `boundary_2d.rs:11` | — |
| Pocket (`Pocket`) | Clear material inside a closed region | Polygons | any | rough | `boundary_2d.rs:63` | LIVE F1, F4 |
| Profile (`Profile`) | Cut along the outside or inside of a boundary | Polygons | any | rough | `boundary_2d.rs:123` | LIVE F1 |
| Adaptive (`Adaptive`) | Constant-engagement rough clearing | Polygons | any | rough | `boundary_2d.rs:219` | — |
| VCarve (`VCarve`) | V-bit engraving with variable depth | Polygons | V-bit | finish | `boundary_2d.rs:298` | LIVE F2 (both arms) |
| Rest Machining (`Rest`) | Clean up areas a larger tool couldn't reach | Polygons | any + previous tool larger + earlier op | rough | `boundary_2d.rs:340` | — |
| Inlay (`Inlay`) | V-bit pocket and plug for inlay work | Polygons | V-bit | finish | `boundary_2d.rs:392` | — |
| Zigzag (`Zigzag`) | Back-and-forth raster clearing at an angle | Polygons | any | rough | `boundary_2d.rs:458` | — |
| Trace (`Trace`) | Follow a path exactly for engraving or scoring | Polygons | any | finish | `engrave.rs:7` | LIVE F2 |
| Drill (`Drill`) | Drill holes from SVG circle positions | Polygons | any | rough | `drill.rs:89` | LIVE F2 (centroid fallback) |
| Chamfer (`Chamfer`) | Bevel edges with a V-bit | Polygons | V-bit | finish | `engrave.rs:44` | — |

## 3D menu — header "3D (from STL)"

| Label (enum) | Description shown on hover | Needs | Tool | Pass role | Form | Live |
|---|---|---|---|---|---|---|
| 3D Finish (`DropCutter`) | Parallel raster passes following the surface | Mesh | any | finish | `surface_3d.rs:18` | — (W02 used it) |
| 3D Rough (`Adaptive3d`) | Load-limiting rough mill on a 3D surface | Mesh | any | rough | `surface_3d.rs:50` | LIVE F3 |
| Waterline (`Waterline`) | Horizontal contours at constant Z levels | Mesh | any | semi-finish | `surface_3d.rs:323` | — |
| Pencil Finish (`Pencil`) | Trace concave edges and creases on the surface | Mesh | any | finish | `surface_3d.rs:344` | — |
| Scallop Finish (`Scallop`) | Variable stepover for constant scallop height | Mesh | ball/tapered | finish | `surface_3d.rs:617` | LIVE F3 (refusal + success) |
| Unified Finish (`UnifiedFinish`) | Bands the surface by true-surface slope and runs waterline/scallop/raster per band | Mesh | ball/tapered | finish | `surface_3d.rs:870` | — |
| Steep/Shallow (`SteepShallow`) | Waterline on steep areas, raster on shallow | Mesh | any | finish | `surface_3d.rs:962` | — |
| Ramp Finish (`RampFinish`) | Continuous Z descent along contours, no retract | Mesh | any | finish | `finishing.rs:10` | — |
| Spiral Finish (`SpiralFinish`) | Archimedean spiral passes over the surface | Mesh | any | finish | `finishing.rs:78` | — |
| Radial Finish (`RadialFinish`) | Spoke-pattern passes radiating from center | Mesh | any | finish | `finishing.rs:127` | — |
| Horizontal Finish (`HorizontalFinish`) | Finish only flat areas of the surface | Mesh | any | finish | `finishing.rs:165` | — |
| Project Curve (`ProjectCurve`) | Project 2D curves onto a 3D mesh surface | Both | any | finish | `project.rs:8` | — |

## System-only (not in either menu)

| Label (enum) | Description | Needs | Tool | Form |
|---|---|---|---|---|
| Pin Drill (`AlignmentPinDrill`) | Drill alignment pin holes through stock | Stock | any | `drill.rs:182` |

## Observations for the report

- The menu grouping is by input format, not by machining goal. Descriptions
  are goal-like for most 2.5D ops ("Clear material inside…", "Cut along…")
  and strategy-like for most 3D ops ("Archimedean spiral passes",
  "Variable stepover…"). Nothing in the menu says which tool an op needs.
- Twelve 3D operations sit in one flat list. Six of them are finish rasters
  or contours that differ mainly in pattern; a newcomer cannot rank them from
  the descriptions alone. The reach map (shown on the finish inspector) is
  the only pre-generation evidence about tool fit.
- Three ops advertise a V-bit in their description; two (Scallop, Unified
  Finish) require a ball tip but their descriptions do not say so.
- "Drill holes from SVG circle positions" does not describe the fallback the
  generator actually applies (polygon centroids) — UX-R03-004.
- No entry is marked deprecated or experimental in the registry.
