//! P22-0c (`planning/feeds_contract_2026-11-03/phases/P22-0c_DESIGN.md`):
//! the tool body. The tool owns its parts; the systems use them.
//!
//! A [`ToolBody`] is the list of axial zones of one tool, from the tip
//! up. A zone cuts ([`Zone::Cutting`]) or only touches
//! ([`Zone::Contact`]). [`ToolBody::zones`] is THE zone table; every view
//! of the body reads the same private facts as that table, and the sentry
//! `one_body_for_every_reader_p22_0c` holds each view equal to it.
//!
//! The body has ONE constructor, [`ToolBody::new`]. Only
//! [`super::ToolDefinition`] calls it, and `compute::cutter::build_cutter`
//! is the production door of `ToolDefinition`. The constructor cannot
//! fail: `ToolConfig::validate` holds the domains first (F029). No type
//! here has a `Default`.
//!
//! [`MountedBody`] is the body plus the machine head at the holder face.
//! The machine owns the head parts ([`HeadBand`]); the mounted body only
//! reads them.

use std::hash::{DefaultHasher, Hash};

use super::MillingCutter;
use crate::compute::sim_prefix::digest::hash_f64_bits;
use crate::compute::tool_config::{
    NON_CENTRE_CUTTING_TOL_MM, NonCentreCuttingGeometry, NonCentreCuttingRefusal,
};
use crate::geo::P3;
use crate::machine::head::{HeadBand, HeadBandKind};
use crate::stock::radial_profile::RadialProfileLUT;

/// One axial span, in mm above the tool tip.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AxialSpan {
    /// The bottom of the span (mm above the tip).
    pub bottom_mm: f64,
    /// The top of the span.
    pub top: SpanTop,
}

/// The top of an [`AxialSpan`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SpanTop {
    /// A top at this height (mm above the tip).
    At(f64),
    /// No top: a head part reads from its bottom upward.
    Open,
}

/// One axial zone of a tool.
#[derive(Debug, Clone, PartialEq)]
pub enum Zone {
    /// A zone that cuts.
    Cutting(CuttingZone),
    /// A zone that does not cut: it only touches.
    Contact(ContactZone),
}

/// A zone that cuts. Its outer edge is the `MillingCutter` profile.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CuttingZone {
    /// The axial span.
    pub span: AxialSpan,
    /// The inner radius (mm): 0, or the core radius of a tool that does
    /// not cut at its centre (the blade ring).
    pub inner_mm: f64,
    /// What cuts.
    pub kind: CuttingKind,
}

/// What a [`CuttingZone`] cuts with.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CuttingKind {
    /// The flutes.
    Flutes,
}

/// A zone that does not cut.
#[derive(Debug, Clone, PartialEq)]
pub struct ContactZone {
    /// The axial span.
    pub span: AxialSpan,
    /// The inner radius (mm).
    pub inner_mm: f64,
    /// The outer radius.
    pub outer: ContactRadius,
    /// The part.
    pub kind: ContactKind,
    /// A head part name, for the finding text. `None` for a tool zone.
    pub name: Option<String>,
}

/// The outer radius of a [`ContactZone`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ContactRadius {
    /// A radius (mm).
    Mm(f64),
    /// A shelf: no width limit.
    Shelf,
}

/// The part a [`ContactZone`] is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContactKind {
    /// The centre core of a tool that does not cut at its centre: from the
    /// blade protrusion to the flute top, radius `core_d / 2`.
    Core,
    /// The face where the flutes end: at the flute length, out to the
    /// cutter envelope radius.
    FluteTop,
    /// The shank, from the flute top to the holder face.
    Shank,
    /// A fixed part of the machine head (H1).
    Holder,
    /// A hard part of the dust shoe (H1).
    Shoe,
}

impl ContactKind {
    /// The wire and chip name of the zone kind (P22-0c-3, operator ruling
    /// Q6): one word on the collision marker, one field in MCP and the CLI.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Core => "core",
            Self::FluteTop => "flute_top",
            Self::Shank => "shank",
            Self::Holder => "holder",
            Self::Shoe => "shoe",
        }
    }
}

impl From<HeadBandKind> for ContactKind {
    fn from(kind: HeadBandKind) -> Self {
        match kind {
            HeadBandKind::Holder => Self::Holder,
            HeadBandKind::Shoe => Self::Shoe,
        }
    }
}

/// The axial zones of one tool (P22-0c). See the module text.
#[derive(Debug, Clone, PartialEq)]
pub struct ToolBody {
    /// The flute length `L` (mm): the cutting zone is `[0, L]`.
    flute_length_mm: f64,
    /// The cutter envelope radius (mm): the radius of the flute top.
    envelope_radius_mm: f64,
    /// The centre core, for a tool that does not cut at its centre.
    core: Option<NonCentreCuttingGeometry>,
    /// The shank diameter (mm).
    shank_diameter_mm: f64,
    /// The stickout (mm): the holder face is at this height above the
    /// tip, always (K1b, B18).
    stickout_mm: f64,
    /// The cutter radius at the flute top (mm): `width(L)`, never above
    /// the envelope radius (P22-0c-6, ruling 1). The cutting zone stops at
    /// the flute length, so no cut is wider than this.
    cut_radius_at_flute_top_mm: f64,
}

/// THE rule "the shank, else the envelope" (P22-0c-6 D3): the diameter
/// above the cutting zone. A shank of 0 is outside its domain (ruling 6),
/// so the envelope arm serves only a record that has not passed
/// `ToolConfig::validate` (the size label must not fail).
#[must_use]
pub fn upper_diameter_mm(shank_d: f64, envelope_d: f64) -> f64 {
    if shank_d > 0.0 { shank_d } else { envelope_d }
}

impl ToolBody {
    /// THE constructor. `cutter` gives the flute length and the envelope
    /// radius; `shank_diameter_mm` and `stickout_mm` are the planning
    /// values (`ToolConfig::*_planning_mm`, one default per fact); `core`
    /// is `Some` only for a `CentreCutting::No(..)` fact.
    pub(crate) fn new(
        cutter: &dyn MillingCutter,
        shank_diameter_mm: f64,
        stickout_mm: f64,
        core: Option<NonCentreCuttingGeometry>,
    ) -> Self {
        let flute_length_mm = cutter.length();
        let envelope_radius_mm = cutter.envelope_radius_mm();
        Self {
            flute_length_mm,
            envelope_radius_mm,
            core,
            shank_diameter_mm,
            stickout_mm,
            cut_radius_at_flute_top_mm: cutter
                .width_at_height(flute_length_mm)
                .min(envelope_radius_mm),
        }
    }

    /// THE zone table, from the tip up:
    ///
    /// 1. `Cutting(Flutes)`: `[0, L]`, inner = the core radius, else 0.
    /// 2. `Contact(Core)` (only with a core): `[protrusion, L]`, `0..core_d/2`.
    /// 3. `Contact(FluteTop)`: `[L, L]`, out to the envelope radius.
    /// 4. `Contact(Shank)`: `[L, stickout]`, `shank_d / 2`, only when
    ///    `stickout > L` (the rule of [`Self::exposed_shank_length_mm`]).
    #[must_use]
    pub fn zones(&self) -> Vec<Zone> {
        let length = self.flute_length_mm;
        let mut zones = vec![Zone::Cutting(CuttingZone {
            span: AxialSpan {
                bottom_mm: 0.0,
                top: SpanTop::At(length),
            },
            inner_mm: self.core().map_or(0.0, |core| core.radius_mm),
            kind: CuttingKind::Flutes,
        })];
        if let Some(core) = self.core() {
            zones.push(Zone::Contact(ContactZone {
                span: AxialSpan {
                    bottom_mm: core.protrusion_mm,
                    top: SpanTop::At(length),
                },
                inner_mm: 0.0,
                outer: ContactRadius::Mm(core.radius_mm),
                kind: ContactKind::Core,
                name: None,
            }));
        }
        zones.push(Zone::Contact(ContactZone {
            span: AxialSpan {
                bottom_mm: length,
                top: SpanTop::At(length),
            },
            inner_mm: 0.0,
            outer: ContactRadius::Mm(self.envelope_radius_mm),
            kind: ContactKind::FluteTop,
            name: None,
        }));
        if self.exposed_shank_length_mm() > 0.0 {
            zones.push(Zone::Contact(ContactZone {
                span: AxialSpan {
                    bottom_mm: length,
                    top: SpanTop::At(self.stickout_mm),
                },
                inner_mm: 0.0,
                outer: ContactRadius::Mm(self.shank_diameter_mm / 2.0),
                kind: ContactKind::Shank,
                name: None,
            }));
        }
        zones
    }

    /// The flute length `L` (mm): the top of the cutting zone.
    #[must_use]
    pub fn flute_length_mm(&self) -> f64 {
        self.flute_length_mm
    }

    /// The radius of the flute top (mm): the cutter envelope radius.
    #[must_use]
    pub fn envelope_radius_mm(&self) -> f64 {
        self.envelope_radius_mm
    }

    /// The shank diameter (mm). The tool states it even when no shank
    /// shows below the holder face.
    #[must_use]
    pub fn shank_diameter_mm(&self) -> f64 {
        self.shank_diameter_mm
    }

    /// The diameter above the cutting zone (mm): the shank, else the
    /// envelope diameter ([`upper_diameter_mm`]). The size label and the
    /// shank section of `ToolDefinition::tip_deflection_mm` read it.
    #[must_use]
    pub fn upper_diameter_mm(&self) -> f64 {
        upper_diameter_mm(self.shank_diameter_mm, 2.0 * self.envelope_radius_mm)
    }

    /// The widest diameter of the cutting zone (mm):
    /// `2 x min(width(L), envelope radius)` (P22-0c-6, ruling 1). The
    /// cutter width grows with the height, so a cap at this value equals
    /// the width at `min(ap, L)`. A tapered ball whose cone meets the shaft
    /// above `L` cuts no wider than its width at `L`.
    #[must_use]
    pub fn max_cut_diameter_mm(&self) -> f64 {
        2.0 * self.cut_radius_at_flute_top_mm
    }

    /// The holder face (mm above the tip): the stickout.
    #[must_use]
    pub fn holder_face_mm(&self) -> f64 {
        self.stickout_mm
    }

    /// The shank length between the flutes and the holder face (mm):
    /// `max(stickout - L, 0)`.
    #[must_use]
    pub fn exposed_shank_length_mm(&self) -> f64 {
        (self.stickout_mm - self.flute_length_mm).max(0.0)
    }

    /// The centre core zone (P22-0c-2): `Some` only for a tool that does
    /// not cut at its centre. Every reader of the core reads it here: the
    /// entry emitters, the helix floor, the cut descents, the core check.
    #[must_use]
    pub fn core(&self) -> Option<CoreZone> {
        self.core.map(CoreZone::from_geometry)
    }

    /// Hash every fact of the body (the simulation-prefix key, D1).
    /// Destructured, `..`-free: a new fact does not compile until it is in
    /// the key.
    pub fn digest(&self, hasher: &mut DefaultHasher) {
        let Self {
            flute_length_mm,
            envelope_radius_mm,
            core,
            shank_diameter_mm,
            stickout_mm,
            cut_radius_at_flute_top_mm,
        } = self;
        for value in [
            *flute_length_mm,
            *envelope_radius_mm,
            *shank_diameter_mm,
            *stickout_mm,
            *cut_radius_at_flute_top_mm,
        ] {
            hash_f64_bits(hasher, value);
        }
        match core {
            Some(NonCentreCuttingGeometry {
                centre_core_diameter_mm,
                blade_protrusion_mm,
            }) => {
                1_u8.hash(hasher);
                hash_f64_bits(hasher, *centre_core_diameter_mm);
                hash_f64_bits(hasher, *blade_protrusion_mm);
            }
            None => 0_u8.hash(hasher),
        }
    }

    /// The body mounted in the machine head: `head` are the hard bands of
    /// the head at this stickout (`MachineHead::bands`).
    #[must_use]
    pub fn mounted(&self, head: Vec<HeadBand>) -> MountedBody {
        MountedBody {
            body: self.clone(),
            head,
        }
    }
}

/// A [`ToolBody`] plus the machine head at its holder face (H1).
#[derive(Debug, Clone, PartialEq)]
pub struct MountedBody {
    body: ToolBody,
    head: Vec<HeadBand>,
}

impl MountedBody {
    /// The tool body.
    #[must_use]
    pub fn body(&self) -> &ToolBody {
        &self.body
    }

    /// The hard bands of the machine head.
    #[must_use]
    pub fn head(&self) -> &[HeadBand] {
        &self.head
    }

    /// Every zone: the tool zones from the tip up, then each head band as
    /// a contact zone from its bottom upward ([`head_zone`]).
    #[must_use]
    pub fn zones(&self) -> Vec<Zone> {
        let mut zones = self.body.zones();
        zones.extend(self.head.iter().map(|band| Zone::Contact(head_zone(band))));
        zones
    }

    /// The first zone above the flutes that has a width (P22-0c-5): the
    /// shank when the body shows one, else the first head band with a
    /// diameter. `None` when the mounted body has neither. It is the part
    /// a wall taller than the flutes meets first (`maps::tier_flute_reach`).
    /// Its radius is taken as is, with no comparison against the envelope
    /// radius.
    #[must_use]
    pub fn first_above_flutes(&self) -> Option<ContactZone> {
        self.zones().into_iter().find_map(|zone| match zone {
            Zone::Contact(contact) => match (contact.kind, contact.outer) {
                (
                    ContactKind::Shank | ContactKind::Holder | ContactKind::Shoe,
                    ContactRadius::Mm(_),
                ) => Some(contact),
                (ContactKind::Core | ContactKind::FluteTop, _) | (_, ContactRadius::Shelf) => None,
            },
            Zone::Cutting(_) => None,
        })
    }
}

/// The contact zone of one head band (H1): from its bottom upward
/// (`SpanTop::Open`); a band with no width limit is a shelf. THE mapping
/// of a band to a zone: [`MountedBody::zones`] and the shoe solver
/// (`compute::shoe`, a head with no tool) read it.
#[must_use]
pub fn head_zone(band: &HeadBand) -> ContactZone {
    ContactZone {
        span: AxialSpan {
            bottom_mm: band.offset_mm,
            top: SpanTop::Open,
        },
        inner_mm: 0.0,
        outer: band
            .radius_mm
            .map_or(ContactRadius::Shelf, ContactRadius::Mm),
        kind: band.kind.into(),
        name: Some(band.name.clone()),
    }
}

/// The kind of any zone (P22-0c-5): the part a collision event names. It
/// replaces the old `AssemblySegment`; its wire names do not change
/// (`cutter`, `shank`, `holder`, `shoe`, and `core`, `flute_top`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ZoneKind {
    /// A cutting zone (K1: the obstacle check includes the cutter).
    Cutting(CuttingKind),
    /// A contact zone.
    Contact(ContactKind),
}

impl ZoneKind {
    /// The wire and report name.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Cutting(CuttingKind::Flutes) => "cutter",
            Self::Contact(kind) => kind.as_str(),
        }
    }
}

impl std::fmt::Display for ZoneKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// The cut table of `cutter` (P22-0c-2, G-SIMCORE): the envelope profile
/// plus the blade ring of the body core. A tool that does not cut at its
/// centre cuts only with the ring `core_r <= r <= R`; the core removes no
/// stock. Every reader of removed stock builds its table here: the
/// simulation stamp, the planner cut and link stamps, the feed optimiser
/// and the own-stock replay. A cutter with no body (a bare shape) is a
/// centre-cutting tool: the table is [`envelope_table`].
#[must_use]
pub fn cut_table(cutter: &dyn MillingCutter, samples: usize) -> RadialProfileLUT {
    let lut = envelope_table(cutter, samples);
    match core_of(cutter) {
        Some(core) => lut.with_blade_ring(core.radius_mm),
        None => lut,
    }
}

/// The envelope table of `cutter` (P22-0c-2): the full disc. The readers
/// that model a solid body, not removed stock, build it here: the rapid
/// probe and the planner entry stamps (a shipped entry of a non-centre
/// tool cuts the core column by construction, P22-0b decision 2).
#[must_use]
pub fn envelope_table(cutter: &dyn MillingCutter, samples: usize) -> RadialProfileLUT {
    RadialProfileLUT::from_cutter(cutter, samples)
}

/// The body of `cutter` (P22-0c-3): the body of a whole tool
/// ([`MillingCutter::tool_body`]), else the body of a bare shape built by
/// THE constructor with no core, no shank zone (stickout = flute length)
/// and the shape's own flute length and envelope radius. The contact
/// kernel reads the zones of either.
#[must_use]
pub fn body_of(cutter: &dyn MillingCutter) -> std::borrow::Cow<'_, ToolBody> {
    match cutter.tool_body() {
        Some(body) => std::borrow::Cow::Borrowed(body),
        None => std::borrow::Cow::Owned(ToolBody::new(
            cutter,
            cutter.diameter(),
            cutter.length(),
            None,
        )),
    }
}

/// The core zone of `cutter`: the body core of a whole tool
/// ([`MillingCutter::tool_body`]); `None` for a centre-cutting tool and
/// for a bare shape.
#[must_use]
pub fn core_of(cutter: &dyn MillingCutter) -> Option<CoreZone> {
    cutter.tool_body().and_then(ToolBody::core)
}

/// The centre core of a tool that does not cut at its centre (P22-0c-2):
/// radius `c` and the blade protrusion `p` below it. The zone owns every
/// rule of the core in ONE place: the plunge, the straight-line limit,
/// the zigzag ramp, the helix pitch and the dwell rule of a path that
/// turns. The emitters, the 3D cut and link descents, the helical
/// waterline, the pencil rest ladder, the feasibility rule and the
/// simulation core check read it through the body ([`ToolBody::core`]).
///
/// The dwell rule: a cell under the core keeps the top it had when the
/// blade ring last passed over it. A piece of path that stays within `c`
/// of a cell has an XY diameter of at most `2 c`. So along every piece of
/// path with an XY diameter of at most `2 c`, the descent must be at most
/// `p`. On one straight move that piece is `2 c` long
/// ([`Self::max_descent_slope`]); at a zigzag reversal it is `4 c`
/// ([`Self::max_ramp_slope`]); on a circle of radius `r` it is the arc
/// `2 r asin(c / r)` ([`Self::max_helix_pitch_mm`]); on any other path
/// [`Self::clamp_descents`] and [`Self::max_dwell_mm`] measure it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CoreZone {
    /// The core radius `c` (mm): half the core diameter.
    pub radius_mm: f64,
    /// The blade protrusion `p` (mm): how far the blades stand below the
    /// core. A straight plunge may go this deep into stock.
    pub protrusion_mm: f64,
}

/// A rise (mm) under this is float noise of the dwell rule, not a change
/// of the path: a straight descent at the straight-line limit keeps its
/// bits.
const DWELL_NOISE_MM: f64 = 1e-9;

/// Bisection steps for the window boundary inside one segment.
const DWELL_BISECTION_STEPS: usize = 40;

impl CoreZone {
    /// The zone of the stated core facts. THE `d / 2` of the core; the
    /// body calls it, and a test builds a probe zone here.
    #[must_use]
    pub fn from_geometry(geometry: NonCentreCuttingGeometry) -> Self {
        Self {
            radius_mm: geometry.centre_core_diameter_mm / 2.0,
            protrusion_mm: geometry.blade_protrusion_mm,
        }
    }

    /// The core diameter (mm), `2 c`.
    #[must_use]
    pub fn diameter_mm(&self) -> f64 {
        2.0 * self.radius_mm
    }

    /// The steepest descent angle (degrees) of ONE straight move into
    /// stock, `atan(p / d_core)`.
    #[must_use]
    pub fn max_descent_angle_deg(&self) -> f64 {
        (self.protrusion_mm / self.diameter_mm())
            .atan()
            .to_degrees()
    }

    /// The steepest descent slope (dz / dxy) of ONE straight move into
    /// stock, `p / d_core`: on a straight move a cell stays under the core
    /// for at most `2 c` of travel. A path that turns needs more
    /// ([`Self::max_ramp_slope`], [`Self::max_helix_pitch_mm`],
    /// [`Self::clamp_descents`]).
    #[must_use]
    pub fn max_descent_slope(&self) -> f64 {
        self.max_descent_angle_deg().to_radians().tan()
    }

    /// G-SIMCORE (P22-0b): the steepest slope (dz / dxy) of a straight
    /// zigzag ramp, `p / (4 c)`: at a reversal a cell at `b < c` behind the
    /// turn point stays under the core for `2 (b + c)` of travel, which
    /// tends to `4 c`. With legs of at least [`Self::min_ramp_leg_mm`] no
    /// piece is longer. Asserted in
    /// `planning/feeds_contract_2026-11-03/phases/p22_0b_core_geometry.py`.
    #[must_use]
    pub fn max_ramp_slope(&self) -> f64 {
        self.protrusion_mm / (4.0 * self.radius_mm)
    }

    /// The angle (degrees) of [`Self::max_ramp_slope`].
    #[must_use]
    pub fn max_ramp_angle_deg(&self) -> f64 {
        self.max_ramp_slope().atan().to_degrees()
    }

    /// The ramp angle (degrees) a zigzag ramp may use: the requested angle,
    /// but never steeper than [`Self::max_ramp_angle_deg`]. A request that
    /// is not a valid angle gets that angle.
    #[must_use]
    pub fn clamp_ramp_angle_deg(&self, requested_deg: f64) -> f64 {
        let limit = self.max_ramp_angle_deg();
        if requested_deg > 0.0 && requested_deg < 90.0 {
            requested_deg.min(limit)
        } else {
            limit
        }
    }

    /// G-SIMCORE (P22-0b): the shortest leg (mm) of a zigzag ramp, the core
    /// diameter. A shorter leg leaves a cell that no blade passes over: a
    /// full-depth post.
    #[must_use]
    pub fn min_ramp_leg_mm(&self) -> f64 {
        self.diameter_mm()
    }

    /// G-SIMCORE (P22-0b): the largest helix pitch (mm per turn) at helix
    /// radius `radius_mm`, for a helix written as `chords_per_turn` straight
    /// chords. The smaller of two bounds:
    ///
    /// - The arc (the dwell rule on a circle): a cell stays under the core
    ///   for `phi = 2 asin(c / r)` of turn (a half turn at `r <= c`), so
    ///   `pitch <= p 2 pi / phi`.
    /// - One chord: a straight move holds the straight-line limit
    ///   ([`Self::max_descent_slope`]), `pitch <= slope x chord x n`.
    ///
    /// A radius under `c` leaves a post at any pitch (`HelixRadius::resolve`
    /// keeps the floor [`Self::radius_mm`]).
    #[must_use]
    pub fn max_helix_pitch_mm(&self, radius_mm: f64, chords_per_turn: usize) -> f64 {
        let ratio = if radius_mm > 0.0 {
            (self.radius_mm / radius_mm).min(1.0)
        } else {
            1.0
        };
        let arc = self.protrusion_mm * std::f64::consts::TAU / (2.0 * ratio.asin());
        let n = chords_per_turn.max(1) as f64;
        let chord = 2.0 * radius_mm.max(0.0) * (std::f64::consts::PI / n).sin();
        arc.min(self.max_descent_slope() * chord * n)
    }

    /// True when a straight plunge of `depth_mm` into stock is allowed.
    #[must_use]
    pub fn allows_plunge(&self, depth_mm: f64) -> bool {
        depth_mm <= self.protrusion_mm + NON_CENTRE_CUTTING_TOL_MM
    }

    /// True when a fed move from `start` to `end` goes down steeper than
    /// the straight-line limit. A vertical descent is steeper. A level move
    /// or a move up is not.
    #[must_use]
    pub fn is_steeper_than_limit(&self, start: P3, end: P3) -> bool {
        let dz = start.z - end.z;
        if dz <= NON_CENTRE_CUTTING_TOL_MM {
            return false;
        }
        let dxy = (end.x - start.x).hypot(end.y - start.y);
        dz > dxy * self.max_descent_slope() * (1.0 + 1e-6) + NON_CENTRE_CUTTING_TOL_MM
    }

    /// The warning of a straight plunge from the material top
    /// `material_top_z` down to `end`, or `None` when the plunge is not
    /// deeper than the blade protrusion. The one plunge check of every
    /// entry builder.
    #[must_use]
    pub fn plunge_refusal(&self, material_top_z: f64, end: P3) -> Option<NonCentreCuttingRefusal> {
        let depth_mm = material_top_z - end.z;
        (!self.allows_plunge(depth_mm)).then_some(NonCentreCuttingRefusal {
            at: end,
            depth_mm,
            max_plunge_depth_mm: self.protrusion_mm,
        })
    }

    /// The dwell rule on a fed path (P22-0c-2): raise each point of `path`
    /// (never lower it) so that no piece of the path with an XY diameter of
    /// at most `2 c` descends more than `p`, and no straight segment is
    /// steeper than [`Self::max_descent_slope`].
    ///
    /// `anchor` is where the tool stands before the path (the first descent
    /// reads from it); `None`: `path[0]`. `path[0]` itself does not move.
    /// The rule reads only this path: the history before `anchor` is the
    /// caller's (an entry, or the cut before it).
    ///
    /// The rule is checked at each vertex, with the window that ends there;
    /// the window start inside a segment is found by bisection. On a
    /// straight descent at the straight-line limit it raises nothing (the
    /// window of `2 c` drops exactly `p`).
    pub fn clamp_descents(&self, path: &mut [P3], anchor: Option<P3>) {
        let slope = self.max_descent_slope();
        let reach = self.diameter_mm();
        for i in 1..path.len() {
            let (Some(prev), Some(cur)) = (vertex(path, anchor, i - 1), vertex(path, anchor, i))
            else {
                continue;
            };
            let dxy = (cur.x - prev.x).hypot(cur.y - prev.y);
            let mut z = cur.z;
            let lowest = prev.z - slope * dxy;
            if z < lowest {
                z = lowest;
            }
            if let Some(top) = window_top(path, anchor, i, reach, z + self.protrusion_mm)
                && top - self.protrusion_mm - z > DWELL_NOISE_MM
            {
                z = top - self.protrusion_mm;
            }
            if z != cur.z
                && let Some(point) = path.get_mut(i)
            {
                point.z = z;
            }
        }
    }

    /// The longest XY arc (mm) of the closed ring `ring` whose XY diameter
    /// is at most `2 c`: the longest dwell of a cell under the core on a lap
    /// of the ring. A lap that descends at a uniform slope holds the dwell
    /// rule when the slope is at most `p / max_dwell_mm`. `None` when the
    /// whole ring fits in `2 c`: the core never leaves a cell in it, so the
    /// ring may not descend at all (the radius rule of a helix).
    #[must_use]
    pub fn max_dwell_mm(&self, ring: &[P3]) -> Option<f64> {
        let n = ring.len();
        if n < 2 {
            return None;
        }
        let reach = self.diameter_mm();
        let at = |k: usize| ring.get(k % n).copied();
        let mut longest = 0.0_f64;
        for i in 0..n {
            let Some(end) = at(i) else { continue };
            let mut window = vec![end];
            let mut arc = 0.0;
            let mut k = i + n;
            let mut closed = true;
            for _ in 1..n {
                let (Some(here), Some(next)) = (at(k), at(k - 1)) else {
                    break;
                };
                let seg = (here.x - next.x).hypot(here.y - next.y);
                if fits(&window, next, reach) {
                    window.push(next);
                    arc += seg;
                    k -= 1;
                    continue;
                }
                arc += seg * boundary_t(&window, here, next, reach);
                closed = false;
                break;
            }
            if closed {
                return None;
            }
            longest = longest.max(arc);
        }
        Some(longest)
    }

    /// The steepest uniform descent slope (dz / dxy) of a lap of the closed
    /// ring `ring` under the dwell rule, `p / max_dwell_mm`; 0 when the ring
    /// fits in the core ([`Self::max_dwell_mm`] is `None`).
    #[must_use]
    pub fn max_ring_slope(&self, ring: &[P3]) -> f64 {
        match self.max_dwell_mm(ring) {
            Some(dwell) if dwell > 0.0 => self.protrusion_mm / dwell,
            Some(_) | None => 0.0,
        }
    }
}

/// Vertex `k` of the path the dwell rule reads: `anchor` in place of
/// `path[0]`.
fn vertex(path: &[P3], anchor: Option<P3>, k: usize) -> Option<P3> {
    if k == 0 {
        anchor.or_else(|| path.first().copied())
    } else {
        path.get(k).copied()
    }
}

/// True when `q` is within `reach` (XY) of every point of `window`.
fn fits(window: &[P3], q: P3, reach: f64) -> bool {
    window.iter().all(|w| (w.x - q.x).hypot(w.y - q.y) <= reach)
}

/// The largest `t` in `[0, 1]` such that the point `from + t (to - from)`
/// fits `window` ([`fits`]); `from` fits and `to` does not. The distance
/// to the farthest window point is convex in `t`, so the set that fits is
/// an interval from 0.
fn boundary_t(window: &[P3], from: P3, to: P3, reach: f64) -> f64 {
    let (mut lo, mut hi) = (0.0_f64, 1.0_f64);
    for _ in 0..DWELL_BISECTION_STEPS {
        let mid = 0.5 * (lo + hi);
        let x = P3::new(
            from.x + mid * (to.x - from.x),
            from.y + mid * (to.y - from.y),
            from.z + mid * (to.z - from.z),
        );
        if fits(window, x, reach) {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    lo
}

/// The highest Z of the dwell window that ends at vertex `i` (the piece of
/// path back from `i` with an XY diameter of at most `reach`), or `None`
/// when the window starts inside the last segment (the straight rule of
/// that segment holds it) or cannot bind (no Z in it reaches `bar`).
///
/// A cheap bound first: every window point is within `reach` of vertex
/// `i`, so the contiguous run of such points (plus the one before it)
/// holds the window. Only when that run reaches `bar` does the exact walk
/// run.
fn window_top(path: &[P3], anchor: Option<P3>, i: usize, reach: f64, bar: f64) -> Option<f64> {
    let end = vertex(path, anchor, i)?;
    let last = vertex(path, anchor, i - 1)?;
    if (last.x - end.x).hypot(last.y - end.y) > reach {
        return None;
    }
    let mut bound = last.z;
    let mut k = i - 1;
    while k > 0 {
        let q = vertex(path, anchor, k - 1)?;
        bound = bound.max(q.z);
        if (q.x - end.x).hypot(q.y - end.y) > reach {
            break;
        }
        k -= 1;
    }
    if bound <= bar {
        return None;
    }
    let mut window = vec![end, last];
    let mut top = last.z;
    let mut k = i - 1;
    while k > 0 {
        let here = vertex(path, anchor, k)?;
        let q = vertex(path, anchor, k - 1)?;
        if fits(&window, q, reach) {
            window.push(q);
            top = top.max(q.z);
            k -= 1;
            continue;
        }
        let t = boundary_t(&window, here, q, reach);
        return Some(top.max(here.z + t * (q.z - here.z)));
    }
    Some(top)
}

#[cfg(test)]
#[expect(
    clippy::unwrap_used,
    reason = "test code: a panic is how a test reports a failure"
)]
mod tests {
    use super::*;
    use crate::tool::{BallEndmill, BullNoseEndmill, FlatEndmill, TaperedBallEndmill, VBitEndmill};

    /// The six-decimal values of the worked table (`P22-0c-6.md` section
    /// 2.4) hold to half a unit in the last place.
    const TABLE_TOL: f64 = 5e-7;

    /// The cone width of a tapered ball at the height `h` (mm), in closed
    /// form: the cone offset is `r (1 - sin a) - r cos a / tan a`.
    fn cone_diameter(tip_d: f64, half_angle_deg: f64, h: f64) -> f64 {
        let r = tip_d / 2.0;
        let a = half_angle_deg.to_radians();
        let offset = r * (1.0 - a.sin()) - r * a.cos() / a.tan();
        2.0 * (h - offset) * a.tan()
    }

    /// P22-0c-6a, ruling 1: the cutting cap is the width at the flute top,
    /// never above the shaft. The worked table of section 2.4: wanaka tool
    /// 4 (tip 2 mm, 5.7106 deg as the operator file states it, shaft 6,
    /// `L` 20) with the legacy shank 6.35.
    #[test]
    fn the_cutting_cap_is_the_width_at_the_flute_top_p22_0c6() {
        let cutter = TaperedBallEndmill::new(2.0, 5.7106, 6.0, 20.0).unwrap();
        let body = ToolBody::new(&cutter, 6.35, 30.0, None);
        let cap = body.max_cut_diameter_mm();
        assert!((cap - 5.809980).abs() < TABLE_TOL, "cap {cap}");
        assert!((cap - cone_diameter(2.0, 5.7106, 20.0)).abs() < 1e-12);
        // (ap, the cutter today: capped at the shaft, after: capped at
        // `width(L)`).
        for (ap, cutter_today, after) in [
            (0.2, 1.200000, 1.200000),
            (10.0, 3.809977, 3.809977),
            (20.0, 5.809980, 5.809980),
            (20.5, 5.909980, 5.809980),
            (21.0, 6.000000, 5.809980),
            (25.0, 6.000000, 5.809980),
        ] {
            let width = 2.0 * cutter.width_at_height(ap);
            assert!((width - cutter_today).abs() < TABLE_TOL, "ap {ap}: {width}");
            let capped = width.min(cap);
            assert!((capped - after).abs() < TABLE_TOL, "ap {ap}: {capped}");
            // One value gives both halves of the ruling: the cap equals
            // the width at `min(ap, L)`.
            assert_eq!(
                capped,
                2.0 * cutter.width_at_height(ap.min(20.0)),
                "ap {ap}"
            );
        }

        // The operator's R2 x 6 x 30.5: the taper from the size,
        // atan((3 - 2) / 30.5). The cone meets the shaft at 32.47 mm,
        // above the flute, so the cap is width(30.5), not the shaft.
        let half_angle = (1.0_f64 / 30.5).atan().to_degrees();
        let cutter = TaperedBallEndmill::new(4.0, half_angle, 6.0, 30.5).unwrap();
        let body = ToolBody::new(&cutter, 6.0, 45.0, None);
        let cap = body.max_cut_diameter_mm();
        assert!((cap - 5.871002).abs() < TABLE_TOL, "cap {cap}");
        assert!((cap - cone_diameter(4.0, half_angle, 30.5)).abs() < 1e-12);
        assert_eq!(2.0 * cutter.width_at_height(33.0), 6.0, "the shaft above");

        // A cone that meets the shaft below `L`: the cap is the shaft.
        let cutter = TaperedBallEndmill::new(1.0, 7.0, 6.0, 30.5).unwrap();
        let body = ToolBody::new(&cutter, 6.0, 45.0, None);
        assert_eq!(body.max_cut_diameter_mm(), 6.0);
    }

    /// The other shapes cut to their diameter when the flute reaches the
    /// widest point; the upper diameter is the shank, else the envelope.
    #[test]
    fn every_shape_caps_at_its_widest_flute_and_reads_one_upper_rule_p22_0c6() {
        let flat = FlatEndmill::new(6.0, 20.0);
        let ball = BallEndmill::new(3.0, 12.0);
        let bull = BullNoseEndmill::new(10.0, 1.0, 25.0).unwrap();
        // 90 deg, D 12: the cone height is 6 mm, below the flute.
        let vbit = VBitEndmill::new(12.0, 90.0, 15.0).unwrap();
        let shapes: [(&dyn MillingCutter, f64); 4] =
            [(&flat, 6.0), (&ball, 3.0), (&bull, 10.0), (&vbit, 12.0)];
        for (cutter, d) in shapes {
            let body = ToolBody::new(cutter, 6.35, 30.0, None);
            assert_eq!(body.max_cut_diameter_mm(), d);
            assert_eq!(body.upper_diameter_mm(), 6.35);
            let bare = ToolBody::new(cutter, 0.0, 30.0, None);
            assert_eq!(bare.upper_diameter_mm(), 2.0 * cutter.envelope_radius_mm());
        }
        assert_eq!(upper_diameter_mm(6.0, 9.0), 6.0);
        assert_eq!(upper_diameter_mm(0.0, 9.0), 9.0);
    }
}
