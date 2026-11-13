//! P20b: the one vertical-rate limit of a plunge, a drill feed and the
//! vertical part of a ramp (`planning/feeds_contract_2026-11-03/phases/P20.md`
//! audit row 18).
//!
//! Two limits bound a vertical (Z) rate:
//!
//! - the G6 axial chip: `axial chip x rpm x Z` ([`AxialChipBound`]; report
//!   02 F-c1), where a G6 claim covers the tool (`RampBasis::Sourced`);
//! - the machine Z axis rate: the third value of
//!   `MachineKinematics::max_rate_xyz_mm_min` (GRBL `$112`). Above it the
//!   machine runs slower than the card says. A machine that states no
//!   per-axis rates has no Z cap, and the card says so ([`ZAxisRate`]).
//!
//! [`VerticalRateLimit`] holds both. Its fields are private and
//! [`VerticalRateLimit::new`] is its one constructor, so every producer
//! reads the same two limits. [`VerticalRateLimit::bound`] takes a candidate
//! vertical rate and gives the rate that ships and the limit that sets it.
//!
//! The callers:
//!
//! - `plunge::resolve_plunge`: the candidate is `min(rule, tip cap)`;
//! - `calculator::close::step9c_drill`: the candidate is the drill feed; the
//!   RPM follows the feed down to hold the chip;
//! - `ramp::resolve_ramp_feed`: the candidate is `cut feed x tan θ`, the
//!   vertical part of a G6 ramp at the cut feed. The ramp keeps the
//!   operator's angle (Q11) and ships `vertical / tan θ`.
//!
//! Until P20b only an op whose entry contract stated the axial chip
//! (Project Curve, lead decision Q8a) read the G6 bound, and no producer
//! read the Z axis rate.

use super::ramp::RampBasis;
use crate::machine::MachineProfile;

/// The G6 axial-chip bound of a vertical rate: `axial_chip_mm x rpm x
/// flutes`. Only [`VerticalRateLimit::new`] builds one, from a G6 claim
/// (`RampBasis::Sourced`).
#[derive(Debug, Clone, PartialEq)]
pub struct AxialChipBound {
    axial_chip_mm: f64,
    flutes: u32,
    side_row: String,
}

impl AxialChipBound {
    /// The G6 axial chip per tooth (mm): the side row's chip / Z, with the
    /// hardness scale (`FeedsResult::ramp`).
    #[must_use]
    pub const fn axial_chip_mm(&self) -> f64 {
        self.axial_chip_mm
    }

    /// The flute count Z of the claim.
    #[must_use]
    pub const fn flutes(&self) -> u32 {
        self.flutes
    }

    /// The side row that the G6 drill claim reads.
    #[must_use]
    pub fn side_row(&self) -> &str {
        &self.side_row
    }

    /// The vertical rate at `rpm` (mm/min).
    #[must_use]
    pub fn at_rpm(&self, rpm: f64) -> f64 {
        self.axial_chip_mm * rpm * f64::from(self.flutes)
    }

    /// The bound in words.
    #[must_use]
    pub fn text(&self) -> String {
        format!(
            "G6 axial chip {:.4} mm/tooth x rpm x {} (row {})",
            self.axial_chip_mm, self.flutes, self.side_row
        )
    }
}

/// The machine Z axis rate, from `MachineProfile::z_axis_rate_mm_min`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ZAxisRate {
    /// The machine states its Z axis rate (mm/min, GRBL `$112`).
    Stated(f64),
    /// The machine states no per-axis rates: no Z cap applies.
    NotStated,
}

impl ZAxisRate {
    /// The rate (mm/min), when the machine states it.
    #[must_use]
    pub const fn mm_min(self) -> Option<f64> {
        match self {
            Self::Stated(rate) => Some(rate),
            Self::NotStated => None,
        }
    }

    /// The rate in words, for a detail line.
    #[must_use]
    pub fn text(self) -> String {
        match self {
            Self::Stated(rate) => {
                format!("the machine Z axis rate ($112) caps the vertical rate at {rate:.0} mm/min")
            }
            Self::NotStated => {
                "the machine states no Z axis rate ($112), so no Z cap applies".to_owned()
            }
        }
    }
}

/// Which limit sets a vertical rate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VerticalBinding {
    /// No limit is below the candidate: the candidate ships.
    Candidate,
    /// The G6 axial chip x rpm x Z.
    AxialChip,
    /// The machine Z axis rate.
    ZAxisRate,
}

/// A vertical rate that [`VerticalRateLimit::bound`] gave, and the limit
/// that set it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VerticalRate {
    mm_min: f64,
    binding: VerticalBinding,
}

impl VerticalRate {
    /// The rate (mm/min). Not rounded: each caller rounds its own value.
    #[must_use]
    pub const fn mm_min(self) -> f64 {
        self.mm_min
    }

    /// The limit that set the rate.
    #[must_use]
    pub const fn binding(self) -> VerticalBinding {
        self.binding
    }
}

/// The one vertical-rate limit (P20b). See the module doc.
#[derive(Debug, Clone, PartialEq)]
pub struct VerticalRateLimit {
    axial: Option<AxialChipBound>,
    z_axis: ZAxisRate,
}

impl VerticalRateLimit {
    /// The limit of one cut: the G6 axial chip when `ramp` holds a G6 claim
    /// (`RampBasis::Sourced`), and the Z axis rate of `machine`. A drill
    /// cycle has a `NoChip` ramp basis, so its limit is the Z axis rate
    /// alone: its feed is already the G6 axial chip x rpm x Z.
    #[must_use]
    pub fn new(ramp: &RampBasis, machine: &MachineProfile) -> Self {
        let axial = match ramp {
            RampBasis::Sourced {
                axial_chip_mm,
                drill,
                ..
            } => Some(AxialChipBound {
                axial_chip_mm: *axial_chip_mm,
                flutes: drill.flutes,
                side_row: drill.side_row.clone(),
            }),
            RampBasis::NoChip { .. } => None,
        };
        let z_axis = machine
            .z_axis_rate_mm_min()
            .map_or(ZAxisRate::NotStated, ZAxisRate::Stated);
        Self { axial, z_axis }
    }

    /// The G6 axial-chip bound, when a G6 claim covers the tool.
    #[must_use]
    pub const fn axial(&self) -> Option<&AxialChipBound> {
        self.axial.as_ref()
    }

    /// The machine Z axis rate.
    #[must_use]
    pub const fn z_axis(&self) -> ZAxisRate {
        self.z_axis
    }

    /// The vertical rate that ships for `candidate_mm_min` at `rpm`:
    /// `min(candidate, axial chip x rpm x Z, Z axis rate)`. On a tie the
    /// candidate binds, then the axial chip. A limit that is not finite and
    /// positive does not bind.
    #[must_use]
    pub fn bound(&self, candidate_mm_min: f64, rpm: f64) -> VerticalRate {
        let mut rate = VerticalRate {
            mm_min: candidate_mm_min,
            binding: VerticalBinding::Candidate,
        };
        let limits = [
            (
                self.axial.as_ref().map(|b| b.at_rpm(rpm)),
                VerticalBinding::AxialChip,
            ),
            (self.z_axis.mm_min(), VerticalBinding::ZAxisRate),
        ];
        for (limit, binding) in limits {
            if let Some(value) = limit
                && value.is_finite()
                && value > 0.0
                && value < rate.mm_min
            {
                rate = VerticalRate {
                    mm_min: value,
                    binding,
                };
            }
        }
        rate
    }

    /// P29-8e (lead ruling Q11): THE limit at `rpm`, with no feed input:
    /// the lower of the G6 axial chip x rpm x Z and the machine Z axis rate,
    /// and the arm that binds (`AxialChip` or `ZAxisRate`, never
    /// `Candidate`). `None` when neither arm holds a finite rate above zero.
    /// [`Self::bound`] of any candidate equals `min(candidate, limit)`.
    #[must_use]
    pub fn limit_at(&self, rpm: f64) -> Option<VerticalRate> {
        let bound = self.bound(f64::INFINITY, rpm);
        match bound.binding {
            VerticalBinding::Candidate => None,
            VerticalBinding::AxialChip | VerticalBinding::ZAxisRate => Some(bound),
        }
    }

    /// The basis of an arm of the limit: the G6 claim on its side row for
    /// `AxialChip`, the machine field for `ZAxisRate`. `None` for
    /// `Candidate` (no limit binds) or an `AxialChip` with no G6 claim.
    #[must_use]
    pub fn basis_of(&self, binding: VerticalBinding) -> Option<crate::feeds::sourced::Basis> {
        use crate::feeds::sourced::{Basis, ClaimKind, ClaimRef, MachineField, RowRef};
        match binding {
            VerticalBinding::Candidate => None,
            VerticalBinding::AxialChip => self.axial.as_ref().map(|b| {
                Basis::Claim(ClaimRef {
                    kind: ClaimKind::DrillG6,
                    row: RowRef(b.side_row().to_owned()),
                    rule: "g6_axial_plunge".to_owned(),
                })
            }),
            VerticalBinding::ZAxisRate => Some(Basis::Machine(MachineField::ZAxisRate)),
        }
    }

    /// The limit in words, for a detail line: the G6 bound when there is
    /// one, then the Z axis rate (or the sentence that no Z cap applies).
    #[must_use]
    pub fn text(&self) -> String {
        match &self.axial {
            Some(bound) => format!("vertical rate: {}; {}", bound.text(), self.z_axis.text()),
            None => format!("vertical rate: {}", self.z_axis.text()),
        }
    }
}

/// The card and diagnostic sentence of `FeedsWarning::DrillFeedAtZAxisRate`.
#[must_use]
pub fn drill_feed_at_z_axis_rate_text(
    requested: f64,
    z_rate_mm_min: f64,
    rpm_from: f64,
    rpm_to: f64,
) -> String {
    let rpm = if rpm_to < rpm_from {
        format!("the RPM follows down {rpm_from:.0} -> {rpm_to:.0} to hold the chip")
    } else {
        format!(
            "the RPM stays at {rpm_to:.0} (the machine minimum), so the chip is thinner than the \
             chart chip"
        )
    };
    format!(
        "The drill feed {requested:.0} mm/min is above the machine Z axis rate ($112); the feed \
         ships at {z_rate_mm_min:.0} mm/min and {rpm}"
    )
}

#[cfg(test)]
#[expect(
    clippy::panic,
    reason = "test code: a panic is how a test reports a failure"
)]
mod tests {
    use super::*;
    use crate::feeds::ramp::RampFallback;
    use crate::machine::kinematics::MachineKinematics;

    fn machine_with_z(z: f64) -> MachineProfile {
        let mut m = MachineProfile::default();
        let mut k = MachineKinematics::generic_wood_router();
        let xy = m.max_feed_mm_min;
        k.max_rate_xyz_mm_min = Some([xy, xy, z]);
        m.kinematics = Some(k);
        m
    }

    fn no_chip() -> RampBasis {
        RampBasis::NoChip {
            reason: RampFallback::NoLut,
        }
    }

    /// No kinematics: no Z cap, and the text says so.
    #[test]
    fn no_kinematics_gives_no_z_cap() {
        let limit = VerticalRateLimit::new(&no_chip(), &MachineProfile::default());
        assert_eq!(limit.z_axis(), ZAxisRate::NotStated);
        let rate = limit.bound(5000.0, 18_000.0);
        assert_eq!(rate.mm_min(), 5000.0);
        assert_eq!(rate.binding(), VerticalBinding::Candidate);
        assert!(
            limit.text().contains("no Z cap applies"),
            "{}",
            limit.text()
        );
    }

    /// A stated Z rate binds above it, and a candidate at the rate ships
    /// as the candidate (a tie keeps the candidate).
    #[test]
    fn the_z_rate_binds_above_it() {
        let z = MachineProfile::default().max_feed_mm_min / 2.0;
        let limit = VerticalRateLimit::new(&no_chip(), &machine_with_z(z));
        let over = limit.bound(z * 1.5, 18_000.0);
        assert_eq!(over.mm_min(), z);
        assert_eq!(over.binding(), VerticalBinding::ZAxisRate);
        let at = limit.bound(z, 18_000.0);
        assert_eq!(at.binding(), VerticalBinding::Candidate);
        match limit.z_axis() {
            ZAxisRate::Stated(r) => assert_eq!(r, z),
            ZAxisRate::NotStated => panic!("the machine states Z"),
        }
    }
}
