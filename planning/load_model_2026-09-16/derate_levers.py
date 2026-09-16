#!/usr/bin/env python3
"""Check that the derate lever rules in DERATE_SPEC.md hold.

This script re-implements the shipped load model in a few lines, then tests
each claim the specification makes. It is a sandbox, not a second source of
truth. The Rust code in `crates/rs_cam_core` is the source of truth. This
file exists so a rule can be disproved before anybody writes Rust for it.

Every constant below is copied from the Rust, with the file and the line.
If a constant changes in the Rust and not here, `verify_against_rust` fails.

Usage:
    python3 planning/load_model_2026-09-16/derate_levers.py
    python3 planning/load_model_2026-09-16/derate_levers.py --thrust 60
"""

from __future__ import annotations

import argparse
import math
from dataclasses import dataclass, replace

# --- Constants, copied from the Rust ---------------------------------------

# crates/rs_cam_core/src/feeds/force.rs
LIT_KS_N_PER_MM2 = 49.95
LIT_FEDGE_N_PER_MM = 5.30
LIT_ANCHOR_KC_N_PER_MM2 = 35.1

# crates/rs_cam_core/src/tool_load/power.rs:95
GRAIN_ANISOTROPY_FACTOR = 2.0

# crates/rs_cam_core/src/machine.rs:190-203 (Shapeoko, VFD)
SHAPEOKO_RATED_KW = 1.5
SHAPEOKO_RATED_RPM = 24000.0
SHAPEOKO_SAFETY = 0.80
# crates/rs_cam_core/src/machine.rs:215-225 (Makita, router)
MAKITA_KW = 0.71
MAKITA_SAFETY = 0.80

# NOT in the Rust. There is no gantry force model. See T-10.
# These come from THRUST_RESEARCH.md (2026-09-16), which is research, not a
# measurement of our own. Treat every number below as a ballpark.

# Axis thrust at the point the machine loses position, in newtons.
# "measured" means somebody pulled the axis against a scale until it skipped.
# "derived" means motor torque and drive geometry, with no measurement.
AXIS_THRUST_N = {
    "belt_measured":      (85.0, "Shapeoko Pro X axis, measured skip"),
    "belt_stall":        (132.0, "Shapeoko 3, derived stall — NOT usable"),
    "ballscrew_derived": (1074.0, "Onefinity X-50 1610, derived"),
    "rack_derived":      (1537.0, "Avid PRO NEMA 34, derived, one drive"),
}
# A skip happens well below stall. Three measured points against one derived
# point put the usable fraction at 0.5 to 0.6. Belt machines only.
SAFE_FRACTION_OF_STALL = 0.55

# The old placeholder, kept only so the checks can show what it got wrong.
SUPERSEDED_RATIO_PLACEHOLDER = 0.5


# --- The model --------------------------------------------------------------


@dataclass(frozen=True)
class Cut:
    """One cut. The units match the Rust: mm, mm/min, rpm, kW, N."""

    diameter_mm: float = 12.0
    flutes: int = 4
    ap_mm: float = 8.4          # axial depth
    ae_mm: float = 4.2          # radial width
    fz_mm: float = 0.0625       # chipload, mm per tooth
    rpm: float = 9000.0
    kc_n_per_mm2: float = 35.1  # GenericHardwood, the anchor wood

    @property
    def feed_mm_min(self) -> float:
        return self.fz_mm * self.flutes * self.rpm

    @property
    def coefficients(self) -> tuple[float, float]:
        """(Ks, F_edge) scaled off the anchor wood. force.rs affine_coeffs."""
        scale = self.kc_n_per_mm2 / LIT_ANCHOR_KC_N_PER_MM2
        return LIT_KS_N_PER_MM2 * scale, LIT_FEDGE_N_PER_MM * scale

    @property
    def psi_rad(self) -> float:
        """Immersion arc. force.rs immersion_angle: cos psi = 1 - ae/r."""
        radius = self.diameter_mm / 2.0
        if self.ae_mm <= 0.0 or radius <= 0.0:
            return 0.0
        return math.acos(max(-1.0, min(1.0, 1.0 - self.ae_mm / radius)))

    @property
    def teeth_in_cut(self) -> float:
        """The duty fraction, z * psi / 2pi. See T-7 — no helix wrap here."""
        return self.flutes * self.psi_rad / (2.0 * math.pi)

    @property
    def peak_chip_mm(self) -> float:
        """h_eff = fz * sin(theta_peak), theta_peak = min(psi, pi/2)."""
        return self.fz_mm * math.sin(min(self.psi_rad, math.pi / 2.0))

    @property
    def lateral_force_n(self) -> float:
        """F_lat = ap * (Ks * h + F_edge). Raw Kc: this is what bends the tool."""
        ks, f_edge = self.coefficients
        return self.ap_mm * (ks * self.peak_chip_mm + f_edge)

    def spindle_power_kw(self) -> float:
        """P = A * (Ks * MRR + F_edge * ap * Vc * duty) / 60e6."""
        ks, f_edge = self.coefficients
        cross_section = self.ap_mm * self.ae_mm
        shear_slope = GRAIN_ANISOTROPY_FACTOR * ks * cross_section / 60_000_000.0
        vc_mm_min = math.pi * self.diameter_mm * self.rpm
        edge = (
            GRAIN_ANISOTROPY_FACTOR
            * f_edge
            * self.ap_mm
            * vc_mm_min
            * self.teeth_in_cut
            / 60_000_000.0
        )
        return shear_slope * self.feed_mm_min + edge

    def feed_force_ratio(self, climb: bool = True, kr: float = 0.15) -> float:
        """Peak feed-direction force divided by peak tangential force.

        This is geometry, not a fitted constant. The feed force at edge angle
        phi is `-(F_t*cos(phi) + K_r*F_t*sin(phi))`. The ratio is the peak of
        that over the engagement arc, divided by the peak tangential force.

        The PEAK is the right statistic. A stepper skips on the worst tooth,
        not on the average one.

        `kr` is the radial-to-tangential ratio. For clear softwood it is below
        0.2 (Caceres 2018, white spruce, four rake angles). Metal uses 0.3 to
        0.5. The low wood value pushes this ratio TOWARD 1.0, so wood is the
        demanding case. See THRUST_RESEARCH.md section 2.4.
        """
        psi = self.psi_rad
        if psi <= 0.0:
            return 0.0
        lo, hi = (math.pi - psi, math.pi) if climb else (0.0, psi)
        peak_t = peak_f = 0.0
        ks, f_edge = self.coefficients
        steps = 400
        for i in range(steps + 1):
            phi = lo + (hi - lo) * i / steps
            ft = ks * self.fz_mm * math.sin(phi) + f_edge
            ff = -(ft * math.cos(phi) + kr * ft * math.sin(phi))
            peak_t = max(peak_t, ft)
            peak_f = max(peak_f, abs(ff))
        return peak_f / peak_t if peak_t > 0.0 else 0.0

    def gantry_force_n(self, ratio: float | None = None,
                       climb: bool = True) -> float:
        """NOT MODELLED IN THE RUST. The peak push along the feed direction.

        See T-10. Do not treat this as a prediction. The SHAPE of the
        dependency is sound; the magnitude rests on research, not on a
        measurement of our own machine.
        """
        r = self.feed_force_ratio(climb) if ratio is None else ratio
        return r * self.lateral_force_n * self.teeth_in_cut


def available_kw(model: str, rpm: float) -> float:
    """machine.rs power_at_rpm, times the safety factor."""
    if model == "vfd":
        rated = SHAPEOKO_RATED_KW * (min(rpm, SHAPEOKO_RATED_RPM) / SHAPEOKO_RATED_RPM)
        return rated * SHAPEOKO_SAFETY
    if model == "router":
        return MAKITA_KW * MAKITA_SAFETY
    raise ValueError(model)


def utilisation(cut: Cut, model: str) -> float:
    return cut.spindle_power_kw() / available_kw(model, cut.rpm)


# --- The checks -------------------------------------------------------------

FAILURES: list[str] = []


def check(name: str, condition: bool, detail: str = "") -> None:
    mark = "PASS" if condition else "FAIL"
    print(f"  [{mark}] {name}")
    if detail:
        print(f"         {detail}")
    if not condition:
        FAILURES.append(name)


def close(a: float, b: float, tol: float = 1e-6) -> bool:
    return abs(a - b) <= tol * max(1.0, abs(a), abs(b))


def verify_against_rust() -> None:
    """Pin two numbers this script must reproduce, or the copy has drifted.

    Both come from the Rust doc comments and from the session measurements
    quoted in DERATE_SPEC.md. If either moves, this file is stale.
    """
    print("\nModel agrees with the shipped Rust")
    cut = Cut()
    check(
        "the reference cut draws 538 W",
        close(cut.spindle_power_kw(), 0.538, tol=2e-3),
        f"computed {cut.spindle_power_kw() * 1000:.1f} W",
    )
    check(
        "the reference cut bends the tool with 69.5 N",
        close(cut.lateral_force_n, 69.5, tol=5e-3),
        f"computed {cut.lateral_force_n:.2f} N",
    )
    crossover = LIT_FEDGE_N_PER_MM / LIT_KS_N_PER_MM2
    check(
        "the crossover chip thickness is 0.106 mm",
        close(crossover, 0.1061, tol=1e-3),
        f"{crossover:.4f} mm — wood routing runs below this, so the edge "
        f"term dominates",
    )


def rule_1_feed_cap() -> None:
    """The feed cap binds: lower the RPM, hold the chip thickness."""
    print("\nRule 1 — at the feed cap, lower RPM to keep the chip thick")
    cap = 2000.0  # mm/min, below the reference cut's 2250
    base = Cut()

    truncated = replace(base, fz_mm=cap / (base.flutes * base.rpm))
    traversed = replace(base, rpm=cap / (base.fz_mm * base.flutes))

    check(
        "both answers obey the feed cap",
        close(truncated.feed_mm_min, cap) and close(traversed.feed_mm_min, cap),
    )
    check(
        "truncating the feed thins the chip",
        truncated.fz_mm < base.fz_mm,
        f"{base.fz_mm:.4f} -> {truncated.fz_mm:.4f} mm/tooth, toward the "
        f"rubbing floor",
    )
    check(
        "the traverse holds the chip thickness",
        close(traversed.fz_mm, base.fz_mm),
        f"{traversed.fz_mm:.4f} mm/tooth at {traversed.rpm:.0f} rpm",
    )


def rule_2_power() -> None:
    """Power binds. The right lever depends on the spindle."""
    print("\nRule 2 — a power limit answers to the spindle type")
    base = Cut()

    print("\n         Holding the chipload and lowering the RPM:")
    print(f"         {'rpm':>7} {'feed':>9} {'power kW':>9} "
          f"{'VFD %':>8} {'router %':>9}")
    vfd_utils = []
    for rpm in (9000.0, 6000.0, 4500.0, 3000.0):
        cut = replace(base, rpm=rpm)
        u_vfd = utilisation(cut, "vfd")
        u_router = utilisation(cut, "router")
        vfd_utils.append(u_vfd)
        print(f"         {rpm:>7.0f} {cut.feed_mm_min:>9.0f} "
              f"{cut.spindle_power_kw():>9.3f} {u_vfd * 100:>7.0f}% "
              f"{u_router * 100:>8.0f}%")

    spread = max(vfd_utils) - min(vfd_utils)
    check(
        "on a VFD the traverse changes nothing",
        spread < 0.02,
        f"utilisation spread is {spread * 100:.2f} % over a 3x RPM range — "
        f"the available power falls exactly as fast as the required power",
    )
    check(
        "on a router the traverse works",
        utilisation(replace(base, rpm=3000.0), "router")
        < 0.4 * utilisation(base, "router"),
        f"{utilisation(base, 'router') * 100:.0f} % -> "
        f"{utilisation(replace(base, rpm=3000.0), 'router') * 100:.0f} %",
    )

    print("\n         Holding the RPM and lowering the depth of cut:")
    print(f"         {'ap mm':>7} {'power kW':>9} {'VFD %':>8} {'router %':>9}")
    for ap in (8.4, 6.0, 4.0, 2.5):
        cut = replace(base, ap_mm=ap)
        print(f"         {ap:>7.1f} {cut.spindle_power_kw():>9.3f} "
              f"{utilisation(cut, 'vfd') * 100:>7.0f}% "
              f"{utilisation(cut, 'router') * 100:>8.0f}%")

    shallow = replace(base, ap_mm=2.5)
    check(
        "a shallower cut works on the VFD, where nothing else did",
        utilisation(shallow, "vfd") < 0.4 * utilisation(base, "vfd"),
        f"{utilisation(base, 'vfd') * 100:.0f} % -> "
        f"{utilisation(shallow, 'vfd') * 100:.0f} % at ap 8.4 -> 2.5 mm",
    )


def rule_3_deflection() -> None:
    """Deflection binds: thin the chip. A traverse does nothing."""
    print("\nRule 3 — deflection answers to the chip, not to the speed")
    base = Cut()
    forces = [replace(base, rpm=r).lateral_force_n
              for r in (3000.0, 9000.0, 18000.0)]
    check(
        "holding the chipload leaves the tool bending the same amount",
        close(min(forces), max(forces)),
        f"{forces[0]:.2f} N at every RPM from 3 000 to 18 000 — there is no "
        f"RPM term in the force",
    )
    thin = replace(base, fz_mm=0.025)
    check(
        "a thinner chip does reduce the force",
        thin.lateral_force_n < base.lateral_force_n,
        f"{base.lateral_force_n:.1f} -> {thin.lateral_force_n:.1f} N",
    )
    check(
        "but the force does not fall to zero, because of the edge term",
        replace(base, fz_mm=1e-9).lateral_force_n > 0.5 * thin.lateral_force_n,
        f"at a chipload of zero the force is still "
        f"{replace(base, fz_mm=1e-9).lateral_force_n:.1f} N — you must cut "
        f"shallower or narrower",
    )


def rule_4_gantry(gantry: str, climb: bool) -> None:
    """The gantry force. Not modelled in the Rust. See T-10."""
    stall, note = AXIS_THRUST_N[gantry]
    usable = stall * SAFE_FRACTION_OF_STALL
    print("\nRule 4 — the gantry force follows the chip, not the feed rate")
    print(f"         {note}: {stall:.0f} N, usable {usable:.0f} N at "
          f"{SAFE_FRACTION_OF_STALL:.0%} of it.")
    print(f"         Cut direction: {'climb' if climb else 'conventional'}. "
          f"Research figures, not our measurement. See T-10.")
    base = Cut()

    print(f"\n         {'rpm':>7} {'feed':>9} {'gantry N':>9} {'spindle W':>10}")
    forces = []
    for rpm in (4500.0, 9000.0, 18000.0):
        cut = replace(base, rpm=rpm)
        f = cut.gantry_force_n(climb=climb)
        forces.append(f)
        print(f"         {rpm:>7.0f} {cut.feed_mm_min:>9.0f} {f:>9.1f} "
              f"{cut.spindle_power_kw() * 1000:>10.0f}")

    check(
        "a 4x feed rate does not change the gantry force",
        close(min(forces), max(forces)),
        f"{forces[0]:.1f} N at 1 125 mm/min and at 4 500 mm/min",
    )

    print(f"\n         {'fz':>7} {'feed':>9} {'gantry N':>9} {'spindle W':>10}")
    for fz in (0.025, 0.0625, 0.100):
        cut = replace(base, fz_mm=fz)
        print(f"         {fz:>7.4f} {cut.feed_mm_min:>9.0f} "
              f"{cut.gantry_force_n(climb=climb):>9.1f} "
              f"{cut.spindle_power_kw() * 1000:>10.0f}")

    # Which dial actually moves the push? Cut each one to a third.
    print(f"\n         Each dial cut to a third of its value:")
    here = base.gantry_force_n(climb=climb)
    print(f"         {'dial':<26}{'gantry N':>9}{'change':>9}")
    print(f"         {'as it stands':<26}{here:>9.1f}{'':>9}")
    moved = {}
    for name, cut in (
        ("chipload", replace(base, fz_mm=base.fz_mm / 3.0)),
        ("depth of cut", replace(base, ap_mm=base.ap_mm / 3.0)),
        ("width of cut", replace(base, ae_mm=base.ae_mm / 3.0)),
    ):
        f = cut.gantry_force_n(climb=climb)
        moved[name] = f / here - 1.0
        print(f"         {name:<26}{f:>9.1f}{moved[name]:>8.0%}")

    check(
        "the depth of cut is the strongest dial on the gantry push",
        moved["depth of cut"] < moved["width of cut"] < moved["chipload"],
        f"depth {moved['depth of cut']:.0%}, width "
        f"{moved['width of cut']:.0%}, chipload {moved['chipload']:.0%}",
    )
    check(
        "thinning the chip barely moves the gantry push",
        abs(moved["chipload"]) < 0.15,
        f"a chipload cut to a third moves the push by "
        f"{moved['chipload']:.0%}. The edge term carries the load, and the "
        f"edge term has no chipload in it.",
    )
    zero_chip = replace(base, fz_mm=1e-9)
    check(
        "most of the push survives a chipload of zero",
        zero_chip.gantry_force_n(climb=climb) > 0.8 * here,
        f"{zero_chip.gantry_force_n(climb=climb):.1f} N of {here:.1f} N "
        f"= {zero_chip.gantry_force_n(climb=climb) / here:.0%} remains. "
        f"Depth and width are the levers, NOT chipload.",
    )

    watts = base.gantry_force_n(climb=climb) * base.feed_mm_min / 60_000.0
    check(
        "the gantry force is invisible to a power model",
        watts < 0.01 * base.spindle_power_kw() * 1000.0,
        f"{watts:.2f} W of feed power against "
        f"{base.spindle_power_kw() * 1000:.0f} W at the spindle — "
        f"under 1 %. This is why no power gate can catch it.",
    )


def rule_5_the_ratio_is_not_a_half() -> None:
    """The finding from THRUST_RESEARCH.md that moves the numbers."""
    print("\nRule 5 — the feed-force ratio is near 1.0, not 0.5")
    base = Cut()
    print(f"\n         {'ae/D':>6} {'conventional':>13} {'climb':>8}")
    worst = 1.0
    for frac in (0.05, 0.10, 0.20, 0.50, 1.00):
        cut = replace(base, ae_mm=frac * base.diameter_mm)
        conv = cut.feed_force_ratio(climb=False)
        clmb = cut.feed_force_ratio(climb=True)
        worst = min(worst, conv, clmb)
        print(f"         {frac:>6.2f} {conv:>13.2f} {clmb:>8.2f}")

    check(
        "the ratio never drops to the old 0.5 placeholder",
        worst > SUPERSEDED_RATIO_PLACEHOLDER,
        f"the lowest peak ratio at any immersion is {worst:.2f}, above "
        f"{SUPERSEDED_RATIO_PLACEHOLDER} — the placeholder was too low "
        f"everywhere, not just in the adaptive band",
    )
    adaptive = replace(base, ae_mm=0.10 * base.diameter_mm)
    r = adaptive.feed_force_ratio(climb=False)
    check(
        "in the adaptive band the placeholder was low by about 2x",
        1.6 < r / SUPERSEDED_RATIO_PLACEHOLDER < 2.1,
        f"at ae/D 0.10 the ratio is {r:.2f}, which is "
        f"{r / SUPERSEDED_RATIO_PLACEHOLDER:.2f}x the old placeholder",
    )


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--gantry", choices=sorted(AXIS_THRUST_N),
                    default="belt_measured", help="which axis thrust figure")
    ap.add_argument("--conventional", action="store_true",
                    help="conventional cutting (the default is climb)")
    args = ap.parse_args()

    print("=" * 70)
    print("Derate lever rules — DERATE_SPEC.md")
    print("=" * 70)

    verify_against_rust()
    rule_1_feed_cap()
    rule_2_power()
    rule_3_deflection()
    rule_4_gantry(args.gantry, not args.conventional)
    rule_5_the_ratio_is_not_a_half()

    print("\n" + "=" * 70)
    if FAILURES:
        print(f"{len(FAILURES)} CHECK(S) FAILED:")
        for name in FAILURES:
            print(f"  - {name}")
        return 1
    print("All checks pass. The rules in DERATE_SPEC.md are consistent.")
    print("Rule 4 stays unproven: its constants are placeholders. See T-10.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
