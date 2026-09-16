# Thrust and feed-force research

Research date: 2026-09-16. Research only. No code reads and no code changes.

This file answers two questions:

1. How much feed force can a hobby or prosumer 3-axis router gantry apply
   before it loses position? The answer is a force in newtons.
2. What fraction of the tangential edge force acts along the feed direction?

**Read section 4 first if you only read one section.** The headline result is
that the current placeholder of 0.5 for the feed-force ratio is too low by
about a factor of two in the radial immersion range that matters most.

---

## 1. Rated axis thrust

### 1.1 Summary table

All figures are for one axis. A figure marked "2 motors" applies to a gantry
axis that has a motor at each end. "Stall" means the static holding limit. It
is not a usable machining limit. See section 1.4 for the derate.

| Machine class | Drive | Stall thrust (N) | Kind | Source |
|---|---|---|---|---|
| Shapeoko 3 / 4 / Pro, X axis | Belt, 20T GT2, 125 oz-in NEMA 23 | **132** | Derived | [wiki](https://wiki.shapeoko.com/index.php/Stepper_Motors) |
| Shapeoko 3, X axis, **measured skip** | Belt, stock motor, stock 9 mm belt | **80** (fast load) / **107** (slow load) | Forum measurement | [Carbide3D forum](https://community.carbide3d.com/t/belt-stretch-and-stepper-holding-measured/18480) |
| Shapeoko Pro, X axis, **measured skip** | Belt, stock motor, 15 mm belt | **85** | Forum measurement | [Carbide3D forum](https://community.carbide3d.com/t/holding-torque-of-so-pro-stepper-motors/35423) |
| Shapeoko Pro, Y axis, **measured skip** | Belt, 2 motors, mid travel | **187** | Forum measurement | [Carbide3D forum](https://community.carbide3d.com/t/holding-torque-of-so-pro-stepper-motors/35423) |
| Shapeoko Pro, Z axis, **measured skip** | Ballscrew | **231** | Forum measurement | [Carbide3D forum](https://community.carbide3d.com/t/holding-torque-of-so-pro-stepper-motors/35423) |
| X-Carve, stock | Belt, 20T GT2, 140 oz-in NEMA 23 | **148** | Derived | [Inventables](https://www.inventables.com/products/stepper-motor-nema-23) |
| X-Carve, upgrade motor | Belt, 20T GT2, 212 oz-in NEMA 23 | **223** | Derived | [Inventables](https://www.inventables.com/products/x-carve-nema-23-kit) |
| Shapeoko 5.1 Pro | Ballscrew 16 mm, **20 mm lead** | **565** | Derived, **motor torque assumed** | [Carbide3D specs](https://carbide3d.com/shapeoko/shapeoko5pro-specs/) |
| Onefinity X-35 | Ballscrew 1610, 1.26 N·m NEMA 23 | **713** | Derived | [Onefinity](https://www.onefinitycnc.com/product-page/stepper-motor-nema-23-178-4oz-in-for-x-and-y-x-35-rails) |
| Onefinity X-50 | Ballscrew 1610, 1.9 N·m NEMA 23 | **1074** | Derived | [Onefinity](https://www.onefinitycnc.com/product-page/stepper-motor-nema-23-high-torque-269oz-in-x-50-x-rails-pro-series-y-rails) |
| Onefinity X-50, 1616 option | Ballscrew 1616, 1.9 N·m NEMA 23 | **672** | Derived | [Onefinity forum](https://forum.onefinitycnc.com/t/x-50-1610-vs-1616-ball-screw/7362) |
| PrintNC | Ballscrew 1610 or 2010, 3.0 N·m NEMA 23 | **1696** | Derived, motor is a build choice | [PrintNC wiki](https://wiki.printnc.info/en/planning) |
| Avid CNC PRO, NEMA 23 | Rack and pinion, 3:1, 380 oz-in | **570** | Derived from the maker's own geometry | [Avid CNC](https://www.avidcnc.com/pro-rack-and-pinion-drive-nema-34-p-226.html) |
| Avid CNC PRO, NEMA 34 | Rack and pinion, 3.2:1, 960 oz-in | **1537** | Derived from the maker's own geometry | [Avid CNC](https://www.avidcnc.com/pro-rack-and-pinion-drive-nema-34-p-226.html) |
| Avid CNC PRO, NEMA 34, gantry (2 motors) | Rack and pinion, 3.2:1, 960 oz-in | **3075** (691 lbf) | Derived | as above |
| Avid CNC, NEMA 34 gantry, maker's claim | Rack and pinion, 2 drives | **"over 600 pounds" = >2670 N** | Maker's claim, repeated in forums | [CNCZone](https://www.cnczone.com/forums/avid-cnc/237006-cncrouterparts-34-nema-rack-pinion-drive.html) |
| Industrial router | Servo, rack and pinion | **not found** | — | see section 1.5 |

### 1.2 The spread

The spread across the classes is large. It is about 20:1 at stall.

- **Belt gantry: 130 N to 220 N at stall. 80 N to 110 N measured.**
- **Ballscrew gantry: 570 N to 1700 N at stall.** The lead dominates. The
  Shapeoko 5.1 Pro uses a 20 mm lead and lands at the bottom of this band. The
  PrintNC uses a 10 mm lead with a stronger motor and lands at the top.
- **Rack and pinion: 570 N (NEMA 23) to 1540 N (NEMA 34) per drive.**

A belt machine and a ballscrew machine do not land in the same place. The gap
is about a factor of 5 to 10.

### 1.3 The derivations

All derivations below are mine. No maker publishes a thrust figure.

**Belt drive.** The tooth pitch sets the pulley pitch radius.

    r = teeth × pitch / (2π)
    F = T × η / r

A 20-tooth GT2 pulley has a 2 mm pitch. Therefore r = 40 mm / 2π = 6.366 mm.
I assumed η = 0.95 for a toothed belt drive.

    Shapeoko 3: T = 125 oz-in = 0.883 N·m
    F = 0.883 × 0.95 / 0.006366 = 132 N

**Ballscrew.**

    F = 2π × T × η / lead

I assumed η = 0.90 for a ballscrew.

    Onefinity X-50, 1610: F = 2π × 1.9 × 0.90 / 0.010 = 1074 N
    Shapeoko 5.1 Pro:     F = 2π × 2.0 × 0.90 / 0.020 =  565 N

**Rack and pinion.** Avid CNC publishes the full drive geometry. The pinion is
20 pitch, so 20 teeth sit on a 1 inch pitch circle. Avid also publishes the
linear travel per motor revolution, which already includes the belt reduction.

    F = 2π × T × η / L

    NEMA 34: T = 960 oz-in = 6.779 N·m, L = 0.9817 in = 24.94 mm, η = 0.90
    F = 2π × 6.779 × 0.90 / 0.02494 = 1537 N per drive

Two drives give 3075 N, or 691 lbf. Avid claims "over 600 pounds" for the same
pair. My derivation and the maker's claim agree to about 13 percent. That
agreement raises my confidence in the rack and pinion row.

### 1.4 How far below stall is safe

This is the most useful part of section 1, and it rests on measurement rather
than on a rule of thumb.

One Shapeoko 3 owner pulled each axis against a fish scale until the motor
broke free. The same test ran ten times per axis.

- The X axis broke free at **18 lbf** when the load arrived in fast jerks.
- The same axis broke free at **24 lbf** when the load built up slowly.

Two conclusions follow.

**A dynamic load skips the motor at about 75 percent of the static threshold.**
18 / 24 = 0.75. A milling force is dynamic by nature, because each tooth enters
and leaves the cut.

**The measured static threshold is about 60 percent of the derived stall
figure.** 107 N measured against 132 N derived gives 0.81. 80 N measured
against 132 N derived gives 0.61. The Shapeoko Pro X axis gives 85 N against
132 N, which is 0.64. Belt stretch, wheel compliance and a driver current below
the motor rating all take a share.

**Therefore: use about 0.5 to 0.6 of the derived stall thrust as the usable
limit for a belt machine.** A single skipped step is unrecoverable, so the
margin must cover the worst tooth, not the mean.

Common sizing advice on the web repeats a 30 to 50 percent margin below the
pull-out curve. I could not trace that advice to an authoritative source, and I
do not rely on it. The measurement above says the same thing with evidence
behind it.

One caution, from a Shapeoko forum member who has measured belt tension on
these machines: holding torque is a static number, and stepper torque falls
fast as speed rises. A thrust budget built only on holding torque will be
optimistic at high feed rates.

### 1.5 What I could not find

- **No maker publishes a thrust or cutting-force rating.** Not Carbide 3D, not
  Onefinity, not Avid CNC, not Inventables. Every figure above except the Avid
  marketing claim is derived or measured by a third party.
- **Carbide 3D does not publish the Shapeoko 5 Pro motor torque.** A Carbide 3D
  staff member states on the company forum that "there is no documentation on
  the stepper motor specs". My 565 N row rests on an assumed 2.0 N·m. Treat
  that row as the weakest in the table.
- **I found no industrial router thrust figure.** Biesse, Homag and similar
  makers publish travel, power and speed. They do not publish an axis thrust. I
  will not guess an upper reference point.
- I found no independent skip-threshold measurement for a ballscrew or a rack
  and pinion machine. Only the belt machines have measurements.

---

## 2. The feed-force ratio

### 2.1 What the ratio depends on

The ratio is not a constant. It falls out of the milling geometry, and the
radial immersion drives it. I computed it from the standard peripheral milling
decomposition, using the `rs_cam` affine model as the edge force law.

Let φ be the immersion angle, measured from the axis normal to the feed. The
chip thickness is h(φ) = f_z·sin φ. The force on the tool is:

    F_t(φ) = K_s·h(φ) + F_edge        (per mm of axial depth)
    F_r(φ) = K_r·F_t(φ)
    F_feed(φ) = -( F_t·cos φ + F_r·sin φ )

I used `Ks = 49.95 N/mm²` and `F_edge = 5.30 N/mm`, the values already fitted in
this codebase. The engagement arc runs from 0 to arccos(1 - 2·ae/D) for
conventional cutting, and from π - arccos(1 - 2·ae/D) to π for climb cutting.

### 2.2 Summary table

The value is mean |F_feed| divided by mean F_t, at f_z = 0.2 mm.

| ae/D | Conventional, K_r=0.15 | Climb, K_r=0.15 | Conventional, K_r=0.30 | Climb, K_r=0.30 |
|---|---|---|---|---|
| 0.02 | 1.01 | 0.96 | 1.03 | 0.94 |
| 0.05 | 1.00 | 0.93 | 1.03 | 0.89 |
| 0.10 | 0.97 | 0.87 | 1.02 | 0.82 |
| 0.15 | 0.94 | 0.81 | 1.01 | 0.75 |
| 0.20 | 0.91 | 0.76 | 0.98 | 0.69 |
| 0.30 | 0.83 | 0.66 | 0.92 | 0.57 |
| 0.40 | 0.75 | 0.56 | 0.85 | 0.46 |
| 0.50 | 0.67 | 0.47 | 0.78 | 0.42 |
| 0.75 | 0.52 | 0.45 | 0.59 | 0.46 |
| 1.00 (slot) | 0.57 | 0.57 | 0.60 | 0.60 |

**Corrected 2026-09-16 — see section 2.7.** The four bottom rows first read
0.45 / 0.20 / 0.11 in the climb and slot cells. Those were SIGNED means. Above
about ae/D 0.3 the arc crosses 90 degrees, the feed force changes sign inside
one tooth pass, and a signed mean cancels against itself. The rows above are
the mean of the absolute value, which is what the heading says and what a
thrust budget needs. Every row at ae/D 0.3 and below is unchanged.

The sign matters as well as the magnitude.

- **Conventional cutting pushes the tool backward, against the feed.**
- **Climb cutting at low immersion pulls the tool forward, along the feed.**
  The axis must hold the tool back. This is the same effect that makes a climb
  cut grab a machine with backlash.

The peak value matters more than the mean for a stepper, because a skip is
driven by the worst tooth. Peak |F_feed| divided by peak F_t, at K_r = 0.15:

| ae/D | Conventional | Climb |
|---|---|---|
| 0.05 | 0.97 | 0.83 |
| 0.10 | 0.89 | 0.72 |
| 0.20 | 0.76 | 0.61 |
| 0.30 | 0.70 | 0.56 |
| 0.50 | 0.66 | 0.53 |
| 1.00 | 0.66 | 0.66 |

### 2.3 Is 0.5 defensible?

**No, not as a single constant.**

- 0.5 is **too low by a factor of about 1.6 to 2.0** for ae/D between 0.05 and
  0.20. That band is exactly where adaptive and trochoidal clearing runs, and
  it is where a hobby router removes most of its material.
- 0.5 is **too low at every immersion, including a slot.** The peak ratio never
  falls below 0.53 anywhere in the table. The first version of this section
  said 0.5 was about right for a slot. That rested on the signed-mean error
  corrected above. See section 2.7.

The real range of the peak ratio is **0.53 to 1.0**.

**Use the peak, not the mean.** A stepper skips on the worst tooth, not on the
average one. The peak table below is the one a thrust budget should read.

If the engine must keep one constant, **0.9** is the defensible choice for
low-immersion work and **0.66** is the floor for a slot. A ratio that varies
with the radial immersion is better than either, and the peak table gives it.

### 2.4 The radial coefficient for wood

The table needs K_r, the radial to tangential ratio. Wood is not metal here.

Cáceres, Uliana and Hernández measured orthogonal cutting of white spruce at
12 percent moisture content, at 1.0 mm depth, at four rake angles. They report
the parallel force F_P and the normal force F_N separately, in N/mm.

| Rake angle | F_P (N/mm) | F_N (N/mm) | K_r = F_N / F_P |
|---|---|---|---|
| 10° | 38 | 5.1 | 0.13 |
| 20° | 32 | 0.2 | 0.01 |
| 30° | 22 | 2.5 | 0.11 |
| 40° | 12 | 2.6 | 0.22 |

Source: peer-reviewed paper, *Wood and Fiber Science* 50(1) 2018, pages 55-65.
<https://wfs.swst.org/index.php/wfs/article/download/2630/2471/7217>

**K_r for clear softwood is below about 0.2.** Metal-cutting sources normally
use 0.3 to 0.5. Wood is lower, and the low value pushes the feed-force ratio
closer to 1.0 at low immersion. The wood case is therefore the demanding case,
not the easy one.

Caution: this is orthogonal cutting with a straight knife, not milling with a
helical router bit. It bounds K_r. It does not prove it.

Their F_P of 38 N/mm at 10° rake and 1.0 mm depth is a useful sanity check on
our own model. Our affine law gives 49.95 × 1.0 + 5.30 = 55.2 N/mm for the same
depth. The two agree to within a factor of 1.5. Spruce at 393 kg/m³ is lighter
than most species in the woodresearch.sk fit, so the direction of the
difference is expected.

### 2.5 A second wood dataset

Shen, Buck, Yuan and Zhu milled Mongolian Scots pine at 12,000 rpm with an 8 mm
two-flute end mill, in up-milling, over a full factorial of 81 runs. They
measured F_x along the feed and F_y across it with a Kistler 9257B dynamometer.
I parsed all 81 rows of their Table 3.

| Radial depth | ae/D | Median F_x / F_y | F_x range (N) |
|---|---|---|---|
| 0.50 mm | 0.062 | 1.80 | 7.5 to 46.2 |
| 1.25 mm | 0.156 | 2.01 | 14.2 to 64.7 |
| 2.00 mm | 0.250 | 2.00 | 16.8 to 75.8 |

Across all 81 runs: minimum 0.99, median 1.87, maximum 2.85.

Source: peer-reviewed paper, *Materials* 19(2) 439, doi 10.3390/ma19020439.
<https://www.ncbi.nlm.nih.gov/pmc/articles/PMC12842951/>

**The feed-direction force is about twice the cross-feed force in low-immersion
wood milling.** The feed axis carries the dominant lateral load. That supports
the conclusion in section 2.3.

This dataset agrees with my model in order of magnitude only. My model predicts
a peak F_x of 63 N where they measured 32 N, at 0.5 mm radial depth and 0.1 mm
feed per tooth. The factor of about two runs through the other conditions as
well. I do not treat this as a validation of `Ks`. Their tools use a 15° to 25°
rake and a 10° to 30° helix, and the helix smears the engagement over an 8 mm
axial depth, which the simple model does not capture.

### 2.6 The metal-cutting fallback

*Modern Machine Shop* states that the tangential force is 70 percent of the
total, the feed force is 20 percent and the radial force is 10 percent. That
implies a feed to tangential ratio of 0.29.

Source: trade magazine article, no material stated, no citation given.
<https://www.mmsonline.com/articles/a-new-milling-101-milling-forces-and-formulas>

**Do not use this figure.** It carries no material, no tool geometry and no
radial immersion. It appears to describe face milling, where many teeth engage
at once and the phase angles average out. It contradicts the two wood
measurements above and my own geometry calculation. I list it only because it
is widely quoted and someone will find it.

---

### 2.7 Verification by the requesting session

The session that commissioned this research re-derived both tables
independently, from the same force law but with its own integration code.

| What was checked | Result |
|---|---|
| Mean ratio table, ae/D 0.02 to 0.30, all four columns | Reproduces exactly |
| Mean ratio table, ae/D 0.50 to 1.00 | **Did not reproduce.** Corrected above |
| Peak ratio table, all twelve values | Reproduces exactly |
| Belt drive derivation, 20T GT2, 0.883 N.m | Reproduces: 132 N |
| Ballscrew derivation, 1.9 N.m, 10 mm lead | Reproduces: 1074 N |

The one defect is the signed mean. It does not touch the headline finding,
because that finding lives at low immersion where the arc never crosses 90
degrees. It does reverse one secondary claim, that 0.5 suits a slot.

The correction makes the conclusion stronger, not weaker. The placeholder of
0.5 is now too low across the whole immersion range, rather than too low only
in the adaptive band.

---

## 3. Sources

| Source | Kind | URL |
|---|---|---|
| Avid CNC PRO rack and pinion drive 2.0 | Maker specification, full drive geometry | <https://www.avidcnc.com/pro-rack-and-pinion-drive-nema-34-p-226.html> |
| Carbide 3D Shapeoko 5.1 Pro specifications | Maker specification, ballscrew only | <https://carbide3d.com/shapeoko/shapeoko5pro-specs/> |
| Onefinity NEMA 23 motor pages | Maker specification, torque only | <https://www.onefinitycnc.com/product-page/stepper-motor-nema-23-high-torque-269oz-in-x-50-x-rails-pro-series-y-rails> |
| Shapeoko wiki, stepper motors | Community wiki | <https://wiki.shapeoko.com/index.php/Stepper_Motors> |
| Belt stretch and stepper holding measured | Forum measurement, fish scale, Shapeoko 3 | <https://community.carbide3d.com/t/belt-stretch-and-stepper-holding-measured/18480> |
| Holding torque of SO Pro stepper motors | Forum measurement, fish scale, Shapeoko Pro | <https://community.carbide3d.com/t/holding-torque-of-so-pro-stepper-motors/35423> |
| Can anyone clarify the 5 Pro stepper specs | Forum, Carbide 3D staff confirm no spec exists | <https://community.carbide3d.com/t/can-anyone-clarify-the-stepper-motor-specs-of-the-5-pro/67685> |
| CNCZone, Avid NEMA 34 rack and pinion | Forum, repeats the maker's 600 lbf claim | <https://www.cnczone.com/forums/avid-cnc/237006-cncrouterparts-34-nema-rack-pinion-drive.html> |
| Cáceres et al 2018, white spruce orthogonal cutting | Peer-reviewed paper, wood | <https://wfs.swst.org/index.php/wfs/article/download/2630/2471/7217> |
| Shen et al, pine milling force, Materials 19(2) 439 | Peer-reviewed paper, wood, 81 runs | <https://www.ncbi.nlm.nih.gov/pmc/articles/PMC12842951/> |
| A New Milling 101 | Trade magazine, no material stated | <https://www.mmsonline.com/articles/a-new-milling-101-milling-forces-and-formulas> |
| PrintNC wiki, planning | Community wiki, no force figure found | <https://wiki.printnc.info/en/planning> |

Two further wood papers exist but block automated access. Somebody with
institutional access should read them:

- "Parallel and normal cutting forces in peripheral milling of wood",
  *European Journal of Wood and Wood Products*, 2003.
  <https://link.springer.com/article/10.1007/s00107-003-0427-0>
  This title is the closest match to what we need. I could not read it.
- "Comparative analysis of cutting forces in CNC milling of MDF",
  *Coatings* 14(9) 1085. <https://www.mdpi.com/2079-6412/14/9/1085>

---

## 4. Confidence, and what is still missing

### High confidence

- **The feed-force ratio is close to 1.0 at low radial immersion, not 0.5.**
  The geometry says so, the pine dataset agrees in direction, and the low wood
  K_r reinforces it. This is the finding with the most weight behind it.
- **Climb cutting at low immersion pulls the tool along the feed.** The axis
  resists a forward pull, not a backward push. The magnitude is close to the
  conventional case, but the sign is opposite.
- **K_r for clear softwood is below 0.2.** One peer-reviewed measurement, four
  rake angles, twenty replicates each.
- **The Avid rack and pinion derivation.** My derivation and the maker's own
  claim agree to 13 percent.

### Medium confidence

- **The belt machine thrust of 80 N to 110 N.** Two independent fish-scale
  measurements on two machine generations agree. Both come from one forum, and
  neither is a controlled experiment.
- **The 0.5 to 0.6 derate from stall.** It rests on three measured points
  against one derived point, on belt machines only.

### Low confidence

- **The Shapeoko 5.1 Pro row.** The motor torque is not published. I assumed
  2.0 N·m. Replace this row as soon as somebody measures a real machine.
- **The ballscrew rows in general.** The derivations are sound arithmetic, but
  no measurement checks them. A ballscrew machine may reach a structural limit,
  a frame deflection limit or a rail limit long before the motor skips. The
  Shapeoko 3 thread found that belt stretch, not the motor, set the practical
  stiffness limit. The same may hold for the frame on a ballscrew machine.

### Still missing

1. **A measured skip threshold for a ballscrew or a rack and pinion machine.**
   Every ballscrew and rack figure in this file is derived. Nobody has pulled
   an Onefinity or an Avid against a scale and published the result.
2. **The Shapeoko 5 Pro motor torque.** Carbide 3D states that no
   documentation exists.
3. **Any industrial router thrust figure.** No maker publishes one.
4. **A wood milling paper that decomposes into tangential and radial on the
   edge**, rather than into machine X and Y. The 2003 Springer paper is the
   likely candidate, and it is paywalled.
5. **A check on whether the frame, not the motor, sets the real limit.** The
   only evidence in this file is the Shapeoko belt-stretch thread, and it says
   the drive compliance dominated. Our thrust budget assumes the motor sets the
   limit. That assumption is untested above the belt machine class.
6. **A resolution of the factor-of-two gap** between our `Ks` value and the
   pine measurements. This is a question for the force model, not for the
   thrust budget, but the two interact. If `Ks` runs high by two, then the
   thrust budget also runs conservative by two.
