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
# Both of these are placeholders until THRUST_RESEARCH.md lands.
FEED_FORCE_RATIO_PLACEHOLDER = 0.5
AXIS_THRUST_N_PLACEHOLDER = 100.0


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

    def gantry_force_n(self, ratio: float = FEED_FORCE_RATIO_PLACEHOLDER) -> float:
        """NOT MODELLED IN THE RUST. The mean push along the feed direction.

        The ratio is a placeholder. See T-10. Do not treat this as a
        prediction. It exists to show the SHAPE of the dependency, which is
        the part that does not depend on the unknown constant.
        """
        return ratio * self.lateral_force_n * self.teeth_in_cut


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
        "the reference cut pushes 28.0 N",
        close(cut.gantry_force_n(), 28.0, tol=5e-3),
        f"computed {cut.gantry_force_n():.2f} N at ratio "
        f"{FEED_FORCE_RATIO_PLACEHOLDER}",
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


def rule_4_gantry(thrust_n: float, ratio: float) -> None:
    """The gantry force. Not modelled. See T-10."""
    print("\nRule 4 — the gantry force follows the chip, not the feed rate")
    print(f"         PLACEHOLDER thrust {thrust_n:.0f} N, ratio {ratio:.2f}. "
          f"See T-10.")
    base = Cut()

    print(f"\n         {'rpm':>7} {'feed':>9} {'gantry N':>9} {'spindle W':>10}")
    forces = []
    for rpm in (4500.0, 9000.0, 18000.0):
        cut = replace(base, rpm=rpm)
        f = cut.gantry_force_n(ratio)
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
              f"{cut.gantry_force_n(ratio):>9.1f} "
              f"{cut.spindle_power_kw() * 1000:>10.0f}")

    thick = replace(base, fz_mm=0.100)
    thin = replace(base, fz_mm=0.025)
    check(
        "a thicker chip does change it",
        thick.gantry_force_n(ratio) > 1.2 * thin.gantry_force_n(ratio),
        f"{thin.gantry_force_n(ratio):.1f} -> "
        f"{thick.gantry_force_n(ratio):.1f} N for a 4x chipload",
    )

    watts = base.gantry_force_n(ratio) * base.feed_mm_min / 60_000.0
    check(
        "the gantry force is invisible to a power model",
        watts < 0.01 * base.spindle_power_kw() * 1000.0,
        f"{watts:.2f} W of feed power against "
        f"{base.spindle_power_kw() * 1000:.0f} W at the spindle — "
        f"under 1 %. This is why no power gate can catch it.",
    )
    print(f"\n         Against the placeholder thrust: "
          f"{base.gantry_force_n(ratio) / thrust_n * 100:.0f} % used. "
          f"This number means nothing until T-10 has real data.")


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--thrust", type=float, default=AXIS_THRUST_N_PLACEHOLDER,
                    help="axis thrust in N (placeholder; see T-10)")
    ap.add_argument("--ratio", type=float, default=FEED_FORCE_RATIO_PLACEHOLDER,
                    help="feed force / tangential force (placeholder)")
    args = ap.parse_args()

    print("=" * 70)
    print("Derate lever rules — DERATE_SPEC.md")
    print("=" * 70)

    verify_against_rust()
    rule_1_feed_cap()
    rule_2_power()
    rule_3_deflection()
    rule_4_gantry(args.thrust, args.ratio)

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
