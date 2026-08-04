# ARP-1 reference measurements

numpy 2.4.2

## 1. Exact slope-band areas (spherical cap, R = 8, capped at 85 deg)

| band | closed form 2*pi*R^2*(cos t0 - cos t1) mm2 | numeric integral mm2 | rel err |
|---|---|---|---|
| Shallow 0-45 | 117.7794 | 117.7794 | 1.29e-12 |
| MidSteep 45-75 | 180.2672 | 180.2672 | 5.71e-13 |
| VerySteep 75-85 | 69.0299 | 69.0299 | 6.34e-14 |
| **total cap** | 367.0765 | | |

45 deg iso-slope circle at r = 5.656854 mm; 75 deg at r = 7.727407 mm (EXACT).

## 2. Exact tool-reach floors (what no algorithm can remove)

### Ball D1.0 (rho=0.5)

| feature | exact residual at centreline mm | enterable? |
|---|---|---|
| U groove R=0.2 | 0.158258 | no |
| U groove R=0.35 | 0.207071 | no |
| U groove R=0.5 | 0.000000 | YES |
| U groove R=0.8 | 0.000000 | YES |
| U groove R=1.5 | 0.000000 | YES |
| U groove R=3 | 0.000000 | YES |
| V groove half-angle 15 deg (flank 75 deg) | 1.431852 | never |
| V groove half-angle 30 deg (flank 60 deg) | 0.500000 | never |
| V groove half-angle 45 deg (flank 45 deg) | 0.207107 | never |

### Ball D3.0 (rho=1.5)

| feature | exact residual at centreline mm | enterable? |
|---|---|---|
| U groove R=0.2 | 0.186607 | no |
| U groove R=0.35 | 0.308595 | no |
| U groove R=0.5 | 0.414214 | no |
| U groove R=0.8 | 0.568858 | no |
| U groove R=1.5 | 0.000000 | YES |
| U groove R=3 | 0.000000 | YES |
| V groove half-angle 15 deg (flank 75 deg) | 4.295555 | never |
| V groove half-angle 30 deg (flank 60 deg) | 1.500000 | never |
| V groove half-angle 45 deg (flank 45 deg) | 0.621320 | never |

## 3. Micro-ripple bridging threshold (ball radius vs trough radius)

| lambda mm | amplitude mm | trough radius mm | max slope deg | D1.0 ball bridges? |
|---|---|---|---|---|
| 0.3 | 0.0300 | 0.0760 | 32.14 | YES (geometry, not defect) |
| 0.6 | 0.0600 | 0.1520 | 32.14 | YES (geometry, not defect) |
| 1.2 | 0.1200 | 0.3040 | 32.14 | YES (geometry, not defect) |
| 2.4 | 0.2400 | 0.6079 | 32.14 | no -- reaches bottom |
| 4.8 | 0.4800 | 1.2159 | 32.14 | no -- reaches bottom |

## 4. Tessellation error, MEASURED

Bound under test: for a piecewise-linear interpolant of a surface of
max principal curvature kappa sampled at chord length L, the sag is
`eps <= kappa L^2 / 8` along an edge and `<= kappa L^2 / 4` across the
diagonal of a square cell.  Pre-registered prediction: **err ~ s^2**,
i.e. a log-log slope of 2.00, and halving the step divides the error
by 4.0.

### Dome R=8, scored 0-40 deg (r <= 5.14)

kappa (analytic) = 0.1250 /mm

| XY step s mm | p50 um | p99 um | max um | bound kappa*s^2/4 um |
|---|---|---|---|---|
| 1.28500 | 48.7681 | 175.7510 | 259.0222 | 51.6008 |
| 0.64250 | 12.1276 | 42.3685 | 94.5548 | 12.9002 |
| 0.32125 | 3.0276 | 10.5594 | 30.7742 | 3.2250 |
| 0.16062 | 0.7567 | 2.6679 | 9.0723 | 0.8063 |
| 0.08031 | 0.1891 | 0.6633 | 2.2699 | 0.2016 |

**log-log slope = 2.009** (predicted 2.00); last halving divided p99 by 4.02 (predicted 4.00)

### Bowl R=8, scored 0-40 deg

kappa (analytic) = 0.1250 /mm

| XY step s mm | p50 um | p99 um | max um | bound kappa*s^2/4 um |
|---|---|---|---|---|
| 1.28500 | 48.7681 | 175.7510 | 259.0222 | 51.6008 |
| 0.64250 | 12.1276 | 42.3685 | 94.5548 | 12.9002 |
| 0.32125 | 3.0276 | 10.5594 | 30.7742 | 3.2250 |
| 0.16062 | 0.7567 | 2.6679 | 9.0723 | 0.8063 |
| 0.08031 | 0.1891 | 0.6633 | 2.2699 | 0.2016 |

**log-log slope = 2.009** (predicted 2.00); last halving divided p99 by 4.02 (predicted 4.00)

### Saddle rho=8

kappa (analytic) = 0.1250 /mm

| XY step s mm | p50 um | p99 um | max um | bound kappa*s^2/4 um |
|---|---|---|---|---|
| 2.25000 | 22.1483 | 74.7207 | 79.1011 | 158.2031 |
| 1.12500 | 5.5371 | 18.6802 | 19.7753 | 39.5508 |
| 0.56250 | 1.3843 | 4.6700 | 4.9438 | 9.8877 |
| 0.28125 | 0.3461 | 1.1675 | 1.2360 | 2.4719 |
| 0.14062 | 0.0865 | 0.2919 | 0.3090 | 0.6180 |

**log-log slope = 2.000** (predicted 2.00); last halving divided p99 by 4.00 (predicted 4.00)

### U groove R=1.5, |u| <= 1.0

kappa (analytic) = 0.6667 /mm

| XY step s mm | p50 um | p99 um | max um | bound kappa*s^2/4 um |
|---|---|---|---|---|
| 0.25000 | 4.7943 | 9.8114 | 9.8269 | 10.4167 |
| 0.12500 | 1.1922 | 2.7355 | 2.7460 | 2.6042 |
| 0.06250 | 0.2991 | 0.7190 | 0.7317 | 0.6510 |
| 0.03125 | 0.0747 | 0.1748 | 0.1893 | 0.1628 |
| 0.01562 | 0.0186 | 0.0443 | 0.0445 | 0.0407 |

**log-log slope = 1.955** (predicted 2.00); last halving divided p99 by 3.95 (predicted 4.00)

### U groove R=0.35, |u| <= 0.25

kappa (analytic) = 2.8571 /mm

| XY step s mm | p50 um | p99 um | max um | bound kappa*s^2/4 um |
|---|---|---|---|---|
| 0.06250 | 1.3180 | 2.9779 | 2.9805 | 2.7902 |
| 0.03125 | 0.3295 | 0.8518 | 0.8559 | 0.6975 |
| 0.01562 | 0.0824 | 0.2277 | 0.2320 | 0.1744 |
| 0.00781 | 0.0205 | 0.0555 | 0.0606 | 0.0436 |
| 0.00391 | 0.0051 | 0.0142 | 0.0142 | 0.0109 |

**log-log slope = 1.937** (predicted 2.00); last halving divided p99 by 3.93 (predicted 4.00)

### Ripple lambda=1.2

kappa (analytic) = 3.2899 /mm

| XY step s mm | p50 um | p99 um | max um | bound kappa*s^2/4 um |
|---|---|---|---|---|
| 1.00000 | 82.3103 | 223.6106 | 223.8585 | 822.4670 |
| 0.50000 | 36.6137 | 86.1525 | 86.1952 | 205.6168 |
| 0.25000 | 9.7788 | 24.5563 | 24.5763 | 51.4042 |
| 0.12500 | 2.4866 | 6.2954 | 6.3511 | 12.8510 |
| 0.06250 | 0.6372 | 1.5893 | 1.5984 | 3.2128 |

**log-log slope = 1.805** (predicted 2.00); last halving divided p99 by 3.96 (predicted 4.00)

### Ripple lambda=0.3

kappa (analytic) = 13.1595 /mm

| XY step s mm | p50 um | p99 um | max um | bound kappa*s^2/4 um |
|---|---|---|---|---|
| 0.25000 | 20.5776 | 55.9027 | 55.9646 | 205.6168 |
| 0.12500 | 9.1534 | 21.5381 | 21.5488 | 51.4042 |
| 0.06250 | 2.4447 | 6.1391 | 6.1441 | 12.8510 |
| 0.03125 | 0.6216 | 1.5739 | 1.5878 | 3.2128 |
| 0.01562 | 0.1593 | 0.3973 | 0.3996 | 0.8032 |

**log-log slope = 1.805** (predicted 2.00); last halving divided p99 by 3.96 (predicted 4.00)

## 5. Why a global XY grid is rejected: the steep-rim measurement

A height field's apparent second derivative on a sphere is
`d2z/dr2 = -R^2 / (R^2 - r^2)^{3/2}`, which DIVERGES at the rim even
though the surface curvature stays 1/R.  A uniform XY lattice must
therefore be refined without bound to hold a fixed sag on steep
ground; a uniform POLAR-ANGLE mesh holds it at constant cost.

| slope theta deg | height-field d2z/dr2 /mm | XY step for 1 um sag mm | arc step for 1 um sag mm |
|---|---|---|---|
| 0 | 0.1250 | 0.17889 | 0.25298 |
| 30 | 0.1925 | 0.14417 | 0.25298 |
| 45 | 0.3536 | 0.10637 | 0.25298 |
| 60 | 1.0000 | 0.06325 | 0.25298 |
| 75 | 7.2098 | 0.02355 | 0.25298 |
| 85 | 188.8087 | 0.00460 | 0.25298 |

| arc chord mm | meridian p50 um | p99 um | max um |
|---|---|---|---|
| 0.800 | 8.9244 | 41.7849 | 71.7516 |
| 0.400 | 2.2319 | 11.3103 | 21.8772 |
| 0.200 | 0.5569 | 2.7282 | 6.1123 |
| 0.100 | 0.1423 | 0.7175 | 1.6441 |
| 0.050 | 0.0356 | 0.1723 | 0.4264 |

**log-log slope = 1.982** (predicted 2.00), and the error is SLOPE-INDEPENDENT: the same chord holds 0 deg and 85 deg.

## 6. Grid-aliasing bound for a COLUMNS-class instrument

A column samples the surface at a FIXED lateral position.  Two arms
whose surfaces differ in phase relative to that lattice by up to one
cell read a Z difference of up to `cell * tan(theta)` from geometry
alone.  A bin narrower than this cannot separate arms on that slope.

| sim cell mm | Shallow (45 deg) um | MidSteep (75 deg) um | VerySteep (85 deg) um |
|---|---|---|---|
| 0.5 | 500 | 1866 | 5715 |
| 0.25 | 250 | 933 | 2858 |
| 0.1 | 100 | 373 | 1143 |
| 0.05 | 50 | 187 | 572 |
| 0.02 | 20 | 75 | 229 |
| 0.01 | 10 | 37 | 114 |

The prior campaign's +-10 um bin at a 0.25 mm cell is below this bound
by a factor of 25 on flat ground and 93 at 75 deg.  That is the
arithmetic behind 'sub-repeatability and grid-aliased'.

