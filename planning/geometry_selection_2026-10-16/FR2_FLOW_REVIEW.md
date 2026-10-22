# FR-2: flow review on the Geometry tab (2026-10-23, night)

Release GUI built from HEAD 89e87887 (UX-8, GT-1..4, F20). The lead drove
the GUI through MCP and took screenshots. The screenshots are in the
session scratchpad (`fr2/01..08`); the morning report carries them.

## What works

- The Geometry tab opens first. A default STEP Pocket shows "Default:
  every closed loop (3)", Floor "Each picked face", and cuts the air above
  each step down to its face (01, 02).
- One face click replaces the default. The Pocket cuts only that step,
  to its face Z (03). Total: Add, click, Generate.
- F20 holds live. A face floor below the picked face cuts into the part.
  The guard gives `safety.cuts_into_model` (4827 of 12184 samples, 10 mm
  deep). The list row and the header read BLOCKED in red (04).
- DXF Drill default: 1 point and the Ø6 circle are drilled; Ø8 and Ø10
  show "larger than the tool" and "2 holes to bore: Add a Bore" (08).

## Findings

- F22 (layout): the inspector column truncates or overflows.
  - With a long row ("Ø 6.00 at (15.0, 15.0) Z 20.0... Rule... ×", the
    floor face slot "Face at (50.0, 20.0), 28.3 m...", a Safety line),
    the panel grows past the window edge. The tab strip then cuts
    "Linking" (04, 05, 06).
  - Labels truncate when the panel fits: "Face Z 20....", "! larger
    tha...", "drilled at ...", "Default: every closed loop, open ...".
- F23: the floor face slot names the face by anchor and size. Name it as
  the list does: "Face Z 10.00".
- F24: the Dressup tab carries the red dot for every Blocking or Caution
  finding (`tab_badges.rs`, the old "Mods" rule). A selection refusal and
  a model gouge are not Dressup problems (04, 05, 06).
- F25: "Floor Each picked face · Z 10.00" names one Z. Each face cuts to
  its own Z; show the range "Z 10.00 to 30.00", or "per face".
- F26 (MCP): the param structs accept unknown fields without an error.
  `add_toolpath_via_gui {model_id, tool_index}` and `list_model_items
  {kinds}` dropped the field and did something else.
- F27: the Drill refusal on a STEP model (05) paints the same paragraph
  three times: beside Generate (ten lines, it pushes the tabs down), under
  the Default row, and in the footer. IC1 allows one owner.
- F28: "Details · 1" does not say what it holds.
- F29 (flow): a default Drill on a STEP model with a Ø6 hole refuses
  "nothing is left to cut. 3 closed loops are not cut". The default rule
  serves Loops only (Q3), so it never takes a circle. The text names
  loops, not the hole. (R-B is the ruling; the text can name the fitting
  rim circle now.)
- F30 (flow): after a pick of the rim circle, the Drill is BLOCKED: the
  default depth goes 12 mm below the hole floor. The guard text says "Set
  the drill depth no deeper than the hole floor". It does not give the
  number (8.0 mm). The user must open Cut and type it.
- F31 (stock): a second import with "auto from model" resizes the stock to
  the second model only (50x40x25). The first model (60x40x30) then sticks
  out of the stock: Top Z 25 under a face at Z 30. `part_models` lists
  both. (Ruling: the auto stock is the union of the part models.)
- F32: the DXF Drill lists six notes "N open paths on layer X are not
  cut" and "Selected 13 items" against "Cuts 1 point · 3 circles". A drill
  never cuts a path; the notes are noise for a default rule.
- F33 (parity): `rule_offers` has no MCP door. An agent cannot see the
  offers that the GUI shows.
- Flow change (from UX-7b, re-pinned in 89e87887): a click on the face at
  the stock top of a fresh Pocket refuses `FloorAboveTop` ("Set a larger
  floor depth..."). Before UX-7b it cut 5 mm into the part. The refusal is
  right; the fix text leads to a depth that the guard then blocks. With
  stock padding (top 35) the same click faces the 5 mm of air (07).
- F21 and F5 stand (Chamfer). The Chamfer add on stepped_block refuses
  `precondition.buried_face` until a Pocket clears the face.

## Overnight builds (no ruling: nothing cuts differently until the user acts)

- V (viz, `ui/properties/`): F22, F23, F24, F25, F27, F28, F32.
- C (core): R-C + F21 as the safety exception (Chamfer default = convex
  edge chains, each at its own Z; ring default off for Chamfer on STEP;
  fallback: refuse with "pick the edges"). F30: the drill gouge text names
  the depth. F29: the Drill refusal names the fitting rim circles.

## For the operator (rulings)

R-A, R-B, R-D, R-E (FR-1), F31 (union stock), F26 (deny unknown MCP
fields: a wire break), Q5-Q10.
