# Industry defaults for rs_cam (2026-10-23)

Sources read:
- FreeCAD: `github.com/FreeCAD/FreeCAD` main, commit d3f4da79 (2026-10-21), `src/Mod/CAM/Path/...`. Code-read. This is main, not a release: `PeckRetract`, `ExtraOffset` names and the 3+2 work plane checks are recent and can differ in release 1.0.
- Kiri:Moto: `github.com/GridSpace/grid-apps` main, `src/kiri/mode/cam/...`, `src/kiri/app/conf/defaults.js`, `src/kiri/app/lang-en.js`. Code-read.
- Fusion: help.autodesk.com manuals and Autodesk support articles. Docs. Inventor CAM docs use the same HSM engine; I mark them as such.
- Confidence tags: [code], [docs], [docs-HSM] (Inventor CAM manual, same engine), [forum] (weaker), [UI] (common knowledge, no written source found; weakest).

## Q1. Chamfer of an edge that runs into a higher wall

| Kiri:Moto | FreeCAD (Deburr) | Fusion (2D Chamfer / Deburr) | Recommendation |
|---|---|---|---|
| No chamfer or deburr op. Trace can follow lines with a V-bit; no wall rule found. [code: no "chamfer" in `src/kiri`] | Cuts the whole chain. `Op/Deburr.py` offsets the edge wire in 2D (`offsetWireCompat`, Clipper open offset) and runs it at the chamfer depth. The code has no test against the model walls. Inference: the tool cuts into the higher wall. `CAMTests/TestPathOpDeburr.py` has no wall or corner case. [code, inferred] | Stops short. "Chamfer Clearance: how far the tool needs to stay away from model geometry that is not being chamfered." Steps: "To avoid nearby walls, enter a Chamfer Clearance distance." Default clearance = 0.1 x tool diameter [forum]. Deburr also checks shaft/holder clearance and retracts [Autodesk blog]. | Trim the chain so the tool clears non-chamfered walls by a clearance (Fusion model). Never gouge. Refuse only when nothing is left. |

Sources: FreeCAD `src/Mod/CAM/Path/Op/Deburr.py` (opExecute, l.210-430), `Path/Op/Util.py` `offsetWireCompat`; help.autodesk.com/view/fusion360/ENU/?contextId=MFG-REF-2D-CHAMFER-CMD; ?contextId=2D-CHAMFER-STEPS; autodesk.com/products/fusion-360/blog/troubleshooting-fusions-deburr-toolpath-how-to-eliminate-retracts-and-smooth-your-edges/.

## Q2. Drill: start, R plane, depth, through amount

| Item | Kiri:Moto | FreeCAD (Drilling) | Fusion (Drill) | Recommendation |
|---|---|---|---|---|
| Feed start ("top") | Hole rim: the top circle Z of the hole (`slice.js holes()`, `h.z`). Option "from stock top" (`camDrillFromStockTop`) default false; the label says stock top, but the code moves the start to the part top (`wztop`). [code] | Stock top. `updateDepths` starts `zmax` at stock ZMax and takes max with model/face ZMax, so `StartDepth` = stock top. [code] | Top height = **Hole top** by default [docs-HSM]. | Hole top (rim). |
| R / retract | `camDrillLift` 2 mm lift between pecks; `camZClearance` 1. [code] | The R word is `SafeHeight` = stock top + 3 mm in both default cases: non-peck passes SafeHeight; peck passes `PeckRetract`, which defaults to the SafeHeight expression (`Generator/drill.py` l.127 `cmdParams["R"] = retractheight`; `SetupSheet` `DefaultSafeHeightOffset = "3 mm"`). Default G98. [code] | Feed height is "the initial feed height and the retract peck height" (R). [docs] | R = feed height above hole top; G98 default. |
| Depth | Rim to the lowest slice where the same-area circle exists (area within 5 %). Absolute zBottom = rim Z - depth. [code] | `FinalDepth` = Z min of the hole face (`faceZmin`): blind hole -> hole floor; through hole -> model bottom. Absolute Z, not measured from hole top. Tip reaches FinalDepth. [code] | Bottom height = **Hole bottom** by default [docs-HSM]. Offsets are "incremental from the Hole top / Hole bottom". [docs] | Hole bottom; tip at floor. |
| Extra past bottom | `camDrillThru` = 5 (workspace units): "additional drill depth below part bottom". The code adds it to every selected hole, blind or not. [code] | `ExtraOffset` = "None" default (options "Tool Tip", "2x Tool Tip"). [code] | "Drill tip through bottom" + "Break-through depth": the reference (?contextId=MFG-REF-DRILL-CMD) gives no default; off in the UI [UI]; a known case where it fails on point-picked holes [support article]. | Off for blind holes; through holes get full-diameter break-through as an opt-in or a small default. Two of three tools add nothing. |

Sources: Kiri `src/kiri/mode/cam/work/slice.js` `holes()` l.702-830, `work/op-drill.js`, `app/conf/defaults.js` l.478-487, `app/lang-en.js` l.637-642; FreeCAD `Path/Op/Base.py` `updateDepths` l.1050-1155, `Path/Op/Drilling.py` l.395-435, 709-735, `Path/Base/SetupSheet.py` l.99-106; help.autodesk.com/cloudhelp/2020/ENU/INVCAM/files/GUID-731124CA-5CE5-443D-8E25-E82C05FED8B0.htm ("the default heights for Top Height and Bottom Height, are Hole top and Hole bottom"); help.autodesk.com/cloudhelp/ENU/Fusion-CAM/files/MFG-REF-FEED-HEIGHT.htm; autodesk.com/support/technical/article/caas/sfdcarticles/sfdcarticles/Drill-tip-through-bottom-does-not-extend-to-bottom-of-hole-in-Fusion.html.

## Q3. Default profile on a 3D part with no selection

| Kiri:Moto (Outline) | FreeCAD (Profile) | Fusion (2D Contour) | Recommendation |
|---|---|---|---|
| Yes. Outline is a "global" op on the part shadow (silhouette) `shadow.base`. `camOutlineOut` true (outside only). Depth = stock top to part bottom (`bottom_z` with `camZBottom` 0). Tabs: none unless the user places them (`cl-tab.js`). [code] | Yes. With empty Base, `_processEachModel` makes the envelope of every Job model. `Side` Outside, `processPerimeter` True. `opUpdateDepths`: start = stock top, final = stock bottom. Tabs are a separate dressup, not added. [code] | No default. The user selects contours; Silhouette is an opt-in selection mode. "By default, the Bottom Height is set to the Selected contour(s)." Tabs: off by default [UI]. | Offer the model silhouette, outside, to model bottom (two of three; FreeCAD goes to stock bottom), tabs off. |

Sources: Kiri `work/op-outline.js`, `work/slice.js` l.120-140, `app/conf/defaults.js` l.556-561, 634; FreeCAD `Path/Op/Profile.py` l.320-335, 474-560; help.autodesk.com/cloudhelp/ENU/Fusion-CAM/files/2D-CONTOUR-STEPS.htm; .../MFG-GEOMETRY-SELECTION-SILHOUETTE.htm.

## Q4. How a refusal reaches the user

| Kiri:Moto | FreeCAD | Fusion | Recommendation |
|---|---|---|---|
| A timed alert (toast), `show.alert(error, 5)`, and a `slice.error` event. No row marker found. [code] | `Path.Log.error` writes to the Report view. An exception in `execute` (e.g. `DepthsException`) puts the object in error state in the tree (error overlay icon). [code; tree icon is FreeCAD core behaviour] | Error icon on the operation in the browser; "click the error icon next to the affected toolpath(s)" to see the message. [docs: Autodesk support] | Persistent error icon on the op row plus the message on click/hover. A toast alone is not enough. |

Sources: Kiri `src/kiri/app/function.js` l.194-220, `app/alerts.js`; FreeCAD `Path/Op/Base.py` `execute` l.1343-1410; autodesk.com/support/technical/article/CAM-error-messages; autodesk.com/support/technical/article/CAM-toolpath-error-contour-selection-has-missing-selections.

## Q5. Move an op to another setup / frame with an absolute depth

| Kiri:Moto | FreeCAD | Fusion | Recommendation |
|---|---|---|---|
| No per-op setup. Op "z top"/"z bottom" are "offset from stock bottom", so they stay relative. [code: `lang-en.js` ou_ztop_l] | Defaults are expressions on Op values (`OpStockZMax+SafeHeightOffset`, `OpStartDepth`, `OpFinalDepth`), so they follow the new frame. A typed number does not: `_checkDepthsInWorkplane` refuses with "Heights and depths set before the work plane was assigned do not carry over - use Reset to defaults". [code] | Heights are an offset "From" a reference (Stock top, Model top, Selection, Hole top, ...). "Origin (absolute)" is measured from the setup or operation origin. So a height stays relative to the references of its setup. Moving between setups is not documented. [docs] | Keep heights relative to a named reference. On a move, re-resolve the reference in the new setup. Refuse a bare absolute value that falls outside the new stock. |

Sources: FreeCAD `Path/Base/SetupSheet.py` l.101-106, `Path/Op/Base.py` `resetDepthDefaults` l.944-990, `_checkDepthsInWorkplane` l.992-1025; help.autodesk.com/cloudhelp/ENU/Fusion-CAM/files/MFG-REF-BOTTOM-HEIGHT.htm.

## Q6. Which model a new op uses

| Kiri:Moto | FreeCAD | Fusion | Recommendation |
|---|---|---|---|
| Global ops (Outline, Rough, Contour, Level) run on every part in the workspace. Specific ops (Drill, Trace, Pocket) use what the user picks. [docs + code] | `self.model = job.Model.Group` (all Job models). On create, the GUI fills Base from the current selection (`setupUi`, `addBaseGeometry(sel)`); empty selection -> all models. [code] | The setup model. The setup "designates which bodies or components are considered when toolpaths are calculated" [forum, Autodesk community manager]; per-op model override exists (deprecated 2022 for "machine/avoid" groups). [docs] | The setup's model (all its models), with the current selection as the op's items if one exists. |

Sources: Kiri `docs/kiri-moto/CAM/ops.mdx`; FreeCAD `Path/Op/Base.py` `_setBaseAndStock` l.1027-1042, `Path/Op/Gui/Base.py` l.1740-1755; help.autodesk.com/cloudhelp/ENU/Fusion-360-API/files/CAMTransitioningToMachineAvoid_UM.htm.

## Q7. Names

| Concept | Kiri:Moto | FreeCAD | Fusion | Recommendation |
|---|---|---|---|---|
| Op top | "z top" | "Start Depth" (`StartDepth`) | "Top height" | Top height |
| Floor | "z bottom" | "Final Depth" (`FinalDepth`) | "Bottom height" | Bottom height |
| Items cut | "select" / holes / surfaces (no one name) | "Base Geometry" (panel; property `Base`) | "Geometry" tab; "Geometry selections" (Chain, Silhouette, ...) | Geometry |
| Rapid planes | "z clearance" | "Clearance Height", "Safe Height" | "Clearance height", "Retract height", "Feed height" | Clearance / Retract / Feed |

Sources: Kiri `app/lang-en.js` l.837-840; FreeCAD `Path/Op/Base.py` l.358-420, `Path/Op/Gui/Base.py` l.806, 1145; help.autodesk.com/cloudhelp/ENU/Fusion-CAM/files/MFG-GEOMETRY-SELECTIONS.htm, MFG-REF-BOTTOM-HEIGHT.htm.

## Q8. Tool for a bore/helix on a hole larger than the drill

| Kiri:Moto | FreeCAD | Fusion | Recommendation |
|---|---|---|---|
| No auto-add. Helical op has its own tool setting (`camHelicalTool`). [code] | No auto-add. A new op (Helix too) takes the tool controller of the previous op in the Job (`setDefaultValues`). Helix "find all" lists cylinders with radius >= tool radius. [code] | Hole Recognition: "A tool must match the hole size or be a smaller substitute"; tools come from the active libraries; "Counter Bore operations look for Flat End mills". If no tool fits, the op is created but does not generate until a tool is picked. [docs] | Pick a flat end mill that fits the hole (Fusion). Do not reuse the drill. If none fits, create the op and flag "no tool". |

Sources: Kiri `app/conf/defaults.js` l.500-515; FreeCAD `Path/Op/Base.py` `setDefaultValues` l.861-885, `Path/Base/Drillable.py` `getDrillableTargets` l.231-250; help.autodesk.com/view/fusion360/ENU/?contextId=EXT-MFG-HOLE-RECOG.

## Q9. "Drill all holes by size" feature

| Kiri:Moto | FreeCAD | Fusion | Recommendation |
|---|---|---|---|
| Drill op buttons: "select" (individual) and "Select Matching Diameter": selects holes whose area is within 5 % of the drill's. One op per drill size. [code] | "Auto-select" button (`resetBase` -> `findAllHoles`): every vertical cylinder with radius >= tool radius, in one op. No grouping. [code] | Hole Recognition (Manufacturing Extension): groups holes **by diameter**; splits stepped holes into segments; each group gets actions (spot, drill, counterbore, bore, tap, ream); one spot drill for all sizes by default. [docs] | Group by diameter (Fusion, and Kiri's match rule). One drill op per group. Open it as a wizard. |

Sources: Kiri `app/cl-ops.js` l.600-605, `app/lang-en.js` l.646-648, `work/slice.js` l.823; FreeCAD `Path/Op/Gui/CircularHoleBase.py` l.497-506, `Path/Op/CircularHoleBase.py` l.265-290; help.autodesk.com/view/fusion360/ENU/?contextId=EXT-MFG-HOLE-RECOG.

## Gaps

- Fusion defaults for "Drill tip through bottom" and 2D Contour tabs: no written source found. Both are tagged [UI].
- Fusion 2D Chamfer default clearance (0.1 x tool diameter) is a forum figure.
- Fusion behaviour when an op moves between setups is not documented.
- Kiri `camDrillThru` adds 5 units to blind holes too; this looks like a Kiri defect, not a rule to copy.
