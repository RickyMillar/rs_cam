# G8: LUT discrepancies

Date: 2026-09-24. Agent: G8 research (long tool and small tool loads).

## Result

I found no wrong LUT row. G8 adds rows on a new axis (extra-long tools).
It does not re-transcribe rows that the LUT holds.

## What I checked

1. **Amana ZrN v8, standard 3-flute ball nose, wood.** The LUT rows
   `amana-zrn-ball-softwood-parallel-9525-3f` (0.1524-0.2032 mm) and
   `amana-zrn-ball-softwood-parallel-12700-3f` (0.1778-0.2286 mm) match the
   chart line `Wood, MDF, Sign-Foam ... 0.006" - 0.008" ... 0.007" - 0.009"`
   (`sources/amana_zrn_3d_profiling_v8.txt`, page 1, 3 Flute Ball Nose).
2. **MDF hardness convention.** The LUT row
   `amana-zrn-flat-mdf-pocket-3175-2f` uses Janka 1100. The G8 candidate
   MDF rows use the same value.

## Manifest notes (not row errors)

1. `helical_machining_guidebook_2016` has no `pdf_sha256` in
   `source_manifest.json`. The file at the manifest URL has the sha256
   `6d6ca79db0b491368e0de925e840caed8a8492e376581665018226d26705e33d`
   (2026-09-24).
2. `amana_zrn_3d_profiling_v8` has no `pdf_sha256` and no stored text in
   the manifest. The file at the manifest URL has the sha256
   `5cdfb9c01ec218e9db992fee90855f005a02902f49a3897c448188cd7cd4479b`
   (2026-09-24). The text is in `sources/amana_zrn_3d_profiling_v8.txt`.
3. `amana_zrn_3d_profiling` (the older, unversioned chart; LUT stored text
   `crates/rs_cam_core/data/vendor_lut/sources/amana_zrn_3d_profiling.txt`)
   prints the 1/4 in extra-long wood cell as `0.0004" - 0.006"`. The v8 chart
   prints `0.004" - 0.006"`. The IPM column (215-320 IPM at 18,000 rpm, 3
   flutes, which gives 0.0040-0.0059 in) supports v8. The LUT holds no row
   from this cell, so nothing is wrong in the LUT. A future transcription
   must use v8 for this cell. The file at the unversioned URL still has the
   manifest hash `52465d1d...` on 2026-09-24.
