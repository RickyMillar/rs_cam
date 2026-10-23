# P2 C13 — the reserved lobe margin: exact and lattice check of the growing-bite lead-in (2026-10-24)

Status: READ-ONLY analysis. Python only. No cargo. No Rust edit. Scripts
are in `phase2/c13/`. `python3 -I final.py` (about 15 minutes) writes
`final.json`. `python3 -I assert_c13.py` asserts every number in this
note. It passes.

Every number comes from R 3, s 2, the planner cell 0.5 mm, the ceiling
formula and the star geometry (`fixtures/demo_star.svg`, the C10 model
seed). The ceiling is derived from the code formula
(`PassLoad::swept_width`: `radial_woc(1.05 · α*/2π)`): c·D = **2.175868
mm**. C10 used 0.3626 × 6 = 2.1756 mm, which is 0.00027 mm low. A number
marked PREDICTION needs the harness.

## Summary

- **The bands.** C10 counts "about 40" bands at levels 8–15. Level 8 is
  not a lobe band: its parent is the closed level-7 wrap at 15.15 mm,
  which is under the inside clearance 19.09 − R. C10's own lobe rule
  ("split closed") stops there. So the lobe bands are levels 9–15, 7 per
  arm, **35 bands**.
- **Exact check of C10's growing bite at C10's spacing: 12 of 35 pass.**
  - 19 bands (levels 9–12) fail by 0.0001–0.0005 mm. The C10 tilt bound
    is not conservative at full bite on a curved front. It was also built
    against 2.1756, not 2.175868.
  - 4 bands fail by **0.020, 0.050, 0.168 and 0.236 mm**: the shortest
    arm (arm 0, the bottom arm) at levels 12–15. The ramp reaches full
    bite only 0.80–1.17 mm before the far wall. There it meets the corner
    stock that the previous arc leaves at that wall.
  - The cut back (F → E) holds the ceiling in all 70 band orientations,
    as C10 claims.
- **Two fixes make all 35 bands pass in exact geometry.**
  - The C13 tilt law: the smaller of the C10 tilt and the exact tilt.
    The exact tilt comes in closed form from R, ρ, Δ and c·D. This law
    removes the 19 small failures and leaves the 4 corner failures.
  - The corner-touch spacing: reduce Δ' until the full-bite point P lies
    at least `l_c` before the far wall end F. `l_c` is the arc length at
    which a plain pass first meets the previous arc's corner stock
    (0.66–1.79 mm on arm 0). With it, 35 of 35 bands pass. A consistent
    level sequence from 17.10 to 31.05 mm (8 bands per arm) also passes,
    80 of 80 band orientations.
- **The lattice cannot hold or verify this lead-in.**
  - The ramp runs at zero slack by design: it uses all of the margin as
    tilt. Every margin is under one quantum.
  - At the plan's own vertex spacing (one cell, as `lead_band` builds the
    pieces), the planner reads the lead-in UNDER the exact width: by
    0.016–0.561 mm on the accepted bands and by up to 0.707 mm on C10's
    bands. It reads the ramp at a median of 0.84–1.68 mm against a true
    2.176 mm. It accepts every accepted band at every phase. It also
    accepts the 4 bands that exact geometry rejects by up to 0.236 mm, at
    3 or 4 of 4 phases.
  - At one pass step (1.5 mm) the reading lies from 0.475 mm under to
    0.281 mm over the exact width. It refuses at least one phase in 25 of
    the 35 bands that exact geometry accepts.
- **Cost.** The reserved spacing adds 1 band per arm (the last step is
  1.05 mm). The lead-in scheme cuts each arc about twice. Total: +478.7 mm
  of cut for 35 plunges removed, so **13.7 mm of cut per plunge removed**.
  Of that, 1.0 mm is the extra passes and 12.7 mm is the ramp plus the
  forward run.
- **Verdict.** In exact geometry the scheme is viable at this stepover on
  the star, with the C13 law and the corner-touch spacing. As a planner
  feature it is NOT viable now: the planner lattice cannot see the ramp's
  width (G-CELLREAD). A replay result for these bands carries no
  information about the true load. The derived rule exists, but it is
  **not** a function of R, s, c·D and the cell alone. It needs `l_c`, a
  property of the two arc ends and the wall.
- **Found on the way (not caused by the lead-in).** Every plain lobe arc
  reads 2.25–2.70 mm in exact geometry where it arrives at a wall, which
  is 0.08–0.52 mm over c·D. The lattice reads the same arrivals at
  1.45–2.34 mm (cell steps) and up to 2.46 mm (pass steps). The lead-in
  scheme replaces one plunge per band with one more wall arrival.

## Method

### Geometry

- The star arms are seed-visible: every lobe arc at levels 8–18 has the
  seed as its pivot and ends on the inset wall at both ends (C10 model,
  branch label 0, wall distance 3.00–3.09 at each end). So each band is
  built from exact circles about the seed (50, 50.6), clipped to the
  tool-centre region. The arc ends are bisected to 1e-12 rad.
- Arm k is the arm of tip k: 0 = (50, 5) at −90°, 1 = (95, 35), 2 = (79,
  91), 3 = (21, 91), 4 = (5, 35). Arm 0 has the shortest arc at every
  level.
- The stock before band k is the part minus the discs of radius R along
  six prior arcs (ρ − Δ', ρ − 2Δ', …). Six arcs are enough: the disc at ρ
  reaches ρ − R, and the oldest wall corner then lies below ρ − R − 6. So
  the stock holds the real wall corners that each arc leaves where it
  meets the inset wall.
- The band (C10 Q2 form): a ramp from E of arc a (bite 0) to full bite
  at P, arc b from P to F, then arc b back from F to E. The ramp is
  integrated in the pivot frame, `dθ = db / (r tan β(b))`, in 4000 steps.
  The tilt is read at the larger bite of each step, as C10 does.
- Three tilt laws:
  - "c10": `β = φ₀ + asin((c·D − e − b − R)/√(h² + R²))`, with h = h(Δ'),
    e = w(ρ, Δ') − Δ', against 2.1756 (C10 `growing_bite_span`).
  - "exact": the largest β with exact width c·D on the stock outside the
    last circle A = ρ − Δ' + R. A rim point at angle φ from the heading
    has `|q|² = r² + R² + 2rR sin(φ + β)`, so the stock on the rim is the
    φ interval where `sin(φ + β) > κ`, `κ = (A² − r² − R²)/(2rR)`. The
    width is then closed form. The script checks it against the numeric
    width to 3e-5 mm. Used alone, this law starts almost radially (β → π/2
    at bite 0) and leaves the region at the wall where it starts, so it
    is not used alone.
  - "min" (C13 law): the smaller of the two. Its span is within 0.03 mm
    of C10's span (14.38 against 14.37 mm at ρ 25.05, Δ 2). The exact
    tilt binds only near full bite.

### The exact width, and why it is fine enough

- The exact swept width at a point is the extent across the heading of
  the stock on the leading half of the cutter rim. This is the limit of
  the simulator's fresh-cell extent as the step goes to zero.
- Stock test, per rim point: inside the part polygon, and more than R +
  1e-5 mm from every prior arc (exact distance to a circular arc) and
  from the band's own earlier path (dense samples 0.01 mm apart, whose
  distance error is at most 0.01²/(8R) = 4e-6 mm).
- The rim is sampled at 3000 points over the half circle (0.0031 mm on
  the rim). The two extreme stock angles are bisected to 1e-10 rad. So
  the width error is about 1e-5 mm. That is 1000 times finer than the
  smallest margin under test (0.0106 mm) and 50 000 times finer than the
  cell.
- Path samples are 0.05 mm apart. The largest sample is refined at
  0.002 mm, or at every integrated ramp node. On the ramp the heading is
  the law tilt at the node, not a chord. Near full bite the width changes
  by `√(R² − (R − c·D)²)` = 2.885 mm per radian of tilt, so a chord
  heading error of 0.002 rad gives a false 0.006 mm.
- Check: at mid-arc a plain pass reads the C5 formula `w(ρ, Δ)` to 2e-5
  mm.

### What counts as the lead-in's load

- The whole ramp counts. On the forward and back runs, a point counts
  unless the plain pass at the same point and heading (prior stock only)
  is also over c·D. Those points are the plain wall arrival, which exists
  without the lead-in. They are reported apart.

### The lattice reading

- `MaterialGrid` with the fringe (4 × 4 sub-points), `clear_capsule`,
  `swept_width_on` and `measure_step` are ported one to one
  (`material_grid.rs`, `search.rs:737-1104`). The ceiling is c and the
  sliver ceiling is c + cell/D.
- The prior arcs are stamped, then the band is read step by step. Each
  step is read, then stamped, as the plan grid of the cascade walk does
  (`research_geodesic.rs:5652`).
- Two vertex spacings:
  - one cell (0.5 mm): the pieces as `lead_band` builds them
    (`subdivide(·, cell)`); the replay keeps 0.5 mm steps;
  - one pass step (1.5 mm): the planner's own walk step.
- Four lattice phases per band: (0, 0), (0.125, 0.25), (0.25, 0.125),
  (0.375, 0.375) mm.
- "Accepts" means that no lead-in step reads over c.

## Law check (C10 tilt, closed-form exact width, no wall)

m is the band's width margin `c·D − w(ρ, Δ')`.

| Level | ρ (mm) | Δ' (C10) | m (mm) | Largest width (mm) | At bite | Over c·D by (mm) |
|---|---|---|---|---|---|---|
| 9 | 19.06 | 1.952 | 0.0167 | 2.17596 | full | +0.00010 |
| 10 | 21.05 | 1.957 | 0.0311 | 2.17616 | full | +0.00029 |
| 11 | 23.05 | 1.947 | 0.0582 | 2.17635 | full | +0.00048 |
| 12 | 25.05 | 1.915 | 0.1045 | 2.17625 | full | +0.00038 |
| 13 | 27.05 | 1.848 | 0.1859 | 2.17497 | full | −0.00089 |
| 14 | 29.05 | 1.752 | 0.2958 | 2.17133 | full | −0.00453 |
| 15 | 31.05 | 1.568 | 0.4962 | 2.16067 | full | −0.01520 |

At full bite the C10 bound adds the straight-edge tilt term to the
circular excess. On a curved front the true tilt term is a little
larger. The excess is under 0.0005 mm, but it is over the ceiling.

## Per-band table: C10's bands at C10's spacing (tasks 2 and 3)

The exact column is the worse of the two orientations. The lattice
columns are orientation +1: the range over the 4 phases of the largest
lead-in reading, and in brackets the phases at which the lattice accepts.
"Spare" is the arc length from P to F.

| Level | ρ | Δ' | Arm | Arc b (mm) | Spare (mm) | Exact max (mm) (piece, wall dist.) | Exact | Lattice, cell steps (mm) | Lattice, pass steps (mm) |
|---|---|---|---|---|---|---|---|---|---|
| 9 | 19.06 | 1.952 | 0 | 14.40 | 1.99 | 2.1760 (ramp, 1.55) | fail +0.0001 | 1.918–2.091 (4/4) | 2.077–2.175 (4/4) |
| 9 | 19.06 | 1.952 | 1 | 15.50 | 3.06 | 2.1760 (ramp, 2.35) | fail +0.0001 | 1.979–2.119 (4/4) | 2.055–2.195 (3/4) |
| 9 | 19.06 | 1.952 | 2 | 15.07 | 2.50 | 2.1760 (ramp, 2.17) | fail +0.0001 | 1.931–2.069 (4/4) | 2.066–2.187 (3/4) |
| 9 | 19.06 | 1.952 | 3 | 15.07 | 2.50 | 2.1760 (ramp, 1.97) | fail +0.0001 | 1.884–2.103 (4/4) | 2.076–2.222 (3/4) |
| 9 | 19.06 | 1.952 | 4 | 15.50 | 3.06 | 2.1760 (ramp, 2.66) | fail +0.0001 | 2.039–2.149 (4/4) | 2.122–2.169 (4/4) |
| 10 | 21.05 | 1.957 | 0 | 12.47 | 1.51 | 2.1761 (ramp, 1.23) | fail +0.0003 | 1.799–2.148 (4/4) | 1.992–2.108 (4/4) |
| 10 | 21.05 | 1.957 | 1 | 13.55 | 2.57 | 2.1761 (ramp, 2.08) | fail +0.0003 | 2.053–2.176 (3/4) | 2.070–2.187 (2/4) |
| 10 | 21.05 | 1.957 | 2 | 13.32 | 2.25 | 2.1761 (ramp, 2.00) | fail +0.0003 | 2.030–2.075 (4/4) | 2.031–2.246 (2/4) |
| 10 | 21.05 | 1.957 | 3 | 13.32 | 2.25 | 2.1761 (ramp, 2.00) | fail +0.0003 | 1.999–2.148 (4/4) | 1.948–2.174 (4/4) |
| 10 | 21.05 | 1.957 | 4 | 13.55 | 2.57 | 2.1761 (ramp, 2.08) | fail +0.0003 | 1.943–2.113 (4/4) | 2.012–2.118 (4/4) |
| 11 | 23.05 | 1.947 | 0 | 10.69 | 1.30 | 2.1764 (ramp, 1.11) | fail +0.0005 | 1.856–2.089 (4/4) | 2.112–2.191 (3/4) |
| 11 | 23.05 | 1.947 | 1 | 11.79 | 2.38 | 2.1763 (ramp, 2.01) | fail +0.0005 | 1.908–2.154 (4/4) | 2.050–2.316 (3/4) |
| 11 | 23.05 | 1.947 | 2 | 11.72 | 2.24 | 2.1763 (ramp, 2.04) | fail +0.0005 | 1.944–2.101 (4/4) | 1.956–2.426 (2/4) |
| 11 | 23.05 | 1.947 | 3 | 11.72 | 2.24 | 2.1763 (ramp, 1.93) | fail +0.0005 | 1.867–2.074 (4/4) | 2.039–2.328 (3/4) |
| 11 | 23.05 | 1.947 | 4 | 11.79 | 2.38 | 2.1763 (ramp, 2.01) | fail +0.0005 | 1.946–2.049 (4/4) | 1.960–2.196 (3/4) |
| 12 | 25.05 | 1.915 | 0 | 9.03 | 1.17 | **2.1962** (ramp, 1.02) | **fail +0.020** | 1.741–2.108 (4/4) | 2.050–2.235 (2/4) |
| 12 | 25.05 | 1.915 | 1 | 10.13 | 2.26 | 2.1762 (ramp, 1.97) | fail +0.0004 | 1.954–2.110 (4/4) | 1.996–2.178 (2/4) |
| 12 | 25.05 | 1.915 | 2 | 10.22 | 2.29 | 2.1762 (ramp, 2.11) | fail +0.0004 | 1.930–2.121 (4/4) | 1.960–2.381 (3/4) |
| 12 | 25.05 | 1.915 | 3 | 10.22 | 2.29 | 2.1762 (ramp, 2.11) | fail +0.0004 | 1.855–2.087 (4/4) | 2.099–2.448 (3/4) |
| 12 | 25.05 | 1.915 | 4 | 10.13 | 2.26 | 2.1762 (ramp, 2.10) | fail +0.0004 | 1.789–2.048 (4/4) | 2.074–2.342 (2/4) |
| 13 | 27.05 | 1.848 | 0 | 7.42 | 1.13 | **2.2257** (ramp, 1.01) | **fail +0.050** | 1.812–2.025 (4/4) | 1.812–2.251 (2/4) |
| 13 | 27.05 | 1.848 | 1 | 8.55 | 2.25 | 2.1750 (ramp, 2.11) | pass | 1.786–2.067 (4/4) | 1.985–2.037 (4/4) |
| 13 | 27.05 | 1.848 | 2 | 8.77 | 2.42 | 2.1754 (fwd, 0.67) | pass | 1.750–1.865 (4/4) | 2.010–2.266 (3/4) |
| 13 | 27.05 | 1.848 | 3 | 8.77 | 2.42 | 2.1754 (fwd, 0.67) | pass | 1.818–1.938 (4/4) | 1.959–2.203 (2/4) |
| 13 | 27.05 | 1.848 | 4 | 8.55 | 2.25 | 2.1750 (ramp, 2.00) | pass | 1.833–2.091 (4/4) | 2.033–2.238 (2/4) |
| 14 | 29.05 | 1.752 | 0 | 5.86 | 0.85 | **2.3435** (ramp, 0.77) | **fail +0.168** | 1.851–2.302 (3/4) | 1.863–2.318 (2/4) |
| 14 | 29.05 | 1.752 | 1 | 7.02 | 2.00 | 2.1752 (fwd, 0.47) | pass | 1.950–2.135 (4/4) | 2.018–2.240 (2/4) |
| 14 | 29.05 | 1.752 | 2 | 7.37 | 2.31 | 2.1758 (fwd, 0.53) | pass | 1.726–1.987 (4/4) | 1.973–2.148 (4/4) |
| 14 | 29.05 | 1.752 | 3 | 7.37 | 2.31 | 2.1758 (fwd, 0.53) | pass | 1.770–2.048 (4/4) | 1.962–2.290 (2/4) |
| 14 | 29.05 | 1.752 | 4 | 7.02 | 2.00 | 2.1752 (fwd, 0.47) | pass | 1.782–1.994 (4/4) | 1.854–2.099 (4/4) |
| 15 | 31.05 | 1.568 | 0 | 4.34 | 0.80 | **2.4118** (ramp, 0.73) | **fail +0.236** | 1.705–1.940 (4/4) | 1.924–2.354 (3/4) |
| 15 | 31.05 | 1.568 | 1 | 5.52 | 1.98 | 2.1748 (fwd, 0.39) | pass | 1.693–1.967 (4/4) | 1.906–2.084 (4/4) |
| 15 | 31.05 | 1.568 | 2 | 5.99 | 2.41 | 2.1756 (fwd, 0.38) | pass | 1.852–1.994 (4/4) | 1.863–2.247 (2/4) |
| 15 | 31.05 | 1.568 | 3 | 5.99 | 2.41 | 2.1756 (fwd, 0.38) | pass | 1.645–1.910 (4/4) | 1.891–2.367 (3/4) |
| 15 | 31.05 | 1.568 | 4 | 5.52 | 1.98 | 2.1748 (fwd, 0.39) | pass | 1.810–2.149 (4/4) | 2.175–2.224 (3/4) |

Reading the table:

- **Exact.** 12 bands pass, 19 fail by under 0.0005 mm (the law), and 4
  fail by 0.020–0.236 mm (the corner, arm 0, levels 12–15). In each
  corner failure the worst point is on the ramp, 0.73–1.02 mm from the
  far wall, at a spare of 0.80–1.17 mm. C10's fit test ("span ≤ arc")
  lets the ramp end inside the far wall's corner zone.
- **Lattice, cell steps.** The reading is 0.000–0.707 mm under the exact
  width. The lattice accepts 33 of 35 bands at all four phases. It
  accepts the 4 corner failures at 3 or 4 phases: for example 1.705–1.940
  mm against an exact 2.4118 mm at level 15.
- **Lattice, pass steps.** The reading lies 0.488 mm under to 0.272 mm
  over the exact width. 26 of 35 bands are refused at one phase or more.

## Where the margin is under one quantum (task 3)

- The ramp's own slack is **zero**. The law spends the whole band margin
  m on tilt, so the true width is c·D along the ramp. No lattice can
  confirm "exactly at the ceiling". Its quantum is one cell in width
  (0.5 mm), and the sliver sub-points are a quarter cell (0.125 mm).
- The band margins m are under one cell at every level of C10's table
  (0.0167–0.4962 mm). At the accepted spacing below they are 0.0106–
  0.3746 mm at levels 9–14 (under one cell) and 0.6248 mm at level 15
  (over one cell).
- At cell steps the lattice does not see the ramp's width at all: the
  median ramp reading on the accepted bands is 0.84–1.68 mm against a
  true 2.176 mm. A 0.5 mm
  step's fresh crescent is too thin at its sides for a cell centre to
  fall in it.
- So the replay accepts this lead-in where exact geometry rejects it.
  At pass steps it refuses it where exact geometry accepts it. **The
  lattice does not decide these bands. Only exact geometry (or the
  simulator) does.**

## The fixes, and the accepted spacing

### B: the C13 law at C10's spacing

31 of 35 bands pass. The 4 corner failures stay: +0.019, +0.049, +0.167
and +0.236 mm. The no-wall width is at most c·D + 2e-5 mm for every
band (that is the sampling bound).

### R: the C13 law at the accepted spacing

The accepted Δ is the largest Δ ≤ min(s, law step) at which the band on
arm 0 passes the exact check (bisection to 2.4e-4 mm). The rule Δ is the
largest Δ at which the spare is at least `l_c`.

| Level | ρ | Δ' (C10) | Accepted Δ | Rule Δ | m at accepted | l_c on arm 0 (mm) | Exact, all arms, both orientations | Lattice cell steps: reading, accepts | Lattice pass steps: bands refused at ≥ 1 phase |
|---|---|---|---|---|---|---|---|---|---|
| 9 | 19.06 | 1.952 | 1.9577 | 1.9577 | 0.0106 | 0.66 | 10/10 pass | 1.965–2.160, all | 4 of 5 |
| 10 | 21.05 | 1.957 | 1.9612 | 1.9612 | 0.0265 | 1.03 | 10/10 pass | 1.802–2.151, all | 3 of 5 |
| 11 | 23.05 | 1.947 | 1.9465 | 1.9470 | 0.0582 | 1.28 | 10/10 pass | 1.857–2.155, all | 5 of 5 |
| 12 | 25.05 | 1.915 | 1.9033 | 1.9045 | 0.1169 | 1.48 | 10/10 pass | 1.793–2.103, all | 4 of 5 |
| 13 | 27.05 | 1.848 | 1.8201 | 1.8164 | 0.2152 | 1.62 | 10/10 pass | 1.654–2.141, all | 4 of 5 |
| 14 | 29.05 | 1.752 | 1.6765 | 1.6558 | 0.3746 | 1.73 | 10/10 pass | 1.664–2.116, all | 2 of 5 |
| 15 | 31.05 | 1.568 | 1.4451 | 1.3828 | 0.6248 | 1.79 | 10/10 pass | 1.614–2.118, all | 3 of 5 |

- At levels 9 and 10 the accepted Δ is larger than C10's. C10 fitted the
  span to the arc length. The real ramp starts at E of the longer arc a,
  so it has more room at those levels.
- The rule Δ is within 0.0013 mm of the accepted Δ at levels 9–12. There
  it is over c·D by at most 0.0002 mm, because `l_c` is read at a rise
  of 1e-4 mm. At levels 13–15 the rule is conservative by 0.004–0.062 mm.
- Lattice over the R bands: at cell steps the reading is 0.016–0.561 mm
  under the exact width, with 0 of 140 (band, phase) pairs refused. At
  pass steps it lies from 0.475 mm under to 0.281 mm over; 36 of 140
  pairs and 25 of 35 bands are refused. The refused step is a forward
  step in 22 pairs and a ramp step in 14.

## Cost (task 4)

The consistent sequence starts at level 8 (17.10 mm) and ends at 31.05
mm. Δ is the accepted Δ, interpolated in ρ and held to the law step.
Radii: 19.058, 21.019, 22.966, 24.873, 26.707, 28.429, 29.996, 31.050 mm.
That is 8 bands per arm (the last a 1.05 mm step) against 7 now (levels
9–15). All 80 band orientations pass the exact check. The prior arcs are
the earlier levels of the sequence, then law levels 7, 6 and 5.

| Arm | Now: cut (mm), 7 bands | Reserved: cut (mm), 8 bands | Extra (mm) | Per plunge removed (mm) |
|---|---|---|---|---|
| 0 | 64.2 | 151.4 | 87.2 | 12.45 |
| 1 | 72.1 | 169.3 | 97.2 | 13.89 |
| 2 | 72.5 | 170.2 | 97.8 | 13.97 |
| 3 | 72.5 | 170.8 | 98.4 | 14.06 |
| 4 | 72.1 | 170.2 | 98.1 | 14.02 |
| **All** | **353.3** | **832.0** | **478.7** | **13.68** |

- "Now" is the arc lengths of levels 9–15, each band cut once, with a
  wrap-end plunge per band (35 plunges). The seam travel of each plunge
  join (about Δ) is left out.
- "Reserved" is arcs 389.2 mm plus ramps and forward runs 442.8 mm, with
  0 plunges.
- **A plunge removed costs 13.7 mm of cut**: 1.0 mm from the extra
  passes the margin needs, and 12.7 mm from the ramp and the forward run
  (this form cuts each arc about twice).
- PREDICTION: 35 of the star's 58 wrap-end joins become lead-ins, so the
  star entries go from 73 to about 38. The harness lobe set differs from
  the model (6 lobes of 34 arcs against 5 of 44), so the harness count
  decides.
- The time cost of a plunge (retract, rapid, plunge feed) depends on the
  machine settings, which this note does not have. The operator compares
  13.7 mm of feed with one plunge.

## Verdict and the derived rule (task 5)

- **Exact geometry: viable at s 2 on the star**, with two changes to
  C10's form:
  1. The C13 tilt law `β(b) = min(β_C10(b), β_exact(b))`, both against
     the derived c·D. Here `β_exact` is the largest tilt with
     `R(sin φ_hi − sin φ_lo) ≤ c·D`, with `φ_lo = asin κ − β` and
     `φ_hi = min(π/2, π − asin κ − β)`, `κ = (A² − r² − R²)/(2rR)`,
     `r = ρ − Δ + b`, `A = ρ − Δ + R`.
  2. **The corner-touch spacing.** For each level, take the largest
     `Δ ≤ min(s, s'(ρ))` such that the C13-law ramp from E of arc k−1
     reaches full bite at least `l_c(ρ, Δ)` before F of arc k. Here
     `l_c` is the arc length from F within which a plain pass at Δ meets
     the corner stock that arc k−1 leaves at that wall. Take the
     shortest arc of the level (the levels are global).
- **The rule has no tuned number, but it is not a function of R, s, c·D
  and the cell alone.** `l_c` (0.66–1.79 mm on arm 0) depends on the
  angle at which the wall meets the front and on R and Δ. A rule from R,
  s, c·D and the cell only does not exist for this lead-in, because the
  binding failure is a wall feature. The cell does not enter the rule,
  because the lattice cannot hold the ramp anyway.
- **Planner: not viable now.** The ramp holds c·D with zero slack, and
  the planner lattice reads it with an error of −0.71 to +0.28 mm (G-
  CELLREAD). A replay would count most of these lead-ins as accepted
  (cell steps), including bands that are over the ceiling, and would
  refuse some correct ones (pass steps). To make the lattice usable, the
  ramp would have to keep a slack of about one quantum. That is a
  different law, and its spans would be much longer (NOT computed here).
- **Next, if the lead wants the evidence:** read 2–3 of the R bands in
  the simulator (exact stamping) with `print_exact_width_along_move`
  (C10's suggestion), before any Rust build.

## Risks and limits

- The model levels are exact circles about the model seed. The harness
  field is first order on the 0.5 mm lattice, so its arcs and lobes
  differ (C10: 6 lobes of 34 arcs against 5 of 44). The corner-touch
  result depends on the arc end geometry, which the lattice contours
  move by up to about one cell.
- The prior arcs in tables A, B and R are spaced at the band's own Δ'.
  The sequence (K) uses the real earlier levels, and it also passes.
- C10's meeting-line trims (`meeting_trim`) are not applied. The star
  arm arcs have one branch label, so no trim is expected there.
- The plain wall-arrival excess (2.25–2.70 mm exact) is a property of
  the current plain arcs too. It is not examined further here. It
  supports G-CELLREAD: the lattice reads those arrivals at 1.45–2.46 mm.
- The 1.5 mm column reads the same plan at the planner's walk step. It
  is a sensitivity case, not the harness replay. The harness replays the
  pieces at their own 0.5 mm vertices.

## Files

- `phase2/c13/geom.py`: the star, the ceiling, the C10 law and the C13
  law, the exact stock and width, and the lattice port.
- `phase2/c13/band.py`: one band (build, exact check, lattice reading).
- `phase2/c13/rule.py`: `corner_touch` (`l_c`) and a stand-alone
  bisection.
- `phase2/c13/final.py`: all the tables (writes `final.json`).
- `phase2/c13/report.py`, `phase2/c13/summary.py`: print the tables and
  the summary numbers from `final.json`.
- `phase2/c13/assert_c13.py`: asserts every number above. It recomputes
  the ceiling, the C10 spacings, the spans, the law check and the closed
  form, and it rebuilds one band of tables A and R.
