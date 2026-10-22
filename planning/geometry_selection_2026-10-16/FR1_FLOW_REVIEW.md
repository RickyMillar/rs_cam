# FR-1 flow review (2026-10-22 night, GUI a51f9620)

The lead drove the GUI through MCP, with screenshots. Fixtures:
- `fixtures/gui_step/stepped_block.step`: stock X −5..65, Y −5..45, Z 0..32;
- `block_with_blind_hole.step`: a Ø6 hole, rim at Z 20, floor at Z 12;
- `mix.dxf`;
- `flat_plate.stl`.

Tools: a 6 mm end mill and a 90° V-bit.

## Regression: UX-7 + Q3 made default STEP ops cut into the part (SAFETY)

Before UX-7, a default op on a STEP model refused, because it had nothing to
cut. UX-7 serves the face rings by default (ruling Q3). Each finding below
was measured live:

- F1: A default Pocket takes the 3 rings. Its floor is Top − 5 from the
  highest ring (Z 30), which gives Z 25. It cuts 5 mm into the Z 30 step;
  the narrate shows a cutting move at (7.3, 30.9, Z 25).
- F2: Q2 did not fire. A face pick (Z 10) after the GUI add kept the floor
  at `{top, 5}`, which gives floor Z 5. That cuts 5 mm into the face.
- F3: A default Profile Outside on the rings goes to Z −2. It cuts through
  the next steps. Profile Outside of a ring that borders higher material
  gouges at any floor below that material.
- F9: A Drill on the blind hole (rule: circle Ø6) uses the default depth of
  32 from Z 20. It drills through the blind floor.

Fix in progress (editor):
- Rings take the floor `each_face` depth 0, the same as faces.
- The Q2 trigger is repaired.
- A gouge guard compares the emitted motion with the model mesh (drop-cutter
  probe, radius less a tolerance). It applies to Pocket, Profile, Zigzag,
  Adaptive 2D and Face. It applies to Drill and Bore only where the model
  has a hole at that point. A gouge gives a blocking diagnostic, and the
  export refuses.

## Other findings

- F4/F16: A STEP model has no part-outline item, so the user cannot profile
  the outline:
  - the rings gouge;
  - the bottom face refuses because it faces down;
  - the edges at Z 0 are 4 open paths under the part.
  An STL has the same gap.
- F5: The rings hold CONCAVE edges (the Z 20 ring edge under the Z 30 wall).
  A Chamfer on a ring cuts into the wall. Chamfer needs the convex edges
  only (the BREP gives the dihedral).
- F6: The Chamfer refusal says "the face is 2.0 mm below", but the user
  picked no face. The selection was the ring default.
- F7: The Geometry panel shows "Each picked face" and "Faces set the top"
  while the floor is Top Z and the user picked no face.
- F8: A step-face pocket keeps the tool inside the ring. Outside the part,
  the stock (5 mm here) stays as a wall. The pocket needs "open edges": a
  ring edge on the part outline that the tool may cross.
- F10/F12: One hole gives 2 circles (rim and floor), 2 small loops and a
  floor face. The panel says "2 circles, Ø6 × 2 drilled" for one hole. The
  floor circle serves no Drill job, and a click on it measures depth from
  Z 12.
- F11: Drill all holes of one size on STEP takes 6 or more clicks (Rule,
  kind, diameter), and the user must type the depth. The default Drill
  refuses on the rings.
- F13: The Drill Points preview draws about 8 crosses (corner points), not
  the one drilled hole.
- F14: The Drill help text names DXF points on a STEP model.
- F15: A second STEP import lands on top of the first at the origin, with
  no note.
- F17: An STL gives flat faces only. Pocket-a-face works; edge jobs do not.
- F18: DXF does not change from UX_REVIEW.

## Click counts (+ = add, M = menu item, P = Pick, c = viewport click, G = Generate)

| Job | STEP today | STL today | DXF today | Target |
|---|---|---|---|---|
| Pocket one face to its floor | 5 (+,M,P,c,G) after the Q2 fix; today it gouges | 5 | n/a | 5 |
| Cut each step to its own depth | 7 (+,M,P,c,c,c,G) | 7 | n/a | 3 (default each-face rings) |
| Profile the outline | not possible | not possible | 3 | 3 (outline default) |
| Chamfer the top edges | ring default gouges concave edges | not possible | 3 | 3 (convex-edge default) |
| Drill all holes of one size | 6+, and the user types the depth | n/a | 3 | 3 (rim circles, depth to the hole floor) |

## Proposed packages: rulings needed (each changes what is cut)

- R-A, outline item: a STEP or STL model gives one part-outline `Loop`
  (the outer boundary of the XY projection) at the model bottom Z.
  Default Profile on a mesh model = the outline, Outside, to the bottom.
- R-B, Drill on STEP: the default selects the hole RIM circles that fit the
  tool. The floor circle is not served. The default depth = rim Z − hole
  floor Z; a through hole goes through.
- R-C, Chamfer on STEP: the default selects the convex edge runs only.
  Concave runs never serve a Chamfer. (This is pure safety; build it first.)
- R-D, open edges: a Pocket on a ring may cross the ring edges that lie on
  the part outline, out to the stock edge.
- R-E: show one hole as one row ("Ø6 hole, 8 deep"), not as 2 circles.
