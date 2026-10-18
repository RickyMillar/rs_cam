# 2.5D geometry selection: gap analysis (2026-10-16)

Trigger: the Basis fork (wearebasis/rs_cam `feature/web-server`) adds auto CAM
and auto tool select. Operator ruling: build generic selection first; an auto
selector comes later, and only as a filter that makes a reviewable selection.

Evidence: a code inventory of master (read-only agent, 2026-10-16) and a web
survey of Fusion 360, Vectric, Carbide Create, Estlcam, FreeCAD CAM,
Kiri:Moto, MeshCAM, SolidCAM, Mastercam and FeatureCAM. Sources are at the end.

## What other tools do

- No mainstream tool detects features AND picks tools in one click with no
  review. Recognition (Fusion hole groups and pocket recognition, SolidCAM
  AFRM, Mastercam FBM) gives an editable list or selection.
- Fusion pocket recognition filters by min/max corner radius, depth and hole
  diameter. Its docs say: set the minimum corner radius to the tool radius.
- Kiri:Moto: whole-part ops (Level, Rough, Outline, Contour) plus feature ops
  that need a picked face (Pocket, Trace, Drill). Rough has a `flats` flag.
  4.5 added "area clear with pocket detection". It has no true rest machining.
- Tool choice (FeatureCAM, the clearest published rule): the largest tool
  that fits the tightest corner AND the tightest passage, with flutes that
  reach the floor; refuse when no tool fits. Fusion and Vectric: a large
  roughing tool, then a smaller rest tool. No tool picks "the widest tool
  whose centre fits" (the fork's rule).

## Must-have capabilities against rs_cam master

| # | Capability (most common first) | rs_cam master | Gap |
|---|---|---|---|
| 1 | Pick a closed loop (vector, edge loop, face) | Whole model polygons, or picked BREP faces. No per-loop pick for DXF/SVG | Partial |
| 2 | Inner loops become islands | Yes: containment nesting, face inner loops | None |
| 3 | Start and cut depth per selection, reference shown | Typed depth per op only | Yes |
| 4 | Depth from a picked face; each picked Z its own floor | Face pick sets the op TOP only (`face_top_z`); floors and tops not separated | Yes |
| 5 | Open contours: on-line, or extend to stock | Project Curve only. Trace/Pocket/Profile/Chamfer appear to close an open path silently (verify) | Yes, maybe a defect |
| 6 | Rest machining with a known prior tool | Sim-stock rest, rest edges declared on the consumer | Ahead |
| 7 | Bulk select by layer, open/closed, circle diameter | Layer filter on Drill only; `Polygon2` has no layer | Yes |
| 8 | Cut direction and start point per loop | Op-level climb/conventional only | Yes |
| 9 | Recognition as an editable selection (corner radius, depth, hole size) | None | Later |
| 10 | Tangent / fillet-aware chain pick | None | Low priority |

## Defects and rough edges found on the way (code reading, not yet run)

- `faces_boundary_as_polygon` keeps only the largest polygon of a union, and
  returns `None` when any one face is not horizontal: one bad face drops the
  whole pick (`geometry/enriched_mesh.rs` ~298-335).
- A pick with no polygon only logs a warning (`session/compute/generation.rs`
  ~110-118). A `FaceSelection` boundary with no pick falls back to the stock
  rectangle (~759).
- `face_top_z` reads the model frame, not the setup frame (comment at ~96).
- `abs(normal.z)` admits downward faces as "horizontal".
- Face geometry is used only when the model has no polygons.
- STL has no flat-face pick (the fork's `face_region.rs` does this).
- SVG imports in px and needs a manual scale; DXF auto-scales.
- Drill: clearing the last hole means "all holes", while core treats
  `Some(empty)` as an error.

## Proposed order

1. **Selection model.** One `Selection` type per op: loops or entities from
   any source (DXF/SVG entity or layer, BREP face, flat region of a mesh),
   stored in the project file. Every 2.5D op reads it through one door.
   Refuse, do not log, when a pick gives nothing.
2. **Depth from geometry.** A picked face gives a floor, a top, or both, in
   the setup frame. Per-selection depth overrides. Separate up and down faces.
3. **Open contours, honestly.** On-line cut, or extend to stock, or refuse.
   First verify what Trace/Pocket/Profile do today with an open DXF path.
4. **Bulk filters.** Layer, open/closed, circle diameter with tolerance (all
   loops get a layer name).
5. **Per-loop direction and start point.**
6. **Recognition as filters** that fill a Selection the user edits: corner
   radius, passage width, depth, hole diameter. Built on 1-5.
7. **Tool advice** (only after 6): the FeatureCAM rule (tightest corner and
   passage, flute reach) and a rest-tool suggestion. Never "widest centre fits".

## Worth taking from the fork

- `face_region.rs`: flat-face pick on an STL (feeds item 1).
- The island-or-opening rule and the edge overrun (feed items 1 and 6).
- `tabs_on_straights` and hand-placed tabs (separate package).
- The zigzag vertex fix: landed as b145de0d with an edge-row correction.

## Sources

- Fusion 2D Pocket: https://help.autodesk.com/cloudhelp/ENU/Fusion-CAM/files/GUID67DC60DE-EE42-4BC4-B50D-B28D938D146E.htm
- Fusion bottom height: https://help.autodesk.com/cloudhelp/ENU/Fusion-CAM/files/MFG-REF-BOTTOM-HEIGHT.htm
- Fusion pocket recognition: https://help.autodesk.com/cloudhelp/ENU/Fusion-CAM/files/MFG-2D-AUTO-POCKET-RECOGNITION.htm
- Fusion hole recognition: https://www.autodesk.com/products/fusion-360/blog/manufacturing-hole-recognition/
- Fusion rest machining: https://help.autodesk.com/cloudhelp/ENU/Fusion-CAM/files/GUID43305374-122F-45DE-B817-98A7921848AC.htm
- Vectric vector selector: https://docs.vectric.com/docs/V12.5/Aspire/ENU/Help/form/vector-selector
- Vectric pocketing: https://docs.vectric.com/docs/V12.0/Aspire/ENU/Help/form/uiPocketMachineForm
- FreeCAD Pocket Shape: https://wiki.freecad.org/CAM_Pocket_Shape
- Kiri:Moto ops: https://docs.grid.space/kiri-moto/CAM/ops
- Kiri:Moto 4.5: https://forum.grid.space/t/kiri-moto-4-5-is-live/2879
- SolidCAM AFRM: https://solidcam.com/cam-modules/afrm/
- FeatureCAM tool choice: https://help.autodesk.com/cloudhelp/2019/ENU/FCAM/files/GUID-9F769884-3F9B-4306-AA42-FD19FBE72D77.htm
- Kiri:Moto, Estlcam and Carbide Create points partly from forums; Mastercam
  FBM material is from about 2008-2012.

## Addendum 2026-10-19: open vectors in other tools

A web research pass (forum posts and docs; Kiri:Moto, Easel and CamBam
have little documentation) found these patterns:

| Topic | What other tools do |
|---|---|
| Join at import | Most tools do not join automatically. LightBurn auto-joins at 0.05 mm (fixed), closes a path at 0.5 mm, and has a user-set "close with tolerance" command. FreeCAD has a DXF "join geometry" preference. |
| Manual join | Vectric: Join Open Vectors with a user tolerance; close with a line or a smooth curve. Carbide Create: Join Vectors (open only). LightBurn: Move Ends Together, Join with Line. |
| Display | Carbide Create shows open vectors in magenta and can select all open vectors. LightBurn marks start and end nodes, has "select open shapes", and warns at preview with "Show Me". Fusion discards an open contour in silence. |
| Closed-only ops | Vectric, Carbide Create: need closed vectors. Easel: "Cannot offset open paths". LightBurn: warns, then skips. Fusion: discards in silence. |
| Open-path ops | Estlcam cuts on, left or right of an open line. Fusion 2D Contour follows an open chain and extends it tangent to the stock (Tangential Extension distance). |
| Duplicates | No tool removes them at import. LightBurn has a separate "Delete duplicates" command. |

Tolerances seen: 0.001–0.01 mm (general CAM advice), 0.05 mm (LightBurn
auto-join), about 0.1 mm (one Vectric user), 0.5 mm (LightBurn close).

What rs_cam takes from this:

- Import join: the existing 0.01 mm closing tolerance stays the default
  (P1b). P4 makes it settable per import and reports the counts: pieces
  joined, open shapes left.
- Display (P4): open paths in their own colour, start and end marked, a
  "select open" filter and a count, and a "show me" action on the
  `SelectionNote` finding.
- Join and close tool (P4): by distance, close with a line, move ends
  together. The import never closes a gap with a chord in silence.
- Closed-only ops: rs_cam skips an open path under the default rule with a
  visible note (as LightBurn does) and refuses an explicit pick of one
  (lead ruling D2 in `PLAN.md`). Fusion's silent discard is the defect to
  avoid.
- Open paths (P6): on the line, left or right (side by path direction,
  start shown), tangent extension to the stock. No lead past the drawn
  ends (lead ruling, P1b).

Sources: docs.vectric.com (Join Vectors V9.5, V12), docs.lightburnsoftware.com
(OpenvsClosedShapes, AutoJoinSelectedShapes), community.carbide3d.com
(joining-vectors-not-working/71460, find-open-vectors-on-imported-dxf/70527),
resources.imaginit.com (fusion-360-milling-chains-boundaries-and-containment),
forums.autodesk.com (unwanted-extension-milling-open-pocket), v1engineering.com
(estlcam-basics), forum.easel.com (cut-path-issue…/26803), wiki.freecad.org/Draft_Join.
