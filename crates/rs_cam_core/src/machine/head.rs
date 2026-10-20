//! H1 (`planning/op_architecture_2026-10-14/PLAN.md`, item H): the machine
//! head and the dust shoe.
//!
//! The head is the solid that moves with the spindle above the tool holder
//! face: the collet nut, the spindle body, the dust collector mount. Most
//! routers have the same shape, so one general model serves them all: a
//! list of parts from the holder face upward. Each part has a diameter or
//! is a SHELF (no width limit), a height, and a kind (hard or soft).
//!
//! The dust shoe is a second part list on an adjustable mount. The operator
//! sets its height at each tool change, raises it clear or removes it. The
//! shoe's reference point is the BRUSH END: the bottom of its first part.
//! The GAP is the distance from the bit end (the tool tip) up to the brush
//! end.
//!
//! # The rules the checks read
//!
//! - A hard part blocks a move. A soft part (a brush) never blocks a move:
//!   it may touch the stock and the clamps.
//! - A part with a diameter is a band of that radius from its bottom
//!   upward, as the obstacle model reads every band
//!   (`compute::obstacle`). The part height places the next part only.
//! - A shelf has no width limit. The checks read it as a Z floor for the
//!   tip: the highest stock or obstacle top in the setup, minus the shelf
//!   height above the tip.
//! - The generation step reads the FIXED head only. The shoe moves to fit
//!   the toolpath; the toolpath does not move to fit the shoe. The shoe
//!   enters the simulation check and `collision_check` at the gap that the
//!   toolpath's shoe setting resolves to (`compute::shoe`).

use std::hash::{DefaultHasher, Hash, Hasher};
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::machine::domain::HeadDimension;
use crate::stock::collision::{AssemblySegment, HeadBand};

/// Does a part block a move?
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PartKind {
    /// A rigid part. It blocks a move.
    Hard,
    /// A flexible part (a brush). It may touch the stock and the clamps,
    /// so it never blocks a move.
    Soft,
}

/// One part of the head or of the shoe, from its bottom upward.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HeadPart {
    /// The name a collision and a note print.
    pub name: String,
    /// The diameter (mm). Absent: a shelf, a part with no width limit.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub diameter_mm: Option<f64>,
    /// The height (mm). It places the next part. A shelf is the last
    /// part, so its height is not read.
    #[serde(default)]
    pub height_mm: f64,
    /// Hard or soft.
    pub kind: PartKind,
}

impl HeadPart {
    fn hard(name: &str, diameter_mm: Option<f64>, height_mm: f64) -> Self {
        Self {
            name: name.to_owned(),
            diameter_mm,
            height_mm,
            kind: PartKind::Hard,
        }
    }

    /// A shelf: a part with no width limit.
    #[must_use]
    pub fn is_shelf(&self) -> bool {
        self.diameter_mm.is_none()
    }
}

/// The travel range of the shoe mount: the lowest and the highest
/// position of the brush end, in mm from the holder face, positive
/// upward. A brush end below the holder face is a negative value.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ShoeTravel {
    /// The lowest brush-end position (mm from the holder face).
    pub min_mm: f64,
    /// The highest brush-end position (mm from the holder face).
    pub max_mm: f64,
}

/// The dust shoe: an adjustable part list.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DustShoe {
    /// The parts from the brush end upward.
    pub parts: Vec<HeadPart>,
    /// The gap (mm, brush end above the bit end) the operator sets for
    /// normal work. The shoe solver gives no gap below it.
    pub normal_gap_mm: f64,
    /// The travel range of the mount. Absent: the range is not known, and
    /// the solver applies no upper limit.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub travel_mm: Option<ShoeTravel>,
}

impl DustShoe {
    /// The gap range the travel allows for a tool of `stickout`:
    /// `(lowest, highest)` gap, in mm above the bit end. Without a travel
    /// range: `(normal gap, infinity)`.
    #[must_use]
    pub fn gap_range(&self, stickout: f64) -> (f64, f64) {
        match self.travel_mm {
            Some(travel) => (
                (stickout + travel.min_mm).max(self.normal_gap_mm),
                stickout + travel.max_mm,
            ),
            None => (self.normal_gap_mm, f64::INFINITY),
        }
    }
}

/// The head of the machine: the fixed parts above the holder face, and an
/// optional dust shoe.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MachineHead {
    /// The name of the head (a preset name or the operator's own).
    #[serde(default)]
    pub name: String,
    /// The fixed mount: the distance (mm) from the holder face up to the
    /// bottom of the first part. The holder face is at the tool stickout
    /// above the tip, so the head moves with the stickout.
    #[serde(default)]
    pub mount_mm: f64,
    /// The fixed parts, from the mount upward.
    pub parts: Vec<HeadPart>,
    /// The dust shoe, when the machine has one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shoe: Option<DustShoe>,
}

/// The diameter (mm) of the head that a project without a `head` table
/// reads: the per-tool holder default before H1 (`holder_diameter = 25`).
pub const UNMODELLED_HEAD_DIAMETER_MM: f64 = 25.0;

impl Default for MachineHead {
    /// The head of a project without a `head` table: one hard part of
    /// [`UNMODELLED_HEAD_DIAMETER_MM`] at the holder face, with no shelf
    /// and no shoe. It is the band every tool had before H1, so a project
    /// that states no head keeps its toolpaths and its checks.
    fn default() -> Self {
        Self::unmodelled()
    }
}

/// Why a head was refused. Each variant names the rule it broke.
#[derive(Debug, Clone, PartialEq)]
pub enum HeadRefusal {
    /// A value is outside its domain
    /// ([`crate::machine::domain::HeadDimension::domain`]).
    BadValue {
        /// The field path, for example `parts[0].diameter_mm`.
        field: String,
        /// The value.
        value: f64,
        /// The rule the value breaks.
        requirement: crate::tool::domain::Requirement,
    },
    /// A part stands above a shelf. A shelf has no width limit, so
    /// nothing above it can be reached.
    PartAboveShelf {
        /// The name of the part.
        part: String,
    },
    /// The shoe has no part.
    EmptyShoe,
    /// The travel range has its minimum above its maximum.
    BadTravel,
}

impl std::fmt::Display for HeadRefusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::BadValue {
                field,
                value,
                requirement,
            } => {
                write!(f, "the head value {field} = {value}: must be {requirement}")
            }
            Self::PartAboveShelf { part } => write!(
                f,
                "the head part '{part}' stands above a shelf; a shelf has no width limit, \
                 so it must be the last part"
            ),
            Self::EmptyShoe => f.write_str("the dust shoe needs at least one part"),
            Self::BadTravel => f.write_str("the shoe travel needs min_mm at or below max_mm"),
        }
    }
}

impl std::error::Error for HeadRefusal {}

/// F210: refuse `value` outside the domain of `dimension`, with the field
/// path `field` builds.
fn check_value(
    dimension: HeadDimension,
    value: f64,
    field: impl FnOnce() -> String,
) -> Result<(), HeadRefusal> {
    match dimension.violation(value) {
        None => Ok(()),
        Some(requirement) => Err(HeadRefusal::BadValue {
            field: field(),
            value,
            requirement,
        }),
    }
}

fn check_parts(parts: &[HeadPart], at: &str) -> Result<(), HeadRefusal> {
    let mut shelf_seen = false;
    for (i, part) in parts.iter().enumerate() {
        if shelf_seen {
            return Err(HeadRefusal::PartAboveShelf {
                part: part.name.clone(),
            });
        }
        if let Some(d) = part.diameter_mm {
            check_value(HeadDimension::PartDiameter, d, || {
                format!("{at}[{i}].diameter_mm")
            })?;
        }
        check_value(HeadDimension::PartHeight, part.height_mm, || {
            format!("{at}[{i}].height_mm")
        })?;
        shelf_seen = part.is_shelf();
    }
    Ok(())
}

/// The bands of `parts` that block a move, from `bottom` (mm above the
/// tip) upward. A soft part is skipped, but its height still places the
/// next part.
fn hard_bands(parts: &[HeadPart], bottom: f64, segment: AssemblySegment) -> Vec<HeadBand> {
    let mut out = Vec::new();
    let mut offset = bottom;
    for part in parts {
        if part.kind == PartKind::Hard {
            out.push(HeadBand {
                segment,
                name: part.name.clone(),
                offset_mm: offset,
                radius_mm: part.diameter_mm.map(|d| d / 2.0),
            });
        }
        offset += part.height_mm;
    }
    out
}

impl MachineHead {
    /// The head a project without a `head` table reads. See
    /// [`Default`]. It is not a preset: it states no real head.
    #[must_use]
    pub fn unmodelled() -> Self {
        Self {
            name: "Unmodelled head".to_owned(),
            mount_mm: 0.0,
            parts: vec![HeadPart::hard(
                "Holder",
                Some(UNMODELLED_HEAD_DIAMETER_MM),
                0.0,
            )],
            shoe: None,
        }
    }

    /// The fixed parts of both presets. Source: the operator facts in
    /// `planning/op_architecture_2026-10-14/PLAN.md`, section H (Scylla,
    /// H100 spindle): a collet nut of about 20 mm diameter, 30 mm tall
    /// above the holder face, then a shelf (no width limit, solid).
    fn collet_parts() -> Vec<HeadPart> {
        vec![
            HeadPart::hard("Collet nut", Some(20.0), 30.0),
            HeadPart::hard("Spindle", None, 0.0),
        ]
    }

    /// The preset "Bare collet": the collet nut and the spindle shelf, no
    /// shoe. Source: `planning/op_architecture_2026-10-14/PLAN.md`,
    /// section H.
    #[must_use]
    pub fn bare_collet() -> Self {
        Self {
            name: "Bare collet".to_owned(),
            mount_mm: 0.0,
            parts: Self::collet_parts(),
            shoe: None,
        }
    }

    /// The preset "Dust shoe": the bare collet plus the shoe. Source:
    /// `planning/op_architecture_2026-10-14/PLAN.md`, section H: a brush
    /// ring of 80 mm diameter, 25 mm long, soft; above it a rigid plate of
    /// 80 mm diameter, 5 mm tall; then a shelf. Normal finishing: the
    /// brush end 25 mm above the bit end. The travel range is not known.
    #[must_use]
    pub fn dust_shoe() -> Self {
        Self {
            name: "Dust shoe".to_owned(),
            mount_mm: 0.0,
            parts: Self::collet_parts(),
            shoe: Some(DustShoe {
                parts: vec![
                    HeadPart {
                        name: "Brush".to_owned(),
                        diameter_mm: Some(80.0),
                        height_mm: 25.0,
                        kind: PartKind::Soft,
                    },
                    HeadPart::hard("Shoe plate", Some(80.0), 5.0),
                    HeadPart::hard("Shoe mount", None, 0.0),
                ],
                normal_gap_mm: 25.0,
                travel_mm: None,
            }),
        }
    }

    /// The preset with `name`, or `None`. The names are the ones
    /// [`Self::PRESETS`] lists.
    #[must_use]
    pub fn preset(name: &str) -> Option<Self> {
        match name {
            "Bare collet" => Some(Self::bare_collet()),
            "Dust shoe" => Some(Self::dust_shoe()),
            _ => None,
        }
    }

    /// The preset names.
    pub const PRESETS: [&'static str; 2] = ["Bare collet", "Dust shoe"];

    /// Is this the head a project without a `head` table reads? The
    /// writer leaves the table out for it, so an old file saves as it
    /// loaded.
    #[must_use]
    pub fn is_unmodelled(&self) -> bool {
        *self == Self::unmodelled()
    }

    /// Check every value.
    ///
    /// # Errors
    /// The [`HeadRefusal`] that names the first rule the head breaks.
    pub fn validate(&self) -> Result<(), HeadRefusal> {
        check_value(HeadDimension::Mount, self.mount_mm, || {
            "mount_mm".to_owned()
        })?;
        check_parts(&self.parts, "parts")?;
        if let Some(shoe) = &self.shoe {
            if shoe.parts.is_empty() {
                return Err(HeadRefusal::EmptyShoe);
            }
            check_parts(&shoe.parts, "shoe.parts")?;
            check_value(HeadDimension::NormalGap, shoe.normal_gap_mm, || {
                "shoe.normal_gap_mm".to_owned()
            })?;
            if let Some(travel) = shoe.travel_mm {
                check_value(HeadDimension::TravelEnd, travel.min_mm, || {
                    "shoe.travel_mm.min_mm".to_owned()
                })?;
                check_value(HeadDimension::TravelEnd, travel.max_mm, || {
                    "shoe.travel_mm.max_mm".to_owned()
                })?;
                if travel.min_mm > travel.max_mm {
                    return Err(HeadRefusal::BadTravel);
                }
            }
        }
        Ok(())
    }

    /// The hard bands of the head for a tool of `stickout`, with the shoe
    /// at `shoe`. Offsets are in mm above the tip.
    #[must_use]
    pub fn bands(&self, stickout: f64, shoe: ShoePosition) -> Vec<HeadBand> {
        let mut out = hard_bands(
            &self.parts,
            stickout + self.mount_mm,
            AssemblySegment::Holder,
        );
        if let (ShoePosition::At { gap_mm }, Some(dust_shoe)) = (shoe, &self.shoe) {
            out.extend(hard_bands(&dust_shoe.parts, gap_mm, AssemblySegment::Shoe));
        }
        out
    }

    /// A digest of the fixed parts: every value the generation step reads.
    /// The shoe is not in it: the shoe does not move a toolpath.
    #[must_use]
    pub fn fixed_digest(&self) -> u64 {
        let mut hasher = DefaultHasher::new();
        format!("{:?}", (self.mount_mm, &self.parts)).hash(&mut hasher);
        hasher.finish()
    }

    /// A digest of the whole head, the shoe included: every value a check
    /// reads.
    #[must_use]
    pub fn effect_digest(&self) -> u64 {
        let mut hasher = DefaultHasher::new();
        format!("{:?}", (self.mount_mm, &self.parts, &self.shoe)).hash(&mut hasher);
        hasher.finish()
    }
}

/// Where the shoe is, for one check.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum ShoePosition {
    /// The shoe is not in the model: the machine has none, it is raised
    /// clear, it is removed, or the reader is the generation step (the
    /// fixed head only). The travel range is optional and often not
    /// known, so a raised shoe is not modelled at its highest position.
    #[default]
    Clear,
    /// The brush end is `gap_mm` above the bit end.
    At {
        /// The gap (mm).
        gap_mm: f64,
    },
}

/// The head as one check reads it: the head and the shoe position.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct HeadFit {
    /// The machine head.
    pub head: Arc<MachineHead>,
    /// The shoe position.
    pub shoe: ShoePosition,
}

impl HeadFit {
    /// The fixed head of `head`, with the shoe out of the model: the
    /// generation step reads this.
    #[must_use]
    pub fn fixed(head: &MachineHead) -> Self {
        Self {
            head: Arc::new(head.clone()),
            shoe: ShoePosition::Clear,
        }
    }
}

/// The shoe setting of one toolpath (H1).
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
#[serde(tag = "mode", rename_all = "snake_case")]
pub enum ShoeSetting {
    /// The shoe solver sets the gap (the default).
    #[default]
    Auto,
    /// The operator states the gap (mm, brush end above the bit end).
    Gap {
        /// The gap (mm).
        gap_mm: f64,
    },
    /// The shoe is raised clear.
    Raised,
    /// The shoe is removed.
    Off,
}

impl ShoeSetting {
    /// F210 (W6b): a stated gap is inside
    /// [`HeadDimension::ShoeGap`]'s domain (finite, `>= 0`). The
    /// `SetToolpathShoe` door and the project loader call it.
    ///
    /// # Errors
    /// [`HeadRefusal::BadValue`] on the field `gap_mm`.
    pub fn validate(&self) -> Result<(), HeadRefusal> {
        match *self {
            Self::Gap { gap_mm } => {
                check_value(HeadDimension::ShoeGap, gap_mm, || "gap_mm".to_owned())
            }
            Self::Auto | Self::Raised | Self::Off => Ok(()),
        }
    }

    /// Is this the default? The writer leaves the key out for it.
    #[must_use]
    pub fn is_auto(&self) -> bool {
        matches!(self, Self::Auto)
    }
}

/// The shoe state one toolpath runs with, after the solver (H1).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ResolvedShoe {
    /// The machine has no shoe.
    NoShoe,
    /// The brush end is `gap_mm` above the bit end.
    Gap {
        /// The gap the toolpath runs with (mm).
        gap_mm: f64,
    },
    /// The shoe is raised clear: the operator chose it, or no gap in the
    /// travel range clears the stock and the obstacles.
    Raised,
    /// The shoe is removed.
    Off,
}

impl ResolvedShoe {
    /// The shoe position the checks read.
    #[must_use]
    pub fn position(self) -> ShoePosition {
        match self {
            Self::Gap { gap_mm } => ShoePosition::At { gap_mm },
            Self::NoShoe | Self::Raised | Self::Off => ShoePosition::Clear,
        }
    }

    /// The short text a surface prints for the operator (MCP, H2 GUI).
    /// The G-code carries no shoe text (operator ruling 2026-10-16).
    /// `None` for a machine with no shoe.
    #[must_use]
    pub fn label(self) -> Option<String> {
        match self {
            Self::NoShoe => None,
            Self::Gap { gap_mm } => Some(format!("brush {gap_mm:.0} mm above tip")),
            Self::Raised => Some("raise clear".to_owned()),
            Self::Off => Some("remove".to_owned()),
        }
    }
}

#[cfg(test)]
#[allow(
    // SAFETY: test code; a failed lookup is a failed test.
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]
mod tests {
    use super::*;

    #[test]
    fn the_unmodelled_head_is_the_old_holder_band() {
        let bands = MachineHead::default().bands(45.0, ShoePosition::Clear);
        assert_eq!(bands.len(), 1);
        assert_eq!(bands[0].offset_mm, 45.0);
        assert_eq!(bands[0].radius_mm, Some(12.5));
        assert_eq!(bands[0].segment, AssemblySegment::Holder);
    }

    #[test]
    fn the_dust_shoe_skips_the_brush_and_places_the_plate_above_it() {
        let head = MachineHead::dust_shoe();
        head.validate().unwrap();
        let bands = head.bands(45.0, ShoePosition::At { gap_mm: 25.0 });
        // Collet nut at 45, spindle shelf at 75, plate at 50, shoe shelf at 55.
        let find = |name: &str| bands.iter().find(|b| b.name == name).unwrap();
        assert_eq!(find("Collet nut").offset_mm, 45.0);
        assert_eq!(find("Spindle").offset_mm, 75.0);
        assert_eq!(find("Spindle").radius_mm, None);
        assert_eq!(find("Shoe plate").offset_mm, 50.0);
        assert_eq!(find("Shoe plate").radius_mm, Some(40.0));
        assert_eq!(find("Shoe mount").offset_mm, 55.0);
        assert!(
            bands.iter().all(|b| b.name != "Brush"),
            "a soft part never blocks"
        );
        // Clear: the fixed head only.
        assert_eq!(head.bands(45.0, ShoePosition::Clear).len(), 2);
    }

    #[test]
    fn a_part_above_a_shelf_is_refused() {
        let mut head = MachineHead::bare_collet();
        head.parts.push(HeadPart::hard("Lid", Some(10.0), 5.0));
        assert!(matches!(
            head.validate(),
            Err(HeadRefusal::PartAboveShelf { .. })
        ));
    }

    #[test]
    fn the_gap_range_reads_the_travel_from_the_holder_face() {
        let mut shoe = MachineHead::dust_shoe().shoe.unwrap();
        assert_eq!(shoe.gap_range(45.0), (25.0, f64::INFINITY));
        shoe.travel_mm = Some(ShoeTravel {
            min_mm: -30.0,
            max_mm: 10.0,
        });
        // Brush end 30 below to 10 above the face: gap 15..55, floor 25.
        assert_eq!(shoe.gap_range(45.0), (25.0, 55.0));
    }
}
