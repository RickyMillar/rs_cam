# Track B — does the SHIPPED shallow raster meet its own scallop spec?

**Rank:** 2 of 6 (`planning/finishing_status_2026-09-01.md` §5 avenue B+E).
**Status: FIXED (2026-09-01, operator ruling: ALWAYS ON, no dial).** The
Shallow arm now derates each region's effective raster stepover by
`cos(theta_max)` before any lattice is built
(`unified_finish::shallow_region_max_slope_deg`). Fix acceptance: the
reworked `shipped_raster_spacing_b1` instrument reads CLEAN on all three
fixtures (planes at exactly 1.0000 x s_max; sphere 0.9391); flat ground
is byte-identical (sentries in
`crates/rs_cam_core/tests/shallow_raster_slope_derate.rs`). Measured
price on the fixtures: 1.093x (20 deg) / 1.249x (40 deg) cutting
distance — see FINDINGS.md §"Fix acceptance".

Original verdict record below, kept verbatim.

**VERDICT: DEFECT CONFIRMED** — on the
tilted-plane fixtures the shipped arm's achieved surface spacing is
`s_XY / cos(theta)` exactly (1.064x at 20 deg, 1.305x at 40 deg, 100% of
sloped samples > 1.05 x s_max; x floor 0.984 / 0.808). The sphere cap
reads CLEAN (max 0.982 x s_max): convex contact focusing
`R_s/(R_s+K_c)` cancels `sec(theta_y)` up to 17.75 deg and the cap tops
out at 17.46 deg. Evidence: `FINDINGS.md`,
`crates/rs_cam_core/tests/shipped_raster_spacing_b1.rs`.

## The question

The harness raster (`raster_candidate`, test-local) spaces passes in XY
projection while the scallop constraint lives on the 3D surface. On the
analytic sphere it measured 0.997× the floor — under-coverage — with
achieved surface spacing a median 3.3 % wider than admissible
(synthesis §1). **Unverified: does the SHIPPED `unified_finish`
shallow-band arm share the defect?** On a slope θ the surface spacing of
an XY-uniform raster is `s_XY / cos θ` — up to 1.41× at the 45° band
edge.

## Pre-registered verdict bands

- **DEFECT CONFIRMED** if the shipped arm's achieved surface spacing
  exceeds `s_max` by > 5 % over a material fraction (> 10 %) of sloped
  samples on the analytic fixture.
- **CLEAN** if achieved spacing ≤ `s_max · 1.02` everywhere the
  instrument can measure.
- Anything between: report as measured, no verdict.

## Evidence rules

- Analytic fixture only (facets ≤ stepover/3). The sphere settles it
  exactly; a tilted-plane fixture isolates the cos θ mechanism.
- Run the SHIPPED code path (`FinishBand::Shallow` arm), not the harness.
- Score ×floor with the L_min integrand; report achieved-spacing
  distribution by slope band.
- Step 2 (only after the verdict): add the HONEST-RASTER arm (stepover
  derated by cos θ) to the ×floor table — avenue E.

## Evidence lands here

`planning/honest_raster_2026-09-01/FINDINGS.md` + instrument test under
`crates/rs_cam_core/tests/`, `#[ignore]` evidence convention.
