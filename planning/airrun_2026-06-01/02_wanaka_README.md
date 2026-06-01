# Wanaka Air-Run

**What:** Full two-setup CAM job for the Wanaka terrain piece.
Setup 1 = back side (drill, rough, holes, V-carve rivers/lakes).
Setup 2 = front side (3D rough + 3D finish).

**Tools called:**
- T3 (End Mill) — Setup 1 ops 1-4, Setup 2 op 1
- T11 (20° V-bit) — Setup 1 ops 5-6 (after M6 T11)
- T2 (Tapered Ball 2 mm) — Setup 2 op 2 (after M6 T2)

**G-code files:**
- `wanaka_setup1.nc` — ~199 KB, 6,570 moves, 41m 44s predicted
- `wanaka_setup2.nc` — ~3.1 MB, 114,457 moves, 38m 27s predicted

**Total air-run time:** ~80 min if you run both setups end-to-end.

## Setup-specific notes

### Setup 1 — back side, ~42 min

Operations in order:
1. **Pin Drill** (T3) — drills 2 corner registration pins. ~30 s.
2. **Back Rough** (T3) — adaptive3d roughing across full back. ~30 min.
3. **Holes** (T3) — drill cycle for through-holes. ~1 min.
4. **Rivers (back)** (T3) — project_curve V-carve. ~2 min.
5. *M6 T11 — tool change to 20° V-bit*
6. **Lakes (back, inside)** (T11) — V-carve fill. ~10 min.

### Setup 2 — front side, ~40 min

Operations in order:
1. **3D Rough 6** (T3) — adaptive3d front-side roughing. ~10 min.
2. *M6 T2 — tool change to Tapered Ball 2 mm tip*
3. **3D Finish 6** (T2) — drop_cutter surface finish. ~28 min.

## Tool numbering note

The project's original tool_number assignments had duplicates
(multiple tools at T1). For this air-run, the tool numbers were
remapped so each tool has a unique slot (tool_number = tool_id).
This is what makes M6 fire correctly between different tools.

If you also use this project for cutting, check that the tool
numbers in your physical tool magazine / library match the
remapped values:
- T2 = Tapered Ball 2mm tip / 7° / 6mm shank
- T3 = End Mill
- T11 = 20° V-bit

## What to watch for

- M6 pause behavior: machine stops, controller display shows pause
  prompt, you can swap tool safely, hit Cycle Start to resume
- Re-touch Z after each tool change (no BitSetter)
- Spindle restarts at correct RPM after M6 (each phase sets its
  own M3 S<rpm>)
- Rapids stay above stock_top_z (30 mm) — no diving through
  imaginary stock
- 3D Finish 6 is the densest move stream — controller buffer
  shouldn't underrun (if it does, that's a real-world finding for
  Carbide Motion / GRBL streaming performance)
