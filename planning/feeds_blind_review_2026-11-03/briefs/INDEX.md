# Briefs index (nz-wanaka-21, Phase 1)

Op index: the 0-based position of a toolpath among the enabled toolpaths of
`planning/rivmap_block_job/nz_wanaka_21_operator.toml`, in file order (setup 1 Back, setup 1b
Clamp pads, setup 2 Epoxy pour (no toolpath), setup 3 Front). All 19 toolpaths are enabled; no
disabled toolpath exists. Groups: A roughing and slotting, B V-bits and engraving, C finishing and
small tools (as in `planning/feeds_blind_review_plan_2026-11-03.md`).

Every reviewer also reads `00_machine_and_material.md`.

| File | Ops | Names | Group |
|---|---|---|---|
| `00_machine_and_material.md` | all | machine limits, power curve, oak, epoxy | A, B, C |
| `01_face_18mm.md` | 0, 12 | Face back to T1; Face front to T | A |
| `02_back_bulk_rough_18mm.md` | 1 | Back bulk rough 18 mm | A |
| `03_back_rest_6mm.md` | 2 | Back rest 6 mm (cavity, hollow, lakes, pockets) | A |
| `04_pin_drill.md` | 3 | Pin drill, flip pins | A |
| `05_light_hole_drill.md` | 4 | Drill light holes 6 mm end mill | A |
| `06_river_channel_6mm.md` | 5 | River channel 6 mm, pass 7 of 7 | A |
| `07_lake_pockets_2mm.md` | 6, 7 | Lake pocket 2 mm, 8 lakes to Z 5.91; Lake pocket 2 mm, 1 lake to Z 3.99 | C |
| `08_antenna_pocket_2mm.md` | 8 | ANTENNA_D5.00 pocket 2 mm | C |
| `09_v20_rivers_lakes.md` | 9 | V rivers and lakes | B |
| `10_v60_slit.md` | 10 | Day strip slit V60 | B |
| `11_clamp_pads_6mm.md` | 11 | Face clamp pads 6 mm | A |
| `12_v60_engrave.md` | 13 | Engrave day labels V60 | B |
| `13_front_bulk_rough_18mm.md` | 14 | Front bulk rough 18 mm | A |
| `14_front_rough_6mm.md` | 15 | Front 3D rough | A |
| `15_front_rest_r1.md` | 16 | Front rest R1 (valleys) | C |
| `16_front_scallop_r1.md` | 17 | Front scallop finish | C |
| `17_outline_profile_6mm.md` | 18 | Outline profile with tabs | A |
