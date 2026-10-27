# D55 pre-registration (G-AIRPECK), written before the change and before any parity run

Base: `dabf258d3` plus the uncommitted D53 (the D53 patch is separate).

## The change

`adaptive3d/path.rs::emit_peck_plunge` (the 3D Rough `Plunge` entry, `EntryStyle3d::Plunge`) takes the planner's contact read (`Adaptive3dSegment::RapidWithFloor::contact_z`, the cell-centre stock read). The helix and the ramp already take it (`EntrySafety::contact_top`, G-AIRRAMP). After the rapid to the sliver-safe floor (unchanged), it feeds straight through air to `contact_top + entry_clearance_mm`. This is the start the helix and the ramp use (`dressup/entry_descent.rs::rapid_to_entry_top`). The peck ladder then starts there, not at the rapid floor. With no contact read (a plain `Rapid` entry, off the planner grid), the ladder is as before.

## Expected effect on shipped output

- Strategies: every `ClearingStrategy3d` with `entry_style = Plunge`, which is the 3D Rough default (`Adaptive3dConfig::default`). AgentSearch and ContourParallel are shipped. The research arm also changes. Helix and Ramp do not change. 2D ops do not change (they do not call this function).
- Which entries change: only a `RapidWithFloor` plunge whose contact start (`contact_z + entry_clearance_mm`) is below its rapid floor, and whose column from the old ladder start to the entry is more than one Depth/Pass. Its peck strokes above the contact start become one straight feed. The moves below the contact start do not change. When the column from the old ladder start is one Depth/Pass or less, no move changes (the old ladder was already one feed).
- The planner stamp does not change: the planner stamps the vertical column from the descent floor to the entry (`clearing.rs::stamp_emitted_segment`), not the peck shape. So the planner's later decisions, the cut moves and the rapid floors do not change.
- The sim result does not change: the same XY column is fed down to the same entry Z. Only the vertical moves above the contact start change (fewer feed moves and retracts). The removed volume and the cut moves are identical.
- Byte-parity fixtures (`adaptive3d_emission_byte_parity`) and the golden toolpaths (`every_op_keeps_its_toolpath_g_opgolden`, the 3D Rough rows): they change if, and only if, one of their plunge entries has more than one Depth/Pass of column above its contact start. Prediction: the fixtures that enter a cleared or air column at a later level change. A first-level entry into fresh stock does not change, because there the contact read equals the floor.
- Goldens are not re-blessed. Every changed fixture is reported with its first differing move.
