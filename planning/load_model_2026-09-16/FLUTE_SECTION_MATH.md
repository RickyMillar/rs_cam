# Flute section mathematics: does the end-mill bending fraction extend to other cutter shapes?

Date: 2026-09-17. Scope: the equivalent bending diameter `d_eq` of a fluted
cross-section, and whether one fraction table serves five cutter shapes.
Companion: `RESEARCH_CORE_DIAMETER.md` (literature survey, same folder).

## Summary

1. For any fluted section, `(d_eq / D)^4` equals the circumferential mean of
   `(rho(theta) / R)^4`. That result is exact and needs no flute profile.
2. Three or more flutes give an isotropic section. Two flutes do not: the
   published 0.889 hides an `I` swing of about 2.5 to 1, and the
   revolution-averaged compliance corresponds to `d_eq / D` of about 0.84,
   not 0.889.
3. The derivation reproduces the three Kivanc and Budak values only as
   "one flute shape, copied N times" (each flute removes 17 to 19 % of `I`).
   A real family (core rises with N, land angle per flute constant) gives the
   opposite trend: `d_eq / D` rises with N. The N-trend of the table is
   therefore a modelling artefact, not a general law.
4. The fraction transfers to a bull nose (identical section, derived) and to
   a ball nose (tip region contributes less than 1 % of compliance at
   `L/D >= 3`, derived bound). It transfers to a tapered ball at each local
   diameter, exact for a proportional grind and conservative otherwise.
5. A V-bit stays unmodelled: its flute geometry is not an end-mill flute, and
   no derivable or published fraction exists. Fraction 1.0 under-predicts the
   deflection by an unknown factor, `(1 / f_V)^4`.

---

## 1. Definitions

| Symbol | Meaning |
|---|---|
| `D`, `R = D/2` | cutting diameter and radius of the outline circle |
| `Rc`, `k = Rc / R` | core (root) radius and the core fraction |
| `N` | number of flutes |
| `rho(theta)` | radial extent of the section in the direction `theta` |
| `phi_f`, `phi_l` | angular width of one flute and of one land; `N (phi_f + phi_l) = 2 pi` |
| `beta = N phi_f / (2 pi)` | fraction of the circumference that is flute |
| `lambda = 1 - beta` | fraction of the circumference that is land |
| `I_0 = pi R^4 / 4 = pi D^4 / 64` | second moment of area of the solid circle |
| `f = d_eq / D` | equivalent-diameter fraction, `I = f^4 I_0` |

A flute is a groove cut into the outline circle. The section is star-shaped
about the axis: every ray from the axis leaves the section exactly once. That
holds for every fluted rotary tool, so `rho(theta)` is single-valued.

## 2. The exact section formula

### 2.1 Second moments in polar form

For a star-shaped region, `dA = r dr dtheta` and `y = r sin(theta)`:

```
I_xx = int_0^{2pi} int_0^{rho(theta)} r^2 sin^2(theta) r dr dtheta
     = (1/4) int_0^{2pi} rho(theta)^4 sin^2(theta) dtheta
I_yy = (1/4) int_0^{2pi} rho(theta)^4 cos^2(theta) dtheta
I_xy = (1/4) int_0^{2pi} rho(theta)^4 sin(theta) cos(theta) dtheta
```

Add the first two. `sin^2 + cos^2 = 1`, so the polar moment is

```
J = I_xx + I_yy = (1/4) int_0^{2pi} rho(theta)^4 dtheta
```

Divide by `2 I_0 = pi R^4 / 2`:

```
(I_xx + I_yy) / (2 I_0) = (1 / 2pi) int_0^{2pi} (rho / R)^4 dtheta  =:  m4
```

**Result A.** The arithmetic mean of `I_xx` and `I_yy`, as a fraction of the
solid-circle value, is the circumferential mean of `(rho / R)^4`. This holds
for every star-shaped section and every N. The centroid is on the axis for
`N >= 2` by rotational symmetry, so no parallel-axis term appears.

### 2.2 Symmetry: when the section is isotropic

The inertia tensor `[[I_xx, -I_xy], [-I_xy, I_yy]]` is symmetric. Split it
into an isotropic part `(J/2) * identity` and a deviatoric part with
components `(I_xx - I_yy)/2` and `I_xy`. Under a rotation by angle `alpha`
the deviatoric pair rotates by `2 alpha` (it is a rank-2 traceless tensor).
A section with N-fold symmetry is unchanged by `alpha = 2 pi / N`, so the
deviatoric pair is unchanged by a rotation of `4 pi / N`. A non-zero pair is
unchanged by that rotation only if `4 pi / N` is a multiple of `2 pi`, that
is, only if `N` divides 2.

**Result B.** For `N >= 3`, `I_xx = I_yy = J/2 = m4 I_0` and `I_xy = 0`
about every axis through the centre. The section bends the same in every
direction. For `N = 2` the section has two distinct principal values
`I_1 > I_2`, and `(I_1 + I_2) / 2 = m4 I_0`.

Consequence: for `N >= 3` the equivalent diameter is unique and is
`f = m4^(1/4)`. The whole question of "which I" exists only for two flutes.

### 2.3 The rotating two-flute section: which `I`?

Assumptions, each stated with its defence:

- The load `F` acts at the tip in a direction fixed in space (the surface
  normal). Defence: form error is the deflection toward the finished wall.
- The response is quasi-static. Defence: a router spindle turns at 100 to
  400 Hz; the first bending mode of a short carbide tool in a collet is in
  the kilohertz range, so the section rotates slowly against the beam's own
  dynamics.
- The beam has principal compliances `c_1 = L^3 / (3 E I_1)` and
  `c_2 = L^3 / (3 E I_2)` in the section's principal frame.

At rotation angle `psi` the compliance tensor in the space frame is
`C(psi) = Rot(psi) diag(c_1, c_2) Rot(psi)^T`. The deflection is
`delta = C(psi) F`. Its component along `F` and perpendicular to `F`:

```
delta_par  = F (c_1 cos^2(psi) + c_2 sin^2(psi))
delta_perp = F (c_1 - c_2) sin(psi) cos(psi)
```

Average over one revolution. `<cos^2> = <sin^2> = 1/2`, `<sin cos> = 0`:

```
<delta_par>  = F (c_1 + c_2) / 2 = F L^3 / (3 E I_h),   I_h = 2 I_1 I_2 / (I_1 + I_2)
<delta_perp> = 0
```

**Result C.** The revolution-averaged in-line deflection of a two-flute tool
corresponds to the **harmonic mean** `I_h` of the two principal values, not
the arithmetic mean `I_a = (I_1 + I_2) / 2`. The instantaneous value swings
between `L^3 / (3 E I_1)` and `L^3 / (3 E I_2)`. Because compliance is
`1 / I`, averaging the compliance is the physically correct operation for a
mean-load model.

Write `epsilon = (I_1 - I_2) / (I_1 + I_2)`. Then
`I_h = I_a (1 - epsilon^2)`, so the harmonic value is always lower.
The deflection ratio of the two choices is `I_a / I_h = 1 / (1 - epsilon^2)`.

### 2.4 Does the helix change the section?

The helical flute is a screw sweep of one profile. The section normal to the
axis is the same shape at every height, rotated by `psi(z) = psi_0 +
z tan(beta_h) / R`, where `beta_h` is the helix angle.

- For `N >= 3` the section is isotropic (Result B). The rotation changes
  nothing. The helix has no effect on `I`.
- For `N = 2` the principal axes rotate along the length. The tip deflection
  under a fixed-direction load is
  `delta = (F / E) int_0^L (L - z)^2 [cos^2(psi(z)) / I_1 + sin^2(psi(z)) / I_2] dz`.
  The weight `(L - z)^2` favours the root. The twist over a fluted length
  `L_f` is `2 (L_f / D) tan(beta_h)`: 2.3 rad for a 30 degree helix at
  `L_f / D = 2`, 3.5 rad at `L_f / D = 3`. Over a full turn the weighted
  `cos^2` tends to 1/2 and the result tends to Result C. Over a partial turn
  the result sits between the instantaneous extremes and `I_h`.

**Result D.** The helix does not alter the section property. It partly
averages the two-flute anisotropy along the length, toward the harmonic
mean. It does nothing for three or more flutes.

## 3. Flute profiles

Result A needs `rho(theta)`. Two profiles bracket a real groove.

**Profile 1, castellated.** `rho = R` on each land, `rho = Rc` across each
flute. This is the maximum material removal for a given core `k` and flute
width, because a real groove reaches `Rc` only at its deepest point.

**Profile 2, parabolic.** Across a flute, with `u` from -1 to +1,
`rho / R = g(u) = k + (1 - k) u^2`. The groove is tangent to the outline at
both edges and reaches `Rc` at its centre. This is closer to a ground groove
and removes less material than Profile 1.

Both profiles ignore relief on the land. Defence: a relief of a few degrees
lowers `rho` to about 0.97 R at the heel; `0.97^4 = 0.885`, so the land still
carries almost its full share. The neglect is small and goes the same way for
every shape.

Define the removal efficiency of one flute as the mean over the flute width
of `1 - (rho / R)^4`, called `eta(k)`. Then for `N >= 3`

```
f^4 = m4 = 1 - beta eta(k)              (Result E)
```

Closed forms, both hand-checkable:

```
eta_cast(k) = 1 - k^4
eta_par(k)  = 1 - [k^4 + (4/3) k^3 (1-k) + (6/5) k^2 (1-k)^2 + (4/7) k (1-k)^3 + (1/9) (1-k)^4]
```

The parabolic form comes from `(1/2) int_{-1}^{1} (k + (1-k) u^2)^4 du` with
`(1/2) int u^{2n} du = 1 / (2n + 1)`.

Hand check at `k = 0.6`: `0.1296 + 0.1152 + 0.0691 + 0.0219 + 0.0028 =
0.3387`, so `eta_par(0.6) = 0.661` against `eta_cast(0.6) = 0.870`.

Castellated two-flute closed forms, flutes centred at `+-90 degrees`,
`s = sin(pi beta) / pi`:

```
I_xx / I_0 = 1 - (1 - k^4) (beta + s)      (weak axis)
I_yy / I_0 = 1 - (1 - k^4) (beta - s)      (strong axis)
```

Derivation: on one flute arc of half-width `a = pi beta / 2` centred at
`pi/2`, `int sin^2 = a + sin(2a)/2`. Two arcs give `pi beta + sin(pi beta)`.
The full circle gives `pi`. Subtract, weight by `R^4` and `Rc^4`, divide by
`I_0`.

## 4. The corroboration test

### 4.1 What the three values constrain

By Result A the table fixes `m4` per N. By Result E, only the product
`beta eta` is constrained, never `beta` and `k` separately.

| N | `f` | `f^4 = m4` | removal `1 - m4` | removal per flute |
|---|---|---|---|---|
| 2 | 0.889 | 0.6246 | 0.3754 | 0.1877 |
| 3 | 0.841 | 0.5002 | 0.4998 | 0.1666 |
| 4 | 0.748 | 0.3130 | 0.6870 | 0.1717 |

The removal per flute is 0.17 to 0.19 for all three N. One flute shape,
copied N times, reproduces the table to within 12 %.

### 4.2 What Kivanc and Budak actually modelled

The thesis (Kivanc 2004, section 3.1.1, equations 3.6 to 3.14) builds the
section from N identical regions. Each region is bounded by the pitch lines
and one off-centre arc of radius `r` and centre offset `a` (equation 3.6,
3.10, 3.13), with a semicircular notch of size `fd` subtracted (the terms
`-(1/8) pi (fd/2)^4` and `pi (fd/2)^2 / 2 * (...)^2` in equations 3.7, 3.11,
3.14). The text says "the flute depth, fd, is in general different for
different end mill generation" but prints no value of `fd`, `r` or `a`, for
any N. Table 3.2 gives deflections only.

So the table is "one region shape, N copies". The derivation here reproduces
that family, and Section 4.1 shows the per-flute removal is nearly constant.
**That is a consistent family in the model's own terms.** The residual is
the 12 % spread of per-flute removal (0.167 to 0.188).

### 4.3 Is that family physically sensible?

The implied geometry for a given core, from Result E:

| profile | `k` | `eta` | N=2 land per flute | N=3 land per flute | N=4 land per flute |
|---|---|---|---|---|---|
| castellated | 0.55 | 0.908 | 106 deg | 54 deg | 22 deg |
| castellated | 0.65 | 0.821 | 98 deg | 47 deg | 15 deg |
| parabolic | 0.55 | 0.702 | 84 deg | 35 deg | 2 deg |
| parabolic | 0.60 | 0.661 | 78 deg | 29 deg | impossible (`beta > 1`) |
| parabolic | 0.65 | 0.614 | 70 deg | 22 deg | impossible |

Hand check, castellated, `k = 0.55`, N=4: `k^4 = 0.0915`, `eta = 0.9085`,
`beta = 0.6870 / 0.9085 = 0.756`, `lambda = 0.244`, land per flute
`= 0.244 * 360 / 4 = 22.0 deg`.

The implied core for a constant land fraction:

| `lambda` | profile | N=2 `k` | N=3 `k` | N=4 `k` |
|---|---|---|---|---|
| 0.2 | castellated | 0.85 | 0.78 | 0.61 |
| 0.3 | castellated | 0.83 | 0.73 | 0.37 |
| 0.3 | parabolic | 0.72 | 0.53 | impossible |

Both tables say the same thing. To hit 0.748 at four flutes with a realistic
groove, the section must have almost no land, or a core below 0.4 D. To hit
0.889 at two flutes, the land must cover more than half the circumference,
or the core must exceed 0.7 D. The table therefore implies:

- the land fraction falls steeply as N rises, or
- the core falls as N rises.

Real tools do the opposite. The core rises with N (`RESEARCH_CORE_DIAMETER.md`
section 2a: two-flute 0.5 to 0.6 D, four-flute 0.6 to 0.7 D), and the land
angle per flute is set by the relief grind, so the total land fraction rises
with N.

Put that real family into Result E. With a constant land angle `phi_l` per
flute, `lambda = N phi_l / (2 pi)` rises with N, and `eta(k_N)` falls as
`k_N` rises. Both factors in `f^4 = 1 - (1 - lambda) eta(k_N)` push `f` up.
**In the real family `f` rises with N.** No constant is needed for that
sign; it follows from the two monotone trends. An illustration with
`phi_l = 30 deg` and `k = 0.55, 0.60, 0.65` gives `f = 0.70, 0.77, 0.82`
(castellated) or `0.80, 0.84, 0.88` (parabolic). The values are
illustrative; the direction is derived.

### 4.4 Verdict on the corroboration

- The derivation reproduces the table, but only under the same "one flute,
  N copies" family that the thesis uses. It does so with a per-flute removal
  of 0.17 to 0.19 and a 12 % residual.
- That family is not how end mills are made. Under a realistic family the
  trend reverses. The three values are therefore **valid as three separate
  literature points for the tools Kivanc modelled**, and **not valid as a
  law "more flutes, smaller `d_eq`"**.
- The mathematics available here cannot replace the table with derived
  values, because Result E needs `beta` and `k` per tool and there is no way
  to measure them in this project.

### 4.5 Literature cross-check

| Source | Figure | Relation to this derivation |
|---|---|---|
| Kops and Vo 1990, Annals of the CIRP 39(1):93-96 | `d_eq` about 0.80 D for two-flute and four-flute tools, from compliance | Flat across N. Consistent with the "real family" argument of 4.3 where the two trends nearly cancel; inconsistent with a strong N-trend. Abstract only. |
| Inscribed-square rule (`RESEARCH_CORE_DIAMETER.md` 1f) | 0.807 for four flutes | A worst case with flutes cut to the centre on both axes. Result A reproduces it: a square inscribed in the circle has `m4 = 64 / (48 pi) = 0.4244`, `f = 0.807`. |
| Nemes, Asamoah-Attiah, Budak 2001, Annals of the CIRP 50(1):65-68 | Section properties from the same off-centre-arc model | The arc model that Kivanc cites (thesis, equation 3.6 attribution). No `d_eq` figure could be read; cited as the origin of the region model only. |
| Schmitz and Smith, *Machining Dynamics*, Springer (2009; 2nd ed. 2019), tool-point RCSA chapter | Fluted portion replaced by an equivalent-diameter beam | Corroborates the method, not a figure. Their `d_eq` follows Kops and Vo. |

No source found gives an equivalent diameter for a ball nose, a bull nose, a
tapered tool or a V-bit.

### 4.6 The two-flute swing, quantified

Fit the castellated and parabolic profiles to `m4 = 0.6246` at N=2 and
compute the principal values by the closed forms of Section 3 (parabolic
case by numerical quadrature of Result A):

| profile | `k` | `beta` | flute width | `I_xx / I_0` | `I_yy / I_0` | ratio | `f_a` | `f_h` | `f_min` | `f_max` | deflection `I_a / I_h` |
|---|---|---|---|---|---|---|---|---|---|---|---|
| castellated | 0.50 | 0.400 | 72 deg | 0.341 | 0.909 | 2.67 | 0.889 | 0.839 | 0.764 | 0.976 | 1.26 |
| castellated | 0.55 | 0.413 | 74 deg | 0.346 | 0.903 | 2.61 | 0.889 | 0.841 | 0.767 | 0.975 | 1.25 |
| castellated | 0.60 | 0.431 | 78 deg | 0.354 | 0.895 | 2.53 | 0.889 | 0.844 | 0.771 | 0.973 | 1.23 |
| parabolic | 0.50 | 0.510 | 92 deg | 0.353 | 0.896 | 2.54 | 0.889 | 0.844 | - | - | 1.23 |
| parabolic | 0.55 | 0.535 | 96 deg | 0.360 | 0.889 | 2.47 | 0.889 | 0.846 | - | - | 1.22 |
| parabolic | 0.60 | 0.568 | 102 deg | 0.371 | 0.878 | 2.37 | 0.889 | 0.850 | - | - | 1.20 |

Hand check, castellated, `k = 0.55`: `1 - k^4 = 0.9085`; `pi beta = 1.298`;
`sin = 0.9631`; `s = 0.3065`; `I_xx / I_0 = 1 - 0.9085 (0.4132 + 0.3065) =
0.3461`; `I_yy / I_0 = 1 - 0.9085 (0.4132 - 0.3065) = 0.9031`;
`I_h = 2 * 0.3461 * 0.9031 / 1.2492 = 0.5004`; `f_h = 0.5004^0.25 = 0.841`.

The result is insensitive to the profile and to `k`: the principal ratio is
2.4 to 2.7 and `f_h` is 0.84 to 0.85. If 0.889 is read as the arithmetic
mean, a mean-load model that uses 0.889 under-predicts the revolution-averaged
deflection by 20 to 26 %. If 0.889 is instead the stiff-direction value from
one FEA load case (the thesis does not say which), the true mean is lower
still. The value 0.841 also happens to equal the published three-flute
figure; that is a coincidence of this fit, not a rule.

## 5. The other cutter shapes

The compliance of a cantilever with a tip load is

```
delta / F = (1 / E) int_0^L h^2 / I(h) dh,      h = height above the tip
```

For a uniform fluted cylinder, `I = f^4 I_0` and `delta / F = L^3 / (3 E f^4 I_0)`.
A region of height `H` at the tip contributes a share `(H / L)^3` of that
total when its `I` equals the cylinder's. Every bound below uses this
integral.

### 5.1 Bull nose

A bull nose is a fluted blank with a corner radius `r_c` ground on the end.
The fluting pass precedes the end grind, so above `h = r_c` the section is
the same fluted circle as the square end mill of the same `D`, `N` and flute
style. That is identity by construction, not approximation.

Below `h = r_c` the outline radius is `r(h) = (R - r_c) + sqrt(r_c^2 - (r_c - h)^2) >= R - r_c`.
With the same fraction, `I(h) >= f^4 (pi/4) (R - r_c)^4`, so the corner
region's share of compliance is at most `(r_c / L)^3 (R / (R - r_c))^4`.
For `r_c = 0.1 D`, `L = 4 D`: `(0.025)^3 * (1 / 0.8)^4 = 3.8e-5`.

**Recommendation: apply the same fraction as a square end mill of the same
`D` and `N`, over the full flute length.** DERIVED. Confidence high.

### 5.2 Ball nose

Above `h = R` the same argument holds: same fluted circle, same fraction.

In the ball region `0 <= h <= R` the outline radius is
`r(h)^2 = 2 R h - h^2` and the flutes run to the centre, so the local core
fraction is not the straight tool's `k`. Write the local fraction as
`c_b(h) = I(h) / ((pi/4) r(h)^4)`. The land material alone keeps `c_b`
above zero, so `c_b >= c_min > 0`. The ball region's share of the whole
compliance, relative to a uniform fluted cylinder of fraction `c = f^4`, is

```
S_ball = (3 c / c_b) (R^4 / L^3) int_0^R dh / (2R - h)^2 = (3/2) (c / c_b) (R / L)^3
```

The integrand `h^2 / I(h)` stays finite at the tip because `r^4 ~ h^2` and
`h^2 / h^2 = 1`. Hand check of the integral: `[1 / (2R - h)]_0^R = 1/R - 1/(2R) = 1/(2R)`.

| `L / D` | `(R/L)^3` | share, `c_b = c` | share, `c_b = c / 3` |
|---|---|---|---|
| 1.5 | 0.0370 | 5.6 % | 16.7 % |
| 2 | 0.0156 | 2.3 % | 7.0 % |
| 3 | 0.0046 | 0.7 % | 2.1 % |
| 4 | 0.0020 | 0.3 % | 0.9 % |
| 5 | 0.0010 | 0.15 % | 0.45 % |

The fluted-circle model stops being true inside the ball, but the ball
contributes under 1 % of the compliance at `L / D >= 3`, and under 7 % at
`L / D = 2` even if the local fraction falls to a third of the straight
value. Compare that with the 20 to 60 % uncertainty of the fraction itself.
A load applied on the ball rather than at the tip shortens the arm and
lowers these shares further.

**Recommendation: apply the straight fraction to the local diameter over the
whole flute, ball included.** The ball-region error is bounded by the table
above and goes toward under-predicting the deflection. DERIVED bound.
Confidence high for `L / D >= 3`, medium below.

### 5.3 Tapered ball

The fraction is dimensionless. By Result E it applies at each height if and
only if the dimensionless section (`beta`, `k`, profile) is the same at
each height. The taper itself does not change `beta` or the profile; the
question is `k(z)`.

Two grinds bound the truth:

- **(i) Proportional grind.** The core tapers with the outline, `k` constant.
  The fraction is exactly the straight value at every height.
- **(ii) Constant flute depth `d_f`.** Then `k(z) = 1 - d_f / r(z)` rises
  toward the shank.

Which one is real follows from what the tool must do, not from measurement.
The flute must exist at the tip, so `d_f <= r_tip`. The flank of a tapered
tool cuts along its whole length (that is the purpose of the taper in 3D
carving), so it needs chip room along the whole flute. Under grind (ii) the
groove at the large end is at most `r_tip` deep on a body several times
larger, which leaves no chip room. Under grind (i) the chip room scales with
the local radius. A functional tapered flute is therefore proportional or
close to it. Grind (ii) is the conservative bound.

Illustration (assumed numbers, labelled): tip diameter 1.0 mm, half-angle
3 deg, flute length 20 mm, so the large-end radius is
`0.5 + 20 tan(3 deg) = 1.548 mm`. Castellated profile with `beta = 0.6`.
Grind (i) with `k = 0.4`: `f = (1 - 0.6 (1 - 0.4^4))^0.25 = 0.803` everywhere.
Grind (ii) with `d_f = 0.3 mm`: at the large end `k = 1 - 0.3 / 1.548 =
0.806`, `f = 0.899`. Applying the grind (i) value there over-predicts the
deflection by `(0.899 / 0.803)^4 = 1.57`. The compliance weight `h^2` is
largest at the large end, so this is where the difference counts.

**Recommendation: apply the straight fraction for the tool's `N` to the
local diameter at each height.** Exact under a proportional grind; under a
constant-depth grind it over-predicts deflection (conservative), by up to
about 1.6x at the large end in the illustration. Most tapered ball tools
have two flutes, so Result C applies: the revolution-averaged compliance
matches `f_h`, not `f_a`. DERIVED under a stated grind assumption.
Confidence medium.

The ball tip of a tapered ball follows Section 5.2 with `R = r_tip`, and
`r_tip / L` is tiny, so its share is negligible.

### 5.4 V-bit

A V-bit is a cone of half-angle 30 to 60 deg with flutes ground in. Its
cone height is `R / tan(half-angle)`: `R` for a 90 deg bit, 1.73 R for
60 deg, 0.58 R for 120 deg. The section at height `h` is a circle of radius
`h tan(half-angle)` with the flutes.

The fluted-circle model breaks down for three reasons:

1. **The flute is not an end-mill flute.** Common V-bits are one-flute or
   two-flute, straight or low-helix, and many are steel bodies with two
   brazed carbide faces. Their section is a chisel-like shape whose land and
   core fractions bear no relation to the Kivanc regions.
2. **A one-flute section has an off-axis centroid.** The neutral axis does
   not pass through the rotation axis, which adds a coupling term that a
   single `I` cannot represent.
3. **The load sits on the cone, not at the tip.** Below the engagement the
   cone carries no moment. The compliance integral runs from the engagement
   height `h_F` to the cone base, `int_{h_F}^{h_c} (h - h_F)^2 / I(h) dh`,
   and the weakest section is the one just above the cut. A fraction there
   multiplies the deflection by `(1 / f_V)^4` directly; it is first order,
   not a tip correction.

Nothing in the literature read here gives `f_V`, and no geometry argument
fixes `beta` or `k` for a V-bit. A borrowed end-mill value would be a
fabricated constant.

**Recommendation: leave the V-bit section factor unmodelled, fraction 1.0,
and mark it as such in the model output.** Cost: the model under-predicts
the deflection of the fluted cone by the unknown factor `(1 / f_V)^4`. For
scale only, the end-mill fractions would give 1.6x (0.889) to 3.2x (0.748).
The direction is non-conservative. The cone taper and the load height, which
the stepped cantilever already handles exactly, remain the larger effect for
shallow V-carving. UNMODELLED. Confidence: none claimed.

## 6. The practical rule

| Cutter shape | Rule for the cutting region | Basis | Confidence |
|---|---|---|---|
| Square end mill | `d_eq = f(N) * D` with the three published values | LITERATURE (Kivanc 2004, Table 3.2, derived in `RESEARCH_CORE_DIAMETER.md`). The N-trend is a fixed-flute artefact (Section 4.3); Kops and Vo's flat 0.80 is the counter-evidence. | medium per value; low for the N-trend |
| Bull nose | same as square end mill, over the full flute length | DERIVED: identical section above `r_c`; corner share `< 1e-4` | high |
| Ball nose | `f(N)` on the local diameter, ball included | DERIVED bound: ball share `(3/2)(c/c_b)(R/L)^3`, under 1 % at `L/D >= 3` | high (`L/D >= 3`), medium below |
| Tapered ball | `f(N)` on the local diameter at each height | DERIVED under a proportional grind (argued from function); conservative under a constant-depth grind | medium |
| V-bit | fraction 1.0, flagged unmodelled | UNMODELLED; error non-conservative by `(1/f_V)^4` | none |
| Any two-flute tool, mean-load model | use `f_h`, about 0.84 to 0.85, in place of 0.889, or carry both principal values | DERIVED (Result C, Section 4.6); depends on reading 0.889 as the arithmetic mean | medium |
| More than four flutes | out of scope; no published value | - | - |

What is derived, what is literature, what is not modelled:

- DERIVED: Results A to E; the isotropy of `N >= 3`; the harmonic-mean rule
  for `N = 2`; the two-flute swing of 2.4 to 2.7 in `I`; the bull-nose
  identity; the ball-region bound; the tapered-ball transfer rule and its
  error direction; the sign of the real-family trend.
- LITERATURE: the three fractions themselves; Kops and Vo's 0.80; the
  Kivanc region model.
- UNMODELLED: the V-bit section; every flute count above four; the actual
  `beta` and `k` of any specific tool.

The honest headline: **the straight-end-mill table transfers to bull nose,
ball nose and tapered ball as a matter of geometry, but the table's own
N-trend is not general, and the table cannot be extended to V-bits on the
mathematics available.**

## 7. Reproduction

Every value in Sections 4 and 5 follows from the closed forms in Sections 2,
3 and 5 with a calculator. The parabolic two-flute rows of Section 4.6 use a
midpoint quadrature of Result A with 200 000 steps over the circle; the
following fragment reproduces them:

```python
import math
def n2_par(k, beta, n=200000):
    a = math.pi * beta / 2
    sxx = syy = 0.0
    for i in range(n):
        th = 2 * math.pi * (i + 0.5) / n
        d = min(abs(((th - math.pi/2 + math.pi) % (2*math.pi)) - math.pi),
                abs(((th + math.pi/2 + math.pi) % (2*math.pi)) - math.pi))
        rho = k + (1 - k) * (d / a) ** 2 if d < a else 1.0
        sxx += rho**4 * math.sin(th)**2
        syy += rho**4 * math.cos(th)**2
    dth = 2 * math.pi / n
    return sxx * dth / math.pi, syy * dth / math.pi   # I_xx/I_0, I_yy/I_0
```

## Sources

- Kivanc, E. B., 2004, "Modeling Statics and Dynamics of Milling Machine
  Components", MSc thesis, Sabanci University. Section 3.1.1 (equations 3.1
  to 3.14, the region model), Table 3.1 (E values), Table 3.2 (deflections).
  `https://research.sabanciuniv.edu/id/eprint/8159/1/kivancevrenburcu.pdf`
- Kivanc, E. B. and Budak, E., 2004, "Structural modeling of end mills for
  form error and stability analysis", *Int. J. Machine Tools and Manufacture*
  44(11):1151-1161. `https://www.sciencedirect.com/science/article/abs/pii/S0890695504000896`
- Kops, L. and Vo, D., 1990, "Determination of the Equivalent Diameter of an
  End Mill Based on Its Compliance", *Annals of the CIRP* 39(1):93-96.
  Abstract only; full text returned HTTP 403.
  `https://www.semanticscholar.org/paper/f7d5fc781a29b7c9b3ed204c3ca8f8cec398d8bc`
- Nemes, J. A., Asamoah-Attiah, S. and Budak, E., 2001, "Cutting Load
  Capacity of End Mills with Complex Geometry", *Annals of the CIRP*
  50(1):65-68. Cited by the thesis as the origin of the off-centre-arc
  region model; not read in full.
- Schmitz, T. L. and Smith, K. S., *Machining Dynamics: Frequency Response
  to Improved Productivity*, Springer, 2009 (2nd ed. 2019), chapter on
  receptance coupling for tool-point dynamics. Method corroboration only.
- `planning/load_model_2026-09-16/RESEARCH_CORE_DIAMETER.md`, 2026-09-17:
  the derivation of the three fractions from Table 3.2, the core-diameter
  survey (section 2a) and the inscribed-square rule (section 1f).
