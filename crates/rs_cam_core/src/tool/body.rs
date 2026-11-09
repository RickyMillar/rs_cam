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
use crate::compute::tool_config::{NonCentreCuttingGeometry, NonCentreCuttingLimits};
use crate::machine::head::{HeadBand, HeadBandKind};

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
        Self {
            flute_length_mm: cutter.length(),
            envelope_radius_mm: cutter.envelope_radius_mm(),
            core,
            shank_diameter_mm,
            stickout_mm,
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
            inner_mm: self.core_radius_mm().unwrap_or(0.0),
            kind: CuttingKind::Flutes,
        })];
        if let Some(core) = self.core {
            zones.push(Zone::Contact(ContactZone {
                span: AxialSpan {
                    bottom_mm: core.blade_protrusion_mm,
                    top: SpanTop::At(length),
                },
                inner_mm: 0.0,
                outer: ContactRadius::Mm(core.centre_core_diameter_mm / 2.0),
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

    /// The core radius (mm), `core_d / 2`; `None` for a centre-cutting tool.
    fn core_radius_mm(&self) -> Option<f64> {
        self.core.map(|core| core.centre_core_diameter_mm / 2.0)
    }

    /// The non-centre-cutting limits of the core
    /// (`NonCentreCuttingGeometry::limits`); `None` for a centre-cutting
    /// tool.
    #[must_use]
    pub fn core_limits(&self) -> Option<NonCentreCuttingLimits> {
        self.core.map(NonCentreCuttingGeometry::limits)
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
        } = self;
        for value in [
            *flute_length_mm,
            *envelope_radius_mm,
            *shank_diameter_mm,
            *stickout_mm,
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
    /// a contact zone from its bottom upward (`SpanTop::Open`).
    #[must_use]
    pub fn zones(&self) -> Vec<Zone> {
        let mut zones = self.body.zones();
        zones.extend(self.head.iter().map(|band| {
            Zone::Contact(ContactZone {
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
            })
        }));
        zones
    }

    /// The old `ToolAssembly` view (P22-0c-1). The collision kernels read
    /// it until P22-0c-5 moves them to the zones.
    #[must_use]
    pub fn assembly(&self) -> crate::stock::collision::ToolAssembly {
        crate::stock::collision::ToolAssembly {
            cutter_radius: self.body.envelope_radius_mm,
            cutter_length: self.body.flute_length_mm,
            shank_diameter: self.body.shank_diameter_mm,
            stickout: self.body.stickout_mm,
            head: self.head.clone(),
        }
    }
}
