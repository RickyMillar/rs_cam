# 2.5D geometry selection: user stories, requirements, op map (2026-10-16)

Follows `GAP_ANALYSIS.md`. Evidence: a web survey of user workflows and pain
points (Vectric, Fusion, Carbide Create, Easel, Kiri:Moto, Estlcam, FreeCAD,
forums) and a code map of all 24 `OperationType::ALL` rows. Forum reports are
reports, not confirmed product behaviour.

## Question: a wizard?

The pain users report has four causes:

1. They cannot see what is selected.
2. A selection goes stale after a design edit.
3. A depth means different things in different places (absolute or relative).
4. An error does not say what to fix.

A wizard fixes none of these. A stored, visible, editable **Selection** object
fixes all four. The tools users call fast (Vectric, Estlcam, Carbide Create)
are feature-first: pick geometry, then say what to do with it. Fusion and
FreeCAD are operation-first and users call them slow and fragile. Both orders
need the same Selection object.

A guided flow helps in three cases only:

- the first job of a beginner (a "job type" that proposes ops and order);
- a job with no geometry pick (flatten the spoilboard);
- a template applied to a new file (a repeat sign, a cabinet part set).

A guided flow hurts when a user edits an existing op: one depth change must
never mean stepping through pages again.

**Proposal:** Selection first, in the op panel, with filters and feedback.
Then a thin "job type / template" flow that writes ordinary ops with ordinary
Selections. The pin wizard (`ui/pin_wizard.rs`: own draft, one apply event)
is the pattern for that flow.

## Jobs the system must serve

| Job | Picks | Depth source | Typical ops |
|---|---|---|---|
| Sign / plaque | text + art loops by layer; border loop | stock top; V angle | VCarve, Pocket, Profile (tabs) |
| Sheet parts (cabinet) | outer loops; dado loops; shelf-pin circles by diameter | through + skim; half thickness; fixed hole depth | Profile (tabs), Pocket, Drill |
| Inlay | the same loops twice (pocket and mirrored plug) | below flat surface; glue gap | Inlay, InlayPlug |
| Two-sided part | dowel points; per-side loops | through for dowels | Pin Drill, per-setup ops |
| STEP part, several floors | faces of the solid | each face sets its own floor | Pocket / Adaptive + rest |
| Text engraving | single-line or outline text, open paths | shallow constant | Trace, VCarve |
| Flatten / surface | stock only | small total | Face |

## User stories (condensed from 31)

Picking
- S1 Pick all closed loops on a layer in one action.
- S2 Pick circles by diameter with a tolerance (shelf pins, not screw holes).
- S3 Pick a face of a solid and get its loop; no edge tracing.
- S4 A duplicate, empty or broken pick is refused with the reason.

Depth
- S5 Every depth field shows its reference: stock top, model top, or a face.
- S6 Each picked face sets its own floor; one op cuts several floor depths.
- S7 Inlay depths shown on a section drawing.

Islands, open paths
- S8 Inner loops become islands automatically (already true).
- S9 An open path is cut on the line, extended to the stock, or refused; never closed silently.

Holes
- S10 Holes that fit the drill are drilled; larger ones are helix-milled.
- S11 The stated hole size is the cut size; no hidden offset.

Holding
- S12 Tabs placed automatically on straight edges, one or more per side.
- S13 A tab can be moved or deleted by hand.
- S14 Inner features are cut before the outer profile.

Tools and rest
- S15 Clear with a large tool, then a small tool for corners.
- S16 See what the large tool leaves before cutting.

Re-import (associativity)
- S17 A re-imported DXF updates every op that picked by layer or filter.
- S18 A lost pick is flagged; tool, feeds and linking stay.
- S19 A face pick survives a depth change in the CAD model.

Reuse
- S20 Save a set of ops with rule-based picks; a new file runs with one action.
- S21 A template names the layers it cannot find.

Feedback
- S22 The pick is highlighted in the viewport with a count ("14 loops, 3 islands, 2 open").
- S23 Areas the tool cannot reach are shown before generation.
- S24 An error names the loop, the cause (gap of N mm, duplicate, open, too narrow) and the fix.

Beginner and expert
- S25 A job type proposes ops and order (beginner).
- S26 Every field of an existing op is editable directly (expert).
- S27 Bulk edit across ops (change one tool in ten ops).

## Requirements

R1 **One Selection type** per op, stored in the project file, changed only
   through `ProjectSession::apply(Command)` (undo, GUI/MCP/CLI parity).
R2 **Rules before indices.** A Selection holds rules (layer, closed/open,
   circle diameter +- tolerance, face normal and Z) and resolves them at
   generation. Explicit picks (an entity, a face, a point) are allowed but
   are matched by geometry on re-import, and flagged when they do not match.
   (FreeCAD's topological naming problem is the warning.)
R3 **Every 2D source gives the same items:** loops (closed), paths (open),
   points, circles; each item carries its source, layer and depth hint.
   Sources: DXF, SVG (with explicit scale), BREP faces, flat regions of a
   mesh (STL), stock.
R4 **Depth references are explicit.** Top and floor each name a reference
   (stock top, model top, picked face, typed). A face pick can set the floor.
   Per-item depth is allowed.
R5 **No silent fallback.** A pick that resolves to nothing, an open path in a
   closed-only op, or a non-flat face in a face pick refuses with a message
   that names the item and the fix. (Today: log-only, largest-polygon-only,
   stock fallback; see GAP_ANALYSIS defects.)
R6 **Feedback without generation.** Highlight and counts update at once.
   Reach (too narrow for the tool) shows before generation.
R7 **Op order rules** (inner before outer) are visible and editable.
R8 **Templates** store ops plus Selection rules, and match tools by property.
R9 **Guided flow** writes ordinary ops; it never owns state that the op panel
   cannot edit.

## What each op should accept

Item kinds: **L** closed loops (islands from containment), **P** open paths,
**F** faces (gives loop + Z), **H** points/circles, **S** stock, **M** mesh
region (boundary).

| Op | Today | Should accept | Depth from | Notes |
|---|---|---|---|---|
| Face | stock box | S, or L as a region | stock top; typed | |
| Pocket | all model loops; F top only | L, F | F floor; per-item | multi-floor in one op |
| Profile | all model loops | L, (P with side On) | typed; F floor | add side On; tabs per side |
| Adaptive 2D | all model loops | L, F | F floor | |
| Zigzag | all model loops | L, F | F floor | |
| Trace | all model loops; P probably closed silently | P, L | typed | open-path honesty first |
| Chamfer | all model loops | L, P (edge) | tool geometry | |
| VCarve | all model loops | L (text, art) | V angle | |
| Inlay / InlayPlug | all model loops | L (same Selection shared by the pair) | typed pair | share one Selection |
| Drill | DXF points/circles; layer, XY picks | H by layer and diameter | typed; F (hole floor) | helix-mill above drill size (S10) |
| Pin Drill | project pins | system | through | no change |
| Project Curve | loops + paths + surface model | P, L + surface | below surface | already honest on open paths |
| 3D ops (12) | whole mesh + boundary | M (boundary from L, F, silhouette, rest) | model | Selection feeds the boundary only |

## Proposed packages (for operator ruling)

G1 **Open-path check.** Run an open DXF path through Trace, Profile, Pocket.
   If it closes silently: refuse now (R5). Small, safety.
G2 **Selection model in core** (R1-R3, R5): one type, resolver, project file,
   Command, MCP. Every 2.5D op reads it through one door. Existing whole-model
   behaviour = the default rule "all closed loops".
G3 **Depth references** (R4): face floor, per-item depth, the reference shown
   on every depth field and in the height diagram.
G4 **Selection UI** (R6): viewport highlight, counts, layer / open-closed /
   diameter filters next to the pick, face pick for STL flat regions.
G5 **Tabs** (S12, S13): straight edges, one or more per side, hand placed.
G6 **Templates and job types** (R8, R9): the thin guided flow.
G7 **Recognition as filters** (corner radius, passage width, depth) that fill
   a Selection; tool advice after it (FeatureCAM rule).
